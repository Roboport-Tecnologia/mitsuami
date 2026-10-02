//! Files' icons: the image the shell gives a file, as Explorer shows it
//! (its icon, or with a thumbnail the preview in its place), read on a
//! thread of its own, since the shell may read the file to make it.

use std::path::{Path, PathBuf};

use windows_core::{HSTRING, Interface};

use crate::bindings as w;

/// The shell's image: premultiplied BGRA, top row first, as XAML takes it.
pub(crate) struct ShellImage {
    pub width: i32,
    pub height: i32,
    pub bgra: Vec<u8>,
}

/// Reads `path`'s image at `pixels` square, then calls `done` on that
/// thread.
pub(crate) fn load(
    path: PathBuf,
    pixels: i32,
    thumbnail: bool,
    done: impl FnOnce(Option<ShellImage>) + Send + 'static,
) {
    std::thread::spawn(move || {
        // Single-threaded, as Microsoft's image factory sample has it: the
        // shell's objects and thumbnail handlers are apartment-threaded.
        let com = unsafe { w::CoInitializeEx(std::ptr::null(), w::COINIT_APARTMENTTHREADED as u32) }.is_ok();
        let image = read(&path, pixels, thumbnail && !icons_only());
        if com {
            unsafe { w::CoUninitialize() };
        }
        done(image);
    });
}

/// Whether the user has Explorer show icons, never thumbnails (Folder
/// Options, or "Show thumbnails instead of icons" off in Performance
/// Options), as a Windows Server may have it. Explorer shows the icon
/// then, so this does too.
fn icons_only() -> bool {
    let (key, value) =
        (HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced"), HSTRING::from("IconsOnly"));
    let (mut data, mut size) = (0u32, size_of::<u32>() as u32);
    let read = unsafe {
        w::RegGetValueW(
            w::HKEY_CURRENT_USER,
            windows_core::PCWSTR(key.as_ptr()),
            windows_core::PCWSTR(value.as_ptr()),
            w::RRF_RT_REG_DWORD as u32,
            std::ptr::null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    read == 0 && data != 0
}

fn read(path: &Path, pixels: i32, thumbnail: bool) -> Option<ShellImage> {
    // The shell parses only backslashes, which Rust's paths needn't have
    // (`dir.join("tests/assets")`): made absolute, they do.
    let path = std::path::absolute(path).ok()?;
    let name = HSTRING::from(path.as_os_str());
    let mut raw = std::ptr::null_mut();
    unsafe {
        w::SHCreateItemFromParsingName(
            windows_core::PCWSTR(name.as_ptr()),
            std::ptr::null_mut(),
            &w::IShellItemImageFactory::IID,
            &mut raw,
        )
        .ok()
        .ok()?;
        let factory = w::IShellItemImageFactory::from_raw(raw);
        // Without a thumbnail, the icon alone; with one, the shell's
        // preview, or its icon where it makes none.
        let flags = if thumbnail { w::SIIGBF_RESIZETOFIT } else { w::SIIGBF_ICONONLY };
        let bitmap = factory.GetImage(w::SIZE { cx: pixels, cy: pixels }, flags).ok()?;
        let image = bits(bitmap);
        _ = w::DeleteObject(bitmap);
        image
    }
}

/// The bitmap's pixels, top row first.
unsafe fn bits(bitmap: w::HBITMAP) -> Option<ShellImage> {
    let mut info = w::BITMAP::default();
    let size = size_of::<w::BITMAP>() as i32;
    if unsafe { w::GetObjectW(bitmap, size, (&raw mut info).cast()) } == 0 {
        return None;
    }
    let (width, height) = (info.bmWidth, info.bmHeight.abs());
    let mut header = w::BITMAPINFO {
        bmiHeader: w::BITMAPINFOHEADER {
            biSize: size_of::<w::BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Negative: top-down rows.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: w::BI_RGB as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    let lines = unsafe {
        let dc = w::CreateCompatibleDC(std::ptr::null_mut());
        let lines =
            w::GetDIBits(dc, bitmap, 0, height as u32, bgra.as_mut_ptr().cast(), &mut header, w::DIB_RGB_COLORS as u32);
        _ = w::DeleteDC(dc);
        lines
    };
    if lines != height {
        return None;
    }
    // Thumbnails of pictures without transparency come with no alpha at
    // all: they're opaque.
    if bgra.chunks_exact(4).all(|p| p[3] == 0) {
        bgra.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    Some(ShellImage { width, height, bgra })
}

/// A bitmap XAML shows, of the shell's image.
pub(crate) fn bitmap(image: &ShellImage) -> windows_core::Result<w::WriteableBitmap> {
    let bitmap = w::WriteableBitmap::CreateInstanceWithDimensions(image.width, image.height)?;
    let buffer = bitmap.PixelBuffer()?;
    let length = buffer.Length()? as usize;
    let access: w::IBufferByteAccess = buffer.cast()?;
    // SAFETY: the buffer holds `length` bytes and outlives the slice.
    let bytes = unsafe { std::slice::from_raw_parts_mut(access.Buffer()?, length) };
    let count = length.min(image.bgra.len());
    bytes[..count].copy_from_slice(&image.bgra[..count]);
    bitmap.Invalidate()?;
    Ok(bitmap)
}
