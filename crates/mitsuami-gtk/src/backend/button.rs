//! Buttons: role classes, and what a button shows (label, icon, both).

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::{ButtonRole, Prop, VectorIcon};

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
    /// The app's own icon, which wins over the named one (`None` inside:
    /// unset). GTK can't give it back.
    pub(super) vector: Option<Option<VectorIcon>>,
}

/// What a `gtk::Button` and a `gtk::MenuButton` both do with a caption
/// and an icon, under different types.
pub(super) trait Face: IsA<gtk::Accessible> + IsA<gtk::Widget> {
    fn show_label(&self, label: &str);
    fn show_icon(&self, icon: &str);
    fn show_child(&self, child: &gtk::Widget);
    /// An image alone, as an icon name shows: no arrow on a menu button,
    /// and the icon button's style.
    fn show_image(&self, image: &gtk::Image);
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
    fn show_image(&self, image: &gtk::Image) {
        self.set_child(Some(image));
        self.add_css_class("image-button");
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
    // The style goes on the menu button's own button, as `icon-name` puts it.
    fn show_image(&self, image: &gtk::Image) {
        self.set_always_show_arrow(false);
        self.set_child(Some(image));
        if let Some(inner) = self.first_child() {
            inner.add_css_class("image-button");
        }
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
        if let Some(gicon) = self.vector.clone().flatten().and_then(|v| super::vector_icon::gicon(&v)) {
            let image = gtk::Image::from_gicon(&gicon);
            if self.icon_only == Some(true) {
                button.show_image(&image);
                button.update_property(&[gtk::accessible::Property::Label(&self.label)]);
            } else {
                // As libadwaita's `ButtonContent` lays out an icon and a
                // label, which takes only icon names.
                let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                content.set_halign(gtk::Align::Center);
                content.append(&image);
                content.append(&gtk::Label::new(Some(&self.label)));
                button.show_child(content.upcast_ref());
                button.add_css_class("image-text-button");
                button.reset_property(gtk::AccessibleProperty::Label);
            }
            return;
        }
        button.remove_css_class("image-text-button");
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
        if let Some(shown) = child.as_ref().and_then(vector_face) {
            // A vector icon hides the name, which is then the one given.
            let only = shown.is_none();
            let mut props = vec![Prop::Label(shown.unwrap_or_else(|| self.label.clone()))];
            props.extend(self.icon.clone().map(Prop::Icon));
            props.extend(self.icon_only.map(|_| Prop::IconOnly(only)));
            props.extend(self.vector.clone().map(Prop::VectorIcon));
            return props;
        }
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
        props.extend(self.vector.clone().map(Prop::VectorIcon));
        props
    }
}

/// What a button shows of a vector icon: the image alone (`Some(None)`), or
/// with its label (`Some(Some(label))`); `None` if it shows none.
fn vector_face(child: &gtk::Widget) -> Option<Option<String>> {
    let is_vector = |w: &gtk::Widget| w.downcast_ref::<gtk::Image>().is_some_and(super::vector_icon::shows_gicon);
    if is_vector(child) {
        return Some(None);
    }
    let content = child.downcast_ref::<gtk::Box>()?;
    let (image, label) = (content.first_child()?, content.last_child()?);
    let label = label.downcast_ref::<gtk::Label>()?;
    is_vector(&image).then(|| Some(label.text().to_string()))
}
