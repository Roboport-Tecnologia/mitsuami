//! `MenuButton`: a button that opens a menu of actions, the platform's
//! own (a pull-down, a menu button, a drop-down button). The native
//! button carries its menu, and tests choose its items as assistive
//! technology does, without opening it: nothing here may open modal UI.

use mitsuami::core::services::{MenuCheck, MenuEntry, find_menu_item};
use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// The menu the native button carries.
fn native_menu(app: &TestApp, label: &str) -> Vec<MenuEntry> {
    let props = app.get(add(label)).native_state().props;
    props
        .into_iter()
        .find_map(|p| match p {
            Prop::Menu(entries) => Some(entries),
            _ => None,
        })
        .expect("a menu")
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

fn add(label: &str) -> Query {
    by_role(Role::MenuButton, label)
}

fn plus() -> String {
    platform! {
        macos => "plus",
        gtk => "list-add-symbolic",
        kde => "list-add",
        windows => "\u{E710}",
        _ => "plus",
    }
    .to_owned()
}

#[mitsuami_test::test]
async fn shows_its_caption_and_carries_its_menu(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).child(MenuButton::new("Add").menu((
            MenuItem::new("Disc image…"),
            MenuItem::new("Folder…"),
            MenuSeparator,
            Menu::new("Guest tools").item(MenuItem::new("3dfx")).item(MenuItem::new("Sound")),
        )))
    });

    app.expect(add("Add")).to_be_visible().await;
    assert_eq!(app.get(add("Add")).native_state().kind, WidgetKind::MenuButton);
    assert!(app.get(add("Add")).native_state().props.contains(&Prop::Label("Add".into())));
    let menu = native_menu(&app, "Add");
    assert_eq!(titles(&menu), ["Disc image…", "Folder…", "-", "Guest tools >"]);
    let Some(MenuEntry::Submenu(tools)) = menu.get(3) else { panic!("a submenu") };
    assert_eq!(titles(&tools.entries), ["3dfx", "Sound"]);
}

#[mitsuami_test::test]
async fn choosing_an_item_runs_it(app: TestApp) {
    let chosen = signal(String::new());
    let choose = move |what: &'static str| move || chosen.set(what.into());
    app.mount(move || {
        Column::new().align(Align::Start).children((
            MenuButton::new("Add").menu((
                MenuItem::new("Disc image…").on_select(choose("image")),
                MenuSeparator,
                Menu::new("Guest tools").item(MenuItem::new("3dfx").on_select(choose("3dfx"))),
            )),
            Text::new(chosen).test_id("chosen"),
        ))
    });

    app.get(add("Add")).choose_menu_item(&["Disc image…"]).await;
    app.expect(by_test_id("chosen")).to_have_text("image").await;
    app.get(add("Add")).choose_menu_item(&["Guest tools", "3dfx"]).await;
    app.expect(by_test_id("chosen")).to_have_text("3dfx").await;
}

/// A menu built with a closure is built again when what it reads changes,
/// and its new items run.
#[mitsuami_test::test]
async fn a_built_menu_follows_what_it_reads(app: TestApp) {
    let recent = signal(vec!["a.txt".to_owned()]);
    let chosen = signal(String::new());
    app.mount(move || {
        Column::new().align(Align::Start).children((
            MenuButton::new("Add").menu_with(move || {
                recent
                    .get()
                    .into_iter()
                    .map(|name| MenuItem::new(name.clone()).on_select(move || chosen.set(name.clone())))
                    .collect::<Vec<_>>()
            }),
            Text::new(chosen).test_id("chosen"),
        ))
    });
    app.expect(add("Add")).to_be_visible().await;
    assert_eq!(titles(&native_menu(&app, "Add")), ["a.txt"]);

    recent.set(vec!["b.txt".to_owned(), "a.txt".to_owned()]);
    app.settle().await;
    assert_eq!(titles(&native_menu(&app, "Add")), ["b.txt", "a.txt"]);
    app.get(add("Add")).choose_menu_item(&["b.txt"]).await;
    app.expect(by_test_id("chosen")).to_have_text("b.txt").await;
}

/// Clicking opens the menu, which would be modal: it has no click of its
/// own for assistive technology to take in its place.
#[mitsuami_test::test]
async fn takes_no_click(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).child(MenuButton::new("Add").menu(MenuItem::new("Folder…"))));
    app.expect(add("Add")).to_be_visible().await;

    let id = app.get(add("Add")).id();
    assert_eq!(app.ui().perform(id, &A11yAction::Activate), Err(ActionError::Unsupported));
}

