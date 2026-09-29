//! Images: symbols by name, images from sources, and where buttons put them.

use mitsuami_core::ImageSource;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBitmapFormat, NSBitmapImageRep, NSCellImagePosition, NSColorSpace, NSDeviceRGBColorSpace, NSFontWeightRegular,
    NSImage, NSImageSymbolConfiguration,
};
use objc2_foundation::NSSize;

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
