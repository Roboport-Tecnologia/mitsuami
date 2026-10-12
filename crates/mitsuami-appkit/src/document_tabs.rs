//! Document tabs: a strip of tabs built from AppKit views, made as the
//! system's window tab bar is on macOS 26 (its glass, its springs, its
//! drag), as Safari's and Xcode's are. It's in the window's title bar,
//! under the title and toolbar, where theirs are; `Strip` keeps it in the
//! content.
//!
//! macOS's own window tabs are the user's (Window > Merge All Windows):
//! each is a window, which can't share one content.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use mitsuami_core::{DocumentTabsStyle, EventSink, EventValue, NodeId, Size, UiEvent, moved_index};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityElement, NSAccessibilityRadioButtonRole, NSAccessibilityTabButtonSubrole,
    NSAccessibilityTabGroupRole, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSAutoresizingMaskOptions, NSBezelStyle, NSBezierPath, NSBox, NSBoxType, NSButton, NSColor,
    NSControlSize, NSEvent, NSEventType, NSFont, NSFontWeightMedium, NSFontWeightRegular, NSGlassEffectView,
    NSGraphicsContext, NSImage, NSImageSymbolConfiguration, NSLayoutAttribute, NSLineBreakMode, NSShadow,
    NSTextAlignment, NSTextField, NSTitlebarAccessoryViewController, NSTrackingArea, NSTrackingAreaOptions,
    NSUserInterfaceLayoutDirection, NSView, NSWindow, NSWindowOrderingMode, NSWindowTitleVisibility, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSNumber, NSPoint, NSProcessInfo, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString, NSTimer,
};

use crate::classes::{ClosureTarget, zero_rect};

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

/// What the core gave, which the strip shows.
#[derive(Default)]
struct Model {
    titles: Vec<String>,
    selected: Option<usize>,
    /// Only what the app gave: the mirror reads back what the core has.
    closable: Option<bool>,
    reorderable: Option<bool>,
    new_tab: Option<bool>,
    style: Option<DocumentTabsStyle>,
    enabled: Option<bool>,
}

impl Model {
    fn closable(&self) -> bool {
        self.closable == Some(true) && self.enabled()
    }

    fn reorderable(&self) -> bool {
        self.reorderable == Some(true) && self.enabled()
    }

    fn new_tab(&self) -> bool {
        self.new_tab == Some(true) && self.enabled()
    }

    fn enabled(&self) -> bool {
        self.enabled != Some(false)
    }

    /// The chosen tab, as the platform shows one: the first if the core's
    /// is past the tabs.
    fn chosen(&self) -> Option<usize> {
        (!self.titles.is_empty()).then(|| self.selected.filter(|i| *i < self.titles.len()).unwrap_or(0))
    }

    fn move_tab(&mut self, from: usize, to: usize) {
        let title = self.titles.remove(from);
        self.titles.insert(to, title);
        self.selected = self.selected.map(|i| moved_index(i, from, to));
    }
}

/// The node's model and where it reports to.
struct Shared {
    id: NodeId,
    events: EventSink,
    model: RefCell<Model>,
}

impl Shared {
    fn emit(&self, event: UiEvent) {
        self.events.emit(self.id, event);
    }
}

pub(crate) struct DocumentTabs {
    /// The node's view in the content: the strip's place, empty while the
    /// strip is in the title bar.
    pub(crate) view: Retained<NSView>,
    pub(crate) strip: Retained<TabStrip>,
    shared: Rc<Shared>,
    /// The title bar's band the strip goes in, as a bottom accessory under
    /// the title and toolbar, where Safari's and Xcode's tab bars are.
    band: Retained<NSView>,
    accessory: Retained<NSTitlebarAccessoryViewController>,
    /// The view the strip is in as a toolbar item, in the title's row.
    pub(crate) row: Retained<NSView>,
    /// The window it's in the chrome of, and where.
    placed: RefCell<Option<(NodeId, Retained<NSWindow>, Place)>>,
}

/// Where in the window's chrome the strip is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Place {
    /// Its own row under the title and toolbar, as Safari's separate tabs.
    Band,
    /// The title's row, as a toolbar item, as Safari's compact tabs.
    Row,
}

