//! A group: WinUI has no group box, so it's what Windows 11's Settings
//! shows for a group of settings, a heading over a card. The heading is a
//! `TextBlock` in `BodyStrongTextBlockStyle`, 6 above the card, as the
//! WinUI Gallery's settings page spaces its section headings
//! (`SettingsSectionHeaderTextBlockStyle`, margin 1,30,0,6: the 30 is
//! between sections, which is the app's layout). The card is a `Border` as
//! the Community Toolkit's `SettingsCard` draws one: the card background
//! and stroke brushes, a 1 epx border, `ControlCornerRadius`, and 16 epx
//! of padding. The group node is a `Canvas` with the heading and the card
//! first, behind the core's children, which go on top.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami_core::{Insets, NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// The heading's height until one is measured: `BodyStrongTextBlockStyle`'s
/// line height.
pub(crate) const HEADING_HEIGHT: f32 = 20.0;

/// The heading's height as measured from a real one, shared by every group
/// and the metrics: `None` until one has loaded.
pub(crate) type HeadingHeight = Rc<Cell<Option<f32>>>;

/// Between the heading and the card.
const HEADING_GAP: f32 = 6.0;
/// The heading's indent, as the Gallery's.
const HEADING_INDENT: f64 = 1.0;
/// The card's border and padding, on each side.
const CARD_INSET: f32 = 1.0 + 16.0;

/// The card's look: a style of theme resources, so it follows the theme.
const CARD_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="Border">
    <Setter Property="Background" Value="{ThemeResource CardBackgroundFillColorDefaultBrush}"/>
    <Setter Property="BorderBrush" Value="{ThemeResource CardStrokeColorDefaultBrush}"/>
    <Setter Property="BorderThickness" Value="1"/>
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}"/>
</Style>"#;

/// How many of the canvas's children are the group's own, before the
/// core's.
pub(crate) const PARTS: u32 = 2;

/// Where a group's content goes, without and with a heading.
pub(crate) fn insets(heading: f32) -> (Insets, Insets) {
    let card = Insets::new(CARD_INSET, CARD_INSET, CARD_INSET, CARD_INSET);
    (card, Insets::new(heading + HEADING_GAP + CARD_INSET, CARD_INSET, CARD_INSET, CARD_INSET))
}

/// The group node's native parts.
pub(crate) struct Group {
    pub canvas: w::Canvas,
    pub heading: w::TextBlock,
    pub card: w::Border,
    /// Its frame's size, to place the card again when the heading comes
    /// or goes.
    size: Cell<(f64, f64)>,
    height: HeadingHeight,
    _loaded: EventRevoker,
}

impl Group {
    pub(crate) fn new(id: NodeId, emitter: Events, height: HeadingHeight) -> R<Group> {
        let canvas = w::Canvas::new()?;
        let heading = w::TextBlock::new()?;
        heading.cast::<w::IFrameworkElement>()?.SetStyle(&crate::backend::style("BodyStrongTextBlockStyle"))?;
        heading.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Collapsed)?;
        let card = w::Border::new()?;
        card.cast::<w::IFrameworkElement>()?.SetStyle(&w::XamlReader::Load(CARD_STYLE)?.cast::<w::Style>()?)?;
        let children = canvas.cast::<w::IPanel>()?.Children()?;
        children.Append(&heading.cast::<w::UIElement>()?)?;
        children.Append(&card.cast::<w::UIElement>()?)?;
        // The first heading in a window gives the height the core leaves
        // above every group's card, as the text scale sets it.
        let loaded = heading.cast::<w::IFrameworkElement>()?.Loaded({
            let (emitter, height) = (emitter.clone(), height.clone());
            move |sender, _| {
                let Some(heading) = sender.as_ref().and_then(|s| s.cast::<w::IUIElement>().ok()) else { return };
                _ = heading.Measure(w::Size { width: f32::INFINITY, height: f32::INFINITY });
                let measured = heading.DesiredSize().unwrap_or_default().height.ceil();
                if measured > 0.0 && height.replace(Some(measured)) != Some(measured) {
                    emitter.emit(id, UiEvent::MetricsChanged);
                }
            }
        })?;
        Ok(Group { canvas, heading, card, size: Cell::new((0.0, 0.0)), height, _loaded: loaded })
    }

    fn titled(&self) -> bool {
        self.heading.cast::<w::IUIElement>().and_then(|h| h.Visibility()).is_ok_and(|v| v == w::Visibility::Visible)
    }

    /// Its heading; empty: none, and the card fills the group.
    pub(crate) fn set_title(&self, title: &str) -> R<()> {
        self.heading.cast::<w::ITextBlock>()?.SetText(title)?;
        let visibility = if title.is_empty() { w::Visibility::Collapsed } else { w::Visibility::Visible };
        self.heading.cast::<w::IUIElement>()?.SetVisibility(visibility)?;
        let (width, height) = self.size.get();
        self.place(width, height)
    }

    pub(crate) fn title(&self) -> String {
        self.heading.cast::<w::ITextBlock>().and_then(|h| h.Text()).map(|t| t.to_string()).unwrap_or_default()
    }

    /// Places the heading at the top and the card in the rest of a frame
    /// of this size.
    pub(crate) fn place(&self, width: f64, height: f64) -> R<()> {
        self.size.set((width, height));
        let top = if self.titled() {
            let heading = self.heading.cast::<w::UIElement>()?;
            w::Canvas::SetLeft(&heading, HEADING_INDENT)?;
            w::Canvas::SetTop(&heading, 0.0)?;
            heading.cast::<w::IFrameworkElement>()?.SetWidth((width - HEADING_INDENT).max(0.0))?;
            (self.height.get().unwrap_or(HEADING_HEIGHT) + HEADING_GAP) as f64
        } else {
            0.0
        };
        let card = self.card.cast::<w::UIElement>()?;
        w::Canvas::SetLeft(&card, 0.0)?;
        w::Canvas::SetTop(&card, top)?;
        let fe = card.cast::<w::IFrameworkElement>()?;
        fe.SetWidth(width.max(0.0))?;
        fe.SetHeight((height - top).max(0.0))
    }

    /// The empty group: its heading's width, and the insets.
    pub(crate) fn strip(&self) -> w::Size {
        let (untitled, titled) = insets(self.height.get().unwrap_or(HEADING_HEIGHT));
        if !self.titled() {
            return w::Size { width: untitled.left + untitled.right, height: untitled.top + untitled.bottom };
        }
        let heading = self.heading.cast::<w::UIElement>().ok();
        let width = heading
            .map(|h| crate::backend::measure_element(&h, w::Size { width: f32::INFINITY, height: f32::INFINITY }).width)
            .unwrap_or(0.0);
        w::Size {
            width: (width + HEADING_INDENT as f32).max(titled.left + titled.right),
            height: titled.top + titled.bottom,
        }
    }
}
