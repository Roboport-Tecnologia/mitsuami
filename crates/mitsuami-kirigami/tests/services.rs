//! The real KDE services, end to end: clipboard, the global drawer menu,
//! alert dialogs, file dialogs and the trash. App tests use scripted
//! services instead, so this is where the native ones are checked. Runs on
//! Qt's offscreen platform, whose clipboard is the process's own.
//!
//! A plain `main` (harness = false): Qt has to run on the main thread.

#[cfg(target_os = "linux")]
mod checks {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use mitsuami_core::services::{Alert, FileFilter, Menu, MenuBar, MenuItem, MenuRole, OpenFile, Shortcut};
    use mitsuami_core::{Modality, Prop, Size, Ui};
    use mitsuami_kirigami::{BackendOptions, KirigamiBackend, KirigamiHandle};
    use mitsuami_reactive::signal;

    pub struct Fixture {
        pub ui: Ui,
        pub handle: KirigamiHandle,
    }

    pub fn fixture() -> Fixture {
        mitsuami_kirigami::init_for_tests();
        let backend = KirigamiBackend::new(BackendOptions::default());
        let handle = backend.handle();
        Fixture { ui: Ui::new(backend), handle }
    }

    /// Lets Qt deliver what it queued and ticks the UI, until `done`.
    fn pump_until(f: &Fixture, what: &str, done: impl Fn() -> bool) {
        let started = Instant::now();
        while !done() {
            assert!(started.elapsed() < Duration::from_secs(5), "timed out waiting for {what}");
            f.handle.pump();
            f.ui.tick();
            f.handle.show_pending_windows();
            std::thread::sleep(Duration::from_millis(5));
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
        // The menu first, as apps do: windows get their drawer with them.
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
        let window = f.ui.create_window("menu host", Size::new(400.0, 300.0));
        f.ui.tick();
        let qml_window = f.handle.qml_window(window).unwrap();
        let drawer = qml_window.object("globalDrawer").expect("a global drawer");
        assert!(drawer.bool("isMenu"), "shown as a menu, from the toolbar");
        // One submenu per menu, then Quit.
        for title in ["File", "View", "Quit"] {
            assert!(drawer.find("text", title).is_some(), "the drawer has {title}");
        }
        let quit = drawer.find("text", "Quit").unwrap();
        assert!(!quit.str("shortcut").is_empty(), "Quit has the standard shortcut");
        let new = drawer.find("text", "New").expect("the New item");
        assert_eq!(new.str("shortcut"), "Ctrl+N");

        // Disabled items don't run.
        drawer.find("text", "Unavailable").unwrap().invoke("trigger");
        new.invoke("trigger");
        f.ui.tick();
        assert_eq!(chosen.get(), 1, "the handler ran, and only the enabled one");
        f.ui.destroy(window);
        f.ui.tick();
    }

    pub fn submenus_check_marks_and_radio_groups(f: &Fixture) {
        let (opened, sidebar, zoom) = (Rc::new(Cell::new(0)), signal(true), signal(1));
        let o = opened.clone();
        f.ui.set_menu(
            MenuBar::new()
                .menu(Menu::new("File").submenu(
                    Menu::new("Open Recent").item(MenuItem::new("notes.txt").on_select(move || o.set(o.get() + 1))),
                ))
                .menu(
                    Menu::new("View")
                        .item(MenuItem::new("Show Sidebar").bind(sidebar))
                        .separator()
                        .item(MenuItem::new("Small").radio((zoom, 1)))
                        .item(MenuItem::new("Large").radio((zoom, 2))),
                ),
        );
        let window = f.ui.create_window("menu host", Size::new(400.0, 300.0));
        f.ui.tick();
        let drawer = f.handle.qml_window(window).unwrap().object("globalDrawer").expect("a global drawer");
        assert!(drawer.find("text", "Open Recent").is_some(), "the submenu");
        drawer.find("text", "notes.txt").expect("its item").invoke("trigger");
        f.ui.tick();
        assert_eq!(opened.get(), 1);

        let show = drawer.find("text", "Show Sidebar").expect("the check item");
        assert!(show.bool("checkable") && show.bool("checked"));
        show.invoke("trigger");
        f.ui.tick();
        assert!(!sidebar.get_untracked(), "choosing it toggles the signal");
        assert!(!show.bool("checked"), "and the drawer follows");

        let (small, large) = (drawer.find("text", "Small").unwrap(), drawer.find("text", "Large").unwrap());
        assert!(small.bool("checked") && !large.bool("checked"));
        large.invoke("trigger");
        f.ui.tick();
        assert_eq!(zoom.get_untracked(), 2);
        assert!(!small.bool("checked") && large.bool("checked"), "one choice of the group");
        f.ui.destroy(window);
        f.ui.tick();
    }

    /// Settings, About and the app's Quit end the drawer, with KDE's icons
    /// and shortcuts; the Help menu About leaves empty goes.
    pub fn role_items_end_the_drawer(f: &Fixture) {
        let quit = Rc::new(Cell::new(0));
        let q = quit.clone();
        f.ui.set_menu(
            MenuBar::new()
                .menu(Menu::new("Edit").item(MenuItem::new("Preferences…").role(MenuRole::Settings)))
                .menu(Menu::new("Help").item(MenuItem::new("About Launcher").role(MenuRole::About)))
                .menu(Menu::new("File").item(MenuItem::new("Exit").role(MenuRole::Quit).on_select(move || q.set(1)))),
        );
        let window = f.ui.create_window("menu host", Size::new(400.0, 300.0));
        f.ui.tick();
        let drawer = f.handle.qml_window(window).unwrap().object("globalDrawer").expect("a global drawer");
        let settings = drawer.find("text", "Preferences…").expect("the settings item");
        assert_eq!(settings.str("shortcut"), "Ctrl+Shift+,");
        assert!(drawer.find("text", "About Launcher").is_some());
        assert!(drawer.find("text", "Help").is_none(), "left empty, so gone");
        assert!(drawer.child("mitsuamiQuit").is_none(), "the app's Quit replaces ours");
        let exit = drawer.find("text", "Quit").expect("the app's Quit, with KDE's title");
        assert_eq!(exit.str("shortcut"), "Ctrl+Q");
        exit.invoke("trigger");
        f.ui.tick();
        assert_eq!(quit.get(), 1, "the app's handler ran");
        f.ui.destroy(window);
        f.ui.tick();
    }

    /// A window's own menus arrive before it's created, as a `MenuBar` in
    /// its content sends them, and are only in its drawer.
    pub fn windows_have_their_own_menus(f: &Fixture) {
        f.ui.set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        let main = f.ui.create_window("main", Size::new(400.0, 300.0));
        let other = f.ui.create_window("other", Size::new(400.0, 300.0));
        f.ui.set_window_menu(main, MenuBar::new().menu(Menu::new("Machine").item(MenuItem::new("Start"))));
        f.ui.tick();
        let drawer = |w| f.handle.qml_window(w).unwrap().object("globalDrawer").expect("a global drawer");
        assert!(drawer(main).find("text", "Start").is_some() && drawer(main).find("text", "New").is_some());
        assert!(drawer(other).find("text", "Start").is_none(), "only in its window");
        assert!(drawer(other).find("text", "New").is_some(), "the app's are in every window");
        f.ui.destroy(main);
        f.ui.destroy(other);
        f.ui.tick();
    }

    /// Dialogs have no app menus on KDE, nor Quit: only their own.
    pub fn dialogs_show_only_their_own_menus(f: &Fixture) {
        let main = f.ui.create_window("main", Size::new(400.0, 300.0));
        let dialog = f.ui.create_window("dialog", Size::new(300.0, 200.0));
        // Set before its Create goes out, as a modal `Window` does.
        f.ui.set_prop(dialog, Prop::Modal { owner: Some(main), modality: Modality::Window });
        f.ui.set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        f.ui.set_window_menu(dialog, MenuBar::new().menu(Menu::new("Format").item(MenuItem::new("Bold"))));
        f.ui.tick();
        let drawer = f.handle.qml_window(dialog).unwrap().object("globalDrawer").expect("the dialog's own drawer");
        assert!(drawer.find("text", "Format").is_some());
        assert!(drawer.find("text", "File").is_none(), "no app menus");
        assert!(drawer.find("text", "Quit").is_none(), "no Quit");
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
        pump_until(f, "the alert", || !f.handle.open_dialogs().is_empty());

        let dialog = f.handle.open_dialogs()[0];
        assert_eq!(dialog.str("title"), "Proceed?");
        let overlay = f.handle.qml_window(window).unwrap().object("overlay");
        assert_eq!(dialog.object("parent"), overlay, "shown in its window");
        pump_until(f, "the dialog to open", || dialog.bool("opened"));
        dialog.find("text", "No").expect("the No button").invoke("trigger");
        pump_until(f, "the answer", || answer.borrow().is_some());

        assert_eq!(*answer.borrow(), Some(1));
        assert!(f.handle.open_dialogs().is_empty());
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
        pump_until(f, "the file dialog", || !f.handle.open_dialogs().is_empty());

        let dialog = f.handle.open_dialogs()[0];
        assert_eq!(dialog.object("parentWindow"), f.handle.qml_window(window), "attached to its window");
        dialog.invoke("reject");
        pump_until(f, "the answer", || *answer.borrow() == Some(None));
        f.ui.destroy(window);
        f.ui.tick();
    }

    /// Destroying their window rejects both, as Escape would: the alert
    /// answers its cancel button, the file dialog no paths.
    pub fn dialogs_close_with_their_window(f: &Fixture) {
        let window = f.ui.create_window("dialog host", Size::new(600.0, 400.0));
        f.ui.tick();
        let answers = Rc::new(RefCell::new((None, None)));
        let (a, b) = (answers.clone(), answers.clone());
        let alert = f.ui.alert(Some(window), Alert::new("Proceed?").button("Yes").button("No"));
        let open = f.ui.open_file(Some(window), OpenFile::new());
        f.ui.spawn_local(async move { a.borrow_mut().0 = Some(alert.await) });
        f.ui.spawn_local(async move { b.borrow_mut().1 = Some(open.await) });
        pump_until(f, "both dialogs", || f.handle.open_dialogs().len() == 2);
        let alert = f.handle.open_dialogs()[0];
        pump_until(f, "the alert to open", || alert.bool("opened"));

        f.ui.destroy(window);
        f.ui.tick();
        pump_until(f, "the answers", || *answers.borrow() == (Some(1), Some(None)));
        assert!(f.handle.open_dialogs().is_empty());
    }

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
        pump_until(f, "the file dialog", || !f.handle.open_dialogs().is_empty());

        let dialog = f.handle.open_dialogs()[0];
        assert_eq!(dialog.str("currentFolder"), format!("file://{}", folder.display()));
        assert_eq!(dialog.str_list("nameFilters"), ["Text (*.txt)", "All files (*)"]);

        dialog.invoke("reject");
        pump_until(f, "the answer", || *answer.borrow() == Some(None));
        f.ui.destroy(window);
        f.ui.tick();
    }

