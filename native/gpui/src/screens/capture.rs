//! Screenshot and screen recording of this app's own window, for Improvements (docs/reporting/README.md "Screen recordings").
//! One design for both: `ffmpeg -f x11grab -window_id <our window>` (never the root window, never another window), a single frame for the
//! screenshot, VP8 WebM at 15 fps with no audio for a recording, ended at 120 s or 9 MiB. Started only by the person pressing the button; a
//! recording stays in the private draft folder until they choose to attach it. Without ffmpeg (or a display) the controls say so and stay off.
use std::{path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::{Duration, Instant}};

pub const MAX_SECS: u32 = 120;
pub const MAX_BYTES: u64 = 9 * 1024 * 1024;
/// ffmpeg stops writing a little before the cap so the finished file is under it.
const FILE_CAP: u64 = MAX_BYTES - 128 * 1024;
pub const FPS: u32 = 15;

/// The ffmpeg to run: `BACKPLANE_GPUI_FFMPEG` (a path, for tests), else the first on PATH.
pub fn ffmpeg() -> Result<PathBuf, &'static str> {
    /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ if let Some(p) = std::env::var_os("BACKPLANE_GPUI_FFMPEG").map(PathBuf::from) { return if p.is_file() { Ok(p) } else { Err("Recording needs ffmpeg, and BACKPLANE_GPUI_FFMPEG does not name one.") }; }
    std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).map(|d| d.join("ffmpeg")).find(|p| p.is_file())
        /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ .ok_or("Screen capture needs ffmpeg, which is not installed.")
}
/// The X display this app is drawn on.
pub fn display() -> Result<String, &'static str> {
    std::env::var("DISPLAY").ok().filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_')))
        /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ .ok_or("Screen capture needs an X11 display.")
}
/// Whether the controls can work at all, as one sentence when they cannot.
pub fn unavailable() -> Option<&'static str> { ffmpeg().err().or(display().err()) }

/// This app's own top-level window: the X window that carries this process's `_NET_WM_PID` (GPUI sets it; its platform window handle is not
/// implemented on X11). Windows a window manager reparents are found below its frame. Only our own pid ever matches.
#[cfg(target_os = "linux")]
pub fn own_window() -> Option<u32> {
    use x11rb::{connection::Connection, protocol::xproto::{AtomEnum, ConnectionExt}, rust_connection::RustConnection};
    let (conn, screen) = RustConnection::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let pid_atom = conn.intern_atom(false, b"_NET_WM_PID").ok()?.reply().ok()?.atom;
    let me = std::process::id();
    let mut level = vec![root];
    for _ in 0..4 {
        let mut next = vec![];
        for w in level {
            let Some(tree) = conn.query_tree(w).ok().and_then(|c| c.reply().ok()) else { continue };
            for child in tree.children {
                let pid = conn.get_property(false, child, pid_atom, AtomEnum::CARDINAL, 0, 1).ok().and_then(|c| c.reply().ok()).and_then(|r| r.value32().and_then(|mut v| v.next()));
                if pid == Some(me) { return Some(child) }
                next.push(child);
            }
        }
        level = next;
    }
    None
}
#[cfg(not(target_os = "linux"))]
pub fn own_window() -> Option<u32> { None }

/// One frame of the window, as a PNG.
pub fn shot_args(xid: u32, display: &str, out: &Path) -> Vec<String> {
    ["-y", "-hide_banner", "-loglevel", "error", "-f", "x11grab", "-draw_mouse", "0", "-window_id", &xid.to_string(), "-i", display, "-frames:v", "1"].iter().map(|s| s.to_string())
        .chain([out.to_string_lossy().into_owned()]).collect()
}
/// The recording: even sides (VP8 wants them), 15 fps, no audio, 120 s, under 9 MiB.
pub fn record_args(xid: u32, display: &str, out: &Path) -> Vec<String> {
    ["-y", "-hide_banner", "-loglevel", "error", "-f", "x11grab", "-draw_mouse", "0", "-framerate", &FPS.to_string(), "-window_id", &xid.to_string(), "-i", display, "-an",
     "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-pix_fmt", "yuv420p", "-c:v", "libvpx", "-b:v", "512k", "-deadline", "realtime", "-cpu-used", "8", "-t", &MAX_SECS.to_string(), "-fs", &FILE_CAP.to_string()]
        .iter().map(|s| s.to_string()).chain([out.to_string_lossy().into_owned()]).collect()
}

/// The PNG bytes' signature and a sane size, so a failed grab is never offered as a screenshot.
pub fn is_png(bytes: &[u8]) -> bool { bytes.len() > 100 && bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) }
/// A finished WebM: the EBML header, not empty, within the cap.
pub fn is_webm(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 4];
    let size = std::fs::metadata(path).map_or(0, |m| m.len());
    size > 0 && size <= MAX_BYTES && std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head == [0x1a, 0x45, 0xdf, 0xa3]
}

