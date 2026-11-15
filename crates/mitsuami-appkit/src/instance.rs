//! Single instance on macOS. Launch Services keeps a bundled app to one
//! process: a second `open`, a Dock click or Finder's Open With reaches the
//! running one as `applicationShouldHandleReopen:hasVisibleWindows:` or
//! `application:openURLs:` (the app delegate's, in `app`), and both come
//! here. A bundled executable started directly (from a terminal) gets past
//! Launch Services: it brings the running one forward and ends. A process
//! outside a bundle (`cargo run`) has no bundle id for Launch Services to
//! go by, so it uses a Unix socket in the user's `$TMPDIR`, named after the
//! app's id, as other platforms' apps do without a bus.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use dispatch2::DispatchQueue;
use mitsuami_core::services::Reopen;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationOptions, NSRunningApplication};
use objc2_foundation::{NSBundle, NSString};

/// Where later launches go: the app's `Ui::reopen`.
type Reopened = Rc<dyn Fn(Reopen)>;

thread_local! {
    static REOPENED: RefCell<Option<Reopened>> = const { RefCell::new(None) };
}

/// How long either side waits for the other over the socket.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Makes sure this is the only process for the app, or brings the running
/// one forward with this launch. Returns whether this process should go on
/// and run the app.
pub(crate) fn claim(id: &str) -> bool {
    match NSBundle::mainBundle().bundleIdentifier() {
        Some(bundle_id) => !activate_other(&bundle_id),
        None => claim_socket(id),
    }
}

/// Brings forward another running process of this bundle, if there is
/// one. Launch Services passes no arguments to a running app, so none go.
fn activate_other(bundle_id: &NSString) -> bool {
    let me = std::process::id() as i32;
    let running = NSRunningApplication::runningApplicationsWithBundleIdentifier(bundle_id);
    let other = running.iter().find(|app| app.processIdentifier() != me);
    if let Some(other) = &other {
        other.activateWithOptions(NSApplicationActivationOptions::empty());
    }
    other.is_some()
}

/// The socket a process outside a bundle listens on. `$TMPDIR` is the
/// user's own on macOS. A socket's path holds 104 bytes, so a long id is
/// hashed.
fn socket_path(id: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("{id}.instance"));
    if path.as_os_str().len() < 100 { path } else { dir.join(format!("mitsuami-{:016x}.instance", fnv(id))) }
}

/// FNV-1a, the same in every build.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3))
}

fn claim_socket(id: &str) -> bool {
    let path = socket_path(id);
    // Twice: another launch may bind the socket between our tries.
    for _ in 0..2 {
        if let Ok(stream) = UnixStream::connect(&path) {
            if let Err(error) = hand_over(stream) {
                // The running one is there but didn't take it: running a
                // second instance would do what single instance prevents.
                eprintln!("mitsuami: couldn't hand this launch to the running {id}: {error}");
            }
            return false;
        }
        // Nobody listens: a socket left by a process that ended goes.
        let _ = std::fs::remove_file(&path);
        match UnixListener::bind(&path) {
            Ok(listener) => {
                listen(listener);
                return true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(error) => {
                eprintln!("mitsuami: {id} runs as its own instance, since it can't look for others: {error}");
                return true;
            }
        }
    }
    true
}

/// Sends this launch's working folder and arguments, each ended by a
/// NUL, and waits for the running one to say it has them.
fn hand_over(mut stream: UnixStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let mut message = Vec::new();
    let cwd = std::env::current_dir().map(|cwd| cwd.to_string_lossy().into_owned()).unwrap_or_default();
    for field in std::iter::once(cwd).chain(std::env::args().skip(1)) {
        message.extend_from_slice(field.as_bytes());
        message.push(0);
    }
    stream.write_all(&message)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut answer = [0u8; 1];
    stream.read_exact(&mut answer)
}

/// Takes later launches on a thread of its own, and hands each to the
/// main thread.
fn listen(listener: UnixListener) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            _ = stream.set_read_timeout(Some(TIMEOUT));
            let mut message = Vec::new();
            if stream.read_to_end(&mut message).is_err() {
                continue;
            }
            let mut fields: Vec<String> =
                message.split(|byte| *byte == 0).map(|field| String::from_utf8_lossy(field).into_owned()).collect();
            // The last NUL leaves an empty field after it.
            fields.pop();
            if fields.is_empty() {
                continue;
            }
            let cwd = fields.remove(0);
            _ = stream.write_all(b"1");
            DispatchQueue::main().exec_async(move || {
                let cwd = (!cwd.is_empty()).then(|| PathBuf::from(cwd));
                if let Some(mtm) = MainThreadMarker::new() {
                    NSApplication::sharedApplication(mtm).activate();
                }
                reopened(Reopen::new(fields, cwd, Vec::new()));
            });
        }
    });
}

/// Where later launches go, set by `run` before the app's setup.
pub(crate) fn connect(f: Reopened) {
    REOPENED.with(|r| *r.borrow_mut() = Some(f));
}

/// Hands a later launch to the app, on the main thread.
pub(crate) fn reopened(reopen: Reopen) {
    if let Some(f) = REOPENED.with(|r| r.borrow().clone()) {
        f(reopen);
    }
}
