//! `Popover`: a XAML `Flyout` whose content is the node's host, a canvas
//! the core sizes, shown at its parent's element (`ShowAt`) while the core
//! says so, under it. Light dismiss is the flyout's own: a click outside
//! or Escape closes it, which is reported.
//!
//! A flyout opens only in a window on screen, and tests never show
//! windows: there, it keeps what the core asked and says it shows under
//! its parent, where it would, and Escape in it closes it as the flyout's
//! light dismiss does (`dismiss`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// How far under its parent a popover's content is in tests: about where
/// the flyout's margin puts it.
const GAP: f64 = 4.0;

struct State {
    /// What the core asked for. The flyout's `Closed` comes after it
    /// closes, so one that closes while it's still wanted is the user's.
    wanted: Cell<bool>,
    anchor: RefCell<Option<w::FrameworkElement>>,
    /// Windows are shown: off in tests (see above).
    on_screen: bool,
}

pub(crate) struct Popover {
    pub(crate) host: w::Canvas,
    flyout: w::Flyout,
    state: Rc<State>,
    id: NodeId,
    events: Events,
    _closed: EventRevoker,
}

impl Popover {
    pub(crate) fn new(id: NodeId, events: Events, on_screen: bool) -> R<Popover> {
        let host = w::Canvas::new()?;
        let flyout = w::Flyout::new()?;
        flyout.cast::<w::IFlyout>()?.SetContent(&host.cast::<w::UIElement>()?)?;
        let base: w::IFlyoutBase = flyout.cast()?;
        base.SetPlacement(w::FlyoutPlacementMode::Bottom)?;
        let state = Rc::new(State { wanted: Cell::new(false), anchor: RefCell::new(None), on_screen });
        let (closing, reported) = (state.clone(), events.clone());
        let closed = base.Closed(move |_, _| {
            if closing.wanted.replace(false) {
                reported.emit(id, UiEvent::PopoverShownChanged(false));
            }
        })?;
        Ok(Popover { host, flyout, state, id, events, _closed: closed })
    }

    /// The element it shows at: its parent's. None: it's out of the tree.
    pub(crate) fn set_anchor(&self, anchor: Option<w::FrameworkElement>) -> R<()> {
        let gone = anchor.is_none();
        *self.state.anchor.borrow_mut() = anchor;
        if gone {
            self.hide()?;
        }
        self.sync()
    }

    pub(crate) fn set_shown(&self, shown: bool) -> R<()> {
        self.state.wanted.set(shown);
        self.sync()
    }

    pub(crate) fn wanted(&self) -> bool {
        self.state.wanted.get()
    }

    /// Its content's size, as the core laid it out.
    pub(crate) fn set_size(&self, width: f64, height: f64) -> R<()> {
        let fe: w::IFrameworkElement = self.host.cast()?;
        fe.SetWidth(width)?;
        fe.SetHeight(height)?;
        self.sync()
    }

    fn is_open(&self) -> bool {
        self.flyout.cast::<w::IFlyoutBase>().and_then(|f| f.IsOpen()).unwrap_or(false)
    }

    /// Shows it once it's wanted, has a size and its parent is in a
    /// window; hides it when it's no longer wanted.
    fn sync(&self) -> R<()> {
        let open = self.is_open();
        if !self.state.wanted.get() {
            return if open { self.hide() } else { Ok(()) };
        }
        let fe: w::IFrameworkElement = self.host.cast()?;
        let empty = |v: f64| v.is_nan() || v <= 0.0;
        if open || !self.state.on_screen || empty(fe.Width()?) || empty(fe.Height()?) {
            return Ok(());
        }
        let Some(anchor) = self.state.anchor.borrow().clone() else { return Ok(()) };
        if anchor.cast::<w::IUIElement>()?.XamlRoot().is_err() {
            return Ok(());
        }
        self.flyout.cast::<w::IFlyoutBase>()?.ShowAt(&anchor)
    }

    fn hide(&self) -> R<()> {
        if self.is_open() {
            self.flyout.cast::<w::IFlyoutBase>()?.Hide()?;
        }
        Ok(())
    }

    /// Whether it shows: as XAML shows it, or in tests as the core asked
    /// while it has a parent.
    pub(crate) fn shown(&self) -> bool {
        match self.state.on_screen {
            true => self.is_open(),
            false => self.state.wanted.get() && self.state.anchor.borrow().is_some(),
        }
    }

    /// The user's Escape in it, in tests: it closes, and says so, as the
    /// flyout's light dismiss does.
    pub(crate) fn dismiss(&self) -> bool {
        if self.state.on_screen || !self.shown() {
            return false;
        }
        self.state.wanted.set(false);
        self.events.emit(self.id, UiEvent::PopoverShownChanged(false));
        true
    }

    /// Where it shows, in its parent's coordinates (x, y, width, height);
    /// None while it's hidden. In tests, under its parent, as it would.
    pub(crate) fn frame(&self) -> Option<(f64, f64, f64, f64)> {
        if !self.shown() {
            return None;
        }
        let anchor = self.state.anchor.borrow().clone()?;
        let fe: w::IFrameworkElement = self.host.cast().ok()?;
        let (width, height) = (fe.Width().ok()?, fe.Height().ok()?);
        if !self.state.on_screen {
            let anchor: w::IFrameworkElement = anchor.cast().ok()?;
            let height = anchor.Height().ok().filter(|h| !h.is_nan()).or_else(|| anchor.ActualHeight().ok())?;
            let below = height + GAP;
            return Some((0.0, below, width, height));
        }
        let transform = self.host.cast::<w::IUIElement>().ok()?.TransformToVisual(&anchor).ok()?;
        let at = transform.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
        Some((at.x as f64, at.y as f64, width, height))
    }
}
