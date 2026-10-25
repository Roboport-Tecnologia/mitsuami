//! `SplitButton`: a button with a menu of other actions beside its own, the
//! platform's (a combo button, a split button; joined buttons on
//! Kirigami). It's clicked as a button is, and tests choose its menu's
//! items as assistive technology does, without opening it: nothing here
//! may open modal UI.

use mitsuami::core::services::{MenuEntry, find_menu_item};
use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// The menu the native button carries.
fn native_menu(app: &TestApp, label: &str) -> Vec<MenuEntry> {
    let props = app.get(button(label)).native_state().props;
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

/// It reads as a button: what pressing it does is its own action.
fn button(label: &str) -> Query {
    by_role(Role::Button, label)
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
        Column::new().align(Align::Start).child(SplitButton::new("Continue").menu((
            MenuItem::new("Commit to Edit"),
            MenuSeparator,
            Menu::new("More").item(MenuItem::new("Skip")),
        )))
    });

    app.expect(button("Continue")).to_be_visible().await;
    assert_eq!(app.get(button("Continue")).native_state().kind, WidgetKind::SplitButton);
    assert!(app.get(button("Continue")).native_state().props.contains(&Prop::Label("Continue".into())));
    let menu = native_menu(&app, "Continue");
    assert_eq!(titles(&menu), ["Commit to Edit", "-", "More >"]);
    let Some(MenuEntry::Submenu(more)) = menu.get(2) else { panic!("a submenu") };
    assert_eq!(titles(&more.entries), ["Skip"]);
}

/// A click does its own action; an item of its menu does the item's.
#[mitsuami_test::test]
async fn clicks_and_chooses_items(app: TestApp) {
    let done = signal(String::new());
    app.mount(move || {
        Column::new().align(Align::Start).children((
            SplitButton::new("Merge")
                .on_click(move || done.set("merge".into()))
                .menu(MenuItem::new("External Tool").on_select(move || done.set("external".into()))),
            Text::new(done).test_id("done"),
        ))
    });

    app.get(button("Merge")).click().await;
    app.expect(by_test_id("done")).to_have_text("merge").await;
    app.get(button("Merge")).choose_menu_item(&["External Tool"]).await;
    app.expect(by_test_id("done")).to_have_text("external").await;
}

/// A menu built with a closure is built again when what it reads changes.
#[mitsuami_test::test]
async fn a_built_menu_follows_what_it_reads(app: TestApp) {
    let message = signal(String::new());
    app.mount(move || {
        Column::new().align(Align::Start).child(
            SplitButton::new("Continue")
                .menu_with(move || MenuItem::new("Commit to Edit").enabled(!message.get().is_empty())),
        )
    });
    app.expect(button("Continue")).to_be_visible().await;
    let enabled = |app: &TestApp| match &native_menu(app, "Continue")[0] {
        MenuEntry::Item(item) => item.enabled,
        _ => panic!("an item"),
    };
    assert!(!enabled(&app));

    message.set("Fix the build".into());
    app.settle().await;
    assert!(enabled(&app));
}

#[mitsuami_test::test]
async fn a_disabled_button_or_item_does_nothing(app: TestApp) {
    let enabled = signal(true);
    let clicks = signal(0);
    app.mount(move || {
        Column::new().align(Align::Start).child(
            SplitButton::new("Continue")
                .enabled(enabled)
                .on_click(move || clicks.update(|c| *c += 1))
                .menu((MenuItem::new("Commit to Edit"), MenuItem::new("Skip").enabled(false))),
        )
    });
    app.expect(button("Continue")).to_be_visible().await;

    let id = app.get(button("Continue")).id();
    let menu = native_menu(&app, "Continue");
    let item = |title: &str| find_menu_item(&menu, &[title]).expect("an item").id;
    assert_eq!(app.ui().perform(id, &A11yAction::MenuItem(item("Skip"))), Err(ActionError::Disabled));

    enabled.set(false);
    app.settle().await;
    assert_eq!(app.ui().perform(id, &A11yAction::MenuItem(item("Commit to Edit"))), Err(ActionError::Disabled));
    assert_eq!(app.ui().perform(id, &A11yAction::Activate), Err(ActionError::Disabled));
    app.settle().await;
    assert_eq!(clicks.get_untracked(), 0);
}

/// Wider than a plain button with the same caption, for its arrow; an icon
/// before the caption makes it wider, and alone, narrower. It reads as its
/// caption either way, and takes focus from Tab as buttons do.
#[mitsuami_test::test]
async fn shows_its_arrow_and_an_icon_before_its_caption_or_alone(app: TestApp) {
    // Long enough that the buttons are wider than Breeze's minimum (80
    // points), which a short caption and its icon both fit within.
    const CAPTION: &str = "Add to the library";
    app.mount(|| {
        Column::new().align(Align::Start).gap(8).children((
            Button::new(CAPTION).test_id("plain"),
            SplitButton::new(CAPTION).menu(MenuItem::new("Folder…")).test_id("caption"),
            SplitButton::new(CAPTION).icon(plus()).menu(MenuItem::new("Folder…")).test_id("icon and caption"),
            SplitButton::new("New").icon(plus()).icon_only(true).menu(MenuItem::new("Folder…")),
        ))
    });
    app.expect(button("New")).to_be_visible().await;

    let width = |query: Query| app.get(query).frame().width();
    let (plain, caption) = (width(by_test_id("plain")), width(by_test_id("caption")));
    let both = width(by_test_id("icon and caption"));
    let alone = width(button("New"));
    assert!(caption > plain, "{caption} > {plain}");
    assert!(both > caption, "{both} > {caption}");
    assert!(alone < caption, "{alone} < {caption}");
    assert!(app.get(by_test_id("icon and caption")).native_state().props.contains(&Prop::Icon(plus())));
    assert!(app.get(button("New")).native_state().props.contains(&Prop::IconOnly(true)));
    assert_eq!(app.ui().focus_order(app.window()).len(), 4);
}

/// A new caption shows; one shown with its icon alone stays its name.
#[mitsuami_test::test]
async fn follows_its_caption(app: TestApp) {
    let caption = signal("Continue".to_owned());
    app.mount(move || {
        Column::new().align(Align::Start).gap(8).children((
            SplitButton::new(caption).menu(MenuItem::new("Commit to Edit")).test_id("text"),
            SplitButton::new(caption)
                .icon(plus())
                .icon_only(true)
                .menu(MenuItem::new("Commit to Edit"))
                .test_id("icon"),
        ))
    });
    app.expect(by_test_id("text")).to_be_visible().await;

    caption.set("Resume".into());
    app.settle().await;
    for id in ["text", "icon"] {
        assert!(app.get(by_test_id(id)).native_state().props.contains(&Prop::Label("Resume".into())), "{id}");
    }
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let (clicked, chosen) = (signal(false), signal(false));
    app.mount(move || {
        view! {
            <SplitButton on_click=move || clicked.set(true) menu=MenuItem::new("Folder…").on_select(move || chosen.set(true))>"Add"</SplitButton>
        }
    });

    app.get(button("Add")).click().await;
    app.get(button("Add")).choose_menu_item(&["Folder…"]).await;
    assert!(clicked.get_untracked() && chosen.get_untracked());
}

mitsuami_test::main!();
