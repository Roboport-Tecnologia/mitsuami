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
    format!(
        r#"
    property string mitsuamiA11yName: ""
    property string mitsuamiA11yDescription: ""
    property bool mitsuamiA11yHidden: false
    Accessible.name: mitsuamiA11yName !== "" ? mitsuamiA11yName : {default_name}
    Accessible.description: mitsuamiA11yDescription
    Accessible.ignored: mitsuamiA11yHidden
"#
    )
}

/// `drawer` is the app menu's global drawer, when there is one: it has to
/// be there from the start (see `MenuParts::install`).
pub(crate) fn window(drawer: Option<&str>) -> String {
    // One page, with no padding: its content item is the content host.
    // The page's title goes in Kirigami's toolbar above it.
    let drawer = drawer.map(|qml| format!("globalDrawer: {qml}")).unwrap_or_default();
    format!(
        r#"
Kirigami.ApplicationWindow {{
    width: 800
    height: 600
    {drawer}
    pageStack.initialPage: Kirigami.Page {{
        objectName: "mitsuamiPage"
        padding: 0
        Item {{
            objectName: "mitsuamiHost"
            anchors.fill: parent
        }}
    }}
}}
"#
    )
}

pub(crate) fn container() -> String {
    format!("Item {{ {} }}", a11y("\"\""))
}

pub(crate) fn label() -> String {
    // Word wrapping: a word longer than the line overflows rather than
    // breaking, so the longest word is the min-content width.
    format!("QQC2.Label {{ wrapMode: Text.WordWrap; verticalAlignment: Text.AlignTop {TEXT_STYLE} {} }}", a11y("text"))
}

/// A button. `mitsuamiDefault` marks it as the default button, which the
/// desktop style draws from `Accessible.defaultButton` (`highlighted` only
/// draws it as focused).
pub(crate) fn button() -> String {
    format!(
        "QQC2.Button {{ property bool mitsuamiDefault: false; Accessible.defaultButton: mitsuamiDefault {TEXT_STYLE} {} }}",
        a11y("text")
    )
}

pub(crate) fn text_field() -> String {
    format!("QQC2.TextField {{ {TEXT_STYLE} {} }}", a11y("placeholderText"))
}

/// KDE's password field: a text field that echoes bullets, with a button
/// that shows the password.
pub(crate) fn password_field() -> String {
    format!("Kirigami.PasswordField {{ {TEXT_STYLE} {} }}", a11y("placeholderText"))
}

pub(crate) fn checkbox() -> String {
    format!("QQC2.CheckBox {{ {TEXT_STYLE} {} }}", a11y("text"))
}

pub(crate) fn switch() -> String {
    // No caption: the label is the accessible name.
    format!("QQC2.Switch {{ text: \"\"; {TEXT_STYLE} {} }}", a11y("\"\""))
}

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

/// A slider. Rust sets `from`, `to`, `stepSize` (0: Qt's default) and
/// `value`, and moves it as the keyboard does, reporting it with `moved`:
/// `mitsuamiStepBy` (1 or -1) steps it, `mitsuamiMoveTo` sets it.
pub(crate) fn slider() -> String {
    format!(
        r#"
QQC2.Slider {{
    property int mitsuamiStepBy: 0
    property real mitsuamiMoveTo: NaN
    onMitsuamiStepByChanged: if (mitsuamiStepBy !== 0) {{
        if (mitsuamiStepBy > 0) increase(); else decrease()
        mitsuamiStepBy = 0
        moved()
    }}
    onMitsuamiMoveToChanged: if (!isNaN(mitsuamiMoveTo)) {{
        value = mitsuamiMoveTo
        mitsuamiMoveTo = NaN
        moved()
    }}
    {}
}}
"#,
        a11y("\"\"")
    )
}

pub(crate) fn spinner() -> String {
    format!("QQC2.BusyIndicator {{ running: false; {} }}", a11y("\"\""))
}

pub(crate) fn progress() -> String {
    format!("QQC2.ProgressBar {{ from: 0; to: 1; {} }}", a11y("\"\""))
}

/// Our content goes in the flickable's content item; the scroll bars follow
/// `mitsuamiAxes` (1 horizontal, 2 vertical, 3 both).
pub(crate) fn scroll_view() -> String {
    format!(
        r#"
QQC2.ScrollView {{
    id: scroll
    property int mitsuamiAxes: 2
    QQC2.ScrollBar.horizontal.policy: (mitsuamiAxes & 1) ? QQC2.ScrollBar.AsNeeded : QQC2.ScrollBar.AlwaysOff
    QQC2.ScrollBar.vertical.policy: (mitsuamiAxes & 2) ? QQC2.ScrollBar.AsNeeded : QQC2.ScrollBar.AlwaysOff
    Flickable {{
        objectName: "mitsuamiFlickable"
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: scroll.mitsuamiAxes === 1 ? Flickable.HorizontalFlick
            : scroll.mitsuamiAxes === 2 ? Flickable.VerticalFlick : Flickable.HorizontalAndVerticalFlick
        clip: true
    }}
    {}
}}
"#,
        a11y("\"\"")
    )
}

