//! Where SolidCraft's programs are, and the install of the pinned release.
//!
//! Found, in order: `BACKPLANE_SOLIDCRAFT` (a folder holding the programs, or its `bin`: a rig's or a developer's), the managed installs
//! under [`parent`] newest first, the hub's PATH, and the places the upstream packages put them. The window program (`solidcraft`) and
//! the command-line one (`solidcraft-cli`: renders, measurements, the MCP server) are looked for one by one, so a machine with only the
//! CLI (a Boat, a Mac with the CLI zip) still renders and serves the agent's tools.
//!
//! The install downloads the pinned release asset from GitHub with the system's own downloader (`kicad::install`'s tools, through
//! proc.rs), checks it against the digest pinned here and the release's SHA256SUMS, unpacks it into a stage beside the destination, checks
//! the archive's member names, its links and that the programs are there, and only then renames it into `<parent>/<version>`. Nothing
//! privileged runs and nothing outside that folder is written. `solidcraft.install` (info) says how it goes; `solidcraft.runtime` what
//! is found.
use crate::{hub::Hub, files::kicad::install as ki};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Arc, OnceLock}};

/// The release this Backplane is tested with.
pub const VERSION: &str = "0.50.0";
/// Where the releases are (said in the consent note, and the download's base).
pub const FROM: &str = "github.com/tangency-labs/solidcraft";

/// One release asset: its platform, name, pinned digest, how it is packed, and where its programs are inside its root folder
/// ("" where the asset has none: macOS's zip holds the CLI only; its window program ships as a disk image the hub does not unpack).
#[derive(Debug, PartialEq)]
pub struct Pin { pub platform: &'static str, pub asset: &'static str, pub root: &'static str, pub sha256: &'static str, pub format: &'static str, pub gui: &'static str, pub cli: &'static str, pub size: u64 }

pub const LINUX_X64: Pin = Pin { platform: "linux-x86_64", asset: "solidcraft-0.50.0-linux-x86_64.tar.gz", root: "solidcraft-0.50.0-linux-x86_64", sha256: "b0c63f45fe58ab13c40145132b21fdc309f45e294301f1ba525050aa99575920", format: "tar.gz", gui: "bin/solidcraft", cli: "bin/solidcraft-cli", size: 228331854 };
pub const LINUX_ARM64: Pin = Pin { platform: "linux-arm64", asset: "solidcraft-0.50.0-linux-aarch64.tar.gz", root: "solidcraft-0.50.0-linux-aarch64", sha256: "5a5c6e87533f70de2ad0fcc23c7b36f45144ccf9ded77ff0099cbbea39b4c337", format: "tar.gz", gui: "bin/solidcraft", cli: "bin/solidcraft-cli", size: 207384338 };
pub const WINDOWS_X64: Pin = Pin { platform: "windows-x86_64", asset: "solidcraft-0.50.0-windows-x64-portable.zip", root: "solidcraft-0.50.0-windows-x64-portable", sha256: "f9b5be186e966e839af6b1a9bf32dfd0e0663cace9ba91f091ef8d99893a965a", format: "zip", gui: "solidcraft.exe", cli: "solidcraft-cli.exe", size: 230169768 };
pub const MAC: Pin = Pin { platform: "darwin-universal", asset: "solidcraft-cli-0.50.0-macos-universal.zip", root: "solidcraft-cli-0.50.0-macos-universal", sha256: "05c16524e1efd1dd74b3b0e7564c6ef92b243e7e7afe79c15717fac4750efcd8", format: "zip", gui: "", cli: "solidcraft-cli", size: 205430085 };
pub const PINS: &[&Pin] = &[&LINUX_X64, &LINUX_ARM64, &WINDOWS_X64, &MAC];

/// The asset for an OS and architecture (Rust's names or uname's); None where upstream publishes none the hub can unpack.
pub fn pick(os: &str, arch: &str) -> Option<&'static Pin> {
    let (os, arch) = (os.to_lowercase(), arch.to_lowercase());
    let arch = match arch.as_str() { "amd64" | "x64" => "x86_64", "arm64" => "aarch64", a => a };
    match (os.as_str(), arch) {
        ("linux", "x86_64") => Some(&LINUX_X64), ("linux", "aarch64") => Some(&LINUX_ARM64),
        ("windows", "x86_64") => Some(&WINDOWS_X64), ("macos" | "darwin", _) => Some(&MAC), _ => None,
    }
}
/// This hub's asset.
pub fn here() -> Option<&'static Pin> { pick(std::env::consts::OS, std::env::consts::ARCH) }
/// A pin's fields as the install trusts them: a one-line change is checked before anything is downloaded.
pub fn check_pin(p: &Pin) -> Result<(), String> {
    let name = |s: &str| !s.is_empty() && s.len() < 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
    if !name(p.asset) || !name(p.root) || !p.asset.contains(VERSION) || !p.asset.ends_with(&format!(".{}", p.format)) { return Err("Invalid SolidCraft archive name".into()) }
    if p.sha256.len() != 64 || !p.sha256.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) { return Err("Invalid pinned digest".into()) }
    if !matches!(p.format, "tar.gz" | "zip") || p.cli.is_empty() { return Err("Invalid SolidCraft package".into()) }
    Ok(())
}
/// Where the download comes from. `BACKPLANE_SOLIDCRAFT_FEED` (a rig's, never a setting a client can send): an http(s) folder holding the
/// pinned asset and SHA256SUMS.txt, used instead of the release's; the pin is checked against it as against the release.
pub fn base_url(feed: Option<&str>) -> String {
    match feed.map(str::trim).filter(|f| f.starts_with("http://") || f.starts_with("https://")) {
        Some(f) => format!("{}/", f.trim_end_matches('/')),
        None => format!("https://{FROM}/releases/download/v{VERSION}/"),
    }
}

