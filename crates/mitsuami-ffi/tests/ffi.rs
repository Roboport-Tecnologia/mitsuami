//! The C ABI drives the same `Ui` the builder API does: nodes made through
//! it are laid out, found, clicked and typed into as any others, and their
//! events come back through the callback.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use mitsuami::core::services::{Launch, Shortcut};
use mitsuami::core::{Color, ImageSource, Modality, NodeId, Pixels, Prop, Size, TextStyle, Ui, VectorIcon, View};
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
        1 if event.value_kind == MitsuamiValueKind::Number as u32 => format!("changed {}", event.number),
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
        13 => format!("pointer {} at {},{}", if event.flag { "down" } else { "up" }, event.x, event.y),
        14 if event.value_kind == MitsuamiValueKind::Text as u32 => {
            let text = unsafe { std::slice::from_raw_parts(event.text, event.text_len) };
            format!("action {} {}", event.index, std::str::from_utf8(text).unwrap())
        }
        14 => format!("action {}", event.index),
        15 => format!("item {}", event.index),
        16 => format!("popover shown {}", event.flag),
        17 => format!("suggestion {}", event.index),
        18 => format!("key {}", event.index),
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

fn px(points: f32) -> MitsuamiLength {
    MitsuamiLength { unit: 1, value: points }
}

fn auto() -> MitsuamiLength {
    MitsuamiLength { unit: 0, value: 0.0 }
}

fn create(kind: MitsuamiKind) -> u32 {
    let id = mitsuami_create(kind as u32);
    assert_ne!(id, 0);
    id
}

