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
                .item(MenuItem::new("Increment", increment).shortcut(Shortcut::primary('i')))
                .item(MenuItem::new("Reset…", confirm_reset).enabled(move || count.get() != 0)),
        ),
    );
    Column::new().gap(Spacing::Md).children((
        Text::new(move || format!("Count: {}", count.get())).text_style(TextStyle::Title),
        Row::new().gap(Spacing::Sm).children((
            Button::new("Increment").role(ButtonRole::Default).on_click(increment),
            Button::new("Reset…").enabled(move || count.get() != 0).on_click(confirm_reset),
        )),
        Show::new(move || count.get() >= 5, || Text::new("That's a lot of clicks.").text_style(TextStyle::Caption)),
    ))
}

fn signup() -> impl View {
    let name = signal(String::new());
    let agreed = signal(false);
    let newsletter = signal(true);
    let plan = signal(0);
    let volume = signal(50.0_f64);
    let submitted = signal(None::<String>);
    let submit = move || submitted.set(Some(name.get_untracked()));
    Column::new().gap(Spacing::Md).children((
        Text::new("Sign up").text_style(TextStyle::Headline),
        Grid::new()
            .columns([Track::MaxContent, Track::Size(1.fr())])
            .column_gap(Spacing::Md)
            .row_gap(Spacing::Sm)
            .align(Align::Center)
            .children((
                Text::new("Name"),
                TextInput::new().a11y_label("Name").placeholder("Ada Lovelace").bind(name).on_submit(submit),
                Text::new("Newsletter"),
                Switch::new("Newsletter").bind(newsletter),
                Text::new("Plan"),
                Select::new("Plan").options(["Free", "Pro", "Team"]).bind(plan),
                Text::new(move || format!("Volume ({})", volume.get().round())),
                Slider::new("Volume").range(0.0, 100.0).step(10.0).bind(volume),
            )),
        Checkbox::new("I agree to the terms").bind(agreed),
        Row::new().gap(Spacing::Sm).align(Align::Center).children((
            Button::new("Sign up").role(ButtonRole::Default).enabled(agreed).on_click(submit),
            Text::new(move || match submitted.get() {
                Some(who) if who.is_empty() => "Signed up anonymously".to_string(),
                Some(who) => format!("Welcome, {who}!"),
                None => String::new(),
            }),
        )),
    ))
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
    Row::new().gap(Spacing::Md).align(Align::Center).children((
        Button::new("Download").enabled(move || !running.get()).on_click(start),
        Progress::new("Download").value(done).indeterminate(connecting).grow(1.0),
        Text::new(move || {
            if connecting.get() { "Connecting…".to_string() } else { format!("{}%", (done.get() * 100.0).round()) }
        }),
    ))
}

fn uptime() -> impl View {
    let seconds = signal(0u64);
    spawn_local(async move {
        loop {
            sleep(std::time::Duration::from_secs(1)).await;
            seconds.update(|s| *s += 1);
        }
    });
    Text::new(move || format!("Open for {}s", seconds.get())).text_style(TextStyle::Caption)
}

fn log(lines: Signal<Vec<u32>>) -> impl View {
    ScrollView::new().height(120).child(For::new(lines, |i: &u32| *i, |i| Text::new(format!("Log line {i}"))))
}

fn main() {
    App::new()
        .window("mitsuami showcase", WindowSize::FitHeight(343.0), || {
            let log_lines = signal((1..=20).collect::<Vec<u32>>());
            Column::new().padding(Spacing::Xl).gap(Spacing::Xl).children((
                counter(log_lines),
                signup(),
                download(),
                log(log_lines),
                uptime(),
            ))
        })
        .run();
}
