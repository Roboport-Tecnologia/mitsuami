use std::fmt;

use mitsuami_reactive::{IntoValue, Value};

/// A size in logical units (points / DIPs), never physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size { width: 0.0, height: 0.0 };

    pub const fn new(width: f32, height: f32) -> Size {
        Size { width, height }
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// The content size a window opens at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowSize {
    Fixed(Size),
    /// This width, and the height of the content at its first layout, so
    /// the window fits what each platform's controls add up to. After that
    /// it's an ordinary window: it doesn't follow later content changes.
    FitHeight(f32),
    /// This width, and the height of the content as it changes: the window
    /// grows and shrinks with it. The user resizes only its width (GTK has
    /// no such window: there it isn't resizable). `Ui::set_window_size`
    /// sets its width.
    FollowHeight(f32),
    /// Like `FollowHeight` until the user, or `Ui::set_window_size`,
    /// changes its height: then it's an ordinary window.
    FollowHeightUntilResized(f32),
}

impl From<Size> for WindowSize {
    fn from(size: Size) -> WindowSize {
        WindowSize::Fixed(size)
    }
}

impl IntoValue<WindowSize> for WindowSize {
    fn into_value(self) -> Value<WindowSize> {
        Value::Static(self)
    }
}

impl IntoValue<WindowSize> for Size {
    fn into_value(self) -> Value<WindowSize> {
        Value::Static(WindowSize::Fixed(self))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }
}

/// Distances in from a rectangle's edges, in logical units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub const ZERO: Insets = Insets { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };

    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Insets {
        Insets { top, right, bottom, left }
    }
}

/// A rectangle in logical units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Rect = Rect { origin: Point::ZERO, size: Size::ZERO };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect { origin: Point { x, y }, size: Size { width, height } }
    }

    pub fn x(&self) -> f32 {
        self.origin.x
    }

    pub fn y(&self) -> f32 {
        self.origin.y
    }

    pub fn width(&self) -> f32 {
        self.size.width
    }

    pub fn height(&self) -> f32 {
        self.size.height
    }

    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn offset(&self, by: Point) -> Rect {
        Rect { origin: Point::new(self.origin.x + by.x, self.origin.y + by.y), size: self.size }
    }

    /// The overlapping area, or `None` when the rectangles don't overlap.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x().max(other.x());
        let y = self.y().max(other.y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        (max_x > x && max_y > y).then(|| Rect::new(x, y, max_x - x, max_y - y))
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{} {}×{}", Num(self.origin.x), Num(self.origin.y), Num(self.size.width), Num(self.size.height))
    }
}

/// Formats a float compactly: no trailing `.0`, at most two decimals.
pub struct Num(pub f32);

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rounded = (self.0 * 100.0).round() / 100.0;
        if rounded == rounded.trunc() { write!(f, "{}", rounded as i64) } else { write!(f, "{rounded}") }
    }
}
