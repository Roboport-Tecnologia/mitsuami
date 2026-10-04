//! Visual baselines: PNG captures compared with a perceptual diff.
//!
//! Baselines live in `<crate>/tests/visual/<backend>/<image>/`. Native
//! rendering differs between OS versions and display scales, so baselines
//! belong to the machine image that produced them: CI's pinned runners, or
//! a developer's OS (see `snapshot::image`).
//!
//! Same rules as text snapshots: missing baselines are created (not on CI),
//! mismatches fail and write `<name>.new.png` plus `<name>.diff.png` (on CI
//! too, which uploads them), `MITSUAMI_UPDATE_SNAPSHOTS=1` accepts changes,
//! and `MITSUAMI_SKIP_MACHINE_SNAPSHOTS=1` skips them all.
//!
//! Each baseline has the layout it was captured with next to it,
//! `<name>.layout.txt` (the tree snapshot's format). It is a baseline too,
//! recorded and accepted with the PNG. When pixels change, comparing it
//! with the current layout says why: the layout moved, or the platform
//! draws the same layout differently. It never fails on its own.
//!
//! The diff is pixelmatch's: colours are compared by their distance in YIQ,
//! which follows how different they look, and pixels that differ only by
//! anti-aliasing (an edge that moved by a fraction of a pixel, a glyph
//! rasterized a little differently) don't count.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use mitsuami_core::Rect;
use mitsuami_core::backend::Image;

use crate::app::TestContext;
use crate::query::Query;

/// How a capture is compared with its baseline.
///
/// ```ignore
/// app.assert_visual_snapshot_with(
///     "ticking",
///     &VisualOptions::new().ignore(by_test_id("clock")).max_changed(0.01),
/// )
/// .await;
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct VisualOptions {
    pub(crate) threshold: f64,
    pub(crate) max_changed: f64,
    pub(crate) ignore: Vec<Query>,
    pub(crate) ignore_rects: Vec<Rect>,
}

impl Default for VisualOptions {
    fn default() -> VisualOptions {
        VisualOptions { threshold: 0.1, max_changed: 0.001, ignore: Vec::new(), ignore_rects: Vec::new() }
    }
}

impl VisualOptions {
    pub fn new() -> VisualOptions {
        VisualOptions::default()
    }

    /// How different two colours must look for a pixel to count as
    /// changed, from 0 (any difference) to 1. The default is 0.1.
    pub fn threshold(mut self, threshold: f64) -> VisualOptions {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// The fraction of pixels that may change before the snapshot fails.
    /// The default is 0.001 (0.1%).
    pub fn max_changed(mut self, fraction: f64) -> VisualOptions {
        self.max_changed = fraction.clamp(0.0, 1.0);
        self
    }

    /// Leaves out the frames of the nodes `query` finds, where they are at
    /// capture time: a blinking caret's field, a clock. The query must find
    /// at least one node. The baseline's pixels are left out in the same
    /// region, so give a node whose content changes a fixed size.
    pub fn ignore(mut self, query: Query) -> VisualOptions {
        self.ignore.push(query);
        self
    }

    /// Leaves out a region, in window coordinates.
    pub fn ignore_rect(mut self, rect: Rect) -> VisualOptions {
        self.ignore_rects.push(rect);
        self
    }
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("cannot create {}: {e}", parent.display()));
    }
    let file = File::create(path).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("PNG header");
    writer.write_image_data(rgba).expect("PNG data");
}

fn read_png(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path).ok()?));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    // Baselines are written as 8-bit RGBA.
    (info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight).then_some((
        info.width,
        info.height,
        buf,
    ))
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{stem}.{suffix}.png"))
}

/// `<name>.layout.txt`, and its pending `<name>.layout.txt.new`.
fn layout_files(png: &Path) -> (PathBuf, PathBuf) {
    let stem = png.file_stem().unwrap_or_default().to_string_lossy();
    (png.with_file_name(format!("{stem}.layout.txt")), png.with_file_name(format!("{stem}.layout.txt.new")))
}