/// Where managed installs go: `<home>/solidcraft`; on Windows `%LOCALAPPDATA%\Backplane\solidcraft` (the home is in the roaming profile,
/// and 700 MB of programs do not belong there). Machine-local on every system: never part of what homes share.
pub fn parent(home: &Path) -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA");
    #[cfg(test)] let local = local.filter(|_| false); // a test never touches the machine's own folder
    match local.filter(|d| cfg!(windows) && !d.is_empty()) { Some(d) => Path::new(&d).join("Backplane").join("solidcraft"), None => home.join("solidcraft") }
}

/// What is found on this machine.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Runtime {
    /// The window program, when there is one (the pane needs it).
    pub gui: Option<PathBuf>,
    /// The command-line program (renders, measurements, MCP).
    pub cli: Option<PathBuf>,
    /// "managed" (installed by Backplane), "env" (`BACKPLANE_SOLIDCRAFT`), "system" (PATH or a package's place), "" when nothing is found.
    pub source: &'static str,
    /// The managed install's version ("" for the others: no program is started to ask).
    pub version: String,
}
impl Runtime {
    pub fn found(&self) -> bool { self.gui.is_some() || self.cli.is_some() }
    pub fn to_json(&self) -> Value {
        let s = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        json!({"installed": self.found(), "gui": s(&self.gui), "cli": s(&self.cli), "path": s(&self.gui.clone().or(self.cli.clone())), "source": self.source, "version": self.version})
    }
}
fn exe(name: &str, windows: bool) -> String { if windows { format!("{name}.exe") } else { name.to_string() } }
/// A program in `dir` (as the Linux archive lays it out, `bin/`; as the portable zip and the CLI zip do, flat).
fn in_dir(dir: &Path, name: &str, windows: bool) -> Option<PathBuf> {
    [dir.join("bin").join(exe(name, windows)), dir.join(exe(name, windows))].into_iter().find(|f| f.is_file())
}
/// Version folder names, newest first ("0.50.0" over "0.49.1" over "0.9.0"; names that are not versions last, by name).
pub fn newest_first(mut names: Vec<String>) -> Vec<String> {
    let key = |n: &str| n.split(['.', '-']).map(|x| x.parse::<u64>().ok()).collect::<Option<Vec<u64>>>();
    names.sort_by(|a, b| match (key(a), key(b)) { (Some(x), Some(y)) => y.cmp(&x), (Some(_), None) => std::cmp::Ordering::Less, (None, Some(_)) => std::cmp::Ordering::Greater, _ => a.cmp(b) });
    names
}
/// The places upstream's packages put the programs, after PATH.
pub fn system_dirs(windows: bool, macos: bool, user_home: Option<&Path>, program_files: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if windows { if let Some(p) = program_files { v.push(p.join("SolidCraft")); } return v }
    if macos { v.push("/Applications/SolidCraft.app/Contents/MacOS".into()); if let Some(h) = user_home { v.push(h.join("Applications/SolidCraft.app/Contents/MacOS")); } v.push("/opt/homebrew/bin".into()); }
    v.extend(["/usr/local/bin", "/usr/bin", "/opt/solidcraft/bin"].map(PathBuf::from));
    if let Some(h) = user_home { v.push(h.join(".local/bin")); }
    v
}
/// The search, pure but for reading folders: `over` (`BACKPLANE_SOLIDCRAFT`), the managed `parent`, then `dirs` (PATH and the packages'
/// places). The first place holding a program gives it; the window program and the CLI may come from different places only when the
/// earlier place has just one of them.
pub fn find_in(over: Option<&Path>, parent: &Path, dirs: &[PathBuf], windows: bool) -> Runtime {
    let mut r = Runtime::default();
    let take = |r: &mut Runtime, dir: &Path, source: &'static str, version: &str| {
        let (g, c) = (in_dir(dir, "solidcraft", windows), in_dir(dir, "solidcraft-cli", windows));
        if (g.is_some() && r.gui.is_none()) || (c.is_some() && r.cli.is_none()) {
            if r.source.is_empty() { r.source = source; r.version = version.to_string(); }
            if r.gui.is_none() { r.gui = g; }
            if r.cli.is_none() { r.cli = c; }
        }
    };
    if let Some(o) = over { take(&mut r, o, "env", ""); }
    let versions: Vec<String> = std::fs::read_dir(parent).into_iter().flatten().flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
    for v in newest_first(versions) { take(&mut r, &parent.join(&v), "managed", &v); }
    for d in dirs { if r.gui.is_some() && r.cli.is_some() { break } take(&mut r, d, "system", ""); }
    r
}
static HOME: OnceLock<PathBuf> = OnceLock::new();
#[cfg(test)] pub(crate) fn test_over() -> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, PathBuf>> { static O: OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, PathBuf>>> = OnceLock::new(); O.get_or_init(Default::default) }
/// What this machine has, read now (a few folder reads; nothing is started).
pub fn find(home: &Path) -> Runtime {
    let over = std::env::var_os("BACKPLANE_SOLIDCRAFT").filter(|v| !v.is_empty()).map(PathBuf::from);
    // a test names its stand-in programs for its own home (the environment is every test's at once)
    #[cfg(test)] let over = test_over().lock().unwrap().get(home).cloned();
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).filter(|d| !d.as_os_str().is_empty()).collect();
    let user = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    dirs.extend(system_dirs(cfg!(windows), cfg!(target_os = "macos"), user.as_deref(), std::env::var_os("ProgramFiles").map(PathBuf::from).as_deref()));
    // a test hub finds only what it is given (`BACKPLANE_SOLIDCRAFT`) or installs: never the developer's own SolidCraft
    #[cfg(test)] dirs.clear();
    find_in(over.as_deref(), &parent(home), &dirs, cfg!(windows))
}
/// The running hub's runtime (None before `start`).
pub fn current() -> Option<Runtime> { HOME.get().map(|h| find(h)) }
/// `solidcraft-cli` on this machine: the agent's MCP server, renders and measurements.
pub fn cli_path() -> Option<PathBuf> { current().and_then(|r| r.cli) }
/// `solidcraft` (the window program) on this machine.
pub fn gui_path() -> Option<PathBuf> { current().and_then(|r| r.gui) }

