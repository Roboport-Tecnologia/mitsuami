//! A libtest-compatible runner that owns the main thread.

use std::any::Any;
use std::cell::RefCell;
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::time::Instant;

use mitsuami_core::{Appearance, Size, WindowSize};

use crate::__private::{StoryCase, StoryFuture, TestCase, TestFuture};
use crate::app::{DEFAULT_WINDOW, TestApp, TestContext};
use crate::driver::{Mode, native_available, with_pool};
use crate::exec::block_on;
use crate::story::Variant;

struct Options {
    filters: Vec<String>,
    skip: Vec<String>,
    exact: bool,
    list: bool,
    native: bool,
    ignored: bool,
}

fn parse(args: impl Iterator<Item = String>) -> Options {
    let mut options =
        Options { filters: Vec::new(), skip: Vec::new(), exact: false, list: false, native: false, ignored: false };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--exact" => options.exact = true,
            "--list" => options.list = true,
            "--native" => options.native = true,
            "--ignored" => options.ignored = true,
            "--skip" => options.skip.extend(args.next()),
            // Accepted for libtest/IDE compatibility; tests always run
            // serially on the main thread and output is not captured.
            "--nocapture" | "--show-output" | "--include-ignored" | "-q" | "--quiet" => {}
            "--test-threads" | "--format" | "--color" | "-Z" | "--logfile" => {
                args.next();
            }
            a if a.starts_with("--test-threads=") || a.starts_with("--format=") || a.starts_with("--color=") => {}
            a if a.starts_with('-') => eprintln!("mitsuami-test: ignoring unknown flag {a}"),
            _ => options.filters.push(arg),
        }
    }
    options
}

/// One test to run: a test case, or a story at one size in one variant.
struct Job {
    /// `module::path::test_name` (plus `@<size>-<variant>` for stories),
    /// without the crate.
    name: String,
    file_prefix: String,
    manifest_dir: &'static str,
    headless_only: bool,
    kind: JobKind,
}

enum JobKind {
    Test(fn(TestApp) -> TestFuture),
    Story {
        run: for<'a> fn(&'a TestApp) -> StoryFuture<'a>,
        visual: fn() -> crate::VisualOptions,
        size: WindowSize,
        variant: Variant,
        label: String,
    },
}

fn display_name(name: &'static str) -> &'static str {
    name.split_once("::").map_or(name, |(_, rest)| rest)
}

fn jobs() -> Vec<Job> {
    let mut jobs: Vec<Job> = inventory::iter::<TestCase>
        .into_iter()
        .map(|case| Job {
            name: display_name(case.name).to_owned(),
            file_prefix: case.name.replace("::", "__"),
            manifest_dir: case.manifest_dir,
            headless_only: case.headless_only,
            kind: JobKind::Test(case.run),
        })
        .collect();
    for story in inventory::iter::<StoryCase> {
        for &(width, height) in story.sizes {
            let (size, height_label) = match height {
                Some(height) => (WindowSize::Fixed(Size::new(width, height)), height.to_string()),
                None => (WindowSize::FitHeight(width), "fit".to_owned()),
            };
            for &variant in story.variants {
                let label = format!("{width}x{height_label}-{}", variant.name());
                jobs.push(Job {
                    name: format!("{}@{label}", display_name(story.name)),
                    file_prefix: story.name.replace("::", "__"),
                    manifest_dir: story.manifest_dir,
                    headless_only: false,
                    kind: JobKind::Story { run: story.run, visual: story.visual, size, variant, label },
                });
            }
        }
    }
    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    jobs
}

fn selected(options: &Options, name: &str) -> bool {
    let matches = |f: &String| if options.exact { name == f } else { name.contains(f.as_str()) };
    (options.filters.is_empty() || options.filters.iter().any(matches)) && !options.skip.iter().any(matches)
}

thread_local! {
    static PANIC_MESSAGE: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn payload_text(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic with a non-string payload".into())
}

pub fn run_main() {
    let options = parse(std::env::args().skip(1));
    let jobs = jobs();
    let total = jobs.len();
    let jobs: Vec<Job> = jobs.into_iter().filter(|j| selected(&options, &j.name)).collect();

    if options.list {
        for job in &jobs {
            println!("{}: test", job.name);
        }
        println!("\n{} tests, 0 benchmarks", jobs.len());
        return;
    }
    if options.ignored {
        println!("\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n");
        return;
    }
    let native = options.native || std::env::var("MITSUAMI_NATIVE").is_ok_and(|v| v == "1");
    if native && !native_available() {
        eprintln!("mitsuami-test: --native needs a native backend for this platform; none is available yet.");
        std::process::exit(2);
    }
    let mode = if native { Mode::Native } else { Mode::Headless };

    panic::set_hook(Box::new(|info| {
        let location = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        let message = format!("{}{location}", payload_text(info.payload()));
        PANIC_MESSAGE.with(|m| *m.borrow_mut() = Some(message));
    }));

    let count = jobs.len();
    println!("\nrunning {count} tests");
    let started = Instant::now();
    let mut failures = Vec::new();
    let mut ignored = 0;
    for job in jobs {
        let name = job.name;
        print!("test {name} ... ");
        let _ = std::io::stdout().flush();
        if mode == Mode::Native && job.headless_only {
            println!("ignored, headless only");
            ignored += 1;
            continue;
        }
        let snapshot_failures = Rc::new(RefCell::new(Vec::new()));
        let l10n_errors = Rc::new(RefCell::new(Vec::new()));
        let context = TestContext {
            name: name.clone(),
            file_prefix: job.file_prefix,
            manifest_dir: job.manifest_dir,
            snapshot_failures: snapshot_failures.clone(),
            l10n_errors: l10n_errors.clone(),
        };
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            with_pool(|| match job.kind {
                JobKind::Test(run) => {
                    let app = TestApp::new(context, mode, Appearance::Light, DEFAULT_WINDOW.into());
                    block_on(run(app));
                }
                JobKind::Story { run, visual, size, variant, label } => {
                    let app = TestApp::new(context, mode, variant.appearance(), size);
                    block_on(async {
                        run(&app).await;
                        app.assert_visual_snapshot_with(&label, &visual()).await;
                    });
                }
            })
        }));
        let mut messages: Vec<String> = Vec::new();
        if let Err(payload) = result {
            messages.push(PANIC_MESSAGE.with(|m| m.borrow_mut().take()).unwrap_or_else(|| payload_text(&*payload)));
        }
        messages.extend(snapshot_failures.take());
        let l10n_errors = l10n_errors.take();
        if !l10n_errors.is_empty() {
            messages.push(format!(
                "localization errors (take them with `app.take_l10n_errors()` if expected):\n  {}",
                l10n_errors.join("\n  ")
            ));
        }
        if messages.is_empty() {
            println!("ok");
        } else {
            println!("FAILED");
            failures.push((name, messages.join("\n\n")));
        }
    }
    let _ = panic::take_hook();

    if !failures.is_empty() {
        println!("\nfailures:\n");
        for (name, message) in &failures {
            println!("---- {name} ----\n{message}\n");
        }
        println!("failures:");
        for (name, _) in &failures {
            println!("    {name}");
        }
    }
    let status = if failures.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {status}. {} passed; {} failed; {ignored} ignored; 0 measured; {} filtered out; finished in {:.2}s\n",
        count - failures.len() - ignored,
        failures.len(),
        total - count,
        started.elapsed().as_secs_f64()
    );
    if !failures.is_empty() {
        std::process::exit(101);
    }
}
