//! `Popover`: a XAML `Flyout` whose content is the node's host, a canvas
//! the core sizes, shown at its parent's element (`ShowAt`) while the core
//! says so, under it, past the window's edge if need be. Light dismiss is the flyout's own: a click outside
//! or Escape closes it, which is reported.
//!
//! Tests' windows are transparent: there, the flyout opens inside its
//! window (constrained to its bounds, which a flyout past the window's
//! edge would leave, in a window of its own that shows), so it's as
//! unseen as the window, and a capture draws it in. Escape in it closes it
//! as the flyout's light dismiss does (`dismiss`).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

struct State {
    /// What the core asked for. The flyout's `Closed` comes after it
    /// closes, so one that closes while it's still wanted is the user's.
    wanted: Cell<bool>,
    /// The backend is closing it: its `Closed` isn't the user's. Wanted
    /// again before that, it shows again then.
    hiding: Cell<bool>,
    anchor: RefCell<Option<w::FrameworkElement>>,
    /// Windows are shown: off in tests (see above).
    on_screen: bool,
    /// What's in it that was measured out of a live tree, where controls
    /// have no style: a `Button` measures 0 × 19.
    unstyled: RefCell<HashSet<NodeId>>,
}

pub(crate) struct Popover {
    pub(crate) host: w::Canvas,
    flyout: w::Flyout,
    state: Rc<State>,
    id: NodeId,
    events: Events,
    _closed: EventRevoker,
    _loaded: EventRevoker,
}

impl Popover {
    pub(crate) fn new(id: NodeId, events: Events, on_screen: bool) -> R<Popover> {
        let host = w::Canvas::new()?;
        let flyout = w::Flyout::new()?;
        flyout.cast::<w::IFlyout>()?.SetContent(&host.cast::<w::UIElement>()?)?;
        let base: w::IFlyoutBase = flyout.cast()?;
        base.SetPlacement(w::FlyoutPlacementMode::Bottom)?;
        // Under its parent even past the window's edge, as a menu
        // flyout opens: kept inside it, a flyout with no room above or
        // below a button went beside it, against the window's top. In
        // tests, inside it (see above).
        base.SetShouldConstrainToRootBounds(!on_screen)?;
        let state = Rc::new(State {
            wanted: Cell::new(false),
            hiding: Cell::new(false),
            anchor: RefCell::new(None),
            on_screen,
            unstyled: RefCell::default(),
        });
        let (closing, reported, shown, content) = (state.clone(), events.clone(), flyout.clone(), host.clone());
        let closed = base.Closed(move |_, _| {
            if closing.hiding.replace(false) {
                // Shown again while it closed: XAML ignored that, as it
                // was still open.
                if closing.wanted.get() {
                    _ = sync(&shown, &content, &closing);
                }
            } else if closing.wanted.replace(false) {
                reported.emit(id, UiEvent::PopoverShownChanged(false));
            }
        })?;
        // Its content joins a live tree only as it opens.
        let (opening, emitter) = (state.clone(), events.clone());
        let loaded = host.cast::<w::IFrameworkElement>()?.Loaded(move |_, _| {
            for measured in opening.unstyled.take() {
                emitter.emit(measured, UiEvent::Remeasure);
            }
        })?;
        Ok(Popover { host, flyout, state, id, events, _closed: closed, _loaded: loaded })
    }

    /// Something in it was measured out of a live tree: it's measured again
    /// when the popover opens.
    pub(crate) fn measured_unstyled(&self, id: NodeId) {
        self.state.unstyled.borrow_mut().insert(id);
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
        is_open(&self.flyout)
    }

    fn sync(&self) -> R<()> {
        sync(&self.flyout, &self.host, &self.state)
    }

    fn hide(&self) -> R<()> {
        hide(&self.flyout, &self.state)
    }

    /// Whether it shows, as XAML shows it.
    pub(crate) fn shown(&self) -> bool {
        self.is_open()
    }

    /// The user's Escape in it, in tests: it closes, and says so, as the
    /// flyout's light dismiss does.
    pub(crate) fn dismiss(&self) -> bool {
        if self.state.on_screen || !self.shown() {
            return false;
        }
        // No longer wanted, its `Closed` isn't reported again.
        self.state.wanted.set(false);
        _ = self.hide();
        self.events.emit(self.id, UiEvent::PopoverShownChanged(false));
        true
    }

    /// Where it shows, in its parent's coordinates (x, y, width, height);
    /// None while it's hidden.
    pub(crate) fn frame(&self) -> Option<(f64, f64, f64, f64)> {
        if !self.shown() {
            return None;
        }
        let anchor = self.state.anchor.borrow().clone()?;
        let fe: w::IFrameworkElement = self.host.cast().ok()?;
        let (width, height) = (fe.Width().ok()?, fe.Height().ok()?);
        let transform = self.host.cast::<w::IUIElement>().ok()?.TransformToVisual(&anchor).ok()?;
        let at = transform.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
        Some((at.x as f64, at.y as f64, width, height))
    }
}

fn is_open(flyout: &w::Flyout) -> bool {
    flyout.cast::<w::IFlyoutBase>().and_then(|f| f.IsOpen()).unwrap_or(false)
}

/// Shows it once it's wanted, has a size and its parent is in a window;
/// hides it when it's no longer wanted. While it closes, it's left to
/// `Closed`, which shows it again if it's wanted by then.
fn sync(flyout: &w::Flyout, host: &w::Canvas, state: &State) -> R<()> {
    let open = is_open(flyout);
    if !state.wanted.get() {
        return if open { hide(flyout, state) } else { Ok(()) };
    }
    let fe: w::IFrameworkElement = host.cast()?;
    let empty = |v: f64| v.is_nan() || v <= 0.0;
    if open || empty(fe.Width()?) || empty(fe.Height()?) {
        return Ok(());
    }
    let Some(anchor) = state.anchor.borrow().clone() else { return Ok(()) };
    if anchor.cast::<w::IUIElement>()?.XamlRoot().is_err() {
        return Ok(());
    }
    flyout.cast::<w::IFlyoutBase>()?.ShowAt(&anchor)
}

fn hide(flyout: &w::Flyout, state: &State) -> R<()> {
    if is_open(flyout) {
        state.hiding.set(true);
        flyout.cast::<w::IFlyoutBase>()?.Hide()?;
    }
    Ok(())
}
