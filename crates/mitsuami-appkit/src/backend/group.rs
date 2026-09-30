//! Groups: the `NSBox` behind a group's children, and where it puts them.

use mitsuami_core::Size;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBox, NSBoxType, NSTitlePosition};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::ns;

/// An `NSBox` as Interface Builder makes one: the primary style, its
/// title (if any) at the top.
pub(super) fn group_box(mtm: MainThreadMarker, size: NSSize) -> Retained<NSBox> {
    // Made at its size: a box made empty and grown keeps its content
    // view's first frame.
    let frame = NSBox::initWithFrame(NSBox::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), size));
    frame.setBoxType(NSBoxType::Primary);
    frame.setTitlePosition(NSTitlePosition::NoTitle);
    frame
}

pub(super) fn set_group_title(frame: &NSBox, title: &str) {
    frame.setTitle(&ns(title));
    frame.setTitlePosition(if title.is_empty() { NSTitlePosition::NoTitle } else { NSTitlePosition::AtTop });
    // The label that draws it keeps its size until the box is laid out
    // again: a longer title was cut at the first one's width.
    frame.setNeedsLayout(true);
    frame.layoutSubtreeIfNeeded();
}

/// Where a box puts its content: its content view's place, with or
/// without a title. A box isn't flipped: y grows up.
pub(super) fn group_insets(mtm: MainThreadMarker, title: &str) -> mitsuami_core::Insets {
    group_probe(mtm, title).0
}

/// A box with this title, big enough for it: where it puts its content,
/// and how wide its title needs it to be (the title inset from both
/// edges as from the leading one).
fn group_probe(mtm: MainThreadMarker, title: &str) -> (mitsuami_core::Insets, f32) {
    let frame = group_box(mtm, NSSize::new(10000.0, 300.0));
    set_group_title(&frame, title);
    probe_insets(&frame)
}

/// A group's own box, as its tweaks left it: a probe set up as it is
/// (where the title goes, its font, the border and margins), since the box
/// itself may be too small to say.
pub(super) fn group_probe_like(shown: &NSBox) -> (mitsuami_core::Insets, f32) {
    let frame = group_box(MainThreadMarker::from(shown), NSSize::new(10000.0, 300.0));
    frame.setBoxType(shown.boxType());
    frame.setTitle(&shown.title());
    frame.setTitleFont(&shown.titleFont());
    frame.setTitlePosition(shown.titlePosition());
    frame.setBorderWidth(shown.borderWidth());
    frame.setContentViewMargins(shown.contentViewMargins());
    probe_insets(&frame)
}

fn probe_insets(frame: &NSBox) -> (mitsuami_core::Insets, f32) {
    let heading = frame.titleRect();
    let titled = frame.titlePosition() != NSTitlePosition::NoTitle && !frame.title().is_empty();
    let heading = if titled { (heading.origin.x * 2.0 + heading.size.width) as f32 } else { 0.0 };
    let (bounds, content) = (frame.bounds(), frame.contentView().map_or(frame.bounds(), |v| v.frame()));
    let insets = mitsuami_core::Insets::new(
        (bounds.size.height - content.origin.y - content.size.height) as f32,
        (bounds.size.width - content.origin.x - content.size.width) as f32,
        content.origin.y as f32,
        content.origin.x as f32,
    );
    (insets, heading)
}

/// A box's size with nothing in it: its border, and as wide as its title
/// needs.
pub(super) fn group_natural_size(frame: &NSBox) -> Size {
    let (insets, heading) = group_probe_like(frame);
    Size::new(heading.max(insets.left + insets.right).ceil(), insets.top + insets.bottom)
}