/// Clone's form, as a binding would build it.
fn clone_form() -> u32 {
    let column = create(MitsuamiKind::Container);
    assert!(mitsuami_set_padding(column, px(16.0), px(16.0), px(16.0), px(16.0)));
    assert!(mitsuami_set_gap(column, px(8.0)));
    assert!(mitsuami_set_align_items(column, 1));

    let title = create(MitsuamiKind::Text);
    set_text(title, MitsuamiTextProp::Text, "Clone a repository");
    assert!(mitsuami_set_enum(title, MitsuamiEnumProp::TextStyle as u32, 2));
    assert!(mitsuami_set_enum(title, MitsuamiEnumProp::FontWeight as u32, 3));

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
    assert!(mitsuami_set_color(title.raw(), MitsuamiColorProp::TextColor as u32, 10, 0, 0, 0, 0));
    assert!(app.ui().props(title).contains(&Prop::TextColor(Color::Placeholder)));
    assert!(!mitsuami_set_color(title.raw(), MitsuamiColorProp::TextColor as u32, 11, 0, 0, 0, 0));
    assert!(mitsuami_set_min_size(column.raw(), px(600.0), auto()));
    app.expect(by_text("Clone a repository")).to_be_visible().await;
    assert!(app.ui().frame(column).unwrap().width() >= 600.0);
    assert!(mitsuami_set_max_size(title.raw(), px(50.0), auto()));
    app.settle().await;
    assert!(app.ui().frame(title).unwrap().width() <= 50.0);
    // A narrower title wraps, moving what's under it: back as it was.
    assert!(mitsuami_set_max_size(title.raw(), auto(), auto()));
    app.settle().await;

    // Lengths in the platform's tokens: a tree's indentation, say.
    let token = |t: f32| MitsuamiLength { unit: 2, value: t };
    assert!(mitsuami_set_padding(column.raw(), token(6.0), px(16.0), px(16.0), px(16.0)));
    app.settle().await;
    let indent = app.ui().metrics().spacing.indent;
    assert_eq!(app.get_by_text("Clone a repository").frame().y(), indent.round());
    assert!(mitsuami_set_padding(column.raw(), px(16.0), px(16.0), px(16.0), px(16.0)));
    assert!(!mitsuami_set_gap(column.raw(), token(7.0)));

    // A header is a prop: the core lays it out as any container.
    assert!(mitsuami_set_bool(column.raw(), MitsuamiBoolProp::Header as u32, true));
    assert!(mitsuami_set_enum(column.raw(), MitsuamiEnumProp::HeaderEdge as u32, 2));
    app.settle().await;
    assert!(app.ui().props(column).contains(&Prop::Header(true)));
    assert!(mitsuami_set_bool(column.raw(), MitsuamiBoolProp::SidebarPane as u32, true));
    assert!(app.ui().props(column).contains(&Prop::SidebarPane(true)));

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
async fn save_dialogs_answer_with_the_path_or_none(app: TestApp) {
    attach(app.ui());
    heard();
    let window = app.window().raw();
    for _ in 0..2 {
        assert!(unsafe {
            mitsuami_save_file(window, str("Archive"), str("main.zip"), str("/tmp"), open_answer, std::ptr::null_mut())
        });
    }
    app.settle().await;

    let save = app.services().take_save_file().expect("a save dialog is showing");
    assert_eq!(save.request.title.as_deref(), Some("Archive"));
    assert_eq!(save.request.default_name.as_deref(), Some("main.zip"));
    assert_eq!(save.request.start_folder, Some("/tmp".into()));
    save.respond(Some("/tmp/main.zip".into()));
    app.services().take_save_file().expect("a second save dialog").respond(None);
    app.settle().await;

    assert_eq!(heard(), ["paths /tmp/main.zip", "paths "]);
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

/// Keys a list doesn't use itself reach it, a letter included (AppKit's
/// table would take it to select a row by typing).
#[mitsuami_test::test]
async fn lists_take_keys_their_control_passes_on(app: TestApp) {
    attach(app.ui());
    heard();
    let list = mitsuami_create_list(MitsuamiListCallbacks {
        build_row,
        release_row,
        selection_changed,
        activated,
        user: std::ptr::null_mut(),
        moves: 0,
        moved,
        still: 1,
    });
    assert!(mitsuami_set_size(list, px(300.0), px(200.0)));
    let keys = strs(&["a", "b"]);
    assert!(unsafe { mitsuami_list_set_keys(list, keys.as_ptr(), keys.len()) });
    let shortcut = |key: MitsuamiKey, ch: char, primary| MitsuamiShortcut {
        key: key as u32,
        ch: ch as u32,
        primary,
        shift: false,
        alt: false,
    };
    let taken = [
        shortcut(MitsuamiKey::Char, 'p', false),
        shortcut(MitsuamiKey::F, '\u{2}', false),
        shortcut(MitsuamiKey::Char, 'd', true),
    ];
    assert!(unsafe { mitsuami_set_keys(list, taken.as_ptr(), taken.len()) });
    assert!(mitsuami_on_event(list, record, std::ptr::null_mut()));
    set_text(list, MitsuamiTextProp::A11yLabel, "Commits");
    app.mount(|| Adopt(list));
    app.expect(by_role(Role::ListItem, "Repository b")).to_be_visible().await;
    heard();

    let commits = app.get_by_role(Role::List, "Commits");
    commits.press('p').await;
    commits.press(Key::F(2)).await;
    commits.press(Shortcut::primary('d')).await;
    assert_eq!(heard(), ["key 0", "key 1", "key 2"]);

    let bad = [MitsuamiShortcut { key: 99, ch: 0, primary: false, shift: false, alt: false }];
    assert!(!unsafe { mitsuami_set_keys(list, bad.as_ptr(), 1) });
    assert!(last_error().starts_with("unknown key 99"), "{}", last_error());
}

#[mitsuami_test::test]
async fn radio_groups_report_the_option_chosen(app: TestApp) {
    attach(app.ui());
    heard();
    let group = create(MitsuamiKind::RadioGroup);
    set_text(group, MitsuamiTextProp::Label, "Local changes");
    let options = strs(&["Do nothing", "Stash and reapply", "Discard"]);
    assert!(unsafe { mitsuami_set_strings(group, MitsuamiStringsProp::Options as u32, options.as_ptr(), 3) });
    assert!(mitsuami_set_index(group, MitsuamiIndexProp::SelectedIndex as u32, 0));
    assert!(mitsuami_on_event(group, record, std::ptr::null_mut()));
    app.mount(|| Adopt(group));

    assert!(app.get_by_role(Role::RadioButton, "Do nothing").is_checked());
    app.get_by_role(Role::RadioButton, "Discard").click().await;
    assert_eq!(heard(), ["chose 2"]);
    assert!(app.get_by_role(Role::RadioButton, "Discard").is_checked());
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
async fn menu_buttons_report_the_item_chosen(app: TestApp) {
    attach(app.ui());
    heard();
    let button = create(MitsuamiKind::MenuButton);
    set_text(button, MitsuamiTextProp::Label, "Open in Browser");
    let entries = [menu_entry(0, 1, "GitHub", 0), menu_entry(0, 2, "Gitee", 0)];
    assert!(unsafe { mitsuami_set_menu(button, entries.as_ptr(), entries.len()) });
    assert!(mitsuami_on_event(button, record, std::ptr::null_mut()));
    app.mount(|| Adopt(button));

    app.get_by_role(Role::MenuButton, "Open in Browser").choose_menu_item(&["Gitee"]).await;
    assert_eq!(heard(), ["item 2"]);
}

#[mitsuami_test::test]
async fn split_buttons_report_clicks_and_the_item_chosen(app: TestApp) {
    attach(app.ui());
    heard();
    let button = create(MitsuamiKind::SplitButton);
    set_text(button, MitsuamiTextProp::Label, "Continue");
    let entries = [menu_entry(0, 1, "Commit to Edit", 0)];
    assert!(unsafe { mitsuami_set_menu(button, entries.as_ptr(), entries.len()) });
    assert!(mitsuami_on_event(button, record, std::ptr::null_mut()));
    app.mount(|| Adopt(button));

    app.get_by_role(Role::Button, "Continue").click().await;
    app.get_by_role(Role::Button, "Continue").choose_menu_item(&["Commit to Edit"]).await;
    assert_eq!(heard(), ["click", "item 1"]);
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
    assert!(mitsuami_set_size(zone, px(200.0), px(100.0)));
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
        still: 1,
    });
    assert_ne!(list, 0);
    assert!(mitsuami_set_size(list, px(300.0), px(200.0)));
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

    // The user picking again what the caller selected is the user's.
    assert!(unsafe { mitsuami_list_set_selected(list, selected.as_ptr(), selected.len()) });
    app.settle().await;
    app.get_by_role(Role::ListItem, "Repository c").select().await;
    app.get_by_role(Role::ListItem, "Repository a").select().await;
    assert_eq!(heard(), ["selected c", "selected a"]);

    // Rows dragged onto another (the list takes drops onto rows).
    app.get_by_role(Role::ListItem, "Repository a").drag_onto(&app.get_by_role(Role::ListItem, "Repository b")).await;
    assert_eq!(heard(), ["moved a onto b"]);

    // Rows that go are let go, and the caller hears it.
    let keys = strs(&["a"]);
    assert!(unsafe { mitsuami_list_set_keys(list, keys.as_ptr(), keys.len()) });
    app.expect(by_role(Role::ListItem, "Repository c")).not_to_exist().await;
    assert!(heard().contains(&"released".to_string()));
}

/// Two bookmark swatches, the chosen one framed: what a binding's drawn
/// widget draws.
extern "C" fn draw_swatches(_user: *mut c_void, _node: u32, canvas: *mut MitsuamiCanvas, width: f32, height: f32) {
    HEARD.with(|h| h.borrow_mut().push(format!("draw {width}x{height}")));
    let accent = MitsuamiColor { semantic: 2, r: 0, g: 0, b: 0, a: 0 };
    let red = MitsuamiColor { semantic: 9, r: 255, g: 0, b: 0, a: 255 };
    let blue = MitsuamiColor { semantic: 9, r: 0, g: 0, b: 255, a: 255 };
    let bookmark = str("M0 0H10V14L5 10L0 14Z");
    unsafe {
        assert!(mitsuami_canvas_rect(canvas, 0.0, 2.0, 18.0, 20.0, 3.0, 1.0, accent));
        assert!(mitsuami_canvas_path(canvas, bookmark, 0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 14.0, 0.0, red));
        assert!(mitsuami_canvas_path(canvas, bookmark, 0.0, 0.0, 0.0, 0.0, 28.0, 5.0, 14.0, 0.0, blue));
        assert!(!mitsuami_canvas_ellipse(canvas, 0.0, 0.0, 4.0, 4.0, 0.0, MitsuamiColor { semantic: 42, ..red }));
    }
}

#[mitsuami_test::test]
async fn drawn_widgets_are_drawn_by_the_caller(app: TestApp) {
    attach(app.ui());
    heard();
    let callbacks = MitsuamiDrawnCallbacks { draw: draw_swatches, user: std::ptr::null_mut() };
    let picker = unsafe { mitsuami_create_drawn(str("Bookmarks"), callbacks, 52.0, 24.0) };
    assert_ne!(picker, 0);
    set_text(picker, MitsuamiTextProp::A11yLabel, "Bookmark");
    let actions = MitsuamiDrawnAction::SetValue as u32 | MitsuamiDrawnAction::Increment as u32;
    assert!(unsafe { mitsuami_drawn_set_semantics(picker, 3, str("Red"), actions) });
    assert!(mitsuami_on_event(picker, record, std::ptr::null_mut()));
    let column = create(MitsuamiKind::Container);
    assert!(mitsuami_set_align_items(column, 1));
    assert!(mitsuami_append_child(column, picker));
    app.mount(|| Adopt(column));

    let slider = app.get_by_role(Role::Slider, "Bookmark");
    assert_eq!(slider.frame().size, Size::new(52.0, 24.0));
    assert_eq!(slider.value().as_deref(), Some("Red"));
    let drawing = app.ui().props(NodeId::from_raw(picker)).into_iter().find_map(|p| match p {
        Prop::Drawing(d) => Some(d),
        _ => None,
    });
    // The ellipse in a colour that isn't one was refused.
    assert_eq!(drawing.unwrap().ops().len(), 3);
    assert_eq!(heard(), ["draw 52x24"]);

    // The caller decides what pointers and actions mean.
    slider.click_at(30.0, 12.0).await;
    assert_eq!(heard(), ["pointer down at 30,12", "pointer up at 30,12"]);
    slider.select_option("Blue").await;
    slider.increment().await;
    assert_eq!(heard(), ["action 2 Blue", "action 4"]);

    // Its state is the caller's: it says when to draw again.
    assert!(unsafe { mitsuami_drawn_set_semantics(picker, 3, str("Blue"), actions) });
    assert!(mitsuami_drawn_redraw(picker));
    app.settle().await;
    assert_eq!(slider.value().as_deref(), Some("Blue"));
    assert_eq!(heard(), ["draw 52x24"]);
}

#[mitsuami_test::test]
async fn splits_lay_out_their_panes_and_report_the_divider(app: TestApp) {
    attach(app.ui());
    heard();
    let split = create(MitsuamiKind::Split);
    let (first, second) = (create(MitsuamiKind::Container), create(MitsuamiKind::Container));
    set_text(first, MitsuamiTextProp::TestId, "first");
    set_text(second, MitsuamiTextProp::TestId, "second");
    set_text(split, MitsuamiTextProp::TestId, "split");
    assert!(mitsuami_set_min_size(first, px(120.0), auto()));
    assert!(mitsuami_append_child(split, first));
    assert!(mitsuami_append_child(split, second));
    assert!(mitsuami_set_number(split, MitsuamiNumberProp::Number as u32, 200.0));
    assert!(mitsuami_set_enum(split, MitsuamiEnumProp::Orientation as u32, 0));
    assert!(mitsuami_on_event(split, record, std::ptr::null_mut()));
    // Its panes are empty: it's as high as the window lets it grow.
    assert!(mitsuami_set_grow(split, 1.0));
    app.mount(|| Adopt(split));
    app.resize(800.0, 400.0).await;

    let divider = app.ui().metrics().split_divider;
    assert_eq!(app.get_by_test_id("first").frame().width(), 200.0);
    assert_eq!(app.get_by_test_id("second").frame().x(), 200.0 + divider);

    // The user's move is reported; the least width stops it.
    app.get_by_test_id("split").set_number(40.0).await;
    assert_eq!(heard(), ["changed 120"]);
    assert_eq!(app.get_by_test_id("first").frame().width(), 120.0);
}

#[mitsuami_test::test]
async fn containers_report_clicks_when_asked(app: TestApp) {
    attach(app.ui());
    heard();
    let header = create(MitsuamiKind::Container);
    let label = create(MitsuamiKind::Text);
    set_text(label, MitsuamiTextProp::Text, "LOCAL BRANCHES");
    set_text(header, MitsuamiTextProp::TestId, "header");
    assert!(mitsuami_append_child(header, label));
    assert!(mitsuami_set_bool(header, MitsuamiBoolProp::Clickable as u32, true));
    assert!(mitsuami_on_event(header, record, std::ptr::null_mut()));
    app.mount(|| Adopt(header));

    app.get_by_test_id("header").click().await;
    assert_eq!(heard(), ["click"]);
}

extern "C" fn build_cell(_user: *mut c_void, _table: u32, key: *const u8, len: usize, column: usize) -> u32 {
    let key = std::str::from_utf8(unsafe { std::slice::from_raw_parts(key, len) }).unwrap();
    let text = create(MitsuamiKind::Text);
    let shown = if column == 0 { format!("Commit {key}") } else { format!("{key}@example.com") };
    set_text(text, MitsuamiTextProp::Text, &shown);
    // A subject on one line, as history views show it.
    assert!(mitsuami_set_number(text, MitsuamiNumberProp::MaxLines as u32, 1.0));
    text
}

extern "C" fn sorted(_user: *mut c_void, _table: u32, column: usize, descending: bool) {
    let order = if descending { "descending" } else { "ascending" };
    HEARD.with(|h| h.borrow_mut().push(format!("sorted {column} {order}")));
}

#[mitsuami_test::test]
async fn tables_ask_for_cells_and_report_the_sort(app: TestApp) {
    attach(app.ui());
    heard();
    let column = |title: &'static str, width: f32, expand: bool| MitsuamiTableColumn {
        title: str(title),
        width,
        expand,
        sortable: true,
    };
    let columns = [column("Subject", 0.0, true), column("Author", 120.0, false)];
    let callbacks = MitsuamiTableCallbacks {
        build_cell,
        release_cell: release_row,
        selection_changed,
        activated,
        sorted,
        user: std::ptr::null_mut(),
        still: 1,
    };
    let table = unsafe { mitsuami_create_table(callbacks, columns.as_ptr(), columns.len()) };
    assert_ne!(table, 0);
    assert!(mitsuami_set_size(table, px(400.0), px(200.0)));
    assert!(mitsuami_set_enum(table, MitsuamiEnumProp::SelectionMode as u32, 2));
    let keys = strs(&["a", "b", "c"]);
    assert!(unsafe { mitsuami_list_set_keys(table, keys.as_ptr(), keys.len()) });
    app.mount(|| Adopt(table));
    app.expect(by_role(Role::Row, "Commit c c@example.com")).to_be_visible().await;

    let subject = app.get_by_text("Commit b").id();
    assert!(app.ui().props(subject).contains(&Prop::MaxLines(Some(1))));

    // A row's cells side by side, in column order.
    let (subject, author) =
        (app.get_by_role(Role::Cell, "Commit b").frame(), app.get_by_role(Role::Cell, "b@example.com").frame());
    assert!(subject.max_x() <= author.x(), "{subject:?} {author:?}");

    app.get_by_role(Role::Cell, "Commit b").select().await;
    app.get_by_role(Role::Cell, "Commit c").click().await;
    let heard_now = heard();
    assert!(heard_now.contains(&"selected b".to_string()), "{heard_now:?}");
    assert!(heard_now.contains(&"activated c".to_string()), "{heard_now:?}");

    // The user picking again what the caller selected is the user's.
    let selected = strs(&["a"]);
    assert!(unsafe { mitsuami_list_set_selected(table, selected.as_ptr(), selected.len()) });
    app.settle().await;
    app.get_by_role(Role::Cell, "Commit c").select().await;
    app.get_by_role(Role::Cell, "Commit a").select().await;
    assert_eq!(heard(), ["selected c", "selected a"]);

    // The caller's sort isn't reported back; the user's is, as the
    // platform sorts (another column ascending, the same one reversed).
    assert!(mitsuami_table_set_sort(table, 1, true));
    app.settle().await;
    assert_eq!(heard(), Vec::<String>::new());
    app.get_by_role(Role::ColumnHeader, "Subject").click().await;
    app.get_by_role(Role::ColumnHeader, "Subject").click().await;
    assert_eq!(heard(), ["sorted 0 ascending", "sorted 0 descending"]);

    // Rows that go let their cells go.
    let keys = strs(&["a"]);
    assert!(unsafe { mitsuami_list_set_keys(table, keys.as_ptr(), keys.len()) });
    app.expect(by_role(Role::Row, "Commit c c@example.com")).not_to_exist().await;
    assert!(heard().contains(&"released".to_string()));
}

