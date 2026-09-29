//! Measurements: `cargo run -p mitsuami --example measurements`.
//!
//! Resize the window and watch the layout adapt:
//!
//! - A media query: `use_viewport()` is the window's content size. Below
//!   640 wide, the sidebar moves above the content.
//! - A container query: `use_size(node_ref)` is a node's size. The panel's
//!   cards go in as many columns as the panel has room for, whatever the
//!   window's width, so they change when the sidebar moves too.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

fn heading(text: &str) -> impl View {
    let text = text.to_string();
    view! { <Text text_style=TextStyle::Headline>{text}</Text> }
}

fn sidebar(wide: Computed<bool>) -> impl View {
    let buttons = ["Inbox", "Drafts", "Sent", "Archive"].map(|name| view! { <Button>{name}</Button> });
    view! {
        // A column beside the content, or a row above it.
        <Container
            flex_direction=move || if wide.get() { FlexDirection::Column } else { FlexDirection::Row }
            width=move || if wide.get() { 160.px() } else { Length::Auto }
            align=Align::Start
            gap=Spacing::Sm
            shrink=0.0
        >
            {buttons.into_iter().collect::<Vec<_>>()}
        </Container>
    }
}

fn card(index: usize, columns: Computed<usize>) -> impl View {
    view! {
        <Column width=move || (100.0 / columns.get() as f32).pct() padding=Spacing::Sm gap=Spacing::Xs>
            <Text>{format!("Card {index}")}</Text>
            <Progress label="Progress" value={index as f64 / 6.0} />
        </Column>
    }
}

fn panel() -> impl View {
    let panel = node_ref();
    let size = use_size(panel);
    let columns = computed(move || match size.get().width {
        w if w >= 480.0 => 3,
        w if w >= 300.0 => 2,
        _ => 1,
    });
    let cards = (1..=6).map(|i| card(i, columns)).collect::<Vec<_>>();
    view! {
        // No wider than the room left, whatever its cards' width.
        <Column node_ref=panel grow=1.0 min_width=0 gap=Spacing::Sm>
            {heading("Panel")}
            <Text text_style=TextStyle::Caption>
                {move || format!("{:.0} wide: {} columns", size.get().width, columns.get())}
            </Text>
            <Row wrap>{cards}</Row>
        </Column>
    }
}

pub fn content() -> impl View {
    let viewport = use_viewport();
    let wide = computed(move || viewport.get().width >= 640.0);
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Lg>
            <Text text_style=TextStyle::Caption>
                {move || {
                    let size = viewport.get();
                    let layout = if wide.get() { "sidebar beside" } else { "sidebar above" };
                    format!("Window {:.0} × {:.0}: {layout}", size.width, size.height)
                }}
            </Text>
            <Container
                flex_direction=move || if wide.get() { FlexDirection::Row } else { FlexDirection::Column }
                gap=Spacing::Lg
            >
                {sidebar(wide)}
                {panel()}
            </Container>
        </Column>
    }
}

fn main() {
    App::new().window("Measurements", Size::new(760.0, 480.0), content).run();
}
