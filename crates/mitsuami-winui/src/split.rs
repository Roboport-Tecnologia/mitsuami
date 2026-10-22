//! A split: WinUI 3 has no splitter, so it's built as File Explorer's
//! navigation pane resizer is: the two panes side by side (or stacked), a
//! 1 epx line between them, and over the line a wider transparent grip
//! that shows the sizing cursor and moves the line as it's dragged. The
//! line is in `DividerStrokeColorDefaultBrush`, Fluent's divider stroke,
//! which the table headers' grippers here use too: File Explorer's pane
//! edge is a hairline in the theme's divider colour.
//!
//! The split node is a `Canvas` with the line and the grip first; the
//! core's pane hosts go after them, and the grip sits above them
//! (`ZIndex`). The core places the panes: the grip follows the first
//! pane's frame, and a drag reports the first pane's new length, which
//! the core lays out.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::{EventValue, NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// The line's thickness: a hairline, as File Explorer's pane edge.
pub(crate) const DIVIDER: f32 = 1.0;
/// How wide the grip is across the line, half on each pane: room to
/// catch it with the pointer, as File Explorer's resizer has.
const GRIP: f64 = 8.0;

/// How many of the canvas's children are the split's own, before the
/// core's panes.
pub(crate) const PARTS: u32 = 2;

/// The line, as XAML's `Separator` would be if it had one.
const LINE_MARKUP: &str = r#"<Border xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Background="{ThemeResource DividerStrokeColorDefaultBrush}"/>"#;

/// The grip: transparent, so it takes the pointer, and over the panes'
/// edges, which it overlaps.
const GRIP_MARKUP: &str = r#"<Border xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Background="Transparent" Canvas.ZIndex="1"/>"#;

#[derive(Default)]
struct State {
    vertical: bool,
    /// The split's size.
    size: (f64, f64),
    /// The first pane's length along the split, from its frame.
    first: f64,
    minimums: [f32; 2],
    /// The first pane's length last reported or set.
    position: Option<f64>,
    /// Where a drag started along the split, and the first pane's length then.
    drag: Option<(f64, f64)>,
}

impl State {
    /// The split's length along it.
    fn length(&self) -> f64 {
        if self.vertical { self.size.1 } else { self.size.0 }
    }

    /// A first pane's length kept within the panes' least lengths.
    fn clamp(&self, position: f64) -> f64 {
        let [first, second] = self.minimums;
        let most = self.length() - f64::from(DIVIDER) - f64::from(second);
        position.min(most).max(f64::from(first))
    }
}

/// The split node's native parts.
pub(crate) struct Split {
    pub canvas: w::Canvas,
    line: w::Border,
    grip: w::Border,
    state: Rc<RefCell<State>>,
    _revokers: Vec<EventRevoker>,
}

impl Split {
    pub(crate) fn new(id: NodeId, emitter: Events) -> R<Split> {
        let canvas = w::Canvas::new()?;
        let line: w::Border = w::XamlReader::Load(LINE_MARKUP)?.cast()?;
        let grip: w::Border = w::XamlReader::Load(GRIP_MARKUP)?.cast()?;
        let children = canvas.cast::<w::IPanel>()?.Children()?;
        children.Append(&line.cast::<w::UIElement>()?)?;
        children.Append(&grip.cast::<w::UIElement>()?)?;
        let state = Rc::new(RefCell::new(State::default()));
        let revokers = drags(id, &emitter, &canvas, &grip, &state)?;
        let split = Split { canvas, line, grip, state, _revokers: revokers };
        split.set_cursor()?;
        Ok(split)
    }

    /// The sizing cursor over the grip, as over File Explorer's resizer.
    /// `ProtectedCursor` is meant for subclasses, but the element takes it
    /// all the same (as the table headers' grippers do).
    fn set_cursor(&self) -> R<()> {
        let shape = if self.state.borrow().vertical {
            w::InputSystemCursorShape::SizeNorthSouth
        } else {
            w::InputSystemCursorShape::SizeWestEast
        };
        let cursor = w::InputSystemCursor::Create(shape)?;
        self.grip.cast::<w::IUIElementProtected>()?.SetProtectedCursor(&cursor)
    }

    pub(crate) fn set_vertical(&self, vertical: bool) -> R<()> {
        self.state.borrow_mut().vertical = vertical;
        self.set_cursor()?;
        self.place()
    }

    pub(crate) fn vertical(&self) -> bool {
        self.state.borrow().vertical
    }

    pub(crate) fn set_minimums(&self, minimums: [f32; 2]) {
        self.state.borrow_mut().minimums = minimums;
    }

    pub(crate) fn minimums(&self) -> [f32; 2] {
        self.state.borrow().minimums
    }

    /// The app's position: the core lays the panes out at it.
    pub(crate) fn set_position(&self, position: f64) {
        self.state.borrow_mut().position = Some(position);
    }

    pub(crate) fn position(&self) -> Option<f64> {
        self.state.borrow().position
    }

