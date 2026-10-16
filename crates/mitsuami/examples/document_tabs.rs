//! Document tabs: `cargo run -p mitsuami --example document_tabs`.
//!
//! A notepad with a tab for each open note, and one text area that shows
//! the chosen note, as a terminal draws its sessions on one surface: the
//! tabs have no pages.
//!
//! - Pick a tab: the area shows its note. Type in it, pick another and
//!   come back: the note kept what was typed.
//! - Close a tab with its close button (or a middle click, where the
//!   platform has one): a note with changes asks first, here by marking
//!   its tab, and goes on the second close.
//! - Drag a tab to another place: the notes' order follows.
//! - The new-tab button opens an empty note, and chooses it.
//! - "Tabs" picks where they show: the platform's place (the title bar on
//!   macOS, as Safari's and Xcode's, and on Windows; a strip on GNOME and
//!   KDE), a strip above the area everywhere, the window's (a strip on
//!   GNOME and KDE, which have none), or the title's row: on macOS beside
//!   the window's buttons with the title hidden, as Safari's compact tabs
//!   (the header bar's title on GNOME, the window's title bar on Windows,
//!   a strip on KDE).
//! - In a title bar, WinUI puts the tabs on Mica Alt, as File Explorer
//!   has them: tinted by the wallpaper more strongly than Mica.
//! - A raw platform setting, through `.native()`: on Windows, "Chosen tab
//!   in the content's colour" gives the chosen tab Mica's solid colour,
//!   as Terminal's takes its terminal's. Colours are each
//!   platform's own.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct Note {
    id: u32,
    name: String,
    text: String,
    /// Changed since it was opened.
    edited: bool,
    /// Asked to close once with changes: the next close goes.
    asked: bool,
}

impl Note {
    fn new(id: u32, name: &str, text: &str) -> Note {
        Note { id, name: name.into(), text: text.into(), edited: false, asked: false }
    }

    /// Its tab's title: marked while it has changes, as editors mark them.
    fn title(&self) -> String {
        if self.asked {
            format!("{} (close again to discard)", self.name)
        } else if self.edited {
            format!("• {}", self.name)
        } else {
            self.name.clone()
        }
    }
}

