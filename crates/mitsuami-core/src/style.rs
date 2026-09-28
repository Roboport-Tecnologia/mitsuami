//! Layout style: the CSS flexbox / grid property set, in abstract units.

use taffy::style_helpers::{TaffyGridLine, TaffyGridSpan};

use crate::units::{Length, ResolveContext, Resolved};
use crate::widget::TextAlign;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    Block,
    /// Not laid out and not visible.
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    /// The default, as in React Native: app UIs stack vertically far more often.
    #[default]
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    End,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

/// Layout direction. Styles use logical edges (`start` / `end`), so the same
/// style works for left-to-right and right-to-left locales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDirection {
    #[default]
    Inherit,
    Ltr,
    Rtl,
}

/// Four edges in logical order: inline start/end, block top/bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub start: Length,
    pub end: Length,
    pub top: Length,
    pub bottom: Length,
}

impl Edges {
    pub fn all(v: Length) -> Edges {
        Edges { start: v, end: v, top: v, bottom: v }
    }

    pub fn zero() -> Edges {
        Edges::all(Length::Px(0.0))
    }
}

/// Where an item sits on one grid axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridPlacement {
    /// 1-based line to start at; negative counts from the end. `None` = auto-placed.
    pub start: Option<i16>,
    /// Number of tracks to span.
    pub span: Option<u16>,
}

impl GridPlacement {
    pub fn at(line: i16) -> GridPlacement {
        GridPlacement { start: Some(line), span: None }
    }

    pub fn span(tracks: u16) -> GridPlacement {
        GridPlacement { start: None, span: Some(tracks) }
    }

    pub fn at_span(line: i16, tracks: u16) -> GridPlacement {
        GridPlacement { start: Some(line), span: Some(tracks) }
    }
}

/// A grid track size: a length (`Fr` allowed), or a `minmax` pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Size(Length),
    MinMax(Length, Length),
    MinContent,
    MaxContent,
}

impl From<Length> for Track {
    fn from(l: Length) -> Track {
        Track::Size(l)
    }
}

/// `repeat(3, 1.fr())`
pub fn repeat(count: usize, track: impl Into<Track>) -> Vec<Track> {
    vec![track.into(); count]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub display: Display,
    /// Hidden nodes act like `display: none` but keep their `display`.
    pub hidden: bool,
    pub position: Position,
    pub direction: TextDirection,
    /// `Text` only: where its lines go, resolved against `direction`.
    pub text_align: Option<TextAlign>,
    /// Content may overflow along these axes and is scrolled natively.
    /// Set by `ScrollView`; also lets it shrink below its content size.
    pub scroll_x: bool,
    pub scroll_y: bool,
    pub inset: Edges,

    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub aspect_ratio: Option<f32>,

    pub margin: Edges,
    pub padding: Edges,

    pub flex_direction: FlexDirection,
    pub flex_wrap: bool,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,

    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    /// Alignment inside a grid cell along the inline axis.
    pub justify_self: Option<Align>,
    pub align_content: Option<Justify>,
    pub justify_content: Option<Justify>,
    pub row_gap: Length,
    pub column_gap: Length,

    pub grid_template_columns: Vec<Track>,
    pub grid_template_rows: Vec<Track>,
    pub grid_column: GridPlacement,
    pub grid_row: GridPlacement,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            display: Display::Flex,
            hidden: false,
            position: Position::Relative,
            direction: TextDirection::Inherit,
            text_align: None,
            scroll_x: false,
            scroll_y: false,
            inset: Edges::default(),
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            aspect_ratio: None,
            margin: Edges::zero(),
            padding: Edges::zero(),
            flex_direction: FlexDirection::Column,
            flex_wrap: false,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            align_items: None,
            align_self: None,
            justify_self: None,
            align_content: None,
            justify_content: None,
            row_gap: Length::Px(0.0),
            column_gap: Length::Px(0.0),
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_column: GridPlacement::default(),
            grid_row: GridPlacement::default(),
        }
    }
}

// ------------------------------------------------------------------ to Taffy

