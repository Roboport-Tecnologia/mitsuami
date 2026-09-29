//! A window's sidebar: a `NavigationView`, as Windows' own Settings has,
//! whose pane lists the items and whose content is the window's content
//! host. Its display mode is XAML's `Auto`: the pane is open in a wide
//! window, icons only in a narrower one, and behind a menu button in a
//! narrow one; open in a wide window, it has no menu button to close it,
//! as in Settings. Icons only needs every item to have an icon, as Fluent's
//! guidance has it: without, the view goes from open to the menu button,
//! and a closed pane, which would show as that strip, is hidden whole,
//! with the menu button in the title bar to bring it back.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use mitsuami_core::{EventValue, NodeId, SidebarSectionData, UiEvent};
use windows_core::{EventRevoker, IInspectable, IUnknown, Interface};

use crate::backend::{Events, boxed};
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// Places the menu buttons (see `Sidebar::follow_title_bar`).
pub(crate) type Place = Rc<dyn Fn()>;

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
    /// Whether the pane is open, set by the core or reported to it: the
    /// view reports the app's openings too.
    open: Rc<Cell<bool>>,
    _panes: [EventRevoker; 2],
    /// Where the menu buttons go, once it's in a window: the window holds
    /// it (`follow_title_bar`), and new items place them again.
    place: RefCell<Option<Weak<dyn Fn()>>>,
}

