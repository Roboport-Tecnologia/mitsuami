//! Abstract units, resolved to logical units at layout time.

use crate::geometry::Size;

/// A length in abstract units, CSS-style.
///
/// Everything resolves to *logical* units (points on macOS, effective pixels
/// on Windows, logical pixels on GTK), so layouts are DPI-independent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    /// Logical pixels.
    Px(f32),
    /// Multiple of the node's inherited font size.
    Em(f32),
    /// Multiple of the platform body font size (follows the user's text size setting).
    Rem(f32),
    /// Percentage (0–100) of the containing block.
    Percent(f32),
    /// Percentage of the window content width.
    Vw(f32),
    /// Percentage of the window content height.
    Vh(f32),
    Vmin(f32),
    Vmax(f32),
    /// Fraction of the free space. Grid tracks only; elsewhere it acts as `Auto`.
    Fr(f32),
    /// A platform spacing token.
    Token(Spacing),
    #[default]
    Auto,
    /// The node's natural size: what it measures with no limit, as CSS's
    /// `max-content`. For the least and most sizes of a measured node (a
    /// list, as high as its rows; a label, its one line), so a list that
    /// grows from nothing stops at its rows. Elsewhere it acts as `Auto`.
    MaxContent,
}

/// Spacing tokens. Each backend decides what they mean, so spacing follows
/// the platform's design language (e.g. HIG vs Fluent vs GNOME HIG).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spacing {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
    /// How far the platform indents each level of a tree or outline: for
    /// rows nested under others, as its own trees nest them.
    Indent,
}

impl From<Spacing> for Length {
    fn from(token: Spacing) -> Length {
        Length::Token(token)
    }
}

/// `16.px()`, `1.5.em()`, `50.pct()`, `1.fr()`, …
pub trait LengthExt {
    fn px(self) -> Length;
    fn em(self) -> Length;
    fn rem(self) -> Length;
    fn pct(self) -> Length;
    fn vw(self) -> Length;
    fn vh(self) -> Length;
    fn vmin(self) -> Length;
    fn vmax(self) -> Length;
    fn fr(self) -> Length;
}

macro_rules! length_ext {
    ($($t:ty),*) => {$(
        impl LengthExt for $t {
            fn px(self) -> Length { Length::Px(self as f32) }
            fn em(self) -> Length { Length::Em(self as f32) }
            fn rem(self) -> Length { Length::Rem(self as f32) }
            fn pct(self) -> Length { Length::Percent(self as f32) }
            fn vw(self) -> Length { Length::Vw(self as f32) }
            fn vh(self) -> Length { Length::Vh(self as f32) }
            fn vmin(self) -> Length { Length::Vmin(self as f32) }
            fn vmax(self) -> Length { Length::Vmax(self as f32) }
            fn fr(self) -> Length { Length::Fr(self as f32) }
        }
    )*};
}

length_ext!(i32, u32, f32, f64);

/// Everything needed to turn a [`Length`] into logical units.
#[derive(Clone, Copy, Debug)]
pub struct ResolveContext<'a> {
    pub font_size: f32,
    pub root_font_size: f32,
    pub viewport: Size,
    pub spacing: &'a SpacingScale,
}

/// Values of the spacing tokens, in logical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    /// A tree's indentation per level.
    pub indent: f32,
}

impl SpacingScale {
    pub fn get(&self, token: Spacing) -> f32 {
        match token {
            Spacing::None => 0.0,
            Spacing::Xs => self.xs,
            Spacing::Sm => self.sm,
            Spacing::Md => self.md,
            Spacing::Lg => self.lg,
            Spacing::Xl => self.xl,
            Spacing::Indent => self.indent,
        }
    }
}

/// A length after unit resolution: only what the layout engine understands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolved {
    Length(f32),
    /// Fraction, 0–1.
    Percent(f32),
    Fr(f32),
    Auto,
}

impl Length {
    pub fn resolve(&self, cx: &ResolveContext<'_>) -> Resolved {
        let vw = cx.viewport.width / 100.0;
        let vh = cx.viewport.height / 100.0;
        match *self {
            Length::Px(v) => Resolved::Length(v),
            Length::Em(v) => Resolved::Length(v * cx.font_size),
            Length::Rem(v) => Resolved::Length(v * cx.root_font_size),
            Length::Percent(v) => Resolved::Percent(v / 100.0),
            Length::Vw(v) => Resolved::Length(v * vw),
            Length::Vh(v) => Resolved::Length(v * vh),
            Length::Vmin(v) => Resolved::Length(v * vw.min(vh)),
            Length::Vmax(v) => Resolved::Length(v * vw.max(vh)),
            Length::Fr(v) => Resolved::Fr(v),
            Length::Token(t) => Resolved::Length(cx.spacing.get(t)),
            // Measured at layout (`Inner::content_limits`).
            Length::Auto | Length::MaxContent => Resolved::Auto,
        }
    }
}

impl From<i32> for Length {
    fn from(v: i32) -> Length {
        Length::Px(v as f32)
    }
}

impl From<f32> for Length {
    fn from(v: f32) -> Length {
        Length::Px(v)
    }
}

/// Plain numbers are logical pixels: `.width(200)`.
macro_rules! into_length_value {
    ($($t:ty => $e:expr),*) => {$(
        impl mitsuami_reactive::IntoValue<Length> for $t {
            fn into_value(self) -> mitsuami_reactive::Value<Length> {
                let f: fn($t) -> Length = $e;
                mitsuami_reactive::Value::Static(f(self))
            }
        }
    )*};
}

into_length_value!(
    Length => |l| l,
    Spacing => Length::Token,
    i32 => |v| Length::Px(v as f32),
    f32 => Length::Px
);
