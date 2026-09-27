//! `view!` and `#[component]`: they build what the builder API builds, and
//! components get typed props, reactive props, callbacks, children and a
//! scope of their own.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami::core::A11yNode;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn counter_with_builders(initial: i32) -> impl View {
    let count = signal(initial);
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Text::new(move || format!("Count: {}", count.get())).text_style(TextStyle::Title),
        Row::new().gap(8).children((
            Button::new("Increment").role(ButtonRole::Default).on_click(move || count.update(|c| *c += 1)),
            Button::new("Reset").on_click(move || count.set(0)),
        )),
        Show::new(move || count.get() >= 3, || Text::new("That's a lot of clicks")),
    ))
}

fn counter_with_view(initial: i32) -> impl View {
    let count = signal(initial);
    view! {
        <Column padding=16 gap=8 align=Align::Start>
            <Text text_style=TextStyle::Title>{move || format!("Count: {}", count.get())}</Text>
            <Row gap=8>
                <Button role=ButtonRole::Default @click=move || count.update(|c| *c += 1)>"Increment"</Button>
                <Button @click=move || count.set(0)>"Reset"</Button>
            </Row>
            <Show when={move || count.get() >= 3}>
                <Text>"That's a lot of clicks"</Text>
            </Show>
        </Column>
    }
}

/// Roles, names, values and frames, without node ids.
fn outline(node: &A11yNode) -> String {
    let mut out = format!("{:?} {:?} {:?} {:?} {:?}", node.role, node.name, node.value, node.checked, node.frame);
    for child in &node.children {
        out.push_str(&format!("\n{}", outline(child).replace('\n', "\n  ")));
    }
    out
}

#[mitsuami_test::test]
async fn view_builds_what_the_builders_build(app: TestApp) {
    app.mount(|| counter_with_builders(5));
    app.settle().await;
    let built = outline(&app.a11y_tree());
    app.unmount();

    app.mount(|| counter_with_view(5));
    app.settle().await;
    assert_eq!(outline(&app.a11y_tree()), built);
}

#[mitsuami_test::test]
async fn view_handlers_and_show_work(app: TestApp) {
    app.mount(|| counter_with_view(1));
    app.expect(by_text("That's a lot of clicks")).not_to_exist().await;

    app.get_by_role(Role::Button, "Increment").click().await;
    app.get_by_role(Role::Button, "Increment").click().await;
    app.expect(by_text("Count: 3")).to_exist().await;
    app.expect(by_text("That's a lot of clicks")).to_be_visible().await;

    app.get_by_role(Role::Button, "Reset").click().await;
    app.expect(by_text("That's a lot of clicks")).not_to_exist().await;
}

#[mitsuami_test::test]
async fn show_takes_a_fallback_before_or_after_when(app: TestApp) {
    let logged_in = signal(false);
    app.mount(move || {
        view! {
            <Column>
                <Show when=logged_in fallback=|| view! { <Button>"Log in"</Button> }>
                    <Text>"Welcome back"</Text>
                </Show>
                <Show fallback=|| "Signed out" when=logged_in>
                    <Text>"Signed in"</Text>
                </Show>
            </Column>
        }
    });
    app.expect(by_role(Role::Button, "Log in")).to_exist().await;
    app.expect(by_text("Signed out")).to_exist().await;

    logged_in.set(true);
    app.expect(by_text("Welcome back")).to_exist().await;
    app.expect(by_text("Signed in")).to_exist().await;
    app.expect(by_role(Role::Button, "Log in")).not_to_exist().await;
}

#[derive(Clone)]
struct Todo {
    id: u32,
    title: &'static str,
}

fn titles(app: &TestApp) -> Vec<String> {
    app.a11y_tree().walk().into_iter().filter(|n| n.role == Role::StaticText).filter_map(|n| n.name.clone()).collect()
}

#[mitsuami_test::test]
async fn for_renders_keyed_rows_with_let(app: TestApp) {
    let todos = signal(vec![Todo { id: 1, title: "a" }, Todo { id: 2, title: "b" }]);
    app.mount(move || {
        view! {
            <Column>
                <For each=todos key=|todo| todo.id let:todo>
                    <Text>{todo.title}</Text>
                </For>
            </Column>
        }
    });
    assert_eq!(titles(&app), ["a", "b"]);

    let a = app.get_by_text("a").id();
    todos.set(vec![Todo { id: 3, title: "c" }, Todo { id: 1, title: "a" }]);
    app.settle().await;
    assert_eq!(titles(&app), ["c", "a"]);
    assert_eq!(app.get_by_text("a").id(), a, "the row kept its node");
}

