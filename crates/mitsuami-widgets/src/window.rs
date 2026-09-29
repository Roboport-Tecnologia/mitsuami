//! Windows the app opens while it runs.

use std::rc::Rc;

use mitsuami_core::{AnyView, CurrentWindow, Modality, NodeId, Prop, Size, Ui, UiEvent, View, WidgetKind, WindowSize};
use mitsuami_reactive::{IntoValue, Signal, Value, computed, effect, inject, on_cleanup, provide, untrack};

/// A window the app opens while it runs: shown while `open` is true, with
/// its content built when it opens and disposed when it closes. Declare it
/// anywhere in the tree, next to the state that opens it; it closes with
/// the scope that declared it, and belongs to the window it's declared in.
///
/// The close button asks: [`bind`](Self::bind) closes it by setting the
/// flag, or [`on_close_request`](Self::on_close_request) lets the app
/// decide (to ask about unsaved changes, say). Without either, the close
/// button does nothing.
///
/// ```ignore
/// let editing = signal(false);
/// Column::new().children((
///     Button::new("Edit…").on_click(move || editing.set(true)),
///     Window::new("Machine").bind(editing).content(|| machine_form()),
/// ))
/// ```
///
/// In `view!`, its title is an attribute and its children are its content:
/// `<Window title="Machine" bind=editing>…</Window>`.
pub struct Window<T = Value<String>> {
    title: T,
    size: Value<WindowSize>,
    modality: Value<Option<Modality>>,
    open: Value<bool>,
    full_screen: Option<Signal<bool>>,
    maximized: Option<Signal<bool>>,
    resizable: Option<Value<bool>>,
    min_size: Option<Value<Size>>,
    on_open: Option<Rc<dyn Fn()>>,
    on_close_request: Option<Rc<dyn Fn()>>,
    content: Option<Rc<dyn Fn() -> AnyView>>,
}

impl Window {
    /// Open, 480 wide, as tall as its content.
    pub fn new(title: impl IntoValue<String>) -> Window {
        Window {
            title: title.into_value(),
            size: Value::Static(WindowSize::FitHeight(480.0)),
            modality: Value::Static(None),
            open: Value::Static(true),
            full_screen: None,
            maximized: None,
            resizable: None,
            min_size: None,
            on_open: None,
            on_close_request: None,
            content: None,
        }
    }
}

impl<T> Window<T> {
    /// Its content size when it opens: a [`Size`](mitsuami_core::Size),
    /// [`WindowSize::FitHeight`] to fit the height to the content, or
    /// [`WindowSize::FollowHeight`] to follow it as it changes. Read each
    /// time it opens, as its modality is, so a change applies at the next
    /// opening.
    pub fn size(mut self, size: impl IntoValue<WindowSize>) -> Window<T> {
        self.size = size.into_value();
        self
    }

    /// Modal: while it's open, it blocks the window it's declared in
    /// ([`Modality::Window`], a sheet on macOS) or the whole app
    /// ([`Modality::Application`]), and stays above that window. Set
    /// before it opens. Like any dialog, Escape asks it to close, as its
    /// close button does (on AppKit a `ButtonRole::Cancel` button takes
    /// Escape first). A sheet has no close button: give its content a
    /// way out, e.g. that Cancel button.
    pub fn modal(mut self, modality: Modality) -> Window<T> {
        self.modality = Value::Static(Some(modality));
        self
    }

    /// Modal or not, as a value that can change: read each time it opens,
    /// so a change applies at the next opening.
    pub fn modality(mut self, modality: impl IntoValue<Option<Modality>>) -> Window<T> {
        self.modality = modality.into_value();
        self
    }

    /// Shown while true.
    pub fn open(mut self, open: impl IntoValue<bool>) -> Window<T> {
        self.open = open.into_value();
        self
    }

    /// Shown while the signal is true; the close button sets it false.
    pub fn bind(self, open: Signal<bool>) -> Window<T> {
        self.open(open).on_close_request(move || open.set(false))
    }

    /// In full screen while `full_screen` is true, the platform's own way:
    /// a Space of its own on macOS, the whole screen elsewhere, with the
    /// platform's own way out. The user can change it too (the title bar's
    /// button on macOS, the window manager's key), which sets the signal.
    pub fn full_screen(mut self, full_screen: Signal<bool>) -> Window<T> {
        self.full_screen = Some(full_screen);
        self
    }

    /// Maximized while `maximized` is true, as the platform maximizes a
    /// window: its screen's working area (on macOS, zoomed to fit it). The
    /// user can change it too (the title bar's button, a double-click on
    /// it, the window manager's key), which sets the signal.
    pub fn maximized(mut self, maximized: Signal<bool>) -> Window<T> {
        self.maximized = Some(maximized);
        self
    }

