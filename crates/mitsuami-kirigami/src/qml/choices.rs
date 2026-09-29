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
    // A window of its own (a Wayland popup), as GTK's is: drawn in the
    // window, it went under a GPU surface's subsurface. `popupType` is Qt
    // 6.8's, and a binding to it wouldn't load before that, so it's set
    // where Qt has it; older ones keep the list in the window.
    Component.onCompleted: if (popup.popupType !== undefined) popup.popupType = QQC2.Popup.Window
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
