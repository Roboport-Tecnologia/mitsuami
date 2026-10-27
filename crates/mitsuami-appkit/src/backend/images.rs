//! Images: symbols by name, images from sources, files' icons, and where
//! buttons put them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use block2::RcBlock;
use mitsuami_core::{Color, FileLookup, ImageSource, PathStep, VectorIcon};
use objc2::rc::{Retained, Weak};
use objc2::runtime::Bool;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSBezierPath, NSBitmapFormat, NSBitmapImageRep, NSCellImagePosition, NSColor, NSColorSpace, NSDeviceRGBColorSpace,
    NSFontWeightRegular, NSImage, NSImageSymbolConfiguration, NSImageView, NSScreen, NSWindingRule, NSWorkspace,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSArray, NSError, NSPoint, NSRect, NSSize, NSURL};
use objc2_quick_look_thumbnailing::{
    QLThumbnailGenerationRequest, QLThumbnailGenerationRequestRepresentationTypes, QLThumbnailGenerator,
    QLThumbnailRepresentation,
};
use objc2_uniform_type_identifiers::{UTType, UTTypeData, UTTypeFolder};

use super::ns;

/// An SF Symbol, or else an image AppKit has by that name (its named
/// images, the app bundle's); at `points` as a font's size, if given.
pub(super) fn symbol(name: &str, points: Option<f32>) -> Option<Retained<NSImage>> {
    if name.is_empty() {
        return None;
    }
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns(name), None)
        .or_else(|| NSImage::imageNamed(&ns(name)))?;
    match points {
        Some(points) => {
            let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(points as f64, unsafe {
                NSFontWeightRegular
            });
            image.imageWithSymbolConfiguration(&config).or(Some(image))
        }
        None => Some(image),
    }
}

/// The app's own icon, `side` points square, as a template image: AppKit
/// tints it as it tints symbols (an image view's grey, a button's title's
/// colour, `contentTintColor`). In a colour, it's an image in that colour,
/// resolved as it's drawn, so a semantic one follows the appearance. It's
/// drawn at each scale it's shown at.
pub(super) fn vector_image(icon: &VectorIcon, side: f32, color: Option<Color>) -> Retained<NSImage> {
    let steps = icon.steps(side);
    let even_odd = icon.even_odd();
    let draw = RcBlock::new(move |_: NSRect| -> Bool {
        let p = |p: mitsuami_core::Point| NSPoint::new(f64::from(p.x), f64::from(p.y));
        let path = NSBezierPath::bezierPath();
        // Quadratic curves as cubic ones: AppKit's own came with macOS 14.
        let mut last = NSPoint::new(0.0, 0.0);
        for step in &steps {
            match *step {
                PathStep::Move(a) => {
                    path.moveToPoint(p(a));
                    last = p(a);
                }
                PathStep::Line(a) => {
                    path.lineToPoint(p(a));
                    last = p(a);
                }
                PathStep::Quad(c, a) => {
                    let (c, a) = (p(c), p(a));
                    let c1 = NSPoint::new(last.x + 2.0 / 3.0 * (c.x - last.x), last.y + 2.0 / 3.0 * (c.y - last.y));
                    let c2 = NSPoint::new(a.x + 2.0 / 3.0 * (c.x - a.x), a.y + 2.0 / 3.0 * (c.y - a.y));
                    path.curveToPoint_controlPoint1_controlPoint2(a, c1, c2);
                    last = a;
                }
                PathStep::Cubic(c1, c2, a) => {
                    path.curveToPoint_controlPoint1_controlPoint2(p(a), p(c1), p(c2));
                    last = p(a);
                }
                PathStep::Close => path.closePath(),
            }
        }
        if even_odd {
            path.setWindingRule(NSWindingRule::EvenOdd);
        }
        color.map_or_else(NSColor::blackColor, crate::custom::ns_color).setFill();
        path.fill();
        Bool::YES
    });
    let size = NSSize::new(f64::from(side), f64::from(side));
    let image = NSImage::imageWithSize_flipped_drawingHandler(size, true, &draw);
    image.setTemplate(color.is_none());
    image
}

