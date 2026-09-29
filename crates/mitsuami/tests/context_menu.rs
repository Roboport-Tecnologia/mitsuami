//! Context menus: a menu any widget or container can have, which the
//! platform shows its own way (a right-click, a long press, the menu key)
//! over the widget and over children without one of their own. The native
//! widget carries it, and tests choose its items as assistive technology
//! does, without opening it: nothing here can right-click a native widget.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::services::{MenuCheck, MenuEntry, find_menu_item};
use mitsuami::core::{A11yAction, ActionError, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// The menu the native widget carries.
fn native_menu(app: &TestApp, query: Query) -> Option<Vec<MenuEntry>> {
    app.get(query).native_state().props.into_iter().find_map(|p| match p {
        Prop::ContextMenu(entries) => Some(entries),
        _ => None,
    })
}

fn titles(entries: &[MenuEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Item(item) => item.title.clone(),
            MenuEntry::Separator => "-".to_owned(),
            MenuEntry::Submenu(menu) => format!("{} >", menu.title),
        })
        .collect()
}

/// On AppKit a `Select` keeps its menu without showing it: a pop-up
/// button's menu is its options, which a right-click shows too.
#[mitsuami_test::test]
async fn any_widget_carries_one(app: TestApp) {
    let red = Pixels::new(1, 1, [255, 0, 0, 255]);
    let menu = |title: &str| (MenuItem::new(format!("{title} item")), MenuSeparator, MenuItem::new("Delete"));
    app.mount(move || {
        Column::new().test_id("box").context_menu(menu("Box")).children((
            Text::new("Status").context_menu(menu("Text")),
            Button::new("Play").context_menu(menu("Button")),
            TextInput::new().a11y_label("Name").context_menu(menu("Field")),
            Checkbox::new("Sound").context_menu(menu("Checkbox")),
            Select::new("Family").options(["98", "XP"]).context_menu(menu("Select")),
            NumberInput::new("Memory").context_menu(menu("Spin")),
            Image::pixels(red.clone()).label("Preview").context_menu(menu("Image")),
        ))
    });

    for (query, title) in [
        (by_test_id("box"), "Box"),
        (by_text("Status"), "Text"),
        (by_role(Role::Button, "Play"), "Button"),
        (by_label("Name"), "Field"),
        (by_role(Role::Checkbox, "Sound"), "Checkbox"),
        (by_role(Role::ComboBox, "Family"), "Select"),
        (by_role(Role::SpinButton, "Memory"), "Spin"),
        (by_role(Role::Image, "Preview"), "Image"),
    ] {
        let entries = native_menu(&app, query).unwrap_or_else(|| panic!("{title} has no menu"));
        assert_eq!(titles(&entries), [format!("{title} item"), "-".into(), "Delete".into()]);
    }
}

#[mitsuami_test::test]
async fn choosing_an_item_runs_it(app: TestApp) {
    let chosen = signal(String::new());
    let choose = move |what: &'static str| move || chosen.set(what.into());
    app.mount(move || {
        Column::new().children((
            Text::new("notes.txt").context_menu((
                MenuItem::new("Open").on_select(choose("open")),
                MenuSeparator,
                Menu::new("Sort By")
                    .item(MenuItem::new("Name").on_select(choose("by name")))
                    .item(MenuItem::new("Date").on_select(choose("by date"))),
            )),
            Text::new(chosen).test_id("chosen"),
        ))
    });

    app.get(by_text("notes.txt")).choose_menu_item(&["Open"]).await;
    assert_eq!(app.get(by_test_id("chosen")).text().as_deref(), Some("open"));

    app.get(by_text("notes.txt")).choose_menu_item(&["Sort By", "Date"]).await;
    assert_eq!(app.get(by_test_id("chosen")).text().as_deref(), Some("by date"));
}

