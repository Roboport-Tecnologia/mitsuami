//! Document tabs: a strip of tabs in the content, built from AppKit views
//! and drawn as Safari's tab bar and the window tab bar of macOS 26 are.
//!
//! macOS's own window tabs are the user's (Window > Merge All Windows):
//! each is a window, which can't share one content, so every style is
//! this strip.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use mitsuami_core::{DocumentTabsStyle, EventSink, EventValue, NodeId, Size, UiEvent, moved_index};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityElement, NSAccessibilityRadioButtonRole, NSAccessibilityTabButtonSubrole,
    NSAccessibilityTabGroupRole, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBezelStyle, NSBezierPath, NSButton, NSColor, NSEvent, NSFont, NSFontWeightMedium, NSFontWeightRegular,
    NSGraphicsContext, NSImage, NSImageSymbolConfiguration, NSLineBreakMode, NSShadow, NSTextAlignment, NSTextField,
    NSTrackingArea, NSTrackingAreaOptions, NSUserInterfaceLayoutDirection, NSView,
};
use objc2_foundation::{NSArray, NSNumber, NSPoint, NSRect, NSSize, NSString};

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
    pub(crate) strip: Retained<TabStrip>,
    shared: Rc<Shared>,
}

impl DocumentTabs {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> DocumentTabs {
        let shared = Rc::new(Shared { id, events, model: RefCell::default() });
        let strip = TabStrip::new(mtm, shared.clone());
        strip.rebuild();
        DocumentTabs { strip, shared }
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

    /// Kept for the mirror: every style is the strip.
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

    pub(crate) fn natural_size(&self) -> Size {
        self.strip.natural_size()
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

/// The geometry of macOS 26's window tab bar, which Safari's shares: a
/// capsule track, tabs inset in it with a point between them for their
/// separator, a close button on each tab's leading side, and a round
/// new-tab button after the track.
const HEIGHT: f64 = 28.0;
const INSET: f64 = 2.0;
const SEPARATOR: f64 = 18.0;
const CLOSE: f64 = 16.0;
const CLOSE_INSET: f64 = 5.0;
const PLUS: f64 = 28.0;
const PLUS_GAP: f64 = 4.0;

/// One tab of the strip: its title, and its close button.
struct Tab {
    label: Retained<NSTextField>,
    close: Retained<NSButton>,
    element: Retained<TabElement>,
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
    tabs: RefCell<Vec<Tab>>,
    plus: RefCell<Option<Retained<NSButton>>>,
    /// The close buttons' and the new-tab button's targets, which buttons
    /// only hold weak references to.
    close_target: RefCell<Option<Retained<ClosureTarget>>>,
    plus_target: RefCell<Option<Retained<ClosureTarget>>>,
    hover: Cell<Option<usize>>,
    /// A press on a tab: where it started, and where the tab is now while
    /// it's dragged.
    drag: Cell<Option<(usize, usize)>>,
    tracking: RefCell<Option<Retained<NSTrackingArea>>>,
}

define_class!(
    /// A strip of tabs in the content, as Safari's: tabs sharing its width
    /// equally, the chosen one lit, a close button on the tab under the
    /// pointer, a new-tab button at the end. Tabs are picked on a press,
    /// dragged to move them, and closed with a middle click too.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = StripIvars]
    pub(crate) struct TabStrip;

    impl TabStrip {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(layout))]
        fn layout(&self) {
            let _: () = unsafe { msg_send![super(self), layout] };
            self.place();
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }

        /// Its buttons take their clicks; the rest of it takes the others
        /// (a tab's title is a label, which would).
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let hit: Option<Retained<NSView>> = unsafe { msg_send![super(self), hitTest: point] };
            match hit {
                Some(view) if view.downcast_ref::<NSButton>().is_some() => Retained::into_raw(view),
                Some(_) => self as *const TabStrip as *mut NSView,
                None => std::ptr::null_mut(),
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let Some(index) = self.tab_at(self.point(event)) else { return };
            if !self.ivars().shared.model.borrow().enabled() {
                return;
            }
            self.ivars().drag.set(Some((index, index)));
            choose(&self.ivars().shared, index);
            self.rebuild();
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let Some((from, at)) = self.ivars().drag.get() else { return };
            if !self.ivars().shared.model.borrow().reorderable() {
                return;
            }
            let count = self.ivars().shared.model.borrow().titles.len();
            let Some(to) = self.tab_at(self.point(event)).or_else(|| {
                // Past either end: the end it's past.
                let x = self.point(event).x;
                let leading = if self.right_to_left() { x > self.bounds().size.width } else { x < 0.0 };
                Some(if leading { 0 } else { count.saturating_sub(1) })
            }) else {
                return;
            };
            if to != at {
                self.ivars().shared.model.borrow_mut().move_tab(at, to);
                self.ivars().drag.set(Some((from, to)));
                self.rebuild();
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            if let Some((from, to)) = self.ivars().drag.take()
                && from != to
            {
                self.ivars().shared.emit(UiEvent::TabMoved { from, to });
            }
        }

        /// A middle click closes a tab, as in Safari.
        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            let closable = self.ivars().shared.model.borrow().closable();
            if event.buttonNumber() == 2
                && closable
                && let Some(index) = self.tab_at(self.point(event))
            {
                self.ivars().shared.emit(UiEvent::TabCloseRequested(index));
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.set_hover(self.tab_at(self.point(event)));
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
                self.rebuild();
            }
        }

        #[unsafe(method(focusRingMaskBounds))]
        fn focus_ring_mask_bounds(&self) -> NSRect {
            self.chosen_rect().unwrap_or_else(zero_rect)
        }

        #[unsafe(method(drawFocusRingMask))]
        fn draw_focus_ring_mask(&self) {
            if let Some(rect) = self.chosen_rect() {
                let radius = rect.size.height / 2.0;
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius).fill();
            }
        }