extern "C" fn draw_nothing(_user: *mut c_void, _node: u32, _canvas: *mut MitsuamiCanvas, _width: f32, _height: f32) {}

#[mitsuami_test::test]
async fn absolute_nodes_sit_over_the_others(app: TestApp) {
    attach(app.ui());
    // A branch's label: a drawn backdrop filling it, behind its name.
    let badge = create(MitsuamiKind::Container);
    assert!(mitsuami_set_direction(badge, true));
    assert!(mitsuami_set_align_self(badge, 1));
    assert!(mitsuami_set_padding(badge, px(0.0), px(4.0), px(0.0), px(20.0)));
    let callbacks = MitsuamiDrawnCallbacks { draw: draw_nothing, user: std::ptr::null_mut() };
    let backdrop = unsafe { mitsuami_create_drawn(str("Backdrop"), callbacks, 0.0, 0.0) };
    set_text(backdrop, MitsuamiTextProp::TestId, "backdrop");
    assert!(mitsuami_set_position(backdrop, true, px(0.0), px(0.0), px(0.0), px(0.0)));
    let name = create(MitsuamiKind::Text);
    set_text(name, MitsuamiTextProp::Text, "main");
    assert!(mitsuami_append_child(badge, backdrop));
    assert!(mitsuami_append_child(badge, name));
    set_text(badge, MitsuamiTextProp::TestId, "badge");
    let column = create(MitsuamiKind::Container);
    assert!(mitsuami_append_child(column, badge));
    app.mount(|| Adopt(column));
    app.settle().await;

    // The backdrop takes the badge's frame; the name is laid out as if it
    // weren't there.
    let (frame, behind) = (app.get_by_test_id("badge").frame(), app.get_by_test_id("backdrop").frame());
    assert_eq!(behind, frame);
    let label = app.get_by_text("main").frame();
    assert_eq!(label.x(), frame.x() + 20.0);
    assert_eq!(frame.width(), label.width() + 24.0);

    // Back in the flow, it takes room again.
    assert!(mitsuami_set_position(backdrop, false, auto(), auto(), auto(), auto()));
    app.settle().await;
    assert!(app.get_by_text("main").frame().x() >= frame.x() + 20.0);
}

