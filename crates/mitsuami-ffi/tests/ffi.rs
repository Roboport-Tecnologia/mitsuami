//! The C ABI drives the same `Ui` the builder API does: nodes made through
//! it are laid out, found, clicked and typed into as any others, and their
//! events come back through the callback.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use mitsuami::core::{Color, Modality, NodeId, Prop, Ui, VectorIcon, View};
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
        6 => {
            let text = unsafe { std::slice::from_raw_parts(event.text, event.text_len) };
            format!("search {}", std::str::from_utf8(text).unwrap())
        }
        1 if event.value_kind == MitsuamiValueKind::Index as u32 => format!("chose {}", event.index),
        7 => format!("close {}", event.index),
        8 => format!("moved {} to {}", event.index, event.to),
        9 => "new tab".to_string(),
        10 => format!("menu {}", event.index),
        11 => format!("hover {}", event.flag),
        12 => {
            let text = unsafe { std::slice::from_raw_parts(event.text, event.text_len) };
            let paths: Vec<&str> = std::str::from_utf8(text).unwrap().split_terminator('\0').collect();
            let names: Vec<&str> = paths.iter().map(|p| p.rsplit('/').next().unwrap()).collect();
            format!("dropped {}", names.join(","))
        }
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
    let column = app.mount(|| Adopt(clone_form()));

    let title = app.get_by_text("Clone a repository").frame();
    let url = app.get_by_role(Role::TextField, "Repository URL").frame();
    let clone = app.get_by_role(Role::Button, "Clone").frame();
    assert_eq!(title.origin.x, 16.0);
    assert_eq!(title.origin.y, 16.0);
    assert_eq!(url.y(), title.max_y() + 8.0);
    // Colours and the least and most sizes.
    let title = app.get_by_text("Clone a repository").id();
    assert!(mitsuami_set_color(title.raw(), MitsuamiColorProp::TextColor as u32, 1, 0, 0, 0, 0));
    assert!(app.ui().props(title).contains(&Prop::TextColor(Color::SecondaryLabel)));
    assert!(mitsuami_set_color(title.raw(), MitsuamiColorProp::TextColor as u32, 9, 255, 0, 0, 255));
    assert!(app.ui().props(title).contains(&Prop::TextColor(Color::rgb(255, 0, 0))));
    assert!(!mitsuami_set_color(title.raw(), MitsuamiColorProp::TextColor as u32, 10, 0, 0, 0, 0));
    assert!(mitsuami_set_min_size(column.raw(), 600.0, -1.0));
    app.expect(by_text("Clone a repository")).to_be_visible().await;
    assert!(app.ui().frame(column).unwrap().width() >= 600.0);
    assert!(mitsuami_set_max_size(title.raw(), 50.0, -1.0));
    app.settle().await;
    assert!(app.ui().frame(title).unwrap().width() <= 50.0);
    // A narrower title wraps, moving what's under it: back as it was.
    assert!(mitsuami_set_max_size(title.raw(), -1.0, -1.0));
    app.settle().await;

    // A header is a prop: the core lays it out as any container.
    assert!(mitsuami_set_bool(column.raw(), MitsuamiBoolProp::Header as u32, true));
    assert!(mitsuami_set_enum(column.raw(), MitsuamiEnumProp::HeaderEdge as u32, 2));
    app.settle().await;
    assert!(app.ui().props(column).contains(&Prop::Header(true)));

    // A hidden node takes no room, and is back where it was when shown.
    let field = app.get_by_role(Role::TextField, "Repository URL").id().raw();
    assert!(mitsuami_set_hidden(field, true));
    app.expect(by_role(Role::TextField, "Repository URL")).to_be_hidden().await;
    assert!(mitsuami_set_hidden(field, false));
    app.expect(by_role(Role::TextField, "Repository URL")).to_have_frame(url).await;
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

