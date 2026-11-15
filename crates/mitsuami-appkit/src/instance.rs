//! Single instance on macOS. Launch Services keeps a bundled app to one
//! process: a second `open`, a Dock click or Finder's Open With reaches the
//! running one as `applicationShouldHandleReopen:hasVisibleWindows:` or
//! `application:openURLs:` (the app delegate's, in `app`), and both come
//! here. A process Launch Services didn't start (a bundled executable run
//! from a terminal, or `cargo run` outside a bundle) gets past it, so a
//! single-instance app also listens on a Unix socket named after its id,
//! as other platforms' apps do without a bus, and a later launch hands its
//! folder and arguments over it. The socket is in the user's own folder
//! (`confstr(_CS_DARWIN_USER_DIR)`, which only they can write to), not
//! `$TMPDIR`, which may be
//! unset (ssh, `env -i`) and fall back to the shared `/tmp`, and whose
//! files `dirhelper` removes after three days, under an app that runs for
//! longer.

use std::cell::RefCell;
use std::ffi::{CStr, OsStr};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use dispatch2::DispatchQueue;
use mitsuami_core::services::Reopen;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

/// Where later launches go: the app's `Ui::reopen`.
type Reopened = Rc<dyn Fn(Reopen)>;

thread_local! {
    static REOPENED: RefCell<Option<Reopened>> = const { RefCell::new(None) };
}

/// How long either side waits for the other over the socket, for the
/// whole hand-over.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The most a launch may send: more than the system's limit on a
/// command line (`ARG_MAX`, 1 MiB), so a real one always fits.
const MAX_MESSAGE: u64 = 2 << 20;

/// Makes sure this is the only process for the app, or hands this launch
/// to the running one. Returns whether this process should go on and run
/// the app.
pub(crate) fn claim(id: &str) -> bool {
    let Some(dir) = user_dir() else {
        eprintln!("mitsuami: {id} runs as its own instance, since it has no private folder to look for others in");
        return true;
    };
    let path = socket_path(&dir, id);
    // Connecting, clearing a stale socket and binding happen under a lock,
    // so two launches at once can't both bind (the second would unlink the
    // first's socket). It's let go once the socket is bound, which later
    // launches then connect to; a crash lets it go too.
    let lock = match lock(&path.with_extension("lock")) {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("mitsuami: {id} runs as its own instance, since it can't look for others: {error}");
            return true;
        }
    };
    match UnixStream::connect(&path) {
        Ok(stream) => {
            drop(lock);
            if let Err(error) = hand_over(stream) {
                // The running one is there but didn't take it: running a
                // second instance would do what single instance prevents.
                eprintln!("mitsuami: couldn't hand this launch to the running {id}: {error}");
            }
            false
        }
        Err(_) => {
            // Nobody listens: a socket left by a process that ended goes.
            let _ = std::fs::remove_file(&path);
            match UnixListener::bind(&path) {
                Ok(listener) => {
                    // Only the user may connect; others can see the folder.
                    _ = std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600));
                    listen(listener);
                }
                Err(error) => {
                    eprintln!("mitsuami: {id} runs as its own instance, since it can't listen for others: {error}")
                }
            }
            true
        }
    }
}

/// The user's private folder, checked to be theirs alone.
fn user_dir() -> Option<PathBuf> {
    let mut buffer = [0u8; 1024];
    // SAFETY: the buffer is as long as we say; confstr NUL-terminates
    // what it writes and returns the length it needed, 0 on failure.
    let needed = unsafe { libc::confstr(libc::_CS_DARWIN_USER_DIR, buffer.as_mut_ptr().cast(), buffer.len()) };
    if needed == 0 || needed > buffer.len() {
        return None;
    }
    let dir = PathBuf::from(OsStr::from_bytes(CStr::from_bytes_until_nul(&buffer).ok()?.to_bytes()));
    let meta = std::fs::symlink_metadata(&dir).ok()?;
    // SAFETY: geteuid can't fail.
    let me = unsafe { libc::geteuid() };
    (meta.is_dir() && meta.uid() == me && meta.mode() & 0o022 == 0).then_some(dir)
}

