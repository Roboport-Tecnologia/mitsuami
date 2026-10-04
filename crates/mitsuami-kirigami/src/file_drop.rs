//! Files dropped on a host (`Prop::FileDrop`): a Qt Quick `DropArea` over
//! it, which asks the backend whether it takes what's dragged in, as Qt
//! Quick apps take file drops.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::{FileDrop, NodeId, UiEvent};

use crate::events::Events;
use crate::ffi::QmlObject;

/// The drop area: it fills its host, and lets the Rust side decide on each
/// drag (`mitsuamiAccept`), so what it takes is `FileDrop::accepted`, as on
/// every platform. Items over it don't block drags: Qt Quick only hands
/// them to items that accept drops.
const QML: &str = r#"
DropArea {
    property list<url> mitsuamiUrls
    property bool mitsuamiAccept: false
    signal mitsuamiEntered()
    signal mitsuamiExited()
    signal mitsuamiDropped()
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0
    keys: ["text/uri-list"]
    onEntered: (drag) => {
        mitsuamiUrls = drag.urls
        mitsuamiEntered()
        if (mitsuamiAccept)
            drag.accept(Qt.CopyAction)
        else
            drag.accepted = false
    }
    onExited: mitsuamiExited()
    onDropped: (drop) => {
        mitsuamiUrls = drop.urls
        mitsuamiDropped()
        if (mitsuamiAccept)
            drop.accept(Qt.CopyAction)
    }
}
"#;

/// A host's drop area, and what it takes.
pub(crate) struct FileDropArea {
    area: QmlObject,
    shared: Rc<Shared>,
}

struct Shared {
    id: NodeId,
    events: Events,
    drop: RefCell<FileDrop>,
    /// Whether files it takes are over it, so `DropHover` comes in pairs.
    hover: Cell<bool>,
}

impl FileDropArea {
    /// A drop area over `host`, after its children (Qt Quick hands drags
    /// to the items that take them wherever they stack, and the backend's
    /// inserts count only the host's first children, which stay before it).
    pub(crate) fn new(host: QmlObject, id: NodeId, events: Events, drop: FileDrop) -> FileDropArea {
        let area = QmlObject::load_in(QML, host);
        area.set_parent_item(Some(host), host.child_items().len());
        let shared = Rc::new(Shared { id, events, drop: RefCell::new(drop), hover: Cell::new(false) });
        let on = shared.clone();
        area.connect("mitsuamiEntered()", move || {
            let accept = on.enter(&area.paths("mitsuamiUrls"));
            area.set_bool("mitsuamiAccept", accept);
        });
        let on = shared.clone();
        area.connect("mitsuamiExited()", move || on.leave());
        let on = shared.clone();
        area.connect("mitsuamiDropped()", move || {
            let accept = on.dropped(&area.paths("mitsuamiUrls"));
            area.set_bool("mitsuamiAccept", accept);
        });
        FileDropArea { area, shared }
    }

    pub(crate) fn set(&self, drop: FileDrop) {
        *self.shared.drop.borrow_mut() = drop;
    }

    pub(crate) fn drop_value(&self) -> FileDrop {
        self.shared.drop.borrow().clone()
    }

    /// Takes the area off its host, ending any hover.
    pub(crate) fn remove(self) {
        self.shared.leave();
        self.area.set_parent_item(None, 0);
        self.area.delete_later();
    }

    /// Deletes it now, without reporting: its host is going. Qt doesn't
    /// delete an item's child items with it (it only lets go of them), and
    /// the area isn't the host's QObject child.
    pub(crate) fn destroy(self) {
        self.area.destroy();
    }

    /// The drag handling the QML signals run, for `synthesize` to run
    /// with the backend's state let go (reports may wake the run loop).
    pub(crate) fn input(&self) -> DropInput {
        DropInput(self.shared.clone())
    }
}

pub(crate) struct DropInput(Rc<Shared>);

impl DropInput {
    /// Files entering it.
    pub(crate) fn enter(&self, paths: &[PathBuf]) {
        self.0.enter(paths);
    }

    pub(crate) fn leave(&self) {
        self.0.leave();
    }

    /// A drop: it enters first, as a real drop has.
    pub(crate) fn dropped(&self, paths: &[PathBuf]) {
        self.0.dropped(paths);
    }
}

impl Shared {
    /// Whether it takes any of these; if so, hovering starts.
    fn enter(&self, paths: &[PathBuf]) -> bool {
        let takes = !self.drop.borrow().accepted(paths).is_empty();
        if takes && !self.hover.replace(true) {
            self.events.emit(self.id, UiEvent::DropHover(true));
        }
        takes
    }

    fn leave(&self) {
        if self.hover.replace(false) {
            self.events.emit(self.id, UiEvent::DropHover(false));
        }
    }

    /// Reports the ones it takes, and ends the hover. Qt sends no
    /// `exited` after a drop.
    fn dropped(&self, paths: &[PathBuf]) -> bool {
        let accepted = self.drop.borrow().accepted(paths);
        let takes = !accepted.is_empty();
        if takes {
            self.enter(paths);
            self.events.emit(self.id, UiEvent::FilesDropped(accepted));
        }
        self.leave();
        takes
    }
}
