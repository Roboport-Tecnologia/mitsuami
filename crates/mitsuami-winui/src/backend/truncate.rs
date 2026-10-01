//! Labels cut off at their start or middle, which XAML's trimming can't do
//! (it only ever cuts off the end): the label shows as much of the text as
//! fits its width, around an ellipsis, as Win32's path ellipsis
//! (`DT_PATH_ELLIPSIS`, `PathCompactPath`) does for Windows' own apps. The
//! other platforms cut off a single line's start or middle, so this does
//! too.

use mitsuami_core::Truncation;
use unicode_segmentation::UnicodeSegmentation;
use windows_core::Interface;

use super::measure::measure_element;
use super::{Node, R, Widget};
use crate::bindings as w;

const ELLIPSIS: &str = "\u{2026}";

/// A label's text as the app gave it, and what it's cut to.
#[derive(Default)]
pub(super) struct LabelText {
    /// The whole text, which the label shows when it isn't cut.
    pub(super) text: String,
    /// The frame's width the core gave, which the text is fitted to.
    pub(super) width: Option<f32>,
    /// The app's accessible name, which wins over the whole text.
    pub(super) name: Option<String>,
    /// Whether the label shows less than the whole text.
    pub(super) cut: bool,
}

/// Shows a label's text: whole, cut off at its end by XAML, or cut off at
/// its start or middle to its width. Run again whenever the text, its font,
/// its limit or its width changes.
pub(super) fn show(node: &mut Node) -> R<()> {
    let Widget::Label(label) = &node.widget else { return Ok(()) };
    let block: w::ITextBlock = label.cast()?;
    let lines = block.MaxLines()?;
    // A selectable label would copy the shortened text: XAML cuts its end.
    let at = match node.truncation {
        Some(at @ (Truncation::Start | Truncation::Middle)) if lines == 1 && !block.IsTextSelectionEnabled()? => {
            Some(at)
        }
        _ => None,
    };
    let fitted = match (at, node.label.width) {
        (Some(at), Some(width)) => fit(label, &node.label.text, width, at)?,
        _ => None,
    };
    // A fitted text that still runs over by a rounding is clipped, not
    // given a second ellipsis.
    block.SetTextTrimming(match (at, lines) {
        (Some(_), _) => w::TextTrimming::Clip,
        (None, 0) => w::TextTrimming::None,
        (None, _) => w::TextTrimming::CharacterEllipsis,
    })?;
    let shown = fitted.as_deref().unwrap_or(&node.label.text);
    if block.Text()? != shown {
        block.SetText(shown)?;
    }
    node.label.cut = fitted.is_some();
    // Narrator reads the whole text, as the other platforms' labels do;
    // an empty name is the text the label shows.
    let whole = node.label.cut.then_some(node.label.text.as_str());
    w::AutomationProperties::SetName(&node.element, node.label.name.as_deref().or(whole).unwrap_or(""))?;
    Ok(())
}

/// The longest cut of the text that fits the width, in the label's font,
/// or `None` if the whole text fits. It cuts between whole characters
/// (grapheme clusters), so an emoji or an accented letter stays whole; in
/// the middle, it keeps as much of the start as of the end, the start
/// taking the odd one.
fn fit(label: &w::TextBlock, text: &str, width: f32, at: Truncation) -> R<Option<String>> {
    let probe = probe(label)?;
    let piece: w::ITextBlock = probe.cast()?;
    let element: w::UIElement = probe.cast()?;
    let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
    let fits = |text: &str| -> R<bool> {
        piece.SetText(text)?;
        Ok(measure_element(&element, infinite).width <= width)
    };
    if fits(text)? {
        return Ok(None);
    }
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let len = graphemes.len();
    let cut = |kept: usize| match at {
        Truncation::Start => format!("{ELLIPSIS}{}", graphemes[len - kept..].concat()),
        _ => format!("{}{ELLIPSIS}{}", graphemes[..kept.div_ceil(2)].concat(), graphemes[len - kept / 2..].concat()),
    };
    // The most characters kept that fit; the ellipsis alone if none do.
    let (mut fitting, mut over) = (0, len);
    while over - fitting > 1 {
        let kept = (fitting + over) / 2;
        if fits(&cut(kept))? {
            fitting = kept;
        } else {
            over = kept;
        }
    }
    Ok(Some(cut(fitting)))
}

/// A text block on one line, in the label's font, to measure pieces in.
pub(super) fn probe(label: &w::TextBlock) -> R<w::TextBlock> {
    let text: w::ITextBlock = label.cast()?;
    let probe = w::TextBlock::new()?;
    let piece: w::ITextBlock = probe.cast()?;
    piece.SetFontFamily(&text.FontFamily()?)?;
    piece.SetFontSize(text.FontSize()?)?;
    piece.SetFontWeight(text.FontWeight()?)?;
    piece.SetFontStyle(text.FontStyle()?)?;
    piece.SetCharacterSpacing(text.CharacterSpacing()?)?;
    Ok(probe)
}
