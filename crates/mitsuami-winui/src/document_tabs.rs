//! Document tabs: a `TabView` whose items have no content, as Windows
//! Terminal's tab row is one, the app's document drawn elsewhere. The node
//! is a `Grid` that holds the view, stretched over it, where it's a strip
//! in the content; where the tabs are the window's, the view goes in the
//! title bar's content instead (`place_document_tabs`), and the grid stays
//! empty.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{DocumentTabsStyle, EventValue, NodeId, UiEvent, moved_index};
use windows_core::{EventRevoker, IInspectable, Interface};

use crate::backend::{Events, boxed, unboxed};
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// The node's native parts.
pub(crate) struct DocumentTabs {
    pub grid: w::Grid,
    pub view: w::TabView,
    /// One per title, in order: what the view's items are compared with.
    items: Rc<RefCell<Vec<w::TabViewItem>>>,
    /// The tab chosen, set by the core or reported to it: `SelectionChanged`
    /// fires for the app's changes too, and while a tab is dragged.
    shown: Rc<Cell<Option<usize>>>,
    /// The backend is changing the items or the selection.
    applying: Rc<Cell<bool>>,
    /// Whether tabs close, which new items take; read back from the items
    /// while there are some.
    closable: Cell<bool>,
    /// The app's choice.
    pub style: Cell<Option<DocumentTabsStyle>>,
    /// The window whose title bar the view is in, while the tabs are the
    /// window's (`place_document_tabs`).
    pub window: Cell<Option<NodeId>>,
    _revokers: Vec<EventRevoker>,
}

/// The view's natural size, once it's in a window and has its template.
fn natural(view: &w::TabView) -> w::Size {
    let Ok(view) = view.cast::<w::IUIElement>() else { return w::Size::default() };
    _ = view.Measure(w::Size { width: f32::INFINITY, height: f32::INFINITY });
    view.DesiredSize().unwrap_or_default()
}

fn index_of(items: &[w::TabViewItem], item: &w::TabViewItem) -> Option<usize> {
    items.iter().position(|i| i.as_raw() == item.as_raw())
}

/// The view's selection, if it's one of the tabs.
fn selected(view: &w::TabView, count: usize) -> Option<usize> {
    let index = view.SelectedIndex().ok()?;
    usize::try_from(index).ok().filter(|i| *i < count)
}

/// A selection the core doesn't know about is the user's: reported.
fn report(view: &w::TabView, items: &[w::TabViewItem], shown: &Cell<Option<usize>>, emitter: &Events, id: NodeId) {
    let Some(index) = selected(view, items.len()) else { return };
    if shown.replace(Some(index)) != Some(index) {
        emitter.emit(id, UiEvent::Changed(EventValue::Index(index)));
    }
}

/// Selects `index` (none for `None`) without reporting it.
fn select(view: &w::TabView, applying: &Cell<bool>, index: Option<usize>) -> R<()> {
    let before = applying.replace(true);
    let result = view.SetSelectedIndex(index.map_or(-1, |i| i as i32));
    applying.set(before);
    result
}

