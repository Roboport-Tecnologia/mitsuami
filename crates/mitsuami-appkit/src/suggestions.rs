//! A search field's suggestions: a list under the field that never takes
//! the keyboard, as Safari's address bar and Apple's suggestions-window
//! sample show theirs (AppKit has no suggestion list of its own, and a
//! popover takes the keyboard, so typing stopped). It's a borderless panel,
//! which can't become key, attached to the field's window, holding a native
//! table on the menu material; the field's delegate moves through it with
//! the arrows, picks with Return and hides it with Escape.

use std::cell::{Cell, RefCell};

use mitsuami_core::{EventSink, NodeId, Suggestion, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSControlTextEditingDelegate, NSPanel, NSScrollView, NSTableColumn, NSTableView,
    NSTableViewDataSource, NSTableViewDelegate, NSTableViewStyle, NSTextField, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{NSIndexSet, NSInteger, NSPoint, NSRect, NSSize, NSString};

/// The list's rows: a title, and a second line under it.
const ROW: f64 = 24.0;
/// The list goes no taller, then scrolls.
const MOST: f64 = 400.0;
/// Between the field and the list, and round the rows.
const GAP: f64 = 4.0;

pub(crate) struct Ivars {
    id: NodeId,
    events: EventSink,
    items: RefCell<Vec<Suggestion>>,
    /// Escape hid it, or a pick did: hidden until the user types again.
    dismissed: Cell<bool>,
}

define_class!(
    /// The table's rows, and its clicks.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    pub(crate) struct SuggestionSource;

    unsafe impl NSObjectProtocol for SuggestionSource {}

    unsafe impl NSControlTextEditingDelegate for SuggestionSource {}

    unsafe impl NSTableViewDataSource for SuggestionSource {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> NSInteger {
            self.ivars().items.borrow().len() as NSInteger
        }
    }

    unsafe impl NSTableViewDelegate for SuggestionSource {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: NSInteger,
        ) -> Option<Retained<NSView>> {
            let items = self.ivars().items.borrow();
            let item = items.get(row as usize);
            item.map(|item| crate::sidebar::cell(self.mtm(), &item.title, None, item.detail.as_deref()).into_super())
        }

        #[unsafe(method(tableView:heightOfRow:))]
        fn height_of_row(&self, _table: &NSTableView, row: NSInteger) -> f64 {
            let two_lines = self.ivars().items.borrow().get(row as usize).is_some_and(|i| i.detail.is_some());
            if two_lines { ROW + crate::sidebar::line_height(&crate::sidebar::subtitle_font()) } else { ROW }
        }
    }

    impl SuggestionSource {
        /// A row was clicked: it's picked, and the list hides.
        #[unsafe(method(clicked:))]
        fn clicked(&self, table: &NSTableView) {
            let row = table.clickedRow();
            if row >= 0 {
                let Ivars { id, events, dismissed, .. } = self.ivars();
                events.emit(*id, UiEvent::SuggestionChosen(row as usize));
                dismissed.set(true);
                if let Some(panel) = table.window() {
                    hide(&panel);
                }
            }
        }
    }
);

pub(crate) struct Suggestions {
    panel: Retained<NSPanel>,
    table: Retained<NSTableView>,
    scroll: Retained<NSScrollView>,
    source: Retained<SuggestionSource>,
    field: Retained<NSTextField>,
}

