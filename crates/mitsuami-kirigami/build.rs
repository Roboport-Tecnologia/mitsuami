//! Compiles the C++ layer (`cpp/`) against the system's Qt 6, when the `qt`
//! feature is on and the target is Linux. Qt is found with pkg-config, and
//! moc runs on the one header that declares QObjects.

use std::path::PathBuf;
use std::process::Command;

const MODULES: [&str; 5] = ["Qt6Widgets", "Qt6Quick", "Qt6Qml", "Qt6QuickControls2", "Qt6Gui"];

fn main() {
    println!("cargo:rerun-if-changed=cpp/shim.h");
    println!("cargo:rerun-if-changed=cpp/shim.cpp");
    println!("cargo:rerun-if-env-changed=MITSUAMI_SKIP_QT");
    let linux = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "linux");
    // `MITSUAMI_SKIP_QT=1`: no C++ layer, for type-checking (`cargo check`,
    // `clippy`) where Qt isn't installed, e.g. from macOS. Doesn't link.
    if !linux || std::env::var_os("CARGO_FEATURE_QT").is_none() || std::env::var_os("MITSUAMI_SKIP_QT").is_some() {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut includes = Vec::new();
    for module in MODULES {
        match pkg_config::Config::new().atleast_version("6.5").probe(module) {
            Ok(lib) => {
                // Qt's platform headers (`qpa/`, for a window's
                // `wl_surface`) are under the module's version.
                if module == "Qt6Gui" {
                    for path in lib.include_paths.iter().filter(|p| p.ends_with("QtGui")) {
                        includes.push(path.join(&lib.version));
                        includes.push(path.join(&lib.version).join("QtGui"));
                    }
                }
                includes.extend(lib.include_paths)
            }
            Err(e) => panic!(
                "mitsuami-kirigami: the Qt 6 development files are missing ({module}): {e}\n\
                 Install Qt 6.5 or newer with Qt Quick Controls, plus Kirigami and qqc2-desktop-style."
            ),
        }
    }
    let libexec = pkg_config::get_variable("Qt6Core", "libexecdir").expect("Qt6Core has a libexecdir");
    let moc_out = out.join("moc_shim.cpp");
    let status = Command::new(PathBuf::from(libexec).join("moc"))
        .arg("cpp/shim.h")
        .arg("-o")
        .arg(&moc_out)
        .status()
        .expect("mitsuami-kirigami: cannot run Qt's moc");
    assert!(status.success(), "mitsuami-kirigami: moc failed");

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17").file("cpp/shim.cpp").file(&moc_out).include("cpp").flag("-fPIC");
    for include in includes {
        build.include(include);
    }
    build.compile("mitsuami_kirigami_shim");
}
