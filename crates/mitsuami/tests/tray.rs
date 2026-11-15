//! The tray icon (`set_tray`, `TrayIcon`), through the `tray` example. The
//! scripted services stand in for the platform: they keep the icon's data,
//! and the test chooses its menu's items as a click would.

#[allow(dead_code)]
#[path = "../examples/tray.rs"]
mod example;

use std::time::Duration;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// Starts the example's app as `App::run` would: no window, in a scope that
/// holds the Ui.
fn start(app: &TestApp) -> Owner {
    app.ui().set_runs_without_windows(true);
    let owner = Owner::new_root();
    owner.with(|| {
        provide(app.ui().clone());
        _ = example::window().build(app.ui());
    });
    owner
}

#[mitsuami_test::test]
async fn the_icon_and_its_menu_reach_the_platform(app: TestApp) {
    let _app = start(&app);
    app.settle().await;
    let tray = app.services().tray().expect("a tray icon");
    assert_eq!(tray.title, "Tray");
    assert_eq!(tray.icon.as_ref().and_then(AppIcon::name), Some("media-playback-start-symbolic"));
    assert!(tray.icon.as_ref().and_then(AppIcon::read).is_some(), "with an image where there's no theme");
    assert_eq!(tray.tooltip, "Running");
    let titles: Vec<_> = tray
        .menu
        .iter()
        .map(|entry| match entry {
            mitsuami::core::services::MenuEntry::Item(item) => item.title.clone(),
            mitsuami::core::services::MenuEntry::Separator => "—".to_owned(),
            mitsuami::core::services::MenuEntry::Submenu(menu) => menu.title.clone(),
        })
        .collect();
    assert_eq!(titles, ["Open Tray", "Paused", "—", "Quit"]);
    assert!(app.ui().tray_shown().get());
}

#[mitsuami_test::test]
async fn its_items_run_and_its_parts_follow(app: TestApp) {
    let _app = start(&app);
    app.settle().await;
    assert!(app.services().choose_tray_item(&["Paused"]));
    app.settle().await;
    let tray = app.services().tray().unwrap();
    assert_eq!(tray.icon.as_ref().and_then(AppIcon::name), Some("media-playback-pause-symbolic"));
    assert_eq!(tray.tooltip, "Paused");
    assert_eq!(app.services().tray_item(&["Paused"]).unwrap().check, mitsuami::core::services::MenuCheck::Check(true));

    assert!(app.ui().windows().is_empty());
    assert!(app.services().choose_tray_item(&["Open Tray"]));
    app.settle().await;
    assert_eq!(app.ui().windows().len(), 1, "the menu opens the window");
    app.expect(by_test_id("state")).to_have_text("Paused.").await;

    assert!(app.services().choose_tray_item(&["Quit"]));
    app.settle().await;
    assert!(app.ui().should_end());
}

#[mitsuami_test::test]
async fn the_window_chooses_how_the_icon_is_given(app: TestApp) {
    let _app = start(&app);
    app.settle().await;
    assert!(app.services().choose_tray_item(&["Open Tray"]));
    app.settle().await;
    let icon = || app.services().tray().expect("a tray icon").icon;

    app.get_by_label("Icon").select_option(example::CHOICES[1]).await;
    assert_eq!(icon(), Some(example::image()), "an image, for every platform");
    // Windows doesn't recolor tray icons, so the example gives a white one
    // for a dark taskbar; the others recolor or tint theirs.
    let image = icon().expect("an image");
    assert_eq!(image.image_for(true) != image.image_for(false), cfg!(windows), "a dark variant on Windows");

    app.get_by_label("Icon").select_option(example::CHOICES[2]).await;
    let named = icon().expect("a named icon");
    assert_eq!(named.name(), Some("media-playback-start-symbolic"));
    assert!(named.read().is_none(), "nothing for platforms without a theme");

    app.get_by_label("Icon").select_option(example::CHOICES[3]).await;
    assert_eq!(icon(), None, "the app's own");
    assert!(app.services().choose_tray_item(&["Paused"]), "its menu stays");

    app.get_by_label("Icon").select_option(example::CHOICES[0]).await;
    let named = icon().expect("a named icon");
    assert_eq!(named.name(), Some("media-playback-pause-symbolic"));
    assert!(named.read().is_some());
    assert!(app.ui().tray_shown().get());
}

