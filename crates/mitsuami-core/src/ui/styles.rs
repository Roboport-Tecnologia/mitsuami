//! Resolving styles into Taffy's, with inherited font sizes and direction.

use crate::geometry::Size;
use crate::style::{Align, Display, FlexDirection, GridPlacement, TextDirection, Track};
use crate::units::{Length, ResolveContext, Resolved};
use crate::widget::{HorizontalAlign, LayoutDirection, NodeId, Prop, TextAlign, WidgetKind};

use super::{Fit, Inner};

impl Inner {
    pub(super) fn resolve_styles(&mut self) {
        self.styles_dirty = false;
        let body = self.metrics.font_sizes.body;
        for window in self.windows.clone() {
            let viewport = self.nodes[&window].window_size;
            let rtl = self.rtl;
            self.resolve_node(window, body, rtl, viewport, false);
        }
    }

    /// `in_stretching_column`: the layout parent is a flex column that
    /// stretches its children across (the default).
    fn resolve_node(
        &mut self,
        id: NodeId,
        inherited_font: f32,
        inherited_rtl: bool,
        viewport: Size,
        in_stretching_column: bool,
    ) {
        let node = &self.nodes[&id];
        let font_size = match crate::find_prop!(node.props, TextStyle) {
            Some(style) => self.metrics.font_sizes.get(style),
            None => inherited_font,
        };
        let rtl = match node.style.direction {
            TextDirection::Inherit => inherited_rtl,
            TextDirection::Ltr => false,
            TextDirection::Rtl => true,
        };
        // A split's panes' least sizes, sent once the node is let go of.
        let mut minimums = None;
        if let Some(t) = node.taffy {
            let cx = ResolveContext {
                font_size,
                root_font_size: self.metrics.font_sizes.body,
                viewport,
                spacing: &self.metrics.spacing,
            };
            let mut style = match node.kind {
                // Its pages share one cell, which fills it.
                WidgetKind::Tabs => {
                    let mut own = node.style.clone();
                    own.display = Display::Grid;
                    own.grid_template_columns = vec![Track::Size(Length::Fr(1.0))];
                    own.grid_template_rows = vec![Track::Size(Length::Fr(1.0))];
                    own.to_taffy(&cx, rtl)
                }
                _ if self.is_page(id) => {
                    let mut own = node.style.clone();
                    own.grid_column = GridPlacement::at(1);
                    own.grid_row = GridPlacement::at(1);
                    own.to_taffy(&cx, rtl)
                }
                // The first pane, the divider, then the second pane.
                WidgetKind::Split => {
                    let (tracks, least) = self.split_tracks(id, &cx);
                    minimums = Some(least);
                    let mut own = node.style.clone();
                    own.display = Display::Grid;
                    let vertical = crate::find_prop!(node.props, Orientation).is_some_and(|o| o.vertical());
                    let (along, across) = match vertical {
                        false => (&mut own.grid_template_columns, &mut own.grid_template_rows),
                        true => (&mut own.grid_template_rows, &mut own.grid_template_columns),
                    };
                    *along = tracks;
                    *across = vec![Track::Size(Length::Fr(1.0))];
                    // A split can be another's pane.
                    if let Some((index, vertical)) = self.pane(id) {
                        place_pane(&mut own, index, vertical);
                    }
                    own.to_taffy(&cx, rtl)
                }
                _ if let Some((index, vertical)) = self.pane(id) => {
                    let mut own = node.style.clone();
                    place_pane(&mut own, index, vertical);
                    own.to_taffy(&cx, rtl)
                }
                _ => node.style.to_taffy(&cx, rtl),
            };
            // Its pages are inside its tab strip and border, and it's at
            // least as big as they are.
            if node.kind == WidgetKind::Tabs {
                let insets = node.insets.unwrap_or(self.metrics.tab_insets);
                style.padding = taffy::Rect {
                    left: taffy::LengthPercentage::length(insets.left),
                    right: taffy::LengthPercentage::length(insets.right),
                    top: taffy::LengthPercentage::length(insets.top),
                    bottom: taffy::LengthPercentage::length(insets.bottom),
                };
                tab_strip_minimum(&mut style, node.strip);
            }
            // Its content is inside its border and heading, past any
            // padding of the app's, and it's at least as wide as its heading.
            if node.kind == WidgetKind::Group {
                let titled = crate::find_prop!(node.props, Title).is_some_and(|t| !t.is_empty());
                let insets = node.insets.unwrap_or(if titled {
                    self.metrics.titled_group_insets
                } else {
                    self.metrics.group_insets
                });
                add(&mut style.padding.left, insets.left);
                add(&mut style.padding.right, insets.right);
                add(&mut style.padding.top, insets.top);
                add(&mut style.padding.bottom, insets.bottom);
                tab_strip_minimum(&mut style, node.strip);
            }
            // Its content is laid out in the viewport the platform leaves
            // inside its scroll bars. Content frames are relative to the
            // viewport, so that room comes off the far edges, whichever
            // side the bars are on.
            if node.kind == WidgetKind::ScrollView {
                let insets = node.viewport_insets;
                add(&mut style.padding.right, insets.left + insets.right);
                add(&mut style.padding.bottom, insets.top + insets.bottom);
            }
            // Toggles have a fixed natural size, like CSS replaced elements:
            // stretched, some platforms draw them centered in the extra
            // space (`NSSwitch`) and all of them take clicks there.
            if matches!(node.kind, WidgetKind::Checkbox | WidgetKind::Switch) {
                style.justify_self.get_or_insert(taffy::AlignSelf::START);
                if in_stretching_column {
                    style.align_self.get_or_insert(taffy::AlignSelf::START);
                }
            }
            // Text cut off at a line limit shrinks to fit, as a label that
            // ellipsizes does on every platform. Flex's automatic minimum
            // would be its longest word, a whole path say.
            if node.kind == WidgetKind::Text
                && crate::find_prop!(node.props, MaxLines).flatten().is_some()
                && style.min_size.width == taffy::LengthPercentageAuto::auto()
            {
                style.min_size.width = taffy::LengthPercentageAuto::length(0.0);
            }
            if node.kind == WidgetKind::Window {
                style.size = taffy::Size {
                    width: taffy::Dimension::length(node.window_size.width),
                    height: if node.fit == Fit::Once {
                        taffy::Dimension::auto()
                    } else {
                        taffy::Dimension::length(node.window_size.height)
                    },
                };
            }
            if self.taffy.style(t).ok() != Some(&style) {
                let _ = self.taffy.set_style(t, style);
            }
        }
        if node.kind == WidgetKind::Text {
            self.resolve_text_align(id, rtl);
        }
        if let Some(prop) = minimums
            && self.nodes[&id].prop(&prop) != Some(&prop)
        {
            let node = self.nodes.get_mut(&id).unwrap();
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            self.queue_prop(id, prop);
        }
        self.resolve_direction(id, rtl);
        let node = &self.nodes[&id];
        // Nodes without a layout box pass their parent's context through.
        let in_stretching_column = match node.taffy {
            Some(_) => {
                let s = &node.style;
                s.display == Display::Flex
                    && matches!(s.flex_direction, FlexDirection::Column | FlexDirection::ColumnReverse)
                    && matches!(s.align_items, None | Some(Align::Stretch))
            }
            None => in_stretching_column,
        };
        for child in self.nodes[&id].children.clone() {
            self.resolve_node(child, font_size, rtl, viewport, in_stretching_column);
        }
    }

