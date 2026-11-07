//! `bp-sh PROGRAM ARGS...` (Windows): run Git for Windows' sh or bash with exactly ARGS. A Cygwin/MSYS program started by a Windows
//! program splits its command line by its own rules (inside quotes a backslash escapes a backslash or a quote), not the C runtime's
//! rules that std writes, so `sh -c '[\\/]'` would reach the shell as `[\/]`. This writes the line for Cygwin's rules instead.
//! It and the shell share a Job Object that dies with it, so ending this process (the hub's process-group kill) ends the shell too.
//! The hub's proc.rs starts every `sh`/`bash` through it when it sits beside the hub. Exits with the shell's code.
fn main() {
    #[cfg(windows)] { std::process::exit(win::run()) }
    #[cfg(not(windows))] { eprintln!("bp-sh is for Windows"); std::process::exit(2) }
}
/// One argument for Cygwin's command-line rules: always quoted, `\` and `"` escaped by a backslash.
#[allow(dead_code)]
fn quote(s: &str) -> String { let mut o = String::with_capacity(s.len() + 2); o.push('"'); for c in s.chars() { if c == '\\' || c == '"' { o.push('\\') } o.push(c) } o.push('"'); o }

#[cfg(windows)]
mod win {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::{JobObjects::*, Threading::GetCurrentProcess};
    pub fn run() -> i32 {
        let mut a = std::env::args_os().skip(1);
        let Some(prog) = a.next() else { eprintln!("usage: bp-sh PROGRAM ARGS..."); return 2 };
        unsafe { // this process and what it starts die together
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if !job.is_null() {
                let mut l: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed(); l.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(job, JobObjectExtendedLimitInformation, &l as *const _ as *const _, std::mem::size_of_val(&l) as u32);
                AssignProcessToJobObject(job, GetCurrentProcess()); // the handle stays open until this process ends
            }
        }
        let mut c = std::process::Command::new(prog);
        for x in a { c.raw_arg(super::quote(&x.to_string_lossy())); }
        match c.status() { Ok(s) => s.code().unwrap_or(1), Err(e) => { eprintln!("bp-sh: {e}"); 127 } }
    }
}

#[cfg(test)]
mod tests {
    #[test] fn every_argument_is_quoted_for_cygwin() {
        assert_eq!(super::quote(r#"[\\/] "q""#), r#""[\\\\/] \"q\"""#);
        assert_eq!(super::quote(""), r#""""#);
        assert_eq!(super::quote(r"C:\x y"), r#""C:\\x y""#);
    }
}
