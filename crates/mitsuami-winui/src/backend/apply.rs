//! Applying commands, and what the backend does after a batch.

use mitsuami_core::a11y::A11yProps;
use mitsuami_core::{Command, EventValue, NodeId, UiEvent, WidgetKind};
use windows_core::{IInspectable, Interface};

use super::fields::{box_text, search_text};
use super::scroll::{scroll_now, shift_wheel};
use super::windows::{
    CONTENT_ROW, apply_min_size, client_insets, in_full_screen, insert_toolbar_item, remove_sidebar,
    remove_toolbar_item, resize_client, resize_client_with, scale_of, update_toolbar,
};
use super::{R, State, Widget, WindowParts, key, set_help_text, set_hit_testable, violation};
use crate::bindings as w;

impl State {
    fn element(&self, id: NodeId, command: &Command) -> w::UIElement {
        match self.nodes.get(&id) {
            Some(node) => node.element.clone(),
            None => violation(command, &format!("node {id} does not exist")),
        }
    }

    /// Runs the node's raw settings, if the app gave any, after its props:
    /// what they set wins.
    fn run_tweak(&self, id: NodeId) -> R<()> {
        let node = &self.nodes[&id];
        match node.tweak.as_ref().and_then(|tweak| tweak.downcast_ref::<crate::tweak::TweakFn>()) {
            // A group's is its card's.
            Some(run) if let Widget::Group(group) = &node.widget => run(&group.card.cast()?),
            // A table's is its list view, not the grid around it.
            Some(run) => run(node.inner.as_ref().unwrap_or(&node.element)),
            None => Ok(()),
        }
    }

