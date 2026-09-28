//! A tab view: a `SelectorBar`, as Windows' own apps switch views (Photos,
//! the Microsoft Store), over its pages. WinUI's `TabView` is for documents
//! (closable, reorderable tabs), so it isn't the one. The view is a
//! `Canvas`: the bar at its top-left at its own size, then the pages' hosts
//! below it, all in one place, the shown one `Visible` and the others
//! `Collapsed`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventValue, NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// The bar's height until one is measured: `SelectorBarPadding` (4 above
/// and below), `SelectorBarItemPadding` (10 above, 7 below), a line of body
/// text (20) and the selection pill (3).
pub(crate) const BAR_HEIGHT: f32 = 48.0;

/// The bar's height as measured from a real one, shared by every tab view
/// and the metrics: `None` until one has loaded.
pub(crate) type BarHeight = Rc<Cell<Option<f32>>>;

/// The tab view node's native parts.
pub(crate) struct Tabs {
    pub canvas: w::Canvas,
    pub bar: w::SelectorBar,
    /// One per title, in order: what the bar's selection is compared with.
    items: Rc<RefCell<Vec<w::SelectorBarItem>>>,
    /// The pages' hosts, in order.
    pages: Rc<RefCell<Vec<w::UIElement>>>,
    /// The page shown, set by the core or reported to it:
    /// `SelectionChanged` fires for the app's changes too.
    shown: Rc<Cell<Option<usize>>>,
    id: NodeId,
    emitter: Events,
    height: BarHeight,
    _selection: EventRevoker,
    _loaded: EventRevoker,
}

/// Which of the items `selected` is.
fn index_of(items: &[w::SelectorBarItem], selected: Option<w::SelectorBarItem>) -> Option<usize> {
    let selected = selected.filter(|s| !s.as_raw().is_null())?;
    items.iter().position(|item| item.as_raw() == selected.as_raw())
}

/// Shows the page at `shown`, and collapses the others.
fn show(pages: &[w::UIElement], shown: Option<usize>) {
    for (index, page) in pages.iter().enumerate() {
        let visibility = if Some(index) == shown { w::Visibility::Visible } else { w::Visibility::Collapsed };
        _ = page.cast::<w::IUIElement>().and_then(|p| p.SetVisibility(visibility));
    }
}

/// The bar's natural size, once it's in a window and has its template.
fn natural(bar: &w::IUIElement) -> w::Size {
    _ = bar.Measure(w::Size { width: f32::INFINITY, height: f32::INFINITY });
    bar.DesiredSize().unwrap_or_default()
}

/// A selection the core doesn't know about is the user's: the page shows
/// and it's reported.
fn report(
    bar: &w::SelectorBar,
    items: &RefCell<Vec<w::SelectorBarItem>>,
    pages: &RefCell<Vec<w::UIElement>>,
    shown: &Cell<Option<usize>>,
    emitter: &Events,
    id: NodeId,
) {
    let Some(index) = index_of(&items.borrow(), bar.SelectedItem().ok()) else { return };
    if shown.replace(Some(index)) != Some(index) {
        show(&pages.borrow(), Some(index));
        emitter.emit(id, UiEvent::Changed(EventValue::Index(index)));
    }
}

impl Tabs {
    pub(crate) fn new(id: NodeId, emitter: Events, height: BarHeight) -> R<Tabs> {
        let canvas = w::Canvas::new()?;
        let bar = w::SelectorBar::new()?;
        canvas.cast::<w::IPanel>()?.Children()?.Append(&bar.cast::<w::UIElement>()?)?;
        let items = Rc::new(RefCell::new(Vec::<w::SelectorBarItem>::new()));
        let pages = Rc::new(RefCell::new(Vec::<w::UIElement>::new()));
        let shown = Rc::new(Cell::new(None));
        // Read from the bar, not the event: XAML may report a set late,
        // and by then the core may have set another.
        let selection = bar.SelectionChanged({
            let (items, pages, shown, emitter) = (items.clone(), pages.clone(), shown.clone(), emitter.clone());
            move |sender, _| {
                if let Some(bar) = sender.as_ref() {
                    report(bar, &items, &pages, &shown, &emitter, id);
                }
            }
        })?;
        // Until it's in a window, it has no template and measures empty:
        // measure it again then. The first bar with tabs gives the height
        // the core leaves above every tab view's pages.
        let loaded = bar.cast::<w::IFrameworkElement>()?.Loaded({
            let (emitter, height, items) = (emitter.clone(), height.clone(), items.clone());
            move |sender, _| {
                emitter.emit(id, UiEvent::Remeasure);
                let Some(bar) = sender.as_ref().and_then(|s| s.cast::<w::IUIElement>().ok()) else { return };
                if items.borrow().is_empty() {
                    return;
                }
                let measured = natural(&bar).height.ceil();
                if measured > 0.0 && height.replace(Some(measured)) != Some(measured) {
                    emitter.emit(id, UiEvent::MetricsChanged);
                }
            }
        })?;
        Ok(Tabs { canvas, bar, items, pages, shown, id, emitter, height, _selection: selection, _loaded: loaded })
    }

