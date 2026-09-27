//! A counter and a small form: `cargo run -p mitsuami --example showcase`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn counter(log_lines: Signal<Vec<u32>>) -> impl View {
    let count = signal(0);
    let increment = move || {
        count.update(|c| *c += 1);
        log_lines.update(|l| l.push(l.len() as u32 + 1));
    };
    let confirm_reset = move || {
        spawn_local(async move {
            let answer = alert(
                Alert::new("Reset the counter?")
                    .message(format!("It is at {}.", count.get_untracked()))
                    .button("Reset")
                    .button("Cancel")
                    .style(AlertStyle::Warning),
            )
            .await;
            if answer == 0 {
                count.set(0);
                log_lines.set(Vec::new());
            }
        });
    };
    set_menu(
        MenuBar::new().menu(
            Menu::new("Counter")
                .item(MenuItem::new("Increment").on_select(increment).shortcut(Shortcut::primary('i')))
                .item(MenuItem::new("Reset…").on_select(confirm_reset).enabled(move || count.get() != 0)),
        ),
    );
    view! {
        <Column gap=Spacing::Md>
            <Text text_style=TextStyle::Title>{move || format!("Count: {}", count.get())}</Text>
            <Row gap=Spacing::Sm>
                <Button role=ButtonRole::Default @click=increment>"Increment"</Button>
                <Button enabled=move || count.get() != 0 @click=confirm_reset>"Reset…"</Button>
            </Row>
            <Show when=move || count.get() >= 5>
                <Text text_style=TextStyle::Caption>"That's a lot of clicks."</Text>
            </Show>
        </Column>
    }
}

fn signup() -> impl View {
    let name = signal(String::new());
    let agreed = signal(false);
    let newsletter = signal(true);
    let plan = signal(0);
    let volume = signal(50.0_f64);
    let submitted = signal(None::<String>);
    let submit = move || submitted.set(Some(name.get_untracked()));
    view! {
        <Column gap=Spacing::Md>
            <Text text_style=TextStyle::Headline>"Sign up"</Text>
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Sm
                align=Align::Center
            >
                <Text>"Name"</Text>
                <TextInput a11y_label="Name" placeholder="Ada Lovelace" bind=name @submit=submit/>
                <Text>"Newsletter"</Text>
                <Switch bind=newsletter>"Newsletter"</Switch>
                <Text>"Plan"</Text>
                <Select label="Plan" options=["Free", "Pro", "Team"] bind=plan/>
                <Text>{move || format!("Volume ({})", volume.get().round())}</Text>
                <Slider label="Volume" range_with=(0.0, 100.0) step=10.0 bind=volume/>
            </Grid>
            <Checkbox bind=agreed>"I agree to the terms"</Checkbox>
            <Row gap=Spacing::Sm align=Align::Center>
                <Button role=ButtonRole::Default enabled=agreed @click=submit>"Sign up"</Button>
                <Text>
                    {move || match submitted.get() {
                        Some(who) if who.is_empty() => "Signed up anonymously".to_string(),
                        Some(who) => format!("Welcome, {who}!"),
                        None => String::new(),
                    }}
                </Text>
            </Row>
        </Column>
    }
}

/// A pretend download: work of unknown length while it connects, then how
/// far along it is.
fn download() -> impl View {
    let running = signal(false);
    let connecting = signal(false);
    let done = signal(0.0_f64);
    let start = move || {
        running.set(true);
        connecting.set(true);
        done.set(0.0);
        spawn_local(async move {
            sleep(std::time::Duration::from_secs(1)).await;
            connecting.set(false);
            for step in 1..=20 {
                sleep(std::time::Duration::from_millis(100)).await;
                done.set(step as f64 / 20.0);
            }
            running.set(false);
        });
    };
    view! {
        <Row gap=Spacing::Md align=Align::Center>
            <Button enabled=move || !running.get() @click=start>"Download"</Button>
            <Progress label="Download" value=done indeterminate=connecting grow=1.0/>
            <Text>
                {move || {
                    if connecting.get() { "Connecting…".to_string() } else { format!("{}%", (done.get() * 100.0).round()) }
                }}
            </Text>
        </Row>
    }
}

fn uptime() -> impl View {
    let seconds = signal(0u64);
    spawn_local(async move {
        loop {
            sleep(std::time::Duration::from_secs(1)).await;
            seconds.update(|s| *s += 1);
        }
    });
    view! { <Text text_style=TextStyle::Caption>{move || format!("Open for {}s", seconds.get())}</Text> }
}

fn log(lines: Signal<Vec<u32>>) -> impl View {
    view! {
        <ScrollView height=120>
            <For each=lines key=|i: &u32| *i let:i>
                <Text>{format!("Log line {i}")}</Text>
            </For>
        </ScrollView>
    }
}

fn main() {
    App::new()
        .window("mitsuami showcase", WindowSize::FitHeight(343.0), || {
            let log_lines = signal((1..=20).collect::<Vec<u32>>());
            view! {
                <Column padding=Spacing::Xl gap=Spacing::Xl>
                    {counter(log_lines)}
                    {signup()}
                    {download()}
                    {log(log_lines)}
                    {uptime()}
                </Column>
            }
        })
        .run();
}
