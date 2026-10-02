//! `FileIcon`: a file's or folder's own icon, as the platform's file
//! manager shows it, and its thumbnail where the platform makes one. The
//! icons are each platform's (and each machine's): tests check that one is
//! drawn, square, at the size given, never what it looks like.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use mitsuami::core::backend::{CaptureError, Image as Capture};
use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A folder of the test's own: a text file, a picture (blue left half, red
/// right half) and a folder.
struct Files {
    root: PathBuf,
}

impl Files {
    fn new(app: &TestApp) -> Files {
        let root = std::env::temp_dir().join("mitsuami-file-icon").join(app.test_name());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Photos")).unwrap();
        std::fs::write(root.join("notes.txt"), "Notes\n").unwrap();
        let picture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/blue-red-20x10.png");
        std::fs::copy(picture, root.join("picture.png")).unwrap();
        Files { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

fn icon(label: &str) -> Query {
    by_role(Role::Image, label)
}

#[mitsuami_test::test]
async fn shows_the_file_s_icon_in_a_square(app: TestApp) {
    let files = Files::new(&app);
    let notes = files.path("notes.txt");
    app.mount(move || Column::new().align(Align::Start).child(FileIcon::new(&notes).label("Notes")));

    app.expect(icon("Notes")).to_be_visible().await;
    assert_eq!(app.get(icon("Notes")).native_state().kind, WidgetKind::FileIcon);
    assert!(has(&app, icon("Notes"), Prop::File(files.path("notes.txt"))));
    let size = app.get(icon("Notes")).frame().size;
    assert!(!size.is_empty() && size.width == size.height, "{size:?}");
}

/// Unlabelled, it's decorative, as it sits beside the file's name. It
/// never takes focus.
#[mitsuami_test::test]
async fn reads_as_an_image_named_by_its_label(app: TestApp) {
    let files = Files::new(&app);
    let (notes, photos) = (files.path("notes.txt"), files.path("Photos"));
    app.mount(move || {
        Column::new().children((FileIcon::new(&notes).label("Notes"), FileIcon::new(&photos), TextInput::new()))
    });

    app.expect(icon("Notes")).to_exist().await;
    assert!(has(&app, icon("Notes"), Prop::Label("Notes".into())));
    let tree = app.ui().a11y_tree(app.window()).expect("a window");
    let names: Vec<_> = tree.walk().into_iter().filter(|n| n.role == Role::Image).map(|n| n.name.clone()).collect();
    assert_eq!(names, [Some("Notes".to_owned()), None]);
    assert_eq!(app.ui().focus_order(app.window()).len(), 1);
}

/// The platform's small icons by default (16 points everywhere so far),
/// a square of the size given otherwise.
#[mitsuami_test::test]
async fn is_as_large_as_its_size(app: TestApp) {
    let files = Files::new(&app);
    let size = signal(32.0_f32);
    let photos = files.path("Photos");
    app.mount(move || {
        Row::new().align(Align::Start).children((
            FileIcon::new(&photos).label("Small").icon_size(12.0),
            FileIcon::new(&photos).label("Default"),
            FileIcon::new(&photos).label("Large").icon_size(size),
        ))
    });
    app.expect(icon("Large")).to_be_visible().await;

    let frame = |label: &str| app.get(icon(label)).frame().size;
    assert!(frame("Small").height < frame("Default").height, "{:?} < {:?}", frame("Small"), frame("Default"));
    assert!(frame("Default").height < frame("Large").height, "{:?} < {:?}", frame("Default"), frame("Large"));
    assert_eq!(frame("Large"), Size::new(32.0, 32.0));
    assert!(has(&app, icon("Large"), Prop::IconSize(32.0)));

    size.set(48.0);
    app.settle().await;
    assert_eq!(frame("Large"), Size::new(48.0, 48.0));
    assert!(has(&app, icon("Large"), Prop::IconSize(48.0)));
}

#[mitsuami_test::test]
async fn follows_a_new_file(app: TestApp) {
    let files = Files::new(&app);
    let file = signal(files.path("notes.txt"));
    app.mount(move || Column::new().align(Align::Start).child(FileIcon::new(file).label("Icon")));
    app.expect(icon("Icon")).to_be_visible().await;

    file.set(files.path("Photos"));
    app.settle().await;
    assert!(has(&app, icon("Icon"), Prop::File(files.path("Photos"))));
    assert!(!app.get(icon("Icon")).frame().size.is_empty());
}

/// The window's pixels in a frame.
fn pixels_in(image: &Capture, frame: Rect) -> Vec<[u8; 4]> {
    let scale = image.scale_factor;
    let (x0, y0) = ((frame.x() * scale) as usize, (frame.y() * scale) as usize);
    let (x1, y1) = (((frame.x() + frame.width()) * scale) as usize, ((frame.y() + frame.height()) * scale) as usize);
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (y * image.width as usize + x) * 4))
        .map(|i| [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2], image.rgba[i + 3]])
        .collect()
}