impl DocumentTabs {
    pub(crate) fn new(id: NodeId, emitter: Events) -> R<DocumentTabs> {
        let grid = w::Grid::new()?;
        let view = w::TabView::new()?;
        // Nothing until the app asks: no new-tab button, no reordering, no
        // tabs dragged out of the window (the app's to do).
        view.SetIsAddTabButtonVisible(false)?;
        view.SetCanReorderTabs(false)?;
        view.SetCanDragTabs(false)?;
        view.SetAllowDropTabs(false)?;
        grid.cast::<w::IPanel>()?.Children()?.Append(&view.cast::<w::UIElement>()?)?;
        let items = Rc::new(RefCell::new(Vec::<w::TabViewItem>::new()));
        let shown = Rc::new(Cell::new(None));
        let applying = Rc::new(Cell::new(false));
        // Where a tab being moved was taken out: XAML moves one by removing
        // it, then inserting it where it's dropped.
        let taken = Rc::new(Cell::new(None::<usize>));
        let mut revokers = Vec::new();
        // Read from the view, not the event: XAML may report a set late,
        // and by then the core may have set another.
        revokers.push(view.SelectionChanged({
            let (view, items, shown, applying, taken, emitter) =
                (view.clone(), items.clone(), shown.clone(), applying.clone(), taken.clone(), emitter.clone());
            move |_, _| {
                // A tab taken out to be moved: the selection is put back
                // once it's in its new place.
                if applying.get() || taken.get().is_some() {
                    return;
                }
                report(&view, &items.borrow(), &shown, &emitter, id);
            }
        })?);
        revokers.push(view.TabCloseRequested({
            let (items, emitter) = (items.clone(), emitter.clone());
            move |_, args| {
                let Some(tab) = args.as_ref().and_then(|a| a.Tab().ok()) else { return };
                if let Some(index) = index_of(&items.borrow(), &tab) {
                    emitter.emit(id, UiEvent::TabCloseRequested(index));
                }
            }
        })?);
        revokers.push(view.AddTabButtonClick({
            let emitter = emitter.clone();
            move |_, _| emitter.emit(id, UiEvent::NewTabRequested)
        })?);
        // A drag moves a tab by taking it out, then putting it back where
        // it's dropped: the chosen tab stays chosen, and the move is
        // reported once it's in place.
        revokers.push(view.TabItemsChanged({
            let (view, items, shown, applying, taken, emitter) =
                (view.clone(), items.clone(), shown.clone(), applying.clone(), taken.clone(), emitter.clone());
            move |_, args| {
                if applying.get() {
                    return;
                }
                let Some(args) = args.as_ref() else { return };
                let (Ok(change), Ok(index)) = (args.CollectionChange(), args.Index()) else { return };
                let index = index as usize;
                if change == w::CollectionChange::ItemRemoved {
                    taken.set(Some(index));
                    return;
                }
                if change != w::CollectionChange::ItemInserted {
                    return;
                }
                let Some(from) = taken.take() else { return };
                {
                    let mut items = items.borrow_mut();
                    if from >= items.len() || index >= items.len() {
                        return;
                    }
                    let item = items.remove(from);
                    items.insert(index, item);
                }
                let chosen = shown.get().map(|i| moved_index(i, from, index));
                shown.set(chosen);
                _ = select(&view, &applying, chosen);
                if from != index {
                    emitter.emit(id, UiEvent::TabMoved { from, to: index });
                }
            }
        })?);
        // Until it's in a window, it has no template and measures empty:
        // measure it again then.
        revokers.push(view.cast::<w::IFrameworkElement>()?.Loaded({
            let emitter = emitter.clone();
            move |_, _| emitter.emit(id, UiEvent::Remeasure)
        })?);
        Ok(DocumentTabs {
            grid,
            view,
            items,
            shown,
            applying,
            closable: Cell::new(false),
            style: Cell::new(None),
            window: Cell::new(None),
            _revokers: revokers,
        })
    }

    /// New titles, relabelling the tabs there are: the chosen one stays
    /// (unless it's gone), which the core sends again if it moved.
    pub(crate) fn set_titles(&self, titles: &[String]) -> R<()> {
        let list = self.view.TabItems()?;
        let before = self.applying.replace(true);
        let result = (|| -> R<()> {
            // Not borrowed while the view changes: it reports removals.
            let mut items = self.items.borrow().clone();
            for (index, title) in titles.iter().enumerate() {
                match items.get(index) {
                    Some(item) => {
                        if unboxed(item.Header()).as_deref() != Some(title.as_str()) {
                            item.SetHeader(&boxed(title))?;
                        }
                    }
                    None => {
                        let item = w::TabViewItem::new()?;
                        item.SetHeader(&boxed(title))?;
                        item.SetIsClosable(self.closable.get())?;
                        list.Append(&item.cast::<IInspectable>()?)?;
                        items.push(item);
                    }
                }
            }
            while items.len() > titles.len() {
                items.pop();
                list.RemoveAtEnd()?;
            }
            *self.items.borrow_mut() = items;
            Ok(())
        })();
        self.applying.set(before);
        result?;
        let shown = self.shown.get().filter(|i| *i < titles.len());
        self.set_selected(shown)
    }

