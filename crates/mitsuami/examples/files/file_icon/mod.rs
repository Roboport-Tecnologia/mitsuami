//! A file's own icon, as the platform's file manager shows it: the icon
//! `NSWorkspace` gives on macOS (an app's own, a folder's custom one, a
//! document's by its type; `macos.rs`), and the icon GIO gives its content
//! type on GTK, from the icon theme (`gtk.rs`). WinUI's is behind the
//! shell's API (`SHGetFileInfo`), and Kirigami's needs the file's MIME
//! type, which QML can't look up, so there it's composed: a built-in icon
//! by kind.

use std::path::PathBuf;

use mitsuami::prelude::*;

#[cfg(all(target_os = "linux", not(feature = "kde")))]
mod gtk;
#[cfg(target_os = "macos")]
mod macos;

pub struct FileIcon;

#[derive(Clone, Debug, PartialEq, IntoValue)]
pub struct FileIconProps {
    pub path: PathBuf,
    /// The built-in icon's name, where the platform's isn't available.
    pub fallback: String,
    /// In points, square.
    pub size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileIconEvent {}

impl CustomWidget for FileIcon {
    const NAME: &'static str = "FileIcon";
    type Props = FileIconProps;
    type Event = FileIconEvent;

    /// A picture beside the name, which says it already.
    fn a11y(_props: &FileIconProps) -> A11yProps {
        let mut a11y = A11yProps::new(Role::Image);
        a11y.hidden = true;
        a11y
    }
}

/// The built-in icon, in a square as big as the platform's would be, so
/// names line up whatever the icon's shape.
fn composed(widget: Composed<FileIcon>) -> impl View {
    let (name, size) = (widget.clone(), widget.props().size);
    Row::new()
        .size(size, size)
        .justify(Justify::Center)
        .align(Align::Center)
        .children(Icon::new(move || name.props().fallback).icon_size(size).color(Color::Accent))
}

impl Render for FileIcon {
    fn renderer() -> Renderer<Self> {
        platform! {
            macos => mitsuami::appkit::native::<Self>().with_composed(composed),
            gtk => mitsuami::gtk::native::<Self>().with_composed(composed),
            _ => Renderer::composed(composed),
        }
    }
}
