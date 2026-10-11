//! Document tabs, two ways, the app's choice: AppKit's own window tabs
//! (each tab an `NSWindow` of a tab group, as Finder's and Terminal's
//! are), or a strip of tabs in the content, built from AppKit views as
//! Safari builds its own.
//!
//! The app has one content for every tab (a terminal draws its sessions
//! on one surface), so with window tabs the node's window is always the
//! chosen tab, and the others are empty placeholder windows in its group.
//! Picking a placeholder puts the window in its place: placeholders move
//! across the window until it's where the tab picked was, and every tab
//! takes its title from where it is. The window the core knows (its
//! toolbar, sheets, full screen, sidebar) stays the one shown.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::{Rc, Weak};

use block2::RcBlock;
use mitsuami_core::{DocumentTabsStyle, EventSink, EventValue, NodeId, Size, UiEvent, moved_index};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityElement, NSAccessibilityRadioButtonRole, NSAccessibilityTabButtonSubrole,
    NSAccessibilityTabGroupRole, NSAppearanceCustomization, NSBackingStoreType, NSBezierPath, NSButton, NSColor,
    NSEvent, NSFont, NSImage, NSImageSymbolConfiguration, NSLineBreakMode, NSResponder, NSTextAlignment, NSTextField,
    NSTrackingArea, NSTrackingAreaOptions, NSUserInterfaceLayoutDirection, NSView, NSWindow, NSWindowDelegate,
    NSWindowStyleMask, NSWindowTabGroup,
};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopCommonModes};
use objc2_foundation::{
    NSArray, NSKeyValueObservingOptions, NSNotification, NSNumber, NSObjectNSKeyValueObserverRegistration, NSPoint,
    NSRect, NSSize, NSString,
};

use crate::classes::{ClosureTarget, WindowDelegate, zero_rect};

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

/// What the core gave, which both ways show.
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

    /// Window tabs: macOS's own, unless the app asked for a strip.
    fn window_tabs(&self) -> bool {
        self.style != Some(DocumentTabsStyle::Strip)
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
    window: RefCell<Option<Rc<WindowTabs>>>,
}

impl DocumentTabs {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> DocumentTabs {
        let shared = Rc::new(Shared { id, events, model: RefCell::default() });
        let strip = TabStrip::new(mtm, shared.clone());
        let tabs = DocumentTabs { strip, shared, window: RefCell::default() };
        tabs.refresh();
        tabs
    }

    fn change(&self, change: impl FnOnce(&mut Model)) {
        change(&mut self.shared.model.borrow_mut());
        self.refresh();
    }

