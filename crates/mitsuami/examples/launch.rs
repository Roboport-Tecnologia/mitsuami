//! Opening files and links in other apps: `cargo run -p mitsuami --example launch`.
//!
//! - The window makes a text file, a web page, a file of a type no app
//!   opens, and a folder, in a folder of its own (`mitsuami launch
//!   example`, in the user's cache folder on Linux, the temporary folder
//!   elsewhere). Each Open button opens one in the app the platform picks
//!   for it, as a double-click in its file manager would: the text editor,
//!   the browser, the file manager.
//! - The platform decides what happens when no app is set for a type:
//!   macOS says so and offers to choose one, GNOME and Windows ask which
//!   app. Dismissing that is reported as cancelled.
//! - Open Link opens the address in the app set for its scheme: the
//!   browser for `https:`, the mail app for `mailto:`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use mitsuami::prelude::*;

/// The folder the example's files go in. Linux's temporary folder is shared
/// by every user, who could make this folder first and plant links in it,
/// so there it's the user's cache folder; macOS's and Windows' are per user.
pub fn folder() -> PathBuf {
    #[cfg(not(any(target_os = "macos", windows)))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".cache"));
    #[cfg(any(target_os = "macos", windows))]
    let base = std::env::temp_dir();
    base.join("mitsuami launch example")
}

/// The things to open, made if they're missing: title and path.
fn things() -> Vec<(&'static str, PathBuf)> {
    let folder = folder();
    let files = [
        ("Text file", "Notes.txt", "Opened from mitsuami.\n"),
        ("Web page", "Page.html", "<!doctype html><title>mitsuami</title><h1>Opened from mitsuami</h1>\n"),
        ("Unknown type", "Mystery.mitsuami-unknown", "No app opens this.\n"),
    ];
    _ = std::fs::create_dir_all(folder.join("A folder"));
    let mut things: Vec<(&'static str, PathBuf)> = files
        .iter()
        .map(|(title, name, text)| {
            let path = folder.join(name);
            if !path.exists() {
                _ = std::fs::write(&path, text);
            }
            (*title, path)
        })
        .collect();
    things.push(("Folder", folder.join("A folder")));
    things
}

fn outcome(what: &str, result: Result<(), ServiceError>) -> String {
    match result {
        Ok(()) => format!("Opened {what}."),
        Err(ServiceError::Cancelled) => format!("{what}: cancelled."),
        Err(ServiceError::Unavailable) => format!("No app opens {what}."),
        Err(error) => format!("{what} couldn't be opened: {error}"),
    }
}

fn thing(title: &'static str, path: PathBuf, status: Signal<String>) -> impl View {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let open = move || {
        let what = name.clone();
        let opening = launch(&path);
        spawn_local(async move { status.set(outcome(&what, opening.await)) });
    };
    view! {
        <Row gap=Spacing::Md align=Align::Center>
            <Text grow=1.0>{title}</Text>
            <Button a11y_label=format!("Open {title}") @click=open>"Open"</Button>
        </Row>
    }
}

pub fn page() -> impl View {
    let status = signal(String::new());
    let link = signal("https://www.rust-lang.org".to_string());
    let open_link = move || {
        let url = link.get_untracked();
        let opening = launch_url(&url);
        spawn_local(async move { status.set(outcome(&url, opening.await)) });
    };
    let rows = things().into_iter().map(|(title, path)| thing(title, path, status)).collect::<Vec<_>>();
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Text color=Color::SecondaryLabel>{format!("In {}", folder().display())}</Text>
            <Group title="Files">
                <Column gap=Spacing::Sm>{rows}</Column>
            </Group>
            <Group title="Links">
                <Row gap=Spacing::Sm align=Align::Center>
                    <TextInput a11y_label="Address" bind=link grow=1.0/>
                    <Button enabled=move || !link.get().trim().is_empty() @click=open_link>"Open Link"</Button>
                </Row>
            </Group>
            <Text test_id="status">{status}</Text>
        </Column>
    }
}

fn main() {
    App::new().window("Open in another app", WindowSize::FitHeight(460.0), page).run();
}
