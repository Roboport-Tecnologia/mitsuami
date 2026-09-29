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

use std::ops::Range;

use crate::geometry::Size;
use crate::task::current_ui;
use crate::ui::Ui;
use crate::widget::{CurrentWindow, NodeId};

/// A handle to the node a view builds, set with `.node_ref(…)`. It follows
/// the node: a view built again (by `Show`, `For`) sets it again, and it's
/// `None` while nothing is built. It also focuses the node from code
/// ([`focus`](NodeRef::focus), [`select_text`](NodeRef::select_text)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRef {
    node: Signal<Option<NodeId>>,
    /// Asked for while nothing was built: done once the node is.
    waiting: Signal<Option<Request>>,
}

/// What code asked of the node.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Request {
    Focus,
    Select(Range<usize>),
}

/// Creates a [`NodeRef`] owned by the current scope.
pub fn node_ref() -> NodeRef {
    NodeRef { node: signal(None), waiting: signal(None) }
}

impl NodeRef {
    /// The node, and subscribes the running observer.
    pub fn get(&self) -> Option<NodeId> {
        self.node.get()
    }

    pub fn get_untracked(&self) -> Option<NodeId> {
        if self.node.is_alive() { self.node.get_untracked() } else { None }
    }

    /// Gives the control keyboard focus, as a click or Tab would: the
    /// first field of a dialog, say, where the platform wouldn't focus it
    /// itself. Asked before the control is built (in a window's
    /// `on_open`), it's focused once it is.
    pub fn focus(&self) {
        self.request(Request::Focus);
    }

    /// Focuses a text field or text area and selects these characters of
    /// its text, counted in Unicode scalar values (`chars`): a file's name
    /// without its extension, as file managers select one to rename. Cut
    /// to the text's end; other controls are only focused. Asked before
    /// the field is built, it's done once it is, after its first text.
    pub fn select_text(&self, range: Range<usize>) {
        self.request(Request::Select(range));
    }

    fn request(&self, request: Request) {
        match self.get_untracked() {
            Some(id) => request.send(&current_ui(), id),
            None if self.waiting.is_alive() => self.waiting.set(Some(request)),
            None => {}
        }
    }

    /// Points the ref at `id` until the current scope ends, and does what
    /// was asked while nothing was built.
    pub(crate) fn attach(self, ui: &Ui, id: NodeId) {
        self.node.set(Some(id));
        if let Some(request) = self.waiting.get_untracked() {
            self.waiting.set(None);
            request.send(ui, id);
        }
        on_cleanup(move || {
            if self.get_untracked() == Some(id) {
                self.node.set(None);
            }
        });
    }
}

impl Request {
    fn send(self, ui: &Ui, id: NodeId) {
        match self {
            Request::Focus => ui.focus(id),
            Request::Select(range) => ui.select_text(id, range),
        }
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