impl DocumentTabs {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> DocumentTabs {
        let shared = Rc::new(Shared { id, events, model: RefCell::default() });
        let strip = TabStrip::new(mtm, shared.clone());
        let view = NSView::new(mtm);
        let band = NSView::new(mtm);
        band.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(0.0, BAND)));
        let accessory = NSTitlebarAccessoryViewController::new(mtm);
        accessory.setView(&band);
        accessory.setLayoutAttribute(NSLayoutAttribute::Bottom);
        // A toolbar sizes its items' views with Auto Layout: at least room
        // for a few tabs, as wide as the room left.
        let row = NSView::new(mtm);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        row.widthAnchor().constraintGreaterThanOrEqualToConstant(ROW_MIN).setActive(true);
        row.widthAnchor().constraintLessThanOrEqualToConstant(ROW_MAX).setActive(true);
        row.heightAnchor().constraintEqualToConstant(HEIGHT).setActive(true);
        let tabs = DocumentTabs { view, strip, shared, band, accessory, row, placed: RefCell::default() };
        tabs.fill_view();
        tabs.strip.rebuild();
        tabs
    }

    /// The strip fills the node's view.
    fn fill_view(&self) {
        self.strip.removeFromSuperview();
        self.strip.setFrame(self.view.bounds());
        self.strip.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        self.view.addSubview(&self.strip);
    }

    /// Where in the window's chrome it asks to be: nowhere for a strip.
    pub(crate) fn wanted(&self) -> Option<Place> {
        match self.shared.model.borrow().style {
            Some(DocumentTabsStyle::Strip) => None,
            Some(DocumentTabsStyle::TitleRow) => Some(Place::Row),
            _ => Some(Place::Band),
        }
    }

    /// The window node it's in the chrome of, and where.
    pub(crate) fn placed(&self) -> Option<(NodeId, Place)> {
        self.placed.borrow().as_ref().map(|(id, _, place)| (*id, *place))
    }

    /// Puts the strip in a window's chrome, or back in the content
    /// (`None`). In the title's row the toolbar takes `row` as an item
    /// (the caller's), and the title hides, as Safari's does with compact
    /// tabs. Moving, it takes room in the content or gives it back, so the
    /// core measures it again.
    pub(crate) fn place(&self, to: Option<(NodeId, &NSWindow, Place)>) {
        let current = self.placed.borrow_mut().take();
        if let Some((_, window, place)) = &current {
            match place {
                Place::Band => self.accessory.removeFromParentViewController(),
                Place::Row => window.setTitleVisibility(NSWindowTitleVisibility::Visible),
            }
            self.fill_view();
        }
        if let Some((id, window, place)) = to {
            self.strip.removeFromSuperview();
            match place {
                Place::Band => {
                    let width = window.frame().size.width;
                    self.band.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(width, BAND)));
                    self.strip.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
                    self.strip.setFrame(NSRect::new(
                        NSPoint::new(BAND_SIDE, BAND - BAND_SIDE - HEIGHT),
                        NSSize::new((width - 2.0 * BAND_SIDE).max(0.0), HEIGHT),
                    ));
                    self.band.addSubview(&self.strip);
                    window.addTitlebarAccessoryViewController(&self.accessory);
                }
                Place::Row => {
                    self.strip.setFrame(self.row.bounds());
                    self.strip.setAutoresizingMask(
                        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
                    );
                    self.row.addSubview(&self.strip);
                    window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
                }
            }
            *self.placed.borrow_mut() = Some((id, window.retain(), place));
        }
        self.shared.emit(UiEvent::Remeasure);
    }

    fn change(&self, change: impl FnOnce(&mut Model)) {
        change(&mut self.shared.model.borrow_mut());
        self.strip.rebuild();
    }

    pub(crate) fn set_titles(&self, titles: Vec<String>) {
        self.change(|m| m.titles = titles);
    }

    pub(crate) fn set_selected(&self, index: Option<usize>) {
        self.change(|m| m.selected = index);
    }

    pub(crate) fn set_closable(&self, on: bool) {
        self.change(|m| m.closable = Some(on));
    }

    pub(crate) fn set_reorderable(&self, on: bool) {
        self.change(|m| m.reorderable = Some(on));
    }

    pub(crate) fn set_new_tab(&self, on: bool) {
        self.change(|m| m.new_tab = Some(on));
    }

    /// The strip in the content, or in the title bar (see `place`).
    pub(crate) fn set_style(&self, style: DocumentTabsStyle) {
        self.change(|m| m.style = Some(style));
    }

    pub(crate) fn set_enabled(&self, on: bool) {
        self.change(|m| m.enabled = Some(on));
    }

    pub(crate) fn enabled(&self) -> bool {
        self.shared.model.borrow().enabled()
    }

    pub(crate) fn titles(&self) -> Vec<String> {
        self.strip.titles()
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.shared.model.borrow().chosen()
    }

    pub(crate) fn closable(&self) -> Option<bool> {
        self.shared.model.borrow().closable?;
        Some(self.strip.has_close_buttons())
    }

    pub(crate) fn reorderable(&self) -> Option<bool> {
        self.shared.model.borrow().reorderable
    }

    pub(crate) fn new_tab(&self) -> Option<bool> {
        self.shared.model.borrow().new_tab?;
        Some(self.strip.has_plus())
    }

    pub(crate) fn style(&self) -> Option<DocumentTabsStyle> {
        self.shared.model.borrow().style
    }

    /// Picks a tab by its title, as a click on it does. Whether there's
    /// one.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let index = self.shared.model.borrow().titles.iter().position(|t| t == title);
        let Some(index) = index else { return false };
        choose(&self.shared, index);
        self.strip.rebuild();
        true
    }

    /// Presses a tab's close button: the app is asked.
    pub(crate) fn close(&self, index: usize) -> bool {
        let model = self.shared.model.borrow();
        if !model.closable() || index >= model.titles.len() {
            return false;
        }
        drop(model);
        self.shared.emit(UiEvent::TabCloseRequested(index));
        true
    }

    /// Moves a tab as a drag does: the strip shows it there before the app
    /// hears.
    pub(crate) fn move_tab(&self, from: usize, to: usize) -> bool {
        let mut model = self.shared.model.borrow_mut();
        if !model.reorderable() || from >= model.titles.len() || to >= model.titles.len() {
            return false;
        }
        if from == to {
            return true;
        }
        model.move_tab(from, to);
        drop(model);
        self.strip.rebuild();
        self.shared.emit(UiEvent::TabMoved { from, to });
        true
    }

    /// Presses the new-tab button.
    pub(crate) fn ask_for_tab(&self) -> bool {
        if !self.shared.model.borrow().new_tab() {
            return false;
        }
        self.shared.emit(UiEvent::NewTabRequested);
        true
    }

    /// The strip's natural size; nothing in the title bar, which is the
    /// window's chrome.
    pub(crate) fn natural_size(&self) -> Size {
        if self.placed.borrow().is_some() { Size::ZERO } else { self.strip.natural_size() }
    }
}

/// The user chose a tab: the model says so, then the app hears.
fn choose(shared: &Shared, index: usize) {
    let mut model = shared.model.borrow_mut();
    if model.chosen() == Some(index) {
        return;
    }
    model.selected = Some(index);
    drop(model);
    shared.emit(UiEvent::Changed(EventValue::Index(index)));
}

// ------------------------------------------------------------------ strip

/// The geometry of macOS 26's window tab bar (Safari draws its own the
/// same): a capsule track, tabs inset in it with a point between them for
/// their separator, a close button on each tab's leading side, and a round
/// new-tab button after the track.
const HEIGHT: f64 = 28.0;
const INSET: f64 = 2.0;
const SEPARATOR: f64 = 18.0;
const CLOSE: f64 = 16.0;
const CLOSE_INSET: f64 = 5.0;
const PLUS: f64 = 28.0;
const PLUS_GAP: f64 = 4.0;
/// The title bar's band: the strip, the room under it, and either side.
const BAND: f64 = HEIGHT + 8.0;
const BAND_SIDE: f64 = 8.0;
/// In the title's row: at least room for a few tabs, and as wide as the
/// toolbar's room.
const ROW_MIN: f64 = 160.0;
const ROW_MAX: f64 = 100_000.0;
/// A dragged tab is narrower, lifted off the track.
const LIFT: f64 = 8.0;
/// How far a pressed tab goes before it's dragged.
const DRAG_START: f64 = 3.0;
/// The springs' rates, critically damped: tabs settle in about 0.3s, as
/// the system's do (measured frame by frame on macOS 26); a dragged tab
/// follows the pointer closer.
const SETTLE: f64 = 20.0;
const FOLLOW: f64 = 40.0;
/// How long a close button takes to fade in or out.
const FADE: f64 = 0.1;

