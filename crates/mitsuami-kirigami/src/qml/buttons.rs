//! Buttons, and the controls that are checked or not.

use super::{TEXT_STYLE, a11y};

/// A button. `mitsuamiDefault` marks it as the default button, which the
/// desktop style draws from `Accessible.defaultButton` (`highlighted` only
/// draws it as focused).
/// Its icon is a themed icon's name (`mitsuamiIcon`, empty for none),
/// beside the text unless `mitsuamiIconOnly`, which keeps the text as its
/// accessible name, as Qt's `IconOnly` buttons do. `mitsuamiShown*` read
/// back what the button shows.
pub(crate) fn button() -> String {
    button_with("")
}

/// A button that opens a menu, as KDE's buttons with one do: a button
/// whose accessible role is `ButtonMenu`, which Qt's desktop style draws
/// with the style's menu arrow (Breeze's), as it draws a `QPushButton`
/// with a menu. A click (or Space, or assistive technology's press) pops
/// up `mitsuamiButtonMenu` under it, and it shows pressed while that's
/// open. The menu is a context menu's (see `ContextMenu::for_button`): in
/// the window's overlay while it's open, back in the button once closed.
pub(crate) fn menu_button() -> String {
    button_with(
        r#"
    property QtObject mitsuamiButtonMenu: null
    Accessible.role: Accessible.ButtonMenu
    down: pressed || (mitsuamiButtonMenu !== null && mitsuamiButtonMenu.visible)
    onClicked: {
        const menu = mitsuamiButtonMenu
        if (!menu || menu.visible) return
        const window = Window.window
        if (window) window.mitsuamiShownMenu = menu
        const overlay = QQC2.Overlay.overlay
        if (overlay) {
            menu.mitsuamiOwner = menu.parent
            menu.parent = overlay
        }
        const at = mapToItem(menu.parent, 0, height)
        menu.popup(at.x, at.y)
    }
"#,
    )
}

fn button_with(extra: &str) -> String {
    format!(
        r#"
QQC2.Button {{
    property bool mitsuamiDefault: false
    property string mitsuamiIcon: ""
    property bool mitsuamiIconOnly: false
    readonly property string mitsuamiShownIcon: icon.name
    readonly property bool mitsuamiShownIconOnly: display === QQC2.AbstractButton.IconOnly
    Accessible.defaultButton: mitsuamiDefault
    icon.name: mitsuamiIcon
    display: mitsuamiIconOnly && mitsuamiIcon !== "" ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
    {TEXT_STYLE}
    {}
    {extra}
}}
"#,
        a11y("text")
    )
}

/// A button that stays pressed: a checkable button, whose `toggled` is
/// the user's.
pub(crate) fn toggle_button() -> String {
    button_with("    checkable: true")
}

pub(crate) fn checkbox() -> String {
    format!("QQC2.CheckBox {{ {TEXT_STYLE} {} }}", a11y("text"))
}

pub(crate) fn switch() -> String {
    // No caption: the label is the accessible name.
    format!("QQC2.Switch {{ text: \"\"; {TEXT_STYLE} {} }}", a11y("\"\""))
}