#[mitsuami_test::test]
async fn search_fields_search_when_their_platform_does(app: TestApp) {
    attach(app.ui());
    heard();
    let search = create(MitsuamiKind::SearchInput);
    set_text(search, MitsuamiTextProp::A11yLabel, "Search repositories");
    assert!(mitsuami_on_event(search, record, std::ptr::null_mut()));
    app.mount(|| Adopt(search));

    app.get_by_role(Role::SearchField, "Search repositories").fill("mitsu").await;
    app.expect(by_role(Role::SearchField, "Search repositories")).to_exist().await;

    let heard = heard();
    assert!(heard.contains(&"changed mitsu".to_string()), "{heard:?}");
    assert!(heard.contains(&"search mitsu".to_string()), "{heard:?}");
}

#[mitsuami_test::test]
async fn document_tabs_are_data_the_caller_keeps(app: TestApp) {
    attach(app.ui());
    heard();
    let tabs = create(MitsuamiKind::DocumentTabs);
    let titles = strs(&["Welcome", "mitsuami", "sourcegit"]);
    assert!(unsafe {
        mitsuami_set_strings(tabs, MitsuamiStringsProp::TabTitles as u32, titles.as_ptr(), titles.len())
    });
    assert!(mitsuami_set_index(tabs, MitsuamiIndexProp::SelectedIndex as u32, 0));
    assert!(mitsuami_set_bool(tabs, MitsuamiBoolProp::TabsClosable as u32, true));
    assert!(mitsuami_set_bool(tabs, MitsuamiBoolProp::TabsReorderable as u32, true));
    assert!(mitsuami_set_enum(tabs, MitsuamiEnumProp::DocumentTabsStyle as u32, 1));
    assert!(mitsuami_on_event(tabs, record, std::ptr::null_mut()));
    app.mount(|| Adopt(tabs));

    app.get_by_role(Role::Tab, "mitsuami").select().await;
    app.get_by_role(Role::Tab, "sourcegit").close().await;
    app.get_by_role(Role::Tab, "Welcome").move_to(2).await;

    let heard = heard();
    assert!(heard.contains(&"chose 1".to_string()), "{heard:?}");
    assert!(heard.contains(&"close 2".to_string()), "{heard:?}");
    assert!(heard.contains(&"moved 0 to 2".to_string()), "{heard:?}");
    // A close only asks: the tab stays until the caller takes it away.
    app.expect(by_role(Role::Tab, "sourcegit")).to_exist().await;
}

#[mitsuami_test::test]
async fn dialogs_are_modal_to_their_window(app: TestApp) {
    attach(app.ui());
    heard();
    let title = "Clone";
    let dialog = unsafe { mitsuami_create_window(title.as_ptr(), title.len(), 400.0, -1.0) };
    assert_ne!(dialog, 0);
    assert!(mitsuami_set_modal(dialog, app.window().raw(), 0));
    assert!(mitsuami_on_event(dialog, record, std::ptr::null_mut()));
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, "Clone a repository");
    assert!(mitsuami_append_child(dialog, text));
    app.settle().await;

    let dialog = app.window_titled("Clone").expect("the dialog is open");
    let modal = app.ui().props(dialog).into_iter().find_map(|p| match p {
        Prop::Modal { owner, modality } => Some((owner, modality)),
        _ => None,
    });
    assert_eq!(modal, Some((Some(app.window()), Modality::Window)));

    // Closing it (Escape, on a sheet) destroys it, and the caller hears.
    app.close_window(dialog).await;
    assert!(app.window_titled("Clone").is_none());
    assert_eq!(heard(), ["event 5"]);
}

extern "C" fn alert_answer(_user: *mut c_void, index: usize) {
    HEARD.with(|h| h.borrow_mut().push(format!("button {index}")));
}

extern "C" fn open_answer(_user: *mut c_void, paths: *const MitsuamiStr, count: usize) {
    let paths: Vec<String> = (0..count)
        .map(|i| {
            let p = unsafe { &*paths.add(i) };
            String::from_utf8(unsafe { std::slice::from_raw_parts(p.ptr, p.len) }.to_vec()).unwrap()
        })
        .collect();
    HEARD.with(|h| h.borrow_mut().push(format!("paths {}", paths.join(","))));
}

