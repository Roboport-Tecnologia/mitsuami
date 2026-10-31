//! `cargo mitsuami visual`, run on a workspace with pending captures: the
//! static report, `accept`, and the review server's accept and reject.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_cargo-mitsuami");

/// A throwaway workspace with one changed capture (`buttons`) and one
/// without a baseline (`toggles`), removed when dropped.
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Workspace {
        let root = std::env::temp_dir().join(format!("cargo-mitsuami-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let visual = root.join("app/tests/visual/appkit/macos-26@2x");
        std::fs::create_dir_all(&visual).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"app\"]\n").unwrap();
        // The tool moves files; their content doesn't matter to it.
        let files = [
            ("stories__buttons@340xfit-light.png", "old pixels"),
            ("stories__buttons@340xfit-light.new.png", "new pixels"),
            ("stories__buttons@340xfit-light.diff.png", "diff pixels"),
            ("stories__buttons@340xfit-light.layout.txt", "Window [0,0 340×56]\n  Button \"OK\" [16,16 68×24]\n"),
            ("stories__buttons@340xfit-light.layout.txt.new", "Window [0,0 340×64]\n  Button \"OK\" [20,20 68×24]\n"),
            (
                "stories__buttons@340xfit-light.notes.txt",
                "Only GTK draws destructive buttons.\n\nAppKit's <combo> button has no prominence.",
            ),
            ("stories__toggles@200xfit-light.new.png", "first pixels"),
            ("stories__toggles@200xfit-light.layout.txt.new", "Window [0,0 200×240]\n"),
            ("stories__text@240xfit-light.png", "unchanged"),
        ];
        for (file, content) in files {
            std::fs::write(visual.join(file), content).unwrap();
        }
        Workspace { root }
    }

    fn visual(&self, file: &str) -> PathBuf {
        self.root.join("app/tests/visual/appkit/macos-26@2x").join(file)
    }

    fn read(&self, file: &str) -> Option<String> {
        std::fs::read_to_string(self.visual(file)).ok()
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        // From a crate inside the workspace, as cargo would.
        Command::new(BIN).args(args).current_dir(self.root.join("app")).output().unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_static_report_shows_each_change_with_its_images_and_why() {
    let ws = Workspace::new("static");
    let out = ws.root.join("report");
    // As cargo runs it, with the subcommand's name first.
    let output = ws.run(&["mitsuami", "visual", "review", "--out", out.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");

    let page = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(page.contains("2 changes to review."));
    assert!(page.contains("stories__buttons@340xfit-light") && page.contains("stories__toggles@200xfit-light"));
    assert!(!page.contains("stories__text@240xfit-light"), "unchanged baselines aren't listed");
    assert!(page.contains("The layout changed:") && page.contains("+   Button &quot;OK&quot; [20,20 68×24]"));
    assert!(page.contains("New baseline."));
    // The story's notes, a paragraph each.
    assert!(
        page.contains(
            "<p>Only GTK draws destructive buttons.</p><p>AppKit's &lt;combo&gt; button has no prominence.</p>"
        )
    );
    assert_eq!(page.matches(r#"class="notes""#).count(), 1, "only the change whose story has notes");
    assert!(!page.contains("data-action=\"accept\""), "a static report can't accept");

    // Changes are sorted by path: buttons first. A new one has no baseline.
    let copied = |file: &str| std::fs::read_to_string(out.join("images").join(file)).ok();
    assert_eq!(copied("0-baseline.png").as_deref(), Some("old pixels"));
    assert_eq!(copied("0-capture.png").as_deref(), Some("new pixels"));
    assert_eq!(copied("0-diff.png").as_deref(), Some("diff pixels"));
    assert_eq!(copied("1-capture.png").as_deref(), Some("first pixels"));
    assert_eq!(copied("1-baseline.png"), None);
    // Writing the report changes nothing.
    assert_eq!(ws.read("stories__buttons@340xfit-light.png").as_deref(), Some("old pixels"));
}

#[test]
fn accept_moves_captures_and_layouts_over_their_baselines() {
    let ws = Workspace::new("accept");
    let output = ws.run(&["visual", "accept", "buttons"]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        stdout(&output).contains("accepted app/tests/visual/appkit/macos-26@2x/stories__buttons@340xfit-light.png")
    );

    assert_eq!(ws.read("stories__buttons@340xfit-light.png").as_deref(), Some("new pixels"));
    assert!(ws.read("stories__buttons@340xfit-light.layout.txt").unwrap().contains("[20,20 68×24]"));
    for gone in [".new.png", ".diff.png", ".layout.txt.new"] {
        assert_eq!(ws.read(&format!("stories__buttons@340xfit-light{gone}")), None, "{gone}");
    }
    // The filter left the other change pending.
    assert_eq!(ws.read("stories__toggles@200xfit-light.new.png").as_deref(), Some("first pixels"));

    let output = ws.run(&["visual", "accept"]);
    assert!(stdout(&output).contains("stories__toggles@200xfit-light.png"));
    assert_eq!(ws.read("stories__toggles@200xfit-light.png").as_deref(), Some("first pixels"));
    assert!(stdout(&ws.run(&["visual", "accept"])).contains("Nothing to accept."));
}

/// Sends a request to the review server and returns the status and body.
fn request(port: u16, method: &str, target: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(stream, "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
    (status, body)
}

#[test]
fn the_review_server_accepts_and_rejects_with_its_token_only() {
    let ws = Workspace::new("serve");
    let mut child = Command::new(BIN)
        .args(["visual", "review", "--no-open"])
        .current_dir(&ws.root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let first = lines.next().unwrap().unwrap();
    let url = first.split_whitespace().last().unwrap().to_owned();
    assert!(first.starts_with("Reviewing 2 changes at http://127.0.0.1:"), "{first}");
    let address = url.trim_start_matches("http://127.0.0.1:");
    let (port, query) = address.split_once("/?").unwrap();
    let port: u16 = port.parse().unwrap();

    let (status, page) = request(port, "GET", &format!("/?{query}"));
    assert_eq!(status, 200);
    assert!(page.contains("data-action=\"accept\"") && page.contains("stories__buttons@340xfit-light"));
    let (status, image) = request(port, "GET", &format!("/image/0/capture?{query}"));
    assert_eq!((status, image.as_str()), (200, "new pixels"));

    // Without the token, nothing happens.
    assert_eq!(request(port, "POST", "/accept/0").0, 403);
    assert_eq!(request(port, "POST", "/accept/0?token=guess").0, 403);
    assert_eq!(ws.read("stories__buttons@340xfit-light.png").as_deref(), Some("old pixels"));

    // Decisions are only recorded: a reload shows them, and they can be
    // changed or undone, until Done applies them.
    assert_eq!(request(port, "POST", &format!("/reject/0?{query}")).0, 200);
    assert_eq!(request(port, "POST", &format!("/accept/1?{query}")).0, 200);
    assert_eq!(ws.read("stories__buttons@340xfit-light.png").as_deref(), Some("old pixels"));
    assert_eq!(ws.read("stories__toggles@200xfit-light.png"), None);
    let (_, page) = request(port, "GET", &format!("/?{query}"));
    assert!(page.contains(r#"class="change rejected" id="change-0""#), "{page}");
    assert!(page.contains(r#"class="change accepted" id="change-1""#), "{page}");
    let (status, image) = request(port, "GET", &format!("/image/0/capture?{query}"));
    assert_eq!((status, image.as_str()), (200, "new pixels"), "a reload still shows the capture");
    assert_eq!(request(port, "POST", &format!("/accept/0?{query}")).0, 200);
    assert_eq!(request(port, "POST", &format!("/undo/1?{query}")).0, 200);
    assert_eq!(request(port, "POST", &format!("/reject/1?{query}")).0, 200);
    assert_eq!(request(port, "POST", &format!("/accept/7?{query}")).0, 404);

    assert_eq!(request(port, "POST", &format!("/done?{query}")).0, 200);
    assert!(child.wait().unwrap().success());
    assert_eq!(ws.read("stories__buttons@340xfit-light.png").as_deref(), Some("new pixels"));
    assert_eq!(ws.read("stories__buttons@340xfit-light.notes.txt"), None, "the notes go with the capture");
    assert_eq!(ws.read("stories__toggles@200xfit-light.new.png"), None);
    assert_eq!(ws.read("stories__toggles@200xfit-light.png"), None, "rejecting a new capture records nothing");
    let rest: Vec<String> = lines.map_while(Result::ok).collect();
    assert!(rest.iter().any(|l| l.contains("accepted") && l.contains("buttons")), "{rest:?}");
    assert!(rest.iter().any(|l| l.contains("rejected") && l.contains("toggles")), "{rest:?}");
}

/// Every accepted baseline is shown, a pending capture's too, but not the
/// captures; a rejected one is deleted with its layout at Done, for the
/// next run to capture again, and Undo keeps one.
#[test]
fn the_baselines_page_shows_every_baseline_and_removes_the_rejected() {
    let ws = Workspace::new("baselines");
    let mut child = Command::new(BIN)
        .args(["visual", "baselines", "--no-open"])
        .current_dir(&ws.root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let first = lines.next().unwrap().unwrap();
    assert!(first.starts_with("Showing 2 baselines at http://127.0.0.1:"), "{first}");
    let url = first.split_whitespace().last().unwrap().to_owned();
    let (port, query) = url.trim_start_matches("http://127.0.0.1:").split_once("/?").unwrap();
    let port: u16 = port.parse().unwrap();

    let (status, page) = request(port, "GET", &format!("/?{query}"));
    assert_eq!(status, 200);
    assert!(page.contains("stories__buttons@340xfit-light") && page.contains("stories__text@240xfit-light"), "{page}");
    assert!(!page.contains("stories__toggles@200xfit-light"), "a capture with no baseline isn't one");
    assert!(page.contains(r#"data-image="appkit/macos-26@2x""#));
    let (status, image) = request(port, "GET", &format!("/image/0/baseline?{query}"));
    assert_eq!((status, image.as_str()), (200, "old pixels"));

    assert_eq!(request(port, "POST", "/reject/1").0, 403, "not without the token");
    assert_eq!(request(port, "POST", &format!("/accept/0?{query}")).0, 404, "baselines are already accepted");
    assert_eq!(request(port, "POST", &format!("/reject/0?{query}")).0, 200);
    assert_eq!(request(port, "POST", &format!("/undo/0?{query}")).0, 200);
    assert_eq!(request(port, "POST", &format!("/reject/1?{query}")).0, 200);
    let (_, page) = request(port, "GET", &format!("/?{query}"));
    assert!(page.contains(r#"class="tile rejected" id="change-1""#), "a reload shows it");
    assert_eq!(ws.read("stories__text@240xfit-light.png").as_deref(), Some("unchanged"), "not before Done");

    assert_eq!(request(port, "POST", &format!("/done?{query}")).0, 200);
    assert!(child.wait().unwrap().success());
    assert_eq!(ws.read("stories__text@240xfit-light.png"), None);
    assert_eq!(ws.read("stories__buttons@340xfit-light.png").as_deref(), Some("old pixels"), "undone, it stays");
    assert!(ws.read("stories__buttons@340xfit-light.layout.txt").is_some());
    let rest: Vec<String> = lines.map_while(Result::ok).collect();
    assert!(rest.iter().any(|l| l.contains("removed") && l.contains("stories__text")), "{rest:?}");
}

#[test]
fn nothing_pending_means_nothing_to_review() {
    let ws = Workspace::new("empty");
    ws.run(&["visual", "accept"]);
    let output = ws.run(&["visual", "review", "--no-open"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("Nothing to review"));
}
