//! The Windows App Runtime and XAML on the current thread, without
//! `Application::Start`: we compose the `Application` and initialize XAML
//! for the thread. Apps run XAML's event loop (`app.rs`); tests pump its
//! messages themselves, so they decide when to tick.

use std::cell::Cell;
use std::time::Duration;

use windows_core::{Array, HSTRING, Interface, PCWSTR, PWSTR, Ref, Result, implement_decl, w};

use crate::bindings::*;

/// The framework package family of Windows App SDK 2.x, and the minimum
/// version we were built against (2.4.0.0).
const FRAMEWORK_FAMILY: PCWSTR = w!("Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe");
const MIN_VERSION: u64 = 0x0002_0004_0000_0000;

thread_local! {
    static READY: Cell<bool> = const { Cell::new(false) };
    /// Set while the backend pumps messages from inside a `Ui` call (waiting
    /// for a window to come alive); scheduled ticks must not run then.
    static NESTED: Cell<u32> = const { Cell::new(0) };
    /// A tick came while nested, and waits for the pump to end.
    static TICK_AFTER: Cell<bool> = const { Cell::new(false) };
}

/// Makes the Windows App Runtime and XAML available on this thread. Idempotent.
///
/// # Panics
/// If the Windows App Runtime 2.4 (or later 2.x) is not installed, or XAML
/// was already started differently on this thread.
pub fn init() {
    if READY.get() {
        return;
    }
    if let Err(error) = try_init() {
        panic!(
            "mitsuami-winui: could not start WinUI ({error}). Mitsuami needs the Windows App Runtime 2.4 or later: \
             https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads"
        );
    }
    READY.set(true);
}

fn try_init() -> Result<()> {
    // A package declares the runtime as a dependency in its manifest, and
    // Windows adds it to the package graph before we start.
    if !crate::backend::packaged() {
        bootstrap()?;
    }
    unsafe {
        // Logical units everywhere; XAML scales per monitor.
        _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if hr == RPC_E_CHANGED_MODE {
            return Err(windows_core::Error::new(hr, "WinUI needs a single-threaded apartment"));
        }
        hr.ok()?;
    }
    // Order matters: the dispatcher queue, then the Application (XAML reads
    // it when it starts), then XAML itself. They live as long as the thread,
    // and are never released: XAML fails fast when torn down late.
    std::mem::forget(DispatcherQueueController::CreateOnCurrentThread()?);
    let application = Application::compose(XamlApp)?;
    std::mem::forget(WindowsXamlManager::InitializeForCurrentThread()?);
    // Control templates (Button, CheckBox, …) live in XamlControlsResources.
    let resources: ResourceDictionary = XamlControlsResources::new()?.cast()?;
    application.cast::<IApplication>()?.Resources()?.MergedDictionaries()?.Append(&resources)?;
    std::mem::forget(application);
    Ok(())
}

/// Framework-dependent bootstrap: adds the installed runtime package to our
/// process's package graph, as `windows-reactor` does.
fn bootstrap() -> Result<()> {
    let (try_create, add) = dynamic_dependencies()?;
    let arch = match std::env::consts::ARCH {
        "x86_64" => PackageDependencyProcessorArchitectures_X64,
        "aarch64" => PackageDependencyProcessorArchitectures_Arm64,
        "x86" => PackageDependencyProcessorArchitectures_X86,
        _ => PackageDependencyProcessorArchitectures_None,
    };
    let mut id = PWSTR::null();
    unsafe {
        try_create(
            std::ptr::null_mut(),
            FRAMEWORK_FAMILY,
            PACKAGE_VERSION { Anonymous: PACKAGE_VERSION_0 { Version: MIN_VERSION } },
            arch | PackageDependencyProcessorArchitectures_Neutral,
            0, // lifetime: the process
            PCWSTR::null(),
            0,
            &mut id,
        )
        .ok()?;
        let mut context = std::ptr::null_mut();
        let mut full_name = PWSTR::null();
        let added = add(PCWSTR(id.0), 0, 0, &mut context, &mut full_name);
        _ = HeapFree(GetProcessHeap(), 0, id.0.cast());
        _ = HeapFree(GetProcessHeap(), 0, full_name.0.cast());
        added.ok()
    }
}