fn write_text(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

/// Why the pixels changed, from the layouts: the lines that differ, or
/// that the layout didn't change.
fn explain(expected: Option<&str>, actual: &str) -> String {
    const MAX_LINES: usize = 12;
    let Some(expected) = expected else {
        return "the baseline has no layout recorded to compare with".to_owned();
    };
    if expected == actual {
        return "the layout is the same: the platform draws it differently".to_owned();
    }
    let diff = similar::TextDiff::from_lines(expected, actual);
    let mut lines: Vec<String> = Vec::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Equal => continue,
            similar::ChangeTag::Delete => '-',
            similar::ChangeTag::Insert => '+',
        };
        lines.push(format!("{sign} {}", change.value().trim_end()));
    }
    let more = lines.len().saturating_sub(MAX_LINES);
    lines.truncate(MAX_LINES);
    if more > 0 {
        lines.push(format!("… and {more} more"));
    }
    format!("the layout changed:\n{}", lines.join("\n"))
}

/// `ignored` holds the regions to leave out, in window coordinates, and
/// `layout` is the window's tree as the tree snapshot formats it.
pub(crate) fn assert(
    context: &TestContext,
    machine: &Path,
    name: &str,
    image: &Image,
    layout: &str,
    options: &VisualOptions,
    ignored: &[Rect],
) {
    if crate::snapshot::skip_machine_snapshots() {
        return;
    }
    let file = PathBuf::from(context.manifest_dir).join("tests").join("visual").join(machine).join(format!(
        "{}@{}.png",
        context.file_prefix,
        crate::snapshot::sanitize(name)
    ));
    let pending = sibling(&file, "new");
    let diff_path = sibling(&file, "diff");
    let (layout_file, layout_pending) = layout_files(&file);
    let update = crate::snapshot::updating();
    let ci = crate::snapshot::on_ci();

    let Some((width, height, expected)) = read_png(&file) else {
        if ci && !update {
            write_png(&pending, image.width, image.height, &image.rgba);
            write_text(&layout_pending, layout);
            context.fail_snapshot(format!(
                "visual baseline {} is missing (not created on CI); the capture is in {}",
                file.display(),
                pending.display()
            ));
            return;
        }
        write_png(&file, image.width, image.height, &image.rgba);
        write_text(&layout_file, layout);
        eprintln!("mitsuami-test: created visual baseline {}", file.display());
        return;
    };
    let expected_layout = std::fs::read_to_string(&layout_file).ok();

    let _ = std::fs::remove_file(&diff_path);
    let problem = if (width, height) != (image.width, image.height) {
        Some(format!("size changed from {width}×{height} to {}×{}", image.width, image.height))
    } else if expected == image.rgba {
        // Most captures match their baseline exactly: no need to look at
        // each pixel.
        None
    } else {
        let mask = Mask::new(width, height, image.scale_factor, ignored);
        let diff = compare(&expected, &image.rgba, width, height, options.threshold, &mask);
        let considered = mask.considered();
        let ratio = if considered == 0 { 0.0 } else { diff.changed as f64 / considered as f64 };
        (ratio > options.max_changed).then(|| {
            write_png(&diff_path, width, height, &diff.image);
            format!(
                "{} pixels changed ({:.3}%, {:.3}% allowed)",
                diff.changed,
                ratio * 100.0,
                options.max_changed * 100.0
            )
        })
    };

    match problem {
        None => {
            let _ = std::fs::remove_file(&pending);
            let _ = std::fs::remove_file(&diff_path);
            // The pixels are what's compared: a layout that differs only
            // follows along when updating.
            match expected_layout {
                Some(expected) if expected == layout => {}
                Some(_) if !update => {}
                // A baseline from before layouts were recorded.
                None if ci && !update => {
                    write_text(&layout_pending, layout);
                    context.fail_snapshot(format!(
                        "visual baseline {} has no layout (not created on CI); it is in {}",
                        file.display(),
                        layout_pending.display()
                    ));
                    return;
                }
                _ => write_text(&layout_file, layout),
            }
            let _ = std::fs::remove_file(&layout_pending);
        }
        Some(_) if update => {
            write_png(&file, image.width, image.height, &image.rgba);
            write_text(&layout_file, layout);
            let _ = std::fs::remove_file(&pending);
            let _ = std::fs::remove_file(&diff_path);
            let _ = std::fs::remove_file(&layout_pending);
        }
        Some(problem) => {
            write_png(&pending, image.width, image.height, &image.rgba);
            write_text(&layout_pending, layout);
            let why = explain(expected_layout.as_deref(), layout);
            // Captures of different sizes have no diff image.
            let diff = if diff_path.exists() { format!("\ndiff: {}", diff_path.display()) } else { String::new() };
            context.fail_snapshot(format!(
                "visual baseline {} does not match: {problem}\n{why}\nnew: {}{diff}",
                file.display(),
                pending.display()
            ));
        }
    }
}