#[mitsuami_test::test]
async fn tabs_show_one_page_and_report_the_user_s_pick(app: TestApp) {
    attach(app.ui());
    heard();
    let tabs = create(MitsuamiKind::Tabs);
    let titles = strs(&["Information", "Changes"]);
    assert!(unsafe {
        mitsuami_set_strings(tabs, MitsuamiStringsProp::TabTitles as u32, titles.as_ptr(), titles.len())
    });
    assert!(mitsuami_set_enum(tabs, MitsuamiEnumProp::TabsStyle as u32, 1));
    for text in ["Author", "Files changed"] {
        let page = create(MitsuamiKind::Container);
        let label = create(MitsuamiKind::Text);
        set_text(label, MitsuamiTextProp::Text, text);
        assert!(mitsuami_append_child(page, label));
        assert!(mitsuami_append_child(tabs, page));
    }
    assert!(mitsuami_set_index(tabs, MitsuamiIndexProp::SelectedIndex as u32, 0));
    assert!(mitsuami_set_size(tabs, px(400.0), px(200.0)));
    assert!(mitsuami_on_event(tabs, record, std::ptr::null_mut()));
    app.mount(|| Adopt(tabs));
    app.expect(by_text("Author")).to_be_visible().await;

    app.get_by_role(Role::Tab, "Changes").select().await;
    assert_eq!(heard(), ["chose 1"]);
    app.expect(by_text("Files changed")).to_be_visible().await;

    // The caller's pick isn't reported back.
    assert!(mitsuami_set_index(tabs, MitsuamiIndexProp::SelectedIndex as u32, 0));
    app.expect(by_text("Author")).to_be_visible().await;
    assert_eq!(heard(), Vec::<String>::new());
}

