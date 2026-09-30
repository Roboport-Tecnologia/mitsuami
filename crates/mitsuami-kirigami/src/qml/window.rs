//! The window and what's in it besides content: its sidebar, and toolbar items.

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
    // How many GPU surfaces are in the window (see `qml::gpu_surface`).
    property int mitsuamiSurfaces: 0
    Shortcut {{
        sequences: [StandardKey.Cancel]
        enabled: mitsuamiWindow.mitsuamiModal && mitsuamiWindow.mitsuamiFocused
        onActivated: mitsuamiWindow.close()
    }}
    // The window's sidebar: a page ahead of the content's, as KDE's System
    // Settings has its categories. Side by side in a wide window, one at a
    // time in a narrow one, the sidebar first.
    property Item mitsuamiSidebar: null
    // How much wider the window is than its content.
    readonly property real mitsuamiSidebarWidth:
        mitsuamiSidebar && mitsuamiSidebarIn && pageStack.wideMode ? mitsuamiSidebar.width : 0
    // Whether its page is in the row: the app can take it out
    // (`mitsuamiShown`), and put it back.
    property bool mitsuamiSidebarIn: false
    function mitsuamiShowSidebar() {{
        mitsuamiSidebar.mitsuamiStack = pageStack
        if (mitsuamiSidebar.mitsuamiShown) mitsuamiAddSidebar()
    }}
    function mitsuamiAddSidebar() {{
        // `insertPage` pops the pages from its position on first, which
        // would take the content's: pushed after it, then moved ahead.
        pageStack.push(mitsuamiSidebar)
        pageStack.movePage(pageStack.depth - 1, 0)
        pageStack.currentIndex = pageStack.wideMode ? 1 : 0
        mitsuamiSidebarIn = true
    }}
    function mitsuamiApplySidebarShown() {{
        if (!mitsuamiSidebar || mitsuamiSidebar.mitsuamiShown === mitsuamiSidebarIn) return
        if (mitsuamiSidebar.mitsuamiShown) {{
            mitsuamiAddSidebar()
        }} else {{
            pageStack.removePage(mitsuamiSidebar)
            mitsuamiSidebarIn = false
        }}
    }}
    function mitsuamiHideSidebar() {{
        if (mitsuamiSidebarIn) pageStack.removePage(mitsuamiSidebar)
        mitsuamiSidebarIn = false
        mitsuamiSidebar = null
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

/// A window's sidebar: a page of `ItemDelegate`s, its sections under
/// `ListSectionHeader`s, as KDE's settings list their categories. Rust sets
/// `mitsuamiSections` (JSON: `[{"title": "…" or null, "items": [{"title":
/// "…", "icon": "…" or null}]}]`), `mitsuamiSelected` (-1: none) and
/// `mitsuamiContent`, the content's page, which is titled after the item
/// chosen. The user's choice (a click, the arrow keys) is reported with
/// `mitsuamiChosen`; setting `mitsuamiChoice` chooses as the user does.
/// In a narrow window, a choice shows the content's page.
pub(crate) fn sidebar() -> String {
    r#"
Kirigami.ScrollablePage {
    id: mitsuamiSidebar
    padding: 0
    property Item mitsuamiContent: null
    // Shown as the app wants it: the window takes the page out of its row.
    property bool mitsuamiShown: true
    // The window's page row, set by the window: the page is made apart
    // from it, where Kirigami's `applicationWindow()` isn't defined.
    property QtObject mitsuamiStack: null
    property string mitsuamiSections: "[]"
    property int mitsuamiSelected: -1
    property int mitsuamiChoice: -1
    // Set while the list follows the app, not the user.
    property bool mitsuamiFollowing: false
    signal mitsuamiChosen()
    function mitsuamiChoose(index) {
        if (index === mitsuamiSelected) return
        mitsuamiSelected = index
        mitsuamiChosen()
        const stack = mitsuamiStack
        if (stack && !stack.wideMode) stack.currentIndex = stack.depth - 1
    }
    function mitsuamiShow() {
        mitsuamiFollowing = true
        mitsuamiList.currentIndex = mitsuamiSelected
        mitsuamiFollowing = false
        if (mitsuamiContent)
            mitsuamiContent.title = mitsuamiSelected >= 0 && mitsuamiSelected < mitsuamiModel.count
                ? mitsuamiModel.get(mitsuamiSelected).title : ""
    }
    onMitsuamiChoiceChanged: if (mitsuamiChoice >= 0) {
        mitsuamiChoose(mitsuamiChoice)
        mitsuamiChoice = -1
    }
    onMitsuamiSelectedChanged: mitsuamiShow()
    onMitsuamiContentChanged: mitsuamiShow()
    onMitsuamiSectionsChanged: {
        mitsuamiFollowing = true
        mitsuamiModel.clear()
        JSON.parse(mitsuamiSections).forEach((section, index) => section.items.forEach(item =>
            mitsuamiModel.append({
                title: item.title,
                iconName: item.icon ?? "",
                section: index + "" + (section.title ?? "")
            })))
        mitsuamiFollowing = false
        mitsuamiShow()
    }
    ListModel { id: mitsuamiModel }
    ListView {
        id: mitsuamiList
        objectName: "mitsuamiSidebarList"
        model: mitsuamiModel
        keyNavigationEnabled: true
        activeFocusOnTab: true
        onCurrentIndexChanged: if (!mitsuamiSidebar.mitsuamiFollowing && currentIndex >= 0)
            mitsuamiSidebar.mitsuamiChoose(currentIndex)
        // Consecutive items of a section share its index and title. One
        // without a title is set apart by the header's line alone, and the
        // first has none. The list shows every section delegate, so it's
        // the header inside that's hidden.
        section.property: "section"
        section.delegate: Item {
            required property string section
            width: ListView.view.width
            height: header.visible ? header.implicitHeight : 0
            Kirigami.ListSectionHeader {
                id: header
                width: parent.width
                text: parent.section.slice(parent.section.indexOf("") + 1)
                visible: text !== "" || !parent.section.startsWith("0")
            }
        }
        delegate: QQC2.ItemDelegate {
            required property int index
            required property string title
            required property string iconName
            width: ListView.view.width
            text: title
            icon.name: iconName
            highlighted: ListView.isCurrentItem
            onClicked: mitsuamiSidebar.mitsuamiChoose(index)
        }
    }
}
"#
    .to_owned()
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
