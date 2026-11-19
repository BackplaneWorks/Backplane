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
/// Git for Windows' installer (its 64-bit release on GitHub; `git-for-windows/git` v2.56.1.windows.1, the digest GitHub publishes for the
/// asset): Inno Setup, run silently into %LOCALAPPDATA%\Programs\Git. Its install.iss has `PrivilegesRequired=none` and no override, so
/// `/CURRENTUSER` decides nothing: a hub that is not elevated installs for this user only (no administrator rights), an elevated one
/// installs for all users, so the hub then gives it Program Files and no `/DIR` (`elevated_line`); a Git already installed for all users
/// is never run over (`machine_git`: the installer would move it into one user's folder; bp-win, Nov 17).
/// (`install_dir`). For a Windows with no winget, as FreeCAD's. One line per pin; the tests check it.
pub const GIT_WIN: WinInstaller = WinInstaller { name: "Git", version: "2.56.1", asset: "Git-2.56.1-64-bit.exe", url: "https://github.com/git-for-windows/git/releases/download/v2.56.1.windows.1/Git-2.56.1-64-bit.exe", sha256: "c330c72e10b408cc39994bfefdb63542b0d93d21699325c3e0bd874af8a5a559", size: 66870448, args: &["/VERYSILENT", "/NORESTART", "/NOCANCEL", "/SP-", "/SUPPRESSMSGBOXES", "/CURRENTUSER"] };
/// Inno Setup (Git for Windows) rather than NSIS (FreeCAD): its switches, folder and registration differ.
fn inno(p: &WinInstaller) -> bool { p.name == "Git" }
/// A pin's fields agree with each other: a version of digits and dots, the asset FreeCAD names for that version, its release's address, a
/// lowercase sha256, a size, and silent per-user switches.
pub fn check_pin(p: &WinInstaller) -> Result<(), String> {
    if p.version.is_empty() || !p.version.chars().all(|c| c.is_ascii_digit() || c == '.') { return Err("Invalid installer version".into()) }
    if inno(p) {
        if p.asset != format!("Git-{}-64-bit.exe", p.version) { return Err("Invalid installer name".into()) }
        if p.url != format!("https://github.com/git-for-windows/git/releases/download/v{}.windows.1/{}", p.version, p.asset) { return Err("Invalid installer address".into()) }
        if p.sha256.len() != 64 || !p.sha256.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) { return Err("Invalid pinned digest".into()) }
        if p.size == 0 || !p.args.contains(&"/VERYSILENT") || !p.args.contains(&"/CURRENTUSER") { return Err("Invalid installer pin".into()) }
        return Ok(())
    }
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
    if inno(p) { return local_app_data.join("Programs").join("Git") } // Git for Windows' own per-user default
    let mm: Vec<&str> = p.version.split('.').take(2).collect();
    local_app_data.join("Programs").join(format!("{} {}", p.name, mm.join(".")))
}
/// The installer's command line after the program: its switches, then NSIS's `/D=<folder>`, which must come last and unquoted (spaces and all).
pub fn installer_line(p: &WinInstaller, dest: &Path) -> String { if inno(p) { format!("{} /DIR=\"{}\"", p.args.join(" "), dest.display()) } else { format!("{} /D={}", p.args.join(" "), dest.display()) } }
/// The key FreeCAD's installer registers itself under (`Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeCAD114` for 1.1.4).
pub fn uninstall_key(p: &WinInstaller) -> String { if inno(p) { "Git_is1".into() } else { format!("{}{}", p.name, p.version.replace('.', "")) } }
/// The program a pinned install leaves in its folder, which the hub then uses: FreeCAD's console program, Git's git.exe.
pub fn placed(p: &WinInstaller, dir: &Path) -> PathBuf { if inno(p) { dir.join("cmd").join("git.exe") } else { dir.join("bin").join("freecadcmd.exe") } }
/// After an install, or a hub started before one: Git for Windows' `cmd` folder put on this hub's PATH when no git is on it, so the hub and
/// the agents it starts find git without a restart (winget's and the installer's own PATH change reaches only new logins). Answers the folder.
pub fn git_pick_up() -> Option<PathBuf> {
    if !cfg!(windows) { return None }
    let path = std::env::var_os("PATH").unwrap_or_default();
    if std::env::split_paths(&path).any(|d| d.join("git.exe").is_file()) { return None }
    let dir = git_dirs(|k| std::env::var_os(k)).into_iter().find(|d| d.join("git.exe").is_file())?;
    let mut ps = vec![dir.clone()]; ps.extend(std::env::split_paths(&path));
    if let Ok(j) = std::env::join_paths(ps) { std::env::set_var("PATH", j) }
    Some(dir)
}
/// Where Git for Windows' `cmd` folder is when it is installed: for all users, then for this one.
pub fn git_dirs(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["ProgramW6432", "ProgramFiles"].iter().filter_map(|k| var(k)).map(|r| PathBuf::from(r).join("Git").join("cmd")).collect();
    if let Some(l) = var("LOCALAPPDATA") { v.push(PathBuf::from(l).join("Programs").join("Git").join("cmd")) }
    v.dedup(); v
}
/// The folder an uninstaller registration names (`reg query ... /v UninstallString`'s answer: `"C:\...\FreeCAD 1.1\Uninstall-FreeCAD.exe"`).
pub fn registered_dir(reg_out: &str) -> Option<PathBuf> {
    let l = reg_out.lines().find(|l| l.trim_start().starts_with("UninstallString"))?;
    let v = l.split_once("REG_SZ")?.1.trim().trim_matches('"');
    // a Windows path, read the same on any system the tests run on
    v.rsplit_once(['\\', '/']).map(|(d, _)| d).filter(|d| !d.is_empty()).map(PathBuf::from)
}
/// Where this version is registered for this user, if it is (Windows' own reg.exe; None elsewhere or when it is not).
async fn registered(p: &WinInstaller) -> Option<PathBuf> { registered_in("HKCU", p).await }
/// A Git for Windows installed for all users (its machine registration): its folder. The pinned installer is not run over it.
async fn machine_git() -> Option<PathBuf> {
    if let Some(d) = registered_in("HKLM", &GIT_WIN).await { return Some(d) }
    registered_in("HKLM\\Software\\WOW6432Node", &GIT_WIN).await
}
/// Whether this hub runs elevated (an administrator's full token): `fltmc` answers only then.
async fn elevated() -> bool {
    if !cfg!(windows) { return false }
    let Some(f) = std::env::var_os("SystemRoot").map(|r| Path::new(&r).join("System32").join("fltmc.exe")) else { return false };
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, f); c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    matches!(tokio::time::timeout(std::time::Duration::from_secs(20), c.status()).await, Ok(Ok(s)) if s.success())
}
/// Git for Windows' command line from an elevated hub: its switches less `/CURRENTUSER` and no `/DIR`, so the all-users install it makes stays in Program Files.
pub fn elevated_line(p: &WinInstaller) -> String { p.args.iter().filter(|a| **a != "/CURRENTUSER").copied().collect::<Vec<_>>().join(" ") }
/// What the press says when Git is installed for all users already (too old, or not found on PATH).
pub fn machine_git_note(dir: &Path) -> String { format!("Git for all users is installed in {}, and Backplane does not move it. Update it there (Git for Windows' installer from https://git-scm.com/download/win, as an administrator), then press Check again", dir.display()) }
async fn registered_in(hive: &str, p: &WinInstaller) -> Option<PathBuf> {
    if !cfg!(windows) { return None }
    let reg = std::env::var_os("SystemRoot").map(|r| Path::new(&r).join("System32").join("reg.exe"))?;
    let key = if hive.contains('\\') { format!("{hive}\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}", uninstall_key(p)) } else { format!("{hive}\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}", uninstall_key(p)) };
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, reg); c.args(["query", &key, "/v", "UninstallString"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let o = tokio::time::timeout(std::time::Duration::from_secs(20), c.output()).await.ok()?.ok()?;
    registered_dir(&String::from_utf8_lossy(&o.stdout))
}
/// An installer's failure in the person's words. FreeCAD's quits silently (NSIS exit 2) when FreeCAD is running or this version is installed already.
pub fn installer_failed(p: &WinInstaller, code: Option<i32>) -> String {
    match code {
        Some(c) if inno(p) => format!("The {} installer stopped (exit {c}); close any Git Bash window, then Try again", p.name),
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

/// Codex's standalone binary at a pinned release: the download from that tag, a check of its size and sha256 against the pin (`CODEX_TAG`,
/// `codex_asset`), then the move into `$1` (a folder the hub puts on its PATH). Arguments: folder, asset, name to give it, kind (tar.gz or raw),
/// tag, sha256, size. Nothing is placed unless both match.
pub const CODEX_SH: &str = r#"set -eu
dest=$1; asset=$2; name=$3; kind=$4; tag=$5; want=$6; size=$7
fail() { echo "$*" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || fail 'curl or wget is needed to download Codex, and neither was found'
if command -v curl >/dev/null 2>&1; then get() { curl --proto '=https' --tlsv1.2 -fsSL --connect-timeout 30 --speed-limit 1024 --speed-time 30 -o "$1" "$2"; }
elif command -v wget >/dev/null 2>&1; then get() { wget -q --https-only --connect-timeout=30 --read-timeout=30 -O "$1" "$2"; }
else get() { echo 'curl or wget is needed to download it, and neither was found' >&2; return 1; }; fi
if command -v cygpath >/dev/null 2>&1; then dest=$(cygpath -u "$dest"); fi
case "$asset" in codex-*) ;; *) fail 'Invalid Codex asset' ;; esac
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' 0
[ -n "$tag" ] && [ "${#want}" -eq 64 ] && [ -n "$size" ] || fail "No pinned checksum for $asset"
case "$tag" in *[!A-Za-z0-9._-]*) fail 'Invalid Codex release' ;; esac
echo "Downloading $asset"
get "$tmp/pkg" "https://github.com/openai/codex/releases/download/$tag/$asset" || fail "Could not download $asset"
echo 'Checking the checksum'
len=$(wc -c < "$tmp/pkg" | tr -d ' ')
[ "$len" = "$size" ] || fail 'Codex size mismatch'
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

/// The Codex release the installer is pinned to (github.com/openai/codex/releases/tag/rust-v0.170.0). Bump it with the table below: each
/// asset's size and the sha256 GitHub lists for it (`gh api repos/openai/codex/releases/tags/<tag>`). The tests check the table's shape.
pub const CODEX_TAG: &str = "rust-v0.170.0";
/// A Codex release asset's pin: (asset, the name it is given, kind, sha256, size).
pub type CodexPin = (&'static str, &'static str, &'static str, &'static str, u64);
pub const CODEX_PINS: &[(&str, &str, CodexPin)] = &[
    ("linux", "x64", ("codex-x86_64-unknown-linux-musl.tar.gz", "codex", "tar.gz", "d054916ccd28a53820376b8d2f7c875184ca8f40fcd44bbabeb8ee240ce247f5", 119025052)),
    ("linux", "arm64", ("codex-aarch64-unknown-linux-musl.tar.gz", "codex", "tar.gz", "0da6b061f6b5223901a5a73e841bd1466596e2ef620d68823360febed2ff7ebc", 121314071)),
    ("darwin", "arm64", ("codex-aarch64-apple-darwin.tar.gz", "codex", "tar.gz", "7c9f21d5cbae4860d14b379fdcf709ff399ce874fd714b58783e1b4199c2b9a9", 114545562)),
    ("darwin", "x64", ("codex-x86_64-apple-darwin.tar.gz", "codex", "tar.gz", "c858bb0149b003a8cf8840a1143c4d953f18bea23ba6ea861d5fdf0b5e29a5a3", 124603199)),
    ("windows", "x64", ("codex-x86_64-pc-windows-msvc.exe", "codex.exe", "raw", "8d06dc231dba649c7d48d3eecab239c452a1ac498c41dd6492a6f301b90a22ce", 385571120)),
    ("windows", "arm64", ("codex-aarch64-pc-windows-msvc.exe", "codex.exe", "raw", "57e60c27714335aa05bfe64c65f11e874bf29633fe7515c1298faf9b5db1ac29", 326498096)),
];
/// Codex's pinned release asset for a system. Hub `os` and `arch` words (linux, darwin, windows; x64, arm64).
pub fn codex_asset(os: &str, arch: &str) -> Option<CodexPin> { CODEX_PINS.iter().find(|(o, a, _)| *o == os && *a == arch).map(|(_, _, p)| *p) }
/// What the Install button says before it runs: whether the install is checked against a pin, or runs the vendor's own script (named, with its address).
pub fn install_notice(tool: &str, os: &str) -> String {
    let script = match (tool, os) {
        ("claude", "windows") => Some("https://claude.ai/install.ps1"), ("claude", _) => Some(CLAUDE_SH),
        ("grok", "windows") => Some("https://x.ai/cli/install.ps1"), ("grok", _) => Some(GROK_SH),
        ("opencode", _) => Some(OPENCODE_SH), _ => None };
    match (tool, script) {
        ("codex", _) => format!("Downloads Codex's release {CODEX_TAG} from GitHub and checks its size and sha256 against Backplane's pinned values before running anything."),
        (_, Some(u)) => format!("Downloads and runs the vendor's own installer script from {u}. It is not checked against a pinned checksum: the vendor publishes none."),
        _ => "Downloads and runs its vendor's own installer.".into(),
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
        "git" if os == "windows" && arch == "x64" => Some(Plan::Direct(&GIT_WIN)),
        "git" if os == "windows" => Some(Plan::Link("https://git-scm.com/download/win")),
        // macOS: Homebrew's git where Homebrew is, else Apple's Command Line Tools (its own window asks and installs; Check again after)
        "git" if os == "darwin" => Some(Plan::Sh(GIT_MAC_SH, vec![])),
        // Linux: the distribution's package, through pkexec (the desktop's own password prompt) where there is one; else the step's command to copy
        "git" if os == "linux" => Some(Plan::Sh(GIT_LINUX_SH, vec![])),
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
            Some((asset, name, kind, sha, size)) => Some(Plan::Sh(CODEX_SH, vec![tools_dir().to_string_lossy().into_owned(), asset.into(), name.into(), kind.into(), CODEX_TAG.into(), sha.into(), size.to_string()])),
            // no standalone build for this system: npm only when Node is already there
            None if node => Some(Plan::Sh("exec npm install -g @openai/codex", vec![])),
            None => Some(Plan::Link(CODEX_RELEASES)),
        },
        _ => None,
    }
}

