//! Values in a range: sliders, spin boxes, progress bars and spinners.

use super::a11y;

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

pub(crate) fn progress() -> String {
    format!("QQC2.ProgressBar {{ from: 0; to: 1; {} }}", a11y("\"\""))
}

pub(crate) fn spinner() -> String {
    format!("QQC2.BusyIndicator {{ running: false; {} }}", a11y("\"\""))
}
