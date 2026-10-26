//! Controls that choose one of a set of options.

use super::{TEXT_STYLE, a11y, a11y_hover};

/// A select. Rust sets `mitsuamiOptions` and `currentIndex`, and reads the
/// options back from `mitsuamiOptionTexts` (joined with U+001F). Setting
/// `mitsuamiChoice` chooses an option as the user does: it reports it with
/// `activated`.
pub(crate) fn select() -> String {
    format!(
        r#"
QQC2.ComboBox {{
    property var mitsuamiOptions: []
    readonly property string mitsuamiOptionTexts: mitsuamiOptions.join("\u001f")
    property int mitsuamiChoice: -1
    model: mitsuamiOptions
    // The list is drawn in the window, as Qt's default is, but that puts
    // it under a GPU surface's subsurface, so in a window with a surface
    // it's a window of its own (a Wayland popup). Only there: on sway, Qt
    // 6.11's popup windows close as the pointer moves over them.
    // `popupType` is Qt 6.8's, and a binding to it wouldn't load before
    // that, so it's bound where Qt has it; older ones keep the list in
    // the window.
    readonly property bool mitsuamiOverSurface: !!(Window.window && Window.window.mitsuamiSurfaces > 0)
    Component.onCompleted: if (popup.popupType !== undefined)
        popup.popupType = Qt.binding(() => mitsuamiOverSurface ? QQC2.Popup.Window : QQC2.Popup.Item)
    onMitsuamiChoiceChanged: if (mitsuamiChoice >= 0) {{
        currentIndex = mitsuamiChoice
        mitsuamiChoice = -1
        activated(currentIndex)
    }}
    {TEXT_STYLE}
    {}
}}
"#,
        a11y("\"\"")
    )
}

/// A radio group: a `QQC2.RadioButton` for each of `mitsuamiOptions`, down
/// a column, exclusive as siblings are (`autoExclusive`). Rust sets the
/// options and `mitsuamiSelected` (-1: none), which chooses a button; a new
/// set of options makes new buttons, and chooses by it again.
/// `mitsuamiReadShown` puts the button that's on in `mitsuamiShown`.
/// Setting `mitsuamiChoice` chooses one as the user does: it reports it
/// with `mitsuamiChosen`, as a button's own `toggled` does. The group
/// hands its focus to the chosen button, or the first.
pub(crate) fn radio_group() -> String {
    format!(
        r#"
ColumnLayout {{
    id: mitsuamiRadioGroup
    property var mitsuamiOptions: []
    readonly property string mitsuamiOptionTexts: mitsuamiOptions.join("\u001f")
    property int mitsuamiSelected: -1
    property int mitsuamiShown: -1
    property int mitsuamiChoice: -1
    readonly property int mitsuamiCount: mitsuamiButtons.count
    signal mitsuamiChosen()
    // As a form's rows of radio buttons are apart.
    spacing: Kirigami.Units.smallSpacing
    Accessible.role: Accessible.Grouping
    function mitsuamiShow() {{
        for (let i = 0; i < mitsuamiButtons.count; i++) {{
            const button = mitsuamiButtons.itemAt(i)
            if (button) button.checked = i === mitsuamiSelected
        }}
    }}
    function mitsuamiReadShown() {{
        mitsuamiShown = -1
        for (let i = 0; i < mitsuamiButtons.count; i++) {{
            const button = mitsuamiButtons.itemAt(i)
            if (button && button.checked) mitsuamiShown = i
        }}
    }}
    onMitsuamiSelectedChanged: mitsuamiShow()
    onMitsuamiChoiceChanged: if (mitsuamiChoice >= 0) {{
        const button = mitsuamiButtons.itemAt(mitsuamiChoice)
        mitsuamiChoice = -1
        if (button && !button.checked) {{
            button.checked = true
            button.toggled()
        }}
    }}
    onActiveFocusChanged: if (activeFocus) {{
        let button = mitsuamiButtons.itemAt(0)
        for (let i = 0; i < mitsuamiButtons.count; i++)
            if (mitsuamiButtons.itemAt(i).checked) button = mitsuamiButtons.itemAt(i)
        if (button) button.forceActiveFocus(Qt.TabFocusReason)
    }}
    Repeater {{
        id: mitsuamiButtons
        model: mitsuamiRadioGroup.mitsuamiOptions
        onItemAdded: (index, item) => item.checked = index === mitsuamiRadioGroup.mitsuamiSelected
        QQC2.RadioButton {{
            required property int index
            required property string modelData
            text: modelData
            // `toggled` is the user's; `checkedChanged` fires for ours too.
            onToggled: if (checked) {{
                mitsuamiRadioGroup.mitsuamiSelected = index
                mitsuamiRadioGroup.mitsuamiChosen()
            }}
        }}
    }}
    {}
}}
"#,
        a11y_hover("\"\"")
    )
}

/// A colour well, as KDE's Qt Quick apps make one (KQuickControls'
/// `ColorButton`): a button showing the colour as a swatch, which opens
/// Qt Quick's `ColorDialog` (Plasma's own through its platform theme),
/// without alpha. Rust sets `mitsuamiRgb` (`0xRRGGBB`), which emits
/// nothing; a pick sets it and emits `mitsuamiPicked`, only for the user:
/// the dialog accepted, or `mitsuamiPick` set as assistive technology sets
/// a value (the dialog never opens then).
pub(crate) fn color_well() -> String {
    format!(
        r#"
import QtQuick.Dialogs
QQC2.Button {{
    id: mitsuamiWell
    property int mitsuamiRgb: 0
    property int mitsuamiPick: -1
    signal mitsuamiPicked()
    readonly property color mitsuamiSwatch: Qt.rgba(((mitsuamiRgb >> 16) & 255) / 255,
        ((mitsuamiRgb >> 8) & 255) / 255, (mitsuamiRgb & 255) / 255, 1)
    function mitsuamiRgbOf(c) {{
        return (Math.round(c.r * 255) << 16) | (Math.round(c.g * 255) << 8) | Math.round(c.b * 255)
    }}
    onMitsuamiPickChanged: if (mitsuamiPick >= 0) {{
        mitsuamiRgb = mitsuamiPick
        mitsuamiPick = -1
        mitsuamiPicked()
    }}
    contentItem: Rectangle {{
        implicitWidth: Kirigami.Units.gridUnit * 2
        implicitHeight: Kirigami.Units.gridUnit
        radius: Kirigami.Units.cornerRadius
        color: mitsuamiWell.mitsuamiSwatch
        border.width: 1
        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g,
            Kirigami.Theme.textColor.b, 0.25)
        opacity: mitsuamiWell.enabled ? 1 : 0.5
    }}
    onClicked: {{
        mitsuamiDialog.selectedColor = mitsuamiSwatch
        mitsuamiDialog.open()
    }}
    ColorDialog {{
        id: mitsuamiDialog
        onAccepted: {{
            mitsuamiWell.mitsuamiRgb = mitsuamiWell.mitsuamiRgbOf(selectedColor)
            mitsuamiWell.mitsuamiPicked()
        }}
    }}
    {}
}}
"#,
        a11y("\"\"")
    )
}
