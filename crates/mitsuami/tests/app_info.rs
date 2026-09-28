//! The app's id, name and icon: each platform takes what it has a place
//! for, and a window shows it.

use mitsuami::core::{NativeAppInfo, NativeIcon};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

const ID: &str = "org.mitsuami.Tests";
const NAME: &str = "Mitsuami Tests";

fn icon_file() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/assets/blue-red-20x10.png")
}

/// What a window shows of an app with `ID`, `NAME` and the 20 × 10 icon.
fn expected(app: &TestApp) -> NativeAppInfo {
    let (id, name) = (Some(ID.to_owned()), Some(NAME.to_owned()));
    let image = Some(NativeIcon::Image { width: 20, height: 10 });
    match app.backend_name() {
        // The id is the bundle's, and tests run from none.
        "appkit" => NativeAppInfo { id: None, name, icon: image },
        // GTK 4 windows show the theme's icon named after the id.
        "gtk" => NativeAppInfo { id, name, icon: Some(NativeIcon::Named(ID.to_owned())) },
        // The name is the executable's.
        "winui" => NativeAppInfo { id, name: None, icon: image },
        // Qt shows the image when the theme has no icon named after the
        // id, as it has none for the tests.
        _ => NativeAppInfo { id, name, icon: image },
    }
}

#[mitsuami_test::test]
async fn windows_show_the_apps_id_name_and_icon(app: TestApp) {
    app.ui().set_app_info(
        AppInfo::new().id(ID).name(NAME).icon(AppIcon::bytes(include_bytes!("assets/blue-red-20x10.png").as_slice())),
    );
    app.mount(|| Text::new("Hello"));
    assert_eq!(app.native_app_info(app.window()), expected(&app));
}

/// Set again with the window open, as an icon from a file.
#[mitsuami_test::test]
async fn open_windows_take_it_too(app: TestApp) {
    app.mount(|| Text::new("Hello"));
    app.ui().set_app_info(AppInfo::new().id(ID).name(NAME).icon(AppIcon::file(icon_file())));
    app.settle().await;
    assert_eq!(app.native_app_info(app.window()), expected(&app));
}

mitsuami_test::main!();