    /// Both ways show the model again. The strip is hidden while the tabs
    /// are the window's.
    fn refresh(&self) {
        let window_tabs = self.shared.model.borrow().window_tabs();
        self.strip.setHidden(window_tabs);
        self.strip.rebuild();
        if let Some(window) = self.window.borrow().clone() {
            window.sync();
        }
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

    pub(crate) fn set_style(&self, style: DocumentTabsStyle) {
        self.change(|m| m.style = Some(style));
    }

    pub(crate) fn set_enabled(&self, on: bool) {
        self.change(|m| m.enabled = Some(on));
    }

    pub(crate) fn enabled(&self) -> bool {
        self.shared.model.borrow().enabled()
    }

    pub(crate) fn window_tabs(&self) -> bool {
        self.shared.model.borrow().window_tabs()
    }

    /// The window's tabs are made once it's on screen (a hidden window's
    /// tab group would order its tabs in), and taken down when the app
    /// asks for a strip or the node leaves the window. `window`: the
    /// node's window and its delegate, while it's in one.
    pub(crate) fn sync_window(&self, mtm: MainThreadMarker, window: Option<(&NSWindow, &WindowDelegate)>) {
        let wanted = window.filter(|(w, _)| self.window_tabs() && w.isVisible());
        let current = self.window.borrow().clone();
        match (current, wanted) {
            (Some(tabs), Some((w, _))) if std::ptr::eq(&*tabs.window, w) => {}
            (current, wanted) => {
                if let Some(tabs) = current {
                    tabs.take_down();
                }
                let made = wanted.map(|(w, delegate)| WindowTabs::new(mtm, self.shared.clone(), w, delegate));
                *self.window.borrow_mut() = made.clone();
                if let Some(tabs) = made {
                    tabs.sync();
                }
            }
        }
    }

    /// Takes the window's tabs down: the node is going.
    pub(crate) fn detach(&self) {
        if let Some(tabs) = self.window.borrow_mut().take() {
            tabs.take_down();
        }
    }

    /// The titles as the tabs show them: the window's tabs', in their
    /// group's order, once they're made.
    pub(crate) fn titles(&self) -> Vec<String> {
        match self.window.borrow().as_ref() {
            Some(tabs) => tabs.titles(),
            None if !self.window_tabs() => self.strip.titles(),
            None => self.shared.model.borrow().titles.clone(),
        }
    }

    /// The tab chosen: where the window is in its group, once its tabs are
    /// made.
    pub(crate) fn selected(&self) -> Option<usize> {
        match self.window.borrow().as_ref() {
            Some(tabs) => tabs.selected(),
            None => self.shared.model.borrow().chosen(),
        }
    }

    pub(crate) fn closable(&self) -> Option<bool> {
        let model = self.shared.model.borrow();
        let given = model.closable?;
        // The strip's close buttons, or the app's choice: window tabs
        // always have theirs.
        Some(if model.window_tabs() { given } else { self.strip.has_close_buttons() })
    }

    pub(crate) fn reorderable(&self) -> Option<bool> {
        self.shared.model.borrow().reorderable
    }

    pub(crate) fn new_tab(&self) -> Option<bool> {
        let model = self.shared.model.borrow();
        let given = model.new_tab?;
        Some(if model.window_tabs() { given } else { self.strip.has_plus() })
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
        self.refresh();
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

    /// Moves a tab as a drag does: the tabs show it there before the app
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
        self.refresh();
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

    /// The strip's natural size; nothing for the window's tabs, which are
    /// its chrome.
    pub(crate) fn natural_size(&self) -> Size {
        if self.window_tabs() { Size::ZERO } else { self.strip.natural_size() }
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

// ------------------------------------------------------------ window tabs

pub(crate) struct ObserverIvars {
    handler: Box<dyn Fn()>,
}

define_class!(
    /// KVO on the tab group's `windows`: a tab dragged elsewhere among
    /// them, out to a window of its own, or a window merged in.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ObserverIvars]
    pub(crate) struct TabGroupObserver;

    impl TabGroupObserver {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(
            &self,
            _key_path: Option<&NSString>,
            _object: Option<&AnyObject>,
            _change: Option<&AnyObject>,
            _context: *mut c_void,
        ) {
            (self.ivars().handler)();
        }
    }

    unsafe impl NSObjectProtocol for TabGroupObserver {}
);

pub(crate) struct PlaceholderIvars {
    close: Box<dyn Fn(&NSWindow)>,
    key: Box<dyn Fn(&NSWindow)>,
}

define_class!(
    /// The placeholder windows' delegate: closing one asks the app to
    /// close its tab, and one becoming key (its tab picked, Control-Tab,
    /// the Window menu) puts the window in its place.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PlaceholderIvars]
    pub(crate) struct PlaceholderDelegate;

    unsafe impl NSObjectProtocol for PlaceholderDelegate {}

    unsafe impl NSWindowDelegate for PlaceholderDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, sender: &NSWindow) -> bool {
            (self.ivars().close)(sender);
            false
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, notification: &NSNotification) {
            if let Some(window) = notification.object().and_then(|o| o.downcast::<NSWindow>().ok()) {
                (self.ivars().key)(&window);
            }
        }
    }
);

pub(crate) struct ResponderIvars {
    shared: Weak<Shared>,
}

define_class!(
    /// The tab bar's new-tab button: AppKit shows one while something in
    /// the window's responder chain answers `newWindowForTab:`, so this
    /// answers only while the app asked for the button.
    #[unsafe(super(NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ResponderIvars]
    pub(crate) struct NewTabResponder;

    impl NewTabResponder {
        #[unsafe(method(newWindowForTab:))]
        fn new_window_for_tab(&self, _sender: Option<&AnyObject>) {
            if let Some(shared) = self.ivars().shared.upgrade()
                && shared.model.borrow().new_tab()
            {
                shared.emit(UiEvent::NewTabRequested);
            }
        }

        #[unsafe(method(respondsToSelector:))]
        fn responds_to_selector(&self, selector: Sel) -> Bool {
            if selector == sel!(newWindowForTab:) {
                return Bool::new(self.ivars().shared.upgrade().is_some_and(|s| s.model.borrow().new_tab()));
            }
            unsafe { msg_send![super(self), respondsToSelector: selector] }
        }
    }
);

const WINDOWS: &str = "windows";

/// The node's window, with placeholder windows for the other tabs in its
/// tab group.
pub(crate) struct WindowTabs {
    mtm: MainThreadMarker,
    shared: Rc<Shared>,
    window: Retained<NSWindow>,
    delegate: Retained<WindowDelegate>,
    group: Retained<NSWindowTabGroup>,
    placeholders: RefCell<Vec<Retained<NSWindow>>>,
    placeholder_delegate: Retained<PlaceholderDelegate>,
    observer: Retained<TabGroupObserver>,
    responder: Retained<NewTabResponder>,
    /// The window's next responder before ours went in.
    next_responder: Option<Retained<NSResponder>>,
    /// The window's tabbing identifier before it got the node's own.
    identifier: Retained<NSString>,
    /// The tab group's windows, by address, as the backend last left
    /// them: what a change the user made is told from.
    order: RefCell<Vec<usize>>,
    /// Set while the backend changes the group itself.
    muted: Cell<bool>,
    /// A look at the group is due on the next run-loop turn.
    pending: Cell<bool>,
}

fn addr(window: &NSWindow) -> usize {
    window as *const NSWindow as usize
}

impl WindowTabs {
    fn new(mtm: MainThreadMarker, shared: Rc<Shared>, window: &NSWindow, delegate: &WindowDelegate) -> Rc<WindowTabs> {
        let identifier = window.tabbingIdentifier();
        // The node's own: the user's "Prefer tabs" and Merge All Windows
        // don't bring the app's other windows in, nor take these out.
        window.setTabbingIdentifier(&ns(&format!("mitsuami.document-tabs.{}", shared.id.raw())));
        let group = window.tabGroup().expect("a window's tab group is made when asked for");
        let tabs = Rc::new_cyclic(|this: &Weak<WindowTabs>| {
            let (closing, keyed, observed) = (this.clone(), this.clone(), this.clone());
            let placeholder_delegate = PlaceholderDelegate::alloc(mtm).set_ivars(PlaceholderIvars {
                close: Box::new(move |placeholder| {
                    if let Some(tabs) = closing.upgrade() {
                        tabs.close_placeholder(placeholder);
                    }
                }),
                key: Box::new(move |placeholder| {
                    if let Some(tabs) = keyed.upgrade() {
                        tabs.picked(placeholder);
                    }
                }),
            });
            let placeholder_delegate: Retained<PlaceholderDelegate> =
                unsafe { msg_send![super(placeholder_delegate), init] };
            let observer = TabGroupObserver::alloc(mtm).set_ivars(ObserverIvars {
                handler: Box::new(move || {
                    if let Some(tabs) = observed.upgrade() {
                        tabs.windows_changed();
                    }
                }),
            });
            let observer: Retained<TabGroupObserver> = unsafe { msg_send![super(observer), init] };
            let responder = NewTabResponder::alloc(mtm).set_ivars(ResponderIvars { shared: Rc::downgrade(&shared) });
            let responder: Retained<NewTabResponder> = unsafe { msg_send![super(responder), init] };
            let next_responder = unsafe { window.nextResponder() };
            unsafe {
                responder.setNextResponder(next_responder.as_deref());
                window.setNextResponder(Some(&responder));
            }
            WindowTabs {
                mtm,
                shared,
                window: window.retain(),
                delegate: delegate.retain(),
                group: group.clone(),
                placeholders: RefCell::default(),
                placeholder_delegate,
                observer,
                responder,
                next_responder,
                identifier,
                order: RefCell::default(),
                muted: Cell::new(false),
                pending: Cell::new(false),
            }
        });
        unsafe {
            group.addObserver_forKeyPath_options_context(
                &tabs.observer,
                &ns(WINDOWS),
                NSKeyValueObservingOptions::New,
                std::ptr::null_mut(),
            )
        };
        // The window's own close (its title bar's button, its tab's, ⌘W)
        // closes the chosen tab while there are others, as a window tab's
        // does; with one tab, or tabs the app doesn't let close, it's the
        // window's.
        let shared = Rc::downgrade(&tabs.shared);
        delegate.set_close_tab(Some(Rc::new(move || {
            let Some(shared) = shared.upgrade() else { return false };
            let model = shared.model.borrow();
            let chosen = model.chosen();
            if !model.closable() || model.titles.len() < 2 {
                return false;
            }
            drop(model);
            if let Some(index) = chosen {
                shared.emit(UiEvent::TabCloseRequested(index));
            }
            true
        })));
        tabs
    }

    /// Runs a change the user didn't make, without reporting it.
    fn quietly(&self, change: impl FnOnce()) {
        let was = self.muted.replace(true);
        change();
        self.muted.set(was);
    }

    fn windows(&self) -> Vec<Retained<NSWindow>> {
        self.group.windows().to_vec()
    }

    fn index_of(&self, window: &NSWindow) -> Option<usize> {
        self.windows().iter().position(|w| addr(w) == addr(window))
    }

    fn is_ours(&self, window: &NSWindow) -> bool {
        addr(window) == addr(&self.window) || self.placeholders.borrow().iter().any(|p| addr(p) == addr(window))
    }

    /// A placeholder: an empty window set up as the node's, so the tab bar
    /// and title bar look the same whichever is selected.
    fn placeholder(&self) -> Retained<NSWindow> {
        let window = &self.window;
        let mask = window.styleMask() & !NSWindowStyleMask::FullScreen;
        let content = window.contentRectForFrameRect(window.frame());
        let placeholder = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(self.mtm),
                content,
                mask,
                NSBackingStoreType::Buffered,
                true,
            )
        };
        unsafe { placeholder.setReleasedWhenClosed(false) };
        placeholder.setTabbingIdentifier(&window.tabbingIdentifier());
        placeholder.setTitlebarAppearsTransparent(window.titlebarAppearsTransparent());
        placeholder.setToolbarStyle(window.toolbarStyle());
        placeholder.setAppearance(window.appearance().as_deref());
        placeholder.setDelegate(Some(ProtocolObject::from_ref(&*self.placeholder_delegate)));
        unsafe { placeholder.setNextResponder(Some(&self.responder)) };
        placeholder
    }

