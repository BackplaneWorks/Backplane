//! Setup's one-press installs of what lives outside Backplane: the agent CLIs with their vendors' own installers (no Node), Codex as its standalone release
//! binary checked against the digest GitHub publishes for it, and Tailscale and FreeCAD through winget on Windows. `plan` is pure: tool, system, what is
//! already there -> what a press does. `start` runs a plan detached and says how it goes in the info key `onboard.installs` (`{tool: {state, note}}`).
//! Nothing here asks for administrator rights itself. winget's own installers may show Windows' permission prompt once; the step says so beside the button.
//! `kicad.install` (kicad/install.rs) is the same for Backplane KiCad.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::{Path, PathBuf}, process::Stdio, sync::{Arc, Mutex}};

/// What a press on a tool's Install button does.
#[derive(Debug, PartialEq, Clone)]
pub enum Plan {
    /// `winget install --id <id>`
    Winget(&'static str),
    /// `sh -c <script> sh <args...>`
    Sh(&'static str, Vec<String>),
    /// `powershell -Command <command>`
    Ps(&'static str),
    /// Nothing to run here: the page to open.
    Link(&'static str),
    /// A pinned Windows installer the hub downloads itself (resumable, checked against its pin) and runs silently for this user: FreeCAD on a
    /// Windows with no winget (Windows Server, LTSC/IoT, a fresh install before App Installer updates).
    Direct(&'static WinInstaller),
    /// A program run as it is, no shell: an agent CLI's own updater (`claude update`, `opencode upgrade`...).
    Run(String, Vec<String>),
}

/// A Windows installer pinned by version, address, size and sha256, run with `args` (silent, this user only: no administrator rights).
#[derive(Debug, PartialEq)]
pub struct WinInstaller { pub name: &'static str, pub version: &'static str, pub asset: &'static str, pub url: &'static str, pub sha256: &'static str, pub size: u64, pub args: &'static [&'static str] }
/// FreeCAD's Windows installer: the release winget's FreeCAD.FreeCAD gives (winget-pkgs manifests/f/FreeCAD/FreeCAD/1.1.4: the same
/// InstallerUrl and InstallerSha256; its user scope is NSIS `/CurrentUser`). `/S /CurrentUser` installs it silently into
/// %LOCALAPPDATA%\Programs (`install_dir`, given to it with `/D=`) (0.16.0 release check on Windows Server 2022: 133 s, 519 MB).
/// One line per pin; the tests check it.
pub const FREECAD_WIN: WinInstaller = WinInstaller { name: "FreeCAD", version: "1.1.4", asset: "FreeCAD_1.1.4-Windows-x86_64-py311-installer.exe", url: "https://github.com/FreeCAD/FreeCAD/releases/download/1.1.4/FreeCAD_1.1.4-Windows-x86_64-py311-installer.exe", sha256: "845f7101d33faf257a82a0cadc5f4f3804441f46ee493eb32b92fcf9c7147a24", size: 519314976, args: &["/S", "/CurrentUser"] };
/// A pin's fields agree with each other: a version of digits and dots, the asset FreeCAD names for that version, its release's address, a
/// lowercase sha256, a size, and silent per-user switches.
pub fn check_pin(p: &WinInstaller) -> Result<(), String> {
    if p.version.is_empty() || !p.version.chars().all(|c| c.is_ascii_digit() || c == '.') { return Err("Invalid installer version".into()) }
    if !p.asset.starts_with(&format!("{}_{}-Windows-x86_64-", p.name, p.version)) || !p.asset.ends_with("-installer.exe") || p.asset.contains(['/', '\\']) { return Err("Invalid installer name".into()) }
    if p.url != format!("https://github.com/FreeCAD/FreeCAD/releases/download/{}/{}", p.version, p.asset) { return Err("Invalid installer address".into()) }
    if p.sha256.len() != 64 || !p.sha256.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) { return Err("Invalid pinned digest".into()) }
    if p.size == 0 || !p.args.contains(&"/S") || !p.args.contains(&"/CurrentUser") { return Err("Invalid installer pin".into()) }
    Ok(())
}
/// A downloaded installer is the pinned one: its size, its sha256 and a Windows program's header. Err says which, in the person's words.
pub fn verify(file: &Path, p: &WinInstaller) -> Result<(), String> {
    let len = std::fs::metadata(file).map_err(|e| format!("Could not read the {} installer: {e}", p.name))?.len();
    if len != p.size { return Err(format!("The {} installer is {len} bytes where {} were expected", p.name, p.size)) }
    let got = crate::files::kicad::install::sha256_file(file)?;
    if got != p.sha256 { return Err(format!("{} installer checksum mismatch", p.name)) }
    let mut mz = [0u8; 2]; let _ = std::io::Read::read(&mut std::fs::File::open(file).map_err(|e| e.to_string())?, &mut mz);
    if mz != *b"MZ" { return Err(format!("The {} installer is not a Windows program", p.name)) }
    Ok(())
}
/// Where the installer puts FreeCAD: `<LOCALAPPDATA>\Programs\FreeCAD <major.minor>` (its own per-user default), said to it with `/D=` so the
/// folder is the one this hub's environment names, and where the hub then finds `bin\freecadcmd.exe`.
pub fn install_dir(local_app_data: &Path, p: &WinInstaller) -> PathBuf {
    let mm: Vec<&str> = p.version.split('.').take(2).collect();
    local_app_data.join("Programs").join(format!("{} {}", p.name, mm.join(".")))
}
/// The installer's command line after the program: its switches, then NSIS's `/D=<folder>`, which must come last and unquoted (spaces and all).
pub fn installer_line(p: &WinInstaller, dest: &Path) -> String { format!("{} /D={}", p.args.join(" "), dest.display()) }
/// The key FreeCAD's installer registers itself under (`Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeCAD114` for 1.1.4).
pub fn uninstall_key(p: &WinInstaller) -> String { format!("{}{}", p.name, p.version.replace('.', "")) }
/// The folder an uninstaller registration names (`reg query ... /v UninstallString`'s answer: `"C:\...\FreeCAD 1.1\Uninstall-FreeCAD.exe"`).
pub fn registered_dir(reg_out: &str) -> Option<PathBuf> {
    let l = reg_out.lines().find(|l| l.trim_start().starts_with("UninstallString"))?;
    let v = l.split_once("REG_SZ")?.1.trim().trim_matches('"');
    // a Windows path, read the same on any system the tests run on
    v.rsplit_once(['\\', '/']).map(|(d, _)| d).filter(|d| !d.is_empty()).map(PathBuf::from)
}
/// Where this version is registered for this user, if it is (Windows' own reg.exe; None elsewhere or when it is not).
async fn registered(p: &WinInstaller) -> Option<PathBuf> {
    if !cfg!(windows) { return None }
    let reg = std::env::var_os("SystemRoot").map(|r| Path::new(&r).join("System32").join("reg.exe"))?;
    let key = format!("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}", uninstall_key(p));
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, reg); c.args(["query", &key, "/v", "UninstallString"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let o = tokio::time::timeout(std::time::Duration::from_secs(20), c.output()).await.ok()?.ok()?;
    registered_dir(&String::from_utf8_lossy(&o.stdout))
}
/// An installer's failure in the person's words. FreeCAD's quits silently (NSIS exit 2) when FreeCAD is running or this version is installed already.
pub fn installer_failed(p: &WinInstaller, code: Option<i32>) -> String {
    match code {
        Some(2) => format!("The {} installer stopped: close {} if it is open (agents' mechanical work uses it too), or remove an earlier {} {} in Windows' Installed apps, then Try again", p.name, p.name, p.name, p.version),
        Some(c) => format!("The {} installer stopped (exit {c})", p.name),
        None => format!("The {} installer was stopped", p.name),
    }
}
/// Where a pinned installer is downloaded and kept until it has run: it survives a failure and a hub restart, and the next try continues it.
/// Windows: %LOCALAPPDATA%\Backplane\downloads (the hub's home roams, and half a gigabyte does not belong there); elsewhere `<home>/downloads`.
pub fn download_dir(windows: bool, local_app_data: Option<&std::ffi::OsStr>, home: &Path) -> PathBuf {
    match local_app_data.filter(|d| windows && !d.is_empty()) { Some(d) => Path::new(d).join("Backplane").join("downloads"), None => home.join("downloads") }
}

pub const CLAUDE_SH: &str = "https://claude.ai/install.sh";
pub const CLAUDE_PS: &str = "irm https://claude.ai/install.ps1 | iex";
pub const GROK_SH: &str = "https://x.ai/cli/install.sh";
pub const GROK_PS: &str = "irm https://x.ai/cli/install.ps1 | iex";
pub const OPENCODE_SH: &str = "https://opencode.ai/install";
pub const CODEX_RELEASES: &str = "https://github.com/openai/codex/releases/latest";
pub const NODE: &str = "https://nodejs.org/en/download";

/// A vendor's own installer script: downloaded to a file (so a failed download stops here) and run with bash.
pub const VENDOR_SH: &str = r#"set -eu
# curl, else wget: a stock Ubuntu desktop has wget and no curl
if command -v curl >/dev/null 2>&1; then get() { curl --proto '=https' --tlsv1.2 -fsSL --connect-timeout 30 --speed-limit 1024 --speed-time 30 -o "$1" "$2"; }
elif command -v wget >/dev/null 2>&1; then get() { wget -q --https-only --connect-timeout=30 --read-timeout=30 -O "$1" "$2"; }
else get() { echo 'curl or wget is needed to download it, and neither was found' >&2; return 1; }; fi
t=$(mktemp); trap 'rm -f "$t"' 0
echo "Downloading the installer"
get "$t" "$1" || { echo "Could not download $1" >&2; exit 1; }
echo "Running the installer"
bash "$t"
"#;

/// Codex's standalone binary: the digest GitHub lists for the release asset, the download from that tag, the check, then the move into `$1` (a folder
/// the hub puts on its PATH). Arguments: folder, asset, name to give it, kind (tar.gz or raw). Nothing is placed unless the digest matches.
pub const CODEX_SH: &str = r#"set -eu
dest=$1; asset=$2; name=$3; kind=$4
fail() { echo "$*" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || fail 'curl or wget is needed to download Codex, and neither was found'
if command -v curl >/dev/null 2>&1; then get() { curl --proto '=https' --tlsv1.2 -fsSL --connect-timeout 30 --speed-limit 1024 --speed-time 30 -o "$1" "$2"; }
elif command -v wget >/dev/null 2>&1; then get() { wget -q --https-only --connect-timeout=30 --read-timeout=30 -O "$1" "$2"; }
else get() { echo 'curl or wget is needed to download it, and neither was found' >&2; return 1; }; fi
if command -v cygpath >/dev/null 2>&1; then dest=$(cygpath -u "$dest"); fi
case "$asset" in codex-*) ;; *) fail 'Invalid Codex asset' ;; esac
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' 0
echo "Looking up the latest Codex release"
get "$tmp/rel.json" https://api.github.com/repos/openai/codex/releases/latest || fail 'Could not read the Codex release list'
# the API answers on one line: one field per line, then read the tag and the digest that follows the asset's name
tr ',{}' '\n\n\n' < "$tmp/rel.json" | tr -d ' \r' > "$tmp/fields"
tag=$(sed -n 's/^"tag_name":"\([^"]*\)".*/\1/p' "$tmp/fields" | head -n 1)
want=$(awk -v n="\"name\":\"$asset\"" '$0==n { f=1 } f && index($0, "\"digest\":\"sha256:") == 1 { sub(/^"digest":"sha256:/, ""); sub(/".*/, ""); print; exit }' "$tmp/fields")
[ -n "$tag" ] && [ "${#want}" -eq 64 ] || fail "The Codex release lists no checksum for $asset"
echo "Downloading $asset"
get "$tmp/pkg" "https://github.com/openai/codex/releases/download/$tag/$asset" || fail "Could not download $asset"
echo 'Checking the checksum'
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$tmp/pkg" | cut -d' ' -f1); else got=$(shasum -a 256 "$tmp/pkg" | cut -d' ' -f1); fi
[ "$want" = "$got" ] || fail 'Codex checksum mismatch'
mkdir -p "$dest" "$tmp/x"
if [ "$kind" = tar.gz ]; then
  tar -xzf "$tmp/pkg" -C "$tmp/x"
  f=$(find "$tmp/x" -type f -name 'codex*' | head -n 1)
  [ -n "$f" ] || fail 'The Codex archive holds no program'
  mv "$f" "$tmp/bin"
else
  mv "$tmp/pkg" "$tmp/bin"
fi
chmod +x "$tmp/bin"
mv "$tmp/bin" "$dest/$name"
echo "Installed in $dest/$name"
"#;

/// FreeCAD's own release for Linux and macOS, from its GitHub releases, into `$2` (the hub's `freecad` folder, where `freecad/command` names
/// the console program the hub then finds: kicad/stepglb.rs `freecad_cmd`). `$1`: Linux-x86_64, Linux-aarch64, macOS-arm64 or macOS-x86_64.
/// The digest GitHub lists for the asset is checked before anything is placed. Linux: the AppImage is unpacked (`--appimage-extract`: no
/// FUSE is needed, and the console program runs from the unpacked tree). macOS: FreeCAD.app is copied out of the disk image. No
/// administrator password, and nothing outside that folder is changed.
pub const FREECAD_SH: &str = r#"set -eu
sys=$1; dest=$2
fail() { echo "$*" >&2; exit 1; }
if command -v curl >/dev/null 2>&1; then get() { curl --proto '=https' --tlsv1.2 -fsSL --connect-timeout 30 --speed-limit 1024 --speed-time 30 -o "$1" "$2"; }
elif command -v wget >/dev/null 2>&1; then get() { wget -q --https-only --connect-timeout=30 --read-timeout=30 -O "$1" "$2"; }
else fail 'curl or wget is needed to download FreeCAD, and neither was found'; fi
case "$sys" in Linux-*) ext=AppImage ;; macOS-*) ext=dmg ;; *) fail "No FreeCAD download for $sys" ;; esac
mkdir -p "$dest"
tmp=$(mktemp -d "$dest/.partial.XXXXXX"); mnt=
cleanup() { if [ -n "$mnt" ]; then hdiutil detach "$mnt" -quiet >/dev/null 2>&1 || true; fi; rm -rf "$tmp"; }
trap cleanup 0
echo 'Looking up the latest FreeCAD release'
get "$tmp/rel.json" https://api.github.com/repos/FreeCAD/FreeCAD/releases/latest || fail 'Could not read the FreeCAD release list'
# the API answers on one line: one field per line, then the tag, the asset's name and the digest that follows it
tr ',{}' '\n\n\n' < "$tmp/rel.json" | tr -d ' \r' > "$tmp/fields"
tag=$(sed -n 's/^"tag_name":"\([^"]*\)".*/\1/p' "$tmp/fields" | head -n 1)
asset=$(sed -n 's/^"name":"\(FreeCAD_[^"]*-'"$sys"'-[^"]*\.'"$ext"'\)"$/\1/p' "$tmp/fields" | head -n 1)
[ -n "$tag" ] && [ -n "$asset" ] || fail "The latest FreeCAD release has no download for $sys"
case "$tag$asset" in *[!A-Za-z0-9._-]*) fail 'Unexpected FreeCAD release name' ;; esac
want=$(awk -v n="\"name\":\"$asset\"" '$0==n { f=1 } f && index($0, "\"digest\":\"sha256:") == 1 { sub(/^"digest":"sha256:/, ""); sub(/".*/, ""); print; exit }' "$tmp/fields")
[ "${#want}" -eq 64 ] || fail "The FreeCAD release lists no checksum for $asset"
[ ! -e "$dest/$tag" ] || fail "$dest/$tag already exists; remove it and try again"
echo "Downloading FreeCAD $tag"
get "$tmp/pkg" "https://github.com/FreeCAD/FreeCAD/releases/download/$tag/$asset" || fail "Could not download $asset"
echo 'Checking the checksum'
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$tmp/pkg" | cut -d' ' -f1); else got=$(shasum -a 256 "$tmp/pkg" | cut -d' ' -f1); fi
[ "$want" = "$got" ] || fail 'FreeCAD checksum mismatch'
echo 'Unpacking'
if [ "$ext" = AppImage ]; then
  chmod +x "$tmp/pkg"
  (cd "$tmp" && ./pkg --appimage-extract >/dev/null 2>&1) || fail 'Could not unpack the FreeCAD AppImage'
  [ -x "$tmp/squashfs-root/usr/bin/freecadcmd" ] || fail 'The FreeCAD AppImage holds no freecadcmd'
  mv "$tmp/squashfs-root" "$dest/$tag"
  cmd="$dest/$tag/usr/bin/freecadcmd"
else
  mnt="$tmp/mnt"; mkdir "$mnt"
  hdiutil attach "$tmp/pkg" -nobrowse -readonly -mountpoint "$mnt" -quiet || fail 'Could not open the FreeCAD disk image'
  [ -d "$mnt/FreeCAD.app" ] || fail 'The FreeCAD disk image holds no FreeCAD.app'
  mkdir "$tmp/out" && cp -R "$mnt/FreeCAD.app" "$tmp/out/" || fail 'Could not copy FreeCAD.app'
  mv "$tmp/out" "$dest/$tag"
  cmd="$dest/$tag/FreeCAD.app/Contents/Resources/bin/freecadcmd"
  [ -x "$cmd" ] || fail 'FreeCAD.app holds no freecadcmd'
fi
printf '%s\n' "$cmd" > "$tmp/command" && mv "$tmp/command" "$dest/command"
echo "Installed in $dest/$tag"
"#;
/// FreeCAD's release name for a system (`FREECAD_SH`'s first argument), None where it has none.
pub fn freecad_sys(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) { ("linux", "x64") => Some("Linux-x86_64"), ("linux", "arm64") => Some("Linux-aarch64"), ("darwin", "arm64") => Some("macOS-arm64"), ("darwin", "x64") => Some("macOS-x86_64"), _ => None }
}

/// Codex's release asset for a system: (asset, the name it is given, kind). Hub `os` and `arch` words (linux, darwin, windows; x64, arm64).
pub fn codex_asset(os: &str, arch: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match (os, arch) {
        ("linux", "x64") => Some(("codex-x86_64-unknown-linux-musl.tar.gz", "codex", "tar.gz")),
        ("linux", "arm64") => Some(("codex-aarch64-unknown-linux-musl.tar.gz", "codex", "tar.gz")),
        ("darwin", "arm64") => Some(("codex-aarch64-apple-darwin.tar.gz", "codex", "tar.gz")),
        ("darwin", "x64") => Some(("codex-x86_64-apple-darwin.tar.gz", "codex", "tar.gz")),
        ("windows", "x64") => Some(("codex-x86_64-pc-windows-msvc.exe", "codex.exe", "raw")),
        ("windows", "arm64") => Some(("codex-aarch64-pc-windows-msvc.exe", "codex.exe", "raw")),
        _ => None,
    }
}

/// The folder the hub puts on the PATH of its agents, for what setup installs itself: Windows `%LOCALAPPDATA%\Backplane\tools\bin`, else
/// `$XDG_DATA_HOME/backplane/tools/bin` or `~/.local/share/backplane/tools/bin`.
pub fn tools_dir_in(windows: bool, local_app_data: Option<&str>, xdg: Option<&str>, home: &str) -> PathBuf {
    match (windows, local_app_data.filter(|d| !d.is_empty())) {
        (true, Some(d)) => Path::new(d).join("Backplane").join("tools").join("bin"),
        _ => Path::new(xdg.filter(|d| !d.is_empty()).map(String::from).unwrap_or_else(|| format!("{home}/.local/share")).as_str()).join("backplane/tools/bin"),
    }
}
pub fn tools_dir() -> PathBuf {
    let home = crate::plat::dirs::home_or_empty();
    tools_dir_in(cfg!(windows), std::env::var("LOCALAPPDATA").ok().as_deref(), std::env::var("XDG_DATA_HOME").ok().as_deref(), &home)
}

/// What a press does for `tool` on this system, or None when the tool has no button (its guide is the way). `winget`/`node`: whether the checks found them.
pub fn plan(tool: &str, os: &str, arch: &str, winget: bool, node: bool) -> Option<Plan> {
    let unix = os == "linux" || os == "darwin";
    match tool {
        "tailscale" => match os { "windows" if winget => Some(Plan::Winget("Tailscale.Tailscale")), "windows" => Some(Plan::Link("https://tailscale.com/download/windows")), "darwin" => Some(Plan::Link("https://tailscale.com/download/mac")), _ => Some(Plan::Link("https://tailscale.com/download/linux")) },
        // Git for Windows: the hub's shell commands and (until the fetch in Rust reaches Windows) KiCad's download run in its sh
        "git" if os == "windows" && winget => Some(Plan::Winget("Git.Git")),
        "git" if os == "windows" => Some(Plan::Link("https://git-scm.com/download/win")),
        // Linux and macOS: FreeCAD's own release into the hub's folder (`start` adds that folder); Windows: winget, else the same release's
        // installer, pinned, downloaded and run by the hub (x64 only: FreeCAD publishes no other Windows build)
        "freecad" => match (os, freecad_sys(os, arch)) {
            ("windows", _) if winget => Some(Plan::Winget("FreeCAD.FreeCAD")),
            ("windows", _) if arch == "x64" => Some(Plan::Direct(&FREECAD_WIN)),
            (_, Some(sys)) => Some(Plan::Sh(FREECAD_SH, vec![sys.into()])),
            _ => Some(Plan::Link("https://www.freecad.org/downloads.php")),
        },
        "claude" if unix => Some(Plan::Sh(VENDOR_SH, vec![CLAUDE_SH.into()])),
        "claude" if os == "windows" => Some(Plan::Ps(CLAUDE_PS)),
        "grok" if unix => Some(Plan::Sh(VENDOR_SH, vec![GROK_SH.into()])),
        "grok" if os == "windows" => Some(Plan::Ps(GROK_PS)),
        "opencode" if unix => Some(Plan::Sh(VENDOR_SH, vec![OPENCODE_SH.into()])),
        "codex" => match codex_asset(os, arch) {
            Some((asset, name, kind)) => Some(Plan::Sh(CODEX_SH, vec![tools_dir().to_string_lossy().into_owned(), asset.into(), name.into(), kind.into()])),
            // no standalone build for this system: npm only when Node is already there
            None if node => Some(Plan::Sh("exec npm install -g @openai/codex", vec![])),
            None => Some(Plan::Link(CODEX_RELEASES)),
        },
        _ => None,
    }
}

/// Each tool's install: state, note and the share downloaded (0 to 1, a download with a known size only).
static STATE: Mutex<BTreeMap<String, (String, String, Option<f64>)>> = Mutex::new(BTreeMap::new());
fn snapshot() -> String {
    let m = STATE.lock().unwrap();
    Value::Object(m.iter().map(|(k, (s, n, b))| { let mut v = json!({"state": s, "note": n}); if let Some(b) = b { v["progress"] = json!(b); } (k.clone(), v) }).collect()).to_string()
}
async fn say(hub: &Arc<Hub>, tool: &str, state: &str, note: &str) { say_bar(hub, tool, state, note, None).await }
async fn say_bar(hub: &Arc<Hub>, tool: &str, state: &str, note: &str, bar: Option<f64>) {
    STATE.lock().unwrap().insert(tool.into(), (state.into(), note.into(), bar));
    hub.set_info("onboard.installs", &snapshot()).await; hub.bump_views();
}
/// The last line a program wrote ("" for none).
pub fn last_line(text: &str) -> String { text.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(200).collect() }

/// Program and arguments for a plan (None for a link).
pub fn command(p: &Plan) -> Option<(String, Vec<String>)> {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match p {
        Plan::Winget(id) => Some(("winget".into(), v(&["install", "--id", id, "-e", "--silent", "--accept-package-agreements", "--accept-source-agreements"]))),
        Plan::Sh(script, args) => { let mut a = v(&["-c", script, "sh"]); a.extend(args.iter().cloned()); Some(("sh".into(), a)) }
        Plan::Ps(c) => Some(("powershell".into(), v(&["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", c]))),
        Plan::Run(prog, args) => Some((prog.clone(), args.clone())),
        Plan::Link(_) | Plan::Direct(_) => None,
    }
}

/// What an agent's Update button (`update:<agent>`) does: the CLI's own updater found where the hub finds the CLI (agent_versions.rs `TABLE`); Codex placed
/// by setup's own install (in the tools folder, where `codex update` cannot replace it) gets that install again; a CLI with no updater, its page.
/// None: the CLI is not found here.
pub fn update_plan(id: &str, path: &Path, tools: &Path, os: &str, arch: &str, node: bool) -> Option<Plan> {
    let s = super::agent_versions::of(id)?;
    if id == "codex" && path.starts_with(tools) { return plan("codex", os, arch, false, node) }
    if s.update.is_empty() { return Some(Plan::Link(s.page)) }
    Some(Plan::Run(path.to_string_lossy().into_owned(), s.update.iter().map(|a| a.to_string()).collect()))
}

/// `onboard.install {tool}`: started, or refused with the reason. How it goes is the info key `onboard.installs`; `after` runs the checks again at the end.
pub async fn start(hub: &Arc<Hub>, tool: &str, winget: bool, node: bool, after: impl FnOnce(Arc<Hub>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + 'static) -> Result<String, String> {
    if crate::copy::on() { return Err("a copy of the log installs nothing".into()) }
    let p = match tool.strip_prefix("update:") {
        Some(id) => { let path = crate::agents::proc::Cli::from_env().resolve(super::agent_versions::program(id)).map_err(|_| format!("{id} is not installed here"))?;
            update_plan(id, &path, &tools_dir(), &hub.cfg.os, &hub.cfg.arch, node) }
        None => plan(tool, &hub.cfg.os, &hub.cfg.arch, winget, node),
    };
    let Some(p) = p else { return Err(format!("{tool} has no install button; follow its guide")) };
    // FreeCAD's own release goes into the hub's folder, where the hub then finds it (freecad/command)
    let p = match p { Plan::Sh(s, mut a) if tool == "freecad" => { a.push(hub.cfg.home.join("freecad").to_string_lossy().into_owned()); Plan::Sh(s, a) } p => p };
    if STATE.lock().unwrap().get(tool).map_or(false, |(s, _, _)| s == "running") { return Err(format!("{tool} is being installed already")) }
    if let Plan::Direct(pin) = p {
        let tool = tool.to_string(); let name = tool.clone();
        say(hub, &tool, "running", "Starting.").await;
        let h = hub.clone();
        let log = hub.cfg.home.join("installs"); let _ = std::fs::create_dir_all(&log); let log = log.join(format!("{}.log", tool.replace(':', "-")));
        let dir = download_dir(cfg!(windows), std::env::var_os("LOCALAPPDATA").as_deref(), &hub.cfg.home);
        tokio::spawn(async move {
            match direct(&h, &tool, pin, &dir, &log).await { Ok(()) => say(&h, &tool, "done", "Installed.").await, Err(why) => say(&h, &tool, "failed", &why).await }
            after(h).await;
        });
        return Ok(format!("installing {name}"))
    }
    let Some((prog, args)) = command(&p) else { return Err("open its download page".into()) };
    let tool = tool.to_string(); let name = tool.clone();
    say(hub, &tool, "running", "Starting.").await;
    let h = hub.clone();
    let log = hub.cfg.home.join("installs"); let _ = std::fs::create_dir_all(&log); let log = log.join(format!("{}.log", tool.replace(':', "-")));
    tokio::spawn(async move {
        let r = run(&h, &tool, &prog, &args, &log).await;
        match r { Ok(()) => say(&h, &tool, "done", "Installed.").await, Err(why) => say(&h, &tool, "failed", &why).await }
        super::models_probe::refresh_soon(&h); // the picker finds what was just installed (or updated) now, not at the next 15-minute round
        after(h).await;
    });
    Ok(format!("installing {name}"))
}
/// A pinned installer: downloaded into `dir` with the system's curl (Windows' own System32 curl.exe), continuing what an earlier try left there
/// and retrying a cut connection as KiCad's download does, then checked (`verify`), run with its silent per-user switches, and the console
/// program it placed written to `<home>/freecad/command` so the hub uses it at once.
async fn direct(hub: &Arc<Hub>, tool: &str, p: &'static WinInstaller, dir: &Path, log: &Path) -> Result<(), String> {
    use crate::files::kicad::install::{get_args, tools, Bytes, Meter, Getter};
    check_pin(p)?;
    let _ = std::fs::write(log, "");
    let t = tools()?;
    let gp = match &t.get { Getter::Curl(x) | Getter::Wget(x) => x.clone() };
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not make {}: {e}", dir.display()))?;
    let file = dir.join(p.asset);
    let have = |f: &Path| std::fs::metadata(f).map_or(0, |m| m.len());
    // more than the pin is never this file: start again
    if have(&file) > p.size { let _ = std::fs::remove_file(&file); }
    let before = have(&file);
    let mb = |b: u64| (b + (1 << 19)) >> 20;
    let head = format!("Downloading {} {}", p.name, p.version);
    if before < p.size {
        say(hub, tool, "running", &if before > 0 { format!("{head} (continuing from {} MB)", mb(before)) } else { head.clone() }).await;
        let mut meter = Meter::default();
        let tick = |meter: &mut Meter| {
            let b = meter.at(std::time::Instant::now(), have(&file), p.size);
            STATE.lock().unwrap().insert(tool.into(), ("running".into(), format!("{head}: {}", b.words()), Some((b.share() * 1000.).round() / 1000.)));
            let h = hub.clone(); tokio::spawn(async move { h.set_info("onboard.installs", &snapshot()).await; h.bump_views(); });
        };
        let mut ok = Err(String::new());
        for _ in 0..3 {
            ok = crate::files::kicad::install::tool(&gp, &get_args(&t.get, p.url, &file), log, || tick(&mut meter)).await;
            if ok.is_ok() { break }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        if let Err(why) = ok {
            let got = have(&file);
            let at = if got == 0 { String::new() } else { format!(" at {}", Bytes { done: got, total: p.size, rate: 0, eta: 0 }.words()) };
            return Err(format!("The download stopped{at} ({}); Try again continues it", if why.is_empty() { "no answer".to_string() } else { why }))
        }
    }
    say_bar(hub, tool, "running", "Checking the download", Some(1.)).await;
    let f = file.clone();
    if let Err(why) = tokio::task::spawn_blocking(move || verify(&f, p)).await.map_err(|e| e.to_string())? {
        let _ = std::fs::remove_file(&file);
        return Err(format!("{why} (the download was removed; Try again downloads it anew)"))
    }
    let lad = std::env::var_os("LOCALAPPDATA").filter(|d| !d.is_empty()).ok_or("LOCALAPPDATA is not set")?;
    let dest = install_dir(Path::new(&lad), p);
    // this version installed for this user already (the installer then quits at once, silently): that one is used
    if let Some(dir) = registered(p).await.filter(|d| d.join("bin").join("freecadcmd.exe").is_file()) {
        let cmd = dir.join("bin").join("freecadcmd.exe").to_string_lossy().into_owned();
        let d = hub.cfg.home.join("freecad"); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join("command"), &cmd);
        say(hub, tool, "running", &format!("{} {} is installed for this user already, in {}", p.name, p.version, dir.display())).await;
        return Ok(())
    }
    say(hub, tool, "running", &format!("Installing {} {} for this Windows user. This takes about two minutes", p.name, p.version)).await;
    let out = std::fs::OpenOptions::new().append(true).create(true).open(log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &file);
    // the whole line as is: NSIS reads `/D=` only unquoted, and the folder may hold spaces
    #[cfg(windows)] c.raw_arg(installer_line(p, &dest));
    #[cfg(not(windows))] c.args(p.args).arg(format!("/D={}", dest.display()));
    c.stdin(Stdio::null()).stdout(Stdio::from(out)).stderr(Stdio::from(err)).kill_on_drop(false);
    crate::plat::proc::own_group_tokio(&mut c);
    let mut child = c.spawn().map_err(|e| format!("Could not start the {} installer: {e}", p.name))?;
    let status = match tokio::time::timeout(std::time::Duration::from_secs(30 * 60), child.wait()).await {
        Ok(r) => r.map_err(|e| e.to_string())?,
        Err(_) => return Err(format!("The {} installer did not finish in 30 minutes", p.name)),
    };
    if !status.success() { return Err(installer_failed(p, status.code())) }
    // the console program it placed; the hub uses it from now on
    let cmd = dest.join("bin").join("freecadcmd.exe");
    if !cmd.is_file() { return Err(format!("The {} installer finished, but {} is not there", p.name, cmd.display())) }
    let cmd = cmd.to_string_lossy().into_owned();
    let d = hub.cfg.home.join("freecad"); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join("command"), &cmd);
    let _ = std::fs::remove_file(&file);
    say(hub, tool, "running", &format!("Installed in {}", dest.display())).await;
    Ok(())
}
async fn run(hub: &Arc<Hub>, tool: &str, prog: &str, args: &[String], log: &Path) -> Result<(), String> {
    let out = std::fs::File::create(log).map_err(|e| format!("could not write {}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, std::env::var_os("BACKPLANE_TOOL_INSTALLER").filter(|s| !s.is_empty()).unwrap_or_else(|| prog.into()));
    c.args(args).stdin(Stdio::null()).stdout(Stdio::from(out)).stderr(Stdio::from(err)).kill_on_drop(false);
    crate::plat::proc::own_group_tokio(&mut c);
    let mut child = c.spawn().map_err(|e| format!("could not start {prog}: {e}"))?;
    let mut said = String::new();
    let status = loop {
        let done = tokio::time::timeout(std::time::Duration::from_millis(400), child.wait()).await;
        let now = last_line(&std::fs::read_to_string(log).unwrap_or_default());
        if !now.is_empty() && now != said { said = now.clone(); say(hub, tool, "running", &now).await }
        if let Ok(r) = done { break r.map_err(|e| e.to_string())? }
    };
    if status.success() { Ok(()) } else { Err(if said.is_empty() { format!("the installer stopped ({status})") } else { said }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)] use std::os::unix::fs::PermissionsExt;

    /// Every tool on every system: the button it gets (or the page).
    #[test] fn each_tool_gets_its_vendors_own_installer_per_system() {
        let cases: &[(&str, &str, &str, bool, bool, fn(&Plan) -> bool)] = &[
            ("claude", "linux", "x64", false, false, |p| matches!(p, Plan::Sh(s, a) if *s == VENDOR_SH && a == &[CLAUDE_SH])),
            ("claude", "darwin", "arm64", false, false, |p| matches!(p, Plan::Sh(_, a) if a == &[CLAUDE_SH])),
            ("claude", "windows", "x64", false, false, |p| *p == Plan::Ps(CLAUDE_PS)),
            ("grok", "linux", "arm64", false, false, |p| matches!(p, Plan::Sh(_, a) if a == &[GROK_SH])),
            ("grok", "windows", "x64", false, false, |p| *p == Plan::Ps(GROK_PS)),
            ("tailscale", "windows", "x64", true, false, |p| *p == Plan::Winget("Tailscale.Tailscale")),
            ("tailscale", "windows", "x64", false, false, |p| *p == Plan::Link("https://tailscale.com/download/windows")),
            ("tailscale", "darwin", "arm64", true, false, |p| *p == Plan::Link("https://tailscale.com/download/mac")),
            ("tailscale", "linux", "x64", false, false, |p| *p == Plan::Link("https://tailscale.com/download/linux")),
            ("freecad", "windows", "x64", true, false, |p| *p == Plan::Winget("FreeCAD.FreeCAD")),
            ("freecad", "windows", "x64", false, false, |p| *p == Plan::Direct(&FREECAD_WIN)),
            ("freecad", "windows", "arm64", false, false, |p| matches!(p, Plan::Link(u) if u.contains("freecad.org"))),
            ("freecad", "windows", "arm64", true, false, |p| *p == Plan::Winget("FreeCAD.FreeCAD")),
            ("freecad", "linux", "x64", true, false, |p| matches!(p, Plan::Sh(s, a) if *s == FREECAD_SH && a == &["Linux-x86_64"])),
            ("freecad", "darwin", "arm64", false, false, |p| matches!(p, Plan::Sh(s, a) if *s == FREECAD_SH && a == &["macOS-arm64"])),
            ("freecad", "freebsd", "x64", false, false, |p| matches!(p, Plan::Link(_))),
        ];
        for (tool, os, arch, winget, node, ok) in cases { let p = plan(tool, os, arch, *winget, *node).unwrap_or_else(|| panic!("{tool} {os}")); assert!(ok(&p), "{tool} {os} {arch}: {p:?}"); }
        assert_eq!(plan("pi", "linux", "x64", false, false), None, "no button for a tool with no installer we know");
    }
    #[test] fn codex_is_its_standalone_binary_and_never_needs_node() {
        for (os, arch, asset) in [("linux", "x64", "codex-x86_64-unknown-linux-musl.tar.gz"), ("linux", "arm64", "codex-aarch64-unknown-linux-musl.tar.gz"), ("darwin", "arm64", "codex-aarch64-apple-darwin.tar.gz"), ("darwin", "x64", "codex-x86_64-apple-darwin.tar.gz"), ("windows", "x64", "codex-x86_64-pc-windows-msvc.exe"), ("windows", "arm64", "codex-aarch64-pc-windows-msvc.exe")] {
            let Some(Plan::Sh(script, a)) = plan("codex", os, arch, false, false) else { panic!("{os} {arch}") };
            assert_eq!(script, CODEX_SH); assert_eq!(a[1], asset); assert!(!script.contains("npm"), "no Node");
            assert_eq!(a[2], if os == "windows" { "codex.exe" } else { "codex" });
        }
        assert_eq!(plan("codex", "freebsd", "x64", false, false), Some(Plan::Link(CODEX_RELEASES)));
        assert!(matches!(plan("codex", "freebsd", "x64", false, true), Some(Plan::Sh(s, _)) if s.contains("npm install -g @openai/codex")), "npm only as a fallback, with Node already there");
    }
    #[test] fn each_agents_update_is_its_own_updater_and_codex_from_setup_is_installed_again() {
        let (t, x) = (Path::new("/h/.local/share/backplane/tools/bin"), Path::new("/h/.local/bin"));
        let run = |id: &str| update_plan(id, &x.join(id), t, "linux", "x64", false);
        assert_eq!(run("claude"), Some(Plan::Run("/h/.local/bin/claude".into(), vec!["update".into()])));
        assert_eq!(run("opencode"), Some(Plan::Run("/h/.local/bin/opencode".into(), vec!["upgrade".into()])));
        assert_eq!(run("pi"), Some(Plan::Run("/h/.local/bin/pi".into(), vec!["update".into(), "--self".into()])));
        assert_eq!(run("codex"), Some(Plan::Run("/h/.local/bin/codex".into(), vec!["update".into()])), "an npm or brew Codex updates itself");
        assert!(matches!(update_plan("codex", &t.join("codex"), t, "linux", "x64", false), Some(Plan::Sh(s, _)) if s == CODEX_SH), "setup's Codex: its checked release again");
        assert!(matches!(run("muse"), Some(Plan::Link(_))), "no updater: its page");
        assert_eq!(command(&run("grok").unwrap()), Some(("/h/.local/bin/grok".into(), vec!["update".into()])));
        assert_eq!(update_plan("nobody", x, t, "linux", "x64", false), None);
    }
    #[test] fn the_tools_folder_follows_the_system() {
        assert_eq!(tools_dir_in(true, Some("C:\\Users\\A B\\AppData\\Local"), None, "C:\\Users\\A B"), Path::new("C:\\Users\\A B\\AppData\\Local").join("Backplane").join("tools").join("bin"));
        assert_eq!(tools_dir_in(false, Some("/ignored"), Some("/x/share"), "/h"), Path::new("/x/share/backplane/tools/bin"));
        assert_eq!(tools_dir_in(false, None, None, "/h"), Path::new("/h/.local/share/backplane/tools/bin"));
    }
    #[test] fn commands_never_use_a_shell_where_the_vendor_has_a_powershell_installer() {
        let (p, a) = command(&Plan::Ps(CLAUDE_PS)).unwrap(); assert_eq!(p, "powershell"); assert!(a.contains(&"-NonInteractive".to_string()) && a.last().unwrap() == CLAUDE_PS);
        let (p, a) = command(&Plan::Winget("FreeCAD.FreeCAD")).unwrap(); assert_eq!(p, "winget"); assert!(a.contains(&"--silent".to_string()) && a.contains(&"--accept-package-agreements".to_string()) && a.contains(&"FreeCAD.FreeCAD".to_string()));
        assert_eq!(command(&Plan::Link("https://x")), None);
    }

    #[cfg(unix)] fn exe(p: &Path, body: &str) { std::fs::write(p, body).unwrap(); std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap() }
    #[cfg(unix)] fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("tools-{n}")) }
    /// The real Codex script against a stand-in `curl` that serves a release listing in GitHub's layout and an archive.
    #[cfg(unix)] fn codex_run(r: &Path, digest_ok: bool) -> (bool, String, PathBuf) {
        let asset = "codex-x86_64-unknown-linux-musl.tar.gz";
        let serve = r.join("serve"); std::fs::create_dir_all(serve.join("pkg")).unwrap();
        exe(&serve.join("pkg/codex-x86_64-unknown-linux-musl"), "#!/bin/sh\necho codex 9.9.9\n");
        assert!(std::process::Command::new("tar").arg("-czf").arg(serve.join(asset)).arg("-C").arg(serve.join("pkg")).arg("codex-x86_64-unknown-linux-musl").status().unwrap().success());
        let sum = String::from_utf8(std::process::Command::new("sha256sum").arg(serve.join(asset)).output().unwrap().stdout).unwrap().split(' ').next().unwrap().to_string();
        let listed = if digest_ok { sum } else { "0".repeat(64) };
        std::fs::write(serve.join("latest"), format!("{{\"tag_name\":\"rust-v9.9.9\",\"author\":{{\"login\":\"bot\"}},\"assets\":[{{\"name\":\"codex-other.tar.gz\",\"uploader\":{{\"login\":\"x\"}},\"size\":1,\"digest\":\"sha256:{}\"}},{{\"name\":\"{asset}\",\"uploader\":{{\"login\":\"x\"}},\"size\":2,\"digest\":\"sha256:{listed}\"}}]}}", "f".repeat(64))).unwrap();
        let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        exe(&bin.join("curl"), &format!("#!/bin/sh\nwhile [ $# -gt 1 ]; do [ \"$1\" = -o ] && out=$2; shift; done\nf={}/$(basename \"$1\")\n[ -f \"$f\" ] || exit 22\ncp \"$f\" \"$out\"\n", serve.display()));
        let dest = r.join("my tools/bin");
        let o = std::process::Command::new("sh").arg("-c").arg(CODEX_SH).arg("sh").arg(&dest).arg(asset).arg("codex").arg("tar.gz").env("PATH", format!("{}:/usr/bin:/bin", bin.display())).output().unwrap();
        (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)), dest)
    }
    #[cfg(unix)] #[test] fn the_codex_script_places_a_binary_whose_digest_matches_and_runs() {
        let r = tmp("ok"); let (ok, out, dest) = codex_run(&r, true);
        assert!(ok, "{out}");
        assert_eq!(out.lines().collect::<Vec<_>>(), ["Looking up the latest Codex release", "Downloading codex-x86_64-unknown-linux-musl.tar.gz", "Checking the checksum", &format!("Installed in {}/codex", dest.display())]);
        let v = std::process::Command::new(dest.join("codex")).arg("--version").output().unwrap(); assert_eq!(String::from_utf8_lossy(&v.stdout).trim(), "codex 9.9.9", "it runs");
        let _ = std::fs::remove_dir_all(&r);
    }
    /// A PATH with this machine's programs but curl and wget (a stock Ubuntu desktop has wget only): `wget` adds the stand-in `wget -O OUT URL`.
    #[cfg(unix)] fn no_curl(r: &Path, serve: &Path, wget: bool) -> PathBuf {
        let b = r.join(if wget { "wget-only" } else { "neither" }); std::fs::create_dir_all(&b).unwrap();
        for d in ["/usr/local/bin", "/usr/bin", "/bin"] { for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let n = e.file_name(); if n == "curl" || n == "wget" || b.join(&n).exists() { continue } let _ = std::os::unix::fs::symlink(e.path(), b.join(&n));
        } }
        if wget { exe(&b.join("wget"), &format!("#!/bin/sh\nwhile [ $# -gt 1 ]; do [ \"$1\" = -O ] && out=$2; shift; done\nf={}/$(basename \"$1\")\n[ -f \"$f\" ] || exit 8\ncp \"$f\" \"$out\"\n", serve.display())); }
        b
    }
    #[cfg(unix)] #[test] fn with_no_curl_the_codex_script_downloads_with_wget_and_with_neither_says_so() {
        let r = tmp("wget"); let _ = codex_run(&r, true); let serve = r.join("serve"); let dest = r.join("w/bin");
        let run = |p: &Path| { let o = std::process::Command::new("sh").arg("-c").arg(CODEX_SH).arg("sh").arg(&dest).arg("codex-x86_64-unknown-linux-musl.tar.gz").arg("codex").arg("tar.gz").env("PATH", p).output().unwrap(); (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))) };
        let (ok, out) = run(&no_curl(&r, &serve, false)); assert!(!ok && out.contains("curl or wget is needed to download Codex"), "{out}");
        let (ok, out) = run(&no_curl(&r, &serve, true)); assert!(ok, "{out}"); assert!(dest.join("codex").is_file());
        // the vendor wrapper (Claude, Grok, OpenCode) downloads its installer with wget too
        let o = std::process::Command::new("sh").arg("-c").arg(VENDOR_SH).arg("sh").arg("https://x/latest").env("PATH", no_curl(&r, &serve, true)).output().unwrap();
        let t = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); assert!(t.contains("Running the installer") && !t.contains("Could not download"), "{t}");
        let _ = std::fs::remove_dir_all(&r);
    }
    #[cfg(unix)] #[test] fn the_codex_script_places_nothing_when_the_digest_differs() {
        let r = tmp("bad"); let (ok, out, dest) = codex_run(&r, false);
        assert!(!ok && out.trim_end().ends_with("Codex checksum mismatch"), "{out}");
        assert!(!dest.join("codex").exists(), "nothing placed");
        let _ = std::fs::remove_dir_all(&r);
    }
    #[cfg(unix)] #[test] fn the_vendor_script_stops_when_the_download_fails() {
        let r = tmp("vendor"); let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        exe(&bin.join("curl"), "#!/bin/sh\nexit 22\n");
        let o = std::process::Command::new("sh").arg("-c").arg(VENDOR_SH).arg("sh").arg(CLAUDE_SH).env("PATH", format!("{}:/usr/bin:/bin", bin.display())).output().unwrap();
        assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("Could not download https://claude.ai/install.sh"));
        let _ = std::fs::remove_dir_all(&r);
    }
    /// winget first where it answers; without it the pinned installer, never only a page (the 0.16.0 blocker on Windows Server 2022).
    #[test] fn windows_freecad_is_winget_when_present_and_the_pinned_installer_when_not() {
        assert_eq!(plan("freecad", "windows", "x64", true, false), Some(Plan::Winget("FreeCAD.FreeCAD")));
        let Some(Plan::Direct(p)) = plan("freecad", "windows", "x64", false, false) else { panic!("no winget: the direct install") };
        assert_eq!(p, &FREECAD_WIN); assert_eq!(command(&Plan::Direct(p)), None, "the hub runs it itself, not a command line");
        assert!(p.args == ["/S", "/CurrentUser"], "silent and per-user: no administrator rights");
    }
    #[test] fn the_freecad_pin_is_whole_and_a_broken_pin_is_refused() {
        check_pin(&FREECAD_WIN).unwrap();
        assert!(FREECAD_WIN.url.ends_with(&format!("/{}/{}", FREECAD_WIN.version, FREECAD_WIN.asset)));
        let bad = |p: WinInstaller, why: &str| assert_eq!(check_pin(&p).unwrap_err(), why);
        let base = || WinInstaller { ..FREECAD_WIN };
        bad(WinInstaller { version: "1.1.4;x", ..base() }, "Invalid installer version");
        bad(WinInstaller { version: "1.1.5", ..base() }, "Invalid installer name");
        bad(WinInstaller { url: "https://example.com/FreeCAD_1.1.4-Windows-x86_64-py311-installer.exe", ..base() }, "Invalid installer address");
        bad(WinInstaller { url: "http://github.com/FreeCAD/FreeCAD/releases/download/1.1.4/FreeCAD_1.1.4-Windows-x86_64-py311-installer.exe", ..base() }, "Invalid installer address");
        bad(WinInstaller { sha256: "845F7101D33FAF257A82A0CADC5F4F3804441F46EE493EB32B92FCF9C7147A24", ..base() }, "Invalid pinned digest");
        bad(WinInstaller { sha256: "845f71", ..base() }, "Invalid pinned digest");
        bad(WinInstaller { args: &["/S"], ..base() }, "Invalid installer pin");
    }
    #[test] fn a_download_is_used_only_when_its_size_digest_and_header_match_the_pin() {
        use sha2::{Digest, Sha256};
        let r = crate::plat::tmp::dir("tools-verify"); let f = r.join("i.exe");
        let body = b"MZ\x90\x00 a stand-in installer".to_vec();
        let sum: &'static str = Box::leak(hex::encode(Sha256::digest(&body)).into_boxed_str());
        let pin = WinInstaller { sha256: sum, size: body.len() as u64, ..FREECAD_WIN };
        std::fs::write(&f, &body).unwrap(); verify(&f, &pin).unwrap();
        std::fs::write(&f, &body[..body.len() - 1]).unwrap(); assert!(verify(&f, &pin).unwrap_err().contains("bytes where"), "a cut download");
        let mut other = body.clone(); other[5] ^= 1; std::fs::write(&f, &other).unwrap(); assert_eq!(verify(&f, &pin).unwrap_err(), "FreeCAD installer checksum mismatch");
        let mut nz = body.clone(); nz[0] = b'X'; let nsum: &'static str = Box::leak(hex::encode(Sha256::digest(&nz)).into_boxed_str());
        std::fs::write(&f, &nz).unwrap(); assert_eq!(verify(&f, &WinInstaller { sha256: nsum, ..pin }).unwrap_err(), "The FreeCAD installer is not a Windows program");
        let _ = std::fs::remove_dir_all(&r);
    }
    #[test] fn the_installer_is_told_its_folder_last_and_unquoted() {
        let d = install_dir(Path::new("C:\\Users\\A B\\AppData\\Local"), &FREECAD_WIN);
        assert_eq!(d, Path::new("C:\\Users\\A B\\AppData\\Local").join("Programs").join("FreeCAD 1.1"));
        let l = installer_line(&FREECAD_WIN, &d);
        assert!(l.starts_with("/S /CurrentUser /D=") && l.ends_with("FreeCAD 1.1") && !l.contains('"'), "{l}");
    }
    #[test] fn an_installed_version_is_found_by_its_registration() {
        assert_eq!(uninstall_key(&FREECAD_WIN), "FreeCAD114");
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\FreeCAD114\r\n    UninstallString    REG_SZ    \"C:\\Users\\A B\\AppData\\Local\\Programs\\FreeCAD 1.1\\Uninstall-FreeCAD.exe\"\r\n\r\n";
        assert_eq!(registered_dir(out), Some(PathBuf::from("C:\\Users\\A B\\AppData\\Local\\Programs\\FreeCAD 1.1")));
        assert_eq!(registered_dir("ERROR: The system was unable to find the specified registry key or value."), None);
        assert!(installer_failed(&FREECAD_WIN, Some(2)).contains("close FreeCAD"));
    }
    #[test] fn the_installer_downloads_outside_the_roaming_home_on_windows() {
        assert_eq!(download_dir(true, Some(std::ffi::OsStr::new("C:\\L")), Path::new("C:\\H")), Path::new("C:\\L").join("Backplane").join("downloads"));
        assert_eq!(download_dir(false, Some(std::ffi::OsStr::new("/l")), Path::new("/h")), Path::new("/h/downloads"));
    }
    #[test] fn the_last_line_is_what_the_row_says() { assert_eq!(last_line("a\n\n b \n\n"), "b"); assert_eq!(last_line(""), ""); }
}
