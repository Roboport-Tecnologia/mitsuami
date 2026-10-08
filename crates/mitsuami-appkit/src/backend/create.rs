//! Creating nodes: the native view for each kind of widget.

use std::rc::Rc;

use mitsuami_core::backend::Appearance;
use mitsuami_core::{Command, NodeId, ScrollAxes, TextStyle, UiEvent, WidgetKind, find_prop};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{MainThreadOnly, msg_send, sel};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSBox, NSBoxType, NSButton, NSButtonType, NSImageScaling, NSImageView, NSMenuItem,
    NSPopUpButton, NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSSearchField, NSSecureTextField,
    NSSlider, NSSwitch, NSTextField, NSViewBoundsDidChangeNotification, NSViewFrameDidChangeNotification, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSNotificationCenter, NSPoint, NSRect, NSSize};

use crate::classes::{ActionTarget, ClosureTarget, DrawnView, HostView, WindowDelegate};
use crate::custom::{AppKitCx, ErasedRender, NativePayload};
use crate::number_field::NumberField;
use crate::radio::RadioGroup;
use crate::sidebar::Sidebar;
use crate::surface::SurfaceView;
use crate::tabs::Tabs;

use super::fonts::font;
use super::group::group_box;
use super::menus::show_pull_down;
use super::{Node, State, Widget, key, ns, violation};

