//! Items that hold other nodes: plain containers, groups, tab views, scroll views and lists.

use super::{a11y_hover, a11y_with, context_menu_handlers};

pub(crate) fn container() -> String {
    format!("Item {{ {} }}", a11y_hover("\"\""))
}

/// A group: a `QQC2.GroupBox` (Breeze draws its frame and title) behind
/// the content item the core's children go in, both filling the host.
/// The box has no content of its own; the core lays the children out
/// inside its paddings (`theme.rs`, `group_insets`). One group to
/// assistive technology, the host, named by the heading.
pub(crate) fn group() -> String {
    format!(
        r#"
Item {{
    property string mitsuamiTitle: ""
    Accessible.role: Accessible.Grouping
    QQC2.GroupBox {{
        objectName: "mitsuamiGroupBox"
        anchors.fill: parent
        title: parent.mitsuamiTitle
        Accessible.ignored: true
    }}
    Item {{
        objectName: "mitsuamiGroupContent"
        anchors.fill: parent
    }}
    {}
}}
"#,
        a11y_hover("mitsuamiTitle")
    )
}

/// A tab view: a strip of tabs over the page hosts. The strip is Kirigami's
/// `NavigationTabBar` (`mitsuamiNavigation`, the default), as Kirigami apps
/// switch views and libadwaita's view switcher does, or a `QQC2.TabBar` of
/// `TabButton`s, as KDE's settings pages pair a tab bar with the pages it
/// picks; `mitsuamiStrip` is the one shown. The pages are in
/// `mitsuamiPages`, below it, each at its top-left at the size the core
/// gave it; the one shown is visible, the others hidden, as a `StackLayout`
/// hides them (one would size them itself). Rust sets `mitsuamiTitles` and
/// `mitsuamiSelected`, calls `mitsuamiShow` once pages or titles come or
/// go, and reads the titles back from `mitsuamiShownTitles` (joined with
/// U+001F). The user's choice (a click, the arrow keys) is reported with
/// `mitsuamiChosen`; setting `mitsuamiChoice` chooses as the user does.
pub(crate) fn tabs() -> String {
    format!(
        r#"
Item {{
    id: mitsuamiTabs
    property var mitsuamiTitles: []
    // Themed icons' names by tab, empty for none.
    property var mitsuamiIcons: []
    property int mitsuamiSelected: -1
    property int mitsuamiChoice: -1
    property bool mitsuamiNavigation: true
    readonly property Item mitsuamiStrip: mitsuamiNavigation ? mitsuamiNavBar : mitsuamiBar
    readonly property int mitsuamiCount: mitsuamiNavigation ? mitsuamiNavBar.count
        : mitsuamiTitles.length > 0 ? mitsuamiBar.count : 0
    readonly property string mitsuamiShownTitles: {{
        const titles = []
        if (mitsuamiNavigation) {{
            for (let i = 0; i < mitsuamiNavBar.actions.length; i++) titles.push(mitsuamiNavBar.actions[i].text)
        }} else {{
            for (let i = 0; i < mitsuamiCount; i++) titles.push(mitsuamiBar.itemAt(i).text)
        }}
        return titles.join("\u001f")
    }}
    readonly property string mitsuamiShownIcons: {{
        const icons = []
        if (mitsuamiNavigation) {{
            for (let i = 0; i < mitsuamiNavBar.actions.length; i++) icons.push(mitsuamiNavBar.actions[i].icon.name)
        }} else {{
            for (let i = 0; i < mitsuamiCount; i++) icons.push(mitsuamiBar.itemAt(i).icon.name)
        }}
        return icons.join("\u001f")
    }}
    function mitsuamiIcon(index) {{
        return index >= 0 && index < mitsuamiIcons.length ? mitsuamiIcons[index] : ""
    }}
    // The navigation bar's natural width: its buttons, all as wide as the
    // widest, as it lays them out. Its own implicit width makes room for
    // five, however many there are.
    readonly property real mitsuamiStripWidth: {{
        if (!mitsuamiNavigation) return mitsuamiBar.implicitWidth
        const buttons = mitsuamiNavBar.tabGroup.buttons
        let widest = 0
        for (let i = 0; i < buttons.length; i++) widest = Math.max(widest, buttons[i].implicitWidth)
        return widest * buttons.length + mitsuamiNavBar.leftPadding + mitsuamiNavBar.rightPadding
    }}
    signal mitsuamiChosen()
    // Sizes the strip now, for measuring: its layouts, and its buttons',
    // size themselves when they're polished, before a frame.
    function mitsuamiPolishStrip() {{
        const polish = item => {{
            for (let i = 0; i < item.children.length; i++) polish(item.children[i])
            item.ensurePolished()
        }}
        polish(mitsuamiStrip)
    }}
    function mitsuamiChoose(index) {{
        if (index !== mitsuamiSelected) {{
            mitsuamiSelected = index
            mitsuamiChosen()
        }}
        mitsuamiShow()
    }}
    // The strips and the pages follow the page chosen: a strip resets its
    // current tab when its buttons are made again. The navigation bar's
    // button is checked, not its `currentIndex` set: that triggers the
    // tab's action, which is the user's.
    function mitsuamiShow() {{
        mitsuamiBar.currentIndex = mitsuamiSelected
        const button = mitsuamiNavBar.tabGroup.buttons[mitsuamiSelected]
        if (button) button.checked = true
        else if (mitsuamiNavBar.tabGroup.checkedButton) mitsuamiNavBar.tabGroup.checkedButton.checked = false
        const pages = mitsuamiPages.children
        for (let i = 0; i < pages.length; i++) pages[i].visible = i === mitsuamiSelected
    }}
    // An action for each title, in the navigation bar; the old ones go.
    function mitsuamiMakeActions() {{
        const old = []
        for (let i = 0; i < mitsuamiNavBar.actions.length; i++) old.push(mitsuamiNavBar.actions[i])
        const made = []
        for (let i = 0; i < mitsuamiTitles.length; i++)
            made.push(mitsuamiAction.createObject(mitsuamiNavBar, {{ text: mitsuamiTitles[i], mitsuamiIndex: i }}))
        mitsuamiNavBar.actions = made
        for (const action of old) action.destroy()
    }}
    // The arrow keys pick the tab beside, as KDE's widget tab bars and
    // every other platform's tab views do; neither Qt Quick's bar nor
    // Kirigami's has keys.
    function mitsuamiStep(by) {{
        const index = mitsuamiSelected + by
        if (index < 0 || index >= mitsuamiCount) return
        mitsuamiChoose(index)
        const button = mitsuamiNavigation ? mitsuamiNavBar.tabGroup.buttons[index] : mitsuamiBar.itemAt(index)
        button.forceActiveFocus(Qt.TabFocusReason)
    }}
    onMitsuamiTitlesChanged: mitsuamiMakeActions()
    onMitsuamiSelectedChanged: mitsuamiShow()
    onMitsuamiChoiceChanged: if (mitsuamiChoice >= 0) {{
        mitsuamiChoose(mitsuamiChoice)
        mitsuamiChoice = -1
    }}
    QQC2.TabBar {{
        id: mitsuamiBar
        objectName: "mitsuamiTabBar"
        visible: !mitsuamiTabs.mitsuamiNavigation
        width: parent.width
        position: QQC2.TabBar.Header
        // Focused, the bar's selected tab takes it, as Tab focuses it.
        onActiveFocusChanged: if (activeFocus && currentItem) currentItem.forceActiveFocus(focusReason)
        // The first tab is always there, hidden without titles: the desktop
        // style's bar is as high as its first tab, and warned while a
        // repeater had yet to make one.
        MitsuamiTab {{
            text: mitsuamiTabs.mitsuamiTitles.length > 0 ? mitsuamiTabs.mitsuamiTitles[0] : ""
            visible: mitsuamiTabs.mitsuamiTitles.length > 0
        }}
        Repeater {{
            model: mitsuamiTabs.mitsuamiTitles.slice(1)
            MitsuamiTab {{
                required property string modelData
                required property int index
                text: modelData
                mitsuamiIndex: index + 1
            }}
        }}
    }}
    component MitsuamiTab: QQC2.TabButton {{
        property int mitsuamiIndex: 0
        icon.name: mitsuamiTabs.mitsuamiIcon(mitsuamiIndex)
        // `clicked` is the user's (and assistive technology's Press); the
        // bar's `currentIndexChanged` is anyone's.
        onClicked: mitsuamiTabs.mitsuamiChoose(mitsuamiIndex)
        Keys.onLeftPressed: mitsuamiTabs.mitsuamiStep(mirrored ? 1 : -1)
        Keys.onRightPressed: mitsuamiTabs.mitsuamiStep(mirrored ? -1 : 1)
    }}
    Component {{
        id: mitsuamiAction
        Kirigami.Action {{
            property int mitsuamiIndex: -1
            icon.name: mitsuamiTabs.mitsuamiIcon(mitsuamiIndex)
            checkable: true
            // `triggered` is the user's (a click, assistive technology's
            // Press); the bar's `currentIndex` is anyone's.
            onTriggered: mitsuamiTabs.mitsuamiChoose(mitsuamiIndex)
        }}
    }}
    Kirigami.NavigationTabBar {{
        id: mitsuamiNavBar
        objectName: "mitsuamiNavigationBar"
        visible: mitsuamiTabs.mitsuamiNavigation
        width: parent.width
        // Above the pages: its line below, as a Kirigami page's header.
        position: QQC2.ToolBar.Header
        // Its buttons are made after their actions: one is checked then.
        onCountChanged: mitsuamiTabs.mitsuamiShow()
        // Focused, the bar's selected tab takes it, as Tab focuses it.
        onActiveFocusChanged: if (activeFocus && tabGroup.checkedButton) tabGroup.checkedButton.forceActiveFocus(focusReason)
        // Kirigami's own, with the arrow keys.
        delegate: Kirigami.NavigationTabButton {{
            required property QtObject modelData
            parent: mitsuamiNavBar.contentItem
            action: modelData
            Layout.minimumWidth: mitsuamiNavBar.buttonWidth
            Layout.maximumWidth: mitsuamiNavBar.buttonWidth
            Layout.fillHeight: true
            Keys.onLeftPressed: mitsuamiTabs.mitsuamiStep(mirrored ? 1 : -1)
            Keys.onRightPressed: mitsuamiTabs.mitsuamiStep(mirrored ? -1 : 1)
        }}
    }}
    Item {{
        id: mitsuamiPages
        objectName: "mitsuamiPages"
        anchors.top: mitsuamiTabs.mitsuamiStrip.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
    }}
    {}
}}
"#,
        a11y_hover("\"\"")
    )
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
