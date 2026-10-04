//! Images: symbols by name, images from sources, files' icons, and where
//! buttons put them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use block2::RcBlock;
use mitsuami_core::ImageSource;
use objc2::rc::{Retained, Weak};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSBitmapFormat, NSBitmapImageRep, NSCellImagePosition, NSColorSpace, NSDeviceRGBColorSpace, NSFontWeightRegular,
    NSImage, NSImageSymbolConfiguration, NSImageView, NSScreen, NSWorkspace,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSError, NSSize, NSURL};
use objc2_quick_look_thumbnailing::{
    QLThumbnailGenerationRequest, QLThumbnailGenerationRequestRepresentationTypes, QLThumbnailGenerator,
    QLThumbnailRepresentation,
};

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
/// draws thumbnails in place of icons.
pub(super) fn show_file_icon(
    mtm: MainThreadMarker,
    view: &NSImageView,
    path: Option<&Path>,
    side: f32,
    thumbnail: bool,
) {
    let address = view as *const NSImageView as usize;
    THUMBNAILS.with(|t| t.borrow_mut().remove(&address));
    let Some(path) = path else { return view.setImage(None) };
    let icon = NSWorkspace::sharedWorkspace().iconForFile(&super::ns(&path.to_string_lossy()));
    // Icons come in several sizes; this picks the one drawn.
    icon.setSize(NSSize::new(side as f64, side as f64));
    view.setImage(Some(&icon));
    if !thumbnail {
        return;
    }
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
