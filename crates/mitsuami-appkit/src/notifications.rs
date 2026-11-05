//! Notifications on macOS: the User Notifications framework, as Mac apps
//! post theirs to Notification Center.

use std::cell::RefCell;
use std::rc::Rc;

use block2::RcBlock;
use dispatch2::DispatchQueue;
use mitsuami_core::services::{Notification, ServiceError};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, define_class, msg_send};
use objc2_foundation::{NSArray, NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNErrorCode, UNErrorDomain, UNMutableNotificationContent,
    UNNotificationDefaultActionIdentifier, UNNotificationRequest, UNNotificationResponse, UNNotificationSound,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use crate::services::{answer, wait};

/// Tells the core which notification was clicked.
type Activated = Rc<dyn Fn(String)>;

thread_local! {
    /// What the core gave with the last notification, for the clicks.
    static ACTIVATED: RefCell<Option<Activated>> = const { RefCell::new(None) };
    static DELEGATE: RefCell<Option<Retained<Delegate>>> = const { RefCell::new(None) };
}

define_class!(
    /// Hears the user's clicks. The framework may call it on any thread,
    /// so the id goes to the main queue.
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    // No `willPresentNotification`: without it, a notification that comes
    // while the app is frontmost goes to Notification Center's list
    // without a banner, macOS's default. The user is in the app already.
    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            done: &block2::DynBlock<dyn Fn()>,
        ) {
            // Only the click on the notification itself; it has no buttons.
            if response.actionIdentifier().isEqualToString(unsafe { UNNotificationDefaultActionIdentifier }) {
                let id = response.notification().request().identifier().to_string();
                DispatchQueue::main().exec_async(move || {
                    if let Some(activated) = ACTIVATED.with(|a| a.borrow().clone()) {
                        activated(id);
                    }
                });
            }
            done.call(());
        }
    }
);

/// The app's notification center, with its delegate. Outside a bundle
/// there's none: `currentNotificationCenter` raises an exception for a
/// process with no bundle id (the test kit's, an example run with
/// `cargo run`).
fn center() -> Option<Retained<UNUserNotificationCenter>> {
    NSBundle::mainBundle().bundleIdentifier()?;
    let center = UNUserNotificationCenter::currentNotificationCenter();
    DELEGATE.with(|delegate| {
        let mut delegate = delegate.borrow_mut();
        let delegate = delegate.get_or_insert_with(|| unsafe { msg_send![Delegate::alloc(), init] });
        // The center holds its delegate weakly; the thread-local keeps it.
        center.setDelegate(Some(ProtocolObject::from_ref(&**delegate)));
    });
    Some(center)
}

/// Sets the delegate as the app starts, so a click that launches or
/// reaches it before its first notification of this run isn't lost to
/// the framework. Its id goes nowhere until the app posts one.
pub(crate) fn listen() {
    center();
}

fn error(error: &NSError) -> ServiceError {
    let domain = unsafe { UNErrorDomain };
    let not_allowed = domain.is_some_and(|domain| error.domain().isEqualToString(domain))
        && error.code() == UNErrorCode::NotificationsNotAllowed.0;
    if not_allowed { ServiceError::Cancelled } else { ServiceError::Failed(error.localizedDescription().to_string()) }
}

/// Asks to post notifications (macOS asks the user the first time, and
/// answers from their choice in System Settings after that), then posts
/// it. The same identifier replaces the one shown.
pub(crate) fn notify(
    notification: &Notification,
    activated: Activated,
    reply: mitsuami_core::services::Reply<Result<(), ServiceError>>,
) {
    let Some(center) = center() else { return reply(Err(ServiceError::Unavailable)) };
    ACTIVATED.with(|a| *a.borrow_mut() = Some(activated));
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&notification.title));
    if let Some(body) = &notification.body {
        content.setBody(&NSString::from_str(body));
    }
    // The user's settings decide whether it plays: without a sound it never does.
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let id = NSString::from_str(notification.id.as_deref().unwrap_or_default());
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);

    let ticket = wait(reply);
    let asked = RcBlock::new(move |granted: Bool, failed: *mut NSError| {
        // SAFETY: the framework passes a valid error or null.
        if let Some(failed) = unsafe { failed.as_ref() } {
            return answer(ticket, Err(error(failed)));
        }
        if !granted.as_bool() {
            return answer(ticket, Err(ServiceError::Cancelled));
        }
        let added = RcBlock::new(move |failed: *mut NSError| {
            answer(ticket, unsafe { failed.as_ref() }.map_or(Ok(()), |failed| Err(error(failed))));
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&added));
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &asked,
    );
}

/// From the screen and Notification Center's list, and before it's shown.
pub(crate) fn withdraw(id: &str) {
    let Some(center) = center() else { return };
    let ids = NSArray::from_retained_slice(&[NSString::from_str(id)]);
    center.removePendingNotificationRequestsWithIdentifiers(&ids);
    center.removeDeliveredNotificationsWithIdentifiers(&ids);
}