/// The system tab bar's glass, which AppKit keeps private: the track is a
/// "subdued" glass view of its own, and each tab an `NSGlassEffectView` in
/// a private variant, the chosen tab's lit, a hovered tab's pressed in.
/// Looked up at run time; without them (before macOS 26, or if AppKit
/// changes) the strip paints those colours itself.
const TRACK_GLASS: &std::ffi::CStr =
    c"_TtC6AppKitP33_C7B8466D3EAE829EF20F43530A1CD21C37NSLessExpensiveSubduedGlassEffectView";
const VARIANT_PLAIN: isize = 13;
const VARIANT_CHOSEN: isize = 1;
const INTERACTION_HOVER: isize = 2;

fn system_glass() -> Option<&'static AnyClass> {
    let track = AnyClass::get(TRACK_GLASS)?;
    let glass = AnyClass::get(c"NSGlassEffectView")?;
    let private = [sel!(set_variant:), sel!(set_subvariant:), sel!(set_interactionState:)];
    (track.responds_to(sel!(setCornerRadius:)) && private.iter().all(|s| glass.responds_to(*s))).then_some(track)
}

fn set_glass(glass: &NSView, chosen: bool, hovered: bool) {
    let variant = if chosen { VARIANT_CHOSEN } else { VARIANT_PLAIN };
    let subvariant = chosen.then(|| ns("tab"));
    // The chosen tab lightens under the pointer too.
    let interaction = if hovered { INTERACTION_HOVER } else { 0 };
    unsafe {
        let _: () = msg_send![glass, set_variant: variant];
        let _: () = msg_send![glass, set_subvariant: subvariant.as_deref()];
        let _: () = msg_send![glass, set_interactionState: interaction];
    }
}

/// A critically damped spring towards `target`.
#[derive(Clone, Copy, Default)]
struct Spring {
    value: f64,
    velocity: f64,
    target: f64,
}

impl Spring {
    fn at(value: f64) -> Spring {
        Spring { value, velocity: 0.0, target: value }
    }

    fn snap(&mut self) {
        *self = Spring::at(self.target);
    }

    fn settled(&self) -> bool {
        self.value == self.target && self.velocity == 0.0
    }

    /// Steadily towards its target, over `duration` for a step of 1.
    fn ramp(&mut self, dt: f64, duration: f64) {
        let step = dt / duration;
        let d = self.target - self.value;
        self.value = if d.abs() <= step { self.target } else { self.value + step * d.signum() };
    }

    /// Where it is `dt` seconds on, at the rate `rate`: exactly, so
    /// any frame rate goes the same way.
    fn step(&mut self, dt: f64, rate: f64) {
        let d = self.value - self.target;
        let b = self.velocity + rate * d;
        let decay = (-rate * dt).exp();
        self.value = self.target + (d + b * dt) * decay;
        self.velocity = (b - rate * (d + b * dt)) * decay;
        if (self.value - self.target).abs() < 0.2 && self.velocity.abs() < 2.0 {
            self.snap();
        }
    }
}

/// One tab of the strip: its view, holding its glass (with the system's)
/// and over it its title and close button, which the glass's lighting
/// leaves alone; where it is and how wide, both moving, and how far its
/// close button has faded in.
struct Tab {
    view: Retained<NSView>,
    glass: Option<Retained<NSView>>,
    content: Retained<NSView>,
    label: Retained<NSTextField>,
    close: Retained<NSButton>,
    element: Retained<TabElement>,
    title: String,
    x: Spring,
    width: Spring,
    shown: Spring,
}

/// A press on a tab: the tab it was (`from`) and where it is now (`at`),
/// where the pointer was in it, and where the press began.
#[derive(Clone, Copy)]
struct Drag {
    from: usize,
    at: usize,
    grab: f64,
    start: f64,
    moving: bool,
}

pub(crate) struct ElementIvars {
    press: Box<dyn Fn()>,
}

define_class!(
    /// A tab of the strip to assistive technology: a tab button, as
    /// AppKit's own tab views have them, which pressing chooses.
    #[unsafe(super(NSAccessibilityElement))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ElementIvars]
    pub(crate) struct TabElement;

    impl TabElement {
        #[unsafe(method(accessibilityPerformPress))]
        fn perform_press(&self) -> bool {
            (self.ivars().press)();
            true
        }
    }
);

pub(crate) struct StripIvars {
    shared: Rc<Shared>,
    /// The system's track glass, or a plain view the strip paints under.
    track: Retained<NSView>,
    glass: bool,
    tabs: RefCell<Vec<Tab>>,
    separators: RefCell<Vec<Retained<NSBox>>>,
    /// The new-tab button, and the view it's in (its glass, with the
    /// system's).
    plus: RefCell<Option<(Retained<NSView>, Retained<NSButton>)>>,
    /// The close buttons' and the new-tab button's targets, which buttons
    /// only hold weak references to.
    close_target: RefCell<Option<Retained<ClosureTarget>>>,
    plus_target: RefCell<Option<Retained<ClosureTarget>>>,
    hover: Cell<Option<usize>>,
    drag: Cell<Option<Drag>>,
    tracking: RefCell<Option<Retained<NSTrackingArea>>>,
    /// Ticks while tabs move, and when it last did.
    timer: RefCell<Option<Retained<NSTimer>>>,
    tick: Cell<f64>,
}

