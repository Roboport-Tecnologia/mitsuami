//! Icons: `cargo run -p mitsuami --example icon`.
//!
//! A disc library, as an emulator's disc picker lists its images: each
//! row has an icon from the platform's own set, and a button that shows
//! only its icon.
//!
//! - The icons are each platform's own: SF Symbols on macOS, the icon
//!   theme's on Linux, Segoe Fluent Icons on Windows.
//! - The drive is in a group, the platform's box around related content,
//!   under its heading.
//! - Move the slider: the drive's icon grows, as the platform sizes icons.
//! - The drive's icon is the accent colour and the rows' the secondary
//!   one: the platform's own, so they follow dark mode, high contrast and
//!   the user's accent. Switch to dark mode to see them follow.
//! - Turn off "Icons only": the row buttons show their caption beside the
//!   icon. VoiceOver, Orca, Narrator and friends read the caption either way.
//! - Drag .iso and .cue files or folders from the file manager onto the
//!   box at the bottom: they join the library. Other files are refused.
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

/// Segoe Fluent Icons has no eject glyph: the button shows its caption.
fn eject() -> &'static str {
    platform! {
        macos => "eject",
        gtk => "media-eject-symbolic",
        kde => "media-eject",
        windows => "",
    }
}

fn download() -> &'static str {
    platform! {
        macos => "square.and.arrow.down",
        gtk => "folder-download-symbolic",
        kde => "download",
        windows => "\u{E896}",
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
    title: String,
    path: String,
    kind: Kind,
}

const LIBRARY: [(&str, &str, Kind); 4] = [
    ("Total Annihilation (1997)", "~/Downloads/Total_Annihilation_1997.iso", Kind::Iso),
    ("Mortal Kombat 1 & 2", "~/Downloads/MK2/cd/mk1&2.cue", Kind::Cue),
    ("Photos", "~/Downloads/Photos", Kind::Folder),
    ("Guest tools (3dfx)", "guest-tools-3dfx.iso", Kind::Tools),
];

pub fn library() -> impl View {
    let discs = signal(
        LIBRARY
            .iter()
            .zip(0..)
            .map(|(&(title, path, kind), id)| Disc { id, title: title.into(), path: path.into(), kind })
            .collect::<Vec<_>>(),
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
            discs.update(|d| d.push(Disc { id, title: title.into(), path: path.into(), kind }));
        }
    };
    // Discs dropped from the file manager, named after their files.
    let add_dropped = move |paths: Vec<std::path::PathBuf>| {
        for path in paths {
            let id = next_id.get_untracked();
            next_id.set(id + 1);
            let kind = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
                Some("cue") => Kind::Cue,
                _ if path.is_dir() => Kind::Folder,
                _ => Kind::Iso,
            };
            let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            discs.update(|d| d.push(Disc { id, title, path: path.display().to_string(), kind }));
        }
    };
    let dropping = signal(false);
    let row = move |disc: Disc| {
        let id = disc.id;
        view! {
            <Row gap=Spacing::Md align=Align::Center>
                <Icon name=icon(disc.kind) color=Color::SecondaryLabel/>
                <Column grow=1.0>
                    <Text>{disc.title.clone()}</Text>
                    <Text text_style=TextStyle::Caption color=Color::SecondaryLabel max_lines=1u32>{disc.path.clone()}</Text>
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
            <Group title="CD drive">
                <Row gap=Spacing::Lg align=Align::Center>
                    <Icon name=icon(Kind::Iso) label="Disc" color=Color::Accent icon_size=move || size.get() as f32/>
                    <Column grow=1.0>
                        <Text text_style=TextStyle::Headline>"Total Annihilation (1997)"</Text>
                        <Text color=Color::Accent>"In the drive"</Text>
                    </Column>
                    <Button icon=eject()>"Eject"</Button>
                </Row>
            </Group>
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
            <Group
                a11y_label="Drop discs"
                file_drop=FileDrop::extensions(["iso", "cue"]).and_folders()
                @drop=add_dropped
                @drop_hover=move |over| dropping.set(over)
            >
                <Row gap=Spacing::Sm justify=Justify::Center>
                    <Icon name=download() color=Color::SecondaryLabel/>
                    <Text color=Color::SecondaryLabel>
                        {move || if dropping.get() { "Release to add" } else { "Drop .iso, .cue files or folders here" }.to_owned()}
                    </Text>
                </Row>
            </Group>
        </Column>
    }
}

fn main() {
    App::new().window("Discs", Size::new(560.0, 520.0), library).run();
}
