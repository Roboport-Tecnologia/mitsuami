//! Files dropped on a host (`Prop::FileDrop`): XAML's drag and drop on
//! its `Canvas`, from Explorer or any app that drags storage items.
//!
//! XAML gives a drag's files only asynchronously (`GetStorageItemsAsync`).
//! Entering starts reading them; until they're read the host takes the
//! drag on its format alone, and from then on only if it takes one of
//! them, so the copy cursor goes away over files it doesn't take.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use mitsuami_core::{FileDrop, NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;
use crate::later;

type R<T> = windows_core::Result<T>;

/// A host's drop target: what it takes, and the drag over it.
pub(crate) struct DropTarget {
    state: Rc<DropState>,
    _revokers: Vec<EventRevoker>,
}

struct DropState {
    id: NodeId,
    events: Events,
    drop: RefCell<FileDrop>,
    /// Whether the host reported `DropHover(true)` for the drag over it.
    hover: Cell<bool>,
    /// The drag's files, once read; `None` while they're being read.
    paths: RefCell<Option<Vec<PathBuf>>>,
    /// Which drag a read belongs to, so a late one is dropped.
    drag: Cell<u64>,
}

impl DropState {
    /// Files entered or moved over the host: whether it takes them, and
    /// the hover it reports for that. Unknown files are taken for now.
    fn over(&self, paths: Option<&[PathBuf]>) -> bool {
        let takes = paths.is_none_or(|paths| !self.drop.borrow().accepted(paths).is_empty());
        self.hover(takes);
        takes
    }

    /// The drag left: a read of its files still under way is dropped.
    fn left(&self) {
        self.drag.set(self.drag.get() + 1);
        self.hover(false);
    }

    fn hover(&self, over: bool) {
        if self.hover.replace(over) != over {
            self.events.emit(self.id, UiEvent::DropHover(over));
        }
    }

    /// Files dropped: the ones it takes, and the drag is over.
    fn dropped(&self, paths: &[PathBuf]) {
        let accepted = self.drop.borrow().accepted(paths);
        if !accepted.is_empty() {
            self.events.emit(self.id, UiEvent::FilesDropped(accepted));
        }
        self.hover(false);
    }
}

impl DropTarget {
    pub(crate) fn new(id: NodeId, events: Events, drop: FileDrop, element: &w::UIElement) -> R<DropTarget> {
        let state = Rc::new(DropState {
            id,
            events,
            drop: RefCell::new(drop),
            hover: Cell::new(false),
            paths: RefCell::new(None),
            drag: Cell::new(0),
        });
        let ui: w::IUIElement = element.cast()?;
        ui.SetAllowDrop(true)?;
        let mut revokers = Vec::new();
        let weak = Rc::downgrade(&state);
        revokers.push(ui.DragEnter(move |_, args| {
            let (Some(state), Some(args)) = (weak.upgrade(), args.as_ref()) else { return };
            state.drag.set(state.drag.get() + 1);
            *state.paths.borrow_mut() = None;
            read_paths(&state, args, None);
            accept(&state, args);
        })?);
        let weak = Rc::downgrade(&state);
        revokers.push(ui.DragOver(move |_, args| {
            let (Some(state), Some(args)) = (weak.upgrade(), args.as_ref()) else { return };
            accept(&state, args);
        })?);
        let weak = Rc::downgrade(&state);
        revokers.push(ui.DragLeave(move |_, _| {
            if let Some(state) = weak.upgrade() {
                state.left();
            }
        })?);
        let weak = Rc::downgrade(&state);
        revokers.push(ui.Drop(move |_, args| {
            let (Some(state), Some(args)) = (weak.upgrade(), args.as_ref()) else { return };
            _ = args.SetHandled(true);
            let known = state.paths.borrow().clone();
            match known {
                Some(paths) => state.dropped(&paths),
                // Dropped before they were read: finish when they are.
                None => read_paths(&state, args, Some(())),
            }
        })?);
        Ok(DropTarget { state, _revokers: revokers })
    }

    pub(crate) fn set(&self, drop: FileDrop) {
        *self.state.drop.borrow_mut() = drop;
    }

    pub(crate) fn file_drop(&self) -> FileDrop {
        self.state.drop.borrow().clone()
    }

    /// `SyntheticInput::DragFiles`: what entering and moving do, with
    /// these files known.
    pub(crate) fn drag(&self, paths: &[PathBuf]) {
        self.state.drag.set(self.state.drag.get() + 1);
        *self.state.paths.borrow_mut() = Some(paths.to_vec());
        self.state.over(Some(paths));
    }

    pub(crate) fn leave(&self) {
        self.state.left();
    }

    /// `SyntheticInput::DropFiles`: entering, then dropping.
    pub(crate) fn drop_files(&self, paths: &[PathBuf]) {
        self.drag(paths);
        self.state.dropped(paths);
    }
}

/// Copies while the host takes the drag, as Explorer's own targets do.
fn accept(state: &DropState, args: &w::DragEventArgs) {
    let files = args.DataView().and_then(|v| v.Contains(&w::StandardDataFormats::StorageItems()?)).unwrap_or(false);
    let paths = state.paths.borrow().clone();
    let takes = files && state.over(paths.as_deref());
    _ = args.SetAcceptedOperation(if takes { w::DataPackageOperation::Copy } else { w::DataPackageOperation::None });
}

/// Reads the drag's files in the background, keeps them for the drag
/// they belong to, and drops them if `then_drop`.
fn read_paths(state: &Rc<DropState>, args: &w::DragEventArgs, then_drop: Option<()>) {
    let Ok(view) = args.DataView() else { return };
    if !w::StandardDataFormats::StorageItems().and_then(|f| view.Contains(&f)).unwrap_or(false) {
        return;
    }
    let Ok(operation) = view.GetStorageItemsAsync() else { return };
    let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() else { return };
    let ticket = later::park_until::<Weak<DropState>>(&queue, Rc::downgrade(state));
    let id = ticket.id();
    let drag = state.drag.get();
    let watched = operation.when(move |items| {
        let paths: Vec<PathBuf> = items
            .ok()
            .map(|items| (&items).into_iter().filter_map(|item| item.Path().ok()).map(PathBuf::from).collect())
            .unwrap_or_default();
        later::on_ui_take(&queue, ticket, move |state: Weak<DropState>| {
            let Some(state) = state.upgrade() else { return };
            if state.drag.get() != drag {
                return;
            }
            *state.paths.borrow_mut() = Some(paths.clone());
            match then_drop {
                Some(()) => state.dropped(&paths),
                None => {
                    state.over(Some(&paths));
                }
            }
        });
    });
    if watched.is_err() {
        later::take::<Weak<DropState>>(id);
    }
}
