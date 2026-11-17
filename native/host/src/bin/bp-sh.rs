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

/// What bp-sh says when PROGRAM does not start: the program by name, and for Git's shell (the hub passes a bare `sh.exe` when it found
/// no Git for Windows) what to install. 0.16.0 said only "bp-sh: program not found", which read as bp-sh itself missing.
#[allow(dead_code)]
fn failed(prog: &str, e: &str) -> String {
    let base = prog.rsplit(['\\', '/']).next().unwrap_or(prog).to_ascii_lowercase();
    if matches!(base.as_str(), "sh.exe" | "bash.exe" | "sh" | "bash") && !prog.contains(['\\', '/']) {
        return format!("bp-sh: {prog}: {e}. Backplane runs its scripts with Git for Windows' shell and found none: install Git for Windows (winget install --id Git.Git -e, or https://git-scm.com/download/win); no restart is needed")
    }
    format!("bp-sh: {prog}: {e}")
}

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
        let mut c = std::process::Command::new(&prog);
        for x in a { c.raw_arg(super::quote(&x.to_string_lossy())); }
        match c.status() { Ok(s) => s.code().unwrap_or(1), Err(e) => { eprintln!("{}", super::failed(&prog.to_string_lossy(), &e.to_string())); 127 } }
    }
}

#[cfg(test)]
mod tests {
    #[test] fn a_missing_shell_says_to_install_git() {
        let m = super::failed("sh.exe", "program not found");
        assert!(m.starts_with("bp-sh: sh.exe: program not found") && m.contains("Git for Windows"), "{m}");
        assert_eq!(super::failed(r"C:\Program Files\Git\bin\sh.exe", "denied"), r"bp-sh: C:\Program Files\Git\bin\sh.exe: denied", "a shell found and failing is not a missing Git");
        assert_eq!(super::failed("python", "program not found"), "bp-sh: python: program not found");
    }
    #[test] fn every_argument_is_quoted_for_cygwin() {
        assert_eq!(super::quote(r#"[\\/] "q""#), r#""[\\\\/] \"q\"""#);
        assert_eq!(super::quote(""), r#""""#);
        assert_eq!(super::quote(r"C:\x y"), r#""C:\\x y""#);
    }
}
