//! The little that differs between Unix (Linux, macOS) and Windows, in one place. Callers use `crate::plat::...` and never `std::os::unix`,
//! `libc::kill` or `pre_exec` themselves. On Unix every item is the call the hub always made (std's own extension traits re-exported, the
//! same libc calls), so Linux and macOS behave exactly as before; `windows.rs` gives the nearest Windows meaning and says where there is none.
#[cfg(unix)] mod unix;
#[cfg(unix)] pub use unix::*;
#[cfg(windows)] mod windows;
#[cfg(windows)] pub use windows::*;
pub mod dirs;
#[cfg(test)] pub mod tmp;

/// A child's exit as the shell says it: its code, else 128 + the signal that ended it (Unix only has those).
pub fn exit_code(s: &std::process::ExitStatus) -> i32 { s.code().unwrap_or_else(|| 128 + proc::signal(s).unwrap_or(0)) }