#[mitsuami_test::test]
async fn scroll_views_scroll_their_content(app: TestApp) {
    attach(app.ui());
    let mut content = 0;
    let scroll = unsafe { mitsuami_create_scroll_view(0, &mut content) };
    assert_ne!(scroll, 0);
    assert_ne!(content, 0);
    for i in 0..60 {
        let line = create(MitsuamiKind::Text);
        set_text(line, MitsuamiTextProp::Text, &format!("Line {i}"));
        assert!(mitsuami_append_child(content, line));
    }
    set_text(scroll, MitsuamiTextProp::TestId, "scroll");
    assert!(mitsuami_set_size(scroll, px(300.0), px(120.0)));
    app.mount(|| Adopt(scroll));
    app.settle().await;

    // Its frame as laid out, for the caller's own sizing.
    let mut frame = [0f32; 4];
    assert!(unsafe { mitsuami_frame(scroll, frame.as_mut_ptr()) });
    assert_eq!(frame[2..], [300.0, 120.0]);

    // The content is as wide as the view, and longer: it scrolls.
    assert_eq!(app.get_by_text("Line 0").frame().width(), app.get_by_test_id("scroll").frame().width());
    app.get_by_test_id("scroll").scroll_by(0.0, 1e6).await;
    app.expect(by_text("Line 59")).to_be_visible().await;
}