fn str(s: &str) -> MitsuamiStr {
    MitsuamiStr { ptr: s.as_ptr(), len: s.len() }
}

#[mitsuami_test::test]
async fn alerts_and_folder_dialogs_answer_through_callbacks(app: TestApp) {
    attach(app.ui());
    heard();
    let buttons = strs(&["Delete", "Cancel"]);
    let window = app.window().raw();
    assert!(unsafe {
        mitsuami_alert(
            window,
            str("Delete the group?"),
            str(""),
            buttons.as_ptr(),
            2,
            2,
            alert_answer,
            std::ptr::null_mut(),
        )
    });
    assert!(unsafe {
        mitsuami_open_file(window, str("Parent folder"), true, false, str("/tmp"), open_answer, std::ptr::null_mut())
    });
    app.settle().await;

    let alert = app.services().take_alert().expect("an alert is showing");
    assert_eq!(alert.request.title, "Delete the group?");
    assert_eq!(alert.request.buttons, ["Delete", "Cancel"]);
    alert.respond(1);
    let open = app.services().take_open_file().expect("a folder dialog is showing");
    assert!(open.request.directories && !open.request.multiple);
    assert_eq!(open.request.start_folder, Some("/tmp".into()));
    open.respond(Some(vec!["/tmp/repos".into()]));
    app.settle().await;

    assert_eq!(heard(), ["button 1", "paths /tmp/repos"]);
}

#[mitsuami_test::test]
async fn drop_downs_report_the_option_chosen(app: TestApp) {
    attach(app.ui());
    heard();
    let select = create(MitsuamiKind::Select);
    set_text(select, MitsuamiTextProp::Label, "Group");
    let options = strs(&["None", "Work", "Personal"]);
    assert!(unsafe { mitsuami_set_strings(select, MitsuamiStringsProp::Options as u32, options.as_ptr(), 3) });
    assert!(mitsuami_set_index(select, MitsuamiIndexProp::SelectedIndex as u32, 0));
    assert!(mitsuami_on_event(select, record, std::ptr::null_mut()));
    app.mount(|| Adopt(select));

    app.get_by_role(Role::ComboBox, "Group").select_option("Personal").await;
    assert_eq!(heard(), ["chose 2"]);
}

#[mitsuami_test::test]
async fn icon_buttons_show_the_apps_own_icon(app: TestApp) {
    attach(app.ui());
    heard();
    let button = create(MitsuamiKind::Button);
    set_text(button, MitsuamiTextProp::Label, "Clone");
    set_text(button, MitsuamiTextProp::Tooltip, "Clone a repository");
    let plus = "M10 0H14V10H24V14H14V24H10V14H0V10H10Z";
    assert!(unsafe { mitsuami_set_vector_icon(button, str(plus), 0.0, 0.0, 0.0, 0.0) });
    assert!(mitsuami_set_bool(button, MitsuamiBoolProp::IconOnly as u32, true));
    assert!(mitsuami_on_event(button, record, std::ptr::null_mut()));
    app.mount(|| Adopt(button));

    let props = app.ui().props(NodeId::from_raw(button));
    assert!(props.contains(&Prop::VectorIcon(Some(VectorIcon::new(plus).unwrap()))));
    assert!(props.contains(&Prop::Tooltip("Clone a repository".into())));
    // Alone, its caption is still its name.
    app.get_by_role(Role::Button, "Clone").click().await;
    assert_eq!(heard(), ["click"]);

    let mut bounds = [0.0; 4];
    assert!(unsafe { mitsuami_vector_icon_bounds(str(plus), bounds.as_mut_ptr()) });
    assert_eq!(bounds, [0.0, 0.0, 24.0, 24.0]);

    assert!(!unsafe { mitsuami_set_vector_icon(button, str("M10 X"), 0.0, 0.0, 0.0, 0.0) });
    assert!(last_error().starts_with("bad path data"), "{}", last_error());
}

