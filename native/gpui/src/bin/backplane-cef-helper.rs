//! macOS: the executable of CEF's helper bundles (`Backplane Helper (GPU).app` and the others, beside the framework: scripts/cef-runtime-pack.sh).
//! It loads the framework three levels up from itself, as CEF's helpers do, and runs the process CEF asked for. Elsewhere the client is its own helper.
#[cfg(target_os = "macos")]
fn main() {
    let exe = std::env::current_exe().expect("current_exe");
    let fw = exe.parent().and_then(|d| d.parent()).and_then(|d| d.parent()).and_then(|d| d.parent()).map(|d| d.join("Chromium Embedded Framework.framework/Chromium Embedded Framework"));
    let Some(fw) = fw else { std::process::exit(1) };
    let c = std::ffi::CString::new(fw.to_string_lossy().as_bytes()).expect("path");
    if cef::load_library(Some(unsafe { &*c.as_ptr() })) != 1 { eprintln!("backplane-cef-helper: {} did not load", fw.display()); std::process::exit(1) }
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    std::process::exit(cef::execute_process(Some(args.as_main_args()), None, std::ptr::null_mut()));
}
#[cfg(not(target_os = "macos"))]
fn main() { eprintln!("backplane-cef-helper is macOS only: the client is its own CEF helper here"); std::process::exit(2) }