/// The install's offer as the pane and Settings say it.
pub fn offer(p: Option<&Pin>, folder: &Path) -> Value {
    match p {
        Some(p) => json!({"available": true, "version": VERSION, "size": p.size, "from": FROM, "folder": folder.to_string_lossy(), "window": !p.gui.is_empty(),
            "note": format!("SolidCraft {VERSION} ({} MB) is downloaded from {FROM} into {}", (p.size + (1 << 19)) >> 20, folder.display())}),
        None => json!({"available": false, "version": VERSION, "note": format!("No SolidCraft download is published for this computer ({} {})", std::env::consts::OS, std::env::consts::ARCH)}),
    }
}
/// Info `solidcraft.runtime`: what is found, and the offer.
pub async fn publish(hub: &Arc<Hub>) -> Runtime {
    let home = hub.cfg.home.clone();
    let r = tokio::task::spawn_blocking(move || find(&home)).await.unwrap_or_default();
    let mut v = r.to_json(); v["offer"] = offer(here(), &parent(&hub.cfg.home));
    hub.set_info("solidcraft.runtime", &v.to_string()).await;
    r
}
pub fn start(hub: &Arc<Hub>) {
    let _ = HOME.set(hub.cfg.home.clone());
    let hub = hub.clone();
    tokio::spawn(async move { publish(&hub).await; });
}

