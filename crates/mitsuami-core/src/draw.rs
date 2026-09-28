//! The drawn tier of custom widgets: a small, platform-neutral 2D API.
//!
//! Drawing records a [`DisplayList`]: plain data the core sends to the
//! backend as a prop. Backends rasterize it with the platform's own 2D
//! library (CoreGraphics, Direct2D, Cairo/GSK) and resolve semantic colors
//! against the current appearance, so drawn widgets follow dark mode and
//! the accent color without redrawing in app code.

use crate::backend::PlatformMetrics;
use crate::geometry::{Point, Rect, Size};

/// Semantic colors follow the platform's appearance; `Rgba` is fixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Label,
    SecondaryLabel,
    Accent,
    Separator,
    ControlBackground,
    WindowBackground,
    /// Something went wrong, or is destructive: the platform's red.
    Error,
    /// Something needs attention: the platform's orange or yellow.
    Warning,
    /// Something went well: the platform's green.
    Success,
    Rgba(u8, u8, u8, u8),
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::Rgba(r, g, b, 255)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathElement {
    MoveTo(Point),
    LineTo(Point),
    /// A cubic Bézier curve to `to`.
    CurveTo {
        c1: Point,
        c2: Point,
        to: Point,
    },
    Close,
}

/// A vector path, built by chaining: `Path::new().move_to(0.0, 0.0).line_to(10.0, 0.0).close()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    elements: Vec<PathElement>,
}

impl Path {
    pub fn new() -> Path {
        Path::default()
    }

    pub fn move_to(mut self, x: f32, y: f32) -> Path {
        self.elements.push(PathElement::MoveTo(Point::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: f32, y: f32) -> Path {
        self.elements.push(PathElement::LineTo(Point::new(x, y)));
        self
    }

    pub fn curve_to(mut self, c1: Point, c2: Point, to: Point) -> Path {
        self.elements.push(PathElement::CurveTo { c1, c2, to });
        self
    }

    pub fn close(mut self) -> Path {
        self.elements.push(PathElement::Close);
        self
    }

    /// A closed polygon through `points`.
    pub fn polygon(points: impl IntoIterator<Item = Point>) -> Path {
        let mut path = Path::new();
        for (i, p) in points.into_iter().enumerate() {
            path = if i == 0 { path.move_to(p.x, p.y) } else { path.line_to(p.x, p.y) };
        }
        path.close()
    }

    pub fn elements(&self) -> &[PathElement] {
        &self.elements
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect(Rect),
    RoundedRect(Rect, f32),
    Ellipse(Rect),
    Path(Path),
}

impl From<Rect> for Shape {
    fn from(rect: Rect) -> Shape {
        Shape::Rect(rect)
    }
}

impl From<Path> for Shape {
    fn from(path: Path) -> Shape {
        Shape::Path(path)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    Fill { shape: Shape, color: Color },
    Stroke { shape: Shape, color: Color, width: f32 },
}

/// What a drawn widget looks like, in its own coordinates (logical units,
/// origin at the top-left).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList(pub Vec<DrawOp>);

impl DisplayList {
    pub fn ops(&self) -> &[DrawOp] {
        &self.0
    }
}

/// Where a drawn widget draws. Records operations; draws nothing itself.
pub struct Canvas<'a> {
    size: Size,
    metrics: &'a PlatformMetrics,
    ops: Vec<DrawOp>,
}

impl<'a> Canvas<'a> {
    pub fn new(size: Size, metrics: &'a PlatformMetrics) -> Canvas<'a> {
        Canvas { size, metrics, ops: Vec::new() }
    }

    /// The widget's laid-out size.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Font sizes and spacing tokens, for sizing things like the platform does.
    pub fn metrics(&self) -> &PlatformMetrics {
        self.metrics
    }

    pub fn fill(&mut self, shape: impl Into<Shape>, color: Color) {
        self.ops.push(DrawOp::Fill { shape: shape.into(), color });
    }

    pub fn stroke(&mut self, shape: impl Into<Shape>, color: Color, width: f32) {
        self.ops.push(DrawOp::Stroke { shape: shape.into(), color, width });
    }

    pub fn finish(self) -> DisplayList {
        DisplayList(self.ops)
    }
}
