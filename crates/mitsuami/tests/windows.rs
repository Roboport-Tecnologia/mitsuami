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

/// Its size is read as it opens: a change applies at the next opening.
#[mitsuami_test::test]
async fn its_size_applies_at_the_next_opening(app: TestApp) {
    let open = signal(true);
    let wide = signal(false);
    app.mount(move || {
        Window::new("Machine")
            .size(move || WindowSize::Fixed(Size::new(if wide.get() { 400.0 } else { 300.0 }, 200.0)))
            .open(open)
            .content(|| Text::new("Screen"))
    });
    assert_eq!(app.ui().window_size(machine(&app).expect("open")).expect("sized").width, 300.0);

    wide.set(true);
    app.settle().await;
    assert_eq!(app.ui().window_size(machine(&app).expect("still open")).expect("sized").width, 300.0);

    open.set(false);
    app.settle().await;
    open.set(true);
    app.settle().await;
    assert_eq!(app.ui().window_size(machine(&app).expect("open again")).expect("sized").width, 400.0);
}

/// Up to a pixel of slack: platforms size windows in physical pixels.
fn height_of(app: &TestApp, window: NodeId) -> f32 {
    app.ui().window_size(window).expect("sized").height
}

fn about(height: f32, expected: f32) -> bool {
    (expected..expected + 1.0).contains(&height)
}

/// 20 points of padding around a 100-point row, and another while `more`.
fn growing(more: Signal<bool>) -> impl View {
    Column::new().padding(20).children((Row::new().height(100), Show::new(more, || Row::new().height(100))))
}

/// A `FollowHeight` window grows and shrinks with its content, and the
/// user resizes only its width.
#[mitsuami_test::test]
async fn follows_its_content_height(app: TestApp) {
    let more = signal(false);
    app.mount(move || Window::new("Machine").size(WindowSize::FollowHeight(360.0)).content(move || growing(more)));
    let window = machine(&app).expect("open");
    assert!(native_props(&app, window).contains(&Prop::HeightFollowsContent(true)));
    assert!(about(height_of(&app, window), 140.0), "{:?}", app.ui().window_size(window));

    more.set(true);
    app.settle().await;
    assert!(about(height_of(&app, window), 240.0), "grew: {:?}", app.ui().window_size(window));

    more.set(false);
    app.settle().await;
    assert!(about(height_of(&app, window), 140.0), "shrank: {:?}", app.ui().window_size(window));

    // GTK can't hold only the height, so there the window isn't
    // resizable at all.
    app.resize_window(window, Size::new(480.0, 400.0)).await;
    let size = app.ui().window_size(window).expect("sized");
    assert!(about(size.height, 140.0), "the user keeps its height: {size:?}");
    if app.backend_name() != "gtk" {
        assert_eq!(size.width, 480.0);
    }
}

/// The app sets its width; the content keeps setting its height.
#[mitsuami_test::test]
async fn the_app_sets_the_width_of_a_window_that_follows(app: TestApp) {
    let more = signal(false);
    app.mount(move || Window::new("Machine").size(WindowSize::FollowHeight(360.0)).content(move || growing(more)));
    let window = machine(&app).expect("open");

    app.ui().set_window_size(window, Size::new(500.0, 600.0));
    app.settle().await;
    let size = app.ui().window_size(window).expect("sized");
    assert_eq!(size.width, 500.0);
    assert!(about(size.height, 140.0), "{size:?}");

    more.set(true);
    app.settle().await;
    assert!(about(height_of(&app, window), 240.0), "still follows: {:?}", app.ui().window_size(window));
}

/// No shorter than its minimum: the content is laid out at the height the
/// window has, and it follows again once the content is taller.
#[mitsuami_test::test]
async fn a_window_that_follows_keeps_its_minimum(app: TestApp) {
    let more = signal(false);
    app.mount(move || {
        Window::new("Machine")
            .size(WindowSize::FollowHeight(360.0))
            .min_size(Size::new(200.0, 200.0))
            .content(move || growing(more))
    });
    let window = machine(&app).expect("open");
    app.settle().await;
    assert!(about(height_of(&app, window), 200.0), "{:?}", app.ui().window_size(window));

    more.set(true);
    app.settle().await;
    assert!(about(height_of(&app, window), 240.0), "{:?}", app.ui().window_size(window));

    more.set(false);
    app.settle().await;
    assert!(about(height_of(&app, window), 200.0), "{:?}", app.ui().window_size(window));
}

