//! The screen, in components.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use mitsuami::prelude::*;

use crate::store::Todos;

pub const QUOTES: [&str; 3] = [
    "Make it work, make it right, make it fast.",
    "Simple things should be simple, complex things should be possible.",
    "The best code is no code at all.",
];

/// How long the quote server takes, and how long a sync takes.
pub const LATENCY: Duration = Duration::from_millis(600);

#[component]
pub fn Screen() -> impl View {
    view! {
        <Column gap=Spacing::Xl>
            <Section title="Todos">
                <NewTodo/>
                <TodoList/>
            </Section>
            <Section title="Quote of the day">
                <Quote/>
            </Section>
            <Section title="Sync">
                <Sync/>
            </Section>
        </Column>
    }
}

/// A headline over whatever goes between the tags.
#[component]
fn Section(#[prop(into)] title: String, children: Slot) -> impl View {
    view! {
        <Column gap=Spacing::Sm>
            <Text text_style=TextStyle::Headline>{title}</Text>
            {children}
        </Column>
    }
}

#[component]
fn NewTodo() -> impl View {
    let todos = use_store::<Todos>();
    let draft = signal(String::new());
    let add = move || {
        todos.add(&draft.get_untracked());
        draft.set(String::new());
    };
    view! {
        <Row gap=Spacing::Sm align=Align::Center>
            <TextInput bind=draft placeholder="What needs doing?" a11y_label="New todo" grow=1.0 @submit=add/>
            <Button enabled=move || !draft.get().trim().is_empty() @click=add>"Add"</Button>
        </Row>
    }
}

#[component]
fn TodoList() -> impl View {
    let todos = use_store::<Todos>();
    view! {
        <Column gap=Spacing::Sm>
            <For each=move || todos.visible() key=|todo| todo.id let:todo>
                <TodoRow id=todo.id title=todo.title @remove=move |id| todos.remove(id)/>
            </For>
            <Show when={move || todos.items.with(Vec::is_empty)}>
                <Text text_style=TextStyle::Caption>"Nothing to do."</Text>
            </Show>
            <Row gap=Spacing::Md align=Align::Center>
                <Text grow=1.0 text_style=TextStyle::Caption>{move || match todos.left() {
                    1 => "1 left".to_string(),
                    n => format!("{n} left"),
                }}</Text>
                <Switch bind=todos.hide_done a11y_label="Hide done"/>
                <Text text_style=TextStyle::Caption>"Hide done"</Text>
            </Row>
        </Column>
    }
}

/// One todo: a checkbox for done, and a button that asks to remove it.
#[component]
fn TodoRow(id: u32, title: String, on_remove: Callback<u32>) -> impl View {
    let todos = use_store::<Todos>();
    view! {
        <Row gap=Spacing::Sm align=Align::Center>
            <Checkbox grow=1.0 checked=move || todos.is_done(id) @change=move |done| todos.set_done(id, done)>
                {title.clone()}
            </Checkbox>
            <Button a11y_label=format!("Remove {title}") @click=move || on_remove.call(id)>"Remove"</Button>
        </Row>
    }
}

/// Loads a quote from a slow server that naps on every third request.
#[component]
fn Quote() -> impl View {
    let requests = Rc::new(Cell::new(0usize));
    let quote = resource(move || {
        let n = requests.get();
        requests.set(n + 1);
        async move {
            sleep(LATENCY).await;
            if n % 3 == 2 { Err("The quote server is napping.".to_string()) } else { Ok(QUOTES[n % QUOTES.len()]) }
        }
    });
    view! {
        <Column gap=Spacing::Sm>
            <Text>{move || quote.data().unwrap_or("…").to_string()}</Text>
            <Row gap=Spacing::Md align=Align::Center>
                <Button enabled=move || !quote.loading() @click=move || quote.refetch()>"Another one"</Button>
                <Text text_style=TextStyle::Caption>{move || match (quote.loading(), quote.error()) {
                    (true, _) => "Loading…".to_string(),
                    (false, Some(error)) => error,
                    (false, None) => String::new(),
                }}</Text>
            </Row>
        </Column>
    }
}

/// Pretends to upload the list.
#[component]
fn Sync() -> impl View {
    let todos = use_store::<Todos>();
    let sync = action(|count: usize| async move {
        sleep(LATENCY).await;
        match count {
            1 => "Synced 1 todo.".to_string(),
            n => format!("Synced {n} todos."),
        }
    });
    view! {
        <Row gap=Spacing::Md align=Align::Center>
            <Button
                role=ButtonRole::Default
                enabled=move || !sync.pending()
                @click=move || sync.dispatch(todos.items.with(Vec::len))
            >"Sync now"</Button>
            <Text>{move || if sync.pending() { "Syncing…".to_string() } else { sync.value().unwrap_or_default() }}</Text>
        </Row>
    }
}
