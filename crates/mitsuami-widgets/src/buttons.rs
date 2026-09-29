//! Push, toggle and menu buttons.

use mitsuami_core::services::{Menu, MenuEntries, install_button_menu};

use mitsuami_core::{
    ButtonRole, ButtonStyle, Element, ElementBuilder, EventValue, NodeId, Prop, Tweak, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal};

pub struct Button(Element);
widget!(Button);

impl Button {
    pub fn new(label: impl IntoValue<String>) -> Button {
        let mut element = Element::new(WidgetKind::Button);
        element.prop(label.into_value(), Prop::Label);
        Button(element)
    }

    /// What the button does in its window: see [`ButtonRole`].
    pub fn role(mut self, role: impl IntoValue<ButtonRole>) -> Button {
        self.0.prop(role.into_value(), Prop::ButtonRole);
        self
    }

    /// How the button is drawn: see [`ButtonStyle`].
    pub fn button_style(mut self, style: impl IntoValue<ButtonStyle>) -> Button {
        self.0.prop(style.into_value(), Prop::ButtonStyle);
        self
    }

    /// An icon before its caption, named in the platform's own set as for
    /// [`Icon`](crate::Icon); empty: none. Each platform places and sizes it
    /// its own way.
    pub fn icon(mut self, name: impl IntoValue<String>) -> Button {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }

    /// Shows only its icon. The caption stays its accessible name; a
    /// tooltip saying it too is up to the app, as platforms leave it.
    pub fn icon_only(mut self, only: impl IntoValue<bool>) -> Button {
        self.0.prop(only.into_value(), Prop::IconOnly);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Button>) -> Button {
        tweak.apply(&mut self.0);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Button {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Button {
        self.0.on(move |event| {
            if *event == UiEvent::Click {
                handler();
            }
        });
        self
    }
}

/// A button that stays pressed until it's clicked again, as the platform
/// makes one: an `NSButton` that pushes on and off, a `gtk::ToggleButton`,
/// a `ToggleButton`, a checkable `QQC2.Button`. Its caption, icon and style
/// are a [`Button`]'s; whether it's pressed is `checked`, as a checkbox's.
pub struct ToggleButton(Element);
widget!(ToggleButton);

impl ToggleButton {
    pub fn new(label: impl IntoValue<String>) -> ToggleButton {
        let mut element = Element::new(WidgetKind::ToggleButton);
        element.prop(label.into_value(), Prop::Label);
        ToggleButton(element)
    }

    /// How the button is drawn: see [`ButtonStyle`].
    pub fn button_style(mut self, style: impl IntoValue<ButtonStyle>) -> ToggleButton {
        self.0.prop(style.into_value(), Prop::ButtonStyle);
        self
    }

    /// An icon before its caption, as for [`Button::icon`].
    pub fn icon(mut self, name: impl IntoValue<String>) -> ToggleButton {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }

    /// Shows only its icon. The caption stays its accessible name.
    pub fn icon_only(mut self, only: impl IntoValue<bool>) -> ToggleButton {
        self.0.prop(only.into_value(), Prop::IconOnly);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<ToggleButton>) -> ToggleButton {
        tweak.apply(&mut self.0);
        self
    }
}

/// A button that opens a menu of actions, as the platform makes one: a
/// pull-down `NSPopUpButton`, a `gtk::MenuButton`, a `DropDownButton`, a
/// `QQC2.Button` that opens a `QQC2.Menu`. Each draws its own arrow. The
/// menu is built as a context menu is, and an item's `on_select` runs when
/// it's chosen. It has no click of its own: clicking opens the menu.
///
/// ```ignore
/// MenuButton::new("Add").icon(add_icon).menu((
///     MenuItem::new("Disc image…").on_select(add_image),
///     MenuItem::new("Folder…").on_select(add_folder),
///     MenuSeparator,
///     MenuItem::new("Guest tools").on_select(add_tools),
/// ))
/// ```
pub struct MenuButton(Element);
widget!(MenuButton);

impl MenuButton {
    pub fn new(label: impl IntoValue<String>) -> MenuButton {
        let mut element = Element::new(WidgetKind::MenuButton);
        element.prop(label.into_value(), Prop::Label);
        MenuButton(element)
    }

    /// Its menu: `MenuItem`s, `MenuSeparator`s and submenus (`Menu`).
    pub fn menu(mut self, entries: impl MenuEntries + 'static) -> MenuButton {
        let menu = Menu::new(String::new()).children(entries);
        self.0.after_build(move |ui, id| install_button_menu(ui, id, menu));
        self
    }

    /// Its menu, built by `entries`, and built again whenever what it reads
    /// changes: the folders above this one, say.
    ///
    /// ```ignore
    /// MenuButton::new("Recent").menu_with(move || {
    ///     recent.get().into_iter().map(|path| MenuItem::new(path.clone()).on_select(move || open(&path))).collect::<Vec<_>>()
    /// })
    /// ```
    pub fn menu_with<E: MenuEntries>(mut self, entries: impl Fn() -> E + 'static) -> MenuButton {
        let menu = Menu::new(String::new()).children_with(entries);
        self.0.after_build(move |ui, id| install_button_menu(ui, id, menu));
        self
    }

    /// An icon before its caption, as for [`Button::icon`].
    pub fn icon(mut self, name: impl IntoValue<String>) -> MenuButton {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }

    /// Shows only its icon (and the platform's arrow). The caption stays
    /// its accessible name.
    pub fn icon_only(mut self, only: impl IntoValue<bool>) -> MenuButton {
        self.0.prop(only.into_value(), Prop::IconOnly);
        self
    }

    /// How the button is drawn: see [`ButtonStyle`].
    pub fn button_style(mut self, style: impl IntoValue<ButtonStyle>) -> MenuButton {
        self.0.prop(style.into_value(), Prop::ButtonStyle);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> MenuButton {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<MenuButton>) -> MenuButton {
        tweak.apply(&mut self.0);
        self
    }
}

// A toggle button's pressed state.
toggle!(ToggleButton);

text_tag!(Button, Label);
text_tag!(ToggleButton, Label);
text_tag!(MenuButton, Label);