    /// New titles, relabelling the tabs there are: the selected one stays
    /// (unless it's gone), which the core sends again if it moved.
    pub(crate) fn set_titles(&self, titles: &[String]) -> R<()> {
        let list = self.bar.Items()?;
        // Not borrowed while the bar changes: removing the selected item
        // reports it.
        let mut items = self.items.borrow().clone();
        for (index, title) in titles.iter().enumerate() {
            match items.get(index) {
                Some(item) => item.SetText(title)?,
                None => {
                    let item = w::SelectorBarItem::new()?;
                    item.SetText(title)?;
                    list.Append(&item)?;
                    items.push(item);
                }
            }
        }
        while items.len() > titles.len() {
            items.pop();
            list.RemoveAtEnd()?;
        }
        *self.items.borrow_mut() = items;
        self.set_selected(self.shown.get())
    }

    pub(crate) fn titles(&self) -> Vec<String> {
        self.items.borrow().iter().map(|item| item.Text().unwrap_or_default()).collect()
    }

    /// Shows a page without reporting it.
    pub(crate) fn set_selected(&self, index: Option<usize>) -> R<()> {
        self.shown.set(index);
        let item = index.and_then(|i| self.items.borrow().get(i).cloned());
        match item {
            Some(item) => self.bar.SetSelectedItem(&item)?,
            None => self.bar.SetSelectedItem(None::<&w::SelectorBarItem>)?,
        }
        show(&self.pages.borrow(), index);
        Ok(())
    }

    /// The tab the bar shows selected.
    pub(crate) fn selected(&self) -> Option<usize> {
        index_of(&self.items.borrow(), self.bar.SelectedItem().ok())
    }

    /// Adds a page's host at `index`, after the bar; it shows if it's the
    /// selected page.
    pub(crate) fn insert(&self, index: usize, host: &w::UIElement) -> R<()> {
        let index = index.min(self.pages.borrow().len());
        self.canvas.cast::<w::IPanel>()?.Children()?.InsertAt(index as u32 + 1, host)?;
        self.pages.borrow_mut().insert(index, host.clone());
        show(&self.pages.borrow(), self.shown.get());
        Ok(())
    }

    pub(crate) fn remove(&self, host: &w::UIElement) -> R<()> {
        let children = self.canvas.cast::<w::IPanel>()?.Children()?;
        let mut at = 0;
        if children.IndexOf(host, &mut at)? {
            children.RemoveAt(at)?;
        }
        self.pages.borrow_mut().retain(|page| page.as_raw() != host.as_raw());
        show(&self.pages.borrow(), self.shown.get());
        Ok(())
    }

    /// Whether a page's host is the one shown.
    pub(crate) fn shows(host: &w::UIElement) -> bool {
        host.cast::<w::IUIElement>().and_then(|h| h.Visibility()).is_ok_and(|v| v == w::Visibility::Visible)
    }

    /// Where the pages go: below the bar, at the height the core left for
    /// it.
    pub(crate) fn page_origin(&self) -> (f64, f64) {
        (0.0, self.height.get().unwrap_or(BAR_HEIGHT) as f64)
    }

    /// The bar's natural size: the tab view's with no page.
    pub(crate) fn strip(&self) -> w::Size {
        self.bar.cast::<w::IUIElement>().map(|bar| natural(&bar)).unwrap_or_default()
    }

    /// Shows the first page with this title as a click on its tab does,
    /// which the bar reports. `false` if there's none.
    pub(crate) fn choose(&self, title: &str) -> R<bool> {
        let item = self.items.borrow().iter().find(|item| item.Text().is_ok_and(|t| t == title)).cloned();
        let Some(item) = item else { return Ok(false) };
        self.bar.SetSelectedItem(&item)?;
        // In case XAML reports it later.
        report(&self.bar, &self.items, &self.pages, &self.shown, &self.emitter, self.id);
        Ok(true)
    }

    /// The bar in a tab view's canvas, if `element` is one: always its
    /// first child.
    pub(crate) fn bar_in(element: &w::IUIElement) -> Option<w::SelectorBar> {
        let children = element.cast::<w::IPanel>().ok()?.Children().ok()?;
        if children.Size().ok()? == 0 {
            return None;
        }
        children.GetAt(0).ok()?.cast().ok()
    }

    /// What takes keyboard focus in it: the selected tab, or the first, as
    /// Tab reaches a selector bar.
    pub(crate) fn focus_target(bar: &w::SelectorBar) -> Option<w::IUIElement> {
        let selected = bar.SelectedItem().ok().filter(|s| !s.as_raw().is_null());
        let first = || {
            let items = bar.Items().ok()?;
            (items.Size().ok()? > 0).then(|| items.GetAt(0).ok()).flatten()
        };
        selected.or_else(first)?.cast().ok()
    }
}