/// What an icon shows: the app's own icon over a named one. Without a
/// size, the app's own is as big as a symbol at the system font's size.
/// In a colour, it's in that colour, not a template AppKit tints: for
/// buttons, which tint only when borderless.
pub(super) fn icon_image(
    name: Option<&str>,
    vector: Option<&VectorIcon>,
    points: Option<f32>,
    color: Option<Color>,
) -> Option<Retained<NSImage>> {
    match vector {
        Some(icon) => Some(vector_image(icon, points.unwrap_or(DEFAULT_VECTOR_SIDE), color)),
        None => {
            let image = name.and_then(|name| symbol(name, points))?;
            let Some(color) = color else { return Some(image) };
            let colors = NSArray::from_retained_slice(&[crate::custom::ns_color(color)]);
            let config = NSImageSymbolConfiguration::configurationWithPaletteColors(&colors);
            image.imageWithSymbolConfiguration(&config).or(Some(image))
        }
    }
}

/// A symbol's height at the system font's size, which an icon of the
/// app's own matches.
const DEFAULT_VECTOR_SIDE: f32 = 16.0;

/// Where a button's image goes: before the title, which reads leading in
/// right-to-left layouts too, or alone.
pub(super) fn image_position(icon_only: Option<bool>) -> NSCellImagePosition {
    match icon_only {
        Some(true) => NSCellImagePosition::ImageOnly,
        _ => NSCellImagePosition::ImageLeading,
    }
}

/// The image a source shows, or none for a file AppKit can't read.
pub(crate) fn ns_image(source: &ImageSource) -> Option<Retained<NSImage>> {
    match source {
        ImageSource::File(path) => NSImage::initWithContentsOfFile(NSImage::alloc(), &ns(&path.to_string_lossy())),
        ImageSource::Pixels(pixels) => {
            let (width, height) = (pixels.width() as isize, pixels.height() as isize);
            // AppKit allocates the buffer; the pixels are copied in.
            let rep = unsafe {
                NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
                    NSBitmapImageRep::alloc(),
                    std::ptr::null_mut(),
                    width,
                    height,
                    8,
                    4,
                    true,
                    false,
                    NSDeviceRGBColorSpace,
                    NSBitmapFormat::AlphaNonpremultiplied,
                    width * 4,
                    32,
                )
            }?;
            let data = rep.bitmapData();
            if data.is_null() {
                return None;
            }
            let rgba = pixels.rgba();
            unsafe { std::ptr::copy_nonoverlapping(rgba.as_ptr(), data, rgba.len()) };
            // The bytes are sRGB, as images on every platform are.
            let rep = rep.bitmapImageRepByRetaggingWithColorSpace(&NSColorSpace::sRGBColorSpace())?;
            let size = pixels.size();
            let size = NSSize::new(size.width as f64, size.height as f64);
            rep.setSize(size);
            let image = NSImage::initWithSize(NSImage::alloc(), size);
            image.addRepresentation(&rep);
            Some(image)
        }
    }
}

/// A file icon's side when the app gives none: Finder's list view's.
pub(crate) const FILE_ICON_SIZE: f32 = 16.0;

/// A thumbnail on its way from QuickLook, whose handler runs on another
/// thread and hops to the main one, where the image views are.
struct Sendable(Option<Retained<QLThumbnailRepresentation>>);
// SAFETY: a representation is immutable once made; it's only read on the
// main thread. Only sent, never shared.
unsafe impl Send for Sendable {}

impl Sendable {
    /// Takes it out whole, so closures capture the wrapper, not its field.
    fn into_inner(self) -> Option<Retained<QLThumbnailRepresentation>> {
        self.0
    }
}

/// A thumbnail a view waits for: its request's ticket, the view, and the
/// side to draw it at.
type Waiting = (u64, Weak<NSImageView>, f32);

