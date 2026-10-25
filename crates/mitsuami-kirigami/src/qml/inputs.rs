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
///
/// With `SearchOnReturn` (`mitsuamiOnReturn`: -1 unset, 0, 1), `autoAccept`
/// is off, Kirigami's own setting: it searches on Return alone, and an
/// emptied field (by the user, or the clear button) searches at once.
///
/// Its suggestions (`mitsuamiTitles`, `mitsuamiDetails`, set by Rust) are a
/// list of `SubtitleDelegate`s in a popup under it that never takes the
/// keyboard, shown while there are some, the field has focus, and Escape
/// hasn't hidden them since they last changed (`mitsuamiDismissed`). The
/// arrows move through the list and Return picks the current one, a click
/// any one: `mitsuamiChoice`, `mitsuamiSuggestionChosen()`. Only the user's
/// picks are reported.
pub(crate) fn search_field() -> String {
    format!(
        r#"import org.kde.kirigami.delegates as KD
Kirigami.SearchField {{
    id: field
    property string mitsuamiShown: ""
    property bool mitsuamiPending: false
    property bool mitsuamiReturn: false
    property var mitsuamiTitles: []
    property var mitsuamiDetails: []
    property bool mitsuamiDismissed: false
    property int mitsuamiChoice: -1
    property int mitsuamiOnReturn: -1
    autoAccept: mitsuamiOnReturn !== 1
    signal mitsuamiEdited()
    signal mitsuamiSearched()
    signal mitsuamiSuggestionChosen()
    onMitsuamiTitlesChanged: {{
        mitsuamiDismissed = false
        mitsuamiList.currentIndex = -1
    }}
    function mitsuamiChoose(index) {{
        mitsuamiChoice = index
        mitsuamiDismissed = true
        mitsuamiSuggestionChosen()
    }}
    QQC2.Popup {{
        id: mitsuamiSuggestions
        y: field.height
        width: field.width
        focus: false
        modal: false
        closePolicy: QQC2.Popup.NoAutoClose
        visible: field.mitsuamiTitles.length > 0 && field.activeFocus && !field.mitsuamiDismissed
        contentItem: ListView {{
            id: mitsuamiList
            implicitHeight: Math.min(contentHeight, Kirigami.Units.gridUnit * 16)
            clip: true
            model: field.mitsuamiTitles.length
            delegate: KD.SubtitleDelegate {{
                required property int index
                width: ListView.view.width
                text: field.mitsuamiTitles[index] ?? ""
                subtitle: field.mitsuamiDetails[index] ?? ""
                highlighted: ListView.isCurrentItem
                focusPolicy: Qt.NoFocus
                onClicked: field.mitsuamiChoose(index)
            }}
        }}
    }}
    // The list's keys, while it shows; the field keeps the keyboard.
    Keys.onDownPressed: (event) => {{
        if (!mitsuamiSuggestions.visible) {{ event.accepted = false; return }}
        mitsuamiList.currentIndex = Math.min(mitsuamiList.currentIndex + 1, mitsuamiList.count - 1)
    }}
    Keys.onUpPressed: (event) => {{
        if (!mitsuamiSuggestions.visible) {{ event.accepted = false; return }}
        mitsuamiList.currentIndex = Math.max(mitsuamiList.currentIndex - 1, 0)
    }}
    Keys.onEscapePressed: (event) => {{
        if (!mitsuamiSuggestions.visible) {{ event.accepted = false; return }}
        mitsuamiDismissed = true
    }}
    onTextEdited: {{
        mitsuamiShown = text
        mitsuamiPending = true
        mitsuamiEdited()
        if (!autoAccept && text === "") {{
            mitsuamiPending = false
            mitsuamiSearched()
        }}
    }}
    // Only on Return: the clear button empties it without a search of
    // Kirigami's, so it's reported here; the core's own text is shown
    // already, so it isn't.
    onTextChanged: {{
        if (autoAccept || text !== "" || mitsuamiShown === "") return
        mitsuamiShown = text
        mitsuamiEdited()
        mitsuamiSearched()
    }}
    // Seen before the field takes it, and left to it. Not `onPressed`:
    // the context menu's handler is (`CONTEXT_MENU`), and it leaves these
    // keys to these handlers.
    // A suggestion the arrows reached is picked, not searched for.
    Keys.onReturnPressed: (event) => {{
        if (mitsuamiSuggestions.visible && mitsuamiList.currentIndex >= 0) {{
            mitsuamiChoose(mitsuamiList.currentIndex)
            return
        }}
        mitsuamiReturn = true
        event.accepted = false
    }}
    Keys.onEnterPressed: (event) => {{
        if (mitsuamiSuggestions.visible && mitsuamiList.currentIndex >= 0) {{
            mitsuamiChoose(mitsuamiList.currentIndex)
            return
        }}
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
    property alias mitsuamiTextStyle: area.mitsuamiTextStyle
    // `Prop::FollowsEnd`: the app's text keeps the end in view while it's
    // there (or the area was empty). Qt has no such setting; Terminal and
    // Console do it. `mitsuamiFollow` lasts till the text's height settles.
    property bool mitsuamiFollowsEnd: false
    property bool mitsuamiFollow: false
    signal mitsuamiEdited()
    function mitsuamiAtEnd() {{
        var f = contentItem
        return f.contentY - f.originY + f.height >= f.contentHeight - 1
    }}
    function mitsuamiToEnd() {{
        var f = contentItem
        f.contentY = f.originY + Math.max(0, f.contentHeight - f.height)
    }}
    function mitsuamiBeforeSet() {{
        mitsuamiFollow = mitsuamiFollowsEnd && (area.length === 0 || mitsuamiAtEnd())
    }}
    function mitsuamiAfterSet() {{
        if (mitsuamiFollow) {{
            mitsuamiToEnd()
            Qt.callLater(function() {{ if (mitsuamiFollow) mitsuamiToEnd(); mitsuamiFollow = false }})
        }}
    }}
    Connections {{
        target: scroll.contentItem
        function onContentHeightChanged() {{ if (scroll.mitsuamiFollow) scroll.mitsuamiToEnd() }}
    }}
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
