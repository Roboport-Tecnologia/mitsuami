//! The browser: where the window is, where it's been, what's there, and the
//! actions on it, as a store.

use std::path::{Path, PathBuf};

use mitsuami::prelude::*;

use crate::fs::{self, Entry, Trash};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortBy {
    Name,
    Kind,
    Size,
    Modified,
}

/// What the browser does outside its window: where trashed items go, and
/// how files are opened. Tests swap them for ones that stay in a folder of
/// their own.
#[derive(Clone)]
pub struct Outside {
    pub trash: Trash,
    pub launch: fn(&Path) -> Result<(), String>,
}

#[derive(Clone, Copy)]
pub struct Browser {
    pub folder: Signal<PathBuf>,
    back: Signal<Vec<PathBuf>>,
    forward: Signal<Vec<PathBuf>>,
    pub listing: Resource<Vec<Entry>, String>,
    pub selected: Signal<Vec<PathBuf>>,
    pub query: Signal<String>,
    pub show_hidden: Signal<bool>,
    pub sort_by: Signal<SortBy>,
    pub descending: Signal<bool>,
    /// The item whose name is being edited.
    pub renaming: Signal<Option<PathBuf>>,
    /// Items made or copied by the last action, to select once the folder
    /// is read again.
    fresh: Signal<Vec<PathBuf>>,
    outside: Signal<Outside>,
    /// The store's scope. Its work runs there: started from a dialog's
    /// button, it would stop when the dialog closes.
    scope: Owner,
}

impl Store for Browser {
    fn create() -> Browser {
        Browser::at(fs::home(), Outside { trash: Trash::Platform, launch: fs::launch })
    }
}

impl Browser {
    pub fn at(folder: PathBuf, outside: Outside) -> Browser {
        let folder = signal(folder);
        let listing = resource_on(move || folder.get(), |folder| spawn_blocking(move || fs::read_folder(&folder)));
        let browser = Browser {
            folder,
            back: signal(Vec::new()),
            forward: signal(Vec::new()),
            listing,
            selected: signal(Vec::new()),
            query: signal(String::new()),
            show_hidden: signal(false),
            sort_by: signal(SortBy::Name),
            descending: signal(false),
            renaming: signal(None),
            fresh: signal(Vec::new()),
            outside: signal(outside),
            scope: Owner::current().expect("a browser is made in a scope"),
        };
        // Once a listing with new items arrives, they're what's selected.
        effect(move || {
            let Some(entries) = listing.data() else { return };
            let fresh = browser.fresh.get_untracked();
            if !fresh.is_empty() && fresh.iter().all(|p| entries.iter().any(|e| &e.path == p)) {
                batch(|| {
                    browser.selected.set(fresh);
                    browser.fresh.set(Vec::new());
                });
            }
        });
        browser
    }