mitsuami_test::main!();

#[mitsuami_test::test]
async fn images_show_the_callers_pixels(app: TestApp) {
    attach(app.ui());
    let image = create(MitsuamiKind::Image);
    // 4 × 2 pixels at 2 to a point: 2 × 1 points.
    let rgba: Vec<u8> = (0..8).flat_map(|i| [i * 30, 0, 0, 255]).collect();
    assert!(unsafe { mitsuami_set_image(image, 4, 2, 2.0, rgba.as_ptr(), rgba.len()) });
    assert!(mitsuami_set_align_self(image, 1));
    app.mount(|| Adopt(image));
    app.settle().await;

    let pixels = Pixels::new(4, 2, rgba.clone()).scale(2.0);
    assert!(app.ui().props(NodeId::from_raw(image)).contains(&Prop::Image(ImageSource::Pixels(pixels))));
    assert_eq!(app.ui().frame(NodeId::from_raw(image)).map(|f| f.size), Some(Size::new(2.0, 1.0)));

    assert!(!unsafe { mitsuami_set_image(image, 4, 4, 2.0, rgba.as_ptr(), rgba.len()) });
    assert!(last_error().contains("RGBA8"), "{}", last_error());
}

#[mitsuami_test::test]
async fn texts_can_be_selectable(app: TestApp) {
    attach(app.ui());
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, "Fix the graph's curves");
    assert!(mitsuami_set_bool(text, MitsuamiBoolProp::Selectable as u32, true));
    app.mount(|| Adopt(text));
    app.settle().await;
    assert!(app.ui().props(NodeId::from_raw(text)).contains(&Prop::Selectable(true)));
}

#[mitsuami_test::test]
async fn the_clipboard_and_links_go_to_the_platform(app: TestApp) {
    attach(app.ui());
    heard();
    assert!(unsafe { mitsuami_set_clipboard_text(str("1a2b3c")) });
    app.settle().await;
    assert_eq!(app.services().clipboard().as_deref(), Some("1a2b3c"));
    assert!(mitsuami_clipboard_text(open_answer, std::ptr::null_mut()));
    app.settle().await;
    assert_eq!(heard(), ["paths 1a2b3c"]);

    let window = app.window().raw();
    assert!(unsafe { mitsuami_launch(window, str("https://example.com/commit/1a2b3c"), true) });
    app.settle().await;
    let launch = app.services().take_launch().expect("a request to open the link");
    assert_eq!(launch.request, Launch::Url("https://example.com/commit/1a2b3c".into()));
    assert_eq!(launch.parent, Some(app.window()));
    launch.respond(Ok(()));
}

