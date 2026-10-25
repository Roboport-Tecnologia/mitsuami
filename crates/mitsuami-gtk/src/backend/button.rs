//! Buttons: role classes, and what a button shows (label, icon, both).

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::{ButtonRole, Color, Prop, VectorIcon};

use super::text::{IconColorRule, icon_color, set_icon_color};

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
    /// The icon's colour, once the app gave one: on the image alone, so
    /// the caption and the button keep the theme's.
    pub(super) color: Option<Color>,
    /// Its rule in the display's style sheet, while the image uses one.
    color_rule: Option<IconColorRule>,
}

/// What a `gtk::Button`, a `gtk::MenuButton` and an `adw::SplitButton` all
/// do with a caption and an icon, under different types.
pub(super) trait Face: IsA<gtk::Widget> {
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

/// libadwaita's split button shows a caption, icon or child as a button
/// does, on its own button, and always draws its arrow beside it.
impl Face for adw::SplitButton {
    fn show_label(&self, label: &str) {
        self.set_label(label);
    }
    fn show_icon(&self, icon: &str) {
        self.set_icon_name(icon);
    }
    fn show_child(&self, child: &gtk::Widget) {
        self.set_child(Some(child));
    }
    // The style goes on its own button, as `icon-name` puts it.
    fn show_image(&self, image: &gtk::Image) {
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
    /// Shows the face, then colours its icon: the image may be a new one.
    pub(super) fn show(&mut self, button: &impl Face) {
        self.show_face(button);
        self.color_icon(button);
    }

    /// Colours the image the button shows, if it has one and a colour.
    pub(super) fn color_icon(&mut self, button: &impl Face) {
        let (Some(color), Some(image)) = (self.color, shown_image(button)) else { return };
        // The new rule before the old one goes, so a shared one stays.
        let rule = set_icon_color(&image, color);
        self.color_rule = rule;
    }

    // Accessible through `gtk::Widget`: libadwaita's types don't say
    // they're accessible themselves.
    fn show_face(&self, button: &impl Face) {
        if let Some(gicon) = self.vector.clone().flatten().and_then(|v| super::vector_icon::gicon(&v)) {
            let image = gtk::Image::from_gicon(&gicon);
            if self.icon_only == Some(true) {
                button.show_image(&image);
                button.upcast_ref::<gtk::Widget>().update_property(&[gtk::accessible::Property::Label(&self.label)]);
            } else {
                // As libadwaita's `ButtonContent` lays out an icon and a
                // label, which takes only icon names.
                let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                content.set_halign(gtk::Align::Center);
                content.append(&image);
                content.append(&gtk::Label::new(Some(&self.label)));
                button.show_child(content.upcast_ref());
                button.add_css_class("image-text-button");
                button.upcast_ref::<gtk::Widget>().reset_property(gtk::AccessibleProperty::Label);
            }
            return;
        }
        button.remove_css_class("image-text-button");
        let icon = self.icon.as_deref().unwrap_or_default();
        if icon.is_empty() {
            button.show_label(&self.label);
            button.upcast_ref::<gtk::Widget>().reset_property(gtk::AccessibleProperty::Label);
        } else if self.icon_only == Some(true) {
            button.show_icon(icon);
            button.upcast_ref::<gtk::Widget>().update_property(&[gtk::accessible::Property::Label(&self.label)]);
        } else {
            let content = adw::ButtonContent::new();
            content.set_icon_name(icon);
            content.set_label(&self.label);
            button.show_child(content.upcast_ref());
            button.upcast_ref::<gtk::Widget>().reset_property(gtk::AccessibleProperty::Label);
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
            props.extend(self.shown_color(button).map(Prop::TextColor));
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
        props.extend(self.shown_color(button).map(Prop::TextColor));
        props
    }

    /// The colour the icon's image has, or the one given while it shows
    /// none (a caption alone).
    fn shown_color(&self, button: &impl Face) -> Option<Color> {
        self.color?;
        match shown_image(button) {
            Some(image) => icon_color(&image, self.color),
            None => self.color,
        }
    }
}

/// The image a button shows: alone, in our box with its caption, or in a
/// `ButtonContent`; GTK's own for an icon name.
fn shown_image(button: &impl Face) -> Option<gtk::Image> {
    let child = button.shown_child()?;
    first_image(&child)
}

fn first_image(widget: &gtk::Widget) -> Option<gtk::Image> {
    if let Some(image) = widget.downcast_ref::<gtk::Image>() {
        return Some(image.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(image) = first_image(&c) {
            return Some(image);
        }
        child = c.next_sibling();
    }
    None
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