async fn capture(app: &TestApp) -> Option<Capture> {
    match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => {
            assert!(app.is_headless(), "only headless may lack capture");
            None
        }
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => Some(image),
    }
}

/// Something is drawn in its square: pixels unlike the window's background
/// around it.
#[mitsuami_test::test]
async fn draws_the_file_s_icon(app: TestApp) {
    let files = Files::new(&app);
    let photos = files.path("Photos");
    app.mount(move || {
        Row::new().align(Align::Start).padding(10).child(FileIcon::new(&photos).label("Photos").icon_size(32.0))
    });
    app.expect(icon("Photos")).to_be_visible().await;

    let Some(image) = capture(&app).await else { return };
    let frame = app.get(icon("Photos")).frame();
    let background = pixels_in(&image, Rect::new(0.0, 0.0, 5.0, 5.0))[0];
    let drawn = pixels_in(&image, frame).into_iter().filter(|p| p != &background).count();
    assert!(drawn > 100, "{drawn} pixels of the icon drawn");
}

/// With a thumbnail, a picture shows what's in it (blue and red), where
/// the platform makes thumbnails: AppKit's QuickLook and the Windows shell,
/// soon after its icon. GTK shows only those the desktop has made, and
/// Kirigami none (Dolphin's are KIO's), so there it's only passed through.
/// Windows shows none where the user has Explorer show icons only, which
/// WinUI follows too.
#[mitsuami_test::test]
async fn a_thumbnail_shows_what_is_in_the_file(app: TestApp) {
    let files = Files::new(&app);
    let picture = files.path("picture.png");
    app.mount(move || {
        Row::new()
            .align(Align::Start)
            .padding(10)
            .child(FileIcon::new(&picture).label("Picture").icon_size(64.0).thumbnail(true))
    });
    app.expect(icon("Picture")).to_be_visible().await;
    assert!(has(&app, icon("Picture"), Prop::Thumbnail(true)));
    if !matches!(app.backend_name(), "appkit" | "winui") || (app.backend_name() == "winui" && icons_only()) {
        return;
    }

    let blue_and_red = |pixels: &[[u8; 4]]| {
        let blue = pixels.iter().filter(|p| p[2] > 200 && p[0] < 80 && p[1] < 80).count();
        let red = pixels.iter().filter(|p| p[0] > 200 && p[1] < 80 && p[2] < 80).count();
        blue > 50 && red > 50
    };
    // Thumbnails are made in the background.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let image = capture(&app).await.expect("a capture");
        if blue_and_red(&pixels_in(&image, app.get(icon("Picture")).frame())) {
            return;
        }
        assert!(Instant::now() < deadline, "no thumbnail of the picture");
        std::thread::sleep(Duration::from_millis(50));
        app.settle().await;
    }
}

/// Whether Explorer shows icons, never thumbnails (Folder Options, or
/// "Show thumbnails instead of icons" off in Performance Options), as a
/// Windows Server may have it.
fn icons_only() -> bool {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
    let Ok(out) = std::process::Command::new("reg").args(["query", key, "/v", "IconsOnly"]).output() else {
        return false;
    };
    // `    IconsOnly    REG_DWORD    0x1`
    let out = String::from_utf8_lossy(&out.stdout);
    out.lines().any(|l| l.trim_start().starts_with("IconsOnly") && l.split_whitespace().last() != Some("0x0"))
}

mitsuami_test::main!();
