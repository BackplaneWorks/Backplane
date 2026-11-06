//! A shell on a pty (src/server/pty.bend, effects/pty.js): the child is a session leader with the pty as its controlling terminal,
//! stdin/stdout/stderr on it, TERM=xterm-256color, COLORTERM=truecolor, no LINES/COLUMNS, and none of our other descriptors.
//! Reading and writing use one thread each (blocking fds) with bounded queues, so a flood from the program or a stuck program
//! never grows our memory: output waits in a 64-chunk queue (the program blocks on its pty), input past 256 queued writes is dropped.
use std::{
    fs::File, io::{Read, Write}, os::{fd::{AsRawFd, FromRawFd, OwnedFd}, unix::process::CommandExt, unix::process::ExitStatusExt},
    path::Path, process::{Command, Stdio}, sync::{mpsc::{sync_channel, SyncSender, TrySendError}, Arc},
};
use tokio::sync::{mpsc, oneshot};

pub const CHUNK: usize = 65536;
const OUT_QUEUE: usize = 64;
const IN_QUEUE: usize = 256;

pub struct Pty {
    pub pid: i32,
    master: Arc<File>,
    input: SyncSender<Vec<u8>>,
    gone: Arc<std::sync::atomic::AtomicBool>,
    closed: Arc<std::sync::atomic::AtomicBool>,
}
pub struct Spawned { pub pty: Pty, pub output: mpsc::Receiver<Vec<u8>>, pub exit: oneshot::Receiver<i32> }

fn winsize(cols: u16, rows: u16) -> libc::winsize { libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 } }