        /// Its colours are painted: light and dark differ.
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeEffectiveAppearance] };
            self.setNeedsDisplay(true);
        }
    }
);

impl TabStrip {
    fn new(mtm: MainThreadMarker, shared: Rc<Shared>) -> Retained<TabStrip> {
        let this = TabStrip::alloc(mtm).set_ivars(StripIvars {
            shared,
            tabs: RefCell::default(),
            plus: RefCell::default(),
            close_target: RefCell::default(),
            plus_target: RefCell::default(),
            hover: Cell::new(None),
            drag: Cell::new(None),
            tracking: RefCell::default(),
        });
        let strip: Retained<TabStrip> = unsafe { msg_send![super(this), initWithFrame: zero_rect()] };
        strip.setAccessibilityElement(true);
        strip.setAccessibilityRole(Some(unsafe { NSAccessibilityTabGroupRole }));
        let weak = Weak::clone(&Rc::downgrade(&strip.ivars().shared));
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

    fn point(&self, event: &NSEvent) -> NSPoint {
        self.convertPoint_fromView(event.locationInWindow(), None)
    }

    fn right_to_left(&self) -> bool {
        self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft
    }

    /// Mirrors a rect across the strip in right-to-left text.
    fn mirrored(&self, rect: NSRect) -> NSRect {
        if !self.right_to_left() {
            return rect;
        }
        let width = self.bounds().size.width;
        NSRect::new(NSPoint::new(width - rect.origin.x - rect.size.width, rect.origin.y), rect.size)
    }

    fn plus_width(&self) -> f64 {
        if self.ivars().plus.borrow().is_some() { PLUS + PLUS_GAP } else { 0.0 }
    }

    /// The track the tabs are in: the strip less the new-tab button.
    fn track_rect(&self) -> NSRect {
        let size = self.bounds().size;
        self.mirrored(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new((size.width - self.plus_width()).max(0.0), size.height),
        ))
    }

