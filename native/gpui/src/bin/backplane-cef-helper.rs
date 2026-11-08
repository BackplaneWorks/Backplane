//! macOS: the program of `Backplane Helper.app` (and its `(GPU)`, `(Renderer)`, `(Plugin)`, `(Alerts)` twins), the embedded Chromium's helper processes
//! (viewers/cef.rs, docs/browser-gpui-cef.md). It loads the framework the client named (or the one beside the helper app) and runs CEF's process.
//! The client cannot be its own helper on macOS: a helper must be a bundle's program, and an update refuses links, so it would be the whole client
//! copied five times. On Linux and Windows the client runs its own helpers; this program only says so.
#[cfg(target_os = "macos")]
#[path = "../viewers/cef_mac.rs"]
#[allow(dead_code)] // the client's half of it (key codes, the app protocol) is not used here
mod mac;

#[cfg(target_os = "macos")]
fn main() {
    let Some(fw) = mac::helper_framework() else { eprintln!("backplane-cef-helper: no Chromium Embedded Framework found"); std::process::exit(1) };
    if !mac::load(&fw) { eprintln!("backplane-cef-helper: could not load {}", fw.display()); std::process::exit(1) }
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    std::process::exit(cef::execute_process(Some(args.as_main_args()), None, std::ptr::null_mut()).max(0));
}

#[cfg(not(target_os = "macos"))]
fn main() { eprintln!("backplane-cef-helper is macOS's; on this system backplane-gpui runs its own helper processes"); std::process::exit(2) }
