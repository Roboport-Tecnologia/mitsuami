//! Scripted platform services for tests: records every request and lets
//! the test answer it.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::NodeId;
use mitsuami_core::services::{
    Alert, MenuBarData, MenuItemData, OpenFile, Reply, SaveFile, ServiceError, Services, find_menu_item,
};

/// A request waiting for the test to answer it.
pub struct Pending<Request, Answer> {
    pub request: Request,
    pub parent: Option<NodeId>,
    reply: Reply<Answer>,
}

impl<Request, Answer> Pending<Request, Answer> {
    /// Answers as the user would. Call `settle` afterwards to let waiting
    /// tasks continue.
    pub fn respond(self, answer: Answer) {
        (self.reply)(answer);
    }
}

pub type PendingAlert = Pending<Alert, usize>;
pub type PendingOpen = Pending<OpenFile, Option<Vec<PathBuf>>>;
pub type PendingSave = Pending<SaveFile, Option<PathBuf>>;

#[derive(Default)]
struct State {
    clipboard: Option<String>,
    alerts: VecDeque<PendingAlert>,
    opens: VecDeque<PendingOpen>,
    saves: VecDeque<PendingSave>,
    menu: Option<MenuBarData>,
    window_menus: BTreeMap<NodeId, MenuBarData>,
    activate: Option<Rc<dyn Fn(u32)>>,
}

/// Install with [`Ui::set_services`](mitsuami_core::Ui::set_services).
#[derive(Default)]
pub struct FakeServices {
    state: Rc<RefCell<State>>,
}

/// The test's side of [`FakeServices`].
#[derive(Clone)]
pub struct FakeServicesHandle {
    state: Rc<RefCell<State>>,
}

impl FakeServices {
    pub fn new() -> (FakeServices, FakeServicesHandle) {
        let services = FakeServices::default();
        let handle = FakeServicesHandle { state: services.state.clone() };
        (services, handle)
    }
}

impl FakeServicesHandle {
    pub fn clipboard(&self) -> Option<String> {
        self.state.borrow().clipboard.clone()
    }

    /// Puts text on the fake clipboard, as if copied from another app.
    pub fn set_clipboard(&self, text: &str) {
        self.state.borrow_mut().clipboard = Some(text.to_owned());
    }

    /// The oldest alert still waiting for an answer.
    pub fn take_alert(&self) -> Option<PendingAlert> {
        self.state.borrow_mut().alerts.pop_front()
    }

    pub fn take_open_file(&self) -> Option<PendingOpen> {
        self.state.borrow_mut().opens.pop_front()
    }

    pub fn take_save_file(&self) -> Option<PendingSave> {
        self.state.borrow_mut().saves.pop_front()
    }

    /// Requests of any kind still waiting for an answer.
    pub fn pending_requests(&self) -> usize {
        let state = self.state.borrow();
        state.alerts.len() + state.opens.len() + state.saves.len()
    }

    /// The menus the app installed.
    pub fn menu(&self) -> Option<MenuBarData> {
        self.state.borrow().menu.clone()
    }

    /// A window's own menus (a `MenuBar` in its content), without the app's.
    pub fn window_menu(&self, window: NodeId) -> Option<MenuBarData> {
        self.state.borrow().window_menus.get(&window).cloned()
    }

    /// The item at `path`, e.g. `["File", "Open Recent", "notes.txt"]`:
    /// a menu, its submenus, then the item's title. Looks in the app's
    /// menus, then in each window's.
    pub fn menu_item(&self, path: &[&str]) -> Option<MenuItemData> {
        let state = self.state.borrow();
        let (menu, rest) = path.split_first()?;
        state.menu.iter().chain(state.window_menus.values()).flat_map(|bar| &bar.menus).find_map(|data| {
            if data.title != *menu {
                return None;
            }
            find_menu_item(&data.entries, rest).cloned()
        })
    }

    /// Chooses the item at `path` (see [`menu_item`](Self::menu_item)),
    /// like a click or its shortcut would. Returns `false` if there is no
    /// such item or it is disabled.
    pub fn choose_menu_item(&self, path: &[&str]) -> bool {
        let Some(item) = self.menu_item(path).filter(|item| item.enabled) else { return false };
        let Some(activate) = self.state.borrow().activate.clone() else { return false };
        activate(item.id);
        true
    }
}

impl Services for FakeServices {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>) {
        let text = self.state.borrow().clipboard.clone();
        reply(text);
    }

    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>) {
        self.state.borrow_mut().clipboard = Some(text.to_owned());
        reply(Ok(()));
    }

    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>) {
        self.state.borrow_mut().alerts.push_back(Pending { request: alert.clone(), parent, reply });
    }

    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>) {
        self.state.borrow_mut().opens.push_back(Pending { request: request.clone(), parent, reply });
    }

    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>) {
        self.state.borrow_mut().saves.push_back(Pending { request: request.clone(), parent, reply });
    }

    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let mut state = self.state.borrow_mut();
        match window {
            None => state.menu = Some(menu.clone()),
            Some(window) if menu.menus.is_empty() => _ = state.window_menus.remove(&window),
            Some(window) => _ = state.window_menus.insert(window, menu.clone()),
        }
        // Ids are unique across bars, and every bar reports to the same Ui.
        state.activate = Some(activate);
    }
}
