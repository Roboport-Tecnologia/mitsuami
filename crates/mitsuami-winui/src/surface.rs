//! `GpuSurface`: a child window (HWND) of the XAML window, over a `Canvas`
//! that keeps the space, which the app presents to (Direct3D, Vulkan).
//! Nothing of XAML draws over a child window. It takes no input
//! (`HTTRANSPARENT`), so XAML keeps the pointer. It's placed before each of
//! XAML's frames (`CompositionTarget.Rendering`), so it follows the canvas
//! wherever layout or scrolling moves it.

use std::cell::RefCell;
use std::num::NonZeroIsize;
use std::rc::{Rc, Weak};
use std::sync::Once;

use mitsuami_core::raw_window_handle::{
    HandleError, RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use mitsuami_core::{NativeSurface, NodeId, SurfaceHandle, SurfaceSize, UiEvent};
use windows_core::{EventRevoker, Interface, PCWSTR, w as wide};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

const CLASS: PCWSTR = wide!("MitsuamiGpuSurface");

/// The canvas that keeps the surface's space, and what it reports.
pub(crate) struct SurfaceHost {
    pub(crate) canvas: w::Canvas,
    state: Rc<RefCell<HostState>>,
}

struct HostState {
    id: NodeId,
    emitter: Events,
    element: w::UIElement,
    /// Made when the node is first in a window.
    child: Option<SurfaceHandle>,
    hwnd: w::HWND,
    rendering: Option<EventRevoker>,
    /// Where it was put, in the window's client pixels; `None` while hidden.
    placed: Option<(i32, i32, i32, i32)>,
}

impl SurfaceHost {
    pub(crate) fn new(id: NodeId, emitter: Events) -> R<SurfaceHost> {
        let canvas = w::Canvas::new()?;
        let element = canvas.cast()?;
        let state = Rc::new(RefCell::new(HostState {
            id,
            emitter,
            element,
            child: None,
            hwnd: std::ptr::null_mut(),
            rendering: None,
            placed: None,
        }));
        Ok(SurfaceHost { canvas, state })
    }

    /// The node is in `window`: make its child window, once.
    pub(crate) fn attach(&self, window: w::HWND) -> R<()> {
        let mut state = self.state.borrow_mut();
        if state.child.is_some() {
            return Ok(());
        }
        let hwnd = create_child(window)?;
        let handle = SurfaceHandle::new(ChildWindow(hwnd as isize));
        state.hwnd = hwnd;
        state.child = Some(handle.clone());
        let s = Rc::downgrade(&self.state);
        state.rendering = Some(w::CompositionTarget::Rendering(move |_, _| HostState::place(&s))?);
        state.emitter.emit(state.id, UiEvent::SurfaceReady(handle));
        Ok(())
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.state.borrow().child.is_some()
    }

    /// The node is gone: stop showing and reporting. The app's handle may
    /// keep the child window, so it leaves the XAML window, which would
    /// destroy it with itself.
    pub(crate) fn detach(&self) {
        let mut state = self.state.borrow_mut();
        state.rendering = None;
        if state.child.take().is_some() {
            unsafe {
                _ = w::SetWindowPos(state.hwnd, w::HWND_TOP, 0, 0, 0, 0, w::SWP_HIDEWINDOW as u32);
                w::SetParent(state.hwnd, w::HWND_MESSAGE);
            }
        }
    }
}

impl HostState {
    /// Puts the child window over the canvas, or hides it while the canvas
    /// has no size, and reports its size in pixels.
    fn place(this: &Weak<RefCell<HostState>>) {
        let Some(this) = this.upgrade() else { return };
        let mut state = this.borrow_mut();
        let Some(handle) = state.child.clone() else { return };
        let at = state.rect().ok().flatten();
        if at == state.placed {
            return;
        }
        state.placed = at;
        let flags = (w::SWP_NOACTIVATE | if at.is_some() { w::SWP_SHOWWINDOW } else { w::SWP_HIDEWINDOW }) as u32;
        let (x, y, width, height) = at.unwrap_or_default();
        unsafe { _ = w::SetWindowPos(state.hwnd, w::HWND_TOP, x, y, width, height, flags) };
        if let Some((_, _, width, height)) = at {
            let scale = state.scale();
            let size = SurfaceSize { width: width as u32, height: height as u32, scale: scale as f32 };
            if handle.set_size(size) {
                state.emitter.emit(state.id, UiEvent::SurfaceResized(size));
            }
        }
    }

    fn scale(&self) -> f64 {
        self.element.XamlRoot().and_then(|r| r.RasterizationScale()).unwrap_or(1.0)
    }

    /// The canvas in the window's client area, in pixels; `None` while it
    /// isn't laid out or has no size. XAML's root fills the client area.
    fn rect(&self) -> R<Option<(i32, i32, i32, i32)>> {
        let fe: w::IFrameworkElement = self.element.cast()?;
        let (width, height) = (fe.ActualWidth()?, fe.ActualHeight()?);
        if !fe.IsLoaded()? || width <= 0.0 || height <= 0.0 {
            return Ok(None);
        }
        let transform = self.element.cast::<w::IUIElement>()?.TransformToVisual(None::<&w::UIElement>)?;
        let origin = transform.cast::<w::IGeneralTransform>()?.TransformPoint(w::Point { x: 0.0, y: 0.0 })?;
        let scale = self.scale();
        let px = |v: f64| (v * scale).round() as i32;
        Ok(Some((px(origin.x as f64), px(origin.y as f64), px(width), px(height))))
    }
}

/// Registers the child windows' class, once.
fn register_class() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| unsafe {
        let class = w::WNDCLASSEXW {
            cbSize: size_of::<w::WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: w::GetModuleHandleW(PCWSTR::null()),
            lpszClassName: CLASS,
            ..Default::default()
        };
        w::RegisterClassExW(&class);
    });
}

/// Clicks and the pointer go to XAML's window under it.
unsafe extern "system" fn window_proc(hwnd: w::HWND, message: u32, wparam: w::WPARAM, lparam: w::LPARAM) -> w::LRESULT {
    if message == w::WM_NCHITTEST as u32 {
        return w::HTTRANSPARENT as w::LRESULT;
    }
    unsafe { w::DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn create_child(parent: w::HWND) -> R<w::HWND> {
    register_class();
    let hwnd = unsafe {
        w::CreateWindowExW(
            0,
            CLASS,
            PCWSTR::null(),
            (w::WS_CHILD | w::WS_CLIPSIBLINGS) as u32,
            0,
            0,
            0,
            0,
            parent,
            std::ptr::null_mut(),
            w::GetModuleHandleW(PCWSTR::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() { Err(windows_core::Error::from_thread()) } else { Ok(hwnd) }
}

/// The app's share of the child window. Windows are destroyed on their own
/// thread: the last handle asks it to close, from wherever it's dropped.
struct ChildWindow(isize);

impl NativeSurface for ChildWindow {
    fn window_handle(&self) -> Result<RawWindowHandle, HandleError> {
        let mut handle = Win32WindowHandle::new(NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?);
        handle.hinstance = NonZeroIsize::new(unsafe { w::GetModuleHandleW(PCWSTR::null()) } as isize);
        Ok(RawWindowHandle::Win32(handle))
    }

    fn display_handle(&self) -> Result<RawDisplayHandle, HandleError> {
        Ok(RawDisplayHandle::Windows(WindowsDisplayHandle::new()))
    }
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        unsafe { _ = w::PostMessageW(self.0 as w::HWND, w::WM_CLOSE as u32, 0, 0) };
    }
}