#[mitsuami_test::test]
async fn inputs_bind_and_emit(app: TestApp) {
    let name = signal(String::new());
    let agreed = signal(false);
    let submitted = Rc::new(Cell::new(0));
    let count = submitted.clone();
    app.mount(move || {
        view! {
            <Column gap=8>
                <TextInput bind=name placeholder="Name" a11y_label="Name" @submit=move || count.set(count.get() + 1)/>
                <Checkbox bind=agreed>"I agree"</Checkbox>
                <Text>{move || format!("{} {}", name.get(), if agreed.get() { "agreed" } else { "didn't agree" })}</Text>
            </Column>
        }
    });

    app.get_by_label("Name").fill("Ada").await;
    app.get_by_label("Name").press(Key::Enter).await;
    app.get_by_role(Role::Checkbox, "I agree").check().await;

    app.expect(by_text("Ada agreed")).to_exist().await;
    assert_eq!(submitted.get(), 1);
}

#[mitsuami_test::test]
async fn attribute_values_can_be_any_expression(app: TestApp) {
    let width = 2 * 60;
    app.mount(move || {
        view! {
            <Row wrap gap={4 + 4} width=width>
                <Text test_id="turbofish">{Vec::<&str>::new().len().to_string()}</Text>
                <Text test_id="closure" hidden=|| -> bool { false }>"shown"</Text>
                <Text test_id="block">{
                    let words = ["a", "b"];
                    words.join(" ")
                }</Text>
            </Row>
        }
    });
    app.expect(by_test_id("turbofish")).to_have_text("0").await;
    app.expect(by_test_id("closure")).to_be_visible().await;
    app.expect(by_test_id("block")).to_have_text("a b").await;
}

#[mitsuami_test::test]
async fn siblings_can_share_variables(app: TestApp) {
    app.mount(|| {
        let name = "Ada".to_string();
        // Children are built right away, like the builder's: the first
        // child borrows `name`, the last one takes it.
        view! {
            <Column>
                <Text>{format!("Hello, {name}")}</Text>
                <Row><Button a11y_label=format!("Greet {name}")>"Greet"</Button></Row>
                <Text>{name}</Text>
            </Column>
        }
    });
    app.expect(by_text("Hello, Ada")).to_exist().await;
    app.expect(by_role(Role::Button, "Greet Ada")).to_exist().await;
    app.expect(by_text("Ada")).to_exist().await;
}

#[mitsuami_test::test]
async fn many_children_and_several_roots(app: TestApp) {
    app.mount(|| {
        view! {
            <Column>
                "1" "2" "3" "4" "5" "6" "7" "8" "9" "10" "11" "12" "13" "14"
                <>
                    "15"
                    <Text>"16"</Text>
                </>
            </Column>
        }
    });
    let expected: Vec<String> = (1..=16).map(|n| n.to_string()).collect();
    assert_eq!(titles(&app), expected);

    app.unmount();
    app.mount(|| Column::new().children(view! { <Text>"one"</Text> <Text>"two"</Text> }));
    assert_eq!(titles(&app), ["one", "two"]);
}

// --------------------------------------------------------- custom widgets

/// A dot that is on or off; activating it asks to toggle.
struct Dot;

#[derive(Clone, Debug, PartialEq)]
struct DotProps {
    on: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum DotEvent {
    Toggle,
}

impl CustomWidget for Dot {
    const NAME: &'static str = "Dot";
    type Props = DotProps;
    type Event = DotEvent;

    fn a11y(props: &DotProps) -> A11yProps {
        A11yProps::new(Role::Button).value(if props.on { "On" } else { "Off" })
    }

    fn action(_props: &DotProps, action: &A11yAction) -> Option<DotEvent> {
        (*action == A11yAction::Activate).then_some(DotEvent::Toggle)
    }
}

impl Drawn for Dot {
    fn measure(_props: &DotProps, _request: &MeasureRequest, _metrics: &PlatformMetrics) -> Size {
        Size::new(16.0, 16.0)
    }

