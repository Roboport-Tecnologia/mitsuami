//! Capturing what GTK has drawn. A popover is a surface of its own, which
//! a capture of its window doesn't hold: each shown over the captured
//! widget is rendered too, and drawn over the capture where it is.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk};
use mitsuami_core::NodeId;
use mitsuami_core::backend::{CaptureError, Image};
use mitsuami_core::services::Reply;

use super::{GtkBackend, Widget};

/// Premultiplied ARGB in native byte order (what `Texture::download` gives)
/// to straight RGBA.
fn to_rgba(argb: &[u8]) -> Vec<u8> {
    argb.chunks_exact(4)
        .flat_map(|p| {
            let [b, g, r, a] = u32::from_ne_bytes([p[0], p[1], p[2], p[3]]).to_le_bytes();
            let straight = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
            [straight(r), straight(g), straight(b), a]
        })
        .collect()
}

/// Renders a widget GTK has laid out, in software: the same pixels
/// whichever GPU renderer the display uses.
pub(super) fn render(widget: &gtk::Widget, size: (i32, i32)) -> Result<Image, CaptureError> {
    let scale = widget.scale_factor();
    let (width, height) = (size.0 * scale, size.1 * scale);
    let snapshot = gtk::Snapshot::new();
    snapshot.scale(scale as f32, scale as f32);
    gtk::WidgetPaintable::new(Some(widget)).snapshot(&snapshot, size.0 as f64, size.1 as f64);
    let node = snapshot.to_node().ok_or_else(|| CaptureError::Failed("nothing was drawn".into()))?;
    let renderer = gsk::CairoRenderer::new();
    renderer.realize(None::<&gdk::Surface>).map_err(|e| CaptureError::Failed(e.to_string()))?;
    let texture = renderer.render_texture(node, Some(&graphene::Rect::new(0.0, 0.0, width as f32, height as f32)));
    renderer.unrealize();
    let (width, height) = (texture.width() as usize, texture.height() as usize);
    let mut bytes = vec![0; width * height * 4];
    texture.download(&mut bytes, width * 4);
    Ok(Image { width: width as u32, height: height as u32, scale_factor: scale as f32, rgba: to_rgba(&bytes) })
}

/// Draws each popover shown over `widget` onto its capture, where it is.
fn with_popovers(mut image: Image, widget: &gtk::Widget, popovers: &[gtk::Popover]) -> Image {
    for popover in popovers {
        let Some((x, y)) = place_over(popover, widget) else { continue };
        let Ok(over) = render(popover.upcast_ref(), (popover.width(), popover.height())) else { continue };
        let scale = image.scale_factor as f64;
        draw_over(&mut image, &over, (x * scale).round() as i64, (y * scale).round() as i64);
    }
    image
}

/// Where a popover is over `widget`, in `widget`'s coordinates, if it
/// shows in `widget`'s window: through its popup's place on the window's
/// surface, as `Popover::frame` finds it.
fn place_over(popover: &gtk::Popover, widget: &gtk::Widget) -> Option<(f64, f64)> {
    if !popover.is_visible() || popover.parent()?.native()? != widget.native()? {
        return None;
    }
    let popup = popover.surface()?.downcast::<gdk::Popup>().ok()?;
    let at = widget.compute_bounds(&widget.native()?)?;
    let (inset_x, inset_y) = popover.surface_transform();
    Some((popup.position_x() as f64 + inset_x - at.x() as f64, popup.position_y() as f64 + inset_y - at.y() as f64))
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

/// How long a capture waits for a window's resize to arrive.
const RESIZE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

impl GtkBackend {
    pub(super) fn capture_node(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        // A window on its way to another size (a sidebar's width changed,
        // say) is captured at that size: GTK gets there at a later frame,
        // and on a display nobody watches frames are far apart.
        let (widget, size, resizing, popovers) = {
            let state = self.state.borrow();
            let Some(node) = state.nodes.get(&id) else { return reply(Err(CaptureError::UnknownNode)) };
            let (size, resizing) = match &node.widget {
                Widget::Window(parts) => {
                    let root = parts.host.window_root().expect("window hosts have a root");
                    (root.resizing.get().unwrap_or(root.size.get()), root.resizing.get().is_some())
                }
                _ => (state.frames.borrow().get(node.widget.widget()).map(|f| f.size).unwrap_or_default(), false),
            };
            let popovers: Vec<gtk::Popover> = state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::Popover(popover) => Some(popover),
                    _ => None,
                })
                .filter(|p| p.shown())
                .map(|p| p.popover.clone())
                .collect();
            (node.widget.widget().clone(), size, resizing, popovers)
        };
        let target = (size.width.round() as i32, size.height.round() as i32);
        let reply = Rc::new(Cell::new(Some(reply)));
        let started = std::time::Instant::now();
        // Tick callbacks run before layout; paint comes after it, in the
        // same frame.
        widget.add_tick_callback(move |widget, clock| {
            if !widget.is_mapped() {
                return glib::ControlFlow::Continue;
            }
            if resizing && (widget.width(), widget.height()) != target && started.elapsed() < RESIZE_WAIT {
                return glib::ControlFlow::Continue;
            }
            let handler: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::default();
            let (reply, widget, slot, popovers) = (reply.clone(), widget.clone(), handler.clone(), popovers.clone());
            handler.set(Some(clock.connect_after_paint(move |clock| {
                if let Some(reply) = reply.take() {
                    reply(if (widget.width(), widget.height()) == target {
                        render(&widget, target).map(|image| with_popovers(image, &widget, &popovers))
                    } else {
                        Err(CaptureError::Failed(format!(
                            "the widget is {}×{}, not {}×{}",
                            widget.width(),
                            widget.height(),
                            target.0,
                            target.1
                        )))
                    });
                }
                if let Some(handler) = slot.take() {
                    clock.disconnect(handler);
                }
            })));
            glib::ControlFlow::Break
        });
    }
}
