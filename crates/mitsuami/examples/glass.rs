//! Headers over what scrolls: `cargo run -p mitsuami --example glass`.
//!
//! A Git client's toolbar band as the window's header (`Container::header`,
//! with the hard edge) over a table of commits whose rows are loud colours,
//! to see them under the title bar. Four windows build it the ways an app
//! might:
//!
//! 1. The band over the table.
//! 2. And document tabs in the title's row.
//! 3. And the table in a split's second pane, beside a sidebar that
//!    doesn't scroll.
//! 4. Both, as SourceGit's repository window is.
//!
//! - Scroll a table: on macOS 26 its rows go under the title bar and the
//!   band, and show through their glass in every window. The glass is
//!   AppKit's: faint where the table runs under the whole band (1, 2),
//!   fuller where part of the band has the sidebar under it (3, 4).
//! - Elsewhere the band is the content's top, and the table starts below
//!   it.
//! - Row 3 starts selected: its bar spans the line, under the band too.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::core::AnyView;
use mitsuami::prelude::*;

#[derive(Clone, PartialEq)]
struct Commit {
    id: u32,
}

fn band() -> impl View {
    Row::new()
        .header(true)
        .header_edge(HeaderEdge::Hard)
        .height(36)
        .padding_y(4)
        .gap(16)
        .justify(Justify::Center)
        .align(Align::Center)
        .children(
            ["Fetch", "Pull", "Push", "Stash"]
                .map(|name| Button::new(name).icon(icon(name)).icon_only(true).button_style(ButtonStyle::Borderless))
                .into_iter()
                .collect::<Vec<_>>(),
        )
}

/// The band's icons, each platform's own.
fn icon(name: &str) -> &'static str {
    match name {
        "Fetch" => platform! {
            macos => "arrow.down.circle", gtk => "view-refresh-symbolic", kde => "view-refresh", windows => "\u{E72C}",
        },
        "Pull" => platform! {
            macos => "tray.and.arrow.down", gtk => "go-down-symbolic", kde => "go-down", windows => "\u{E896}",
        },
        "Push" => platform! {
            macos => "arrow.up.circle", gtk => "go-up-symbolic", kde => "go-up", windows => "\u{E898}",
        },
        _ => platform! {
            macos => "square.stack.3d.up", gtk => "document-save-symbolic", kde => "document-save", windows => "\u{E7B8}",
        },
    }
}

fn table() -> impl View {
    let commits: Vec<Commit> = (0..300).map(|id| Commit { id }).collect();
    // A row selected from the start: its bar spans the whole line.
    let selected = signal(vec![3u32]);
    Table::new(commits, |c: &Commit| c.id)
        .selected(selected)
        .column(
            TableColumn::new("GRAPH & SUBJECT", |c: Commit| {
                // Loud rows, to see them under the title bar.
                const COLORS: [(u8, u8, u8); 6] =
                    [(255, 40, 40), (255, 150, 0), (240, 220, 0), (0, 200, 60), (0, 120, 255), (190, 60, 255)];
                const BLOCKS: [&str; 6] = ["🟥🟥🟥", "🟧🟧🟧", "🟨🟨🟨", "🟩🟩🟩", "🟦🟦🟦", "🟪🟪🟪"];
                let i = c.id as usize % 6;
                let (r, g, b) = COLORS[i];
                Row::new().gap(8).align(Align::Center).children((
                    Text::new(BLOCKS[i]).text_style(TextStyle::Title),
                    Text::new(format!("Commit subject {}", c.id))
                        .text_style(TextStyle::Title)
                        .weight(FontWeight::Bold)
                        .color(Color::rgb(r, g, b)),
                ))
            })
            .expand(),
        )
        .column(TableColumn::new("AUTHOR", |_: Commit| Text::new("David Rios")).width(120))
        .column(TableColumn::new("SHA", |c: Commit| Text::new(format!("{:010x}", c.id * 7919))).width(100))
        .grow(1.0)
}

fn tabs() -> impl View {
    let current = signal(1u32);
    DocumentTabs::new(current)
        .tabs_style(DocumentTabsStyle::TitleRow)
        .children_with(|| vec![DocumentTab::new("Welcome", 0u32), DocumentTab::new("graphrepo", 1u32)])
}

fn body(split: bool) -> AnyView {
    if !split {
        return AnyView::new(table());
    }
    let width = signal(250.0);
    let split = Split::new().bind(width).grow(1.0).children((
        Column::new().padding(12).gap(6).min_width(200).children((
            Text::new("LOCAL BRANCHES").color(Color::SecondaryLabel),
            Text::new("main"),
            Text::new("feature"),
        )),
        Column::new().min_width(100).align(Align::Stretch).child(table()),
    ));
    AnyView::new(split)
}

fn window(title_tabs: bool, split: bool) -> impl Fn() -> AnyView {
    move || {
        let page = Column::new().align(Align::Stretch).grow(1.0);
        match title_tabs {
            true => AnyView::new(page.children((tabs(), band(), body(split)))),
            false => AnyView::new(page.children((band(), body(split)))),
        }
    }
}

fn main() {
    App::new()
        .window("1 Band over the table", Size::new(560.0, 420.0), window(false, false))
        .window("2 And title-row tabs", Size::new(560.0, 420.0), window(true, false))
        .window("3 And a split", Size::new(560.0, 420.0), window(false, true))
        .window("4 SourceGit", Size::new(560.0, 420.0), window(true, true))
        .run();
}