/// `FollowHeightUntilResized` follows until the user changes its height,
/// and keeps following while they change only its width.
#[mitsuami_test::test]
async fn follows_its_content_height_until_the_user_resizes_it(app: TestApp) {
    let more = signal(false);
    app.mount(move || {
        Window::new("Machine").size(WindowSize::FollowHeightUntilResized(360.0)).content(move || growing(more))
    });
    let window = machine(&app).expect("open");
    assert!(!native_props(&app, window).contains(&Prop::HeightFollowsContent(true)));
    let height = height_of(&app, window);
    assert!(about(height, 140.0), "{:?}", app.ui().window_size(window));

    app.resize_window(window, Size::new(480.0, height)).await;
    more.set(true);
    app.settle().await;
    assert!(about(height_of(&app, window), 240.0), "still follows: {:?}", app.ui().window_size(window));

    app.resize_window(window, Size::new(480.0, 400.0)).await;
    more.set(false);
    app.settle().await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(480.0, 400.0)), "the user's size stays");
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

fn native_props(app: &TestApp, window: NodeId) -> Vec<Prop> {
    app.ui().native_state(window).expect("a native window").props
}

/// Full screen follows the app, and the window comes back to its size.
/// A window shows it the platform's way: headless fills its screen;
/// AppKit waits for the window to be shown, which tests don't do, and
/// the mirror check compares what it will show.
#[mitsuami_test::test]
async fn full_screen_follows_the_app(app: TestApp) {
    let full = signal(false);
    app.mount(move || {
        Window::new("Machine").size(Size::new(400.0, 300.0)).full_screen(full).content(|| Text::new("Screen"))
    });
    let window = machine(&app).expect("open");
    let windowed = app.ui().window_size(window).expect("sized");

    full.set(true);
    app.settle().await;
    assert!(native_props(&app, window).contains(&Prop::FullScreen(true)));
    if app.is_headless() {
        assert_eq!(app.ui().window_size(window), Some(app.headless().screen()));
    }
    assert!(full.get_untracked());

    full.set(false);
    app.settle().await;
    assert!(native_props(&app, window).contains(&Prop::FullScreen(false)));
    assert_eq!(app.ui().window_size(window), Some(windowed));
}

/// The user puts it in full screen, or takes it out, the platform's way
/// (the title bar's button, the window manager's key): the app's signal
/// follows.
#[mitsuami_test::test(headless)]
async fn the_user_changes_full_screen_too(app: TestApp) {
    let full = signal(false);
    app.mount(move || Window::new("Machine").full_screen(full).content(|| Text::new("Screen")));
    let window = machine(&app).expect("open");

    app.headless().set_full_screen(window, true);
    app.settle().await;
    assert!(full.get_untracked());
    assert_eq!(app.ui().window_size(window), Some(app.headless().screen()));

    app.headless().set_full_screen(window, false);
    app.settle().await;
    assert!(!full.get_untracked());
}

/// A window smaller than its minimum grows to it, whenever the minimum
/// changes, and the app's sizes go no smaller.
#[mitsuami_test::test]
async fn grows_to_its_minimum_size(app: TestApp) {
    let min = signal(Size::new(400.0, 250.0));
    app.mount(move || {
        Window::new("Machine").size(Size::new(300.0, 200.0)).min_size(min).content(|| Text::new("Screen"))
    });
    let window = machine(&app).expect("open");
    app.settle().await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(400.0, 250.0)));
    assert!(native_props(&app, window).contains(&Prop::MinSize(Size::new(400.0, 250.0))));

    min.set(Size::new(500.0, 200.0));
    app.settle().await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(500.0, 250.0)));

    app.ui().set_window_size(window, Size::new(200.0, 400.0));
    app.settle().await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(500.0, 400.0)));
}

/// The user can't make it smaller than its minimum either.
#[mitsuami_test::test]
async fn the_user_goes_no_smaller_than_its_minimum(app: TestApp) {
    app.mount(|| {
        Window::new("Machine")
            .size(Size::new(400.0, 300.0))
            .min_size(Size::new(320.0, 240.0))
            .content(|| Text::new("Screen"))
    });
    let window = machine(&app).expect("open");

    app.resize_window(window, Size::new(100.0, 100.0)).await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(320.0, 240.0)));
}

/// The app resizes it as it likes, and the content follows.
#[mitsuami_test::test]
async fn the_app_resizes_it(app: TestApp) {
    app.mount(|| {
        Window::new("Machine")
            .size(Size::new(400.0, 300.0))
            .content(|| Column::new().child(Row::new().test_id("fill").size(100.vw(), 100.vh())))
    });
    let window = machine(&app).expect("open");

    app.ui().set_window_size(window, Size::new(640.0, 480.0));
    app.settle().await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(640.0, 480.0)));
    app.expect(by_test_id("fill")).to_have_frame(Rect::new(0.0, 0.0, 640.0, 480.0)).await;
}

