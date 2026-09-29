//! Capturing what GTK has drawn.

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

impl GtkBackend {
    pub(super) fn capture_node(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        let (widget, size) = {
            let state = self.state.borrow();
            let Some(node) = state.nodes.get(&id) else { return reply(Err(CaptureError::UnknownNode)) };
            let size = match &node.widget {
                Widget::Window(parts) => parts.host.window_root().expect("window hosts have a root").size.get(),
                _ => state.frames.borrow().get(node.widget.widget()).map(|f| f.size).unwrap_or_default(),
            };
            (node.widget.widget().clone(), size)
        };
        let target = (size.width.round() as i32, size.height.round() as i32);
        let reply = Rc::new(Cell::new(Some(reply)));
        // Tick callbacks run before layout; paint comes after it, in the
        // same frame.
        widget.add_tick_callback(move |widget, clock| {
            if !widget.is_mapped() {
                return glib::ControlFlow::Continue;
            }
            let handler: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::default();
            let (reply, widget, slot) = (reply.clone(), widget.clone(), handler.clone());
            handler.set(Some(clock.connect_after_paint(move |clock| {
                if let Some(reply) = reply.take() {
                    reply(if (widget.width(), widget.height()) == target {
                        render(&widget, target)
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
