//! Menus: submenus, check marks and radio groups, reactive titles and
//! structure, roles, and a window's own menus written in `view!`. Tests see
//! the menus through the scripted services, which get what the platform
//! gets; the platforms' own services tests check what they build from it.

use mitsuami::core::services::{MenuCheck, MenuEntry, MenuRole};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Zoom {
    Small,
    Large,
}

#[mitsuami_test::test]
async fn submenus_hold_items_and_submenus(app: TestApp) {
    let opened = signal(String::new());
    app.mount(move || {
        set_menu(
            MenuBar::new().menu(
                Menu::new("File").item(MenuItem::new("New")).submenu(
                    Menu::new("Open Recent")
                        .item(MenuItem::new("notes.txt").on_select(move || opened.set("notes.txt".into())))
                        .submenu(Menu::new("Older").item(MenuItem::new("old.txt"))),
                ),
            ),
        );
        Text::new(opened)
    });
    let menu = app.services().menu().unwrap();
    let MenuEntry::Submenu(recent) = &menu.menus[0].entries[1] else { panic!("a submenu") };
    assert_eq!(recent.title, "Open Recent");

    assert!(app.services().choose_menu_item(&["File", "Open Recent", "notes.txt"]));
    app.expect(by_text("notes.txt")).to_exist().await;
    assert!(app.services().menu_item(&["File", "Open Recent", "Older", "old.txt"]).is_some());
    assert!(app.services().menu_item(&["File", "notes.txt"]).is_none(), "only through its submenu");
}

#[mitsuami_test::test]
async fn bound_check_marks_toggle_their_signal(app: TestApp) {
    let sidebar = signal(true);
    app.mount(move || {
        set_menu(MenuBar::new().menu(Menu::new("View").item(MenuItem::new("Show Sidebar").bind(sidebar))));
        Show::new(move || sidebar.get(), || Text::new("Sidebar"))
    });
    let check = || app.services().menu_item(&["View", "Show Sidebar"]).unwrap().check;
    assert_eq!(check(), MenuCheck::Check(true));

    assert!(app.services().choose_menu_item(&["View", "Show Sidebar"]));
    app.expect(by_text("Sidebar")).not_to_exist().await;
    assert_eq!(check(), MenuCheck::Check(false));

    sidebar.set(true);
    app.settle().await;
    assert_eq!(check(), MenuCheck::Check(true), "the menu follows the signal");
}

#[mitsuami_test::test]
async fn radio_items_choose_a_value(app: TestApp) {
    let zoom = signal(Zoom::Small);
    app.mount(move || {
        set_menu(
            MenuBar::new().menu(
                Menu::new("View")
                    .item(MenuItem::new("Small").radio((zoom, Zoom::Small)))
                    .item(MenuItem::new("Large").radio((zoom, Zoom::Large)))
                    .separator()
                    .item(MenuItem::new("Reset")),
            ),
        );
        Text::new(move || format!("{:?}", zoom.get()))
    });
    let check = |title| app.services().menu_item(&["View", title]).unwrap().check;
    assert_eq!((check("Small"), check("Large")), (MenuCheck::Radio(true), MenuCheck::Radio(false)));

    assert!(app.services().choose_menu_item(&["View", "Large"]));
    app.expect(by_text("Large")).to_exist().await;
    assert_eq!((check("Small"), check("Large")), (MenuCheck::Radio(false), MenuCheck::Radio(true)));

    // Radio items next to each other are one group, named by the first.
    let menu = app.services().menu().unwrap();
    let small = app.services().menu_item(&["View", "Small"]).unwrap().id;
    assert_eq!(menu.menus[0].radio_groups(), [Some(small), Some(small), None, None]);
}