extern "C" fn captured(_user: *mut c_void, ok: bool) {
    HEARD.with(|h| h.borrow_mut().push(format!("captured {ok}")));
}

#[mitsuami_test::test]
async fn windows_capture_to_png_files(app: TestApp) {
    attach(app.ui());
    heard();
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, "Captured");
    app.mount(|| Adopt(text));
    let path = std::env::temp_dir().join(format!("mitsuami-ffi-capture-{}.png", std::process::id()));
    let path_str = path.to_string_lossy().into_owned();

    assert!(unsafe { mitsuami_capture_png(app.window().raw(), str(&path_str), captured, std::ptr::null_mut()) });
    app.expect(by_text("Captured")).to_exist().await;
    for _ in 0..50 {
        if HEARD.with(|h| !h.borrow().is_empty()) {
            break;
        }
        app.advance(std::time::Duration::from_millis(20)).await;
    }

    // Headless has no pixels to capture.
    let ok = !app.is_headless();
    assert_eq!(heard(), [format!("captured {ok}")]);
    if ok {
        let png = std::fs::read(&path).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        std::fs::remove_file(&path).unwrap();
    }
}

fn menu_entry(kind: u32, id: u32, title: &str, check: u32) -> MitsuamiMenuEntry {
    MitsuamiMenuEntry { kind, id, title: str(title), enabled: true, check }
}

#[mitsuami_test::test]
async fn context_menus_report_the_item_chosen(app: TestApp) {
    attach(app.ui());
    heard();
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, "sourcegit");
    let entries = [
        menu_entry(0, 1, "Open", 0),
        menu_entry(1, 0, "", 0),
        menu_entry(2, 0, "Bookmark", 0),
        menu_entry(0, 2, "None", 4),
        menu_entry(0, 3, "Red", 3),
        menu_entry(3, 0, "", 0),
        menu_entry(0, 4, "Delete", 0),
    ];
    assert!(unsafe { mitsuami_set_context_menu(text, entries.as_ptr(), entries.len()) });
    assert!(mitsuami_on_event(text, record, std::ptr::null_mut()));
    app.mount(|| Adopt(text));

    app.get_by_text("sourcegit").choose_menu_item(&["Bookmark", "Red"]).await;
    app.get_by_text("sourcegit").choose_menu_item(&["Delete"]).await;
    assert_eq!(heard(), ["menu 3", "menu 4"]);

    let unended = [menu_entry(2, 0, "Bookmark", 0)];
    assert!(!unsafe { mitsuami_set_context_menu(text, unended.as_ptr(), 1) });
}