pub fn notepad() -> impl View {
    let notes = signal(vec![
        Note::new(1, "Groceries", "Milk\nEggs\nBread\n"),
        Note::new(2, "Ideas", "A notepad with tabs.\n"),
        Note::new(3, "Todo", "Write the docs.\n"),
    ]);
    let current = signal(1);
    let next = signal(4);
    let style = signal(0);
    let styles = ["The platform's place", "A strip", "The window's", "The title's row"];
    let tabs_style = move || match style.get() {
        1 => DocumentTabsStyle::Strip,
        2 => DocumentTabsStyle::Window,
        3 => DocumentTabsStyle::TitleRow,
        _ => DocumentTabsStyle::Automatic,
    };
    let close = move |id: u32| {
        let ask = notes.with(|n| n.iter().any(|n| n.id == id && n.edited && !n.asked));
        if ask {
            notes.update(|n| n.iter_mut().filter(|n| n.id == id).for_each(|n| n.asked = true));
        } else {
            notes.update(|n| n.retain(|n| n.id != id));
        }
    };
    let reorder = move |id: u32, to: usize| {
        notes.update(|n| {
            if let Some(from) = n.iter().position(|n| n.id == id) {
                let note = n.remove(from);
                n.insert(to.min(n.len()), note);
            }
        })
    };
    let new_tab = move || {
        let id = next.get_untracked();
        next.set(id + 1);
        notes.update(|n| n.push(Note::new(id, &format!("Untitled {}", id - 3), "")));
        current.set(id);
    };
    let (tweakable, about) = platform! {
        windows => (true, "WinUI: the chosen tab and the title bar in the window's background, as Terminal's tab takes its terminal's colour."),
        _ => (false, ""),
    };
    let tweaked = signal(false);
    let tabs = move |tweak: Tweak<DocumentTabs<u32>>| {
        DocumentTabs::new(current)
            .tabs_style(tabs_style)
            .children_with(move || {
                notes.with(|n| n.iter().map(|n| DocumentTab::new(n.title(), n.id)).collect::<Vec<_>>())
            })
            .on_close(close)
            .on_reorder(reorder)
            .on_new_tab(new_tab)
            .native(tweak)
    };
    let text = move || {
        let id = current.get();
        notes.with(|n| n.iter().find(|n| n.id == id).map(|n| n.text.clone()).unwrap_or_default())
    };
    let typed = move |typed: String| {
        let id = current.get_untracked();
        notes.update(|n| {
            if let Some(note) = n.iter_mut().find(|n| n.id == id)
                && note.text != typed
            {
                note.text = typed;
                note.edited = true;
                note.asked = false;
            }
        })
    };
    let empty = move || notes.with(|n| n.is_empty());
    view! {
        // No gap after the tabs: where they're the window's, they take no
        // room here.
        <Column grow=1.0>
            // Tweaks apply as the tabs are made: made again to change.
            <Show when=move || !tweaked.get()>{tabs(Tweak::none())}</Show>
            <Show when=move || tweaked.get()>{tabs(tab_in_content_colour())}</Show>
            <Column grow=1.0 gap=Spacing::Md>
                <Show when=move || !empty()>
                    <TextArea a11y_label="Note" value=text grow=1.0 @input=typed/>
                </Show>
                <Show when=empty>
                    <Text>"No notes open: the new-tab button opens one."</Text>
                </Show>
                <Row gap=Spacing::Md align=Align::Center>
                    <Text>"Tabs"</Text>
                    <Select label="Tabs" options=styles bind=style/>
                    <Show when=move || tweakable>
                        <Row gap=Spacing::Sm align=Align::Center>
                            <Text>"Chosen tab in the content's colour"</Text>
                            <Switch bind=tweaked>"Chosen tab in the content's colour"</Switch>
                        </Row>
                    </Show>
                </Row>
                <Show when=move || tweaked.get()>
                    <Text text_style=TextStyle::Caption>{about}</Text>
                </Show>
            </Column>
        </Column>
    }
}

/// On WinUI, the chosen tab's and the strip's theme resources in Mica's
/// solid colour (an inactive window's background), light and dark. The
/// others keep theirs.
fn tab_in_content_colour() -> Tweak<DocumentTabs<u32>> {
    platform! {
        windows => mitsuami::winui::tweak(|view: &mitsuami::winui::bindings::TabView| {
            use mitsuami::winui::bindings::{IFrameworkElement, ResourceDictionary, XamlReader};
            use mitsuami::winui::windows_core::Interface;
            // The chosen tab, and the strip (in the title bar, the band
            // behind it), on Mica's solid colour.
            let selected = r#"<StaticResource x:Key="TabViewItemHeaderBackgroundSelected" ResourceKey="SolidBackgroundFillColorBaseBrush"/>
                <StaticResource x:Key="TabViewBackground" ResourceKey="SolidBackgroundFillColorBaseBrush"/>"#;
            let markup = format!(
                r#"<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
                  <ResourceDictionary.ThemeDictionaries>
                    <ResourceDictionary x:Key="Default">{selected}</ResourceDictionary>
                    <ResourceDictionary x:Key="Light">{selected}</ResourceDictionary>
                  </ResourceDictionary.ThemeDictionaries>
                </ResourceDictionary>"#
            );
            let colours: ResourceDictionary = XamlReader::Load(&markup)?.cast()?;
            view.cast::<IFrameworkElement>()?.Resources()?.MergedDictionaries()?.Append(&colours)
        }),
        _ => Tweak::none(),
    }
}

fn main() {
    App::new().window("Notes", Size::new(640.0, 420.0), notepad).run();
}
