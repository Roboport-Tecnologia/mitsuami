//! Generates `src/bindings.rs` for `mitsuami-winui`, the same way
//! `windows-reactor` generates its own: minimal mode, member-level filters,
//! `Application`, and a tree's list view, rows and their automation
//! peers, composed from Rust. The NuGet packages are cached under
//! this tool's `target/` directory.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The component packages behind Microsoft.WindowsAppSDK 2.4.0, and the
/// metadata directory inside each.
const PACKAGES: &[(&str, &str, &str)] = &[
    ("Microsoft.WindowsAppSDK.WinUI", "2.3.6", "metadata"),
    ("Microsoft.WindowsAppSDK.InteractiveExperiences", "2.1.6", "metadata/10.0.18362.0"),
    ("Microsoft.WindowsAppSDK.Foundation", "2.3.9", "metadata"),
];

fn main() {
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cache = tool.join("target").join("nuget");
    let output = tool.parent().unwrap().join("src").join("bindings.rs");

    let mut bindgen = windows_bindgen::builder();
    bindgen.input_default();
    for (name, version, dir) in PACKAGES {
        bindgen.input(fetch(&cache, name, version).join(dir));
    }
    bindgen
        .output(&output)
        .implements([
            "Microsoft.UI.Xaml.IApplicationOverrides",
            "Microsoft.UI.Xaml.Markup.IXamlMetadataProvider",
            "Microsoft.UI.Xaml.Controls.IItemsControlOverrides",
            "Microsoft.UI.Xaml.IUIElementOverrides",
            "Microsoft.UI.Xaml.Automation.Peers.IAutomationPeerOverrides",
            "Microsoft.UI.Xaml.Automation.Provider.IExpandCollapseProvider",
        ])
        .composes([
            "Microsoft.UI.Xaml.Application",
            "Microsoft.UI.Xaml.Controls.ListView",
            "Microsoft.UI.Xaml.Controls.ListViewItem",
            "Microsoft.UI.Xaml.Automation.Peers.ListViewItemAutomationPeer",
        ])
        .minimal()
        .flat()
        .filter_file(tool.join("filter.txt"))
        .write();
    // Format with the workspace's settings, so `cargo fmt` leaves it alone.
    let workspace = tool.ancestors().nth(3).unwrap();
    run(Command::new("rustfmt")
        .args(["--edition", "2024", "--config-path"])
        .arg(workspace.join("rustfmt.toml"))
        .arg(&output));
    println!("wrote {}", output.display());
}

fn fetch(cache: &Path, name: &str, version: &str) -> PathBuf {
    let dir = cache.join(format!("{name}.{version}"));
    if dir.exists() {
        return dir;
    }
    std::fs::create_dir_all(&dir).unwrap();
    let nupkg = cache.join(format!("{name}.{version}.zip"));
    let url = format!("https://www.nuget.org/api/v2/package/{name}/{version}");
    run(Command::new("curl").args(["-sSfL", "-o"]).arg(&nupkg).arg(url));
    run(Command::new("tar").arg("-xf").arg(&nupkg).arg("-C").arg(&dir));
    dir
}

fn run(command: &mut Command) {
    let status = command.status().expect("spawn");
    assert!(status.success(), "{command:?} failed");
}