define_class!(
    /// A strip of tabs, as the system's tab bar and Safari's: tabs sharing
    /// a capsule track equally, the chosen one raised, a close button on
    /// the tab under the pointer, a new-tab button after the track. Tabs
    /// are picked on a press, dragged to move them (the others make way),
    /// closed with a middle click too, and grow in and out as they open
    /// and close.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = StripIvars]
    pub(crate) struct TabStrip;

    impl TabStrip {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        /// A new size moves the tabs at once.
        #[unsafe(method(layout))]
        fn layout(&self) {
            let _: () = unsafe { msg_send![super(self), layout] };
            self.retarget();
            if self.ivars().timer.borrow().is_none() {
                self.snap();
            }
            self.place();
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }

        /// Its buttons take their clicks while they show; the rest of it
        /// takes the others (a tab's title is a label, which would).
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let hit: Option<Retained<NSView>> = unsafe { msg_send![super(self), hitTest: point] };
            match hit {
                Some(view) if view.downcast_ref::<NSButton>().is_some_and(|b| b.alphaValue() > 0.0) => {
                    Retained::into_raw(view)
                }
                Some(_) => self as *const TabStrip as *mut NSView,
                None => std::ptr::null_mut(),
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        /// It takes the focus from the keyboard; a click gives it only
        /// with Full Keyboard Access, as the system's tab bar does.
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            let app = NSApplication::sharedApplication(self.mtm());
            let clicked = app.currentEvent().is_some_and(|e| {
                matches!(e.r#type(), NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown)
            });
            !clicked || app.isFullKeyboardAccessEnabled()
        }

        /// In the title bar, a press on a tab is the tab's, not a drag of
        /// the window.
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let x = self.track_x(event);
            let Some(index) = self.tab_at(x) else { return };
            if !self.ivars().shared.model.borrow().enabled() {
                return;
            }
            let grab = x - self.ivars().tabs.borrow()[index].x.value;
            self.ivars().drag.set(Some(Drag { from: index, at: index, grab, start: x, moving: false }));
            choose(&self.ivars().shared, index);
            self.restyle();
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let Some(mut drag) = self.ivars().drag.get() else { return };
            if !self.ivars().shared.model.borrow().reorderable() {
                return;
            }
            let x = self.track_x(event);
            if !drag.moving && (x - drag.start).abs() < DRAG_START {
                return;
            }
            drag.moving = true;
            drag.at = self.drag_to(drag.at, x - drag.grab);
            self.ivars().drag.set(Some(drag));
            // The pointer is on the dragged tab, wherever it's gone (a
            // drag sends no moves).
            self.ivars().hover.set(Some(drag.at));
            self.retarget();
            self.restyle();
            self.animate();
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            let Some(drag) = self.ivars().drag.take() else { return };
            if drag.moving {
                // Into its place.
                self.retarget();
                self.restyle();
                self.animate();
                if drag.from != drag.at {
                    self.ivars().shared.emit(UiEvent::TabMoved { from: drag.from, to: drag.at });
                }
            }
        }

        /// A middle click closes a tab, as in Safari.
        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            let closable = self.ivars().shared.model.borrow().closable();
            if event.buttonNumber() == 2
                && closable
                && let Some(index) = self.tab_at(self.track_x(event))
            {
                self.ivars().shared.emit(UiEvent::TabCloseRequested(index));
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let track = self.track_rect();
            let over = point.x >= track.origin.x
                && point.x < track.origin.x + track.size.width
                && point.y >= track.origin.y
                && point.y < track.origin.y + track.size.height;
            self.set_hover(if over { self.tab_at(self.track_x(event)) } else { None });
            self.close_under(point);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.set_hover(None);
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
            if self.ivars().tracking.borrow().is_some() {
                return;
            }
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    zero_rect(),
                    NSTrackingAreaOptions::MouseEnteredAndExited
                        | NSTrackingAreaOptions::MouseMoved
                        | NSTrackingAreaOptions::ActiveInKeyWindow
                        | NSTrackingAreaOptions::InVisibleRect,
                    Some(self),
                    None,
                )
            };
            self.addTrackingArea(&area);
            *self.ivars().tracking.borrow_mut() = Some(area);
        }

        /// The arrow keys pick the tab before or after, as a tab view's do.
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let step: isize = match event.keyCode() {
                123 => -1,
                124 => 1,
                _ => {
                    let _: () = unsafe { msg_send![super(self), keyDown: event] };
                    return;
                }
            };
            let step = if self.right_to_left() { -step } else { step };
            let (chosen, count, enabled) = {
                let model = self.ivars().shared.model.borrow();
                (model.chosen(), model.titles.len(), model.enabled())
            };
            let Some(chosen) = chosen.filter(|_| enabled) else { return };
            let next = chosen as isize + step;
            if (0..count as isize).contains(&next) {
                choose(&self.ivars().shared, next as usize);
                self.restyle();
            }
        }

        #[unsafe(method(focusRingMaskBounds))]
        fn focus_ring_mask_bounds(&self) -> NSRect {
            self.chosen_rect().unwrap_or_else(zero_rect)
        }

        #[unsafe(method(drawFocusRingMask))]
        fn draw_focus_ring_mask(&self) {
            if let Some(rect) = self.chosen_rect() {
                pill(rect).fill();
            }
        }

        /// The painted colours differ in light and dark.
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeEffectiveAppearance] };
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            if self.window().is_none() {
                self.stop();
            }
        }
    }
);

impl TabStrip {
    fn new(mtm: MainThreadMarker, shared: Rc<Shared>) -> Retained<TabStrip> {
        let glass = system_glass();
        let track: Retained<NSView> = match glass {
            Some(class) => unsafe {
                let track: Allocated<NSView> = msg_send![class, alloc];
                let track: Retained<NSView> = msg_send![track, initWithFrame: zero_rect()];
                let _: () = msg_send![&*track, setCornerRadius: HEIGHT / 2.0];
                track
            },
            None => NSView::new(mtm),
        };
        let this = TabStrip::alloc(mtm).set_ivars(StripIvars {
            shared,
            track,
            glass: glass.is_some(),
            tabs: RefCell::default(),
            separators: RefCell::default(),
            plus: RefCell::default(),
            close_target: RefCell::default(),
            plus_target: RefCell::default(),
            hover: Cell::new(None),
            drag: Cell::new(None),
            tracking: RefCell::default(),
            timer: RefCell::default(),
            tick: Cell::new(0.0),
        });
        let strip: Retained<TabStrip> = unsafe { msg_send![super(this), initWithFrame: zero_rect()] };
        strip.addSubview(&strip.ivars().track);
        strip.setAccessibilityElement(true);
        strip.setAccessibilityRole(Some(unsafe { NSAccessibilityTabGroupRole }));
        let weak = Rc::downgrade(&strip.ivars().shared);
        let close_target = ClosureTarget::new(mtm, move |sender| {
            let Some(shared) = weak.upgrade() else { return };
            let closable = shared.model.borrow().closable();
            if let Some(button) = sender.downcast_ref::<NSButton>()
                && closable
            {
                shared.emit(UiEvent::TabCloseRequested(button.tag() as usize));
            }
        });
        let weak = Rc::downgrade(&strip.ivars().shared);
        let plus_target = ClosureTarget::new(mtm, move |_| {
            if let Some(shared) = weak.upgrade()
                && shared.model.borrow().new_tab()
            {
                shared.emit(UiEvent::NewTabRequested);
            }
        });
        *strip.ivars().close_target.borrow_mut() = Some(close_target);
        *strip.ivars().plus_target.borrow_mut() = Some(plus_target);
        strip
    }

