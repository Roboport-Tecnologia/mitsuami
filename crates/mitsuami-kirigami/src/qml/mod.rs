//! The QML each node kind is created from. Every snippet is compiled once.
//!
//! Controls take two properties of ours: `mitsuamiTextStyle` (see
//! [`text_style`]) and the accessibility overrides `mitsuamiA11yName`,
//! `mitsuamiA11yDescription` and `mitsuamiA11yHidden`.

use mitsuami_core::TextStyle;

/// The number a text style has in `mitsuamiTextStyle`.
pub(crate) fn text_style(style: TextStyle) -> i32 {
    match style {
        TextStyle::Body => 0,
        TextStyle::LargeTitle => 1,
        TextStyle::Title => 2,
        TextStyle::Headline => 3,
        TextStyle::Callout => 4,
        TextStyle::Caption => 5,
        TextStyle::Monospace => 6,
    }
}

/// Kirigami's type scale: titles are `Kirigami.Heading` sizes (levels 1, 2
/// and 3: 1.35, 1.2 and 1.15 × the default font), captions the small font,
/// monospace the fixed-width one. KDE has no callout size. Without Plasma's
/// platform theme the small font can be the larger one; captions are then
/// 0.8 × the default, Plasma's ratio (8 and 10 pt).
const TEXT_STYLE: &str = r#"
    property int mitsuamiTextStyle: 0
    font.family: mitsuamiTextStyle === 6 ? Kirigami.Theme.fixedWidthFont.family
        : mitsuamiTextStyle === 5 ? Kirigami.Theme.smallFont.family : Kirigami.Theme.defaultFont.family
    font.pointSize: mitsuamiTextStyle === 6 ? Kirigami.Theme.fixedWidthFont.pointSize
        : mitsuamiTextStyle === 5 ? (Kirigami.Theme.smallFont.pointSize < Kirigami.Theme.defaultFont.pointSize
            ? Kirigami.Theme.smallFont.pointSize : Kirigami.Theme.defaultFont.pointSize * 0.8)
        : Kirigami.Theme.defaultFont.pointSize * [1, 1.35, 1.2, 1.15, 1, 1, 1][mitsuamiTextStyle]
"#;

fn a11y(default_name: &str) -> String {
    format!("{}{}", a11y_with(default_name, "hovered"), context_menu_handlers("parent"))
}

/// For items that aren't controls (labels, images, container hosts), which
/// have no `hovered`: a `HoverHandler` says when the pointer is on them.
fn a11y_hover(default_name: &str) -> String {
    format!(
        "{}\n    HoverHandler {{ id: mitsuamiHover }}\n{}",
        a11y_with(default_name, "mitsuamiHover.hovered"),
        context_menu_handlers("parent")
    )
}

/// What shows an item's context menu with the pointer: a right-click, on
/// the press as KDE's menus open, and a long press on touch. `owner` is the
/// item with the menu (`mitsuamiContextMenu`), which the handlers' own
/// item is or is in. They're off while it has none, so a press goes on to
/// the items under it: a child without a menu shows its container's. A
/// right press is taken whole (`WithinBounds`), so only the innermost menu
/// shows; a long press is only watched, so it still scrolls and presses.
fn context_menu_handlers(owner: &str) -> String {
    format!(
        r#"
    TapHandler {{
        acceptedButtons: Qt.RightButton
        gesturePolicy: TapHandler.WithinBounds
        enabled: {owner}.mitsuamiContextMenu !== null
        onPressedChanged: if (pressed) {owner}.mitsuamiPopupContextMenu(parent, point.position.x, point.position.y)
    }}
    TapHandler {{
        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Stylus
        enabled: {owner}.mitsuamiContextMenu !== null
        onLongPressed: {owner}.mitsuamiPopupContextMenu(parent, point.position.x, point.position.y)
    }}
"#
    )
}

/// The accessible name, description and hiding, and the tooltip: Qt
/// Quick's attached `ToolTip`, drawn by the desktop style, shown while
/// `hovered` after the press-and-hold delay, as Kirigami apps show theirs.
/// Read as the description unless the app gave one.
fn a11y_with(default_name: &str, hovered: &str) -> String {
    format!(
        r#"
    property string mitsuamiA11yName: ""
    property string mitsuamiA11yDescription: ""
    property bool mitsuamiA11yHidden: false
    property string mitsuamiTooltip: ""
    Accessible.name: mitsuamiA11yName !== "" ? mitsuamiA11yName : {default_name}
    Accessible.description: mitsuamiA11yDescription !== "" ? mitsuamiA11yDescription : mitsuamiTooltip
    Accessible.ignored: mitsuamiA11yHidden
    QQC2.ToolTip.text: mitsuamiTooltip
    QQC2.ToolTip.visible: mitsuamiTooltip !== "" && {hovered}
    QQC2.ToolTip.delay: Qt.styleHints.mousePressAndHoldInterval
{CONTEXT_MENU}"#
    )
}