/// A list: a `ListView` over the row keys (as strings), in a scroll view,
/// whose delegates are Qt Quick Controls' item delegates, drawn by the
/// style with its own highlight. A delegate holds its row's host
/// (`mitsuamiHost`) once the backend puts it there, and is as high as it,
/// or the estimate until then.
///
/// The scroll view is the style's: Breeze's gives its scroll bar a column
/// of its own, so rows are narrower than the list, and draws its frame when
/// `mitsuamiFramed` is set (the list view is then on the View colours). Its
/// `background` is that frame; setting it directly works on styles from
/// before `Kirigami.StyleHints.showFramedBackground`.
///
/// Rust sets, on the list view (`mitsuamiListView`), `mitsuamiKeys`,
/// `mitsuamiSelected`, `mitsuamiMode` (0 none, 1 single, 2 multiple),
/// `mitsuamiEstimate` and `mitsuamiScrollTo` (an index), and listens to
/// `mitsuamiRowsChanged()` (delegates came or went, coalesced to once per
/// event loop turn), `mitsuamiSelectionChanged()` (the user changed the
/// selection) and `mitsuamiActivate()` (the row `mitsuamiActivated` was
/// double-clicked, or Return pressed on it).
pub(crate) fn list() -> String {
    format!(
        r#"
QQC2.ScrollView {{
    id: scroll
    property bool mitsuamiFramed: false
    QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff
    {}
    ListView {{
        id: view
        objectName: "mitsuamiListView"
        // In the list view: a scroll view with more than one child makes
        // its own flickable.
        Binding {{
            target: scroll.background
            when: scroll.background !== null
            property: "visible"
            value: scroll.mitsuamiFramed
        }}
        property var mitsuamiKeys: []
        property var mitsuamiSelected: []
        readonly property string mitsuamiSelectedKeys: mitsuamiSelected.join(",")
        property int mitsuamiMode: 0
        property real mitsuamiEstimate: 24
        property int mitsuamiScrollTo: -1
        property string mitsuamiActivated: ""
        property bool mitsuamiMuted: false
        // At the end, a list stays there as rows turn out taller than estimated.
        property bool mitsuamiAtEnd: false
        onContentYChanged: mitsuamiAtEnd = count > 0 && atYEnd
        onContentHeightChanged: if (mitsuamiAtEnd) Qt.callLater(view.positionViewAtEnd)
        signal mitsuamiRowsChanged()
        signal mitsuamiSelectionChanged()
        signal mitsuamiActivate()
        model: mitsuamiKeys
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        keyNavigationEnabled: true
        currentIndex: -1
        activeFocusOnTab: true
        function mitsuamiNotify() {{ Qt.callLater(view.mitsuamiRowsChanged) }}
        function mitsuamiShow(index) {{
            if (index >= 0) positionViewAtIndex(index, ListView.Contain)
        }}
        function mitsuamiPick(key) {{
            if (mitsuamiMode === 0) return
            mitsuamiSelected = [key]
            mitsuamiMuted = true
            currentIndex = mitsuamiKeys.indexOf(key)
            mitsuamiMuted = false
            mitsuamiSelectionChanged()
        }}
        function mitsuamiOpen(key) {{
            mitsuamiActivated = key
            mitsuamiActivate()
        }}
        onMitsuamiScrollToChanged: if (mitsuamiScrollTo >= 0) {{
            mitsuamiShow(mitsuamiScrollTo)
            mitsuamiScrollTo = -1
        }}
        // The keyboard moves the current row; it's the selection.
        onCurrentIndexChanged: if (!mitsuamiMuted && currentIndex >= 0) {{
            mitsuamiShow(currentIndex)
            mitsuamiPick(mitsuamiKeys[currentIndex])
        }}
        Keys.onPressed: (event) => {{
            if (event.key === Qt.Key_Home && count > 0) {{ currentIndex = 0; event.accepted = true }}
            else if (event.key === Qt.Key_End && count > 0) {{ currentIndex = count - 1; event.accepted = true }}
            else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && currentIndex >= 0) {{
                mitsuamiOpen(mitsuamiKeys[currentIndex])
                event.accepted = true
            }}
        }}
        delegate: QQC2.ItemDelegate {{
            required property string modelData
            property string mitsuamiKey: modelData
            property Item mitsuamiHost: null
            width: view.width
            height: mitsuamiHost ? mitsuamiHost.height : view.mitsuamiEstimate
            padding: 0
            topInset: 0
            bottomInset: 0
            // The style's inset for its own padding, which rows don't have.
            leftInset: Kirigami.Units.mediumSpacing
            rightInset: Kirigami.Units.mediumSpacing
            focusPolicy: Qt.NoFocus
            highlighted: view.mitsuamiSelected.indexOf(modelData) >= 0
            contentItem: Item {{ }}
            onClicked: view.mitsuamiPick(modelData)
            onDoubleClicked: view.mitsuamiOpen(modelData)
            Component.onCompleted: view.mitsuamiNotify()
            Component.onDestruction: view.mitsuamiNotify()
        }}
    }}
}}
"#,
        a11y("\"\"")
    )
}

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
}
"#
    .into()
}