/// Which menu button shows, the view's or the title bar's, and whether
/// the pane shows at all (see `Sidebar::follow_title_bar`).
fn place_toggles(view: &w::NavigationView, title_bar: &w::ITitleBar) -> R<()> {
    let mode = view.DisplayMode()?;
    let minimal = mode == w::NavigationViewDisplayMode::Minimal;
    let open = view.IsPaneOpen()?;
    let without_icons = view.CompactModeThresholdWidth()? >= view.ExpandedModeThresholdWidth()?;
    let gone = !minimal && without_icons && !open;
    // Open in a wide window, the pane stays: Windows' Settings has no menu
    // button there.
    let fixed = mode == w::NavigationViewDisplayMode::Expanded && open;
    view.cast::<w::INavigationView2>()?.SetIsPaneVisible(!gone)?;
    view.SetIsPaneToggleButtonVisible(!minimal && !gone && !fixed)?;
    title_bar.SetIsPaneToggleButtonVisible(minimal || gone)
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
            let (items, shown, emitter) = (items.clone(), shown.clone(), emitter.clone());
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
        // Opened or closed by the user (its menu button, the title bar's,
        // a click beside a pane over the content), or by `Auto` as the
        // window's width changes.
        let open = Rc::new(Cell::new(view.IsPaneOpen()?));
        let view2 = view.cast::<w::INavigationView2>()?;
        let panes = [true, false].map(|opened| {
            let (emitter, open) = (emitter.clone(), open.clone());
            let handler = move |_: windows_core::Ref<w::NavigationView>, _: windows_core::Ref<IInspectable>| {
                if open.replace(opened) != opened {
                    emitter.emit(id, UiEvent::SidebarShownChanged(opened));
                }
            };
            if opened { view2.PaneOpened(handler) } else { view2.PaneClosed(handler) }
        });
        let [opened, closed] = panes;
        Ok(Sidebar {
            view,
            items,
            sections: RefCell::new(Vec::new()),
            shown,
            _selection: selection,
            open,
            _panes: [opened?, closed?],
            place: RefCell::new(None),
        })
    }

    /// Opens or closes the pane without reporting it.
    pub(crate) fn set_shown(&self, shown: bool) -> R<()> {
        self.open.set(shown);
        // Unloaded, the pane reads closed, so closing it is no change and
        // `Auto` opens it in a wide window: closed from open, it's the app's
        // choice, which `Auto` keeps.
        if !shown && !self.view.cast::<w::IFrameworkElement>()?.IsLoaded()? {
            self.view.SetIsPaneOpen(true)?;
        }
        self.view.SetIsPaneOpen(shown)?;
        // The view doesn't report the app's closing.
        self.place_again();
        Ok(())
    }

    /// Places the menu buttons again, once it's in a window.
    fn place_again(&self) {
        let place = self.place.borrow().as_ref().and_then(Weak::upgrade);
        if let Some(place) = place {
            place();
        }
    }

    pub(crate) fn is_shown(&self) -> bool {
        self.view.IsPaneOpen().unwrap_or(self.open.get())
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
        // Compact mode shows icons alone: an item without one would show its
        // title cut down to the strip. Without icons for all, `Auto` skips
        // it, going from open to the menu button.
        let all_icons = sections.iter().flat_map(|s| &s.items).all(|i| i.icon.as_ref().is_some_and(|g| !g.is_empty()));
        let expanded = self.view.ExpandedModeThresholdWidth().unwrap_or(EXPANDED_FROM as f64);
        self.view.SetCompactModeThresholdWidth(if all_icons { COMPACT_FROM as f64 } else { expanded })?;
        self.place_again();
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
        if !view.cast::<w::INavigationView2>().and_then(|v| v.IsPaneVisible()).unwrap_or(true) {
            return 0.0;
        }
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
    /// as `Auto` shows it at the window's width (never icons only while an
    /// item has none: the thresholds are then the same).
    pub(crate) fn extra_width(view: &w::NavigationView, content: f32) -> f32 {
        let open = view.OpenPaneLength().unwrap_or(320.0) as f32;
        let compact = view.CompactPaneLength().unwrap_or(48.0) as f32;
        let expanded_from = view.ExpandedModeThresholdWidth().map_or(EXPANDED_FROM, |w| w as f32);
        let compact_from = view.CompactModeThresholdWidth().map_or(COMPACT_FROM, |w| w as f32);
        if content + open >= expanded_from {
            open
        } else if compact_from < expanded_from && content + compact >= compact_from {
            compact
        } else {
            0.0
        }
    }

    /// Puts the menu button in the window's title bar while the pane is
    /// hidden, as Task Manager has it, and in the pane while it shows: the
    /// view's own sits over the content's top corner. The title bar's
    /// button opens the pane. Hidden is `Minimal`, or a closed pane while
    /// an item has no icon: the icons-only strip it would show cuts titles
    /// down, so the pane goes whole (`IsPaneVisible`), and comes back open
    /// from the title bar's button. Undone by `leave_title_bar` and the
    /// revokers; the window keeps the placing, which the sidebar calls
    /// again when its items change.
    pub(crate) fn follow_title_bar(&self, title_bar: &w::TitleBar) -> R<(Vec<EventRevoker>, Place)> {
        let title_bar: w::ITitleBar = title_bar.cast()?;
        // Weakly, as the view's handlers hold it.
        let weak = self.view.downgrade()?;
        let place: Place = {
            let (weak, title_bar) = (weak.clone(), title_bar.clone());
            Rc::new(move || {
                if let Some(view) = weak.upgrade() {
                    let _ = place_toggles(&view, &title_bar);
                }
            })
        };
        *self.place.borrow_mut() = Some(Rc::downgrade(&place));
        place();
        // Once the view is done: it reports the new mode before `Auto` opens
        // the pane, which it does only while the pane is visible.
        let modes = self.view.DisplayModeChanged({
            let place = Rc::downgrade(&place);
            move |_, _| {
                let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else { return };
                let ticket = crate::later::park(place.clone());
                crate::later::on_ui(&queue, move || {
                    if let Some(place) = crate::later::take::<Weak<dyn Fn()>>(ticket).and_then(|p| p.upgrade()) {
                        place();
                    }
                });
            }
        })?;
        let view2 = self.view.cast::<w::INavigationView2>()?;
        let panes = [true, false].map(|opened| {
            let place = Rc::downgrade(&place);
            let handler = move |_: windows_core::Ref<w::NavigationView>, _: windows_core::Ref<IInspectable>| {
                if let Some(place) = place.upgrade() {
                    place();
                }
            };
            if opened { view2.PaneOpened(handler) } else { view2.PaneClosed(handler) }
        });
        let [opened, closed] = panes;
        let toggle = title_bar.PaneToggleRequested(move |_, _| {
            if let Some(view) = weak.upgrade() {
                let open = view.IsPaneOpen().unwrap_or(false);
                // A pane hidden whole comes back open.
                let _ = view.cast::<w::INavigationView2>().and_then(|v| v.SetIsPaneVisible(true));
                let _ = view.SetIsPaneOpen(!open);
            }
        })?;
        Ok((vec![modes, opened?, closed?, toggle], place))
    }

    /// The title bar without the sidebar's menu button, and the pane shown.
    pub(crate) fn leave_title_bar(view: &w::NavigationView, title_bar: &w::TitleBar) -> R<()> {
        view.cast::<w::INavigationView2>()?.SetIsPaneVisible(true)?;
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