/// `view!` takes them as attributes.
#[mitsuami_test::test]
async fn full_screen_and_minimum_size_in_view_macros(app: TestApp) {
    let full = signal(false);
    app.mount(move || {
        view! {
            <Window title="Machine" full_screen=full min_size=Size::new(320.0, 240.0)>
                <Text>"Screen"</Text>
            </Window>
        }
    });
    let window = machine(&app).expect("open");
    assert!(native_props(&app, window).contains(&Prop::MinSize(Size::new(320.0, 240.0))));
}

/// A minimum larger than the screen goes no larger than the screen: a
/// window grows to fit it at most (headless's screen is 1280 × 800).
#[mitsuami_test::test]
async fn its_minimum_goes_no_larger_than_the_screen(app: TestApp) {
    app.mount(|| {
        Window::new("Machine")
            .size(Size::new(400.0, 300.0))
            .min_size(Size::new(10_000.0, 10_000.0))
            .content(|| Text::new("Screen"))
    });
    let window = machine(&app).expect("open");
    app.settle().await;

    let size = app.ui().window_size(window).expect("sized");
    assert!(size.width > 400.0 && size.height > 300.0, "it grew: {size:?}");
    assert!(size.width < 10_000.0 && size.height < 10_000.0, "to the screen at most: {size:?}");
    if app.is_headless() {
        assert_eq!(size, app.headless().screen());
    }
}

/// Maximized follows the app, and the window comes back to its size. A
/// window shows it the platform's way: headless fills its work area,
/// AppKit zooms it to its screen's. Windows maximizes a window by showing
/// it, so WinUI keeps the state until the window shows (test windows
/// never do), and the size meanwhile.
#[mitsuami_test::test]
async fn maximized_follows_the_app(app: TestApp) {
    let zoomed = signal(false);
    app.mount(move || {
        Window::new("Machine").size(Size::new(400.0, 300.0)).maximized(zoomed).content(|| Text::new("Screen"))
    });
    let window = machine(&app).expect("open");
    let restored = app.ui().window_size(window).expect("sized");

    zoomed.set(true);
    app.settle().await;
    assert!(native_props(&app, window).contains(&Prop::Maximized(true)));
    let size = app.ui().window_size(window).expect("sized");
    if app.backend_name() == "winui" {
        assert_eq!(size, restored);
    } else {
        assert!(size.width > restored.width && size.height > restored.height, "{size:?} from {restored:?}");
    }
    if app.is_headless() {
        assert_eq!(size, app.headless().work_area());
    }
    assert!(zoomed.get_untracked());

    zoomed.set(false);
    app.settle().await;
    assert!(native_props(&app, window).contains(&Prop::Maximized(false)));
    assert_eq!(app.ui().window_size(window), Some(restored));
}

/// The user maximizes it, or restores it, the platform's way (the title
/// bar's button, a double-click on it): the app's signal follows.
#[mitsuami_test::test(headless)]
async fn the_user_maximizes_it_too(app: TestApp) {
    let zoomed = signal(false);
    app.mount(move || Window::new("Machine").maximized(zoomed).content(|| Text::new("Screen")));
    let window = machine(&app).expect("open");

    app.headless().set_maximized(window, true);
    app.settle().await;
    assert!(zoomed.get_untracked());
    assert_eq!(app.ui().window_size(window), Some(app.headless().work_area()));

    app.headless().set_maximized(window, false);
    app.settle().await;
    assert!(!zoomed.get_untracked());
}

/// The user can't resize a window that isn't resizable; the app still
/// sizes it, and can make it resizable again.
#[mitsuami_test::test]
async fn a_fixed_window_keeps_its_size(app: TestApp) {
    let resizable = signal(false);
    app.mount(move || {
        Window::new("Machine").size(Size::new(400.0, 300.0)).resizable(resizable).content(|| Text::new("Screen"))
    });
    let window = machine(&app).expect("open");
    assert!(native_props(&app, window).contains(&Prop::Resizable(false)));

    app.resize_window(window, Size::new(500.0, 400.0)).await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(400.0, 300.0)));

    app.ui().set_window_size(window, Size::new(480.0, 360.0));
    app.settle().await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(480.0, 360.0)));

    resizable.set(true);
    app.settle().await;
    assert!(native_props(&app, window).contains(&Prop::Resizable(true)));
    app.resize_window(window, Size::new(500.0, 400.0)).await;
    assert_eq!(app.ui().window_size(window), Some(Size::new(500.0, 400.0)));
}

mitsuami_test::main!();
