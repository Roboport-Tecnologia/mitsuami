//! The C ABI drives the same `Ui` the builder API does: nodes made through
//! it are laid out, found, clicked and typed into as any others, and their
//! events come back through the callback.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use mitsuami::core::{NodeId, Ui, View};
use mitsuami_ffi::*;
use mitsuami_test::prelude::*;

thread_local! {
    static HEARD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

extern "C" fn record(_user: *mut c_void, _node: u32, event: *const MitsuamiEvent) {
    let event = unsafe { &*event };
    let heard = match event.kind {
        0 => "click".to_string(),
        1 if event.value_kind == MitsuamiValueKind::Text as u32 => {
            let text = unsafe { std::slice::from_raw_parts(event.text, event.text_len) };
            format!("changed {}", std::str::from_utf8(text).unwrap())
        }
        1 if event.value_kind == MitsuamiValueKind::Bool as u32 => format!("changed {}", event.flag),
        // Where focus goes on a click differs per platform.
        3 | 4 => return,
        kind => format!("event {kind}"),
    };
    HEARD.with(|h| h.borrow_mut().push(heard));
}

fn heard() -> Vec<String> {
    HEARD.with(|h| std::mem::take(&mut *h.borrow_mut()))
}

/// A node the C ABI made, mounted as a view.
struct Adopt(u32);

impl View for Adopt {
    fn build(self, _: &Ui) -> NodeId {
        NodeId::from_raw(self.0)
    }
}

fn set_text(id: u32, prop: MitsuamiTextProp, text: &str) {
    assert!(unsafe { mitsuami_set_text(id, prop as u32, text.as_ptr(), text.len()) });
}

fn create(kind: MitsuamiKind) -> u32 {
    let id = mitsuami_create(kind as u32);
    assert_ne!(id, 0);
    id
}

/// Clone's form, as a binding would build it.
fn clone_form() -> u32 {
    let column = create(MitsuamiKind::Container);
    assert!(mitsuami_set_padding(column, 16.0, 16.0, 16.0, 16.0));
    assert!(mitsuami_set_gap(column, 8.0));
    assert!(mitsuami_set_align_items(column, 1));

    let title = create(MitsuamiKind::Text);
    set_text(title, MitsuamiTextProp::Text, "Clone a repository");
    assert!(mitsuami_set_enum(title, MitsuamiEnumProp::TextStyle as u32, 2));

    let url = create(MitsuamiKind::TextInput);
    set_text(url, MitsuamiTextProp::Placeholder, "https://");
    set_text(url, MitsuamiTextProp::A11yLabel, "Repository URL");
    assert!(mitsuami_on_event(url, record, std::ptr::null_mut()));

    let bare = create(MitsuamiKind::Checkbox);
    set_text(bare, MitsuamiTextProp::Label, "Bare");
    assert!(mitsuami_on_event(bare, record, std::ptr::null_mut()));

    let clone = create(MitsuamiKind::Button);
    set_text(clone, MitsuamiTextProp::Label, "Clone");
    assert!(mitsuami_set_enum(clone, MitsuamiEnumProp::ButtonRole as u32, 1));
    assert!(mitsuami_on_event(clone, record, std::ptr::null_mut()));

    for child in [title, url, bare, clone] {
        assert!(mitsuami_append_child(column, child));
    }
    column
}

#[mitsuami_test::test]
async fn nodes_it_makes_are_laid_out_and_found(app: TestApp) {
    attach(app.ui());
    app.mount(|| Adopt(clone_form()));

    let title = app.get_by_text("Clone a repository").frame();
    let url = app.get_by_role(Role::TextField, "Repository URL").frame();
    let clone = app.get_by_role(Role::Button, "Clone").frame();
    assert_eq!(title.origin.x, 16.0);
    assert_eq!(title.origin.y, 16.0);
    assert_eq!(url.y(), title.max_y() + 8.0);
    // Aligned to the start, the button keeps its own width.
    assert!(clone.width() < url.width() || url.width() < 100.0);
}

#[mitsuami_test::test]
async fn events_come_back_through_the_callback(app: TestApp) {
    attach(app.ui());
    heard();
    app.mount(|| Adopt(clone_form()));

    app.get_by_role(Role::Button, "Clone").click().await;
    app.get_by_role(Role::TextField, "Repository URL").fill("https://example.com/a.git").await;
    app.get_by_role(Role::Checkbox, "Bare").click().await;

    assert_eq!(heard(), ["click", "changed https://example.com/a.git", "changed true"]);
}

static POSTED_TO: AtomicU32 = AtomicU32::new(0);

extern "C" fn show_posted(_user: *mut c_void) {
    set_text(POSTED_TO.load(Ordering::SeqCst), MitsuamiTextProp::Text, "Posted from a thread");
}

#[mitsuami_test::test]
async fn other_threads_post_to_the_ui_thread(app: TestApp) {
    attach(app.ui());
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, "Waiting");
    POSTED_TO.store(text, Ordering::SeqCst);
    app.mount(|| Adopt(text));

    std::thread::spawn(|| mitsuami_post(show_posted, std::ptr::null_mut())).join().unwrap();

    app.expect(by_text("Posted from a thread")).to_exist().await;
}

fn last_error() -> String {
    let mut ptr = std::ptr::null();
    let len = unsafe { mitsuami_last_error(&mut ptr) };
    String::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()).unwrap()
}

#[mitsuami_test::test]
async fn failures_are_returned_not_panicked(app: TestApp) {
    attach(app.ui());
    assert_eq!(mitsuami_create(99), 0);
    assert_eq!(last_error(), "unknown kind 99");

    let text = create(MitsuamiKind::Text);
    let bad = [0xff, 0xfe];
    assert!(!unsafe { mitsuami_set_text(text, MitsuamiTextProp::Text as u32, bad.as_ptr(), bad.len()) });
    assert_eq!(last_error(), "string isn't UTF-8");

    // A destroyed node is never another one: calls on it do nothing.
    assert!(mitsuami_destroy(text));
    set_text(text, MitsuamiTextProp::Text, "Gone");
}

mitsuami_test::main!();
