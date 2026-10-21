//! Items that only show something: images, icons, separators and GPU surfaces.

use super::a11y_hover;

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

/// A themed icon, `mitsuamiName`, or the app's own (`mitsuamiFile`, an
/// SVG file's URL, which wins), at `mitsuamiSize` (Kirigami's small
/// size, 16 at the default scale, the one KDE's buttons and menus show
/// inline, unless the app gave one). No name shows nothing and takes no
/// room; a name the theme lacks shows Kirigami's fallback icon.
/// Symbolic icons take the theme's text colour, as Kirigami colours them,
/// or the colour the app gave, bound as a label's (`LABEL_OPTIONS`):
/// `transparent`, Kirigami's default, leaves them to the theme. Icons in
/// full colour keep theirs. The app's own icon is a mask, so it's drawn
/// as a symbolic one is: in the icon's colour.
pub(crate) fn icon() -> String {
    format!(
        r#"
Kirigami.Icon {{
    property int mitsuamiColor: -1
    property int mitsuamiRgba: 0
    color: {} ?? "transparent"
    property string mitsuamiName: ""
    property string mitsuamiFile: ""
    property real mitsuamiSize: Kirigami.Units.iconSizes.small
    readonly property bool mitsuamiShowsIcon: mitsuamiName !== "" || mitsuamiFile !== ""
    readonly property string mitsuamiShownName: mitsuamiFile !== "" ? mitsuamiName : (typeof source === "string" ? source : "")
    readonly property string mitsuamiShownFile: mitsuamiFile !== "" && isMask ? mitsuamiFile : ""
    source: mitsuamiFile !== "" ? mitsuamiFile : mitsuamiName
    isMask: mitsuamiFile !== ""
    implicitWidth: mitsuamiShowsIcon ? mitsuamiSize : 0
    implicitHeight: mitsuamiShowsIcon ? mitsuamiSize : 0
    Accessible.role: Accessible.Graphic
    {}
}}
"#,
        color_binding!(),
        a11y_hover("\"\"")
    )
}

/// A file's icon: its MIME type's from the icon theme, as Dolphin shows
/// it, which the backend sets as `source` (with the generic one as
/// `fallback`), in a square of `mitsuamiSize`. The file and the thumbnail
/// flag are kept for reading back: QML has no thumbnailer (Dolphin's are
/// KIO's), so a thumbnail is never shown.
pub(crate) fn file_icon() -> String {
    format!(
        r#"
Kirigami.Icon {{
    property string mitsuamiFile: ""
    property bool mitsuamiThumbnail: false
    property real mitsuamiSize: Kirigami.Units.iconSizes.small
    implicitWidth: mitsuamiSize
    implicitHeight: mitsuamiSize
    Accessible.role: Accessible.Graphic
    {}
}}
"#,
        a11y_hover("\"\"")
    )
}

/// A 1 px line in the separator colour, which reads as a separator. It
/// has no orientation: its frame is long one way and 1 px the other.
pub(crate) fn separator() -> String {
    format!("Kirigami.Separator {{ {} }}", a11y_hover("\"\""))
}

/// What keeps a GPU surface's space; the surface is over it, and takes
/// no input. A focus scope: the input item in it (`mq_surface_input_new`)
/// takes the focus it's given. Its window counts it (`mitsuamiSurfaces`),
/// for a select's list to open above it (see `qml::select`).
pub(crate) fn gpu_surface() -> String {
    format!(
        r#"
FocusScope {{
    Accessible.role: Accessible.Graphic
    property QtObject mitsuamiCounted: null
    function mitsuamiCountIn(window) {{
        if (mitsuamiCounted) mitsuamiCounted.mitsuamiSurfaces--
        mitsuamiCounted = window && window.mitsuamiSurfaces !== undefined ? window : null
        if (mitsuamiCounted) mitsuamiCounted.mitsuamiSurfaces++
    }}
    Window.onWindowChanged: mitsuamiCountIn(Window.window)
    Component.onCompleted: mitsuamiCountIn(Window.window)
    Component.onDestruction: mitsuamiCountIn(null)
    {}
}}
"#,
        a11y_hover("\"\"")
    )
}
