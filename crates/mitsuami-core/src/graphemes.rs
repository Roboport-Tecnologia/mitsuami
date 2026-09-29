//! Text selections count grapheme clusters, what people see as one
//! character (`é` with a combining accent, a flag, a family emoji), so
//! none is ever split. Backends get Unicode scalar values (`char`s).

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// Where each grapheme of `text` starts, in `char`s, and its end.
fn boundaries(text: &str) -> Vec<usize> {
    let mut at = 0;
    let mut bounds = vec![0];
    for grapheme in text.graphemes(true) {
        at += grapheme.chars().count();
        bounds.push(at);
    }
    bounds
}

/// These graphemes of `text`, in `char`s, cut to its end.
pub fn chars_of(text: &str, graphemes: Range<usize>) -> Range<usize> {
    let bounds = boundaries(text);
    let at = |g: usize| bounds[g.min(bounds.len() - 1)];
    let end = at(graphemes.end);
    at(graphemes.start).min(end)..end
}

/// The graphemes these `char`s of `text` touch: a range that ends inside
/// a grapheme takes all of it.
pub fn graphemes_of(text: &str, chars: Range<usize>) -> Range<usize> {
    let bounds = boundaries(text);
    let start = bounds.iter().rposition(|b| *b <= chars.start).unwrap_or(0);
    let end = bounds.iter().position(|b| *b >= chars.end).unwrap_or(bounds.len() - 1);
    start..end.max(start)
}
