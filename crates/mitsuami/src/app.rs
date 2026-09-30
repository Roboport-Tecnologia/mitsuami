//! Starting an app on the native backend of the target platform.

use mitsuami_core::l10n::Locales;
use mitsuami_core::{AnyView, AppIcon, AppInfo, CurrentWindow, Prop, Ui, UiEvent, View, WindowSize, provide_stores};
use mitsuami_reactive::{IntoValue, Owner, Value, effect, provide};
use mitsuami_widgets::Window;

struct WindowSpec {
    title: Value<String>,
    size: WindowSize,
    content: Box<dyn FnOnce() -> AnyView>,
}

/// A window opened at startup.
enum Startup {
    Spec(WindowSpec),
    View(Box<dyn FnOnce() -> Window>),
}

/// An application: who it is, its windows and how to start it.
///
/// ```ignore
/// App::new()
///     .id("org.example.Counter")
///     .name("Counter")
///     .icon(AppIcon::bytes(include_bytes!("../icon.png")))
///     .locales(locales!("../locales"))
///     .window(t!("counter-title"), Size::new(360.0, 200.0), || counter(0))
///     .run();
/// ```
#[derive(Default)]
pub struct App {
    info: AppInfo,
    locales: Option<Locales>,
    windows: Vec<Startup>,
}

impl App {
    pub fn new() -> App {
        App::default()
    }

    /// Its id, in reverse DNS (`org.example.Player`): what Linux desktops
    /// match its windows to its `.desktop` file and icon by, and an
    /// unpackaged Windows app's AppUserModelID. A macOS app's id is its
    /// bundle's. See [`AppInfo::id`].
    pub fn id(mut self, id: impl Into<String>) -> App {
        self.info.id = Some(id.into());
        self
    }

    /// The name people know it by. See [`AppInfo::name`].
    pub fn name(mut self, name: impl Into<String>) -> App {
        self.info.name = Some(name.into());
        self
    }

    /// Its icon, for platforms that take one at run time. See
    /// [`AppInfo::icon`].
    pub fn icon(mut self, icon: AppIcon) -> App {
        self.info.icon = Some(icon);
        self
    }

    /// Its translations, usually `locales!("../locales")`: Fluent files per
    /// language. The app's language is the first of the user's it has,
    /// and `t!` and `tr!` show its messages. See [`mitsuami_core::l10n`].
    pub fn locales(mut self, locales: Locales) -> App {
        self.locales = Some(locales);
        self
    }

    /// Adds a window, opened at startup. `size` is the content size: a
    /// [`Size`](mitsuami_core::Size), [`WindowSize::FitHeight`] to fit
    /// the height to the content, or [`WindowSize::FollowHeight`] to follow
    /// it as it changes.
    pub fn window<V: View>(
        mut self,
        title: impl IntoValue<String>,
        size: impl Into<WindowSize>,
        content: impl FnOnce() -> V + 'static,
    ) -> App {
        let content = Box::new(move || AnyView::new(content()));
        self.windows.push(Startup::Spec(WindowSpec { title: title.into_value(), size: size.into(), content }));
        self
    }

    /// Adds a [`Window`], opened at startup (while its `open` value is
    /// true): for what only a `Window` has, such as full screen, a minimum
    /// size or a title that changes. As any `Window`'s, its close button
    /// does nothing unless it's bound (`bind`) or handled
    /// (`on_close_request`).
    ///
    /// `window` makes it in the app scope, once the app has started, so
    /// it can use stores and signals made there live as long as the app:
    ///
    /// ```ignore
    /// App::new().open(|| {
    ///     let browser = use_store::<Browser>();
    ///     let open = signal(true);
    ///     Window::new(move || browser.title()).bind(open).content(Finder::new)
    /// })
    /// ```
    pub fn open(mut self, window: impl FnOnce() -> Window + 'static) -> App {
        self.windows.push(Startup::View(Box::new(window)));
        self
    }

    /// Runs until the last window closes.
    pub fn run(self) {
        let (info, locales, windows) = (self.info, self.locales, self.windows);
        let setup = move |ui: &Ui| {
            // Before any window, so the first is in the app's language.
            if let Some(locales) = locales {
                ui.set_locales(locales);
            }
            // The app scope makes the Ui available to every component
            // (`inject::<Ui>()`, `spawn_local`, `sleep`) and holds the
            // stores; it lives as long as the app.
            let app = Owner::new_root();
            app.with(|| {
                provide(ui.clone());
                provide_stores();
            });
            for window in windows {
                match window {
                    Startup::Spec(spec) => open(ui, app, spec),
                    // Made and built in the app scope; its placeholder is in
                    // no tree.
                    Startup::View(window) => {
                        app.with(|| window().build(ui));
                    }
                }
            }
        };
        #[cfg(target_os = "macos")]
        mitsuami_appkit::run(info, setup);
        #[cfg(all(target_os = "linux", feature = "kde"))]
        mitsuami_kirigami::run(info, setup);
        #[cfg(all(target_os = "linux", feature = "gtk", not(feature = "kde")))]
        mitsuami_gtk::run(info, setup);
        #[cfg(all(target_os = "linux", not(any(feature = "gtk", feature = "kde"))))]
        let _ = (info, setup);
        #[cfg(windows)]
        mitsuami_winui::run(info, setup);
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            let _ = (info, setup);
            panic!("mitsuami: no native backend for this platform");
        }
    }
}

fn open(ui: &Ui, app: Owner, spec: WindowSpec) {
    let title = spec.title;
    let window = ui.create_window(mitsuami_reactive::untrack(|| title.get()), spec.size);
    // Each window owns its reactive state; closing it disposes everything,
    // including the tasks it started.
    let owner = app.child();
    let root = owner.with(|| {
        if let Value::Dynamic(_) = title {
            let ui = ui.clone();
            effect(move || ui.set_prop(window, Prop::Title(title.get())));
        }
        provide(CurrentWindow(window));
        (spec.content)().build(ui)
    });
    ui.append_child(window, root);
    let weak = ui.downgrade();
    ui.on_event(window, move |event| {
        if *event == UiEvent::WindowCloseRequested
            && let Some(ui) = weak.upgrade()
        {
            owner.dispose();
            ui.destroy(window);
        }
    });
}
