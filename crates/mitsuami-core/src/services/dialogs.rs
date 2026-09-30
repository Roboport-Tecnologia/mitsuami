//! Alerts, file dialogs and launching: what an app asks for.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertStyle {
    #[default]
    Info,
    Warning,
    /// Destructive or irreversible consequences.
    Critical,
}

/// A message with buttons. The reply is the index of the chosen button.
#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub title: String,
    pub message: Option<String>,
    /// In order of importance: the first is the default (Return) button.
    pub buttons: Vec<String>,
    pub style: AlertStyle,
}

impl Alert {
    pub fn new(title: impl Into<String>) -> Alert {
        Alert { title: title.into(), message: None, buttons: Vec::new(), style: AlertStyle::Info }
    }

    pub fn message(mut self, message: impl Into<String>) -> Alert {
        self.message = Some(message.into());
        self
    }

    pub fn button(mut self, title: impl Into<String>) -> Alert {
        self.buttons.push(title.into());
        self
    }

    pub fn style(mut self, style: AlertStyle) -> Alert {
        self.style = style;
        self
    }

    /// The buttons to show: "OK", in the app's language, when none were
    /// given.
    pub fn effective_buttons(&self) -> Vec<String> {
        if self.buttons.is_empty() { vec![crate::l10n::tr("mitsuami-alert-ok", &[])] } else { self.buttons.clone() }
    }
}

/// What [`launch`](super::launch) opens, in the app the platform picks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    /// A file in the app set for its type, a folder in the file manager,
    /// an app itself.
    Path(PathBuf),
    /// A URL in the app set for its scheme: the browser for `https:`, the
    /// mail app for `mailto:`.
    Url(String),
}

/// Files to offer in a file dialog, e.g. `FileFilter::new("Images", ["png", "jpg"])`.
/// Where the platform lets the user choose between filters, they're offered
/// in order, the first one chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct FileFilter {
    pub name: String,
    /// Empty: every file ([`FileFilter::all`]).
    pub extensions: Vec<String>,
}

impl FileFilter {
    pub fn new<S: Into<String>>(name: impl Into<String>, extensions: impl IntoIterator<Item = S>) -> FileFilter {
        FileFilter { name: name.into(), extensions: extensions.into_iter().map(Into::into).collect() }
    }

    /// Every file, e.g. `FileFilter::all("All files")` after a filter by
    /// type, so a file the types miss can still be chosen.
    pub fn all(name: impl Into<String>) -> FileFilter {
        FileFilter { name: name.into(), extensions: Vec::new() }
    }

    /// Whether this filter lets every file through.
    pub fn is_all(&self) -> bool {
        self.extensions.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenFile {
    pub title: Option<String>,
    pub multiple: bool,
    /// Choose folders instead of files.
    pub directories: bool,
    pub filters: Vec<FileFilter>,
    /// The folder the dialog opens in. `None`, or one that isn't there,
    /// leaves it to the platform (often the last folder used).
    pub start_folder: Option<PathBuf>,
}

impl OpenFile {
    pub fn new() -> OpenFile {
        OpenFile::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> OpenFile {
        self.title = Some(title.into());
        self
    }

    pub fn multiple(mut self) -> OpenFile {
        self.multiple = true;
        self
    }

    pub fn directories(mut self) -> OpenFile {
        self.directories = true;
        self
    }

    pub fn filter(mut self, filter: FileFilter) -> OpenFile {
        self.filters.push(filter);
        self
    }

    /// Opens the dialog in `folder`, e.g. the folder of the file a field
    /// names.
    pub fn start_folder(mut self, folder: impl Into<PathBuf>) -> OpenFile {
        self.start_folder = Some(folder.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveFile {
    pub title: Option<String>,
    pub default_name: Option<String>,
    pub filters: Vec<FileFilter>,
    /// The folder the dialog opens in, as for [`OpenFile::start_folder`].
    pub start_folder: Option<PathBuf>,
}

impl SaveFile {
    pub fn new() -> SaveFile {
        SaveFile::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> SaveFile {
        self.title = Some(title.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> SaveFile {
        self.default_name = Some(name.into());
        self
    }

    pub fn filter(mut self, filter: FileFilter) -> SaveFile {
        self.filters.push(filter);
        self
    }

    /// Opens the dialog in `folder`.
    pub fn start_folder(mut self, folder: impl Into<PathBuf>) -> SaveFile {
        self.start_folder = Some(folder.into());
        self
    }
}

/// For backends: a dialog's start folder, if it's there, so a missing one
/// leaves the choice to the platform everywhere, whatever each dialog
/// would make of it.
pub fn existing_folder(folder: &Option<PathBuf>) -> Option<&Path> {
    folder.as_deref().filter(|f| f.is_dir())
}