/// Shortcuts name keys that type nothing too, with or without modifiers:
/// the menu shows them, the platform's way, and they read back the same.
#[mitsuami_test::test]
async fn shortcuts_name_any_key(app: TestApp) {
    let shortcuts = [
        Shortcut::primary(Key::Backspace),
        Shortcut::new(Key::F(2)),
        Shortcut::new(Key::Up).alt(),
        Shortcut::primary(Key::Down),
        Shortcut::new(Key::Delete),
        Shortcut::new(Key::Enter),
        Shortcut::primary('O').shift(),
    ];
    app.mount(move || {
        let items: Vec<MenuItem> =
            shortcuts.iter().enumerate().map(|(i, s)| MenuItem::new(format!("Item {i}")).shortcut(*s)).collect();
        Text::new("notes.txt").context_menu(items)
    });
    let entries = native_menu(&app, by_text("notes.txt")).expect("a menu");
    let shown: Vec<Option<Shortcut>> = entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Item(item) => item.shortcut,
            _ => None,
        })
        .collect();
    assert_eq!(shown, shortcuts.map(Some));
    assert_eq!(shortcuts[6].key, Key::Char('o'), "a letter is lower case, with Shift apart");
}

/// As a right-click does, a child without a menu shows its container's.
#[mitsuami_test::test]
async fn children_show_their_containers_menu(app: TestApp) {
    let opened = signal(String::new());
    app.mount(move || {
        Column::new().children((
            Row::new()
                .test_id("row")
                .gap(8)
                .context_menu(MenuItem::new("Open").on_select(move || opened.set("notes.txt".into())))
                .children((Text::new("notes.txt"), Button::new("Share").context_menu(MenuItem::new("Share With…")))),
            Text::new(opened).test_id("opened"),
        ))
    });

    let (owner, entries) = app.get(by_text("notes.txt")).context_menu().expect("the row's menu");
    assert_eq!((owner, titles(&entries)), (app.get(by_test_id("row")).id(), vec!["Open".to_owned()]));
    app.get(by_text("notes.txt")).choose_menu_item(&["Open"]).await;
    assert_eq!(app.get(by_test_id("opened")).text().as_deref(), Some("notes.txt"));

    let (_, entries) = app.get(by_role(Role::Button, "Share")).context_menu().expect("its own");
    assert_eq!(titles(&entries), ["Share With…"]);
    assert!(app.get(by_test_id("opened")).context_menu().is_none());
}

