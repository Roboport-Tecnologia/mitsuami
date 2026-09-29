//! Text fields and areas.

use super::{TEXT_STYLE, a11y, a11y_with};

pub(crate) fn text_field() -> String {
    format!("QQC2.TextField {{ {TEXT_STYLE} {} }}", a11y("placeholderText"))
}

/// KDE's password field: a text field that echoes bullets, with a button
/// that shows the password.
pub(crate) fn password_field() -> String {
    format!("Kirigami.PasswordField {{ {TEXT_STYLE} {} }}", a11y("placeholderText"))
}

/// KDE's search field, with Kirigami's own search timing: `accepted` a
/// short pause after the text changes (`autoAccept`), on Return, and from
/// its clear button. It fires for text the backend set too, so a search is
/// reported (`mitsuamiSearched`) only after a user's edit, on Return, or
/// when the clear button emptied it; `mitsuamiShown` is the text the core
/// knows, which Rust sets with its own. The clear button's edit isn't a
/// `textEdited`, so it's reported (`mitsuamiEdited`) as the search comes.
pub(crate) fn search_field() -> String {
    format!(
        r#"
Kirigami.SearchField {{
    id: field
    property string mitsuamiShown: ""
    property bool mitsuamiPending: false
    property bool mitsuamiReturn: false
    signal mitsuamiEdited()
    signal mitsuamiSearched()
    onTextEdited: {{
        mitsuamiShown = text
        mitsuamiPending = true
        mitsuamiEdited()
    }}
    // Seen before the field takes it, and left to it. Not `onPressed`:
    // the context menu's handler is (`CONTEXT_MENU`), and it leaves these
    // keys to these handlers.
    Keys.onReturnPressed: (event) => {{
        mitsuamiReturn = true
        event.accepted = false
    }}
    Keys.onEnterPressed: (event) => {{
        mitsuamiReturn = true
        event.accepted = false
    }}
    onAccepted: {{
        const cleared = text !== mitsuamiShown
        if (cleared) {{
            mitsuamiShown = text
            mitsuamiEdited()
        }}
        const search = cleared || mitsuamiPending || mitsuamiReturn
        mitsuamiPending = false
        mitsuamiReturn = false
        if (search) mitsuamiSearched()
    }}
    {TEXT_STYLE}
    {}
}}
"#,
        a11y("placeholderText")
    )
}

/// A text area in a scroll view, as KDE apps make one: it wraps, and the
/// desktop style frames the scroll view as a field. The scroll view is the
/// node's item, with the text area's properties; `mitsuamiEdited` is a
/// user's edit, not the backend's while `mitsuamiSetting`. It's
/// `mitsuamiLines` of the text area's font tall, and a text field's width
/// on the other platforms wide.
pub(crate) fn text_area() -> String {
    format!(
        r#"
QQC2.ScrollView {{
    id: scroll
    property alias text: area.text
    property alias placeholderText: area.placeholderText
    property alias readOnly: area.readOnly
    property int mitsuamiLines: 1
    property bool mitsuamiSetting: false
    signal mitsuamiEdited()
    implicitWidth: 200
    implicitHeight: Math.ceil(topPadding + bottomPadding + area.topPadding + area.bottomPadding
        + mitsuamiLines * mitsuamiMetrics.height)
    FontMetrics {{ id: mitsuamiMetrics; font: area.font }}
    QQC2.TextArea {{
        id: area
        objectName: "mitsuamiTextArea"
        wrapMode: TextEdit.Wrap
        {TEXT_STYLE}
        Accessible.name: scroll.Accessible.name
        Accessible.description: scroll.Accessible.description
        onTextChanged: if (!scroll.mitsuamiSetting) scroll.mitsuamiEdited()
    }}
    {}
}}
"#,
        a11y_with("placeholderText", "hovered")
    )
}