impl Suggestions {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink, field: &NSTextField) -> Suggestions {
        let source = SuggestionSource::alloc(mtm).set_ivars(Ivars {
            id,
            events,
            items: RefCell::new(Vec::new()),
            dismissed: Cell::new(false),
        });
        let source: Retained<SuggestionSource> = unsafe { msg_send![super(source), init] };
        let table = NSTableView::new(mtm);
        table.setStyle(NSTableViewStyle::Inset);
        table.setHeaderView(None);
        table.setBackgroundColor(&NSColor::clearColor());
        table.setRefusesFirstResponder(true);
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &NSString::from_str("suggestion"));
        table.addTableColumn(&column);
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*source)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*source)));
            table.setTarget(Some(&source));
            table.setAction(Some(sel!(clicked:)));
        }
        let scroll = NSScrollView::new(mtm);
        scroll.setDocumentView(Some(&table));
        scroll.setHasVerticalScroller(true);
        scroll.setDrawsBackground(false);
        // The menu's material, round its corners, as a menu shows.
        let effect = NSVisualEffectView::new(mtm);
        effect.setMaterial(NSVisualEffectMaterial::Menu);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setWantsLayer(true);
        if let Some(layer) = effect.layer() {
            let _: () = unsafe { msg_send![&layer, setCornerRadius: 8.0f64] };
            let _: () = unsafe { msg_send![&layer, setMasksToBounds: true] };
        }
        effect.addSubview(&scroll);
        // Borderless, so it never becomes key: the field keeps the keyboard.
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            true,
        );
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(true);
        panel.setContentView(Some(&effect));
        Suggestions { panel, table, scroll, source, field: field.retain() }
    }

    pub(crate) fn items(&self) -> Vec<Suggestion> {
        self.source.ivars().items.borrow().clone()
    }

    pub(crate) fn set(&self, items: &[Suggestion]) {
        *self.source.ivars().items.borrow_mut() = items.to_vec();
        self.table.reloadData();
        self.sync();
    }

    /// The user typed: a list Escape hid shows again.
    pub(crate) fn typed(&self) {
        self.source.ivars().dismissed.set(false);
        self.sync();
    }

    /// Shows the list under the field while it has suggestions, the field
    /// is being edited and the user hasn't dismissed it.
    pub(crate) fn sync(&self) {
        let count = self.source.ivars().items.borrow().len();
        let editing = self.field.currentEditor().is_some();
        let Some(window) = self.field.window().filter(|_| count > 0 && editing && !self.source.ivars().dismissed.get())
        else {
            return self.hide();
        };
        let height = (0..count).map(|row| self.table.rectOfRow(row as NSInteger).size.height).sum::<f64>();
        let height = (height + 2.0 * GAP).min(MOST);
        let on_screen = window.convertRectToScreen(self.field.convertRect_toView(self.field.bounds(), None));
        let frame = NSRect::new(
            NSPoint::new(on_screen.origin.x, on_screen.origin.y - GAP - height),
            NSSize::new(on_screen.size.width.max(260.0), height),
        );
        self.panel.setFrame_display(frame, true);
        if let Some(content) = self.panel.contentView() {
            let bounds = content.bounds();
            self.scroll.setFrame(NSRect::new(
                NSPoint::new(bounds.origin.x, bounds.origin.y + GAP),
                NSSize::new(bounds.size.width, bounds.size.height - 2.0 * GAP),
            ));
        }
        if self.panel.parentWindow().is_none() {
            unsafe { window.addChildWindow_ordered(&self.panel, NSWindowOrderingMode::Above) };
        }
        self.panel.orderFront(None);
    }

    pub(crate) fn shown(&self) -> bool {
        self.panel.isVisible()
    }

    pub(crate) fn hide(&self) {
        hide(&self.panel);
        unsafe { self.table.deselectAll(None) };
    }

    /// Escape: hidden until the user types again.
    pub(crate) fn dismiss(&self) {
        self.source.ivars().dismissed.set(true);
        self.hide();
    }

    /// The arrows: the next or previous row, from none.
    pub(crate) fn step(&self, by: isize) {
        let count = self.table.numberOfRows();
        if count == 0 {
            return;
        }
        let row = match self.table.selectedRow() {
            -1 if by > 0 => 0,
            -1 => count - 1,
            row => (row + by as NSInteger).clamp(0, count - 1),
        };
        self.table.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row as usize), false);
        self.table.scrollRowToVisible(row);
    }

    /// The row the arrows reached, if any.
    pub(crate) fn selected(&self) -> Option<usize> {
        let row = self.table.selectedRow();
        (self.shown() && row >= 0).then_some(row as usize)
    }

    /// Picks a row, as a click does, and hides the list.
    pub(crate) fn choose(&self, row: usize) {
        let Ivars { id, events, .. } = self.source.ivars();
        events.emit(*id, UiEvent::SuggestionChosen(row));
        self.dismiss();
    }
}

/// Takes the panel off its field's window.
fn hide(panel: &objc2_app_kit::NSWindow) {
    if let Some(parent) = panel.parentWindow() {
        parent.removeChildWindow(panel);
    }
    panel.orderOut(None);
}