impl Pty {
    /// Run `cmd args` in `cwd` ("" keeps ours) on a new pty. Errors are the exec failure's (ENOENT for an unknown command).
    pub fn spawn(cmd: &str, args: &[String], cwd: &str, cols: u16, rows: u16) -> std::io::Result<Spawned> {
        // a hub on a copied log runs no shell (proc.rs would make the program a path that does not exist): say why, and open no pty for it
        if crate::copy::on() { return Err(std::io::Error::other(crate::copy::refusal("a terminal"))) }
        let (master, slave) = unsafe {
            let m = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY); // (Darwin takes only these two flags)
            if m >= 0 { libc::fcntl(m, libc::F_SETFD, libc::FD_CLOEXEC); }
            if m < 0 { return Err(std::io::Error::last_os_error()) }
            let m = OwnedFd::from_raw_fd(m);
            if libc::grantpt(m.as_raw_fd()) != 0 || libc::unlockpt(m.as_raw_fd()) != 0 { return Err(std::io::Error::last_os_error()) }
            let mut name = [0 as libc::c_char; 128];
            #[cfg(target_os = "linux")]
            if libc::ptsname_r(m.as_raw_fd(), name.as_mut_ptr(), name.len()) != 0 { return Err(std::io::Error::last_os_error()) }
            // macOS has no ptsname_r: ptsname returns a static buffer, so one at a time
            #[cfg(not(target_os = "linux"))]
            { static PTSNAME: std::sync::Mutex<()> = std::sync::Mutex::new(()); let _g = PTSNAME.lock().unwrap_or_else(|e| e.into_inner());
              let p = libc::ptsname(m.as_raw_fd()); if p.is_null() { return Err(std::io::Error::last_os_error()) }
              let b = std::ffi::CStr::from_ptr(p).to_bytes_with_nul(); if b.len() > name.len() { return Err(std::io::Error::other("pty name too long")) }
              for (i, c) in b.iter().enumerate() { name[i] = *c as libc::c_char } }
            let s = libc::open(name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY);
            if s < 0 { return Err(std::io::Error::last_os_error()) }
            libc::fcntl(s, libc::F_SETFD, libc::FD_CLOEXEC);
            let ws = winsize(cols, rows);
            libc::ioctl(m.as_raw_fd(), libc::TIOCSWINSZ, &ws);
            (File::from(m), File::from_raw_fd(s))
        };
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Terminal, cmd);
        c.args(args).env("TERM", "xterm-256color").env("COLORTERM", "truecolor").env_remove("LINES").env_remove("COLUMNS");
        if !cwd.is_empty() { c.current_dir(Path::new(cwd)); }
        c.stdin(Stdio::from(slave.try_clone()?)).stdout(Stdio::from(slave.try_clone()?)).stderr(Stdio::from(slave));
        unsafe { c.pre_exec(|| {
            if libc::setsid() < 0 { return Err(std::io::Error::last_os_error()) }
            if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 { return Err(std::io::Error::last_os_error()) }
            for s in [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGPIPE, libc::SIGCHLD, libc::SIGWINCH] { libc::signal(s, libc::SIG_DFL); }
            Ok(())
        }) };
        let mut child = c.spawn()?;
        drop(c); // the parent's copies of the slave go
        let pid = child.id() as i32;
        let master = Arc::new(master);
        // reader: blocking read, bounded queue; EIO is the pty's hang-up
        let (otx, orx) = mpsc::channel::<Vec<u8>>(OUT_QUEUE);
        let mut rd = master.try_clone()?;
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false)); let c2 = closed.clone();
        std::thread::Builder::new().name("pty-read".into()).spawn(move || {
            let mut buf = vec![0u8; CHUNK];
            loop {
                // wake every 200 ms to see whether the Pty was dropped: the thread (and its dup of the master) must not outlive it
                if c2.load(std::sync::atomic::Ordering::SeqCst) { break } // every turn: a program that floods and ignores the hang-up must not keep us reading
                let mut pfd = libc::pollfd { fd: rd.as_raw_fd(), events: libc::POLLIN, revents: 0 };
                if unsafe { libc::poll(&mut pfd, 1, 200) } == 0 { continue }
                match rd.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => if otx.blocking_send(buf[..n].to_vec()).is_err() { break } } }
        })?;
        // writer: bounded queue of writes
        let (itx, irx) = sync_channel::<Vec<u8>>(IN_QUEUE);
        let mut wr = master.try_clone()?;
        std::thread::Builder::new().name("pty-write".into()).spawn(move || { while let Ok(b) = irx.recv() { if wr.write_all(&b).is_err() { break } } })?;
        // reaper
        let (etx, erx) = oneshot::channel();
        let gone = Arc::new(std::sync::atomic::AtomicBool::new(false)); let g2 = gone.clone();
        std::thread::Builder::new().name("pty-wait".into()).spawn(move || {
            let code = match child.wait() { Ok(s) => s.code().unwrap_or_else(|| 128 + s.signal().unwrap_or(0)), Err(_) => 255 };
            g2.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = etx.send(code);
        })?;
        Ok(Spawned { pty: Pty { pid, master, input: itx, gone, closed }, output: orx, exit: erx })
    }
    /// Keys, a paste. False when the queue is full (the program is not reading) or the pty is gone: the bytes are dropped.
    pub fn write(&self, bytes: Vec<u8>) -> bool { match self.input.try_send(bytes) { Ok(()) => true, Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false } }
    pub fn resize(&self, cols: u16, rows: u16) { let ws = winsize(cols, rows); unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &ws); } }
    /// A hang-up for the whole process group (the leader's pid negated): the command the shell runs goes too.
    pub fn hangup(&self) { if self.pid > 1 && !self.gone.load(std::sync::atomic::Ordering::SeqCst) { unsafe { libc::kill(-self.pid, libc::SIGHUP); } } }
}
impl Pty {
    /// SIGKILL for a program that ignored the hang-up.
    pub fn force(&self) { if self.pid > 1 && !self.gone.load(std::sync::atomic::Ordering::SeqCst) { unsafe { libc::kill(-self.pid, libc::SIGKILL); } } }
}
/// Dropping hangs up, stops the reader (which closes its dup of the master) and the writer (its queue closes).
impl Drop for Pty { fn drop(&mut self) { self.hangup(); self.closed.store(true, std::sync::atomic::Ordering::SeqCst) } }