    fn mtm(&self) -> MainThreadMarker {
        MainThreadMarker::from(self)
    }

    fn right_to_left(&self) -> bool {
        self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft
    }

    fn track_width(&self) -> f64 {
        let plus = if self.ivars().plus.borrow().is_some() { PLUS + PLUS_GAP } else { 0.0 };
        (self.bounds().size.width - plus).max(0.0)
    }

    /// Where the track is: the strip's leading part, as high as the
    /// system's, centred.
    fn track_rect(&self) -> NSRect {
        let size = self.bounds().size;
        let width = self.track_width();
        let x = if self.right_to_left() { size.width - width } else { 0.0 };
        NSRect::new(NSPoint::new(x, ((size.height - HEIGHT) / 2.0).round()), NSSize::new(width, HEIGHT))
    }

    /// A pointer's place along the track, from its leading end.
    fn track_x(&self, event: &NSEvent) -> f64 {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let track = self.track_rect();
        if self.right_to_left() { track.origin.x + track.size.width - point.x } else { point.x - track.origin.x }
    }

    /// Each place's leading edge and the tabs' width: the track's inside
    /// shared equally, a point between each for its separator.
    fn slots(&self, count: usize) -> (Vec<f64>, f64) {
        let inside = self.track_width() - 2.0 * INSET;
        let width = if count == 0 { 0.0 } else { ((inside - (count - 1) as f64) / count as f64).max(0.0) };
        ((0..count).map(|i| INSET + i as f64 * (width + 1.0)).collect(), width)
    }

    /// The tab at a place along the track.
    fn tab_at(&self, x: f64) -> Option<usize> {
        let tabs = self.ivars().tabs.borrow();
        tabs.iter().position(|t| x >= t.x.value && x < t.x.value + t.width.value + 1.0)
    }

    /// A tab's rect in the strip, as it is now.
    fn tab_rect(&self, tab: &Tab) -> NSRect {
        let track = self.track_rect();
        let height = HEIGHT - 2.0 * INSET;
        let x = if self.right_to_left() {
            track.origin.x + track.size.width - tab.x.value - tab.width.value
        } else {
            track.origin.x + tab.x.value
        };
        NSRect::new(NSPoint::new(x, track.origin.y + INSET), NSSize::new(tab.width.value.max(0.0), height))
    }

    fn chosen_rect(&self) -> Option<NSRect> {
        let chosen = self.ivars().shared.model.borrow().chosen()?;
        self.ivars().tabs.borrow().get(chosen).map(|t| self.tab_rect(t))
    }

    /// The dragged tab follows the pointer along the track; it takes the
    /// place of a tab whose middle its far edge passes, as the system's
    /// does, the model with it. Where it is now.
    fn drag_to(&self, mut at: usize, x: f64) -> usize {
        let count = self.ivars().tabs.borrow().len();
        let (slots, width) = self.slots(count);
        let lifted = (width - LIFT).max(0.0);
        let x = (x + LIFT / 2.0).clamp(INSET, (self.track_width() - INSET - lifted).max(INSET));
        {
            let mut tabs = self.ivars().tabs.borrow_mut();
            tabs[at].x.target = x;
            tabs[at].width.target = lifted;
        }
        let middle = |i: usize| slots[i] + width / 2.0;
        loop {
            let to = if at + 1 < count && x + lifted > middle(at + 1) {
                at + 1
            } else if at > 0 && x < middle(at - 1) {
                at - 1
            } else {
                break;
            };
            self.ivars().shared.model.borrow_mut().move_tab(at, to);
            self.ivars().tabs.borrow_mut().swap(at, to);
            at = to;
        }
        self.renumber();
        at
    }

    /// Every tab's place, but a dragged one's, which follows the pointer.
    fn retarget(&self) {
        let dragged = self.ivars().drag.get().filter(|d| d.moving).map(|d| d.at);
        let mut tabs = self.ivars().tabs.borrow_mut();
        let (slots, width) = self.slots(tabs.len());
        for (index, tab) in tabs.iter_mut().enumerate() {
            if Some(index) != dragged {
                tab.x.target = slots[index];
                tab.width.target = width;
            }
        }
    }

    fn snap(&self) {
        for tab in self.ivars().tabs.borrow_mut().iter_mut() {
            tab.x.snap();
            tab.width.snap();
            tab.shown.snap();
        }
    }

