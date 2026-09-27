//! Stable, human-readable renderings for snapshots and failure messages.

use std::fmt::Write;

use mitsuami_core::draw::{DrawOp, PathElement};
use mitsuami_core::geometry::Num;
use mitsuami_core::{A11yNode, Color, Command, DisplayList, NodeInfo, Point, Prop, Shape, WidgetKind};

fn describe_props(props: &[Prop]) -> String {
    let mut quoted = None;
    let mut extra = Vec::new();
    let mut sorted: Vec<&Prop> = props.iter().collect();
    sorted.sort_by_key(|p| format!("{p:?}"));
    for prop in sorted {
        match prop {
            Prop::Title(s) | Prop::Text(s) | Prop::Label(s) => quoted = Some(format!("{s:?}")),
            Prop::Value(s) => extra.push(format!("value={s:?}")),
            Prop::Placeholder(s) => extra.push(format!("placeholder={s:?}")),
            Prop::Options(options) => extra.push(format!("options={options:?}")),
            Prop::SelectedIndex(index) => {
                extra.push(format!("selected={}", index.map_or("none".to_owned(), |i| i.to_string())))
            }
            Prop::Checked(b) => extra.push(format!("checked={b}")),
            Prop::Enabled(b) => extra.push(format!("enabled={b}")),
            Prop::TextStyle(s) => extra.push(format!("style={s:?}")),
            Prop::Variant(v) => extra.push(format!("variant={v:?}")),
            Prop::ScrollAxes(a) => extra.push(format!("scroll={a:?}")),
            // A list's data can be long: its size is enough.
            Prop::Rows(rows) => extra.push(format!("rows={}", rows.len())),
            Prop::EstimatedRowHeight(h) => extra.push(format!("estimated_row_height={}", Num(*h))),
            Prop::Row(key) => extra.push(format!("row={}", key.0)),
            Prop::SelectionMode(mode) => extra.push(format!("selection={mode:?}")),
            Prop::Selected(rows) => {
                let rows: Vec<String> = rows.iter().map(|r| r.0.to_string()).collect();
                extra.push(format!("selected=[{}]", rows.join(" ")));
            }
            Prop::Custom(c) => extra.push(format!("{c:?}")),
            Prop::Drawing(d) => extra.push(format!("drawing={}ops", d.ops().len())),
            Prop::Native(n) => extra.push(format!("{n:?}")),
        }
    }
    let mut out = String::new();
    if let Some(q) = quoted {
        out.push(' ');
        out.push_str(&q);
    }
    for e in extra {
        out.push(' ');
        out.push_str(&e);
    }
    out
}