    /// The group shows the model: a window per tab, the node's at the
    /// chosen tab, each tab titled from where it is.
    fn sync(&self) {
        let (count, chosen) = {
            let model = self.shared.model.borrow();
            (model.titles.len(), model.chosen())
        };
        self.quietly(|| {
            if self.index_of(&self.window).is_none() {
                self.group.insertWindow_atIndex(&self.window, 0);
            }
            let wanted = count.saturating_sub(1);
            while self.placeholders.borrow().len() < wanted {
                let placeholder = self.placeholder();
                self.group.addWindow(&placeholder);
                self.placeholders.borrow_mut().push(placeholder);
            }
            while self.placeholders.borrow().len() > wanted {
                let placeholder = self.placeholders.borrow_mut().pop().unwrap();
                self.drop_placeholder(&placeholder);
            }
            self.place(chosen.unwrap_or(0));
            self.title_tabs();
            self.group.setSelectedWindow(Some(&self.window));
            self.remember_order();
        });
    }

    fn drop_placeholder(&self, placeholder: &NSWindow) {
        self.group.removeWindow(placeholder);
        placeholder.setDelegate(None);
        placeholder.orderOut(None);
        placeholder.close();
    }

    /// Moves placeholders across the node's window until it's at `index`:
    /// only windows that aren't shown move, and placeholders are all
    /// alike, so the tabs only change titles.
    fn place(&self, index: usize) {
        // Each step moves the window one place; more steps than windows
        // means AppKit didn't move them.
        for _ in 0..=self.group.windows().len() {
            let windows = self.windows();
            let Some(at) = windows.iter().position(|w| addr(w) == addr(&self.window)) else { return };
            let index = index.min(windows.len() - 1);
            if at == index {
                return;
            }
            if at < index {
                // The one after it goes first.
                let next = windows[at + 1].clone();
                self.group.removeWindow(&next);
                self.group.insertWindow_atIndex(&next, 0);
            } else {
                // The one before it goes last.
                let previous = windows[at - 1].clone();
                self.group.removeWindow(&previous);
                let count = self.group.windows().len();
                self.group.insertWindow_atIndex(&previous, count as isize);
            }
        }
    }

