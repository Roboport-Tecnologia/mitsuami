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

/// `drawer` is the window's menus' global drawer, when it has menus: it has to
/// be there from the start (see `Wiring::install` in `services.rs`).
///
/// A modal window (`mitsuamiModal`) is a dialog, and Escape asks it to
/// close, as a `QDialog`'s does: through `close()`, whose close event the
/// backend reports and vetoes. `StandardKey.Cancel` is Escape (and ⌘. on
/// macOS). Qt matches window shortcuts by `active`, which a dialog reports
/// whenever its owner or another of its owner's dialogs has the keyboard,
/// so the shortcut is on only while its window has it (`mitsuamiFocused`).
pub(crate) fn window(drawer: Option<&str>) -> String {
    // One page, with no padding: its content item is the content host.
    // The page's title goes in Kirigami's toolbar above it.
    let drawer = drawer.map(|qml| format!("globalDrawer: {qml}")).unwrap_or_default();
    format!(
        r#"
Kirigami.ApplicationWindow {{
    id: mitsuamiWindow
    // Kirigami's windows show themselves; the backend shows them after
    // their first layout, once modality is set: Qt ignores it on a
    // window already shown.
    visible: false
    width: 800
    height: 600
    property bool mitsuamiModal: false
    // Set by the backend: whether this is Qt's focus window.
    property bool mitsuamiFocused: false
    // The context menu last shown in the window (see `qml::CONTEXT_MENU`).
    property QtObject mitsuamiShownMenu: null
    Shortcut {{
        sequences: [StandardKey.Cancel]
        enabled: mitsuamiWindow.mitsuamiModal && mitsuamiWindow.mitsuamiFocused
        onActivated: mitsuamiWindow.close()
    }}
    {drawer}
    pageStack.initialPage: Kirigami.Page {{
        objectName: "mitsuamiPage"
        padding: 0
        // Toolbar items are actions of the page, which Kirigami's toolbar
        // shows at its trailing end: the backend inserts and removes
        // `mitsuamiAction` at `mitsuamiIndex`.
        property QtObject mitsuamiAction: null
        property int mitsuamiIndex: 0
        function mitsuamiInsert() {{
            const list = []
            for (let i = 0; i < actions.length; i++) list.push(actions[i])
            list.splice(mitsuamiIndex, 0, mitsuamiAction)
            actions = list
        }}
        function mitsuamiRemove() {{
            const list = []
            for (let i = 0; i < actions.length; i++)
                if (actions[i] !== mitsuamiAction) list.push(actions[i])
            actions = list
        }}
        Item {{
            objectName: "mitsuamiHost"
            anchors.fill: parent
        }}
    }}
}}
"#
    )
}

/// A toolbar item: an action the page's toolbar shows as its own item,
/// never folded into the overflow menu, which holds the node's host
/// (`mitsuamiItem`) at the size the core gave it. Hidden while empty.
pub(crate) fn toolbar_action() -> String {
    r#"
Kirigami.Action {
    id: mitsuamiAction
    property Item mitsuamiItem: null
    visible: false
    displayHint: Kirigami.DisplayHint.KeepVisible
    displayComponent: Item {
        id: mitsuamiHolder
        readonly property Item held: mitsuamiAction.mitsuamiItem
        implicitWidth: held ? held.width : 0
        implicitHeight: held ? held.height : 0
        onHeldChanged: if (held) held.parent = mitsuamiHolder
        Component.onCompleted: if (held) held.parent = mitsuamiHolder
    }
}
"#
    .to_owned()
}

pub(crate) fn container() -> String {
    format!("Item {{ {} }}", a11y_hover("\"\""))
}