/// Take the screenshot now (blocking, a second or so: call it off the UI thread, with the panel hidden).
pub fn shot(xid: u32, out: &Path) -> Result<Vec<u8>, String> {
    let (bin, disp) = (ffmpeg()?, display()?);
    let _ = std::fs::remove_file(out);
    /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ let status = Command::new(bin).args(shot_args(xid, &disp, out)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(|_| "ffmpeg did not start.".to_string())?;
    let bytes = std::fs::read(out).unwrap_or_default();
    /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ if !status.success() || !is_png(&bytes) { let _ = std::fs::remove_file(out); return Err("The window could not be captured. Capture again or choose an image.".into()); }
    Ok(bytes)
}

/// A recording in progress: ffmpeg writing the draft folder's file. `q` on its stdin finishes the file properly.
pub struct Recording { child: Child, pub path: PathBuf, pub started: Instant }
impl Recording {
    pub fn start(xid: u32, out: &Path) -> Result<Self, String> {
        let (bin, disp) = (ffmpeg()?, display()?);
        let _ = std::fs::remove_file(out);
        let log = std::fs::File::create(out.with_extension("log")).map_or_else(|_| Stdio::null(), Stdio::from);
        /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ let child = Command::new(bin).args(record_args(xid, &disp, out)).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(log).spawn().map_err(|_| "ffmpeg did not start.".to_string())?;
        Ok(Self { child, path: out.to_path_buf(), started: Instant::now() })
    }
    pub fn elapsed(&self) -> Duration { self.started.elapsed() }
    pub fn bytes(&self) -> u64 { std::fs::metadata(&self.path).map_or(0, |m| m.len()) }
    /// Whether ffmpeg is still writing (it ends itself at the time or size cap).
    pub fn running(&mut self) -> bool { matches!(self.child.try_wait(), Ok(None)) }
    /// Finish the file: ask ffmpeg to stop, wait up to 5 s, then end it. Ok with the clip's length if a whole WebM resulted.
    pub fn stop(mut self) -> Result<u64, String> {
        use std::io::Write;
        if let Some(mut i) = self.child.stdin.take() { let _ = i.write_all(b"q\n"); }
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end { if !matches!(self.child.try_wait(), Ok(None)) { break; } std::thread::sleep(Duration::from_millis(50)); }
        if matches!(self.child.try_wait(), Ok(None)) { let _ = self.child.kill(); let _ = self.child.wait(); }
        let ms = self.started.elapsed().as_millis() as u64;
        /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ if is_webm(&self.path) { Ok(ms.min(MAX_SECS as u64 * 1000)) } else { let _ = std::fs::remove_file(&self.path); Err(self.why()) }
    }
    /// Why it failed: ffmpeg's last line when it said one, else the usual reasons.
    fn why(&self) -> String {
        let last = std::fs::read_to_string(self.path.with_extension("log")).ok().and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().chars().take(160).collect::<String>()));
        /* i18n-ok: English here, in the catalog for where improvements.rs draws it */ match last { Some(l) => format!("Recording failed: {l}"), None => "Recording failed or went over the size limit. Record a shorter clip.".into() }
    }
    /// Cancel: end ffmpeg and delete whatever it wrote.
    pub fn cancel(mut self) { let _ = self.child.kill(); let _ = self.child.wait(); let _ = std::fs::remove_file(&self.path); }
}
impl Drop for Recording { fn drop(&mut self) { if matches!(self.child.try_wait(), Ok(None)) { let _ = self.child.kill(); let _ = self.child.wait(); } } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_commands_grab_only_this_window_with_no_audio_and_the_documented_limits() {
        let a = record_args(77, ":61", Path::new("/d/rec.webm")).join(" ");
        for want in ["-f x11grab", "-window_id 77", "-i :61", "-framerate 15", "-an", "-c:v libvpx", "-t 120", "-fs 9306112", "/d/rec.webm"] { assert!(a.contains(want), "{want}: {a}"); }
        assert!(!a.contains("-video_size") && !a.contains("-i :61.0+"), "no root geometry"); assert!(!a.contains("pulse") && !a.contains("alsa"), "no audio device");
        let s = shot_args(77, ":61", Path::new("/d/s.png")).join(" "); assert!(s.contains("-window_id 77") && s.contains("-frames:v 1") && s.ends_with("/d/s.png"));
    }
    #[test] fn a_file_is_a_screenshot_or_a_recording_only_by_its_bytes() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]; png.extend(vec![0; 200]); assert!(is_png(&png)); assert!(!is_png(b"\x89PNG") && !is_png(&vec![0; 300]));
        let d = std::env::temp_dir().join(format!("bp-cap-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let ok = d.join("a.webm"); std::fs::write(&ok, [0x1a, 0x45, 0xdf, 0xa3, 1, 2, 3]).unwrap(); assert!(is_webm(&ok));
        let bad = d.join("b.webm"); std::fs::write(&bad, b"nope").unwrap(); assert!(!is_webm(&bad)); assert!(!is_webm(&d.join("missing")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
