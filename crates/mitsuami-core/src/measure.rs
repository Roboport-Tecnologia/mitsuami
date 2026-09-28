//! Measurements: the sizes layout gave, as signals, for views that adapt to
//! the room they have (media and container queries).
//!
//! ```ignore
//! let viewport = use_viewport();
//! let sidebar = node_ref();
//! let width = use_size(sidebar);
//! view! {
//!     <Row>
//!         <Column node_ref=sidebar grow=1.0>…</Column>
//!         <Show when=move || viewport.get().width >= 600.0>…</Show>
//!     </Row>
//! }
//! ```
//!
//! Sizes are reported after each layout, in the same run-loop turn, so a
//! view that changes with them does so before anything is shown.

use mitsuami_reactive::{Computed, Signal, computed, inject, on_cleanup, signal};

use crate::geometry::Size;
use crate::task::current_ui;
use crate::widget::{CurrentWindow, NodeId};

/// A handle to the node a view builds, set with `.node_ref(…)`. It follows
/// the node: a view built again (by `Show`, `For`) sets it again, and it's
/// `None` while nothing is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRef(Signal<Option<NodeId>>);

/// Creates a [`NodeRef`] owned by the current scope.
pub fn node_ref() -> NodeRef {
    NodeRef(signal(None))
}

impl NodeRef {
    /// The node, and subscribes the running observer.
    pub fn get(&self) -> Option<NodeId> {
        self.0.get()
    }

    pub fn get_untracked(&self) -> Option<NodeId> {
        if self.0.is_alive() { self.0.get_untracked() } else { None }
    }

    /// Points the ref at `id` until the current scope ends.
    pub(crate) fn attach(self, id: NodeId) {
        self.0.set(Some(id));
        on_cleanup(move || {
            if self.get_untracked() == Some(id) {
                self.0.set(None);
            }
        });
    }
}

/// The content size of the window the view is in, like a media query.
/// Zero until its first layout.
///
/// # Panics
/// Outside of a window's content.
pub fn use_viewport() -> Computed<Size> {
    let Some(CurrentWindow(window)) = inject::<CurrentWindow>() else {
        panic!("mitsuami: use_viewport() outside of a window's content");
    };
    observe(move || Some(window))
}

/// The size of the node `node` refers to, like a container query. Zero
/// while it refers to none, and until its first layout.
pub fn use_size(node: NodeRef) -> Computed<Size> {
    observe(move || node.get_untracked())
}

fn observe(target: impl Fn() -> Option<NodeId> + 'static) -> Computed<Size> {
    let ui = current_ui();
    let size = signal(ui.measured_size(target()));
    let id = ui.observe_size(Box::new(target), size);
    let weak = ui.downgrade();
    on_cleanup(move || {
        if let Some(ui) = weak.upgrade() {
            ui.unobserve_size(id);
        }
    });
    computed(move || size.get())
}