    /// Sends a native node its direction where it's right to left, and in
    /// an app whose language is, so the platform's own direction for the
    /// app (GTK's default, Qt's) doesn't decide it.
    fn resolve_direction(&mut self, id: NodeId, rtl: bool) {
        let node = &self.nodes[&id];
        if !node.kind.is_native() {
            return;
        }
        let had = crate::find_prop!(node.props, LayoutDirection).is_some();
        if !(rtl || self.rtl || had) {
            return;
        }
        let prop = Prop::LayoutDirection(if rtl { LayoutDirection::RightToLeft } else { LayoutDirection::LeftToRight });
        if node.prop(&prop) != Some(&prop) {
            let node = self.nodes.get_mut(&id).unwrap();
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            self.queue_prop(id, prop);
        }
    }

    /// Sends a `Text`'s alignment as left or right for its direction, once
    /// the app has set one or the text is right to left.
    fn resolve_text_align(&mut self, id: NodeId, rtl: bool) {
        let node = &self.nodes[&id];
        let had = crate::find_prop!(node.props, TextAlign).is_some();
        // Right-to-left text starts on the right even where the app didn't
        // align it: not every platform's label takes its direction from
        // the widget (AppKit's takes the app's).
        let Some(align) = node.style.text_align.or((had || rtl).then_some(TextAlign::Start)) else { return };
        let align = match (align, rtl) {
            (TextAlign::Center, _) => HorizontalAlign::Center,
            (TextAlign::Start, false) | (TextAlign::End, true) => HorizontalAlign::Left,
            (TextAlign::Start, true) | (TextAlign::End, false) => HorizontalAlign::Right,
        };
        let prop = Prop::TextAlign(align);
        if node.prop(&prop) != Some(&prop) {
            let node = self.nodes.get_mut(&id).unwrap();
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            self.queue_prop(id, prop);
        }
    }
}

