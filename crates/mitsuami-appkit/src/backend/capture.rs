//! Capturing a view's pixels.

use mitsuami_core::NodeId;
use mitsuami_core::backend::{CaptureError, Image};
use objc2::Message;
use objc2_app_kit::NSBitmapFormat;

use super::{AppKitBackend, Widget};

impl AppKitBackend {
    pub(super) fn capture_now(&self, id: NodeId) -> Result<Image, CaptureError> {
        let (view, window_host) = {
            let state = self.state.borrow();
            let widget = &state.nodes.get(&id).ok_or(CaptureError::UnknownNode)?.widget;
            let host = match widget {
                Widget::Window { host, .. } => Some(host.retain()),
                _ => None,
            };
            (widget.view().retain(), host)
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
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        if let Some(host) = &window_host {
            host.set_fill(false);
        }
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
        let scale = if bounds.size.width > 0.0 { width as f32 / bounds.size.width as f32 } else { 1.0 };
        Ok(Image { width: width as u32, height: height as u32, scale_factor: scale, rgba })
    }
}
