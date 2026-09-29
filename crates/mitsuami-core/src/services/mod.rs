//! Platform services: clipboard, dialogs and the menu bar. Widgets'
//! context menus are made of the same menus.
//!
//! [`Services`] is the contract each platform implements, separately from
//! the widget [`Backend`](crate::Backend). Tests install a scripted fake in
//! its place, so they never open real dialogs or touch the real clipboard.
//!
//! App code uses the async functions here (`alert(...).await`, …) or the
//! same methods on [`Ui`](crate::Ui).

mod dialogs;
mod futures;
mod menu;
mod menu_collect;
mod menu_data;

pub use dialogs::{Alert, AlertStyle, FileFilter, OpenFile, SaveFile, existing_folder};
pub(crate) use futures::reply_future;
pub use futures::{alert, clipboard_text, open_file, save_file, set_clipboard_text, set_menu};
pub use menu::{Menu, MenuBar, MenuEntries, MenuItem, MenuSeparator, Menus};
#[doc(hidden)]
pub use menu_collect::install_button_menu;
pub(crate) use menu_collect::install_context_menu;
pub use menu_data::{
    MenuBarData, MenuCheck, MenuData, MenuEntry, MenuItemData, MenuRole, Shortcut, find_menu_item, menu_item_by_id,
};

use std::path::PathBuf;
use std::rc::Rc;

use crate::widget::NodeId;

/// Delivers the user's answer. Called once, possibly long after the request.
pub type Reply<T> = Box<dyn FnOnce(T)>;

/// Why a service request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The platform doesn't offer this service (or not right now).
    Unavailable,
    Failed(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Unavailable => f.write_str("the service is not available"),
            ServiceError::Failed(why) => write!(f, "the service failed: {why}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// What a platform provides beyond widgets.
pub trait Services {
    /// Reads the clipboard's text. Async because GTK and WinUI only read the
    /// clipboard asynchronously; platforms that can may reply right away.
    fn clipboard_text(&mut self, reply: Reply<Option<String>>);
    /// Writes the clipboard (it can fail: e.g. another process holds it).
    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>);

    /// Shows an alert, attached to `parent` if given (a sheet on macOS) or
    /// to the active window. Must not block: reply when the user answers.
    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>);
    /// Replies with the chosen paths, or `None` if cancelled.
    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>);
    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>);

    /// Installs a menu bar: the app's (`window` is `None`), or one
    /// window's own menus, which it shows with the app's
    /// ([`MenuBarData::merged`]). Called again whenever the bar changes;
    /// an empty bar removes a window's menus. A window's menus may arrive
    /// before the window is created, and after it's destroyed.
    ///
    /// Platforms keep their standard menus (e.g. the macOS app and Edit
    /// menus) and call `activate` with an item's id when it is chosen.
    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>);
}
