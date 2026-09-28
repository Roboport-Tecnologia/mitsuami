//! Icons: `cargo run -p mitsuami --example icon`.
//!
//! A disc library, as an emulator's disc picker lists its images: each
//! row has an icon from the platform's own set, and a button that shows
//! only its icon.
//!
//! - The icons are each platform's own: SF Symbols on macOS, the icon
//!   theme's on Linux, Segoe Fluent Icons on Windows.
//! - Move the slider: the drive's icon grows, as the platform sizes icons.
//! - The drive's icon is the accent colour and the rows' the secondary
//!   one: the platform's own, so they follow dark mode, high contrast and
//!   the user's accent. Switch to dark mode to see them follow.
//! - Turn off "Icons only": the row buttons show their caption beside the
//!   icon. VoiceOver, Orca, Narrator and friends read the caption either way.
//! - The trash buttons remove their row. "Add" is the platform's menu
//!   button: its menu adds a disc of the kind chosen, and Guest tools is a
//!   submenu.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Iso,
    Cue,
    Folder,
    Tools,
}

/// Each kind's icon, in each platform's set.
fn icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Iso => platform! {
            macos => "opticaldisc",
            gtk => "media-optical-symbolic",
            kde => "media-optical",
            windows => "\u{E958}",
        },
        Kind::Cue => platform! {
            macos => "list.bullet.rectangle",
            gtk => "view-list-symbolic",
            kde => "view-list-details",
            windows => "\u{E8FD}",
        },
        Kind::Folder => platform! {
            macos => "folder",
            gtk => "folder-symbolic",
            kde => "folder",
            windows => "\u{E8B7}",
        },
        Kind::Tools => platform! {
            macos => "wrench.and.screwdriver",
            gtk => "applications-engineering-symbolic",
            kde => "tools",
            windows => "\u{E90F}",
        },
    }
}

fn trash() -> &'static str {
    platform! {
        macos => "trash",
        gtk => "user-trash-symbolic",
        kde => "edit-delete",
        windows => "\u{E74D}",
    }
}

fn add() -> &'static str {
    platform! {
        macos => "plus",
        gtk => "list-add-symbolic",
        kde => "list-add",
        windows => "\u{E710}",
    }
}

#[derive(Clone, PartialEq)]
struct Disc {
    id: u32,
    title: &'static str,
    path: &'static str,
    kind: Kind,
}

const LIBRARY: [(&str, &str, Kind); 4] = [
    ("Total Annihilation (1997)", "~/Downloads/Total_Annihilation_1997.iso", Kind::Iso),
    ("Mortal Kombat 1 & 2", "~/Downloads/MK2/cd/mk1&2.cue", Kind::Cue),
    ("Photos", "~/Downloads/Photos", Kind::Folder),
    ("Guest tools (3dfx)", "guest-tools-3dfx.iso", Kind::Tools),
];

fn library() -> impl View {
    let discs = signal(
        LIBRARY.iter().zip(0..).map(|(&(title, path, kind), id)| Disc { id, title, path, kind }).collect::<Vec<_>>(),
    );
    let next_id = signal(LIBRARY.len() as u32);
    let size = signal(32.0_f64);
    let icons_only = signal(true);
    // Adds another of the library's discs of this kind.
    let add_disc = move |kind: Kind| {
        move || {
            let id = next_id.get_untracked();
            next_id.set(id + 1);
            let (title, path, kind) = *LIBRARY.iter().find(|(_, _, k)| *k == kind).expect("a disc of the kind");
            discs.update(|d| d.push(Disc { id, title, path, kind }));
        }
    };
    let row = move |disc: Disc| {
        let id = disc.id;
        view! {
            <Row gap=Spacing::Md align=Align::Center>
                <Icon name=icon(disc.kind) color=Color::SecondaryLabel/>
                <Column grow=1.0>
                    <Text>{disc.title}</Text>
                    <Text text_style=TextStyle::Caption color=Color::SecondaryLabel max_lines=1u32>{disc.path}</Text>
                </Column>
                <Button
                    icon=trash()
                    icon_only=icons_only
                    button_style=ButtonStyle::Borderless
                    @click=move || discs.update(|d| d.retain(|d| d.id != id))
                >
                    {format!("Remove {}", disc.title)}
                </Button>
            </Row>
        }
    };
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Lg>
            <Row gap=Spacing::Lg align=Align::Center>
                <Icon name=icon(Kind::Iso) label="CD drive" color=Color::Accent icon_size=move || size.get() as f32/>
                <Column grow=1.0>
                    <Text text_style=TextStyle::Headline>"Total Annihilation (1997)"</Text>
                    <Text color=Color::Accent>"In the drive"</Text>
                </Column>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text>"Drive icon size"</Text>
                <Slider label="Drive icon size" range_with=(16.0, 64.0) step=4.0 bind=size grow=1.0/>
                <Checkbox bind=icons_only>"Icons only"</Checkbox>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text text_style=TextStyle::Headline grow=1.0>"Library"</Text>
                <MenuButton icon=add() menu=(
                    MenuItem::new("Disc image…").on_select(add_disc(Kind::Iso)),
                    MenuItem::new("CUE sheet…").on_select(add_disc(Kind::Cue)),
                    MenuItem::new("Folder…").on_select(add_disc(Kind::Folder)),
                    MenuSeparator,
                    Menu::new("Guest tools").item(MenuItem::new("3dfx").on_select(add_disc(Kind::Tools))),
                )>"Add"</MenuButton>
            </Row>
            <Column gap=Spacing::Md>
                <For each=discs key=|d: &Disc| d.id let:disc>{row(disc)}</For>
            </Column>
        </Column>
    }
}

fn main() {
    App::new().window("Discs", Size::new(560.0, 520.0), library).run();
}