    fn draw(props: &DotProps, canvas: &mut Canvas) {
        let dot = Path::polygon([(0.0, 0.0), (16.0, 0.0), (16.0, 16.0), (0.0, 16.0)].map(|(x, y)| Point::new(x, y)));
        if props.on {
            canvas.fill(dot, Color::Accent);
        } else {
            canvas.stroke(dot, Color::SecondaryLabel, 1.0);
        }
    }
}

impl Render for Dot {
    fn renderer() -> Renderer<Self> {
        Renderer::drawn()
    }
}

#[mitsuami_test::test]
async fn custom_widgets_are_tags(app: TestApp) {
    let on = signal(false);
    app.mount(move || {
        view! {
            <Row gap=8>
                <Dot props=move || DotProps { on: on.get() } a11y_label="First" @event=move |_| on.update(|on| *on = !*on)/>
                // Attributes can come before the props.
                <Dot a11y_label="Second" test_id="second" props=|| DotProps { on: true }/>
            </Row>
        }
    });
    app.expect(by_role(Role::Button, "First")).to_exist().await;
    assert_eq!(app.get_by_role(Role::Button, "First").value().as_deref(), Some("Off"));
    assert_eq!(app.get_by_test_id("second").value().as_deref(), Some("On"));

    app.get_by_role(Role::Button, "First").click().await;
    assert!(on.get_untracked());
    assert_eq!(app.get_by_role(Role::Button, "First").value().as_deref(), Some("On"));
}

// ------------------------------------------------------------- components

/// A counter with a caption, a step, and a callback on every change.
#[component]
fn Stepper(
    #[prop(into)] label: String,
    count: Signal<i32>,
    #[prop(default = 1)] step: i32,
    on_change: Callback<i32>,
) -> impl View {
    view! {
        <Row gap=8 align=Align::Center>
            <Text>{move || format!("{label}: {}", count.get())}</Text>
            <Button a11y_label=format!("Add {step}") @click=move || {
                count.update(|c| *c += step);
                on_change.call(count.get_untracked());
            }>"+"</Button>
        </Row>
    }
}

#[mitsuami_test::test]
async fn components_take_required_default_and_callback_props(app: TestApp) {
    let apples = signal(0);
    let pears = signal(0);
    let changes = Rc::new(Cell::new(0));
    let seen = changes.clone();
    app.mount(move || {
        view! {
            <Column>
                <Stepper label="Apples" count=apples/>
                <Stepper label="Pears" count=pears step=5 @change=move |n| seen.set(n)/>
            </Column>
        }
    });

    app.get_by_role(Role::Button, "Add 1").click().await;
    app.get_by_role(Role::Button, "Add 5").click().await;
    app.get_by_role(Role::Button, "Add 5").click().await;

    app.expect(by_text("Apples: 1")).to_exist().await;
    app.expect(by_text("Pears: 10")).to_exist().await;
    assert_eq!(changes.get(), 10);
}

#[component]
fn Greeting(name: Value<String>) -> impl View {
    view! { <Text>{move || format!("Hello, {}!", name.get())}</Text> }
}

#[mitsuami_test::test]
async fn value_props_are_static_or_reactive(app: TestApp) {
    let who = signal("Ada".to_string());
    app.mount(move || {
        view! {
            <Column>
                <Greeting name="world"/>
                <Greeting name=who/>
                {Greeting::new().name(move || who.get().to_uppercase())}
            </Column>
        }
    });
    app.expect(by_text("Hello, world!")).to_exist().await;
    app.expect(by_text("Hello, Ada!")).to_exist().await;
    app.expect(by_text("Hello, ADA!")).to_exist().await;

    who.set("Grace".into());
    app.expect(by_text("Hello, Grace!")).to_exist().await;
    app.expect(by_text("Hello, GRACE!")).to_exist().await;
}

#[component]
fn Card(#[prop(into)] title: String, subtitle: Option<String>, children: Slot) -> impl View {
    view! {
        <Column gap=4 padding=8>
            <Text text_style=TextStyle::Headline>{title}</Text>
            {subtitle.map(|s| view! { <Text text_style=TextStyle::Caption>{s}</Text> })}
            <Column>{children}</Column>
        </Column>
    }
}

#[mitsuami_test::test]
async fn components_render_their_children(app: TestApp) {
    app.mount(|| {
        view! {
            <Column>
                <Card title="Plain"/>
                <Card title="Full" subtitle="With a subtitle".to_string()>
                    <Text>"First"</Text>
                    <Text>"Second"</Text>
                </Card>
            </Column>
        }
    });
    assert_eq!(titles(&app), ["Plain", "Full", "With a subtitle", "First", "Second"]);
}

#[derive(Clone)]
struct Theme(&'static str);

#[component]
fn Themed(theme: &'static str, children: Slot) -> impl View {
    provide(Theme(theme));
    let disposed = inject::<Rc<Cell<u32>>>();
    on_cleanup(move || {
        if let Some(disposed) = disposed {
            disposed.set(disposed.get() + 1);
        }
    });
    view! { <Column>{children}</Column> }
}

#[component]
fn ThemeName() -> impl View {
    let theme = inject::<Theme>().map_or("none", |t| t.0);
    view! { <Text>{theme}</Text> }
}

#[mitsuami_test::test]
async fn components_have_a_scope_of_their_own(app: TestApp) {
    let disposed = Rc::new(Cell::new(0u32));
    let open = signal(true);
    let log = disposed.clone();
    app.mount(move || {
        provide(log.clone());
        view! {
            <Column>
                <Show when=open>
                    <Themed theme="dark"><ThemeName/></Themed>
                </Show>
                <ThemeName/>
            </Column>
        }
    });
    // What a component provides reaches its children, not its siblings.
    assert_eq!(titles(&app), ["dark", "none"]);

    open.set(false);
    app.settle().await;
    assert_eq!(disposed.get(), 1, "the component's scope was disposed with it");
}

mitsuami_test::main!();