    /// Where each tab is: the track's inside, shared equally, a point
    /// between each for its separator.
    fn tab_rects(&self) -> Vec<NSRect> {
        let count = self.ivars().shared.model.borrow().titles.len();
        let size = self.bounds().size;
        let inside = size.width - self.plus_width() - 2.0 * INSET;
        let width = if count == 0 { 0.0 } else { ((inside - (count - 1) as f64) / count as f64).max(0.0) };
        let height = (size.height - 2.0 * INSET).max(0.0);
        (0..count)
            .map(|i| {
                let x = INSET + i as f64 * (width + 1.0);
                self.mirrored(NSRect::new(NSPoint::new(x, INSET), NSSize::new(width, height)))
            })
            .collect()
    }

    fn plus_rect(&self) -> NSRect {
        let size = self.bounds().size;
        let y = ((size.height - PLUS) / 2.0).round();
        self.mirrored(NSRect::new(NSPoint::new(size.width - PLUS, y), NSSize::new(PLUS, PLUS)))
    }

    fn tab_at(&self, point: NSPoint) -> Option<usize> {
        self.tab_rects().iter().position(|r| {
            point.x >= r.origin.x
                && point.x < r.origin.x + r.size.width
                && point.y >= r.origin.y
                && point.y < r.origin.y + r.size.height
        })
    }

    fn chosen_rect(&self) -> Option<NSRect> {
        let chosen = self.ivars().shared.model.borrow().chosen()?;
        self.tab_rects().get(chosen).copied()
    }

    fn set_hover(&self, hover: Option<usize>) {
        if self.ivars().hover.replace(hover) != hover {
            self.show_close_buttons();
            self.setNeedsDisplay(true);
        }
    }

    /// A tab's close button shows while the pointer is over the tab, as on
    /// the system's tab bar.
    fn show_close_buttons(&self) {
        let closable = self.ivars().shared.model.borrow().closable == Some(true);
        let hover = self.ivars().hover.get();
        for (index, tab) in self.ivars().tabs.borrow().iter().enumerate() {
            tab.close.setHidden(!closable || hover != Some(index));
        }
    }

    /// Makes the tabs and buttons the model has, titled from it.
    fn rebuild(&self) {
        let mtm = self.mtm();
        let (titles, chosen, new_tab, enabled) = {
            let model = self.ivars().shared.model.borrow();
            (model.titles.clone(), model.chosen(), model.new_tab == Some(true), model.enabled())
        };
        {
            let mut tabs = self.ivars().tabs.borrow_mut();
            while tabs.len() > titles.len() {
                let tab = tabs.pop().unwrap();
                tab.label.removeFromSuperview();
                tab.close.removeFromSuperview();
            }
            while tabs.len() < titles.len() {
                let index = tabs.len();
                tabs.push(self.make_tab(mtm, index));
            }
            for (index, (tab, title)) in tabs.iter().zip(&titles).enumerate() {
                let shown = chosen == Some(index);
                tab.label.setStringValue(&ns(title));
                tab.label.setFont(Some(&title_font(shown)));
                tab.label.setTextColor(Some(&*if shown {
                    NSColor::labelColor()
                } else {
                    NSColor::secondaryLabelColor()
                }));
                tab.label.setEnabled(enabled);
                tab.close.setTag(index as isize);
                tab.close.setEnabled(enabled);
                tab.element.setAccessibilityLabel(Some(&ns(title)));
                tab.element.setAccessibilitySelected(shown);
                unsafe { tab.element.setAccessibilityValue(Some(&NSNumber::new_bool(shown))) };
            }
        }
        let has_plus = self.ivars().plus.borrow().is_some();
        if new_tab && !has_plus {
            let button = self.button(mtm, "plus", &mitsuami_core::l10n::tr("mitsuami-tab-new", &[]));
            if let Some(target) = self.ivars().plus_target.borrow().as_ref() {
                unsafe {
                    button.setTarget(Some(target));
                    button.setAction(Some(sel!(fire:)));
                }
            }
            self.addSubview(&button);
            *self.ivars().plus.borrow_mut() = Some(button);
        } else if !new_tab && let Some(button) = self.ivars().plus.borrow_mut().take() {
            button.removeFromSuperview();
        }
        if let Some(plus) = self.ivars().plus.borrow().as_ref() {
            plus.setEnabled(enabled);
        }
        self.show_close_buttons();
        self.set_accessibility_children();
        self.setNeedsLayout(true);
        self.setNeedsDisplay(true);
        self.noteFocusRingMaskChanged();
    }