/// The pixels to leave out, from regions in window coordinates: each is
/// rounded outwards to whole physical pixels.
/// Whether two captures look the same, as a baseline is compared: within
/// the options' thresholds, leaving out the ignored regions.
pub(crate) fn alike(a: &Image, b: &Image, options: &VisualOptions, ignored: &[Rect]) -> bool {
    if (a.width, a.height) != (b.width, b.height) {
        return false;
    }
    // A window that has settled captures the same bytes twice.
    if a.rgba == b.rgba {
        return true;
    }
    let mask = Mask::new(a.width, a.height, a.scale_factor, ignored);
    let diff = compare(&a.rgba, &b.rgba, a.width, a.height, options.threshold, &mask);
    let considered = mask.considered();
    considered == 0 || diff.changed as f64 / considered as f64 <= options.max_changed
}

struct Mask {
    width: u32,
    ignored: Vec<bool>,
}

impl Mask {
    fn new(width: u32, height: u32, scale: f32, regions: &[Rect]) -> Mask {
        let mut ignored = vec![false; (width * height) as usize];
        for r in regions {
            let px = |v: f32, max: u32, round: fn(f32) -> f32| round(v * scale).clamp(0.0, max as f32) as u32;
            let (x0, x1) = (px(r.x(), width, f32::floor), px(r.x() + r.width(), width, f32::ceil));
            let (y0, y1) = (px(r.y(), height, f32::floor), px(r.y() + r.height(), height, f32::ceil));
            for y in y0..y1 {
                let row = (y * width) as usize;
                // A rect of negative width ignores nothing.
                ignored[row + x0 as usize..row + x1.max(x0) as usize].fill(true);
            }
        }
        Mask { width, ignored }
    }

    fn is_ignored(&self, x: u32, y: u32) -> bool {
        self.ignored[(y * self.width + x) as usize]
    }

    fn considered(&self) -> usize {
        self.ignored.iter().filter(|i| !**i).count()
    }
}

struct Diff {
    changed: usize,
    /// The capture faded, with changes in red, anti-aliasing in yellow and
    /// ignored regions in blue.
    image: Vec<u8>,
}

/// The largest YIQ distance between two colours (black and white).
const MAX_DELTA: f64 = 35215.0;

fn compare(expected: &[u8], actual: &[u8], width: u32, height: u32, threshold: f64, mask: &Mask) -> Diff {
    let max_delta = MAX_DELTA * threshold * threshold;
    let mut image = Vec::with_capacity(actual.len());
    let mut changed = 0;
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let colour = if mask.is_ignored(x, y) {
                Some([96, 160, 255])
            } else if colour_delta(expected, actual, i, i, false).abs() <= max_delta {
                None
            } else if antialiased(expected, actual, x, y, width, height)
                || antialiased(actual, expected, x, y, width, height)
            {
                Some([255, 200, 0])
            } else {
                changed += 1;
                Some([255, 0, 0])
            };
            match colour {
                Some(c) => image.extend_from_slice(&c),
                // Faded capture, so the changes stand out.
                None => image.extend(actual[i..i + 3].iter().map(|c| 255 - (255 - c) / 4)),
            }
            image.push(255);
        }
    }
    Diff { changed, image }
}