#[mitsuami_test::test]
async fn a_disabled_item_or_button_cannot_be_chosen(app: TestApp) {
    let enabled = signal(true);
    app.mount(move || {
        Column::new().align(Align::Start).child(
            MenuButton::new("Add")
                .enabled(enabled)
                .menu((MenuItem::new("Folder…"), MenuItem::new("Disc image…").enabled(false))),
        )
    });
    app.expect(add("Add")).to_be_visible().await;

    let id = app.get(add("Add")).id();
    let menu = native_menu(&app, "Add");
    let item = |title: &str| find_menu_item(&menu, &[title]).expect("an item").id;
    let MenuEntry::Item(image) = &menu[1] else { panic!("an item") };
    assert!(!image.enabled);
    assert_eq!(app.ui().perform(id, &A11yAction::MenuItem(item("Disc image…"))), Err(ActionError::Disabled));

    enabled.set(false);
    app.settle().await;
    assert_eq!(app.ui().perform(id, &A11yAction::MenuItem(item("Folder…"))), Err(ActionError::Disabled));
}

/// Titles, check marks and which items show follow the app's state.
#[mitsuami_test::test]
async fn follows_its_menu(app: TestApp) {
    let show_hidden = signal(false);
    let tools = signal(false);
    let title = move || if show_hidden.get() { "Hide hidden files" } else { "Show hidden files" }.to_owned();
    app.mount(move || {
        Column::new().align(Align::Start).child(MenuButton::new("View").menu((
            MenuItem::new(title).on_select(move || show_hidden.update(|s| *s = !*s)),
            MenuItem::new("Guest tools").bind(tools),
        )))
    });
    app.expect(add("View")).to_be_visible().await;
    assert_eq!(titles(&native_menu(&app, "View")), ["Show hidden files", "Guest tools"]);

    app.get(add("View")).choose_menu_item(&["Show hidden files"]).await;
    assert_eq!(titles(&native_menu(&app, "View")), ["Hide hidden files", "Guest tools"]);

    app.get(add("View")).choose_menu_item(&["Guest tools"]).await;
    assert!(tools.get_untracked());
    let menu = native_menu(&app, "View");
    let MenuEntry::Item(item) = &menu[1] else { panic!("an item") };
    assert_eq!(item.check, MenuCheck::Check(true));
}

/// An icon before the caption makes it wider; alone, narrower. It reads
/// as its caption either way, and takes focus from Tab as buttons do.
#[mitsuami_test::test]
async fn shows_an_icon_before_its_caption_or_alone(app: TestApp) {
    // The same caption with and without the icon (captions of the same
    // length aren't as wide in proportional fonts), long enough that the
    // button is wider than Breeze's minimum (80 points), which a short
    // caption and its icon both fit within.
    const CAPTION: &str = "Add to the library";
    app.mount(|| {
        Column::new().align(Align::Start).gap(8).children((
            MenuButton::new(CAPTION).menu(MenuItem::new("Folder…")).test_id("caption"),
            MenuButton::new(CAPTION).icon(plus()).menu(MenuItem::new("Folder…")).test_id("icon and caption"),
            MenuButton::new("New").icon(plus()).icon_only(true).menu(MenuItem::new("Folder…")),
        ))
    });
    app.expect(add("New")).to_be_visible().await;

    let width = |query: Query| app.get(query).frame().width();
    let (caption, both) = (width(by_test_id("caption")), width(by_test_id("icon and caption")));
    let alone = width(add("New"));
    assert!(both > caption, "{both} > {caption}");
    assert!(alone < caption, "{alone} < {caption}");
    assert!(app.get(by_test_id("icon and caption")).native_state().props.contains(&Prop::Icon(plus())));
    assert!(app.get(add("New")).native_state().props.contains(&Prop::IconOnly(true)));
    assert_eq!(app.ui().focus_order(app.window()).len(), 3);
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let chosen = signal(false);
    app.mount(move || {
        view! {
            <MenuButton icon=plus() menu=MenuItem::new("Folder…").on_select(move || chosen.set(true))>"Add"</MenuButton>
        }
    });

    app.get(add("Add")).choose_menu_item(&["Folder…"]).await;
    assert!(chosen.get_untracked());
}

mitsuami_test::main!();