    fn make_tab(&self, mtm: MainThreadMarker, index: usize) -> Tab {
        let label = NSTextField::labelWithString(&ns(""), mtm);
        label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        label.setAlignment(NSTextAlignment::Center);
        label.setFont(Some(&title_font(false)));
        // The tab reads as its title: the label isn't another element.
        label.setAccessibilityElement(false);
        self.addSubview(&label);
        let close = self.button(mtm, "xmark", &mitsuami_core::l10n::tr("mitsuami-tab-close", &[]));
        if let Some(target) = self.ivars().close_target.borrow().as_ref() {
            unsafe {
                close.setTarget(Some(target));
                close.setAction(Some(sel!(fire:)));
            }
        }
        close.setTag(index as isize);
        close.setHidden(true);
        self.addSubview(&close);
        let shared = Rc::downgrade(&self.ivars().shared);
        let strip = objc2::rc::Weak::from(self);
        let element = TabElement::alloc(mtm).set_ivars(ElementIvars {
            // Tabs are by position: the one at `index` is this one's.
            press: Box::new(move || {
                let (Some(shared), Some(strip)) = (shared.upgrade(), strip.load()) else { return };
                if shared.model.borrow().enabled() {
                    choose(&shared, index);
                    strip.rebuild();
                }
            }),
        });
        let element: Retained<TabElement> = unsafe { msg_send![super(element), init] };
        element.setAccessibilityRole(Some(unsafe { NSAccessibilityRadioButtonRole }));
        element.setAccessibilitySubrole(Some(unsafe { NSAccessibilityTabButtonSubrole }));
        unsafe { element.setAccessibilityParent(Some(self)) };
        Tab { label, close, element }
    }

