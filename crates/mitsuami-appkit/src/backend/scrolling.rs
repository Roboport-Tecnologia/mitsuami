//! Scroll views: scrolling as the user does, and their scrollers.

use mitsuami_core::ScrollAxes;
use objc2_app_kit::NSScrollView;
use objc2_foundation::NSPoint;

/// Scrolls like the user would, so the clip view reports the change.
pub(super) fn scroll_to(scroll: &NSScrollView, origin: NSPoint) {
    let clip = scroll.contentView();
    clip.scrollToPoint(crate::classes::clip_origin(&clip, origin));
    scroll.reflectScrolledClipView(&clip);
}

/// Scrollers for the axes that scroll, if the scroll view shows any.
pub(super) fn set_scrollers(scroll: &NSScrollView, axes: ScrollAxes, show: bool) {
    scroll.setHasVerticalScroller(show && axes.vertical());
    scroll.setHasHorizontalScroller(show && axes.horizontal());
}
