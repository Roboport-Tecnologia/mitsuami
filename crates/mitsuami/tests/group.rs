//! `Group`: a box around related content, under an optional heading, as
//! each platform draws one (a box with its title inside on AppKit and
//! Qt, a card under a heading on GNOME and Windows). The core lays out
//! its content inside the platform's insets, which differ per platform:
//! tests read them from the metrics, and compare the rest.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn group(title: &str) -> Query {
    by_role(Role::Group, title)
}

/// Where a child is in its group, in the group's coordinates.
fn offset(app: &TestApp, group: Query, child: Query) -> Point {
    let (g, c) = (app.get(group).frame(), app.get(child).frame());
    Point::new(c.x() - g.x(), c.y() - g.y())
}

#[mitsuami_test::test]
async fn lays_out_its_content_inside_its_insets(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).padding(10).child(
            Group::new().title("Drive").gap(6).children((Text::new("Total Annihilation"), Text::new("ISO image"))),
        )
    });
    app.expect(by_text("ISO image")).to_be_visible().await;

    let insets = app.ui().metrics().titled_group_insets;
    let first = offset(&app, group("Drive"), by_text("Total Annihilation"));
    assert_eq!((first.x, first.y), (insets.left, insets.top));
    // A column's children, `gap` apart.
    let second = offset(&app, group("Drive"), by_text("ISO image"));
    let height = app.get(by_text("Total Annihilation")).frame().height();
    assert_eq!(second.y, first.y + height + 6.0);
    // As big as its content, and its insets.
    let (g, text) = (app.get(group("Drive")).frame(), app.get(by_text("Total Annihilation")).frame());
    assert!(g.width() >= text.width() + insets.left + insets.right, "{g:?}");
    assert_eq!(app.get(group("Drive")).native_state().kind, WidgetKind::Group);
}

/// A heading takes room: inside the box at its top (AppKit, Qt), or above
/// a card (GNOME, Windows). Without one, the content moves up.
#[mitsuami_test::test]
async fn a_heading_takes_room_at_the_top(app: TestApp) {
    let title = signal("Drive".to_owned());
    app.mount(move || {
        Column::new().align(Align::Start).child(Group::new().title(title).a11y_label("Box").child(Text::new("Disc")))
    });
    app.expect(by_text("Disc")).to_be_visible().await;
    let metrics = app.ui().metrics();
    assert!(metrics.titled_group_insets.top > metrics.group_insets.top, "{metrics:?}");
    assert_eq!(offset(&app, group("Box"), by_text("Disc")).y, metrics.titled_group_insets.top);

    title.set(String::new());
    app.settle().await;
    assert_eq!(offset(&app, group("Box"), by_text("Disc")).y, metrics.group_insets.top);
    assert!(app.get(group("Box")).native_state().props.contains(&Prop::Title(String::new())));

    title.set("Library".to_owned());
    app.settle().await;
    assert_eq!(offset(&app, group("Box"), by_text("Disc")).y, metrics.titled_group_insets.top);
    assert!(app.get(group("Box")).native_state().props.contains(&Prop::Title("Library".into())));
}

/// Its content is smaller than its heading: the group is still as wide as
/// the heading needs.
#[mitsuami_test::test]
async fn is_at_least_as_wide_as_its_heading(app: TestApp) {
    let heading = "A heading much longer than what's in the group";
    app.mount(move || {
        Column::new().align(Align::Start).gap(8).children((
            Group::new().title(heading).child(Text::new("x")),
            Group::new().a11y_label("Untitled").child(Text::new("y")),
        ))
    });
    app.expect(by_text("x")).to_be_visible().await;

    let titled = app.get(group(heading)).frame().width();
    let untitled = app.get(group("Untitled")).frame().width();
    assert!(titled > untitled * 2.0, "{titled} > 2 × {untitled}");
}

/// The app's padding adds to the platform's insets.
#[mitsuami_test::test]
async fn padding_adds_to_its_insets(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).child(Group::new().a11y_label("Box").padding(10).child(Text::new("Disc")))
    });
    app.expect(by_text("Disc")).to_be_visible().await;

    let insets = app.ui().metrics().group_insets;
    let at = offset(&app, group("Box"), by_text("Disc"));
    assert_eq!((at.x, at.y), (insets.left + 10.0, insets.top + 10.0));
}

/// A group to assistive technology, named by its heading, around its
/// content; it takes no focus, its content does.
#[mitsuami_test::test]
async fn reads_as_a_group_named_by_its_heading(app: TestApp) {
    app.mount(|| {
        Column::new().child(Group::new().title("CD drive").children((Text::new("Disc"), Button::new("Eject"))))
    });
    app.expect(group("CD drive")).to_exist().await;

    let tree = app.ui().a11y_tree(app.window()).expect("a window");
    let drive = tree.walk().into_iter().find(|n| n.role == Role::Group).expect("a group").clone();
    assert_eq!(drive.name.as_deref(), Some("CD drive"));
    let inside: Vec<_> = drive.walk().into_iter().skip(1).map(|n| (n.role, n.name.clone())).collect();
    assert_eq!(inside, [(Role::StaticText, Some("Disc".into())), (Role::Button, Some("Eject".into()))]);
    assert_eq!(app.ui().focus_order(app.window()).len(), 1);
    app.get_by_role(Role::Button, "Eject").click().await;
}

/// Counts the runs of the tweak, which gets the platform's box: the
/// `NSBox`, GTK's card, the `QQC2.GroupBox`, WinUI's card `Border`. A
/// tweak typed for another view never runs.
fn count_runs(log: Rc<RefCell<u32>>) -> Tweak<Group> {
    platform! {
        macos => mitsuami::appkit::tweak(move |_: &mitsuami::appkit::objc2_app_kit::NSBox| *log.borrow_mut() += 1),
        gtk => mitsuami::gtk::tweak(move |_: &mitsuami::gtk::gtk::Box| *log.borrow_mut() += 1),
        kde => mitsuami::kirigami::tweak(move |_: &mitsuami::kirigami::QmlObject| *log.borrow_mut() += 1),
        windows => mitsuami::winui::tweak(move |_: &mitsuami::winui::bindings::Border| {
            *log.borrow_mut() += 1;
            Ok(())
        }),
    }
}

/// It runs after the group's props, and again when they change: a new
/// heading puts AppKit's title back at the top, and the tweak moves it
/// again.
#[mitsuami_test::test]
async fn a_tweak_gets_the_box_after_its_props(app: TestApp) {
    let runs = Rc::new(RefCell::new(0));
    let title = signal("Drive".to_owned());
    let tweak = count_runs(runs.clone());
    app.mount(move || Column::new().child(Group::new().title(title).native(tweak).child(Text::new("Disc"))));
    app.expect(group("Drive")).to_exist().await;

    assert!(app.get(group("Drive")).native_state().props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert_eq!(*runs.borrow(), 0);
        return;
    }
    let before = *runs.borrow();
    assert!(before > 0, "the tweak never ran on the box");
    title.set("CD drive".to_owned());
    app.settle().await;
    assert!(*runs.borrow() > before);
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    app.mount(|| view! { <Group title="CD drive" gap=Spacing::Sm><Text>"Disc"</Text></Group> });

    app.expect(group("CD drive")).to_exist().await;
    app.expect(by_text("Disc")).to_be_visible().await;
}

mitsuami_test::main!();
