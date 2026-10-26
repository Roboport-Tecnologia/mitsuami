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

mitsuami_test::main!();