    /// Each tab shows the title of where it is; placeholders take theirs as
    /// their windows' titles too (the Window menu lists them). The node's
    /// window keeps the window's title.
    fn title_tabs(&self) {
        let titles = self.shared.model.borrow().titles.clone();
        for (index, window) in self.windows().iter().enumerate() {
            let title = titles.get(index).map(|t| ns(t));
            window.tab().setTitle(title.as_deref());
            if addr(window) != addr(&self.window) {
                window.setTitle(title.as_deref().unwrap_or(&ns("")));
            }
        }
    }

    fn remember_order(&self) {
        *self.order.borrow_mut() = self.windows().iter().map(|w| addr(w)).collect();
    }

    /// A placeholder became key: its tab was picked. The node's window
    /// takes its place, and the app hears.
    fn picked(&self, placeholder: &NSWindow) {
        if self.muted.get() || self.index_of(&self.window).is_none() {
            return;
        }
        let Some(index) = self.index_of(placeholder) else { return };
        let enabled = self.shared.model.borrow().enabled();
        if enabled {
            self.shared.model.borrow_mut().selected = Some(index);
        }
        self.quietly(|| {
            self.group.setSelectedWindow(Some(&self.window));
            if enabled {
                self.place(index);
                self.title_tabs();
            }
            self.window.makeKeyAndOrderFront(None);
            self.remember_order();
        });
        if enabled {
            self.shared.emit(UiEvent::Changed(EventValue::Index(index)));
        }
    }

