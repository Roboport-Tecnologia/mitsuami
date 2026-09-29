//! Fixed metrics, and the sizes of the platform's own chrome.

use mitsuami_core::backend::{FontSizes, PlatformMetrics};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{Insets, Size};

/// A window's toolbar: this high, above its content, with its items this
/// far apart and from its trailing edge.
pub(super) const TOOLBAR_HEIGHT: f32 = 40.0;
pub(super) const TOOLBAR_SPACING: f32 = 8.0;

/// A window's sidebar: this wide, on the leading side of its content, as
/// high as it.
pub(super) const SIDEBAR_WIDTH: f32 = 200.0;

/// A tab view: its pages this far in from its edges, below its tab strip,
/// and each tab this much wider than its title.
pub(super) const TAB_INSETS: Insets = Insets::new(32.0, 8.0, 8.0, 8.0);
pub(super) const TAB_PADDING: f32 = 24.0;
/// A group's border and margins, and room for a heading at the top.
pub(super) const GROUP_INSETS: Insets = Insets::new(8.0, 8.0, 8.0, 8.0);
pub(super) const TITLED_GROUP_INSETS: Insets = Insets::new(32.0, 8.0, 8.0, 8.0);
/// Between a radio group's buttons.
pub(super) const RADIO_GAP: f32 = 6.0;

/// The screen a window in full screen fills.
pub(super) const SCREEN: Size = Size::new(1280.0, 800.0);
/// What a maximized window's content fills: the screen less a bar and the
/// window's title bar.
pub(super) const WORK_AREA: Size = Size::new(1280.0, 740.0);

/// Fixed metrics: 16px body text, 4/8/12/16/24 spacing, scale factor 1.
pub fn metrics() -> PlatformMetrics {
    PlatformMetrics {
        scale_factor: 1.0,
        spacing: SpacingScale { xs: 4.0, sm: 8.0, md: 12.0, lg: 16.0, xl: 24.0 },
        font_sizes: FontSizes {
            large_title: 32.0,
            title: 24.0,
            headline: 18.0,
            body: 16.0,
            callout: 15.0,
            caption: 12.0,
            monospace: 14.0,
        },
        dark_mode: false,
        high_contrast: false,
        reduced_motion: false,
        tab_insets: TAB_INSETS,
        group_insets: GROUP_INSETS,
        titled_group_insets: TITLED_GROUP_INSETS,
    }
}
