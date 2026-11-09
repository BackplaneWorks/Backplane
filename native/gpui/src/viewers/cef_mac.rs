//! macOS specifics of the embedded Chromium (viewers/cef.rs): the engine is `Chromium Embedded Framework.framework`, loaded at run time; its helper
//! processes run from `Backplane Helper.app` beside the client (and the `(GPU)`, `(Renderer)`, `(Plugin)`, `(Alerts)` variants Chromium asks for when the
//! client itself runs from a bundle), whose program is `backplane-cef-helper` (src/bin; scripts/cef-mac-helpers.sh makes the bundles).
use std::{ffi::{c_char, c_void, CString}, path::{Path, PathBuf}};

pub const FRAMEWORK: &str = "Chromium Embedded Framework.framework";
pub const LIB: &str = "Chromium Embedded Framework";
pub const HELPER: &str = "Backplane Helper";

/// Loads the framework at `fw` (once per process; CEF ignores a second load).
pub fn load(fw: &Path) -> bool {
    let Ok(p) = CString::new(fw.join(LIB).to_string_lossy().as_bytes()) else { return false };
    cef::load_library(Some(unsafe { &*p.as_ptr() })) == 1
}

/// In a helper process: the framework the browser process named (`BACKPLANE_CEF_FRAMEWORK`), else the one beside the helper app.
pub fn helper_framework() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_CEF_FRAMEWORK").map(PathBuf::from).filter(|p| p.join(LIB).exists()) { return Some(p) }
    let exe = std::env::current_exe().ok()?;
    // <dir>/Backplane Helper (Renderer).app/Contents/MacOS/Backplane Helper (Renderer): the framework is in <dir>
    let fw = exe.parent()?.parent()?.parent()?.parent()?.join(FRAMEWORK);
    fw.join(LIB).exists().then_some(fw)
}

/// The helper program beside the client at `exe` (its folder, or the Frameworks folder of the bundle the client runs from).
pub fn helper(exe: &Path) -> Option<PathBuf> {
    let d = exe.parent()?;
    [d.to_path_buf(), d.join("../Frameworks")].into_iter().map(|d| d.join(format!("{HELPER}.app/Contents/MacOS/{HELPER}"))).find(|p| p.exists())
}

/// The bundle Chromium takes as the application's (`main_bundle_path`): the one the client runs from (`X.app` for `X.app/Contents/MacOS/x`), else, for the
/// plain program an install runs, the helper app beside it. Chromium names its helpers' rendezvous (a Mach service) after this bundle's identifier, and a
/// helper finds it the same way: without a bundle the browser process and its helpers named different services and every helper quit at start.
pub fn main_bundle(exe: &Path, helper: &Path) -> PathBuf {
    let b = exe.parent().and_then(Path::parent).and_then(Path::parent);
    match b {
        Some(b) if b.extension().map_or(false, |e| e == "app") => b.to_path_buf(),
        // <dir>/Backplane Helper.app/Contents/MacOS/Backplane Helper -> <dir>/Backplane Helper.app
        _ => helper.parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default(),
    }
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getProtocol(name: *const c_char) -> *mut c_void;
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn class_addProtocol(cls: *mut c_void, proto: *mut c_void) -> bool;
}
/// gpui's NSApplication (`GPUIApplication`, vendor/gpui) answers `isHandlingSendEvent`/`setHandlingSendEvent:`; once the framework is loaded it also says
/// it conforms to Chromium's protocols, which Chromium checks before it trusts those answers.
pub fn adopt_app_protocol() {
    unsafe {
        let cls = objc_getClass(c"GPUIApplication".as_ptr());
        if cls.is_null() { return }
        for p in [c"CrAppProtocol", c"CrAppControlProtocol", c"CefAppProtocol"] {
            let proto = objc_getProtocol(p.as_ptr());
            if !proto.is_null() { class_addProtocol(cls, proto); }
        }
    }
}

/// The Mac virtual key code (kVK_*) for a Windows virtual-key code (what `cef::vk` gives); CEF's macOS OSR makes its NSEvent from it.
pub fn key_code(vk: i32) -> Option<i32> {
    Some(match vk {
        65 => 0x00, 83 => 0x01, 68 => 0x02, 70 => 0x03, 72 => 0x04, 71 => 0x05, 90 => 0x06, 88 => 0x07, 67 => 0x08, 86 => 0x09, 66 => 0x0B, 81 => 0x0C,
        87 => 0x0D, 69 => 0x0E, 82 => 0x0F, 89 => 0x10, 84 => 0x11, 79 => 0x1F, 85 => 0x20, 73 => 0x22, 80 => 0x23, 76 => 0x25, 74 => 0x26, 75 => 0x28,
        78 => 0x2D, 77 => 0x2E,
        49 => 0x12, 50 => 0x13, 51 => 0x14, 52 => 0x15, 54 => 0x16, 53 => 0x17, 57 => 0x19, 55 => 0x1A, 56 => 0x1C, 48 => 0x1D,
        187 => 0x18, 189 => 0x1B, 221 => 0x1E, 219 => 0x21, 222 => 0x27, 186 => 0x29, 220 => 0x2A, 188 => 0x2B, 191 => 0x2C, 190 => 0x2F, 192 => 0x32,
        13 => 0x24, 9 => 0x30, 32 => 0x31, 8 => 0x33, 27 => 0x35, 46 => 0x75, 36 => 0x73, 35 => 0x77, 33 => 0x74, 34 => 0x79, 45 => 0x72,
        37 => 0x7B, 39 => 0x7C, 40 => 0x7D, 38 => 0x7E,
        112 => 0x7A, 113 => 0x78, 114 => 0x63, 115 => 0x76, 116 => 0x60, 117 => 0x61, 118 => 0x62, 119 => 0x64, 120 => 0x65, 121 => 0x6D, 122 => 0x67, 123 => 0x6F,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn mac_key_codes() { assert_eq!(key_code(65), Some(0)); assert_eq!(key_code(13), Some(0x24)); assert_eq!(key_code(37), Some(0x7B)); assert_eq!(key_code(7), None); }
    #[test] fn a_plain_program_takes_its_helper_app_as_the_bundle() {
        let h = Path::new("/opt/bp/Backplane Helper.app/Contents/MacOS/Backplane Helper");
        assert_eq!(main_bundle(Path::new("/opt/bp/backplane-gpui"), h), PathBuf::from("/opt/bp/Backplane Helper.app"));
        assert_eq!(main_bundle(Path::new("/A/X.app/Contents/MacOS/x"), h), PathBuf::from("/A/X.app"));
    }
}
