//! File snapshots under `<crate>/tests/snapshots/`.
//!
//! - Snapshots of the native backends depend on the machine (fonts, OS
//!   version, display scale), so they live in `<backend>/<image>/` (see
//!   [`image`]); headless ones, and those the core computes, at the top.
//! - A missing snapshot is written and the assertion passes, except on CI
//!   (`CI` set), where it fails.
//! - Failures don't stop the test: it fails when it ends, with every
//!   snapshot that failed, so one run writes them all for review.
//! - A mismatch fails with a diff. The actual text is written to
//!   `<file>.new` next to the snapshot for review, on CI too, which uploads
//!   them (`.github/scripts/accept-snapshots.sh` accepts them).
//! - `MITSUAMI_UPDATE_SNAPSHOTS=1` accepts every mismatch instead.
//! - `MITSUAMI_SKIP_MACHINE_SNAPSHOTS=1` skips the native backends'
//!   snapshots and visual baselines.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use similar::TextDiff;

use crate::app::TestContext;

/// Whether snapshots and visual baselines that depend on the machine are
/// skipped (`MITSUAMI_SKIP_MACHINE_SNAPSHOTS=1`).
pub(crate) fn skip_machine_snapshots() -> bool {
    std::env::var("MITSUAMI_SKIP_MACHINE_SNAPSHOTS").is_ok_and(|v| v == "1" || v == "true")
}

pub(crate) fn updating() -> bool {
    std::env::var("MITSUAMI_UPDATE_SNAPSHOTS").is_ok_and(|v| v == "1" || v == "true")
}

pub(crate) fn on_ci() -> bool {
    std::env::var_os("CI").is_some()
}

/// The machine image that native snapshots and baselines belong to:
/// `MITSUAMI_IMAGE` if set (CI sets it to the runner's image, like
/// `macos-26`), or else the OS and its version (`macos-26`, `ubuntu-24.04`,
/// `windows-26100`). A display scale other than 1× is appended
/// (`macos-26@2x`): captures are in physical pixels.
pub(crate) fn image(scale_factor: f32) -> String {
    static OS: OnceLock<String> = OnceLock::new();
    let os = OS.get_or_init(|| {
        std::env::var("MITSUAMI_IMAGE")
            .ok()
            .filter(|i| !i.is_empty())
            .map(|i| sanitize_image(&i))
            .unwrap_or_else(os_image)
    });
    if scale_factor == 1.0 { os.clone() } else { format!("{os}@{scale_factor}x") }
}

#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program).args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(target_os = "macos")]
fn os_image() -> String {
    // Major versions change how controls look; minor ones rarely do.
    let version = command_output("sw_vers", &["-productVersion"]).unwrap_or_default();
    format!("macos-{}", version.split('.').next().unwrap_or("unknown"))
}

#[cfg(target_os = "linux")]
fn os_image() -> String {
    // `ubuntu-24.04`, as GitHub names its runner images.
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let field = |key: &str| {
        release
            .lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim_matches('"').to_owned())
            .unwrap_or_else(|| "unknown".into())
    };
    sanitize_image(&format!("{}-{}", field("ID"), field("VERSION_ID")))
}

#[cfg(windows)]
fn os_image() -> String {
    // "Microsoft Windows [Version 10.0.26100.4652]": the build number.
    let ver = command_output("cmd", &["/c", "ver"]).unwrap_or_default();
    let build = ver.rsplit("Version ").next().and_then(|v| v.split('.').nth(2)).unwrap_or("unknown");
    format!("windows-{build}")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn os_image() -> String {
    std::env::consts::OS.to_owned()
}

pub(crate) fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Like [`sanitize`], keeping dots for versions (`ubuntu-24.04`).
fn sanitize_image(s: &str) -> String {
    s.split('.').map(sanitize).collect::<Vec<_>>().join(".")
}

/// `machine` is `<backend>/<image>` for the native backends' snapshots.
#[track_caller]
pub(crate) fn assert(context: &TestContext, machine: Option<&Path>, name: &str, extension: &str, actual: &str) {
    if machine.is_some() && skip_machine_snapshots() {
        return;
    }
    let mut dir = PathBuf::from(context.manifest_dir).join("tests").join("snapshots");
    if let Some(machine) = machine {
        dir.push(machine);
    }
    let file = dir.join(format!("{}@{}.{extension}", context.file_prefix, sanitize(name)));
    let mut pending = file.clone().into_os_string();
    pending.push(".new");
    let pending = PathBuf::from(pending);
    let update = updating();
    let ci = on_ci();

    let write = |path: &PathBuf| {
        std::fs::create_dir_all(&dir).expect("create snapshot directory");
        std::fs::write(path, actual).expect("write snapshot");
    };

    let expected = match std::fs::read_to_string(&file) {
        Ok(expected) => expected,
        Err(_) if ci && !update => {
            write(&pending);
            context.fail_snapshot(format!(
                "snapshot {} is missing (not created on CI); the actual one is in {}",
                file.display(),
                pending.display()
            ));
            return;
        }
        Err(_) => {
            write(&file);
            eprintln!("mitsuami-test: created snapshot {}", file.display());
            return;
        }
    };
    if expected.replace("\r\n", "\n") == actual {
        let _ = std::fs::remove_file(&pending);
        return;
    }
    if update {
        write(&file);
        let _ = std::fs::remove_file(&pending);
        return;
    }
    write(&pending);
    let diff = TextDiff::from_lines(expected.as_str(), actual).unified_diff().header("expected", "actual").to_string();
    context.fail_snapshot(format!(
        "snapshot {} does not match\n\n{diff}\nreview {} and rename it over the snapshot to accept, \
         or run with MITSUAMI_UPDATE_SNAPSHOTS=1",
        file.display(),
        pending.display()
    ));
}
