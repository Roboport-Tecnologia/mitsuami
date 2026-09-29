//! The path to the folder shown, each folder in it a way back up. macOS
//! has the control: `NSPathControl`, Finder's own path bar (`macos.rs`),
//! which shortens a deep path itself. So has WinUI: `BreadcrumbBar`, File
//! Explorer's address bar (`windows.rs`). GTK and Kirigami have none, so
//! there it's composed as Nautilus and Dolphin build theirs: a button per
//! folder, the folder shown last, and the folders that don't fit in a
//! menu at the start.

use std::path::{Path, PathBuf};

use mitsuami::prelude::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub struct PathBar;

#[derive(Clone, Debug, PartialEq)]
pub struct PathBarProps {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PathBarEvent {
    /// A folder in the path was chosen; the app decides whether to go.
    Chosen(PathBuf),
}

impl CustomWidget for PathBar {
    const NAME: &'static str = "PathBar";
    type Props = PathBarProps;
    type Event = PathBarEvent;

    fn a11y(props: &PathBarProps) -> A11yProps {
        A11yProps::new(Role::Group).value(props.path.display().to_string())
    }

    /// Setting its value to a folder on the path chooses that folder, as a
    /// screen reader choosing one of its items does.
    fn action(props: &PathBarProps, action: &A11yAction) -> Option<PathBarEvent> {
        let A11yAction::SetValue(path) = action else { return None };
        let path = PathBuf::from(path);
        props.path.starts_with(&path).then_some(PathBarEvent::Chosen(path))
    }
}

/// The folders on the way to `path`, the root first.
pub fn folders(path: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = path.ancestors().map(Path::to_path_buf).collect();
    folders.reverse();
    folders
}

/// A folder's name, as a path bar shows it: the root by its path.
pub fn name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// How many folders show as buttons before the rest go in the menu.
const SHOWN: usize = 4;

/// Nautilus's and Dolphin's: the folders as buttons, the one shown as a
/// label, and a menu for the ones before the last few.
fn composed(widget: Composed<PathBar>) -> impl View {
    let label = widget.label().unwrap_or_default();
    let split = {
        let widget = widget.clone();
        move || {
            let mut shown = folders(&widget.props().path);
            let hidden: Vec<PathBuf> = shown.drain(..shown.len().saturating_sub(SHOWN)).collect();
            (hidden, shown)
        }
    };
    let (hidden, shown) = (split.clone(), split);
    let (more, crumb) = (widget.clone(), widget);
    Row::new().a11y_label(label).align(Align::Center).min_width(0).children((
        // A menu button's items are built with it, so it's built again when
        // they change: keyed by them, one or none.
        For::new(
            move || Some(hidden().0).filter(|h| !h.is_empty()).into_iter().collect::<Vec<_>>(),
            Vec::clone,
            move |hidden: Vec<PathBuf>| {
                let items: Vec<MenuItem> = hidden
                    .into_iter()
                    .map(|folder| {
                        let widget = more.clone();
                        MenuItem::new(name(&folder))
                            .on_select(move || widget.emit(PathBarEvent::Chosen(folder.clone())))
                    })
                    .collect();
                MenuButton::new("\u{2026}")
                    .a11y_label("Folders above")
                    .button_style(ButtonStyle::Borderless)
                    .menu(items)
            },
        ),
        For::new(
            move || shown().1,
            PathBuf::clone,
            move |folder: PathBuf| {
                let widget = crumb.clone();
                let last = {
                    let (widget, folder) = (widget.clone(), folder.clone());
                    move || widget.props().path == folder
                };
                let here = {
                    let folder = folder.clone();
                    move || Text::new(name(&folder)).weight(FontWeight::Semibold).max_lines(1).padding_x(Spacing::Sm)
                };
                let button = move || {
                    let (widget, folder) = (widget.clone(), folder.clone());
                    Row::new().align(Align::Center).children((
                        Button::new(name(&folder))
                            .button_style(ButtonStyle::Borderless)
                            .on_click(move || widget.emit(PathBarEvent::Chosen(folder.clone()))),
                        Text::new("\u{203A}").text_style(TextStyle::Caption),
                    ))
                };
                Show::new(last, here).fallback(button)
            },
        ),
    ))
}

impl Render for PathBar {
    fn renderer() -> Renderer<Self> {
        platform! {
            macos => mitsuami::appkit::native::<Self>().with_composed(composed),
            windows => mitsuami::winui::native::<Self>().with_composed(composed),
            _ => Renderer::composed(composed),
        }
    }
}
