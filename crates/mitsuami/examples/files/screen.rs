//! The window: places down the sidebar, the folder's items in the
//! platform's table, the selection's preview beside them, and the path to
//! the folder along the bottom.

use std::path::{Path, PathBuf};

use mitsuami::core::services::MenuEntries;
use mitsuami::prelude::*;
use mitsuami::unicode_segmentation::UnicodeSegmentation;

use crate::browser::{Browser, SortBy};
use crate::fs::{self, Entry};
use crate::path_bar::{PathBar, PathBarEvent, PathBarProps};

/// The columns' widths, where they start.
const MODIFIED: f32 = 136.0;
const SIZE: f32 = 76.0;
const KIND: f32 = 104.0;

#[component]
pub fn Finder() -> impl View {
    let (preview, going_to) = (signal(true), signal(false));
    view! {
        <Column grow=1.0>
            <Places/>
            <Tools preview=preview/>
            <Menus preview=preview going_to=going_to/>
            <Row grow=1.0 basis=0 min_height=0>
                <Files preview=preview/>
                <Show when=preview>
                    <Separator orientation=Orientation::Vertical/>
                    <Preview/>
                </Show>
            </Row>
            <Separator/>
            <PathBarRow/>
            <RenameDialog/>
            <GoToDialog open=going_to/>
        </Column>
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Favourite folders and the disk. The sidebar follows the folder shown:
/// a folder that isn't one of its places chooses none.
#[component]
fn Places() -> impl View {
    let browser = use_store::<Browser>();
    let place = signal(browser.folder.get_untracked());
    watch(move || browser.folder.get(), move |folder, _| place.set(folder.clone()));
    watch(move || place.get(), move |place, _| browser.go(place.clone()));
    let item = |(title, path, icon): (String, PathBuf, &'static str)| {
        let item = SidebarItem::new(title, path);
        if icon.is_empty() { item } else { item.icon(icon) }
    };
    Sidebar::new(place)
        .native(platform! {
            // Narrower than Kirigami's default column (20 grid units), as
            // Dolphin's places are. KDE's apps set their page row's; the
            // window gives the page its row once it's in it, and the tweak
            // runs again as the items come.
            kde => mitsuami::kirigami::tweak(|page: &mitsuami::kirigami::QmlObject| {
                if let Some(row) = page.object("mitsuamiStack") {
                    row.set_int("defaultColumnWidth", (mitsuami::kirigami::grid_unit() * 12.0).round() as i32);
                }
            }),
            // 70% of NavigationView's default pane (320), closer to File
            // Explorer's.
            windows => mitsuami::winui::tweak(|view: &mitsuami::winui::bindings::NavigationView| {
                view.SetOpenPaneLength(224.0)
            }),
            // Wider than AppKit's narrowest sidebar (140), as the
            // showcase's is. The split view is the window's, which the
            // table is in once the window takes it.
            macos => mitsuami::appkit::tweak(|table: &mitsuami::appkit::objc2_app_kit::NSTableView| {
                use mitsuami::appkit::objc2_app_kit::NSSplitViewController;
                let split = table.window().and_then(|w| w.contentViewController());
                let split = split.and_then(|c| c.downcast::<NSSplitViewController>().ok());
                if let Some(item) = split.and_then(|s| s.splitViewItems().firstObject()) {
                    item.setMinimumThickness(184.0);
                }
            }),
            _ => Tweak::none(),
        })
        .children((
            SidebarSection::new("Favorites").children(fs::favourites().into_iter().map(item).collect::<Vec<_>>()),
            SidebarSection::new("Locations").children(item(fs::computer())),
        ))
}

#[component]
fn Tools(preview: Signal<bool>) -> impl View {
    let browser = use_store::<Browser>();
    let back = platform! { macos => "chevron.left", gtk => "go-previous-symbolic", kde => "go-previous", windows => "\u{E72B}" };
    let forward =
        platform! { macos => "chevron.right", gtk => "go-next-symbolic", kde => "go-next", windows => "\u{E72A}" };
    let new_folder = platform! {
        macos => "folder.badge.plus", gtk => "folder-new-symbolic", kde => "folder-new", windows => "\u{E8F4}",
    };
    let view_options = platform! {
        macos => "list.bullet", gtk => "view-list-symbolic", kde => "view-list-details", windows => "\u{E8FD}",
    };
    let show_preview = platform! {
        macos => "sidebar.right", gtk => "sidebar-show-right-symbolic", kde => "dialog-information", windows => "\u{E8A0}",
    };
    view! {
        <Toolbar>
            <Show when=move || browser.listing.loading()>
                <Spinner label="Reading the folder"/>
            </Show>
            <Button icon=back icon_only=true button_style=ButtonStyle::Borderless
                enabled=move || browser.can_go_back() @click=move || browser.go_back()>"Back"</Button>
            <Button icon=forward icon_only=true button_style=ButtonStyle::Borderless
                enabled=move || browser.can_go_forward() @click=move || browser.go_forward()>"Forward"</Button>
            <Button icon=new_folder icon_only=true button_style=ButtonStyle::Borderless
                @click=move || browser.new_folder()>"New Folder"</Button>
            <MenuButton icon=view_options icon_only=true button_style=ButtonStyle::Borderless menu=(
                sort_items(browser),
                MenuSeparator,
                MenuItem::new("Show Hidden Files").bind(browser.show_hidden),
            )>"View Options"</MenuButton>
            <ToggleButton icon=show_preview icon_only=true button_style=ButtonStyle::Borderless bind=preview>
                "Show Preview"
            </ToggleButton>
            <SearchInput placeholder="Search" a11y_label="Search" width=180
                value=browser.query @search=move |q| browser.query.set(q)/>
        </Toolbar>
    }
}

fn sort_items(browser: Browser) -> Menu {
    Menu::new("Sort By")
        .item(MenuItem::new("Name").radio((browser.sort_by, SortBy::Name)))
        .item(MenuItem::new("Kind").radio((browser.sort_by, SortBy::Kind)))
        .item(MenuItem::new("Size").radio((browser.sort_by, SortBy::Size)))
        .item(MenuItem::new("Date Modified").radio((browser.sort_by, SortBy::Modified)))
        .separator()
        .item(
            MenuItem::new("Reversed")
                .checked(move || browser.sort.get().order == SortOrder::Descending)
                .on_select(move || browser.reverse()),
        )
}

/// The shortcuts the platform's file manager has for these items: Finder,
/// Nautilus, Dolphin and File Explorer agree off macOS.
struct Keys {
    rename: Option<Shortcut>,
    trash: Shortcut,
    back: Shortcut,
    forward: Shortcut,
    up: Shortcut,
}

fn keys() -> Keys {
    platform! {
        macos => Keys {
            // Finder renames with Return, which opens here.
            rename: None,
            trash: Shortcut::primary(Key::Backspace),
            back: Shortcut::primary('['),
            forward: Shortcut::primary(']'),
            up: Shortcut::primary(Key::Up),
        },
        _ => Keys {
            rename: Some(Shortcut::new(Key::F(2))),
            trash: Shortcut::new(Key::Delete),
            back: Shortcut::new(Key::Left).alt(),
            forward: Shortcut::new(Key::Right).alt(),
            up: Shortcut::new(Key::Up).alt(),
        },
    }
}

/// The window's own menus.
#[component]
fn Menus(preview: Signal<bool>, going_to: Signal<bool>) -> impl View {
    let browser = use_store::<Browser>();
    let one = move || browser.selection().len() == 1;
    let some = move || !browser.selection().is_empty();
    let paths = move || browser.selection().into_iter().map(|e| e.path).collect::<Vec<_>>();
    let place = move |path: PathBuf| move || browser.go(path.clone());
    let (home, computer) = (fs::home(), fs::computer().1);
    let keys = keys();
    view! {
        <MenuBar>
            <Menu title="File">
                <MenuItem shortcut=Shortcut::primary('n').shift() @select=move || browser.new_folder()>"New Folder"</MenuItem>
                <MenuItem shortcut=Shortcut::primary('o') enabled=some @select=move || browser.open(browser.selection())>"Open"</MenuItem>
                <MenuSeparator/>
                <MenuItem shortcut=keys.rename enabled=one @select=move || browser.renaming.set(paths().pop())>"Rename…"</MenuItem>
                <MenuItem shortcut=Shortcut::primary('d') enabled=some @select=move || browser.duplicate(paths())>"Duplicate"</MenuItem>
                <MenuItem shortcut=keys.trash enabled=some @select=move || browser.trash(paths())>"Move to Trash"</MenuItem>
                <MenuSeparator/>
                <MenuItem shortcut=Shortcut::primary('c').alt() enabled=some @select=move || copy_paths(paths())>"Copy as Pathname"</MenuItem>
            </Menu>
            <Menu title="View">
                <MenuItem shortcut=Shortcut::primary('.').shift() bind=browser.show_hidden>"Show Hidden Files"</MenuItem>
                <MenuItem shortcut=Shortcut::primary('p').shift() bind=preview>"Show Preview"</MenuItem>
                <MenuSeparator/>
                {sort_items(browser)}
            </Menu>
            <Menu title="Go">
                <MenuItem shortcut=keys.back enabled=move || browser.can_go_back() @select=move || browser.go_back()>"Back"</MenuItem>
                <MenuItem shortcut=keys.forward enabled=move || browser.can_go_forward() @select=move || browser.go_forward()>"Forward"</MenuItem>
                <MenuItem shortcut=keys.up enabled=move || browser.can_go_up() @select=move || browser.go_up()>"Enclosing Folder"</MenuItem>
                <MenuSeparator/>
                <MenuItem shortcut=Shortcut::primary('h').shift() @select=place(home)>"Home"</MenuItem>
                <MenuItem shortcut=Shortcut::primary('c').shift() @select=place(computer)>"Computer"</MenuItem>
                <MenuSeparator/>
                <MenuItem shortcut=Shortcut::primary('g').shift() @select=move || going_to.set(true)>"Go to Folder…"</MenuItem>
            </Menu>
        </MenuBar>
    }
}

fn copy_paths(paths: Vec<PathBuf>) {
    let text = paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
    spawn_local(async move {
        let _ = set_clipboard_text(&text).await;
    });
}

/// The key that shows the selection larger: Finder's Quick Look is on
/// Space. Nautilus's previewer is too, but GTK's table keeps Space (it
/// selects the focused row), as XAML's does; Dolphin and File Explorer
/// have no such key.
pub fn preview_key() -> Option<Shortcut> {
    platform! { macos => Some(Shortcut::new(' ')), _ => None }
}

/// The folder's items, in the platform's table: its header sorts them.
/// Files dropped on it are copied in. The preview key shows or hides the
/// preview, where the table doesn't use it itself.
#[component]
fn Files(preview: Signal<bool>) -> impl View {
    let browser = use_store::<Browser>();
    let handle = ListHandle::new();
    // Focused as the window opens, as Finder's, Nautilus's and Dolphin's
    // views are. GTK would focus the sidebar, and its list selects the row
    // it focuses: the first place, which would go there.
    let items = node_ref();
    items.focus();
    let entries = computed(move || browser.entries());
    // Keep the selection in view as it changes from the app's side.
    let follow = handle.clone();
    watch(
        move || browser.selected.get(),
        move |selected, _| {
            if let Some(first) = selected.first() {
                follow.scroll_to(first);
            }
        },
    );
    // A folder opened shows its top once its items arrive, unless the
    // folder come back up from is to be selected, which is scrolled to.
    let to_top = signal(false);
    watch(move || browser.folder.get(), move |_, _| to_top.set(browser.fresh.get_untracked().is_empty()));
    let top = handle.clone();
    watch(
        move || entries.with(|e| e.first().map(|e| e.path.clone())),
        move |first, _| {
            if let Some(first) = first
                && to_top.get_untracked()
            {
                to_top.set(false);
                top.scroll_to(first);
            }
        },
    );
    let empty = move || {
        let loaded = browser.listing.data().is_some();
        loaded && entries.with(Vec::is_empty)
    };
    let mut table = Table::new(entries, |e: &Entry| e.path.clone())
        .columns(columns(browser))
        .sort(browser.sort)
        .selected(browser.selected)
        .selection_mode(SelectionMode::Multiple)
        .handle(handle)
        .node_ref(items)
        .estimated_row_height(28)
        .on_activate(move |path: PathBuf| browser.open_path(&path))
        // To the file manager, Mail, another folder: a copy.
        .drag_files(|e: &Entry| Some(e.path.clone()))
        .a11y_label("Items")
        .hidden(move || browser.listing.error().is_some() || empty())
        .grow(1.0)
        .basis(0);
    if let Some(key) = preview_key() {
        table = table.on_key(key, move || preview.update(|p| *p = !*p));
    }
    Column::new()
        .grow(1.0)
        .min_width(320)
        .a11y_label("Contents")
        .file_drop(FileDrop::files().and_folders())
        .on_drop(move |paths| browser.copy_in(paths))
        .context_menu((
            MenuItem::new("New Folder").on_select(move || browser.new_folder()),
            MenuSeparator,
            sort_items(browser),
            MenuItem::new("Show Hidden Files").bind(browser.show_hidden),
        ))
        .children((
            Show::new(
                move || browser.listing.error().is_some(),
                move || {
                    Text::new(move || browser.listing.error().unwrap_or_default())
                        .text_style(TextStyle::Caption)
                        .padding(Spacing::Lg)
                },
            ),
            Show::new(empty, move || {
                hint(move || if browser.query.get().is_empty() { "No items" } else { "No matches" }.to_owned())
            }),
            table,
        ))
}

/// A message in place of content, larger and dimmer than body text and
/// in the middle of the room, as file managers show an empty folder.
fn hint(text: impl IntoValue<String>) -> impl View {
    Column::new().grow(1.0).justify(Justify::Center).align(Align::Center).padding(Spacing::Lg).children(
        Text::new(text).text_style(TextStyle::Title).color(Color::SecondaryLabel).text_align(TextAlign::Center),
    )
}

/// Name, taking the room left, then Finder's columns.
fn columns(browser: Browser) -> Vec<TableColumn<Entry>> {
    let caption = |text: String| Text::new(text).text_style(TextStyle::Caption).max_lines(1);
    vec![
        TableColumn::new("Name", move |e: Entry| {
            let name = Row::new().gap(Spacing::Sm).align(Align::Center).children((
                FileIcon::new(&e.path),
                // The whole path, which the cell cuts off and the path bar
                // shows only for the folder.
                Text::new(e.name.clone())
                    .max_lines(1)
                    .tooltip(e.path.display().to_string())
                    .grow(1.0)
                    .shrink(1.0)
                    .basis(0),
            ));
            cell(browser, e.path.clone(), name)
        })
        .expand()
        .sort_key(SortBy::Name),
        TableColumn::new("Date Modified", move |e: Entry| {
            cell(browser, e.path.clone(), caption(e.modified.map(fs::civil).unwrap_or_default()))
        })
        .width(MODIFIED)
        .sort_key(SortBy::Modified),
        TableColumn::new("Size", move |e: Entry| {
            cell(browser, e.path.clone(), caption(e.size.map(fs::human_size).unwrap_or_else(|| "--".into())))
        })
        .width(SIZE)
        .sort_key(SortBy::Size),
        TableColumn::new("Kind", move |e: Entry| cell(browser, e.path.clone(), caption(e.kind())))
            .width(KIND)
            .sort_key(SortBy::Kind),
    ]
}

/// A cell, with its row's menu: a right-click lands on a cell.
fn cell(browser: Browser, path: PathBuf, content: impl View) -> impl View {
    Row::new().align(Align::Center).context_menu(row_menu(browser, path)).children(content)
}

/// A row's menu acts on the selection when the row is in it, on the row
/// alone when it isn't.
fn row_menu(browser: Browser, path: PathBuf) -> impl MenuEntries {
    let rename = path.clone();
    let targets = std::rc::Rc::new(move || {
        let selected = browser.selected.get_untracked();
        if selected.contains(&path) { selected } else { vec![path.clone()] }
    });
    let (open, duplicate, trash, copy) = (targets.clone(), targets.clone(), targets.clone(), targets);
    (
        MenuItem::new("Open").on_select(move || {
            let paths = open();
            let entries = browser.listing.data().unwrap_or_default();
            browser.open(entries.into_iter().filter(|e| paths.contains(&e.path)).collect());
        }),
        MenuSeparator,
        MenuItem::new("Rename…").on_select(move || browser.renaming.set(Some(rename.clone()))),
        MenuItem::new("Duplicate").on_select(move || browser.duplicate(duplicate())),
        MenuItem::new("Move to Trash").on_select(move || browser.trash(trash())),
        MenuSeparator,
        MenuItem::new("Copy as Pathname").on_select(move || copy_paths(copy())),
    )
}

/// The selection, larger: a picture's pixels, a text file's first lines,
/// or the item's icon, with what the list's columns say and more.
#[component]
fn Preview() -> impl View {
    let browser = use_store::<Browser>();
    let chosen = computed(move || browser.selection());
    let one = computed(move || match chosen.get().as_slice() {
        [entry] => Some(entry.clone()),
        _ => None,
    });
    let text = resource_on(
        move || one.get().filter(Entry::is_text).map(|e| e.path),
        |path| async move {
            match path {
                Some(path) => spawn_blocking(move || fs::head(&path, 16)).await.map(Some),
                None => Ok(None),
            }
        },
    );
    let items = resource_on(
        move || one.get().filter(|e| e.is_dir).map(|e| e.path),
        |path| async move { Ok::<_, ()>(spawn_blocking(move || path.and_then(|p| fs::count(&p))).await) },
    );
    let field = move |f: fn(&Entry) -> String| move || one.get().as_ref().map(f).unwrap_or_default();
    let size = move || match (one.get(), items.data().flatten()) {
        (Some(Entry { size: Some(size), .. }), _) => fs::human_size(size),
        (_, Some(1)) => "1 item".into(),
        (_, Some(n)) => format!("{n} items"),
        _ => "--".into(),
    };
    let summary = move || match chosen.get().len() {
        0 => "Select an item".into(),
        n => format!("{n} items selected"),
    };
    let info = |label: &'static str, value: Box<dyn Fn() -> String>| {
        Row::new().gap(Spacing::Sm).children((
            Text::new(label).text_style(TextStyle::Caption).width(64).text_align(TextAlign::End),
            Text::new(value).text_style(TextStyle::Caption).max_lines(3).grow(1.0).shrink(1.0).basis(0),
        ))
    };
    let picture = move || {
        Image::new(move || ImageSource::File(one.get().map(|e| e.path).unwrap_or_default()))
            .fit(ImageFit::Contain)
            .label(field(|e| e.name.clone()))
            .height(160)
    };
    // A document's thumbnail where the platform makes one, as Finder's
    // preview shows it.
    let icon = move || {
        let path = move || one.get().map(|e| e.path).unwrap_or_default();
        Row::new().justify(Justify::Center).children(FileIcon::new(path).icon_size(64.0).thumbnail(true))
    };
    view! {
        // The message outside the scroll view, to be in the pane's middle.
        <Column width=240 a11y_label="Preview">
            <Show when=move || one.get().is_some() fallback=move || hint(summary)>
                <ScrollView grow=1.0 basis=0 min_height=0>
                    <Column padding=Spacing::Lg gap=Spacing::Md>
                        <Show when=move || one.get().is_some_and(|e| e.is_image()) fallback=icon>
                            {picture()}
                        </Show>
                        <Text text_style=TextStyle::Headline max_lines=3 selectable=true>{field(|e| e.name.clone())}</Text>
                        <Column gap=Spacing::Xs>
                            {info("Kind", Box::new(field(Entry::kind)))}
                            {info("Size", Box::new(size))}
                            {info("Modified", Box::new(field(|e| e.modified.map(fs::civil).unwrap_or_default())))}
                            {info("Where", Box::new(field(|e| e.path.parent().map(|p| p.display().to_string()).unwrap_or_default())))}
                        </Column>
                        <Show when=move || text.data().flatten().is_some()>
                            <Separator/>
                            <Text text_style=TextStyle::Monospace max_lines=16>{move || text.data().flatten().unwrap_or_default()}</Text>
                        </Show>
                    </Column>
                </ScrollView>
            </Show>
        </Column>
    }
}

/// The path to the folder, each folder on it a way back up, and how many
/// items there are.
#[component]
fn PathBarRow() -> impl View {
    let browser = use_store::<Browser>();
    let status = move || {
        let (shown, selected) = (browser.entries().len(), browser.selection().len());
        let items = if shown == 1 { "1 item".to_owned() } else { format!("{shown} items") };
        if selected == 0 { items } else { format!("{selected} of {items} selected") }
    };
    view! {
        <Row padding_x=Spacing::Sm padding_y=Spacing::Xs gap=Spacing::Md align=Align::Center>
            <PathBar props=move || PathBarProps { path: browser.folder.get() } a11y_label="Path"
                grow=1.0 shrink=1.0 basis=0 min_width=0
                @event=move |PathBarEvent::Chosen(folder)| browser.go(folder.clone())/>
            <Text text_style=TextStyle::Caption max_lines=1>{status}</Text>
        </Row>
    }
}

/// The part of a name file managers select to rename it, in graphemes: a
/// file's name without its extension, all of a folder's or a dot file's.
pub fn stem_len(name: &str, folder: bool) -> usize {
    match name.rfind('.') {
        Some(dot) if dot > 0 && !folder => name[..dot].graphemes(true).count(),
        _ => name.graphemes(true).count(),
    }
}

/// Asks for an item's new name, while the browser is renaming one. It
/// opens with the name in its field, the part to rename selected.
#[component]
fn RenameDialog() -> impl View {
    let browser = use_store::<Browser>();
    let (draft, field) = (signal(String::new()), node_ref());
    let start = move || {
        let Some(path) = browser.renaming.get_untracked() else { return };
        let folder = browser.listing.data().unwrap_or_default().iter().any(|e| e.path == path && e.is_dir);
        let name = name_of(&path);
        field.select_text(0..stem_len(&name, folder));
        draft.set(name);
    };
    let cancel = move || browser.renaming.set(None);
    let commit = move || {
        if let Some(path) = browser.renaming.get_untracked() {
            browser.renaming.set(None);
            browser.rename(path, draft.get_untracked());
        }
    };
    let prompt = move || {
        format!("Rename \u{201C}{}\u{201D} to:", browser.renaming.get().as_deref().map(name_of).unwrap_or_default())
    };
    view! {
        <Window title="Rename" open=move || browser.renaming.get().is_some() modal=Modality::Window
            size=WindowSize::FitHeight(360.0) @open=start @close_request=cancel>
            <Column padding=Spacing::Xl gap=Spacing::Md>
                <Text>{prompt}</Text>
                <TextInput bind=draft a11y_label="New name" node_ref=field @submit=commit/>
                <Row gap=Spacing::Sm justify=Justify::End>
                    <Button role=ButtonRole::Cancel @click=cancel>"Cancel"</Button>
                    <Button role=ButtonRole::Default enabled=move || !draft.get().trim().is_empty() @click=commit>
                        "Rename"
                    </Button>
                </Row>
            </Column>
        </Window>
    }
}

/// Goes to a folder typed in: `~` is the home folder. It opens with the
/// path typed last selected, as Finder's does.
#[component]
fn GoToDialog(open: Signal<bool>) -> impl View {
    let browser = use_store::<Browser>();
    let (draft, field) = (signal(String::new()), node_ref());
    let cancel = move || open.set(false);
    let commit = move || {
        let typed = draft.get_untracked();
        let path = match typed.trim().strip_prefix('~') {
            Some(rest) => fs::home().join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(typed.trim()),
        };
        if path.is_dir() {
            open.set(false);
            browser.go(path);
        } else {
            let warning = Alert::new("The folder can't be found.").message(typed).style(AlertStyle::Warning);
            spawn_local(async move {
                alert(warning).await;
            });
        }
    };
    view! {
        <Window title="Go to Folder" bind=open modal=Modality::Window size=WindowSize::FitHeight(420.0)
            @open=move || field.select_text(0..usize::MAX)>
            <Column padding=Spacing::Xl gap=Spacing::Md>
                <TextInput bind=draft a11y_label="Folder" placeholder="~/Documents" node_ref=field @submit=commit/>
                <Row gap=Spacing::Sm justify=Justify::End>
                    <Button role=ButtonRole::Cancel @click=cancel>"Cancel"</Button>
                    <Button role=ButtonRole::Default enabled=move || !draft.get().trim().is_empty() @click=commit>"Go"</Button>
                </Row>
            </Column>
        </Window>
    }
}
