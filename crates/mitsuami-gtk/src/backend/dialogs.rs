//! Helpers for dialogs: their parent window and file filters.

use gtk::gio;
use gtk::prelude::*;
use mitsuami_core::NodeId;

use super::GtkHandle;

/// Opens `files` dialogs and alerts on this window, or the active one,
/// and says which window that is.
pub(crate) fn dialog_parent(handle: &GtkHandle, parent: Option<NodeId>) -> Option<(NodeId, gtk::Window)> {
    let windows = handle.windows();
    parent
        .and_then(|id| windows.iter().find(|(w, _)| *w == id))
        .or_else(|| windows.iter().find(|(_, w)| w.is_active()))
        .or_else(|| windows.first())
        .cloned()
}

pub(crate) fn file_filters(filters: &[mitsuami_core::services::FileFilter]) -> Option<gio::ListStore> {
    if filters.is_empty() {
        return None;
    }
    let store = gio::ListStore::new::<gtk::FileFilter>();
    for filter in filters {
        let gtk_filter = gtk::FileFilter::new();
        gtk_filter.set_name(Some(&filter.name));
        for extension in &filter.extensions {
            gtk_filter.add_suffix(extension.trim_start_matches('.'));
        }
        if filter.is_all() {
            gtk_filter.add_pattern("*");
        }
        store.append(&gtk_filter);
    }
    Some(store)
}
