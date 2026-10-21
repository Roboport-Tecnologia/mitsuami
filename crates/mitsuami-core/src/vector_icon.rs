//! An app's own icon, from vector path data, shown the way each platform
//! shows its symbolic icons: in the colour it gives icons, which follows
//! dark mode, and in an icon's size.

use std::fmt;
use std::sync::Arc;

use kurbo::{Affine, BezPath, PathEl, Shape};

use crate::geometry::{Point, Rect, Size};

/// An icon the app draws, as path data in the SVG path syntax (`M0 0L10
/// 10Z`, arcs too), which SVG files, XAML's and Avalonia's geometries and
/// most icon sets use. A leading `F0` (even-odd) or `F1` (non-zero), as
/// XAML's path data has, sets the fill rule; non-zero by default.
///
/// It's filled in the colour the platform gives symbolic icons: AppKit
/// draws it as a template image, GTK as a symbolic SVG, Kirigami as a mask,
/// WinUI as a `PathIcon`.
#[derive(Clone)]
pub struct VectorIcon(Arc<Inner>);

struct Inner {
    path: BezPath,
    view_box: Rect,
    even_odd: bool,
}

/// A step of an icon's path, in points from the icon's top left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathStep {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}

impl VectorIcon {
    /// The path fitted to its own bounds, as XAML's `Path` with uniform
    /// stretch shows it.
    pub fn new(data: &str) -> Result<VectorIcon, String> {
        let (path, even_odd) = parse(data)?;
        let b = path.bounding_box();
        let view_box = Rect::new(b.x0 as f32, b.y0 as f32, b.width() as f32, b.height() as f32);
        Ok(VectorIcon(Arc::new(Inner { path, view_box, even_odd })))
    }

    /// The path in a view box, as an SVG file's `viewBox`: icon sets draw
    /// on a grid (24 by 24, say), with room around each shape.
    pub fn with_view_box(data: &str, view_box: Rect) -> Result<VectorIcon, String> {
        let (path, even_odd) = parse(data)?;
        Ok(VectorIcon(Arc::new(Inner { path, view_box, even_odd })))
    }

    pub fn view_box(&self) -> Rect {
        self.0.view_box
    }

    /// Whether it fills by the even-odd rule, not the non-zero one.
    pub fn even_odd(&self) -> bool {
        self.0.even_odd
    }

    /// The path in a square of `side` points, its view box fitted inside,
    /// keeping its proportions, and centred; y goes down.
    pub fn steps(&self, side: f32) -> Vec<PathStep> {
        let p = |p: kurbo::Point| Point::new(p.x as f32, p.y as f32);
        self.fitted(side)
            .elements()
            .iter()
            .map(|el| match *el {
                PathEl::MoveTo(a) => PathStep::Move(p(a)),
                PathEl::LineTo(a) => PathStep::Line(p(a)),
                PathEl::QuadTo(a, b) => PathStep::Quad(p(a), p(b)),
                PathEl::CurveTo(a, b, c) => PathStep::Cubic(p(a), p(b), p(c)),
                PathEl::ClosePath => PathStep::Close,
            })
            .collect()
    }

    /// The path data fitted in a square of `side` points, as for
    /// [`steps`](Self::steps): for platforms that take path data (XAML).
    pub fn path_data(&self, side: f32) -> String {
        self.fitted(side).to_svg()
    }

    /// An SVG file of it, `side` points square, filled black: for
    /// platforms that recolour symbolic SVGs (GTK, Kirigami).
    pub fn svg(&self, side: f32) -> String {
        let rule = if self.0.even_odd { "evenodd" } else { "nonzero" };
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{side}\" height=\"{side}\" viewBox=\"0 0 {side} {side}\">\
             <path fill=\"#000\" fill-rule=\"{rule}\" d=\"{}\"/></svg>",
            self.path_data(side)
        )
    }

    fn fitted(&self, side: f32) -> BezPath {
        let Rect { origin, size: Size { width, height } } = self.0.view_box;
        let longest = width.max(height);
        if longest <= 0.0 {
            return BezPath::new();
        }
        let scale = f64::from(side / longest);
        let dx = f64::from(side - width * side / longest) / 2.0 - f64::from(origin.x) * scale;
        let dy = f64::from(side - height * side / longest) / 2.0 - f64::from(origin.y) * scale;
        Affine::new([scale, 0.0, 0.0, scale, dx, dy]) * &self.0.path
    }
}

fn parse(data: &str) -> Result<(BezPath, bool), String> {
    let data = data.trim();
    let (data, even_odd) = match data.get(..2) {
        Some("F0") => (&data[2..], true),
        Some("F1") => (&data[2..], false),
        _ => (data, false),
    };
    let path = BezPath::from_svg(data).map_err(|e| format!("bad path data: {e}"))?;
    Ok((path, even_odd))
}

impl PartialEq for VectorIcon {
    fn eq(&self, other: &VectorIcon) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.view_box == other.0.view_box
                && self.0.even_odd == other.0.even_odd
                && self.0.path == other.0.path)
    }
}

/// Its size, not its path: props print in snapshots.
impl fmt::Debug for VectorIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Size { width, height } = self.0.view_box.size;
        write!(f, "VectorIcon({} steps in {width}×{height})", self.0.path.elements().len())
    }
}
