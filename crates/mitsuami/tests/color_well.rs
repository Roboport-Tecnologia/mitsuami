//! `ColorWell`: an opaque colour, reported when the user picks one and not
//! when the app sets it. The platform's picker never opens in tests: a pick
//! is set as assistive technology sets one.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn props(app: &TestApp) -> Vec<Prop> {
    app.get_by_role(Role::ColorWell, "Colour").native_state().props
}

#[mitsuami_test::test]
async fn picking_reports_and_setting_doesnt(app: TestApp) {
    let color = signal(Color::rgb(0, 120, 215));
    let picks = Rc::new(RefCell::new(Vec::new()));
    let log = picks.clone();
    app.mount(move || ColorWell::new("Colour").color(color).on_change(move |c| log.borrow_mut().push(c)));
    assert!(props(&app).contains(&Prop::ColorValue(Color::rgb(0, 120, 215))));

    color.set(Color::rgb(16, 124, 16));
    app.settle().await;
    assert!(props(&app).contains(&Prop::ColorValue(Color::rgb(16, 124, 16))));
    assert!(picks.borrow().is_empty(), "{:?}", picks.borrow());

    app.get_by_role(Role::ColorWell, "Colour").set_color(Color::rgb(255, 140, 0)).await;
    assert_eq!(*picks.borrow(), [Color::rgb(255, 140, 0)]);
    assert!(props(&app).contains(&Prop::ColorValue(Color::rgb(255, 140, 0))));
}

/// Read out as `#rrggbb`, as the colour it shows; alpha isn't a well's.
#[mitsuami_test::test]
async fn it_reads_as_its_colour_without_alpha(app: TestApp) {
    let color = signal(Color::Rgba(255, 67, 67, 128));
    app.mount(move || ColorWell::new("Colour").bind(color));
    app.expect(by_role(Role::ColorWell, "Colour")).to_have_value("#ff4343").await;

    app.get_by_role(Role::ColorWell, "Colour").set_color(Color::rgb(0, 153, 188)).await;
    assert_eq!(color.get(), Color::rgb(0, 153, 188));
    app.expect(by_role(Role::ColorWell, "Colour")).to_have_value("#0099bc").await;
}

#[mitsuami_test::test]
async fn a_disabled_well_reads_as_disabled(app: TestApp) {
    app.mount(|| ColorWell::new("Colour").color(Color::rgb(0, 0, 0)).enabled(false));
    app.expect(by_role(Role::ColorWell, "Colour")).to_be_disabled().await;
    assert!(props(&app).contains(&Prop::Enabled(false)));
}

/// AppKit's colour panel is shared: an active well that goes (its sheet
/// closed) takes the panel with it, as the other platforms' pickers go with
/// their window. It stayed open, over nothing.
#[cfg(target_os = "macos")]
#[mitsuami_test::test]
async fn the_colour_panel_closes_with_its_well(app: TestApp) {
    use mitsuami::appkit::objc2::rc::Retained;
    use mitsuami::appkit::objc2::{MainThreadMarker, Message};
    use mitsuami::appkit::objc2_app_kit::{NSColorPanel, NSColorWell};

    if app.is_headless() {
        return;
    }
    let shown = signal(true);
    let well: Rc<RefCell<Option<Retained<NSColorWell>>>> = Rc::default();
    let kept = well.clone();
    app.mount(move || {
        let kept = kept.clone();
        Column::new().child(Show::new(shown, move || {
            let kept = kept.clone();
            let tweak = mitsuami::appkit::tweak(move |w: &NSColorWell| *kept.borrow_mut() = Some(w.retain()));
            ColorWell::new("Colour").color(Color::rgb(0, 120, 215)).native(tweak)
        }))
    });
    let mtm = MainThreadMarker::new().unwrap();
    let well = well.borrow().clone().expect("the native well");
    // As a click on it does.
    well.activate(true);
    assert!(well.isActive());
    assert!(NSColorPanel::sharedColorPanel(mtm).isVisible());

    shown.set(false);
    app.settle().await;
    assert!(!well.isActive());
    assert!(!NSColorPanel::sharedColorPanel(mtm).isVisible(), "the panel closed with its well");
}

mitsuami_test::main!();
