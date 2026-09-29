windows_core::link!("user32.dll" "system" fn CallNextHookEx(hhk : HHOOK, ncode : i32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ClipCursor(lprect : *const RECT) -> windows_core::BOOL);
windows_core::link!("ole32.dll" "system" fn CoCreateInstance(rclsid : *const windows_core::GUID, punkouter : *mut core::ffi::c_void, dwclscontext : u32, riid : *const windows_core::GUID, ppv : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("ole32.dll" "system" fn CoInitializeEx(pvreserved : *const core::ffi::c_void, dwcoinit : u32) -> windows_core::HRESULT);
windows_core::link!("ole32.dll" "system" fn CoTaskMemFree(pv : *mut core::ffi::c_void));
windows_core::link!("ole32.dll" "system" fn CoUninitialize());
windows_core::link!("gdi32.dll" "system" fn CreateBitmap(nwidth : i32, nheight : i32, nplanes : u32, nbitcount : u32, lpbits : *const core::ffi::c_void) -> HBITMAP);
windows_core::link!("gdi32.dll" "system" fn CreateCompatibleDC(hdc : HDC) -> HDC);
windows_core::link!("gdi32.dll" "system" fn CreateDIBSection(hdc : HDC, pbmi : *const BITMAPINFO, usage : u32, ppvbits : *mut *mut core::ffi::c_void, hsection : HANDLE, offset : u32) -> HBITMAP);
windows_core::link!("user32.dll" "system" fn CreateIconFromResourceEx(presbits : *const u8, dwressize : u32, ficon : windows_core::BOOL, dwver : u32, cxdesired : i32, cydesired : i32, flags : u32) -> HICON);
windows_core::link!("user32.dll" "system" fn CreateIconIndirect(piconinfo : *const ICONINFO) -> HICON);
windows_core::link!("user32.dll" "system" fn CreateWindowExW(dwexstyle : u32, lpclassname : windows_core::PCWSTR, lpwindowname : windows_core::PCWSTR, dwstyle : u32, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : HWND, hmenu : HMENU, hinstance : HINSTANCE, lpparam : *const core::ffi::c_void) -> HWND);
windows_core::link!("comctl32.dll" "system" fn DefSubclassProc(hwnd : HWND, umsg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn DefWindowProcW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("gdi32.dll" "system" fn DeleteDC(hdc : HDC) -> windows_core::BOOL);
windows_core::link!("gdi32.dll" "system" fn DeleteObject(ho : HGDIOBJ) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DestroyIcon(hicon : HICON) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DestroyWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DispatchMessageW(lpmsg : *const MSG) -> LRESULT);
windows_core::link!("user32.dll" "system" fn EnableWindow(hwnd : HWND, benable : windows_core::BOOL) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EnumChildWindows(hwndparent : HWND, lpenumfunc : WNDENUMPROC, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EnumWindows(lpenumfunc : WNDENUMPROC, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn FileTimeToSystemTime(lpfiletime : *const FILETIME, lpsystemtime : *mut SYSTEMTIME) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetActiveWindow() -> HWND);
windows_core::link!("user32.dll" "system" fn GetAncestor(hwnd : HWND, gaflags : u32) -> HWND);
windows_core::link!("user32.dll" "system" fn GetClassNameW(hwnd : HWND, lpclassname : windows_core::PWSTR, nmaxcount : i32) -> i32);
windows_core::link!("kernel32.dll" "system" fn GetCurrentPackageFullName(packagefullnamelength : *mut u32, packagefullname : windows_core::PWSTR) -> i32);
windows_core::link!("shell32.dll" "system" fn GetCurrentProcessExplicitAppUserModelID(appid : *mut windows_core::PWSTR) -> windows_core::HRESULT);
windows_core::link!("kernel32.dll" "system" fn GetCurrentProcessId() -> u32);
windows_core::link!("kernel32.dll" "system" fn GetCurrentThreadId() -> u32);
windows_core::link!("user32.dll" "system" fn GetCursorPos(lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("gdi32.dll" "system" fn GetDIBits(hdc : HDC, hbm : HBITMAP, start : u32, clines : u32, lpvbits : *mut core::ffi::c_void, lpbmi : *mut BITMAPINFO, usage : u32) -> i32);
windows_core::link!("kernel32.dll" "system" fn GetDateFormatEx(lplocalename : windows_core::PCWSTR, dwflags : u32, lpdate : *const SYSTEMTIME, lpformat : windows_core::PCWSTR, lpdatestr : windows_core::PWSTR, cchdate : i32, lpcalendar : windows_core::PCWSTR) -> i32);
windows_core::link!("user32.dll" "system" fn GetDpiForSystem() -> u32);
windows_core::link!("user32.dll" "system" fn GetDpiForWindow(hwnd : HWND) -> u32);
windows_core::link!("user32.dll" "system" fn GetFocus() -> HWND);
windows_core::link!("user32.dll" "system" fn GetForegroundWindow() -> HWND);
windows_core::link!("user32.dll" "system" fn GetIconInfo(hicon : HICON, piconinfo : *mut ICONINFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetKeyState(nvirtkey : i32) -> i16);
windows_core::link!("kernel32.dll" "system" fn GetLastError() -> u32);
windows_core::link!("kernel32.dll" "system" fn GetLocaleInfoEx(lplocalename : windows_core::PCWSTR, lctype : LCTYPE, lplcdata : windows_core::PWSTR, cchdata : i32) -> i32);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
windows_core::link!("user32.dll" "system" fn GetMonitorInfoW(hmonitor : HMONITOR, lpmi : *mut MONITORINFO) -> windows_core::BOOL);
windows_core::link!("gdi32.dll" "system" fn GetObjectW(h : HANDLE, c : i32, pv : *mut core::ffi::c_void) -> i32);
windows_core::link!("kernel32.dll" "system" fn GetProcAddress(hmodule : HMODULE, lpprocname : windows_core::PCSTR) -> FARPROC);
windows_core::link!("kernel32.dll" "system" fn GetProcessHeap() -> HANDLE);
windows_core::link!("user32.dll" "system" fn GetRawInputData(hrawinput : HRAWINPUT, uicommand : u32, pdata : *mut core::ffi::c_void, pcbsize : *mut u32, cbsizeheader : u32) -> u32);
windows_core::link!("kernel32.dll" "system" fn GetTimeFormatEx(lplocalename : windows_core::PCWSTR, dwflags : u32, lptime : *const SYSTEMTIME, lpformat : windows_core::PCWSTR, lptimestr : windows_core::PWSTR, cchtime : i32) -> i32);
windows_core::link!("kernel32.dll" "system" fn GetUserPreferredUILanguages(dwflags : u32, pulnumlanguages : *mut u32, pwszlanguagesbuffer : *mut u16, pcchlanguagesbuffer : *mut u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetWindowLongW(hwnd : HWND, nindex : i32) -> i32);
windows_core::link!("user32.dll" "system" fn GetWindowTextW(hwnd : HWND, lpstring : windows_core::PWSTR, nmaxcount : i32) -> i32);
windows_core::link!("user32.dll" "system" fn GetWindowThreadProcessId(hwnd : HWND, lpdwprocessid : *mut u32) -> u32);
windows_core::link!("kernel32.dll" "system" fn HeapFree(hheap : HANDLE, dwflags : u32, lpmem : *mut core::ffi::c_void) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn IsWindowEnabled(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn IsWindowVisible(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn LoadCursorW(hinstance : HINSTANCE, lpcursorname : windows_core::PCWSTR) -> HCURSOR);
windows_core::link!("user32.dll" "system" fn MessageBoxW(hwnd : HWND, lptext : windows_core::PCWSTR, lpcaption : windows_core::PCWSTR, utype : u32) -> i32);
windows_core::link!("user32.dll" "system" fn MonitorFromWindow(hwnd : HWND, dwflags : u32) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn MsgWaitForMultipleObjectsEx(ncount : u32, phandles : *const HANDLE, dwmilliseconds : u32, dwwakemask : u32, dwflags : u32) -> u32);
windows_core::link!("user32.dll" "system" fn PeekMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostThreadMessageW(idthread : u32, msg : u32, wparam : WPARAM, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn RegisterClassExW(param0 : *const WNDCLASSEXW) -> ATOM);
windows_core::link!("user32.dll" "system" fn RegisterRawInputDevices(prawinputdevices : *const RAWINPUTDEVICE, uinumdevices : u32, cbsize : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ReleaseCapture() -> windows_core::BOOL);
windows_core::link!("comctl32.dll" "system" fn RemoveWindowSubclass(hwnd : HWND, pfnsubclass : SUBCLASSPROC, uidsubclass : usize) -> windows_core::BOOL);
windows_core::link!("shell32.dll" "system" fn SHCreateItemFromParsingName(pszpath : windows_core::PCWSTR, pbc : *mut core::ffi::c_void, riid : *const windows_core::GUID, ppv : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("user32.dll" "system" fn SendMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn SetCapture(hwnd : HWND) -> HWND);
windows_core::link!("shell32.dll" "system" fn SetCurrentProcessExplicitAppUserModelID(appid : windows_core::PCWSTR) -> windows_core::HRESULT);
windows_core::link!("user32.dll" "system" fn SetCursor(hcursor : HCURSOR) -> HCURSOR);
windows_core::link!("user32.dll" "system" fn SetCursorPos(x : i32, y : i32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetForegroundWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetLayeredWindowAttributes(hwnd : HWND, crkey : COLORREF, balpha : u8, dwflags : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetParent(hwndchild : HWND, hwndnewparent : HWND) -> HWND);
windows_core::link!("user32.dll" "system" fn SetProcessDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> windows_core::BOOL);
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_arch = "x86_64"))]
windows_core::link!("user32.dll" "system" fn SetWindowLongPtrW(hwnd : HWND, nindex : i32, dwnewlong : isize) -> isize);
#[cfg(target_pointer_width = "32")]
pub use SetWindowLongW as SetWindowLongPtrW;
windows_core::link!("user32.dll" "system" fn SetWindowLongW(hwnd : HWND, nindex : i32, dwnewlong : i32) -> i32);
windows_core::link!("user32.dll" "system" fn SetWindowPos(hwnd : HWND, hwndinsertafter : HWND, x : i32, y : i32, cx : i32, cy : i32, uflags : u32) -> windows_core::BOOL);
windows_core::link!("comctl32.dll" "system" fn SetWindowSubclass(hwnd : HWND, pfnsubclass : SUBCLASSPROC, uidsubclass : usize, dwrefdata : usize) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetWindowsHookExW(idhook : i32, lpfn : HOOKPROC, hmod : HINSTANCE, dwthreadid : u32) -> HHOOK);
windows_core::link!("shell32.dll" "system" fn ShellExecuteExW(pexecinfo : *mut SHELLEXECUTEINFOW) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ShowCursor(bshow : windows_core::BOOL) -> i32);
windows_core::link!("user32.dll" "system" fn ShutdownBlockReasonCreate(hwnd : HWND, pwszreason : windows_core::PCWSTR) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ShutdownBlockReasonDestroy(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn SystemTimeToTzSpecificLocalTime(lptimezoneinformation : *const TIME_ZONE_INFORMATION, lpuniversaltime : *const SYSTEMTIME, lplocaltime : *mut SYSTEMTIME) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TrackMouseEvent(lpeventtrack : *mut TRACKMOUSEEVENT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg : *const MSG) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn UnhookWindowsHookEx(hhk : HHOOK) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn WindowFromPoint(point : POINT) -> HWND);
pub const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;
pub type ATOM = u16;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilitySettings(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AccessibilitySettings, windows_core::IUnknown, windows_core::IInspectable);
impl AccessibilitySettings {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<AccessibilitySettings, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for AccessibilitySettings {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAccessibilitySettings>();
}
unsafe impl windows_core::Interface for AccessibilitySettings {
    type Vtable = <IAccessibilitySettings as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAccessibilitySettings as windows_core::Interface>::IID;
}
impl core::ops::Deref for AccessibilitySettings {
    type Target = IAccessibilitySettings;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AccessibilitySettings {
    const NAME: &'static str = "Windows.UI.ViewManagement.AccessibilitySettings";
}
unsafe impl Send for AccessibilitySettings {}
unsafe impl Sync for AccessibilitySettings {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccessibilityView(pub i32);
impl AccessibilityView {
    pub const Raw: Self = Self(0);
    pub const Control: Self = Self(1);
    pub const Content: Self = Self(2);
}
impl windows_core::imp::TypeKind for AccessibilityView {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for AccessibilityView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Automation.Peers.AccessibilityView;i4)");
}
pub type AddPackageDependencyOptions = u32;
pub const AddPackageDependencyOptions_None: AddPackageDependencyOptions = 0;
pub const AddPackageDependencyOptions_PrependIfRankCollision: AddPackageDependencyOptions = 1;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(AppBar, ContentControl, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for AppBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IAppBar>();
}
unsafe impl windows_core::Interface for AppBar {
    type Vtable = <IAppBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppBar {
    type Target = IAppBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.AppBar";
}
unsafe impl Send for AppBar {}
unsafe impl Sync for AppBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppBarElementContainer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppBarElementContainer, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    AppBarElementContainer,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl AppBarElementContainer {
    pub fn new() -> windows_core::Result<Self> {
        Self::IAppBarElementContainerFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IAppBarElementContainerFactory<R, F: FnOnce(&IAppBarElementContainerFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<AppBarElementContainer, IAppBarElementContainerFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for AppBarElementContainer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAppBarElementContainer>();
}
unsafe impl windows_core::Interface for AppBarElementContainer {
    type Vtable = <IAppBarElementContainer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppBarElementContainer as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppBarElementContainer {
    type Target = IAppBarElementContainer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppBarElementContainer {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.AppBarElementContainer";
}
unsafe impl Send for AppBarElementContainer {}
unsafe impl Sync for AppBarElementContainer {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppWindow(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppWindow, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for AppWindow {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IAppWindow>();
}
unsafe impl windows_core::Interface for AppWindow {
    type Vtable = <IAppWindow as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppWindow as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppWindow {
    type Target = IAppWindow;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppWindow {
    const NAME: &'static str = "Microsoft.UI.Windowing.AppWindow";
}
unsafe impl Send for AppWindow {}
unsafe impl Sync for AppWindow {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppWindowChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppWindowChangedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for AppWindowChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAppWindowChangedEventArgs>();
}
unsafe impl windows_core::Interface for AppWindowChangedEventArgs {
    type Vtable = <IAppWindowChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppWindowChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppWindowChangedEventArgs {
    type Target = IAppWindowChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppWindowChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Windowing.AppWindowChangedEventArgs";
}
unsafe impl Send for AppWindowChangedEventArgs {}
unsafe impl Sync for AppWindowChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppWindowClosingEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppWindowClosingEventArgs, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for AppWindowClosingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAppWindowClosingEventArgs>();
}
unsafe impl windows_core::Interface for AppWindowClosingEventArgs {
    type Vtable = <IAppWindowClosingEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppWindowClosingEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppWindowClosingEventArgs {
    type Target = IAppWindowClosingEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppWindowClosingEventArgs {
    const NAME: &'static str = "Microsoft.UI.Windowing.AppWindowClosingEventArgs";
}
unsafe impl Send for AppWindowClosingEventArgs {}
unsafe impl Sync for AppWindowClosingEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppWindowPresenter(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppWindowPresenter, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for AppWindowPresenter {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAppWindowPresenter>();
}
unsafe impl windows_core::Interface for AppWindowPresenter {
    type Vtable = <IAppWindowPresenter as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppWindowPresenter as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppWindowPresenter {
    type Target = IAppWindowPresenter;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppWindowPresenter {
    const NAME: &'static str = "Microsoft.UI.Windowing.AppWindowPresenter";
}
unsafe impl Send for AppWindowPresenter {}
unsafe impl Sync for AppWindowPresenter {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AppWindowPresenterKind(pub i32);
impl AppWindowPresenterKind {
    pub const Default: Self = Self(0);
    pub const CompactOverlay: Self = Self(1);
    pub const FullScreen: Self = Self(2);
    pub const Overlapped: Self = Self(3);
}
impl windows_core::imp::TypeKind for AppWindowPresenterKind {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for AppWindowPresenterKind {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Windowing.AppWindowPresenterKind;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppWindowTitleBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AppWindowTitleBar, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for AppWindowTitleBar {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAppWindowTitleBar>();
}
unsafe impl windows_core::Interface for AppWindowTitleBar {
    type Vtable = <IAppWindowTitleBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAppWindowTitleBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for AppWindowTitleBar {
    type Target = IAppWindowTitleBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AppWindowTitleBar {
    const NAME: &'static str = "Microsoft.UI.Windowing.AppWindowTitleBar";
}
unsafe impl Send for AppWindowTitleBar {}
unsafe impl Sync for AppWindowTitleBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Application(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Application, windows_core::IUnknown, windows_core::IInspectable);
impl Application {
    pub fn compose<T>(compose: T) -> windows_core::Result<Self>
    where
        T: windows_core::Compose,
    {
        Self::IApplicationFactory(|this| unsafe {
            let (derived__, base__) = windows_core::Compose::compose(compose);
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&derived__),
                base__ as *mut _ as _,
                &mut result__,
            )
            .ok()?;
            let _ = &derived__;
            windows_core::imp::Type::from_abi(result__)
        })
    }
    pub fn Current() -> windows_core::Result<Self> {
        Self::IApplicationStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Current)(windows_core::Interface::as_raw(this), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IApplicationFactory<R, F: FnOnce(&IApplicationFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Application, IApplicationFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
    fn IApplicationStatics<R, F: FnOnce(&IApplicationStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Application, IApplicationStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Application {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IApplication>();
}
unsafe impl windows_core::Interface for Application {
    type Vtable = <IApplication as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IApplication as windows_core::Interface>::IID;
}
impl core::ops::Deref for Application {
    type Target = IApplication;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Application {
    const NAME: &'static str = "Microsoft.UI.Xaml.Application";
}
unsafe impl Send for Application {}
unsafe impl Sync for Application {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApplicationTheme(pub i32);
impl ApplicationTheme {
    pub const Light: Self = Self(0);
    pub const Dark: Self = Self(1);
}
impl windows_core::imp::TypeKind for ApplicationTheme {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ApplicationTheme {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.ApplicationTheme;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoSuggestBox(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AutoSuggestBox, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    AutoSuggestBox,
    ItemsControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl AutoSuggestBox {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<AutoSuggestBox, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for AutoSuggestBox {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAutoSuggestBox>();
}
unsafe impl windows_core::Interface for AutoSuggestBox {
    type Vtable = <IAutoSuggestBox as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAutoSuggestBox as windows_core::Interface>::IID;
}
impl core::ops::Deref for AutoSuggestBox {
    type Target = IAutoSuggestBox;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AutoSuggestBox {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.AutoSuggestBox";
}
unsafe impl Send for AutoSuggestBox {}
unsafe impl Sync for AutoSuggestBox {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoSuggestBoxQuerySubmittedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    AutoSuggestBoxQuerySubmittedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(AutoSuggestBoxQuerySubmittedEventArgs, DependencyObject);
impl windows_core::RuntimeType for AutoSuggestBoxQuerySubmittedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAutoSuggestBoxQuerySubmittedEventArgs>();
}
unsafe impl windows_core::Interface for AutoSuggestBoxQuerySubmittedEventArgs {
    type Vtable = <IAutoSuggestBoxQuerySubmittedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAutoSuggestBoxQuerySubmittedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for AutoSuggestBoxQuerySubmittedEventArgs {
    type Target = IAutoSuggestBoxQuerySubmittedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AutoSuggestBoxQuerySubmittedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.AutoSuggestBoxQuerySubmittedEventArgs";
}
unsafe impl Send for AutoSuggestBoxQuerySubmittedEventArgs {}
unsafe impl Sync for AutoSuggestBoxQuerySubmittedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoSuggestBoxTextChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    AutoSuggestBoxTextChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(AutoSuggestBoxTextChangedEventArgs, DependencyObject);
impl windows_core::RuntimeType for AutoSuggestBoxTextChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAutoSuggestBoxTextChangedEventArgs>();
}
unsafe impl windows_core::Interface for AutoSuggestBoxTextChangedEventArgs {
    type Vtable = <IAutoSuggestBoxTextChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAutoSuggestBoxTextChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for AutoSuggestBoxTextChangedEventArgs {
    type Target = IAutoSuggestBoxTextChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AutoSuggestBoxTextChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.AutoSuggestBoxTextChangedEventArgs";
}
unsafe impl Send for AutoSuggestBoxTextChangedEventArgs {}
unsafe impl Sync for AutoSuggestBoxTextChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutomationPeer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AutomationPeer, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(AutomationPeer, DependencyObject);
impl windows_core::RuntimeType for AutomationPeer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAutomationPeer>();
}
unsafe impl windows_core::Interface for AutomationPeer {
    type Vtable = <IAutomationPeer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAutomationPeer as windows_core::Interface>::IID;
}
impl core::ops::Deref for AutomationPeer {
    type Target = IAutomationPeer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AutomationPeer {
    const NAME: &'static str = "Microsoft.UI.Xaml.Automation.Peers.AutomationPeer";
}
unsafe impl Send for AutomationPeer {}
unsafe impl Sync for AutomationPeer {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutomationProperties(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(AutomationProperties, windows_core::IUnknown, windows_core::IInspectable);
impl AutomationProperties {
    pub fn SetHelpText<P0>(element: P0, value: &str) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IAutomationPropertiesStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetHelpText)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        })
    }
    pub fn GetName<P0>(element: P0) -> windows_core::Result<String>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IAutomationPropertiesStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetName)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                &mut result__,
            )
            .map(|| {
                let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                hstring.to_string_lossy()
            })
        })
    }
    pub fn SetName<P0>(element: P0, value: &str) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IAutomationPropertiesStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetName)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        })
    }
    pub fn SetAccessibilityView<P0>(element: P0, value: AccessibilityView) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IAutomationPropertiesStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetAccessibilityView)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                value,
            )
            .ok()
        })
    }
    fn IAutomationPropertiesStatics<R, F: FnOnce(&IAutomationPropertiesStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<AutomationProperties, IAutomationPropertiesStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for AutomationProperties {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAutomationProperties>();
}
unsafe impl windows_core::Interface for AutomationProperties {
    type Vtable = <IAutomationProperties as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAutomationProperties as windows_core::Interface>::IID;
}
impl core::ops::Deref for AutomationProperties {
    type Target = IAutomationProperties;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AutomationProperties {
    const NAME: &'static str = "Microsoft.UI.Xaml.Automation.AutomationProperties";
}
unsafe impl Send for AutomationProperties {}
unsafe impl Sync for AutomationProperties {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BITMAP {
    pub bmType: i32,
    pub bmWidth: i32,
    pub bmHeight: i32,
    pub bmWidthBytes: i32,
    pub bmPlanes: u16,
    pub bmBitsPixel: u16,
    pub bmBits: *mut core::ffi::c_void,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [RGBQUAD; 1],
}
impl Default for BITMAPINFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}
pub const BI_RGB: i32 = 0;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitmapImage(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(BitmapImage, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(BitmapImage, BitmapSource, ImageSource, DependencyObject);
impl BitmapImage {
    pub fn CreateInstanceWithUriSource<P0>(urisource: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<Uri>,
    {
        Self::IBitmapImageFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstanceWithUriSource)(
                windows_core::Interface::as_raw(this),
                urisource.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IBitmapImageFactory<R, F: FnOnce(&IBitmapImageFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<BitmapImage, IBitmapImageFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for BitmapImage {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IBitmapImage>();
}
unsafe impl windows_core::Interface for BitmapImage {
    type Vtable = <IBitmapImage as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IBitmapImage as windows_core::Interface>::IID;
}
impl core::ops::Deref for BitmapImage {
    type Target = IBitmapImage;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for BitmapImage {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.Imaging.BitmapImage";
}
unsafe impl Send for BitmapImage {}
unsafe impl Sync for BitmapImage {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitmapSource(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(BitmapSource, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(BitmapSource, ImageSource, DependencyObject);
impl windows_core::RuntimeType for BitmapSource {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IBitmapSource>();
}
unsafe impl windows_core::Interface for BitmapSource {
    type Vtable = <IBitmapSource as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IBitmapSource as windows_core::Interface>::IID;
}
impl core::ops::Deref for BitmapSource {
    type Target = IBitmapSource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for BitmapSource {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.Imaging.BitmapSource";
}
unsafe impl Send for BitmapSource {}
unsafe impl Sync for BitmapSource {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Border(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Border, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Border, FrameworkElement, UIElement, DependencyObject);
impl Border {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Border, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Border {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IBorder>();
}
unsafe impl windows_core::Interface for Border {
    type Vtable = <IBorder as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IBorder as windows_core::Interface>::IID;
}
impl core::ops::Deref for Border {
    type Target = IBorder;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Border {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Border";
}
unsafe impl Send for Border {}
unsafe impl Sync for Border {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BreadcrumbBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(BreadcrumbBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(BreadcrumbBar, Control, FrameworkElement, UIElement, DependencyObject);
impl BreadcrumbBar {
    pub fn new() -> windows_core::Result<Self> {
        Self::IBreadcrumbBarFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IBreadcrumbBarFactory<R, F: FnOnce(&IBreadcrumbBarFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<BreadcrumbBar, IBreadcrumbBarFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for BreadcrumbBar {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IBreadcrumbBar>();
}
unsafe impl windows_core::Interface for BreadcrumbBar {
    type Vtable = <IBreadcrumbBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IBreadcrumbBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for BreadcrumbBar {
    type Target = IBreadcrumbBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for BreadcrumbBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.BreadcrumbBar";
}
unsafe impl Send for BreadcrumbBar {}
unsafe impl Sync for BreadcrumbBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BreadcrumbBarItemClickedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    BreadcrumbBarItemClickedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for BreadcrumbBarItemClickedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IBreadcrumbBarItemClickedEventArgs>();
}
unsafe impl windows_core::Interface for BreadcrumbBarItemClickedEventArgs {
    type Vtable = <IBreadcrumbBarItemClickedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IBreadcrumbBarItemClickedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for BreadcrumbBarItemClickedEventArgs {
    type Target = IBreadcrumbBarItemClickedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for BreadcrumbBarItemClickedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.BreadcrumbBarItemClickedEventArgs";
}
unsafe impl Send for BreadcrumbBarItemClickedEventArgs {}
unsafe impl Sync for BreadcrumbBarItemClickedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Brush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Brush, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Brush, DependencyObject);
impl windows_core::RuntimeType for Brush {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IBrush>();
}
unsafe impl windows_core::Interface for Brush {
    type Vtable = <IBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for Brush {
    type Target = IBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Brush {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.Brush";
}
unsafe impl Send for Brush {}
unsafe impl Sync for Brush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Button(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Button, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    Button,
    ButtonBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl Button {
    pub fn new() -> windows_core::Result<Self> {
        Self::IButtonFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IButtonFactory<R, F: FnOnce(&IButtonFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Button, IButtonFactory> = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Button {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IButton>();
}
unsafe impl windows_core::Interface for Button {
    type Vtable = <IButton as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IButton as windows_core::Interface>::IID;
}
impl core::ops::Deref for Button {
    type Target = IButton;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Button {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Button";
}
unsafe impl Send for Button {}
unsafe impl Sync for Button {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ButtonBase(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ButtonBase, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ButtonBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl windows_core::RuntimeType for ButtonBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IButtonBase>();
}
unsafe impl windows_core::Interface for ButtonBase {
    type Vtable = <IButtonBase as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IButtonBase as windows_core::Interface>::IID;
}
impl core::ops::Deref for ButtonBase {
    type Target = IButtonBase;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ButtonBase {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.ButtonBase";
}
unsafe impl Send for ButtonBase {}
unsafe impl Sync for ButtonBase {}
pub const CLSCTX_ALL: i32 = 23;
pub type COINIT = i32;
pub const COINIT_APARTMENTTHREADED: COINIT = 2;
pub const COINIT_MULTITHREADED: COINIT = 0;
pub type COLORREF = u32;
pub const COPYENGINE_E_USER_CANCELLED: windows_core::HRESULT = windows_core::HRESULT(0x80270000_u32 as _);
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Canvas(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Canvas, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Canvas, Panel, FrameworkElement, UIElement, DependencyObject);
impl Canvas {
    pub fn new() -> windows_core::Result<Self> {
        Self::ICanvasFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn GetLeft<P0>(element: P0) -> windows_core::Result<f64>
    where
        P0: windows_core::Param<UIElement>,
    {
        Self::ICanvasStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetLeft)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        })
    }
    pub fn SetLeft<P0>(element: P0, length: f64) -> windows_core::Result<()>
    where
        P0: windows_core::Param<UIElement>,
    {
        Self::ICanvasStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetLeft)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                length,
            )
            .ok()
        })
    }
    pub fn GetTop<P0>(element: P0) -> windows_core::Result<f64>
    where
        P0: windows_core::Param<UIElement>,
    {
        Self::ICanvasStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetTop)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        })
    }
    pub fn SetTop<P0>(element: P0, length: f64) -> windows_core::Result<()>
    where
        P0: windows_core::Param<UIElement>,
    {
        Self::ICanvasStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetTop)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                length,
            )
            .ok()
        })
    }
    fn ICanvasFactory<R, F: FnOnce(&ICanvasFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Canvas, ICanvasFactory> = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
    fn ICanvasStatics<R, F: FnOnce(&ICanvasStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Canvas, ICanvasStatics> = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Canvas {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ICanvas>();
}
unsafe impl windows_core::Interface for Canvas {
    type Vtable = <ICanvas as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICanvas as windows_core::Interface>::IID;
}
impl core::ops::Deref for Canvas {
    type Target = ICanvas;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Canvas {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Canvas";
}
unsafe impl Send for Canvas {}
unsafe impl Sync for Canvas {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckBox(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(CheckBox, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    CheckBox,
    ToggleButton,
    ButtonBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl CheckBox {
    pub fn new() -> windows_core::Result<Self> {
        Self::ICheckBoxFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ICheckBoxFactory<R, F: FnOnce(&ICheckBoxFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<CheckBox, ICheckBoxFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for CheckBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ICheckBox>();
}
unsafe impl windows_core::Interface for CheckBox {
    type Vtable = <ICheckBox as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICheckBox as windows_core::Interface>::IID;
}
impl core::ops::Deref for CheckBox {
    type Target = ICheckBox;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CheckBox {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.CheckBox";
}
unsafe impl Send for CheckBox {}
unsafe impl Sync for CheckBox {}
pub struct Clipboard;
impl Clipboard {
    pub fn GetContent() -> windows_core::Result<DataPackageView> {
        Self::IClipboardStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetContent)(windows_core::Interface::as_raw(this), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn SetContent<P0>(content: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DataPackage>,
    {
        Self::IClipboardStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetContent)(
                windows_core::Interface::as_raw(this),
                content.param().abi(),
            )
            .ok()
        })
    }
    pub fn Flush() -> windows_core::Result<()> {
        Self::IClipboardStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).Flush)(windows_core::Interface::as_raw(this)).ok()
        })
    }
    fn IClipboardStatics<R, F: FnOnce(&IClipboardStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Clipboard, IClipboardStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeName for Clipboard {
    const NAME: &'static str = "Windows.ApplicationModel.DataTransfer.Clipboard";
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl windows_core::imp::TypeKind for Color {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Color {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.UI.Color;u1;u1;u1;u1)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComboBox(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ComboBox, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ComboBox,
    Selector,
    ItemsControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ComboBox {
    pub fn new() -> windows_core::Result<Self> {
        Self::IComboBoxFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IComboBoxFactory<R, F: FnOnce(&IComboBoxFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ComboBox, IComboBoxFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ComboBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IComboBox>();
}
unsafe impl windows_core::Interface for ComboBox {
    type Vtable = <IComboBox as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IComboBox as windows_core::Interface>::IID;
}
impl core::ops::Deref for ComboBox {
    type Target = IComboBox;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ComboBox {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ComboBox";
}
unsafe impl Send for ComboBox {}
unsafe impl Sync for ComboBox {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComboBoxItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ComboBoxItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ComboBoxItem,
    SelectorItem,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ComboBoxItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::IComboBoxItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IComboBoxItemFactory<R, F: FnOnce(&IComboBoxItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ComboBoxItem, IComboBoxItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ComboBoxItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IComboBoxItem>();
}
unsafe impl windows_core::Interface for ComboBoxItem {
    type Vtable = <IComboBoxItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IComboBoxItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for ComboBoxItem {
    type Target = IComboBoxItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ComboBoxItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ComboBoxItem";
}
unsafe impl Send for ComboBoxItem {}
unsafe impl Sync for ComboBoxItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(CommandBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    CommandBar,
    AppBar,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl CommandBar {
    pub fn new() -> windows_core::Result<Self> {
        Self::ICommandBarFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ICommandBarFactory<R, F: FnOnce(&ICommandBarFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<CommandBar, ICommandBarFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for CommandBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ICommandBar>();
}
unsafe impl windows_core::Interface for CommandBar {
    type Vtable = <ICommandBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICommandBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for CommandBar {
    type Target = ICommandBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CommandBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.CommandBar";
}
unsafe impl Send for CommandBar {}
unsafe impl Sync for CommandBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionTarget(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(CompositionTarget, windows_core::IUnknown, windows_core::IInspectable);
impl CompositionTarget {
    pub fn Rendering<F>(handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<windows_core::IInspectable>) + 'static,
    {
        let handler: EventHandler<windows_core::IInspectable> = {
            let com = windows_core::imp::DelegateBox::<EventHandler<windows_core::IInspectable>, F>::new(
                &EventHandlerBox::<windows_core::IInspectable, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        Self::ICompositionTargetStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(this).Rendering)(
                windows_core::Interface::as_raw(this),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                this.clone(),
                token__,
                windows_core::Interface::vtable(this).RemoveRendering,
            ))
        })
    }
    fn ICompositionTargetStatics<R, F: FnOnce(&ICompositionTargetStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<CompositionTarget, ICompositionTargetStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for CompositionTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionTarget>();
}
unsafe impl windows_core::Interface for CompositionTarget {
    type Vtable = <ICompositionTarget as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionTarget as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionTarget {
    type Target = ICompositionTarget;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionTarget {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.CompositionTarget";
}
unsafe impl Send for CompositionTarget {}
unsafe impl Sync for CompositionTarget {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerContentChangingEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ContainerContentChangingEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for ContainerContentChangingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IContainerContentChangingEventArgs>();
}
unsafe impl windows_core::Interface for ContainerContentChangingEventArgs {
    type Vtable = <IContainerContentChangingEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IContainerContentChangingEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ContainerContentChangingEventArgs {
    type Target = IContainerContentChangingEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ContainerContentChangingEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ContainerContentChangingEventArgs";
}
unsafe impl Send for ContainerContentChangingEventArgs {}
unsafe impl Sync for ContainerContentChangingEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentControl(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ContentControl, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ContentControl, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for ContentControl {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IContentControl>();
}
unsafe impl windows_core::Interface for ContentControl {
    type Vtable = <IContentControl as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IContentControl as windows_core::Interface>::IID;
}
impl core::ops::Deref for ContentControl {
    type Target = IContentControl;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ContentControl {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ContentControl";
}
unsafe impl Send for ContentControl {}
unsafe impl Sync for ContentControl {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDialog(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ContentDialog, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ContentDialog,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ContentDialog {
    pub fn new() -> windows_core::Result<Self> {
        Self::IContentDialogFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IContentDialogFactory<R, F: FnOnce(&IContentDialogFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ContentDialog, IContentDialogFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ContentDialog {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IContentDialog>();
}
unsafe impl windows_core::Interface for ContentDialog {
    type Vtable = <IContentDialog as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IContentDialog as windows_core::Interface>::IID;
}
impl core::ops::Deref for ContentDialog {
    type Target = IContentDialog;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ContentDialog {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ContentDialog";
}
unsafe impl Send for ContentDialog {}
unsafe impl Sync for ContentDialog {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentDialogButton(pub i32);
impl ContentDialogButton {
    pub const None: Self = Self(0);
    pub const Primary: Self = Self(1);
    pub const Secondary: Self = Self(2);
    pub const Close: Self = Self(3);
}
impl windows_core::imp::TypeKind for ContentDialogButton {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ContentDialogButton {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.ContentDialogButton;i4)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentDialogResult(pub i32);
impl ContentDialogResult {
    pub const None: Self = Self(0);
    pub const Primary: Self = Self(1);
    pub const Secondary: Self = Self(2);
}
impl windows_core::imp::TypeKind for ContentDialogResult {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ContentDialogResult {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.ContentDialogResult;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentIsland(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ContentIsland, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for ContentIsland {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IContentIsland>();
}
unsafe impl windows_core::Interface for ContentIsland {
    type Vtable = <IContentIsland as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IContentIsland as windows_core::Interface>::IID;
}
impl core::ops::Deref for ContentIsland {
    type Target = IContentIsland;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ContentIsland {
    const NAME: &'static str = "Microsoft.UI.Content.ContentIsland";
}
unsafe impl Send for ContentIsland {}
unsafe impl Sync for ContentIsland {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Control(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Control, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for Control {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IControl>();
}
unsafe impl windows_core::Interface for Control {
    type Vtable = <IControl as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IControl as windows_core::Interface>::IID;
}
impl core::ops::Deref for Control {
    type Target = IControl;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Control {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Control";
}
unsafe impl Send for Control {}
unsafe impl Sync for Control {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CorePhysicalKeyStatus {
    pub repeat_count: u32,
    pub scan_code: u32,
    pub is_extended_key: bool,
    pub is_menu_key_down: bool,
    pub was_key_down: bool,
    pub is_key_released: bool,
}
impl windows_core::imp::TypeKind for CorePhysicalKeyStatus {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for CorePhysicalKeyStatus {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.UI.Core.CorePhysicalKeyStatus;u4;u4;b1;b1;b1;b1)");
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadius {
    pub top_left: f64,
    pub top_right: f64,
    pub bottom_right: f64,
    pub bottom_left: f64,
}
impl windows_core::imp::TypeKind for CornerRadius {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for CornerRadius {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Microsoft.UI.Xaml.CornerRadius;f8;f8;f8;f8)");
}
pub type CreatePackageDependencyOptions = u32;
pub const CreatePackageDependencyOptions_DoNotVerifyDependencyResolution: CreatePackageDependencyOptions = 1;
pub const CreatePackageDependencyOptions_None: CreatePackageDependencyOptions = 0;
pub const CreatePackageDependencyOptions_ScopeIsSystem: CreatePackageDependencyOptions = 2;
pub const DATE_LONGDATE: i32 = 2;
pub const DATE_SHORTDATE: i32 = 1;
pub const DIB_RGB_COLORS: i32 = 0;
pub type DPI_AWARENESS_CONTEXT = *mut core::ffi::c_void;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT = -4 as _;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPackage(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DataPackage, windows_core::IUnknown, windows_core::IInspectable);
impl DataPackage {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<DataPackage, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for DataPackage {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IDataPackage>();
}
unsafe impl windows_core::Interface for DataPackage {
    type Vtable = <IDataPackage as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDataPackage as windows_core::Interface>::IID;
}
impl core::ops::Deref for DataPackage {
    type Target = IDataPackage;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DataPackage {
    const NAME: &'static str = "Windows.ApplicationModel.DataTransfer.DataPackage";
}
unsafe impl Send for DataPackage {}
unsafe impl Sync for DataPackage {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DataPackageOperation(pub u32);
impl DataPackageOperation {
    pub const None: Self = Self(0);
    pub const Copy: Self = Self(1);
    pub const Move: Self = Self(2);
    pub const Link: Self = Self(4);
    pub const NewTarget: Self = Self(1073741824);
    pub const BackgroundTarget: Self = Self(536870912);
}
impl windows_core::imp::TypeKind for DataPackageOperation {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for DataPackageOperation {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.ApplicationModel.DataTransfer.DataPackageOperation;u4)",
    );
}
impl DataPackageOperation {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for DataPackageOperation {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for DataPackageOperation {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for DataPackageOperation {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for DataPackageOperation {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for DataPackageOperation {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPackageView(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DataPackageView, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DataPackageView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDataPackageView>();
}
unsafe impl windows_core::Interface for DataPackageView {
    type Vtable = <IDataPackageView as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDataPackageView as windows_core::Interface>::IID;
}
impl core::ops::Deref for DataPackageView {
    type Target = IDataPackageView;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DataPackageView {
    const NAME: &'static str = "Windows.ApplicationModel.DataTransfer.DataPackageView";
}
unsafe impl Send for DataPackageView {}
unsafe impl Sync for DataPackageView {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataProviderDeferral(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DataProviderDeferral, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DataProviderDeferral {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDataProviderDeferral>();
}
unsafe impl windows_core::Interface for DataProviderDeferral {
    type Vtable = <IDataProviderDeferral as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDataProviderDeferral as windows_core::Interface>::IID;
}
impl core::ops::Deref for DataProviderDeferral {
    type Target = IDataProviderDeferral;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DataProviderDeferral {
    const NAME: &'static str = "Windows.ApplicationModel.DataTransfer.DataProviderDeferral";
}
unsafe impl Send for DataProviderDeferral {}
unsafe impl Sync for DataProviderDeferral {}
windows_core::imp::define_interface!(
    DataProviderHandler,
    DataProviderHandler_Vtbl,
    0xe7ecd720_f2f4_4a2d_920e_170a2f482a27
);
impl windows_core::RuntimeType for DataProviderHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl DataProviderHandler {
    pub fn new<F: Fn(windows_core::Ref<DataProviderRequest>) + 'static>(invoke: F) -> Self {
        let com = windows_core::imp::DelegateBox::<Self, F>::new(&DataProviderHandlerBox::<F>::VTABLE, invoke);
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}
#[repr(C)]
pub struct DataProviderHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        request: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct DataProviderHandlerBox<F: Fn(windows_core::Ref<DataProviderRequest>) + 'static>(
    core::marker::PhantomData<(fn() -> F,)>,
);
impl<F: Fn(windows_core::Ref<DataProviderRequest>) + 'static> DataProviderHandlerBox<F> {
    const VTABLE: DataProviderHandler_Vtbl = DataProviderHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<DataProviderHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DataProviderHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DataProviderHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        request: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<DataProviderHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&request));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataProviderRequest(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DataProviderRequest, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DataProviderRequest {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDataProviderRequest>();
}
unsafe impl windows_core::Interface for DataProviderRequest {
    type Vtable = <IDataProviderRequest as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDataProviderRequest as windows_core::Interface>::IID;
}
impl core::ops::Deref for DataProviderRequest {
    type Target = IDataProviderRequest;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DataProviderRequest {
    const NAME: &'static str = "Windows.ApplicationModel.DataTransfer.DataProviderRequest";
}
unsafe impl Send for DataProviderRequest {}
unsafe impl Sync for DataProviderRequest {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataReader(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DataReader, windows_core::IUnknown, windows_core::IInspectable, IDataReader);
impl DataReader {
    pub fn FromBuffer<P0>(buffer: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<IBuffer>,
    {
        Self::IDataReaderStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).FromBuffer)(
                windows_core::Interface::as_raw(this),
                buffer.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IDataReaderStatics<R, F: FnOnce(&IDataReaderStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<DataReader, IDataReaderStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for DataReader {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IDataReader>();
}
unsafe impl windows_core::Interface for DataReader {
    type Vtable = <IDataReader as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDataReader as windows_core::Interface>::IID;
}
impl core::ops::Deref for DataReader {
    type Target = IDataReader;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DataReader {
    const NAME: &'static str = "Windows.Storage.Streams.DataReader";
}
unsafe impl Send for DataReader {}
unsafe impl Sync for DataReader {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataTemplate(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DataTemplate, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(DataTemplate, FrameworkTemplate, DependencyObject);
impl windows_core::RuntimeType for DataTemplate {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDataTemplate>();
}
unsafe impl windows_core::Interface for DataTemplate {
    type Vtable = <IDataTemplate as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDataTemplate as windows_core::Interface>::IID;
}
impl core::ops::Deref for DataTemplate {
    type Target = IDataTemplate;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DataTemplate {
    const NAME: &'static str = "Microsoft.UI.Xaml.DataTemplate";
}
unsafe impl Send for DataTemplate {}
unsafe impl Sync for DataTemplate {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyObject(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DependencyObject, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DependencyObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDependencyObject>();
}
unsafe impl windows_core::Interface for DependencyObject {
    type Vtable = <IDependencyObject as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDependencyObject as windows_core::Interface>::IID;
}
impl core::ops::Deref for DependencyObject {
    type Target = IDependencyObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DependencyObject {
    const NAME: &'static str = "Microsoft.UI.Xaml.DependencyObject";
}
unsafe impl Send for DependencyObject {}
unsafe impl Sync for DependencyObject {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyProperty(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DependencyProperty, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DependencyProperty {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDependencyProperty>();
}
unsafe impl windows_core::Interface for DependencyProperty {
    type Vtable = <IDependencyProperty as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDependencyProperty as windows_core::Interface>::IID;
}
impl core::ops::Deref for DependencyProperty {
    type Target = IDependencyProperty;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DependencyProperty {
    const NAME: &'static str = "Microsoft.UI.Xaml.DependencyProperty";
}
unsafe impl Send for DependencyProperty {}
unsafe impl Sync for DependencyProperty {}
windows_core::imp::define_interface!(
    DependencyPropertyChangedCallback,
    DependencyPropertyChangedCallback_Vtbl,
    0xf055bb21_219b_5b0c_805d_bcaedae15458
);
impl windows_core::RuntimeType for DependencyPropertyChangedCallback {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl DependencyPropertyChangedCallback {
    pub fn new<F: Fn(windows_core::Ref<DependencyObject>, windows_core::Ref<DependencyProperty>) + 'static>(
        invoke: F,
    ) -> Self {
        let com =
            windows_core::imp::DelegateBox::<Self, F>::new(&DependencyPropertyChangedCallbackBox::<F>::VTABLE, invoke);
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}
#[repr(C)]
pub struct DependencyPropertyChangedCallback_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        dp: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct DependencyPropertyChangedCallbackBox<
    F: Fn(windows_core::Ref<DependencyObject>, windows_core::Ref<DependencyProperty>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<DependencyObject>, windows_core::Ref<DependencyProperty>) + 'static>
    DependencyPropertyChangedCallbackBox<F>
{
    const VTABLE: DependencyPropertyChangedCallback_Vtbl = DependencyPropertyChangedCallback_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<DependencyPropertyChangedCallback, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DependencyPropertyChangedCallback, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DependencyPropertyChangedCallback, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        dp: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<DependencyPropertyChangedCallback, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&dp));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatcherQueue(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DispatcherQueue, windows_core::IUnknown, windows_core::IInspectable);
impl DispatcherQueue {
    pub fn GetForCurrentThread() -> windows_core::Result<Self> {
        Self::IDispatcherQueueStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetForCurrentThread)(
                windows_core::Interface::as_raw(this),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IDispatcherQueueStatics<R, F: FnOnce(&IDispatcherQueueStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<DispatcherQueue, IDispatcherQueueStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for DispatcherQueue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueue>();
}
unsafe impl windows_core::Interface for DispatcherQueue {
    type Vtable = <IDispatcherQueue as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDispatcherQueue as windows_core::Interface>::IID;
}
impl core::ops::Deref for DispatcherQueue {
    type Target = IDispatcherQueue;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DispatcherQueue {
    const NAME: &'static str = "Microsoft.UI.Dispatching.DispatcherQueue";
}
unsafe impl Send for DispatcherQueue {}
unsafe impl Sync for DispatcherQueue {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatcherQueueController(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DispatcherQueueController, windows_core::IUnknown, windows_core::IInspectable);
impl DispatcherQueueController {
    pub fn CreateOnCurrentThread() -> windows_core::Result<Self> {
        Self::IDispatcherQueueControllerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateOnCurrentThread)(
                windows_core::Interface::as_raw(this),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IDispatcherQueueControllerStatics<
        R,
        F: FnOnce(&IDispatcherQueueControllerStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<DispatcherQueueController, IDispatcherQueueControllerStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for DispatcherQueueController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueueController>();
}
unsafe impl windows_core::Interface for DispatcherQueueController {
    type Vtable = <IDispatcherQueueController as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDispatcherQueueController as windows_core::Interface>::IID;
}
impl core::ops::Deref for DispatcherQueueController {
    type Target = IDispatcherQueueController;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DispatcherQueueController {
    const NAME: &'static str = "Microsoft.UI.Dispatching.DispatcherQueueController";
}
unsafe impl Send for DispatcherQueueController {}
unsafe impl Sync for DispatcherQueueController {}
windows_core::imp::define_interface!(
    DispatcherQueueHandler,
    DispatcherQueueHandler_Vtbl,
    0x2e0872a9_4e29_5f14_b688_fb96d5f9d5f8
);
impl windows_core::RuntimeType for DispatcherQueueHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl DispatcherQueueHandler {
    pub fn new<F: Fn() + 'static>(invoke: F) -> Self {
        let com = windows_core::imp::DelegateBox::<Self, F>::new(&DispatcherQueueHandlerBox::<F>::VTABLE, invoke);
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}
#[repr(C)]
pub struct DispatcherQueueHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> windows_core::HRESULT,
}
struct DispatcherQueueHandlerBox<F: Fn() + 'static>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn() + 'static> DispatcherQueueHandlerBox<F> {
    const VTABLE: DispatcherQueueHandler_Vtbl = DispatcherQueueHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(this: *mut core::ffi::c_void) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<DispatcherQueueHandler, F>);
            (this.invoke)();
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatcherQueueTimer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DispatcherQueueTimer, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DispatcherQueueTimer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueueTimer>();
}
unsafe impl windows_core::Interface for DispatcherQueueTimer {
    type Vtable = <IDispatcherQueueTimer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDispatcherQueueTimer as windows_core::Interface>::IID;
}
impl core::ops::Deref for DispatcherQueueTimer {
    type Target = IDispatcherQueueTimer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DispatcherQueueTimer {
    const NAME: &'static str = "Microsoft.UI.Dispatching.DispatcherQueueTimer";
}
unsafe impl Send for DispatcherQueueTimer {}
unsafe impl Sync for DispatcherQueueTimer {}
windows_core::imp::define_interface!(
    DoubleTappedEventHandler,
    DoubleTappedEventHandler_Vtbl,
    0xf7a501b9_e277_5611_87b0_0e0607622183
);
impl windows_core::RuntimeType for DoubleTappedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct DoubleTappedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct DoubleTappedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DoubleTappedRoutedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DoubleTappedRoutedEventArgs>) + 'static>
    DoubleTappedEventHandlerBox<F>
{
    const VTABLE: DoubleTappedEventHandler_Vtbl = DoubleTappedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<DoubleTappedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DoubleTappedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DoubleTappedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<DoubleTappedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoubleTappedRoutedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DoubleTappedRoutedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(DoubleTappedRoutedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for DoubleTappedRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDoubleTappedRoutedEventArgs>();
}
unsafe impl windows_core::Interface for DoubleTappedRoutedEventArgs {
    type Vtable = <IDoubleTappedRoutedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDoubleTappedRoutedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for DoubleTappedRoutedEventArgs {
    type Target = IDoubleTappedRoutedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DoubleTappedRoutedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.DoubleTappedRoutedEventArgs";
}
unsafe impl Send for DoubleTappedRoutedEventArgs {}
unsafe impl Sync for DoubleTappedRoutedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DragEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(DragEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for DragEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDragEventArgs>();
}
unsafe impl windows_core::Interface for DragEventArgs {
    type Vtable = <IDragEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDragEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for DragEventArgs {
    type Target = IDragEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DragEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.DragEventArgs";
}
unsafe impl Send for DragEventArgs {}
unsafe impl Sync for DragEventArgs {}
windows_core::imp::define_interface!(DragEventHandler, DragEventHandler_Vtbl, 0x277afc83_cb67_56c8_b601_1b9c0f1c3d32);
impl windows_core::RuntimeType for DragEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct DragEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct DragEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragEventArgs>) + 'static>
    DragEventHandlerBox<F>
{
    const VTABLE: DragEventHandler_Vtbl = DragEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<DragEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DragEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DragEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this =
                &mut *(this as *mut *mut core::ffi::c_void as *mut windows_core::imp::DelegateBox<DragEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragItemsStartingEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DragItemsStartingEventArgs, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for DragItemsStartingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDragItemsStartingEventArgs>();
}
unsafe impl windows_core::Interface for DragItemsStartingEventArgs {
    type Vtable = <IDragItemsStartingEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDragItemsStartingEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for DragItemsStartingEventArgs {
    type Target = IDragItemsStartingEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DragItemsStartingEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.DragItemsStartingEventArgs";
}
unsafe impl Send for DragItemsStartingEventArgs {}
unsafe impl Sync for DragItemsStartingEventArgs {}
windows_core::imp::define_interface!(
    DragItemsStartingEventHandler,
    DragItemsStartingEventHandler_Vtbl,
    0x55532800_7617_5d67_80bf_b98c0a41b9d6
);
impl windows_core::RuntimeType for DragItemsStartingEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct DragItemsStartingEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct DragItemsStartingEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragItemsStartingEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragItemsStartingEventArgs>) + 'static>
    DragItemsStartingEventHandlerBox<F>
{
    const VTABLE: DragItemsStartingEventHandler_Vtbl = DragItemsStartingEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<DragItemsStartingEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DragItemsStartingEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DragItemsStartingEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<DragItemsStartingEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropDownButton(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(DropDownButton, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    DropDownButton,
    Button,
    ButtonBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl DropDownButton {
    pub fn new() -> windows_core::Result<Self> {
        Self::IDropDownButtonFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IDropDownButtonFactory<R, F: FnOnce(&IDropDownButtonFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<DropDownButton, IDropDownButtonFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for DropDownButton {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDropDownButton>();
}
unsafe impl windows_core::Interface for DropDownButton {
    type Vtable = <IDropDownButton as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDropDownButton as windows_core::Interface>::IID;
}
impl core::ops::Deref for DropDownButton {
    type Target = IDropDownButton;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DropDownButton {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.DropDownButton";
}
unsafe impl Send for DropDownButton {}
unsafe impl Sync for DropDownButton {}
pub const ERROR_CANCELLED: i32 = 1223;
pub const ERROR_NO_ASSOCIATION: i32 = 1155;
pub const E_FAIL: windows_core::HRESULT = windows_core::HRESULT(0x80004005_u32 as _);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ElementTheme(pub i32);
impl ElementTheme {
    pub const Default: Self = Self(0);
    pub const Light: Self = Self(1);
    pub const Dark: Self = Self(2);
}
impl windows_core::imp::TypeKind for ElementTheme {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ElementTheme {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.ElementTheme;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHandler<T>(windows_core::IUnknown, core::marker::PhantomData<T>)
where
    T: windows_core::RuntimeType + 'static;
unsafe impl<T: windows_core::RuntimeType + 'static> windows_core::Interface for EventHandler<T> {
    type Vtable = EventHandler_Vtbl<T>;
    const IID: windows_core::GUID = windows_core::GUID::from_signature(<Self as windows_core::RuntimeType>::SIGNATURE);
}
impl<T: windows_core::RuntimeType + 'static> windows_core::RuntimeType for EventHandler<T> {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
        .push_slice(b"pinterface({9de1c535-6ae1-11e0-84e1-18a905bcc53f}")
        .push_slice(b";")
        .push_other(T::SIGNATURE)
        .push_slice(b")");
}
#[repr(C)]
pub struct EventHandler_Vtbl<T>
where
    T: windows_core::RuntimeType + 'static,
{
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        args: windows_core::imp::AbiType<T>,
    ) -> windows_core::HRESULT,
    T: core::marker::PhantomData<T>,
}
struct EventHandlerBox<T, F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<T>) + 'static>(
    core::marker::PhantomData<(T, fn() -> F)>,
)
where
    T: windows_core::RuntimeType + 'static;
impl<
    T: windows_core::RuntimeType + 'static,
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<T>) + 'static,
> EventHandlerBox<T, F>
{
    const VTABLE: EventHandler_Vtbl<T> = EventHandler_Vtbl::<T> {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<EventHandler<T>, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<EventHandler<T>, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<EventHandler<T>, F>::Release,
        },
        Invoke: Self::Invoke,
        T: core::marker::PhantomData::<T>,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        args: windows_core::imp::AbiType<T>,
    ) -> windows_core::HRESULT {
        unsafe {
            let this =
                &mut *(this as *mut *mut core::ffi::c_void as *mut windows_core::imp::DelegateBox<EventHandler<T>, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&args));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExceptionRoutedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ExceptionRoutedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ExceptionRoutedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for ExceptionRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IExceptionRoutedEventArgs>();
}
unsafe impl windows_core::Interface for ExceptionRoutedEventArgs {
    type Vtable = <IExceptionRoutedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IExceptionRoutedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ExceptionRoutedEventArgs {
    type Target = IExceptionRoutedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ExceptionRoutedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.ExceptionRoutedEventArgs";
}
unsafe impl Send for ExceptionRoutedEventArgs {}
unsafe impl Sync for ExceptionRoutedEventArgs {}
windows_core::imp::define_interface!(
    ExceptionRoutedEventHandler,
    ExceptionRoutedEventHandler_Vtbl,
    0x45fbb85d_54f9_5a2a_8a38_00a3b7761f96
);
impl windows_core::RuntimeType for ExceptionRoutedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ExceptionRoutedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct ExceptionRoutedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<ExceptionRoutedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<ExceptionRoutedEventArgs>) + 'static>
    ExceptionRoutedEventHandlerBox<F>
{
    const VTABLE: ExceptionRoutedEventHandler_Vtbl = ExceptionRoutedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<ExceptionRoutedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<ExceptionRoutedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<ExceptionRoutedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<ExceptionRoutedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
pub type FARPROC = Option<unsafe extern "system" fn() -> isize>;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}
pub const FOFX_RECYCLEONDELETE: i32 = 524288;
pub const FOF_ALLOWUNDO: i32 = 64;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileOpenPicker(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FileOpenPicker, windows_core::IUnknown, windows_core::IInspectable);
impl FileOpenPicker {
    pub fn CreateInstance(windowid: WindowId) -> windows_core::Result<Self> {
        Self::IFileOpenPickerFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                windowid,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFileOpenPickerFactory<R, F: FnOnce(&IFileOpenPickerFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<FileOpenPicker, IFileOpenPickerFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FileOpenPicker {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFileOpenPicker>();
}
unsafe impl windows_core::Interface for FileOpenPicker {
    type Vtable = <IFileOpenPicker as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFileOpenPicker as windows_core::Interface>::IID;
}
impl core::ops::Deref for FileOpenPicker {
    type Target = IFileOpenPicker;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FileOpenPicker {
    const NAME: &'static str = "Microsoft.Windows.Storage.Pickers.FileOpenPicker";
}
unsafe impl Send for FileOpenPicker {}
unsafe impl Sync for FileOpenPicker {}
pub const FileOperation: windows_core::GUID = windows_core::GUID::from_u128(0x3ad05575_8857_4850_9277_11b85bdb8e09);
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSavePicker(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FileSavePicker, windows_core::IUnknown, windows_core::IInspectable);
impl FileSavePicker {
    pub fn CreateInstance(windowid: WindowId) -> windows_core::Result<Self> {
        Self::IFileSavePickerFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                windowid,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFileSavePickerFactory<R, F: FnOnce(&IFileSavePickerFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<FileSavePicker, IFileSavePickerFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FileSavePicker {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFileSavePicker>();
}
unsafe impl windows_core::Interface for FileSavePicker {
    type Vtable = <IFileSavePicker as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFileSavePicker as windows_core::Interface>::IID;
}
impl core::ops::Deref for FileSavePicker {
    type Target = IFileSavePicker;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FileSavePicker {
    const NAME: &'static str = "Microsoft.Windows.Storage.Pickers.FileSavePicker";
}
unsafe impl Send for FileSavePicker {}
unsafe impl Sync for FileSavePicker {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FlowDirection(pub i32);
impl FlowDirection {
    pub const LeftToRight: Self = Self(0);
    pub const RightToLeft: Self = Self(1);
}
impl windows_core::imp::TypeKind for FlowDirection {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for FlowDirection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.FlowDirection;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlyoutBase(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FlyoutBase, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(FlyoutBase, DependencyObject);
impl windows_core::RuntimeType for FlyoutBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IFlyoutBase>();
}
unsafe impl windows_core::Interface for FlyoutBase {
    type Vtable = <IFlyoutBase as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFlyoutBase as windows_core::Interface>::IID;
}
impl core::ops::Deref for FlyoutBase {
    type Target = IFlyoutBase;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FlyoutBase {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.FlyoutBase";
}
unsafe impl Send for FlyoutBase {}
unsafe impl Sync for FlyoutBase {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FlyoutPlacementMode(pub i32);
impl FlyoutPlacementMode {
    pub const Top: Self = Self(0);
    pub const Bottom: Self = Self(1);
    pub const Left: Self = Self(2);
    pub const Right: Self = Self(3);
    pub const Full: Self = Self(4);
    pub const TopEdgeAlignedLeft: Self = Self(5);
    pub const TopEdgeAlignedRight: Self = Self(6);
    pub const BottomEdgeAlignedLeft: Self = Self(7);
    pub const BottomEdgeAlignedRight: Self = Self(8);
    pub const LeftEdgeAlignedTop: Self = Self(9);
    pub const LeftEdgeAlignedBottom: Self = Self(10);
    pub const RightEdgeAlignedTop: Self = Self(11);
    pub const RightEdgeAlignedBottom: Self = Self(12);
    pub const Auto: Self = Self(13);
}
impl windows_core::imp::TypeKind for FlyoutPlacementMode {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for FlyoutPlacementMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusManager(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FocusManager, windows_core::IUnknown, windows_core::IInspectable);
impl FocusManager {
    pub fn GetFocusedElementWithRoot<P0>(xamlroot: P0) -> windows_core::Result<windows_core::IInspectable>
    where
        P0: windows_core::Param<XamlRoot>,
    {
        Self::IFocusManagerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetFocusedElementWithRoot)(
                windows_core::Interface::as_raw(this),
                xamlroot.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFocusManagerStatics<R, F: FnOnce(&IFocusManagerStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<FocusManager, IFocusManagerStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FocusManager {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFocusManager>();
}
unsafe impl windows_core::Interface for FocusManager {
    type Vtable = <IFocusManager as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFocusManager as windows_core::Interface>::IID;
}
impl core::ops::Deref for FocusManager {
    type Target = IFocusManager;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FocusManager {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.FocusManager";
}
unsafe impl Send for FocusManager {}
unsafe impl Sync for FocusManager {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FocusState(pub i32);
impl FocusState {
    pub const Unfocused: Self = Self(0);
    pub const Pointer: Self = Self(1);
    pub const Keyboard: Self = Self(2);
    pub const Programmatic: Self = Self(3);
}
impl windows_core::imp::TypeKind for FocusState {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for FocusState {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.FocusState;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderPicker(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FolderPicker, windows_core::IUnknown, windows_core::IInspectable);
impl FolderPicker {
    pub fn CreateInstance(windowid: WindowId) -> windows_core::Result<Self> {
        Self::IFolderPickerFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                windowid,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFolderPickerFactory<R, F: FnOnce(&IFolderPickerFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<FolderPicker, IFolderPickerFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FolderPicker {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFolderPicker>();
}
unsafe impl windows_core::Interface for FolderPicker {
    type Vtable = <IFolderPicker as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFolderPicker as windows_core::Interface>::IID;
}
impl core::ops::Deref for FolderPicker {
    type Target = IFolderPicker;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FolderPicker {
    const NAME: &'static str = "Microsoft.Windows.Storage.Pickers.FolderPicker";
}
unsafe impl Send for FolderPicker {}
unsafe impl Sync for FolderPicker {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontFamily(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FontFamily, windows_core::IUnknown, windows_core::IInspectable);
impl FontFamily {
    pub fn CreateInstanceWithName(familyname: &str) -> windows_core::Result<Self> {
        Self::IFontFamilyFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstanceWithName)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(familyname)),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFontFamilyFactory<R, F: FnOnce(&IFontFamilyFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<FontFamily, IFontFamilyFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FontFamily {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IFontFamily>();
}
unsafe impl windows_core::Interface for FontFamily {
    type Vtable = <IFontFamily as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFontFamily as windows_core::Interface>::IID;
}
impl core::ops::Deref for FontFamily {
    type Target = IFontFamily;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FontFamily {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.FontFamily";
}
unsafe impl Send for FontFamily {}
unsafe impl Sync for FontFamily {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontIcon(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FontIcon, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(FontIcon, IconElement, FrameworkElement, UIElement, DependencyObject);
impl FontIcon {
    pub fn new() -> windows_core::Result<Self> {
        Self::IFontIconFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFontIconFactory<R, F: FnOnce(&IFontIconFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<FontIcon, IFontIconFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FontIcon {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IFontIcon>();
}
unsafe impl windows_core::Interface for FontIcon {
    type Vtable = <IFontIcon as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFontIcon as windows_core::Interface>::IID;
}
impl core::ops::Deref for FontIcon {
    type Target = IFontIcon;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FontIcon {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.FontIcon";
}
unsafe impl Send for FontIcon {}
unsafe impl Sync for FontIcon {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FontStyle(pub i32);
impl FontStyle {
    pub const Normal: Self = Self(0);
    pub const Oblique: Self = Self(1);
    pub const Italic: Self = Self(2);
}
impl windows_core::imp::TypeKind for FontStyle {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for FontStyle {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.UI.Text.FontStyle;i4)");
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FontWeight {
    pub weight: u16,
}
impl windows_core::imp::TypeKind for FontWeight {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for FontWeight {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.UI.Text.FontWeight;u2)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkElement(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FrameworkElement, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for FrameworkElement {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFrameworkElement>();
}
unsafe impl windows_core::Interface for FrameworkElement {
    type Vtable = <IFrameworkElement as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFrameworkElement as windows_core::Interface>::IID;
}
impl core::ops::Deref for FrameworkElement {
    type Target = IFrameworkElement;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FrameworkElement {
    const NAME: &'static str = "Microsoft.UI.Xaml.FrameworkElement";
}
unsafe impl Send for FrameworkElement {}
unsafe impl Sync for FrameworkElement {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkElementAutomationPeer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    FrameworkElementAutomationPeer,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(FrameworkElementAutomationPeer, AutomationPeer, DependencyObject);
impl FrameworkElementAutomationPeer {
    pub fn CreatePeerForElement<P0>(element: P0) -> windows_core::Result<AutomationPeer>
    where
        P0: windows_core::Param<UIElement>,
    {
        Self::IFrameworkElementAutomationPeerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreatePeerForElement)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IFrameworkElementAutomationPeerStatics<
        R,
        F: FnOnce(&IFrameworkElementAutomationPeerStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            FrameworkElementAutomationPeer,
            IFrameworkElementAutomationPeerStatics,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for FrameworkElementAutomationPeer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFrameworkElementAutomationPeer>();
}
unsafe impl windows_core::Interface for FrameworkElementAutomationPeer {
    type Vtable = <IFrameworkElementAutomationPeer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFrameworkElementAutomationPeer as windows_core::Interface>::IID;
}
impl core::ops::Deref for FrameworkElementAutomationPeer {
    type Target = IFrameworkElementAutomationPeer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FrameworkElementAutomationPeer {
    const NAME: &'static str = "Microsoft.UI.Xaml.Automation.Peers.FrameworkElementAutomationPeer";
}
unsafe impl Send for FrameworkElementAutomationPeer {}
unsafe impl Sync for FrameworkElementAutomationPeer {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkTemplate(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(FrameworkTemplate, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(FrameworkTemplate, DependencyObject);
impl windows_core::RuntimeType for FrameworkTemplate {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IFrameworkTemplate>();
}
unsafe impl windows_core::Interface for FrameworkTemplate {
    type Vtable = <IFrameworkTemplate as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IFrameworkTemplate as windows_core::Interface>::IID;
}
impl core::ops::Deref for FrameworkTemplate {
    type Target = IFrameworkTemplate;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for FrameworkTemplate {
    const NAME: &'static str = "Microsoft.UI.Xaml.FrameworkTemplate";
}
unsafe impl Send for FrameworkTemplate {}
unsafe impl Sync for FrameworkTemplate {}
pub const GA_ROOT: i32 = 2;
pub const GWLP_HWNDPARENT: i32 = -8;
pub const GWL_EXSTYLE: i32 = -20;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralTransform(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(GeneralTransform, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(GeneralTransform, DependencyObject);
impl windows_core::RuntimeType for GeneralTransform {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IGeneralTransform>();
}
unsafe impl windows_core::Interface for GeneralTransform {
    type Vtable = <IGeneralTransform as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IGeneralTransform as windows_core::Interface>::IID;
}
impl core::ops::Deref for GeneralTransform {
    type Target = IGeneralTransform;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for GeneralTransform {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.GeneralTransform";
}
unsafe impl Send for GeneralTransform {}
unsafe impl Sync for GeneralTransform {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Geometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Geometry, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Geometry, DependencyObject);
impl windows_core::RuntimeType for Geometry {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IGeometry>();
}
unsafe impl windows_core::Interface for Geometry {
    type Vtable = <IGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for Geometry {
    type Target = IGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Geometry {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.Geometry";
}
unsafe impl Send for Geometry {}
unsafe impl Sync for Geometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grid(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Grid, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Grid, Panel, FrameworkElement, UIElement, DependencyObject);
impl Grid {
    pub fn SetRow<P0>(element: P0, value: i32) -> windows_core::Result<()>
    where
        P0: windows_core::Param<FrameworkElement>,
    {
        Self::IGridStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetRow)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                value,
            )
            .ok()
        })
    }
    fn IGridStatics<R, F: FnOnce(&IGridStatics) -> windows_core::Result<R>>(callback: F) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Grid, IGridStatics> = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Grid {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IGrid>();
}
unsafe impl windows_core::Interface for Grid {
    type Vtable = <IGrid as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IGrid as windows_core::Interface>::IID;
}
impl core::ops::Deref for Grid {
    type Target = IGrid;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Grid {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Grid";
}
unsafe impl Send for Grid {}
unsafe impl Sync for Grid {}
pub type HANDLE = *mut core::ffi::c_void;
pub type HBITMAP = *mut core::ffi::c_void;
pub type HBRUSH = *mut core::ffi::c_void;
pub type HCURSOR = HICON;
pub type HDC = *mut core::ffi::c_void;
pub type HGDIOBJ = *mut core::ffi::c_void;
pub type HHOOK = *mut core::ffi::c_void;
pub type HICON = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HKEY = *mut core::ffi::c_void;
pub type HMENU = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
pub type HMONITOR = *mut core::ffi::c_void;
pub type HOOKPROC = Option<unsafe extern "system" fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT>;
pub type HRAWINPUT = *mut core::ffi::c_void;
pub const HTCLIENT: i32 = 1;
pub const HTTRANSPARENT: i32 = -1;
pub type HWND = *mut core::ffi::c_void;
pub const HWND_MESSAGE: HWND = -3 as _;
pub const HWND_TOP: HWND = 0 as _;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HorizontalAlignment(pub i32);
impl HorizontalAlignment {
    pub const Left: Self = Self(0);
    pub const Center: Self = Self(1);
    pub const Right: Self = Self(2);
    pub const Stretch: Self = Self(3);
}
impl windows_core::imp::TypeKind for HorizontalAlignment {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for HorizontalAlignment {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.HorizontalAlignment;i4)");
}
windows_core::imp::define_interface!(
    IAccessibilitySettings,
    IAccessibilitySettings_Vtbl,
    0xfe0e8147_c4c0_4562_b962_1327b52ad5b9
);
impl windows_core::RuntimeType for IAccessibilitySettings {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAccessibilitySettings {
    pub fn HighContrast(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).HighContrast)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IAccessibilitySettings_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub HighContrast: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IAppBar, IAppBar_Vtbl, 0x3d8c2927_5ac5_51bb_8bec_13ff4c1bd6c8);
impl windows_core::RuntimeType for IAppBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAppBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IAppBarElementContainer,
    IAppBarElementContainer_Vtbl,
    0x55d90715_70bc_5b2b_bd12_a940297f935f
);
impl windows_core::RuntimeType for IAppBarElementContainer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAppBarElementContainer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IAppBarElementContainerFactory,
    IAppBarElementContainerFactory_Vtbl,
    0x0664bf62_1fab_5873_9608_117c3b4483ff
);
impl windows_core::RuntimeType for IAppBarElementContainerFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAppBarElementContainerFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IAppWindow, IAppWindow_Vtbl, 0xcfa788b3_643b_5c5e_ad4e_321d48a82acd);
impl windows_core::RuntimeType for IAppWindow {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindow {
    pub fn Id(&self) -> windows_core::Result<WindowId> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Id)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsShownInSwitchers(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsShownInSwitchers)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn Position(&self) -> windows_core::Result<PointInt32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Presenter(&self) -> windows_core::Result<AppWindowPresenter> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Presenter)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn Size(&self) -> windows_core::Result<SizeInt32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Size)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn TitleBar(&self) -> windows_core::Result<AppWindowTitleBar> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TitleBar)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn Move(&self, position: PointInt32) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Move)(windows_core::Interface::as_raw(self), position).ok() }
    }
    pub fn SetIcon(&self, iconpath: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIcon)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(iconpath)),
            )
            .ok()
        }
    }
    pub fn SetIconWithIconId(&self, iconid: IconId) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIconWithIconId)(windows_core::Interface::as_raw(self), iconid)
                .ok()
        }
    }
    pub fn SetPresenter<P0>(&self, appwindowpresenter: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<AppWindowPresenter>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetPresenter)(
                windows_core::Interface::as_raw(self),
                appwindowpresenter.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetPresenterByKind(&self, appwindowpresenterkind: AppWindowPresenterKind) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPresenterByKind)(
                windows_core::Interface::as_raw(self),
                appwindowpresenterkind,
            )
            .ok()
        }
    }
    pub fn Show(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Show)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn Changed<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<AppWindow>, windows_core::Ref<AppWindowChangedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<AppWindow, AppWindowChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<TypedEventHandler<AppWindow, AppWindowChangedEventArgs>, F>::new(
                &TypedEventHandlerBox::<AppWindow, AppWindowChangedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Changed)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveChanged,
            ))
        }
    }
    pub fn Closing<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<AppWindow>, windows_core::Ref<AppWindowClosingEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<AppWindow, AppWindowClosingEventArgs> = {
            let com = windows_core::imp::DelegateBox::<TypedEventHandler<AppWindow, AppWindowClosingEventArgs>, F>::new(
                &TypedEventHandlerBox::<AppWindow, AppWindowClosingEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Closing)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveClosing,
            ))
        }
    }
}
#[repr(C)]
pub struct IAppWindow_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Id: unsafe extern "system" fn(*mut core::ffi::c_void, *mut WindowId) -> windows_core::HRESULT,
    IsShownInSwitchers: usize,
    pub SetIsShownInSwitchers: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsVisible: usize,
    OwnerWindowId: usize,
    pub Position: unsafe extern "system" fn(*mut core::ffi::c_void, *mut PointInt32) -> windows_core::HRESULT,
    pub Presenter:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Size: unsafe extern "system" fn(*mut core::ffi::c_void, *mut SizeInt32) -> windows_core::HRESULT,
    Title: usize,
    SetTitle: usize,
    pub TitleBar:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    Destroy: usize,
    Hide: usize,
    pub Move: unsafe extern "system" fn(*mut core::ffi::c_void, PointInt32) -> windows_core::HRESULT,
    MoveAndResize: usize,
    MoveAndResizeRelativeToDisplayArea: usize,
    Resize: usize,
    pub SetIcon: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetIconWithIconId: unsafe extern "system" fn(*mut core::ffi::c_void, IconId) -> windows_core::HRESULT,
    pub SetPresenter:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPresenterByKind:
        unsafe extern "system" fn(*mut core::ffi::c_void, AppWindowPresenterKind) -> windows_core::HRESULT,
    pub Show: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    ShowWithActivation: usize,
    pub Changed:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Closing:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveClosing: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IAppWindow2, IAppWindow2_Vtbl, 0x6cd41292_794c_5cac_8961_210d012c6ebc);
impl windows_core::RuntimeType for IAppWindow2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindow2 {
    pub fn ClientSize(&self) -> windows_core::Result<SizeInt32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ClientSize)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn ResizeClient(&self, size: SizeInt32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).ResizeClient)(windows_core::Interface::as_raw(self), size).ok()
        }
    }
}
#[repr(C)]
pub struct IAppWindow2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ClientSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut SizeInt32) -> windows_core::HRESULT,
    MoveInZOrderAtBottom: usize,
    MoveInZOrderAtTop: usize,
    MoveInZOrderBelow: usize,
    pub ResizeClient: unsafe extern "system" fn(*mut core::ffi::c_void, SizeInt32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAppWindowChangedEventArgs,
    IAppWindowChangedEventArgs_Vtbl,
    0x2182bc5d_fdac_5c3e_bf37_7d8d684e9d1d
);
impl windows_core::RuntimeType for IAppWindowChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindowChangedEventArgs {
    pub fn DidPositionChange(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DidPositionChange)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn DidPresenterChange(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DidPresenterChange)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn DidSizeChange(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DidSizeChange)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IAppWindowChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub DidPositionChange: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub DidPresenterChange: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub DidSizeChange: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAppWindowClosingEventArgs,
    IAppWindowClosingEventArgs_Vtbl,
    0x0e09d90b_2261_590b_9ad1_8504991d8754
);
impl windows_core::RuntimeType for IAppWindowClosingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindowClosingEventArgs {
    pub fn SetCancel(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetCancel)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IAppWindowClosingEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Cancel: usize,
    pub SetCancel: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAppWindowPresenter,
    IAppWindowPresenter_Vtbl,
    0xbc3042c2_c6c6_5632_8989_ff0ec6d3b40d
);
impl windows_core::RuntimeType for IAppWindowPresenter {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindowPresenter {
    pub fn Kind(&self) -> windows_core::Result<AppWindowPresenterKind> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Kind)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IAppWindowPresenter_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Kind: unsafe extern "system" fn(*mut core::ffi::c_void, *mut AppWindowPresenterKind) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAppWindowTitleBar,
    IAppWindowTitleBar_Vtbl,
    0x5574efa2_c91c_5700_a363_539c71a7aaf4
);
impl windows_core::RuntimeType for IAppWindowTitleBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAppWindowTitleBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IAppWindowTitleBar2,
    IAppWindowTitleBar2_Vtbl,
    0x86faed38_748a_5b4b_9ccf_3ba0496c9041
);
impl windows_core::RuntimeType for IAppWindowTitleBar2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindowTitleBar2 {
    pub fn SetPreferredHeightOption(&self, value: TitleBarHeightOption) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPreferredHeightOption)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IAppWindowTitleBar2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PreferredHeightOption: usize,
    pub SetPreferredHeightOption:
        unsafe extern "system" fn(*mut core::ffi::c_void, TitleBarHeightOption) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAppWindowTitleBar3,
    IAppWindowTitleBar3_Vtbl,
    0x07146e74_0410_5597_aba7_1af276d2ae07
);
impl windows_core::RuntimeType for IAppWindowTitleBar3 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAppWindowTitleBar3 {
    pub fn SetPreferredTheme(&self, value: TitleBarTheme) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPreferredTheme)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IAppWindowTitleBar3_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PreferredTheme: usize,
    pub SetPreferredTheme: unsafe extern "system" fn(*mut core::ffi::c_void, TitleBarTheme) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IApplication, IApplication_Vtbl, 0x06a8f4e7_1146_55af_820d_ebd55643b021);
impl windows_core::RuntimeType for IApplication {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IApplication {
    pub fn Resources(&self) -> windows_core::Result<ResourceDictionary> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Resources)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn RequestedTheme(&self) -> windows_core::Result<ApplicationTheme> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestedTheme)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IApplication_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Resources:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    SetResources: usize,
    DebugSettings: usize,
    pub RequestedTheme:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut ApplicationTheme) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IApplicationFactory,
    IApplicationFactory_Vtbl,
    0x9fd96657_5294_5a65_a1db_4fea143597da
);
impl windows_core::RuntimeType for IApplicationFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IApplicationFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IApplicationOverrides,
    IApplicationOverrides_Vtbl,
    0xa33e81ef_c665_503b_8827_d27ef1720a06
);
impl windows_core::RuntimeType for IApplicationOverrides {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.IApplicationOverrides");
}
impl windows_core::RuntimeName for IApplicationOverrides {
    const NAME: &'static str = "Microsoft.UI.Xaml.IApplicationOverrides";
}
pub trait IApplicationOverrides_Impl: windows_core::IUnknownImpl {
    fn OnLaunched(&self, args: windows_core::Ref<LaunchActivatedEventArgs>) -> windows_core::Result<()>;
}
impl IApplicationOverrides_Vtbl {
    pub const fn new<Identity: IApplicationOverrides_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn OnLaunched<Identity: IApplicationOverrides_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IApplicationOverrides_Impl::OnLaunched(this, core::mem::transmute_copy(&args)).into()
            }
        }
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<Identity, IApplicationOverrides, OFFSET>(),
            OnLaunched: OnLaunched::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IApplicationOverrides as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IApplicationOverrides_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub OnLaunched: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IApplicationStatics,
    IApplicationStatics_Vtbl,
    0x4e0d09f5_4358_512c_a987_503b52848e95
);
impl windows_core::RuntimeType for IApplicationStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IApplicationStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Current:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IAutoSuggestBox, IAutoSuggestBox_Vtbl, 0x3eea809e_b2db_521d_97db_e0648fb5d798);
impl windows_core::RuntimeType for IAutoSuggestBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAutoSuggestBox {
    pub fn Text(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Text)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn PlaceholderText(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PlaceholderText)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| {
                let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                hstring.to_string_lossy()
            })
        }
    }
    pub fn SetPlaceholderText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPlaceholderText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetHeader<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetHeader)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn QueryIcon(&self) -> windows_core::Result<IconElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).QueryIcon)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetQueryIcon<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<IconElement>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetQueryIcon)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn TextChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<AutoSuggestBox>, windows_core::Ref<AutoSuggestBoxTextChangedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<AutoSuggestBox, AutoSuggestBoxTextChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<AutoSuggestBox, AutoSuggestBoxTextChangedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<AutoSuggestBox, AutoSuggestBoxTextChangedEventArgs, F>::VTABLE, handler
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).TextChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveTextChanged,
            ))
        }
    }
    pub fn QuerySubmitted<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<AutoSuggestBox>, windows_core::Ref<AutoSuggestBoxQuerySubmittedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<AutoSuggestBox, AutoSuggestBoxQuerySubmittedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<AutoSuggestBox, AutoSuggestBoxQuerySubmittedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<AutoSuggestBox, AutoSuggestBoxQuerySubmittedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).QuerySubmitted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveQuerySubmitted,
            ))
        }
    }
}
#[repr(C)]
pub struct IAutoSuggestBox_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    MaxSuggestionListHeight: usize,
    SetMaxSuggestionListHeight: usize,
    IsSuggestionListOpen: usize,
    SetIsSuggestionListOpen: usize,
    TextMemberPath: usize,
    SetTextMemberPath: usize,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    UpdateTextOnSelect: usize,
    SetUpdateTextOnSelect: usize,
    pub PlaceholderText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPlaceholderText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Header: usize,
    pub SetHeader: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    AutoMaximizeSuggestionArea: usize,
    SetAutoMaximizeSuggestionArea: usize,
    TextBoxStyle: usize,
    SetTextBoxStyle: usize,
    pub QueryIcon:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetQueryIcon:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    LightDismissOverlayMode: usize,
    SetLightDismissOverlayMode: usize,
    Description: usize,
    SetDescription: usize,
    SuggestionChosen: usize,
    RemoveSuggestionChosen: usize,
    pub TextChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveTextChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub QuerySubmitted:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveQuerySubmitted: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAutoSuggestBoxQuerySubmittedEventArgs,
    IAutoSuggestBoxQuerySubmittedEventArgs_Vtbl,
    0x26da5de4_57a6_57bf_acc9_aac599c0b22b
);
impl windows_core::RuntimeType for IAutoSuggestBoxQuerySubmittedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAutoSuggestBoxQuerySubmittedEventArgs {
    pub fn QueryText(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).QueryText)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
}
#[repr(C)]
pub struct IAutoSuggestBoxQuerySubmittedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub QueryText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAutoSuggestBoxTextChangedEventArgs,
    IAutoSuggestBoxTextChangedEventArgs_Vtbl,
    0xd7191d84_e886_547f_a3e2_12f0e05b20fa
);
impl windows_core::RuntimeType for IAutoSuggestBoxTextChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAutoSuggestBoxTextChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IAutomationPeer, IAutomationPeer_Vtbl, 0xe51d3e4e_34f0_568c_999f_6277e2afe6d7);
impl windows_core::RuntimeType for IAutomationPeer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IAutomationPeer {
    pub fn GetPattern(&self, patterninterface: PatternInterface) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetPattern)(
                windows_core::Interface::as_raw(self),
                patterninterface,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IAutomationPeer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    EventsSource: usize,
    SetEventsSource: usize,
    pub GetPattern: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        PatternInterface,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IAutomationProperties,
    IAutomationProperties_Vtbl,
    0x525c6a71_dd8a_52a0_977b_db1b02f8e896
);
impl windows_core::RuntimeType for IAutomationProperties {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAutomationProperties_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IAutomationPropertiesStatics,
    IAutomationPropertiesStatics_Vtbl,
    0xb1e3e0f3_112f_5966_87dc_7862d4ad50e5
);
impl windows_core::RuntimeType for IAutomationPropertiesStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAutomationPropertiesStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AcceleratorKeyProperty: usize,
    GetAcceleratorKey: usize,
    SetAcceleratorKey: usize,
    AccessKeyProperty: usize,
    GetAccessKey: usize,
    SetAccessKey: usize,
    AutomationIdProperty: usize,
    GetAutomationId: usize,
    SetAutomationId: usize,
    HelpTextProperty: usize,
    GetHelpText: usize,
    pub SetHelpText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    IsRequiredForFormProperty: usize,
    GetIsRequiredForForm: usize,
    SetIsRequiredForForm: usize,
    ItemStatusProperty: usize,
    GetItemStatus: usize,
    SetItemStatus: usize,
    ItemTypeProperty: usize,
    GetItemType: usize,
    SetItemType: usize,
    LabeledByProperty: usize,
    GetLabeledBy: usize,
    SetLabeledBy: usize,
    NameProperty: usize,
    pub GetName: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetName: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    LiveSettingProperty: usize,
    GetLiveSetting: usize,
    SetLiveSetting: usize,
    AccessibilityViewProperty: usize,
    GetAccessibilityView: usize,
    pub SetAccessibilityView: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        AccessibilityView,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IBindCtx, IBindCtx_Vtbl, 0x0000000e_0000_0000_c000_000000000046);
windows_core::imp::interface_hierarchy!(IBindCtx, windows_core::IUnknown);
#[repr(C)]
pub struct IBindCtx_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    RegisterObjectBound: usize,
    RevokeObjectBound: usize,
    ReleaseBoundObjects: usize,
    SetBindOptions: usize,
    GetBindOptions: usize,
    GetRunningObjectTable: usize,
    RegisterObjectParam: usize,
    GetObjectParam: usize,
    EnumObjectParam: usize,
    RevokeObjectParam: usize,
}
windows_core::imp::define_interface!(IBitmapImage, IBitmapImage_Vtbl, 0x5cc29916_a411_5bc2_a3c5_a00d99a59da8);
impl windows_core::RuntimeType for IBitmapImage {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IBitmapImage_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IBitmapImageFactory,
    IBitmapImageFactory_Vtbl,
    0xf037e0e9_f229_522e_95c9_da2211a14b05
);
impl windows_core::RuntimeType for IBitmapImageFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IBitmapImageFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstanceWithUriSource: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IBitmapSource, IBitmapSource_Vtbl, 0x8424269d_9b82_534f_8fea_af5b5ef96bf2);
impl windows_core::RuntimeType for IBitmapSource {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IBitmapSource {
    pub fn PixelWidth(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PixelWidth)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn PixelHeight(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PixelHeight)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IBitmapSource_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PixelWidth: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub PixelHeight: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IBorder, IBorder_Vtbl, 0x1ca13b47_ff5c_5abc_a411_a177df9482a9);
impl windows_core::RuntimeType for IBorder {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IBorder {
    pub fn CornerRadius(&self) -> windows_core::Result<CornerRadius> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CornerRadius)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetCornerRadius(&self, value: CornerRadius) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCornerRadius)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Child(&self) -> windows_core::Result<UIElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Child)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetChild<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<UIElement>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetChild)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
}
#[repr(C)]
pub struct IBorder_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BorderBrush: usize,
    SetBorderBrush: usize,
    BorderThickness: usize,
    SetBorderThickness: usize,
    Background: usize,
    SetBackground: usize,
    BackgroundSizing: usize,
    SetBackgroundSizing: usize,
    pub CornerRadius: unsafe extern "system" fn(*mut core::ffi::c_void, *mut CornerRadius) -> windows_core::HRESULT,
    pub SetCornerRadius: unsafe extern "system" fn(*mut core::ffi::c_void, CornerRadius) -> windows_core::HRESULT,
    Padding: usize,
    SetPadding: usize,
    pub Child: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetChild: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IBreadcrumbBar, IBreadcrumbBar_Vtbl, 0x2e47b7d6_5fbd_54c7_b0b1_ceff4a19c744);
impl windows_core::RuntimeType for IBreadcrumbBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IBreadcrumbBar {
    pub fn ItemsSource(&self) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ItemsSource)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetItemsSource<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetItemsSource)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ItemClicked<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<BreadcrumbBar>, windows_core::Ref<BreadcrumbBarItemClickedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<BreadcrumbBar, BreadcrumbBarItemClickedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<BreadcrumbBar, BreadcrumbBarItemClickedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<BreadcrumbBar, BreadcrumbBarItemClickedEventArgs, F>::VTABLE, handler
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ItemClicked)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveItemClicked,
            ))
        }
    }
}
#[repr(C)]
pub struct IBreadcrumbBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ItemsSource:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetItemsSource:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    ItemTemplate: usize,
    SetItemTemplate: usize,
    pub ItemClicked:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveItemClicked: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IBreadcrumbBarFactory,
    IBreadcrumbBarFactory_Vtbl,
    0xd5b6a6d9_3148_5cbc_a6ae_0f44cde41952
);
impl windows_core::RuntimeType for IBreadcrumbBarFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IBreadcrumbBarFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IBreadcrumbBarItemClickedEventArgs,
    IBreadcrumbBarItemClickedEventArgs_Vtbl,
    0x1ceea503_365e_580d_bcd4_e9ad0248f6b5
);
impl windows_core::RuntimeType for IBreadcrumbBarItemClickedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IBreadcrumbBarItemClickedEventArgs {
    pub fn Index(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Index)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IBreadcrumbBarItemClickedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Index: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IBrush, IBrush_Vtbl, 0x2de3cb83_1329_5679_88f8_c822bc5442cb);
impl windows_core::RuntimeType for IBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IBuffer, IBuffer_Vtbl, 0x905a0fe0_bc53_11df_8c49_001e4fc686da);
impl windows_core::RuntimeType for IBuffer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IBuffer, windows_core::IUnknown, windows_core::IInspectable);
impl IBuffer {
    pub fn Length(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Length)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IBuffer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Capacity: usize,
    pub Length: unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IBufferByteAccess, IBufferByteAccess_Vtbl, 0x905a0fef_bc53_11df_8c49_001e4fc686da);
windows_core::imp::interface_hierarchy!(IBufferByteAccess, windows_core::IUnknown);
impl IBufferByteAccess {
    pub unsafe fn Buffer(&self) -> windows_core::Result<*mut byte> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Buffer)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IBufferByteAccess_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Buffer: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut byte) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IButton, IButton_Vtbl, 0x216c183d_d07a_5aa5_b8a4_0300a2683e87);
impl windows_core::RuntimeType for IButton {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IButton {
    pub fn Flyout(&self) -> windows_core::Result<FlyoutBase> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Flyout)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetFlyout<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<FlyoutBase>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFlyout)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IButton_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Flyout: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetFlyout: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IButtonBase, IButtonBase_Vtbl, 0x65714269_2473_5327_a652_0ea6bce7f403);
impl windows_core::RuntimeType for IButtonBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IButtonBase {
    pub fn Click<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Click)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveClick,
            ))
        }
    }
}
#[repr(C)]
pub struct IButtonBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ClickMode: usize,
    SetClickMode: usize,
    IsPointerOver: usize,
    IsPressed: usize,
    Command: usize,
    SetCommand: usize,
    CommandParameter: usize,
    SetCommandParameter: usize,
    pub Click:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveClick: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IButtonFactory, IButtonFactory_Vtbl, 0xfe393422_d91c_57b1_9a9c_2c7e3f41f77c);
impl windows_core::RuntimeType for IButtonFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IButtonFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ICONINFO {
    pub fIcon: windows_core::BOOL,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: HBITMAP,
    pub hbmColor: HBITMAP,
}
pub const ICON_BIG: i32 = 1;
windows_core::imp::define_interface!(ICanvas, ICanvas_Vtbl, 0x457ba139_1146_51d2_807e_d9d65c927060);
impl windows_core::RuntimeType for ICanvas {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICanvas_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(ICanvasFactory, ICanvasFactory_Vtbl, 0x374c5050_3481_5557_9948_804c0b8eea89);
impl windows_core::RuntimeType for ICanvasFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICanvasFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ICanvasStatics, ICanvasStatics_Vtbl, 0xc00d5e0f_77e3_5c59_8fcd_86761f0c6607);
impl windows_core::RuntimeType for ICanvasStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICanvasStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    LeftProperty: usize,
    pub GetLeft:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetLeft:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    TopProperty: usize,
    pub GetTop:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetTop: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ICheckBox, ICheckBox_Vtbl, 0xc5830000_4c9d_5fdd_9346_674c71cd80c5);
impl windows_core::RuntimeType for ICheckBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICheckBox_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(ICheckBoxFactory, ICheckBoxFactory_Vtbl, 0xf43ff58d_31d5_5835_af7b_375bc6a9bcf3);
impl windows_core::RuntimeType for ICheckBoxFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICheckBoxFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IClipboardStatics, IClipboardStatics_Vtbl, 0xc627e291_34e2_4963_8eed_93cbb0ea3d70);
impl windows_core::RuntimeType for IClipboardStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IClipboardStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetContent:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetContent: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Flush: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IComboBox, IComboBox_Vtbl, 0xc77da58b_4fd7_51e0_a431_f84658a83e9e);
impl windows_core::RuntimeType for IComboBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IComboBox {
    pub fn SetHeader<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetHeader)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IComboBox_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsDropDownOpen: usize,
    SetIsDropDownOpen: usize,
    IsEditable: usize,
    SetIsEditable: usize,
    IsSelectionBoxHighlighted: usize,
    MaxDropDownHeight: usize,
    SetMaxDropDownHeight: usize,
    SelectionBoxItem: usize,
    SelectionBoxItemTemplate: usize,
    TemplateSettings: usize,
    Header: usize,
    pub SetHeader: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IComboBoxFactory, IComboBoxFactory_Vtbl, 0x71c1014b_acdf_5c03_b5ed_02871caaeb6b);
impl windows_core::RuntimeType for IComboBoxFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IComboBoxFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IComboBoxItem, IComboBoxItem_Vtbl, 0xdf65f6f4_863f_558d_bc4e_82c12f28062f);
impl windows_core::RuntimeType for IComboBoxItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IComboBoxItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IComboBoxItemFactory,
    IComboBoxItemFactory_Vtbl,
    0x8540b906_c96b_592d_88d7_0c23a3386d93
);
impl windows_core::RuntimeType for IComboBoxItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IComboBoxItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ICommandBar, ICommandBar_Vtbl, 0xb7ca8ee3_a07a_5f69_8ab8_be4e3e4cf0c8);
impl windows_core::RuntimeType for ICommandBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICommandBar {
    pub fn PrimaryCommands(&self) -> windows_core::Result<windows_collections::IObservableVector<ICommandBarElement>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PrimaryCommands)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICommandBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PrimaryCommands:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICommandBarElement,
    ICommandBarElement_Vtbl,
    0xf8eb20b4_373e_5327_9942_66a1ea21f5f9
);
impl windows_core::RuntimeType for ICommandBarElement {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(ICommandBarElement, windows_core::IUnknown, windows_core::IInspectable);
impl ICommandBarElement {
    pub fn IsInOverflow(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInOverflow)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ICommandBarElement_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsCompact: usize,
    SetIsCompact: usize,
    pub IsInOverflow: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICommandBarFactory,
    ICommandBarFactory_Vtbl,
    0x8d4079c3_fa0a_5bb1_b45d_499c378761b4
);
impl windows_core::RuntimeType for ICommandBarFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICommandBarFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionTarget,
    ICompositionTarget_Vtbl,
    0x7d938324_e3ad_597c_93f6_520725410e68
);
impl windows_core::RuntimeType for ICompositionTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionTarget_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionTargetStatics,
    ICompositionTargetStatics_Vtbl,
    0x12a4be6f_6db1_5165_b622_d57ab782745b
);
impl windows_core::RuntimeType for ICompositionTargetStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionTargetStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Rendering:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveRendering: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IContainerContentChangingEventArgs,
    IContainerContentChangingEventArgs_Vtbl,
    0xf4c8c937_b070_53ce_a76c_074ee5750a71
);
impl windows_core::RuntimeType for IContainerContentChangingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IContainerContentChangingEventArgs {
    pub fn ItemContainer(&self) -> windows_core::Result<SelectorItem> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ItemContainer)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn InRecycleQueue(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).InRecycleQueue)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Item(&self) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Item)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetHandled)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IContainerContentChangingEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ItemContainer:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub InRecycleQueue: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    ItemIndex: usize,
    pub Item: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    Phase: usize,
    Handled: usize,
    pub SetHandled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IContentControl, IContentControl_Vtbl, 0x07e81761_11b2_52ae_8f8b_4d53d2b5900a);
impl windows_core::RuntimeType for IContentControl {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IContentControl {
    pub fn Content(&self) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Content)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetContent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ContentTemplateRoot(&self) -> windows_core::Result<UIElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContentTemplateRoot)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IContentControl_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Content:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetContent: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    ContentTemplate: usize,
    SetContentTemplate: usize,
    ContentTemplateSelector: usize,
    SetContentTemplateSelector: usize,
    ContentTransitions: usize,
    SetContentTransitions: usize,
    pub ContentTemplateRoot:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IContentDialog, IContentDialog_Vtbl, 0xac2145a3_4a32_5305_a81d_47509515bfce);
impl windows_core::RuntimeType for IContentDialog {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IContentDialog {
    pub fn SetTitle<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetTitle)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
    pub fn SetPrimaryButtonText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPrimaryButtonText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetSecondaryButtonText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSecondaryButtonText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetCloseButtonText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCloseButtonText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetDefaultButton(&self, value: ContentDialogButton) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDefaultButton)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn ShowAsync(&self) -> windows_core::Result<windows_future::IAsyncOperation<ContentDialogResult>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ShowAsync)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IContentDialog_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Title: usize,
    pub SetTitle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    TitleTemplate: usize,
    SetTitleTemplate: usize,
    FullSizeDesired: usize,
    SetFullSizeDesired: usize,
    PrimaryButtonText: usize,
    pub SetPrimaryButtonText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    SecondaryButtonText: usize,
    pub SetSecondaryButtonText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    CloseButtonText: usize,
    pub SetCloseButtonText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    PrimaryButtonCommand: usize,
    SetPrimaryButtonCommand: usize,
    SecondaryButtonCommand: usize,
    SetSecondaryButtonCommand: usize,
    CloseButtonCommand: usize,
    SetCloseButtonCommand: usize,
    PrimaryButtonCommandParameter: usize,
    SetPrimaryButtonCommandParameter: usize,
    SecondaryButtonCommandParameter: usize,
    SetSecondaryButtonCommandParameter: usize,
    CloseButtonCommandParameter: usize,
    SetCloseButtonCommandParameter: usize,
    IsPrimaryButtonEnabled: usize,
    SetIsPrimaryButtonEnabled: usize,
    IsSecondaryButtonEnabled: usize,
    SetIsSecondaryButtonEnabled: usize,
    PrimaryButtonStyle: usize,
    SetPrimaryButtonStyle: usize,
    SecondaryButtonStyle: usize,
    SetSecondaryButtonStyle: usize,
    CloseButtonStyle: usize,
    SetCloseButtonStyle: usize,
    DefaultButton: usize,
    pub SetDefaultButton:
        unsafe extern "system" fn(*mut core::ffi::c_void, ContentDialogButton) -> windows_core::HRESULT,
    Closing: usize,
    RemoveClosing: usize,
    Closed: usize,
    RemoveClosed: usize,
    Opened: usize,
    RemoveOpened: usize,
    PrimaryButtonClick: usize,
    RemovePrimaryButtonClick: usize,
    SecondaryButtonClick: usize,
    RemoveSecondaryButtonClick: usize,
    CloseButtonClick: usize,
    RemoveCloseButtonClick: usize,
    Hide: usize,
    pub ShowAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IContentDialogFactory,
    IContentDialogFactory_Vtbl,
    0xa05b3ad7_c60e_545a_9ee4_f098220ed816
);
impl windows_core::RuntimeType for IContentDialogFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IContentDialogFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IContentIsland, IContentIsland_Vtbl, 0x5b2504ba_361c_50aa_bd6e_4122c6d93889);
impl windows_core::RuntimeType for IContentIsland {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IContentIsland_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IControl, IControl_Vtbl, 0x857d6e8a_d45a_5c69_a99c_bf6a5c54fb38);
impl windows_core::RuntimeType for IControl {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IControl {
    pub fn FontSize(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FontSize)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontSize)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn FontFamily(&self) -> windows_core::Result<FontFamily> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FontFamily)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetFontFamily<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<FontFamily>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontFamily)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetFontWeight(&self, value: FontWeight) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontWeight)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn IsEnabled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsEnabled)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsEnabled)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetVerticalContentAlignment(&self, value: VerticalAlignment) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalContentAlignment)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetBackground<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetBackground)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetBorderThickness(&self, value: Thickness) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBorderThickness)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn CornerRadius(&self) -> windows_core::Result<CornerRadius> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CornerRadius)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetCornerRadius(&self, value: CornerRadius) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCornerRadius)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IControl_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsFocusEngagementEnabled: usize,
    SetIsFocusEngagementEnabled: usize,
    IsFocusEngaged: usize,
    SetIsFocusEngaged: usize,
    RequiresPointer: usize,
    SetRequiresPointer: usize,
    pub FontSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetFontSize: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub FontFamily:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetFontFamily:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    FontWeight: usize,
    pub SetFontWeight: unsafe extern "system" fn(*mut core::ffi::c_void, FontWeight) -> windows_core::HRESULT,
    FontStyle: usize,
    SetFontStyle: usize,
    FontStretch: usize,
    SetFontStretch: usize,
    CharacterSpacing: usize,
    SetCharacterSpacing: usize,
    Foreground: usize,
    SetForeground: usize,
    IsTextScaleFactorEnabled: usize,
    SetIsTextScaleFactorEnabled: usize,
    pub IsEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    TabNavigation: usize,
    SetTabNavigation: usize,
    Template: usize,
    SetTemplate: usize,
    Padding: usize,
    SetPadding: usize,
    HorizontalContentAlignment: usize,
    SetHorizontalContentAlignment: usize,
    VerticalContentAlignment: usize,
    pub SetVerticalContentAlignment:
        unsafe extern "system" fn(*mut core::ffi::c_void, VerticalAlignment) -> windows_core::HRESULT,
    Background: usize,
    pub SetBackground:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    BackgroundSizing: usize,
    SetBackgroundSizing: usize,
    BorderThickness: usize,
    pub SetBorderThickness: unsafe extern "system" fn(*mut core::ffi::c_void, Thickness) -> windows_core::HRESULT,
    BorderBrush: usize,
    SetBorderBrush: usize,
    DefaultStyleResourceUri: usize,
    SetDefaultStyleResourceUri: usize,
    ElementSoundMode: usize,
    SetElementSoundMode: usize,
    pub CornerRadius: unsafe extern "system" fn(*mut core::ffi::c_void, *mut CornerRadius) -> windows_core::HRESULT,
    pub SetCornerRadius: unsafe extern "system" fn(*mut core::ffi::c_void, CornerRadius) -> windows_core::HRESULT,
}
pub const IDC_ARROW: windows_core::PCWSTR = windows_core::PCWSTR(32512 as _);
windows_core::imp::define_interface!(IDataPackage, IDataPackage_Vtbl, 0x61ebf5c7_efea_4346_9554_981d7e198ffe);
impl windows_core::RuntimeType for IDataPackage {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDataPackage {
    pub fn SetRequestedOperation(&self, value: DataPackageOperation) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRequestedOperation)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetDataProvider<P1>(&self, formatid: &str, delayrenderer: P1) -> windows_core::Result<()>
    where
        P1: windows_core::Param<DataProviderHandler>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetDataProvider)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(formatid)),
                delayrenderer.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IDataPackage_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    GetView: usize,
    Properties: usize,
    RequestedOperation: usize,
    pub SetRequestedOperation:
        unsafe extern "system" fn(*mut core::ffi::c_void, DataPackageOperation) -> windows_core::HRESULT,
    OperationCompleted: usize,
    RemoveOperationCompleted: usize,
    Destroyed: usize,
    RemoveDestroyed: usize,
    SetData: usize,
    pub SetDataProvider: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IDataPackageView, IDataPackageView_Vtbl, 0x7b840471_5900_4d85_a90b_10cb85fe3552);
impl windows_core::RuntimeType for IDataPackageView {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDataPackageView {
    pub fn Contains(&self, formatid: &str) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Contains)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(formatid)),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn GetTextAsync(&self) -> windows_core::Result<windows_future::IAsyncOperation<windows_core::HSTRING>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetTextAsync)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn GetStorageItemsAsync(
        &self,
    ) -> windows_core::Result<windows_future::IAsyncOperation<windows_collections::IVectorView<IStorageItem>>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetStorageItemsAsync)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDataPackageView_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Properties: usize,
    RequestedOperation: usize,
    ReportOperationCompleted: usize,
    AvailableFormats: usize,
    pub Contains:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    GetDataAsync: usize,
    pub GetTextAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    GetCustomTextAsync: usize,
    GetUriAsync: usize,
    GetHtmlFormatAsync: usize,
    GetResourceMapAsync: usize,
    GetRtfAsync: usize,
    GetBitmapAsync: usize,
    pub GetStorageItemsAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDataProviderDeferral,
    IDataProviderDeferral_Vtbl,
    0xc2cf2373_2d26_43d9_b69d_dcb86d03f6da
);
impl windows_core::RuntimeType for IDataProviderDeferral {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDataProviderDeferral {
    pub fn Complete(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Complete)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IDataProviderDeferral_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Complete: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDataProviderRequest,
    IDataProviderRequest_Vtbl,
    0xebbc7157_d3c8_47da_acde_f82388d5f716
);
impl windows_core::RuntimeType for IDataProviderRequest {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDataProviderRequest {
    pub fn GetDeferral(&self) -> windows_core::Result<DataProviderDeferral> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDeferral)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetData<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetData)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
}
#[repr(C)]
pub struct IDataProviderRequest_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FormatId: usize,
    Deadline: usize,
    pub GetDeferral:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetData: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IDataReader, IDataReader_Vtbl, 0xe2b50029_b4c1_4314_a4b8_fb813a2f275e);
impl windows_core::RuntimeType for IDataReader {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IDataReader, windows_core::IUnknown, windows_core::IInspectable);
impl IDataReader {
    pub fn ReadBytes(&self, value: &mut [u8]) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).ReadBytes)(
                windows_core::Interface::as_raw(self),
                value.len().try_into().unwrap(),
                value.as_mut_ptr(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IDataReader_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    UnconsumedBufferLength: usize,
    UnicodeEncoding: usize,
    SetUnicodeEncoding: usize,
    ByteOrder: usize,
    SetByteOrder: usize,
    InputStreamOptions: usize,
    SetInputStreamOptions: usize,
    ReadByte: usize,
    pub ReadBytes: unsafe extern "system" fn(*mut core::ffi::c_void, u32, *mut u8) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDataReaderStatics,
    IDataReaderStatics_Vtbl,
    0x11fcbfc8_f93a_471b_b121_f379e349313c
);
impl windows_core::RuntimeType for IDataReaderStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDataReaderStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub FromBuffer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IDataTemplate, IDataTemplate_Vtbl, 0x08fa70fa_ee75_5e92_a101_f52d0e1e9fab);
impl windows_core::RuntimeType for IDataTemplate {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDataTemplate_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IDependencyObject, IDependencyObject_Vtbl, 0xe7beaee7_160e_50f7_8789_d63463f979fa);
impl windows_core::RuntimeType for IDependencyObject {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDependencyObject {
    pub fn RegisterPropertyChangedCallback<P0, P1>(&self, dp: P0, callback: P1) -> windows_core::Result<i64>
    where
        P0: windows_core::Param<DependencyProperty>,
        P1: windows_core::Param<DependencyPropertyChangedCallback>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RegisterPropertyChangedCallback)(
                windows_core::Interface::as_raw(self),
                dp.param().abi(),
                callback.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDependencyObject_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    GetValue: usize,
    SetValue: usize,
    ClearValue: usize,
    ReadLocalValue: usize,
    GetAnimationBaseValue: usize,
    pub RegisterPropertyChangedCallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDependencyProperty,
    IDependencyProperty_Vtbl,
    0x960eab49_9672_58a0_995b_3a42e5ea6278
);
impl windows_core::RuntimeType for IDependencyProperty {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDependencyProperty_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IDispatcherQueue, IDispatcherQueue_Vtbl, 0xf6ebf8fa_be1c_5bf6_a467_73da28738ae8);
impl windows_core::RuntimeType for IDispatcherQueue {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDispatcherQueue {
    pub fn CreateTimer(&self) -> windows_core::Result<DispatcherQueueTimer> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateTimer)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn TryEnqueue<P0>(&self, callback: P0) -> windows_core::Result<bool>
    where
        P0: windows_core::Param<DispatcherQueueHandler>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryEnqueue)(
                windows_core::Interface::as_raw(self),
                callback.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDispatcherQueue_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateTimer:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub TryEnqueue:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IDispatcherQueue3, IDispatcherQueue3_Vtbl, 0x14a7a175_5c27_5a35_b079_21960cf764a8);
impl windows_core::RuntimeType for IDispatcherQueue3 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDispatcherQueue3 {
    pub fn EnqueueEventLoopExit(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).EnqueueEventLoopExit)(windows_core::Interface::as_raw(self)).ok()
        }
    }
    pub fn RunEventLoop(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).RunEventLoop)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IDispatcherQueue3_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub EnqueueEventLoopExit: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    EnsureSystemDispatcherQueue: usize,
    pub RunEventLoop: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDispatcherQueueController,
    IDispatcherQueueController_Vtbl,
    0xbce8178d_2183_584c_9e5b_f9366f6ae484
);
impl windows_core::RuntimeType for IDispatcherQueueController {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDispatcherQueueController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IDispatcherQueueControllerStatics,
    IDispatcherQueueControllerStatics_Vtbl,
    0xf18d6145_722b_593d_bcf2_a61e713f0037
);
impl windows_core::RuntimeType for IDispatcherQueueControllerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDispatcherQueueControllerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateOnDedicatedThread: usize,
    pub CreateOnCurrentThread:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDispatcherQueueStatics,
    IDispatcherQueueStatics_Vtbl,
    0xcd3382ea_a455_5124_b63a_ca40d34ca23c
);
impl windows_core::RuntimeType for IDispatcherQueueStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDispatcherQueueStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetForCurrentThread:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDispatcherQueueTimer,
    IDispatcherQueueTimer_Vtbl,
    0xad4d63fd_88fe_541f_ac11_bf2dc1ed2ce5
);
impl windows_core::RuntimeType for IDispatcherQueueTimer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDispatcherQueueTimer {
    pub fn SetInterval(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInterval)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetIsRepeating(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsRepeating)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Start(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Start)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn Stop(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Stop)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn Tick<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<DispatcherQueueTimer>, windows_core::Ref<windows_core::IInspectable>) + 'static,
    {
        let handler: TypedEventHandler<DispatcherQueueTimer, windows_core::IInspectable> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<DispatcherQueueTimer, windows_core::IInspectable>,
                F,
            >::new(
                &TypedEventHandlerBox::<DispatcherQueueTimer, windows_core::IInspectable, F>::VTABLE, handler
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Tick)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(self.clone(), token__, windows_core::Interface::vtable(self).RemoveTick))
        }
    }
}
#[repr(C)]
pub struct IDispatcherQueueTimer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Interval: usize,
    pub SetInterval: unsafe extern "system" fn(*mut core::ffi::c_void, windows_time::TimeSpan) -> windows_core::HRESULT,
    IsRunning: usize,
    IsRepeating: usize,
    pub SetIsRepeating: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub Start: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Stop: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Tick:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveTick: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDoubleTappedRoutedEventArgs,
    IDoubleTappedRoutedEventArgs_Vtbl,
    0x32b9549d_11d8_53a5_a953_02409537a11f
);
impl windows_core::RuntimeType for IDoubleTappedRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDoubleTappedRoutedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IDragEventArgs, IDragEventArgs_Vtbl, 0x47ac5757_e4bc_52ba_8ab9_1bf81aad7900);
impl windows_core::RuntimeType for IDragEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDragEventArgs {
    pub fn Handled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Handled)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetHandled)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn DataView(&self) -> windows_core::Result<DataPackageView> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DataView)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn AcceptedOperation(&self) -> windows_core::Result<DataPackageOperation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AcceptedOperation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetAcceptedOperation(&self, value: DataPackageOperation) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetAcceptedOperation)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
}
#[repr(C)]
pub struct IDragEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Handled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetHandled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    Data: usize,
    SetData: usize,
    pub DataView:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    DragUIOverride: usize,
    Modifiers: usize,
    pub AcceptedOperation:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut DataPackageOperation) -> windows_core::HRESULT,
    pub SetAcceptedOperation:
        unsafe extern "system" fn(*mut core::ffi::c_void, DataPackageOperation) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDragItemsStartingEventArgs,
    IDragItemsStartingEventArgs_Vtbl,
    0xa6938886_20df_558e_ac74_bb5b7f2f7e90
);
impl windows_core::RuntimeType for IDragItemsStartingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDragItemsStartingEventArgs {
    pub fn SetCancel(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetCancel)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<windows_core::IInspectable>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn Data(&self) -> windows_core::Result<DataPackage> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Data)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDragItemsStartingEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Cancel: usize,
    pub SetCancel: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Data: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IDropDownButton, IDropDownButton_Vtbl, 0xc1e9fa91_4f95_5796_8a7b_3b7594a12c69);
impl windows_core::RuntimeType for IDropDownButton {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDropDownButton_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IDropDownButtonFactory,
    IDropDownButtonFactory_Vtbl,
    0x7cf3e13b_668d_57e7_b5d6_f5ca3dbc80bd
);
impl windows_core::RuntimeType for IDropDownButtonFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDropDownButtonFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IExceptionRoutedEventArgs,
    IExceptionRoutedEventArgs_Vtbl,
    0xe8bcb6d2_d3f5_5393_a84f_dfcd44a2df34
);
impl windows_core::RuntimeType for IExceptionRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IExceptionRoutedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IFileOpenPicker, IFileOpenPicker_Vtbl, 0x9d00f175_c783_51bd_8c93_fb63695d3abc);
impl windows_core::RuntimeType for IFileOpenPicker {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFileOpenPicker {
    pub fn FileTypeFilter(&self) -> windows_core::Result<windows_collections::IVector<windows_core::HSTRING>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FileTypeFilter)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn PickSingleFileAsync(&self) -> windows_core::Result<windows_future::IAsyncOperation<PickFileResult>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PickSingleFileAsync)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn PickMultipleFilesAsync(
        &self,
    ) -> windows_core::Result<windows_future::IAsyncOperation<windows_collections::IVectorView<PickFileResult>>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PickMultipleFilesAsync)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IFileOpenPicker_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ViewMode: usize,
    SetViewMode: usize,
    SuggestedStartLocation: usize,
    SetSuggestedStartLocation: usize,
    CommitButtonText: usize,
    SetCommitButtonText: usize,
    pub FileTypeFilter:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub PickSingleFileAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub PickMultipleFilesAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFileOpenPicker2, IFileOpenPicker2_Vtbl, 0xb77a4106_895b_5af9_91c3_93e5b058706c);
impl windows_core::RuntimeType for IFileOpenPicker2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFileOpenPicker2 {
    pub fn FileTypeChoices(
        &self,
    ) -> windows_core::Result<
        windows_collections::IMap<windows_core::HSTRING, windows_collections::IVector<windows_core::HSTRING>>,
    > {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FileTypeChoices)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetSuggestedFolder(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSuggestedFolder)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IFileOpenPicker2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Title: usize,
    SetTitle: usize,
    SettingsIdentifier: usize,
    SetSettingsIdentifier: usize,
    pub FileTypeChoices:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    InitialFileTypeIndex: usize,
    SetInitialFileTypeIndex: usize,
    SuggestedFolder: usize,
    pub SetSuggestedFolder:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IFileOpenPickerFactory,
    IFileOpenPickerFactory_Vtbl,
    0x315e86d7_d7a2_5d81_b379_7af78207b1af
);
impl windows_core::RuntimeType for IFileOpenPickerFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFileOpenPickerFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WindowId,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFileOperation, IFileOperation_Vtbl, 0x947aab5f_0a5c_4c13_b4d6_4bf7836fc9f8);
windows_core::imp::interface_hierarchy!(IFileOperation, windows_core::IUnknown);
impl IFileOperation {
    pub unsafe fn SetOperationFlags(&self, dwoperationflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetOperationFlags)(
                windows_core::Interface::as_raw(self),
                dwoperationflags,
            )
        }
    }
    pub unsafe fn SetOwnerWindow(&self, hwndowner: HWND) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetOwnerWindow)(windows_core::Interface::as_raw(self), hwndowner)
        }
    }
    pub unsafe fn DeleteItem<P0, P1>(&self, psiitem: P0, pfopsitem: P1) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IShellItem>,
        P1: windows_core::Param<IFileOperationProgressSink>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DeleteItem)(
                windows_core::Interface::as_raw(self),
                psiitem.param().abi(),
                pfopsitem.param().abi(),
            )
        }
    }
    pub unsafe fn PerformOperations(&self) -> windows_core::HRESULT {
        unsafe { (windows_core::Interface::vtable(self).PerformOperations)(windows_core::Interface::as_raw(self)) }
    }
    pub unsafe fn GetAnyOperationsAborted(&self) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetAnyOperationsAborted)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IFileOperation_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Advise: usize,
    Unadvise: usize,
    pub SetOperationFlags: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    SetProgressMessage: usize,
    SetProgressDialog: usize,
    SetProperties: usize,
    pub SetOwnerWindow: unsafe extern "system" fn(*mut core::ffi::c_void, HWND) -> windows_core::HRESULT,
    ApplyPropertiesToItem: usize,
    ApplyPropertiesToItems: usize,
    RenameItem: usize,
    RenameItems: usize,
    MoveItem: usize,
    MoveItems: usize,
    CopyItem: usize,
    CopyItems: usize,
    pub DeleteItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    DeleteItems: usize,
    NewItem: usize,
    pub PerformOperations: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub GetAnyOperationsAborted:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut windows_core::BOOL) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IFileOperationProgressSink,
    IFileOperationProgressSink_Vtbl,
    0x04b0f1a7_9490_44bc_96e1_4296a31252e2
);
windows_core::imp::interface_hierarchy!(IFileOperationProgressSink, windows_core::IUnknown);
#[repr(C)]
pub struct IFileOperationProgressSink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    StartOperations: usize,
    FinishOperations: usize,
    PreRenameItem: usize,
    PostRenameItem: usize,
    PreMoveItem: usize,
    PostMoveItem: usize,
    PreCopyItem: usize,
    PostCopyItem: usize,
    PreDeleteItem: usize,
    PostDeleteItem: usize,
    PreNewItem: usize,
    PostNewItem: usize,
    UpdateProgress: usize,
    ResetTimer: usize,
    PauseTimer: usize,
    ResumeTimer: usize,
}
windows_core::imp::define_interface!(IFileSavePicker, IFileSavePicker_Vtbl, 0x79f1f4df_741b_59b2_aa06_fe9ac817b7dd);
impl windows_core::RuntimeType for IFileSavePicker {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFileSavePicker {
    pub fn FileTypeChoices(
        &self,
    ) -> windows_core::Result<
        windows_collections::IMap<windows_core::HSTRING, windows_collections::IVector<windows_core::HSTRING>>,
    > {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FileTypeChoices)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetSuggestedFileName(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSuggestedFileName)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetSuggestedFolder(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSuggestedFolder)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn PickSaveFileAsync(&self) -> windows_core::Result<windows_future::IAsyncOperation<PickFileResult>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PickSaveFileAsync)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IFileSavePicker_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    SuggestedStartLocation: usize,
    SetSuggestedStartLocation: usize,
    CommitButtonText: usize,
    SetCommitButtonText: usize,
    pub FileTypeChoices:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    DefaultFileExtension: usize,
    SetDefaultFileExtension: usize,
    SuggestedFileName: usize,
    pub SetSuggestedFileName:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    SuggestedFolder: usize,
    pub SetSuggestedFolder:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub PickSaveFileAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IFileSavePickerFactory,
    IFileSavePickerFactory_Vtbl,
    0x2e256696_30b6_5a05_a8f5_c752db6dd268
);
impl windows_core::RuntimeType for IFileSavePickerFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFileSavePickerFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WindowId,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFlyoutBase, IFlyoutBase_Vtbl, 0xbb6603bf_744d_5c31_a87d_744394634d77);
impl windows_core::RuntimeType for IFlyoutBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFlyoutBase {
    pub fn SetPlacement(&self, value: FlyoutPlacementMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPlacement)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IFlyoutBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Placement: usize,
    pub SetPlacement: unsafe extern "system" fn(*mut core::ffi::c_void, FlyoutPlacementMode) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFocusManager, IFocusManager_Vtbl, 0x9fd07bc5_d2d4_53fe_a31a_846de8b7a257);
impl windows_core::RuntimeType for IFocusManager {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFocusManager_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IFocusManagerStatics,
    IFocusManagerStatics_Vtbl,
    0xe73dce04_e23a_5fb3_96ab_7df04c51dff2
);
impl windows_core::RuntimeType for IFocusManagerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFocusManagerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    GotFocus: usize,
    RemoveGotFocus: usize,
    LostFocus: usize,
    RemoveLostFocus: usize,
    GettingFocus: usize,
    RemoveGettingFocus: usize,
    LosingFocus: usize,
    RemoveLosingFocus: usize,
    TryFocusAsync: usize,
    TryMoveFocusAsync: usize,
    TryMoveFocusWithOptionsAsync: usize,
    TryMoveFocusWithOptions: usize,
    FindNextElement: usize,
    FindFirstFocusableElement: usize,
    FindLastFocusableElement: usize,
    FindNextElementWithOptions: usize,
    FindNextFocusableElement: usize,
    FindNextFocusableElementWithHint: usize,
    TryMoveFocus: usize,
    GetFocusedElement: usize,
    pub GetFocusedElementWithRoot: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFolderPicker, IFolderPicker_Vtbl, 0x3ef0d1ca_97c6_5873_8ea2_02c450174290);
impl windows_core::RuntimeType for IFolderPicker {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFolderPicker {
    pub fn PickSingleFolderAsync(&self) -> windows_core::Result<windows_future::IAsyncOperation<PickFolderResult>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PickSingleFolderAsync)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IFolderPicker_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ViewMode: usize,
    SetViewMode: usize,
    SuggestedStartLocation: usize,
    SetSuggestedStartLocation: usize,
    CommitButtonText: usize,
    SetCommitButtonText: usize,
    pub PickSingleFolderAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFolderPicker2, IFolderPicker2_Vtbl, 0x12647ff3_8cca_5d1f_9ee4_ee4195ee155d);
impl windows_core::RuntimeType for IFolderPicker2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFolderPicker2 {
    pub fn SetSuggestedFolder(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSuggestedFolder)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IFolderPicker2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Title: usize,
    SetTitle: usize,
    SettingsIdentifier: usize,
    SetSettingsIdentifier: usize,
    SuggestedFolder: usize,
    pub SetSuggestedFolder:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IFolderPickerFactory,
    IFolderPickerFactory_Vtbl,
    0xe1550d89_b389_5886_8395_022b1588d6a8
);
impl windows_core::RuntimeType for IFolderPickerFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFolderPickerFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WindowId,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFontFamily, IFontFamily_Vtbl, 0x18fa5bc1_7294_527c_bb02_b213e0b3a2a3);
impl windows_core::RuntimeType for IFontFamily {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFontFamily_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IFontFamilyFactory,
    IFontFamilyFactory_Vtbl,
    0x61b88a77_d0f9_5e9e_8c28_eda01fede22e
);
impl windows_core::RuntimeType for IFontFamilyFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFontFamilyFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstanceWithName: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFontIcon, IFontIcon_Vtbl, 0x6eba5ed9_d233_5f5e_91a8_f5134292658a);
impl windows_core::RuntimeType for IFontIcon {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFontIcon {
    pub fn Glyph(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Glyph)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetGlyph(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetGlyph)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn FontSize(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FontSize)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontSize)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IFontIcon_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Glyph: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetGlyph: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub FontSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetFontSize: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFontIconFactory, IFontIconFactory_Vtbl, 0xaa9a24fe_bef8_564a_b200_694cd6f6ba4e);
impl windows_core::RuntimeType for IFontIconFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFontIconFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IFrameworkElement, IFrameworkElement_Vtbl, 0xfe08f13d_dc6a_5495_ad44_c2d8d21863b0);
impl windows_core::RuntimeType for IFrameworkElement {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IFrameworkElement {
    pub fn ActualWidth(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ActualWidth)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn ActualHeight(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ActualHeight)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Width(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Width)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetWidth(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetWidth)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn Height(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Height)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetHeight(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetHeight)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn SetMinWidth(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMinWidth)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetHorizontalAlignment(&self, value: HorizontalAlignment) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHorizontalAlignment)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetVerticalAlignment(&self, value: VerticalAlignment) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalAlignment)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetMargin(&self, value: Thickness) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetMargin)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn SetStyle<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Style>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetStyle)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
    pub fn FlowDirection(&self) -> windows_core::Result<FlowDirection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FlowDirection)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetFlowDirection(&self, value: FlowDirection) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFlowDirection)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetRequestedTheme(&self, value: ElementTheme) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRequestedTheme)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn IsLoaded(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsLoaded)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Loaded<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Loaded)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveLoaded,
            ))
        }
    }
    pub fn SizeChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SizeChangedEventArgs>) + 'static,
    {
        let handler: SizeChangedEventHandler = {
            let com = windows_core::imp::DelegateBox::<SizeChangedEventHandler, F>::new(
                &SizeChangedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).SizeChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveSizeChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct IFrameworkElement_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Triggers: usize,
    Resources: usize,
    SetResources: usize,
    Tag: usize,
    SetTag: usize,
    Language: usize,
    SetLanguage: usize,
    pub ActualWidth: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub ActualHeight: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub Width: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetWidth: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub Height: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetHeight: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    MinWidth: usize,
    pub SetMinWidth: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    MaxWidth: usize,
    SetMaxWidth: usize,
    MinHeight: usize,
    SetMinHeight: usize,
    MaxHeight: usize,
    SetMaxHeight: usize,
    HorizontalAlignment: usize,
    pub SetHorizontalAlignment:
        unsafe extern "system" fn(*mut core::ffi::c_void, HorizontalAlignment) -> windows_core::HRESULT,
    VerticalAlignment: usize,
    pub SetVerticalAlignment:
        unsafe extern "system" fn(*mut core::ffi::c_void, VerticalAlignment) -> windows_core::HRESULT,
    Margin: usize,
    pub SetMargin: unsafe extern "system" fn(*mut core::ffi::c_void, Thickness) -> windows_core::HRESULT,
    Name: usize,
    SetName: usize,
    BaseUri: usize,
    DataContext: usize,
    SetDataContext: usize,
    AllowFocusOnInteraction: usize,
    SetAllowFocusOnInteraction: usize,
    FocusVisualMargin: usize,
    SetFocusVisualMargin: usize,
    FocusVisualSecondaryThickness: usize,
    SetFocusVisualSecondaryThickness: usize,
    FocusVisualPrimaryThickness: usize,
    SetFocusVisualPrimaryThickness: usize,
    FocusVisualSecondaryBrush: usize,
    SetFocusVisualSecondaryBrush: usize,
    FocusVisualPrimaryBrush: usize,
    SetFocusVisualPrimaryBrush: usize,
    AllowFocusWhenDisabled: usize,
    SetAllowFocusWhenDisabled: usize,
    Style: usize,
    pub SetStyle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Parent: usize,
    pub FlowDirection: unsafe extern "system" fn(*mut core::ffi::c_void, *mut FlowDirection) -> windows_core::HRESULT,
    pub SetFlowDirection: unsafe extern "system" fn(*mut core::ffi::c_void, FlowDirection) -> windows_core::HRESULT,
    RequestedTheme: usize,
    pub SetRequestedTheme: unsafe extern "system" fn(*mut core::ffi::c_void, ElementTheme) -> windows_core::HRESULT,
    pub IsLoaded: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    ActualTheme: usize,
    pub Loaded:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveLoaded: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    Unloaded: usize,
    RemoveUnloaded: usize,
    DataContextChanged: usize,
    RemoveDataContextChanged: usize,
    pub SizeChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveSizeChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IFrameworkElementAutomationPeer,
    IFrameworkElementAutomationPeer_Vtbl,
    0x7dab4f24_605c_51cb_87db_3eed1b9fb37b
);
impl windows_core::RuntimeType for IFrameworkElementAutomationPeer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFrameworkElementAutomationPeer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IFrameworkElementAutomationPeerStatics,
    IFrameworkElementAutomationPeerStatics_Vtbl,
    0x081f6fbe_6500_528a_a506_f5a4d41ddf6c
);
impl windows_core::RuntimeType for IFrameworkElementAutomationPeerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFrameworkElementAutomationPeerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FromElement: usize,
    pub CreatePeerForElement: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IFrameworkTemplate,
    IFrameworkTemplate_Vtbl,
    0x0084c7c2_de48_5b0b_8a5a_e4fb76b7f7d1
);
impl windows_core::RuntimeType for IFrameworkTemplate {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IFrameworkTemplate_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IGeneralTransform, IGeneralTransform_Vtbl, 0x04eedeeb_31e5_54c0_ae3f_8bd06645d339);
impl windows_core::RuntimeType for IGeneralTransform {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IGeneralTransform {
    pub fn TransformPoint(&self, point: Point) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TransformPoint)(
                windows_core::Interface::as_raw(self),
                point,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IGeneralTransform_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Inverse: usize,
    pub TransformPoint: unsafe extern "system" fn(*mut core::ffi::c_void, Point, *mut Point) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IGeometry, IGeometry_Vtbl, 0xdc102dcc_3be2_5414_8599_94b6e76ef39b);
impl windows_core::RuntimeType for IGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IGrid, IGrid_Vtbl, 0xc4496219_9014_58a1_b4ad_c5044913a5bb);
impl windows_core::RuntimeType for IGrid {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IGrid_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IGridStatics, IGridStatics_Vtbl, 0xef9cf81d_a431_50f4_abf5_3023fe447704);
impl windows_core::RuntimeType for IGridStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IGridStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BackgroundSizingProperty: usize,
    BorderBrushProperty: usize,
    BorderThicknessProperty: usize,
    CornerRadiusProperty: usize,
    PaddingProperty: usize,
    RowSpacingProperty: usize,
    ColumnSpacingProperty: usize,
    RowProperty: usize,
    GetRow: usize,
    pub SetRow: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IIconElement, IIconElement_Vtbl, 0x18f69350_279e_50ea_8d23_138e717ed939);
impl windows_core::RuntimeType for IIconElement {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IIconElement_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IImage, IImage_Vtbl, 0x220d3d8d_66de_53a1_a215_ba9c165565ab);
impl windows_core::RuntimeType for IImage {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IImage {
    pub fn Source(&self) -> windows_core::Result<ImageSource> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Source)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetSource<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ImageSource>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSource)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn Stretch(&self) -> windows_core::Result<Stretch> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Stretch)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetStretch(&self, value: Stretch) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetStretch)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn ImageFailed<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<ExceptionRoutedEventArgs>) + 'static,
    {
        let handler: ExceptionRoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<ExceptionRoutedEventHandler, F>::new(
                &ExceptionRoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ImageFailed)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveImageFailed,
            ))
        }
    }
    pub fn ImageOpened<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ImageOpened)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveImageOpened,
            ))
        }
    }
}
#[repr(C)]
pub struct IImage_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Source: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetSource: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Stretch: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Stretch) -> windows_core::HRESULT,
    pub SetStretch: unsafe extern "system" fn(*mut core::ffi::c_void, Stretch) -> windows_core::HRESULT,
    NineGrid: usize,
    SetNineGrid: usize,
    pub ImageFailed:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveImageFailed: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ImageOpened:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveImageOpened: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IImageSource, IImageSource_Vtbl, 0x6c2038f6_d6d5_55e9_9b9e_082f12dbff60);
impl windows_core::RuntimeType for IImageSource {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IImageSource_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IInputFocusController,
    IInputFocusController_Vtbl,
    0x8dfdc26c_8b8d_515d_8ddd_4685b3a540e9
);
impl windows_core::RuntimeType for IInputFocusController {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInputFocusController {
    pub fn HasFocus(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).HasFocus)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn TrySetFocus(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TrySetFocus)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInputFocusController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub HasFocus: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub TrySetFocus: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInputFocusControllerStatics,
    IInputFocusControllerStatics_Vtbl,
    0xaeb311da_da9b_5a1b_92f4_83ddde933e00
);
impl windows_core::RuntimeType for IInputFocusControllerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInputFocusControllerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetForIsland: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IInputObject, IInputObject_Vtbl, 0x42edbc88_d386_544d_b1b8_68617fe68282);
impl windows_core::RuntimeType for IInputObject {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInputObject_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IInputScope, IInputScope_Vtbl, 0x76ea58b1_e910_5176_9147_695cc95e7da2);
impl windows_core::RuntimeType for IInputScope {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInputScope {
    pub fn Names(&self) -> windows_core::Result<windows_collections::IVector<InputScopeName>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Names)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IInputScope_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Names: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IInputScopeName, IInputScopeName_Vtbl, 0xee99a66d_28d0_53cb_82ee_1b6ee58bcc35);
impl windows_core::RuntimeType for IInputScopeName {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInputScopeName {
    pub fn NameValue(&self) -> windows_core::Result<InputScopeNameValue> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).NameValue)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetNameValue(&self, value: InputScopeNameValue) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetNameValue)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IInputScopeName_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub NameValue: unsafe extern "system" fn(*mut core::ffi::c_void, *mut InputScopeNameValue) -> windows_core::HRESULT,
    pub SetNameValue: unsafe extern "system" fn(*mut core::ffi::c_void, InputScopeNameValue) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IInvokeProvider, IInvokeProvider_Vtbl, 0x02481105_3378_544d_b4e1_a1b368afbc02);
impl windows_core::RuntimeType for IInvokeProvider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IInvokeProvider, windows_core::IUnknown, windows_core::IInspectable);
impl IInvokeProvider {
    pub fn Invoke(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Invoke)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IInvokeProvider_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Invoke: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IItemContainer, IItemContainer_Vtbl, 0x6332a67f_7fd9_53c7_afd8_cfa1237cf6d1);
impl windows_core::RuntimeType for IItemContainer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IItemContainer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IItemContainerMapping,
    IItemContainerMapping_Vtbl,
    0xb3e03a32_ea83_5626_bfda_d327a1c7550c
);
impl windows_core::RuntimeType for IItemContainerMapping {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IItemContainerMapping, windows_core::IUnknown, windows_core::IInspectable);
impl IItemContainerMapping {
    pub fn ItemFromContainer<P0>(&self, container: P0) -> windows_core::Result<windows_core::IInspectable>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ItemFromContainer)(
                windows_core::Interface::as_raw(self),
                container.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn ContainerFromItem<P0>(&self, item: P0) -> windows_core::Result<DependencyObject>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContainerFromItem)(
                windows_core::Interface::as_raw(self),
                item.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IItemContainerMapping_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ItemFromContainer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ContainerFromItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IItemsControl, IItemsControl_Vtbl, 0xbf1ccb54_83e2_5b98_acbc_736f876c3d35);
impl windows_core::RuntimeType for IItemsControl {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IItemsControl {
    pub fn Items(&self) -> windows_core::Result<ItemCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetItemTemplate<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DataTemplate>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetItemTemplate)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetItemContainerStyle<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Style>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetItemContainerStyle)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IItemsControl_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ItemsSource: usize,
    SetItemsSource: usize,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    ItemTemplate: usize,
    pub SetItemTemplate:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    ItemTemplateSelector: usize,
    SetItemTemplateSelector: usize,
    ItemsPanel: usize,
    SetItemsPanel: usize,
    DisplayMemberPath: usize,
    SetDisplayMemberPath: usize,
    ItemsPanelRoot: usize,
    ItemContainerStyle: usize,
    pub SetItemContainerStyle:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IKeyRoutedEventArgs,
    IKeyRoutedEventArgs_Vtbl,
    0xee357007_a2d6_5c75_9431_05fd66ec7915
);
impl windows_core::RuntimeType for IKeyRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IKeyRoutedEventArgs {
    pub fn Key(&self) -> windows_core::Result<VirtualKey> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Key)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn KeyStatus(&self) -> windows_core::Result<CorePhysicalKeyStatus> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).KeyStatus)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetHandled)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IKeyRoutedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Key: unsafe extern "system" fn(*mut core::ffi::c_void, *mut VirtualKey) -> windows_core::HRESULT,
    pub KeyStatus:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut CorePhysicalKeyStatus) -> windows_core::HRESULT,
    Handled: usize,
    pub SetHandled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IKeyboardAccelerator,
    IKeyboardAccelerator_Vtbl,
    0x6f8bf1e2_4e91_5cf9_a6be_4770caf3d770
);
impl windows_core::RuntimeType for IKeyboardAccelerator {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IKeyboardAccelerator {
    pub fn Key(&self) -> windows_core::Result<VirtualKey> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Key)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetKey(&self, value: VirtualKey) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetKey)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn Modifiers(&self) -> windows_core::Result<VirtualKeyModifiers> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Modifiers)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetModifiers(&self, value: VirtualKeyModifiers) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetModifiers)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Invoked<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<KeyboardAccelerator>, windows_core::Ref<KeyboardAcceleratorInvokedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<KeyboardAccelerator, KeyboardAcceleratorInvokedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<KeyboardAccelerator, KeyboardAcceleratorInvokedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<KeyboardAccelerator, KeyboardAcceleratorInvokedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Invoked)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveInvoked,
            ))
        }
    }
}
#[repr(C)]
pub struct IKeyboardAccelerator_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Key: unsafe extern "system" fn(*mut core::ffi::c_void, *mut VirtualKey) -> windows_core::HRESULT,
    pub SetKey: unsafe extern "system" fn(*mut core::ffi::c_void, VirtualKey) -> windows_core::HRESULT,
    pub Modifiers: unsafe extern "system" fn(*mut core::ffi::c_void, *mut VirtualKeyModifiers) -> windows_core::HRESULT,
    pub SetModifiers: unsafe extern "system" fn(*mut core::ffi::c_void, VirtualKeyModifiers) -> windows_core::HRESULT,
    IsEnabled: usize,
    SetIsEnabled: usize,
    ScopeOwner: usize,
    SetScopeOwner: usize,
    pub Invoked:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveInvoked: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IKeyboardAcceleratorFactory,
    IKeyboardAcceleratorFactory_Vtbl,
    0xca1d410a_af2a_51b9_a1de_6c0af9f3b598
);
impl windows_core::RuntimeType for IKeyboardAcceleratorFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IKeyboardAcceleratorFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IKeyboardAcceleratorInvokedEventArgs,
    IKeyboardAcceleratorInvokedEventArgs_Vtbl,
    0x62c9fdb0_b574_527d_97eb_5c7f674441e0
);
impl windows_core::RuntimeType for IKeyboardAcceleratorInvokedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IKeyboardAcceleratorInvokedEventArgs {
    pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetHandled)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IKeyboardAcceleratorInvokedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Handled: usize,
    pub SetHandled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ILaunchActivatedEventArgs,
    ILaunchActivatedEventArgs_Vtbl,
    0xd505cea9_1bcb_5b29_a8be_944e00f06f78
);
impl windows_core::RuntimeType for ILaunchActivatedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ILaunchActivatedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IListView, IListView_Vtbl, 0xf6015db1_df63_52fd_a164_0df44715ee0a);
impl windows_core::RuntimeType for IListView {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IListView_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IListViewBase, IListViewBase_Vtbl, 0x775c57ac_abce_5beb_8e34_3b8158aedd80);
impl windows_core::RuntimeType for IListViewBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IListViewBase {
    pub fn SelectedItems(&self) -> windows_core::Result<windows_collections::IVector<windows_core::IInspectable>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedItems)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SelectionMode(&self) -> windows_core::Result<ListViewSelectionMode> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectionMode)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetSelectionMode(&self, value: ListViewSelectionMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectionMode)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn CanDragItems(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CanDragItems)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetCanDragItems(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCanDragItems)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SingleSelectionFollowsFocus(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SingleSelectionFollowsFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetSingleSelectionFollowsFocus(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSingleSelectionFollowsFocus)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn DragItemsStarting<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragItemsStartingEventArgs>) + 'static,
    {
        let handler: DragItemsStartingEventHandler = {
            let com = windows_core::imp::DelegateBox::<DragItemsStartingEventHandler, F>::new(
                &DragItemsStartingEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).DragItemsStarting)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDragItemsStarting,
            ))
        }
    }
    pub fn ContainerContentChanging<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<ListViewBase>, windows_core::Ref<ContainerContentChangingEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<ListViewBase, ContainerContentChangingEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<ListViewBase, ContainerContentChangingEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<ListViewBase, ContainerContentChangingEventArgs, F>::VTABLE, handler
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ContainerContentChanging)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveContainerContentChanging,
            ))
        }
    }
    pub fn ScrollIntoView<P0>(&self, item: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ScrollIntoView)(
                windows_core::Interface::as_raw(self),
                item.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IListViewBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub SelectedItems:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SelectionMode:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut ListViewSelectionMode) -> windows_core::HRESULT,
    pub SetSelectionMode:
        unsafe extern "system" fn(*mut core::ffi::c_void, ListViewSelectionMode) -> windows_core::HRESULT,
    IsSwipeEnabled: usize,
    SetIsSwipeEnabled: usize,
    pub CanDragItems: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetCanDragItems: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    CanReorderItems: usize,
    SetCanReorderItems: usize,
    IsItemClickEnabled: usize,
    SetIsItemClickEnabled: usize,
    DataFetchSize: usize,
    SetDataFetchSize: usize,
    IncrementalLoadingThreshold: usize,
    SetIncrementalLoadingThreshold: usize,
    IncrementalLoadingTrigger: usize,
    SetIncrementalLoadingTrigger: usize,
    ShowsScrollingPlaceholders: usize,
    SetShowsScrollingPlaceholders: usize,
    ReorderMode: usize,
    SetReorderMode: usize,
    SelectedRanges: usize,
    IsMultiSelectCheckBoxEnabled: usize,
    SetIsMultiSelectCheckBoxEnabled: usize,
    pub SingleSelectionFollowsFocus:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetSingleSelectionFollowsFocus:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    ItemClick: usize,
    RemoveItemClick: usize,
    pub DragItemsStarting:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDragItemsStarting: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    DragItemsCompleted: usize,
    RemoveDragItemsCompleted: usize,
    pub ContainerContentChanging:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveContainerContentChanging: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    ChoosingItemContainer: usize,
    RemoveChoosingItemContainer: usize,
    ChoosingGroupHeaderContainer: usize,
    RemoveChoosingGroupHeaderContainer: usize,
    pub ScrollIntoView:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IListViewFactory, IListViewFactory_Vtbl, 0x03ebefb8_f64a_5bf9_9570_cb09eeea2335);
impl windows_core::RuntimeType for IListViewFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IListViewFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IListViewItem, IListViewItem_Vtbl, 0x05fe41c2_0451_5d38_9c55_5d10cfd08889);
impl windows_core::RuntimeType for IListViewItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IListViewItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IMenuBar, IMenuBar_Vtbl, 0xba97f337_8f1e_5141_b53f_e77a8ba3ebbd);
impl windows_core::RuntimeType for IMenuBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IMenuBar {
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<MenuBarItem>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IMenuBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IMenuBarFactory, IMenuBarFactory_Vtbl, 0x76aa8759_04ee_5a4c_b98c_d03742d47cdb);
impl windows_core::RuntimeType for IMenuBarFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuBarFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IMenuBarItem, IMenuBarItem_Vtbl, 0xa7900980_51cc_531d_97c5_356b13573398);
impl windows_core::RuntimeType for IMenuBarItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IMenuBarItem {
    pub fn Title(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Title)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetTitle(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTitle)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<MenuFlyoutItemBase>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IMenuBarItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Title: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetTitle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMenuBarItemFactory,
    IMenuBarItemFactory_Vtbl,
    0x87d02172_83cb_5459_940f_173f7501b300
);
impl windows_core::RuntimeType for IMenuBarItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuBarItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IMenuFlyout, IMenuFlyout_Vtbl, 0xf4c77c6c_1fa5_5d85_8559_5d02b7d4e5e7);
impl windows_core::RuntimeType for IMenuFlyout {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IMenuFlyout {
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<MenuFlyoutItemBase>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IMenuFlyout_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMenuFlyoutFactory,
    IMenuFlyoutFactory_Vtbl,
    0xa3d225de_6b35_5442_b6c9_06fd24139a63
);
impl windows_core::RuntimeType for IMenuFlyoutFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuFlyoutFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IMenuFlyoutItem, IMenuFlyoutItem_Vtbl, 0x4252df5a_44f9_5ee8_b1cc_53de9aaa4095);
impl windows_core::RuntimeType for IMenuFlyoutItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IMenuFlyoutItem {
    pub fn Text(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Text)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn Click<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Click)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveClick,
            ))
        }
    }
}
#[repr(C)]
pub struct IMenuFlyoutItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Command: usize,
    SetCommand: usize,
    CommandParameter: usize,
    SetCommandParameter: usize,
    Icon: usize,
    SetIcon: usize,
    KeyboardAcceleratorTextOverride: usize,
    SetKeyboardAcceleratorTextOverride: usize,
    TemplateSettings: usize,
    pub Click:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveClick: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMenuFlyoutItemBase,
    IMenuFlyoutItemBase_Vtbl,
    0x4bee2715_44a1_5f94_86e8_02ddbe3dc6b9
);
impl windows_core::RuntimeType for IMenuFlyoutItemBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuFlyoutItemBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IMenuFlyoutItemFactory,
    IMenuFlyoutItemFactory_Vtbl,
    0x9c3c9a1f_89af_521a_81a5_8a01db7a79af
);
impl windows_core::RuntimeType for IMenuFlyoutItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuFlyoutItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMenuFlyoutSeparator,
    IMenuFlyoutSeparator_Vtbl,
    0x3eaf5fd5_935e_5ed7_8d05_f6bafa936d25
);
impl windows_core::RuntimeType for IMenuFlyoutSeparator {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuFlyoutSeparator_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IMenuFlyoutSeparatorFactory,
    IMenuFlyoutSeparatorFactory_Vtbl,
    0x26156c9c_95ef_5e55_8342_773fc43baac3
);
impl windows_core::RuntimeType for IMenuFlyoutSeparatorFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IMenuFlyoutSeparatorFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMenuFlyoutSubItem,
    IMenuFlyoutSubItem_Vtbl,
    0x6b0688c1_47b0_53b5_b6f9_5ec5d6623b84
);
impl windows_core::RuntimeType for IMenuFlyoutSubItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IMenuFlyoutSubItem {
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<MenuFlyoutItemBase>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn Text(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Text)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IMenuFlyoutSubItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
pub const INFINITE: u32 = 4294967295;
windows_core::imp::define_interface!(INavigationView, INavigationView_Vtbl, 0xe77a4b36_3dd1_53d9_9f97_65dccaa74a5c);
impl windows_core::RuntimeType for INavigationView {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl INavigationView {
    pub fn IsPaneOpen(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsPaneOpen)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsPaneOpen(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPaneOpen)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn CompactModeThresholdWidth(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CompactModeThresholdWidth)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetCompactModeThresholdWidth(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCompactModeThresholdWidth)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn ExpandedModeThresholdWidth(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ExpandedModeThresholdWidth)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn DisplayMode(&self) -> windows_core::Result<NavigationViewDisplayMode> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DisplayMode)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsSettingsVisible(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsSettingsVisible)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetIsPaneToggleButtonVisible(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPaneToggleButtonVisible)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn CompactPaneLength(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CompactPaneLength)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn OpenPaneLength(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).OpenPaneLength)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetOpenPaneLength(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOpenPaneLength)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SelectedItem(&self) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedItem)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetSelectedItem<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectedItem)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn MenuItems(&self) -> windows_core::Result<windows_collections::IVector<windows_core::IInspectable>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MenuItems)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SelectionChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<NavigationView>, windows_core::Ref<NavigationViewSelectionChangedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<NavigationView, NavigationViewSelectionChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<NavigationView, NavigationViewSelectionChangedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<NavigationView, NavigationViewSelectionChangedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).SelectionChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveSelectionChanged,
            ))
        }
    }
    pub fn DisplayModeChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<NavigationView>, windows_core::Ref<NavigationViewDisplayModeChangedEventArgs>)
            + 'static,
    {
        let handler: TypedEventHandler<NavigationView, NavigationViewDisplayModeChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<NavigationView, NavigationViewDisplayModeChangedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<NavigationView, NavigationViewDisplayModeChangedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).DisplayModeChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDisplayModeChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct INavigationView_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsPaneOpen: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsPaneOpen: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub CompactModeThresholdWidth: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetCompactModeThresholdWidth: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub ExpandedModeThresholdWidth:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    SetExpandedModeThresholdWidth: usize,
    FooterMenuItems: usize,
    FooterMenuItemsSource: usize,
    SetFooterMenuItemsSource: usize,
    PaneFooter: usize,
    SetPaneFooter: usize,
    Header: usize,
    SetHeader: usize,
    HeaderTemplate: usize,
    SetHeaderTemplate: usize,
    pub DisplayMode:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut NavigationViewDisplayMode) -> windows_core::HRESULT,
    IsSettingsVisible: usize,
    pub SetIsSettingsVisible: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsPaneToggleButtonVisible: usize,
    pub SetIsPaneToggleButtonVisible: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    AlwaysShowHeader: usize,
    SetAlwaysShowHeader: usize,
    pub CompactPaneLength: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    SetCompactPaneLength: usize,
    pub OpenPaneLength: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetOpenPaneLength: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    PaneToggleButtonStyle: usize,
    SetPaneToggleButtonStyle: usize,
    pub SelectedItem:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetSelectedItem:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub MenuItems:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    MenuItemsSource: usize,
    SetMenuItemsSource: usize,
    SettingsItem: usize,
    AutoSuggestBox: usize,
    SetAutoSuggestBox: usize,
    MenuItemTemplate: usize,
    SetMenuItemTemplate: usize,
    MenuItemTemplateSelector: usize,
    SetMenuItemTemplateSelector: usize,
    MenuItemContainerStyle: usize,
    SetMenuItemContainerStyle: usize,
    MenuItemContainerStyleSelector: usize,
    SetMenuItemContainerStyleSelector: usize,
    MenuItemFromContainer: usize,
    ContainerFromMenuItem: usize,
    pub SelectionChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveSelectionChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    ItemInvoked: usize,
    RemoveItemInvoked: usize,
    pub DisplayModeChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDisplayModeChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(INavigationView2, INavigationView2_Vtbl, 0x05b428cf_014c_56dd_896a_a3e7089d73b5);
impl windows_core::RuntimeType for INavigationView2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl INavigationView2 {
    pub fn SetIsBackButtonVisible(&self, value: NavigationViewBackButtonVisible) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsBackButtonVisible)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn PaneClosed<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<NavigationView>, windows_core::Ref<windows_core::IInspectable>) + 'static,
    {
        let handler: TypedEventHandler<NavigationView, windows_core::IInspectable> = {
            let com =
                windows_core::imp::DelegateBox::<TypedEventHandler<NavigationView, windows_core::IInspectable>, F>::new(
                    &TypedEventHandlerBox::<NavigationView, windows_core::IInspectable, F>::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PaneClosed)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePaneClosed,
            ))
        }
    }
    pub fn PaneOpened<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<NavigationView>, windows_core::Ref<windows_core::IInspectable>) + 'static,
    {
        let handler: TypedEventHandler<NavigationView, windows_core::IInspectable> = {
            let com =
                windows_core::imp::DelegateBox::<TypedEventHandler<NavigationView, windows_core::IInspectable>, F>::new(
                    &TypedEventHandlerBox::<NavigationView, windows_core::IInspectable, F>::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PaneOpened)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePaneOpened,
            ))
        }
    }
    pub fn IsPaneVisible(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsPaneVisible)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsPaneVisible(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPaneVisible)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct INavigationView2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsBackButtonVisible: usize,
    pub SetIsBackButtonVisible:
        unsafe extern "system" fn(*mut core::ffi::c_void, NavigationViewBackButtonVisible) -> windows_core::HRESULT,
    IsBackEnabled: usize,
    SetIsBackEnabled: usize,
    PaneTitle: usize,
    SetPaneTitle: usize,
    BackRequested: usize,
    RemoveBackRequested: usize,
    pub PaneClosed:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePaneClosed: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    PaneClosing: usize,
    RemovePaneClosing: usize,
    pub PaneOpened:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePaneOpened: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    PaneOpening: usize,
    RemovePaneOpening: usize,
    PaneDisplayMode: usize,
    SetPaneDisplayMode: usize,
    PaneHeader: usize,
    SetPaneHeader: usize,
    PaneCustomContent: usize,
    SetPaneCustomContent: usize,
    ContentOverlay: usize,
    SetContentOverlay: usize,
    pub IsPaneVisible: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsPaneVisible: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INavigationViewDisplayModeChangedEventArgs,
    INavigationViewDisplayModeChangedEventArgs_Vtbl,
    0x58dcf1ea_9e56_522c_b3f8_34bd55ecaca4
);
impl windows_core::RuntimeType for INavigationViewDisplayModeChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewDisplayModeChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    INavigationViewFactory,
    INavigationViewFactory_Vtbl,
    0xffea1ada_9232_5507_a320_ed2fadbe6127
);
impl windows_core::RuntimeType for INavigationViewFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INavigationViewItem,
    INavigationViewItem_Vtbl,
    0x3ab3d503_a37c_5836_8adb_2882062e73a1
);
impl windows_core::RuntimeType for INavigationViewItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl INavigationViewItem {
    pub fn SetIcon<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<IconElement>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetIcon)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
}
#[repr(C)]
pub struct INavigationViewItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Icon: usize,
    pub SetIcon: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INavigationViewItemBase,
    INavigationViewItemBase_Vtbl,
    0x33586494_af48_513f_be4d_f645e8c89005
);
impl windows_core::RuntimeType for INavigationViewItemBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewItemBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    INavigationViewItemFactory,
    INavigationViewItemFactory_Vtbl,
    0xde60a001_9385_5535_80e1_2b68f4bfde26
);
impl windows_core::RuntimeType for INavigationViewItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INavigationViewItemHeader,
    INavigationViewItemHeader_Vtbl,
    0x432bc062_45bc_57ef_a2d3_11851a56a882
);
impl windows_core::RuntimeType for INavigationViewItemHeader {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewItemHeader_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    INavigationViewItemHeaderFactory,
    INavigationViewItemHeaderFactory_Vtbl,
    0x6a5447cd_2918_5fe3_899b_93d6961285e6
);
impl windows_core::RuntimeType for INavigationViewItemHeaderFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewItemHeaderFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INavigationViewItemSeparator,
    INavigationViewItemSeparator_Vtbl,
    0xe3c35630_1d5f_5689_a0cb_8794485caacf
);
impl windows_core::RuntimeType for INavigationViewItemSeparator {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewItemSeparator_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    INavigationViewItemSeparatorFactory,
    INavigationViewItemSeparatorFactory_Vtbl,
    0x181266a1_d4f8_5af7_810b_aee8a1d452ed
);
impl windows_core::RuntimeType for INavigationViewItemSeparatorFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INavigationViewItemSeparatorFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INavigationViewSelectionChangedEventArgs,
    INavigationViewSelectionChangedEventArgs_Vtbl,
    0x14a064a5_c79d_5f63_ac6e_1c313fe63566
);
impl windows_core::RuntimeType for INavigationViewSelectionChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl INavigationViewSelectionChangedEventArgs {
    pub fn SelectedItem(&self) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedItem)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct INavigationViewSelectionChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub SelectedItem:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(INumberBox, INumberBox_Vtbl, 0xc18eb0e9_29fb_525d_abbc_d6b2110f542e);
impl windows_core::RuntimeType for INumberBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl INumberBox {
    pub fn Minimum(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Minimum)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetMinimum(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetMinimum)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn Maximum(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Maximum)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetMaximum(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetMaximum)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn Value(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Value)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetValue)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn SmallChange(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SmallChange)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetSmallChange(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSmallChange)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SpinButtonPlacementMode(&self) -> windows_core::Result<NumberBoxSpinButtonPlacementMode> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SpinButtonPlacementMode)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetSpinButtonPlacementMode(&self, value: NumberBoxSpinButtonPlacementMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSpinButtonPlacementMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn IsWrapEnabled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsWrapEnabled)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsWrapEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsWrapEnabled)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn ValueChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<NumberBox>, windows_core::Ref<NumberBoxValueChangedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<NumberBox, NumberBoxValueChangedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<TypedEventHandler<NumberBox, NumberBoxValueChangedEventArgs>, F>::new(
                    &TypedEventHandlerBox::<NumberBox, NumberBoxValueChangedEventArgs, F>::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ValueChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveValueChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct INumberBox_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Minimum: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetMinimum: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub Maximum: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetMaximum: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub Value: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub SmallChange: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetSmallChange: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    LargeChange: usize,
    SetLargeChange: usize,
    Text: usize,
    SetText: usize,
    Header: usize,
    SetHeader: usize,
    HeaderTemplate: usize,
    SetHeaderTemplate: usize,
    PlaceholderText: usize,
    SetPlaceholderText: usize,
    SelectionFlyout: usize,
    SetSelectionFlyout: usize,
    SelectionHighlightColor: usize,
    SetSelectionHighlightColor: usize,
    TextReadingOrder: usize,
    SetTextReadingOrder: usize,
    PreventKeyboardDisplayOnProgrammaticFocus: usize,
    SetPreventKeyboardDisplayOnProgrammaticFocus: usize,
    Description: usize,
    SetDescription: usize,
    ValidationMode: usize,
    SetValidationMode: usize,
    pub SpinButtonPlacementMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut NumberBoxSpinButtonPlacementMode,
    ) -> windows_core::HRESULT,
    pub SetSpinButtonPlacementMode:
        unsafe extern "system" fn(*mut core::ffi::c_void, NumberBoxSpinButtonPlacementMode) -> windows_core::HRESULT,
    pub IsWrapEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsWrapEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    AcceptsExpression: usize,
    SetAcceptsExpression: usize,
    NumberFormatter: usize,
    SetNumberFormatter: usize,
    pub ValueChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveValueChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(INumberBoxFactory, INumberBoxFactory_Vtbl, 0x6b81f3cb_45a4_5d19_9bbb_a9fe4656ac4d);
impl windows_core::RuntimeType for INumberBoxFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INumberBoxFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    INumberBoxValueChangedEventArgs,
    INumberBoxValueChangedEventArgs_Vtbl,
    0xc66cf16e_7c8a_532e_9d23_058c1c98dd50
);
impl windows_core::RuntimeType for INumberBoxValueChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INumberBoxValueChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IOverlappedPresenter,
    IOverlappedPresenter_Vtbl,
    0x21693970_4f4c_5172_9e9d_682a2d174884
);
impl windows_core::RuntimeType for IOverlappedPresenter {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IOverlappedPresenter {
    pub fn SetIsMaximizable(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsMaximizable)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetIsMinimizable(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsMinimizable)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn IsModal(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsModal)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsModal(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetIsModal)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn IsResizable(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsResizable)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsResizable(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsResizable)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn State(&self) -> windows_core::Result<OverlappedPresenterState> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).State)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Maximize(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Maximize)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn Restore(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Restore)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IOverlappedPresenter_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    HasBorder: usize,
    HasTitleBar: usize,
    IsAlwaysOnTop: usize,
    SetIsAlwaysOnTop: usize,
    IsMaximizable: usize,
    pub SetIsMaximizable: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsMinimizable: usize,
    pub SetIsMinimizable: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub IsModal: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsModal: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub IsResizable: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsResizable: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub State:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut OverlappedPresenterState) -> windows_core::HRESULT,
    pub Maximize: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    Minimize: usize,
    pub Restore: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IOverlappedPresenter3,
    IOverlappedPresenter3_Vtbl,
    0x55d26138_4c38_57e7_a0c1_d467b774db8c
);
impl windows_core::RuntimeType for IOverlappedPresenter3 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IOverlappedPresenter3 {
    pub fn SetPreferredMinimumHeight(&self, value: Option<i32>) -> windows_core::Result<()> {
        let value__ = value.map(<windows_reference::IReference<i32> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetPreferredMinimumHeight)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
    pub fn SetPreferredMinimumWidth(&self, value: Option<i32>) -> windows_core::Result<()> {
        let value__ = value.map(<windows_reference::IReference<i32> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetPreferredMinimumWidth)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
    pub fn PreferredMaximumHeight(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PreferredMaximumHeight)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
            .and_then(|r__: windows_reference::IReference<i32>| r__.Value())
        }
    }
    pub fn SetPreferredMaximumHeight(&self, value: Option<i32>) -> windows_core::Result<()> {
        let value__ = value.map(<windows_reference::IReference<i32> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetPreferredMaximumHeight)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IOverlappedPresenter3_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PreferredMinimumHeight: usize,
    pub SetPreferredMinimumHeight:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    PreferredMinimumWidth: usize,
    pub SetPreferredMinimumWidth:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    PreferredMaximumWidth: usize,
    SetPreferredMaximumWidth: usize,
    pub PreferredMaximumHeight:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPreferredMaximumHeight:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPanel, IPanel_Vtbl, 0x27a1b418_56f3_525e_b883_cefed905eed3);
impl windows_core::RuntimeType for IPanel {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPanel {
    pub fn Children(&self) -> windows_core::Result<UIElementCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Children)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetBackground<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetBackground)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IPanel_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Children:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    Background: usize,
    pub SetBackground:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPasswordBox, IPasswordBox_Vtbl, 0x6d3ccff7_aaee_5adc_8298_33300fa119da);
impl windows_core::RuntimeType for IPasswordBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPasswordBox {
    pub fn Password(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Password)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetPassword(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPassword)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn PasswordChar(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PasswordChar)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                })
        }
    }
    pub fn SetPasswordChar(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPasswordChar)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn PlaceholderText(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PlaceholderText)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| {
                let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                hstring.to_string_lossy()
            })
        }
    }
    pub fn SetPlaceholderText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPlaceholderText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn PasswordChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PasswordChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePasswordChanged,
            ))
        }
    }
    pub fn SelectAll(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SelectAll)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IPasswordBox_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Password:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPassword: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub PasswordChar:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPasswordChar:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    IsPasswordRevealButtonEnabled: usize,
    SetIsPasswordRevealButtonEnabled: usize,
    MaxLength: usize,
    SetMaxLength: usize,
    Header: usize,
    SetHeader: usize,
    HeaderTemplate: usize,
    SetHeaderTemplate: usize,
    pub PlaceholderText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPlaceholderText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    SelectionHighlightColor: usize,
    SetSelectionHighlightColor: usize,
    PreventKeyboardDisplayOnProgrammaticFocus: usize,
    SetPreventKeyboardDisplayOnProgrammaticFocus: usize,
    PasswordRevealMode: usize,
    SetPasswordRevealMode: usize,
    TextReadingOrder: usize,
    SetTextReadingOrder: usize,
    InputScope: usize,
    SetInputScope: usize,
    CanPasteClipboardContent: usize,
    SelectionFlyout: usize,
    SetSelectionFlyout: usize,
    Description: usize,
    SetDescription: usize,
    pub PasswordChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePasswordChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    ContextMenuOpening: usize,
    RemoveContextMenuOpening: usize,
    Paste: usize,
    RemovePaste: usize,
    PasswordChanging: usize,
    RemovePasswordChanging: usize,
    pub SelectAll: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPickFileResult, IPickFileResult_Vtbl, 0xe6f2e3d6_7bb0_5d81_9e7d_6fd35a1f25ab);
impl windows_core::RuntimeType for IPickFileResult {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPickFileResult {
    pub fn Path(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Path)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
}
#[repr(C)]
pub struct IPickFileResult_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Path: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPickFolderResult, IPickFolderResult_Vtbl, 0x6f7fd316_fe29_59d1_9343_c49cf5cde680);
impl windows_core::RuntimeType for IPickFolderResult {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPickFolderResult {
    pub fn Path(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Path)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
}
#[repr(C)]
pub struct IPickFolderResult_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Path: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPipsPager, IPipsPager_Vtbl, 0xde7fc5d5_9446_5693_bbf3_fd7f943a567c);
impl windows_core::RuntimeType for IPipsPager {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPipsPager {
    pub fn NumberOfPages(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).NumberOfPages)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetNumberOfPages(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetNumberOfPages)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SelectedPageIndex(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedPageIndex)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetSelectedPageIndex(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectedPageIndex)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SelectedIndexChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<PipsPager>, windows_core::Ref<PipsPagerSelectedIndexChangedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<PipsPager, PipsPagerSelectedIndexChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<PipsPager, PipsPagerSelectedIndexChangedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<PipsPager, PipsPagerSelectedIndexChangedEventArgs, F>::VTABLE, handler
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).SelectedIndexChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveSelectedIndexChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct IPipsPager_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub NumberOfPages: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetNumberOfPages: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    pub SelectedPageIndex: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetSelectedPageIndex: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    MaxVisiblePips: usize,
    SetMaxVisiblePips: usize,
    Orientation: usize,
    SetOrientation: usize,
    PreviousButtonVisibility: usize,
    SetPreviousButtonVisibility: usize,
    NextButtonVisibility: usize,
    SetNextButtonVisibility: usize,
    PreviousButtonStyle: usize,
    SetPreviousButtonStyle: usize,
    NextButtonStyle: usize,
    SetNextButtonStyle: usize,
    SelectedPipStyle: usize,
    SetSelectedPipStyle: usize,
    NormalPipStyle: usize,
    SetNormalPipStyle: usize,
    pub SelectedIndexChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveSelectedIndexChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPipsPagerFactory, IPipsPagerFactory_Vtbl, 0x020722cd_813a_5165_a899_3df9adcd805e);
impl windows_core::RuntimeType for IPipsPagerFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPipsPagerFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPipsPagerSelectedIndexChangedEventArgs,
    IPipsPagerSelectedIndexChangedEventArgs_Vtbl,
    0x6c2ce4fc_bf52_5ca6_9da4_b0bd5b928d97
);
impl windows_core::RuntimeType for IPipsPagerSelectedIndexChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPipsPagerSelectedIndexChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IPipsPagerStatics, IPipsPagerStatics_Vtbl, 0x37714cd8_fba6_5d98_a395_0a7a3ea64867);
impl windows_core::RuntimeType for IPipsPagerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPipsPagerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    NumberOfPagesProperty: usize,
    pub SelectedPageIndexProperty:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPointer, IPointer_Vtbl, 0x1f9afbf5_11a3_5e68_aa1b_72febfa0ab23);
impl windows_core::RuntimeType for IPointer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPointer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IPointerPoint, IPointerPoint_Vtbl, 0x0d430ee6_252c_59a4_b2a2_d44264dc6a40);
impl windows_core::RuntimeType for IPointerPoint {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPointerPoint {
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Properties(&self) -> windows_core::Result<PointerPointProperties> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Properties)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IPointerPoint_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FrameId: usize,
    IsInContact: usize,
    PointerDeviceType: usize,
    PointerId: usize,
    pub Position: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub Properties:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerPointProperties,
    IPointerPointProperties_Vtbl,
    0xd760ed77_4b10_57a5_b3cc_d9bf3413e996
);
impl windows_core::RuntimeType for IPointerPointProperties {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPointerPointProperties {
    pub fn IsHorizontalMouseWheel(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsHorizontalMouseWheel)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn MouseWheelDelta(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MouseWheelDelta)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn PointerUpdateKind(&self) -> windows_core::Result<PointerUpdateKind> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerUpdateKind)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IPointerPointProperties_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ContactRect: usize,
    IsBarrelButtonPressed: usize,
    IsCanceled: usize,
    IsEraser: usize,
    pub IsHorizontalMouseWheel: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    IsInRange: usize,
    IsInverted: usize,
    IsLeftButtonPressed: usize,
    IsMiddleButtonPressed: usize,
    IsPrimary: usize,
    IsRightButtonPressed: usize,
    IsXButton1Pressed: usize,
    IsXButton2Pressed: usize,
    pub MouseWheelDelta: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    Orientation: usize,
    pub PointerUpdateKind:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut PointerUpdateKind) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerRoutedEventArgs,
    IPointerRoutedEventArgs_Vtbl,
    0x66e78a9a_1bec_5f92_b1a1_ea6334ee511c
);
impl windows_core::RuntimeType for IPointerRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPointerRoutedEventArgs {
    pub fn Pointer(&self) -> windows_core::Result<Pointer> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Pointer)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn KeyModifiers(&self) -> windows_core::Result<VirtualKeyModifiers> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).KeyModifiers)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetHandled(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetHandled)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn GetCurrentPoint<P0>(&self, relativeto: P0) -> windows_core::Result<PointerPoint>
    where
        P0: windows_core::Param<UIElement>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetCurrentPoint)(
                windows_core::Interface::as_raw(self),
                relativeto.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IPointerRoutedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Pointer:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub KeyModifiers:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut VirtualKeyModifiers) -> windows_core::HRESULT,
    Handled: usize,
    pub SetHandled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsGenerated: usize,
    pub GetCurrentPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPopup, IPopup_Vtbl, 0x4e3ab19d_2f95_579c_9535_906c58629437);
impl windows_core::RuntimeType for IPopup {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPopup {
    pub fn Child(&self) -> windows_core::Result<UIElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Child)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IPopup_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Child: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IProgressBar, IProgressBar_Vtbl, 0x87555c8c_0aaf_52c1_8390_0db17f40438e);
impl windows_core::RuntimeType for IProgressBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IProgressBar {
    pub fn IsIndeterminate(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsIndeterminate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetIsIndeterminate(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsIndeterminate)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetShowPaused(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetShowPaused)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IProgressBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsIndeterminate: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsIndeterminate: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    ShowError: usize,
    SetShowError: usize,
    ShowPaused: usize,
    pub SetShowPaused: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IProgressBarFactory,
    IProgressBarFactory_Vtbl,
    0x189826ad_f6f2_533e_9ddb_b6600e88675b
);
impl windows_core::RuntimeType for IProgressBarFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IProgressBarFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IProgressRing, IProgressRing_Vtbl, 0x2670d03f_e28c_5652_bee2_b5212ebdf7ff);
impl windows_core::RuntimeType for IProgressRing {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IProgressRing {
    pub fn IsActive(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsActive)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsActive(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsActive)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetIsIndeterminate(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsIndeterminate)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetValue)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IProgressRing_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsActive: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsActive: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsIndeterminate: usize,
    pub SetIsIndeterminate: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    TemplateSettings: usize,
    Value: usize,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IProgressRingFactory,
    IProgressRingFactory_Vtbl,
    0x092fa98c_62a7_5dbc_9a85_3e556ba81f79
);
impl windows_core::RuntimeType for IProgressRingFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IProgressRingFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IPropertyValue, IPropertyValue_Vtbl, 0x4bd682dd_7554_40e9_9a9b_82654ede7e62);
impl windows_core::RuntimeType for IPropertyValue {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IPropertyValue, windows_core::IUnknown, windows_core::IInspectable);
impl IPropertyValue {
    pub fn GetBoolean(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetBoolean)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn GetString(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetString)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
}
#[repr(C)]
pub struct IPropertyValue_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Type: usize,
    IsNumericScalar: usize,
    GetUInt8: usize,
    GetInt16: usize,
    GetUInt16: usize,
    GetInt32: usize,
    GetUInt32: usize,
    GetInt64: usize,
    GetUInt64: usize,
    GetSingle: usize,
    GetDouble: usize,
    GetChar16: usize,
    pub GetBoolean: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub GetString:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPropertyValueStatics,
    IPropertyValueStatics_Vtbl,
    0x629bdbc8_d932_4ff4_96b9_8d96c5c1e858
);
impl windows_core::RuntimeType for IPropertyValueStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPropertyValueStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateEmpty: usize,
    CreateUInt8: usize,
    CreateInt16: usize,
    CreateUInt16: usize,
    CreateInt32: usize,
    CreateUInt32: usize,
    CreateInt64: usize,
    CreateUInt64: usize,
    CreateSingle: usize,
    pub CreateDouble:
        unsafe extern "system" fn(*mut core::ffi::c_void, f64, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    CreateChar16: usize,
    pub CreateBoolean:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub CreateString: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IRadioButtons, IRadioButtons_Vtbl, 0x966daf80_ee85_5d90_b6b3_80bec9134673);
impl windows_core::RuntimeType for IRadioButtons {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadioButtons {
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<windows_core::IInspectable>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn ContainerFromIndex(&self, index: i32) -> windows_core::Result<UIElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContainerFromIndex)(
                windows_core::Interface::as_raw(self),
                index,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SelectedIndex(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedIndex)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetSelectedIndex(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectedIndex)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SelectionChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SelectionChangedEventArgs>) + 'static,
    {
        let handler: SelectionChangedEventHandler = {
            let com = windows_core::imp::DelegateBox::<SelectionChangedEventHandler, F>::new(
                &SelectionChangedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).SelectionChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveSelectionChanged,
            ))
        }
    }
    pub fn SetHeader<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetHeader)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IRadioButtons_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ItemsSource: usize,
    SetItemsSource: usize,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    ItemTemplate: usize,
    SetItemTemplate: usize,
    pub ContainerFromIndex:
        unsafe extern "system" fn(*mut core::ffi::c_void, i32, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SelectedIndex: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetSelectedIndex: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    SelectedItem: usize,
    SetSelectedItem: usize,
    pub SelectionChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveSelectionChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    MaxColumns: usize,
    SetMaxColumns: usize,
    Header: usize,
    pub SetHeader: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadioButtonsFactory,
    IRadioButtonsFactory_Vtbl,
    0x2cf95efb_a7a2_5d85_8ead_ea222baa3c55
);
impl windows_core::RuntimeType for IRadioButtonsFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRadioButtonsFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadioMenuFlyoutItem,
    IRadioMenuFlyoutItem_Vtbl,
    0x9d6f45f5_5532_5669_8f02_05735953026a
);
impl windows_core::RuntimeType for IRadioMenuFlyoutItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadioMenuFlyoutItem {
    pub fn IsChecked(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsChecked)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsChecked(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsChecked)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn GroupName(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GroupName)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetGroupName(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetGroupName)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IRadioMenuFlyoutItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsChecked: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsChecked: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub GroupName:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetGroupName:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadioMenuFlyoutItemFactory,
    IRadioMenuFlyoutItemFactory_Vtbl,
    0x113822d2_35a3_5b0d_9d0d_c3830ec1973b
);
impl windows_core::RuntimeType for IRadioMenuFlyoutItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRadioMenuFlyoutItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IRangeBase, IRangeBase_Vtbl, 0x540d6d61_8fac_5d5c_b5b0_e172a7dde103);
impl windows_core::RuntimeType for IRangeBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRangeBase {
    pub fn Minimum(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Minimum)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetMinimum(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetMinimum)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn Maximum(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Maximum)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetMaximum(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetMaximum)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn SmallChange(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SmallChange)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetSmallChange(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSmallChange)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Value(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Value)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetValue)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn ValueChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RangeBaseValueChangedEventArgs>)
            + 'static,
    {
        let handler: RangeBaseValueChangedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RangeBaseValueChangedEventHandler, F>::new(
                &RangeBaseValueChangedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ValueChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveValueChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct IRangeBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Minimum: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetMinimum: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub Maximum: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetMaximum: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub SmallChange: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetSmallChange: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    LargeChange: usize,
    SetLargeChange: usize,
    pub Value: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub ValueChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveValueChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IRangeBaseStatics, IRangeBaseStatics_Vtbl, 0x4aed5e49_64ec_56f1_874d_b8c0f83f9ac8);
impl windows_core::RuntimeType for IRangeBaseStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRangeBaseStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    MinimumProperty: usize,
    MaximumProperty: usize,
    SmallChangeProperty: usize,
    LargeChangeProperty: usize,
    pub ValueProperty:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRangeBaseValueChangedEventArgs,
    IRangeBaseValueChangedEventArgs_Vtbl,
    0xb0181692_9578_51c7_9d1c_adfcf8945aa9
);
impl windows_core::RuntimeType for IRangeBaseValueChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRangeBaseValueChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IRangeValueProvider,
    IRangeValueProvider_Vtbl,
    0x729ae414_1e8f_5020_82bb_bb574d145fd8
);
impl windows_core::RuntimeType for IRangeValueProvider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IRangeValueProvider, windows_core::IUnknown, windows_core::IInspectable);
impl IRangeValueProvider {
    pub fn Maximum(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Maximum)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Minimum(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Minimum)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SmallChange(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SmallChange)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn Value(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Value)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetValue)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IRangeValueProvider_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsReadOnly: usize,
    LargeChange: usize,
    pub Maximum: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub Minimum: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SmallChange: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub Value: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IRatingControl, IRatingControl_Vtbl, 0x5488193b_ea4b_52c6_8544_c063219bcd90);
impl windows_core::RuntimeType for IRatingControl {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRatingControl {
    pub fn SetIsClearEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsClearEnabled)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn IsReadOnly(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsReadOnly)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsReadOnly(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsReadOnly)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn MaxRating(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MaxRating)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetMaxRating(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaxRating)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Value(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Value)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetValue(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetValue)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn ValueChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<RatingControl>, windows_core::Ref<windows_core::IInspectable>) + 'static,
    {
        let handler: TypedEventHandler<RatingControl, windows_core::IInspectable> = {
            let com =
                windows_core::imp::DelegateBox::<TypedEventHandler<RatingControl, windows_core::IInspectable>, F>::new(
                    &TypedEventHandlerBox::<RatingControl, windows_core::IInspectable, F>::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ValueChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveValueChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct IRatingControl_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Caption: usize,
    SetCaption: usize,
    InitialSetValue: usize,
    SetInitialSetValue: usize,
    IsClearEnabled: usize,
    pub SetIsClearEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub IsReadOnly: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsReadOnly: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub MaxRating: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetMaxRating: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    PlaceholderValue: usize,
    SetPlaceholderValue: usize,
    ItemInfo: usize,
    SetItemInfo: usize,
    pub Value: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub ValueChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveValueChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRatingControlFactory,
    IRatingControlFactory_Vtbl,
    0xa53b9b73_bff9_548d_a294_ac63d819f78a
);
impl windows_core::RuntimeType for IRatingControlFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRatingControlFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRatingControlStatics,
    IRatingControlStatics_Vtbl,
    0xdac61d65_e8f9_5e4d_813d_05c980b2f118
);
impl windows_core::RuntimeType for IRatingControlStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRatingControlStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CaptionProperty: usize,
    InitialSetValueProperty: usize,
    IsClearEnabledProperty: usize,
    IsReadOnlyProperty: usize,
    MaxRatingProperty: usize,
    PlaceholderValueProperty: usize,
    ItemInfoProperty: usize,
    pub ValueProperty:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRectangleGeometry,
    IRectangleGeometry_Vtbl,
    0xb6143890_a5f5_54e0_ab42_d88bab451f04
);
impl windows_core::RuntimeType for IRectangleGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRectangleGeometry {
    pub fn SetRect(&self, value: Rect) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetRect)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IRectangleGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Rect: usize,
    pub SetRect: unsafe extern "system" fn(*mut core::ffi::c_void, Rect) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRenderTargetBitmap,
    IRenderTargetBitmap_Vtbl,
    0xcf10407d_fa8b_57a3_9574_710529ae0b04
);
impl windows_core::RuntimeType for IRenderTargetBitmap {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRenderTargetBitmap {
    pub fn PixelWidth(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PixelWidth)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn PixelHeight(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PixelHeight)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn RenderAsync<P0>(&self, element: P0) -> windows_core::Result<windows_future::IAsyncAction>
    where
        P0: windows_core::Param<UIElement>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RenderAsync)(
                windows_core::Interface::as_raw(self),
                element.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn GetPixelsAsync(&self) -> windows_core::Result<windows_future::IAsyncOperation<IBuffer>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetPixelsAsync)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRenderTargetBitmap_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PixelWidth: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub PixelHeight: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub RenderAsync: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    RenderToSizeAsync: usize,
    pub GetPixelsAsync:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IResourceDictionary,
    IResourceDictionary_Vtbl,
    0x1b690975_a710_5783_a6e1_15836f6186c2
);
impl windows_core::RuntimeType for IResourceDictionary {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IResourceDictionary {
    pub fn MergedDictionaries(&self) -> windows_core::Result<windows_collections::IVector<ResourceDictionary>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MergedDictionaries)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IResourceDictionary_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Source: usize,
    SetSource: usize,
    pub MergedDictionaries:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IRoutedEventArgs, IRoutedEventArgs_Vtbl, 0x0908c407_1c7d_5de3_9c50_d971c62ec8ec);
impl windows_core::RuntimeType for IRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRoutedEventArgs {
    pub fn OriginalSource(&self) -> windows_core::Result<windows_core::IInspectable> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).OriginalSource)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRoutedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub OriginalSource:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IScrollViewer, IScrollViewer_Vtbl, 0x1dc28c2e_996c_5394_89c3_4dc656b4ad46);
impl windows_core::RuntimeType for IScrollViewer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IScrollViewer {
    pub fn HorizontalScrollBarVisibility(&self) -> windows_core::Result<ScrollBarVisibility> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).HorizontalScrollBarVisibility)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetHorizontalScrollBarVisibility(&self, value: ScrollBarVisibility) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHorizontalScrollBarVisibility)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn VerticalScrollBarVisibility(&self) -> windows_core::Result<ScrollBarVisibility> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).VerticalScrollBarVisibility)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetVerticalScrollBarVisibility(&self, value: ScrollBarVisibility) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalScrollBarVisibility)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn IsScrollInertiaEnabled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsScrollInertiaEnabled)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetIsScrollInertiaEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsScrollInertiaEnabled)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHorizontalScrollMode(&self, value: ScrollMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHorizontalScrollMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetVerticalScrollMode(&self, value: ScrollMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalScrollMode)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn HorizontalOffset(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).HorizontalOffset)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn ScrollableWidth(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ScrollableWidth)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn VerticalOffset(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).VerticalOffset)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn ScrollableHeight(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ScrollableHeight)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn ViewChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<ScrollViewerViewChangedEventArgs>)
            + 'static,
    {
        let handler: EventHandler<ScrollViewerViewChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<EventHandler<ScrollViewerViewChangedEventArgs>, F>::new(
                &EventHandlerBox::<ScrollViewerViewChangedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ViewChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveViewChanged,
            ))
        }
    }
    pub fn ChangeViewWithOptionalAnimation(
        &self,
        horizontaloffset: Option<f64>,
        verticaloffset: Option<f64>,
        zoomfactor: Option<f32>,
        disableanimation: bool,
    ) -> windows_core::Result<bool> {
        let horizontaloffset__ = horizontaloffset.map(<windows_reference::IReference<f64> as From<_>>::from);
        let verticaloffset__ = verticaloffset.map(<windows_reference::IReference<f64> as From<_>>::from);
        let zoomfactor__ = zoomfactor.map(<windows_reference::IReference<f32> as From<_>>::from);
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ChangeViewWithOptionalAnimation)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(horizontaloffset__.as_ref()).abi(),
                windows_core::Param::param(verticaloffset__.as_ref()).abi(),
                windows_core::Param::param(zoomfactor__.as_ref()).abi(),
                disableanimation,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IScrollViewer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub HorizontalScrollBarVisibility:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut ScrollBarVisibility) -> windows_core::HRESULT,
    pub SetHorizontalScrollBarVisibility:
        unsafe extern "system" fn(*mut core::ffi::c_void, ScrollBarVisibility) -> windows_core::HRESULT,
    pub VerticalScrollBarVisibility:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut ScrollBarVisibility) -> windows_core::HRESULT,
    pub SetVerticalScrollBarVisibility:
        unsafe extern "system" fn(*mut core::ffi::c_void, ScrollBarVisibility) -> windows_core::HRESULT,
    IsHorizontalRailEnabled: usize,
    SetIsHorizontalRailEnabled: usize,
    IsVerticalRailEnabled: usize,
    SetIsVerticalRailEnabled: usize,
    IsHorizontalScrollChainingEnabled: usize,
    SetIsHorizontalScrollChainingEnabled: usize,
    IsVerticalScrollChainingEnabled: usize,
    SetIsVerticalScrollChainingEnabled: usize,
    IsZoomChainingEnabled: usize,
    SetIsZoomChainingEnabled: usize,
    pub IsScrollInertiaEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsScrollInertiaEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsZoomInertiaEnabled: usize,
    SetIsZoomInertiaEnabled: usize,
    HorizontalScrollMode: usize,
    pub SetHorizontalScrollMode: unsafe extern "system" fn(*mut core::ffi::c_void, ScrollMode) -> windows_core::HRESULT,
    VerticalScrollMode: usize,
    pub SetVerticalScrollMode: unsafe extern "system" fn(*mut core::ffi::c_void, ScrollMode) -> windows_core::HRESULT,
    ZoomMode: usize,
    SetZoomMode: usize,
    HorizontalSnapPointsAlignment: usize,
    SetHorizontalSnapPointsAlignment: usize,
    VerticalSnapPointsAlignment: usize,
    SetVerticalSnapPointsAlignment: usize,
    HorizontalSnapPointsType: usize,
    SetHorizontalSnapPointsType: usize,
    VerticalSnapPointsType: usize,
    SetVerticalSnapPointsType: usize,
    ZoomSnapPointsType: usize,
    SetZoomSnapPointsType: usize,
    pub HorizontalOffset: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    ViewportWidth: usize,
    pub ScrollableWidth: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    ComputedHorizontalScrollBarVisibility: usize,
    ExtentWidth: usize,
    pub VerticalOffset: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    ViewportHeight: usize,
    pub ScrollableHeight: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    ComputedVerticalScrollBarVisibility: usize,
    ExtentHeight: usize,
    MinZoomFactor: usize,
    SetMinZoomFactor: usize,
    MaxZoomFactor: usize,
    SetMaxZoomFactor: usize,
    ZoomFactor: usize,
    ZoomSnapPoints: usize,
    TopLeftHeader: usize,
    SetTopLeftHeader: usize,
    LeftHeader: usize,
    SetLeftHeader: usize,
    TopHeader: usize,
    SetTopHeader: usize,
    ReduceViewportForCoreInputViewOcclusions: usize,
    SetReduceViewportForCoreInputViewOcclusions: usize,
    HorizontalAnchorRatio: usize,
    SetHorizontalAnchorRatio: usize,
    VerticalAnchorRatio: usize,
    SetVerticalAnchorRatio: usize,
    CanContentRenderOutsideBounds: usize,
    SetCanContentRenderOutsideBounds: usize,
    AnchorRequested: usize,
    RemoveAnchorRequested: usize,
    ViewChanging: usize,
    RemoveViewChanging: usize,
    pub ViewChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveViewChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    DirectManipulationStarted: usize,
    RemoveDirectManipulationStarted: usize,
    DirectManipulationCompleted: usize,
    RemoveDirectManipulationCompleted: usize,
    ScrollToHorizontalOffset: usize,
    ScrollToVerticalOffset: usize,
    ZoomToFactor: usize,
    ChangeView: usize,
    pub ChangeViewWithOptionalAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        bool,
        *mut bool,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IScrollViewerStatics,
    IScrollViewerStatics_Vtbl,
    0xd971fd86_0a96_50c4_a6e1_9975faa2a142
);
impl windows_core::RuntimeType for IScrollViewerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IScrollViewerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    HorizontalSnapPointsAlignmentProperty: usize,
    VerticalSnapPointsAlignmentProperty: usize,
    HorizontalSnapPointsTypeProperty: usize,
    VerticalSnapPointsTypeProperty: usize,
    ZoomSnapPointsTypeProperty: usize,
    HorizontalOffsetProperty: usize,
    ViewportWidthProperty: usize,
    ScrollableWidthProperty: usize,
    ComputedHorizontalScrollBarVisibilityProperty: usize,
    ExtentWidthProperty: usize,
    VerticalOffsetProperty: usize,
    ViewportHeightProperty: usize,
    ScrollableHeightProperty: usize,
    ComputedVerticalScrollBarVisibilityProperty: usize,
    ExtentHeightProperty: usize,
    MinZoomFactorProperty: usize,
    MaxZoomFactorProperty: usize,
    ZoomFactorProperty: usize,
    ZoomSnapPointsProperty: usize,
    TopLeftHeaderProperty: usize,
    LeftHeaderProperty: usize,
    TopHeaderProperty: usize,
    ReduceViewportForCoreInputViewOcclusionsProperty: usize,
    HorizontalAnchorRatioProperty: usize,
    VerticalAnchorRatioProperty: usize,
    HorizontalScrollBarVisibilityProperty: usize,
    GetHorizontalScrollBarVisibility: usize,
    pub SetHorizontalScrollBarVisibility: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        ScrollBarVisibility,
    ) -> windows_core::HRESULT,
    VerticalScrollBarVisibilityProperty: usize,
    GetVerticalScrollBarVisibility: usize,
    pub SetVerticalScrollBarVisibility: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        ScrollBarVisibility,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IScrollViewerViewChangedEventArgs,
    IScrollViewerViewChangedEventArgs_Vtbl,
    0xbf7bb85b_1d46_5004_a370_ecb626630588
);
impl windows_core::RuntimeType for IScrollViewerViewChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IScrollViewerViewChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ISelectionChangedEventArgs,
    ISelectionChangedEventArgs_Vtbl,
    0xb6c18076_4b76_5416_ad29_e2dc20c46246
);
impl windows_core::RuntimeType for ISelectionChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISelectionChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(ISelector, ISelector_Vtbl, 0x8f7e2159_e61d_576f_8476_f83fde3d689e);
impl windows_core::RuntimeType for ISelector {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISelector {
    pub fn SelectedIndex(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedIndex)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetSelectedIndex(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectedIndex)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SelectionChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SelectionChangedEventArgs>) + 'static,
    {
        let handler: SelectionChangedEventHandler = {
            let com = windows_core::imp::DelegateBox::<SelectionChangedEventHandler, F>::new(
                &SelectionChangedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).SelectionChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveSelectionChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct ISelector_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub SelectedIndex: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetSelectedIndex: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    SelectedItem: usize,
    SetSelectedItem: usize,
    SelectedValue: usize,
    SetSelectedValue: usize,
    SelectedValuePath: usize,
    SetSelectedValuePath: usize,
    IsSynchronizedWithCurrentItem: usize,
    SetIsSynchronizedWithCurrentItem: usize,
    pub SelectionChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveSelectionChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ISelectorBar, ISelectorBar_Vtbl, 0x7f4ad191_55ea_508e_bf47_7047d8677370);
impl windows_core::RuntimeType for ISelectorBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISelectorBar {
    pub fn Items(&self) -> windows_core::Result<windows_collections::IVector<SelectorBarItem>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SelectedItem(&self) -> windows_core::Result<SelectorBarItem> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedItem)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetSelectedItem<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<SelectorBarItem>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectedItem)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SelectionChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<SelectorBar>, windows_core::Ref<SelectorBarSelectionChangedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<SelectorBar, SelectorBarSelectionChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<SelectorBar, SelectorBarSelectionChangedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<SelectorBar, SelectorBarSelectionChangedEventArgs, F>::VTABLE, handler
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).SelectionChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveSelectionChanged,
            ))
        }
    }
}
#[repr(C)]
pub struct ISelectorBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Items: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SelectedItem:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetSelectedItem:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SelectionChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveSelectionChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISelectorBarFactory,
    ISelectorBarFactory_Vtbl,
    0x71243dc7_b46c_5a04_9894_e420e462703f
);
impl windows_core::RuntimeType for ISelectorBarFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISelectorBarFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ISelectorBarItem, ISelectorBarItem_Vtbl, 0x3cdba1f9_a13a_56a2_b9a9_f954998d3658);
impl windows_core::RuntimeType for ISelectorBarItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISelectorBarItem {
    pub fn Text(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Text)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn Icon(&self) -> windows_core::Result<IconElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Icon)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetIcon<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<IconElement>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetIcon)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
}
#[repr(C)]
pub struct ISelectorBarItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Icon: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetIcon: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISelectorBarItemFactory,
    ISelectorBarItemFactory_Vtbl,
    0xe46b62ea_e60d_5989_bea7_5470da326816
);
impl windows_core::RuntimeType for ISelectorBarItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISelectorBarItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISelectorBarSelectionChangedEventArgs,
    ISelectorBarSelectionChangedEventArgs_Vtbl,
    0x73b3f6c5_5050_5c5a_899c_4e6e0474cb63
);
impl windows_core::RuntimeType for ISelectorBarSelectionChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISelectorBarSelectionChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(ISelectorItem, ISelectorItem_Vtbl, 0x5772c4de_60ea_5492_8c5e_b3323d5a3ca6);
impl windows_core::RuntimeType for ISelectorItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISelectorItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(IShellItem, IShellItem_Vtbl, 0x43826d1e_e718_42ee_bc55_a1e261c37bfe);
windows_core::imp::interface_hierarchy!(IShellItem, windows_core::IUnknown);
#[repr(C)]
pub struct IShellItem_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    BindToHandler: usize,
    GetParent: usize,
    GetDisplayName: usize,
    GetAttributes: usize,
    Compare: usize,
}
windows_core::imp::define_interface!(
    IShellItemImageFactory,
    IShellItemImageFactory_Vtbl,
    0xbcc18b79_ba16_442f_80c4_8a59c30c463b
);
windows_core::imp::interface_hierarchy!(IShellItemImageFactory, windows_core::IUnknown);
impl IShellItemImageFactory {
    pub unsafe fn GetImage(&self, size: SIZE, flags: SIIGBF) -> windows_core::Result<HBITMAP> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetImage)(
                windows_core::Interface::as_raw(self),
                size,
                flags,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IShellItemImageFactory_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetImage:
        unsafe extern "system" fn(*mut core::ffi::c_void, SIZE, SIIGBF, *mut HBITMAP) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISizeChangedEventArgs,
    ISizeChangedEventArgs_Vtbl,
    0xfe76324e_6dfb_58b1_9dcd_886ca8f9a2ea
);
impl windows_core::RuntimeType for ISizeChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISizeChangedEventArgs {
    pub fn NewSize(&self) -> windows_core::Result<Size> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).NewSize)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ISizeChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PreviousSize: usize,
    pub NewSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Size) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ISlider, ISlider_Vtbl, 0xf7418ecf_7c35_5216_8bf1_d82d47cce5df);
impl windows_core::RuntimeType for ISlider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISlider {
    pub fn StepFrequency(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).StepFrequency)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetStepFrequency(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStepFrequency)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetTickFrequency(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTickFrequency)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetTickPlacement(&self, value: TickPlacement) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTickPlacement)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Orientation(&self) -> windows_core::Result<Orientation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Orientation)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetOrientation(&self, value: Orientation) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOrientation)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct ISlider_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IntermediateValue: usize,
    SetIntermediateValue: usize,
    pub StepFrequency: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SetStepFrequency: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    SnapsTo: usize,
    SetSnapsTo: usize,
    TickFrequency: usize,
    pub SetTickFrequency: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    TickPlacement: usize,
    pub SetTickPlacement: unsafe extern "system" fn(*mut core::ffi::c_void, TickPlacement) -> windows_core::HRESULT,
    pub Orientation: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Orientation) -> windows_core::HRESULT,
    pub SetOrientation: unsafe extern "system" fn(*mut core::ffi::c_void, Orientation) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ISliderFactory, ISliderFactory_Vtbl, 0x06604d71_34ca_5f39_9656_29d81d3c110c);
impl windows_core::RuntimeType for ISliderFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISliderFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ISolidColorBrush, ISolidColorBrush_Vtbl, 0xb3865c31_37c8_55c1_8a72_d41c67642e2a);
impl windows_core::RuntimeType for ISolidColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISolidColorBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ISolidColorBrushFactory,
    ISolidColorBrushFactory_Vtbl,
    0x7b559384_4daa_54f4_91ef_33a23fd816ca
);
impl windows_core::RuntimeType for ISolidColorBrushFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISolidColorBrushFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstanceWithColor:
        unsafe extern "system" fn(*mut core::ffi::c_void, Color, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IStackPanel, IStackPanel_Vtbl, 0x493ab00b_3a6a_5e4a_9452_407cd5197406);
impl windows_core::RuntimeType for IStackPanel {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IStackPanel {
    pub fn SetOrientation(&self, value: Orientation) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOrientation)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetSpacing(&self, value: f64) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetSpacing)(windows_core::Interface::as_raw(self), value).ok() }
    }
}
#[repr(C)]
pub struct IStackPanel_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AreScrollSnapPointsRegular: usize,
    SetAreScrollSnapPointsRegular: usize,
    Orientation: usize,
    pub SetOrientation: unsafe extern "system" fn(*mut core::ffi::c_void, Orientation) -> windows_core::HRESULT,
    BackgroundSizing: usize,
    SetBackgroundSizing: usize,
    BorderBrush: usize,
    SetBorderBrush: usize,
    BorderThickness: usize,
    SetBorderThickness: usize,
    CornerRadius: usize,
    SetCornerRadius: usize,
    Padding: usize,
    SetPadding: usize,
    Spacing: usize,
    pub SetSpacing: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IStackPanelFactory,
    IStackPanelFactory_Vtbl,
    0x64c1d388_47a2_5a74_a75b_559d151ee5ac
);
impl windows_core::RuntimeType for IStackPanelFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IStackPanelFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IStandardDataFormatsStatics,
    IStandardDataFormatsStatics_Vtbl,
    0x7ed681a1_a880_40c9_b4ed_0bee1e15f549
);
impl windows_core::RuntimeType for IStandardDataFormatsStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IStandardDataFormatsStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    Uri: usize,
    Html: usize,
    Rtf: usize,
    Bitmap: usize,
    pub StorageItems:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IStorageFile, IStorageFile_Vtbl, 0xfa3f6186_4214_428c_a64c_14c9ac7315ea);
impl windows_core::RuntimeType for IStorageFile {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IStorageFile, windows_core::IUnknown, windows_core::IInspectable);
#[repr(C)]
pub struct IStorageFile_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IStorageFileStatics,
    IStorageFileStatics_Vtbl,
    0x5984c710_daf2_43c8_8bb4_a4d3eacfd03f
);
impl windows_core::RuntimeType for IStorageFileStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IStorageFileStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetFileFromPathAsync: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IStorageFolder, IStorageFolder_Vtbl, 0x72d1cb78_b3ef_4f75_a80b_6fd9dae2944b);
impl windows_core::RuntimeType for IStorageFolder {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IStorageFolder, windows_core::IUnknown, windows_core::IInspectable);
#[repr(C)]
pub struct IStorageFolder_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IStorageFolderStatics,
    IStorageFolderStatics_Vtbl,
    0x08f327ff_85d5_48b9_aee9_28511e339f9f
);
impl windows_core::RuntimeType for IStorageFolderStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IStorageFolderStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetFolderFromPathAsync: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IStorageItem, IStorageItem_Vtbl, 0x4207a996_ca2f_42f7_bde8_8b10457a7f30);
impl windows_core::RuntimeType for IStorageItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IStorageItem, windows_core::IUnknown, windows_core::IInspectable);
impl IStorageItem {
    pub fn Path(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Path)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
}
#[repr(C)]
pub struct IStorageItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    RenameAsyncOverloadDefaultOptions: usize,
    RenameAsync: usize,
    DeleteAsyncOverloadDefaultOptions: usize,
    DeleteAsync: usize,
    GetBasicPropertiesAsync: usize,
    Name: usize,
    pub Path: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IStyle, IStyle_Vtbl, 0x65e1d164_572f_5b0e_a80f_9c02441fac49);
impl windows_core::RuntimeType for IStyle {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IStyle {
    pub fn SetBasedOn<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Style>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetBasedOn)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IStyle_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsSealed: usize,
    Setters: usize,
    TargetType: usize,
    SetTargetType: usize,
    BasedOn: usize,
    pub SetBasedOn: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ISymbolIcon, ISymbolIcon_Vtbl, 0xa4322906_0dbe_5eb7_8b64_3e832246eb7f);
impl windows_core::RuntimeType for ISymbolIcon {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISymbolIcon_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ISymbolIconFactory,
    ISymbolIconFactory_Vtbl,
    0xd4430447_567c_5aad_996a_a547774e2c3c
);
impl windows_core::RuntimeType for ISymbolIconFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ISymbolIconFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstanceWithSymbol:
        unsafe extern "system" fn(*mut core::ffi::c_void, Symbol, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ITextBlock, ITextBlock_Vtbl, 0x1ac8d84f_392c_5c7e_83f5_a53e3bf0abb0);
impl windows_core::RuntimeType for ITextBlock {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ITextBlock {
    pub fn SetFontSize(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontSize)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetFontFamily<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<FontFamily>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontFamily)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn FontWeight(&self) -> windows_core::Result<FontWeight> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FontWeight)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetFontWeight(&self, value: FontWeight) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontWeight)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn FontStyle(&self) -> windows_core::Result<FontStyle> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FontStyle)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetFontStyle(&self, value: FontStyle) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontStyle)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetCharacterSpacing(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCharacterSpacing)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SetTextWrapping(&self, value: TextWrapping) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextWrapping)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetTextTrimming(&self, value: TextTrimming) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextTrimming)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn TextAlignment(&self) -> windows_core::Result<TextAlignment> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TextAlignment)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetTextAlignment(&self, value: TextAlignment) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextAlignment)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn Text(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Text)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn IsTextSelectionEnabled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsTextSelectionEnabled)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetIsTextSelectionEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsTextSelectionEnabled)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn MaxLines(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MaxLines)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetMaxLines(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaxLines)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct ITextBlock_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FontSize: usize,
    pub SetFontSize: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    FontFamily: usize,
    pub SetFontFamily:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub FontWeight: unsafe extern "system" fn(*mut core::ffi::c_void, *mut FontWeight) -> windows_core::HRESULT,
    pub SetFontWeight: unsafe extern "system" fn(*mut core::ffi::c_void, FontWeight) -> windows_core::HRESULT,
    pub FontStyle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut FontStyle) -> windows_core::HRESULT,
    pub SetFontStyle: unsafe extern "system" fn(*mut core::ffi::c_void, FontStyle) -> windows_core::HRESULT,
    FontStretch: usize,
    SetFontStretch: usize,
    CharacterSpacing: usize,
    pub SetCharacterSpacing: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    Foreground: usize,
    SetForeground: usize,
    TextWrapping: usize,
    pub SetTextWrapping: unsafe extern "system" fn(*mut core::ffi::c_void, TextWrapping) -> windows_core::HRESULT,
    TextTrimming: usize,
    pub SetTextTrimming: unsafe extern "system" fn(*mut core::ffi::c_void, TextTrimming) -> windows_core::HRESULT,
    pub TextAlignment: unsafe extern "system" fn(*mut core::ffi::c_void, *mut TextAlignment) -> windows_core::HRESULT,
    pub SetTextAlignment: unsafe extern "system" fn(*mut core::ffi::c_void, TextAlignment) -> windows_core::HRESULT,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Inlines: usize,
    Padding: usize,
    SetPadding: usize,
    LineHeight: usize,
    SetLineHeight: usize,
    LineStackingStrategy: usize,
    SetLineStackingStrategy: usize,
    pub IsTextSelectionEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsTextSelectionEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    SelectedText: usize,
    ContentStart: usize,
    ContentEnd: usize,
    SelectionStart: usize,
    SelectionEnd: usize,
    BaselineOffset: usize,
    SelectionHighlightColor: usize,
    SetSelectionHighlightColor: usize,
    pub MaxLines: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetMaxLines: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ITextBox, ITextBox_Vtbl, 0x873af7c2_ab89_5d76_8dbe_3d6325669df5);
impl windows_core::RuntimeType for ITextBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ITextBox {
    pub fn Text(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Text)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SelectedText(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectedText)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                })
        }
    }
    pub fn SetSelectedText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectedText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SelectionLength(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectionLength)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetSelectionLength(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectionLength)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn SelectionStart(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SelectionStart)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetSelectionStart(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelectionStart)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn IsReadOnly(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsReadOnly)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsReadOnly(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsReadOnly)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn AcceptsReturn(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AcceptsReturn)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetAcceptsReturn(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetAcceptsReturn)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn TextWrapping(&self) -> windows_core::Result<TextWrapping> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TextWrapping)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetTextWrapping(&self, value: TextWrapping) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextWrapping)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn InputScope(&self) -> windows_core::Result<InputScope> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).InputScope)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetInputScope<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InputScope>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetInputScope)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetHeader<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetHeader)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn PlaceholderText(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PlaceholderText)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| {
                let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                hstring.to_string_lossy()
            })
        }
    }
    pub fn SetPlaceholderText(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPlaceholderText)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn TextChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<TextChangedEventArgs>) + 'static,
    {
        let handler: TextChangedEventHandler = {
            let com = windows_core::imp::DelegateBox::<TextChangedEventHandler, F>::new(
                &TextChangedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).TextChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveTextChanged,
            ))
        }
    }
    pub fn Select(&self, start: i32, length: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Select)(windows_core::Interface::as_raw(self), start, length).ok()
        }
    }
}
#[repr(C)]
pub struct ITextBox_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Text: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SelectedText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetSelectedText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SelectionLength: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetSelectionLength: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    pub SelectionStart: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub SetSelectionStart: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    MaxLength: usize,
    SetMaxLength: usize,
    pub IsReadOnly: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsReadOnly: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub AcceptsReturn: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetAcceptsReturn: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    TextAlignment: usize,
    SetTextAlignment: usize,
    pub TextWrapping: unsafe extern "system" fn(*mut core::ffi::c_void, *mut TextWrapping) -> windows_core::HRESULT,
    pub SetTextWrapping: unsafe extern "system" fn(*mut core::ffi::c_void, TextWrapping) -> windows_core::HRESULT,
    IsSpellCheckEnabled: usize,
    SetIsSpellCheckEnabled: usize,
    IsTextPredictionEnabled: usize,
    SetIsTextPredictionEnabled: usize,
    pub InputScope:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetInputScope:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Header: usize,
    pub SetHeader: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    HeaderTemplate: usize,
    SetHeaderTemplate: usize,
    pub PlaceholderText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetPlaceholderText:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    SelectionHighlightColor: usize,
    SetSelectionHighlightColor: usize,
    PreventKeyboardDisplayOnProgrammaticFocus: usize,
    SetPreventKeyboardDisplayOnProgrammaticFocus: usize,
    IsColorFontEnabled: usize,
    SetIsColorFontEnabled: usize,
    SelectionHighlightColorWhenNotFocused: usize,
    SetSelectionHighlightColorWhenNotFocused: usize,
    HorizontalTextAlignment: usize,
    SetHorizontalTextAlignment: usize,
    CharacterCasing: usize,
    SetCharacterCasing: usize,
    PlaceholderForeground: usize,
    SetPlaceholderForeground: usize,
    CanPasteClipboardContent: usize,
    CanUndo: usize,
    CanRedo: usize,
    SelectionFlyout: usize,
    SetSelectionFlyout: usize,
    ProofingMenuFlyout: usize,
    Description: usize,
    SetDescription: usize,
    pub TextChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveTextChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    SelectionChanged: usize,
    RemoveSelectionChanged: usize,
    ContextMenuOpening: usize,
    RemoveContextMenuOpening: usize,
    Paste: usize,
    RemovePaste: usize,
    TextCompositionStarted: usize,
    RemoveTextCompositionStarted: usize,
    TextCompositionChanged: usize,
    RemoveTextCompositionChanged: usize,
    TextCompositionEnded: usize,
    RemoveTextCompositionEnded: usize,
    CopyingToClipboard: usize,
    RemoveCopyingToClipboard: usize,
    CuttingToClipboard: usize,
    RemoveCuttingToClipboard: usize,
    BeforeTextChanging: usize,
    RemoveBeforeTextChanging: usize,
    SelectionChanging: usize,
    RemoveSelectionChanging: usize,
    pub Select: unsafe extern "system" fn(*mut core::ffi::c_void, i32, i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(ITextBoxFactory, ITextBoxFactory_Vtbl, 0xe1d8b82e_bc60_5d27_b646_5ca4c4a69432);
impl windows_core::RuntimeType for ITextBoxFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ITextBoxFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITextChangedEventArgs,
    ITextChangedEventArgs_Vtbl,
    0x71c37e43_7be7_52fc_bf8c_9867f44be5f4
);
impl windows_core::RuntimeType for ITextChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ITextChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(ITitleBar, ITitleBar_Vtbl, 0xc552714d_5d30_5a2b_9c7a_d68bea3dde8d);
impl windows_core::RuntimeType for ITitleBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ITitleBar {
    pub fn Title(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Title)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetTitle(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTitle)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetIsPaneToggleButtonVisible(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPaneToggleButtonVisible)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn PaneToggleRequested<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<TitleBar>, windows_core::Ref<windows_core::IInspectable>) + 'static,
    {
        let handler: TypedEventHandler<TitleBar, windows_core::IInspectable> = {
            let com = windows_core::imp::DelegateBox::<TypedEventHandler<TitleBar, windows_core::IInspectable>, F>::new(
                &TypedEventHandlerBox::<TitleBar, windows_core::IInspectable, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PaneToggleRequested)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePaneToggleRequested,
            ))
        }
    }
}
#[repr(C)]
pub struct ITitleBar_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Title: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetTitle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Subtitle: usize,
    SetSubtitle: usize,
    IconSource: usize,
    SetIconSource: usize,
    LeftHeader: usize,
    SetLeftHeader: usize,
    Content: usize,
    SetContent: usize,
    RightHeader: usize,
    SetRightHeader: usize,
    IsBackButtonVisible: usize,
    SetIsBackButtonVisible: usize,
    IsBackButtonEnabled: usize,
    SetIsBackButtonEnabled: usize,
    IsPaneToggleButtonVisible: usize,
    pub SetIsPaneToggleButtonVisible: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    TemplateSettings: usize,
    BackRequested: usize,
    RemoveBackRequested: usize,
    pub PaneToggleRequested:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePaneToggleRequested: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IToggleButton, IToggleButton_Vtbl, 0x686fbaa4_c866_568b_8f75_481d8d545291);
impl windows_core::RuntimeType for IToggleButton {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IToggleButton {
    pub fn IsChecked(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsChecked)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
                .and_then(|r__: windows_reference::IReference<bool>| r__.Value())
        }
    }
    pub fn SetIsChecked(&self, value: Option<bool>) -> windows_core::Result<()> {
        let value__ = value.map(<windows_reference::IReference<bool> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetIsChecked)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
    pub fn Checked<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Checked)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveChecked,
            ))
        }
    }
    pub fn Unchecked<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Unchecked)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveUnchecked,
            ))
        }
    }
}
#[repr(C)]
pub struct IToggleButton_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsChecked:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetIsChecked:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    IsThreeState: usize,
    SetIsThreeState: usize,
    pub Checked:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveChecked: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Unchecked:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveUnchecked: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IToggleButtonFactory,
    IToggleButtonFactory_Vtbl,
    0x519511bb_d35b_5e2d_966c_8369405a4408
);
impl windows_core::RuntimeType for IToggleButtonFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IToggleButtonFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IToggleMenuFlyoutItem,
    IToggleMenuFlyoutItem_Vtbl,
    0x1803f260_67e4_5bc1_a63a_123510167bb8
);
impl windows_core::RuntimeType for IToggleMenuFlyoutItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IToggleMenuFlyoutItem {
    pub fn IsChecked(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsChecked)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsChecked(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsChecked)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
}
#[repr(C)]
pub struct IToggleMenuFlyoutItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsChecked: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsChecked: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IToggleMenuFlyoutItemFactory,
    IToggleMenuFlyoutItemFactory_Vtbl,
    0x426dfd57_6cc9_570f_950d_37437235dc89
);
impl windows_core::RuntimeType for IToggleMenuFlyoutItemFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IToggleMenuFlyoutItemFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IToggleProvider, IToggleProvider_Vtbl, 0x021080c2_30a9_52ef_bc32_2b79847b6ba7);
impl windows_core::RuntimeType for IToggleProvider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IToggleProvider, windows_core::IUnknown, windows_core::IInspectable);
impl IToggleProvider {
    pub fn Toggle(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Toggle)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IToggleProvider_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ToggleState: usize,
    pub Toggle: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IToggleSwitch, IToggleSwitch_Vtbl, 0x1b17eeb1_74bf_5a83_8161_a86f0fdcdf24);
impl windows_core::RuntimeType for IToggleSwitch {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IToggleSwitch {
    pub fn IsOn(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsOn)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsOn(&self, value: bool) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).SetIsOn)(windows_core::Interface::as_raw(self), value).ok() }
    }
    pub fn SetOnContent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetOnContent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetOffContent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_core::IInspectable>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetOffContent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn Toggled<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Toggled)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveToggled,
            ))
        }
    }
}
#[repr(C)]
pub struct IToggleSwitch_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsOn: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetIsOn: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    Header: usize,
    SetHeader: usize,
    HeaderTemplate: usize,
    SetHeaderTemplate: usize,
    OnContent: usize,
    pub SetOnContent:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    OnContentTemplate: usize,
    SetOnContentTemplate: usize,
    OffContent: usize,
    pub SetOffContent:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    OffContentTemplate: usize,
    SetOffContentTemplate: usize,
    TemplateSettings: usize,
    pub Toggled:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveToggled: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IToolTipService, IToolTipService_Vtbl, 0x01140768_2727_5f89_80e0_5210326a3431);
impl windows_core::RuntimeType for IToolTipService {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IToolTipService_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IToolTipServiceStatics,
    IToolTipServiceStatics_Vtbl,
    0x5aa38adc_9874_5e0a_8d8e_1574efc0b88f
);
impl windows_core::RuntimeType for IToolTipServiceStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IToolTipServiceStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PlacementProperty: usize,
    GetPlacement: usize,
    SetPlacement: usize,
    PlacementTargetProperty: usize,
    GetPlacementTarget: usize,
    SetPlacementTarget: usize,
    ToolTipProperty: usize,
    pub GetToolTip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetToolTip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IUIElement, IUIElement_Vtbl, 0xc3c01020_320c_5cf6_9d24_d396bbfa4d8b);
impl windows_core::RuntimeType for IUIElement {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IUIElement {
    pub fn DesiredSize(&self) -> windows_core::Result<Size> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DesiredSize)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn AllowDrop(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AllowDrop)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetAllowDrop(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetAllowDrop)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetClip<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<RectangleGeometry>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetClip)(windows_core::Interface::as_raw(self), value.param().abi())
                .ok()
        }
    }
    pub fn Visibility(&self) -> windows_core::Result<Visibility> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Visibility)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetVisibility(&self, value: Visibility) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVisibility)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn ContextFlyout(&self) -> windows_core::Result<FlyoutBase> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContextFlyout)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetContextFlyout<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<FlyoutBase>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetContextFlyout)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn KeyboardAccelerators(&self) -> windows_core::Result<windows_collections::IVector<KeyboardAccelerator>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).KeyboardAccelerators)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetTabFocusNavigation(&self, value: KeyboardNavigationMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTabFocusNavigation)(windows_core::Interface::as_raw(self), value)
                .ok()
        }
    }
    pub fn XamlRoot(&self) -> windows_core::Result<XamlRoot> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).XamlRoot)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetXamlRoot<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<XamlRoot>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetXamlRoot)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn FocusState(&self) -> windows_core::Result<FocusState> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FocusState)(windows_core::Interface::as_raw(self), &mut result__)
                .map(|| result__)
        }
    }
    pub fn SetIsTabStop(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsTabStop)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn SetTabIndex(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTabIndex)(windows_core::Interface::as_raw(self), value).ok()
        }
    }
    pub fn KeyUp<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<KeyRoutedEventArgs>) + 'static,
    {
        let handler: KeyEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<KeyEventHandler, F>::new(&KeyEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).KeyUp)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveKeyUp,
            ))
        }
    }
    pub fn KeyDown<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<KeyRoutedEventArgs>) + 'static,
    {
        let handler: KeyEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<KeyEventHandler, F>::new(&KeyEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).KeyDown)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveKeyDown,
            ))
        }
    }
    pub fn GotFocus<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).GotFocus)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveGotFocus,
            ))
        }
    }
    pub fn LostFocus<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
    {
        let handler: RoutedEventHandler = {
            let com = windows_core::imp::DelegateBox::<RoutedEventHandler, F>::new(
                &RoutedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).LostFocus)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveLostFocus,
            ))
        }
    }
    pub fn DragEnter<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragEventArgs>) + 'static,
    {
        let handler: DragEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<DragEventHandler, F>::new(&DragEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).DragEnter)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDragEnter,
            ))
        }
    }
    pub fn DragLeave<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragEventArgs>) + 'static,
    {
        let handler: DragEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<DragEventHandler, F>::new(&DragEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).DragLeave)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDragLeave,
            ))
        }
    }
    pub fn DragOver<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragEventArgs>) + 'static,
    {
        let handler: DragEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<DragEventHandler, F>::new(&DragEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).DragOver)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDragOver,
            ))
        }
    }
    pub fn Drop<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DragEventArgs>) + 'static,
    {
        let handler: DragEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<DragEventHandler, F>::new(&DragEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Drop)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(self.clone(), token__, windows_core::Interface::vtable(self).RemoveDrop))
        }
    }
    pub fn PointerPressed<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static,
    {
        let handler: PointerEventHandler = {
            let com = windows_core::imp::DelegateBox::<PointerEventHandler, F>::new(
                &PointerEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PointerPressed)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePointerPressed,
            ))
        }
    }
    pub fn PointerMoved<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static,
    {
        let handler: PointerEventHandler = {
            let com = windows_core::imp::DelegateBox::<PointerEventHandler, F>::new(
                &PointerEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PointerMoved)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePointerMoved,
            ))
        }
    }
    pub fn PointerReleased<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static,
    {
        let handler: PointerEventHandler = {
            let com = windows_core::imp::DelegateBox::<PointerEventHandler, F>::new(
                &PointerEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PointerReleased)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePointerReleased,
            ))
        }
    }
    pub fn PointerExited<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static,
    {
        let handler: PointerEventHandler = {
            let com = windows_core::imp::DelegateBox::<PointerEventHandler, F>::new(
                &PointerEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PointerExited)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePointerExited,
            ))
        }
    }
    pub fn PointerWheelChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static,
    {
        let handler: PointerEventHandler = {
            let com = windows_core::imp::DelegateBox::<PointerEventHandler, F>::new(
                &PointerEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PointerWheelChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePointerWheelChanged,
            ))
        }
    }
    pub fn DoubleTapped<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<DoubleTappedRoutedEventArgs>) + 'static,
    {
        let handler: DoubleTappedEventHandler = {
            let com = windows_core::imp::DelegateBox::<DoubleTappedEventHandler, F>::new(
                &DoubleTappedEventHandlerBox::<F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).DoubleTapped)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDoubleTapped,
            ))
        }
    }
    pub fn PreviewKeyDown<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<KeyRoutedEventArgs>) + 'static,
    {
        let handler: KeyEventHandler = {
            let com =
                windows_core::imp::DelegateBox::<KeyEventHandler, F>::new(&KeyEventHandlerBox::<F>::VTABLE, handler);
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).PreviewKeyDown)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemovePreviewKeyDown,
            ))
        }
    }
    pub fn Measure(&self, availablesize: Size) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Measure)(windows_core::Interface::as_raw(self), availablesize).ok()
        }
    }
    pub fn CapturePointer<P0>(&self, value: P0) -> windows_core::Result<bool>
    where
        P0: windows_core::Param<Pointer>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CapturePointer)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn TransformToVisual<P0>(&self, visual: P0) -> windows_core::Result<GeneralTransform>
    where
        P0: windows_core::Param<UIElement>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TransformToVisual)(
                windows_core::Interface::as_raw(self),
                visual.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn UpdateLayout(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).UpdateLayout)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn Focus(&self, value: FocusState) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Focus)(windows_core::Interface::as_raw(self), value, &mut result__)
                .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IUIElement_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub DesiredSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Size) -> windows_core::HRESULT,
    pub AllowDrop: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub SetAllowDrop: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    Opacity: usize,
    SetOpacity: usize,
    Clip: usize,
    pub SetClip: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    RenderTransform: usize,
    SetRenderTransform: usize,
    Projection: usize,
    SetProjection: usize,
    Transform3D: usize,
    SetTransform3D: usize,
    RenderTransformOrigin: usize,
    SetRenderTransformOrigin: usize,
    IsHitTestVisible: usize,
    SetIsHitTestVisible: usize,
    pub Visibility: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Visibility) -> windows_core::HRESULT,
    pub SetVisibility: unsafe extern "system" fn(*mut core::ffi::c_void, Visibility) -> windows_core::HRESULT,
    RenderSize: usize,
    UseLayoutRounding: usize,
    SetUseLayoutRounding: usize,
    Transitions: usize,
    SetTransitions: usize,
    CacheMode: usize,
    SetCacheMode: usize,
    IsTapEnabled: usize,
    SetIsTapEnabled: usize,
    IsDoubleTapEnabled: usize,
    SetIsDoubleTapEnabled: usize,
    CanDrag: usize,
    SetCanDrag: usize,
    IsRightTapEnabled: usize,
    SetIsRightTapEnabled: usize,
    IsHoldingEnabled: usize,
    SetIsHoldingEnabled: usize,
    ManipulationMode: usize,
    SetManipulationMode: usize,
    PointerCaptures: usize,
    pub ContextFlyout:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetContextFlyout:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    CompositeMode: usize,
    SetCompositeMode: usize,
    Lights: usize,
    CanBeScrollAnchor: usize,
    SetCanBeScrollAnchor: usize,
    ExitDisplayModeOnAccessKeyInvoked: usize,
    SetExitDisplayModeOnAccessKeyInvoked: usize,
    IsAccessKeyScope: usize,
    SetIsAccessKeyScope: usize,
    AccessKeyScopeOwner: usize,
    SetAccessKeyScopeOwner: usize,
    AccessKey: usize,
    SetAccessKey: usize,
    KeyTipPlacementMode: usize,
    SetKeyTipPlacementMode: usize,
    KeyTipHorizontalOffset: usize,
    SetKeyTipHorizontalOffset: usize,
    KeyTipVerticalOffset: usize,
    SetKeyTipVerticalOffset: usize,
    KeyTipTarget: usize,
    SetKeyTipTarget: usize,
    XYFocusKeyboardNavigation: usize,
    SetXYFocusKeyboardNavigation: usize,
    XYFocusUpNavigationStrategy: usize,
    SetXYFocusUpNavigationStrategy: usize,
    XYFocusDownNavigationStrategy: usize,
    SetXYFocusDownNavigationStrategy: usize,
    XYFocusLeftNavigationStrategy: usize,
    SetXYFocusLeftNavigationStrategy: usize,
    XYFocusRightNavigationStrategy: usize,
    SetXYFocusRightNavigationStrategy: usize,
    pub KeyboardAccelerators:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    KeyboardAcceleratorPlacementTarget: usize,
    SetKeyboardAcceleratorPlacementTarget: usize,
    KeyboardAcceleratorPlacementMode: usize,
    SetKeyboardAcceleratorPlacementMode: usize,
    HighContrastAdjustment: usize,
    SetHighContrastAdjustment: usize,
    TabFocusNavigation: usize,
    pub SetTabFocusNavigation:
        unsafe extern "system" fn(*mut core::ffi::c_void, KeyboardNavigationMode) -> windows_core::HRESULT,
    OpacityTransition: usize,
    SetOpacityTransition: usize,
    Translation: usize,
    SetTranslation: usize,
    TranslationTransition: usize,
    SetTranslationTransition: usize,
    Rotation: usize,
    SetRotation: usize,
    RotationTransition: usize,
    SetRotationTransition: usize,
    Scale: usize,
    SetScale: usize,
    ScaleTransition: usize,
    SetScaleTransition: usize,
    TransformMatrix: usize,
    SetTransformMatrix: usize,
    CenterPoint: usize,
    SetCenterPoint: usize,
    RotationAxis: usize,
    SetRotationAxis: usize,
    ActualOffset: usize,
    ActualSize: usize,
    pub XamlRoot:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetXamlRoot: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    Shadow: usize,
    SetShadow: usize,
    RasterizationScale: usize,
    SetRasterizationScale: usize,
    pub FocusState: unsafe extern "system" fn(*mut core::ffi::c_void, *mut FocusState) -> windows_core::HRESULT,
    UseSystemFocusVisuals: usize,
    SetUseSystemFocusVisuals: usize,
    XYFocusLeft: usize,
    SetXYFocusLeft: usize,
    XYFocusRight: usize,
    SetXYFocusRight: usize,
    XYFocusUp: usize,
    SetXYFocusUp: usize,
    XYFocusDown: usize,
    SetXYFocusDown: usize,
    IsTabStop: usize,
    pub SetIsTabStop: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    TabIndex: usize,
    pub SetTabIndex: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    pub KeyUp:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveKeyUp: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub KeyDown:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveKeyDown: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub GotFocus:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveGotFocus: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub LostFocus:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveLostFocus: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    DragStarting: usize,
    RemoveDragStarting: usize,
    DropCompleted: usize,
    RemoveDropCompleted: usize,
    CharacterReceived: usize,
    RemoveCharacterReceived: usize,
    pub DragEnter:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDragEnter: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub DragLeave:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDragLeave: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub DragOver:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDragOver: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Drop:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDrop: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub PointerPressed:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePointerPressed: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub PointerMoved:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePointerMoved: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub PointerReleased:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePointerReleased: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    PointerEntered: usize,
    RemovePointerEntered: usize,
    pub PointerExited:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePointerExited: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    PointerCaptureLost: usize,
    RemovePointerCaptureLost: usize,
    PointerCanceled: usize,
    RemovePointerCanceled: usize,
    pub PointerWheelChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePointerWheelChanged: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    Tapped: usize,
    RemoveTapped: usize,
    pub DoubleTapped:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveDoubleTapped: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    Holding: usize,
    RemoveHolding: usize,
    ContextRequested: usize,
    RemoveContextRequested: usize,
    ContextCanceled: usize,
    RemoveContextCanceled: usize,
    RightTapped: usize,
    RemoveRightTapped: usize,
    ManipulationStarting: usize,
    RemoveManipulationStarting: usize,
    ManipulationInertiaStarting: usize,
    RemoveManipulationInertiaStarting: usize,
    ManipulationStarted: usize,
    RemoveManipulationStarted: usize,
    ManipulationDelta: usize,
    RemoveManipulationDelta: usize,
    ManipulationCompleted: usize,
    RemoveManipulationCompleted: usize,
    AccessKeyDisplayRequested: usize,
    RemoveAccessKeyDisplayRequested: usize,
    AccessKeyDisplayDismissed: usize,
    RemoveAccessKeyDisplayDismissed: usize,
    AccessKeyInvoked: usize,
    RemoveAccessKeyInvoked: usize,
    ProcessKeyboardAccelerators: usize,
    RemoveProcessKeyboardAccelerators: usize,
    GettingFocus: usize,
    RemoveGettingFocus: usize,
    LosingFocus: usize,
    RemoveLosingFocus: usize,
    NoFocusCandidateFound: usize,
    RemoveNoFocusCandidateFound: usize,
    pub PreviewKeyDown:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemovePreviewKeyDown: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    PreviewKeyUp: usize,
    RemovePreviewKeyUp: usize,
    BringIntoViewRequested: usize,
    RemoveBringIntoViewRequested: usize,
    pub Measure: unsafe extern "system" fn(*mut core::ffi::c_void, Size) -> windows_core::HRESULT,
    Arrange: usize,
    pub CapturePointer:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    ReleasePointerCapture: usize,
    ReleasePointerCaptures: usize,
    AddHandler: usize,
    RemoveHandler: usize,
    pub TransformToVisual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    InvalidateMeasure: usize,
    InvalidateArrange: usize,
    pub UpdateLayout: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    CancelDirectManipulations: usize,
    StartDragAsync: usize,
    StartBringIntoView: usize,
    StartBringIntoViewWithOptions: usize,
    TryInvokeKeyboardAccelerator: usize,
    pub Focus: unsafe extern "system" fn(*mut core::ffi::c_void, FocusState, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IUISettings, IUISettings_Vtbl, 0x85361600_1c63_4627_bcb1_3a89e0bc9c55);
impl windows_core::RuntimeType for IUISettings {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IUISettings {
    pub fn AnimationsEnabled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AnimationsEnabled)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IUISettings_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    HandPreference: usize,
    CursorSize: usize,
    ScrollBarSize: usize,
    ScrollBarArrowSize: usize,
    ScrollBarThumbBoxSize: usize,
    MessageDuration: usize,
    pub AnimationsEnabled: unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IUISettings2, IUISettings2_Vtbl, 0xbad82401_2721_44f9_bb91_2bb228be442f);
impl windows_core::RuntimeType for IUISettings2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IUISettings2 {
    pub fn TextScaleFactor(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TextScaleFactor)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IUISettings2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub TextScaleFactor: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IUriRuntimeClass, IUriRuntimeClass_Vtbl, 0x9e365e57_48b2_4160_956f_c7385120bbfc);
impl windows_core::RuntimeType for IUriRuntimeClass {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IUriRuntimeClass_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IUriRuntimeClassFactory,
    IUriRuntimeClassFactory_Vtbl,
    0x44a9796f_723e_4fdf_a218_033e75b0c084
);
impl windows_core::RuntimeType for IUriRuntimeClassFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IUriRuntimeClassFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateUri: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IValueProvider, IValueProvider_Vtbl, 0x984f11cf_4611_588e_b52e_b96a12322c71);
impl windows_core::RuntimeType for IValueProvider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IValueProvider, windows_core::IUnknown, windows_core::IInspectable);
impl IValueProvider {
    pub fn SetValue(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetValue)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IValueProvider_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsReadOnly: usize,
    Value: usize,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IVisualTreeHelper, IVisualTreeHelper_Vtbl, 0x5f69ac1e_6504_5e3f_a11c_87684c1db814);
impl windows_core::RuntimeType for IVisualTreeHelper {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IVisualTreeHelper_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IVisualTreeHelperStatics,
    IVisualTreeHelperStatics_Vtbl,
    0x5aece43c_7651_5bb5_855c_2198496e455e
);
impl windows_core::RuntimeType for IVisualTreeHelperStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IVisualTreeHelperStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FindElementsInHostCoordinatesPoint: usize,
    FindElementsInHostCoordinatesRect: usize,
    FindAllElementsInHostCoordinatesPoint: usize,
    FindAllElementsInHostCoordinatesRect: usize,
    pub GetChild: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        i32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetChildrenCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub GetParent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    DisconnectChildrenRecursive: usize,
    GetOpenPopups: usize,
    pub GetOpenPopupsForXamlRoot: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IWindow, IWindow_Vtbl, 0x61f0ec79_5d52_56b5_86fb_40fa4af288b0);
impl windows_core::RuntimeType for IWindow {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IWindow {
    pub fn Content(&self) -> windows_core::Result<UIElement> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Content)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetContent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<UIElement>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetContent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn Title(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Title)(windows_core::Interface::as_raw(self), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        }
    }
    pub fn SetTitle(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTitle)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
    pub fn SetExtendsContentIntoTitleBar(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetExtendsContentIntoTitleBar)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn Activated<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<WindowActivatedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<windows_core::IInspectable, WindowActivatedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<windows_core::IInspectable, WindowActivatedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<windows_core::IInspectable, WindowActivatedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Activated)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveActivated,
            ))
        }
    }
    pub fn Activate(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Activate)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn Close(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Close)(windows_core::Interface::as_raw(self)).ok() }
    }
    pub fn SetTitleBar<P0>(&self, titlebar: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<UIElement>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetTitleBar)(
                windows_core::Interface::as_raw(self),
                titlebar.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IWindow_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Bounds: usize,
    Visible: usize,
    pub Content:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetContent: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    CoreWindow: usize,
    Compositor: usize,
    Dispatcher: usize,
    DispatcherQueue: usize,
    pub Title: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetTitle: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
    ExtendsContentIntoTitleBar: usize,
    pub SetExtendsContentIntoTitleBar: unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub Activated:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub RemoveActivated: unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    Closed: usize,
    RemoveClosed: usize,
    SizeChanged: usize,
    RemoveSizeChanged: usize,
    VisibilityChanged: usize,
    RemoveVisibilityChanged: usize,
    pub Activate: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Close: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetTitleBar: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IWindow2, IWindow2_Vtbl, 0x42febaa5_1c32_522a_a591_57618c6f665d);
impl windows_core::RuntimeType for IWindow2 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IWindow2 {
    pub fn AppWindow(&self) -> windows_core::Result<AppWindow> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AppWindow)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IWindow2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    SystemBackdrop: usize,
    SetSystemBackdrop: usize,
    pub AppWindow:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IWindowActivatedEventArgs,
    IWindowActivatedEventArgs_Vtbl,
    0xc723a5ea_82c4_5dd6_861b_70ef573b88d6
);
impl windows_core::RuntimeType for IWindowActivatedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IWindowActivatedEventArgs {
    pub fn WindowActivationState(&self) -> windows_core::Result<WindowActivationState> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).WindowActivationState)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IWindowActivatedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Handled: usize,
    SetHandled: usize,
    pub WindowActivationState:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut WindowActivationState) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IWindowFactory, IWindowFactory_Vtbl, 0xf0441536_afef_5222_918f_324a9b2dec75);
impl windows_core::RuntimeType for IWindowFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IWindowFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstance: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IWindowsXamlManager,
    IWindowsXamlManager_Vtbl,
    0x85a2e562_7e8f_5333_a104_a3e672a2ffee
);
impl windows_core::RuntimeType for IWindowsXamlManager {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IWindowsXamlManager_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IWindowsXamlManagerStatics,
    IWindowsXamlManagerStatics_Vtbl,
    0x56cb591d_de97_539f_881d_8ccdc44fa6c4
);
impl windows_core::RuntimeType for IWindowsXamlManagerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IWindowsXamlManagerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub InitializeForCurrentThread:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IWriteableBitmap, IWriteableBitmap_Vtbl, 0x78c824a9_0e43_5f1e_93bc_d046cca82b7e);
impl windows_core::RuntimeType for IWriteableBitmap {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IWriteableBitmap {
    pub fn PixelBuffer(&self) -> windows_core::Result<IBuffer> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PixelBuffer)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn Invalidate(&self) -> windows_core::Result<()> {
        unsafe { (windows_core::Interface::vtable(self).Invalidate)(windows_core::Interface::as_raw(self)).ok() }
    }
}
#[repr(C)]
pub struct IWriteableBitmap_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PixelBuffer:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Invalidate: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IWriteableBitmapFactory,
    IWriteableBitmapFactory_Vtbl,
    0x26e861d9_b080_512b_96c4_80050e7e08d1
);
impl windows_core::RuntimeType for IWriteableBitmapFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IWriteableBitmapFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateInstanceWithDimensions: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IXamlControlsResources,
    IXamlControlsResources_Vtbl,
    0x918ca043_f42c_5805_861b_62d6d1d0c162
);
impl windows_core::RuntimeType for IXamlControlsResources {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IXamlControlsResources_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IXamlMetadataProvider,
    IXamlMetadataProvider_Vtbl,
    0xa96251f0_2214_5d53_8746_ce99a2593cd7
);
impl windows_core::RuntimeType for IXamlMetadataProvider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"Microsoft.UI.Xaml.Markup.IXamlMetadataProvider");
}
windows_core::imp::interface_hierarchy!(IXamlMetadataProvider, windows_core::IUnknown, windows_core::IInspectable);
impl IXamlMetadataProvider {
    pub fn GetXamlType(&self, r#type: &TypeName) -> windows_core::Result<IXamlType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetXamlType)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(r#type),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn GetXamlTypeByFullName(&self, fullname: &str) -> windows_core::Result<IXamlType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetXamlTypeByFullName)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(fullname)),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn GetXmlnsDefinitions(&self) -> windows_core::Result<windows_core::Array<XmlnsDefinition>> {
        unsafe {
            let mut result__ = core::mem::MaybeUninit::zeroed();
            (windows_core::Interface::vtable(self).GetXmlnsDefinitions)(
                windows_core::Interface::as_raw(self),
                windows_core::Array::<XmlnsDefinition>::set_abi_len(core::mem::transmute(&mut result__)),
                result__.as_mut_ptr() as *mut _ as _,
            )
            .map(|| result__.assume_init())
        }
    }
}
impl windows_core::RuntimeName for IXamlMetadataProvider {
    const NAME: &'static str = "Microsoft.UI.Xaml.Markup.IXamlMetadataProvider";
}
pub trait IXamlMetadataProvider_Impl: windows_core::IUnknownImpl {
    fn GetXamlType(&self, r#type: &TypeName) -> windows_core::Result<IXamlType>;
    fn GetXamlTypeByFullName(&self, fullName: &windows_core::HSTRING) -> windows_core::Result<IXamlType>;
    fn GetXmlnsDefinitions(&self) -> windows_core::Result<windows_core::Array<XmlnsDefinition>>;
}
impl IXamlMetadataProvider_Vtbl {
    pub const fn new<Identity: IXamlMetadataProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetXamlType<Identity: IXamlMetadataProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            r#type: core::mem::MaybeUninit<TypeName>,
            result__: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IXamlMetadataProvider_Impl::GetXamlType(this, core::mem::transmute(&r#type)) {
                    Ok(ok__) => {
                        result__.write(core::mem::transmute_copy(&ok__));
                        core::mem::forget(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetXamlTypeByFullName<Identity: IXamlMetadataProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            fullname: *mut core::ffi::c_void,
            result__: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IXamlMetadataProvider_Impl::GetXamlTypeByFullName(this, core::mem::transmute(&fullname)) {
                    Ok(ok__) => {
                        result__.write(core::mem::transmute_copy(&ok__));
                        core::mem::forget(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetXmlnsDefinitions<Identity: IXamlMetadataProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            result_size__: *mut u32,
            result__: *mut *mut core::mem::MaybeUninit<XmlnsDefinition>,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity = &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IXamlMetadataProvider_Impl::GetXmlnsDefinitions(this) {
                    Ok(ok__) => {
                        let (ok_data__, ok_data_len__) = ok__.into_abi();
                        result__.write(ok_data__);
                        result_size__.write(ok_data_len__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<Identity, IXamlMetadataProvider, OFFSET>(),
            GetXamlType: GetXamlType::<Identity, OFFSET>,
            GetXamlTypeByFullName: GetXamlTypeByFullName::<Identity, OFFSET>,
            GetXmlnsDefinitions: GetXmlnsDefinitions::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IXamlMetadataProvider as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IXamlMetadataProvider_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetXamlType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        core::mem::MaybeUninit<TypeName>,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetXamlTypeByFullName: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetXmlnsDefinitions: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut u32,
        *mut *mut core::mem::MaybeUninit<XmlnsDefinition>,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IXamlReader, IXamlReader_Vtbl, 0x54ce54c8_38c6_50d9_ac98_4b03eddbde9f);
impl windows_core::RuntimeType for IXamlReader {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IXamlReader_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IXamlReaderStatics,
    IXamlReaderStatics_Vtbl,
    0x82a4cd9e_435e_5aeb_8c4f_300cece45cae
);
impl windows_core::RuntimeType for IXamlReaderStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IXamlReaderStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Load: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IXamlRoot, IXamlRoot_Vtbl, 0x60cb215a_ad15_520a_8b01_4416824f0441);
impl windows_core::RuntimeType for IXamlRoot {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IXamlRoot {
    pub fn RasterizationScale(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RasterizationScale)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IXamlRoot_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Content: usize,
    Size: usize,
    pub RasterizationScale: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IXamlRoot4, IXamlRoot4_Vtbl, 0x377bec22_632b_52be_b26f_5edf7838e5ca);
impl windows_core::RuntimeType for IXamlRoot4 {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IXamlRoot4 {
    pub fn ContentIsland(&self) -> windows_core::Result<ContentIsland> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContentIsland)(windows_core::Interface::as_raw(self), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IXamlRoot4_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ContentIsland:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(IXamlType, IXamlType_Vtbl, 0xd24219df_7ec9_57f1_a27b_6af251d9c5bc);
impl windows_core::RuntimeType for IXamlType {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(IXamlType, windows_core::IUnknown, windows_core::IInspectable);
#[repr(C)]
pub struct IXamlType_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IconElement(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(IconElement, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(IconElement, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for IconElement {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IIconElement>();
}
unsafe impl windows_core::Interface for IconElement {
    type Vtable = <IIconElement as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IIconElement as windows_core::Interface>::IID;
}
impl core::ops::Deref for IconElement {
    type Target = IIconElement;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for IconElement {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.IconElement";
}
unsafe impl Send for IconElement {}
unsafe impl Sync for IconElement {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IconId {
    pub value: u64,
}
impl windows_core::imp::TypeKind for IconId {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for IconId {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Microsoft.UI.IconId;u8)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Image, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Image, FrameworkElement, UIElement, DependencyObject);
impl Image {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Image, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Image {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IImage>();
}
unsafe impl windows_core::Interface for Image {
    type Vtable = <IImage as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IImage as windows_core::Interface>::IID;
}
impl core::ops::Deref for Image {
    type Target = IImage;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Image {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Image";
}
unsafe impl Send for Image {}
unsafe impl Sync for Image {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSource(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ImageSource, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ImageSource, DependencyObject);
impl windows_core::RuntimeType for ImageSource {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IImageSource>();
}
unsafe impl windows_core::Interface for ImageSource {
    type Vtable = <IImageSource as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IImageSource as windows_core::Interface>::IID;
}
impl core::ops::Deref for ImageSource {
    type Target = IImageSource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ImageSource {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.ImageSource";
}
unsafe impl Send for ImageSource {}
unsafe impl Sync for ImageSource {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputFocusController(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(InputFocusController, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(InputFocusController, InputObject);
impl InputFocusController {
    pub fn GetForIsland<P0>(island: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<ContentIsland>,
    {
        Self::IInputFocusControllerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetForIsland)(
                windows_core::Interface::as_raw(this),
                island.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IInputFocusControllerStatics<R, F: FnOnce(&IInputFocusControllerStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<InputFocusController, IInputFocusControllerStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InputFocusController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInputFocusController>();
}
unsafe impl windows_core::Interface for InputFocusController {
    type Vtable = <IInputFocusController as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInputFocusController as windows_core::Interface>::IID;
}
impl core::ops::Deref for InputFocusController {
    type Target = IInputFocusController;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InputFocusController {
    const NAME: &'static str = "Microsoft.UI.Input.InputFocusController";
}
unsafe impl Send for InputFocusController {}
unsafe impl Sync for InputFocusController {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputObject(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(InputObject, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for InputObject {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IInputObject>();
}
unsafe impl windows_core::Interface for InputObject {
    type Vtable = <IInputObject as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInputObject as windows_core::Interface>::IID;
}
impl core::ops::Deref for InputObject {
    type Target = IInputObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InputObject {
    const NAME: &'static str = "Microsoft.UI.Input.InputObject";
}
unsafe impl Send for InputObject {}
unsafe impl Sync for InputObject {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputScope(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(InputScope, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(InputScope, DependencyObject);
impl InputScope {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<InputScope, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InputScope {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IInputScope>();
}
unsafe impl windows_core::Interface for InputScope {
    type Vtable = <IInputScope as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInputScope as windows_core::Interface>::IID;
}
impl core::ops::Deref for InputScope {
    type Target = IInputScope;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InputScope {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.InputScope";
}
unsafe impl Send for InputScope {}
unsafe impl Sync for InputScope {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputScopeName(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(InputScopeName, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(InputScopeName, DependencyObject);
impl InputScopeName {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<InputScopeName, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InputScopeName {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInputScopeName>();
}
unsafe impl windows_core::Interface for InputScopeName {
    type Vtable = <IInputScopeName as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInputScopeName as windows_core::Interface>::IID;
}
impl core::ops::Deref for InputScopeName {
    type Target = IInputScopeName;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InputScopeName {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.InputScopeName";
}
unsafe impl Send for InputScopeName {}
unsafe impl Sync for InputScopeName {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputScopeNameValue(pub i32);
impl InputScopeNameValue {
    pub const Default: Self = Self(0);
    pub const Url: Self = Self(1);
    pub const EmailSmtpAddress: Self = Self(5);
    pub const PersonalFullName: Self = Self(7);
    pub const CurrencyAmountAndSymbol: Self = Self(20);
    pub const CurrencyAmount: Self = Self(21);
    pub const DateMonthNumber: Self = Self(23);
    pub const DateDayNumber: Self = Self(24);
    pub const DateYear: Self = Self(25);
    pub const Digits: Self = Self(28);
    pub const Number: Self = Self(29);
    pub const Password: Self = Self(31);
    pub const TelephoneNumber: Self = Self(32);
    pub const TelephoneCountryCode: Self = Self(33);
    pub const TelephoneAreaCode: Self = Self(34);
    pub const TelephoneLocalNumber: Self = Self(35);
    pub const TimeHour: Self = Self(37);
    pub const TimeMinutesOrSeconds: Self = Self(38);
    pub const NumberFullWidth: Self = Self(39);
    pub const AlphanumericHalfWidth: Self = Self(40);
    pub const AlphanumericFullWidth: Self = Self(41);
    pub const Hiragana: Self = Self(44);
    pub const KatakanaHalfWidth: Self = Self(45);
    pub const KatakanaFullWidth: Self = Self(46);
    pub const Hanja: Self = Self(47);
    pub const HangulHalfWidth: Self = Self(48);
    pub const HangulFullWidth: Self = Self(49);
    pub const Search: Self = Self(50);
    pub const Formula: Self = Self(51);
    pub const SearchIncremental: Self = Self(52);
    pub const ChineseHalfWidth: Self = Self(53);
    pub const ChineseFullWidth: Self = Self(54);
    pub const NativeScript: Self = Self(55);
    pub const Text: Self = Self(57);
    pub const Chat: Self = Self(58);
    pub const NameOrPhoneNumber: Self = Self(59);
    pub const EmailNameOrAddress: Self = Self(60);
    pub const Maps: Self = Self(62);
    pub const NumericPassword: Self = Self(63);
    pub const NumericPin: Self = Self(64);
    pub const AlphanumericPin: Self = Self(65);
    pub const FormulaNumber: Self = Self(67);
    pub const ChatWithoutEmoji: Self = Self(68);
}
impl windows_core::imp::TypeKind for InputScopeNameValue {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for InputScopeNameValue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Input.InputScopeNameValue;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ItemCollection,
    windows_core::IUnknown,
    windows_core::IInspectable,
    windows_collections::IObservableVector<windows_core::IInspectable>
);
impl windows_core::RuntimeType for ItemCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        windows_collections::IObservableVector<windows_core::IInspectable>,
    >();
}
unsafe impl windows_core::Interface for ItemCollection {
    type Vtable =
        <windows_collections::IObservableVector<windows_core::IInspectable> as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <windows_collections::IObservableVector<windows_core::IInspectable> as windows_core::Interface>::IID;
}
impl core::ops::Deref for ItemCollection {
    type Target = windows_collections::IObservableVector<windows_core::IInspectable>;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ItemCollection {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ItemCollection";
}
unsafe impl Send for ItemCollection {}
unsafe impl Sync for ItemCollection {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemContainer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ItemContainer, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ItemContainer, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for ItemContainer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IItemContainer>();
}
unsafe impl windows_core::Interface for ItemContainer {
    type Vtable = <IItemContainer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IItemContainer as windows_core::Interface>::IID;
}
impl core::ops::Deref for ItemContainer {
    type Target = IItemContainer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ItemContainer {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ItemContainer";
}
unsafe impl Send for ItemContainer {}
unsafe impl Sync for ItemContainer {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemsControl(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ItemsControl, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ItemsControl, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for ItemsControl {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IItemsControl>();
}
unsafe impl windows_core::Interface for ItemsControl {
    type Vtable = <IItemsControl as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IItemsControl as windows_core::Interface>::IID;
}
impl core::ops::Deref for ItemsControl {
    type Target = IItemsControl;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ItemsControl {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ItemsControl";
}
unsafe impl Send for ItemsControl {}
unsafe impl Sync for ItemsControl {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: u32,
    pub scanCode: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}
windows_core::imp::define_interface!(KeyEventHandler, KeyEventHandler_Vtbl, 0xdb68e7cc_9a2b_527d_9989_25284daccc03);
impl windows_core::RuntimeType for KeyEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct KeyEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct KeyEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<KeyRoutedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<KeyRoutedEventArgs>) + 'static>
    KeyEventHandlerBox<F>
{
    const VTABLE: KeyEventHandler_Vtbl = KeyEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<KeyEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<KeyEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<KeyEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this =
                &mut *(this as *mut *mut core::ffi::c_void as *mut windows_core::imp::DelegateBox<KeyEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyRoutedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(KeyRoutedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(KeyRoutedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for KeyRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IKeyRoutedEventArgs>();
}
unsafe impl windows_core::Interface for KeyRoutedEventArgs {
    type Vtable = <IKeyRoutedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IKeyRoutedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for KeyRoutedEventArgs {
    type Target = IKeyRoutedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for KeyRoutedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.KeyRoutedEventArgs";
}
unsafe impl Send for KeyRoutedEventArgs {}
unsafe impl Sync for KeyRoutedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardAccelerator(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(KeyboardAccelerator, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(KeyboardAccelerator, DependencyObject);
impl KeyboardAccelerator {
    pub fn new() -> windows_core::Result<Self> {
        Self::IKeyboardAcceleratorFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IKeyboardAcceleratorFactory<R, F: FnOnce(&IKeyboardAcceleratorFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<KeyboardAccelerator, IKeyboardAcceleratorFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for KeyboardAccelerator {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IKeyboardAccelerator>();
}
unsafe impl windows_core::Interface for KeyboardAccelerator {
    type Vtable = <IKeyboardAccelerator as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IKeyboardAccelerator as windows_core::Interface>::IID;
}
impl core::ops::Deref for KeyboardAccelerator {
    type Target = IKeyboardAccelerator;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for KeyboardAccelerator {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.KeyboardAccelerator";
}
unsafe impl Send for KeyboardAccelerator {}
unsafe impl Sync for KeyboardAccelerator {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardAcceleratorInvokedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    KeyboardAcceleratorInvokedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for KeyboardAcceleratorInvokedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IKeyboardAcceleratorInvokedEventArgs>();
}
unsafe impl windows_core::Interface for KeyboardAcceleratorInvokedEventArgs {
    type Vtable = <IKeyboardAcceleratorInvokedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IKeyboardAcceleratorInvokedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for KeyboardAcceleratorInvokedEventArgs {
    type Target = IKeyboardAcceleratorInvokedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for KeyboardAcceleratorInvokedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.KeyboardAcceleratorInvokedEventArgs";
}
unsafe impl Send for KeyboardAcceleratorInvokedEventArgs {}
unsafe impl Sync for KeyboardAcceleratorInvokedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyboardNavigationMode(pub i32);
impl KeyboardNavigationMode {
    pub const Local: Self = Self(0);
    pub const Cycle: Self = Self(1);
    pub const Once: Self = Self(2);
}
impl windows_core::imp::TypeKind for KeyboardNavigationMode {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for KeyboardNavigationMode {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Input.KeyboardNavigationMode;i4)");
}
pub type LCTYPE = u32;
pub const LLKHF_EXTENDED: i32 = 1;
pub const LLKHF_UP: i32 = 128;
pub const LOCALE_ICURRENCY: i32 = 27;
pub const LOCALE_IPOSITIVEPERCENT: i32 = 117;
pub const LOCALE_RETURN_NUMBER: i32 = 536870912;
pub const LOCALE_SDECIMAL: i32 = 14;
pub const LOCALE_SGROUPING: i32 = 16;
pub const LOCALE_SMONDECIMALSEP: i32 = 22;
pub const LOCALE_SMONGROUPING: i32 = 24;
pub const LOCALE_SMONTHOUSANDSEP: i32 = 23;
pub const LOCALE_STHOUSAND: i32 = 15;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub const LR_DEFAULTCOLOR: i32 = 0;
pub const LWA_ALPHA: i32 = 2;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchActivatedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(LaunchActivatedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for LaunchActivatedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ILaunchActivatedEventArgs>();
}
unsafe impl windows_core::Interface for LaunchActivatedEventArgs {
    type Vtable = <ILaunchActivatedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ILaunchActivatedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for LaunchActivatedEventArgs {
    type Target = ILaunchActivatedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for LaunchActivatedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.LaunchActivatedEventArgs";
}
unsafe impl Send for LaunchActivatedEventArgs {}
unsafe impl Sync for LaunchActivatedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListView(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ListView, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ListView,
    ListViewBase,
    Selector,
    ItemsControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ListView {
    pub fn new() -> windows_core::Result<Self> {
        Self::IListViewFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IListViewFactory<R, F: FnOnce(&IListViewFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ListView, IListViewFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ListView {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IListView>();
}
unsafe impl windows_core::Interface for ListView {
    type Vtable = <IListView as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IListView as windows_core::Interface>::IID;
}
impl core::ops::Deref for ListView {
    type Target = IListView;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ListView {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ListView";
}
unsafe impl Send for ListView {}
unsafe impl Sync for ListView {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListViewBase(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ListViewBase, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ListViewBase,
    Selector,
    ItemsControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl windows_core::RuntimeType for ListViewBase {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IListViewBase>();
}
unsafe impl windows_core::Interface for ListViewBase {
    type Vtable = <IListViewBase as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IListViewBase as windows_core::Interface>::IID;
}
impl core::ops::Deref for ListViewBase {
    type Target = IListViewBase;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ListViewBase {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ListViewBase";
}
unsafe impl Send for ListViewBase {}
unsafe impl Sync for ListViewBase {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListViewItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ListViewItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ListViewItem,
    SelectorItem,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl windows_core::RuntimeType for ListViewItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IListViewItem>();
}
unsafe impl windows_core::Interface for ListViewItem {
    type Vtable = <IListViewItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IListViewItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for ListViewItem {
    type Target = IListViewItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ListViewItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ListViewItem";
}
unsafe impl Send for ListViewItem {}
unsafe impl Sync for ListViewItem {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ListViewSelectionMode(pub i32);
impl ListViewSelectionMode {
    pub const None: Self = Self(0);
    pub const Single: Self = Self(1);
    pub const Multiple: Self = Self(2);
    pub const Extended: Self = Self(3);
}
impl windows_core::imp::TypeKind for ListViewSelectionMode {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ListViewSelectionMode {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.ListViewSelectionMode;i4)");
}
pub const MB_ICONERROR: i32 = 16;
pub const MK_LBUTTON: i32 = 1;
pub const MK_MBUTTON: i32 = 16;
pub const MK_RBUTTON: i32 = 2;
pub const MK_XBUTTON1: i32 = 32;
pub const MK_XBUTTON2: i32 = 64;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}
pub const MONITOR_DEFAULTTONEAREST: i32 = 2;
pub const MOUSE_MOVE_ABSOLUTE: i32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}
pub const MUI_LANGUAGE_NAME: i32 = 8;
pub const MWMO_INPUTAVAILABLE: i32 = 4;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(MenuBar, Control, FrameworkElement, UIElement, DependencyObject);
impl MenuBar {
    pub fn new() -> windows_core::Result<Self> {
        Self::IMenuBarFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IMenuBarFactory<R, F: FnOnce(&IMenuBarFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<MenuBar, IMenuBarFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for MenuBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IMenuBar>();
}
unsafe impl windows_core::Interface for MenuBar {
    type Vtable = <IMenuBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuBar {
    type Target = IMenuBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuBar";
}
unsafe impl Send for MenuBar {}
unsafe impl Sync for MenuBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuBarItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuBarItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(MenuBarItem, Control, FrameworkElement, UIElement, DependencyObject);
impl MenuBarItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::IMenuBarItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IMenuBarItemFactory<R, F: FnOnce(&IMenuBarItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<MenuBarItem, IMenuBarItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for MenuBarItem {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IMenuBarItem>();
}
unsafe impl windows_core::Interface for MenuBarItem {
    type Vtable = <IMenuBarItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuBarItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuBarItem {
    type Target = IMenuBarItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuBarItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuBarItem";
}
unsafe impl Send for MenuBarItem {}
unsafe impl Sync for MenuBarItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuFlyout(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuFlyout, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(MenuFlyout, FlyoutBase, DependencyObject);
impl MenuFlyout {
    pub fn new() -> windows_core::Result<Self> {
        Self::IMenuFlyoutFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IMenuFlyoutFactory<R, F: FnOnce(&IMenuFlyoutFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<MenuFlyout, IMenuFlyoutFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for MenuFlyout {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IMenuFlyout>();
}
unsafe impl windows_core::Interface for MenuFlyout {
    type Vtable = <IMenuFlyout as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuFlyout as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuFlyout {
    type Target = IMenuFlyout;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuFlyout {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuFlyout";
}
unsafe impl Send for MenuFlyout {}
unsafe impl Sync for MenuFlyout {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuFlyoutItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuFlyoutItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    MenuFlyoutItem,
    MenuFlyoutItemBase,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl MenuFlyoutItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::IMenuFlyoutItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IMenuFlyoutItemFactory<R, F: FnOnce(&IMenuFlyoutItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<MenuFlyoutItem, IMenuFlyoutItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for MenuFlyoutItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IMenuFlyoutItem>();
}
unsafe impl windows_core::Interface for MenuFlyoutItem {
    type Vtable = <IMenuFlyoutItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuFlyoutItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuFlyoutItem {
    type Target = IMenuFlyoutItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuFlyoutItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuFlyoutItem";
}
unsafe impl Send for MenuFlyoutItem {}
unsafe impl Sync for MenuFlyoutItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuFlyoutItemBase(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuFlyoutItemBase, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(MenuFlyoutItemBase, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for MenuFlyoutItemBase {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IMenuFlyoutItemBase>();
}
unsafe impl windows_core::Interface for MenuFlyoutItemBase {
    type Vtable = <IMenuFlyoutItemBase as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuFlyoutItemBase as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuFlyoutItemBase {
    type Target = IMenuFlyoutItemBase;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuFlyoutItemBase {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuFlyoutItemBase";
}
unsafe impl Send for MenuFlyoutItemBase {}
unsafe impl Sync for MenuFlyoutItemBase {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuFlyoutSeparator(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuFlyoutSeparator, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    MenuFlyoutSeparator,
    MenuFlyoutItemBase,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl MenuFlyoutSeparator {
    pub fn new() -> windows_core::Result<Self> {
        Self::IMenuFlyoutSeparatorFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IMenuFlyoutSeparatorFactory<R, F: FnOnce(&IMenuFlyoutSeparatorFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<MenuFlyoutSeparator, IMenuFlyoutSeparatorFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for MenuFlyoutSeparator {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IMenuFlyoutSeparator>();
}
unsafe impl windows_core::Interface for MenuFlyoutSeparator {
    type Vtable = <IMenuFlyoutSeparator as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuFlyoutSeparator as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuFlyoutSeparator {
    type Target = IMenuFlyoutSeparator;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuFlyoutSeparator {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuFlyoutSeparator";
}
unsafe impl Send for MenuFlyoutSeparator {}
unsafe impl Sync for MenuFlyoutSeparator {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuFlyoutSubItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(MenuFlyoutSubItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    MenuFlyoutSubItem,
    MenuFlyoutItemBase,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl MenuFlyoutSubItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<MenuFlyoutSubItem, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for MenuFlyoutSubItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IMenuFlyoutSubItem>();
}
unsafe impl windows_core::Interface for MenuFlyoutSubItem {
    type Vtable = <IMenuFlyoutSubItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IMenuFlyoutSubItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for MenuFlyoutSubItem {
    type Target = IMenuFlyoutSubItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for MenuFlyoutSubItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.MenuFlyoutSubItem";
}
unsafe impl Send for MenuFlyoutSubItem {}
unsafe impl Sync for MenuFlyoutSubItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationView(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(NavigationView, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    NavigationView,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl NavigationView {
    pub fn new() -> windows_core::Result<Self> {
        Self::INavigationViewFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn INavigationViewFactory<R, F: FnOnce(&INavigationViewFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<NavigationView, INavigationViewFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for NavigationView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationView>();
}
unsafe impl windows_core::Interface for NavigationView {
    type Vtable = <INavigationView as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationView as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationView {
    type Target = INavigationView;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationView {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationView";
}
unsafe impl Send for NavigationView {}
unsafe impl Sync for NavigationView {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NavigationViewBackButtonVisible(pub i32);
impl NavigationViewBackButtonVisible {
    pub const Collapsed: Self = Self(0);
    pub const Visible: Self = Self(1);
    pub const Auto: Self = Self(2);
}
impl windows_core::imp::TypeKind for NavigationViewBackButtonVisible {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for NavigationViewBackButtonVisible {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Microsoft.UI.Xaml.Controls.NavigationViewBackButtonVisible;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NavigationViewDisplayMode(pub i32);
impl NavigationViewDisplayMode {
    pub const Minimal: Self = Self(0);
    pub const Compact: Self = Self(1);
    pub const Expanded: Self = Self(2);
}
impl windows_core::imp::TypeKind for NavigationViewDisplayMode {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for NavigationViewDisplayMode {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.NavigationViewDisplayMode;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationViewDisplayModeChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    NavigationViewDisplayModeChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for NavigationViewDisplayModeChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationViewDisplayModeChangedEventArgs>();
}
unsafe impl windows_core::Interface for NavigationViewDisplayModeChangedEventArgs {
    type Vtable = <INavigationViewDisplayModeChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationViewDisplayModeChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationViewDisplayModeChangedEventArgs {
    type Target = INavigationViewDisplayModeChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationViewDisplayModeChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationViewDisplayModeChangedEventArgs";
}
unsafe impl Send for NavigationViewDisplayModeChangedEventArgs {}
unsafe impl Sync for NavigationViewDisplayModeChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationViewItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(NavigationViewItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    NavigationViewItem,
    NavigationViewItemBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl NavigationViewItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::INavigationViewItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn INavigationViewItemFactory<R, F: FnOnce(&INavigationViewItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<NavigationViewItem, INavigationViewItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for NavigationViewItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationViewItem>();
}
unsafe impl windows_core::Interface for NavigationViewItem {
    type Vtable = <INavigationViewItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationViewItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationViewItem {
    type Target = INavigationViewItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationViewItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationViewItem";
}
unsafe impl Send for NavigationViewItem {}
unsafe impl Sync for NavigationViewItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationViewItemBase(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(NavigationViewItemBase, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    NavigationViewItemBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl windows_core::RuntimeType for NavigationViewItemBase {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationViewItemBase>();
}
unsafe impl windows_core::Interface for NavigationViewItemBase {
    type Vtable = <INavigationViewItemBase as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationViewItemBase as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationViewItemBase {
    type Target = INavigationViewItemBase;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationViewItemBase {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationViewItemBase";
}
unsafe impl Send for NavigationViewItemBase {}
unsafe impl Sync for NavigationViewItemBase {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationViewItemHeader(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(NavigationViewItemHeader, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    NavigationViewItemHeader,
    NavigationViewItemBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl NavigationViewItemHeader {
    pub fn new() -> windows_core::Result<Self> {
        Self::INavigationViewItemHeaderFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn INavigationViewItemHeaderFactory<R, F: FnOnce(&INavigationViewItemHeaderFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<NavigationViewItemHeader, INavigationViewItemHeaderFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for NavigationViewItemHeader {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationViewItemHeader>();
}
unsafe impl windows_core::Interface for NavigationViewItemHeader {
    type Vtable = <INavigationViewItemHeader as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationViewItemHeader as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationViewItemHeader {
    type Target = INavigationViewItemHeader;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationViewItemHeader {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationViewItemHeader";
}
unsafe impl Send for NavigationViewItemHeader {}
unsafe impl Sync for NavigationViewItemHeader {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationViewItemSeparator(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    NavigationViewItemSeparator,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    NavigationViewItemSeparator,
    NavigationViewItemBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl NavigationViewItemSeparator {
    pub fn new() -> windows_core::Result<Self> {
        Self::INavigationViewItemSeparatorFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn INavigationViewItemSeparatorFactory<
        R,
        F: FnOnce(&INavigationViewItemSeparatorFactory) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            NavigationViewItemSeparator,
            INavigationViewItemSeparatorFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for NavigationViewItemSeparator {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationViewItemSeparator>();
}
unsafe impl windows_core::Interface for NavigationViewItemSeparator {
    type Vtable = <INavigationViewItemSeparator as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationViewItemSeparator as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationViewItemSeparator {
    type Target = INavigationViewItemSeparator;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationViewItemSeparator {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationViewItemSeparator";
}
unsafe impl Send for NavigationViewItemSeparator {}
unsafe impl Sync for NavigationViewItemSeparator {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationViewSelectionChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    NavigationViewSelectionChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for NavigationViewSelectionChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INavigationViewSelectionChangedEventArgs>();
}
unsafe impl windows_core::Interface for NavigationViewSelectionChangedEventArgs {
    type Vtable = <INavigationViewSelectionChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INavigationViewSelectionChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for NavigationViewSelectionChangedEventArgs {
    type Target = INavigationViewSelectionChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NavigationViewSelectionChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NavigationViewSelectionChangedEventArgs";
}
unsafe impl Send for NavigationViewSelectionChangedEventArgs {}
unsafe impl Sync for NavigationViewSelectionChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NumberBox(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(NumberBox, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(NumberBox, Control, FrameworkElement, UIElement, DependencyObject);
impl NumberBox {
    pub fn new() -> windows_core::Result<Self> {
        Self::INumberBoxFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn INumberBoxFactory<R, F: FnOnce(&INumberBoxFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<NumberBox, INumberBoxFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for NumberBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, INumberBox>();
}
unsafe impl windows_core::Interface for NumberBox {
    type Vtable = <INumberBox as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INumberBox as windows_core::Interface>::IID;
}
impl core::ops::Deref for NumberBox {
    type Target = INumberBox;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NumberBox {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NumberBox";
}
unsafe impl Send for NumberBox {}
unsafe impl Sync for NumberBox {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NumberBoxSpinButtonPlacementMode(pub i32);
impl NumberBoxSpinButtonPlacementMode {
    pub const Hidden: Self = Self(0);
    pub const Compact: Self = Self(1);
    pub const Inline: Self = Self(2);
}
impl windows_core::imp::TypeKind for NumberBoxSpinButtonPlacementMode {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for NumberBoxSpinButtonPlacementMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Microsoft.UI.Xaml.Controls.NumberBoxSpinButtonPlacementMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NumberBoxValueChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    NumberBoxValueChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for NumberBoxValueChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INumberBoxValueChangedEventArgs>();
}
unsafe impl windows_core::Interface for NumberBoxValueChangedEventArgs {
    type Vtable = <INumberBoxValueChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INumberBoxValueChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for NumberBoxValueChangedEventArgs {
    type Target = INumberBoxValueChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NumberBoxValueChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.NumberBoxValueChangedEventArgs";
}
unsafe impl Send for NumberBoxValueChangedEventArgs {}
unsafe impl Sync for NumberBoxValueChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Orientation(pub i32);
impl Orientation {
    pub const Vertical: Self = Self(0);
    pub const Horizontal: Self = Self(1);
}
impl windows_core::imp::TypeKind for Orientation {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Orientation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.Orientation;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlappedPresenter(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(OverlappedPresenter, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(OverlappedPresenter, AppWindowPresenter);
impl windows_core::RuntimeType for OverlappedPresenter {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IOverlappedPresenter>();
}
unsafe impl windows_core::Interface for OverlappedPresenter {
    type Vtable = <IOverlappedPresenter as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IOverlappedPresenter as windows_core::Interface>::IID;
}
impl core::ops::Deref for OverlappedPresenter {
    type Target = IOverlappedPresenter;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for OverlappedPresenter {
    const NAME: &'static str = "Microsoft.UI.Windowing.OverlappedPresenter";
}
unsafe impl Send for OverlappedPresenter {}
unsafe impl Sync for OverlappedPresenter {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverlappedPresenterState(pub i32);
impl OverlappedPresenterState {
    pub const Maximized: Self = Self(0);
    pub const Minimized: Self = Self(1);
    pub const Restored: Self = Self(2);
}
impl windows_core::imp::TypeKind for OverlappedPresenterState {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for OverlappedPresenterState {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Windowing.OverlappedPresenterState;i4)");
}
pub type PACKAGEDEPENDENCY_CONTEXT = *mut core::ffi::c_void;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PACKAGE_VERSION {
    pub Anonymous: PACKAGE_VERSION_0,
}
impl Default for PACKAGE_VERSION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub union PACKAGE_VERSION_0 {
    pub Version: u64,
    pub Anonymous: PACKAGE_VERSION_0_0,
}
impl Default for PACKAGE_VERSION_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PACKAGE_VERSION_0_0 {
    pub Revision: u16,
    pub Build: u16,
    pub Minor: u16,
    pub Major: u16,
}
pub const PM_REMOVE: i32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
pub type PSID = *mut core::ffi::c_void;
pub type PackageDependencyLifetimeKind = i32;
pub const PackageDependencyLifetimeKind_FilePath: PackageDependencyLifetimeKind = 1;
pub const PackageDependencyLifetimeKind_Process: PackageDependencyLifetimeKind = 0;
pub const PackageDependencyLifetimeKind_RegistryKey: PackageDependencyLifetimeKind = 2;
pub type PackageDependencyProcessorArchitectures = u32;
pub const PackageDependencyProcessorArchitectures_Arm: PackageDependencyProcessorArchitectures = 8;
pub const PackageDependencyProcessorArchitectures_Arm64: PackageDependencyProcessorArchitectures = 16;
pub const PackageDependencyProcessorArchitectures_Neutral: PackageDependencyProcessorArchitectures = 1;
pub const PackageDependencyProcessorArchitectures_None: PackageDependencyProcessorArchitectures = 0;
pub const PackageDependencyProcessorArchitectures_X64: PackageDependencyProcessorArchitectures = 4;
pub const PackageDependencyProcessorArchitectures_X86: PackageDependencyProcessorArchitectures = 2;
pub const PackageDependencyProcessorArchitectures_X86A64: PackageDependencyProcessorArchitectures = 32;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Panel(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Panel, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Panel, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for Panel {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IPanel>();
}
unsafe impl windows_core::Interface for Panel {
    type Vtable = <IPanel as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPanel as windows_core::Interface>::IID;
}
impl core::ops::Deref for Panel {
    type Target = IPanel;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Panel {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Panel";
}
unsafe impl Send for Panel {}
unsafe impl Sync for Panel {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasswordBox(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PasswordBox, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(PasswordBox, Control, FrameworkElement, UIElement, DependencyObject);
impl PasswordBox {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PasswordBox, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for PasswordBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IPasswordBox>();
}
unsafe impl windows_core::Interface for PasswordBox {
    type Vtable = <IPasswordBox as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPasswordBox as windows_core::Interface>::IID;
}
impl core::ops::Deref for PasswordBox {
    type Target = IPasswordBox;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PasswordBox {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.PasswordBox";
}
unsafe impl Send for PasswordBox {}
unsafe impl Sync for PasswordBox {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PatternInterface(pub i32);
impl PatternInterface {
    pub const Invoke: Self = Self(0);
    pub const Selection: Self = Self(1);
    pub const Value: Self = Self(2);
    pub const RangeValue: Self = Self(3);
    pub const Scroll: Self = Self(4);
    pub const ScrollItem: Self = Self(5);
    pub const ExpandCollapse: Self = Self(6);
    pub const Grid: Self = Self(7);
    pub const GridItem: Self = Self(8);
    pub const MultipleView: Self = Self(9);
    pub const Window: Self = Self(10);
    pub const SelectionItem: Self = Self(11);
    pub const Dock: Self = Self(12);
    pub const Table: Self = Self(13);
    pub const TableItem: Self = Self(14);
    pub const Toggle: Self = Self(15);
    pub const Transform: Self = Self(16);
    pub const Text: Self = Self(17);
    pub const ItemContainer: Self = Self(18);
    pub const VirtualizedItem: Self = Self(19);
    pub const Text2: Self = Self(20);
    pub const TextChild: Self = Self(21);
    pub const TextRange: Self = Self(22);
    pub const Annotation: Self = Self(23);
    pub const Drag: Self = Self(24);
    pub const DropTarget: Self = Self(25);
    pub const ObjectModel: Self = Self(26);
    pub const Spreadsheet: Self = Self(27);
    pub const SpreadsheetItem: Self = Self(28);
    pub const Styles: Self = Self(29);
    pub const Transform2: Self = Self(30);
    pub const SynchronizedInput: Self = Self(31);
    pub const TextEdit: Self = Self(32);
    pub const CustomNavigation: Self = Self(33);
}
impl windows_core::imp::TypeKind for PatternInterface {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for PatternInterface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Automation.Peers.PatternInterface;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickFileResult(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PickFileResult, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for PickFileResult {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPickFileResult>();
}
unsafe impl windows_core::Interface for PickFileResult {
    type Vtable = <IPickFileResult as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPickFileResult as windows_core::Interface>::IID;
}
impl core::ops::Deref for PickFileResult {
    type Target = IPickFileResult;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PickFileResult {
    const NAME: &'static str = "Microsoft.Windows.Storage.Pickers.PickFileResult";
}
unsafe impl Send for PickFileResult {}
unsafe impl Sync for PickFileResult {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickFolderResult(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PickFolderResult, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for PickFolderResult {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPickFolderResult>();
}
unsafe impl windows_core::Interface for PickFolderResult {
    type Vtable = <IPickFolderResult as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPickFolderResult as windows_core::Interface>::IID;
}
impl core::ops::Deref for PickFolderResult {
    type Target = IPickFolderResult;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PickFolderResult {
    const NAME: &'static str = "Microsoft.Windows.Storage.Pickers.PickFolderResult";
}
unsafe impl Send for PickFolderResult {}
unsafe impl Sync for PickFolderResult {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipsPager(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PipsPager, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(PipsPager, Control, FrameworkElement, UIElement, DependencyObject);
impl PipsPager {
    pub fn new() -> windows_core::Result<Self> {
        Self::IPipsPagerFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn SelectedPageIndexProperty() -> windows_core::Result<DependencyProperty> {
        Self::IPipsPagerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).SelectedPageIndexProperty)(
                windows_core::Interface::as_raw(this),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IPipsPagerFactory<R, F: FnOnce(&IPipsPagerFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PipsPager, IPipsPagerFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
    fn IPipsPagerStatics<R, F: FnOnce(&IPipsPagerStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PipsPager, IPipsPagerStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for PipsPager {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IPipsPager>();
}
unsafe impl windows_core::Interface for PipsPager {
    type Vtable = <IPipsPager as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPipsPager as windows_core::Interface>::IID;
}
impl core::ops::Deref for PipsPager {
    type Target = IPipsPager;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PipsPager {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.PipsPager";
}
unsafe impl Send for PipsPager {}
unsafe impl Sync for PipsPager {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipsPagerSelectedIndexChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    PipsPagerSelectedIndexChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for PipsPagerSelectedIndexChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPipsPagerSelectedIndexChangedEventArgs>();
}
unsafe impl windows_core::Interface for PipsPagerSelectedIndexChangedEventArgs {
    type Vtable = <IPipsPagerSelectedIndexChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPipsPagerSelectedIndexChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for PipsPagerSelectedIndexChangedEventArgs {
    type Target = IPipsPagerSelectedIndexChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PipsPagerSelectedIndexChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.PipsPagerSelectedIndexChangedEventArgs";
}
unsafe impl Send for PipsPagerSelectedIndexChangedEventArgs {}
unsafe impl Sync for PipsPagerSelectedIndexChangedEventArgs {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl windows_core::imp::TypeKind for Point {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Point {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.Point;f4;f4)");
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointInt32 {
    pub x: i32,
    pub y: i32,
}
impl windows_core::imp::TypeKind for PointInt32 {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for PointInt32 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Graphics.PointInt32;i4;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pointer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Pointer, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for Pointer {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IPointer>();
}
unsafe impl windows_core::Interface for Pointer {
    type Vtable = <IPointer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointer as windows_core::Interface>::IID;
}
impl core::ops::Deref for Pointer {
    type Target = IPointer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Pointer {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.Pointer";
}
unsafe impl Send for Pointer {}
unsafe impl Sync for Pointer {}
windows_core::imp::define_interface!(
    PointerEventHandler,
    PointerEventHandler_Vtbl,
    0xa48a71e1_8bb4_5597_9e31_903a3f6a04fb
);
impl windows_core::RuntimeType for PointerEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct PointerEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct PointerEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<PointerRoutedEventArgs>) + 'static>
    PointerEventHandlerBox<F>
{
    const VTABLE: PointerEventHandler_Vtbl = PointerEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<PointerEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<PointerEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<PointerEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<PointerEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerPoint(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PointerPoint, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for PointerPoint {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPointerPoint>();
}
unsafe impl windows_core::Interface for PointerPoint {
    type Vtable = <IPointerPoint as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointerPoint as windows_core::Interface>::IID;
}
impl core::ops::Deref for PointerPoint {
    type Target = IPointerPoint;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PointerPoint {
    const NAME: &'static str = "Microsoft.UI.Input.PointerPoint";
}
unsafe impl Send for PointerPoint {}
unsafe impl Sync for PointerPoint {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerPointProperties(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PointerPointProperties, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for PointerPointProperties {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPointerPointProperties>();
}
unsafe impl windows_core::Interface for PointerPointProperties {
    type Vtable = <IPointerPointProperties as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointerPointProperties as windows_core::Interface>::IID;
}
impl core::ops::Deref for PointerPointProperties {
    type Target = IPointerPointProperties;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PointerPointProperties {
    const NAME: &'static str = "Microsoft.UI.Input.PointerPointProperties";
}
unsafe impl Send for PointerPointProperties {}
unsafe impl Sync for PointerPointProperties {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerRoutedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(PointerRoutedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(PointerRoutedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for PointerRoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPointerRoutedEventArgs>();
}
unsafe impl windows_core::Interface for PointerRoutedEventArgs {
    type Vtable = <IPointerRoutedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointerRoutedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for PointerRoutedEventArgs {
    type Target = IPointerRoutedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PointerRoutedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Input.PointerRoutedEventArgs";
}
unsafe impl Send for PointerRoutedEventArgs {}
unsafe impl Sync for PointerRoutedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerUpdateKind(pub i32);
impl PointerUpdateKind {
    pub const Other: Self = Self(0);
    pub const LeftButtonPressed: Self = Self(1);
    pub const LeftButtonReleased: Self = Self(2);
    pub const RightButtonPressed: Self = Self(3);
    pub const RightButtonReleased: Self = Self(4);
    pub const MiddleButtonPressed: Self = Self(5);
    pub const MiddleButtonReleased: Self = Self(6);
    pub const XButton1Pressed: Self = Self(7);
    pub const XButton1Released: Self = Self(8);
    pub const XButton2Pressed: Self = Self(9);
    pub const XButton2Released: Self = Self(10);
}
impl windows_core::imp::TypeKind for PointerUpdateKind {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for PointerUpdateKind {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Input.PointerUpdateKind;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Popup(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Popup, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Popup, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for Popup {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IPopup>();
}
unsafe impl windows_core::Interface for Popup {
    type Vtable = <IPopup as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPopup as windows_core::Interface>::IID;
}
impl core::ops::Deref for Popup {
    type Target = IPopup;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Popup {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.Popup";
}
unsafe impl Send for Popup {}
unsafe impl Sync for Popup {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ProgressBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ProgressBar, RangeBase, Control, FrameworkElement, UIElement, DependencyObject);
impl ProgressBar {
    pub fn new() -> windows_core::Result<Self> {
        Self::IProgressBarFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IProgressBarFactory<R, F: FnOnce(&IProgressBarFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ProgressBar, IProgressBarFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ProgressBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IProgressBar>();
}
unsafe impl windows_core::Interface for ProgressBar {
    type Vtable = <IProgressBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IProgressBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for ProgressBar {
    type Target = IProgressBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ProgressBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ProgressBar";
}
unsafe impl Send for ProgressBar {}
unsafe impl Sync for ProgressBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressRing(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ProgressRing, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ProgressRing, Control, FrameworkElement, UIElement, DependencyObject);
impl ProgressRing {
    pub fn new() -> windows_core::Result<Self> {
        Self::IProgressRingFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IProgressRingFactory<R, F: FnOnce(&IProgressRingFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ProgressRing, IProgressRingFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ProgressRing {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IProgressRing>();
}
unsafe impl windows_core::Interface for ProgressRing {
    type Vtable = <IProgressRing as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IProgressRing as windows_core::Interface>::IID;
}
impl core::ops::Deref for ProgressRing {
    type Target = IProgressRing;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ProgressRing {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ProgressRing";
}
unsafe impl Send for ProgressRing {}
unsafe impl Sync for ProgressRing {}
pub struct PropertyValue;
impl PropertyValue {
    pub fn CreateDouble(value: f64) -> windows_core::Result<windows_core::IInspectable> {
        Self::IPropertyValueStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateDouble)(
                windows_core::Interface::as_raw(this),
                value,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn CreateBoolean(value: bool) -> windows_core::Result<windows_core::IInspectable> {
        Self::IPropertyValueStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateBoolean)(
                windows_core::Interface::as_raw(this),
                value,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn CreateString(value: &str) -> windows_core::Result<windows_core::IInspectable> {
        Self::IPropertyValueStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateString)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IPropertyValueStatics<R, F: FnOnce(&IPropertyValueStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PropertyValue, IPropertyValueStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeName for PropertyValue {
    const NAME: &'static str = "Windows.Foundation.PropertyValue";
}
pub const QS_ALLINPUT: i32 = 7423;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RAWHID {
    pub dwSizeHid: u32,
    pub dwCount: u32,
    pub bRawData: [u8; 1],
}
impl Default for RAWHID {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RAWINPUT {
    pub header: RAWINPUTHEADER,
    pub data: RAWINPUT_0,
}
impl Default for RAWINPUT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union RAWINPUT_0 {
    pub mouse: RAWMOUSE,
    pub keyboard: RAWKEYBOARD,
    pub hid: RAWHID,
}
impl Default for RAWINPUT_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RAWINPUTDEVICE {
    pub usUsagePage: u16,
    pub usUsage: u16,
    pub dwFlags: u32,
    pub hwndTarget: HWND,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RAWINPUTHEADER {
    pub dwType: u32,
    pub dwSize: u32,
    pub hDevice: HANDLE,
    pub wParam: WPARAM,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RAWKEYBOARD {
    pub MakeCode: u16,
    pub Flags: u16,
    pub Reserved: u16,
    pub VKey: u16,
    pub Message: u32,
    pub ExtraInformation: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RAWMOUSE {
    pub usFlags: u16,
    pub Anonymous: RAWMOUSE_0,
    pub ulRawButtons: u32,
    pub lLastX: i32,
    pub lLastY: i32,
    pub ulExtraInformation: u32,
}
impl Default for RAWMOUSE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union RAWMOUSE_0 {
    pub ulButtons: u32,
    pub Anonymous: RAWMOUSE_0_0,
}
impl Default for RAWMOUSE_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RAWMOUSE_0_0 {
    pub usButtonFlags: u16,
    pub usButtonData: u16,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RGBQUAD {
    pub rgbBlue: u8,
    pub rgbGreen: u8,
    pub rgbRed: u8,
    pub rgbReserved: u8,
}
pub const RIDEV_REMOVE: i32 = 1;
pub const RID_INPUT: i32 = 268435459;
pub const RIM_TYPEMOUSE: i32 = 0;
pub const RPC_E_CHANGED_MODE: windows_core::HRESULT = windows_core::HRESULT(0x80010106_u32 as _);
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioButtons(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RadioButtons, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(RadioButtons, Control, FrameworkElement, UIElement, DependencyObject);
impl RadioButtons {
    pub fn new() -> windows_core::Result<Self> {
        Self::IRadioButtonsFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IRadioButtonsFactory<R, F: FnOnce(&IRadioButtonsFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RadioButtons, IRadioButtonsFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RadioButtons {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadioButtons>();
}
unsafe impl windows_core::Interface for RadioButtons {
    type Vtable = <IRadioButtons as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRadioButtons as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadioButtons {
    type Target = IRadioButtons;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadioButtons {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.RadioButtons";
}
unsafe impl Send for RadioButtons {}
unsafe impl Sync for RadioButtons {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioMenuFlyoutItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RadioMenuFlyoutItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    RadioMenuFlyoutItem,
    MenuFlyoutItem,
    MenuFlyoutItemBase,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl RadioMenuFlyoutItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::IRadioMenuFlyoutItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IRadioMenuFlyoutItemFactory<R, F: FnOnce(&IRadioMenuFlyoutItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RadioMenuFlyoutItem, IRadioMenuFlyoutItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RadioMenuFlyoutItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadioMenuFlyoutItem>();
}
unsafe impl windows_core::Interface for RadioMenuFlyoutItem {
    type Vtable = <IRadioMenuFlyoutItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRadioMenuFlyoutItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadioMenuFlyoutItem {
    type Target = IRadioMenuFlyoutItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadioMenuFlyoutItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.RadioMenuFlyoutItem";
}
unsafe impl Send for RadioMenuFlyoutItem {}
unsafe impl Sync for RadioMenuFlyoutItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeBase(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RangeBase, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(RangeBase, Control, FrameworkElement, UIElement, DependencyObject);
impl RangeBase {
    pub fn ValueProperty() -> windows_core::Result<DependencyProperty> {
        Self::IRangeBaseStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).ValueProperty)(windows_core::Interface::as_raw(this), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IRangeBaseStatics<R, F: FnOnce(&IRangeBaseStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RangeBase, IRangeBaseStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RangeBase {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IRangeBase>();
}
unsafe impl windows_core::Interface for RangeBase {
    type Vtable = <IRangeBase as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRangeBase as windows_core::Interface>::IID;
}
impl core::ops::Deref for RangeBase {
    type Target = IRangeBase;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RangeBase {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.RangeBase";
}
unsafe impl Send for RangeBase {}
unsafe impl Sync for RangeBase {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeBaseValueChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RangeBaseValueChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(RangeBaseValueChangedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for RangeBaseValueChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRangeBaseValueChangedEventArgs>();
}
unsafe impl windows_core::Interface for RangeBaseValueChangedEventArgs {
    type Vtable = <IRangeBaseValueChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRangeBaseValueChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RangeBaseValueChangedEventArgs {
    type Target = IRangeBaseValueChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RangeBaseValueChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs";
}
unsafe impl Send for RangeBaseValueChangedEventArgs {}
unsafe impl Sync for RangeBaseValueChangedEventArgs {}
windows_core::imp::define_interface!(
    RangeBaseValueChangedEventHandler,
    RangeBaseValueChangedEventHandler_Vtbl,
    0x23f0e209_9455_54cb_b8bc_0b49553c7dcc
);
impl windows_core::RuntimeType for RangeBaseValueChangedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct RangeBaseValueChangedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct RangeBaseValueChangedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RangeBaseValueChangedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RangeBaseValueChangedEventArgs>) + 'static>
    RangeBaseValueChangedEventHandlerBox<F>
{
    const VTABLE: RangeBaseValueChangedEventHandler_Vtbl = RangeBaseValueChangedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<RangeBaseValueChangedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<RangeBaseValueChangedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<RangeBaseValueChangedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<RangeBaseValueChangedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RatingControl(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RatingControl, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(RatingControl, Control, FrameworkElement, UIElement, DependencyObject);
impl RatingControl {
    pub fn new() -> windows_core::Result<Self> {
        Self::IRatingControlFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn ValueProperty() -> windows_core::Result<DependencyProperty> {
        Self::IRatingControlStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).ValueProperty)(windows_core::Interface::as_raw(this), &mut result__)
                .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IRatingControlFactory<R, F: FnOnce(&IRatingControlFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RatingControl, IRatingControlFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
    fn IRatingControlStatics<R, F: FnOnce(&IRatingControlStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RatingControl, IRatingControlStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RatingControl {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRatingControl>();
}
unsafe impl windows_core::Interface for RatingControl {
    type Vtable = <IRatingControl as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRatingControl as windows_core::Interface>::IID;
}
impl core::ops::Deref for RatingControl {
    type Target = IRatingControl;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RatingControl {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.RatingControl";
}
unsafe impl Send for RatingControl {}
unsafe impl Sync for RatingControl {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl windows_core::imp::TypeKind for Rect {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Rect {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.Rect;f4;f4;f4;f4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RectangleGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RectangleGeometry, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(RectangleGeometry, Geometry, DependencyObject);
impl RectangleGeometry {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RectangleGeometry, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RectangleGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRectangleGeometry>();
}
unsafe impl windows_core::Interface for RectangleGeometry {
    type Vtable = <IRectangleGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRectangleGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for RectangleGeometry {
    type Target = IRectangleGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RectangleGeometry {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.RectangleGeometry";
}
unsafe impl Send for RectangleGeometry {}
unsafe impl Sync for RectangleGeometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderTargetBitmap(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RenderTargetBitmap, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(RenderTargetBitmap, ImageSource, DependencyObject);
impl RenderTargetBitmap {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<RenderTargetBitmap, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RenderTargetBitmap {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRenderTargetBitmap>();
}
unsafe impl windows_core::Interface for RenderTargetBitmap {
    type Vtable = <IRenderTargetBitmap as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRenderTargetBitmap as windows_core::Interface>::IID;
}
impl core::ops::Deref for RenderTargetBitmap {
    type Target = IRenderTargetBitmap;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RenderTargetBitmap {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap";
}
unsafe impl Send for RenderTargetBitmap {}
unsafe impl Sync for RenderTargetBitmap {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDictionary(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ResourceDictionary, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ResourceDictionary, DependencyObject);
impl windows_core::RuntimeType for ResourceDictionary {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IResourceDictionary>();
}
unsafe impl windows_core::Interface for ResourceDictionary {
    type Vtable = <IResourceDictionary as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IResourceDictionary as windows_core::Interface>::IID;
}
impl core::ops::Deref for ResourceDictionary {
    type Target = IResourceDictionary;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ResourceDictionary {
    const NAME: &'static str = "Microsoft.UI.Xaml.ResourceDictionary";
}
unsafe impl Send for ResourceDictionary {}
unsafe impl Sync for ResourceDictionary {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(RoutedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for RoutedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRoutedEventArgs>();
}
unsafe impl windows_core::Interface for RoutedEventArgs {
    type Vtable = <IRoutedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRoutedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RoutedEventArgs {
    type Target = IRoutedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RoutedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.RoutedEventArgs";
}
unsafe impl Send for RoutedEventArgs {}
unsafe impl Sync for RoutedEventArgs {}
windows_core::imp::define_interface!(
    RoutedEventHandler,
    RoutedEventHandler_Vtbl,
    0xdae23d85_69ca_5bdf_805b_6161a3a215cc
);
impl windows_core::RuntimeType for RoutedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct RoutedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct RoutedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<RoutedEventArgs>) + 'static>
    RoutedEventHandlerBox<F>
{
    const VTABLE: RoutedEventHandler_Vtbl = RoutedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<RoutedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<RoutedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<RoutedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<RoutedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
#[derive(Clone, Copy)]
pub struct SHELLEXECUTEINFOW {
    pub cbSize: u32,
    pub fMask: u32,
    pub hwnd: HWND,
    pub lpVerb: windows_core::PCWSTR,
    pub lpFile: windows_core::PCWSTR,
    pub lpParameters: windows_core::PCWSTR,
    pub lpDirectory: windows_core::PCWSTR,
    pub nShow: i32,
    pub hInstApp: HINSTANCE,
    pub lpIDList: *mut core::ffi::c_void,
    pub lpClass: windows_core::PCWSTR,
    pub hkeyClass: HKEY,
    pub dwHotKey: u32,
    pub Anonymous: SHELLEXECUTEINFOW_0,
    pub hProcess: HANDLE,
}
#[cfg(target_arch = "x86")]
impl Default for SHELLEXECUTEINFOW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
#[derive(Clone, Copy)]
pub union SHELLEXECUTEINFOW_0 {
    pub hIcon: HANDLE,
    pub hMonitor: HANDLE,
}
#[cfg(target_arch = "x86")]
impl Default for SHELLEXECUTEINFOW_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_arch = "x86_64"))]
#[derive(Clone, Copy)]
pub struct SHELLEXECUTEINFOW {
    pub cbSize: u32,
    pub fMask: u32,
    pub hwnd: HWND,
    pub lpVerb: windows_core::PCWSTR,
    pub lpFile: windows_core::PCWSTR,
    pub lpParameters: windows_core::PCWSTR,
    pub lpDirectory: windows_core::PCWSTR,
    pub nShow: i32,
    pub hInstApp: HINSTANCE,
    pub lpIDList: *mut core::ffi::c_void,
    pub lpClass: windows_core::PCWSTR,
    pub hkeyClass: HKEY,
    pub dwHotKey: u32,
    pub Anonymous: SHELLEXECUTEINFOW_0,
    pub hProcess: HANDLE,
}
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_arch = "x86_64"))]
impl Default for SHELLEXECUTEINFOW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_arch = "x86_64"))]
#[derive(Clone, Copy)]
pub union SHELLEXECUTEINFOW_0 {
    pub hIcon: HANDLE,
    pub hMonitor: HANDLE,
}
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_arch = "x86_64"))]
impl Default for SHELLEXECUTEINFOW_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type SIIGBF = i32;
pub const SIIGBF_ICONONLY: SIIGBF = 4;
pub const SIIGBF_RESIZETOFIT: SIIGBF = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}
pub const STATEREPOSITORY_E_DEPENDENCY_NOT_RESOLVED: windows_core::HRESULT = windows_core::HRESULT(0x80670016_u32 as _);
pub type SUBCLASSPROC = Option<
    unsafe extern "system" fn(
        hwnd: HWND,
        umsg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        uidsubclass: usize,
        dwrefdata: usize,
    ) -> LRESULT,
>;
pub const SWP_HIDEWINDOW: i32 = 128;
pub const SWP_NOACTIVATE: i32 = 16;
pub const SWP_SHOWWINDOW: i32 = 64;
pub const SW_SHOWNORMAL: i32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollBarVisibility(pub i32);
impl ScrollBarVisibility {
    pub const Disabled: Self = Self(0);
    pub const Auto: Self = Self(1);
    pub const Hidden: Self = Self(2);
    pub const Visible: Self = Self(3);
}
impl windows_core::imp::TypeKind for ScrollBarVisibility {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ScrollBarVisibility {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.ScrollBarVisibility;i4)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollMode(pub i32);
impl ScrollMode {
    pub const Disabled: Self = Self(0);
    pub const Enabled: Self = Self(1);
    pub const Auto: Self = Self(2);
}
impl windows_core::imp::TypeKind for ScrollMode {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for ScrollMode {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.ScrollMode;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollViewer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ScrollViewer, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ScrollViewer,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ScrollViewer {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ScrollViewer, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
    pub fn SetHorizontalScrollBarVisibility<P0>(
        element: P0,
        horizontalscrollbarvisibility: ScrollBarVisibility,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IScrollViewerStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetHorizontalScrollBarVisibility)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                horizontalscrollbarvisibility,
            )
            .ok()
        })
    }
    pub fn SetVerticalScrollBarVisibility<P0>(
        element: P0,
        verticalscrollbarvisibility: ScrollBarVisibility,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IScrollViewerStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetVerticalScrollBarVisibility)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                verticalscrollbarvisibility,
            )
            .ok()
        })
    }
    fn IScrollViewerStatics<R, F: FnOnce(&IScrollViewerStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ScrollViewer, IScrollViewerStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ScrollViewer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IScrollViewer>();
}
unsafe impl windows_core::Interface for ScrollViewer {
    type Vtable = <IScrollViewer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IScrollViewer as windows_core::Interface>::IID;
}
impl core::ops::Deref for ScrollViewer {
    type Target = IScrollViewer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ScrollViewer {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ScrollViewer";
}
unsafe impl Send for ScrollViewer {}
unsafe impl Sync for ScrollViewer {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollViewerViewChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ScrollViewerViewChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for ScrollViewerViewChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IScrollViewerViewChangedEventArgs>();
}
unsafe impl windows_core::Interface for ScrollViewerViewChangedEventArgs {
    type Vtable = <IScrollViewerViewChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IScrollViewerViewChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ScrollViewerViewChangedEventArgs {
    type Target = IScrollViewerViewChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ScrollViewerViewChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ScrollViewerViewChangedEventArgs";
}
unsafe impl Send for ScrollViewerViewChangedEventArgs {}
unsafe impl Sync for ScrollViewerViewChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SelectionChangedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(SelectionChangedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for SelectionChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISelectionChangedEventArgs>();
}
unsafe impl windows_core::Interface for SelectionChangedEventArgs {
    type Vtable = <ISelectionChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISelectionChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for SelectionChangedEventArgs {
    type Target = ISelectionChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SelectionChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs";
}
unsafe impl Send for SelectionChangedEventArgs {}
unsafe impl Sync for SelectionChangedEventArgs {}
windows_core::imp::define_interface!(
    SelectionChangedEventHandler,
    SelectionChangedEventHandler_Vtbl,
    0xa232390d_0e34_595e_8931_fa928a9909f4
);
impl windows_core::RuntimeType for SelectionChangedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct SelectionChangedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct SelectionChangedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SelectionChangedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SelectionChangedEventArgs>) + 'static>
    SelectionChangedEventHandlerBox<F>
{
    const VTABLE: SelectionChangedEventHandler_Vtbl = SelectionChangedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<SelectionChangedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<SelectionChangedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<SelectionChangedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<SelectionChangedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selector(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Selector, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Selector, ItemsControl, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for Selector {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ISelector>();
}
unsafe impl windows_core::Interface for Selector {
    type Vtable = <ISelector as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISelector as windows_core::Interface>::IID;
}
impl core::ops::Deref for Selector {
    type Target = ISelector;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Selector {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.Selector";
}
unsafe impl Send for Selector {}
unsafe impl Sync for Selector {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SelectorBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(SelectorBar, Control, FrameworkElement, UIElement, DependencyObject);
impl SelectorBar {
    pub fn new() -> windows_core::Result<Self> {
        Self::ISelectorBarFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ISelectorBarFactory<R, F: FnOnce(&ISelectorBarFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<SelectorBar, ISelectorBarFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for SelectorBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ISelectorBar>();
}
unsafe impl windows_core::Interface for SelectorBar {
    type Vtable = <ISelectorBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISelectorBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for SelectorBar {
    type Target = ISelectorBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SelectorBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.SelectorBar";
}
unsafe impl Send for SelectorBar {}
unsafe impl Sync for SelectorBar {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorBarItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SelectorBarItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    SelectorBarItem,
    ItemContainer,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl SelectorBarItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::ISelectorBarItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ISelectorBarItemFactory<R, F: FnOnce(&ISelectorBarItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<SelectorBarItem, ISelectorBarItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for SelectorBarItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISelectorBarItem>();
}
unsafe impl windows_core::Interface for SelectorBarItem {
    type Vtable = <ISelectorBarItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISelectorBarItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for SelectorBarItem {
    type Target = ISelectorBarItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SelectorBarItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.SelectorBarItem";
}
unsafe impl Send for SelectorBarItem {}
unsafe impl Sync for SelectorBarItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorBarSelectionChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    SelectorBarSelectionChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for SelectorBarSelectionChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISelectorBarSelectionChangedEventArgs>();
}
unsafe impl windows_core::Interface for SelectorBarSelectionChangedEventArgs {
    type Vtable = <ISelectorBarSelectionChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISelectorBarSelectionChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for SelectorBarSelectionChangedEventArgs {
    type Target = ISelectorBarSelectionChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SelectorBarSelectionChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.SelectorBarSelectionChangedEventArgs";
}
unsafe impl Send for SelectorBarSelectionChangedEventArgs {}
unsafe impl Sync for SelectorBarSelectionChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SelectorItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    SelectorItem,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl windows_core::RuntimeType for SelectorItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISelectorItem>();
}
unsafe impl windows_core::Interface for SelectorItem {
    type Vtable = <ISelectorItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISelectorItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for SelectorItem {
    type Target = ISelectorItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SelectorItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.SelectorItem";
}
unsafe impl Send for SelectorItem {}
unsafe impl Sync for SelectorItem {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
impl windows_core::imp::TypeKind for Size {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Size {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.Size;f4;f4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SizeChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SizeChangedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(SizeChangedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for SizeChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISizeChangedEventArgs>();
}
unsafe impl windows_core::Interface for SizeChangedEventArgs {
    type Vtable = <ISizeChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISizeChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for SizeChangedEventArgs {
    type Target = ISizeChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SizeChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.SizeChangedEventArgs";
}
unsafe impl Send for SizeChangedEventArgs {}
unsafe impl Sync for SizeChangedEventArgs {}
windows_core::imp::define_interface!(
    SizeChangedEventHandler,
    SizeChangedEventHandler_Vtbl,
    0x8d7b1a58_14c6_51c9_892c_9fcce368e77d
);
impl windows_core::RuntimeType for SizeChangedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct SizeChangedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct SizeChangedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SizeChangedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<SizeChangedEventArgs>) + 'static>
    SizeChangedEventHandlerBox<F>
{
    const VTABLE: SizeChangedEventHandler_Vtbl = SizeChangedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<SizeChangedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<SizeChangedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<SizeChangedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<SizeChangedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SizeInt32 {
    pub width: i32,
    pub height: i32,
}
impl windows_core::imp::TypeKind for SizeInt32 {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for SizeInt32 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Graphics.SizeInt32;i4;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Slider(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Slider, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Slider, RangeBase, Control, FrameworkElement, UIElement, DependencyObject);
impl Slider {
    pub fn new() -> windows_core::Result<Self> {
        Self::ISliderFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ISliderFactory<R, F: FnOnce(&ISliderFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Slider, ISliderFactory> = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Slider {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ISlider>();
}
unsafe impl windows_core::Interface for Slider {
    type Vtable = <ISlider as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISlider as windows_core::Interface>::IID;
}
impl core::ops::Deref for Slider {
    type Target = ISlider;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Slider {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Slider";
}
unsafe impl Send for Slider {}
unsafe impl Sync for Slider {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolidColorBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SolidColorBrush, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(SolidColorBrush, Brush, DependencyObject);
impl SolidColorBrush {
    pub fn CreateInstanceWithColor(color: Color) -> windows_core::Result<Self> {
        Self::ISolidColorBrushFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstanceWithColor)(
                windows_core::Interface::as_raw(this),
                color,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ISolidColorBrushFactory<R, F: FnOnce(&ISolidColorBrushFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<SolidColorBrush, ISolidColorBrushFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for SolidColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISolidColorBrush>();
}
unsafe impl windows_core::Interface for SolidColorBrush {
    type Vtable = <ISolidColorBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISolidColorBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for SolidColorBrush {
    type Target = ISolidColorBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SolidColorBrush {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.SolidColorBrush";
}
unsafe impl Send for SolidColorBrush {}
unsafe impl Sync for SolidColorBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackPanel(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(StackPanel, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(StackPanel, Panel, FrameworkElement, UIElement, DependencyObject);
impl StackPanel {
    pub fn new() -> windows_core::Result<Self> {
        Self::IStackPanelFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IStackPanelFactory<R, F: FnOnce(&IStackPanelFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<StackPanel, IStackPanelFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for StackPanel {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IStackPanel>();
}
unsafe impl windows_core::Interface for StackPanel {
    type Vtable = <IStackPanel as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IStackPanel as windows_core::Interface>::IID;
}
impl core::ops::Deref for StackPanel {
    type Target = IStackPanel;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for StackPanel {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.StackPanel";
}
unsafe impl Send for StackPanel {}
unsafe impl Sync for StackPanel {}
pub struct StandardDataFormats;
impl StandardDataFormats {
    pub fn Text() -> windows_core::Result<String> {
        Self::IStandardDataFormatsStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Text)(windows_core::Interface::as_raw(this), &mut result__).map(
                || {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                },
            )
        })
    }
    pub fn StorageItems() -> windows_core::Result<String> {
        Self::IStandardDataFormatsStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).StorageItems)(windows_core::Interface::as_raw(this), &mut result__)
                .map(|| {
                    let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                    hstring.to_string_lossy()
                })
        })
    }
    fn IStandardDataFormatsStatics<R, F: FnOnce(&IStandardDataFormatsStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<StandardDataFormats, IStandardDataFormatsStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeName for StandardDataFormats {
    const NAME: &'static str = "Windows.ApplicationModel.DataTransfer.StandardDataFormats";
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageFile(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(StorageFile, windows_core::IUnknown, windows_core::IInspectable, IStorageFile);
impl StorageFile {
    pub fn GetFileFromPathAsync(path: &str) -> windows_core::Result<windows_future::IAsyncOperation<Self>> {
        Self::IStorageFileStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetFileFromPathAsync)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(path)),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IStorageFileStatics<R, F: FnOnce(&IStorageFileStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<StorageFile, IStorageFileStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for StorageFile {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IStorageFile>();
}
unsafe impl windows_core::Interface for StorageFile {
    type Vtable = <IStorageFile as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IStorageFile as windows_core::Interface>::IID;
}
impl core::ops::Deref for StorageFile {
    type Target = IStorageFile;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for StorageFile {
    const NAME: &'static str = "Windows.Storage.StorageFile";
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageFolder(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    StorageFolder,
    windows_core::IUnknown,
    windows_core::IInspectable,
    IStorageFolder
);
impl StorageFolder {
    pub fn GetFolderFromPathAsync(path: &str) -> windows_core::Result<windows_future::IAsyncOperation<Self>> {
        Self::IStorageFolderStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetFolderFromPathAsync)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(path)),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IStorageFolderStatics<R, F: FnOnce(&IStorageFolderStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<StorageFolder, IStorageFolderStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for StorageFolder {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IStorageFolder>();
}
unsafe impl windows_core::Interface for StorageFolder {
    type Vtable = <IStorageFolder as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IStorageFolder as windows_core::Interface>::IID;
}
impl core::ops::Deref for StorageFolder {
    type Target = IStorageFolder;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for StorageFolder {
    const NAME: &'static str = "Windows.Storage.StorageFolder";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stretch(pub i32);
impl Stretch {
    pub const None: Self = Self(0);
    pub const Fill: Self = Self(1);
    pub const Uniform: Self = Self(2);
    pub const UniformToFill: Self = Self(3);
}
impl windows_core::imp::TypeKind for Stretch {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Stretch {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Media.Stretch;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Style(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Style, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Style, DependencyObject);
impl windows_core::RuntimeType for Style {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IStyle>();
}
unsafe impl windows_core::Interface for Style {
    type Vtable = <IStyle as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IStyle as windows_core::Interface>::IID;
}
impl core::ops::Deref for Style {
    type Target = IStyle;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Style {
    const NAME: &'static str = "Microsoft.UI.Xaml.Style";
}
unsafe impl Send for Style {}
unsafe impl Sync for Style {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Symbol(pub i32);
impl Symbol {
    pub const Previous: Self = Self(57600);
    pub const Next: Self = Self(57601);
    pub const Play: Self = Self(57602);
    pub const Pause: Self = Self(57603);
    pub const Edit: Self = Self(57604);
    pub const Save: Self = Self(57605);
    pub const Clear: Self = Self(57606);
    pub const Delete: Self = Self(57607);
    pub const Remove: Self = Self(57608);
    pub const Add: Self = Self(57609);
    pub const Cancel: Self = Self(57610);
    pub const Accept: Self = Self(57611);
    pub const More: Self = Self(57612);
    pub const Redo: Self = Self(57613);
    pub const Undo: Self = Self(57614);
    pub const Home: Self = Self(57615);
    pub const Up: Self = Self(57616);
    pub const Forward: Self = Self(57617);
    pub const Back: Self = Self(57618);
    pub const Favorite: Self = Self(57619);
    pub const Camera: Self = Self(57620);
    pub const Setting: Self = Self(57621);
    pub const Video: Self = Self(57622);
    pub const Sync: Self = Self(57623);
    pub const Download: Self = Self(57624);
    pub const Mail: Self = Self(57625);
    pub const Find: Self = Self(57626);
    pub const Help: Self = Self(57627);
    pub const Upload: Self = Self(57628);
    pub const Emoji: Self = Self(57629);
    pub const TwoPage: Self = Self(57630);
    pub const LeaveChat: Self = Self(57631);
    pub const MailForward: Self = Self(57632);
    pub const Clock: Self = Self(57633);
    pub const Send: Self = Self(57634);
    pub const Crop: Self = Self(57635);
    pub const RotateCamera: Self = Self(57636);
    pub const People: Self = Self(57637);
    pub const OpenPane: Self = Self(57638);
    pub const ClosePane: Self = Self(57639);
    pub const World: Self = Self(57640);
    pub const Flag: Self = Self(57641);
    pub const PreviewLink: Self = Self(57642);
    pub const Globe: Self = Self(57643);
    pub const Trim: Self = Self(57644);
    pub const AttachCamera: Self = Self(57645);
    pub const ZoomIn: Self = Self(57646);
    pub const Bookmarks: Self = Self(57647);
    pub const Document: Self = Self(57648);
    pub const ProtectedDocument: Self = Self(57649);
    pub const Page: Self = Self(57650);
    pub const Bullets: Self = Self(57651);
    pub const Comment: Self = Self(57652);
    pub const MailFilled: Self = Self(57653);
    pub const ContactInfo: Self = Self(57654);
    pub const HangUp: Self = Self(57655);
    pub const ViewAll: Self = Self(57656);
    pub const MapPin: Self = Self(57657);
    pub const Phone: Self = Self(57658);
    pub const VideoChat: Self = Self(57659);
    pub const Switch: Self = Self(57660);
    pub const Contact: Self = Self(57661);
    pub const Rename: Self = Self(57662);
    pub const Pin: Self = Self(57665);
    pub const MusicInfo: Self = Self(57666);
    pub const Go: Self = Self(57667);
    pub const Keyboard: Self = Self(57668);
    pub const DockLeft: Self = Self(57669);
    pub const DockRight: Self = Self(57670);
    pub const DockBottom: Self = Self(57671);
    pub const Remote: Self = Self(57672);
    pub const Refresh: Self = Self(57673);
    pub const Rotate: Self = Self(57674);
    pub const Shuffle: Self = Self(57675);
    pub const List: Self = Self(57676);
    pub const Shop: Self = Self(57677);
    pub const SelectAll: Self = Self(57678);
    pub const Orientation: Self = Self(57679);
    pub const Import: Self = Self(57680);
    pub const ImportAll: Self = Self(57681);
    pub const BrowsePhotos: Self = Self(57685);
    pub const WebCam: Self = Self(57686);
    pub const Pictures: Self = Self(57688);
    pub const SaveLocal: Self = Self(57689);
    pub const Caption: Self = Self(57690);
    pub const Stop: Self = Self(57691);
    pub const ShowResults: Self = Self(57692);
    pub const Volume: Self = Self(57693);
    pub const Repair: Self = Self(57694);
    pub const Message: Self = Self(57695);
    pub const Page2: Self = Self(57696);
    pub const CalendarDay: Self = Self(57697);
    pub const CalendarWeek: Self = Self(57698);
    pub const Calendar: Self = Self(57699);
    pub const Character: Self = Self(57700);
    pub const MailReplyAll: Self = Self(57701);
    pub const Read: Self = Self(57702);
    pub const Link: Self = Self(57703);
    pub const Account: Self = Self(57704);
    pub const ShowBcc: Self = Self(57705);
    pub const HideBcc: Self = Self(57706);
    pub const Cut: Self = Self(57707);
    pub const Attach: Self = Self(57708);
    pub const Paste: Self = Self(57709);
    pub const Filter: Self = Self(57710);
    pub const Copy: Self = Self(57711);
    pub const Emoji2: Self = Self(57712);
    pub const Important: Self = Self(57713);
    pub const MailReply: Self = Self(57714);
    pub const SlideShow: Self = Self(57715);
    pub const Sort: Self = Self(57716);
    pub const Manage: Self = Self(57720);
    pub const AllApps: Self = Self(57721);
    pub const DisconnectDrive: Self = Self(57722);
    pub const MapDrive: Self = Self(57723);
    pub const NewWindow: Self = Self(57724);
    pub const OpenWith: Self = Self(57725);
    pub const ContactPresence: Self = Self(57729);
    pub const Priority: Self = Self(57730);
    pub const GoToToday: Self = Self(57732);
    pub const Font: Self = Self(57733);
    pub const FontColor: Self = Self(57734);
    pub const Contact2: Self = Self(57735);
    pub const Folder: Self = Self(57736);
    pub const Audio: Self = Self(57737);
    pub const Placeholder: Self = Self(57738);
    pub const View: Self = Self(57739);
    pub const SetLockScreen: Self = Self(57740);
    pub const SetTile: Self = Self(57741);
    pub const ClosedCaption: Self = Self(57744);
    pub const StopSlideShow: Self = Self(57745);
    pub const Permissions: Self = Self(57746);
    pub const Highlight: Self = Self(57747);
    pub const DisableUpdates: Self = Self(57748);
    pub const UnFavorite: Self = Self(57749);
    pub const UnPin: Self = Self(57750);
    pub const OpenLocal: Self = Self(57751);
    pub const Mute: Self = Self(57752);
    pub const Italic: Self = Self(57753);
    pub const Underline: Self = Self(57754);
    pub const Bold: Self = Self(57755);
    pub const MoveToFolder: Self = Self(57756);
    pub const LikeDislike: Self = Self(57757);
    pub const Dislike: Self = Self(57758);
    pub const Like: Self = Self(57759);
    pub const AlignRight: Self = Self(57760);
    pub const AlignCenter: Self = Self(57761);
    pub const AlignLeft: Self = Self(57762);
    pub const Zoom: Self = Self(57763);
    pub const ZoomOut: Self = Self(57764);
    pub const OpenFile: Self = Self(57765);
    pub const OtherUser: Self = Self(57766);
    pub const Admin: Self = Self(57767);
    pub const Street: Self = Self(57795);
    pub const Map: Self = Self(57796);
    pub const ClearSelection: Self = Self(57797);
    pub const FontDecrease: Self = Self(57798);
    pub const FontIncrease: Self = Self(57799);
    pub const FontSize: Self = Self(57800);
    pub const CellPhone: Self = Self(57801);
    pub const ReShare: Self = Self(57802);
    pub const Tag: Self = Self(57803);
    pub const RepeatOne: Self = Self(57804);
    pub const RepeatAll: Self = Self(57805);
    pub const OutlineStar: Self = Self(57806);
    pub const SolidStar: Self = Self(57807);
    pub const Calculator: Self = Self(57808);
    pub const Directions: Self = Self(57809);
    pub const Target: Self = Self(57810);
    pub const Library: Self = Self(57811);
    pub const PhoneBook: Self = Self(57812);
    pub const Memo: Self = Self(57813);
    pub const Microphone: Self = Self(57814);
    pub const PostUpdate: Self = Self(57815);
    pub const BackToWindow: Self = Self(57816);
    pub const FullScreen: Self = Self(57817);
    pub const NewFolder: Self = Self(57818);
    pub const CalendarReply: Self = Self(57819);
    pub const UnSyncFolder: Self = Self(57821);
    pub const ReportHacked: Self = Self(57822);
    pub const SyncFolder: Self = Self(57823);
    pub const BlockContact: Self = Self(57824);
    pub const SwitchApps: Self = Self(57825);
    pub const AddFriend: Self = Self(57826);
    pub const TouchPointer: Self = Self(57827);
    pub const GoToStart: Self = Self(57828);
    pub const ZeroBars: Self = Self(57829);
    pub const OneBar: Self = Self(57830);
    pub const TwoBars: Self = Self(57831);
    pub const ThreeBars: Self = Self(57832);
    pub const FourBars: Self = Self(57833);
    pub const Scan: Self = Self(58004);
    pub const Preview: Self = Self(58005);
    pub const GlobalNavigationButton: Self = Self(59136);
    pub const Share: Self = Self(59181);
    pub const Print: Self = Self(59209);
    pub const XboxOneConsole: Self = Self(59792);
}
impl windows_core::imp::TypeKind for Symbol {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Symbol {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.Symbol;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolIcon(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(SymbolIcon, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(SymbolIcon, IconElement, FrameworkElement, UIElement, DependencyObject);
impl SymbolIcon {
    pub fn CreateInstanceWithSymbol(symbol: Symbol) -> windows_core::Result<Self> {
        Self::ISymbolIconFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstanceWithSymbol)(
                windows_core::Interface::as_raw(this),
                symbol,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ISymbolIconFactory<R, F: FnOnce(&ISymbolIconFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<SymbolIcon, ISymbolIconFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for SymbolIcon {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ISymbolIcon>();
}
unsafe impl windows_core::Interface for SymbolIcon {
    type Vtable = <ISymbolIcon as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISymbolIcon as windows_core::Interface>::IID;
}
impl core::ops::Deref for SymbolIcon {
    type Target = ISymbolIcon;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SymbolIcon {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.SymbolIcon";
}
unsafe impl Send for SymbolIcon {}
unsafe impl Sync for SymbolIcon {}
pub const TIME_NOSECONDS: i32 = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TIME_ZONE_INFORMATION {
    pub Bias: i32,
    pub StandardName: [u16; 32],
    pub StandardDate: SYSTEMTIME,
    pub StandardBias: i32,
    pub DaylightName: [u16; 32],
    pub DaylightDate: SYSTEMTIME,
    pub DaylightBias: i32,
}
impl Default for TIME_ZONE_INFORMATION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const TME_LEAVE: i32 = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub hwndTrack: HWND,
    pub dwHoverTime: u32,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextAlignment(pub i32);
impl TextAlignment {
    pub const Center: Self = Self(0);
    pub const Left: Self = Self(1);
    pub const Start: Self = Self(1);
    pub const Right: Self = Self(2);
    pub const End: Self = Self(2);
    pub const Justify: Self = Self(3);
    pub const DetectFromContent: Self = Self(4);
}
impl windows_core::imp::TypeKind for TextAlignment {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TextAlignment {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.TextAlignment;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBlock(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(TextBlock, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(TextBlock, FrameworkElement, UIElement, DependencyObject);
impl TextBlock {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<TextBlock, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for TextBlock {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ITextBlock>();
}
unsafe impl windows_core::Interface for TextBlock {
    type Vtable = <ITextBlock as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ITextBlock as windows_core::Interface>::IID;
}
impl core::ops::Deref for TextBlock {
    type Target = ITextBlock;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for TextBlock {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TextBlock";
}
unsafe impl Send for TextBlock {}
unsafe impl Sync for TextBlock {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBox(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(TextBox, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(TextBox, Control, FrameworkElement, UIElement, DependencyObject);
impl TextBox {
    pub fn new() -> windows_core::Result<Self> {
        Self::ITextBoxFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn ITextBoxFactory<R, F: FnOnce(&ITextBoxFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<TextBox, ITextBoxFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for TextBox {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ITextBox>();
}
unsafe impl windows_core::Interface for TextBox {
    type Vtable = <ITextBox as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ITextBox as windows_core::Interface>::IID;
}
impl core::ops::Deref for TextBox {
    type Target = ITextBox;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for TextBox {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TextBox";
}
unsafe impl Send for TextBox {}
unsafe impl Sync for TextBox {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(TextChangedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(TextChangedEventArgs, RoutedEventArgs);
impl windows_core::RuntimeType for TextChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ITextChangedEventArgs>();
}
unsafe impl windows_core::Interface for TextChangedEventArgs {
    type Vtable = <ITextChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ITextChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for TextChangedEventArgs {
    type Target = ITextChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for TextChangedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TextChangedEventArgs";
}
unsafe impl Send for TextChangedEventArgs {}
unsafe impl Sync for TextChangedEventArgs {}
windows_core::imp::define_interface!(
    TextChangedEventHandler,
    TextChangedEventHandler_Vtbl,
    0x5d8ddcff_45d8_5e7c_9b8b_c41d2893c6a1
);
impl windows_core::RuntimeType for TextChangedEventHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct TextChangedEventHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
struct TextChangedEventHandlerBox<
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<TextChangedEventArgs>) + 'static,
>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<TextChangedEventArgs>) + 'static>
    TextChangedEventHandlerBox<F>
{
    const VTABLE: TextChangedEventHandler_Vtbl = TextChangedEventHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<TextChangedEventHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<TextChangedEventHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<TextChangedEventHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        e: *mut core::ffi::c_void,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<TextChangedEventHandler, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&e));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextTrimming(pub i32);
impl TextTrimming {
    pub const None: Self = Self(0);
    pub const CharacterEllipsis: Self = Self(1);
    pub const WordEllipsis: Self = Self(2);
    pub const Clip: Self = Self(3);
}
impl windows_core::imp::TypeKind for TextTrimming {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TextTrimming {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.TextTrimming;i4)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextWrapping(pub i32);
impl TextWrapping {
    pub const NoWrap: Self = Self(1);
    pub const Wrap: Self = Self(2);
    pub const WrapWholeWords: Self = Self(3);
}
impl windows_core::imp::TypeKind for TextWrapping {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TextWrapping {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.TextWrapping;i4)");
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thickness {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
impl windows_core::imp::TypeKind for Thickness {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Thickness {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Microsoft.UI.Xaml.Thickness;f8;f8;f8;f8)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TickPlacement(pub i32);
impl TickPlacement {
    pub const None: Self = Self(0);
    pub const TopLeft: Self = Self(1);
    pub const BottomRight: Self = Self(2);
    pub const Outside: Self = Self(3);
    pub const Inline: Self = Self(4);
}
impl windows_core::imp::TypeKind for TickPlacement {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TickPlacement {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Controls.Primitives.TickPlacement;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TitleBar(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(TitleBar, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(TitleBar, Control, FrameworkElement, UIElement, DependencyObject);
impl windows_core::RuntimeType for TitleBar {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, ITitleBar>();
}
unsafe impl windows_core::Interface for TitleBar {
    type Vtable = <ITitleBar as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ITitleBar as windows_core::Interface>::IID;
}
impl core::ops::Deref for TitleBar {
    type Target = ITitleBar;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for TitleBar {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.TitleBar";
}
unsafe impl Send for TitleBar {}
unsafe impl Sync for TitleBar {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TitleBarHeightOption(pub i32);
impl TitleBarHeightOption {
    pub const Standard: Self = Self(0);
    pub const Tall: Self = Self(1);
    pub const Collapsed: Self = Self(2);
}
impl windows_core::imp::TypeKind for TitleBarHeightOption {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TitleBarHeightOption {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Windowing.TitleBarHeightOption;i4)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TitleBarTheme(pub i32);
impl TitleBarTheme {
    pub const Legacy: Self = Self(0);
    pub const UseDefaultAppMode: Self = Self(1);
    pub const Light: Self = Self(2);
    pub const Dark: Self = Self(3);
}
impl windows_core::imp::TypeKind for TitleBarTheme {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TitleBarTheme {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Windowing.TitleBarTheme;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToggleButton(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ToggleButton, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ToggleButton,
    ButtonBase,
    ContentControl,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ToggleButton {
    pub fn new() -> windows_core::Result<Self> {
        Self::IToggleButtonFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IToggleButtonFactory<R, F: FnOnce(&IToggleButtonFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ToggleButton, IToggleButtonFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ToggleButton {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IToggleButton>();
}
unsafe impl windows_core::Interface for ToggleButton {
    type Vtable = <IToggleButton as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IToggleButton as windows_core::Interface>::IID;
}
impl core::ops::Deref for ToggleButton {
    type Target = IToggleButton;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ToggleButton {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.Primitives.ToggleButton";
}
unsafe impl Send for ToggleButton {}
unsafe impl Sync for ToggleButton {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToggleMenuFlyoutItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ToggleMenuFlyoutItem, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(
    ToggleMenuFlyoutItem,
    MenuFlyoutItem,
    MenuFlyoutItemBase,
    Control,
    FrameworkElement,
    UIElement,
    DependencyObject
);
impl ToggleMenuFlyoutItem {
    pub fn new() -> windows_core::Result<Self> {
        Self::IToggleMenuFlyoutItemFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IToggleMenuFlyoutItemFactory<R, F: FnOnce(&IToggleMenuFlyoutItemFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ToggleMenuFlyoutItem, IToggleMenuFlyoutItemFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ToggleMenuFlyoutItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IToggleMenuFlyoutItem>();
}
unsafe impl windows_core::Interface for ToggleMenuFlyoutItem {
    type Vtable = <IToggleMenuFlyoutItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IToggleMenuFlyoutItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for ToggleMenuFlyoutItem {
    type Target = IToggleMenuFlyoutItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ToggleMenuFlyoutItem {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ToggleMenuFlyoutItem";
}
unsafe impl Send for ToggleMenuFlyoutItem {}
unsafe impl Sync for ToggleMenuFlyoutItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToggleSwitch(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ToggleSwitch, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(ToggleSwitch, Control, FrameworkElement, UIElement, DependencyObject);
impl ToggleSwitch {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ToggleSwitch, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ToggleSwitch {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IToggleSwitch>();
}
unsafe impl windows_core::Interface for ToggleSwitch {
    type Vtable = <IToggleSwitch as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IToggleSwitch as windows_core::Interface>::IID;
}
impl core::ops::Deref for ToggleSwitch {
    type Target = IToggleSwitch;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ToggleSwitch {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ToggleSwitch";
}
unsafe impl Send for ToggleSwitch {}
unsafe impl Sync for ToggleSwitch {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolTipService(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(ToolTipService, windows_core::IUnknown, windows_core::IInspectable);
impl ToolTipService {
    pub fn GetToolTip<P0>(element: P0) -> windows_core::Result<windows_core::IInspectable>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IToolTipServiceStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetToolTip)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn SetToolTip<P0, P1>(element: P0, value: P1) -> windows_core::Result<()>
    where
        P0: windows_core::Param<DependencyObject>,
        P1: windows_core::Param<windows_core::IInspectable>,
    {
        Self::IToolTipServiceStatics(|this| unsafe {
            (windows_core::Interface::vtable(this).SetToolTip)(
                windows_core::Interface::as_raw(this),
                element.param().abi(),
                value.param().abi(),
            )
            .ok()
        })
    }
    fn IToolTipServiceStatics<R, F: FnOnce(&IToolTipServiceStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<ToolTipService, IToolTipServiceStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for ToolTipService {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IToolTipService>();
}
unsafe impl windows_core::Interface for ToolTipService {
    type Vtable = <IToolTipService as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IToolTipService as windows_core::Interface>::IID;
}
impl core::ops::Deref for ToolTipService {
    type Target = IToolTipService;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ToolTipService {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.ToolTipService";
}
unsafe impl Send for ToolTipService {}
unsafe impl Sync for ToolTipService {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TypeKind(pub i32);
impl TypeKind {
    pub const Primitive: Self = Self(0);
    pub const Metadata: Self = Self(1);
    pub const Custom: Self = Self(2);
}
impl windows_core::imp::TypeKind for TypeKind {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for TypeKind {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.UI.Xaml.Interop.TypeKind;i4)");
}
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeName {
    pub name: windows_core::HSTRING,
    pub kind: TypeKind,
}
impl windows_core::imp::TypeKind for TypeName {
    type TypeKind = windows_core::imp::CloneType;
}
impl windows_core::RuntimeType for TypeName {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"struct(Windows.UI.Xaml.Interop.TypeName;string;enum(Windows.UI.Xaml.Interop.TypeKind;i4))",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedEventHandler<TSender, TResult>(
    windows_core::IUnknown,
    core::marker::PhantomData<TSender>,
    core::marker::PhantomData<TResult>,
)
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static;
unsafe impl<TSender: windows_core::RuntimeType + 'static, TResult: windows_core::RuntimeType + 'static>
    windows_core::Interface for TypedEventHandler<TSender, TResult>
{
    type Vtable = TypedEventHandler_Vtbl<TSender, TResult>;
    const IID: windows_core::GUID = windows_core::GUID::from_signature(<Self as windows_core::RuntimeType>::SIGNATURE);
}
impl<TSender: windows_core::RuntimeType + 'static, TResult: windows_core::RuntimeType + 'static>
    windows_core::RuntimeType for TypedEventHandler<TSender, TResult>
{
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
        .push_slice(b"pinterface({9de1c534-6ae1-11e0-84e1-18a905bcc53f}")
        .push_slice(b";")
        .push_other(TSender::SIGNATURE)
        .push_slice(b";")
        .push_other(TResult::SIGNATURE)
        .push_slice(b")");
}
impl<TSender: windows_core::RuntimeType + 'static, TResult: windows_core::RuntimeType + 'static>
    TypedEventHandler<TSender, TResult>
{
    pub fn new<F: Fn(windows_core::Ref<TSender>, windows_core::Ref<TResult>) + 'static>(invoke: F) -> Self {
        let com = windows_core::imp::DelegateBox::<Self, F>::new(
            &TypedEventHandlerBox::<TSender, TResult, F>::VTABLE,
            invoke,
        );
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}
#[repr(C)]
pub struct TypedEventHandler_Vtbl<TSender, TResult>
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static,
{
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: windows_core::imp::AbiType<TSender>,
        args: windows_core::imp::AbiType<TResult>,
    ) -> windows_core::HRESULT,
    TSender: core::marker::PhantomData<TSender>,
    TResult: core::marker::PhantomData<TResult>,
}
struct TypedEventHandlerBox<TSender, TResult, F: Fn(windows_core::Ref<TSender>, windows_core::Ref<TResult>) + 'static>(
    core::marker::PhantomData<(TSender, TResult, fn() -> F)>,
)
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static;
impl<
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static,
    F: Fn(windows_core::Ref<TSender>, windows_core::Ref<TResult>) + 'static,
> TypedEventHandlerBox<TSender, TResult, F>
{
    const VTABLE: TypedEventHandler_Vtbl<TSender, TResult> = TypedEventHandler_Vtbl::<TSender, TResult> {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<TypedEventHandler<TSender, TResult>, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<TypedEventHandler<TSender, TResult>, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<TypedEventHandler<TSender, TResult>, F>::Release,
        },
        Invoke: Self::Invoke,
        TSender: core::marker::PhantomData::<TSender>,
        TResult: core::marker::PhantomData::<TResult>,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: windows_core::imp::AbiType<TSender>,
        args: windows_core::imp::AbiType<TResult>,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<TypedEventHandler<TSender, TResult>, F>);
            (this.invoke)(core::mem::transmute_copy(&sender), core::mem::transmute_copy(&args));
            windows_core::HRESULT(0)
        }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UIElement(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(UIElement, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(UIElement, DependencyObject);
impl windows_core::RuntimeType for UIElement {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IUIElement>();
}
unsafe impl windows_core::Interface for UIElement {
    type Vtable = <IUIElement as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IUIElement as windows_core::Interface>::IID;
}
impl core::ops::Deref for UIElement {
    type Target = IUIElement;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for UIElement {
    const NAME: &'static str = "Microsoft.UI.Xaml.UIElement";
}
unsafe impl Send for UIElement {}
unsafe impl Sync for UIElement {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UIElementCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    UIElementCollection,
    windows_core::IUnknown,
    windows_core::IInspectable,
    windows_collections::IVector<UIElement>
);
impl windows_core::RuntimeType for UIElementCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, windows_collections::IVector<UIElement>>();
}
unsafe impl windows_core::Interface for UIElementCollection {
    type Vtable = <windows_collections::IVector<UIElement> as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <windows_collections::IVector<UIElement> as windows_core::Interface>::IID;
}
impl core::ops::Deref for UIElementCollection {
    type Target = windows_collections::IVector<UIElement>;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for UIElementCollection {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.UIElementCollection";
}
unsafe impl Send for UIElementCollection {}
unsafe impl Sync for UIElementCollection {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UISettings(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(UISettings, windows_core::IUnknown, windows_core::IInspectable);
impl UISettings {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<UISettings, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for UISettings {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IUISettings>();
}
unsafe impl windows_core::Interface for UISettings {
    type Vtable = <IUISettings as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IUISettings as windows_core::Interface>::IID;
}
impl core::ops::Deref for UISettings {
    type Target = IUISettings;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for UISettings {
    const NAME: &'static str = "Windows.UI.ViewManagement.UISettings";
}
unsafe impl Send for UISettings {}
unsafe impl Sync for UISettings {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Uri(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Uri, windows_core::IUnknown, windows_core::IInspectable);
impl Uri {
    pub fn CreateUri(uri: &str) -> windows_core::Result<Self> {
        Self::IUriRuntimeClassFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateUri)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(uri)),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IUriRuntimeClassFactory<R, F: FnOnce(&IUriRuntimeClassFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Uri, IUriRuntimeClassFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Uri {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IUriRuntimeClass>();
}
unsafe impl windows_core::Interface for Uri {
    type Vtable = <IUriRuntimeClass as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IUriRuntimeClass as windows_core::Interface>::IID;
}
impl core::ops::Deref for Uri {
    type Target = IUriRuntimeClass;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Uri {
    const NAME: &'static str = "Windows.Foundation.Uri";
}
unsafe impl Send for Uri {}
unsafe impl Sync for Uri {}
pub const VK_CONTROL: i32 = 17;
pub const VK_LWIN: i32 = 91;
pub const VK_MENU: i32 = 18;
pub const VK_RWIN: i32 = 92;
pub const VK_SHIFT: i32 = 16;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VerticalAlignment(pub i32);
impl VerticalAlignment {
    pub const Top: Self = Self(0);
    pub const Center: Self = Self(1);
    pub const Bottom: Self = Self(2);
    pub const Stretch: Self = Self(3);
}
impl windows_core::imp::TypeKind for VerticalAlignment {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for VerticalAlignment {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.VerticalAlignment;i4)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VirtualKey(pub i32);
impl VirtualKey {
    pub const None: Self = Self(0);
    pub const LeftButton: Self = Self(1);
    pub const RightButton: Self = Self(2);
    pub const Cancel: Self = Self(3);
    pub const MiddleButton: Self = Self(4);
    pub const XButton1: Self = Self(5);
    pub const XButton2: Self = Self(6);
    pub const Back: Self = Self(8);
    pub const Tab: Self = Self(9);
    pub const Clear: Self = Self(12);
    pub const Enter: Self = Self(13);
    pub const Shift: Self = Self(16);
    pub const Control: Self = Self(17);
    pub const Menu: Self = Self(18);
    pub const Pause: Self = Self(19);
    pub const CapitalLock: Self = Self(20);
    pub const Kana: Self = Self(21);
    pub const Hangul: Self = Self(21);
    pub const ImeOn: Self = Self(22);
    pub const Junja: Self = Self(23);
    pub const Final: Self = Self(24);
    pub const Hanja: Self = Self(25);
    pub const Kanji: Self = Self(25);
    pub const ImeOff: Self = Self(26);
    pub const Escape: Self = Self(27);
    pub const Convert: Self = Self(28);
    pub const NonConvert: Self = Self(29);
    pub const Accept: Self = Self(30);
    pub const ModeChange: Self = Self(31);
    pub const Space: Self = Self(32);
    pub const PageUp: Self = Self(33);
    pub const PageDown: Self = Self(34);
    pub const End: Self = Self(35);
    pub const Home: Self = Self(36);
    pub const Left: Self = Self(37);
    pub const Up: Self = Self(38);
    pub const Right: Self = Self(39);
    pub const Down: Self = Self(40);
    pub const Select: Self = Self(41);
    pub const Print: Self = Self(42);
    pub const Execute: Self = Self(43);
    pub const Snapshot: Self = Self(44);
    pub const Insert: Self = Self(45);
    pub const Delete: Self = Self(46);
    pub const Help: Self = Self(47);
    pub const Number0: Self = Self(48);
    pub const Number1: Self = Self(49);
    pub const Number2: Self = Self(50);
    pub const Number3: Self = Self(51);
    pub const Number4: Self = Self(52);
    pub const Number5: Self = Self(53);
    pub const Number6: Self = Self(54);
    pub const Number7: Self = Self(55);
    pub const Number8: Self = Self(56);
    pub const Number9: Self = Self(57);
    pub const A: Self = Self(65);
    pub const B: Self = Self(66);
    pub const C: Self = Self(67);
    pub const D: Self = Self(68);
    pub const E: Self = Self(69);
    pub const F: Self = Self(70);
    pub const G: Self = Self(71);
    pub const H: Self = Self(72);
    pub const I: Self = Self(73);
    pub const J: Self = Self(74);
    pub const K: Self = Self(75);
    pub const L: Self = Self(76);
    pub const M: Self = Self(77);
    pub const N: Self = Self(78);
    pub const O: Self = Self(79);
    pub const P: Self = Self(80);
    pub const Q: Self = Self(81);
    pub const R: Self = Self(82);
    pub const S: Self = Self(83);
    pub const T: Self = Self(84);
    pub const U: Self = Self(85);
    pub const V: Self = Self(86);
    pub const W: Self = Self(87);
    pub const X: Self = Self(88);
    pub const Y: Self = Self(89);
    pub const Z: Self = Self(90);
    pub const LeftWindows: Self = Self(91);
    pub const RightWindows: Self = Self(92);
    pub const Application: Self = Self(93);
    pub const Sleep: Self = Self(95);
    pub const NumberPad0: Self = Self(96);
    pub const NumberPad1: Self = Self(97);
    pub const NumberPad2: Self = Self(98);
    pub const NumberPad3: Self = Self(99);
    pub const NumberPad4: Self = Self(100);
    pub const NumberPad5: Self = Self(101);
    pub const NumberPad6: Self = Self(102);
    pub const NumberPad7: Self = Self(103);
    pub const NumberPad8: Self = Self(104);
    pub const NumberPad9: Self = Self(105);
    pub const Multiply: Self = Self(106);
    pub const Add: Self = Self(107);
    pub const Separator: Self = Self(108);
    pub const Subtract: Self = Self(109);
    pub const Decimal: Self = Self(110);
    pub const Divide: Self = Self(111);
    pub const F1: Self = Self(112);
    pub const F2: Self = Self(113);
    pub const F3: Self = Self(114);
    pub const F4: Self = Self(115);
    pub const F5: Self = Self(116);
    pub const F6: Self = Self(117);
    pub const F7: Self = Self(118);
    pub const F8: Self = Self(119);
    pub const F9: Self = Self(120);
    pub const F10: Self = Self(121);
    pub const F11: Self = Self(122);
    pub const F12: Self = Self(123);
    pub const F13: Self = Self(124);
    pub const F14: Self = Self(125);
    pub const F15: Self = Self(126);
    pub const F16: Self = Self(127);
    pub const F17: Self = Self(128);
    pub const F18: Self = Self(129);
    pub const F19: Self = Self(130);
    pub const F20: Self = Self(131);
    pub const F21: Self = Self(132);
    pub const F22: Self = Self(133);
    pub const F23: Self = Self(134);
    pub const F24: Self = Self(135);
    pub const NavigationView: Self = Self(136);
    pub const NavigationMenu: Self = Self(137);
    pub const NavigationUp: Self = Self(138);
    pub const NavigationDown: Self = Self(139);
    pub const NavigationLeft: Self = Self(140);
    pub const NavigationRight: Self = Self(141);
    pub const NavigationAccept: Self = Self(142);
    pub const NavigationCancel: Self = Self(143);
    pub const NumberKeyLock: Self = Self(144);
    pub const Scroll: Self = Self(145);
    pub const LeftShift: Self = Self(160);
    pub const RightShift: Self = Self(161);
    pub const LeftControl: Self = Self(162);
    pub const RightControl: Self = Self(163);
    pub const LeftMenu: Self = Self(164);
    pub const RightMenu: Self = Self(165);
    pub const GoBack: Self = Self(166);
    pub const GoForward: Self = Self(167);
    pub const Refresh: Self = Self(168);
    pub const Stop: Self = Self(169);
    pub const Search: Self = Self(170);
    pub const Favorites: Self = Self(171);
    pub const GoHome: Self = Self(172);
    pub const GamepadA: Self = Self(195);
    pub const GamepadB: Self = Self(196);
    pub const GamepadX: Self = Self(197);
    pub const GamepadY: Self = Self(198);
    pub const GamepadRightShoulder: Self = Self(199);
    pub const GamepadLeftShoulder: Self = Self(200);
    pub const GamepadLeftTrigger: Self = Self(201);
    pub const GamepadRightTrigger: Self = Self(202);
    pub const GamepadDPadUp: Self = Self(203);
    pub const GamepadDPadDown: Self = Self(204);
    pub const GamepadDPadLeft: Self = Self(205);
    pub const GamepadDPadRight: Self = Self(206);
    pub const GamepadMenu: Self = Self(207);
    pub const GamepadView: Self = Self(208);
    pub const GamepadLeftThumbstickButton: Self = Self(209);
    pub const GamepadRightThumbstickButton: Self = Self(210);
    pub const GamepadLeftThumbstickUp: Self = Self(211);
    pub const GamepadLeftThumbstickDown: Self = Self(212);
    pub const GamepadLeftThumbstickRight: Self = Self(213);
    pub const GamepadLeftThumbstickLeft: Self = Self(214);
    pub const GamepadRightThumbstickUp: Self = Self(215);
    pub const GamepadRightThumbstickDown: Self = Self(216);
    pub const GamepadRightThumbstickRight: Self = Self(217);
    pub const GamepadRightThumbstickLeft: Self = Self(218);
}
impl windows_core::imp::TypeKind for VirtualKey {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for VirtualKey {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.System.VirtualKey;i4)");
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VirtualKeyModifiers(pub u32);
impl VirtualKeyModifiers {
    pub const None: Self = Self(0);
    pub const Control: Self = Self(1);
    pub const Menu: Self = Self(2);
    pub const Shift: Self = Self(4);
    pub const Windows: Self = Self(8);
}
impl windows_core::imp::TypeKind for VirtualKeyModifiers {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for VirtualKeyModifiers {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.System.VirtualKeyModifiers;u4)");
}
impl VirtualKeyModifiers {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for VirtualKeyModifiers {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for VirtualKeyModifiers {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for VirtualKeyModifiers {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for VirtualKeyModifiers {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for VirtualKeyModifiers {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Visibility(pub i32);
impl Visibility {
    pub const Visible: Self = Self(0);
    pub const Collapsed: Self = Self(1);
}
impl windows_core::imp::TypeKind for Visibility {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Visibility {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.Visibility;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualTreeHelper(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(VisualTreeHelper, windows_core::IUnknown, windows_core::IInspectable);
impl VisualTreeHelper {
    pub fn GetChild<P0>(reference: P0, childindex: i32) -> windows_core::Result<DependencyObject>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IVisualTreeHelperStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetChild)(
                windows_core::Interface::as_raw(this),
                reference.param().abi(),
                childindex,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn GetChildrenCount<P0>(reference: P0) -> windows_core::Result<i32>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IVisualTreeHelperStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetChildrenCount)(
                windows_core::Interface::as_raw(this),
                reference.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        })
    }
    pub fn GetParent<P0>(reference: P0) -> windows_core::Result<DependencyObject>
    where
        P0: windows_core::Param<DependencyObject>,
    {
        Self::IVisualTreeHelperStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetParent)(
                windows_core::Interface::as_raw(this),
                reference.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    pub fn GetOpenPopupsForXamlRoot<P0>(xamlroot: P0) -> windows_core::Result<windows_collections::IVectorView<Popup>>
    where
        P0: windows_core::Param<XamlRoot>,
    {
        Self::IVisualTreeHelperStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetOpenPopupsForXamlRoot)(
                windows_core::Interface::as_raw(this),
                xamlroot.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IVisualTreeHelperStatics<R, F: FnOnce(&IVisualTreeHelperStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<VisualTreeHelper, IVisualTreeHelperStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for VisualTreeHelper {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVisualTreeHelper>();
}
unsafe impl windows_core::Interface for VisualTreeHelper {
    type Vtable = <IVisualTreeHelper as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVisualTreeHelper as windows_core::Interface>::IID;
}
impl core::ops::Deref for VisualTreeHelper {
    type Target = IVisualTreeHelper;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for VisualTreeHelper {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.VisualTreeHelper";
}
unsafe impl Send for VisualTreeHelper {}
unsafe impl Sync for VisualTreeHelper {}
pub const WH_KEYBOARD_LL: i32 = 13;
pub const WM_CLOSE: i32 = 16;
pub const WM_ENDSESSION: i32 = 22;
pub const WM_GETICON: i32 = 127;
pub const WM_INPUT: i32 = 255;
pub const WM_LBUTTONDOWN: i32 = 513;
pub const WM_LBUTTONUP: i32 = 514;
pub const WM_MBUTTONDOWN: i32 = 519;
pub const WM_MBUTTONUP: i32 = 520;
pub const WM_MOUSEHWHEEL: i32 = 526;
pub const WM_MOUSELEAVE: i32 = 675;
pub const WM_MOUSEMOVE: i32 = 512;
pub const WM_MOUSEWHEEL: i32 = 522;
pub const WM_NCDESTROY: i32 = 130;
pub const WM_NCHITTEST: i32 = 132;
pub const WM_NULL: i32 = 0;
pub const WM_QUERYENDSESSION: i32 = 17;
pub const WM_RBUTTONDOWN: i32 = 516;
pub const WM_RBUTTONUP: i32 = 517;
pub const WM_SETCURSOR: i32 = 32;
pub const WM_XBUTTONDOWN: i32 = 523;
pub const WM_XBUTTONUP: i32 = 524;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: windows_core::PCWSTR,
    pub lpszClassName: windows_core::PCWSTR,
    pub hIconSm: HICON,
}
pub type WNDENUMPROC = Option<unsafe extern "system" fn(param0: HWND, param1: LPARAM) -> windows_core::BOOL>;
pub type WNDPROC =
    Option<unsafe extern "system" fn(param0: HWND, param1: u32, param2: WPARAM, param3: LPARAM) -> LRESULT>;
pub type WPARAM = usize;
pub const WS_CHILD: i32 = 1073741824;
pub const WS_CLIPSIBLINGS: i32 = 67108864;
pub const WS_EX_LAYERED: i32 = 524288;
pub const WS_EX_TRANSPARENT: i32 = 32;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Window(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Window, windows_core::IUnknown, windows_core::IInspectable);
impl Window {
    pub fn new() -> windows_core::Result<Self> {
        Self::IWindowFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstance)(
                windows_core::Interface::as_raw(this),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IWindowFactory<R, F: FnOnce(&IWindowFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<Window, IWindowFactory> = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Window {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IWindow>();
}
unsafe impl windows_core::Interface for Window {
    type Vtable = <IWindow as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IWindow as windows_core::Interface>::IID;
}
impl core::ops::Deref for Window {
    type Target = IWindow;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Window {
    const NAME: &'static str = "Microsoft.UI.Xaml.Window";
}
unsafe impl Send for Window {}
unsafe impl Sync for Window {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowActivatedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(WindowActivatedEventArgs, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for WindowActivatedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IWindowActivatedEventArgs>();
}
unsafe impl windows_core::Interface for WindowActivatedEventArgs {
    type Vtable = <IWindowActivatedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IWindowActivatedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for WindowActivatedEventArgs {
    type Target = IWindowActivatedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for WindowActivatedEventArgs {
    const NAME: &'static str = "Microsoft.UI.Xaml.WindowActivatedEventArgs";
}
unsafe impl Send for WindowActivatedEventArgs {}
unsafe impl Sync for WindowActivatedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowActivationState(pub i32);
impl WindowActivationState {
    pub const CodeActivated: Self = Self(0);
    pub const Deactivated: Self = Self(1);
    pub const PointerActivated: Self = Self(2);
}
impl windows_core::imp::TypeKind for WindowActivationState {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for WindowActivationState {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Microsoft.UI.Xaml.WindowActivationState;i4)");
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowId {
    pub value: u64,
}
impl windows_core::imp::TypeKind for WindowId {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for WindowId {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Microsoft.UI.WindowId;u8)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsXamlManager(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(WindowsXamlManager, windows_core::IUnknown, windows_core::IInspectable);
impl WindowsXamlManager {
    pub fn InitializeForCurrentThread() -> windows_core::Result<Self> {
        Self::IWindowsXamlManagerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).InitializeForCurrentThread)(
                windows_core::Interface::as_raw(this),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IWindowsXamlManagerStatics<R, F: FnOnce(&IWindowsXamlManagerStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<WindowsXamlManager, IWindowsXamlManagerStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for WindowsXamlManager {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IWindowsXamlManager>();
}
unsafe impl windows_core::Interface for WindowsXamlManager {
    type Vtable = <IWindowsXamlManager as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IWindowsXamlManager as windows_core::Interface>::IID;
}
impl core::ops::Deref for WindowsXamlManager {
    type Target = IWindowsXamlManager;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for WindowsXamlManager {
    const NAME: &'static str = "Microsoft.UI.Xaml.Hosting.WindowsXamlManager";
}
unsafe impl Send for WindowsXamlManager {}
unsafe impl Sync for WindowsXamlManager {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteableBitmap(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(WriteableBitmap, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(WriteableBitmap, BitmapSource, ImageSource, DependencyObject);
impl WriteableBitmap {
    pub fn CreateInstanceWithDimensions(pixelwidth: i32, pixelheight: i32) -> windows_core::Result<Self> {
        Self::IWriteableBitmapFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateInstanceWithDimensions)(
                windows_core::Interface::as_raw(this),
                pixelwidth,
                pixelheight,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IWriteableBitmapFactory<R, F: FnOnce(&IWriteableBitmapFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<WriteableBitmap, IWriteableBitmapFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for WriteableBitmap {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IWriteableBitmap>();
}
unsafe impl windows_core::Interface for WriteableBitmap {
    type Vtable = <IWriteableBitmap as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IWriteableBitmap as windows_core::Interface>::IID;
}
impl core::ops::Deref for WriteableBitmap {
    type Target = IWriteableBitmap;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for WriteableBitmap {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.Imaging.WriteableBitmap";
}
unsafe impl Send for WriteableBitmap {}
unsafe impl Sync for WriteableBitmap {}
pub const XBUTTON1: i32 = 1;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XamlControlsResources(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(XamlControlsResources, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(XamlControlsResources, ResourceDictionary, DependencyObject);
impl XamlControlsResources {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<XamlControlsResources, windows_core::imp::IGenericFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for XamlControlsResources {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IXamlControlsResources>();
}
unsafe impl windows_core::Interface for XamlControlsResources {
    type Vtable = <IXamlControlsResources as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IXamlControlsResources as windows_core::Interface>::IID;
}
impl core::ops::Deref for XamlControlsResources {
    type Target = IXamlControlsResources;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for XamlControlsResources {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.XamlControlsResources";
}
unsafe impl Send for XamlControlsResources {}
unsafe impl Sync for XamlControlsResources {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XamlControlsXamlMetaDataProvider(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    XamlControlsXamlMetaDataProvider,
    windows_core::IUnknown,
    windows_core::IInspectable,
    IXamlMetadataProvider
);
impl XamlControlsXamlMetaDataProvider {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<R, F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            XamlControlsXamlMetaDataProvider,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for XamlControlsXamlMetaDataProvider {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IXamlMetadataProvider>();
}
unsafe impl windows_core::Interface for XamlControlsXamlMetaDataProvider {
    type Vtable = <IXamlMetadataProvider as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IXamlMetadataProvider as windows_core::Interface>::IID;
}
impl core::ops::Deref for XamlControlsXamlMetaDataProvider {
    type Target = IXamlMetadataProvider;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for XamlControlsXamlMetaDataProvider {
    const NAME: &'static str = "Microsoft.UI.Xaml.XamlTypeInfo.XamlControlsXamlMetaDataProvider";
}
unsafe impl Send for XamlControlsXamlMetaDataProvider {}
unsafe impl Sync for XamlControlsXamlMetaDataProvider {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XamlReader(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(XamlReader, windows_core::IUnknown, windows_core::IInspectable);
impl XamlReader {
    pub fn Load(xaml: &str) -> windows_core::Result<windows_core::IInspectable> {
        Self::IXamlReaderStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Load)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(xaml)),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        })
    }
    fn IXamlReaderStatics<R, F: FnOnce(&IXamlReaderStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<XamlReader, IXamlReaderStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for XamlReader {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IXamlReader>();
}
unsafe impl windows_core::Interface for XamlReader {
    type Vtable = <IXamlReader as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IXamlReader as windows_core::Interface>::IID;
}
impl core::ops::Deref for XamlReader {
    type Target = IXamlReader;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for XamlReader {
    const NAME: &'static str = "Microsoft.UI.Xaml.Markup.XamlReader";
}
unsafe impl Send for XamlReader {}
unsafe impl Sync for XamlReader {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XamlRoot(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(XamlRoot, windows_core::IUnknown, windows_core::IInspectable);
impl windows_core::RuntimeType for XamlRoot {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<Self, IXamlRoot>();
}
unsafe impl windows_core::Interface for XamlRoot {
    type Vtable = <IXamlRoot as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IXamlRoot as windows_core::Interface>::IID;
}
impl core::ops::Deref for XamlRoot {
    type Target = IXamlRoot;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for XamlRoot {
    const NAME: &'static str = "Microsoft.UI.Xaml.XamlRoot";
}
unsafe impl Send for XamlRoot {}
unsafe impl Sync for XamlRoot {}
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XmlnsDefinition {
    pub xml_namespace: windows_core::HSTRING,
    pub namespace: windows_core::HSTRING,
}
impl windows_core::imp::TypeKind for XmlnsDefinition {
    type TypeKind = windows_core::imp::CloneType;
}
impl windows_core::RuntimeType for XmlnsDefinition {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Microsoft.UI.Xaml.Markup.XmlnsDefinition;string;string)");
}
pub type byte = u8;
