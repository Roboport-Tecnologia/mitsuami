//! `Backend::capture`: `RenderTargetBitmap`, which renders asynchronously.

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
    reply: CaptureReply,
}

fn failed(error: windows_core::Error) -> CaptureError {
    CaptureError::Failed(error.to_string())
}

pub(crate) fn capture(element: w::UIElement, reply: CaptureReply) {
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
    let ticket = later::park_until(&queue, Pending { bitmap, scale, reply });
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

/// Premultiplied BGRA rows → straight RGBA.
fn image(pending: &Pending, buffer: &w::IBuffer) -> windows_core::Result<Image> {
    let bitmap: w::IRenderTargetBitmap = pending.bitmap.cast()?;
    let (width, height) = (bitmap.PixelWidth()? as u32, bitmap.PixelHeight()? as u32);
    let mut rgba = vec![0; buffer.Length()? as usize];
    w::DataReader::FromBuffer(buffer)?.ReadBytes(&mut rgba)?;
    for px in rgba.chunks_exact_mut(4) {
        let [b, g, r, a] = [px[0], px[1], px[2], px[3]];
        let straight = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        px.copy_from_slice(&[straight(r), straight(g), straight(b), a]);
    }
    Ok(Image { width, height, scale_factor: pending.scale as f32, rgba })
}
