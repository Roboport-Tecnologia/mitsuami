//! Later launches of a single-instance app, handed to the running one.

use std::path::{Path, PathBuf};

/// A launch of a single-instance app (`App::single_instance`) while it's
/// already running: the platform or the second process hands it to the
/// first, and [`on_reopen`](super::on_reopen) handlers get it there.
/// What's in it is what the platform passes on: macOS gives no arguments
/// or folder to a running app, and Windows no folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reopen {
    args: Vec<String>,
    cwd: Option<PathBuf>,
    files: Vec<PathBuf>,
}

impl Reopen {
    /// What backends (and tests) hand the core: the launch's arguments
    /// without the program, its working folder, and the files the platform
    /// opened the app with.
    pub fn new(args: Vec<String>, cwd: Option<PathBuf>, files: Vec<PathBuf>) -> Reopen {
        Reopen { args, cwd, files }
    }

    /// The arguments the app was launched with, without `argv[0]`. Empty
    /// for a launch from the platform's shell (macOS's Dock and Finder).
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The folder the launch ran in, to resolve relative paths in
    /// [`args`](Self::args) against, where the platform passes it on.
    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    /// Files the platform opened the app with: Finder's Open With on macOS,
    /// a D-Bus activated `.desktop` file's `%F` on Linux. A plain launch
    /// passes paths as arguments instead.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
}