    /// A placeholder's close button: the app is asked to close its tab.
    fn close_placeholder(&self, placeholder: &NSWindow) {
        if self.muted.get() {
            return;
        }
        let closable = self.shared.model.borrow().closable();
        if let Some(index) = self.index_of(placeholder).filter(|_| closable) {
            self.shared.emit(UiEvent::TabCloseRequested(index));
        }
    }

    /// The group's windows changed, not by the backend: looked at once
    /// AppKit is done (a drag may still be under way).
    fn windows_changed(self: &Rc<Self>) {
        if self.muted.get() || self.pending.replace(true) {
            return;
        }
        let this = Rc::downgrade(self);
        let block = RcBlock::new(move || {
            if let Some(tabs) = this.upgrade() {
                tabs.pending.set(false);
                tabs.reconcile();
            }
        });
        if let Some(run_loop) = CFRunLoop::main() {
            unsafe { run_loop.perform_block(kCFRunLoopCommonModes.map(|m| &**m), Some(&block)) };
            run_loop.wake_up();
        }
    }

    /// What the user did to the group: a tab moved is reported (or put
    /// back, if the app doesn't let tabs move); a tab dragged out to a
    /// window of its own comes back, and a window merged in goes out. The
    /// tabs are the app's, and the window is always one with them.
    fn reconcile(&self) {
        let before = self.order.borrow().clone();
        let mut moved = None;
        self.quietly(|| {
            for window in self.windows() {
                if !self.is_ours(&window) {
                    self.group.removeWindow(&window);
                }
            }
            let mut ours: Vec<Retained<NSWindow>> = vec![self.window.clone()];
            ours.extend(self.placeholders.borrow().iter().cloned());
            for window in &ours {
                if self.index_of(window).is_none() {
                    let at = before.iter().position(|a| *a == addr(window)).unwrap_or(0);
                    let count = self.group.windows().len();
                    self.group.insertWindow_atIndex(window, at.min(count) as isize);
                }
            }
            let now: Vec<usize> = self.windows().iter().map(|w| addr(w)).collect();
            if let Some((from, to)) = one_move(&before, &now) {
                let reorderable = self.shared.model.borrow().reorderable();
                if reorderable {
                    self.shared.model.borrow_mut().move_tab(from, to);
                    moved = Some((from, to));
                } else if let Some(window) = self.windows().get(to).cloned() {
                    self.group.removeWindow(&window);
                    self.group.insertWindow_atIndex(&window, from as isize);
                }
            }
            let chosen = self.shared.model.borrow().chosen();
            self.place(chosen.unwrap_or(0));
            self.title_tabs();
            self.group.setSelectedWindow(Some(&self.window));
            self.remember_order();
        });
        if let Some((from, to)) = moved {
            self.shared.emit(UiEvent::TabMoved { from, to });
        }
    }

