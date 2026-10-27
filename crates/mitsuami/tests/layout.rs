//! Flexbox, grid, units, RTL and adaptive layout, checked against what a
//! browser computes for the equivalent CSS.

use mitsuami::core::Point;
use mitsuami::core::backend::PlatformMetrics;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn boxed(id: &str, width: impl IntoValue<Length>, height: impl IntoValue<Length>) -> Container {
    Container::new().size(width, height).test_id(id)
}

async fn frame_of(app: &TestApp, id: &str) -> Rect {
    app.settle().await;
    app.get_by_test_id(id).frame()
}

#[mitsuami_test::test]
async fn columns_stack_and_stretch_by_default(app: TestApp) {
    app.mount(|| Column::new().children((boxed("a", Length::Auto, 20), boxed("b", Length::Auto, 30))));
    assert_eq!(frame_of(&app, "a").await, Rect::new(0.0, 0.0, 800.0, 20.0));
    assert_eq!(frame_of(&app, "b").await, Rect::new(0.0, 20.0, 800.0, 30.0));
}

#[mitsuami_test::test]
async fn rows_apply_padding_and_gap(app: TestApp) {
    app.mount(|| {
        Row::new().padding(10).gap(5).align(Align::Start).children((
            boxed("a", 50, 20),
            boxed("b", 50, 20),
            boxed("c", 50, 20),
        ))
    });
    assert_eq!(frame_of(&app, "a").await, Rect::new(10.0, 10.0, 50.0, 20.0));
    assert_eq!(frame_of(&app, "b").await, Rect::new(65.0, 10.0, 50.0, 20.0));
    assert_eq!(frame_of(&app, "c").await, Rect::new(120.0, 10.0, 50.0, 20.0));
}

#[mitsuami_test::test]
async fn units_resolve_against_font_viewport_parent_and_tokens(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            boxed("em", 2.em(), 1.5.em()),
            boxed("rem", 3.rem(), 10),
            boxed("pct", 50.pct(), 10),
            boxed("viewport", 10.vw(), 10.vh()),
            boxed("vmin", 10.vmin(), 10.vmax()),
            boxed("token", Spacing::Md, Spacing::Xl),
            boxed("indent", Spacing::Indent, 10),
        ))
    });
    // Font sizes and spacing tokens come from the platform; the window is 800×600.
    let metrics = app.ui().metrics();
    let body = metrics.font_sizes.body;
    let size = |w: f32, h: f32| Size::new(w.round(), h.round());
    assert_eq!(frame_of(&app, "em").await.size, size(2.0 * body, 1.5 * body));
    assert_eq!(frame_of(&app, "rem").await.size, size(3.0 * body, 10.0));
    assert_eq!(frame_of(&app, "pct").await.size, Size::new(400.0, 10.0));
    assert_eq!(frame_of(&app, "viewport").await.size, Size::new(80.0, 60.0));
    assert_eq!(frame_of(&app, "vmin").await.size, Size::new(60.0, 80.0));
    assert_eq!(frame_of(&app, "token").await.size, size(metrics.spacing.md, metrics.spacing.xl));
    // A tree's indentation per level, as the platform's own trees indent.
    assert!(metrics.spacing.indent > 0.0);
    assert_eq!(frame_of(&app, "indent").await.size, size(metrics.spacing.indent, 10.0));
}

#[mitsuami_test::test]
async fn grid_distributes_fr_tracks_after_fixed_ones(app: TestApp) {
    app.mount(|| {
        Grid::new().width(400).columns([100.px(), 1.fr(), 2.fr()]).column_gap(0).children((
            boxed("a", Length::Auto, 20),
            boxed("b", Length::Auto, 20),
            boxed("c", Length::Auto, 20),
        ))
    });
    assert_eq!(frame_of(&app, "a").await, Rect::new(0.0, 0.0, 100.0, 20.0));
    assert_eq!(frame_of(&app, "b").await, Rect::new(100.0, 0.0, 100.0, 20.0));
    assert_eq!(frame_of(&app, "c").await, Rect::new(200.0, 0.0, 200.0, 20.0));
}

#[mitsuami_test::test]
async fn grid_items_can_span_and_be_placed(app: TestApp) {
    app.mount(|| {
        Grid::new().width(300).columns(repeat(3, 1.fr())).rows([20.px(), 20.px()]).children((
            boxed("header", Length::Auto, Length::Auto).grid_column(GridPlacement::span(3)),
            boxed("side", Length::Auto, Length::Auto).grid_column(GridPlacement::at(1)),
            boxed("main", Length::Auto, Length::Auto).grid_column(GridPlacement::at_span(2, 2)),
        ))
    });
    assert_eq!(frame_of(&app, "header").await, Rect::new(0.0, 0.0, 300.0, 20.0));
    assert_eq!(frame_of(&app, "side").await, Rect::new(0.0, 20.0, 100.0, 20.0));
    assert_eq!(frame_of(&app, "main").await, Rect::new(100.0, 20.0, 200.0, 20.0));
}

