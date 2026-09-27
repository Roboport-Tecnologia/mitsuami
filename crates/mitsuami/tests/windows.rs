//! `Window`: windows the app opens while it runs. Each is shown while its
//! flag is true, builds its content when it opens and disposes it when it
//! closes, closes with the scope that declared it, and leaves the close
//! button to the app.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn machine(app: &TestApp) -> Option<NodeId> {
    app.window_titled("Machine")
}

#[mitsuami_test::test]
async fn opens_and_closes_with_its_flag(app: TestApp) {
    let open = signal(false);
    app.mount(move || {
        Column::new().children((
            Text::new("Launcher"),
            Window::new("Machine").open(open).content(|| Text::new("Machine settings")),
        ))
    });
    assert_eq!(machine(&app), None);
    app.expect(by_text("Machine settings")).not_to_exist().await;

    open.set(true);
    app.settle().await;
    let window = machine(&app).expect("the window opened");
    assert_ne!(window, app.window());
    app.expect(by_text("Machine settings")).to_be_visible().await;
    assert_eq!(app.ui().window_of(app.get(by_text("Machine settings")).id()), Some(window));

    open.set(false);
    app.settle().await;
    assert_eq!(machine(&app), None);
    app.expect(by_text("Machine settings")).not_to_exist().await;
    app.expect(by_text("Launcher")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn the_close_button_clears_a_bound_flag(app: TestApp) {
    let open = signal(true);
    app.mount(move || Window::new("Machine").bind(open).content(|| Text::new("Machine settings")));

    app.close_window(machine(&app).expect("open")).await;
    assert!(!open.get_untracked());
    assert_eq!(machine(&app), None);

    open.set(true);
    app.settle().await;
    assert!(machine(&app).is_some());
}

/// The app decides: here it keeps the window open, as it would to ask about
/// unsaved changes first.
#[mitsuami_test::test]
async fn the_app_decides_whether_it_closes(app: TestApp) {
    let asked = Rc::new(Cell::new(0));
    let count = asked.clone();
    app.mount(move || {
        let count = count.clone();
        Window::new("Machine").on_close_request(move || count.set(count.get() + 1)).content(|| Text::new("Unsaved"))
    });

    app.close_window(machine(&app).expect("open")).await;
    assert_eq!(asked.get(), 1);
    assert!(machine(&app).is_some());
    app.expect(by_text("Unsaved")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn without_a_handler_the_close_button_does_nothing(app: TestApp) {
    app.mount(|| Window::new("Machine").content(|| Text::new("Stays")));

    app.close_window(machine(&app).expect("open")).await;
    assert!(machine(&app).is_some());
}

/// Declared in a part of the tree that goes away, it goes with it.
#[mitsuami_test::test]
async fn closes_with_the_scope_that_declared_it(app: TestApp) {
    let shown = signal(true);
    app.mount(move || {
        Show::new(shown, || {
            Column::new().children((Text::new("Part"), Window::new("Machine").content(|| Text::new("Inside"))))
        })
    });
    assert!(machine(&app).is_some());

    shown.set(false);
    app.settle().await;
    assert_eq!(machine(&app), None);
    app.expect(by_text("Inside")).not_to_exist().await;
}

#[mitsuami_test::test]
async fn its_title_follows_the_app(app: TestApp) {
    let title = signal("Machine".to_owned());
    app.mount(move || Window::new(title).content(|| Text::new("Inside")));
    let window = machine(&app).expect("open");

    title.set("Windows 98".into());
    app.settle().await;
    assert_eq!(app.window_titled("Windows 98"), Some(window));
    let native = app.ui().native_state(window).expect("a native window");
    assert!(native.props.contains(&Prop::Title("Windows 98".into())));
}

/// Each opening builds the content afresh: what was typed last time is gone.
#[mitsuami_test::test]
async fn reopening_builds_fresh_content(app: TestApp) {
    let open = signal(true);
    app.mount(move || {
        Window::new("Machine").bind(open).content(|| {
            let name = signal(String::new());
            Column::new().children((
                TextInput::new().a11y_label("Name").bind(name),
                Text::new(move || format!("Name: {}", name.get())),
            ))
        })
    });

    app.get_by_label("Name").fill("Win98").await;
    app.expect(by_text("Name: Win98")).to_exist().await;
    open.set(false);
    app.settle().await;
    open.set(true);
    app.settle().await;
    app.expect(by_text("Name: ")).to_exist().await;
}

#[mitsuami_test::test]
async fn fits_its_height_to_its_content(app: TestApp) {
    app.mount(|| {
        Window::new("Machine")
            .size(WindowSize::FitHeight(360.0))
            .content(|| Column::new().padding(20).child(Row::new().height(100)))
    });
    let size = app.ui().window_size(machine(&app).expect("open")).expect("sized");

    assert_eq!(size.width, 360.0);
    // Up to a pixel of slack: platforms size windows in physical pixels.
    assert!((140.0..141.0).contains(&size.height), "{size:?}");
}

/// Its controls work, and share the app's state with the other windows.
#[mitsuami_test::test]
async fn shares_state_with_the_other_windows(app: TestApp) {
    let count = signal(0);
    app.mount(move || {
        Column::new().children((
            Text::new(move || format!("Clicked {} times", count.get())),
            Window::new("Machine").content(move || Button::new("Click").on_click(move || count.update(|c| *c += 1))),
        ))
    });

    app.get_by_role(Role::Button, "Click").click().await;
    app.get_by_role(Role::Button, "Click").click().await;
    app.expect(by_text("Clicked 2 times")).to_exist().await;
}

/// The modal prop, as the native window carries it.
fn modal(app: &TestApp, window: NodeId) -> Option<Prop> {
    let native = app.ui().native_state(window).expect("a native window");
    native.props.into_iter().find(|p| matches!(p, Prop::Modal { .. }))
}

/// A modal window belongs to the window it's declared in; a plain one
/// belongs to none. How it's shown (a sheet, a modal loop, a modal dialog)
/// is the platform's, and needs a window on screen, which tests don't
/// have.
#[mitsuami_test::test]
async fn a_modal_window_belongs_to_the_window_it_is_declared_in(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Window::new("Sheet").modal(Modality::Window).content(|| Text::new("In the sheet")),
            Window::new("Dialog").modal(Modality::Application).content(|| {
                // Declared inside the dialog: it belongs to the dialog.
                Window::new("Nested").modal(Modality::Window).content(|| Text::new("Deeper"))
            }),
            Window::new("Plain").content(|| Text::new("Free")),
        ))
    });
    let (sheet, dialog, nested, plain) = (
        app.window_titled("Sheet").expect("open"),
        app.window_titled("Dialog").expect("open"),
        app.window_titled("Nested").expect("open"),
        app.window_titled("Plain").expect("open"),
    );

    assert_eq!(modal(&app, sheet), Some(Prop::Modal { owner: Some(app.window()), modality: Modality::Window }));
    assert_eq!(modal(&app, dialog), Some(Prop::Modal { owner: Some(app.window()), modality: Modality::Application }));
    assert_eq!(modal(&app, nested), Some(Prop::Modal { owner: Some(dialog), modality: Modality::Window }));
    assert_eq!(modal(&app, plain), None);
}

