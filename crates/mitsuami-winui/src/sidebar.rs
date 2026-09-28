//! A window's sidebar: a `NavigationView`, as Windows' own Settings has,
//! whose pane lists the items and whose content is the window's content
//! host. Its display mode is XAML's `Auto`: the pane is open in a wide
//! window, icons only in a narrower one, and behind a menu button in a
//! narrow one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventValue, NodeId, SidebarSectionData, UiEvent};
use windows_core::{EventRevoker, IInspectable, IUnknown, Interface};

use crate::backend::{Events, boxed};
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// The widths from which `Auto` shows the pane open, and icons only
/// (XAML's `ExpandedModeThresholdWidth` and `CompactModeThresholdWidth`).
const EXPANDED_FROM: f32 = 1008.0;
const COMPACT_FROM: f32 = 641.0;

/// The sidebar node's native parts.
pub(crate) struct Sidebar {
    pub view: w::NavigationView,
    /// One per item, in order: what the view's selection is compared with.
    items: Rc<RefCell<Vec<w::NavigationViewItem>>>,
    sections: RefCell<Vec<SidebarSectionData>>,
    /// The item the view is known to show selected, set by the core or
    /// reported to it: `SelectionChanged` fires for the app's changes too.
    shown: Rc<Cell<Option<usize>>>,
    _selection: EventRevoker,
}

/// Which of the items `selected` is, by COM identity: the `IUnknown`
/// pointers, as each interface of an object has a pointer of its own.
fn index_of(items: &[w::NavigationViewItem], selected: Option<IInspectable>) -> Option<usize> {
    let selected = selected.filter(|s| !s.as_raw().is_null())?.cast::<IUnknown>().ok()?;
    items.iter().position(|item| item.cast::<IUnknown>().is_ok_and(|item| item.as_raw() == selected.as_raw()))
}

impl Sidebar {
    pub(crate) fn new(id: NodeId, emitter: Events) -> R<Sidebar> {
        let view = w::NavigationView::new()?;
        // No Settings item or back button: the app's items are all it has,
        // and it has no pages to go back through.
        view.SetIsSettingsVisible(false)?;
        view.cast::<w::INavigationView2>()?.SetIsBackButtonVisible(w::NavigationViewBackButtonVisible::Collapsed)?;
        let items = Rc::new(RefCell::new(Vec::<w::NavigationViewItem>::new()));
        let shown = Rc::new(Cell::new(None));
        let selection = view.SelectionChanged({
            let (items, shown) = (items.clone(), shown.clone());
            move |_, args| {
                let Some(args) =
                    args.as_ref().and_then(|a| a.cast::<w::INavigationViewSelectionChangedEventArgs>().ok())
                else {
                    return;
                };
                let index = index_of(&items.borrow(), args.SelectedItem().ok());
                if let Some(index) = index
                    && shown.replace(Some(index)) != Some(index)
                {
                    emitter.emit(id, UiEvent::Changed(EventValue::Index(index)));
                }
            }
        })?;
        Ok(Sidebar { view, items, sections: RefCell::new(Vec::new()), shown, _selection: selection })
    }

    /// New items: a header for each titled section, a line between
    /// untitled ones, and the items with their Segoe Fluent Icons glyphs.
    /// The selected item stays, which the core sends again if it moved.
    pub(crate) fn set_sections(&self, sections: Vec<SidebarSectionData>) -> R<()> {
        let menu = self.view.MenuItems()?;
        menu.Clear()?;
        let mut items = Vec::new();
        for (index, section) in sections.iter().enumerate() {
            match &section.title {
                Some(title) => {
                    let header = w::NavigationViewItemHeader::new()?;
                    header.cast::<w::IContentControl>()?.SetContent(&boxed(title))?;
                    menu.Append(&header.cast::<IInspectable>()?)?;
                }
                None if index > 0 => menu.Append(&w::NavigationViewItemSeparator::new()?.cast::<IInspectable>()?)?,
                None => {}
            }
            for data in &section.items {
                let item = w::NavigationViewItem::new()?;
                item.cast::<w::IContentControl>()?.SetContent(&boxed(&data.title))?;
                if let Some(glyph) = &data.icon {
                    let icon = w::FontIcon::new()?;
                    icon.cast::<w::IFontIcon>()?.SetGlyph(glyph)?;
                    item.cast::<w::INavigationViewItem>()?.SetIcon(&icon.cast::<w::IconElement>()?)?;
                }
                menu.Append(&item.cast::<IInspectable>()?)?;
                items.push(item);
            }
        }
        *self.items.borrow_mut() = items;
        *self.sections.borrow_mut() = sections;
        let kept = self.shown.get().filter(|i| *i < self.items.borrow().len());
        self.set_selected(kept)
    }

