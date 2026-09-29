//! Sliders: steps, marks and snapping.

use std::cell::Cell;

use gtk::prelude::*;

/// GTK's scales move by their step from the keyboard. Without a step they
/// need one anyway: a tenth of the range, with pages of ten steps as
/// `gtk::Scale::with_range` makes them.
pub(super) fn set_increments(scale: &gtk::Scale, step: Option<f64>) {
    let adjustment = scale.adjustment();
    let step = step.unwrap_or((adjustment.upper() - adjustment.lower()) / 10.0);
    scale.set_increments(step, step * 10.0);
}

/// A slider's step, whether a mark shows at each one, and the length its
/// knob travels, which decides whether the marks fit.
#[derive(Default)]
pub(crate) struct Steps {
    pub(super) step: Cell<Option<f64>>,
    pub(crate) hidden: Cell<bool>,
    length: Cell<f64>,
    /// The marks on the scale: range and step, if any.
    shown: Cell<Option<(f64, f64, f64)>>,
}

/// Where a scale keeps its [`Steps`].
pub(crate) const STEPS: &str = "mitsuami-steps";

/// How far from its mark GTK holds the knob while dragging
/// (`MARK_SNAP_LENGTH` in `gtkrange.c`). With steps closer than twice that,
/// it holds the knob past the next step, which then jumps several at once.
const MARK_HOLD: f64 = 12.0;

/// A mark at every step, as AppKit draws its tick marks, unless the app
/// turned them off or the steps are too close for GTK's drag.
pub(crate) fn update_marks(scale: &gtk::Scale, steps: &Steps) {
    let adjustment = scale.adjustment();
    let (min, max) = (adjustment.lower(), adjustment.upper());
    let wanted = steps
        .step
        .get()
        .filter(|step| *step > 0.0 && !steps.hidden.get())
        .filter(|step| steps.length.get() * step / (max - min) >= 2.0 * MARK_HOLD)
        .map(|step| (min, max, step));
    if steps.shown.replace(wanted) == wanted {
        return;
    }
    scale.clear_marks();
    if let Some((min, max, step)) = wanted {
        for i in 0..=((max - min) / step + 1e-9).floor() as u32 {
            scale.add_mark(min + i as f64 * step, gtk::PositionType::Bottom, None);
        }
    }
}

/// Sets the length a slider's knob travels in a frame of `length` along it:
/// the frame less the scale's padding, which the knob overhangs.
pub(super) fn set_travel(scale: &gtk::Scale, steps: &Steps, length: f32) {
    #[allow(deprecated)] // The only way to read a widget's CSS padding.
    let padding = scale.style_context().padding();
    let padding = match scale.orientation() {
        gtk::Orientation::Vertical => padding.top() + padding.bottom(),
        _ => padding.left() + padding.right(),
    };
    steps.length.set((length as f64 - padding as f64).max(0.0));
    update_marks(scale, steps);
}

/// The step nearest `value`, counting from the minimum, within the range.
pub(super) fn snap(adjustment: &gtk::Adjustment, value: f64, step: f64) -> f64 {
    let min = adjustment.lower();
    (min + ((value - min) / step).round() * step).clamp(min, adjustment.upper())
}