    /// Moves the tabs to their places: over time while the strip shows,
    /// at once otherwise or with Reduce Motion.
    fn animate(&self) {
        let shown = self.window().is_some_and(|w| w.isVisible());
        let reduce = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
        if !shown || reduce {
            self.snap();
            self.place();
            return;
        }
        if self.ivars().timer.borrow().is_some() {
            return;
        }
        self.ivars().tick.set(now());
        let strip = objc2::rc::Weak::from(self);
        let block = RcBlock::new(move |_: NonNull<NSTimer>| {
            if let Some(strip) = strip.load() {
                strip.step();
            }
        });
        let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(1.0 / 120.0, true, &block) };
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        *self.ivars().timer.borrow_mut() = Some(timer);
    }

    fn step(&self) {
        let time = now();
        let dt = (time - self.ivars().tick.replace(time)).min(0.05);
        let dragged = self.ivars().drag.get().filter(|d| d.moving).map(|d| d.at);
        let mut moving = false;
        for (index, tab) in self.ivars().tabs.borrow_mut().iter_mut().enumerate() {
            tab.x.step(dt, if Some(index) == dragged { FOLLOW } else { SETTLE });
            tab.width.step(dt, SETTLE);
            tab.shown.ramp(dt, FADE);
            moving |= !tab.x.settled() || !tab.width.settled() || !tab.shown.settled();
        }
        self.place();
        if !moving {
            self.stop();
        }
    }

    fn stop(&self) {
        if let Some(timer) = self.ivars().timer.borrow_mut().take() {
            timer.invalidate();
        }
    }

    fn set_hover(&self, hover: Option<usize>) {
        if self.ivars().hover.replace(hover) != hover {
            self.restyle();
        }
    }

    /// Lights the chosen tab and the one under the pointer, titles them,
    /// and shows the close buttons that show.
    fn restyle(&self) {
        let (chosen, closable, enabled) = {
            let model = self.ivars().shared.model.borrow();
            (model.chosen(), model.closable == Some(true), model.enabled())
        };
        let hover = self.ivars().hover.get().filter(|_| enabled);
        let dragging = self.ivars().drag.get().is_some_and(|d| d.moving);
        for (index, tab) in self.ivars().tabs.borrow_mut().iter_mut().enumerate() {
            let shown = chosen == Some(index);
            if let Some(glass) = &tab.glass {
                set_glass(glass, shown, hover == Some(index));
            }
            tab.label.setFont(Some(&title_font(shown)));
            tab.label.setTextColor(Some(&*if shown { NSColor::labelColor() } else { NSColor::secondaryLabelColor() }));
            tab.label.setEnabled(enabled);
            tab.close.setEnabled(enabled);
            tab.element.setAccessibilitySelected(shown);
            unsafe { tab.element.setAccessibilityValue(Some(&NSNumber::new_bool(shown))) };
            // Fading in and out, and not while a tab is dragged, as on
            // the system's bar.
            tab.shown.target = if closable && hover == Some(index) && !dragging { 1.0 } else { 0.0 };
        }
        self.chosen_in_front();
        self.animate();
        self.set_accessibility_children();
        self.place();
        self.setNeedsDisplay(true);
        self.noteFocusRingMaskChanged();
    }

    /// A close button under the pointer shows at once: its bezel drawn
    /// while it's still fading in would flicker.
    fn close_under(&self, point: NSPoint) {
        for tab in self.ivars().tabs.borrow_mut().iter_mut() {
            let frame = tab.close.convertRect_toView(tab.close.bounds(), Some(self));
            let inside = point.x >= frame.origin.x
                && point.x < frame.origin.x + frame.size.width
                && point.y >= frame.origin.y
                && point.y < frame.origin.y + frame.size.height;
            if inside && tab.shown.target > 0.0 {
                tab.shown.snap();
                tab.close.setAlphaValue(1.0);
            }
        }
    }

    /// The chosen tab over the others: glass shows only what's under it,
    /// and a dragged (chosen) tab's must go over the others' titles.
    fn chosen_in_front(&self) {
        let Some(chosen) = self.ivars().shared.model.borrow().chosen() else { return };
        let tabs = self.ivars().tabs.borrow();
        let Some(tab) = tabs.get(chosen) else { return };
        let track = &self.ivars().track;
        let front = track.subviews().lastObject().is_some_and(|v| std::ptr::eq(&*v, &*tab.view));
        if !front {
            track.addSubview_positioned_relativeTo(&tab.view, NSWindowOrderingMode::Above, None);
        }
    }

    /// Tags each close button with its tab's place.
    fn renumber(&self) {
        for (index, tab) in self.ivars().tabs.borrow().iter().enumerate() {
            tab.close.setTag(index as isize);
        }
    }

    /// Makes the tabs and buttons the model has: tabs whose titles stay
    /// keep their views, new ones grow in at their place, and closed ones
    /// go, the others closing up.
    fn rebuild(&self) {
        let mtm = self.mtm();
        let (titles, new_tab) = {
            let model = self.ivars().shared.model.borrow();
            (model.titles.clone(), model.new_tab == Some(true))
        };
        let has_plus = self.ivars().plus.borrow().is_some();
        if new_tab && !has_plus {
            let plus = self.make_plus(mtm);
            self.addSubview(&plus.0);
            *self.ivars().plus.borrow_mut() = Some(plus);
        } else if !new_tab && let Some((view, _)) = self.ivars().plus.borrow_mut().take() {
            view.removeFromSuperview();
        }
        let enabled = self.ivars().shared.model.borrow().enabled();
        if let Some((_, button)) = self.ivars().plus.borrow().as_ref() {
            button.setEnabled(enabled);
        }
        let old: Vec<String> = self.ivars().tabs.borrow().iter().map(|t| t.title.clone()).collect();
        if old != titles {
            // A drag's tabs are the model's already: it ends.
            self.ivars().drag.set(None);
            let plan = align(&old, &titles);
            let mut old_tabs: Vec<Option<Tab>> = self.ivars().tabs.take().into_iter().map(Some).collect();
            let (slots, _) = self.slots(titles.len());
            let mut tabs = Vec::with_capacity(titles.len());
            for (index, from) in plan.iter().enumerate() {
                let tab = from.and_then(|i| old_tabs[i].take()).unwrap_or_else(|| {
                    let mut tab = self.make_tab(mtm, index);
                    // Grows from nothing at its place.
                    tab.x = Spring::at(slots[index]);
                    tab.width = Spring::at(0.0);
                    tab
                });
                tabs.push(tab);
            }
            for tab in old_tabs.into_iter().flatten() {
                tab.view.removeFromSuperview();
            }
            for (tab, title) in tabs.iter_mut().zip(&titles) {
                tab.title = title.clone();
                tab.label.setStringValue(&ns(title));
                tab.element.setAccessibilityLabel(Some(&ns(title)));
            }
            *self.ivars().tabs.borrow_mut() = tabs;
            self.renumber();
            self.make_separators(mtm);
        }
        self.retarget();
        self.restyle();
        self.animate();
        self.setNeedsLayout(true);
    }

    fn make_tab(&self, mtm: MainThreadMarker, index: usize) -> Tab {
        let view = NSView::new(mtm);
        let glass = self.ivars().glass.then(|| {
            let glass = NSGlassEffectView::new(mtm);
            glass.setCornerRadius((HEIGHT - 2.0 * INSET) / 2.0);
            view.addSubview(&glass);
            Retained::into_super(glass)
        });
        let content = NSView::new(mtm);
        view.addSubview(&content);
        let label = NSTextField::labelWithString(&ns(""), mtm);
        label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        label.setAlignment(NSTextAlignment::Center);
        label.setFont(Some(&title_font(false)));
        // The tab reads as its title: the label isn't another element.
        label.setAccessibilityElement(false);
        content.addSubview(&label);
        let close = self.button(mtm, "xmark", &mitsuami_core::l10n::tr("mitsuami-tab-close", &[]));
        if let Some(target) = self.ivars().close_target.borrow().as_ref() {
            unsafe {
                close.setTarget(Some(target));
                close.setAction(Some(sel!(fire:)));
            }
        }
        close.setTag(index as isize);
        close.setAlphaValue(0.0);
        content.addSubview(&close);
        self.ivars().track.addSubview(&view);
        let shared = Rc::downgrade(&self.ivars().shared);
        let strip = objc2::rc::Weak::from(self);
        let element = TabElement::alloc(mtm).set_ivars(ElementIvars {
            // Tabs are by position: the close button's tag is this one's.
            press: Box::new({
                let close = close.clone();
                move || {
                    let (Some(shared), Some(strip)) = (shared.upgrade(), strip.load()) else { return };
                    if shared.model.borrow().enabled() {
                        choose(&shared, close.tag() as usize);
                        strip.restyle();
                    }
                }
            }),
        });
        let element: Retained<TabElement> = unsafe { msg_send![super(element), init] };
        element.setAccessibilityRole(Some(unsafe { NSAccessibilityRadioButtonRole }));
        element.setAccessibilitySubrole(Some(unsafe { NSAccessibilityTabButtonSubrole }));
        unsafe { element.setAccessibilityParent(Some(self)) };
        Tab {
            view,
            glass,
            content,
            label,
            close,
            element,
            title: String::new(),
            x: Spring::default(),
            width: Spring::default(),
            shown: Spring::default(),
        }
    }

    /// A separator between each two tabs, under them in the track.
    fn make_separators(&self, mtm: MainThreadMarker) {
        let wanted = self.ivars().tabs.borrow().len().saturating_sub(1);
        let mut separators = self.ivars().separators.borrow_mut();
        while separators.len() > wanted {
            separators.pop().unwrap().removeFromSuperview();
        }
        while separators.len() < wanted {
            let line = NSBox::new(mtm);
            line.setBoxType(NSBoxType::Separator);
            self.ivars().track.addSubview_positioned_relativeTo(&line, NSWindowOrderingMode::Below, None);
            separators.push(line);
        }
    }

    /// The new-tab button: round glass with the system's, or AppKit's
    /// round button.
    fn make_plus(&self, mtm: MainThreadMarker) -> (Retained<NSView>, Retained<NSButton>) {
        let button = self.button(mtm, "plus", &mitsuami_core::l10n::tr("mitsuami-tab-new", &[]));
        if let Some(target) = self.ivars().plus_target.borrow().as_ref() {
            unsafe {
                button.setTarget(Some(target));
                button.setAction(Some(sel!(fire:)));
            }
        }
        if !self.ivars().glass {
            button.setBezelStyle(NSBezelStyle::Circular);
            return (Retained::into_super(Retained::into_super(button.clone())), button);
        }
        button.setBordered(false);
        let glass = NSGlassEffectView::new(mtm);
        glass.setCornerRadius(PLUS / 2.0);
        button.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        button.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(PLUS, PLUS)));
        glass.setContentView(Some(&button));
        (Retained::into_super(glass), button)
    }

    /// A button showing an SF Symbol, named for assistive technology and
    /// its tooltip: a close button is AppKit's small round one, its bezel
    /// showing under the pointer, as the system's tab bar has it.
    fn button(&self, mtm: MainThreadMarker, symbol: &str, name: &str) -> Retained<NSButton> {
        let close = symbol == "xmark";
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns(symbol), Some(&ns(name)))
            .map(|image| {
                let (size, weight) =
                    if close { (10.0, unsafe { NSFontWeightRegular }) } else { (13.0, unsafe { NSFontWeightMedium }) };
                let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(size, weight);
                image.imageWithSymbolConfiguration(&config).unwrap_or(image)
            })
            .unwrap_or_default();
        let button = unsafe { NSButton::buttonWithImage_target_action(&image, None, None, mtm) };
        if close {
            // Its round bezel shows only under the pointer.
            button.setBezelStyle(NSBezelStyle::Circular);
            button.setControlSize(NSControlSize::Small);
            button.setShowsBorderOnlyWhileMouseInside(true);
            button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        }
        button.setToolTip(Some(&ns(name)));
        button.setAccessibilityLabel(Some(&ns(name)));
        button
    }

    /// Tabs, then their close buttons that show, then the new-tab button.
    fn set_accessibility_children(&self) {
        let tabs = self.ivars().tabs.borrow();
        let mut children: Vec<Retained<AnyObject>> =
            tabs.iter().map(|t| Retained::into_super(t.element.clone()).into()).collect();
        children.extend(
            tabs.iter()
                .filter(|t| t.shown.target > 0.0)
                .map(|t| Retained::into_super(Retained::into_super(Retained::into_super(t.close.clone()))).into()),
        );
        if let Some((_, plus)) = self.ivars().plus.borrow().as_ref() {
            children.push(Retained::into_super(Retained::into_super(Retained::into_super(plus.clone()))).into());
        }
        let array = NSArray::from_retained_slice(&children);
        unsafe { self.setAccessibilityChildren(Some(&array)) };
    }

    /// Puts the track, the tabs where they are now, their titles and close
    /// buttons, the separators between unlit tabs, and the new-tab button.
    fn place(&self) {
        let track = self.track_rect();
        self.ivars().track.setFrame(track);
        let rtl = self.right_to_left();
        let tabs = self.ivars().tabs.borrow();
        let chosen = self.ivars().shared.model.borrow().chosen();
        let hover = self.ivars().hover.get();
        let dragging = self.ivars().drag.get().is_some_and(|d| d.moving);
        for tab in tabs.iter() {
            let rect = self.tab_rect(tab);
            // In the track's coordinates.
            let local = NSRect::new(NSPoint::new(rect.origin.x - track.origin.x, INSET), rect.size);
            tab.view.setFrame(local);
            let size = local.size;
            if let Some(glass) = &tab.glass {
                glass.setFrame(NSRect::new(NSPoint::ZERO, size));
            }
            tab.content.setFrame(NSRect::new(NSPoint::ZERO, size));
            tab.close.setAlphaValue(tab.shown.value);
            let label_height = tab.label.intrinsicContentSize().height.max(0.0);
            // Room for the close button on the tab's leading side, and as
            // much on the other, so the title stays centred.
            let inset = CLOSE_INSET + CLOSE + 4.0;
            tab.label.setFrame(NSRect::new(
                NSPoint::new(inset, ((size.height - label_height) / 2.0).round()),
                NSSize::new((size.width - 2.0 * inset).max(0.0), label_height),
            ));
            let close_x = if rtl { size.width - CLOSE_INSET - CLOSE } else { CLOSE_INSET };
            tab.close.setFrame(NSRect::new(
                NSPoint::new(close_x, ((size.height - CLOSE) / 2.0).round()),
                NSSize::new(CLOSE, CLOSE),
            ));
            tab.element.setAccessibilityFrameInParentSpace(rect);
        }
        let lit = |i: usize| Some(i) == chosen || Some(i) == hover;
        for (index, line) in self.ivars().separators.borrow().iter().enumerate() {
            let (before, after) = (&tabs[index], &tabs[index + 1]);
            line.setHidden(dragging || lit(index) || lit(index + 1));
            let x = if rtl { track.size.width - after.x.value } else { before.x.value + before.width.value };
            line.setFrame(NSRect::new(
                NSPoint::new(x, ((HEIGHT - SEPARATOR) / 2.0).round()),
                NSSize::new(1.0, SEPARATOR),
            ));
        }
        if let Some((view, _)) = self.ivars().plus.borrow().as_ref() {
            let size = self.bounds().size;
            let x = if rtl { 0.0 } else { size.width - PLUS };
            view.setFrame(NSRect::new(NSPoint::new(x, ((size.height - PLUS) / 2.0).round()), NSSize::new(PLUS, PLUS)));
        }
        if !self.ivars().glass {
            self.setNeedsDisplay(true);
        }
        self.noteFocusRingMaskChanged();
    }

    fn dark(&self) -> bool {
        let candidates = unsafe { [NSAppearanceNameAqua, NSAppearanceNameDarkAqua] };
        let names = NSArray::from_slice(&candidates);
        self.effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names)
            .is_some_and(|name| &*name == unsafe { NSAppearanceNameDarkAqua })
    }

    /// Without the system's glass: the track, the chosen tab raised on it,
    /// a fainter pill under the pointer, in the colours the glass has in
    /// the title bar on macOS 26 (measured from captures of it).
    fn draw(&self) {
        if self.ivars().glass {
            return;
        }
        let dark = self.dark();
        let white = |alpha| NSColor::colorWithWhite_alpha(1.0, alpha);
        let (track, raised, edge, hovered) = if dark {
            (white(0.05), white(0.2), Some(white(0.12)), white(0.08))
        } else {
            (NSColor::colorWithWhite_alpha(0.0, 0.08), NSColor::colorWithWhite_alpha(0.98, 1.0), None, white(0.55))
        };
        track.setFill();
        pill(self.track_rect()).fill();
        let tabs = self.ivars().tabs.borrow();
        let chosen = self.ivars().shared.model.borrow().chosen();
        let enabled = self.ivars().shared.model.borrow().enabled();
        let hover = self.ivars().hover.get().filter(|_| enabled);
        if let Some(tab) = hover.filter(|h| Some(*h) != chosen).and_then(|i| tabs.get(i)) {
            hovered.setFill();
            pill(self.tab_rect(tab)).fill();
        }
        if let Some(tab) = chosen.and_then(|i| tabs.get(i)) {
            let rect = self.tab_rect(tab);
            // Raised: a soft shadow in light mode, a lit edge in dark.
            if let Some(context) = NSGraphicsContext::currentContext() {
                context.saveGraphicsState();
                if !dark {
                    let shadow = NSShadow::new();
                    shadow.setShadowColor(Some(&NSColor::colorWithWhite_alpha(0.0, 0.18)));
                    shadow.setShadowBlurRadius(2.0);
                    shadow.setShadowOffset(NSSize::new(0.0, -0.5));
                    shadow.set();
                }
                raised.setFill();
                pill(rect).fill();
                context.restoreGraphicsState();
            }
            if let Some(edge) = &edge {
                edge.setStroke();
                let path = pill(NSRect::new(
                    NSPoint::new(rect.origin.x + 0.5, rect.origin.y + 0.5),
                    NSSize::new(rect.size.width - 1.0, rect.size.height - 1.0),
                ));
                path.setLineWidth(1.0);
                path.stroke();
            }
        }
    }

    /// As wide as its titles with room for their close buttons, and the
    /// new-tab button; as high as the system's tab bar.
    fn natural_size(&self) -> Size {
        let tabs = self.ivars().tabs.borrow();
        let titles: f64 = tabs
            .iter()
            .map(|t| t.label.intrinsicContentSize().width.max(0.0) + 2.0 * (CLOSE_INSET + CLOSE + 4.0))
            .sum();
        let separators = tabs.len().saturating_sub(1) as f64;
        let plus = if self.ivars().shared.model.borrow().new_tab == Some(true) { PLUS + PLUS_GAP } else { 0.0 };
        Size::new((titles + separators + 2.0 * INSET + plus).ceil() as f32, HEIGHT as f32)
    }

    fn titles(&self) -> Vec<String> {
        self.ivars().tabs.borrow().iter().map(|t| t.label.stringValue().to_string()).collect()
    }

    fn has_close_buttons(&self) -> bool {
        // They show only under the pointer: there's one for each tab.
        let model = self.ivars().shared.model.borrow();
        model.closable == Some(true) && self.ivars().tabs.borrow().len() == model.titles.len()
    }

    fn has_plus(&self) -> bool {
        self.ivars().plus.borrow().is_some()
    }
}