    fn titles(&self) -> Vec<String> {
        let count = self.shared.model.borrow().titles.len();
        self.windows().iter().take(count).map(|w| w.tab().title().to_string()).collect()
    }

    fn selected(&self) -> Option<usize> {
        let count = self.shared.model.borrow().titles.len();
        self.index_of(&self.window).filter(|_| count > 0)
    }

    /// The window is a window again: no placeholders, its own title on its
    /// tab, its own close, responder chain and tabbing identifier.
    fn take_down(&self) {
        unsafe { self.group.removeObserver_forKeyPath(&self.observer, &ns(WINDOWS)) };
        self.quietly(|| {
            for placeholder in self.placeholders.borrow_mut().drain(..) {
                self.drop_placeholder(&placeholder);
            }
            self.window.tab().setTitle(None);
        });
        self.delegate.set_close_tab(None);
        let ours = unsafe { self.window.nextResponder() }.is_some_and(|r| std::ptr::eq(&*r, &**self.responder));
        if ours {
            unsafe { self.window.setNextResponder(self.next_responder.as_deref()) };
        }
        self.window.setTabbingIdentifier(&self.identifier);
    }
}

/// The one window that moved between two orders of the same windows, if
/// one did: `(from, to)`.
fn one_move(before: &[usize], now: &[usize]) -> Option<(usize, usize)> {
    if before.len() != now.len() || before == now {
        return None;
    }
    let mut sorted = (before.to_vec(), now.to_vec());
    sorted.0.sort_unstable();
    sorted.1.sort_unstable();
    if sorted.0 != sorted.1 {
        return None;
    }
    let first = before.iter().zip(now).position(|(a, b)| a != b)?;
    let last = before.iter().zip(now).rposition(|(a, b)| a != b)?;
    if now[first] == before[last] { Some((last, first)) } else { Some((first, last)) }
}

// ------------------------------------------------------------------ strip