thread_local! {
    /// The thumbnail each image view (by address) waits for: the latest
    /// request's, so an older one arriving late is dropped.
    static THUMBNAILS: RefCell<HashMap<usize, Waiting>> = RefCell::new(HashMap::new());
    static NEXT_THUMBNAIL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Whether a thumbnail is on its way to a view that's still there. Its
/// handler hops to the main queue, which only a running run loop drains.
pub(super) fn thumbnails_pending() -> bool {
    THUMBNAILS.with(|t| t.borrow().values().any(|(_, view, _)| view.load().is_some()))
}

/// Shows the icon Finder shows for `path` (an app's own, a folder's custom
/// one, a document's by its type) in a `side`-point square; with
/// `thumbnail`, QuickLook's preview replaces it once made, drawn as Finder
/// draws thumbnails in place of icons. By name, it's the icon of the
/// name's type (by its extension: macOS types files by nothing else), or
/// a plain folder's, and nothing is read.
pub(super) fn show_file_icon(
    mtm: MainThreadMarker,
    view: &NSImageView,
    path: Option<&Path>,
    side: f32,
    thumbnail: bool,
    lookup: FileLookup,
) {
    let address = view as *const NSImageView as usize;
    THUMBNAILS.with(|t| t.borrow_mut().remove(&address));
    let workspace = NSWorkspace::sharedWorkspace();
    let icon = match (lookup, path) {
        (FileLookup::Folder, _) => workspace.iconForContentType(unsafe { UTTypeFolder }),
        (_, None) => return view.setImage(None),
        (FileLookup::Disk, Some(path)) => workspace.iconForFile(&super::ns(&path.to_string_lossy())),
        (FileLookup::FileName, Some(path)) => {
            let extension = path.extension().map(|e| e.to_string_lossy());
            let typed = extension.and_then(|e| UTType::typeWithFilenameExtension(&super::ns(&e)));
            match typed {
                Some(typed) => workspace.iconForContentType(&typed),
                // No extension: a document of no particular type, as Finder
                // shows one.
                None => workspace.iconForContentType(unsafe { UTTypeData }),
            }
        }
    };
    // Icons come in several sizes; this picks the one drawn.
    icon.setSize(NSSize::new(side as f64, side as f64));
    view.setImage(Some(&icon));
    let Some(path) = path.filter(|_| thumbnail && lookup == FileLookup::Disk) else { return };
    let scale = view
        .window()
        .map(|w| w.backingScaleFactor())
        .or_else(|| NSScreen::mainScreen(mtm).map(|s| s.backingScaleFactor()))
        .unwrap_or(2.0);
    let url = NSURL::fileURLWithPath(&super::ns(&path.to_string_lossy()));
    let request = unsafe {
        QLThumbnailGenerationRequest::initWithFileAtURL_size_scale_representationTypes(
            QLThumbnailGenerationRequest::alloc(),
            &url,
            CGSize::new(side as f64, side as f64),
            scale,
            QLThumbnailGenerationRequestRepresentationTypes::Thumbnail,
        )
    };
    unsafe { request.setIconMode(true) };
    let ticket = NEXT_THUMBNAIL.replace(NEXT_THUMBNAIL.get() + 1);
    THUMBNAILS.with(|t| t.borrow_mut().insert(address, (ticket, Weak::from(view), side)));
    let done = RcBlock::new(move |made: *mut QLThumbnailRepresentation, _error: *mut NSError| {
        // SAFETY: QuickLook passes a valid representation or null.
        let made = Sendable(unsafe { Retained::retain(made) });
        dispatch2::DispatchQueue::main().exec_async(move || {
            let waiting = THUMBNAILS.with(|t| {
                let mut t = t.borrow_mut();
                let latest = t.get(&address).is_some_and(|(latest, ..)| *latest == ticket);
                if latest { t.remove(&address) } else { None }
            });
            // No thumbnail (a folder, a type QuickLook can't preview): the
            // icon stays.
            let (Some((_, view, side)), Some(made)) = (waiting, made.into_inner()) else { return };
            let Some(view) = view.load() else { return };
            let image = unsafe { made.NSImage() };
            image.setSize(NSSize::new(side as f64, side as f64));
            view.setImage(Some(&image));
        });
    });
    unsafe {
        QLThumbnailGenerator::sharedGenerator().generateBestRepresentationForRequest_completionHandler(&request, &done)
    };
}