fn pill(rect: NSRect) -> Retained<NSBezierPath> {
    let radius = rect.size.height / 2.0;
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius)
}

fn now() -> f64 {
    NSProcessInfo::processInfo().systemUptime()
}

/// Which tab each title is, from the titles the tabs show: for each new
/// title, the old tab it's (`None`: a tab opened there). Titles that
/// match keep their tabs; with more old ones left than new, an old one
/// before the next match closed, and with fewer, a new one opened before
/// it; otherwise a tab was renamed. Old tabs left over closed.
fn align(old: &[String], new: &[String]) -> Vec<Option<usize>> {
    let mut plan = Vec::with_capacity(new.len());
    let (mut i, mut j) = (0, 0);
    while j < new.len() {
        if i >= old.len() {
            plan.push(None);
            j += 1;
            continue;
        }
        if old[i] == new[j] {
            plan.push(Some(i));
            (i, j) = (i + 1, j + 1);
            continue;
        }
        let (old_left, new_left) = (old.len() - i, new.len() - j);
        if old_left > new_left
            && let Some(k) = old[i..].iter().take(old_left - new_left + 1).position(|t| *t == new[j])
        {
            i += k;
        } else if new_left > old_left
            && let Some(k) = new[j..].iter().take(new_left - old_left + 1).position(|t| *t == old[i])
        {
            plan.extend(std::iter::repeat_n(None, k));
            j += k;
        } else {
            plan.push(Some(i));
            (i, j) = (i + 1, j + 1);
        }
    }
    plan
}

/// A tab's title: the system tab bar's small font, the chosen tab's a
/// little heavier.
fn title_font(chosen: bool) -> Retained<NSFont> {
    let weight = if chosen { unsafe { NSFontWeightMedium } } else { unsafe { NSFontWeightRegular } };
    NSFont::systemFontOfSize_weight(NSFont::smallSystemFontSize(), weight)
}
