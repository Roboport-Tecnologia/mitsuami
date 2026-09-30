//! The browser: where the window is, where it's been, what's there, and the
//! actions on it, as a store.

use std::path::{Path, PathBuf};

use mitsuami::prelude::*;

use crate::fs::{self, Entry};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortBy {
    Name,
    Kind,
    Size,
    Modified,
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
    /// The column and order, as the table's header shows them.
    pub sort: Signal<Sort<SortBy>>,
    /// The column alone, for the menus' choices.
    pub sort_by: Signal<SortBy>,
    /// The item whose name is being edited.
    pub renaming: Signal<Option<PathBuf>>,
    /// Items made or copied by the last action, to select once the folder
    /// is read again.
    fresh: Signal<Vec<PathBuf>>,
}

impl Store for Browser {
    fn create() -> Browser {
        Browser::at(fs::home())
    }
}

impl Browser {
    pub fn at(folder: PathBuf) -> Browser {
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
            sort: signal(Sort::ascending(SortBy::Name)),
            sort_by: signal(SortBy::Name),
            renaming: signal(None),
            fresh: signal(Vec::new()),
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
        // The header sets the column and order, the menus the column: another
        // one sorts ascending, as its header does.
        watch(move || browser.sort.get().by, move |by, _| browser.sort_by.set(*by));
        watch(
            move || browser.sort_by.get(),
            move |by, _| {
                if browser.sort.get_untracked().by != *by {
                    browser.sort.set(Sort::ascending(*by));
                }
            },
        );
        browser
    }

    /// Sorts the other way round, as the menus' Reversed does.
    pub fn reverse(&self) {
        self.sort.update(|s| {
            s.order = match s.order {
                SortOrder::Ascending => SortOrder::Descending,
                SortOrder::Descending => SortOrder::Ascending,
            }
        });
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
        let Sort { by, order } = self.sort.get();
        match by {
            SortBy::Name => entries.sort_by(by_name),
            SortBy::Kind => entries.sort_by(|a, b| a.kind().cmp(&b.kind()).then_with(|| by_name(a, b))),
            // Folders have no size: they come first.
            SortBy::Size => entries.sort_by(|a, b| a.size.cmp(&b.size).then_with(|| by_name(a, b))),
            SortBy::Modified => entries.sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| by_name(a, b))),
        }
        if order == SortOrder::Descending {
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

    /// Opens entries: a folder in this window, files in their apps. Where
    /// no app is set, the platform says so or asks which (except KDE's,
    /// which only fails: then the browser says so).
    pub fn open(&self, entries: Vec<Entry>) {
        if let [entry] = entries.as_slice()
            && entry.is_dir
            && !entry.is_app()
        {
            return self.go(entry.path.clone());
        }
        let browser = *self;
        for entry in entries {
            let opening = launch(&entry.path);
            self.spawn(async move {
                let why = match opening.await {
                    Ok(()) | Err(ServiceError::Cancelled) => return,
                    Err(ServiceError::Unavailable) => "No app is set to open it.".to_string(),
                    Err(error) => error.to_string(),
                };
                browser.fail(format!("\u{201C}{}\u{201D} couldn't be opened.", entry.name), why);
            });
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
        self.spawn(async move {
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
        self.spawn(async move {
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

    /// Copies what was dropped into the folder shown. Items dropped where
    /// they already are (dragged from this list) stay as they are, as in
    /// file managers.
    pub fn copy_in(&self, paths: Vec<PathBuf>) {
        let folder = self.folder.get_untracked();
        let paths: Vec<PathBuf> = paths.into_iter().filter(|p| p.parent() != Some(folder.as_path())).collect();
        if !paths.is_empty() {
            self.copy_here(paths, "");
        }
    }

    fn copy_here(&self, paths: Vec<PathBuf>, suffix: &'static str) {
        let (browser, folder) = (*self, self.folder.get_untracked());
        self.spawn(async move {
            match spawn_blocking(move || fs::copy_into(&paths, &folder, suffix)).await {
                Ok(copies) => browser.fresh.set(copies),
                Err(error) => browser.fail("The items couldn't be copied.", error),
            }
            browser.reload();
        });
    }

    /// Moves items to the platform's trash; where their disk has none,
    /// asks before deleting them for good, as Finder and Files do.
    pub fn trash(&self, paths: Vec<PathBuf>) {
        let browser = *self;
        self.spawn(async move {
            match trash(paths.clone()).await {
                Ok(()) | Err(ServiceError::Cancelled) => {}
                // Those before the one that failed are in the trash.
                Err(ServiceError::Unavailable) => {
                    browser.delete(paths.into_iter().filter(|p| p.symlink_metadata().is_ok()).collect()).await
                }
                Err(error) => browser.fail("The items couldn't be moved to the trash.", error.to_string()),
            }
            browser.selected.set(Vec::new());
            browser.reload();
        });
    }

    async fn delete(&self, paths: Vec<PathBuf>) {
        let title = match paths.as_slice() {
            [one] => {
                format!("Delete \u{201C}{}\u{201D} immediately?", one.file_name().unwrap_or_default().to_string_lossy())
            }
            many => format!("Delete {} items immediately?", many.len()),
        };
        let confirm = Alert::new(title)
            .message("This disk has no trash. You can't undo this action.")
            .style(AlertStyle::Warning)
            .button("Delete")
            .button("Cancel");
        if alert(confirm).await != 0 {
            return;
        }
        if let Err(error) = spawn_blocking(move || fs::delete(&paths)).await {
            self.fail("The items couldn't be deleted.", error);
        }
    }

    fn fail(&self, what: impl Into<String>, why: String) {
        let warning = Alert::new(what).message(why).style(AlertStyle::Warning);
        self.spawn(async move {
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
