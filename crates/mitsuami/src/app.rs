//! Starting an app on the native backend of the target platform.

use mitsuami_core::{AnyView, CurrentWindow, Ui, UiEvent, View, WindowSize, provide_stores};
use mitsuami_reactive::{Owner, provide};

struct WindowSpec {
    title: String,
    size: WindowSize,
    content: Box<dyn FnOnce() -> AnyView>,
}

/// An application: its windows and how to start it.
///
/// ```ignore
/// App::new().window("Counter", Size::new(360.0, 200.0), || counter(0)).run();
/// ```
#[derive(Default)]
pub struct App {
    windows: Vec<WindowSpec>,
}

impl App {
    pub fn new() -> App {
        App::default()
    }

    /// Adds a window, opened at startup. `size` is the content size: a
    /// [`Size`](mitsuami_core::Size), or [`WindowSize::FitHeight`] to fit
    /// the height to the content.
    pub fn window<V: View>(
        mut self,
        title: impl Into<String>,
        size: impl Into<WindowSize>,
        content: impl FnOnce() -> V + 'static,
    ) -> App {
        let content = Box::new(move || AnyView::new(content()));
        self.windows.push(WindowSpec { title: title.into(), size: size.into(), content });
        self
    }

    /// Runs until the last window closes.
    pub fn run(self) {
        let windows = self.windows;
        let setup = move |ui: &Ui| {
            // The app scope makes the Ui available to every component
            // (`inject::<Ui>()`, `spawn_local`, `sleep`) and holds the
            // stores; it lives as long as the app.
            let app = Owner::new_root();
            app.with(|| {
                provide(ui.clone());
                provide_stores();
            });
            for spec in windows {
                open(ui, app, spec);
            }
        };
        #[cfg(target_os = "macos")]
        mitsuami_appkit::run(setup);
        #[cfg(all(target_os = "linux", feature = "kde"))]
        mitsuami_kirigami::run(setup);
        #[cfg(all(target_os = "linux", feature = "gtk", not(feature = "kde")))]
        mitsuami_gtk::run(setup);
        #[cfg(all(target_os = "linux", not(any(feature = "gtk", feature = "kde"))))]
        let _ = setup;
        #[cfg(windows)]
        mitsuami_winui::run(setup);
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            let _ = setup;
            panic!("mitsuami: no native backend for this platform");
        }
    }
}

fn open(ui: &Ui, app: Owner, spec: WindowSpec) {
    let window = ui.create_window(spec.title, spec.size);
    // Each window owns its reactive state; closing it disposes everything,
    // including the tasks it started.
    let owner = app.child();
    let root = owner.with(|| {
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
