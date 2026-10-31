//! `Backend::capture`: `RenderTargetBitmap`, which renders asynchronously.
//! It doesn't render popups (a popover's flyout): each shown popover's
//! popup is rendered on its own and drawn over the capture where it is.

use mitsuami_core::backend::{CaptureError, Image};
use mitsuami_core::services::Reply;
use windows_core::Interface;

use crate::bindings as w;
use crate::later;

type CaptureReply = Reply<Result<Image, CaptureError>>;

struct Pending {
    bitmap: w::RenderTargetBitmap,
    /// Physical pixels per logical unit.
    scale: f64,
    /// An opaque colour it's drawn on: a window's backdrop's.
    under: Option<w::Color>,
    reply: CaptureReply,
}

fn failed(error: windows_core::Error) -> CaptureError {
    CaptureError::Failed(error.to_string())
}

/// Captures `element`, with the popovers shown over it (their content,
/// `popovers`) drawn in. One that can't be is left out, and said so: the
/// capture would look as if it weren't open.
pub(crate) fn capture_with_popups(
    element: w::UIElement,
    under: Option<w::Color>,
    popovers: &[w::UIElement],
    reply: CaptureReply,
) {
    let popups = popups_of(&element, popovers);
    capture(
        element,
        under,
        Box::new(move |captured| match captured {
            Ok(image) => draw_popups(image, popups, reply),
            Err(error) => reply(Err(error)),
        }),
    );
}

/// The top of each popover's popup (its flyout's presenter, with its
/// border), from its content up, and where it is over `element`, in
/// logical units. XAML's list of a window's open popups
/// (`GetOpenPopupsForXamlRoot`) came back empty in tests with a flyout
/// open.
fn popups_of(element: &w::UIElement, popovers: &[w::UIElement]) -> Vec<(w::UIElement, f64, f64)> {
    let mut popups = Vec::new();
    for content in popovers {
        let mut top: w::DependencyObject = match content.cast() {
            Ok(top) => top,
            Err(e) => {
                eprintln!("mitsuami-winui: a capture can't read a popover: {e}");
                continue;
            }
        };
        // Up to the popup, not past it into the window's tree, whichever
        // way XAML links them.
        while let Ok(parent) = w::VisualTreeHelper::GetParent(&top) {
            let class = parent.cast::<windows_core::IInspectable>().and_then(|p| p.GetRuntimeClassName());
            if class.is_ok_and(|c| {
                let c = c.to_string_lossy();
                c.ends_with(".Popup") || c.ends_with("PopupRoot")
            }) {
                break;
            }
            top = parent;
        }
        let child: w::UIElement = match top.cast() {
            Ok(child) => child,
            Err(e) => {
                eprintln!("mitsuami-winui: a capture can't read a popover's popup: {e}");
                continue;
            }
        };
        let at = match child
            .cast::<w::IUIElement>()
            .and_then(|c| c.TransformToVisual(element))
            .and_then(|t| t.TransformPoint(w::Point { x: 0.0, y: 0.0 }))
        {
            Ok(at) => at,
            Err(e) => {
                eprintln!("mitsuami-winui: a capture can't place an open popup: {e}");
                continue;
            }
        };
        popups.push((child, at.x as f64, at.y as f64));
    }
    popups
}

/// Captures each popup's content in turn and draws it over `image`; one
/// that can't be captured is left out.
fn draw_popups(mut image: Image, mut popups: Vec<(w::UIElement, f64, f64)>, reply: CaptureReply) {
    let Some((child, x, y)) = popups.pop() else { return reply(Ok(image)) };
    capture(
        child,
        None,
        Box::new(move |captured| {
            match captured {
                Ok(over) => {
                    let scale = image.scale_factor as f64;
                    draw_over(&mut image, &over, (x * scale).round() as i64, (y * scale).round() as i64);
                }
                Err(e) => eprintln!("mitsuami-winui: a capture can't render an open popup: {e:?}"),
            }
            draw_popups(image, popups, reply);
        }),
    );
}