/// A modal window that closes gives the app back.
#[mitsuami_test::test]
async fn a_modal_window_closes_like_any_other(app: TestApp) {
    let open = signal(true);
    let count = signal(0);
    app.mount(move || {
        Column::new().children((
            Button::new("Count").on_click(move || count.update(|c| *c += 1)),
            Window::new("Sheet")
                .modal(Modality::Window)
                .bind(open)
                .content(move || Button::new("Done").role(ButtonRole::Cancel).on_click(move || open.set(false))),
        ))
    });

    app.get_by_role(Role::Button, "Done").click().await;
    assert_eq!(app.window_titled("Sheet"), None);
    app.get_by_role(Role::Button, "Count").click().await;
    assert_eq!(count.get_untracked(), 1);
}

/// Escape asks a dialog (a modal window) to close, as the close button
/// does, whichever modality; a plain window ignores it, as on every
/// platform. The app-modal dialog goes first: while it shows, it blocks
/// the other windows (Qt doesn't give them keys).
#[mitsuami_test::test]
async fn escape_asks_a_modal_window_to_close(app: TestApp) {
    let (sheet, dialog, plain) = (signal(true), signal(true), signal(true));
    app.mount(move || {
        let field = |name: &'static str| move || TextInput::new().a11y_label(name);
        Column::new().children((
            Window::new("Sheet").modal(Modality::Window).bind(sheet).content(field("In the sheet")),
            Window::new("Dialog").modal(Modality::Application).bind(dialog).content(field("In the dialog")),
            Window::new("Plain").bind(plain).content(field("In the window")),
        ))
    });

    app.get_by_label("In the dialog").press(Key::Escape).await;
    assert!(!dialog.get_untracked());
    app.get_by_label("In the sheet").press(Key::Escape).await;
    assert!(!sheet.get_untracked());
    app.get_by_label("In the window").press(Key::Escape).await;
    assert!(plain.get_untracked());
}