/// The socket a single-instance app listens on. A socket's path holds 104
/// bytes, so a long id is hashed.
fn socket_path(dir: &std::path::Path, id: &str) -> PathBuf {
    let path = dir.join(format!("{id}.instance"));
    if path.as_os_str().len() < 100 { path } else { dir.join(format!("mitsuami-{:016x}.instance", fnv(id))) }
}

/// FNV-1a, the same in every build.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3))
}

/// Opens and exclusively locks the lock file beside the socket, waiting
/// for a launch that holds it while it binds.
fn lock(path: &std::path::Path) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

/// Whether the other end of the socket runs as this user. The folder
/// already keeps others out; this keeps it so if its mode ever changes.
fn same_user(stream: &UnixStream) -> bool {
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: a connected socket's descriptor, and two places to write to.
    let ok = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } == 0;
    // SAFETY: geteuid can't fail.
    ok && uid == unsafe { libc::geteuid() }
}

/// Sends this launch's working folder and arguments, each ended by a
/// NUL, and waits for the running one to say it has them.
fn hand_over(mut stream: UnixStream) -> std::io::Result<()> {
    if !same_user(&stream) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "another user's process listens"));
    }
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let mut message = Vec::new();
    // As bytes: `args()` would panic on a name in another encoding.
    let cwd = std::env::current_dir().map(|cwd| cwd.into_os_string()).unwrap_or_default();
    for field in std::iter::once(cwd).chain(std::env::args_os().skip(1)) {
        message.extend_from_slice(field.as_bytes());
        message.push(0);
    }
    stream.write_all(&message)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut answer = [0u8; 1];
    stream.read_exact(&mut answer)
}

/// Takes later launches on a thread of its own, each read on a thread of
/// its own so one that stalls can't hold up the rest, and hands each to
/// the main thread.
fn listen(listener: UnixListener) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = match stream {
                Ok(stream) => stream,
                // Out of descriptors, say: accepting again at once would
                // spin while the connection waits.
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
            };
            if !same_user(&stream) {
                continue;
            }
            // A thread that can't be made drops the connection, rather than
            // ending the listener, after which a launch would run as a
            // second instance.
            _ = std::thread::Builder::new().spawn(move || receive(stream));
        }
    });
}

fn receive(mut stream: UnixStream) {
    let deadline = std::time::Instant::now() + TIMEOUT;
    let mut message = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        // The timeout bounds the whole message, not each read, so a
        // launch that trickles bytes ends too.
        let Some(left) = deadline.checked_duration_since(std::time::Instant::now()).filter(|d| !d.is_zero()) else {
            return;
        };
        if stream.set_read_timeout(Some(left)).is_err() {
            return;
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) if (message.len() + n) as u64 <= MAX_MESSAGE => message.extend_from_slice(&chunk[..n]),
            Ok(_) => return,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
    let mut fields: Vec<String> =
        message.split(|byte| *byte == 0).map(|field| String::from_utf8_lossy(field).into_owned()).collect();
    // The last NUL leaves an empty field after it.
    fields.pop();
    if fields.is_empty() {
        return;
    }
    let cwd = fields.remove(0);
    _ = stream.set_write_timeout(Some(TIMEOUT));
    _ = stream.write_all(b"1");
    DispatchQueue::main().exec_async(move || {
        let cwd = (!cwd.is_empty()).then(|| PathBuf::from(cwd));
        if let Some(mtm) = MainThreadMarker::new() {
            bring_forward(mtm);
        }
        reopened(Reopen::new(fields, cwd, Vec::new()));
    });
}

/// What a Dock click does, which Launch Services' reopen gets from AppKit:
/// the app active, and the window the user last used in front, restored if
/// it's minimized.
fn bring_forward(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.activate();
    let window = app.mainWindow().or_else(|| {
        app.windows()
            .iter()
            .find(|window| window.canBecomeMainWindow() && (window.isVisible() || window.isMiniaturized()))
    });
    if let Some(window) = window {
        if window.isMiniaturized() {
            window.deminiaturize(None);
        }
        window.makeKeyAndOrderFront(None);
    }
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