/// Draws `over`, straight RGBA, onto `image` with its top left at (x, y)
/// in physical pixels, clipped to `image`.
fn draw_over(image: &mut Image, over: &Image, x: i64, y: i64) {
    for row in 0..over.height as i64 {
        let ty = y + row;
        if ty < 0 || ty >= image.height as i64 {
            continue;
        }
        for col in 0..over.width as i64 {
            let tx = x + col;
            if tx < 0 || tx >= image.width as i64 {
                continue;
            }
            let src = &over.rgba[((row * over.width as i64 + col) * 4) as usize..][..4];
            let i = ((ty * image.width as i64 + tx) * 4) as usize;
            let dst = &mut image.rgba[i..i + 4];
            let a = src[3] as u32;
            let under = dst[3] as u32 * (255 - a) / 255;
            let out = a + under;
            for c in 0..3 {
                dst[c] = (src[c] as u32 * a + dst[c] as u32 * under).checked_div(out).unwrap_or(0) as u8;
            }
            dst[3] = out as u8;
        }
    }
}

pub(crate) fn capture(element: w::UIElement, under: Option<w::Color>, reply: CaptureReply) {
    let started = (|| {
        let bitmap = w::RenderTargetBitmap::new()?;
        let render = bitmap.cast::<w::IRenderTargetBitmap>()?.RenderAsync(&element)?;
        let scale = element.cast::<w::IUIElement>()?.XamlRoot()?.RasterizationScale()?;
        Ok((bitmap, render, scale))
    })();
    let (bitmap, render, scale) = match started {
        Ok(started) => started,
        Err(error) => return reply(Err(failed(error))),
    };
    // XAML completes renders on the UI thread, where the tickets are taken.
    let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else {
        return reply(Err(CaptureError::Failed("no dispatcher queue on this thread".into())));
    };
    let ticket = later::park_until(&queue, Pending { bitmap, scale, under, reply });
    let id = ticket.id();
    let watched = render.when(move |rendered| {
        let Some(pending) = ticket.take::<Pending>() else { return };
        let pixels = rendered.and_then(|()| pending.bitmap.cast::<w::IRenderTargetBitmap>()?.GetPixelsAsync());
        match pixels {
            Ok(pixels) => {
                let ticket = later::park_until(&queue, pending);
                let id = ticket.id();
                let watched = pixels.when(move |buffer| {
                    if let Some(pending) = ticket.take::<Pending>() {
                        let image = buffer.and_then(|buffer| image(&pending, &buffer)).map_err(failed);
                        (pending.reply)(image);
                    }
                });
                if let Err(error) = watched
                    && let Some(pending) = later::take::<Pending>(id)
                {
                    (pending.reply)(Err(failed(error)));
                }
            }
            Err(error) => (pending.reply)(Err(failed(error))),
        }
    });
    if let Err(error) = watched
        && let Some(pending) = later::take::<Pending>(id)
    {
        (pending.reply)(Err(failed(error)));
    }
}

/// Premultiplied BGRA rows → straight RGBA, or opaque on `under`.
fn image(pending: &Pending, buffer: &w::IBuffer) -> windows_core::Result<Image> {
    let bitmap: w::IRenderTargetBitmap = pending.bitmap.cast()?;
    let (width, height) = (bitmap.PixelWidth()? as u32, bitmap.PixelHeight()? as u32);
    let mut rgba = vec![0; buffer.Length()? as usize];
    w::DataReader::FromBuffer(buffer)?.ReadBytes(&mut rgba)?;
    for px in rgba.chunks_exact_mut(4) {
        let [b, g, r, a] = [px[0], px[1], px[2], px[3]];
        if let Some(under) = pending.under {
            // Premultiplied: the colour over it is added as it is.
            let over = |c: u8, u: u8| c.saturating_add(((u as u32 * (255 - a as u32) + 127) / 255) as u8);
            px.copy_from_slice(&[over(r, under.r), over(g, under.g), over(b, under.b), 255]);
            continue;
        }
        let straight = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        px.copy_from_slice(&[straight(r), straight(g), straight(b), a]);
    }
    Ok(Image { width, height, scale_factor: pending.scale as f32, rgba })
}