/// AppKit gives Escape to a Cancel button first (its key equivalent); the
/// other platforms' Cancel buttons are ordinary, so the window takes it.
#[mitsuami_test::test]
async fn escape_presses_the_cancel_button_on_appkit(app: TestApp) {
    let by = Rc::new(Cell::new(""));
    let (button, request) = (by.clone(), by.clone());
    app.mount(move || {
        let (button, request) = (button.clone(), request.clone());
        Window::new("Sheet").modal(Modality::Window).on_close_request(move || request.set("request")).content(
            move || {
                let button = button.clone();
                Column::new().children((
                    TextInput::new().a11y_label("Name"),
                    Button::new("Cancel").role(ButtonRole::Cancel).on_click(move || button.set("button")),
                ))
            },
        )
    });

    app.get_by_label("Name").press(Key::Escape).await;
    assert_eq!(by.get(), if app.backend_name() == "appkit" { "button" } else { "request" });
}

/// In `view!`, the title is an attribute, wherever it's written, and the
/// children are the content, built at each opening.
#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let open = signal(true);
    let memory = signal(64);
    app.mount(move || {
        view! {
            <Column>
                <Text>"Launcher"</Text>
                <Window bind=open modal=Modality::Window title="Machine">
                    <Text>{move || format!("{} MB", memory.get())}</Text>
                </Window>
            </Column>
        }
    });
    let window = machine(&app).expect("the window opened");
    assert_eq!(modal(&app, window), Some(Prop::Modal { owner: Some(app.window()), modality: Modality::Window }));
    app.expect(by_text("64 MB")).to_be_visible().await;

    app.close_window(window).await;
    assert!(!open.get_untracked());
    app.expect(by_text("64 MB")).not_to_exist().await;

    memory.set(128);
    open.set(true);
    app.settle().await;
    app.expect(by_text("128 MB")).to_be_visible().await;
}

/// A dialog nested in a window, as apps nest them: it edits a draft of the
/// window's form, started at each opening, which OK applies and Cancel or
/// the close button drops.
#[mitsuami_test::test]
async fn a_nested_dialog_applies_or_drops_its_changes(app: TestApp) {
    let open = signal(true);
    let form = signal(2);
    let dialog = signal(false);
    app.mount(move || {
        let draft = signal(0);
        view! {
            <Window title="Machine" bind=open>
                <Column>
                    <Text>{move || format!("{} CPUs", form.get())}</Text>
                    <Button @click=move || dialog.set(true)>"Advanced…"</Button>
                    <Window title="Advanced" modal=Modality::Window bind=dialog @open=move || draft.set(form.get_untracked())>
                        <Column>
                            <NumberInput label="Processors" bind=draft/>
                            <Button role=ButtonRole::Cancel @click=move || dialog.set(false)>"Cancel"</Button>
                            <Button role=ButtonRole::Default @click=move || {
                                form.set(draft.get_untracked());
                                dialog.set(false);
                            }>"OK"</Button>
                        </Column>
                    </Window>
                </Column>
            </Window>
        }
    });
    let owner = machine(&app).expect("open");
    let processors = || by_role(Role::SpinButton, "Processors");

    app.get_by_role(Role::Button, "Advanced…").click().await;
    let advanced = app.window_titled("Advanced").expect("the dialog opened");
    assert_eq!(modal(&app, advanced), Some(Prop::Modal { owner: Some(owner), modality: Modality::Window }));
    app.expect(processors()).to_have_value("2").await;
    app.get(processors()).set_number(4.0).await;
    app.get_by_role(Role::Button, "Cancel").click().await;
    assert_eq!(app.window_titled("Advanced"), None);
    app.expect(by_text("2 CPUs")).to_be_visible().await;

    // Each opening starts from the form, not from the dropped draft.
    app.get_by_role(Role::Button, "Advanced…").click().await;
    app.expect(processors()).to_have_value("2").await;
    app.get(processors()).set_number(4.0).await;
    app.get_by_role(Role::Button, "OK").click().await;
    app.expect(by_text("4 CPUs")).to_be_visible().await;

    app.get_by_role(Role::Button, "Advanced…").click().await;
    app.close_window(app.window_titled("Advanced").expect("open")).await;
    assert_eq!(app.window_titled("Advanced"), None);
    assert!(!dialog.get_untracked());

    // It closes with the window it's nested in.
    app.get_by_role(Role::Button, "Advanced…").click().await;
    open.set(false);
    app.settle().await;
    assert_eq!(app.window_titled("Advanced"), None);
    assert_eq!(machine(&app), None);
}

mitsuami_test::main!();
