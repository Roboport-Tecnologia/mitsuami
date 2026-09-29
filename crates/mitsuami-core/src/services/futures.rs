//! Async glue: replies as futures, and the service calls on the current
//! `Ui`.

use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::task::{Poll, Waker};

use crate::ui::Ui;

use super::{Alert, MenuBar, OpenFile, Reply, SaveFile, ServiceError};

struct OneShot<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

/// A reply callback and the future that resolves when it is called.
pub(crate) fn reply_future<T: 'static>() -> (Reply<T>, impl Future<Output = T> + use<T>) {
    let shared = Rc::new(RefCell::new(OneShot { value: None, waker: None }));
    let sender = shared.clone();
    let reply: Reply<T> = Box::new(move |value| {
        let waker = {
            let mut shared = sender.borrow_mut();
            shared.value = Some(value);
            shared.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    });
    let future = std::future::poll_fn(move |cx| {
        let mut shared = shared.borrow_mut();
        match shared.value.take() {
            Some(value) => Poll::Ready(value),
            None => {
                shared.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    });
    (reply, future)
}

fn ui() -> Ui {
    crate::task::current_ui()
}

pub fn clipboard_text() -> impl Future<Output = Option<String>> + use<> {
    ui().clipboard_text()
}

/// Puts text on the clipboard right away; await to learn whether it worked.
pub fn set_clipboard_text(text: &str) -> impl Future<Output = Result<(), ServiceError>> + use<> {
    ui().set_clipboard_text(text)
}

/// Shows an alert on the active window; resolves to the chosen button's index.
pub fn alert(alert: Alert) -> impl Future<Output = usize> + use<> {
    ui().alert(None, alert)
}

pub fn open_file(request: OpenFile) -> impl Future<Output = Option<Vec<PathBuf>>> + use<> {
    ui().open_file(None, request)
}

pub fn save_file(request: SaveFile) -> impl Future<Output = Option<PathBuf>> + use<> {
    ui().save_file(None, request)
}

/// Installs the app's menus; see [`Ui::set_menu`]. A window's own menus
/// are a [`MenuBar`] in its content.
pub fn set_menu(menu: MenuBar) {
    ui().set_menu(menu);
}