/// ```text
/// Window "mitsuami test" [0,0 800×600]
///   Container [0,0 800×600]
///     Text "Count: 0" [0,0 64×20]
/// ```
pub(crate) fn tree(root: &NodeInfo) -> String {
    fn walk(node: &NodeInfo, depth: usize, out: &mut String) {
        let test_id = node.test_id.as_ref().map(|t| format!(" #{t}")).unwrap_or_default();
        let _ = writeln!(
            out,
            "{}{}{}{} [{}]",
            "  ".repeat(depth),
            node.kind.name(),
            describe_props(&node.props),
            test_id,
            node.frame
        );
        for child in &node.children {
            walk(child, depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(root, 0, &mut out);
    out
}

pub(crate) fn a11y(root: &A11yNode) -> String {
    fn walk(node: &A11yNode, depth: usize, out: &mut String) {
        let _ = write!(out, "{}{:?}", "  ".repeat(depth), node.role);
        if let Some(name) = &node.name {
            let _ = write!(out, " {name:?}");
        }
        if let Some(value) = &node.value {
            let _ = write!(out, " value={value:?}");
        }
        if let Some(checked) = node.checked {
            let _ = write!(out, " checked={checked}");
        }
        if node.selected == Some(true) {
            out.push_str(" selected");
        }
        if !node.enabled {
            out.push_str(" disabled");
        }
        if let Some(description) = &node.description {
            let _ = write!(out, " description={description:?}");
        }
        out.push('\n');
        for child in &node.children {
            walk(child, depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(root, 0, &mut out);
    out
}

pub(crate) fn commands(log: &[Command]) -> String {
    let mut out = String::new();
    for command in log {
        let line = match command {
            Command::Create { id, kind, props } => format!("create {id} {}{}", kind.name(), describe_props(props)),
            Command::SetProp { id, prop } => format!("set {id}{}", describe_props(std::slice::from_ref(prop))),
            Command::Insert { parent, child, index } => format!("insert {child} into {parent} at {index}"),
            Command::Remove { parent, child } => format!("remove {child} from {parent}"),
            Command::Destroy { id } => format!("destroy {id}"),
            Command::SetFrame { id, frame } => format!("frame {id} [{frame}]"),
            Command::SetA11y { id, a11y } => format!("a11y {id} {a11y:?}"),
            Command::SetWindowSize { id, size } => {
                format!("window size {id} {}×{}", Num(size.width), Num(size.height))
            }
            Command::SetFocusOrder { window, order } => {
                let order: Vec<String> = order.iter().map(|id| id.to_string()).collect();
                format!("focus order {window} [{}]", order.join(" "))
            }
            Command::ScrollTo { id, offset } => format!("scroll {id} to {},{}", Num(offset.x), Num(offset.y)),
            Command::Focus { id } => format!("focus {id}"),
            Command::ScrollToRow { id, row } => format!("scroll {id} to row {}", row.0),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// A drawn widget's display list, as SVG shapes. Semantic colors get
/// fixed stand-ins, so wireframes don't depend on the appearance.
fn draw(drawing: &DisplayList, origin: Point, out: &mut String) {
    fn color(color: Color) -> String {
        match color {
            Color::Label => "#222222".into(),
            Color::SecondaryLabel => "#777777".into(),
            Color::Accent => "#2f6fdf".into(),
            Color::Separator => "#cccccc".into(),
            Color::ControlBackground | Color::WindowBackground => "#f4f4f4".into(),
            Color::Rgba(r, g, b, a) => format!("rgba({r},{g},{b},{})", Num(a as f32 / 255.0)),
        }
    }
    fn shape(shape: &Shape, paint: &str) -> String {
        match shape {
            Shape::Rect(r) => format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" {paint}/>"#,
                Num(r.x()),
                Num(r.y()),
                Num(r.width()),
                Num(r.height())
            ),
            Shape::RoundedRect(r, radius) => format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" {paint}/>"#,
                Num(r.x()),
                Num(r.y()),
                Num(r.width()),
                Num(r.height()),
                Num(*radius)
            ),
            Shape::Ellipse(r) => format!(
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {paint}/>"#,
                Num(r.x() + r.width() / 2.0),
                Num(r.y() + r.height() / 2.0),
                Num(r.width() / 2.0),
                Num(r.height() / 2.0)
            ),
            Shape::Path(path) => {
                let mut d = String::new();
                for element in path.elements() {
                    let _ = match element {
                        PathElement::MoveTo(p) => write!(d, "M{} {} ", Num(p.x), Num(p.y)),
                        PathElement::LineTo(p) => write!(d, "L{} {} ", Num(p.x), Num(p.y)),
                        PathElement::CurveTo { c1, c2, to } => write!(
                            d,
                            "C{} {} {} {} {} {} ",
                            Num(c1.x),
                            Num(c1.y),
                            Num(c2.x),
                            Num(c2.y),
                            Num(to.x),
                            Num(to.y)
                        ),
                        PathElement::Close => write!(d, "Z "),
                    };
                }
                format!(r#"<path d="{}" {paint}/>"#, d.trim_end())
            }
        }
    }
    let _ = writeln!(out, r#"  <g transform="translate({} {})">"#, Num(origin.x), Num(origin.y));
    for op in drawing.ops() {
        let element = match op {
            DrawOp::Fill { shape: s, color: c } => shape(s, &format!(r#"fill="{}""#, color(*c))),
            DrawOp::Stroke { shape: s, color: c, width } => {
                shape(s, &format!(r#"fill="none" stroke="{}" stroke-width="{}""#, color(*c), Num(*width)))
            }
        };
        let _ = writeln!(out, "    {element}");
    }
    let _ = writeln!(out, "  </g>");
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A deterministic SVG of the layout: one outlined box per native node,
/// labelled with its kind and text. Platform-independent, so it diffs
/// cleanly in review.
pub(crate) fn wireframe(root: &NodeInfo) -> String {
    fn color(kind: WidgetKind) -> &'static str {
        match kind {
            WidgetKind::Window => "#8a8f98",
            WidgetKind::Container | WidgetKind::Fragment => "#b5bac2",
            WidgetKind::ScrollView | WidgetKind::List => "#5f7fa0",
            WidgetKind::Text => "#3f7f5f",
            WidgetKind::Button => "#2f6fdf",
            WidgetKind::TextInput => "#a0602a",
            WidgetKind::Checkbox | WidgetKind::Switch => "#8a4fbf",
            WidgetKind::Select => "#1f8a8a",
            WidgetKind::Custom(_) | WidgetKind::Native => "#c0392b",
        }
    }
    fn walk(node: &NodeInfo, out: &mut String) {
        let f = node.frame;
        let dashed = if node.kind == WidgetKind::Container { r#" stroke-dasharray="4 3""# } else { "" };
        let _ = writeln!(
            out,
            r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}"{dashed}/>"#,
            Num(f.x()),
            Num(f.y()),
            Num(f.width()),
            Num(f.height()),
            color(node.kind)
        );
        if node.kind != WidgetKind::Container && node.kind != WidgetKind::Window {
            let label = format!("{}{}", node.kind.name(), describe_props(&node.props));
            let _ = writeln!(
                out,
                r#"  <text x="{}" y="{}" fill="{}">{}</text>"#,
                Num(f.x() + 2.0),
                Num(f.y() + f.height().min(12.0) - 2.0),
                color(node.kind),
                escape(&label)
            );
        }
        if let Some(drawing) = mitsuami_core::find_prop!(node.props, Drawing) {
            draw(&drawing, f.origin, out);
        }
        for child in &node.children {
            walk(child, out);
        }
    }
    let size = root.frame.size;
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="monospace" font-size="9">"#,
        w = Num(size.width),
        h = Num(size.height)
    );
    let _ = writeln!(out, r#"  <rect width="100%" height="100%" fill="white"/>"#);
    walk(root, &mut out);
    out.push_str("</svg>\n");
    out
}
