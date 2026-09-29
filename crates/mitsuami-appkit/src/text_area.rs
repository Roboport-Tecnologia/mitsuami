//! How AppKit shows a `TextArea`: an `NSTextView` in an `NSScrollView`, as
//! `NSTextView.scrollableTextView()` makes them, in a bezel as Interface
//! Builder's text views come.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{ClassType, MainThreadMarker};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBorderType, NSColor, NSControlSize, NSFont, NSScrollView, NSScroller, NSTextView,
    NSTextViewDelegate,
};
use objc2_foundation::{NSRange, NSSize, NSString};

use mitsuami_core::Size;

/// A text field's width, which a text area has on the other platforms too.
const WIDTH: f64 = 200.0;

pub(crate) struct TextArea {
    pub(crate) scroll: Retained<NSScrollView>,
    pub(crate) text: Retained<NSTextView>,
    /// What the app gave, which AppKit has no place for (a text view has
    /// no placeholder) or can't hold apart (a text view has no enabled
    /// state: disabled, it's neither editable nor selectable).
    pub(crate) placeholder: Option<String>,
    pub(crate) lines: u32,
    read_only: bool,
    enabled: bool,
}

impl TextArea {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        delegate: Option<&ProtocolObject<dyn NSTextViewDelegate>>,
        font: &NSFont,
    ) -> TextArea {
        let scroll = NSTextView::scrollableTextView(mtm);
        scroll.setBorderType(NSBorderType::BezelBorder);
        let text = scroll
            .documentView()
            .and_then(|v| v.downcast::<NSTextView>().ok())
            .expect("a scrollable text view's document view is its text view");
        // Plain text in the system font, with undo, as a form's text view
        // is set up in Interface Builder.
        text.setRichText(false);
        text.setAllowsUndo(true);
        text.setFont(Some(font));
        // Held weakly: the node keeps it alive as long as the view.
        text.setDelegate(delegate);
        TextArea { scroll, text, placeholder: None, lines: 1, read_only: false, enabled: true }
    }

    pub(crate) fn set_string(&self, value: &str) {
        // Don't disturb the caret when it already shows it.
        if self.text.string().to_string() != value {
            self.text.setString(&NSString::from_str(value));
            self.fit_width();
        }
    }

    pub(crate) fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.apply_state();
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.apply_state();
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// As the text view holds them: read-only while it's disabled is what
    /// the app gave.
    pub(crate) fn read_only(&self) -> bool {
        if self.text.isSelectable() { !self.text.isEditable() } else { self.read_only }
    }

    /// Disabled as AppKit apps disable a text view: not editable, not
    /// selectable, in the disabled text colour, and without the selection,
    /// which it would go on showing. Read-only, it's still selectable.
    fn apply_state(&self) {
        self.text.setEditable(self.enabled && !self.read_only);
        self.text.setSelectable(self.enabled);
        if !self.enabled {
            let selected = self.text.selectedRange();
            self.text.setSelectedRange(NSRange::new(selected.location, 0));
        }
        let color = if self.enabled { NSColor::textColor() } else { NSColor::disabledControlTextColor() };
        self.text.setTextColor(Some(&color));
    }

    /// Lines wrapped to the view's width, or as long as their text, with
    /// a horizontal scroller, as Apple's text system guide sets a text
    /// view up for each.
    pub(crate) fn set_line_wrap(&self, wrap: bool) {
        let Some(container) = (unsafe { self.text.textContainer() }) else { return };
        let huge = f32::MAX as f64;
        self.scroll.setHasHorizontalScroller(!wrap);
        self.text.setHorizontallyResizable(!wrap);
        self.text.setMaxSize(NSSize::new(huge, huge));
        container.setWidthTracksTextView(wrap);
        if wrap {
            let width = self.scroll.contentSize().width;
            container.setContainerSize(NSSize::new(width, huge));
            self.text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
            self.text.setFrameSize(NSSize::new(width, self.text.frame().size.height));
        } else {
            container.setContainerSize(NSSize::new(huge, huge));
            self.text.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            self.fit_width();
        }
    }

    /// Unwrapped, as wide as its longest line: laid out now, as the text
    /// system would at the next display.
    fn fit_width(&self) {
        if let (Some(container), Some(layout)) =
            (unsafe { self.text.textContainer() }, unsafe { self.text.layoutManager() })
            && !container.widthTracksTextView()
        {
            layout.ensureLayoutForTextContainer(&container);
            self.text.sizeToFit();
        }
    }

    pub(crate) fn line_wrap(&self) -> bool {
        unsafe { self.text.textContainer() }.is_none_or(|c| c.widthTracksTextView())
    }

    /// A text field's width, and `lines` lines of its font inside the
    /// text container's insets and the scroll view's border and scroller.
    pub(crate) fn natural_size(&self, mtm: MainThreadMarker) -> Size {
        let Some(font) = self.text.font() else { return Size::ZERO };
        let line = (font.ascender() - font.descender() + font.leading()).ceil();
        let inset = self.text.textContainerInset();
        let content = NSSize::new(WIDTH, line * self.lines as f64 + inset.height * 2.0);
        // SAFETY: `NSScroller` is the scroller class the scroll view uses.
        let frame = unsafe {
            NSScrollView::frameSizeForContentSize_horizontalScrollerClass_verticalScrollerClass_borderType_controlSize_scrollerStyle(
                content,
                None,
                self.scroll.hasVerticalScroller().then(NSScroller::class),
                self.scroll.borderType(),
                NSControlSize::Regular,
                self.scroll.scrollerStyle(),
                mtm,
            )
        };
        Size::new(WIDTH as f32, frame.height.ceil() as f32)
    }
}