#[mitsuami_test::test]
async fn containers_take_dropped_folders(app: TestApp) {
    attach(app.ui());
    heard();
    let dir = std::env::temp_dir().join(format!("mitsuami-ffi-drop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("repo")).unwrap();
    std::fs::write(dir.join("notes.txt"), b"").unwrap();

    let zone = create(MitsuamiKind::Container);
    assert!(mitsuami_set_size(zone, 200.0, 100.0));
    set_text(zone, MitsuamiTextProp::TestId, "repositories");
    assert!(unsafe { mitsuami_set_file_drop(zone, true, false, std::ptr::null(), 0) });
    assert!(mitsuami_on_event(zone, record, std::ptr::null_mut()));
    app.mount(|| Adopt(zone));

    // Folders only: the file is left out.
    app.get(by_test_id("repositories")).drop_files(&[dir.join("repo"), dir.join("notes.txt")]).await;
    let heard = heard();
    assert!(heard.contains(&"dropped repo".to_string()), "{heard:?}");
    std::fs::remove_dir_all(&dir).unwrap();
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

extern "C" fn build_row(_user: *mut c_void, _list: u32, key: *const u8, len: usize) -> u32 {
    let key = std::str::from_utf8(unsafe { std::slice::from_raw_parts(key, len) }).unwrap();
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, &format!("Repository {key}"));
    text
}

extern "C" fn release_row(_user: *mut c_void, _list: u32, _row: u32) {
    HEARD.with(|h| h.borrow_mut().push("released".to_string()));
}

extern "C" fn selection_changed(_user: *mut c_void, list: u32) {
    let keys: Vec<String> = (0..mitsuami_list_selected_count(list))
        .map(|i| {
            let mut ptr = std::ptr::null();
            let len = unsafe { mitsuami_list_selected_key(list, i, &mut ptr) };
            String::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()).unwrap()
        })
        .collect();
    HEARD.with(|h| h.borrow_mut().push(format!("selected {}", keys.join(","))));
}

extern "C" fn moved(
    _user: *mut c_void,
    _list: u32,
    keys: *const MitsuamiStr,
    count: usize,
    to: u32,
    key: *const u8,
    len: usize,
) {
    let text = |ptr, len| std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }).unwrap().to_string();
    let keys: Vec<String> = (0..count).map(|i| unsafe { &*keys.add(i) }).map(|k| text(k.ptr, k.len)).collect();
    let to = match to {
        0 => format!("onto {}", text(key, len)),
        1 => format!("before {}", text(key, len)),
        _ => "at the end".to_string(),
    };
    HEARD.with(|h| h.borrow_mut().push(format!("moved {} {to}", keys.join(","))));
}

extern "C" fn activated(_user: *mut c_void, _list: u32, key: *const u8, len: usize) {
    let key = std::str::from_utf8(unsafe { std::slice::from_raw_parts(key, len) }).unwrap();
    HEARD.with(|h| h.borrow_mut().push(format!("activated {key}")));
}

fn strs(keys: &[&str]) -> Vec<MitsuamiStr> {
    keys.iter().map(|k| MitsuamiStr { ptr: k.as_ptr(), len: k.len() }).collect()
}

#[mitsuami_test::test]
async fn lists_ask_for_rows_and_report_the_selection(app: TestApp) {
    attach(app.ui());
    heard();
    let list = mitsuami_create_list(MitsuamiListCallbacks {
        build_row,
        release_row,
        selection_changed,
        activated,
        user: std::ptr::null_mut(),
        moves: 1,
        moved,
    });
    assert_ne!(list, 0);
    assert!(mitsuami_set_size(list, 300.0, 200.0));
    let keys = strs(&["a", "b", "c"]);
    assert!(unsafe { mitsuami_list_set_keys(list, keys.as_ptr(), keys.len()) });
    app.mount(|| Adopt(list));
    app.expect(by_role(Role::ListItem, "Repository c")).to_be_visible().await;

    // The caller's own selection isn't reported back as the user's.
    let selected = strs(&["a"]);
    assert!(unsafe { mitsuami_list_set_selected(list, selected.as_ptr(), selected.len()) });
    app.settle().await;
    assert!(app.get_by_role(Role::ListItem, "Repository a").node().selected == Some(true));
    assert_eq!(heard(), Vec::<String>::new());

    app.get_by_role(Role::ListItem, "Repository c").select().await;
    app.get_by_role(Role::ListItem, "Repository b").click().await;
    let heard_now = heard();
    assert!(heard_now.contains(&"selected c".to_string()), "{heard_now:?}");
    assert!(heard_now.contains(&"activated b".to_string()), "{heard_now:?}");

    // Rows dragged onto another (the list takes drops onto rows).
    app.get_by_role(Role::ListItem, "Repository a").drag_onto(&app.get_by_role(Role::ListItem, "Repository b")).await;
    assert_eq!(heard(), ["moved a onto b"]);

    // Rows that go are let go, and the caller hears it.
    let keys = strs(&["a"]);
    assert!(unsafe { mitsuami_list_set_keys(list, keys.as_ptr(), keys.len()) });
    app.expect(by_role(Role::ListItem, "Repository c")).not_to_exist().await;
    assert!(heard().contains(&"released".to_string()));
}

mitsuami_test::main!();
