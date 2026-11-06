//! NDJSON events on stdout, one object a line, written at once; diagnostics
//! on stderr. `end` is always the last event.

use serde_json::{json, Value};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static FAILED: AtomicBool = AtomicBool::new(false);
static ENDED: Mutex<bool> = Mutex::new(false);

fn write_line(ended: &bool, ev: &Value) {
    if *ended {
        return;
    }
    let mut line = ev.to_string();
    line.push('\n');
    let mut o = std::io::stdout().lock();
    // a reader that went away is not ours to report on stdout
    let _ = o.write_all(line.as_bytes());
    let _ = o.flush();
}

pub fn emit(ev: Value) {
    let ended = ENDED.lock().unwrap_or_else(|e| e.into_inner());
    write_line(&ended, &ev);
}

pub fn error(message: &str) {
    FAILED.store(true, Ordering::SeqCst);
    emit(json!({"event": "error", "error": message}));
}

pub fn end() -> ! {
    {
        let mut ended = ENDED.lock().unwrap_or_else(|e| e.into_inner());
        write_line(&ended, &json!({"event": "end"}));
        *ended = true;
    }
    exit(if FAILED.load(Ordering::SeqCst) { 1 } else { 0 })
}

pub fn fail(message: &str) -> ! {
    error(message);
    end()
}

pub fn log(s: &str) {
    eprintln!("backplane-voice: {s}");
}

/// Leaves at once. Nothing here buffers output, and running atexit handlers
/// would tear ONNX Runtime down under a decode still on another thread.
pub fn exit(code: i32) -> ! {
    #[cfg(unix)]
    unsafe {
        libc::_exit(code)
    }
    #[cfg(not(unix))]
    std::process::exit(code)
}
