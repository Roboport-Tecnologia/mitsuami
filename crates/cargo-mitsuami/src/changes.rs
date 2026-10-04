//! Pending visual changes: the captures tests wrote next to their
//! baselines, `<name>.new.png`, with the diff image and the layouts.

use std::io;
use std::path::{Path, PathBuf};

/// A capture waiting for review.
#[derive(Clone, Debug)]
pub struct Change {
    /// The baseline, `<crate>/tests/visual/<backend>/<image>/<name>.png`. It
    /// doesn't exist yet for a new one.
    pub baseline: PathBuf,
    /// The path shown for it, relative to the workspace.
    pub display: String,
    pub backend: String,
    pub image: String,
    /// The test or story and its snapshot, `<file>__<test>@<snapshot>`.
    pub name: String,
}

impl Change {
    fn sibling(&self, suffix: &str) -> PathBuf {
        let stem = self.baseline.file_stem().unwrap_or_default().to_string_lossy();
        self.baseline.with_file_name(format!("{stem}{suffix}"))
    }

    pub fn capture(&self) -> PathBuf {
        self.sibling(".new.png")
    }

    pub fn diff(&self) -> PathBuf {
        self.sibling(".diff.png")
    }

    pub fn layout(&self) -> PathBuf {
        self.sibling(".layout.txt")
    }

    pub fn pending_layout(&self) -> PathBuf {
        self.sibling(".layout.txt.new")
    }

    /// The display scale the image was captured at: `macos-26@2x` is 2.
    pub fn scale(&self) -> f32 {
        self.image.rsplit_once('@').and_then(|(_, s)| s.strip_suffix('x')?.parse().ok()).unwrap_or(1.0)
    }

    pub fn is_new(&self) -> bool {
        !self.baseline.exists()
    }

    /// Moves the capture and its layout over the baseline.
    pub fn accept(&self) -> io::Result<()> {
        std::fs::rename(self.capture(), &self.baseline)?;
        if self.pending_layout().exists() {
            std::fs::rename(self.pending_layout(), self.layout())?;
        }
        remove(&self.diff())
    }

    /// Deletes the capture, keeping the baseline.
    pub fn reject(&self) -> io::Result<()> {
        remove(&self.capture())?;
        remove(&self.pending_layout())?;
        remove(&self.diff())
    }

    /// Why the pixels changed, from the layouts, as the test reported it.
    pub fn explanation(&self) -> String {
        let actual = std::fs::read_to_string(self.pending_layout()).ok();
        let expected = std::fs::read_to_string(self.layout()).ok();
        match (expected, actual) {
            _ if self.is_new() => "New baseline.".to_owned(),
            (_, None) => "No layout was recorded with the capture.".to_owned(),
            (None, Some(_)) => "The baseline has no layout recorded to compare with.".to_owned(),
            (Some(expected), Some(actual)) if expected == actual => {
                "The layout is the same: the platform draws it differently.".to_owned()
            }
            (Some(expected), Some(actual)) => {
                let diff = similar::TextDiff::from_lines(&expected, &actual);
                let mut out = String::from("The layout changed:\n");
                for change in diff.iter_all_changes() {
                    let sign = match change.tag() {
                        similar::ChangeTag::Equal => continue,
                        similar::ChangeTag::Delete => '-',
                        similar::ChangeTag::Insert => '+',
                    };
                    out.push_str(&format!("{sign} {}\n", change.value().trim_end()));
                }
                out
            }
        }
    }
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The workspace around `dir`: the nearest ancestor whose `Cargo.toml`
/// has a `[workspace]`, or else the nearest with a `Cargo.toml`.
pub fn workspace_root(dir: &Path) -> PathBuf {
    let manifest = |d: &Path| std::fs::read_to_string(d.join("Cargo.toml")).ok();
    dir.ancestors()
        .find(|d| manifest(d).is_some_and(|m| m.lines().any(|l| l.trim() == "[workspace]")))
        .or_else(|| dir.ancestors().find(|d| manifest(d).is_some()))
        .unwrap_or(dir)
        .to_path_buf()
}

/// Every pending change under `root`, sorted by path.
pub fn find(root: &Path) -> Vec<Change> {
    let mut changes = Vec::new();
    walk(root, root, &mut changes);
    changes.sort_by(|a, b| a.display.cmp(&b.display));
    changes
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Change>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // Not following links, which could lead back up the tree.
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                walk(root, &path, out);
            }
        } else if let Some(stem) = name.strip_suffix(".new.png")
            && let Some(change) = change(root, &path.with_file_name(format!("{stem}.png")))
        {
            out.push(change);
        }
    }
}

/// A change for `baseline`, if it is under `tests/visual/<backend>/<image>/`.
fn change(root: &Path, baseline: &Path) -> Option<Change> {
    let relative = baseline.strip_prefix(root).unwrap_or(baseline);
    let parts: Vec<String> = relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let visual = parts.windows(2).position(|w| w[0] == "tests" && w[1] == "visual")?;
    // tests/visual/<backend>/<image>/<name>.png
    let [backend, image, _file] = &parts[visual + 2..] else { return None };
    Some(Change {
        baseline: baseline.to_path_buf(),
        display: parts.join("/"),
        backend: backend.clone(),
        image: image.clone(),
        name: baseline.file_stem()?.to_string_lossy().into_owned(),
    })
}