#[mitsuami_test::test]
async fn items_follow_state(app: TestApp) {
    let playing = signal(false);
    let sound = signal(true);
    app.mount(move || {
        Text::new("Machine").context_menu((
            MenuItem::new(move || if playing.get() { "Pause" } else { "Start" }.to_owned())
                .on_select(move || playing.update(|p| *p = !*p)),
            MenuItem::new("Reset").enabled(playing),
            MenuItem::new("Debug").visible(playing),
            MenuSeparator,
            MenuItem::new("Sound").bind(sound),
        ))
    });
    let machine = || app.get(by_text("Machine"));
    let menu = || native_menu(&app, by_text("Machine")).unwrap();
    let item = |title: &str| find_menu_item(&menu(), &[title]).cloned().unwrap();
    assert_eq!(titles(&menu()), ["Start", "Reset", "-", "Sound"]);
    assert!(!item("Reset").enabled);
    assert_eq!(item("Sound").check, MenuCheck::Check(true));

    machine().choose_menu_item(&["Start"]).await;
    assert_eq!(titles(&menu()), ["Pause", "Reset", "Debug", "-", "Sound"]);
    assert!(item("Reset").enabled);

    machine().choose_menu_item(&["Sound"]).await;
    assert!(!sound.get());
    assert_eq!(item("Sound").check, MenuCheck::Check(false));
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sort {
    Name,
    Date,
}

#[mitsuami_test::test]
async fn radio_items_choose_a_value(app: TestApp) {
    let sort = signal(Sort::Name);
    app.mount(move || {
        Text::new(move || format!("{:?}", sort.get())).context_menu(
            Menu::new("Sort By")
                .item(MenuItem::new("Name").radio((sort, Sort::Name)))
                .item(MenuItem::new("Date").radio((sort, Sort::Date))),
        )
    });
    let check = |title: &str| {
        let menu = native_menu(&app, by_role(Role::StaticText, format!("{:?}", sort.get()))).unwrap();
        find_menu_item(&menu, &["Sort By", title]).unwrap().check
    };
    assert_eq!((check("Name"), check("Date")), (MenuCheck::Radio(true), MenuCheck::Radio(false)));

    app.get(by_text("Name")).choose_menu_item(&["Sort By", "Date"]).await;
    app.expect(by_text("Date")).to_exist().await;
    assert_eq!((check("Name"), check("Date")), (MenuCheck::Radio(false), MenuCheck::Radio(true)));
}

/// Disabled items can't be chosen, as the platform's menu won't, and a
/// disabled widget shows no menu.
#[mitsuami_test::test]
async fn disabled_items_and_widgets_cant_be_chosen(app: TestApp) {
    let ran = Rc::new(RefCell::new(Vec::new()));
    let log = ran.clone();
    app.mount(move || {
        let (a, b) = (log.clone(), log.clone());
        Column::new().children((
            Text::new("Status")
                .context_menu(MenuItem::new("Copy").enabled(false).on_select(move || a.borrow_mut().push("copy"))),
            Button::new("Play")
                .enabled(false)
                .context_menu(MenuItem::new("Play Twice").on_select(move || b.borrow_mut().push("twice"))),
        ))
    });

    for (query, title) in [(by_text("Status"), "Copy"), (by_role(Role::Button, "Play"), "Play Twice")] {
        let (node, menu) = app.get(query).context_menu().unwrap();
        let id = find_menu_item(&menu, &[title]).unwrap().id;
        assert_eq!(app.ui().perform(node, &A11yAction::ContextMenuItem(id)), Err(ActionError::Disabled), "{title}");
    }
    app.settle().await;
    assert!(ran.borrow().is_empty());
}

/// Hiding every item leaves no menu: a right-click shows the platform's
/// own, or none.
#[mitsuami_test::test]
async fn no_items_no_menu(app: TestApp) {
    let editable = signal(true);
    app.mount(move || Text::new("Status").context_menu(MenuItem::new("Edit").visible(editable)));
    assert!(app.get(by_text("Status")).context_menu().is_some());

    editable.set(false);
    app.settle().await;
    assert!(app.get(by_text("Status")).context_menu().is_none());
    assert_eq!(native_menu(&app, by_text("Status")), Some(Vec::new()));
}

#[derive(Clone, PartialEq, Debug)]
struct Machine {
    id: u32,
    name: String,
}

/// 2ksbox's machine list: each row has Start and Delete.
#[mitsuami_test::test]
async fn list_rows_have_their_own(app: TestApp) {
    let machines =
        signal(vec![Machine { id: 1, name: "Windows 98".into() }, Machine { id: 2, name: "Windows XP".into() }]);
    let started = signal(String::new());
    app.mount(move || {
        Column::new().children((
            List::new(
                machines,
                |m: &Machine| m.id,
                move |m| {
                    let (name, id) = (m.name.clone(), m.id);
                    Container::new().height(24).child(Text::new(m.name)).context_menu((
                        MenuItem::new("Start").on_select(move || started.set(name.clone())),
                        MenuSeparator,
                        MenuItem::new("Delete").on_select(move || machines.update(|all| all.retain(|m| m.id != id))),
                    ))
                },
            )
            .height(100),
            Text::new(started).test_id("started"),
        ))
    });

    app.get(by_text("Windows XP")).choose_menu_item(&["Start"]).await;
    assert_eq!(app.get(by_test_id("started")).text().as_deref(), Some("Windows XP"));

    app.get(by_text("Windows 98")).choose_menu_item(&["Delete"]).await;
    app.expect(by_text("Windows 98")).not_to_exist().await;
    app.expect(by_role(Role::ListItem, "Windows XP")).to_exist().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let chosen = signal(false);
    app.mount(move || {
        view! {
            <Column>
                <Button context_menu=(MenuItem::new("Copy Link").on_select(move || chosen.set(true)),)>"Share"</Button>
            </Column>
        }
    });

    app.get(by_role(Role::Button, "Share")).choose_menu_item(&["Copy Link"]).await;
    assert!(chosen.get());
}

mitsuami_test::main!();