/// Whether the pixel at (`x`, `y`) in `a` is likely anti-aliasing: it lies
/// on an edge between a darker and a brighter neighbour, and one of those
/// has flat surroundings in both images. From pixelmatch, after "Anti-
/// aliased Pixel and Intensity Slope Detector" (Vysniauskas, 2009).
fn antialiased(a: &[u8], b: &[u8], x: u32, y: u32, width: u32, height: u32) -> bool {
    let (x0, y0) = (x.saturating_sub(1), y.saturating_sub(1));
    let (x1, y1) = ((x + 1).min(width - 1), (y + 1).min(height - 1));
    let at = |x: u32, y: u32| ((y * width + x) * 4) as usize;
    let centre = at(x, y);
    // Pixels on the image's edge have fewer neighbours: count the missing
    // ones as equal.
    let mut zeroes = usize::from(x == x0 || x == x1 || y == y0 || y == y1);
    let (mut min, mut max) = (0.0, 0.0);
    let (mut min_at, mut max_at) = ((0, 0), (0, 0));
    for ny in y0..=y1 {
        for nx in x0..=x1 {
            if (nx, ny) == (x, y) {
                continue;
            }
            // Brightness only.
            let delta = colour_delta(a, a, centre, at(nx, ny), true);
            if delta == 0.0 {
                zeroes += 1;
                // More than two equal neighbours: a flat area, not an edge.
                if zeroes > 2 {
                    return false;
                }
            } else if delta < min {
                (min, min_at) = (delta, (nx, ny));
            } else if delta > max {
                (max, max_at) = (delta, (nx, ny));
            }
        }
    }
    // Neighbours only darker or only brighter: not between two colours.
    if min == 0.0 || max == 0.0 {
        return false;
    }
    (flat_around(a, min_at, width, height) && flat_around(b, min_at, width, height))
        || (flat_around(a, max_at, width, height) && flat_around(b, max_at, width, height))
}

/// Whether at least three neighbours of (`x`, `y`) have exactly its colour.
fn flat_around(img: &[u8], (x, y): (u32, u32), width: u32, height: u32) -> bool {
    let (x0, y0) = (x.saturating_sub(1), y.saturating_sub(1));
    let (x1, y1) = ((x + 1).min(width - 1), (y + 1).min(height - 1));
    let at = |x: u32, y: u32| ((y * width + x) * 4) as usize;
    let centre = &img[at(x, y)..at(x, y) + 4];
    let mut zeroes = usize::from(x == x0 || x == x1 || y == y0 || y == y1);
    for ny in y0..=y1 {
        for nx in x0..=x1 {
            if (nx, ny) != (x, y) && img[at(nx, ny)..at(nx, ny) + 4] == *centre {
                zeroes += 1;
                if zeroes > 2 {
                    return true;
                }
            }
        }
    }
    false
}

/// The squared YIQ distance between two pixels, each blended over white.
/// Negative when the second is brighter. `y_only` compares brightness.
fn colour_delta(a: &[u8], b: &[u8], i: usize, j: usize, y_only: bool) -> f64 {
    let blend = |p: &[u8], k: usize| {
        let alpha = f64::from(p[k + 3]) / 255.0;
        let c = |v: u8| 255.0 + (f64::from(v) - 255.0) * alpha;
        (c(p[k]), c(p[k + 1]), c(p[k + 2]))
    };
    let (r1, g1, b1) = blend(a, i);
    let (r2, g2, b2) = blend(b, j);
    if (r1, g1, b1) == (r2, g2, b2) {
        return 0.0;
    }
    let luma = |r: f64, g: f64, b: f64| r * 0.298_895_31 + g * 0.586_622_47 + b * 0.114_482_23;
    let y = luma(r1, g1, b1) - luma(r2, g2, b2);
    if y_only {
        return y;
    }
    let i_ = |r: f64, g: f64, b: f64| r * 0.595_977_99 - g * 0.274_176_10 - b * 0.321_801_89;
    let q = |r: f64, g: f64, b: f64| r * 0.211_470_17 - g * 0.522_617_46 + b * 0.311_147_29;
    let i_delta = i_(r1, g1, b1) - i_(r2, g2, b2);
    let q_delta = q(r1, g1, b1) - q(r2, g2, b2);
    let delta = 0.5053 * y * y + 0.299 * i_delta * i_delta + 0.1957 * q_delta * q_delta;
    if y > 0.0 { -delta } else { delta }
}
