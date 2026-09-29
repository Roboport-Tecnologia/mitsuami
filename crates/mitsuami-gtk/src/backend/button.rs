//! Buttons: role classes, and what a button shows (label, icon, both).

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::{ButtonRole, Prop};

/// GNOME has no cancel style: cancel buttons are normal buttons.
pub(super) fn role_class(role: ButtonRole) -> Option<&'static str> {
    match role {
        ButtonRole::Normal | ButtonRole::Cancel => None,
        ButtonRole::Default => Some("suggested-action"),
        ButtonRole::Destructive => Some("destructive-action"),
    }
}

pub(super) const ROLE_CLASSES: [&str; 2] = ["suggested-action", "destructive-action"];

/// What a button shows: its label, an icon before it in libadwaita's
/// `ButtonContent` (as GNOME apps put one there), or the icon alone, GTK's
/// own icon button, with the label as its accessible name.
#[derive(Default)]
pub(super) struct ButtonFace {
    pub(super) label: String,
    /// The icon, once the app gave one (empty: none).
    pub(super) icon: Option<String>,
    /// Whether the app gave `IconOnly`.
    pub(super) icon_only: Option<bool>,
}

/// What a `gtk::Button` and a `gtk::MenuButton` both do with a caption
/// and an icon, under different types.
pub(super) trait Face: IsA<gtk::Accessible> {
    fn show_label(&self, label: &str);
    fn show_icon(&self, icon: &str);
    fn show_child(&self, child: &gtk::Widget);
    fn shown_label(&self) -> Option<glib::GString>;
    fn shown_icon(&self) -> Option<glib::GString>;
    fn shown_child(&self) -> Option<gtk::Widget>;
}

impl Face for gtk::Button {
    fn show_label(&self, label: &str) {
        self.set_label(label);
    }
    fn show_icon(&self, icon: &str) {
        self.set_icon_name(icon);
    }
    fn show_child(&self, child: &gtk::Widget) {
        self.set_child(Some(child));
    }
    fn shown_label(&self) -> Option<glib::GString> {
        self.label()
    }
    fn shown_icon(&self) -> Option<glib::GString> {
        self.icon_name()
    }
    fn shown_child(&self) -> Option<gtk::Widget> {
        self.child()
    }
}

/// GTK draws the arrow after a caption on its own, and none after an icon
/// alone, as GNOME's icon menu buttons have none; a child of our own
/// (icon and caption) needs `always-show-arrow` for it.
impl Face for gtk::MenuButton {
    fn show_label(&self, label: &str) {
        self.set_always_show_arrow(false);
        self.set_label(label);
    }
    fn show_icon(&self, icon: &str) {
        self.set_always_show_arrow(false);
        self.set_icon_name(icon);
    }
    fn show_child(&self, child: &gtk::Widget) {
        self.set_child(Some(child));
        self.set_always_show_arrow(true);
    }
    fn shown_label(&self) -> Option<glib::GString> {
        self.label()
    }
    fn shown_icon(&self) -> Option<glib::GString> {
        self.icon_name()
    }
    fn shown_child(&self) -> Option<gtk::Widget> {
        self.child()
    }
}

impl ButtonFace {
    pub(super) fn show(&self, button: &impl Face) {
        let icon = self.icon.as_deref().unwrap_or_default();
        if icon.is_empty() {
            button.show_label(&self.label);
            button.reset_property(gtk::AccessibleProperty::Label);
        } else if self.icon_only == Some(true) {
            button.show_icon(icon);
            button.update_property(&[gtk::accessible::Property::Label(&self.label)]);
        } else {
            let content = adw::ButtonContent::new();
            content.set_icon_name(icon);
            content.set_label(&self.label);
            button.show_child(content.upcast_ref());
            button.reset_property(gtk::AccessibleProperty::Label);
        }
    }

    /// Read from what the button shows; an icon button's label is only its
    /// accessible name, which GTK doesn't read back.
    pub(super) fn read(&self, button: &impl Face) -> Vec<Prop> {
        let child = button.shown_child();
        let content = child.as_ref().and_then(|c| c.downcast_ref::<adw::ButtonContent>());
        let (label, icon, only) = match (button.shown_label(), content, button.shown_icon()) {
            (Some(label), _, _) => (label.to_string(), String::new(), false),
            (None, Some(content), _) => (content.label().to_string(), content.icon_name().to_string(), false),
            (None, None, Some(icon)) => (self.label.clone(), icon.to_string(), true),
            (None, None, None) => (String::new(), String::new(), false),
        };
        let mut props = vec![Prop::Label(label)];
        let given = self.icon.as_deref().unwrap_or_default();
        if self.icon.is_some() || !icon.is_empty() {
            props.push(Prop::Icon(icon));
        }
        if let Some(asked) = self.icon_only {
            // Without an icon a button shows its caption, as asked or not.
            props.push(Prop::IconOnly(if given.is_empty() { asked } else { only }));
        }
        props
    }
}
