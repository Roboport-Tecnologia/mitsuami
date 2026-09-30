//! `NumberField`: how AppKit shows a `NumberInput`. AppKit has no spin box;
//! its apps put an `NSStepper` beside a text field, and so does this.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSControl, NSStepper, NSTextField, NSUserInterfaceLayoutDirection, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use mitsuami_core::{EventSink, EventValue, NodeId, Size, UiEvent};

use crate::classes::zero_rect;

/// Between the field and its stepper.
const GAP: f64 = 4.0;

pub struct NumberFieldIvars {
    id: NodeId,
    events: EventSink,
    field: Retained<NSTextField>,
    stepper: Retained<NSStepper>,
}

define_class!(
    /// A text field with an `NSStepper` beside it, kept in step: the
    /// stepper holds the number, its range and its increment, and the
    /// field shows the number and commits what's typed into it when editing
    /// ends. Tweaks get this view: reach the parts with
    /// [`field`](Self::field) and [`stepper`](Self::stepper).
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = NumberFieldIvars]
    pub struct NumberField;

    impl NumberField {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.place_parts();
        }

        /// The stepper was clicked (or incremented by assistive technology).
        #[unsafe(method(stepped:))]
        fn stepped(&self, _sender: &AnyObject) {
            self.show_number();
            self.report();
        }

        /// Editing ended: Return, Tab or a click elsewhere. What was typed
        /// becomes the number, rounded and kept in range, as the other
        /// platforms' spin boxes take it; text that isn't a number puts
        /// the number back.
        #[unsafe(method(committed:))]
        fn committed(&self, _sender: &AnyObject) {
            let NumberFieldIvars { field, stepper, .. } = self.ivars();
            let typed = field.stringValue().to_string().trim().parse::<f64>().ok().filter(|v| v.is_finite());
            let before = stepper.doubleValue();
            if let Some(typed) = typed {
                stepper.setDoubleValue(typed.round().clamp(stepper.minValue(), stepper.maxValue()));
            }
            self.show_number();
            if stepper.doubleValue() != before {
                self.report();
            }
        }
    }
);

impl NumberField {
    pub(crate) fn new(mtm: MainThreadMarker, id: NodeId, events: EventSink) -> Retained<NumberField> {
        let field = NSTextField::textFieldWithString(&objc2_foundation::NSString::from_str("0"), mtm);
        // Leaving the field commits what was typed, as Return does.
        if let Some(cell) = field.cell() {
            cell.setSendsActionOnEndEditing(true);
        }
        let stepper = NSStepper::new(mtm);
        // Whole numbers only: Qt's spin box holds an `int`.
        stepper.setIncrement(1.0);
        let this = NumberField::alloc(mtm).set_ivars(NumberFieldIvars {
            id,
            events,
            field: field.clone(),
            stepper: stepper.clone(),
        });
        let this: Retained<NumberField> = unsafe { msg_send![super(this), initWithFrame: zero_rect()] };
        // SAFETY: the view owns both controls, so it outlives them as their
        // (weakly held) target.
        unsafe {
            field.setTarget(Some(&this));
            field.setAction(Some(sel!(committed:)));
            stepper.setTarget(Some(&this));
            stepper.setAction(Some(sel!(stepped:)));
        }
        this.addSubview(&field);
        this.addSubview(&stepper);
        this
    }

    /// The text field that shows and takes the number.
    pub fn field(&self) -> &NSTextField {
        &self.ivars().field
    }

    /// The stepper beside it, which holds the number, its range and its
    /// increment.
    pub fn stepper(&self) -> &NSStepper {
        &self.ivars().stepper
    }

    pub(crate) fn controls(&self) -> [&NSControl; 2] {
        [&self.ivars().field, &self.ivars().stepper]
    }

    /// Shows the stepper's number in the field.
    pub(crate) fn show_number(&self) {
        let NumberFieldIvars { field, stepper, .. } = self.ivars();
        let text = format!("{}", stepper.doubleValue() as i64);
        if field.stringValue().to_string() != text {
            field.setStringValue(&objc2_foundation::NSString::from_str(&text));
        }
    }

    fn report(&self) {
        let NumberFieldIvars { id, events, stepper, .. } = self.ivars();
        events.emit(*id, UiEvent::Changed(EventValue::Number(stepper.doubleValue())));
    }

    /// As wide as the range's longest number needs, and the stepper.
    pub(crate) fn natural_size(&self) -> Size {
        let NumberFieldIvars { field, stepper, .. } = self.ivars();
        let longest = [stepper.minValue(), stepper.maxValue()]
            .map(|v| format!("{}", v as i64))
            .into_iter()
            .max_by_key(|s| s.len())
            .unwrap_or_default();
        let text_width = field.cell().map_or(0.0, |cell| {
            // A copy, so the field's own text stays put.
            let cell: Retained<objc2_app_kit::NSCell> = unsafe { msg_send![&*cell, copy] };
            cell.setStringValue(&objc2_foundation::NSString::from_str(&longest));
            cell.cellSizeForBounds(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0e7, 1.0e7))).width
        });
        // Room for the caret after the last digit.
        let field_size = NSSize::new(text_width.ceil() + 4.0, field.intrinsicContentSize().height);
        let stepper_size = stepper.intrinsicContentSize();
        Size::new(
            (field_size.width + GAP + stepper_size.width).ceil() as f32,
            field_size.height.max(stepper_size.height).ceil() as f32,
        )
    }

    /// The field takes the width the stepper leaves; both are centred
    /// vertically, by their alignment rects, as Auto Layout places them.
    /// The stepper is on the trailing side: the left in right-to-left.
    fn place_parts(&self) {
        let NumberFieldIvars { field, stepper, .. } = self.ivars();
        let bounds = self.bounds().size;
        let stepper_size = stepper.intrinsicContentSize();
        let field_height = field.intrinsicContentSize().height;
        let field_width = (bounds.width - GAP - stepper_size.width).max(0.0);
        let centred = |height: f64| ((bounds.height - height) / 2.0).round();
        let rtl = self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft;
        let (field_x, stepper_x) = if rtl { (stepper_size.width + GAP, 0.0) } else { (0.0, field_width + GAP) };
        field.setFrame(field.frameForAlignmentRect(NSRect::new(
            NSPoint::new(field_x, centred(field_height)),
            NSSize::new(field_width, field_height),
        )));
        stepper.setFrame(
            stepper.frameForAlignmentRect(NSRect::new(
                NSPoint::new(stepper_x, centred(stepper_size.height)),
                stepper_size,
            )),
        );
    }

    /// Its parts' direction, and their places.
    pub(crate) fn set_direction(&self, direction: NSUserInterfaceLayoutDirection) {
        self.setUserInterfaceLayoutDirection(direction);
        self.field().setUserInterfaceLayoutDirection(direction);
        self.stepper().setUserInterfaceLayoutDirection(direction);
        self.field().setAlignment(crate::backend::field_alignment(direction));
        self.place_parts();
    }
}
