//! Notifications on KDE: the freedesktop.org notification server on the
//! session bus (Plasma's, or whichever the desktop runs), which is what
//! KNotification shows Plasma's own apps' notifications through.
//!
//! The server names notifications by numbers of its own, so the app's ids
//! are kept with them: a notification with an id shown before replaces it
//! (`replaces_id`), and withdrawing closes it. Notifications go one at a
//! time, so one that replaces another goes with the server's id of that
//! one, even when the app sends both at once.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};

use mitsuami_core::services::{Notification, Reply, ServiceError};

use crate::ffi::{self, NotificationEvent};

/// The spec's reason for a notification that timed out: Plasma keeps it in
/// its list, where it can still be clicked or replaced.
const EXPIRED: u32 = 1;

#[derive(Default)]
pub(crate) struct Notifications {
    state: Rc<RefCell<State>>,
}

#[derive(Default)]
struct State {
    /// The server's ids of the notifications it has, by the app's.
    ids: HashMap<String, u32>,
    /// The activation tokens the server gave for the clicks that follow.
    tokens: HashMap<u32, String>,
    /// Waiting for the one being sent.
    queue: VecDeque<Request>,
    /// The one being sent, until the server answers.
    sending: Option<Sending>,
    activated: Option<Rc<dyn Fn(String)>>,
    watching: bool,
}

struct Request {
    notification: Notification,
    reply: Reply<Result<(), ServiceError>>,
}

struct Sending {
    id: String,
    /// The app withdrew it before the server answered: it's closed then.
    withdrawn: bool,
}

impl Notifications {
    pub(crate) fn notify(
        &self,
        notification: Notification,
        activated: Rc<dyn Fn(String)>,
        reply: Reply<Result<(), ServiceError>>,
    ) {
        let watch = {
            let mut state = self.state.borrow_mut();
            state.activated = Some(activated);
            state.queue.push_back(Request { notification, reply });
            !std::mem::replace(&mut state.watching, true)
        };
        // Before the first one, so no click on it is missed. Without a
        // session bus the notification answers `Unavailable` itself.
        if watch {
            let weak = Rc::downgrade(&self.state);
            let watching = ffi::watch_notifications(move |event| heard(&weak, event));
            self.state.borrow_mut().watching = watching;
        }
        send_next(&self.state);
    }

    pub(crate) fn withdraw(&self, id: &str) {
        let (dropped, shown) = {
            let mut state = self.state.borrow_mut();
            let (dropped, queue): (VecDeque<Request>, _) =
                std::mem::take(&mut state.queue).into_iter().partition(|r| r.notification.id.as_deref() == Some(id));
            state.queue = queue;
            if let Some(sending) = state.sending.as_mut().filter(|s| s.id == id) {
                sending.withdrawn = true;
            }
            let shown = state.ids.remove(id);
            if let Some(shown) = shown {
                state.tokens.remove(&shown);
            }
            (dropped, shown)
        };
        if let Some(shown) = shown {
            ffi::close_notification(shown);
        }
        // Never shown, and not to be: the app has what it asked for.
        for request in dropped {
            (request.reply)(Ok(()));
        }
    }
}

/// Sends the next notification waiting, unless one is being sent.
fn send_next(state: &Rc<RefCell<State>>) {
    let (request, replaces) = {
        let mut s = state.borrow_mut();
        if s.sending.is_some() {
            return;
        }
        let Some(request) = s.queue.pop_front() else { return };
        let id = request.notification.id.clone().unwrap_or_default();
        let replaces = s.ids.get(&id).copied().unwrap_or(0);
        s.sending = Some(Sending { id, withdrawn: false });
        (request, replaces)
    };
    let Request { notification, reply } = request;
    let weak = Rc::downgrade(state);
    ffi::notify(replaces, &notification.title, notification.body.as_deref(), move |answer| {
        let Some(state) = weak.upgrade() else { return reply(answer.map(|_| ())) };
        let close = {
            let mut s = state.borrow_mut();
            match (s.sending.take(), &answer) {
                (Some(sent), Ok(shown)) if sent.withdrawn => Some(*shown),
                (Some(sent), Ok(shown)) => {
                    s.ids.insert(sent.id, *shown);
                    None
                }
                _ => None,
            }
        };
        if let Some(shown) = close {
            ffi::close_notification(shown);
        }
        reply(answer.map(|_| ()));
        send_next(&state);
    });
}

/// What the server says of notifications, its own and other apps'.
fn heard(state: &Weak<RefCell<State>>, event: NotificationEvent) {
    let Some(state) = state.upgrade() else { return };
    let mut s = state.borrow_mut();
    let ours = |s: &State, shown: u32| s.ids.iter().find(|(_, id)| **id == shown).map(|(id, _)| id.clone());
    match event {
        // The spec sends it before the action it's for.
        NotificationEvent::Token(shown, token) if ours(&s, shown).is_some() => _ = s.tokens.insert(shown, token),
        NotificationEvent::Action(shown, action) if action == "default" => {
            let Some(id) = ours(&s, shown) else { return };
            let token = s.tokens.remove(&shown);
            let activated = s.activated.clone();
            drop(s);
            ffi::raise_app(token.as_deref());
            if let Some(activated) = activated {
                activated(id);
            }
        }
        NotificationEvent::Closed(shown, reason) => {
            s.tokens.remove(&shown);
            if reason != EXPIRED {
                s.ids.retain(|_, id| *id != shown);
            }
        }
        _ => {}
    }
}