#[mitsuami_test::test]
async fn titles_and_which_items_there_are_follow_state(app: TestApp) {
    let playing = signal(false);
    let recent = signal(vec!["a.txt".to_string()]);
    app.mount(move || {
        set_menu(
            MenuBar::new()
                .menu(
                    Menu::new("Machine")
                        .item(MenuItem::new(move || if playing.get() { "Pause" } else { "Start" }.to_string()))
                        .item(MenuItem::new("Reset").visible(playing)),
                )
                .menu(
                    Menu::new("File").submenu(
                        Menu::new("Open Recent")
                            .children_with(move || recent.get().into_iter().map(MenuItem::new).collect::<Vec<_>>()),
                    ),
                )
                .menu(Menu::new("Debug").visible(playing)),
        );
        Text::new("Launcher")
    });
    let titles = || app.services().menu().unwrap().items().iter().map(|i| i.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles(), ["Start", "a.txt"]);
    let start = app.services().menu_item(&["Machine", "Start"]).unwrap().id;

    playing.set(true);
    recent.update(|r| r.insert(0, "b.txt".into()));
    app.settle().await;
    assert_eq!(titles(), ["Pause", "Reset", "b.txt", "a.txt"]);
    let menus: Vec<String> = app.services().menu().unwrap().menus.iter().map(|m| m.title.clone()).collect();
    assert_eq!(menus, ["Machine", "File", "Debug"]);
    assert_eq!(app.services().menu_item(&["Machine", "Pause"]).unwrap().id, start, "same place, same id");
}

/// The data says what each item is; where it goes is the platform's
/// choice (`MenuBarData::take_role`).
#[mitsuami_test::test]
async fn roles_are_sent_with_their_items(app: TestApp) {
    app.mount(|| {
        set_menu(
            MenuBar::new()
                .menu(
                    Menu::new("Edit")
                        .item(MenuItem::new("Find"))
                        .separator()
                        .item(MenuItem::new("Settings…").role(MenuRole::Settings)),
                )
                .menu(Menu::new("Help").item(MenuItem::new("About Launcher").role(MenuRole::About))),
        );
        Text::new("Launcher")
    });
    let mut menu = app.services().menu().unwrap();
    assert_eq!(app.services().menu_item(&["Edit", "Settings…"]).unwrap().role, MenuRole::Settings);

    // As on macOS: both move to the app menu, taking with them the
    // separator before Settings and the Help menu they leave empty.
    let settings = menu.take_role(MenuRole::Settings).unwrap();
    let about = menu.take_role(MenuRole::About).unwrap();
    assert_eq!((settings.title.as_str(), about.title.as_str()), ("Settings…", "About Launcher"));
    assert_eq!(menu.menus.len(), 1);
    assert_eq!(menu.menus[0].entries.len(), 1, "Find, without a separator");
}