impl Inner {
    /// Which pane of a `Split` a node is, and whether the split is
    /// vertical.
    fn pane(&self, id: NodeId) -> Option<(usize, bool)> {
        let parent = self.nodes[&id].parent?;
        let split = &self.nodes[&parent];
        if split.kind != WidgetKind::Split {
            return None;
        }
        let index = split.children.iter().position(|c| *c == id)?;
        Some((index, crate::find_prop!(split.props, Orientation).is_some_and(|o| o.vertical())))
    }

    /// A `Split`'s tracks along it: its first pane at the app's or the
    /// user's size (its content's, until there's one), kept within the
    /// panes' least sizes and the split's last length; the divider; the
    /// second pane with what's left. And the least sizes, which the
    /// platform stops the divider at.
    fn split_tracks(&self, id: NodeId, cx: &ResolveContext<'_>) -> (Vec<Track>, Prop) {
        let node = &self.nodes[&id];
        let vertical = crate::find_prop!(node.props, Orientation).is_some_and(|o| o.vertical());
        let least = |pane: Option<&NodeId>| {
            let style = &self.nodes[pane?].style;
            match (if vertical { style.min_height } else { style.min_width }).resolve(cx) {
                Resolved::Length(points) => Some(points),
                _ => None,
            }
        };
        let (first, second) = (least(node.children.first()).unwrap_or(0.0), least(node.children.get(1)).unwrap_or(0.0));
        let divider = self.metrics.split_divider;
        let length = if vertical { node.frame.height() } else { node.frame.width() };
        let position = crate::find_prop!(node.props, Number).map(|p| {
            let most = if length > 0.0 { (length - divider - second).max(first) } else { f32::INFINITY };
            (p as f32).clamp(first, most)
        });
        let tracks = vec![
            match position {
                Some(points) => Track::Size(Length::Px(points)),
                None => Track::MinMax(Length::Px(first), Length::Auto),
            },
            Track::Size(Length::Px(divider)),
            Track::MinMax(Length::Px(second), Length::Fr(1.0)),
        ];
        (tracks, Prop::PaneMinimums([first, second]))
    }
}

/// Puts a split's pane in its track: the first, or the third (past the
/// divider).
fn place_pane(style: &mut crate::style::Style, index: usize, vertical: bool) {
    let (along, across) = match vertical {
        false => (&mut style.grid_column, &mut style.grid_row),
        true => (&mut style.grid_row, &mut style.grid_column),
    };
    *along = GridPlacement::at(if index == 0 { 1 } else { 3 });
    *across = GridPlacement::at(1);
}

/// Makes a tab view at least as big as its tab strip (and border), or a
/// group as its heading, unless its style gives it a minimum of its own.
fn tab_strip_minimum(style: &mut taffy::Style, strip: Size) {
    let auto = taffy::LengthPercentageAuto::auto();
    let at_least = |min: &mut taffy::LengthPercentageAuto, v: f32| {
        if *min == auto || *min == taffy::LengthPercentageAuto::length(0.0) {
            *min = if v > 0.0 { taffy::LengthPercentageAuto::length(v) } else { auto };
        }
    };
    at_least(&mut style.min_size.width, strip.width);
    at_least(&mut style.min_size.height, strip.height);
}

/// Adds an inset to the app's padding on one side: its length, or none.
fn add(side: &mut taffy::LengthPercentage, inset: f32) {
    let own = side.into_raw();
    *side = match own.tag() {
        taffy::CompactLength::LENGTH_TAG => taffy::LengthPercentage::length(own.value() + inset),
        _ => taffy::LengthPercentage::length(inset),
    };
}
