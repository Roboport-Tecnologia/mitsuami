//! The real WinUI services, end to end: clipboard (kept in memory), the
//! menu bar, alerts (`ContentDialog`) and file pickers. App tests use
//! scripted services instead, so this is where the native ones are checked.
//!
//! A plain `main` (harness = false): XAML runs on the thread that started it.

#[cfg(all(windows, target_env = "msvc"))]
mod checks {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use mitsuami_core::services::{Alert, Menu, MenuBar, MenuItem, OpenFile, Shortcut};
    use mitsuami_core::{Modality, NodeId, Prop, Size, Ui};
    use mitsuami_reactive::signal;
    use mitsuami_winui::bindings as w;
    use mitsuami_winui::{BackendOptions, WinUiBackend, WinUiHandle};
    use windows_core::Interface;

    pub struct Fixture {
        pub ui: Ui,
        pub handle: WinUiHandle,
    }

    pub fn fixture() -> Fixture {
        mitsuami_winui::init_for_tests();
        let options = BackendOptions { show_windows: false, private_clipboard: true, ..BackendOptions::default() };
        let backend = WinUiBackend::new(options);
        let handle = backend.handle();
        Fixture { ui: Ui::new(backend), handle }
    }

    /// Lets XAML deliver completions, then ticks the UI, until `done`.
    fn pump_until(ui: &Ui, what: &str, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            mitsuami_winui::pump();
            ui.tick();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn children(object: &w::DependencyObject) -> Vec<w::DependencyObject> {
        let count = w::VisualTreeHelper::GetChildrenCount(object).unwrap_or(0);
        (0..count).filter_map(|i| w::VisualTreeHelper::GetChild(object, i).ok()).collect()
    }

    /// A button in the visual tree under `object` whose content is `label`.
    fn find_button(object: &w::DependencyObject, label: &str) -> Option<w::Button> {
        if let Ok(button) = object.cast::<w::Button>() {
            let content = button.cast::<w::IContentControl>().ok()?.Content().ok();
            let text = content.and_then(|c| c.cast::<w::IPropertyValue>().ok()?.GetString().ok());
            if text.as_deref() == Some(label) {
                return Some(button);
            }
        }
        children(object).iter().find_map(|c| find_button(c, label))
    }

    fn invoke(element: &impl Interface) {
        let peer = w::FrameworkElementAutomationPeer::CreatePeerForElement(&element.cast::<w::UIElement>().unwrap())
            .expect("an automation peer");
        let invoke: w::IInvokeProvider =
            peer.GetPattern(w::PatternInterface::Invoke).and_then(|p| p.cast()).expect("the Invoke pattern");
        invoke.Invoke().unwrap();
    }

    fn window(f: &Fixture, title: &str) -> NodeId {
        let window = f.ui.create_window(title, Size::new(400.0, 300.0));
        f.ui.tick();
        window
    }

    /// The window's root grid: the menu bar row, then the content.
    fn root(f: &Fixture, window: NodeId) -> w::DependencyObject {
        let xaml = f.handle.xaml_window(window).expect("a XAML window");
        xaml.cast::<w::IWindow>().unwrap().Content().unwrap().cast().unwrap()
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
        f.ui.tick();
        assert_eq!(*written.borrow(), Some(Ok(())));
        assert_eq!(read.borrow().as_deref(), Some("mitsuami ✓"));
    }

    pub fn menus_are_installed_and_activate(f: &Fixture) {
        let window = window(f, "menu host");
        let chosen = Rc::new(Cell::new(0));
        let c = chosen.clone();
        f.ui.set_menu(
            MenuBar::new()
                .menu(
                    Menu::new("File")
                        .item(
                            MenuItem::new("New").on_select(move || c.set(c.get() + 1)).shortcut(Shortcut::primary('n')),
                        )
                        .item(MenuItem::new("Unavailable").enabled(false)),
                )
                .menu(Menu::new("View").item(MenuItem::new("Zoom"))),
        );
        f.ui.tick();

        let bar: w::MenuBar =
            children(&root(f, window)).into_iter().find_map(|c| c.cast().ok()).expect("a MenuBar in the window");
        let menus = bar.cast::<w::IMenuBar>().unwrap().Items().unwrap();
        let titles: Vec<String> = (0..menus.Size().unwrap())
            .map(|i| menus.GetAt(i).unwrap().cast::<w::IMenuBarItem>().unwrap().Title().unwrap())
            .collect();
        assert_eq!(titles, ["File", "View"]);

        let file = menus.GetAt(0).unwrap().cast::<w::IMenuBarItem>().unwrap().Items().unwrap();
        let new = file.GetAt(0).unwrap();
        let accelerators = new.cast::<w::IUIElement>().unwrap().KeyboardAccelerators().unwrap();
        let accelerator = accelerators.GetAt(0).expect("a keyboard accelerator");
        let accelerator: w::IKeyboardAccelerator = accelerator.cast().unwrap();
        assert_eq!(accelerator.Key().unwrap(), w::VirtualKey(b'N' as i32));
        assert_eq!(accelerator.Modifiers().unwrap(), w::VirtualKeyModifiers::Control);
        assert!(new.cast::<w::IControl>().unwrap().IsEnabled().unwrap());
        assert!(!file.GetAt(1).unwrap().cast::<w::IControl>().unwrap().IsEnabled().unwrap());

        invoke(&new);
        f.ui.tick();
        assert_eq!(chosen.get(), 1, "the handler ran");
        f.ui.set_menu(MenuBar::new());
        f.ui.destroy(window);
        f.ui.tick();
    }

    fn menu_bar(f: &Fixture, window: NodeId) -> Option<windows_collections::IVector<w::MenuBarItem>> {
        let bar: w::MenuBar = children(&root(f, window)).into_iter().find_map(|c| c.cast().ok())?;
        bar.cast::<w::IMenuBar>().unwrap().Items().ok()
    }

    fn titles(menus: &windows_collections::IVector<w::MenuBarItem>) -> Vec<String> {
        (0..menus.Size().unwrap())
            .map(|i| menus.GetAt(i).unwrap().cast::<w::IMenuBarItem>().unwrap().Title().unwrap())
            .collect()
    }

    fn text(item: &w::MenuFlyoutItemBase) -> String {
        match item.cast::<w::IMenuFlyoutSubItem>() {
            Ok(sub) => sub.Text().unwrap(),
            Err(_) => item.cast::<w::IMenuFlyoutItem>().map(|i| i.Text().unwrap()).unwrap_or_default(),
        }
    }

    /// A window's own menus join the app's in its bar, and only its bar;
    /// check marks are toggle and radio items, updated in place.
    pub fn window_menus_nest_and_check(f: &Fixture) {
        let (host, other) = (window(f, "menu host"), window(f, "other window"));
        let sidebar = signal(true);
        let zoom = signal(1);
        f.ui.set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        f.ui.set_window_menu(
            host,
            MenuBar::new().menu(Menu::new("File").item(MenuItem::new("Run"))).menu(
                Menu::new("View")
                    .item(MenuItem::new("Show Sidebar").bind(sidebar))
                    .separator()
                    .item(MenuItem::new("Small").radio((zoom, 1)))
                    .item(MenuItem::new("Large").radio((zoom, 2)))
                    .submenu(Menu::new("Tools").item(MenuItem::new("Options"))),
            ),
        );
        f.ui.tick();

        let menus = menu_bar(f, host).expect("a MenuBar in the window");
        assert_eq!(titles(&menus), ["File", "View"]);
        let file = menus.GetAt(0).unwrap().cast::<w::IMenuBarItem>().unwrap().Items().unwrap();
        let file: Vec<String> = (0..file.Size().unwrap()).map(|i| text(&file.GetAt(i).unwrap())).collect();
        assert_eq!(file, ["New", "", "Run"], "the window's File joins the app's, after a separator");
        assert_eq!(titles(&menu_bar(f, other).expect("the app's menus")), ["File"]);

        let view = menus.GetAt(1).unwrap().cast::<w::IMenuBarItem>().unwrap().Items().unwrap();
        let toggle = view.GetAt(0).unwrap();
        assert!(toggle.cast::<w::IToggleMenuFlyoutItem>().unwrap().IsChecked().unwrap());
        let radio = |i| view.GetAt(i).unwrap().cast::<w::IRadioMenuFlyoutItem>().unwrap();
        assert_eq!(radio(2).GroupName().unwrap(), radio(3).GroupName().unwrap(), "one group");
        assert!(radio(2).IsChecked().unwrap() && !radio(3).IsChecked().unwrap());
        let tools = view.GetAt(4).unwrap().cast::<w::IMenuFlyoutSubItem>().expect("a submenu");
        assert_eq!(tools.Text().unwrap(), "Tools");
        assert_eq!(text(&tools.Items().unwrap().GetAt(0).unwrap()), "Options");

        sidebar.set(false);
        zoom.set(2);
        f.ui.tick();
        assert!(!toggle.cast::<w::IToggleMenuFlyoutItem>().unwrap().IsChecked().unwrap(), "the same item, updated");
        assert!(!radio(2).IsChecked().unwrap() && radio(3).IsChecked().unwrap());

        f.ui.set_window_menu(host, MenuBar::new());
        f.ui.set_menu(MenuBar::new());
        f.ui.tick();
        assert!(menu_bar(f, host).is_none(), "no menus, no bar");
        f.ui.destroy(host);
        f.ui.destroy(other);
        f.ui.tick();
    }

    /// Windows dialogs have no menu bar of the app's: only their own.
    pub fn dialogs_show_only_their_own_menus(f: &Fixture) {
        let main = f.ui.create_window("main", Size::new(400.0, 300.0));
        let dialog = f.ui.create_window("dialog", Size::new(300.0, 200.0));
        // Set before its Create goes out, as a modal `Window` does.
        f.ui.set_prop(dialog, Prop::Modal { owner: Some(main), modality: Modality::Window });
        f.ui.set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        f.ui.set_window_menu(dialog, MenuBar::new().menu(Menu::new("Format").item(MenuItem::new("Bold"))));
        f.ui.tick();
        let bar: w::MenuBar =
            children(&root(f, dialog)).into_iter().find_map(|c| c.cast().ok()).expect("the dialog's own MenuBar");
        let menus = bar.cast::<w::IMenuBar>().unwrap().Items().unwrap();
        let titles: Vec<String> = (0..menus.Size().unwrap())
            .map(|i| menus.GetAt(i).unwrap().cast::<w::IMenuBarItem>().unwrap().Title().unwrap().to_string())
            .collect();
        assert_eq!(titles, ["Format"]);
        f.ui.destroy(dialog);
        f.ui.destroy(main);
        f.ui.set_menu(MenuBar::new());
        f.ui.tick();
    }

    pub fn alerts_are_answered_through_their_dialog(f: &Fixture) {
        let window = window(f, "alert host");
        let answer = Rc::new(RefCell::new(None));
        let a = answer.clone();
        let reply = f.ui.alert(Some(window), Alert::new("Proceed?").button("Yes").button("Maybe").button("No"));
        f.ui.spawn_local(async move { *a.borrow_mut() = Some(reply.await) });

        let xaml_root = root(f, window).cast::<w::IUIElement>().unwrap().XamlRoot().unwrap();
        let no = Rc::new(RefCell::new(None));
        pump_until(&f.ui, "the dialog's No button", || {
            let popups = w::VisualTreeHelper::GetOpenPopupsForXamlRoot(&xaml_root).unwrap();
            let found = (0..popups.Size().unwrap_or(0)).find_map(|i| {
                let child = popups.GetAt(i).ok()?.cast::<w::IPopup>().ok()?.Child().ok()?;
                find_button(&child.cast().ok()?, "No")
            });
            *no.borrow_mut() = found;
            no.borrow().is_some()
        });
        invoke(no.borrow().as_ref().unwrap());
        pump_until(&f.ui, "the answer", || answer.borrow().is_some());

        assert_eq!(*answer.borrow(), Some(2), "the last button is the dialog's close button");
        f.ui.destroy(window);
        f.ui.tick();
    }

    /// The file dialog's top-level window, owned by our process.
    fn file_dialog() -> Option<w::HWND> {
        unsafe extern "system" fn visit(hwnd: w::HWND, found: w::LPARAM) -> windows_core::BOOL {
            let mut pid = 0;
            unsafe { w::GetWindowThreadProcessId(hwnd, &mut pid) };
            let mut class = [0u16; 64];
            let len = unsafe { w::GetClassNameW(hwnd, windows_core::PWSTR(class.as_mut_ptr()), 64) };
            let is_dialog = String::from_utf16_lossy(&class[..len.max(0) as usize]) == "#32770";
            if pid == unsafe { w::GetCurrentProcessId() } && is_dialog && unsafe { w::IsWindowVisible(hwnd) }.as_bool()
            {
                unsafe { *(found as *mut w::HWND) = hwnd };
                return false.into();
            }
            true.into()
        }
        let mut found: w::HWND = std::ptr::null_mut();
        unsafe { _ = w::EnumWindows(Some(visit), &mut found as *mut w::HWND as w::LPARAM) };
        (!found.is_null()).then_some(found)
    }

    pub fn open_pickers_report_cancellation(f: &Fixture) {
        let window = window(f, "picker host");
        let answer = Rc::new(RefCell::new(Some(Some(vec![]))));
        let a = answer.clone();
        let reply = f.ui.open_file(Some(window), OpenFile::new().multiple());
        f.ui.spawn_local(async move { *a.borrow_mut() = Some(reply.await) });

        let dialog = Rc::new(Cell::new(None));
        pump_until(&f.ui, "the file dialog", || {
            dialog.set(file_dialog());
            dialog.get().is_some()
        });
        unsafe { _ = w::PostMessageW(dialog.get().unwrap(), w::WM_CLOSE as u32, 0, 0) };
        pump_until(&f.ui, "the answer", || !matches!(*answer.borrow(), Some(Some(_))));

        assert_eq!(*answer.borrow(), Some(None), "cancelled");
        f.ui.destroy(window);
        f.ui.tick();
    }
}

#[cfg(all(windows, target_env = "msvc"))]
fn main() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    type Check = (&'static str, fn(&checks::Fixture));
    let checks: [Check; 6] = [
        ("clipboard_round_trips", checks::clipboard_round_trips),
        ("menus_are_installed_and_activate", checks::menus_are_installed_and_activate),
        ("window_menus_nest_and_check", checks::window_menus_nest_and_check),
        ("dialogs_show_only_their_own_menus", checks::dialogs_show_only_their_own_menus),
        ("alerts_are_answered_through_their_dialog", checks::alerts_are_answered_through_their_dialog),
        ("open_pickers_report_cancellation", checks::open_pickers_report_cancellation),
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

#[cfg(not(all(windows, target_env = "msvc")))]
fn main() {}
