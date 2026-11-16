//! KiCad's lock files, read by the hub before it lets a KiCad open a project or a document (the release check's Linux stall, Nov 16;
//! bp.13 opens over a lock without asking, so this is what keeps two KiCads from saving over each other).
//!
//! KiCad writes `~<file>.lck` beside the project, board and schematic it opens: JSON {"username","hostname","token"}, and holds a
//! `flock` on it while it runs (since 9ededb57e9, in bp.12.4: a lock nobody holds is one it reclaims by itself). So a lock is:
//!   - stale: nobody holds it (its KiCad died): the hub removes it and logs it;
//!   - ours: held by a KiCad this hub started (`--parent-pid <this hub>`): the GUI session we have, or our headless server (retired first);
//!   - foreign: held by any other live process (the person's own KiCad, another Backplane's), or written by another computer (a shared
//!     folder: its holder is not ours to see): the person is asked in the pane, Open anyway or Cancel, before KiCad opens the document.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The lock file KiCad keeps for `doc`.
pub fn lock_path(doc: &str) -> Option<PathBuf> {
    let p = Path::new(doc);
    Some(p.parent()?.join(format!("~{}.lck", p.file_name()?.to_string_lossy())))
}

#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// no lock file
    Free,
    /// a lock file nobody holds: left by a KiCad that ended
    Stale,
    /// a lock file whose holder this platform cannot see (no flock test): left alone, KiCad decides
    Unknown,
    /// held by a KiCad this hub started (its pid)
    Ours(u32),
    /// held by another process here (its pid when known), or written on another computer
    Foreign { pid: Option<u32>, user: String, host: String, what: String },
}

#[derive(Clone, Debug, Default)]
pub struct Seen { pub user: String, pub host: String, pub held: Option<bool>, pub pid: Option<u32>, pub cmdline: String }

/// What the lock of `doc` is, against this hub's pid and this computer's user and host.
pub fn classify(seen: Option<&Seen>, hub_pid: u32, _me_user: &str, me_host: &str) -> Verdict {
    let Some(s) = seen else { return Verdict::Free };
    let other_host = !s.host.is_empty() && !me_host.is_empty() && !s.host.eq_ignore_ascii_case(me_host);
    match s.held {
        // nobody holds it here; written by another computer it may still be open there (a shared folder: no flock across machines)
        Some(false) if !other_host => Verdict::Stale,
        None if !other_host => Verdict::Unknown,
        Some(true) if s.pid.is_some() && parent_pid_arg(&s.cmdline) == Some(hub_pid) => Verdict::Ours(s.pid.unwrap_or(0)),
        _ => Verdict::Foreign { pid: s.pid, user: s.user.clone(), host: s.host.clone(), what: program_of(&s.cmdline) },
    }
}

/// The `--parent-pid` a KiCad of Backplane's was started with.
pub fn parent_pid_arg(cmdline: &str) -> Option<u32> {
    let mut it = cmdline.split('\0').flat_map(|a| a.split(' '));
    while let Some(a) = it.next() { if a == "--parent-pid" { return it.next()?.parse().ok() } }
    None
}

/// What holds it, in words: "KiCad (pid 123)", "kicad-cli (pid 9)", "" unknown.
fn program_of(cmdline: &str) -> String {
    let first = cmdline.split('\0').next().unwrap_or("");
    Path::new(first).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
}

/// Read the lock of `doc` on this computer: its owner (the JSON), whether a process holds its flock, and which (Linux: /proc/locks).
pub fn inspect(doc: &str) -> Option<Seen> {
    let path = lock_path(doc)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let j: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let mut s = Seen { user: j["username"].as_str().unwrap_or("").into(), host: j["hostname"].as_str().unwrap_or("").into(), ..Default::default() };
    s.held = held(&path);
    if s.held == Some(true) {
        s.pid = holder_pid(&path);
        if let Some(pid) = s.pid { s.cmdline = cmdline_of(pid); }
    }
    Some(s)
}

/// A process's command line, its arguments split by NUL (Linux /proc; elsewhere `ps`, split at spaces).
fn cmdline_of(pid: u32) -> String {
    if let Ok(b) = std::fs::read(format!("/proc/{pid}/cmdline")) { return String::from_utf8_lossy(&b).to_string() }
    crate::proc::std_cmd(crate::proc::Kind::Local, "ps").args(["-o", "command=", "-p", &pid.to_string()]).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().replace(' ', "\0")).unwrap_or_default()
}

