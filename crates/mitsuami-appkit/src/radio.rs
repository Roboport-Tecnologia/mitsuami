//! A radio group: `NSButton`s of the radio type down an `NSStackView`. As
//! siblings with the same action, AppKit turns the others off when one is
//! clicked. The stack view is the group to assistive technology.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mitsuami_core::{EventSink, EventValue, NodeId, UiEvent};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityRadioGroupRole, NSButton, NSControlStateValueOff, NSControlStateValueOn,
    NSLayoutAttribute, NSResponder, NSStackView, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSArray, NSString};

use crate::classes::ClosureTarget;

pub(crate) struct RadioGroup {
    pub stack: Retained<NSStackView>,
    buttons: RefCell<Vec<Retained<NSButton>>>,
    /// The option chosen, as last set or reported: a click on the chosen
    /// button sends its action again, and that isn't a change.
    chosen: Rc<Cell<Option<usize>>>,
    enabled: Cell<bool>,
    target: Retained<ClosureTarget>,
    mtm: MainThreadMarker,
}

impl RadioGroup {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> RadioGroup {
        // Down a column at the stack view's own spacing, leading edges
        // lined up, as AppKit stacks controls.
        let stack = NSStackView::stackViewWithViews(&NSArray::new(), mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::Leading);
        // A stack view sizes itself to fit in AppKit's layout passes (a
        // window's display, a capture) unless its frame is its size: the
        // core's frame, whose leading edge is the right one in
        // right-to-left. Measuring (`fittingSize`) is the same either way.
        stack.setTranslatesAutoresizingMaskIntoConstraints(true);
        stack.setAccessibilityElement(true);
        stack.setAccessibilityRole(Some(unsafe { NSAccessibilityRadioGroupRole }));
        let chosen: Rc<Cell<Option<usize>>> = Rc::default();
        let c = chosen.clone();
        let target = ClosureTarget::new(mtm, move |sender: &AnyObject| {
            let Some(button) = sender.downcast_ref::<NSButton>() else { return };
            let index = button.tag() as usize;
            if c.replace(Some(index)) != Some(index) {
                events.emit(id, UiEvent::Changed(EventValue::Index(index)));
            }
        });
        RadioGroup { stack, buttons: RefCell::default(), chosen, enabled: Cell::new(true), target, mtm }
    }

    /// A button for each option. The ones there already keep their place
    /// (and whether they're on), with the option now at it.
    pub(crate) fn set_options(&self, options: &[String]) {
        let mut buttons = self.buttons.borrow_mut();
        while buttons.len() > options.len() {
            let button = buttons.pop().unwrap();
            self.stack.removeArrangedSubview(&button);
            button.removeFromSuperview();
        }
        for (index, option) in options.iter().enumerate() {
            if let Some(button) = buttons.get(index) {
                button.setTitle(&NSString::from_str(option));
                continue;
            }
            let target: &AnyObject = self.target.as_ref();
            let button = unsafe {
                NSButton::radioButtonWithTitle_target_action(
                    &NSString::from_str(option),
                    Some(target),
                    Some(sel!(fire:)),
                    self.mtm,
                )
            };
            button.setTag(index as isize);
            button.setEnabled(self.enabled.get());
            button.setUserInterfaceLayoutDirection(self.stack.userInterfaceLayoutDirection());
            button.setImagePosition(crate::backend::toggle_image_position(self.stack.userInterfaceLayoutDirection()));
            self.stack.addArrangedSubview(&button);
            buttons.push(button);
        }
        if self.chosen.get().is_some_and(|i| i >= buttons.len()) {
            self.chosen.set(None);
        }
    }

    /// The stack lines the buttons up on its leading edge, the right one
    /// in right-to-left; each button puts its circle on that side.
    pub(crate) fn set_direction(&self, direction: objc2_app_kit::NSUserInterfaceLayoutDirection) {
        self.stack.setUserInterfaceLayoutDirection(direction);
        for button in self.buttons.borrow().iter() {
            button.setUserInterfaceLayoutDirection(direction);
            button.setImagePosition(crate::backend::toggle_image_position(direction));
        }
    }

    pub(crate) fn options(&self) -> Vec<String> {
        self.buttons.borrow().iter().map(|b| b.title().to_string()).collect()
    }

    /// Every button's state: AppKit only turns the others off on a click.
    pub(crate) fn set_selected(&self, index: Option<usize>) {
        let buttons = self.buttons.borrow();
        let index = index.filter(|i| *i < buttons.len());
        for (i, button) in buttons.iter().enumerate() {
            button.setState(if Some(i) == index { NSControlStateValueOn } else { NSControlStateValueOff });
        }
        self.chosen.set(index);
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.buttons.borrow().iter().position(|b| b.state() == NSControlStateValueOn)
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.buttons.borrow().iter().for_each(|b| b.setEnabled(enabled));
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Clicks the first button with this option, as the user does: its
    /// action reports it, unless it was the chosen one.
    pub(crate) fn choose(&self, option: &str) -> bool {
        let button = self.buttons.borrow().iter().find(|b| b.title().to_string() == option).cloned();
        let Some(button) = button else { return false };
        unsafe { button.performClick(None) };
        true
    }

    /// The button that takes focus: the chosen one, or the first, as
    /// Tab reaches a group.
    pub(crate) fn key_view(&self) -> Retained<NSView> {
        let buttons = self.buttons.borrow();
        let button = buttons.iter().find(|b| b.state() == NSControlStateValueOn).or(buttons.first());
        match button {
            Some(button) => Retained::into_super(Retained::into_super(button.clone())),
            None => Retained::into_super(self.stack.clone()),
        }
    }

    /// One of its buttons is the first responder.
    pub(crate) fn has_focus(&self, responder: &NSResponder) -> bool {
        self.buttons.borrow().iter().any(|b| std::ptr::eq(responder as *const _ as *const NSButton, &**b))
    }
}
