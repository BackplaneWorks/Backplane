//! Pictures of a model with no window: `solidcraft-cli snapshot --in <file> --out <png> --view <name> --width W --height H`.
//! One process per view, each bounded ([`LIMIT`]): a kernel that stalls on a file is killed and the caller falls back to the hub's
//! own mesher (`kicad::vrender`). The CLI never gets a display (it draws off screen), so nothing appears on the person's screen.
use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};

/// The longest one view may take (a 76 MB assembly's import is tens of seconds; past this the kernel is taken as stalled).
pub const LIMIT: Duration = Duration::from_secs(180);
/// The views `snapshot --view` takes by name (the hub's mech views are the same seven).
pub const VIEWS: &[&str] = &["iso", "front", "back", "top", "bottom", "left", "right"];

/// The arguments for one view.
pub fn args(src: &Path, out: &Path, view: &str, w: u32, h: u32) -> Vec<String> {
    let s = |p: &Path| p.to_string_lossy().into_owned();
    vec!["snapshot".into(), "--in".into(), s(src), "--out".into(), s(out), "--view".into(), view.into(), "--width".into(), w.to_string(), "--height".into(), h.to_string()]
}
/// What the CLI said went wrong, in a line: its last `error:` line, else its last line.
pub fn failure(stderr: &str, code: Option<i32>) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let l = lines.iter().rev().find(|l| l.to_ascii_lowercase().starts_with("error")).or(lines.last()).copied().unwrap_or("");
    if l.is_empty() { format!("solidcraft-cli stopped ({})", code.map_or("killed".into(), |c| format!("exit {c}"))) } else { l.chars().take(300).collect() }
}
/// A PNG's first bytes: what a finished snapshot must start with.
pub fn is_png(f: &Path) -> bool { let mut b = [0u8; 8]; std::fs::File::open(f).and_then(|mut r| std::io::Read::read_exact(&mut r, &mut b)).is_ok() && b == *b"\x89PNG\r\n\x1a\n" }

/// One view of `src` written to `out` (through a part file, renamed when whole). Err says why, and leaves nothing behind.
pub async fn snapshot(cli: &Path, src: &Path, out: &Path, view: &str, w: u32, h: u32, limit: Duration) -> Result<(), String> {
    if !VIEWS.contains(&view) { return Err(format!("unknown view {view}")) }
    let part: PathBuf = out.with_extension(format!("part-{}.png", std::process::id()));
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, cli);
    c.args(args(src, &part, view, w, h)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped())
        .env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY").kill_on_drop(true);
    if let Some(d) = src.parent() { c.current_dir(d); }
    let child = c.spawn().map_err(|e| format!("could not start {}: {e}", cli.display()))?;
    let r = match tokio::time::timeout(limit, child.wait_with_output()).await {
        Err(_) => Err(format!("solidcraft-cli did not finish the {view} view within {} s and was stopped", limit.as_secs())), // dropped: killed
        Ok(Err(e)) => Err(e.to_string()),
        Ok(Ok(o)) if !o.status.success() => Err(failure(&String::from_utf8_lossy(&o.stderr), o.status.code())),
        Ok(Ok(_)) if !is_png(&part) => Err("solidcraft-cli wrote no picture".into()),
        Ok(Ok(_)) => crate::plat::fs::rename(&part, out).map_err(|e| format!("could not publish {}: {e}", out.display())),
    };
    if r.is_err() { let _ = std::fs::remove_file(&part); }
    r
}
/// Every view of `src` into `out_dir` as `<stem>.<view>.png`, in order. The first failure stops it (the caller falls back whole).
pub async fn views(cli: &Path, src: &Path, out_dir: &Path, stem: &str, views: &[String], w: u32, h: u32) -> Result<Vec<String>, String> {
    let mut paths = Vec::with_capacity(views.len());
    for v in views {
        let out = out_dir.join(format!("{stem}.{v}.png"));
        snapshot(cli, src, &out, v, w, h, LIMIT).await?;
        paths.push(out.to_string_lossy().into_owned());
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_and_failures() {
        assert_eq!(args(Path::new("/p/a.step"), Path::new("/o/a.iso.png"), "iso", 800, 600).join(" "), "snapshot --in /p/a.step --out /o/a.iso.png --view iso --width 800 --height 600");
        assert_eq!(failure("reading\nerror: not a STEP file\n  at line 1\n", Some(1)), "error: not a STEP file");
        assert_eq!(failure("something odd\n", Some(1)), "something odd");
        assert_eq!(failure("", Some(2)), "solidcraft-cli stopped (exit 2)");
        assert_eq!(failure("", None), "solidcraft-cli stopped (killed)");
    }
    /// A stand-in CLI: writes a PNG header where asked, fails for a file named bad, and never ends for one named slow.
    #[cfg(unix)]
    #[tokio::test]
    async fn snapshots_with_a_stand_in() {
        use std::os::unix::fs::PermissionsExt;
        let t = crate::plat::tmp::dir("sc-render"); let d = t.to_path_buf();
        let cli = d.join("solidcraft-cli");
        std::fs::write(&cli, "#!/bin/sh\n[ -z \"$DISPLAY\" ] || { echo 'error: has a display' >&2; exit 3; }\ncase \"$3\" in *bad*) echo 'error: cannot read' >&2; exit 1;; *slow*) exec sleep 30;; *empty*) exit 0;; esac\nprintf '\\211PNG\\r\\n\\032\\n' > \"$5\"\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        for n in ["a.step", "bad.step", "slow.step", "empty.step"] { std::fs::write(d.join(n), "x").unwrap(); }
        let got = views(&cli, &d.join("a.step"), &d, "a", &["iso".to_string(), "top".to_string()], 400, 300).await.unwrap();
        assert_eq!(got.len(), 2); assert!(is_png(&d.join("a.iso.png")) && is_png(&d.join("a.top.png")));
        assert_eq!(snapshot(&cli, &d.join("bad.step"), &d.join("b.png"), "iso", 400, 300, LIMIT).await.unwrap_err(), "error: cannot read");
        assert!(snapshot(&cli, &d.join("empty.step"), &d.join("e.png"), "iso", 400, 300, LIMIT).await.unwrap_err().contains("no picture"));
        assert!(snapshot(&cli, &d.join("a.step"), &d.join("x.png"), "sideways", 400, 300, LIMIT).await.unwrap_err().contains("unknown view"));
        let t0 = std::time::Instant::now();
        assert!(snapshot(&cli, &d.join("slow.step"), &d.join("s.png"), "iso", 400, 300, Duration::from_millis(300)).await.unwrap_err().contains("was stopped"));
        assert!(t0.elapsed() < Duration::from_secs(5));
        let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("part-")).collect();
        assert!(left.is_empty(), "{left:?}");
    }
}