// ---- The install ----------------------------------------------------------------------------------------------------------------

static RUNNING: AtomicBool = AtomicBool::new(false);
pub fn running() -> bool { RUNNING.load(Ordering::SeqCst) }
struct Run;
impl Drop for Run { fn drop(&mut self) { RUNNING.store(false, Ordering::SeqCst) } }

/// Why an install is refused, None when it may start.
pub fn refusal(consent: bool, running: bool, found: &Runtime, p: Option<&Pin>, copy: bool) -> Option<String> {
    if copy { return Some("This is a copy of a hub: it installs nothing".into()) }
    if !consent { return Some("Installing SolidCraft needs the person's consent".into()) }
    if running { return Some("SolidCraft is already being installed".into()) }
    let Some(p) = p else { return Some(offer(None, Path::new(""))["note"].as_str().unwrap_or("").to_string()) };
    // what the package brings is already there: nothing to do (a system CLI without the window program still gets the package's window)
    if found.cli.is_some() && (found.gui.is_some() || p.gui.is_empty()) { return Some("SolidCraft is already installed".into()) }
    None
}
fn sc(e: String) -> String { e.replace("Backplane KiCad", "SolidCraft").replace("KiCad", "SolidCraft") }
/// The stage's unpacked root checked and moved to `<parent>/<version>`: every link inside it, the programs there and runnable.
pub fn place(unpack: &Path, parent: &Path, p: &Pin) -> Result<PathBuf, String> {
    let root = unpack.join(p.root);
    if !root.is_dir() { return Err("SolidCraft archive layout is not the expected one".into()) }
    ki::links_inside(unpack, &root).map_err(sc)?;
    for rel in [p.cli, p.gui].into_iter().filter(|r| !r.is_empty()) {
        let f = root.join(rel);
        if !std::fs::metadata(&f).is_ok_and(|m| m.is_file() && m.len() > 0) { return Err(format!("SolidCraft archive is incomplete: {rel} is missing")) }
        if p.platform.starts_with("windows") {
            let mut mz = [0u8; 2]; let _ = std::io::Read::read(&mut std::fs::File::open(&f).map_err(|e| e.to_string())?, &mut mz);
            if mz != *b"MZ" { return Err(format!("SolidCraft's {rel} is not a Windows program")) }
        } else {
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let m = std::fs::metadata(&f).map_err(|e| e.to_string())?.permissions().mode(); let _ = std::fs::set_permissions(&f, std::fs::Permissions::from_mode(m | 0o755)); }
        }
    }
    let dir = parent.join(VERSION);
    if dir.symlink_metadata().is_ok() { return Err(format!("{} already exists; the installation there is kept", dir.display())) }
    std::fs::rename(&root, &dir).map_err(|e| format!("Could not move SolidCraft into {}: {e}", dir.display()))?;
    Ok(dir)
}
/// The archive checked against the pin and the release's sums, then unpacked into `stage/unpack`.
async fn unpack(t: &ki::Tools, pkg: &Path, sums: &str, stage: &Path, p: &Pin, log: &Path) -> Result<PathBuf, String> {
    let want = ki::sum_for(sums, p.asset).unwrap_or_default();
    let pk = pkg.to_path_buf(); let got = tokio::task::spawn_blocking(move || ki::sha256_file(&pk)).await.map_err(|e| e.to_string())??;
    if want != got || got != p.sha256 { let _ = std::fs::remove_file(pkg); return Err("SolidCraft archive checksum mismatch (the download was removed; Try again downloads it anew)".into()) }
    let unpack = stage.join("unpack"); std::fs::create_dir(&unpack).map_err(|e| e.to_string())?;
    if p.format == "zip" {
        let (z, r, u) = (pkg.to_path_buf(), p.root.to_string(), unpack.clone());
        tokio::task::spawn_blocking(move || ki::unzip(&z, &r, &u)).await.map_err(|e| e.to_string())?.map_err(sc)?;
    } else {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let members = ki::tool(&t.tar, &s(&["-tzf", &pkg.to_string_lossy()]), log, || {}).await.map_err(|e| format!("Could not read the SolidCraft archive: {e}"))?;
        ki::members_ok(&members, p.root, false).map_err(sc)?;
        ki::tool(&t.tar, &s(&["-xzf", &pkg.to_string_lossy(), "-C", &unpack.to_string_lossy()]), log, || {}).await.map_err(|e| format!("Could not unpack SolidCraft: {e}"))?;
    }
    Ok(unpack)
}
struct Stage(PathBuf);
impl Drop for Stage { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
/// Download, check, unpack, place. `say(state words, bytes done, bytes in all)` as it goes. Ok(the installed folder).
pub async fn fetch(base: &str, parent: &Path, p: &'static Pin, mut say: impl FnMut(&str, u64, u64)) -> Result<PathBuf, String> {
    check_pin(p)?;
    let t = ki::tools().map_err(sc)?;
    std::fs::create_dir_all(parent.join(".partial")).map_err(|e| format!("Could not make {}: {e}", parent.display()))?;
    let log = parent.join(".install.log");
    let stage = Stage(parent.join(format!(".install-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()))));
    std::fs::create_dir(&stage.0).map_err(|e| e.to_string())?;
    let pkg = parent.join(".partial").join(p.asset);
    let gp = match &t.get { ki::Getter::Curl(x) | ki::Getter::Wget(x) => x.clone() };
    let have = |f: &Path| std::fs::metadata(f).map_or(0, |m| m.len());
    // an http feed is a rig's on loopback: the downloader's https-only arguments are for the release
    let args = |url: &str, to: &Path| { let a = ki::get_args(&t.get, url, to); if url.starts_with("http://") { a.into_iter().filter(|x| !matches!(x.as_str(), "--proto" | "=https" | "--tlsv1.2" | "--https-only")).collect() } else { a } };
    say("Downloading", have(&pkg), p.size);
    if have(&pkg) != p.size {
        if have(&pkg) > p.size { let _ = std::fs::remove_file(&pkg); }
        let mut ok = Err(String::new());
        for _ in 0..3 {
            ok = ki::tool(&gp, &args(&format!("{base}{}", p.asset), &pkg), &log, || say("Downloading", have(&pkg), p.size)).await;
            if ok.is_ok() { break }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        if let Err(why) = ok { return Err(format!("The download stopped ({})", if why.is_empty() { "no answer".to_string() } else { why })) }
    }
    let sums = stage.0.join("SUMS");
    ki::tool(&gp, &args(&format!("{base}SHA256SUMS.txt"), &sums), &log, || {}).await.map_err(|_| format!("Could not download {base}SHA256SUMS.txt"))?;
    say("Checking", p.size, p.size);
    let un = unpack(&t, &pkg, &std::fs::read_to_string(&sums).unwrap_or_default(), &stage.0, p, &log).await?;
    say("Unpacking", p.size, p.size);
    let (u, par) = (un.clone(), parent.to_path_buf());
    let dir = tokio::task::spawn_blocking(move || place(&u, &par, p)).await.map_err(|e| e.to_string())??;
    let _ = std::fs::remove_file(&pkg); let _ = std::fs::remove_dir(parent.join(".partial"));
    Ok(dir)
}
async fn say_state(hub: &Arc<Hub>, state: &str, note: &str, done: u64, total: u64) {
    hub.set_info("solidcraft.install", &json!({"state": state, "note": note, "version": VERSION, "done": done, "total": total}).to_string()).await;
}
/// `editor.solidcraft.install {consent:true}`: starts the install and returns at once; info `solidcraft.install` follows it
/// (`running` with bytes, then `done` or `failed` with why), and `solidcraft.runtime` says the programs when it is done.
pub async fn install(hub: &Arc<Hub>, consent: bool) -> Result<String, String> {
    let found = publish(hub).await;
    let p = here();
    if let Some(why) = refusal(consent, running(), &found, p, crate::copy::on()) { return Err(why) }
    let p = p.ok_or("no package")?;
    if RUNNING.swap(true, Ordering::SeqCst) { return Err("SolidCraft is already being installed".into()) }
    let run = Run;
    say_state(hub, "running", "Downloading", 0, p.size).await;
    let hub = hub.clone();
    tokio::spawn(async move {
        let _run = run;
        let par = parent(&hub.cfg.home);
        let base = base_url(std::env::var("BACKPLANE_SOLIDCRAFT_FEED").ok().as_deref());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, u64, u64)>();
        let h2 = hub.clone();
        let teller = tokio::spawn(async move {
            let mut last = (String::new(), 0u64, std::time::Instant::now());
            while let Some((s, d, t)) = rx.recv().await {
                // a line a second while bytes move, and every change of step
                if s != last.0 || (d != last.1 && last.2.elapsed().as_millis() >= 900) { say_state(&h2, "running", &s, d, t).await; last = (s, d, std::time::Instant::now()); }
            }
        });
        let r = fetch(&base, &par, p, |s, d, t| { let _ = tx.send((s.to_string(), d, t)); }).await;
        drop(tx); let _ = teller.await;
        match r {
            Ok(dir) => { eprintln!("solidcraft: installed in {}", dir.display()); publish(&hub).await; say_state(&hub, "done", "", p.size, p.size).await; }
            Err(why) => { eprintln!("solidcraft: install failed: {why}"); say_state(&hub, "failed", &why, 0, p.size).await; }
        }
    });
    Ok(format!("Installing SolidCraft {VERSION}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn touch(f: &Path) { std::fs::create_dir_all(f.parent().unwrap()).unwrap(); std::fs::write(f, "#!/bin/sh\n").unwrap(); }

    #[test]
    fn pins_are_sound_and_picked() {
        for p in PINS { check_pin(p).unwrap(); assert!(p.root.contains(VERSION) && p.size > 100 << 20, "{}", p.asset); }
        assert_eq!(pick("linux", "x86_64"), Some(&LINUX_X64)); assert_eq!(pick("Linux", "aarch64"), Some(&LINUX_ARM64)); assert_eq!(pick("linux", "arm64"), Some(&LINUX_ARM64));
        assert_eq!(pick("windows", "amd64"), Some(&WINDOWS_X64)); assert_eq!(pick("macos", "aarch64"), Some(&MAC)); assert_eq!(pick("darwin", "x86_64"), Some(&MAC));
        assert_eq!(pick("windows", "aarch64"), None); assert_eq!(pick("freebsd", "x86_64"), None);
        assert!(MAC.gui.is_empty() && !LINUX_X64.gui.is_empty());
        let bad = Pin { sha256: "xyz", ..LINUX_X64 }; assert!(check_pin(&bad).is_err());
        let bad = Pin { asset: "../x-0.50.0.tar.gz", ..LINUX_X64 }; assert!(check_pin(&bad).is_err());
    }
    #[test]
    fn download_base() {
        assert_eq!(base_url(None), "https://github.com/tangency-labs/solidcraft/releases/download/v0.50.0/");
        assert_eq!(base_url(Some("http://127.0.0.1:9/feed/")), "http://127.0.0.1:9/feed/");
        assert_eq!(base_url(Some("file:///etc")), base_url(None), "only http(s) feeds");
    }
    #[test]
    fn versions_newest_first() {
        assert_eq!(newest_first(vec!["0.9.0".into(), "0.50.0".into(), "old".into(), "0.49.1".into()]), vec!["0.50.0", "0.49.1", "0.9.0", "old"]);
    }
    #[test]
    fn found_in_order() {
        let t = crate::plat::tmp::dir("sc-find"); let d = t.to_path_buf();
        let (parent, sys, over) = (d.join("managed"), d.join("sys"), d.join("over"));
        assert_eq!(find_in(None, &parent, &[sys.clone()], false), Runtime::default());
        // the system has both: found there
        touch(&sys.join("solidcraft")); touch(&sys.join("solidcraft-cli"));
        let r = find_in(None, &parent, &[sys.clone()], false);
        assert_eq!((r.source, r.gui.clone(), r.cli.clone()), ("system", Some(sys.join("solidcraft")), Some(sys.join("solidcraft-cli"))));
        assert!(r.found() && r.to_json()["installed"] == true);
        // a managed install wins, the newest of them, and says its version
        touch(&parent.join("0.9.0/bin/solidcraft")); touch(&parent.join("0.9.0/bin/solidcraft-cli"));
        touch(&parent.join("0.50.0/bin/solidcraft")); touch(&parent.join("0.50.0/bin/solidcraft-cli"));
        std::fs::create_dir_all(parent.join(".partial")).unwrap();
        let r = find_in(None, &parent, &[sys.clone()], false);
        assert_eq!((r.source, r.version.as_str(), r.gui.clone()), ("managed", "0.50.0", Some(parent.join("0.50.0/bin/solidcraft"))));
        // the override first; with only the CLI there, the window program comes from the next place that has one
        touch(&over.join("solidcraft-cli"));
        let r = find_in(Some(&over), &parent, &[sys.clone()], false);
        assert_eq!((r.source, r.cli.clone(), r.gui.clone()), ("env", Some(over.join("solidcraft-cli")), Some(parent.join("0.50.0/bin/solidcraft"))));
        // Windows' names
        touch(&d.join("win/solidcraft.exe"));
        assert_eq!(find_in(None, &d.join("none"), &[d.join("win")], true).gui, Some(d.join("win/solidcraft.exe")));
        assert_eq!(find_in(None, &d.join("none"), &[d.join("win")], false).gui, None);
    }
    #[test]
    fn refusals() {
        let none = Runtime::default();
        let both = Runtime { gui: Some("g".into()), cli: Some("c".into()), source: "system", version: String::new() };
        let cli = Runtime { gui: None, cli: Some("c".into()), source: "system", version: String::new() };
        assert!(refusal(false, false, &none, Some(&LINUX_X64), false).unwrap().contains("consent"));
        assert!(refusal(true, true, &none, Some(&LINUX_X64), false).unwrap().contains("already being"));
        assert!(refusal(true, false, &none, None, false).unwrap().contains("No SolidCraft download"));
        assert!(refusal(true, false, &none, Some(&LINUX_X64), true).unwrap().contains("copy"));
        assert!(refusal(true, false, &both, Some(&LINUX_X64), false).unwrap().contains("already installed"));
        assert_eq!(refusal(true, false, &none, Some(&LINUX_X64), false), None);
        assert_eq!(refusal(true, false, &cli, Some(&LINUX_X64), false), None, "the package brings the window program a CLI-only machine lacks");
        assert!(refusal(true, false, &cli, Some(&MAC), false).is_some(), "macOS's package is the CLI alone");
    }
    #[test]
    fn placed_only_when_whole() {
        let t = crate::plat::tmp::dir("sc-place"); let d = t.to_path_buf();
        let (un, parent) = (d.join("unpack"), d.join("managed")); std::fs::create_dir_all(&parent).unwrap();
        assert!(place(&un, &parent, &LINUX_X64).is_err(), "no root");
        touch(&un.join(LINUX_X64.root).join("bin/solidcraft-cli"));
        assert!(place(&un, &parent, &LINUX_X64).unwrap_err().contains("bin/solidcraft is missing"));
        touch(&un.join(LINUX_X64.root).join("bin/solidcraft"));
        let dir = place(&un, &parent, &LINUX_X64).unwrap();
        assert_eq!(dir, parent.join(VERSION));
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(dir.join("bin/solidcraft-cli")).unwrap().permissions().mode() & 0o111, 0o111); }
        assert_eq!(find_in(None, &parent, &[], false).version, VERSION);
        // a second install never lands over the first
        touch(&un.join(LINUX_X64.root).join("bin/solidcraft-cli")); touch(&un.join(LINUX_X64.root).join("bin/solidcraft"));
        assert!(place(&un, &parent, &LINUX_X64).unwrap_err().contains("already exists"));
        // a link out of the runtime is refused
        #[cfg(unix)] {
            let un2 = d.join("unpack2"); touch(&un2.join(LINUX_ARM64.root).join("bin/solidcraft-cli")); touch(&un2.join(LINUX_ARM64.root).join("bin/solidcraft"));
            std::os::unix::fs::symlink("/etc/passwd", un2.join(LINUX_ARM64.root).join("bin/out")).unwrap();
            assert!(place(&un2, &d.join("m2"), &LINUX_ARM64).is_err());
        }
        // Windows' programs must be programs
        let un3 = d.join("unpack3"); touch(&un3.join(WINDOWS_X64.root).join("solidcraft-cli.exe")); touch(&un3.join(WINDOWS_X64.root).join("solidcraft.exe"));
        std::fs::create_dir_all(d.join("m3")).unwrap();
        assert!(place(&un3, &d.join("m3"), &WINDOWS_X64).unwrap_err().contains("not a Windows program"));
    }
    #[test]
    fn offers() {
        let o = offer(Some(&LINUX_X64), Path::new("/home/user/.backplane/solidcraft"));
        assert_eq!(o["available"], true); assert_eq!(o["window"], true); assert!(o["note"].as_str().unwrap().contains("218 MB"));
        assert_eq!(offer(Some(&MAC), Path::new("/x"))["window"], false);
        assert_eq!(offer(None, Path::new("/x"))["available"], false);
    }
}