/// Whether some process holds the lock file's flock (None: cannot tell on this platform).
#[cfg(unix)]
fn held(path: &Path) -> Option<bool> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::File::open(path).ok()?;
    // a shared, non-blocking try: refused while KiCad holds its exclusive lock; when free, released at once (the file closes)
    let r = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) };
    if r == 0 { unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_UN); } Some(false) }
    else if std::io::Error::last_os_error().raw_os_error() == Some(libc::EWOULDBLOCK) { Some(true) } else { None }
}
/// Windows (bp.13, 48440e2996): KiCad holds an exclusive byte-range lock on 1 byte at 0x40000000 of the .lck for its life (the file is
/// opened share read/write/delete, so the share mode says nothing); the test is KiCad's own FILE_LOCK::OpenForInspect (Carbon, Nov 16).
/// An older runtime takes no such lock: its lock reads as not held (stale), as KiCad itself would take it.
#[cfg(windows)]
fn held(path: &Path) -> Option<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Foundation::{ERROR_IO_PENDING, ERROR_LOCK_VIOLATION, HANDLE}, Storage::FileSystem::{LockFileEx, UnlockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY}, System::IO::OVERLAPPED};
    use std::os::windows::fs::OpenOptionsExt;
    let f = std::fs::OpenOptions::new().read(true).share_mode(0x1 | 0x2 | 0x4).open(path).ok()?;
    let h = f.as_raw_handle() as HANDLE;
    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    ov.Anonymous.Anonymous.Offset = 0x4000_0000;
    let ok = unsafe { LockFileEx(h, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, 1, 0, &mut ov) };
    if ok != 0 { unsafe { UnlockFileEx(h, 0, 1, 0, &mut ov); } return Some(false) }
    match std::io::Error::last_os_error().raw_os_error().map(|e| e as u32) { Some(ERROR_LOCK_VIOLATION) | Some(ERROR_IO_PENDING) => Some(true), _ => None }
}
#[cfg(not(any(unix, windows)))]
fn held(_path: &Path) -> Option<bool> { None }

/// The pid holding the flock on `path` (Linux: /proc/locks by the file's device and inode).
fn holder_pid(path: &Path) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path).ok()?;
        let (dev, ino) = (m.dev(), m.ino());
        let (maj, min) = (((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff), (dev & 0xff) | ((dev >> 12) & !0xff));
        let want = format!("{maj:02x}:{min:02x}:{ino}");
        let locks = std::fs::read_to_string("/proc/locks").ok()?;
        return locks.lines().find_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            // "1: FLOCK  ADVISORY  WRITE 1332987 103:02:12345 0 EOF" (a blocked waiter's line has "->" after the number)
            let f: Vec<&str> = f.into_iter().filter(|x| *x != "->").collect();
            (f.get(1) == Some(&"FLOCK") && f.get(5).is_some_and(|d| same_dev_ino(d, &want))).then(|| f.get(4)?.parse().ok()).flatten()
        });
    }
    // macOS: no /proc/locks; lsof on the one file names the processes that have it open (lane M: ~50 ms, no sudo for one's own)
    #[cfg(target_os = "macos")]
    {
        let me = std::process::id();
        let out = crate::proc::std_cmd(crate::proc::Kind::Local, "lsof").arg("-t").arg("--").arg(path).output().ok()?;
        return String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().parse::<u32>().ok()).find(|p| *p != me);
    }
    #[allow(unreachable_code)]
    { let _ = path; None }
}
/// /proc/locks prints the device as hex major:minor with at least two digits each; compare numerically.
fn same_dev_ino(a: &str, b: &str) -> bool {
    let p = |s: &str| -> Option<(u64, u64, u64)> { let mut it = s.split(':'); Some((u64::from_str_radix(it.next()?, 16).ok()?, u64::from_str_radix(it.next()?, 16).ok()?, it.next()?.parse().ok()?)) };
    p(a).is_some() && p(a) == p(b)
}

/// This computer's user and host as KiCad writes them (wxGetUserId, wxGetHostName: the short host name).
pub fn me() -> (String, String) {
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    let host = hostname();
    (user, host)
}
fn hostname() -> String {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
            let n = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
            return String::from_utf8_lossy(&buf[..n]).split('.').next().unwrap_or("").to_string();
        }
    }
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