#[mitsuami_test::test]
async fn grow_fills_the_remaining_space(app: TestApp) {
    app.mount(|| Row::new().width(300).children((boxed("fixed", 100, 20), boxed("fill", Length::Auto, 20).grow(1.0))));
    assert_eq!(frame_of(&app, "fill").await, Rect::new(100.0, 0.0, 200.0, 20.0));
}

#[mitsuami_test::test]
async fn text_wraps_to_the_available_width(app: TestApp) {
    app.mount(|| {
        Row::new()
            .align(Align::Start)
            .children((Column::new().width(100).child(Text::new("hello wonderful world")), Text::new("hello")))
    });
    let wrapped = app.get_by_text("hello wonderful world").frame();
    let one_line = app.get_by_text("hello").frame();
    assert_eq!(wrapped.width(), 100.0);
    assert!(wrapped.height() >= 2.0 * one_line.height(), "{wrapped} should span several lines of {one_line}");
}

#[mitsuami_test::test(headless)]
async fn text_wraps_word_by_word_with_headless_metrics(app: TestApp) {
    app.mount(|| Column::new().width(100).child(Text::new("hello wonderful world")));
    // 8px per character: "hello" / "wonderful" / "world" on three 20px lines.
    app.expect(by_text("hello wonderful world")).to_have_frame(Rect::new(0.0, 0.0, 100.0, 60.0)).await;
}

#[mitsuami_test::test]
async fn absolute_children_are_positioned_by_inset(app: TestApp) {
    app.mount(|| Container::new().size(200, 200).child(boxed("badge", 20, 20).absolute().top(5).end(5)));
    assert_eq!(frame_of(&app, "badge").await, Rect::new(175.0, 5.0, 20.0, 20.0));
}

#[mitsuami_test::test]
async fn right_to_left_mirrors_rows_and_logical_edges(app: TestApp) {
    app.mount(|| {
        Row::new()
            .direction(TextDirection::Rtl)
            .padding_start(10)
            .align(Align::Start)
            .children((boxed("first", 50, 20), boxed("second", 50, 20)))
    });
    assert_eq!(frame_of(&app, "first").await, Rect::new(740.0, 0.0, 50.0, 20.0));
    assert_eq!(frame_of(&app, "second").await, Rect::new(690.0, 0.0, 50.0, 20.0));
}

#[mitsuami_test::test]
async fn viewport_units_follow_window_resizes(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).child(boxed("half", 50.vw(), 10.vh())));
    assert_eq!(frame_of(&app, "half").await.size, Size::new(400.0, 60.0));

    app.resize(400.0, 300.0).await;

    assert_eq!(frame_of(&app, "half").await.size, Size::new(200.0, 30.0));
}

#[mitsuami_test::test(headless)]
async fn rem_and_text_follow_the_system_text_size(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).children((boxed("rem", 10.rem(), 10), Text::new("Hi"))));
    assert_eq!(frame_of(&app, "rem").await.size.width, 160.0);

    let mut larger: PlatformMetrics = app.ui().metrics();
    larger.font_sizes.body = 20.0;
    app.headless().set_metrics(larger);
    app.settle().await;

    assert_eq!(frame_of(&app, "rem").await.size.width, 200.0);
    // 2 characters at 10px, 25px line height.
    app.expect(by_text("Hi")).to_have_frame(Rect::new(0.0, 10.0, 20.0, 25.0)).await;
}

#[mitsuami_test::test]
async fn text_styles_change_font_size(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((Text::new("Title").text_style(TextStyle::Title), Text::new("Body")))
    });
    let title = app.get_by_text("Title").frame();
    let body = app.get_by_text("Body").frame();
    assert_eq!(title.origin, Point::new(0.0, 0.0));
    assert_eq!(body.y(), title.max_y());
    assert!(title.height() > body.height(), "title {title} should be taller than body {body}");
}

#[mitsuami_test::test]
async fn hidden_nodes_take_no_space_and_leave_the_a11y_tree(app: TestApp) {
    app.mount(|| Column::new().children((Text::new("Secret").hidden(true), boxed("after", 10, 10))));
    assert_eq!(frame_of(&app, "after").await.origin, Point::new(0.0, 0.0));
    app.expect(by_text("Secret")).not_to_exist().await;
}

#[mitsuami_test::test]
async fn styles_follow_signals(app: TestApp) {
    let wide = signal(false);
    let gap = signal(0.px());
    app.mount(move || {
        Row::new()
            .align(Align::Start)
            .gap(gap)
            .children((boxed("a", move || if wide.get() { 200.px() } else { 100.px() }, 20), boxed("b", 50, 20)))
    });
    assert_eq!(frame_of(&app, "a").await.width(), 100.0);
    assert_eq!(frame_of(&app, "b").await.x(), 100.0);

    wide.set(true);
    gap.set(10.px());

    assert_eq!(frame_of(&app, "a").await.width(), 200.0);
    assert_eq!(frame_of(&app, "b").await.x(), 210.0);
}