    /// A button showing an SF Symbol, named for assistive technology and
    /// its tooltip: a close button's borderless, the new-tab button round,
    /// as the system's tab bar has them.
    fn button(&self, mtm: MainThreadMarker, symbol: &str, name: &str) -> Retained<NSButton> {
        let close = symbol == "xmark";
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns(symbol), Some(&ns(name)))
            .map(|image| {
                let (size, weight) = if close {
                    (9.0, unsafe { objc2_app_kit::NSFontWeightSemibold })
                } else {
                    (13.0, unsafe { NSFontWeightMedium })
                };
                let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(size, weight);
                image.imageWithSymbolConfiguration(&config).unwrap_or(image)
            })
            .unwrap_or_default();
        let button = unsafe { NSButton::buttonWithImage_target_action(&image, None, None, mtm) };
        if close {
            button.setBordered(false);
            button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        } else {
            button.setBezelStyle(NSBezelStyle::Circular);
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
                .filter(|t| !t.close.isHidden())
                .map(|t| Retained::into_super(Retained::into_super(Retained::into_super(t.close.clone()))).into()),
        );
        if let Some(plus) = self.ivars().plus.borrow().as_ref() {
            children.push(Retained::into_super(Retained::into_super(Retained::into_super(plus.clone()))).into());
        }
        let array = NSArray::from_retained_slice(&children);
        unsafe { self.setAccessibilityChildren(Some(&array)) };
    }

    /// Places each tab's title and close button, and the new-tab button.
    fn place(&self) {
        let rects = self.tab_rects();
        let rtl = self.right_to_left();
        for (tab, rect) in self.ivars().tabs.borrow().iter().zip(&rects) {
            let label_height = tab.label.intrinsicContentSize().height.max(0.0);
            // Room for the close button on the tab's leading side, and as
            // much on the other, so the title stays centred.
            let inset = CLOSE_INSET + CLOSE + 4.0;
            tab.label.setFrame(NSRect::new(
                NSPoint::new(rect.origin.x + inset, rect.origin.y + ((rect.size.height - label_height) / 2.0).round()),
                NSSize::new((rect.size.width - 2.0 * inset).max(0.0), label_height),
            ));
            let close_x =
                if rtl { rect.origin.x + rect.size.width - CLOSE_INSET - CLOSE } else { rect.origin.x + CLOSE_INSET };
            tab.close.setFrame(NSRect::new(
                NSPoint::new(close_x, rect.origin.y + ((rect.size.height - CLOSE) / 2.0).round()),
                NSSize::new(CLOSE, CLOSE),
            ));
            tab.element.setAccessibilityFrameInParentSpace(*rect);
        }
        if let Some(plus) = self.ivars().plus.borrow().as_ref() {
            plus.setFrame(self.plus_rect());
        }
    }

    fn dark(&self) -> bool {
        let candidates = unsafe { [NSAppearanceNameAqua, NSAppearanceNameDarkAqua] };
        let names = NSArray::from_slice(&candidates);
        self.effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names)
            .is_some_and(|name| &*name == unsafe { NSAppearanceNameDarkAqua })
    }

    /// The track, the chosen tab raised on it as a pill, a fainter pill
    /// under the pointer, and separators between the other tabs: the
    /// system tab bar's look, measured from it on macOS 26. Its track and
    /// pill are Liquid Glass, which in the content would take the content's
    /// colour, so the strip paints the colours glass has in the title bar.
    fn draw(&self) {
        let dark = self.dark();
        let white = |alpha| NSColor::colorWithWhite_alpha(1.0, alpha);
        let (track, raised, edge, hovered) = if dark {
            (white(0.05), white(0.2), Some(white(0.12)), white(0.08))
        } else {
            (NSColor::colorWithWhite_alpha(0.0, 0.08), NSColor::colorWithWhite_alpha(0.98, 1.0), None, white(0.55))
        };
        let pill = |rect: NSRect| {
            let radius = rect.size.height / 2.0;
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius)
        };
        let enabled = self.ivars().shared.model.borrow().enabled();
        track.setFill();
        pill(self.track_rect()).fill();
        let rects = self.tab_rects();
        let chosen = self.ivars().shared.model.borrow().chosen();
        let hover = self.ivars().hover.get().filter(|_| enabled);
        if let Some(rect) = hover.filter(|h| Some(*h) != chosen).and_then(|i| rects.get(i)) {
            hovered.setFill();
            pill(*rect).fill();
        }
        if let Some(rect) = chosen.and_then(|i| rects.get(i)) {
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
                pill(*rect).fill();
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
        // Between two tabs, unless either is lit.
        NSColor::separatorColor().setFill();
        let lit = |i: usize| Some(i) == chosen || Some(i) == hover;
        for (index, rect) in rects.iter().enumerate().skip(1) {
            if lit(index) || lit(index - 1) {
                continue;
            }
            let x = if self.right_to_left() { rect.origin.x + rect.size.width } else { rect.origin.x - 1.0 };
            let y = rect.origin.y + ((rect.size.height - SEPARATOR) / 2.0).round();
            NSBezierPath::fillRect(NSRect::new(NSPoint::new(x, y), NSSize::new(1.0, SEPARATOR)));
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
        self.ivars().plus.borrow().as_ref().is_some_and(|p| !p.isHidden())
    }
}

/// A tab's title: the system tab bar's small font, the chosen tab's a
/// little heavier.
fn title_font(chosen: bool) -> Retained<NSFont> {
    let weight = if chosen { unsafe { NSFontWeightMedium } } else { unsafe { NSFontWeightRegular } };
    NSFont::systemFontOfSize_weight(NSFont::smallSystemFontSize(), weight)
}