    /// Trashes a file of its own into the home trash (the file is made in
    /// the data folder, on the trash's disk), checks the freedesktop.org
    /// trash has it with the `.trashinfo` Dolphin restores from, and deletes both.
    pub fn trash_moves_files_to_the_home_trash(f: &Fixture) {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::home_dir().unwrap().join(".local/share"));
        let name = format!("mitsuami-trash-check-{}.txt", std::process::id());
        let file = data.join(&name);
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(&file, b"trash me").unwrap();
        let answer = Rc::new(RefCell::new(None));
        let a = answer.clone();
        let reply = f.ui.trash(None, vec![file.clone()]);
        f.ui.spawn_local(async move { *a.borrow_mut() = Some(reply.await) });
        pump_until(f, "the trash", || answer.borrow().is_some());

        assert_eq!(*answer.borrow(), Some(Ok(())));
        assert!(!file.exists(), "the file left its folder");
        let (trashed, info) =
            (data.join("Trash/files").join(&name), data.join("Trash/info").join(format!("{name}.trashinfo")));
        assert!(trashed.exists() && info.exists(), "in the trash, with what restoring it needs");
        std::fs::remove_file(trashed).unwrap();
        std::fs::remove_file(info).unwrap();
    }
}

#[cfg(target_os = "linux")]
fn main() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    type Check = (&'static str, fn(&checks::Fixture));
    let checks: [Check; 11] = [
        ("clipboard_round_trips", checks::clipboard_round_trips),
        ("menus_are_installed_and_activate", checks::menus_are_installed_and_activate),
        ("submenus_check_marks_and_radio_groups", checks::submenus_check_marks_and_radio_groups),
        ("role_items_end_the_drawer", checks::role_items_end_the_drawer),
        ("windows_have_their_own_menus", checks::windows_have_their_own_menus),
        ("dialogs_show_only_their_own_menus", checks::dialogs_show_only_their_own_menus),
        ("alerts_are_answered_through_their_buttons", checks::alerts_are_answered_through_their_buttons),
        ("file_dialogs_report_cancellation", checks::file_dialogs_report_cancellation),
        ("dialogs_close_with_their_window", checks::dialogs_close_with_their_window),
        (
            "file_dialogs_start_in_their_folder_and_offer_every_file",
            checks::file_dialogs_start_in_their_folder_and_offer_every_file,
        ),
        ("trash_moves_files_to_the_home_trash", checks::trash_moves_files_to_the_home_trash),
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
