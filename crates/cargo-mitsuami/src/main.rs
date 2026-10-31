//! `cargo mitsuami`: the mitsuami CLI.
//!
//! ```text
//! cargo mitsuami visual                   run the native tests, then list what to review
//! cargo mitsuami visual review            review pending captures in the browser
//!     --out <dir>                         write a static report there instead
//!     --no-open                           print the page's URL, don't open it
//!     --port <port>                       serve on this port (default: any free one)
//! cargo mitsuami visual accept [filter]   accept pending captures (those whose path contains filter)
//! ```
//!
//! Pending captures are the `<name>.new.png` files tests write next to
//! their baselines in `tests/visual/<backend>/<image>/`, when a capture
//! doesn't match or has no baseline yet.

mod changes;
mod report;
mod server;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use changes::Change;

const USAGE: &str = "usage:
    cargo mitsuami visual                   run the native tests, then list what to review
    cargo mitsuami visual review [--out <dir>] [--no-open] [--port <port>]
    cargo mitsuami visual accept [filter]";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Cargo runs `cargo-mitsuami mitsuami <args>`.
    if args.first().map(String::as_str) == Some("mitsuami") {
        args.remove(0);
    }
    let root = changes::workspace_root(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["visual"] => run(&root),
        ["visual", "review", options @ ..] => review(&root, options),
        ["visual", "accept"] => accept(&root, None),
        ["visual", "accept", filter] => accept(&root, Some(filter)),
        ["help" | "--help" | "-h"] | [] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("unknown command\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cargo-mitsuami: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(root: &Path) -> Result<(), String> {
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["test", "--workspace", "--no-fail-fast"])
        .env("MITSUAMI_NATIVE", "1")
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    let pending = changes::find(root);
    if !pending.is_empty() {
        println!("\n{} captures to review: cargo mitsuami visual review", pending.len());
    }
    if status.success() { Ok(()) } else { Err("tests failed".to_owned()) }
}

fn review(root: &Path, options: &[&str]) -> Result<(), String> {
    let (mut out, mut open, mut port) = (None, true, 0);
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match *option {
            "--out" => out = Some(PathBuf::from(options.next().ok_or("--out needs a folder")?)),
            "--no-open" => open = false,
            "--port" => port = options.next().and_then(|p| p.parse().ok()).ok_or("--port needs a number")?,
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let pending = changes::find(root);
    if let Some(out) = out {
        return write_static(&pending, &out);
    }
    if pending.is_empty() {
        println!("Nothing to review: every capture matches its baseline.");
        return Ok(());
    }
    server::serve(&pending, port, |url| {
        println!("Reviewing {} changes at {url}", pending.len());
        println!("Click Done on the page to apply your decisions; Ctrl-C stops without applying them.");
        if open {
            open_browser(url);
        }
    })
    .map_err(|e| format!("cannot serve the review: {e}"))
}

fn write_static(pending: &[Change], out: &Path) -> Result<(), String> {
    let images = out.join("images");
    std::fs::create_dir_all(&images).map_err(|e| format!("cannot create {}: {e}", images.display()))?;
    for (index, change) in pending.iter().enumerate() {
        let copies = [("baseline", change.baseline.clone()), ("capture", change.capture()), ("diff", change.diff())];
        for (kind, from) in copies {
            if from.exists() {
                let to = out.join(report::image_url(&report::Mode::Static, index, kind));
                std::fs::copy(&from, &to).map_err(|e| format!("cannot copy {}: {e}", from.display()))?;
            }
        }
    }
    let page = out.join("index.html");
    std::fs::write(&page, report::page(pending, &vec![None; pending.len()], &report::Mode::Static))
        .map_err(|e| format!("cannot write {}: {e}", page.display()))?;
    println!("Wrote the review of {} changes to {}", pending.len(), page.display());
    Ok(())
}

fn accept(root: &Path, filter: Option<&str>) -> Result<(), String> {
    let pending: Vec<Change> =
        changes::find(root).into_iter().filter(|c| filter.is_none_or(|f| c.display.contains(f))).collect();
    if pending.is_empty() {
        println!("Nothing to accept.");
    }
    for change in &pending {
        change.accept().map_err(|e| format!("cannot accept {}: {e}", change.display))?;
        println!("accepted {}", change.display);
    }
    Ok(())
}

fn open_browser(url: &str) {
    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        // `start` takes its first quoted argument as the window's title.
        Command::new("cmd").args(["/c", "start", "", url]).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    };
    if result.is_err() {
        eprintln!("cargo-mitsuami: cannot open a browser; open {url}");
    }
}