pub(crate) fn label() -> String {
    // Word wrapping: a word longer than the line overflows rather than
    // breaking, so the longest word is the min-content width.
    format!(
        "QQC2.Label {{ wrapMode: Text.WordWrap; verticalAlignment: Text.AlignTop {TEXT_STYLE} {} }}",
        a11y_hover("text")
    )
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

/// A spin box for whole numbers (Qt's holds an `int`). Rust sets `from`,
/// `to`, `stepSize` and `value`, and changes it as assistive technology
/// does, reporting it with `valueModified`, which Qt only emits for the
/// user: `mitsuamiStepBy` (1 or -1) steps it with Qt's own `increase()` and
/// `decrease()`, which stop at the ends; `mitsuamiMoveTo` sets it, clamped.
pub(crate) fn number_input() -> String {
    format!(
        r#"
QQC2.SpinBox {{
    editable: true
    property int mitsuamiStepBy: 0
    property real mitsuamiMoveTo: NaN
    onMitsuamiStepByChanged: if (mitsuamiStepBy !== 0) {{
        if (mitsuamiStepBy > 0) increase(); else decrease()
        mitsuamiStepBy = 0
        valueModified()
    }}
    onMitsuamiMoveToChanged: if (!isNaN(mitsuamiMoveTo)) {{
        value = Math.round(mitsuamiMoveTo)
        mitsuamiMoveTo = NaN
        valueModified()
    }}
    {}
}}
"#,
        a11y("\"\"")
    )
}

/// An image, from a file or from the pixels provider. Loaded as it's set,
/// not in the background, so it's measured at its size; QML's cache is off,
/// since every new set of pixels has a url of its own.
pub(crate) fn image() -> String {
    format!(
        r#"
Image {{
    asynchronous: false
    cache: false
    Accessible.role: Accessible.Graphic
    {}
}}
"#,
        a11y_hover("\"\"")
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
    // Without scroll bars, the flickable still scrolls by wheel and touch.
    property bool mitsuamiBars: true
    QQC2.ScrollBar.horizontal.policy: mitsuamiBars && (mitsuamiAxes & 1) ? QQC2.ScrollBar.AsNeeded : QQC2.ScrollBar.AlwaysOff
    QQC2.ScrollBar.vertical.policy: mitsuamiBars && (mitsuamiAxes & 2) ? QQC2.ScrollBar.AsNeeded : QQC2.ScrollBar.AlwaysOff
    Flickable {{
        objectName: "mitsuamiFlickable"
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: scroll.mitsuamiAxes === 1 ? Flickable.HorizontalFlick
            : scroll.mitsuamiAxes === 2 ? Flickable.VerticalFlick : Flickable.HorizontalAndVerticalFlick
        clip: true
        // On the flickable, where the scroll view puts its children's:
        // under the content, which shows its own menus first.
        {}
    }}
    {}
}}
"#,
        context_menu_handlers("scroll"),
        a11y_with("\"\"", "hovered")
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
        // its own flickable. Rows without a menu show the list's.
        {}
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
        // Multiple selection is KDE's (Dolphin's, Qt's extended selection):
        // Ctrl-click toggles a row, Shift-click or Shift and the arrows
        // select the rows from the anchor, and Ctrl+A selects them all.
        property string mitsuamiAnchor: ""
        property int mitsuamiKeyModifiers: 0
        function mitsuamiPick(key, modifiers) {{
            if (mitsuamiMode === 0) return
            const anchor = mitsuamiKeys.indexOf(mitsuamiAnchor)
            if (mitsuamiMode === 2 && (modifiers & Qt.ShiftModifier) && anchor >= 0) {{
                const index = mitsuamiKeys.indexOf(key)
                mitsuamiSelected = mitsuamiKeys.slice(Math.min(anchor, index), Math.max(anchor, index) + 1)
            }} else if (mitsuamiMode === 2 && (modifiers & Qt.ControlModifier)) {{
                const on = mitsuamiSelected.indexOf(key) < 0
                mitsuamiSelected = mitsuamiKeys.filter(k => k === key ? on : mitsuamiSelected.indexOf(k) >= 0)
                mitsuamiAnchor = key
            }} else {{
                mitsuamiSelected = [key]
                mitsuamiAnchor = key
            }}
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
            mitsuamiPick(mitsuamiKeys[currentIndex], mitsuamiKeyModifiers)
        }}
        // Before the view's own handling, which moves the current row.
        Keys.onPressed: (event) => {{
            mitsuamiKeyModifiers = event.modifiers & Qt.ShiftModifier
            if (event.matches(StandardKey.SelectAll) && mitsuamiMode === 2) {{
                mitsuamiSelected = mitsuamiKeys.slice()
                mitsuamiSelectionChanged()
                event.accepted = true
            }}
            else if (event.key === Qt.Key_Home && count > 0) {{ currentIndex = 0; event.accepted = true }}
            else if (event.key === Qt.Key_End && count > 0) {{ currentIndex = count - 1; event.accepted = true }}
            else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && currentIndex >= 0) {{
                mitsuamiOpen(mitsuamiKeys[currentIndex])
                event.accepted = true
            }}
            // The Menu key shows the current row's menu, as in Dolphin.
            else if (scroll.mitsuamiIsMenuKey(event) && currentItem && currentItem.mitsuamiHost
                     && currentItem.mitsuamiHost.mitsuamiContextMenu) {{
                const host = currentItem.mitsuamiHost
                host.mitsuamiPopupContextMenu(host, host.width / 2, host.height / 2)
                event.accepted = true
            }}
        }}
        delegate: QQC2.ItemDelegate {{
            required property string modelData
            property string mitsuamiKey: modelData
            property Item mitsuamiHost: null
            width: view.width
            height: mitsuamiHost ? mitsuamiHost.height : view.mitsuamiEstimate
            // The sides' padding and insets stay the style's: Breeze draws
            // the highlight from both, and rows don't use the padding.
            topInset: 0
            bottomInset: 0
            focusPolicy: Qt.NoFocus
            highlighted: view.mitsuamiSelected.indexOf(modelData) >= 0
            contentItem: Item {{ }}
            // `clicked` doesn't say which modifiers were held.
            TapHandler {{
                onTapped: view.mitsuamiPick(modelData, point.modifiers)
            }}
            onDoubleClicked: view.mitsuamiOpen(modelData)
            Component.onCompleted: view.mitsuamiNotify()
            Component.onDestruction: view.mitsuamiNotify()
        }}
    }}
}}
"#,
        a11y_with("\"\"", "hovered"),
        context_menu_handlers("scroll")
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