#[mitsuami_test::test]
async fn a_window_has_its_own_menus_while_they_are_declared(app: TestApp) {
    let running = signal(false);
    let editing = signal(false);
    app.mount(move || {
        set_menu(MenuBar::new().menu(Menu::new("File").item(MenuItem::new("New"))));
        view! {
            <Column>
                <MenuBar>
                    <Menu title="File">
                        <MenuItem shortcut=Shortcut::primary('r') @select=move || running.set(true)>"Run"</MenuItem>
                    </Menu>
                    <Menu title="Machine">
                        <MenuItem bind=running>"Running"</MenuItem>
                        <MenuSeparator/>
                        <Menu title="Zoom">
                            <MenuItem role=MenuRole::Settings>"Settings…"</MenuItem>
                        </Menu>
                    </Menu>
                </MenuBar>
                <Text>{move || if running.get() { "Running" } else { "Stopped" }.to_string()}</Text>
                <Window title="Editor" bind=editing>
                    <Column>
                        <MenuBar>
                            <Menu title="Format">
                                <MenuItem>"Bold"</MenuItem>
                            </Menu>
                        </MenuBar>
                        <Text>"Editing"</Text>
                    </Column>
                </Window>
            </Column>
        }
    });
    let own = app.services().window_menu(app.window()).expect("the window's menus");
    let titles: Vec<&str> = own.menus.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, ["File", "Machine"]);
    assert_eq!(app.services().menu().unwrap().menus.len(), 1, "the app's menus stay apart");

    // Shown with the app's: a window's File joins the app's File.
    let shown = app.services().menu().unwrap().merged(&own);
    let file: Vec<&str> = shown.menus[0]
        .entries
        .iter()
        .map(|e| match e {
            MenuEntry::Item(item) => item.title.as_str(),
            MenuEntry::Separator => "-",
            MenuEntry::Submenu(menu) => menu.title.as_str(),
        })
        .collect();
    assert_eq!(file, ["New", "-", "Run"]);
    // Where each window has its menus, a dialog shows only its own.
    let app_menus = app.services().menu().unwrap();
    assert_eq!(app_menus.for_window(Some(&own), true), own);
    assert!(app_menus.for_window(None, true).menus.is_empty());

    assert!(app.services().choose_menu_item(&["File", "Run"]));
    app.expect(by_text("Running")).to_exist().await;
    assert_eq!(app.services().menu_item(&["Machine", "Running"]).unwrap().check, MenuCheck::Check(true));

    editing.set(true);
    app.settle().await;
    let editor = app.window_titled("Editor").expect("the editor opened");
    assert!(app.services().window_menu(editor).is_some());
    assert!(app.services().choose_menu_item(&["Format", "Bold"]));

    editing.set(false);
    app.settle().await;
    assert!(app.services().window_menu(editor).is_none(), "gone with the window");
    assert!(app.services().menu_item(&["Format", "Bold"]).is_none());
    assert!(app.services().window_menu(app.window()).is_some());
}

/// Installing the app's menus again replaces them, and the replaced ones
/// stop following their state.
#[mitsuami_test::test]
async fn app_menus_are_replaced(app: TestApp) {
    let title = signal("First".to_string());
    app.mount(move || {
        set_menu(MenuBar::new().menu(Menu::new("Old").item(MenuItem::new(title))));
        set_menu(MenuBar::new().menu(Menu::new("New").item(MenuItem::new("Second"))));
        Text::new("Launcher")
    });
    title.set("Changed".into());
    app.settle().await;
    let menus: Vec<String> = app.services().menu().unwrap().menus.iter().map(|m| m.title.clone()).collect();
    assert_eq!(menus, ["New"]);
}

/// The platform's Quit from outside the app (the Dock, logging out) is the
/// app's Quit item, as if chosen: it decides, here by asking first.
#[mitsuami_test::test]
async fn the_platform_s_quit_is_the_app_s_quit_item(app: TestApp) {
    let asked = signal(0);
    let open = signal(true);
    app.mount(move || {
        set_menu(
            MenuBar::new().menu(
                Menu::new("File")
                    .item(MenuItem::new("Quit").role(MenuRole::Quit).on_select(move || asked.update(|n| *n += 1))),
            ),
        );
        Window::new("Machine").bind(open).content(|| Text::new("Running"))
    });

    app.ui().request_quit();
    app.settle().await;
    assert_eq!(asked.get_untracked(), 1);
    assert!(open.get_untracked(), "the app decides; the windows aren't asked");
}

/// Without a Quit item of the app's, every window is asked to close, as by
/// its close button: those that close go, those whose app keeps them stay.
#[mitsuami_test::test]
async fn without_a_quit_item_every_window_is_asked_to_close(app: TestApp) {
    let (closes, kept) = (signal(true), signal(0));
    app.mount(move || {
        Column::new().children((
            Window::new("Closes").bind(closes).content(|| Text::new("One")),
            Window::new("Asks").on_close_request(move || kept.update(|n| *n += 1)).content(|| Text::new("Two")),
        ))
    });

    app.ui().request_quit();
    app.settle().await;
    assert!(!closes.get_untracked());
    assert_eq!(app.window_titled("Closes"), None);
    assert_eq!(kept.get_untracked(), 1);
    assert!(app.window_titled("Asks").is_some());
}

mitsuami_test::main!();
