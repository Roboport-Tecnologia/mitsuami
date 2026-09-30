//! Reading back what the native widgets show, for the mirror check.

use mitsuami_core::backend::NativeState;
use mitsuami_core::{Color, HorizontalAlign, ImageFit, NodeId, Orientation, Point, Prop, Rect, ScrollAxes, WidgetKind};
use objc2_app_kit::{
    NSAccessibility, NSCellImagePosition, NSColor, NSColorSpace, NSControlStateValueMixed, NSControlStateValueOn,
    NSFontDescriptorSymbolicTraits, NSImageScaling, NSTextAlignment, NSTextContent, NSTitlePosition, NSView,
    NSWindowStyleMask,
};
use objc2_foundation::NSObjectProtocol;

use super::fonts::font_weight;
use super::menus::pull_down_title;
use super::props::input_purpose;
use super::{State, Widget, key};

pub(super) fn native_state(state: &State, id: NodeId) -> Option<NativeState> {
    let node = state.nodes.get(&id)?;
    let mut props = Vec::new();
    let checked = |s: isize| Prop::Checked(s == NSControlStateValueOn);
    match &node.widget {
        Widget::Window { window, _delegate, .. } => {
            props.push(Prop::Title(window.title().to_string()));
            props.push(Prop::FullScreen(_delegate.full_screen(window)));
            props.push(Prop::MinSize(_delegate.min_size(window)));
            props.push(Prop::HeightFollowsContent(_delegate.height_locked(window)));
            props.push(Prop::Maximized(_delegate.zoomed(window)));
            props.push(Prop::Resizable(window.styleMask().contains(NSWindowStyleMask::Resizable)));
            props.extend(node.modal.map(|(owner, modality)| Prop::Modal { owner, modality }));
        }
        Widget::Label(l) => {
            props.push(Prop::Text(l.stringValue().to_string()));
            props.push(Prop::Selectable(l.isSelectable()));
            let lines = l.maximumNumberOfLines();
            props.push(Prop::MaxLines((lines > 0).then_some(lines as u32)));
            if let Some(font) = l.font() {
                if node.weight.is_some() {
                    props.push(Prop::FontWeight(font_weight(&font)));
                }
                if node.italic.is_some() {
                    let italic = font.fontDescriptor().symbolicTraits();
                    props.push(Prop::Italic(italic.contains(NSFontDescriptorSymbolicTraits::TraitItalic)));
                }
            }
            // The colour it shows, if it's the one given; else what it is.
            if let (Some(sent), Some(shown)) = (node.text_color, l.textColor()) {
                let given = crate::custom::ns_color(sent);
                props.push(Prop::TextColor(if shown.isEqual(Some(&given)) { sent } else { rgba(&shown) }));
            }
            if node.align {
                let align = l.alignment();
                props.push(Prop::TextAlign(if align == NSTextAlignment::Center {
                    HorizontalAlign::Center
                } else if align == NSTextAlignment::Right {
                    HorizontalAlign::Right
                } else {
                    HorizontalAlign::Left
                }));
            }
        }
        Widget::Field(f) => {
            props.push(Prop::Value(f.stringValue().to_string()));
            if let Some(p) = f.placeholderString() {
                props.push(Prop::Placeholder(p.to_string()));
            }
            props.push(Prop::ReadOnly(!f.isEditable()));
            if node.kind == WidgetKind::TextInput {
                props.push(Prop::InputPurpose(input_purpose(f.contentType().as_deref())));
            }
        }
        Widget::TextArea(area) => {
            props.push(Prop::LineWrap(area.line_wrap()));
            props.push(Prop::Value(area.text.string().to_string()));
            props.extend(area.placeholder.clone().map(Prop::Placeholder));
            props.push(Prop::ReadOnly(area.read_only()));
            props.push(Prop::Enabled(area.text.isSelectable()));
            props.push(Prop::Lines(area.lines));
        }
        Widget::MenuButton { popup, sent, .. } => {
            props.push(Prop::Label(pull_down_title(&node.widget)));
            props.extend(node.icon.clone().map(Prop::Icon));
            if node.icon_only.is_some() {
                props.push(Prop::IconOnly(popup.imagePosition() == NSCellImagePosition::ImageOnly));
            }
            if let Some(menu) = popup.menu() {
                let mut entries = crate::services::context_menu_entries(&menu, sent);
                // The title item.
                if !entries.is_empty() {
                    entries.remove(0);
                }
                props.push(Prop::Menu(entries));
            }
        }
        Widget::Group { frame, .. } => {
            let titled = frame.titlePosition() != NSTitlePosition::NoTitle;
            props.push(Prop::Title(if titled { frame.title().to_string() } else { String::new() }));
        }
        Widget::Button(b) => {
            props.push(Prop::Label(b.title().to_string()));
            if node.kind == WidgetKind::ToggleButton {
                props.push(checked(b.state()));
            }
            props.extend(node.icon.clone().map(Prop::Icon));
            if node.icon_only.is_some() {
                props.push(Prop::IconOnly(b.imagePosition() == NSCellImagePosition::ImageOnly));
            }
        }
        Widget::Icon(view) => {
            if let Some(label) = view.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.extend(node.icon.clone().map(Prop::Icon));
            props.extend(node.icon_size.map(Prop::IconSize));
            if let (Some(sent), Some(shown)) = (node.text_color, view.contentTintColor()) {
                let given = crate::custom::ns_color(sent);
                props.push(Prop::TextColor(if shown.isEqual(Some(&given)) { sent } else { rgba(&shown) }));
            }
        }
        Widget::FileIcon(view) => {
            if let Some(label) = view.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.extend(node.file.clone().map(Prop::File));
            props.extend(node.icon_size.map(Prop::IconSize));
            props.extend(node.thumbnail.map(Prop::Thumbnail));
        }
        Widget::Checkbox(b) => {
            props.push(Prop::Label(b.title().to_string()));
            let mixed = b.state() == NSControlStateValueMixed;
            props.push(if mixed { Prop::Checked(node.checked) } else { checked(b.state()) });
            if node.mixed.is_some() {
                props.push(Prop::Mixed(mixed));
            }
        }
        Widget::Switch(s) => {
            if let Some(label) = s.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(checked(s.state()));
        }
        Widget::RadioGroup(group) => {
            if let Some(label) = group.stack.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(Prop::Options(group.options()));
            props.push(Prop::SelectedIndex(group.selected()));
            props.push(Prop::Enabled(group.is_enabled()));
        }
        Widget::Select(p) => {
            if let Some(label) = p.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(Prop::Options(p.itemTitles().iter().map(|t| t.to_string()).collect()));
            let index = p.indexOfSelectedItem();
            props.push(Prop::SelectedIndex((index >= 0).then_some(index as usize)));
        }
        Widget::Slider { slider, step } => {
            if let Some(label) = slider.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(Prop::Range { min: slider.minValue(), max: slider.maxValue() });
            props.push(Prop::Step(*step));
            props.push(Prop::Number(slider.doubleValue()));
            if node.orientation.is_some() {
                props.push(Prop::Orientation(if slider.isVertical() {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                }));
            }
        }
        Widget::NumberInput(n) => {
            if let Some(label) = n.field().accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            let stepper = n.stepper();
            props.push(Prop::Range { min: stepper.minValue(), max: stepper.maxValue() });
            props.push(Prop::Step(Some(stepper.increment())));
            props.push(Prop::WrapAround(stepper.valueWraps()));
            // What the field shows, which is the stepper's number.
            props.extend(n.field().stringValue().to_string().parse().ok().map(Prop::Number));
        }
        Widget::Progress(p) => {
            if let Some(label) = p.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(Prop::Progress((!p.isIndeterminate()).then(|| p.doubleValue())));
        }
        Widget::Spinner { indicator, running } => {
            if let Some(label) = indicator.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(Prop::Running(*running));
        }
        // AppKit reads it from the frame's shape: keep the app's.
        Widget::Separator(_) => props.extend(node.orientation.map(Prop::Orientation)),
        Widget::Image(view) => {
            if let Some(label) = view.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.extend(node.image.clone().map(Prop::Image));
            if node.fit.is_some() {
                props.push(Prop::ImageFit(match view.imageScaling() {
                    NSImageScaling::ScaleAxesIndependently => ImageFit::Stretch,
                    _ => ImageFit::Contain,
                }));
            }
        }
        Widget::GpuSurface(view) => {
            if let Some(label) = view.accessibilityLabel() {
                props.push(Prop::Label(label.to_string()));
            }
            props.push(Prop::TakesInput(view.takes_input()));
            props.push(Prop::PointerLock(view.pointer_locked()));
            props.push(Prop::KeyboardGrab(view.keyboard_grabbed()));
            props.push(Prop::Cursor(view.cursor()));
        }
        Widget::Scroll(scroll) => {
            let shown = (scroll.hasHorizontalScroller(), scroll.hasVerticalScroller());
            // Hidden scrollers leave only the node to say which axes scroll.
            props.push(Prop::ScrollAxes(match shown {
                (true, true) => ScrollAxes::Both,
                (true, false) => ScrollAxes::Horizontal,
                (false, true) => ScrollAxes::Vertical,
                (false, false) => node.scroll_axes,
            }));
            props.push(Prop::ScrollBars(shown != (false, false)));
        }
        Widget::Custom { view, render, props: last } => {
            props.push(Prop::Custom(last.with_props(render.read(view, last.props()))))
        }
        Widget::Drawn { view, props: last } => {
            props.push(Prop::Custom(last.clone()));
            props.push(Prop::Drawing(view.drawing()));
        }
        Widget::Native { last, .. } => props.push(Prop::Native(last.clone())),
        Widget::List(list) => {
            if node.kind == WidgetKind::Table {
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
        Widget::Sidebar(sidebar) => {
            props.push(Prop::Sections(sidebar.sections()));
            props.push(Prop::SelectedIndex(sidebar.selected()));
            // As the split shows it, or the app wants it before there's one.
            let split = node.parent.and_then(|p| state.nodes.get(&p)).and_then(|p| match &p.widget {
                Widget::Window { split: Some(split), .. } => Some(split),
                _ => None,
            });
            props.extend(
                match split {
                    Some(split) => Some(!split.collapsed()),
                    None => sidebar.shown.get(),
                }
                .map(Prop::SidebarShown),
            );
        }
        Widget::Tabs(tabs) => {
            props.push(Prop::TabTitles(tabs.titles()));
            props.push(Prop::SelectedIndex(tabs.selected()));
            props.extend(node.tab_icons.clone().map(Prop::TabIcons));
        }
    }
    if node.file_drop
        && let Widget::Host(host) | Widget::Group { host, .. } = &node.widget
    {
        props.push(Prop::FileDrop(host.file_drop()));
    }
    match &node.widget {
        Widget::Host(host) | Widget::Group { host, .. } => props.extend(host.keys().map(Prop::Keys)),
        Widget::List(list) => props.extend(list.keys().map(Prop::Keys)),
        _ => {}
    }
    if let Some(control) = node.widget.control() {
        props.push(Prop::Enabled(control.isEnabled()));
    }
    props.extend(node.text_style.map(Prop::TextStyle));
    props.extend(node.role.map(Prop::ButtonRole));
    props.extend(node.button_style.map(Prop::ButtonStyle));
    props.extend(node.tabs_style.map(Prop::TabsStyle));
    props.extend(node.tweak.clone().map(Prop::Tweak));
    let view = node.widget.view();
    props.push(Prop::LayoutDirection(match view.userInterfaceLayoutDirection() {
        objc2_app_kit::NSUserInterfaceLayoutDirection::RightToLeft => mitsuami_core::LayoutDirection::RightToLeft,
        _ => mitsuami_core::LayoutDirection::LeftToRight,
    }));
    props.push(Prop::Tooltip(view.toolTip().map(|t| t.to_string()).unwrap_or_default()));
    if let Some((sent, _)) = &node.context_menu {
        props.push(Prop::ContextMenu(match (&node.widget, view.menu()) {
            (Widget::Select(_) | Widget::MenuButton { .. }, _) => sent.clone(),
            (_, Some(menu)) => crate::services::context_menu_entries(&menu, sent),
            (_, None) => Vec::new(),
        }));
    }
    let f = match node.widget {
        // Placed by its frame (see `SetFrame`).
        Widget::Separator(_) => view.frame(),
        _ => view.alignmentRectForFrame(view.frame()),
    };
    let mut frame = Rect::new(f.origin.x as f32, f.origin.y as f32, f.size.width as f32, f.size.height as f32);
    // A row is where the table put it, and so is a cell.
    if let (Some(row), Some(Widget::List(list))) =
        (node.row, node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget))
    {
        frame = match node.column {
            Some(column) => list.cell_rect(row, column, view),
            None => list.row_rect(row),
        }
        .unwrap_or(frame);
    }
    // So is a page, by its tab view; one not shown isn't anywhere.
    if let Some(Widget::Tabs(tabs)) = node.parent.and_then(|p| state.nodes.get(&p)).map(|p| &p.widget) {
        frame = tabs.page_frame(id).unwrap_or(frame);
    }
    // A toolbar's button is its item, all of it.
    if let Some(item) = state.toolbar_item_of(id)
        && let Some(Widget::Window { toolbar: Some(toolbar), .. }) =
            state.nodes[&item].parent.and_then(|w| state.nodes.get(&w)).map(|w| &w.widget)
        && let Some(size) = toolbar.adopted_size(item)
    {
        frame = Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
    }
    // So is a toolbar item, by the toolbar; an empty one isn't shown.
    if let Some(Widget::Window { host, toolbar: Some(toolbar), .. }) = node
        .parent
        .filter(|_| node.kind == WidgetKind::ToolbarItem)
        .and_then(|p| state.nodes.get(&p))
        .map(|p| &p.widget)
    {
        let f = toolbar.frame(id, host).unwrap_or(crate::classes::zero_rect());
        frame = Rect::new(f.origin.x as f32, f.origin.y as f32, f.size.width as f32, f.size.height as f32);
    }
    // A sidebar is where the split put it, beside the content (so at
    // negative x), and full height; collapsed, it isn't shown.
    if let Some(Widget::Window { host, split: Some(split), .. }) =
        node.parent.filter(|_| node.kind == WidgetKind::Sidebar).and_then(|p| state.nodes.get(&p)).map(|p| &p.widget)
    {
        let f = if split.collapsed() {
            crate::classes::zero_rect()
        } else {
            view.convertRect_toView(view.bounds(), Some(host))
        };
        frame = Rect::new(f.origin.x as f32, f.origin.y as f32, f.size.width as f32, f.size.height as f32);
    }
    let by_view = state.by_view.borrow();
    let (children, scroll_offset) = match &node.widget {
        Widget::List(list) => {
            let origin = crate::classes::scrolled(&list.scroll.contentView());
            (list.children(), Some(Point::new(origin.x as f32, origin.y as f32)))
        }
        Widget::Tabs(tabs) => (tabs.ids(), None),
        Widget::Scroll(scroll) => {
            let origin = scroll.contentView().bounds().origin;
            (
                scroll.documentView().and_then(|d| by_view.get(&key(&d)).copied()).into_iter().collect(),
                Some(Point::new(origin.x as f32, origin.y as f32)),
            )
        }
        _ => {
            let mut children: Vec<NodeId> =
                view.subviews().iter().filter_map(|v| by_view.get(&key(&v)).copied()).collect();
            if let Widget::Window { toolbar: Some(toolbar), .. } = &node.widget {
                children.extend(toolbar.ids());
            }
            if let Widget::Window { split: Some(split), .. } = &node.widget {
                children.push(split.sidebar);
            }
            // An item's button is its view, not in its host.
            if node.kind == WidgetKind::ToolbarItem
                && let Some(Widget::Window { toolbar: Some(toolbar), .. }) =
                    node.parent.and_then(|w| state.nodes.get(&w)).map(|w| &w.widget)
                && let Some(button) = toolbar.adopted_view(id)
            {
                children.extend(by_view.get(&key(button)).copied());
            }
            (children, None)
        }
    };
    Some(NativeState {
        kind: node.kind,
        props,
        frame,
        parent: node.parent,
        children,
        focused: focused(&node.widget),
        scroll_offset,
        selection: super::selection::selection(&node.widget, focused(&node.widget)),
    })
}

fn focused(widget: &Widget) -> bool {
    let view = widget.key_view();
    let Some(responder) = view.window().and_then(|w| w.firstResponder()) else { return false };
    match widget {
        // While editing, the window's field editor is first responder.
        Widget::Field(field) => field.currentEditor().is_some(),
        Widget::NumberInput(n) => n.field().currentEditor().is_some(),
        Widget::RadioGroup(group) => group.has_focus(&responder),
        _ => std::ptr::eq(&*responder as *const _ as *const NSView, &*view as *const NSView),
    }
}

/// A colour as sRGB components, for reporting one that isn't the given one.
fn rgba(color: &NSColor) -> Color {
    let Some(color) = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) else {
        return Color::Rgba(0, 0, 0, 0);
    };
    let c = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::Rgba(c(color.redComponent()), c(color.greenComponent()), c(color.blueComponent()), c(color.alphaComponent()))
}