/// The strip's height, its new-tab button's width, each tab's close
/// button's side, and how far titles are from their tab's edges: the
/// window tab bar's, near enough.
const HEIGHT: f64 = 28.0;
const PLUS: f64 = 28.0;
const CLOSE: f64 = 16.0;
const PADDING: f64 = 8.0;

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
                NSBezierPath::fillRect(rect);
            }
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
        if self.ivars().plus.borrow().is_some() { PLUS } else { 0.0 }
    }

    /// Where each tab is: the strip's width less the new-tab button's,
    /// shared equally.
    fn tab_rects(&self) -> Vec<NSRect> {
        let count = self.ivars().shared.model.borrow().titles.len();
        let size = self.bounds().size;
        let width = if count == 0 { 0.0 } else { ((size.width - self.plus_width()) / count as f64).max(0.0) };
        (0..count)
            .map(|i| self.mirrored(NSRect::new(NSPoint::new(i as f64 * width, 0.0), NSSize::new(width, size.height))))
            .collect()
    }

    fn plus_rect(&self) -> NSRect {
        let size = self.bounds().size;
        self.mirrored(NSRect::new(NSPoint::new(size.width - PLUS, 0.0), NSSize::new(PLUS, size.height)))
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
        label.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSize())));
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

    /// A borderless button showing an SF Symbol, named for assistive
    /// technology and its tooltip.
    fn button(&self, mtm: MainThreadMarker, symbol: &str, name: &str) -> Retained<NSButton> {
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns(symbol), Some(&ns(name)))
            .map(|image| {
                let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(
                    if symbol == "xmark" { 9.0 } else { 12.0 },
                    unsafe { objc2_app_kit::NSFontWeightSemibold },
                );
                image.imageWithSymbolConfiguration(&config).unwrap_or(image)
            })
            .unwrap_or_default();
        let button = unsafe { NSButton::buttonWithImage_target_action(&image, None, None, mtm) };
        button.setBordered(false);
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
            let inset = PADDING + CLOSE;
            tab.label.setFrame(NSRect::new(
                NSPoint::new(rect.origin.x + inset, rect.origin.y + ((rect.size.height - label_height) / 2.0).round()),
                NSSize::new((rect.size.width - 2.0 * inset).max(0.0), label_height),
            ));
            let close_x = if rtl { rect.origin.x + rect.size.width - PADDING - CLOSE } else { rect.origin.x + PADDING };
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

    /// The strip's ground, the tabs on it, the chosen one lit as the
    /// window's background (as the system's tab bar has it), and lines
    /// between the others.
    fn draw(&self) {
        let bounds = self.bounds();
        let ground = NSColor::windowBackgroundColor();
        let shade = NSColor::blackColor();
        let dim = |fraction| ground.blendedColorWithFraction_ofColor(fraction, &shade).unwrap_or(ground.clone());
        dim(0.08).setFill();
        NSBezierPath::fillRect(bounds);
        let rects = self.tab_rects();
        let chosen = self.ivars().shared.model.borrow().chosen();
        let hover = self.ivars().hover.get();
        for (index, rect) in rects.iter().enumerate() {
            if chosen == Some(index) {
                ground.setFill();
                NSBezierPath::fillRect(*rect);
            } else if hover == Some(index) {
                dim(0.04).setFill();
                NSBezierPath::fillRect(*rect);
            }
        }
        NSColor::separatorColor().setFill();
        for (index, rect) in rects.iter().enumerate().skip(1) {
            if chosen == Some(index) || chosen == Some(index - 1) {
                continue;
            }
            let x = if self.right_to_left() { rect.origin.x + rect.size.width } else { rect.origin.x };
            NSBezierPath::fillRect(NSRect::new(NSPoint::new(x - 0.5, 4.0), NSSize::new(1.0, bounds.size.height - 8.0)));
        }
        // A line under the strip, except under the chosen tab, which runs
        // into the content as the system's does.
        let line = NSRect::new(NSPoint::new(0.0, bounds.size.height - 1.0), NSSize::new(bounds.size.width, 1.0));
        NSBezierPath::fillRect(line);
        if let Some(rect) = chosen.and_then(|i| rects.get(i)) {
            ground.setFill();
            NSBezierPath::fillRect(NSRect::new(
                NSPoint::new(rect.origin.x, line.origin.y),
                NSSize::new(rect.size.width, 1.0),
            ));
        }
    }

    /// As wide as its titles with room for their buttons, and the new-tab
    /// button; as high as the system's tab bar.
    fn natural_size(&self) -> Size {
        let tabs: f64 = self
            .ivars()
            .tabs
            .borrow()
            .iter()
            .map(|t| t.label.intrinsicContentSize().width.max(0.0) + 2.0 * (PADDING + CLOSE))
            .sum();
        let plus = if self.ivars().shared.model.borrow().new_tab == Some(true) { PLUS } else { 0.0 };
        Size::new((tabs + plus).ceil() as f32, HEIGHT as f32)
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