impl State {
    pub(super) fn create(&mut self, id: NodeId, kind: WidgetKind, command: &Command) {
        let mtm = self.mtm;
        let target = matches!(
            kind,
            WidgetKind::Button
                | WidgetKind::ToggleButton
                | WidgetKind::Checkbox
                | WidgetKind::Switch
                | WidgetKind::Select
                | WidgetKind::Slider
                | WidgetKind::TextInput
                | WidgetKind::PasswordInput
                | WidgetKind::SearchInput
                | WidgetKind::TextArea
                | WidgetKind::ScrollView
                | WidgetKind::List
                | WidgetKind::Table
        )
        .then(|| ActionTarget::new(mtm, id, kind, self.events.clone()));
        let action = Some(sel!(fire:));
        let mut targets = Vec::new();
        let target_obj: Option<&AnyObject> = target.as_deref().map(|t| t.as_ref());
        let widget = match kind {
            WidgetKind::Window => {
                let style = NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable;
                let window = unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        NSWindow::alloc(mtm),
                        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0)),
                        style,
                        NSBackingStoreType::Buffered,
                        false,
                    )
                };
                unsafe { window.setReleasedWhenClosed(false) };
                // The core owns the Tab order (reading order, not geometry)
                // and sends it with SetFocusOrder.
                window.setAutorecalculatesKeyViewLoop(false);
                let host = HostView::new(mtm);
                window.setContentView(Some(&host));
                let delegate = WindowDelegate::new(mtm, id, self.events.clone(), self.by_view.clone());
                window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
                delegate.observe_focus(&window);
                if let Some(appearance) = self.options.appearance {
                    let name = match appearance {
                        Appearance::Light => unsafe { NSAppearanceNameAqua },
                        Appearance::Dark => unsafe { NSAppearanceNameDarkAqua },
                    };
                    window.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
                }
                if self.options.show_windows {
                    self.pending_show.push(id);
                }
                Widget::Window { window, host, _delegate: delegate, toolbar: None, split: None }
            }
            WidgetKind::Sidebar => Widget::Sidebar(Sidebar::new(mtm, id, self.events.clone())),
            WidgetKind::RadioGroup => Widget::RadioGroup(RadioGroup::new(mtm, id, self.events.clone())),
            WidgetKind::Container | WidgetKind::ToolbarItem => Widget::Host(HostView::new(mtm)),
            WidgetKind::Group => {
                let host = HostView::new(mtm);
                let frame = group_box(mtm, NSSize::new(0.0, 0.0));
                frame.setAutoresizingMask(
                    NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
                );
                host.addSubview(&frame);
                Widget::Group { host, frame }
            }
            WidgetKind::Tabs => Widget::Tabs(Tabs::new(mtm, id, self.events.clone())),
            WidgetKind::Custom(_) => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(custom) = find_prop!(props, Custom) else {
                    violation(command, "a custom widget needs its Prop::Custom")
                };
                match custom.native() {
                    Some(native) => {
                        let Some(render) = native.downcast_ref::<Rc<dyn ErasedRender>>().cloned() else {
                            violation(command, "the native render is not an AppKit one")
                        };
                        let mut cx = AppKitCx::new(mtm, id, self.events.clone(), &mut targets);
                        let view = render.create(custom.props(), &mut cx);
                        Widget::Custom { view, render, props: custom }
                    }
                    None => Widget::Drawn { view: DrawnView::new(mtm, id, self.events.clone()), props: custom },
                }
            }
            WidgetKind::Native => {
                let Command::Create { props, .. } = command else { unreachable!() };
                let Some(opaque) = find_prop!(props, Native) else {
                    violation(command, "a native view needs its Prop::Native")
                };
                let Some(payload) = opaque.downcast_ref::<NativePayload>() else {
                    violation(command, "the native view is not an AppKit one")
                };
                let Some(create) = payload.spec.create.borrow_mut().take() else {
                    violation(command, "this native view was already created")
                };
                let measure = payload.spec.measure.clone();
                let mut cx = AppKitCx::new(mtm, id, self.events.clone(), &mut targets);
                let view = create(&mut cx);
                payload.apply(&view);
                Widget::Native { view, measure, last: opaque }
            }
            WidgetKind::Text => {
                let label = NSTextField::wrappingLabelWithString(&ns(""), mtm);
                label.setSelectable(false);
                Widget::Label(label)
            }
            WidgetKind::Button => {
                Widget::Button(unsafe { NSButton::buttonWithTitle_target_action(&ns(""), target_obj, action, mtm) })
            }
            // A push button that stays in once clicked, with a darker bezel
            // while it's in (macOS 26).
            WidgetKind::ToggleButton => {
                let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), target_obj, action, mtm) };
                button.setButtonType(NSButtonType::PushOnPushOff);
                Widget::Button(button)
            }
            WidgetKind::Checkbox => {
                Widget::Checkbox(unsafe { NSButton::checkboxWithTitle_target_action(&ns(""), target_obj, action, mtm) })
            }
            WidgetKind::Switch => {
                let switch = NSSwitch::new(mtm);
                unsafe {
                    switch.setTarget(target_obj);
                    switch.setAction(action);
                }
                Widget::Switch(switch)
            }
            WidgetKind::Select => {
                let popup = NSPopUpButton::initWithFrame_pullsDown(
                    NSPopUpButton::alloc(mtm),
                    crate::classes::zero_rect(),
                    false,
                );
                unsafe {
                    popup.setTarget(target_obj);
                    popup.setAction(action);
                }
                Widget::Select(popup)
            }
            WidgetKind::MenuButton => {
                let popup = NSPopUpButton::initWithFrame_pullsDown(
                    NSPopUpButton::alloc(mtm),
                    crate::classes::zero_rect(),
                    true,
                );
                let events = self.events.clone();
                let target = ClosureTarget::new(mtm, move |sender| {
                    if let Some(item) = sender.downcast_ref::<NSMenuItem>() {
                        events.emit(id, UiEvent::MenuItem(item.tag() as u32));
                    }
                });
                let widget = Widget::MenuButton { popup, target, sent: Vec::new() };
                show_pull_down(&widget, "", None, None);
                widget
            }
            WidgetKind::Slider => {
                let slider = NSSlider::new(mtm);
                unsafe {
                    slider.setTarget(target_obj);
                    slider.setAction(action);
                }
                Widget::Slider { slider, step: None }
            }
            WidgetKind::NumberInput => Widget::NumberInput(NumberField::new(mtm, id, self.events.clone())),
            // Not editable, framed or animated: what `NSImageView` is
            // made as.
            WidgetKind::Image => Widget::Image(NSImageView::new(mtm)),
            // At the symbol's own size, which its configuration sets.
            WidgetKind::Icon => {
                let view = NSImageView::new(mtm);
                view.setImageScaling(NSImageScaling::ScaleNone);
                Widget::Icon(view)
            }
            // Scaled to its square, which its thumbnail may not fill.
            WidgetKind::FileIcon => {
                let view = NSImageView::new(mtm);
                view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                Widget::FileIcon(view)
            }
            WidgetKind::GpuSurface => {
                let (view, handle) = SurfaceView::new(mtm, id, self.events.clone());
                self.events.emit(id, UiEvent::SurfaceReady(handle));
                Widget::GpuSurface(view)
            }
            WidgetKind::Progress => {
                let progress = NSProgressIndicator::new(mtm);
                progress.setMinValue(0.0);
                progress.setMaxValue(1.0);
                Widget::Progress(progress)
            }
            WidgetKind::Spinner => {
                let indicator = NSProgressIndicator::new(mtm);
                indicator.setStyle(NSProgressIndicatorStyle::Spinning);
                indicator.setIndeterminate(true);
                // Stopped, it shows nothing, as the other platforms' do.
                indicator.setDisplayedWhenStopped(false);
                Widget::Spinner { indicator, running: false }
            }
            // It runs the long way of its frame, which the layout gives.
            WidgetKind::Separator => {
                let line = NSBox::new(mtm);
                line.setBoxType(NSBoxType::Separator);
                Widget::Separator(line)
            }
            WidgetKind::TextInput => {
                // No target-action: submit comes from the delegate (Return
                // only), edits from `controlTextDidChange:`.
                let field = NSTextField::textFieldWithString(&ns(""), mtm);
                if let Some(target) = &target {
                    // SAFETY: the node keeps the target alive as long as the field.
                    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&**target))) };
                }
                Widget::Field(field)
            }
            WidgetKind::PasswordInput => {
                // A text field whose cell and field editor hide the text;
                // the rest is a text field's, delegate included.
                // `textFieldWithString:` is inherited: called on the secure
                // class, it makes a secure field set up as a text field.
                // SAFETY: the class method returns an autoreleased instance
                // of the class it's called on.
                let field: Retained<NSSecureTextField> = unsafe {
                    msg_send![<NSSecureTextField as objc2::ClassType>::class(), textFieldWithString: &*ns("")]
                };
                let field = field.into_super();
                if let Some(target) = &target {
                    // SAFETY: the node keeps the target alive as long as the field.
                    unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&**target))) };
                }
                Widget::Field(field)
            }
            WidgetKind::SearchInput => {
                // Edits come from the delegate, as a text field's; searches
                // from the field's action, which AppKit sends when typing
                // pauses, on Return, and when the field is cleared, but not
                // for text set in code.
                let field = NSSearchField::new(mtm);
                if let Some(target) = &target {
                    // SAFETY: the node keeps the target alive as long as the field.
                    unsafe {
                        field.setTarget(Some(target.as_ref()));
                        field.setAction(action);
                        field.setDelegate(Some(ProtocolObject::from_ref(&**target)));
                    }
                }
                Widget::Field(field.into_super())
            }
            // Edits come from the delegate's `textDidChange:`.
            WidgetKind::TextArea => Widget::TextArea(crate::text_area::TextArea::new(
                mtm,
                target.as_deref().map(ProtocolObject::from_ref),
                &font(TextStyle::Body),
            )),
            WidgetKind::ScrollView => {
                let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), crate::classes::zero_rect());
                scroll.setDrawsBackground(false);
                scroll.setAutohidesScrollers(true);
                // The core places the content; AppKit mustn't inset it for
                // the title bar on top of that.
                scroll.setAutomaticallyAdjustsContentInsets(false);
                observe_scrolling(&scroll, target.as_deref());
                Widget::Scroll(scroll)
            }
            WidgetKind::List | WidgetKind::Table => {
                let list = crate::list::List::new(mtm, id, self.events.clone(), kind == WidgetKind::Table);
                observe_scrolling(&list.scroll, target.as_deref());
                Widget::List(list)
            }
            WidgetKind::Fragment => violation(command, "fragments are core-only"),
        };
        // The core assumes new nodes start with a zero frame and only sends
        // frames that differ; AppKit controls come with their own.
        if !matches!(widget, Widget::Window { .. }) {
            widget.view().setFrame(crate::classes::zero_rect());
        }
        self.by_view.borrow_mut().insert(key(widget.view()), id);
        self.nodes.insert(
            id,
            Node {
                kind,
                widget,
                _target: target,
                _targets: targets,
                parent: None,
                row: None,
                column: None,
                text_style: None,
                weight: None,
                italic: None,
                text_color: None,
                align: false,
                truncation: None,
                role: None,
                button_style: None,
                tabs_style: None,
                tab_icons: None,
                orientation: None,
                scroll_axes: ScrollAxes::default(),
                scroll_bars: true,
                image: None,
                fit: None,
                icon: None,
                icon_size: None,
                icon_only: None,
                file: None,
                thumbnail: None,
                modal: None,
                mixed: None,
                checked: false,
                tweak: None,
                file_drop: false,
                context_menu: None,
                hover: None,
                double_click: None,
            },
        );
    }
}

/// Reports a scroll view's clip view moving, and its frame changing (the
/// core lays out the content in it), through the node's target.
fn observe_scrolling(scroll: &NSScrollView, target: Option<&ActionTarget>) {
    let clip = scroll.contentView();
    clip.setPostsBoundsChangedNotifications(true);
    clip.setPostsFrameChangedNotifications(true);
    if let Some(target) = target {
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: the target is removed as an observer when the node is destroyed.
        unsafe {
            center.addObserver_selector_name_object(
                target,
                sel!(scrolled:),
                Some(NSViewBoundsDidChangeNotification),
                Some(&clip),
            );
            center.addObserver_selector_name_object(
                target,
                sel!(viewportChanged:),
                Some(NSViewFrameDidChangeNotification),
                Some(&clip),
            );
        }
    }
}