#[mitsuami_test::test]
async fn it_goes_with_its_scope(app: TestApp) {
    let owner = start(&app);
    app.settle().await;
    assert!(app.services().tray().is_some());
    owner.dispose();
    app.settle().await;
    assert!(app.services().tray().is_none());
    assert!(!app.ui().tray_shown().get());
    assert!(!app.services().choose_tray_item(&["Open Tray"]), "its items went with it");
}

#[mitsuami_test::test]
async fn a_later_one_replaces_it(app: TestApp) {
    let first = Owner::new_root();
    first.with(|| app.ui().set_tray(TrayIcon::new("First")));
    let second = Owner::new_root();
    second.with(|| app.ui().set_tray(TrayIcon::new("Second")));
    app.settle().await;
    assert_eq!(app.services().tray().unwrap().title, "Second");
    first.dispose();
    app.settle().await;
    assert_eq!(app.services().tray().unwrap().title, "Second", "the first's scope ending leaves the second");
}

#[mitsuami_test::test]
async fn in_view_its_children_are_its_menu(app: TestApp) {
    let chosen = signal(0);
    app.mount(move || {
        view! {
            <Column>
                <TrayIcon title="Viewed" icon="mail-unread-symbolic" tooltip="Two unread">
                    <MenuItem @select=move || chosen.update(|n| *n += 1)>"Check Mail"</MenuItem>
                </TrayIcon>
                <Text>"Mail"</Text>
            </Column>
        }
    });
    let tray = app.services().tray().unwrap();
    assert_eq!((tray.title.as_str(), tray.tooltip.as_str()), ("Viewed", "Two unread"));
    assert_eq!(tray.icon, Some(AppIcon::named("mail-unread-symbolic")));
    assert!(app.services().choose_tray_item(&["Check Mail"]));
    app.settle().await;
    assert_eq!(chosen.get(), 1);
    app.unmount();
    app.settle().await;
    assert!(app.services().tray().is_none());
}

#[mitsuami_test::test]
async fn without_a_tray_the_window_opens(app: TestApp) {
    app.services().set_tray_host(false);
    let _app = start(&app);
    app.settle().await;
    assert!(!app.ui().tray_shown().get(), "nothing to show it in");
    app.advance(Duration::from_secs(1)).await;
    assert_eq!(app.ui().windows().len(), 1, "nothing else could bring it back");

    app.services().set_tray_host(true);
    app.settle().await;
    assert!(app.ui().tray_shown().get(), "a host that comes shows it");
}

#[mitsuami_test::test]
async fn what_watches_it_may_call_services(app: TestApp) {
    // The platform reports `tray_shown` while it takes the icon; an effect
    // that watches it and calls a service runs after.
    let owner = Owner::new_root();
    owner.with(|| {
        provide(app.ui().clone());
        let ui = app.ui().clone();
        effect(move || {
            if ui.tray_shown().get() {
                _ = set_clipboard_text("shown");
            }
        });
        app.ui().set_tray(TrayIcon::new("Watched"));
    });
    app.settle().await;
    assert!(app.ui().tray_shown().get());
    owner.dispose();
    app.settle().await;
    assert!(!app.ui().tray_shown().get());
}

#[mitsuami_test::test]
async fn a_named_icons_dark_image_goes_in_either_order(app: TestApp) {
    let (light, dark) = (AppIcon::bytes(vec![1]), AppIcon::bytes(vec![2]));
    for icon in [
        AppIcon::named("n").image(light.clone()).dark(dark.clone()),
        AppIcon::named("n").dark(dark.clone()).image(light.clone()),
    ] {
        assert_eq!(icon.name(), Some("n"));
        assert_eq!(icon.image_for(false), Some(&light));
        assert_eq!(icon.image_for(true), Some(&dark));
    }
    _ = app;
}

mitsuami_test::main!();
