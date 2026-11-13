//! Platform services on a `Ui`: the clipboard, dialogs, the trash,
//! launching, notifications, menus and quitting.

use std::rc::Rc;

use crate::command::UiEvent;
use crate::services::{
    Alert, Launch, MenuBar, MenuBarData, MenuRole, Notification, OpenFile, SaveFile, ServiceError, Services,
    reply_future,
};
use crate::widget::NodeId;

use super::{Handler0, NotificationHandler, Ui};

impl Ui {
    /// Replaces the platform services (tests install a scripted fake).
    pub fn set_services(&self, services: Box<dyn Services>) {
        *self.services.borrow_mut() = services;
    }

    pub fn clipboard_text(&self) -> impl std::future::Future<Output = Option<String>> + use<> {
        let (reply, text) = reply_future();
        self.services.borrow_mut().clipboard_text(reply);
        text
    }

    /// Puts text on the clipboard. The write is issued right away; await
    /// the result to learn whether it worked.
    pub fn set_clipboard_text(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + use<> {
        let (reply, done) = reply_future();
        self.services.borrow_mut().set_clipboard_text(text, reply);
        done
    }

    /// Shows an alert; resolves to the index of the chosen button.
    pub fn alert(&self, parent: Option<NodeId>, alert: Alert) -> impl std::future::Future<Output = usize> + use<> {
        let (reply, answer) = reply_future();
        self.services.borrow_mut().alert(parent, &alert, reply);
        answer
    }

    /// Resolves to the chosen paths, or `None` if cancelled.
    pub fn open_file(
        &self,
        parent: Option<NodeId>,
        request: OpenFile,
    ) -> impl std::future::Future<Output = Option<Vec<std::path::PathBuf>>> + use<> {
        let (reply, answer) = reply_future();
        self.services.borrow_mut().open_file(parent, &request, reply);
        answer
    }

    pub fn save_file(
        &self,
        parent: Option<NodeId>,
        request: SaveFile,
    ) -> impl std::future::Future<Output = Option<std::path::PathBuf>> + use<> {
        let (reply, answer) = reply_future();
        self.services.borrow_mut().save_file(parent, &request, reply);
        answer
    }

    /// Moves files and folders to the trash; see [`Services::trash`].
    pub fn trash(
        &self,
        parent: Option<NodeId>,
        paths: Vec<std::path::PathBuf>,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + use<> {
        let (reply, done) = reply_future();
        self.services.borrow_mut().trash(parent, &paths, reply);
        done
    }

    /// Opens a file, folder or URL in another app; see [`Services::launch`].
    pub fn launch(
        &self,
        parent: Option<NodeId>,
        target: Launch,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + use<> {
        let (reply, done) = reply_future();
        self.services.borrow_mut().launch(parent, &target, reply);
        done
    }

    /// Shows a notification outside the app's windows; see
    /// [`Services::notify`]. Resolves once the platform has it.
    pub fn notify(
        &self,
        mut notification: Notification,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + use<> {
        if notification.id.is_none() {
            let mut inner = self.inner.borrow_mut();
            notification.id = Some(format!("mitsuami-{}", inner.next_notification));
            inner.next_notification += 1;
        }
        let (reply, done) = reply_future();
        self.services.borrow_mut().notify(&notification, self.notification_activated(), reply);
        done
    }

    /// Takes away the notification with this id, if it's still there.
    pub fn withdraw_notification(&self, id: &str) {
        self.services.borrow_mut().withdraw_notification(id);
    }

    /// Runs `handler` with a notification's id when the user clicks it,
    /// until the current scope ends. The platform has brought the app
    /// forward by then; the app shows what the notification was about.
    pub fn on_notification_activated(&self, handler: impl Fn(&str) + 'static) {
        // In the scope it was installed from, so it can spawn tasks that
        // live as long as it, as menu items do.
        let scope = mitsuami_reactive::Owner::current();
        let handler: NotificationHandler = Rc::new(move |id| match scope {
            Some(scope) if scope.is_alive() => scope.with(|| handler(id)),
            _ => handler(id),
        });
        let token = {
            let mut inner = self.inner.borrow_mut();
            let token = inner.next_notification_handler;
            inner.next_notification_handler += 1;
            inner.notification_handlers.insert(token, handler);
            token
        };
        let ui = self.downgrade();
        mitsuami_reactive::on_cleanup(move || {
            if let Some(ui) = ui.upgrade() {
                ui.inner.borrow_mut().notification_handlers.remove(&token);
            }
        });
    }

    /// What platforms call with the id of the notification clicked.
    fn notification_activated(&self) -> Rc<dyn Fn(String)> {
        let weak = self.downgrade();
        Rc::new(move |id| {
            if let Some(ui) = weak.upgrade() {
                ui.inner.borrow_mut().notification_queue.push_back(id);
                ui.changed();
            }
        })
    }

    /// Installs the app's menus, replacing the ones installed before.
    /// Their reactive parts (titles, enabled and checked states, which
    /// items there are) are sent to the platform again when they change,
    /// for as long as the current scope lives.
    pub fn set_menu(&self, menu: MenuBar) {
        self.install_menu(None, menu);
    }

    /// Installs a window's own menus, shown with the app's, until the
    /// current scope ends. A [`MenuBar`] in the window's content does this.
    pub fn set_window_menu(&self, window: NodeId, menu: MenuBar) {
        self.install_menu(Some(window), menu);
        let ui = self.downgrade();
        mitsuami_reactive::on_cleanup(move || {
            if let Some(ui) = ui.upgrade() {
                ui.remove_menu(Some(window));
            }
        });
    }

    fn install_menu(&self, target: Option<NodeId>, menu: MenuBar) {
        // Handlers run in the scope the menu was installed from, so they
        // can spawn tasks that live as long as it.
        let scope = mitsuami_reactive::Owner::current();
        let activate = self.menu_activate();
        // Weak: menus installed outside any scope (a C caller's) keep their
        // effect, which mustn't keep the `Ui` and the platform alive.
        let weak = self.downgrade();
        let mut ids = Vec::new();
        let effect = mitsuami_reactive::effect(move || {
            // Backends title some items themselves (AppKit's Quit, GNOME's
            // and KDE's), in the app's language.
            crate::l10n::track();
            let Some(ui) = weak.upgrade() else { return };
            let mut new_id = || {
                let mut inner = ui.inner.borrow_mut();
                inner.next_menu_id += 1;
                inner.next_menu_id - 1
            };
            let (data, handlers) = menu.collect(&mut ids, &mut new_id);
            let handlers = handlers
                .into_iter()
                .map(|(id, handler)| {
                    let handler: Handler0 = Rc::new(move || match scope {
                        Some(scope) if scope.is_alive() => scope.with(|| handler()),
                        _ => handler(),
                    });
                    (id, handler)
                })
                .collect();
            let quit = data.clone().take_role(MenuRole::Quit).filter(|item| item.enabled).map(|item| item.id);
            let mut inner = ui.inner.borrow_mut();
            inner.menu_handlers.insert(target, handlers);
            match quit {
                Some(id) => _ = inner.quit_items.insert(target, id),
                None => _ = inner.quit_items.remove(&target),
            }
            drop(inner);
            ui.services.borrow_mut().set_menu(target, &data, activate.clone());
        });
        if let Some(replaced) = self.inner.borrow_mut().menu_effects.insert(target, effect) {
            replaced.dispose();
        }
    }

    /// Removes a window's menus, when the scope that installed them ends.
    fn remove_menu(&self, target: Option<NodeId>) {
        let effect = {
            let mut inner = self.inner.borrow_mut();
            inner.menu_handlers.remove(&target);
            inner.quit_items.remove(&target);
            inner.menu_effects.remove(&target)
        };
        if let Some(effect) = effect {
            effect.dispose();
            self.services.borrow_mut().set_menu(target, &MenuBarData::default(), self.menu_activate());
        }
    }

    /// Keeps the app running while no window is open, until [`Ui::quit`]:
    /// for an app that waits before it shows one, as for a notification's
    /// click or a request from another process. Without, an app ends when
    /// its last window closes.
    pub fn set_runs_without_windows(&self, runs: bool) {
        self.inner.borrow_mut().runs_without_windows = runs;
        self.changed();
    }

    /// Ends the app once this turn of its run loop is done, with any
    /// windows still open, without asking them first (unlike
    /// [`Ui::request_quit`]).
    pub fn quit(&self) {
        self.inner.borrow_mut().quit_requested = true;
        self.changed();
    }

    /// Whether the run loop should end now: [`Ui::quit`] was called, or no
    /// window is open and the app doesn't run without them. Backends ask
    /// after each turn, and before running at all.
    pub fn should_end(&self) -> bool {
        let inner = self.inner.borrow();
        inner.quit_requested || (inner.windows.is_empty() && !inner.runs_without_windows)
    }

    /// Quitting as the platform asks for it from outside the app's menus
    /// (macOS's Dock, logging out, the session ending): the app's Quit item,
    /// as if chosen, if it has one; otherwise every window is asked to
    /// close, as by its close button. Either way the app decides, and it
    /// ends when its last window closes. Backends call it, then see
    /// whether windows are left.
    pub fn request_quit(&self) {
        let (quit, events, windows) = {
            let inner = self.inner.borrow();
            // The app's own item first, else a window's.
            let quit = inner.quit_items.get(&None).or_else(|| inner.quit_items.values().next()).copied();
            (quit, inner.events.clone(), inner.windows.clone())
        };
        match quit {
            Some(id) => self.inner.borrow_mut().menu_queue.push_back(id),
            None => {
                for window in windows {
                    events.emit(window, UiEvent::WindowCloseRequested);
                }
            }
        }
        self.changed();
    }

    /// What platforms call with the id of the item chosen.
    fn menu_activate(&self) -> Rc<dyn Fn(u32)> {
        let weak = self.downgrade();
        Rc::new(move |id| {
            if let Some(ui) = weak.upgrade() {
                ui.inner.borrow_mut().menu_queue.push_back(id);
                ui.changed();
            }
        })
    }
}
