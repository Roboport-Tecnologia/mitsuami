//! The real GTK services, end to end: clipboard, the header bar menu, alert
//! dialogs and file dialogs. App tests use scripted services instead, so
//! this is where the native ones are checked. Runs on the private test
//! display, whose clipboard is its own.
//!
//! A plain `main` (harness = false): GTK has to run on the main thread.

#[cfg(target_os = "linux")]
mod checks {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gtk::glib;
    use gtk::prelude::*;
    use mitsuami_core::NodeId;
    use mitsuami_core::services::{Alert, FileFilter, Menu, MenuBar, MenuItem, MenuRole, OpenFile, Shortcut};
    use mitsuami_core::{Modality, Prop, Size, Ui};
    use mitsuami_gtk::{BackendOptions, GtkBackend, GtkHandle};
    use mitsuami_reactive::signal;

    pub struct Fixture {
        pub ui: Ui,
        pub handle: GtkHandle,
    }

    pub fn fixture() -> Fixture {
        mitsuami_gtk::init_for_tests();
        let backend = GtkBackend::new(BackendOptions::default());
        let handle = backend.handle();
        Fixture { ui: Ui::new(backend), handle }
    }

    /// Lets GTK deliver what it queued and ticks the UI, until `done`.
    fn pump_until(f: &Fixture, what: &str, done: impl Fn() -> bool) {
        let started = Instant::now();
        while !done() {
            assert!(started.elapsed() < Duration::from_secs(5), "timed out waiting for {what}");
            while glib::MainContext::default().iteration(false) {}
            f.ui.tick();
            f.handle.show_pending_windows();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn find<W: IsA<gtk::Widget>>(root: &gtk::Widget, matches: &dyn Fn(&W) -> bool) -> Option<W> {
        if let Some(w) = root.downcast_ref::<W>()
            && matches(w)
        {
            return Some(w.clone());
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if let Some(found) = find(&c, matches) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }

    /// A toplevel that isn't one of ours: the dialog GTK opened.
    fn dialog_window(f: &Fixture) -> Option<gtk::Window> {
        let ours: Vec<gtk::Window> = f.handle_windows();
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.is_visible() && !ours.contains(w))
    }

    trait Windows {
        fn handle_windows(&self) -> Vec<gtk::Window>;
    }

    impl Windows for Fixture {
        fn handle_windows(&self) -> Vec<gtk::Window> {
            self.ui.windows().into_iter().filter_map(|id| self.handle.gtk_window(id)).collect()
        }
    }

    pub fn clipboard_round_trips(f: &Fixture) {
        let written = Rc::new(RefCell::new(None));
        let read = Rc::new(RefCell::new(None));
        let (w, r) = (written.clone(), read.clone());
        let write = f.ui.set_clipboard_text("mitsuami ✓");
        let ui = f.ui.clone();
        f.ui.spawn_local(async move {
            *w.borrow_mut() = Some(write.await);
            *r.borrow_mut() = ui.clipboard_text().await;
        });
        pump_until(f, "the clipboard", || read.borrow().is_some());
        assert_eq!(*written.borrow(), Some(Ok(())));
        assert_eq!(read.borrow().as_deref(), Some("mitsuami ✓"));
    }

    pub fn menus_are_installed_and_activate(f: &Fixture) {
        let window = f.ui.create_window("menu host", Size::new(400.0, 300.0));
        f.ui.tick();
        let chosen = Rc::new(Cell::new(0));
        let (c, unavailable) = (chosen.clone(), chosen.clone());
        f.ui.set_menu(
            MenuBar::new()
                .menu(
                    Menu::new("File")
                        .item(
                            MenuItem::new("New").on_select(move || c.set(c.get() + 1)).shortcut(Shortcut::primary('n')),
                        )
                        .item(MenuItem::new("Unavailable").on_select(move || unavailable.set(100)).enabled(false)),
                )
                .menu(Menu::new("View").item(MenuItem::new("Zoom"))),
        );
        f.ui.tick();
        let gtk_window = f.handle.gtk_window(window).unwrap();
        let header = gtk_window.titlebar().expect("a header bar");
        let button = find::<gtk::MenuButton>(&header, &|_| true).expect("the menu button");
        assert!(button.get_visible(), "the menu button shows");
        let model = button.menu_model().expect("a menu model");
        // One section per menu (labelled with its title), then Quit.
        let label = |i| model.item_attribute_value(i, "label", None).and_then(|v| v.get::<String>());
        assert_eq!(model.n_items(), 3);
        assert_eq!((label(0).as_deref(), label(1).as_deref(), label(2)), (Some("File"), Some("View"), None));
        let file = model.item_link(0, "section").expect("the File section");
        let attr = |i, name| file.item_attribute_value(i, name, None).and_then(|v| v.get::<String>());
        assert_eq!(attr(0, "label").as_deref(), Some("New"));
        assert_eq!(attr(0, "accel").as_deref(), Some("<Control>n"));
        let action = attr(0, "action").unwrap();
        let triggers = gtk_window
            .observe_controllers()
            .iter::<glib::Object>()
            .filter_map(|c| c.ok()?.downcast::<gtk::ShortcutController>().ok())
            .flat_map(|c| {
                c.iter::<glib::Object>().filter_map(|s| s.ok()?.downcast::<gtk::Shortcut>().ok()).collect::<Vec<_>>()
            })
            .filter_map(|s| s.trigger().map(|t| t.to_str().to_string()))
            .collect::<Vec<_>>();
        assert!(triggers.contains(&"<Control>n".to_string()), "the shortcut works in the window: {triggers:?}");

        // Disabled items don't run.
        let _ = gtk_window.activate_action(&attr(1, "action").unwrap(), None);
        gtk_window.activate_action(&action, None).expect("the action exists");
        f.ui.tick();
        assert_eq!(chosen.get(), 1, "the handler ran, and only the enabled one");
        f.ui.destroy(window);
        f.ui.tick();
    }

    /// The window's primary menu model.
    fn menu_model(f: &Fixture, window: NodeId) -> gtk::gio::MenuModel {
        let header = f.handle.gtk_window(window).unwrap().titlebar().expect("a header bar");
        let button = find::<gtk::MenuButton>(&header, &|_| true).expect("the menu button");
        button.menu_model().expect("a menu model")
    }

    fn string(model: &gtk::gio::MenuModel, i: i32, name: &str) -> Option<String> {
        model.item_attribute_value(i, name, None).and_then(|v| v.get::<String>())
    }

    fn labels(model: &gtk::gio::MenuModel) -> Vec<String> {
        (0..model.n_items()).map(|i| string(model, i, "label").unwrap_or_default()).collect()
    }

    /// `mitsuami.item-3` → `item-3`, as the window's action group names it.
    fn action(model: &gtk::gio::MenuModel, i: i32) -> String {
        string(model, i, "action").unwrap().trim_start_matches("mitsuami.").to_string()
    }

    pub fn items_check_nest_and_take_roles(f: &Fixture) {
        let window = f.ui.create_window("menu host", Size::new(400.0, 300.0));
        f.ui.tick();
        let (sidebar, zoom) = (signal(true), signal(1));
        f.ui.set_menu(
            MenuBar::new()
                .menu(
                    Menu::new("View")
                        .item(MenuItem::new("Sidebar").bind(sidebar))
                        .separator()
                        .item(MenuItem::new("Small").radio((zoom, 1)))
                        .item(MenuItem::new("Large").radio((zoom, 2)))
                        .submenu(Menu::new("Recent").item(MenuItem::new("a.txt"))),
                )
                .menu(Menu::new("Help").item(MenuItem::new("About Host").role(MenuRole::About)))
                .menu(Menu::new("Edit").item(MenuItem::new("Preferences").role(MenuRole::Settings))),
        );
        f.ui.tick();
        let model = menu_model(f, window);
        // View's two sections, then the last one: Help and Edit are left
        // empty by their roles' items, and go.
        assert_eq!(labels(&model), ["View", "", ""]);
        let last = model.item_link(2, "section").unwrap();
        assert_eq!(labels(&last), ["Preferences", "About Host", "Quit"], "GNOME's order");
        assert_eq!(string(&last, 0, "accel").as_deref(), Some("<Control>comma"));

        let actions = f.handle.menu_actions(window).expect("the window's menu actions");
        let first = model.item_link(0, "section").unwrap();
        let check = action(&first, 0);
        assert_eq!(actions.action_state(&check).and_then(|s| s.get::<bool>()), Some(true));
        actions.activate_action(&check, None);
        f.ui.tick();
        assert!(!sidebar.get_untracked(), "choosing toggles the signal");
        assert_eq!(actions.action_state(&check).and_then(|s| s.get::<bool>()), Some(false));
        assert_eq!(menu_model(f, window), model, "states change in place");

        // Radio items: each item's action holds its id while chosen, and
        // the item targets its id.
        let second = model.item_link(1, "section").unwrap();
        let (small, large) = (action(&second, 0), action(&second, 1));
        let state = |name: &str| actions.action_state(name).and_then(|s| s.get::<String>()).unwrap();
        let large_target = string(&second, 1, "target").expect("a radio target");
        assert_eq!(state(&small), string(&second, 0, "target").unwrap());
        assert_eq!(state(&large), "");
        actions.activate_action(&large, Some(&large_target.to_variant()));
        f.ui.tick();
        assert_eq!(zoom.get_untracked(), 2);
        assert_eq!((state(&small), state(&large)), (String::new(), large_target));

        let recent = second.item_link(2, "submenu").expect("the Recent submenu");
        assert_eq!(string(&second, 2, "label").as_deref(), Some("Recent"));
        assert_eq!(labels(&recent.item_link(0, "section").unwrap()), ["a.txt"]);
        f.ui.set_menu(MenuBar::new());
        f.ui.destroy(window);
        f.ui.tick();
    }

    pub fn windows_show_their_own_menus(f: &Fixture) {
        let (first, second) = (
            f.ui.create_window("first", Size::new(400.0, 300.0)),
            f.ui.create_window("second", Size::new(400.0, 300.0)),
        );
        f.ui.set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        // Before the windows are created: the view builds its menus first.
        f.ui.set_window_menu(
            first,
            MenuBar::new()
                .menu(Menu::new("File").item(MenuItem::new("Run")))
                .menu(Menu::new("Machine").item(MenuItem::new("Start"))),
        );
        f.ui.tick();
        // The window's File joins the app's, after a separator: a section.
        assert_eq!(labels(&menu_model(f, first)), ["File", "", "Machine", ""]);
        assert_eq!(labels(&menu_model(f, second)), ["File", ""]);

        f.ui.set_window_menu(first, MenuBar::new());
        f.ui.tick();
        assert_eq!(labels(&menu_model(f, first)), ["File", ""]);
        f.ui.set_menu(MenuBar::new());
        f.ui.destroy(first);
        f.ui.destroy(second);
        f.ui.tick();
    }

    /// Dialogs have no app menus on GNOME, nor Quit: only their own.
    pub fn dialogs_show_only_their_own_menus(f: &Fixture) {
        let main = f.ui.create_window("main", Size::new(400.0, 300.0));
        let dialog = f.ui.create_window("dialog", Size::new(300.0, 200.0));
        // Set before its Create goes out, as a modal `Window` does.
        f.ui.set_prop(dialog, Prop::Modal { owner: Some(main), modality: Modality::Window });
        f.ui.set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        f.ui.set_window_menu(dialog, MenuBar::new().menu(Menu::new("Format").item(MenuItem::new("Bold"))));
        f.ui.tick();
        assert_eq!(labels(&menu_model(f, main)), ["File", ""], "File, then Quit");
        assert_eq!(labels(&menu_model(f, dialog)), ["Format"]);
        f.ui.destroy(dialog);
        f.ui.destroy(main);
        f.ui.set_menu(MenuBar::new());
        f.ui.tick();
    }

    pub fn alerts_are_answered_through_their_buttons(f: &Fixture) {
        let window = f.ui.create_window("alert host", Size::new(400.0, 300.0));
        f.ui.tick();
        let answer = Rc::new(RefCell::new(None));
        let a = answer.clone();
        let reply = f.ui.alert(Some(window), Alert::new("Proceed?").button("Yes").button("No"));
        f.ui.spawn_local(async move { *a.borrow_mut() = Some(reply.await) });
        pump_until(f, "the alert", || dialog_window(f).is_some());

        let dialog = dialog_window(f).unwrap();
        assert_eq!(dialog.transient_for(), f.handle.gtk_window(window), "attached to its window");
        let no =
            find::<gtk::Button>(dialog.upcast_ref(), &|b| b.label().as_deref() == Some("No")).expect("the No button");
        no.emit_clicked();
        pump_until(f, "the answer", || answer.borrow().is_some());

        assert_eq!(*answer.borrow(), Some(1));
        f.ui.destroy(window);
        f.ui.tick();
    }

    pub fn file_dialogs_report_cancellation(f: &Fixture) {
        let window = f.ui.create_window("dialog host", Size::new(600.0, 400.0));
        f.ui.tick();
        let answer = Rc::new(RefCell::new(Some(Some(vec![]))));
        let a = answer.clone();
        let reply = f.ui.open_file(Some(window), OpenFile::new().multiple());
        f.ui.spawn_local(async move { *a.borrow_mut() = Some(reply.await) });
        pump_until(f, "the file dialog", || dialog_window(f).is_some());

        dialog_window(f).unwrap().close();
        pump_until(f, "the answer", || *answer.borrow() == Some(None));
        f.ui.destroy(window);
        f.ui.tick();
    }

    #[allow(deprecated)] // GtkFileChooser: what GtkFileDialog shows without a portal
    pub fn file_dialogs_start_in_their_folder_and_offer_every_file(f: &Fixture) {
        let window = f.ui.create_window("dialog host", Size::new(600.0, 400.0));
        f.ui.tick();
        let folder = std::env::temp_dir().canonicalize().unwrap();
        let request = OpenFile::new()
            .filter(FileFilter::new("Text", ["txt"]))
            .filter(FileFilter::all("All files"))
            .start_folder(&folder);
        let answer = Rc::new(RefCell::new(None));
        let a = answer.clone();
        let reply = f.ui.open_file(Some(window), request);
        f.ui.spawn_local(async move { *a.borrow_mut() = Some(reply.await) });
        pump_until(f, "the file dialog", || dialog_window(f).is_some());

        let dialog = dialog_window(f).unwrap();
        let chooser = dialog.dynamic_cast_ref::<gtk::FileChooser>().expect("a file chooser");
        pump_until(f, "the start folder", || chooser.current_folder().and_then(|d| d.path()).as_ref() == Some(&folder));
        let filters: Vec<gtk::FileFilter> =
            chooser.filters().iter::<gtk::FileFilter>().filter_map(Result::ok).collect();
        let names: Vec<_> = filters.iter().map(|f| f.name().unwrap_or_default().to_string()).collect();
        assert_eq!(names, ["Text", "All files"]);
        assert!(filters[1].to_gvariant().print(false).contains("'*'"), "every file: {}", filters[1].to_gvariant());

        dialog.close();
        pump_until(f, "the answer", || *answer.borrow() == Some(None));
        f.ui.destroy(window);
        f.ui.tick();
    }
}

#[cfg(target_os = "linux")]
fn main() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    type Check = (&'static str, fn(&checks::Fixture));
    let checks: [Check; 8] = [
        ("clipboard_round_trips", checks::clipboard_round_trips),
        ("menus_are_installed_and_activate", checks::menus_are_installed_and_activate),
        ("items_check_nest_and_take_roles", checks::items_check_nest_and_take_roles),
        ("windows_show_their_own_menus", checks::windows_show_their_own_menus),
        ("dialogs_show_only_their_own_menus", checks::dialogs_show_only_their_own_menus),
        ("alerts_are_answered_through_their_buttons", checks::alerts_are_answered_through_their_buttons),
        ("file_dialogs_report_cancellation", checks::file_dialogs_report_cancellation),
        (
            "file_dialogs_start_in_their_folder_and_offer_every_file",
            checks::file_dialogs_start_in_their_folder_and_offer_every_file,
        ),
    ];
    let filter: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let fixture = checks::fixture();
    let mut failed = 0;
    println!("\nrunning {} tests", checks.len());
    for (name, check) in checks {
        if !filter.is_empty() && !filter.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let ok = catch_unwind(AssertUnwindSafe(|| check(&fixture))).is_ok();
        println!("test {name} ... {}", if ok { "ok" } else { "FAILED" });
        failed += usize::from(!ok);
    }
    println!("\ntest result: {}. {failed} failed\n", if failed == 0 { "ok" } else { "FAILED" });
    std::process::exit(if failed == 0 { 0 } else { 101 });
}

#[cfg(not(target_os = "linux"))]
fn main() {}