/// macOS: `brew install git` with Homebrew, else Apple's Command Line Tools (`xcode-select --install` opens Apple's own installer window).
pub const GIT_MAC_SH: &str = "set -e; for b in /opt/homebrew/bin/brew /usr/local/bin/brew; do if [ -x \"$b\" ]; then exec \"$b\" install git; fi; done; xcode-select --install 2>&1 || true; echo 'The Apple installer window is open: finish it there, then press Check again.'";
/// Linux: the distribution's git through pkexec (a desktop password prompt) when there is one and a desktop to ask on; otherwise it stops
/// with the command to run in a terminal.
pub const GIT_LINUX_SH: &str = "for m in apt-get dnf pacman zypper apk; do command -v $m >/dev/null 2>&1 && break; m=; done
case $m in apt-get) a='apt-get install -y git';; dnf) a='dnf install -y git';; pacman) a='pacman -S --noconfirm git';; zypper) a='zypper --non-interactive install git';; apk) a='apk add git';; *) echo 'No package manager found: install git with the software tool of your system'; exit 1;; esac
if command -v pkexec >/dev/null 2>&1 && [ -n \"$DISPLAY$WAYLAND_DISPLAY\" ]; then exec pkexec $a; fi
echo \"Run in a terminal: sudo $a\"; exit 1";

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
    let mut dest = install_dir(Path::new(&lad), p);
    // Git for Windows: never over a Git installed for all users; an elevated hub's install is an all-users one, into Program Files
    let all_users = inno(p) && elevated().await;
    if inno(p) {
        if let Some(d) = machine_git().await { if placed(p, &d).is_file() { git_pick_up(); } return Err(machine_git_note(&d)) }
        if all_users { if let Some(pf) = std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles")) { dest = Path::new(&pf).join("Git") } }
    }
    // this version installed for this user already (the installer then quits at once, silently): that one is used
    if let Some(dir) = registered(p).await.filter(|d| placed(p, d).is_file()) {
        let cmd = placed(p, &dir).to_string_lossy().into_owned();
        if inno(p) { git_pick_up(); } else { let d = hub.cfg.home.join("freecad"); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join("command"), &cmd); }
        say(hub, tool, "running", &format!("{} {} is installed for this user already, in {}", p.name, p.version, dir.display())).await;
        return Ok(())
    }
    say(hub, tool, "running", &format!("Installing {} {} for this Windows user. This takes {}", p.name, p.version, if inno(p) { "about a minute" } else { "about two minutes" })).await;
    let out = std::fs::OpenOptions::new().append(true).create(true).open(log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &file);
    // the whole line as is: NSIS reads `/D=` only unquoted, and the folder may hold spaces
    #[cfg(windows)] c.raw_arg(if all_users { elevated_line(p) } else { installer_line(p, &dest) });
    #[cfg(not(windows))] c.args(p.args).arg(format!("/D={}", dest.display()));
    c.stdin(Stdio::null()).stdout(Stdio::from(out)).stderr(Stdio::from(err)).kill_on_drop(false);
    crate::plat::proc::own_group_tokio(&mut c);
    let mut child = c.spawn().map_err(|e| format!("Could not start the {} installer: {e}", p.name))?;
    let status = match tokio::time::timeout(std::time::Duration::from_secs(30 * 60), child.wait()).await {
        Ok(r) => r.map_err(|e| e.to_string())?,
        Err(_) => return Err(format!("The {} installer did not finish in 30 minutes", p.name)),
    };
    if !status.success() { return Err(installer_failed(p, status.code())) }
    // the program it placed; the hub uses it from now on
    let cmd = placed(p, &dest);
    if !cmd.is_file() { return Err(format!("The {} installer finished, but {} is not there", p.name, cmd.display())) }
    let cmd = cmd.to_string_lossy().into_owned();
    if inno(p) { git_pick_up(); } else { let d = hub.cfg.home.join("freecad"); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join("command"), &cmd); }
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
            assert_eq!(a[4], CODEX_TAG); assert_eq!(a[5].len(), 64); assert!(a[5].chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "a pinned lowercase sha256"); assert!(a[6].parse::<u64>().unwrap() > 0, "a pinned size");
            assert!(!script.contains("releases/latest"), "pinned, never the latest");
            assert_eq!(a[2], if os == "windows" { "codex.exe" } else { "codex" });
        }
        assert_eq!(plan("codex", "freebsd", "x64", false, false), Some(Plan::Link(CODEX_RELEASES)));
        assert!(matches!(plan("codex", "freebsd", "x64", false, true), Some(Plan::Sh(s, _)) if s.contains("npm install -g @openai/codex")), "npm only as a fallback, with Node already there");
    }
    #[test] fn each_agents_update_is_its_own_updater_and_codex_from_setup_is_installed_again() {
        let (t, x) = (Path::new("/h/.local/share/backplane/tools/bin"), Path::new("/h/.local/bin"));
        let run = |id: &str| update_plan(id, Path::new(&format!("{}/{id}", x.display())), t, "linux", "x64", false); // a Linux path, spelled as Linux does on any system
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
    #[cfg(unix)] fn codex_run(r: &Path, digest_ok: bool) -> (bool, String, PathBuf) { codex_run_with(r, digest_ok, 0) }
    /// `size_delta`: how far the pinned size is from the real one.
    #[cfg(unix)] fn codex_run_with(r: &Path, digest_ok: bool, size_delta: u64) -> (bool, String, PathBuf) {
        let asset = "codex-x86_64-unknown-linux-musl.tar.gz";
        let serve = r.join("serve"); std::fs::create_dir_all(serve.join("pkg")).unwrap();
        exe(&serve.join("pkg/codex-x86_64-unknown-linux-musl"), "#!/bin/sh\necho codex 9.9.9\n");
        assert!(std::process::Command::new("tar").arg("-czf").arg(serve.join(asset)).arg("-C").arg(serve.join("pkg")).arg("codex-x86_64-unknown-linux-musl").status().unwrap().success());
        let sum = String::from_utf8(std::process::Command::new("sha256sum").arg(serve.join(asset)).output().unwrap().stdout).unwrap().split(' ').next().unwrap().to_string();
        let bad = "0".repeat(64); let size = std::fs::metadata(serve.join(asset)).unwrap().len();
        // the pinned release's download address: the script asks for nothing else (no release listing is served)
        std::fs::write(serve.join("sentinel"), "x").unwrap();
        let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        exe(&bin.join("curl"), &format!("#!/bin/sh\nwhile [ $# -gt 1 ]; do [ \"$1\" = -o ] && out=$2; shift; done\nf={}/$(basename \"$1\")\n[ -f \"$f\" ] || exit 22\ncp \"$f\" \"$out\"\n", serve.display()));
        let dest = r.join("my tools/bin");
        let o = std::process::Command::new("sh").arg("-c").arg(CODEX_SH).arg("sh").arg(&dest).arg(asset).arg("codex").arg("tar.gz").arg("rust-v9.9.9").arg(if digest_ok { &sum } else { &bad }).arg((size + size_delta).to_string()).env("PATH", format!("{}:/usr/bin:/bin", bin.display())).output().unwrap();
        (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)), dest)
    }
    #[cfg(unix)] #[test] fn the_codex_script_places_a_binary_whose_digest_matches_and_runs() {
        let r = tmp("ok"); let (ok, out, dest) = codex_run(&r, true);
        assert!(ok, "{out}");
        assert_eq!(out.lines().collect::<Vec<_>>(), ["Downloading codex-x86_64-unknown-linux-musl.tar.gz", "Checking the checksum", &format!("Installed in {}/codex", dest.display())]);
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
        let r = tmp("wget"); let _ = codex_run(&r, true); let serve = r.join("serve");
        let sum = String::from_utf8(std::process::Command::new("sha256sum").arg(serve.join("codex-x86_64-unknown-linux-musl.tar.gz")).output().unwrap().stdout).unwrap().split(' ').next().unwrap().to_string();
        let size = std::fs::metadata(serve.join("codex-x86_64-unknown-linux-musl.tar.gz")).unwrap().len(); let dest = r.join("w/bin");
        let run = |p: &Path| { let o = std::process::Command::new("sh").arg("-c").arg(CODEX_SH).arg("sh").arg(&dest).arg("codex-x86_64-unknown-linux-musl.tar.gz").arg("codex").arg("tar.gz").arg("rust-v9.9.9").arg(&sum).arg(size.to_string()).env("PATH", p).output().unwrap(); (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))) };
        let (ok, out) = run(&no_curl(&r, &serve, false)); assert!(!ok && out.contains("curl or wget is needed to download Codex"), "{out}");
        let (ok, out) = run(&no_curl(&r, &serve, true)); assert!(ok, "{out}"); assert!(dest.join("codex").is_file());
        // the vendor wrapper (Claude, Grok, OpenCode) downloads its installer with wget too
        std::fs::write(serve.join("latest"), "echo vendor script\n").unwrap();
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
    #[cfg(unix)] #[test] fn the_codex_script_places_nothing_when_the_size_differs() {
        let r = tmp("size"); let (ok, out, dest) = codex_run_with(&r, true, 1);
        assert!(!ok && out.trim_end().ends_with("Codex size mismatch"), "{out}");
        assert!(!dest.join("codex").exists(), "nothing placed");
        let _ = std::fs::remove_dir_all(&r);
    }
    #[cfg(unix)] #[test] fn the_codex_script_refuses_a_missing_pin() {
        let r = tmp("nopin"); let o = std::process::Command::new("sh").arg("-c").arg(CODEX_SH).arg("sh").arg(r.join("d")).arg("codex-x.tar.gz").arg("codex").arg("tar.gz").arg("rust-v1").arg("").arg("1").output().unwrap();
        assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("No pinned checksum"));
        let _ = std::fs::remove_dir_all(&r);
    }
    #[test] fn the_codex_pins_are_whole_and_the_notice_names_the_vendor_script() {
        assert_eq!(CODEX_PINS.len(), 6);
        for (os, arch, (asset, name, kind, sha, size)) in CODEX_PINS { assert_eq!(codex_asset(os, arch).unwrap().0, *asset); assert!(asset.starts_with("codex-") && !asset.contains(['/', '\\'])); assert!(!name.is_empty() && (*kind == "tar.gz" || *kind == "raw")); assert!(sha.len() == 64 && sha.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "{asset}"); assert!(*size > 0); }
        assert!(CODEX_TAG.starts_with("rust-v"));
        let n = install_notice("claude", "linux"); assert!(n.contains("runs the vendor's own installer script from https://claude.ai/install.sh") && n.contains("not checked"), "{n}");
        assert!(install_notice("claude", "windows").contains("https://claude.ai/install.ps1"));
        assert!(install_notice("grok", "darwin").contains(GROK_SH) && install_notice("opencode", "linux").contains(OPENCODE_SH));
        assert!(install_notice("codex", "linux").contains(CODEX_TAG) && install_notice("codex", "linux").contains("sha256"));
    }
    #[cfg(unix)] #[test] fn the_vendor_script_stops_when_the_download_fails() {
        let r = tmp("vendor"); let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        exe(&bin.join("curl"), "#!/bin/sh\nexit 22\n");
        let o = std::process::Command::new("sh").arg("-c").arg(VENDOR_SH).arg("sh").arg(CLAUDE_SH).env("PATH", format!("{}:/usr/bin:/bin", bin.display())).output().unwrap();
        assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("Could not download https://claude.ai/install.sh"));
        let _ = std::fs::remove_dir_all(&r);
    }
    /// Git: winget, else Git for Windows' pinned installer (per user, Inno's own switches and folder); macOS and Linux their own scripts.
    #[test] fn git_installs_on_every_system_and_windows_needs_no_winget() {
        assert_eq!(plan("git", "windows", "x64", true, false), Some(Plan::Winget("Git.Git")));
        let Some(Plan::Direct(p)) = plan("git", "windows", "x64", false, false) else { panic!("no winget: the pinned installer") }; assert_eq!(p, &GIT_WIN);
        check_pin(&GIT_WIN).unwrap();
        let d = install_dir(Path::new("C:\\Users\\A B\\AppData\\Local"), &GIT_WIN); assert!(d.ends_with("Programs/Git") || d.to_string_lossy().ends_with("Programs\\Git"), "{}", d.display());
        assert!(installer_line(&GIT_WIN, &d).starts_with("/VERYSILENT /NORESTART /NOCANCEL /SP- /SUPPRESSMSGBOXES /CURRENTUSER /DIR=\""));
        assert_eq!(uninstall_key(&GIT_WIN), "Git_is1");
        assert_eq!(elevated_line(&GIT_WIN), "/VERYSILENT /NORESTART /NOCANCEL /SP- /SUPPRESSMSGBOXES", "elevated: no /CURRENTUSER, no /DIR");
        assert!(machine_git_note(Path::new("C:/Program Files/Git")).starts_with("Git for all users is installed in C:/Program Files/Git, and Backplane does not move it.")); assert!(placed(&GIT_WIN, Path::new("G")).ends_with("cmd/git.exe") || placed(&GIT_WIN, Path::new("G")).to_string_lossy().ends_with("cmd\\git.exe"));
        assert_eq!(plan("git", "darwin", "arm64", false, false), Some(Plan::Sh(GIT_MAC_SH, vec![]))); assert_eq!(plan("git", "linux", "x64", false, false), Some(Plan::Sh(GIT_LINUX_SH, vec![])));
        let v = |k: &str| match k { "ProgramFiles" => Some("C:\\PF".into()), "LOCALAPPDATA" => Some("C:\\L".into()), _ => None };
        assert_eq!(git_dirs(v).len(), 2);
        // the Linux script with no desktop: the command to run, never a hang on a password prompt
        #[cfg(unix)] {
            // only a stand-in apt-get on its PATH (no pkexec: nothing may ask for a password on this machine)
            let d = crate::plat::tmp::dir("git-linux-sh"); std::fs::write(d.join("apt-get"), "#!/bin/sh\nexit 0\n").unwrap(); std::fs::set_permissions(d.join("apt-get"), std::fs::Permissions::from_mode(0o755)).unwrap();
            let sh = ["/bin/sh", "/usr/bin/sh"].into_iter().find(|p| Path::new(p).exists()).unwrap();
            let o = std::process::Command::new(sh).arg("-c").arg(GIT_LINUX_SH).env_clear().env("PATH", d.as_os_str()).output().unwrap();
            let out = String::from_utf8_lossy(&o.stdout); assert!(!o.status.success() && out.trim() == "Run in a terminal: sudo apt-get install -y git", "{out}{}", String::from_utf8_lossy(&o.stderr));
        }
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
