//! Capturing a view's pixels.

use std::cell::Cell;

use mitsuami_core::NodeId;
use mitsuami_core::backend::{CaptureError, Image};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, NSObjectProtocol};
use objc2::{Message, msg_send, sel};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSBitmapFormat, NSBitmapImageRep, NSGraphicsContext, NSScroller,
    NSScrollerPart, NSScrollerStyle, NSUsableScrollerParts, NSView,
};
use objc2_foundation::NSAffineTransform;

use super::{AppKitBackend, Widget};

thread_local! {
    static CAPTURING: Cell<bool> = const { Cell::new(false) };
}

/// Whether a capture is being drawn, which windows draw as the key one.
pub(crate) fn capturing() -> bool {
    CAPTURING.get()
}

impl AppKitBackend {
    pub(super) fn capture_now(&self, id: NodeId) -> Result<Image, CaptureError> {
        let (view, window_host, headers) = {
            let state = self.state.borrow();
            let widget = &state.nodes.get(&id).ok_or(CaptureError::UnknownNode)?.widget;
            let host = match widget {
                Widget::Window { host, .. } => Some(host.retain()),
                _ => None,
            };
            let headers: Vec<(Retained<NSView>, Retained<NSView>)> = state
                .nodes
                .values()
                .filter_map(|n| {
                    let header = n.header.as_ref().filter(|h| h.window.get().is_some())?;
                    Some((n.widget.view().retain(), Retained::into_super(header.content.clone())))
                })
                .collect();
            (widget.view().retain(), host, headers)
        };
        // Views that lay out their own subviews (a table's rows) do it in
        // a layout pass, which offscreen windows only get when asked.
        view.layoutSubtreeIfNeeded();
        let bounds = view.bounds();
        let rep = view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .ok_or_else(|| CaptureError::Failed("no bitmap for view".into()))?;
        // The window's background isn't in the capture, so its host paints
        // one for it.
        if let Some(host) = &window_host {
            host.set_fill(true);
        }
        // A scroll view under the toolbar has an edge effect (the bar's
        // blur, a private NSScrollPocket) that draws nothing offscreen and
        // blanks the whole capture, so it's left out.
        let pockets = scroll_pockets(&view);
        for pocket in &pockets {
            pocket.setHidden(true);
        }
        // A header in its window's title bar is outside the host, and the
        // title bar draws it dimmed offscreen: it's drawn in its
        // placeholder, as the content would have it, and put back.
        let moved: Vec<_> = headers
            .iter()
            .filter(|(placeholder, _)| placeholder.isDescendantOf(&view))
            .filter_map(|(placeholder, content)| {
                let (superview, frame) = (unsafe { content.superview() }?, content.frame());
                content.removeFromSuperview();
                content.setFrame(placeholder.bounds());
                placeholder.addSubview(content);
                Some((content, superview, frame))
            })
            .collect();
        // Drawn as the active window, as the user sees the one they work
        // in (see `NodeWindow`): its views take the key state again, as
        // AppKit has them do when a window becomes key, and after.
        CAPTURING.set(true);
        key_state_changed(&view);
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        draw_scrollers(&view, &rep);
        CAPTURING.set(false);
        key_state_changed(&view);
        for (content, superview, frame) in moved {
            content.removeFromSuperview();
            content.setFrame(frame);
            superview.addSubview(content);
        }
        for pocket in &pockets {
            pocket.setHidden(false);
        }
        if let Some(host) = &window_host {
            host.set_fill(false);
        }
        let (width, height, rgba) = pixels(&rep)?;
        let scale = if bounds.size.width > 0.0 { width as f32 / bounds.size.width as f32 } else { 1.0 };
        Ok(Image { width: width as u32, height: height as u32, scale_factor: scale, rgba })
    }
}