#[mitsuami_test::test]
async fn unhiding_restores_the_original_display(app: TestApp) {
    let hidden = signal(true);
    app.mount(move || {
        Column::new().children((
            Grid::new()
                .test_id("grid")
                .columns([50.px(), 50.px()])
                .hidden(hidden)
                .children((boxed("left", Length::Auto, 20), boxed("right", Length::Auto, 20))),
            boxed("after", 10, 10),
        ))
    });
    assert_eq!(frame_of(&app, "after").await.y(), 0.0);

    hidden.set(false);

    // Still a grid: the two cells sit side by side.
    assert_eq!(frame_of(&app, "right").await, Rect::new(50.0, 0.0, 50.0, 20.0));
    assert_eq!(frame_of(&app, "after").await.y(), 20.0);
}

#[mitsuami_test::test]
async fn controls_hidden_from_the_start_have_their_size_once_shown(app: TestApp) {
    // Each platform hides what has no room, its own way (AppKit hides the
    // view, WinUI makes it clear): shown again, its controls still measure.
    let hidden = signal(true);
    app.mount(move || {
        Column::new().align(Align::Start).children((
            Column::new()
                .hidden(hidden)
                .children((Button::new("Private").test_id("button"), Checkbox::new("Reset Author").test_id("check"))),
            Checkbox::new("Amend").test_id("after"),
        ))
    });
    assert_eq!(frame_of(&app, "after").await.y(), 0.0);

    hidden.set(false);

    let button = frame_of(&app, "button").await;
    let check = frame_of(&app, "check").await;
    assert!(button.width() > 0.0 && button.height() > 0.0, "button {button}");
    assert!(check.width() > 0.0 && check.height() > 0.0, "check {check}");
    assert!(frame_of(&app, "after").await.y() >= check.max_y());
}

#[mitsuami_test::test]
async fn style_with_edits_several_fields_reactively(app: TestApp) {
    let compact = signal(false);
    app.mount(move || {
        Column::new().align(Align::Start).child(boxed("card", 100, 40).style_with(move |s| {
            let pad = if compact.get() { 4.0 } else { 16.0 };
            s.margin = mitsuami::core::style::Edges::all(pad.px());
        }))
    });
    assert_eq!(frame_of(&app, "card").await.origin, Point::new(16.0, 16.0));

    compact.set(true);

    assert_eq!(frame_of(&app, "card").await.origin, Point::new(4.0, 4.0));
}

mitsuami_test::main!();

#[mitsuami_test::test]
async fn toggles_keep_their_natural_width_in_a_stretched_grid_cell(app: TestApp) {
    app.mount(|| {
        Grid::new().width(400).columns([100.px(), 1.fr()]).children((
            Text::new("Newsletter"),
            Switch::new("Newsletter").test_id("switch"),
            Text::new("Terms"),
            Checkbox::new("Agree").test_id("checkbox"),
        ))
    });
    for id in ["switch", "checkbox"] {
        let frame = frame_of(&app, id).await;
        assert_eq!(frame.x(), 100.0, "{id} starts its cell");
        assert!(frame.width() < 300.0, "{id} is not stretched across its cell: {frame}");
    }
}

#[mitsuami_test::test]
async fn toggles_keep_their_natural_width_in_a_column(app: TestApp) {
    app.mount(|| {
        Column::new().width(400).children((
            Switch::new("Newsletter").test_id("switch"),
            Checkbox::new("Agree").test_id("checkbox"),
            boxed("stretched", Length::Auto, 10),
        ))
    });
    for id in ["switch", "checkbox"] {
        let frame = frame_of(&app, id).await;
        assert_eq!(frame.x(), 0.0);
        assert!(frame.width() < 400.0, "{id} is not stretched across the column: {frame}");
    }
    assert_eq!(frame_of(&app, "stretched").await.width(), 400.0);
}

#[mitsuami_test::test]
async fn toggles_follow_explicit_alignment(app: TestApp) {
    app.mount(|| {
        Column::new().width(400).children((
            Grid::new()
                .width(400)
                .columns([1.fr()])
                .child(Switch::new("Grid").test_id("grid").justify_self(Align::End)),
            Column::new().width(400).align(Align::Center).child(Switch::new("Column").test_id("column")),
        ))
    });
    let grid = frame_of(&app, "grid").await;
    assert_eq!(grid.x() + grid.width(), 400.0);
    let column = frame_of(&app, "column").await;
    assert_eq!(column.x(), (400.0 - column.width()) / 2.0);
}