    pub(crate) fn titles(&self) -> Vec<String> {
        self.items.borrow().iter().map(|item| unboxed(item.Header()).unwrap_or_default()).collect()
    }

    /// Chooses a tab without reporting it.
    pub(crate) fn set_selected(&self, index: Option<usize>) -> R<()> {
        self.shown.set(index);
        select(&self.view, &self.applying, index)
    }

    /// The tab the view shows chosen.
    pub(crate) fn selected(&self) -> Option<usize> {
        selected(&self.view, self.items.borrow().len())
    }

    pub(crate) fn set_closable(&self, closable: bool) -> R<()> {
        self.closable.set(closable);
        for item in self.items.borrow().iter() {
            item.SetIsClosable(closable)?;
        }
        Ok(())
    }

    /// Whether tabs close, as the items say while there are some.
    pub(crate) fn closable(&self) -> bool {
        match self.items.borrow().first() {
            Some(item) => item.IsClosable().unwrap_or(false),
            None => self.closable.get(),
        }
    }

    /// Dragging to reorder; tabs still can't be dragged out of the window.
    pub(crate) fn set_reorderable(&self, reorderable: bool) -> R<()> {
        self.view.SetCanReorderTabs(reorderable)?;
        self.view.SetAllowDropTabs(reorderable)
    }

    pub(crate) fn reorderable(&self) -> bool {
        self.view.CanReorderTabs().unwrap_or(false)
    }

    pub(crate) fn new_tab_button(&self) -> bool {
        self.view.IsAddTabButtonVisible().unwrap_or(false)
    }

    /// The strip's natural size, where it's in the content.
    pub(crate) fn strip(&self) -> w::Size {
        if self.window.get().is_some() { w::Size::default() } else { natural(&self.view) }
    }

    /// Takes the view out of the node's grid, for the title bar.
    pub(crate) fn leave_grid(&self) -> R<()> {
        let children = self.grid.cast::<w::IPanel>()?.Children()?;
        let mut at = 0;
        if children.IndexOf(&self.view.cast::<w::UIElement>()?, &mut at)? {
            children.RemoveAt(at)?;
        }
        Ok(())
    }

    /// Puts the view back in the node's grid, stretched over it.
    pub(crate) fn back_to_grid(&self) -> R<()> {
        let fe: w::IFrameworkElement = self.view.cast()?;
        fe.SetHorizontalAlignment(w::HorizontalAlignment::Stretch)?;
        fe.SetVerticalAlignment(w::VerticalAlignment::Stretch)?;
        self.grid.cast::<w::IPanel>()?.Children()?.Append(&self.view.cast::<w::UIElement>()?)
    }

    /// Chooses the first tab with this title as a click on it does, which
    /// the view reports. `false` if there's none.
    pub(crate) fn choose(&self, title: &str, emitter: &Events, id: NodeId) -> R<bool> {
        let Some(index) = self.titles().iter().position(|t| t == title) else { return Ok(false) };
        self.view.SetSelectedIndex(index as i32)?;
        // In case XAML reports it later.
        report(&self.view, &self.items.borrow(), &self.shown, emitter, id);
        Ok(true)
    }

    /// Moves a tab as a drag does: out, then in at `to`, which the view
    /// reports as the user's move.
    pub(crate) fn move_tab(&self, from: usize, to: usize) -> R<()> {
        let list = self.view.TabItems()?;
        let item = list.GetAt(from as u32)?;
        list.RemoveAt(from as u32)?;
        list.InsertAt(to as u32, &item)
    }

    /// What takes keyboard focus in it: the chosen tab, or the first.
    pub(crate) fn focus_target(&self) -> Option<w::IUIElement> {
        let items = self.items.borrow();
        let index = self.selected().unwrap_or(0);
        items.get(index)?.cast().ok()
    }
}