/// Children that don't fit give up room by their shrink: one that never
/// shrinks keeps its height, one at its least height stops there.
#[mitsuami_test::test]
async fn children_shrink_as_told(app: TestApp) {
    attach(app.ui());
    let column = create(MitsuamiKind::Container);
    assert!(mitsuami_set_size(column, auto(), px(300.0)));
    let (kept, floor, rest) =
        (create(MitsuamiKind::Container), create(MitsuamiKind::Container), create(MitsuamiKind::Container));
    for (child, height) in [(kept, 100.0), (floor, 400.0), (rest, 200.0)] {
        assert!(mitsuami_set_size(child, auto(), px(height)));
        assert!(mitsuami_append_child(column, child));
    }
    assert!(mitsuami_set_shrink(kept, 0.0));
    assert!(mitsuami_set_min_size(floor, auto(), px(120.0)));
    assert!(mitsuami_set_shrink(floor, 1000.0));
    app.mount(|| Adopt(column));
    app.settle().await;

    let height = |id| app.ui().frame(NodeId::from_raw(id)).unwrap().height();
    assert_eq!((height(kept), height(floor)), (100.0, 120.0));
    assert_eq!(height(rest), 80.0);
    assert!(!mitsuami_set_shrink(kept, -1.0));
}

#[mitsuami_test::test]
async fn lists_scroll_to_a_row(app: TestApp) {
    attach(app.ui());
    let list = mitsuami_create_list(MitsuamiListCallbacks {
        build_row,
        release_row,
        selection_changed,
        activated,
        user: std::ptr::null_mut(),
        moves: 0,
        moved,
        still: 0,
    });
    assert!(mitsuami_set_size(list, px(300.0), px(200.0)));
    let names: Vec<String> = (0..500).map(|i| format!("r{i}")).collect();
    let keys: Vec<MitsuamiStr> = names.iter().map(|k| MitsuamiStr { ptr: k.as_ptr(), len: k.len() }).collect();
    assert!(unsafe { mitsuami_list_set_keys(list, keys.as_ptr(), keys.len()) });
    app.mount(|| Adopt(list));
    app.settle().await;
    app.expect(by_role(Role::ListItem, "Repository r400")).not_to_exist().await;

    assert!(unsafe { mitsuami_list_scroll_to(list, str("r400")) });
    app.settle().await;
    assert!(app.get_by_role(Role::ListItem, "Repository r400").is_visible());
    assert!(!unsafe { mitsuami_list_scroll_to(create(MitsuamiKind::Text), str("r400")) });
}