    /// Its frame's size.
    pub(crate) fn set_size(&self, width: f64, height: f64) -> R<()> {
        self.state.borrow_mut().size = (width, height);
        self.place()
    }

    /// A pane's frame, which the core placed: the first one's, at the
    /// split's start, says where the line goes.
    pub(crate) fn pane_placed(&self, x: f64, y: f64, width: f64, height: f64) -> R<()> {
        {
            let mut state = self.state.borrow_mut();
            let (start, length) = if state.vertical { (y, height) } else { (x, width) };
            if start != 0.0 {
                return Ok(());
            }
            state.first = length;
        }
        self.place()
    }

    /// Moves the divider as the user would (assistive technology sets a
    /// splitter's value), within the panes' least lengths: it's reported.
    pub(crate) fn move_divider(&self, emitter: &Events, id: NodeId, position: f64) {
        let position = {
            let mut state = self.state.borrow_mut();
            let position = state.clamp(position);
            state.position = Some(position);
            position
        };
        emitter.emit(id, UiEvent::Changed(EventValue::Number(position)));
    }

    /// The line after the first pane, the grip centred on it.
    fn place(&self) -> R<()> {
        let state = self.state.borrow();
        let (width, height) = state.size;
        let divider = f64::from(DIVIDER);
        let line = self.line.cast::<w::UIElement>()?;
        let grip = self.grip.cast::<w::UIElement>()?;
        let (line_fe, grip_fe) = (line.cast::<w::IFrameworkElement>()?, grip.cast::<w::IFrameworkElement>()?);
        let at = state.first;
        let centre = at + divider / 2.0 - GRIP / 2.0;
        if state.vertical {
            w::Canvas::SetLeft(&line, 0.0)?;
            w::Canvas::SetTop(&line, at)?;
            line_fe.SetWidth(width.max(0.0))?;
            line_fe.SetHeight(divider)?;
            w::Canvas::SetLeft(&grip, 0.0)?;
            w::Canvas::SetTop(&grip, centre)?;
            grip_fe.SetWidth(width.max(0.0))?;
            grip_fe.SetHeight(GRIP)?;
        } else {
            w::Canvas::SetLeft(&line, at)?;
            w::Canvas::SetTop(&line, 0.0)?;
            line_fe.SetWidth(divider)?;
            line_fe.SetHeight(height.max(0.0))?;
            w::Canvas::SetLeft(&grip, centre)?;
            w::Canvas::SetTop(&grip, 0.0)?;
            grip_fe.SetWidth(GRIP)?;
            grip_fe.SetHeight(height.max(0.0))?;
        }
        Ok(())
    }
}

/// Dragging the grip moves the divider, reported as the first pane's new
/// length as it goes (File Explorer's panes follow the pointer); the core
/// lays the panes out at it.
fn drags(
    id: NodeId,
    emitter: &Events,
    canvas: &w::Canvas,
    grip: &w::Border,
    state: &Rc<RefCell<State>>,
) -> R<Vec<EventRevoker>> {
    let element: w::IUIElement = grip.cast()?;
    let canvas: w::UIElement = canvas.cast()?;
    // Where the pointer is along the split, which doesn't move.
    let along = {
        let (canvas, state) = (canvas.clone(), state.clone());
        move |args: &windows_core::Ref<w::PointerRoutedEventArgs>| -> Option<f64> {
            let args: w::IPointerRoutedEventArgs = args.as_ref()?.cast().ok()?;
            let point = args.GetCurrentPoint(&canvas).ok()?.cast::<w::IPointerPoint>().ok()?.Position().ok()?;
            Some(if state.borrow().vertical { point.y as f64 } else { point.x as f64 })
        }
    };
    let pressed = element.PointerPressed({
        let (state, element, along) = (state.clone(), element.clone(), along.clone());
        move |_, args| {
            let Some(start) = along(&args) else { return };
            let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IPointerRoutedEventArgs>().ok()) else { return };
            if let Ok(pointer) = args.Pointer() {
                _ = element.CapturePointer(&pointer);
            }
            let mut state = state.borrow_mut();
            state.drag = Some((start, state.first));
            _ = args.SetHandled(true);
        }
    })?;
    let moved = element.PointerMoved({
        let (state, emitter, along) = (state.clone(), emitter.clone(), along.clone());
        move |_, args| {
            let Some(now) = along(&args) else { return };
            let position = {
                let mut state = state.borrow_mut();
                let Some((start, first)) = state.drag else { return };
                let position = state.clamp(first + now - start);
                if state.position == Some(position) {
                    return;
                }
                state.position = Some(position);
                position
            };
            emitter.emit(id, UiEvent::Changed(EventValue::Number(position)));
        }
    })?;
    // XAML lets the capture go as the button goes up.
    let released = element.PointerReleased({
        let state = state.clone();
        move |_, _| state.borrow_mut().drag = None
    })?;
    Ok(vec![pressed, moved, released])
}