fn dimension(l: Length, cx: &ResolveContext<'_>) -> taffy::Dimension {
    match l.resolve(cx) {
        Resolved::Length(v) => taffy::Dimension::length(v),
        Resolved::Percent(v) => taffy::Dimension::percent(v),
        Resolved::Fr(_) | Resolved::Auto => taffy::Dimension::auto(),
    }
}

fn lpa(l: Length, cx: &ResolveContext<'_>) -> taffy::LengthPercentageAuto {
    match l.resolve(cx) {
        Resolved::Length(v) => taffy::LengthPercentageAuto::length(v),
        Resolved::Percent(v) => taffy::LengthPercentageAuto::percent(v),
        Resolved::Fr(_) | Resolved::Auto => taffy::LengthPercentageAuto::auto(),
    }
}

fn lp(l: Length, cx: &ResolveContext<'_>) -> taffy::LengthPercentage {
    match l.resolve(cx) {
        Resolved::Length(v) => taffy::LengthPercentage::length(v),
        Resolved::Percent(v) => taffy::LengthPercentage::percent(v),
        Resolved::Fr(_) | Resolved::Auto => taffy::LengthPercentage::length(0.0),
    }
}

fn min_track(l: Length, cx: &ResolveContext<'_>) -> taffy::MinTrackSizingFunction {
    match l.resolve(cx) {
        Resolved::Length(v) => taffy::MinTrackSizingFunction::length(v),
        Resolved::Percent(v) => taffy::MinTrackSizingFunction::percent(v),
        Resolved::Fr(_) | Resolved::Auto => taffy::MinTrackSizingFunction::auto(),
    }
}

fn max_track(l: Length, cx: &ResolveContext<'_>) -> taffy::MaxTrackSizingFunction {
    match l.resolve(cx) {
        Resolved::Length(v) => taffy::MaxTrackSizingFunction::length(v),
        Resolved::Percent(v) => taffy::MaxTrackSizingFunction::percent(v),
        Resolved::Fr(v) => taffy::MaxTrackSizingFunction::fr(v),
        Resolved::Auto => taffy::MaxTrackSizingFunction::auto(),
    }
}

fn track(t: Track, cx: &ResolveContext<'_>) -> taffy::TrackSizingFunction {
    match t {
        // `1fr` in CSS is `minmax(auto, 1fr)`.
        Track::Size(l) => taffy::MinMax { min: min_track(l, cx), max: max_track(l, cx) },
        Track::MinMax(min, max) => taffy::MinMax { min: min_track(min, cx), max: max_track(max, cx) },
        Track::MinContent => taffy::MinMax {
            min: taffy::MinTrackSizingFunction::min_content(),
            max: taffy::MaxTrackSizingFunction::min_content(),
        },
        Track::MaxContent => taffy::MinMax {
            min: taffy::MinTrackSizingFunction::max_content(),
            max: taffy::MaxTrackSizingFunction::max_content(),
        },
    }
}

fn placement(p: GridPlacement) -> taffy::Line<taffy::GridPlacement> {
    let start = match p.start {
        Some(line) => taffy::GridPlacement::from_line_index(line),
        None => taffy::GridPlacement::Auto,
    };
    let end = match p.span {
        Some(n) => taffy::GridPlacement::from_span(n),
        None => taffy::GridPlacement::Auto,
    };
    taffy::Line { start, end }
}

fn align_items(a: Align) -> taffy::AlignItems {
    match a {
        Align::Start => taffy::AlignItems::START,
        Align::End => taffy::AlignItems::END,
        Align::Center => taffy::AlignItems::CENTER,
        Align::Stretch => taffy::AlignItems::STRETCH,
        Align::Baseline => taffy::AlignItems::BASELINE,
    }
}

fn justify(j: Justify) -> taffy::JustifyContent {
    match j {
        Justify::Start => taffy::JustifyContent::START,
        Justify::End => taffy::JustifyContent::END,
        Justify::Center => taffy::JustifyContent::CENTER,
        Justify::Stretch => taffy::JustifyContent::STRETCH,
        Justify::SpaceBetween => taffy::JustifyContent::SPACE_BETWEEN,
        Justify::SpaceAround => taffy::JustifyContent::SPACE_AROUND,
        Justify::SpaceEvenly => taffy::JustifyContent::SPACE_EVENLY,
    }
}