/// Draws the legacy scroll bars in `view` into its capture, where they
/// are: `cacheDisplayInRect:` leaves them out (macOS 26 draws a scroller
/// on its layer), and a capture showed the room they take, empty. Drawn
/// by the scroller itself, its slot and its knob. Overlay scroll bars
/// show only while scrolling, as captures don't.
fn draw_scrollers(view: &NSView, rep: &NSBitmapImageRep) {
    fn find(view: &NSView, out: &mut Vec<Retained<NSScroller>>) {
        for subview in view.subviews().iter() {
            match subview.downcast::<NSScroller>() {
                Ok(scroller) => out.push(scroller),
                Err(subview) => find(&subview, out),
            }
        }
    }
    let mut scrollers = Vec::new();
    find(view, &mut scrollers);
    scrollers.retain(|s| {
        s.scrollerStyle() == NSScrollerStyle::Legacy
            && !s.isHiddenOrHasHiddenAncestor()
            && s.alphaValue() > 0.0
            && s.frame().size.width > 0.0
            && s.frame().size.height > 0.0
    });
    let Some(context) =
        (!scrollers.is_empty()).then(|| NSGraphicsContext::graphicsContextWithBitmapImageRep(rep)).flatten()
    else {
        return;
    };
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    let height = view.bounds().size.height;
    for scroller in scrollers {
        // Where it is in the capture, whose origin is at the bottom.
        let rect = scroller.convertRect_toView(scroller.bounds(), Some(view));
        let y = if view.isFlipped() { height - rect.origin.y - rect.size.height } else { rect.origin.y };
        context.saveGraphicsState();
        let transform = NSAffineTransform::transform();
        transform.translateXBy_yBy(rect.origin.x, y);
        if scroller.isFlipped() {
            transform.translateXBy_yBy(0.0, rect.size.height);
            transform.scaleXBy_yBy(1.0, -1.0);
        }
        transform.concat();
        scroller.drawKnobSlotInRect_highlight(scroller.rectForPart(NSScrollerPart::KnobSlot), false);
        // With nothing to scroll, a legacy scroll bar is its empty slot.
        if scroller.usableParts() == NSUsableScrollerParts::AllScrollerParts {
            scroller.drawKnob();
        }
        context.restoreGraphicsState();
    }
    NSGraphicsContext::restoreGraphicsState_class();
}

/// Tells `view` and the views in it that their window's key state changed,
/// as AppKit does when a window becomes or stops being key: controls keep
/// the state they draw (a default button's accent), and ask for it again
/// only then. AppKit's own message; a view without it is left as it is.
fn key_state_changed(view: &NSView) {
    if view.respondsToSelector(sel!(_windowChangedKeyState)) {
        let _: () = unsafe { msg_send![view, _windowChangedKeyState] };
    }
    for subview in view.subviews().iter() {
        key_state_changed(&subview);
    }
}

/// A bitmap's pixels, as RGBA: its width, height and bytes.
fn pixels(rep: &NSBitmapImageRep) -> Result<(usize, usize, Vec<u8>), CaptureError> {
    let (width, height) = (rep.pixelsWide() as usize, rep.pixelsHigh() as usize);
    let (samples, bits, row) = (rep.samplesPerPixel() as usize, rep.bitsPerSample(), rep.bytesPerRow() as usize);
    if bits != 8 || !(samples == 3 || samples == 4) {
        return Err(CaptureError::Failed(format!("unsupported bitmap: {samples} samples × {bits} bits")));
    }
    let alpha_first = rep.bitmapFormat().contains(NSBitmapFormat::AlphaFirst);
    let data = rep.bitmapData();
    if data.is_null() {
        return Err(CaptureError::Failed("bitmap has no data".into()));
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, row * height) };
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let p = &bytes[y * row + x * samples..][..samples];
            let [r, g, b, a] = match (samples, alpha_first) {
                (4, true) => [p[1], p[2], p[3], p[0]],
                (4, false) => [p[0], p[1], p[2], p[3]],
                _ => [p[0], p[1], p[2], 255],
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    Ok((width, height, rgba))
}

/// The shown scroll edge effects in this view.
fn scroll_pockets(view: &NSView) -> Vec<Retained<NSView>> {
    fn find(view: &NSView, class: &AnyClass, pockets: &mut Vec<Retained<NSView>>) {
        for subview in view.subviews().iter() {
            if subview.isKindOfClass(class) {
                if !subview.isHidden() {
                    pockets.push(subview.retain());
                }
            } else {
                find(&subview, class, pockets);
            }
        }
    }
    let mut pockets = Vec::new();
    // Before macOS 26 there's none.
    if let Some(class) = AnyClass::get(c"NSScrollPocket") {
        find(view, class, &mut pockets);
    }
    pockets
}