type TryCreatePackageDependency = unsafe extern "system" fn(
    PSID,
    PCWSTR,
    PACKAGE_VERSION,
    PackageDependencyProcessorArchitectures,
    PackageDependencyLifetimeKind,
    PCWSTR,
    CreatePackageDependencyOptions,
    *mut PWSTR,
) -> windows_core::HRESULT;
type AddPackageDependency = unsafe extern "system" fn(
    PCWSTR,
    i32,
    AddPackageDependencyOptions,
    *mut PACKAGEDEPENDENCY_CONTEXT,
    *mut PWSTR,
) -> windows_core::HRESULT;

/// Windows' dynamic dependency API, looked up rather than linked: it came
/// with Windows 11, and a link would keep the executable from loading on
/// Windows 10 even packaged, where it isn't needed.
fn dynamic_dependencies() -> Result<(TryCreatePackageDependency, AddPackageDependency)> {
    let missing = || {
        windows_core::Error::new(
            windows_core::HRESULT(0x8007_007Fu32 as i32), // ERROR_PROC_NOT_FOUND
            "an unpackaged app needs Windows 11; on Windows 10 install it as an MSIX package",
        )
    };
    unsafe {
        let module = GetModuleHandleW(w!("kernelbase.dll"));
        if module.is_null() {
            return Err(missing());
        }
        let try_create = GetProcAddress(module, windows_core::s!("TryCreatePackageDependency")).ok_or_else(missing)?;
        let add = GetProcAddress(module, windows_core::s!("AddPackageDependency")).ok_or_else(missing)?;
        Ok((
            std::mem::transmute::<unsafe extern "system" fn() -> isize, TryCreatePackageDependency>(try_create),
            std::mem::transmute::<unsafe extern "system" fn() -> isize, AddPackageDependency>(add),
        ))
    }
}

/// Dispatches every queued message (input, XAML, dispatcher queue work).
pub(crate) fn pump() {
    let mut msg = MSG::default();
    unsafe {
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE as u32).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Pumps from inside a `Ui` call. Scheduled ticks are held back meanwhile.
pub(crate) fn pump_nested() {
    NESTED.set(NESTED.get() + 1);
    pump();
    NESTED.set(NESTED.get() - 1);
    if NESTED.get() == 0 && TICK_AFTER.replace(false) {
        crate::app::schedule_tick();
    }
}

/// Holds a scheduled tick back until the nested pump ends.
pub(crate) fn tick_after_nested() {
    TICK_AFTER.set(true);
}

pub(crate) fn nested() -> bool {
    NESTED.get() > 0
}

/// Sleeps until a message arrives or `timeout` passes.
pub(crate) fn wait(timeout: Option<Duration>) {
    let ms = timeout.map_or(INFINITE, |t| t.as_millis().min(u128::from(INFINITE - 1)) as u32);
    unsafe {
        MsgWaitForMultipleObjectsEx(0, std::ptr::null(), ms, QS_ALLINPUT as u32, MWMO_INPUTAVAILABLE as u32);
    }
}

// ------------------------------------------------------------ application

/// The composed `Application`: no launch logic (we never call `Start`), and
/// the XAML type information of the stock controls.
pub struct XamlApp;

implement_decl! {
    impl XamlApp as pub XamlApp_Impl: [IApplicationOverrides, IXamlMetadataProvider]
}

thread_local! {
    static PROVIDER: XamlControlsXamlMetaDataProvider =
        XamlControlsXamlMetaDataProvider::new().expect("XAML controls metadata");
}

impl IApplicationOverrides_Impl for XamlApp_Impl {
    fn OnLaunched(&self, _args: Ref<LaunchActivatedEventArgs>) -> Result<()> {
        Ok(())
    }
}

impl IXamlMetadataProvider_Impl for XamlApp_Impl {
    fn GetXamlType(&self, r#type: &TypeName) -> Result<IXamlType> {
        PROVIDER.with(|p| p.GetXamlType(r#type))
    }

    fn GetXamlTypeByFullName(&self, full_name: &HSTRING) -> Result<IXamlType> {
        PROVIDER.with(|p| p.GetXamlTypeByFullName(&full_name.to_string_lossy()))
    }

    fn GetXmlnsDefinitions(&self) -> Result<Array<XmlnsDefinition>> {
        PROVIDER.with(|p| p.GetXmlnsDefinitions())
    }
}