    pub(crate) fn sections(&self) -> Vec<SidebarSectionData> {
        self.sections.borrow().clone()
    }

    /// Selects an item without reporting it.
    pub(crate) fn set_selected(&self, index: Option<usize>) -> R<()> {
        self.shown.set(index);
        let item = index.and_then(|i| self.items.borrow().get(i).cloned());
        match item {
            Some(item) => self.view.SetSelectedItem(&item.cast::<IInspectable>()?),
            None => self.view.SetSelectedItem(None::<&IInspectable>),
        }
    }

    /// The item the view shows selected.
    pub(crate) fn selected(&self) -> Option<usize> {
        index_of(&self.items.borrow(), self.view.SelectedItem().ok())
    }

    /// Selects the first item with this title as a click does, which the
    /// view reports. `false` if there's none.
    pub(crate) fn choose(&self, title: &str) -> R<bool> {
        let index = self.sections.borrow().iter().flat_map(|s| &s.items).position(|i| i.title == title);
        let Some(item) = index.and_then(|i| self.items.borrow().get(i).cloned()) else { return Ok(false) };
        self.view.SetSelectedItem(&item.cast::<IInspectable>()?)?;
        Ok(true)
    }

    /// Where the pane is in the view: open, icons only, or closed.
    pub(crate) fn pane_width(view: &w::NavigationView) -> f32 {
        match view.DisplayMode() {
            Ok(w::NavigationViewDisplayMode::Expanded) if view.IsPaneOpen().unwrap_or(false) => {
                view.OpenPaneLength().unwrap_or(0.0) as f32
            }
            Ok(w::NavigationViewDisplayMode::Expanded | w::NavigationViewDisplayMode::Compact) => {
                view.CompactPaneLength().unwrap_or(0.0) as f32
            }
            _ => 0.0,
        }
    }

    /// How much wider the window is than a content this wide: by the pane,
    /// as `Auto` shows it at the window's width.
    pub(crate) fn extra_width(view: &w::NavigationView, content: f32) -> f32 {
        let open = view.OpenPaneLength().unwrap_or(320.0) as f32;
        let compact = view.CompactPaneLength().unwrap_or(48.0) as f32;
        if content + open >= EXPANDED_FROM {
            open
        } else if content + compact >= COMPACT_FROM {
            compact
        } else {
            0.0
        }
    }

    /// Puts the menu button in the window's title bar while the pane is
    /// hidden, as Task Manager has it, and in the pane while it shows: the
    /// view's own sits over the content's top corner. The title bar's
    /// button opens the pane. Undone by `leave_title_bar`, and the
    /// revokers.
    pub(crate) fn follow_title_bar(view: &w::NavigationView, title_bar: &w::TitleBar) -> R<Vec<EventRevoker>> {
        let title_bar: w::ITitleBar = title_bar.cast()?;
        let place = {
            let title_bar = title_bar.clone();
            move |view: &w::NavigationView| -> R<()> {
                let hidden = view.DisplayMode()? == w::NavigationViewDisplayMode::Minimal;
                view.SetIsPaneToggleButtonVisible(!hidden)?;
                title_bar.SetIsPaneToggleButtonVisible(hidden)
            }
        };
        place(view)?;
        let modes = view.DisplayModeChanged(move |view, _| {
            if let Some(view) = view.as_ref() {
                let _ = place(view);
            }
        })?;
        // Weakly, as the view's handler holds the title bar.
        let weak = view.downgrade()?;
        let toggle = title_bar.PaneToggleRequested(move |_, _| {
            if let Some(view) = weak.upgrade() {
                let open = view.IsPaneOpen().unwrap_or(false);
                let _ = view.SetIsPaneOpen(!open);
            }
        })?;
        Ok(vec![modes, toggle])
    }

    /// The title bar without the sidebar's menu button.
    pub(crate) fn leave_title_bar(title_bar: &w::TitleBar) -> R<()> {
        title_bar.cast::<w::ITitleBar>()?.SetIsPaneToggleButtonVisible(false)
    }

    /// What takes keyboard focus in it: the selected item, or the first.
    pub(crate) fn focus_target(view: &w::NavigationView) -> Option<w::IUIElement> {
        let selected = view.SelectedItem().ok().filter(|s| !s.as_raw().is_null());
        let first = || {
            let menu = view.MenuItems().ok()?;
            (0..menu.Size().ok()?)
                .filter_map(|i| menu.GetAt(i).ok())
                .find(|item| item.cast::<w::NavigationViewItem>().is_ok())
        };
        selected.or_else(first)?.cast().ok()
    }
}
