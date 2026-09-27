//! `Image`: a picture from a file or from pixels in memory, in the
//! platform's image view. It's as large as the image in points unless the
//! layout sizes it, follows a reactive source, draws what it's given, and
//! reads as an image named by its label.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami::core::backend::CaptureError;
use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// 20 × 10: the left half blue, the right half red.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/blue-red-20x10.png")
}

/// One colour all over.
fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Pixels {
    Pixels::new(width, height, rgba.repeat((width * height) as usize))
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn picture() -> Query {
    by_role(Role::Image, "Picture")
}

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

#[mitsuami_test::test]
async fn shows_pixels_at_their_size(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).child(Image::pixels(solid(40, 30, RED)).label("Picture")));

    assert_eq!(app.get(picture()).native_state().kind, WidgetKind::Image);
    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 40.0, 30.0)).await;
}

/// Pixels made at a scale of 2 take half as many points.
#[mitsuami_test::test]
async fn shows_scaled_pixels_at_their_scale(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).child(Image::pixels(solid(80, 60, RED).scale(2.0)).label("Picture"))
    });

    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 40.0, 30.0)).await;
}

/// Decoding may finish after the image is shown (WinUI decodes files in the
/// background); it's measured again then.
#[mitsuami_test::test]
async fn reads_a_file(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).child(Image::file(fixture()).label("Picture")));

    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 20.0, 10.0)).await;
    assert!(has(&app, picture(), Prop::Image(ImageSource::File(fixture()))));
}

#[mitsuami_test::test]
async fn shows_nothing_for_a_missing_file(app: TestApp) {
    app.mount(|| {
        Column::new()
            .align(Align::Start)
            .children((Image::file("/nowhere/at/all.png").label("Picture"), Text::new("After")))
    });
    app.settle().await;

    assert!(app.get(picture()).frame().size.is_empty());
    app.expect(by_text("After")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn follows_a_new_source(app: TestApp) {
    let source = signal(ImageSource::Pixels(solid(40, 30, RED)));
    app.mount(move || Column::new().align(Align::Start).child(Image::new(source).label("Picture")));
    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 40.0, 30.0)).await;

    source.set(ImageSource::Pixels(solid(16, 12, BLUE)));
    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 16.0, 12.0)).await;
    assert!(has(&app, picture(), Prop::Image(ImageSource::Pixels(solid(16, 12, BLUE)))));

    source.set(ImageSource::File(fixture()));
    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 20.0, 10.0)).await;
}

/// Unlabelled, it's decorative: still an image, but with no name to read.
#[mitsuami_test::test]
async fn reads_as_an_image_named_by_its_label(app: TestApp) {
    app.mount(|| Column::new().children((Image::pixels(solid(8, 8, RED)).label("Picture"), Image::file(fixture()))));

    app.expect(picture()).to_exist().await;
    assert!(has(&app, picture(), Prop::Label("Picture".into())));
    let tree = app.ui().a11y_tree(app.window()).expect("a window");
    let names: Vec<_> = tree.walk().into_iter().filter(|n| n.role == Role::Image).map(|n| n.name.clone()).collect();
    assert_eq!(names, [Some("Picture".to_owned()), None]);
}

/// Sized by the layout, it fills the frame as asked; how it looks inside
/// is the platform's drawing, checked by the story.
#[mitsuami_test::test]
async fn fills_a_frame_as_asked(app: TestApp) {
    let fit = signal(ImageFit::Stretch);
    app.mount(move || {
        Column::new()
            .align(Align::Start)
            .child(Image::pixels(solid(40, 30, RED)).label("Picture").fit(fit).width(100).height(50))
    });

    app.expect(picture()).to_have_frame(Rect::new(0.0, 0.0, 100.0, 50.0)).await;
    assert!(has(&app, picture(), Prop::ImageFit(ImageFit::Stretch)));
    fit.set(ImageFit::Contain);
    app.settle().await;
    assert!(has(&app, picture(), Prop::ImageFit(ImageFit::Contain)));
}

/// What the window shows under the image is its pixels: blue on the left,
/// red on the right, the right way round.
#[mitsuami_test::test]
async fn draws_what_it_is_given(app: TestApp) {
    app.mount(|| {
        Column::new()
            .align(Align::Start)
            .padding(10)
            .children((Image::file(fixture()).label("Picture"), Image::pixels(solid(20, 10, RED)).label("Pixels")))
    });
    app.expect(picture()).to_have_frame(Rect::new(10.0, 10.0, 20.0, 10.0)).await;

    let image = match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => return assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => image,
    };
    // Colour management may shift the colours a little.
    let at = |x: f32, y: f32| {
        let (x, y) = ((x * image.scale_factor) as usize, (y * image.scale_factor) as usize);
        let i = (y * image.width as usize + x) * 4;
        [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2]]
    };
    let red = |[r, g, b]: [u8; 3]| r > 200 && g < 80 && b < 80;
    let blue = |[r, g, b]: [u8; 3]| b > 200 && r < 80 && g < 80;
    assert!(blue(at(15.0, 15.0)), "the file's left half: {:?}", at(15.0, 15.0));
    assert!(red(at(25.0, 15.0)), "the file's right half: {:?}", at(25.0, 15.0));
    let pixels = app.get(by_role(Role::Image, "Pixels")).frame();
    let middle = (pixels.x() + pixels.width() / 2.0, pixels.y() + pixels.height() / 2.0);
    assert!(red(at(middle.0, middle.1)), "the pixels: {:?}", at(middle.0, middle.1));
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let source = ImageSource::Pixels(solid(12, 12, RED));
    app.mount(move || view! { <Image source=source.clone() label="Picture"/> });

    app.expect(picture()).to_exist().await;
}

/// Counts the runs of the tweak, which gets the native image view.
fn count_runs(log: Rc<RefCell<u32>>) -> Tweak<Image> {
    platform! {
        macos => mitsuami::appkit::tweak(move |_: &mitsuami::appkit::objc2_app_kit::NSImageView| *log.borrow_mut() += 1),
        gtk => mitsuami::gtk::tweak(move |_: &mitsuami::gtk::gtk::Picture| *log.borrow_mut() += 1),
        kde => mitsuami::kirigami::tweak(move |_: &mitsuami::kirigami::QmlObject| *log.borrow_mut() += 1),
        windows => mitsuami::winui::tweak(move |_: &mitsuami::winui::bindings::Image| {
            *log.borrow_mut() += 1;
            Ok(())
        }),
    }
}

#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_image_view_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(0));
    let source = signal(ImageSource::Pixels(solid(8, 8, RED)));
    let tweak = count_runs(log.clone());
    app.mount(move || Image::new(source).native(tweak).label("Picture"));

    assert!(app.get(picture()).native_state().props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert_eq!(*log.borrow(), 0);
        return;
    }
    let before = *log.borrow();
    assert!(before > 0);
    source.set(ImageSource::Pixels(solid(8, 8, BLUE)));
    app.settle().await;
    assert!(*log.borrow() > before);
}

mitsuami_test::main!();
