//! Focus from code: `cargo run -p mitsuami --example focus`.
//!
//! - A form's buttons focus a field, as a click or Tab would: the one
//!   left empty, say, after a failed Save.
//! - A file name's field, with buttons that select its name without the
//!   extension (as file managers do to rename), the extension, or all of
//!   it. What's typed then replaces the selection. Characters are counted
//!   as people see them, so the emoji in the name is one.
//! - Rename… opens a dialog whose field is focused with the name selected
//!   as it opens: asked in `on_open`, before the field is built, it's done
//!   once it is.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;
use mitsuami::unicode_segmentation::UnicodeSegmentation;

/// A name's length in characters, and where its extension starts.
fn parts(name: &str) -> (usize, usize) {
    let len = name.graphemes(true).count();
    let stem = name.rfind('.').filter(|&dot| dot > 0).map_or(len, |dot| name[..dot].graphemes(true).count());
    (len, stem)
}

fn form() -> impl View {
    let (name, email) = (signal(String::new()), signal(String::new()));
    let (name_field, email_field) = (node_ref(), node_ref());
    let status = signal(String::new());
    let save = move || {
        if name.get_untracked().trim().is_empty() {
            status.set("A name is needed.".into());
            name_field.focus();
        } else if email.get_untracked().trim().is_empty() {
            status.set("An email address is needed.".into());
            email_field.focus();
        } else {
            status.set("Saved.".into());
        }
    };
    view! {
        <Group title="Focus a field">
            <Column gap=Spacing::Sm>
                <TextInput bind=name a11y_label="Name" placeholder="Name" node_ref=name_field/>
                <TextInput bind=email a11y_label="Email" placeholder="Email" node_ref=email_field/>
                <Row gap=Spacing::Sm align=Align::Center>
                    <Button @click=move || name_field.focus()>"Focus Name"</Button>
                    <Button @click=move || email_field.focus()>"Focus Email"</Button>
                    <Button role=ButtonRole::Default @click=save>"Save"</Button>
                    <Text color=Color::SecondaryLabel>{status}</Text>
                </Row>
            </Column>
        </Group>
    }
}

fn selection() -> impl View {
    let file = signal("🌅 Sunrise.2026.jpeg".to_owned());
    let field = node_ref();
    let stem = move || {
        let (_, stem) = parts(&file.get_untracked());
        field.select_text(0..stem);
    };
    let extension = move || {
        let (len, stem) = parts(&file.get_untracked());
        field.select_text((stem + 1).min(len)..len);
    };
    view! {
        <Group title="Select part of a field">
            <Column gap=Spacing::Sm>
                <TextInput bind=file a11y_label="File name" node_ref=field/>
                <Row gap=Spacing::Sm>
                    <Button @click=stem>"Select Name"</Button>
                    <Button @click=extension>"Select Extension"</Button>
                    <Button @click=move || field.select_text(0..usize::MAX)>"Select All"</Button>
                </Row>
            </Column>
        </Group>
    }
}

fn dialog() -> impl View {
    let file = signal("Quarterly report.pdf".to_owned());
    let (open, draft, field) = (signal(false), signal(String::new()), node_ref());
    let start = move || {
        let name = file.get_untracked();
        field.select_text(0..parts(&name).1);
        draft.set(name);
    };
    let commit = move || {
        open.set(false);
        file.set(draft.get_untracked());
    };
    view! {
        <Group title="Focus as a dialog opens">
            <Row gap=Spacing::Md align=Align::Center>
                <Text grow=1.0>{file}</Text>
                <Button @click=move || open.set(true)>"Rename…"</Button>
                <Window title="Rename" bind=open modal=Modality::Window size=WindowSize::FitHeight(360.0) @open=start>
                    <Column padding=Spacing::Xl gap=Spacing::Md>
                        <Text>"New name:"</Text>
                        <TextInput bind=draft a11y_label="New name" node_ref=field @submit=commit/>
                        <Row gap=Spacing::Sm justify=Justify::End>
                            <Button role=ButtonRole::Cancel @click=move || open.set(false)>"Cancel"</Button>
                            <Button role=ButtonRole::Default enabled=move || !draft.get().trim().is_empty() @click=commit>
                                "Rename"
                            </Button>
                        </Row>
                    </Column>
                </Window>
            </Row>
        </Group>
    }
}

pub fn page() -> impl View {
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Lg>
            {form()}
            {selection()}
            {dialog()}
        </Column>
    }
}

fn main() {
    App::new().window("Focus", WindowSize::FitHeight(460.0), page).run();
}