    /// The window's title: the folder's name.
    pub fn title(&self) -> String {
        let folder = self.folder.get();
        folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| folder.display().to_string())
    }

    /// What's shown: filtered by the search and hidden files, sorted.
    pub fn entries(&self) -> Vec<Entry> {
        let query = self.query.get().to_lowercase();
        let show_hidden = self.show_hidden.get();
        let mut entries: Vec<Entry> = self.listing.data().unwrap_or_default();
        entries.retain(|e| {
            (show_hidden || !e.is_hidden()) && (query.is_empty() || e.name.to_lowercase().contains(&query))
        });
        let by_name = |a: &Entry, b: &Entry| natural(&a.name, &b.name);
        match self.sort_by.get() {
            SortBy::Name => entries.sort_by(by_name),
            SortBy::Kind => entries.sort_by(|a, b| a.kind().cmp(&b.kind()).then_with(|| by_name(a, b))),
            // Largest and newest first, as file managers sort them.
            SortBy::Size => entries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| by_name(a, b))),
            SortBy::Modified => entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| by_name(a, b))),
        }
        if self.descending.get() {
            entries.reverse();
        }
        entries
    }

    /// The selected entries that are still listed.
    pub fn selection(&self) -> Vec<Entry> {
        let selected = self.selected.get();
        self.listing.data().unwrap_or_default().into_iter().filter(|e| selected.contains(&e.path)).collect()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.with(Vec::is_empty)
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.with(Vec::is_empty)
    }

    pub fn can_go_up(&self) -> bool {
        self.folder.with(|f| f.parent().is_some())
    }

    /// Shows `folder`, remembering where the window was.
    pub fn go(&self, folder: PathBuf) {
        let from = self.folder.get_untracked();
        if folder == from {
            return;
        }
        batch(|| {
            self.back.update(|b| b.push(from.clone()));
            self.forward.set(Vec::new());
            self.show(folder, Some(from));
        });
    }

    pub fn go_back(&self) {
        let Some(to) = self.back.get_untracked().pop() else { return };
        let from = self.folder.get_untracked();
        batch(|| {
            self.back.update(|b| {
                b.pop();
            });
            self.forward.update(|f| f.push(from.clone()));
            self.show(to, Some(from));
        });
    }

    pub fn go_forward(&self) {
        let Some(to) = self.forward.get_untracked().pop() else { return };
        let from = self.folder.get_untracked();
        batch(|| {
            self.forward.update(|f| {
                f.pop();
            });
            self.back.update(|b| b.push(from.clone()));
            self.show(to, Some(from));
        });
    }

    /// The enclosing folder, with the one left selected.
    pub fn go_up(&self) {
        if let Some(parent) = self.folder.get_untracked().parent() {
            self.go(parent.to_owned());
        }
    }

    /// Shows `folder`. Coming back up from a subfolder selects the folder
    /// come through, as file managers do.
    fn show(&self, folder: PathBuf, from: Option<PathBuf>) {
        let child =
            from.and_then(|f| f.ancestors().find(|a| a.parent() == Some(folder.as_path())).map(Path::to_path_buf));
        self.selected.set(Vec::new());
        self.fresh.set(child.into_iter().collect());
        self.query.set(String::new());
        self.folder.set(folder);
    }

    /// Reads the folder again, after a change.
    pub fn reload(&self) {
        self.listing.refetch();
    }

    /// Opens entries: a folder in this window, files in their apps.
    pub fn open(&self, entries: Vec<Entry>) {
        let launch = self.outside.get_untracked().launch;
        if let [entry] = entries.as_slice()
            && entry.is_dir
            && !entry.is_app()
        {
            return self.go(entry.path.clone());
        }
        for entry in entries {
            if let Err(error) = launch(&entry.path) {
                self.fail(format!("\u{201C}{}\u{201D} couldn't be opened.", entry.name), error);
            }
        }
    }

    pub fn open_path(&self, path: &Path) {
        let listed = self.listing.data().unwrap_or_default().into_iter().find(|e| e.path == path);
        if let Some(entry) = listed {
            self.open(vec![entry]);
        }
    }

    /// Makes an untitled folder and starts renaming it.
    pub fn new_folder(&self) {
        let (browser, folder) = (*self, self.folder.get_untracked());
        self.run(async move {
            match spawn_blocking(move || fs::new_folder(&folder)).await {
                Ok(path) => {
                    browser.fresh.set(vec![path.clone()]);
                    browser.reload();
                    browser.renaming.set(Some(path));
                }
                Err(error) => browser.fail("The folder couldn't be made.", error),
            }
        });
    }

    pub fn rename(&self, path: PathBuf, name: String) {
        let browser = *self;
        self.run(async move {
            match spawn_blocking(move || fs::rename(&path, &name)).await {
                Ok(to) => {
                    browser.fresh.set(vec![to]);
                    browser.reload();
                }
                Err(error) => browser.fail("The item couldn't be renamed.", error),
            }
        });
    }

    pub fn duplicate(&self, paths: Vec<PathBuf>) {
        self.copy_here(paths, "copy");
    }

    /// Copies what was dropped into the folder shown.
    pub fn copy_in(&self, paths: Vec<PathBuf>) {
        self.copy_here(paths, "");
    }

    fn copy_here(&self, paths: Vec<PathBuf>, suffix: &'static str) {
        let (browser, folder) = (*self, self.folder.get_untracked());
        self.run(async move {
            match spawn_blocking(move || fs::copy_into(&paths, &folder, suffix)).await {
                Ok(copies) => browser.fresh.set(copies),
                Err(error) => browser.fail("The items couldn't be copied.", error),
            }
            browser.reload();
        });
    }

    /// Moves items to the trash; where there's none, asks before deleting
    /// them for good.
    pub fn trash(&self, paths: Vec<PathBuf>) {
        let browser = *self;
        let trash = self.outside.get_untracked().trash;
        self.run(async move {
            if trash.deletes() {
                let title = match paths.as_slice() {
                    [one] => {
                        format!("Delete \u{201C}{}\u{201D}?", one.file_name().unwrap_or_default().to_string_lossy())
                    }
                    many => format!("Delete {} items?", many.len()),
                };
                let confirm = Alert::new(title)
                    .message("This can't be undone.")
                    .style(AlertStyle::Warning)
                    .button("Delete")
                    .button("Cancel");
                if alert(confirm).await != 0 {
                    return;
                }
            }
            let result = spawn_blocking(move || fs::trash(&paths, &trash)).await;
            if let Err(error) = result {
                browser.fail("The items couldn't be moved to the trash.", error);
            }
            browser.selected.set(Vec::new());
            browser.reload();
        });
    }

    fn run(&self, work: impl Future<Output = ()> + 'static) {
        self.scope.with(|| spawn_local(work));
    }

    fn fail(&self, what: impl Into<String>, why: String) {
        let warning = Alert::new(what).message(why).style(AlertStyle::Warning);
        self.run(async move {
            alert(warning).await;
        });
    }
}

/// Compares names as people count: `file 2` before `file 10`, ignoring case.
pub fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let chunks = |s: &str| {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.to_lowercase().chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, chunk)) if *d == digit => chunk.push(c),
                _ => out.push((digit, c.to_string())),
            }
        }
        out
    };
    let (a, b) = (chunks(a), chunks(b));
    for ((da, ca), (db, cb)) in a.iter().zip(&b) {
        let order = if *da && *db {
            let (ta, tb) = (ca.trim_start_matches('0'), cb.trim_start_matches('0'));
            ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb))
        } else {
            ca.cmp(cb)
        };
        if order.is_ne() {
            return order;
        }
    }
    a.len().cmp(&b.len())
}
