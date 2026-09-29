//! Every example in one window:
//! `cargo run --manifest-path examples/showcase/Cargo.toml`.
//!
//! The window's sidebar lists the examples, and the page chosen beside it
//! is that example's window, from the same code: the Button page is
//! `button.rs`'s `page()`, the GPU surface page `examples/gpu-surface`'s.
//! The `sidebar` example isn't one of them, as the showcase is one.
//!
//! - Pages that fit their window's height scroll here; the ones sized to
//!   fill theirs (File drop, GPU surface, Icon, List, Measurements, Tabs)
//!   fill the page.
//! - Leaving a page drops it and its state, as closing its window would;
//!   the Menus page takes the app's menus with it, the GPU surface page
//!   its render thread.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mitsuami::core::AnyView;
use mitsuami::prelude::*;

// Each example as it is: its `main` goes unused here, and its crate
// attributes are the showcase's own.
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/button.rs"]
mod button;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/checkbox.rs"]
mod checkbox;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/context_menu.rs"]
mod context_menu;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/file_drop.rs"]
mod file_drop;
#[allow(dead_code, unused_attributes)]
#[path = "../../gpu-surface/src/main.rs"]
mod gpu_surface;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/group.rs"]
mod group;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/icon.rs"]
mod icon;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/image.rs"]
mod image;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/list.rs"]
mod list;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/measurements.rs"]
mod measurements;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/menu_button.rs"]
mod menu_button;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/menus.rs"]
mod menus;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/number_input.rs"]
mod number_input;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/password_input.rs"]
mod password_input;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/progress.rs"]
mod progress;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/scroll_view.rs"]
mod scroll_view;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/select.rs"]
mod select;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/separator.rs"]
mod separator;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/slider.rs"]
mod slider;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/spinner.rs"]
mod spinner;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/switch.rs"]
mod switch;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/tabs.rs"]
mod tabs;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/text.rs"]
mod text;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/text_input.rs"]
mod text_input;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/toolbar.rs"]
mod toolbar;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/tooltip.rs"]
mod tooltip;
#[allow(dead_code, unused_attributes)]
#[path = "../../../crates/mitsuami/examples/windows.rs"]
mod windows;

/// An example's page: its title, which is also its value in the sidebar,
/// and whether it scrolls or fills the page.
struct Example {
    title: &'static str,
    page: fn() -> AnyView,
    scrolls: bool,
}

const fn scrolls(title: &'static str, page: fn() -> AnyView) -> Example {
    Example { title, page, scrolls: true }
}

const fn fills(title: &'static str, page: fn() -> AnyView) -> Example {
    Example { title, page, scrolls: false }
}

const SECTIONS: [(&str, &[Example]); 5] = [
    (
        "Controls",
        &[
            scrolls("Button", || AnyView::new(button::page())),
            scrolls("Checkbox", || AnyView::new(checkbox::page())),
            scrolls("Switch", || AnyView::new(switch::page())),
            scrolls("Slider", || AnyView::new(slider::page())),
            scrolls("Number input", || AnyView::new(number_input::page())),
            scrolls("Select", || AnyView::new(select::page())),
            scrolls("Menu button", || AnyView::new(menu_button::page())),
        ],
    ),
    (
        "Text",
        &[
            scrolls("Text", || AnyView::new(text::page())),
            scrolls("Text input", || AnyView::new(text_input::page())),
            scrolls("Password input", || AnyView::new(password_input::page())),
        ],
    ),
    (
        "Images and status",
        &[
            fills("Icon", || AnyView::new(icon::library())),
            scrolls("Image", || AnyView::new(image::page())),
            scrolls("Progress", || AnyView::new(progress::page())),
            scrolls("Spinner", || AnyView::new(spinner::page())),
            fills("GPU surface", || AnyView::new(gpu_surface::page(full_screen()))),
        ],
    ),
    (
        "Containers",
        &[
            scrolls("Group", || AnyView::new(group::settings())),
            scrolls("Separator", || AnyView::new(separator::page())),
            // Scrolls itself, around its lists.
            fills("List", || AnyView::new(list::page())),
            scrolls("Scroll view", || AnyView::new(scroll_view::page())),
            fills("Tabs", || AnyView::new(tabs::preferences())),
            fills("Measurements", || AnyView::new(measurements::content())),
        ],
    ),
    (
        "Windows and menus",
        &[
            scrolls("Toolbar", || AnyView::new(toolbar::launcher())),
            scrolls("Menus", || AnyView::new(menus::launcher())),
            scrolls("Context menu", || AnyView::new(context_menu::page())),
            scrolls("Tooltip", || AnyView::new(tooltip::page())),
            scrolls("Windows", || AnyView::new(windows::page())),
            fills("File drop", || AnyView::new(file_drop::window())),
        ],
    ),
];

fn showcase() -> impl View {
    let chosen = signal("Button");
    let sections = SECTIONS
        .iter()
        .map(|(title, examples)| {
            let items = examples.iter().map(|e| SidebarItem::new(e.title, e.title)).collect::<Vec<_>>();
            SidebarSection::new(*title).children(items)
        })
        .collect::<Vec<_>>();
    let pages = SECTIONS
        .iter()
        .flat_map(|(_, examples)| examples.iter())
        .map(|&Example { title, page, scrolls }| {
            Show::new(
                move || chosen.get() == title,
                move || {
                    if title == "Menus" {
                        // The page installs the app's menus; they go with it.
                        on_cleanup(|| set_menu(MenuBar::new()));
                    }
                    if scrolls { AnyView::new(view! { <ScrollView grow=1.0>{page()}</ScrollView> }) } else { page() }
                },
            )
        })
        .collect::<Vec<_>>();
    view! {
        <Column grow=1.0>
            <Sidebar selection=chosen>{sections}</Sidebar>
            {pages}
        </Column>
    }
}

/// The window's full screen, which the GPU surface page has a checkbox for.
#[derive(Clone, Copy)]
struct FullScreen(Signal<bool>);

fn full_screen() -> Signal<bool> {
    inject::<FullScreen>().expect("the showcase's window provides it").0
}

fn main() {
    let (open, full) = (signal(true), signal(false));
    App::new()
        .open(Window::new("mitsuami showcase").size(Size::new(760.0, 640.0)).full_screen(full).bind(open).content(
            move || {
                provide(FullScreen(full));
                showcase()
            },
        ))
        .run();
}
