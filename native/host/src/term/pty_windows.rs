//! A shell on a Windows pseudo console (ConPTY), with what `pty.rs` gives on Unix: TERM=xterm-256color, COLORTERM=truecolor, no
//! LINES/COLUMNS, none of our handles; one thread each for reading and writing with the same bounded queues. The command is built by
//! `proc::std_cmd` (the copy gate and the scrubbed environment) and started with CreateProcessW, which can attach a pseudo console.
//! The leader is put in a Job Object that dies with the hub (plat::proc::adopt); a hang-up ends that job (Windows has no SIGHUP).
//! ConPTY keeps its output pipe open after the program exits, so the waiter closes the pseudo console then, and the reader sees the end.
use std::{
    ffi::OsStr, fs::File, io::{Read, Write}, os::windows::{ffi::OsStrExt, io::FromRawHandle},
    sync::{atomic::{AtomicBool, Ordering::SeqCst}, mpsc::{sync_channel, SyncSender, TrySendError}, Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};
use windows_sys::Win32::{Foundation::*, System::{Console::*, Pipes::CreatePipe, Threading::*}};

pub const CHUNK: usize = 65536;
const OUT_QUEUE: usize = 64;
const IN_QUEUE: usize = 256;

/// The pseudo console, closed once (by the waiter when the program ends, or on drop).
struct Con(Mutex<Option<HPCON>>);
impl Con {
    fn close(&self) { if let Some(h) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() { unsafe { ClosePseudoConsole(h) } } }
    fn resize(&self, cols: u16, rows: u16) { if let Some(h) = *self.0.lock().unwrap_or_else(|e| e.into_inner()) { unsafe { ResizePseudoConsole(h, size(cols, rows)); } } }
}
pub struct Pty {
    pub pid: i32,
    con: Arc<Con>,
    input: SyncSender<Vec<u8>>,
    gone: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}
pub struct Spawned { pub pty: Pty, pub output: mpsc::Receiver<Vec<u8>>, pub exit: oneshot::Receiver<i32> }

fn size(cols: u16, rows: u16) -> COORD { COORD { X: cols.max(1).min(i16::MAX as u16) as i16, Y: rows.max(1).min(i16::MAX as u16) as i16 } }
fn wide(s: &OsStr) -> Vec<u16> { s.encode_wide().chain(Some(0)).collect() }
/// One argument as the C runtime splits a command line (backslashes double only before a quote).
fn quote(a: &OsStr, out: &mut Vec<u16>) {
    let w: Vec<u16> = a.encode_wide().collect();
    if !w.is_empty() && !w.iter().any(|&c| c == b' ' as u16 || c == b'\t' as u16 || c == b'"' as u16) { out.extend(w); return }
    out.push(b'"' as u16); let mut bs = 0;
    for c in w { if c == b'\\' as u16 { bs += 1; continue } if c == b'"' as u16 { out.extend(std::iter::repeat(b'\\' as u16).take(bs * 2 + 1)) } else { out.extend(std::iter::repeat(b'\\' as u16).take(bs)) } bs = 0; out.push(c) }
    out.extend(std::iter::repeat(b'\\' as u16).take(bs * 2)); out.push(b'"' as u16);
}

impl Pty {
    /// Run `cmd args` in `cwd` ("" keeps ours) on a new pseudo console. Errors are CreateProcessW's (NotFound for an unknown command).
    pub fn spawn(cmd: &str, args: &[String], cwd: &str, cols: u16, rows: u16) -> std::io::Result<Spawned> {
        if crate::copy::on() { return Err(std::io::Error::other(crate::copy::refusal("a terminal"))) }
        Self::spawn_as(crate::proc::Kind::Terminal, cmd, args, cwd, cols, rows, &mut |_| {})
    }
    /// `spawn` as another kind of process, its environment then set by `env` (pty.rs `spawn_as`).
    pub fn spawn_as(kind: crate::proc::Kind, cmd: &str, args: &[String], cwd: &str, cols: u16, rows: u16, env: &mut dyn FnMut(&mut std::process::Command)) -> std::io::Result<Spawned> {
        let mut c = crate::proc::std_cmd(kind, cmd);
        c.args(args).env("TERM", "xterm-256color").env("COLORTERM", "truecolor").env_remove("LINES").env_remove("COLUMNS");
        env(&mut c);
        if !cwd.is_empty() { c.current_dir(cwd); }
        // the command line, environment and folder the gate made, handed to CreateProcessW
        let mut line = vec![]; quote(c.get_program(), &mut line); for a in c.get_args() { line.push(b' ' as u16); quote(a, &mut line); } line.push(0);
        let mut env: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os().collect();
        for (k, v) in c.get_envs() { let up = k.to_string_lossy().to_uppercase(); env.retain(|(e, _)| e.to_string_lossy().to_uppercase() != up); if let Some(v) = v { env.push((k.to_os_string(), v.to_os_string())); } }
        env.sort_by_key(|(k, _)| k.to_string_lossy().to_uppercase());
        let mut block: Vec<u16> = vec![]; for (k, v) in &env { block.extend(k.encode_wide()); block.push(b'=' as u16); block.extend(v.encode_wide()); block.push(0); } block.push(0);
        let dir = c.get_current_dir().map(|d| wide(d.as_os_str()));
        unsafe {
            let (mut in_r, mut in_w, mut out_r, mut out_w): (HANDLE, HANDLE, HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            if CreatePipe(&mut in_r, &mut in_w, std::ptr::null(), 0) == 0 { return Err(std::io::Error::last_os_error()) }
            if CreatePipe(&mut out_r, &mut out_w, std::ptr::null(), 0) == 0 { let e = std::io::Error::last_os_error(); CloseHandle(in_r); CloseHandle(in_w); return Err(e) }
            let mut hpc: HPCON = 0;
            let hr = CreatePseudoConsole(size(cols, rows), in_r, out_w, 0, &mut hpc);
            CloseHandle(in_r); CloseHandle(out_w); // conhost holds its own copies
            let (wr, rd) = (File::from_raw_handle(in_w as _), File::from_raw_handle(out_r as _));
            if hr != 0 { return Err(std::io::Error::other(format!("CreatePseudoConsole failed (0x{hr:08x})"))) }
            let con = Arc::new(Con(Mutex::new(Some(hpc))));
            let mut n = 0usize; InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut n);
            let mut list = vec![0u8; n]; let lp = list.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
            if InitializeProcThreadAttributeList(lp, 1, 0, &mut n) == 0 { con.close(); return Err(std::io::Error::last_os_error()) }
            if UpdateProcThreadAttribute(lp, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize, hpc as *const _, std::mem::size_of::<HPCON>(), std::ptr::null_mut(), std::ptr::null()) == 0 { let e = std::io::Error::last_os_error(); DeleteProcThreadAttributeList(lp); con.close(); return Err(e) }
            let mut si: STARTUPINFOEXW = std::mem::zeroed();
            si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32; si.lpAttributeList = lp;
            // no standard handles of ours: the program's are the pseudo console's
            si.StartupInfo.dwFlags = STARTF_USESTDHANDLES; si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE; si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE; si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
            let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
            let ok = CreateProcessW(std::ptr::null(), line.as_mut_ptr(), std::ptr::null(), std::ptr::null(), 0, EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                block.as_ptr() as *const _, dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()), &si.StartupInfo, &mut pi);
            let err = std::io::Error::last_os_error();
            DeleteProcThreadAttributeList(lp);
            if ok == 0 { con.close(); return Err(err) }
            CloseHandle(pi.hThread);
            let pid = pi.dwProcessId; crate::plat::proc::adopt(pid, true);
            let process = pi.hProcess as usize;
            // reader: blocking reads, bounded queue; the end comes when the pseudo console is closed
            let (otx, orx) = mpsc::channel::<Vec<u8>>(OUT_QUEUE);
            let closed = Arc::new(AtomicBool::new(false)); let c2 = closed.clone(); let mut rd = rd;
            std::thread::Builder::new().name("pty-read".into()).spawn(move || {
                let mut buf = vec![0u8; CHUNK];
                loop { if c2.load(SeqCst) { break } match rd.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => if otx.blocking_send(buf[..n].to_vec()).is_err() { break } } }
            })?;
            let (itx, irx) = sync_channel::<Vec<u8>>(IN_QUEUE); let mut wr = wr;
            std::thread::Builder::new().name("pty-write".into()).spawn(move || { while let Ok(b) = irx.recv() { if wr.write_all(&b).is_err() { break } } })?;
            // waiter: the exit code, then the pseudo console goes (its last output reaches the reader first)
            let (etx, erx) = oneshot::channel();
            let gone = Arc::new(AtomicBool::new(false)); let g2 = gone.clone(); let con2 = con.clone();
            std::thread::Builder::new().name("pty-wait".into()).spawn(move || {
                let h = process as HANDLE; let mut code = 255u32;
                if WaitForSingleObject(h, INFINITE) == 0 && GetExitCodeProcess(h, &mut code) == 0 { code = 255 }
                CloseHandle(h);
                g2.store(true, SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(50));
                con2.close();
                let _ = etx.send(code as i32);
            })?;
            Ok(Spawned { pty: Pty { pid: pid as i32, con, input: itx, gone, closed }, output: orx, exit: erx })
        }
    }
    /// Keys, a paste. False when the queue is full (the program is not reading) or the console is gone: the bytes are dropped.
    pub fn write(&self, bytes: Vec<u8>) -> bool { match self.input.try_send(bytes) { Ok(()) => true, Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false } }
    pub fn resize(&self, cols: u16, rows: u16) { self.con.resize(cols, rows) }
    /// The hang-up: Windows has none to send, so the program's job ends (what it started goes too).
    pub fn hangup(&self) { if self.pid > 1 && !self.gone.load(SeqCst) { crate::plat::proc::kill_group(self.pid as u32, crate::plat::proc::HUP); } }
    pub fn force(&self) { if self.pid > 1 && !self.gone.load(SeqCst) { crate::plat::proc::kill_group(self.pid as u32, crate::plat::proc::KILL); } }
}
/// Dropping ends the program, stops the reader (the console closes) and the writer (its queue closes).
impl Drop for Pty { fn drop(&mut self) { self.hangup(); self.closed.store(true, SeqCst); self.con.close() } }