impl Style {
    /// Resolves units and maps logical edges for the given direction.
    pub(crate) fn to_taffy(&self, cx: &ResolveContext<'_>, rtl: bool) -> taffy::Style {
        let edges_lpa = |e: &Edges| {
            let (left, right) = if rtl { (e.end, e.start) } else { (e.start, e.end) };
            taffy::Rect { left: lpa(left, cx), right: lpa(right, cx), top: lpa(e.top, cx), bottom: lpa(e.bottom, cx) }
        };
        let edges_lp = |e: &Edges| {
            let (left, right) = if rtl { (e.end, e.start) } else { (e.start, e.end) };
            taffy::Rect { left: lp(left, cx), right: lp(right, cx), top: lp(e.top, cx), bottom: lp(e.bottom, cx) }
        };
        taffy::Style {
            display: match if self.hidden { Display::None } else { self.display } {
                Display::Flex => taffy::Display::Flex,
                Display::Grid => taffy::Display::Grid,
                Display::Block => taffy::Display::Block,
                Display::None => taffy::Display::None,
            },
            direction: if rtl { taffy::Direction::Rtl } else { taffy::Direction::Ltr },
            position: match self.position {
                Position::Relative => taffy::Position::Relative,
                Position::Absolute => taffy::Position::Absolute,
            },
            inset: edges_lpa(&self.inset),
            overflow: taffy::Point {
                x: if self.scroll_x { taffy::Overflow::Scroll } else { taffy::Overflow::Visible },
                y: if self.scroll_y { taffy::Overflow::Scroll } else { taffy::Overflow::Visible },
            },
            scrollbar_width: 0.0,
            size: taffy::Size { width: dimension(self.width, cx), height: dimension(self.height, cx) },
            min_size: taffy::Size { width: lpa(self.min_width, cx), height: lpa(self.min_height, cx) },
            max_size: taffy::Size { width: lpa(self.max_width, cx), height: lpa(self.max_height, cx) },
            aspect_ratio: self.aspect_ratio,
            margin: edges_lpa(&self.margin),
            padding: edges_lp(&self.padding),
            flex_direction: match self.flex_direction {
                FlexDirection::Row => taffy::FlexDirection::Row,
                FlexDirection::Column => taffy::FlexDirection::Column,
                FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
                FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
            },
            flex_wrap: if self.flex_wrap { taffy::FlexWrap::Wrap } else { taffy::FlexWrap::NoWrap },
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            flex_basis: dimension(self.flex_basis, cx),
            align_items: self.align_items.map(align_items),
            align_self: self.align_self.map(align_items),
            justify_self: self.justify_self.map(align_items),
            align_content: self.align_content.map(justify),
            justify_content: self.justify_content.map(justify),
            gap: taffy::Size { width: lp(self.column_gap, cx), height: lp(self.row_gap, cx) },
            grid_template_columns: self
                .grid_template_columns
                .iter()
                .map(|t| taffy::GridTemplateComponent::Single(track(*t, cx)))
                .collect(),
            grid_template_rows: self
                .grid_template_rows
                .iter()
                .map(|t| taffy::GridTemplateComponent::Single(track(*t, cx)))
                .collect(),
            grid_column: placement(self.grid_column),
            grid_row: placement(self.grid_row),
            ..taffy::Style::default()
        }
    }
}

impl Style {
    /// Takes no space and is not shown.
    pub fn is_hidden(&self) -> bool {
        self.hidden || self.display == Display::None
    }
}

macro_rules! static_value {
    ($($t:ty),*) => {$(
        impl mitsuami_reactive::IntoValue<$t> for $t {
            fn into_value(self) -> mitsuami_reactive::Value<$t> {
                mitsuami_reactive::Value::Static(self)
            }
        }
    )*};
}

static_value!(Display, FlexDirection, Align, Justify, Position, TextDirection, GridPlacement);