    /// Whether the user can resize it; they can unless it's set false.
    /// The app can still size it, and a window that follows its content's
    /// height still does.
    pub fn resizable(mut self, resizable: impl IntoValue<bool>) -> Window<T> {
        self.resizable = Some(resizable.into_value());
        self
    }

    /// The smallest content size the user can make it. Smaller when it's
    /// set, it grows to it.
    pub fn min_size(mut self, size: impl IntoValue<Size>) -> Window<T> {
        self.min_size = Some(size.into_value());
        self
    }

    /// Called each time it opens, before its content is built: to start a
    /// form from what's saved, say.
    pub fn on_open(mut self, handler: impl Fn() + 'static) -> Window<T> {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Called when the user asks to close it (the close button, ⌘W, Alt+F4).
    /// It stays open unless the app closes it.
    pub fn on_close_request(mut self, handler: impl Fn() + 'static) -> Window<T> {
        self.on_close_request = Some(Rc::new(handler));
        self
    }

    /// What it shows, built each time it opens.
    pub fn content<V: View>(mut self, content: impl Fn() -> V + 'static) -> Window<T> {
        self.content = Some(Rc::new(move || AnyView::new(content())));
        self
    }
}

impl View for Window {
    /// A placeholder in the tree, which takes no room; the window is
    /// top-level.
    fn build(self, ui: &Ui) -> NodeId {
        let placeholder = ui.create(WidgetKind::Fragment, Vec::new());
        let Window {
            title,
            size,
            modality,
            open,
            full_screen,
            maximized,
            resizable,
            min_size,
            on_open,
            on_close_request,
            content,
        } = self;
        // The window it's declared in, which a modal window belongs to.
        let owner = inject::<CurrentWindow>().map(|w| w.0);
        let ui = ui.clone();
        let show = move || {
            let window = ui.create_window(String::new(), size.get());
            if let Some(modality) = modality.get() {
                // With no window to block, it blocks the app.
                let modality = if owner.is_some() { modality } else { Modality::Application };
                ui.set_prop(window, Prop::Modal { owner, modality });
            }
            let title = title.clone();
            let titled = ui.clone();
            effect(move || titled.set_prop(window, Prop::Title(title.get())));
            if let Some(min_size) = min_size.clone() {
                let ui = ui.clone();
                effect(move || ui.set_prop(window, Prop::MinSize(min_size.get())));
            }
            if let Some(full_screen) = full_screen {
                let filled = ui.clone();
                effect(move || filled.set_prop(window, Prop::FullScreen(full_screen.get())));
                ui.on_event(window, move |event| {
                    if let UiEvent::FullScreenChanged(on) = event {
                        full_screen.set(*on);
                    }
                });
            }
            if let Some(maximized) = maximized {
                let zoomed = ui.clone();
                effect(move || zoomed.set_prop(window, Prop::Maximized(maximized.get())));
                ui.on_event(window, move |event| {
                    if let UiEvent::MaximizedChanged(on) = event {
                        maximized.set(*on);
                    }
                });
            }
            if let Some(resizable) = resizable.clone() {
                let ui = ui.clone();
                effect(move || ui.set_prop(window, Prop::Resizable(resizable.get())));
            }
            provide(CurrentWindow(window));
            if let Some(handler) = &on_open {
                handler();
            }
            if let Some(content) = &content {
                let root = content().build(&ui);
                ui.append_child(window, root);
            }
            if let Some(handler) = on_close_request.clone() {
                ui.on_event(window, move |event| {
                    if *event == UiEvent::WindowCloseRequested {
                        handler();
                    }
                });
            }
            let ui = ui.clone();
            on_cleanup(move || ui.destroy(window));
        };
        // In a scope of its own even when it's always open, so what it
        // provides (its `CurrentWindow`) stays inside it.
        let open = computed(move || open.get());
        effect(move || {
            if open.get() {
                untrack(&show);
            }
        });
        placeholder
    }
}

impl Window {
    /// `<Window title="Machine" bind=editing>…</Window>`: a `Window<()>`
    /// until `title` is set, and only then a `View`.
    #[doc(hidden)]
    pub fn __tag() -> Window<()> {
        Window::new(String::new()).retitled(())
    }

    /// Its content, built each time it opens.
    #[doc(hidden)]
    pub fn __children<V: View>(self, content: impl Fn() -> V + 'static) -> Window {
        self.content(content)
    }
}

impl Window<()> {
    pub fn title(self, title: impl IntoValue<String>) -> Window {
        self.retitled(title.into_value())
    }
}

impl<T> Window<T> {
    /// The same window with another title type.
    fn retitled<U>(self, title: U) -> Window<U> {
        let Window {
            title: _,
            size,
            modality,
            open,
            full_screen,
            maximized,
            resizable,
            min_size,
            on_open,
            on_close_request,
            content,
        } = self;
        Window {
            title,
            size,
            modality,
            open,
            full_screen,
            maximized,
            resizable,
            min_size,
            on_open,
            on_close_request,
            content,
        }
    }
}
