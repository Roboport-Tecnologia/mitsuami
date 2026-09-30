//! Reading back what native widgets show.

use mitsuami_core::backend::NativeState;
use mitsuami_core::{HorizontalAlign, InputPurpose, LayoutDirection};
use mitsuami_core::{NodeId, Orientation, Point, Prop, Rect, WidgetKind};
use windows_core::{IInspectable, Interface};

use super::controls::{button_content, option_texts, radio_options};
use super::fields::{box_text, search_text};
use super::focus::is_control;
use super::menus::read_menu;
use super::scroll::{scroll_axes, scroll_bars};
use super::styles::weight_of;
use super::windows::{height_locked, in_full_screen, is_maximized, overlapped, sidebar_frame, toolbar_item_frame};
use super::{Widget, WinUiBackend, key, unboxed};
use crate::bindings as w;

impl WinUiBackend {
    pub(super) fn read_native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        let mut props = Vec::new();
        // What XAML shows for controls; what the core gave for the rest,
        // which stay left to right (see `Prop::LayoutDirection`).
        let flow = node.control().cast::<w::IFrameworkElement>().ok()?.FlowDirection().ok()?;
        props.push(Prop::LayoutDirection(match (node.direction, flow) {
            (Some(direction), w::FlowDirection::LeftToRight) => direction,
            (_, w::FlowDirection::RightToLeft) => LayoutDirection::RightToLeft,
            _ => LayoutDirection::LeftToRight,
        }));
        match &node.widget {
            Widget::Window(parts) => {
                props.push(Prop::Title(parts.window.cast::<w::IWindow>().ok()?.Title().ok()?));
                props.extend(parts.modal.map(|(owner, modality)| Prop::Modal { owner, modality }));
                // Until it's shown, what it will show.
                let full = if parts.shown { in_full_screen(&parts.app_window) } else { parts.full_screen.get() };
                props.push(Prop::FullScreen(full));
                props.extend(parts.min_size.map(Prop::MinSize));
                props.push(Prop::HeightFollowsContent(height_locked(parts)));
                // Until it's shown, and in full screen, what it will show.
                let shows = parts.shown && !in_full_screen(&parts.app_window);
                props.push(Prop::Maximized(if shows {
                    is_maximized(&parts.app_window)
                } else {
                    parts.maximized.get()
                }));
                let resizable = overlapped(&parts.app_window).and_then(|p| p.IsResizable().ok());
                props.push(Prop::Resizable(
                    resizable.filter(|_| !in_full_screen(&parts.app_window)).unwrap_or(parts.resizable),
                ));
            }
            Widget::Label(l) => {
                let text: w::ITextBlock = l.cast().ok()?;
                props.push(Prop::Text(text.Text().ok()?));
                let lines = text.MaxLines().ok()?;
                props.push(Prop::MaxLines((lines > 0).then_some(lines as u32)));
                props.push(Prop::FontWeight(weight_of(text.FontWeight().ok()?.weight)));
                props.push(Prop::Italic(text.FontStyle().ok()? == w::FontStyle::Italic));
                props.push(Prop::Selectable(text.IsTextSelectionEnabled().ok()?));
                // Left and right are the start and end of its flow.
                let rtl = flow == w::FlowDirection::RightToLeft;
                props.push(Prop::TextAlign(match (text.TextAlignment().ok()?, rtl) {
                    (w::TextAlignment::Center, _) => HorizontalAlign::Center,
                    (w::TextAlignment::Right, false) | (w::TextAlignment::Left, true) => HorizontalAlign::Right,
                    _ => HorizontalAlign::Left,
                }));
                // Its brush is a theme resource in its style, which can't
                // be told apart from another once resolved.
                props.extend(node.text_color.map(Prop::TextColor));
            }
            Widget::Field(f) | Widget::TextArea { field: f, .. } => {
                let field: w::ITextBox = f.cast().ok()?;
                props.push(Prop::Value(box_text(&field).ok()?));
                if let Widget::TextArea { lines, .. } = &node.widget {
                    props.push(Prop::Lines(*lines));
                    props.push(Prop::LineWrap(field.TextWrapping().ok()? != w::TextWrapping::NoWrap));
                } else {
                    let scope = field.InputScope().ok();
                    let names = scope.and_then(|s| s.cast::<w::IInputScope>().ok()?.Names().ok());
                    let name = names.and_then(|n| n.GetAt(0).ok()?.cast::<w::IInputScopeName>().ok()?.NameValue().ok());
                    props.push(Prop::InputPurpose(match name {
                        Some(w::InputScopeNameValue::EmailSmtpAddress) => InputPurpose::Email,
                        Some(w::InputScopeNameValue::Url) => InputPurpose::Url,
                        Some(w::InputScopeNameValue::TelephoneNumber) => InputPurpose::Phone,
                        _ => InputPurpose::Text,
                    }));
                }
                props.push(Prop::ReadOnly(field.IsReadOnly().ok()?));
                let placeholder = field.PlaceholderText().ok()?;
                if !placeholder.is_empty() {
                    props.push(Prop::Placeholder(placeholder));
                }
            }
            Widget::Search(s) => {
                let search: w::IAutoSuggestBox = s.cast().ok()?;
                props.push(Prop::Value(search_text(s)?));
                let placeholder = search.PlaceholderText().ok()?;
                if !placeholder.is_empty() {
                    props.push(Prop::Placeholder(placeholder));
                }
            }
            Widget::Password(f) => {
                let field: w::IPasswordBox = f.cast().ok()?;
                props.push(Prop::Value(field.Password().ok()?));
                let placeholder = field.PlaceholderText().ok()?;
                if !placeholder.is_empty() {
                    props.push(Prop::Placeholder(placeholder));
                }
            }
            Widget::Button(_) => props.extend(button_content(node)),
            Widget::Toggle(b) => {
                props.extend(button_content(node));
                props.push(Prop::Checked(b.cast::<w::IToggleButton>().ok()?.IsChecked().unwrap_or(false)));
            }
            Widget::MenuButton(b) => {
                props.extend(button_content(node));
                if let Some(menu) = &node.button_menu {
                    let shown = b.cast::<w::IButton>().ok()?.Flyout().ok();
                    let ours =
                        menu.flyout.as_ref().filter(|flyout| shown.as_ref().is_some_and(|s| key(s) == key(*flyout)));
                    props.push(Prop::Menu(match ours {
                        Some(flyout) => read_menu(&flyout.cast::<w::IMenuFlyout>().ok()?.Items().ok()?, menu),
                        None => Vec::new(),
                    }));
                }
            }
            Widget::FileIcon { file, thumbnail, size, .. } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.extend(file.clone().map(Prop::File));
                props.extend(thumbnail.map(Prop::Thumbnail));
                props.extend(size.map(Prop::IconSize));
            }
            Widget::Icon(icon) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let icon = icon.cast::<w::IFontIcon>().ok()?;
                props.push(Prop::Icon(icon.Glyph().ok()?));
                if node.icon_size {
                    props.push(Prop::IconSize(icon.FontSize().ok()? as f32));
                }
                // A theme resource in its style, as for text.
                props.extend(node.text_color.map(Prop::TextColor));
            }
            Widget::Checkbox(b) => {
                props.extend(unboxed(node.element.cast::<w::IContentControl>().ok()?.Content()).map(Prop::Label));
                // `IsChecked` is null while mixed.
                let shown = b.cast::<w::IToggleButton>().ok()?.IsChecked().ok();
                props.push(Prop::Checked(shown.unwrap_or(node.shown_checked.get())));
                if node.mixed.is_some() {
                    props.push(Prop::Mixed(shown.is_none()));
                }
            }
            Widget::Switch(s) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Checked(s.cast::<w::IToggleSwitch>().ok()?.IsOn().ok()?));
            }
            Widget::Slider { slider, step } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let range: w::IRangeBase = slider.cast().ok()?;
                props.push(Prop::Range { min: range.Minimum().ok()?, max: range.Maximum().ok()? });
                props.push(Prop::Step(*step));
                props.push(Prop::Number(range.Value().ok()?));
                if node.orientation.is_some() {
                    let vertical = slider.cast::<w::ISlider>().ok()?.Orientation().ok()? == w::Orientation::Vertical;
                    props.push(Prop::Orientation(if vertical {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    }));
                }
            }
            Widget::Number { number, step } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let iface: w::INumberBox = number.cast().ok()?;
                props.push(Prop::Range { min: iface.Minimum().ok()?, max: iface.Maximum().ok()? });
                props.push(Prop::Step(*step));
                props.push(Prop::Number(iface.Value().ok()?));
                props.push(Prop::WrapAround(iface.IsWrapEnabled().ok()?));
            }
            Widget::Progress(p) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                let indeterminate = p.cast::<w::IProgressBar>().ok()?.IsIndeterminate().ok()?;
                let value = p.cast::<w::IRangeBase>().ok()?.Value().ok()?;
                props.push(Prop::Progress((!indeterminate).then_some(value)));
            }
            Widget::Spinner(ring) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Running(ring.cast::<w::IProgressRing>().ok()?.IsActive().ok()?));
            }
            Widget::Separator(_) => props.extend(node.orientation.map(Prop::Orientation)),
            Widget::GpuSurface(surface) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.extend(surface.props());
            }
            Widget::Image { source, fit, .. } => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.extend(source.clone().map(Prop::Image));
                props.extend(fit.map(Prop::ImageFit));
            }
            Widget::Select(combo) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Options(option_texts(combo)));
                let index = combo.cast::<w::ISelector>().ok()?.SelectedIndex().ok()?;
                props.push(Prop::SelectedIndex(usize::try_from(index).ok()));
            }
            Widget::RadioGroup(group) => {
                let name = w::AutomationProperties::GetName(&node.element).unwrap_or_default();
                if !name.is_empty() {
                    props.push(Prop::Label(name));
                }
                props.push(Prop::Options(radio_options(group)));
                props.push(Prop::SelectedIndex(usize::try_from(group.SelectedIndex().ok()?).ok()));
            }
            Widget::Sidebar(sidebar) => {
                props.push(Prop::Sections(sidebar.sections()));
                props.push(Prop::SelectedIndex(sidebar.selected()));
                props.push(Prop::SidebarShown(sidebar.is_shown()));
            }
            Widget::Group(group) => props.push(Prop::Title(group.title())),
            Widget::Tabs(tabs) => {
                props.push(Prop::TabTitles(tabs.titles()));
                props.push(Prop::TabIcons(tabs.icons()));
                props.push(Prop::SelectedIndex(tabs.selected()));
                props.push(Prop::Enabled(tabs.bar.cast::<w::IControl>().ok()?.IsEnabled().ok()?));
            }
            Widget::Scroll(s) => {
                let scroll: w::IScrollViewer = s.cast().ok()?;
                props.push(Prop::ScrollAxes(scroll_axes(&scroll).ok()?));
                props.push(Prop::ScrollBars(scroll_bars(&scroll).ok()?));
            }
            Widget::Custom { render, props: last } => {
                props.push(Prop::Custom(last.with_props(render.read(node.control(), last.props()))))
            }
            Widget::Drawn { view, props: last } => {
                props.push(Prop::Custom(last.clone()));
                props.push(Prop::Drawing(view.drawing()));
            }
            Widget::Native { last, .. } => props.push(Prop::Native(last.clone())),
            Widget::List(list) => {
                if list.is_table() {
                    props.push(Prop::Columns(list.columns()));
                    props.push(Prop::Sort(list.sort()));
                }
                props.push(Prop::Rows(list.rows()));
                props.extend(list.estimate().map(Prop::EstimatedRowHeight));
                props.push(Prop::SelectionMode(list.mode()));
                props.extend(list.style().map(Prop::ListStyle));
                props.push(Prop::Selected(list.selected()));
                props.extend(list.row_files().map(Prop::RowFiles));
            }
            Widget::Host(_) => props.extend(node.row.map(|row| match node.column {
                Some(column) => Prop::Cell(mitsuami_core::CellKey { row, column }),
                None => Prop::Row(row),
            })),
        }
        if is_control(&node.widget) && !matches!(node.widget, Widget::Scroll(_) | Widget::List(_)) {
            props.push(Prop::Enabled(node.element.cast::<w::IControl>().ok()?.IsEnabled().ok()?));
        }
        props.extend(node.text_style.map(Prop::TextStyle));
        props.extend(node.role.map(Prop::ButtonRole));
        props.extend(node.button_style.map(Prop::ButtonStyle));
        props.extend(node.tabs_style.map(Prop::TabsStyle));
        props.extend(node.tweak.clone().map(Prop::Tweak));
        // "" when it has none.
        props.push(Prop::Tooltip(unboxed(w::ToolTipService::GetToolTip(node.control())).unwrap_or_default()));
        if node.file_drop_sent {
            props.push(Prop::FileDrop(node.file_drop.as_ref().map(|t| t.file_drop())));
        }
        // What its `KeyDown` handler takes: XAML has no keys to read back.
        props.extend(node.keys.as_ref().map(|k| Prop::Keys(k.keys())));
        if let Some(menu) = &node.context_menu {
            let shown = node.control().cast::<w::IUIElement>().ok()?.ContextFlyout().ok();
            // Ours, or the control's own (or none) while the app's is empty.
            let ours = menu.flyout.as_ref().filter(|flyout| shown.as_ref().is_some_and(|s| key(s) == key(*flyout)));
            props.push(Prop::ContextMenu(match ours {
                Some(flyout) => read_menu(&flyout.cast::<w::IMenuFlyout>().ok()?.Items().ok()?, menu),
                None => Vec::new(),
            }));
        }

        let frame = match &node.widget {
            Widget::Window(_) => Rect::ZERO,
            _ => {
                let fe: w::IFrameworkElement = node.element.cast().ok()?;
                let finite = |v: f64| if v.is_nan() { 0.0 } else { v as f32 };
                Rect::new(
                    finite(w::Canvas::GetLeft(&node.element).unwrap_or(0.0)),
                    finite(w::Canvas::GetTop(&node.element).unwrap_or(0.0)),
                    finite(fe.Width().unwrap_or(0.0)),
                    finite(fe.Height().unwrap_or(0.0)),
                )
            }
        };
        // A row is where the list view put it, a toolbar item where the
        // toolbar did.
        let frame = match node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget) {
            Some(Widget::List(list)) if node.column.is_some() => list.cell_rect(&node.element, frame),
            Some(Widget::List(list)) => list.row_rect(&node.element, frame),
            Some(Widget::Window(parts)) if node.kind == WidgetKind::ToolbarItem => {
                toolbar_item_frame(parts, id, &node.element).unwrap_or(frame)
            }
            Some(Widget::Window(parts)) if node.kind == WidgetKind::Sidebar => {
                sidebar_frame(parts).unwrap_or(Rect::ZERO)
            }
            // A page its tab view doesn't show is collapsed: nowhere.
            Some(Widget::Tabs(_)) if !crate::tabs::Tabs::shows(&node.element) => Rect::ZERO,
            _ => frame,
        };
        let by_element = state.by_element.borrow();
        let known = |element: &IInspectable| by_element.get(&key(element)).copied();
        let (children, scroll_offset) = match &node.widget {
            Widget::List(list) => (list.children(), Some(list.scroll_offset())),
            Widget::Scroll(s) => {
                let scroll: w::IScrollViewer = s.cast().ok()?;
                let content = node.element.cast::<w::IContentControl>().ok()?.Content().ok();
                (
                    content.as_ref().and_then(known).into_iter().collect(),
                    Some(Point::new(
                        scroll.HorizontalOffset().unwrap_or(0.0) as f32,
                        scroll.VerticalOffset().unwrap_or(0.0) as f32,
                    )),
                )
            }
            Widget::Window(parts) => {
                let mut children = panel_children(&parts.host, &known);
                children.extend(parts.toolbar_items.iter().map(|(item, _)| *item));
                children.extend(parts.sidebar.as_ref().map(|(sidebar, _)| *sidebar));
                (children, None)
            }
            Widget::Host(canvas) => (panel_children(canvas, &known), None),
            // Its pages; the bar isn't a node.
            Widget::Tabs(tabs) => (panel_children(&tabs.canvas, &known), None),
            // Its content; the heading and card aren't nodes.
            Widget::Group(group) => (panel_children(&group.canvas, &known), None),
            _ => (Vec::new(), None),
        };
        // Composite controls give focus to a part (a list's row container,
        // a number box's text box): the control has it when the window's
        // focus tracking (which walks up to it) says so.
        let tracked = !matches!(node.widget, Widget::Window(_))
            && state.window_of(id).is_some_and(|parts| parts.focus.get() == Some(id));
        let focused = tracked
            || !matches!(node.widget, Widget::Window(_))
                && node
                    .control()
                    .cast::<w::IUIElement>()
                    .and_then(|e| e.FocusState())
                    .is_ok_and(|f| f != w::FocusState::Unfocused);
        let selection = super::selection::selection(node, focused);
        Some(NativeState {
            kind: node.kind,
            props,
            frame,
            parent: node.parent,
            children,
            focused,
            scroll_offset,
            selection,
        })
    }
}

fn panel_children(panel: &w::Canvas, known: &dyn Fn(&IInspectable) -> Option<NodeId>) -> Vec<NodeId> {
    let Ok(children) = panel.cast::<w::IPanel>().and_then(|p| p.Children()) else { return Vec::new() };
    elements(&children).into_iter().filter_map(|c| c.cast::<IInspectable>().ok()).filter_map(|c| known(&c)).collect()
}

pub(super) fn elements(collection: &w::UIElementCollection) -> Vec<w::UIElement> {
    let size = collection.Size().unwrap_or(0);
    (0..size).filter_map(|i| collection.GetAt(i).ok()).collect()
}