/// Remove a stale lock (nobody holds it), saying so in the hub log. Never one that is held.
pub fn clear_stale(doc: &str, why: &str) -> bool { lock_path(doc).is_some_and(|p| clear_stale_file(&p, why)) }
/// The same for a lock file by its own path (a sweep of a folder's `~*.lck`): only one this platform can see is not held.
pub fn clear_stale_file(p: &Path, why: &str) -> bool {
    if held(p) != Some(false) { return false }
    let ok = std::fs::remove_file(&p).is_ok();
    if ok { eprintln!("kicad locks: removed the stale lock {} ({why})", p.display()); }
    ok
}

/// The words for who holds a lock: "KiCad (pid 412) of h at desktop", "h at laptop".
pub fn holder_words(v: &Verdict) -> String {
    match v {
        Verdict::Foreign { pid, user, host, what } => {
            let prog = if what.is_empty() { "another KiCad".to_string() } else { what.trim_end_matches(".real").to_string() };
            let who = match (user.is_empty(), host.is_empty()) { (false, false) => format!(" of {user} at {host}"), (false, true) => format!(" of {user}"), _ => String::new() };
            match pid { Some(p) => format!("{prog} (pid {p}){who}"), None => format!("{prog}{who}") }
        }
        _ => String::new(),
    }
}

/// The person's answer was Open anyway: the lock is removed so this KiCad opens without asking (the other KiCad keeps its own open
/// file and may save over this one's work: the pane said so).
pub fn override_lock(doc: &str, holder: &str) {
    if let Some(p) = lock_path(doc) {
        let _ = std::fs::remove_file(&p);
        eprintln!("kicad locks: the person chose Open anyway over {holder}'s lock {}", p.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_lock_is_stale_ours_or_foreign() {
        let seen = |held: Option<bool>, pid: Option<u32>, cmd: &str, host: &str| Seen { user: "h".into(), host: host.into(), held, pid, cmdline: cmd.into() };
        assert_eq!(classify(None, 10, "h", "desktop"), Verdict::Free);
        assert_eq!(classify(Some(&seen(Some(false), None, "", "desktop")), 10, "h", "desktop"), Verdict::Stale, "nobody holds it: its KiCad ended");
        assert_eq!(classify(Some(&seen(Some(true), Some(77), "/rt/bin/kicad-cli.real\0api-server\0/p/a.kicad_pro\0--socket\0/s\0--parent-pid\0010\0", "desktop")), 10, "h", "desktop"), Verdict::Ours(77));
        assert_eq!(classify(Some(&seen(Some(true), Some(77), "/rt/bin/kicad.real\0--embedded\0--parent-pid\010\0/p/a.kicad_pro", "desktop")), 10, "h", "desktop"), Verdict::Ours(77));
        assert!(matches!(classify(Some(&seen(Some(true), Some(55), "/usr/bin/kicad\0/p/a.kicad_pro", "desktop")), 10, "h", "desktop"), Verdict::Foreign { pid: Some(55), .. }), "the person's own KiCad");
        assert!(matches!(classify(Some(&seen(Some(true), Some(55), "/rt/kicad.real\0--parent-pid\0999", "desktop")), 10, "h", "desktop"), Verdict::Foreign { .. }), "another Backplane's KiCad");
        assert!(matches!(classify(Some(&seen(Some(false), None, "", "laptop")), 10, "h", "desktop"), Verdict::Foreign { .. }), "written on another computer: open there, perhaps");
        assert_eq!(parent_pid_arg("a\0--parent-pid\042\0b"), Some(42));
    }
    #[cfg(target_os = "linux")]
    #[test] fn a_held_flock_is_seen_with_its_holder_and_a_free_one_is_stale() {
        use std::os::unix::io::AsRawFd;
        let d = crate::plat::tmp::dir("locks");
        let doc = d.join("a.kicad_pro"); let lck = lock_path(&doc.to_string_lossy()).unwrap();
        std::fs::write(&lck, r#"{"hostname":"desktop","username":"h","token":"t"}"#).unwrap();
        let s = inspect(&doc.to_string_lossy()).unwrap();
        assert_eq!((s.held, s.user.as_str(), s.host.as_str()), (Some(false), "h", "desktop"));
        // held by another open file description (as another process's would be)
        let f = std::fs::File::open(&lck).unwrap();
        assert_eq!(unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }, 0);
        let s = inspect(&doc.to_string_lossy()).unwrap();
        assert_eq!(s.held, Some(true)); assert_eq!(s.pid, Some(std::process::id()), "/proc/locks names the holder");
        assert!(!clear_stale(&doc.to_string_lossy(), "test"), "a held lock is never removed");
        drop(f);
        assert!(clear_stale(&doc.to_string_lossy(), "test") && !lck.exists());
    }
}
