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
    // The window's menus. Kirigami's desktop way is the global drawer as a
    // menu, opened by the menu button in the page header and drawn in the
    // window: under a GPU surface's subsurface. A window with a surface
    // shows them in a classic menu bar instead, whose menus are windows of
    // their own, above the surface, as a select's list is (see
    // `qml::select`). As a window, the drawer's menu showed at the height
    // of an empty menu about half the time: a popup window takes its size
    // when it opens, and that menu makes its items only as it opens. The
    // bar's menus have theirs from the start. They hold the drawer's own
    // actions, so states and shortcuts stay the drawer's; the role items
    // (Settings, About, Quit) end the first menu. The drawer stops being a
    // menu, and closed and disabled, so Kirigami hides its menu button
    // (shown for a menu, or for an enabled drawer with its handle). A window
    // keeps the bar once it has had a surface: giving the drawer back its
    // rules as the window closed (its surface counted out first) looped a
    // binding in Kirigami's button, and a window's menus don't change style
    // while it's open.
    property bool mitsuamiBarWanted: false
    onMitsuamiSurfacesChanged: if (mitsuamiSurfaces > 0) mitsuamiBarWanted = true
    Binding {{
        target: mitsuamiWindow.globalDrawer; property: "isMenu"; value: false
        when: mitsuamiWindow.mitsuamiBarWanted; restoreMode: Binding.RestoreNone
    }}
    Binding {{
        target: mitsuamiWindow.globalDrawer; property: "enabled"; value: false
        when: mitsuamiWindow.mitsuamiBarWanted; restoreMode: Binding.RestoreNone
    }}
    // Kirigami's own Quit (Ctrl+Q closes the window) would take the bar's
    // Quit shortcut from the app's.
    Binding {{
        target: mitsuamiWindow.quitAction; property: "enabled"; value: false
        when: mitsuamiWindow.mitsuamiBarWanted; restoreMode: Binding.RestoreNone
    }}
    Binding {{
        target: mitsuamiWindow.globalDrawer; property: "handleVisible"; value: false
        when: mitsuamiWindow.mitsuamiBarWanted; restoreMode: Binding.RestoreNone
    }}
    // Full screen is the picture alone, as a video player's is: the bar is
    // taken down, and made again as the window leaves it. Taken down, not
    // hidden: with the bar's items hidden, their shortcuts, the actions'
    // own and the window's (below) were all on at once, and Qt runs none of
    // an ambiguous sequence. With no bar the drawer's actions have their
    // shortcuts to themselves, as with Kirigami's menu button, the way out
    // of full screen among them.
    readonly property bool mitsuamiBarShown: mitsuamiBarWanted && visibility !== Window.FullScreen
    onMitsuamiBarShownChanged: mitsuamiApplyMenuBar()
    onGlobalDrawerChanged: mitsuamiApplyMenuBar()
    // The bar's shortcuts (see `mitsuamiMakeMenuBar`), made with it.
    property var mitsuamiBarShortcuts: []
    function mitsuamiApplyMenuBar() {{
        const old = menuBar
        for (const shortcut of mitsuamiBarShortcuts) shortcut.destroy()
        mitsuamiBarShortcuts = []
        menuBar = mitsuamiBarShown && globalDrawer ? mitsuamiMakeMenuBar(globalDrawer.actions) : null
        if (old) old.destroy()
    }}
    function mitsuamiMakeMenuBar(actions) {{
        const make = (qml, parent) => Qt.createQmlObject("import QtQuick.Controls as QQC2\n" + qml, parent)
        const bar = make("QQC2.MenuBar {{ }}", mitsuamiWindow)
        const menu = title => {{
            const m = make("QQC2.Menu {{ }}", bar)
            m.title = title
            // `popupType` is Qt 6.8's: before it, the menus are in the window
            if (m.popupType !== undefined) m.popupType = QQC2.Popup.Window
            return m
        }}
        const keyed = []
        const fill = (into, list) => {{
            for (let i = 0; i < list.length; i++) {{
                const action = list[i]
                if (action.separator) into.addItem(make("QQC2.MenuSeparator {{ }}", into))
                else if (action.children && action.children.length > 0) {{
                    const sub = menu(action.text)
                    fill(sub, action.children)
                    into.addMenu(sub)
                }} else {{
                    into.addAction(action)
                    if (action.shortcut) keyed.push(action)
                }}
            }}
        }}
        const roles = []
        for (let i = 0; i < actions.length; i++) {{
            const action = actions[i]
            if (action.children && action.children.length > 0) {{
                const top = menu(action.text)
                fill(top, action.children)
                bar.addMenu(top)
            }} else roles.push(action)
        }}
        if (roles.length > 0 && bar.count > 0) {{
            const first = bar.menuAt(0)
            first.addItem(make("QQC2.MenuSeparator {{ }}", first))
            fill(first, roles)
        }}
        // An action's shortcut follows the items that show it, here items
        // of the menus' own windows, which aren't the window with focus
        // while the menus are closed: Qt left Ctrl+Q to close the window
        // instead of running Quit. So the window has a shortcut of its own
        // for each, while the action's work in an open menu.
        mitsuamiBarShortcuts = keyed.map(action => {{
            const shortcut = Qt.createQmlObject("import QtQuick\nShortcut {{ }}", mitsuamiWindow)
            shortcut.sequence = action.shortcut
            shortcut.enabled = Qt.binding(() => action.enabled)
            shortcut.activated.connect(() => action.trigger())
            return shortcut
        }})
        return bar
    }}
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
    // With the menu bar and no toolbar items, Kirigami's toolbar would show
    // only the window's title again, under the bar: it goes, as a KDE app
    // with a menu bar has no such row.
    pageStack.globalToolBar.style: mitsuamiBarWanted && mitsuamiContentPage.actions.length === 0
        ? Kirigami.ApplicationHeaderStyle.None : Kirigami.ApplicationHeaderStyle.Auto
    pageStack.initialPage: Kirigami.Page {{
        id: mitsuamiContentPage
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