/// The context menu: a `QQC2.Menu` the backend makes from the app's (see
/// `ContextMenu` in `services.rs`), whose parent is this item, popped up at
/// a point of `item`. The Menu key and Shift+F10 show it at the item's
/// centre, as Qt's widgets do, when the item or a child without a menu has
/// the focus: keys a child doesn't take come to its parent.
///
/// A long press reaches the handlers of every item under the finger: the
/// innermost shows its menu, and the others see it open.
///
/// While it's open, and until its exit transition ends, the menu is in the
/// window's overlay: an item it chose may move its item (a list's rows
/// reset, and a row's host leaves the window between delegates), and a
/// popup whose parent changes window shows itself again in the new one.
/// It goes back to its item once closed (`mitsuamiOwner`, see
/// `context_menu_qml` in `services.rs`).
const CONTEXT_MENU: &str = r#"
    property QtObject mitsuamiContextMenu: null
    function mitsuamiIsMenuKey(event) {
        return event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))
    }
    function mitsuamiPopupContextMenu(item, x, y) {
        const menu = mitsuamiContextMenu
        const window = Window.window
        if (!menu || (window && window.mitsuamiShownMenu && window.mitsuamiShownMenu.visible)) return
        if (window) window.mitsuamiShownMenu = menu
        const overlay = QQC2.Overlay.overlay
        if (overlay) {
            menu.mitsuamiOwner = menu.parent
            menu.parent = overlay
        }
        const at = item.mapToItem(menu.parent, x, y)
        menu.popup(at.x, at.y)
    }
    Keys.onPressed: (event) => {
        if (mitsuamiContextMenu && mitsuamiIsMenuKey(event)) {
            mitsuamiPopupContextMenu(mitsuamiContextMenu.parent, width / 2, height / 2)
            event.accepted = true
        }
    }
"#;

/// The colour `mitsuamiColor` names (`mitsuamiRgba` for `RGBA_COLOR`), or
/// `undefined` without one, for the item to fall back on its own: a
/// label's colour, or an icon's.
macro_rules! color_binding {
    () => {
        r#"mitsuamiColor === 9
        ? Qt.rgba(((mitsuamiRgba >>> 24) & 255) / 255, ((mitsuamiRgba >>> 16) & 255) / 255,
            ((mitsuamiRgba >>> 8) & 255) / 255, (mitsuamiRgba & 255) / 255)
        : [Kirigami.Theme.textColor, Kirigami.Theme.disabledTextColor, Kirigami.Theme.highlightColor,
            Kirigami.Theme.separatorColor, Kirigami.Theme.viewBackgroundColor, Kirigami.Theme.backgroundColor,
            Kirigami.Theme.negativeTextColor, Kirigami.Theme.neutralTextColor,
            Kirigami.Theme.positiveTextColor][mitsuamiColor]"#
    };
}

mod buttons;
mod choices;
mod containers;
mod display;
mod inputs;
mod ranges;
mod text;
mod window;

pub(crate) use buttons::*;
pub(crate) use choices::*;
pub(crate) use containers::*;
pub(crate) use display::*;
pub(crate) use inputs::*;
pub(crate) use ranges::*;
pub(crate) use text::*;
pub(crate) use window::*;

/// The theme's values, read by the backend: fonts, spacing and colors.
pub(crate) fn theme() -> String {
    r#"
QtObject {
    property font defaultFont: Kirigami.Theme.defaultFont
    property font smallFont: Kirigami.Theme.smallFont
    property font fixedWidthFont: Kirigami.Theme.fixedWidthFont
    property real smallSpacing: Kirigami.Units.smallSpacing
    property real mediumSpacing: Kirigami.Units.mediumSpacing
    property real largeSpacing: Kirigami.Units.largeSpacing
    property real gridUnit: Kirigami.Units.gridUnit
    property real longDuration: Kirigami.Units.longDuration
    property color textColor: Kirigami.Theme.textColor
    property color disabledTextColor: Kirigami.Theme.disabledTextColor
    property color highlightColor: Kirigami.Theme.highlightColor
    property color backgroundColor: Kirigami.Theme.backgroundColor
    property color viewBackgroundColor: viewProbe.Kirigami.Theme.backgroundColor
    property color separatorColor: Kirigami.ColorUtils.linearInterpolation(
        Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, Kirigami.Theme.frameContrast)
    // The View color set (text fields, lists) is another item's theme.
    property Item viewProbe: Item {
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        Kirigami.Theme.inherit: false
    }
    // A tab bar with a tab, for how far below its top a tab view's pages
    // are (`qml::tabs`).
    property Item tabProbe: QQC2.TabBar {
        position: QQC2.TabBar.Header
        QQC2.TabButton { text: "Tab" }
    }
    // Group boxes without and with a title, for where a group's content
    // goes (`qml::group`).
    property Item groupProbe: QQC2.GroupBox {}
    property Item titledGroupProbe: QQC2.GroupBox { title: "Title" }
}
"#
    .into()
}
