//! mitsuami: native widgets on macOS (AppKit), Windows (WinUI 3) and Linux
//! (GTK 4, or Qt Quick and Kirigami for KDE Plasma), driven by one
//! declarative, Vue-inspired layer.
//!
//! ```ignore
//! use mitsuami::prelude::*;
//!
//! #[component]
//! fn Counter(initial: i32) -> impl View {
//!     let count = signal(initial);
//!     view! {
//!         <Column gap=Spacing::Md padding=2.em()>
//!             <Text text_style=TextStyle::Title>{move || format!("Count: {}", count.get())}</Text>
//!             <Button @click=move || count.update(|c| *c += 1)>"Increment"</Button>
//!         </Column>
//!     }
//! }
//! ```
//!
//! [`view!`] and [`#[component]`](component) are sugar: they expand to the
//! builder API, which works on its own.
//!
//! ```ignore
//! Column::new().gap(Spacing::Md).padding(2.em()).children((
//!     Text::new(move || format!("Count: {}", count.get())).text_style(TextStyle::Title),
//!     Button::new("Increment").on_click(move || count.update(|c| *c += 1)),
//! ))
//! ```
//!
//! The platform backend is chosen by target OS: AppKit on macOS, WinUI 3 on
//! Windows, and on Linux GTK 4, or Qt Quick and Kirigami (KDE Plasma) with
//! the `kde` feature.
//!
//! Escape hatches, for when the shared widgets aren't enough:
//! - [`platform!`] picks per-platform code (a whole screen, a detail) at
//!   compile time, while stores and composables stay shared.
//! - `NativeView` embeds any native view in the shared tree
//!   (`appkit::NativeView` on macOS, `gtk::NativeView` or
//!   `kirigami::NativeView` on Linux, `winui::NativeView` on Windows).
//! - Custom widgets: one [`CustomWidget`](core::CustomWidget) definition,
//!   rendered natively per platform or drawn with the
//!   [`Canvas`](core::Canvas) API.

mod app;
mod platforms;

pub use app::App;
pub use mitsuami_core as core;
/// The `raw-window-handle` a [`SurfaceHandle`](prelude::SurfaceHandle) implements.
pub use mitsuami_core::raw_window_handle;
pub use mitsuami_core::unicode_segmentation;
pub use mitsuami_macros::{IntoValue, component, view};
pub use mitsuami_reactive as reactive;
pub use mitsuami_widgets as widgets;

/// The AppKit backend: native renders, native views, and the `objc2`
/// bindings to write them with.
#[cfg(target_os = "macos")]
pub use mitsuami_appkit as appkit;

/// The GTK 4 backend: native renders, native views, and the `gtk4`
/// bindings to write them with (`gtk::gtk`).
#[cfg(all(target_os = "linux", feature = "gtk", not(feature = "kde")))]
pub use mitsuami_gtk as gtk;

/// The Qt Quick and Kirigami backend (the `kde` feature): native renders,
/// native views, and `QmlObject`, the handle to write them with.
#[cfg(all(target_os = "linux", feature = "kde"))]
pub use mitsuami_kirigami as kirigami;

#[cfg(all(target_os = "linux", not(any(feature = "gtk", feature = "kde"))))]
compile_error!("mitsuami: pick a Linux toolkit with the `gtk` (default) or `kde` feature");

/// The WinUI 3 backend: native renders, native views, and the XAML
/// bindings to write them with (`winui::bindings`, `winui::windows_core`).
#[cfg(windows)]
pub use mitsuami_winui as winui;

pub mod prelude {
    pub use crate::{App, IntoValue, component, platform, view};
    pub use mitsuami_core::draw::DisplayList;
    pub use mitsuami_core::services::{
        Alert, AlertStyle, FileFilter, Launch, Menu, MenuBar, MenuItem, MenuRole, MenuSeparator, OpenFile, SaveFile,
        ServiceError, Shortcut, alert, clipboard_text, launch, launch_url, open_file, save_file, set_clipboard_text,
        set_menu, trash,
    };
    pub use mitsuami_core::task::{TaskHandle, sleep, spawn_blocking, spawn_local};
    pub use mitsuami_core::{
        A11yAction, A11yProps, AppIcon, AppInfo, Canvas, Color, Composed, Custom, CustomView, CustomWidget, Drawn,
        MeasureRequest, Path, PlatformMetrics, PointerEvent, PointerKind, Rect, Render, Renderer, Shape,
    };
    pub use mitsuami_core::{Action, Resource, Store, action, resource, resource_on, use_store};
    pub use mitsuami_core::{
        Align, ButtonRole, ButtonStyle, Callback, Children, Cursor, Element, ElementBuilder, FileDrop, FlexDirection,
        FontWeight, For, GridPlacement, ImageFit, ImageSource, InputPurpose, Justify, Key, KeyCode, Length, LengthExt,
        List, ListHandle, ListStyle, Modality, Modifiers, MouseButton, NodeId, NodeRef, Orientation, Pixels, Point,
        Role, ScrollAxes, ScrollDelta, SelectionMode, Show, Size, Slot, Sort, SortOrder, Spacing, SurfaceHandle,
        SurfaceInput, SurfaceSize, Table, TableColumn, TabsStyle, TextAlign, TextDirection, TextStyle, Track, Tweak,
        Ui, View, WindowSize, node_ref, repeat, use_size, use_viewport,
    };
    pub use mitsuami_reactive::{
        Computed, IntoValue, Owner, Signal, Value, batch, computed, effect, inject, on_cleanup, provide, signal,
        untrack, watch,
    };
    pub use mitsuami_widgets::{
        Button, Checkbox, Column, Container, FileIcon, GpuSurface, Grid, Group, Icon, Image, MenuButton, NumberInput,
        PasswordInput, Progress, RadioGroup, Row, ScrollView, SearchInput, Select, Separator, Sidebar, SidebarItem,
        SidebarSection, Slider, Spinner, Switch, Tab, Tabs, Text, TextArea, TextInput, ToggleButton, Toolbar, Window,
    };
}