thread_local! {
    static OPENED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A menu that says how many times it was built.
extern "C" fn build_counted_menu(_user: *mut c_void, node: u32) {
    let times = OPENED.with(|o| {
        o.set(o.get() + 1);
        o.get()
    });
    let title = format!("Opened {times}");
    let entries = [menu_entry(0, times, &title, 0)];
    assert!(unsafe { mitsuami_set_context_menu(node, entries.as_ptr(), entries.len()) });
}

#[mitsuami_test::test]
async fn context_menus_can_be_built_as_they_open(app: TestApp) {
    attach(app.ui());
    heard();
    OPENED.with(|o| o.set(0));
    let text = create(MitsuamiKind::Text);
    set_text(text, MitsuamiTextProp::Text, "commit");
    assert!(mitsuami_set_context_menu_on_open(text, Some(build_counted_menu), std::ptr::null_mut()));
    assert!(mitsuami_on_event(text, record, std::ptr::null_mut()));
    app.mount(|| Adopt(text));
    app.settle().await;
    assert_eq!(OPENED.with(|o| o.get()), 0, "built before it opens");

    app.get_by_text("commit").choose_menu_item(&["Opened 1"]).await;
    app.get_by_text("commit").choose_menu_item(&["Opened 2"]).await;
    assert_eq!(heard(), ["menu 1", "menu 2"]);
}

extern "C" fn build_counted_button_menu(_user: *mut c_void, node: u32) {
    let times = OPENED.with(|o| {
        o.set(o.get() + 1);
        o.get()
    });
    let title = format!("Opened {times}");
    let entries = [menu_entry(0, times, &title, 0)];
    assert!(unsafe { mitsuami_set_menu(node, entries.as_ptr(), entries.len()) });
}

#[mitsuami_test::test]
async fn menu_buttons_menus_can_be_built_as_they_open(app: TestApp) {
    attach(app.ui());
    heard();
    OPENED.with(|o| o.set(0));
    let button = create(MitsuamiKind::MenuButton);
    set_text(button, MitsuamiTextProp::Label, "Git-Flow");
    assert!(mitsuami_set_menu_on_open(button, Some(build_counted_button_menu), std::ptr::null_mut()));
    assert!(mitsuami_on_event(button, record, std::ptr::null_mut()));
    app.mount(|| Adopt(button));
    app.settle().await;
    assert_eq!(OPENED.with(|o| o.get()), 0, "built before it opens");

    app.get_by_role(Role::MenuButton, "Git-Flow").choose_menu_item(&["Opened 1"]).await;
    app.get_by_role(Role::MenuButton, "Git-Flow").choose_menu_item(&["Opened 2"]).await;
    assert_eq!(heard(), ["item 1", "item 2"]);
}

/// A row that wraps puts what doesn't fit on another line.
#[mitsuami_test::test]
async fn rows_wrap_when_told(app: TestApp) {
    attach(app.ui());
    let row = create(MitsuamiKind::Container);
    assert!(mitsuami_set_direction(row, true));
    assert!(mitsuami_set_wrap(row, true));
    assert!(mitsuami_set_size(row, px(250.0), auto()));
    assert!(mitsuami_set_align_self(row, 1));
    let chips: Vec<u32> = (0..3).map(|_| create(MitsuamiKind::Container)).collect();
    for &chip in &chips {
        assert!(mitsuami_set_size(chip, px(100.0), px(20.0)));
        assert!(mitsuami_append_child(row, chip));
    }
    app.mount(|| Adopt(row));
    app.settle().await;

    let frame = |id| app.ui().frame(NodeId::from_raw(id)).unwrap();
    assert_eq!((frame(chips[0]).y(), frame(chips[1]).y()), (0.0, 0.0));
    assert_eq!((frame(chips[2]).x(), frame(chips[2]).y()), (0.0, 20.0));
    assert_eq!(frame(row).height(), 40.0);
}

#[mitsuami_test::test]
async fn popovers_show_and_report_their_dismissal(app: TestApp) {
    attach(app.ui());
    heard();
    let anchor = create(MitsuamiKind::Container);
    let button = create(MitsuamiKind::Button);
    set_text(button, MitsuamiTextProp::Label, "Contains in");
    let popover = create(MitsuamiKind::Popover);
    let branch = create(MitsuamiKind::Text);
    set_text(branch, MitsuamiTextProp::Text, "develop");
    assert!(mitsuami_append_child(popover, branch));
    assert!(mitsuami_append_child(anchor, button));
    assert!(mitsuami_append_child(anchor, popover));
    assert!(mitsuami_on_event(popover, record, std::ptr::null_mut()));
    app.mount(|| Adopt(anchor));
    app.settle().await;
    app.expect(by_text("develop")).to_be_hidden().await;

    assert!(mitsuami_set_bool(popover, MitsuamiBoolProp::PopoverShown as u32, true));
    app.expect(by_text("develop")).to_be_visible().await;
    app.get_by_text("develop").press(Key::Escape).await;
    app.expect(by_text("develop")).to_be_hidden().await;
    assert_eq!(heard(), ["popover shown false"]);
}

#[mitsuami_test::test]
async fn search_fields_suggest_and_report_the_pick(app: TestApp) {
    attach(app.ui());
    heard();
    let field = create(MitsuamiKind::SearchInput);
    set_text(field, MitsuamiTextProp::A11yLabel, "Path");
    let (titles, details) = (strs(&["App.cs", "Program.cs"]), strs(&["mitsuami", ""]));
    assert!(unsafe { mitsuami_set_suggestions(field, titles.as_ptr(), details.as_ptr(), 2) });
    assert!(mitsuami_on_event(field, record, std::ptr::null_mut()));
    app.mount(|| Adopt(field));

    app.get_by_label("Path").choose_suggestion("Program.cs").await;
    assert_eq!(heard(), ["suggestion 1"]);
}

#[mitsuami_test::test]
async fn text_areas_can_be_logs(app: TestApp) {
    attach(app.ui());
    let log = create(MitsuamiKind::TextArea);
    set_text(log, MitsuamiTextProp::A11yLabel, "Log");
    assert!(mitsuami_set_bool(log, MitsuamiBoolProp::ReadOnly as u32, true));
    assert!(mitsuami_set_bool(log, MitsuamiBoolProp::LineWrap as u32, false));
    assert!(mitsuami_set_bool(log, MitsuamiBoolProp::FollowsEnd as u32, true));
    assert!(mitsuami_set_enum(log, MitsuamiEnumProp::TextStyle as u32, 6));
    app.mount(|| Adopt(log));
    app.settle().await;
    let props = app.get_by_label("Log").native_state().props;
    for prop in
        [Prop::ReadOnly(true), Prop::LineWrap(false), Prop::FollowsEnd(true), Prop::TextStyle(TextStyle::Monospace)]
    {
        assert!(props.contains(&prop), "{prop:?}");
    }
}