    pub(super) fn apply(&mut self, command: &Command) -> R<()> {
        match command {
            Command::Create { id, kind, props } => {
                if self.nodes.contains_key(id) {
                    violation(command, "node already exists");
                }
                self.create(*id, *kind, command)?;
                for prop in props {
                    self.set_prop(*id, prop, command)?;
                }
                self.run_tweak(*id)?;
            }
            Command::SetProp { id, prop } => {
                self.set_prop(*id, prop, command)?;
                self.run_tweak(*id)?;
            }
            Command::Insert { parent, child, index } => {
                let child_element = self.element(*child, command);
                if self.nodes[child].parent.is_some() {
                    violation(command, "child is still attached");
                }
                if let Widget::Sidebar(sidebar) = &self.nodes[child].widget {
                    let view = sidebar.view.clone();
                    let Some(Widget::Window(parts)) = self.nodes.get(parent).map(|n| &n.widget) else {
                        violation(command, "a sidebar goes in a window")
                    };
                    if parts.sidebar.is_some() {
                        violation(command, "a window has one sidebar");
                    }
                    let (revokers, place) = sidebar.follow_title_bar(&parts.title_bar)?;
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        unreachable!()
                    };
                    // The view takes the host's place, with the host as
                    // its content, and fills it (not our zero frame).
                    let children = parts.root.cast::<w::IPanel>()?.Children()?;
                    let host: w::UIElement = parts.host.cast()?;
                    let mut at = 0;
                    if children.IndexOf(&host, &mut at)? {
                        children.RemoveAt(at)?;
                    }
                    let fe: w::IFrameworkElement = view.cast()?;
                    fe.SetWidth(f64::NAN)?;
                    fe.SetHeight(f64::NAN)?;
                    w::Grid::SetRow(&view.cast::<w::FrameworkElement>()?, CONTENT_ROW)?;
                    view.cast::<w::IContentControl>()?.SetContent(&host)?;
                    children.Append(&view.cast::<w::UIElement>()?)?;
                    parts.sidebar_revokers = revokers;
                    parts.sidebar_place = Some(place);
                    parts.sidebar = Some((*child, view));
                    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
                    // The content keeps its size: the window grows by the
                    // pane.
                    if let Some(size) = parts.requested.or(parts.size.get()) {
                        resize_client(parts, size);
                    }
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return Ok(());
                }
                if self.nodes[child].kind == WidgetKind::ToolbarItem {
                    // Items come after the window's content, and before its sidebar.
                    let content = self
                        .nodes
                        .values()
                        .filter(|n| {
                            n.parent == Some(*parent)
                                && !matches!(n.kind, WidgetKind::ToolbarItem | WidgetKind::Sidebar)
                        })
                        .count();
                    let Some(index) = index.checked_sub(content) else {
                        violation(command, "toolbar items go after the window's content")
                    };
                    let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget) else {
                        violation(command, "toolbar items go in windows")
                    };
                    insert_toolbar_item(parts, *child, &child_element, index)?;
                    self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                    return Ok(());
                }
                match &self.nodes.get(parent).map(|n| &n.widget) {
                    Some(Widget::Tabs(tabs)) => {
                        if self.nodes[child].kind != WidgetKind::Container {
                            violation(command, "a Tabs' children are page hosts (Containers)");
                        }
                        tabs.insert(*index, &child_element)?;
                    }
                    // After its heading and card, which are behind it.
                    Some(Widget::Group(group)) => {
                        let children = group.canvas.cast::<w::IPanel>()?.Children()?;
                        let index = (*index as u32 + crate::group::PARTS).min(children.Size()?);
                        children.InsertAt(index, &child_element)?;
                    }
                    Some(Widget::List(list)) => {
                        let Some(row) = self.nodes[child].row else {
                            violation(command, "a List's children are row hosts, a Table's cell hosts")
                        };
                        list.insert(row, self.nodes[child].column.unwrap_or(0), *child, child_element.clone());
                    }
                    Some(Widget::Scroll(scroll)) => {
                        let content = scroll.cast::<w::IContentControl>()?;
                        if content.Content().is_ok_and(|c| !c.as_raw().is_null()) {
                            violation(command, "a ScrollView has a single native child (its content)");
                        }
                        // The content has a fixed size: at the default Stretch,
                        // XAML centers it when it's smaller than the viewport.
                        let fe: w::IFrameworkElement = child_element.cast()?;
                        fe.SetHorizontalAlignment(w::HorizontalAlignment::Left)?;
                        fe.SetVerticalAlignment(w::VerticalAlignment::Top)?;
                        content.SetContent(&child_element)?;
                        let wheel = shift_wheel(scroll, &child_element)?;
                        self.nodes.get_mut(parent).unwrap().shift_wheel = Some(wheel);
                        let child_node = self.nodes.get_mut(child).unwrap();
                        child_node.scroll_content = true;
                        set_hit_testable(child_node)?;
                    }
                    Some(_) => {
                        let children = self.children(*parent, command)?;
                        let index = (*index as u32).min(children.Size()?);
                        children.InsertAt(index, &child_element)?;
                    }
                    None => violation(command, "parent does not exist"),
                }
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
            }
            Command::Remove { parent, child } => {
                if self.nodes.get(child).and_then(|n| n.parent) != Some(*parent) {
                    violation(command, "not a child of this parent");
                }
                let child_element = self.element(*child, command);
                if self.nodes[child].kind == WidgetKind::ToolbarItem
                    && let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget)
                {
                    remove_toolbar_item(parts, *child)?;
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return Ok(());
                }
                if self.nodes[child].kind == WidgetKind::Sidebar
                    && let Some(Widget::Window(parts)) = self.nodes.get_mut(parent).map(|n| &mut n.widget)
                {
                    remove_sidebar(parts)?;
                    self.nodes.get_mut(child).unwrap().parent = None;
                    return Ok(());
                }
                match &self.nodes[parent].widget {
                    Widget::Tabs(tabs) => tabs.remove(&child_element)?,
                    Widget::List(list) => list.remove(
                        self.nodes[child].row.expect("inserted with a row"),
                        self.nodes[child].column.unwrap_or(0),
                    ),
                    Widget::Scroll(scroll) => {
                        scroll.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?;
                        self.nodes.get_mut(parent).unwrap().shift_wheel = None;
                        let child_node = self.nodes.get_mut(child).unwrap();
                        child_node.scroll_content = false;
                        set_hit_testable(child_node)?;
                    }
                    _ => {
                        let children = self.children(*parent, command)?;
                        let mut index = 0;
                        if children.IndexOf(&child_element, &mut index)? {
                            children.RemoveAt(index)?;
                        }
                    }
                }
                self.nodes.get_mut(child).unwrap().parent = None;
            }
            Command::Destroy { id } => {
                if let Some(parts) = self.window_of(*id)
                    && parts.focus.get() == Some(*id)
                {
                    parts.focus.set(None);
                }
                let Some(node) = self.nodes.remove(id) else { violation(command, "node does not exist") };
                self.by_element.borrow_mut().remove(&key(&node.element));
                self.pending_show.retain(|w| w != id);
                self.menus.windows.remove(id);
                drop(node.revokers);
                // The app's handle may keep its child window: it just stops
                // showing.
                if let Widget::GpuSurface(surface) = &node.widget {
                    surface.detach();
                }
                if let Widget::Window(parts) = node.widget {
                    let WindowParts { window, menu_revokers, modal, disabled, .. } = *parts;
                    drop(menu_revokers);
                    // What it disabled comes back before it closes, so its
                    // owner is the window that becomes active.
                    for other in disabled {
                        unsafe { _ = w::EnableWindow(other, true.into()) };
                    }
                    window.cast::<w::IWindow>()?.Close()?;
                    if let Some(Some(Widget::Window(owner))) =
                        modal.and_then(|(owner, _)| owner).map(|o| self.nodes.get(&o).map(|n| &n.widget))
                    {
                        unsafe { _ = w::SetForegroundWindow(owner.hwnd) };
                    }
                }
            }
            Command::SetFrame { id, frame } => {
                let element = self.element(*id, command);
                // A toolbar item is where the toolbar puts it, at this size;
                // an empty one is collapsed.
                if self.nodes[id].kind == WidgetKind::ToolbarItem
                    && let Some(window) = self.nodes[id].parent
                    && let Some(Widget::Window(parts)) = self.nodes.get_mut(&window).map(|n| &mut n.widget)
                {
                    let fe: w::IFrameworkElement = element.cast()?;
                    fe.SetWidth(frame.width() as f64)?;
                    fe.SetHeight(frame.height() as f64)?;
                    if let Some((_, container)) = parts.toolbar_items.iter().find(|(item, _)| item == id) {
                        let empty = frame.size.is_empty();
                        let visibility = if empty { w::Visibility::Collapsed } else { w::Visibility::Visible };
                        container.cast::<w::IUIElement>()?.SetVisibility(visibility)?;
                    }
                    update_toolbar(parts)?;
                    return Ok(());
                }
                // A page goes where its tab view shows pages, at this size.
                let (x, y) = match self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget) {
                    Some(Widget::Tabs(tabs)) => tabs.page_origin(),
                    _ => (frame.x() as f64, frame.y() as f64),
                };
                w::Canvas::SetLeft(&element, x)?;
                w::Canvas::SetTop(&element, y)?;
                let fe: w::IFrameworkElement = element.cast()?;
                fe.SetWidth(frame.width() as f64)?;
                fe.SetHeight(frame.height() as f64)?;
                if let Widget::List(list) = &self.nodes[id].widget {
                    list.set_width(frame.width());
                }
                if let Widget::Group(group) = &self.nodes[id].widget {
                    group.place(frame.width() as f64, frame.height() as f64)?;
                }
                if let (Some(row), Some(Widget::List(list))) =
                    (self.nodes[id].row, self.nodes[id].parent.and_then(|p| self.nodes.get(&p)).map(|p| &p.widget))
                {
                    list.set_row_height(row, self.nodes[id].column.unwrap_or(0), frame.height());
                }
            }
            Command::SetA11y { id, a11y } => {
                self.element(*id, command);
                // On the control itself, not the Border a native render sits in.
                let element = self.nodes[id].control().clone();
                let A11yProps { label, description, hidden, .. } = a11y;
                // An empty name means "derive it from the content".
                let named_by_label = matches!(
                    self.nodes[id].widget,
                    Widget::Switch(_)
                        | Widget::Select(_)
                        | Widget::RadioGroup(_)
                        | Widget::Slider { .. }
                        | Widget::Number { .. }
                        | Widget::Progress(_)
                        | Widget::Spinner(_)
                );
                if !named_by_label || label.is_some() {
                    w::AutomationProperties::SetName(&element, label.as_deref().unwrap_or(""))?;
                }
                let node = self.nodes.get_mut(id).unwrap();
                node.description = description.clone();
                set_help_text(node)?;
                w::AutomationProperties::SetAccessibilityView(
                    &element,
                    if *hidden { w::AccessibilityView::Raw } else { w::AccessibilityView::Content },
                )?;
            }
            Command::SetWindowSize { id, size } => match self.nodes.get_mut(id).map(|n| &mut n.widget) {
                Some(Widget::Window(parts)) => {
                    if !in_full_screen(&parts.app_window) {
                        parts.requested = Some(*size);
                        // A locked height moves first, which Windows may
                        // apply itself: the resize uses the insets from
                        // before (see `apply_min_size`).
                        let insets = client_insets(parts, scale_of(parts));
                        if parts.height_locked {
                            apply_min_size(parts);
                        }
                        resize_client_with(parts, *size, insets);
                    }
                }
                _ => violation(command, "not a window"),
            },
            Command::SetFocusOrder { window, order } => match self.nodes.get(window).map(|n| &n.widget) {
                // XAML scopes TabIndex to each container, so it can't express
                // a window-wide order across nested hosts: Tab is handled on
                // the window's root instead (see `tab`).
                Some(Widget::Window(parts)) => *parts.tab_order.borrow_mut() = order.clone(),
                _ => violation(command, "not a window"),
            },
            Command::ScrollTo { id, offset } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::Scroll(scroll)) => {
                    let node = &self.nodes[id];
                    scroll_now(&self.emitter, *id, &node.offset, &scroll.cast()?, *offset)?;
                }
                Some(Widget::List(list)) => list.scroll_to(*offset)?,
                _ => violation(command, "not a ScrollView or List"),
            },
            Command::Focus { id } => {
                self.element(*id, command);
                let focused = self.focus(*id, w::FocusState::Programmatic);
                if let Some(parts) = self.window_of(*id) {
                    *parts.wanted_focus.borrow_mut() = (!focused).then_some((*id, None));
                }
            }
            Command::SelectText { id, range } => {
                let Some(node) = self.nodes.get(id) else { violation(command, "node does not exist") };
                if !matches!(
                    node.widget,
                    Widget::Field(_) | Widget::TextArea { .. } | Widget::Password(_) | Widget::Search(_)
                ) {
                    violation(command, "not a text field or text area");
                }
                // Selected once it has focus, if it's still waiting for it.
                match self.window_of(*id).map(|p| p.wanted_focus.borrow_mut()) {
                    Some(mut wanted) if wanted.as_ref().is_some_and(|(w, _)| w == id) => {
                        *wanted = Some((*id, Some(range.clone())));
                    }
                    _ => super::selection::select(node, range.clone())?,
                }
            }
            Command::ScrollToRow { id, row } => match self.nodes.get(id).map(|n| &n.widget) {
                Some(Widget::List(list)) => list.scroll_to_row(*row)?,
                _ => violation(command, "not a List"),
            },
        }
        Ok(())
    }

    /// Lets list views lay out now rather than at XAML's next layout pass:
    /// they realise the containers of the rows in view, and the rows they
    /// report are built in the same run-loop turn. Their handlers only
    /// touch their own data and emit.
    /// Brings scroll views' content into XAML's live tree. A new
    /// `ScrollViewer` shows its content only once a layout pass has applied
    /// its template, and XAML measures only elements in the live tree: until
    /// then, controls in it measured as if untemplated (buttons 0 wide).
    /// Scroll views inside scroll views connect one level per pass.
    pub(super) fn connect_scroll_content(&self) {
        let live = |element: &w::UIElement| {
            element.cast::<w::IUIElement>().and_then(|e| e.XamlRoot()).is_ok_and(|r| !r.as_raw().is_null())
        };
        loop {
            let waiting = self.nodes.values().find_map(|node| {
                let Widget::Scroll(scroll) = &node.widget else { return None };
                let content = scroll.cast::<w::IContentControl>().ok()?.Content().ok()?;
                let content: w::UIElement = content.cast().ok()?;
                (live(&node.element) && !live(&content)).then(|| node.element.clone())
            });
            let Some(scroll) = waiting else { break };
            _ = scroll.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
            let content = scroll.cast::<w::IContentControl>().and_then(|c| c.Content()).and_then(|c| c.cast());
            if !content.is_ok_and(|c: w::UIElement| live(&c)) {
                // Nothing more to do this batch; don't spin.
                break;
            }
        }
    }

    pub(super) fn layout_lists(&self) {
        for node in self.nodes.values() {
            if let Widget::List(list) = &node.widget {
                list.layout();
            }
        }
    }

    /// The window `id` is in (or is).
    /// Gives GPU surfaces that are now in a window their child windows.
    pub(super) fn attach_surfaces(&self) {
        for (id, node) in &self.nodes {
            if let Widget::GpuSurface(surface) = &node.widget
                && !surface.is_attached()
                && let Some(parts) = self.window_of(*id)
                && let Err(error) = surface.attach(parts.hwnd)
            {
                panic!("winui backend: a GpuSurface's child window: {error}");
            }
        }
    }

    pub(super) fn window_of(&self, id: NodeId) -> Option<&WindowParts> {
        let mut current = Some(id);
        while let Some(id) = current {
            let node = self.nodes.get(&id)?;
            if let Widget::Window(parts) = &node.widget {
                return Some(parts);
            }
            current = node.parent;
        }
        None
    }

    /// Reports a value the user changed through us (a toggle, typing) right
    /// away; XAML's change events arrive later and find it reported.
    pub(super) fn report_value(&self, id: NodeId) {
        let Some(node) = self.nodes.get(&id) else { return };
        let changed = match &node.widget {
            Widget::Field(f) | Widget::TextArea { field: f, .. } => {
                f.cast::<w::ITextBox>().and_then(|f| box_text(&f)).ok().and_then(|text| {
                    let mut shown = node.shown_text.borrow_mut();
                    (*shown != text).then(|| {
                        *shown = text.clone();
                        EventValue::Text(text)
                    })
                })
            }
            Widget::Password(f) => f.cast::<w::IPasswordBox>().and_then(|f| f.Password()).ok().and_then(|text| {
                let mut shown = node.shown_text.borrow_mut();
                (*shown != text).then(|| {
                    *shown = text.clone();
                    EventValue::Text(text)
                })
            }),
            Widget::Search(s) => search_text(s).and_then(|text| {
                let mut shown = node.shown_text.borrow_mut();
                (*shown != text).then(|| {
                    *shown = text.clone();
                    EventValue::Text(text)
                })
            }),
            Widget::Slider { slider, .. } => slider
                .cast::<w::IRangeBase>()
                .and_then(|r| r.Value())
                .ok()
                .and_then(|v| (node.shown_number.replace(v) != v).then_some(EventValue::Number(v))),
            Widget::Number { number, .. } => number
                .cast::<w::INumberBox>()
                .and_then(|n| n.Value())
                .ok()
                .filter(|v| !v.is_nan())
                .and_then(|v| (node.shown_number.replace(v) != v).then_some(EventValue::Number(v))),
            Widget::Select(combo) => combo
                .cast::<w::ISelector>()
                .and_then(|s| s.SelectedIndex())
                .ok()
                .and_then(|i| (i >= 0 && node.shown_index.replace(i) != i).then_some(EventValue::Index(i as usize))),
            Widget::RadioGroup(group) => group
                .SelectedIndex()
                .ok()
                .and_then(|i| (i >= 0 && node.shown_index.replace(i) != i).then_some(EventValue::Index(i as usize))),
            Widget::Checkbox(_) | Widget::Switch(_) | Widget::Toggle(_) => {
                let value = match &node.widget {
                    Widget::Checkbox(b) => b.cast::<w::IToggleButton>().and_then(|b| b.IsChecked()).ok(),
                    Widget::Toggle(b) => b.cast::<w::IToggleButton>().and_then(|b| b.IsChecked()).ok(),
                    Widget::Switch(s) => s.cast::<w::IToggleSwitch>().and_then(|s| s.IsOn()).ok(),
                    _ => None,
                };
                value.and_then(|v| {
                    let was_mixed = node.shown_mixed.replace(false);
                    (node.shown_checked.replace(v) != v || was_mixed).then_some(EventValue::Bool(v))
                })
            }
            _ => None,
        };
        if let Some(value) = changed {
            // A search box's edit is a search too, as its TextChanged is.
            let search = match (&node.widget, &value) {
                (Widget::Search(_), EventValue::Text(text)) => Some(text.clone()),
                _ => None,
            };
            self.emitter.emit(id, UiEvent::Changed(value));
            if let Some(text) = search {
                self.emitter.emit(id, UiEvent::Search(text));
            }
        }
    }

    fn children(&self, parent: NodeId, command: &Command) -> R<w::UIElementCollection> {
        match &self.nodes.get(&parent).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => parts.host.cast::<w::IPanel>()?.Children(),
            Some(Widget::Host(canvas)) => canvas.cast::<w::IPanel>()?.Children(),
            Some(Widget::Group(group)) => group.canvas.cast::<w::IPanel>()?.Children(),
            Some(_) => violation(command, "not a container"),
            None => violation(command, "node does not exist"),
        }
    }
}
