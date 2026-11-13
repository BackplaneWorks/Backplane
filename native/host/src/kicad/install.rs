//! Install KiCad (0.14's `kicad-install` on Settings > CAD: core/kicad_install and server/kicad's `KiCad.install`). The published Backplane
//! KiCad for this hub's OS and architecture, downloaded from its GitHub release, checked against the digest pinned here and the release's
//! SHA256SUMS, unpacked beside the hub's data (`<home>/kicad/<version>`, where `ipc::runtime` looks) and only then moved into place.
//! Nothing privileged ever runs: no sudo, no package manager, nothing outside that folder, so there is no password to ask for.
//! `kicad.install {consent:true}` runs it detached (its own process group, its output into `<home>/kicad/.install.log`, so a hub
//! restart neither kills it nor leaves it half-placed) and says how it goes in the info key `kicad.install` (Settings > CAD's row).
//! It refuses without the person's consent, while one runs, on a hub with no published package, and when KiCad is already found.
use crate::hub::Hub;
use serde_json::json;
use std::{path::{Path, PathBuf}, process::Stdio, sync::{atomic::{AtomicBool, Ordering}, Arc}};

#[derive(Debug, PartialEq)]
pub struct Package { pub platform: &'static str, pub version: &'static str, pub release: &'static str, pub asset: &'static str, pub sha256: &'static str, pub format: &'static str, pub cli: &'static str }
/// Linux and Windows: bp.10 (the embedded editor with live theme, reload on disk, cross-probe, comments, align keys and the fast switch; the
/// person's default, Nov 11: the newest fork release each time); Apple
/// Silicon keeps bp.6 (bp.8 has no macOS build).
pub const LINUX: Package = Package { platform: "linux-x86_64", version: "10.99.0-bp.10", release: "backplane-kicad-v10.99.0-bp.10", asset: "backplane-kicad-linux-x86_64-10.99.0-bp.10.tar.gz", sha256: "21f67f7fab54800831394e3ef80557016f769d99e46f694e7cccb386f7a41181", format: "tar.gz", cli: "bin/kicad-cli" };
pub const MAC: Package = Package { platform: "darwin-arm64", version: "10.99.0-bp.6", release: "backplane-kicad-v10.99.0-bp.6", asset: "backplane-kicad-darwin-arm64-10.99.0-bp.6.tar.gz", sha256: "99584fb0ee490354098778d39e3aaed9e0f274ee39972851a9cbeabfb271462d", format: "tar.gz", cli: "bin/kicad-cli" };
pub const WINDOWS: Package = Package { platform: "windows-x86_64", version: "10.99.0-bp.10", release: "backplane-kicad-v10.99.0-bp.10", asset: "backplane-kicad-windows-x86_64-10.99.0-bp.10.zip", sha256: "68b3796f1de3a8c0a39dd20ca96a43f3354de47facbe5de45d0bd4668c32599c", format: "zip", cli: "bin/kicad-cli.exe" };
/// Where the releases are (said in the consent note, and the script's download base).
pub const FROM: &str = "github.com/i2cjak/Backplane_KiCad";

/// Install.pick: the package for an OS and architecture (Rust's names or uname's), None where none is published.
pub fn pick(os: &str, arch: &str) -> Option<&'static Package> {
    let os = os.to_lowercase(); let arch = arch.to_lowercase();
    let os = if ["cygwin_nt-", "mingw64_nt-", "msys_nt-"].iter().any(|p| os.starts_with(p)) || os == "windows_nt" || os == "win32" { "windows".to_string() } else if os == "macos" { "darwin".into() } else { os };
    let arch = match arch.as_str() { "amd64" | "x64" => "x86_64", "aarch64" => "arm64", a => a };
    match (os.as_str(), arch) { ("linux", "x86_64") => Some(&LINUX), ("darwin", "arm64") => Some(&MAC), ("windows", "x86_64") => Some(&WINDOWS), _ => None }
}
/// This hub's package (the hub's own OS, never a connecting client's).
pub fn package() -> Option<&'static Package> { pick(std::env::consts::OS, std::env::consts::ARCH) }
/// Where installs go: the folder `ipc::runtime` searches newest first. Windows: %LOCALAPPDATA%\Backplane\kicad, because the hub's home is in the
/// roaming profile and a runtime of about a gigabyte does not belong there; everywhere else `<home>/kicad`.
pub fn parent(home: &Path) -> PathBuf { managed_parent(cfg!(windows), std::env::var_os("LOCALAPPDATA").as_deref(), home) }
pub fn managed_parent(windows: bool, local_app_data: Option<&std::ffi::OsStr>, home: &Path) -> PathBuf {
    match local_app_data.filter(|d| windows && !d.is_empty()) { Some(d) => Path::new(d).join("Backplane").join("kicad"), None => home.join("kicad") }
}
/// This computer in words, for "none is published for …".
pub fn this_computer() -> String { format!("{} {}", std::env::consts::OS, std::env::consts::ARCH) }

/// 0.14's install script (Install.script), with a line on stdout per step. Arguments come only from the catalog above, never a client.
/// It never unpacks over an existing runtime, holds a lock folder, stages beside the destination and removes the stage however it ends.
pub const SCRIPT: &str = r#"set -eu
parent=$1; platform=$2; version=$3; release=$4; asset=$5; pinned=$6; format=$7; cli=$8
fail() { echo "$*" >&2; exit 1; }
case "$platform:$format:$cli" in
  linux-x86_64:tar.gz:bin/kicad-cli|darwin-arm64:tar.gz:bin/kicad-cli|windows-x86_64:zip:bin/kicad-cli.exe) ;;
  *) fail 'No published Backplane KiCad package for this hub platform' ;;
esac
# Windows: the hub hands over a native path (C:\Users\Some Name\...): the shell works on its unix form, and every use below is quoted
case "$platform" in windows-*) parent=$(cygpath -u "$parent") || fail 'cygpath could not read the install folder' ;; esac
case "$version" in ''|*[!0-9A-Za-z.-]*) fail 'Invalid runtime version' ;; esac
[ "$release" = "backplane-kicad-v$version" ] || fail 'Invalid runtime release'
root=backplane-kicad-$platform-$version
[ "$asset" = "$root.$format" ] || fail 'Invalid runtime archive'
case "$pinned" in ''|*[!0-9a-f]*) fail 'Invalid pinned digest' ;; esac
[ "${#pinned}" -eq 64 ] || fail 'Invalid pinned digest'
# curl, else wget: a stock Ubuntu desktop has wget and no curl
if command -v curl >/dev/null 2>&1; then get() { curl --proto '=https' --tlsv1.2 -fsSL -o "$1" "$2"; }
elif command -v wget >/dev/null 2>&1; then get() { wget -q --https-only -O "$1" "$2"; }
else fail 'curl or wget is needed to download KiCad, and neither was found'; fi
dir=$parent/$version
# Never unpack over an existing runtime (including an incomplete old install).
[ ! -e "$dir" ] && [ ! -L "$dir" ] || fail "$dir already exists; the installation there is kept. Set its path in Settings, CAD, or remove it and try again"
umask 077
mkdir -p "$parent" || fail "Could not make $parent"
lock=$parent/.install-$version.lock
mkdir "$lock" 2>/dev/null || fail 'Another KiCad installation is active; retry when it finishes'
stage=
cleanup() { if [ -n "$stage" ]; then rm -rf "$stage"; fi; rmdir "$lock"; }
trap cleanup 0
trap 'exit 1' 1 2 15
stage=$(mktemp -d "$parent/.install-XXXXXX")
pkg=$stage/$asset
base=https://github.com/i2cjak/Backplane_KiCad/releases/download/$release/
echo "Downloading $asset"
get "$pkg" "$base$asset" || fail "Could not download $base$asset"
get "$stage/SUMS" "${base}SHA256SUMS" || fail "Could not download ${base}SHA256SUMS"
echo 'Checking the checksum'
want=$(awk -v n="$asset" '{ f=$2; sub(/^\*/, "", f); sub(/^\.\//, "", f); if (f==n) print $1 }' "$stage/SUMS")
if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$pkg" | cut -d' ' -f1)
else
  got=$(shasum -a 256 "$pkg" | cut -d' ' -f1)
fi
[ "$want" = "$got" ] && [ "$got" = "$pinned" ] || fail 'KiCad archive checksum mismatch'
magic=$(od -An -N4 -tx1 "$pkg" | tr -d ' \n')
mkdir "$stage/unpack"
echo 'Unpacking'
case "$format:$magic" in
  tar.gz:1f8b08*)
    tar -tzf "$pkg" > "$stage/members"
    while IFS= read -r member; do
      case "$member" in ./*) member=${member#./} ;; esac
      case "/$member/" in *'/../'*|*'/./'*) fail 'Unsafe runtime archive path' ;; esac
      case "$member" in "$root"|"$root/"*) ;; *) fail 'Unexpected runtime archive root' ;; esac
    done < "$stage/members"
    tar -xzf "$pkg" -C "$stage/unpack"
    ;;
  zip:504b0304*)
    # Windows ships PowerShell outside the packaged runtime PATH.
    powershell=$(cygpath -S)/WindowsPowerShell/v1.0/powershell.exe
    [ -x "$powershell" ] || fail 'Windows PowerShell is missing from the system directory'
    # Pass native paths as environment values, never interpolate shell text.
    BP_KICAD_ZIP=$(cygpath -w "$pkg") \
    BP_KICAD_OUT=$(cygpath -w "$stage/unpack") \
    BP_KICAD_ROOT=$root "$powershell" -NoProfile -NonInteractive -Command '
      $ErrorActionPreference="Stop"
      Add-Type -AssemblyName System.IO.Compression.FileSystem
      $zip=[IO.Compression.ZipFile]::OpenRead($env:BP_KICAD_ZIP)
      try {
        foreach($entry in $zip.Entries) {
          $n=$entry.FullName.Replace("\", "/")
          if (!$n.StartsWith($env:BP_KICAD_ROOT+"/", [StringComparison]::Ordinal) -or
              @($n.Split("/") | Where-Object {$_ -eq ".." -or $_ -eq "."}).Count -gt 0 -or
              $n.Contains(":")) { throw "Unsafe runtime archive path" }
        }
      } finally { $zip.Dispose() }
      Expand-Archive -LiteralPath $env:BP_KICAD_ZIP -DestinationPath $env:BP_KICAD_OUT -ErrorAction Stop
    '
    ;;
  *) fail 'KiCad archive header does not match its platform format' ;;
esac
runtime=$stage/unpack/$root
[ -f "$runtime/$cli" ] && [ -s "$runtime/manifest.json" ] && [ -s "$runtime/share/backplane/api.desc" ] || fail 'KiCad archive runtime layout is incomplete'
if [ "$platform" = windows-x86_64 ]; then
  [ "$(od -An -N2 -tx1 "$runtime/$cli" | tr -d ' \n')" = 4d5a ] || fail 'Windows KiCad CLI is not a PE executable'
else
  [ -x "$runtime/$cli" ] || fail 'KiCad launcher is not executable'
fi
tr -d '[:space:]' < "$runtime/manifest.json" > "$stage/manifest"
grep -Fq "\"version\":\"$version\"" "$stage/manifest" || fail 'KiCad manifest version mismatch'
grep -Fq "\"platform\":\"$platform\"" "$stage/manifest" || fail 'KiCad manifest platform mismatch'
[ ! -e "$dir" ] && [ ! -L "$dir" ] || fail 'Runtime destination appeared; existing installation preserved'
mv "$runtime" "$dir"
echo "Installed in $dir"
"#;

/// The installer's arguments (the script's $1..$8).
pub fn args(parent: &Path, p: &Package) -> Vec<String> {
    [parent.to_string_lossy().as_ref(), p.platform, p.version, p.release, p.asset, p.sha256, p.format, p.cli].iter().map(|s| s.to_string()).collect()
}
/// What setup does about KiCad, from the package this computer has (`pick`), the status of the KiCad found (`ipc::publish_cad`: ok, old, stock,
/// broken, missing, checking) and the install's own state in info `kicad.install` ("", running, failed, done).
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Action {
    /// Nothing to do yet: still looking, or an install is running.
    Wait,
    /// A compatible Backplane KiCad is there.
    Nothing,
    /// None found and a package exists: setup starts the install by itself, once.
    Auto,
    /// A KiCad is there that is not a compatible Backplane KiCad (stock, old, broken), or a first try failed: one press installs the fork.
    Offer,
    /// No package for this computer: say so, offer the path field and the stock download.
    NoPackage,
}
/// Whether the Backplane KiCad found (`version`, as `kicad.version` says it) is an older build of the same line than the package: one press
/// updates it (the new one goes beside it, and `ipc::runtime` takes the newest). Stock and other lines are never "older".
pub fn older(version: &str, p: &Package) -> bool {
    // "-bp.12" and a hotfix's "-bp.12.1" (each dot part a number; a build suffix like "-851cbefda6" is not one, so not comparable)
    let bp = |v: &str| v.split_once("-bp.").and_then(|(base, n)| n.split('.').map(|x| x.parse::<u32>().ok()).collect::<Option<Vec<u32>>>().map(|n| (base.to_string(), n)));
    matches!((bp(version), bp(p.version)), (Some((a, n)), Some((b, m))) if a == b && n < m)
}
pub fn decide(pkg: Option<&Package>, status: &str, version: &str, install_state: &str) -> Action {
    if install_state == "running" { return Action::Wait }
    match (status, pkg) {
        ("ok", Some(p)) if older(version, p) && install_state != "failed" => Action::Offer,
        ("ok", _) => Action::Nothing,
        ("" | "checking", _) => Action::Wait,
        (_, None) => Action::NoPackage,
        ("missing", Some(_)) if install_state != "failed" => Action::Auto,
        _ => Action::Offer,
    }
}
/// Why a request is refused, None when it may run. `found`: the KiCad `ipc::runtime` found ("" for none); `status` its status. Only a compatible
/// Backplane KiCad refuses: a stock or old one is what the install replaces as the runtime the hub picks (the managed folder is searched first).
pub fn refusal(found: &str, status: &str, version: &str, consent: bool, running: bool, p: Option<&Package>) -> Option<String> {
    if running { return Some("KiCad is being installed already".into()) }
    if status == "ok" && !p.map_or(false, |p| older(version, p)) { return Some(format!("Backplane KiCad is already installed: {found}")) }
    let Some(p) = p else { return Some(format!("No Backplane KiCad is published for this computer ({}). Install KiCad from kicad.org, then set its path in Settings, CAD", this_computer())) };
    if !consent { return Some(format!("Installing needs your consent: it downloads Backplane KiCad {} from {FROM}. Press Install in Settings, CAD", p.version)) }
    None
}
/// The last line the installer wrote ("" for none).
pub fn last_line(text: &str) -> String { text.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(300).collect() }

/// Run the installer to its end: `standin` (a program given the same arguments) or the script under `sh`, detached, its output into `log`.
/// `step` hears each new last line while it runs. Ok(the installed folder) or Err(why, the installer's own last words).
pub async fn run(standin: Option<&Path>, parent: &Path, p: &Package, log: &Path, mut step: impl FnMut(String)) -> Result<String, String> {
    std::fs::create_dir_all(parent).map_err(|e| format!("could not make {}: {e}", parent.display()))?;
    let out = std::fs::File::create(log).map_err(|e| format!("could not write {}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    // Net: it downloads (a copy of the log runs none)
    let mut c = match standin { Some(s) => crate::proc::tokio_cmd(crate::proc::Kind::Net, s), None => { let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, "sh"); c.arg("-c").arg(SCRIPT).arg("sh"); c } };
    c.args(args(parent, p)).current_dir(parent).stdin(Stdio::null()).stdout(Stdio::from(out)).stderr(Stdio::from(err));
    crate::plat::proc::own_group_tokio(&mut c);
    // "Text file busy": an installer written a moment ago is still open for writing in a child another thread just forked; it passes in a few milliseconds
    let mut tries = 0;
    let mut child = loop {
        match c.spawn() {
            Ok(c) => break c,
            Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) && tries < 40 => { tries += 1; tokio::time::sleep(std::time::Duration::from_millis(25)).await }
            Err(e) => return Err(format!("could not start the installer: {e}")),
        }
    };
    if let Some(id) = child.id() { crate::plat::proc::adopt(id, false) }
    let mut said = String::new();
    let status = loop {
        let done = tokio::time::timeout(std::time::Duration::from_millis(300), child.wait()).await;
        let now = last_line(&std::fs::read_to_string(log).unwrap_or_default());
        if !now.is_empty() && now != said { said = now.clone(); step(now) }
        if let Ok(r) = done { break r.map_err(|e| e.to_string())? }
    };
    let dir = parent.join(p.version);
    if status.success() && dir.join(p.cli).is_file() { Ok(dir.to_string_lossy().into()) }
    else if status.success() { Err(format!("the installer ended without {}", dir.join(p.cli).display())) }
    else { Err(if said.is_empty() { format!("the installer stopped ({status})") } else { said }) }
}

static RUNNING: AtomicBool = AtomicBool::new(false);
/// What Settings > CAD's row says: `{"state":"running"|"failed"|"done","version","note"}` in info `kicad.install`.
/// An info key is not in the log, so open views are told to ask again (`bump_views`): the row moves with each step.
async fn say(hub: &Arc<Hub>, state: &str, version: &str, note: &str) { hub.set_info("kicad.install", &json!({"state": state, "version": version, "note": note}).to_string()).await; hub.bump_views(); }

/// `kicad.install {consent}`: refused, or started (the reply says so; the info key says the rest).
pub async fn install(hub: &Arc<Hub>, consent: bool) -> Result<String, String> {
    if crate::copy::on() { return Err("a copy of the log installs nothing".into()) }
    let env = super::ipc::runtime(hub).await;
    if let Some(why) = refusal(&env.cli, &env.status, &env.version, consent, RUNNING.load(Ordering::SeqCst), package()) { return Err(why) }
    let p = package().expect("refusal checks the package");
    if RUNNING.swap(true, Ordering::SeqCst) { return Err("KiCad is being installed already".into()) }
    let parent = parent(&hub.cfg.home);
    // a stand-in installer (tests and rigs): the same arguments, so no real KiCad is downloaded or touched
    let standin = std::env::var_os("BACKPLANE_KICAD_INSTALLER").filter(|s| !s.is_empty()).map(PathBuf::from);
    eprintln!("kicad: installing Backplane KiCad {} into {}{}", p.version, parent.display(), standin.as_ref().map_or(String::new(), |s| format!(" (stand-in {})", s.display())));
    say(hub, "running", p.version, &format!("Starting the download from {FROM}")).await;
    let started = format!("installing Backplane KiCad {} into {}", p.version, parent.display());
    let h = hub.clone();
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let h2 = h.clone();
        let relay = tokio::spawn(async move { while let Some(l) = rx.recv().await { say(&h2, "running", p.version, &l).await } });
        let r = run(standin.as_deref(), &parent, p, &parent.join(".install.log"), |l| { let _ = tx.send(l); }).await;
        drop(tx); let _ = relay.await;
        RUNNING.store(false, Ordering::SeqCst);
        // Settings > CAD names the new KiCad (and its Install row goes) before the install says it is done: whoever reads "done" reads the new status with it
        super::ipc::publish_cad(&h).await;
        match &r {
            Ok(dir) => { eprintln!("kicad: installed Backplane KiCad {} in {dir}", p.version); say(&h, "done", p.version, &format!("Installed Backplane KiCad {} in {dir}", p.version)).await }
            Err(why) => { eprintln!("kicad: install failed: {why}"); say(&h, "failed", p.version, &format!("KiCad install failed: {why}")).await }
        }
        h.bump_views();
    });
    Ok(started)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)] use std::os::unix::fs::PermissionsExt;
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("kinst-{n}")) }
    #[cfg(unix)] fn exe(p: &Path, body: &str) { std::fs::write(p, body).unwrap(); std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap() }
    fn rt() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap() }

    #[test] fn the_catalog_is_0_14s_by_the_hubs_own_os_and_arch() {
        assert_eq!(pick("linux", "x86_64"), Some(&LINUX)); assert_eq!(pick("Linux", "amd64"), Some(&LINUX));
        assert_eq!(pick("macos", "aarch64"), Some(&MAC)); assert_eq!(pick("Darwin", "arm64"), Some(&MAC));
        assert_eq!(pick("windows", "x86_64"), Some(&WINDOWS)); assert_eq!(pick("MINGW64_NT-10.0", "x86_64"), Some(&WINDOWS));
        assert_eq!(pick("linux", "aarch64"), None); assert_eq!(pick("macos", "x86_64"), None);
        for p in [&LINUX, &MAC, &WINDOWS] { assert_eq!(p.release, format!("backplane-kicad-v{}", p.version)); assert_eq!(p.asset, format!("backplane-kicad-{}-{}.{}", p.platform, p.version, p.format)); assert_eq!(p.sha256.len(), 64) }
    }
    #[test] fn windows_installs_under_local_app_data_and_others_beside_the_home() {
        let home = Path::new("/h/hub");
        assert_eq!(managed_parent(true, Some(std::ffi::OsStr::new("C:\\Users\\Some Name\\AppData\\Local")), home), Path::new("C:\\Users\\Some Name\\AppData\\Local").join("Backplane").join("kicad"));
        assert_eq!(managed_parent(true, None, home), home.join("kicad"), "no LOCALAPPDATA: beside the home");
        assert_eq!(managed_parent(false, Some(std::ffi::OsStr::new("/x")), home), home.join("kicad"), "LOCALAPPDATA means nothing off Windows");
    }
    #[test] fn the_script_turns_a_windows_folder_into_a_shell_path_before_using_it() {
        let (c, u) = (SCRIPT.find("cygpath -u \"$parent\"").expect("cygpath -u"), SCRIPT.find("dir=$parent/$version").unwrap());
        assert!(c < u, "converted before the first use");
        assert!(SCRIPT.contains("windows-*)"), "only on Windows");
    }
    #[test] fn nothing_privileged_ever_runs() {
        for w in ["sudo", "pkexec", "doas", "apt", "dnf", "yum", "pacman", "zypper", "brew", "flatpak", "snap", "winget", "choco", "/usr", "/opt", "/Applications"] {
            assert!(!SCRIPT.split(|c: char| !(c.is_alphanumeric() || c == '/')).any(|t| t == w), "{w} in the script");
        }
    }
    #[test] fn refused_when_found_running_unpublished_or_without_consent() {
        assert_eq!(refusal("/k/bin/kicad-cli", "ok", "10.99.0-bp.10", true, false, Some(&LINUX)), Some("Backplane KiCad is already installed: /k/bin/kicad-cli".into()));
        assert_eq!(refusal("", "missing", "", true, true, Some(&LINUX)), Some("KiCad is being installed already".into()));
        assert!(refusal("", "missing", "", true, false, None).unwrap().starts_with("No Backplane KiCad is published for this computer"));
        for s in ["stock", "old", "broken"] { assert_eq!(refusal("/usr/bin/kicad-cli", s, "", true, false, Some(&LINUX)), None, "{s}: the fork is installed beside it"); }
        let no = refusal("", "missing", "", false, false, Some(&LINUX)).unwrap();
        assert!(no.starts_with("Installing needs your consent") && no.contains("10.99.0-bp.10") && no.contains(FROM), "{no}");
        assert_eq!(refusal("", "missing", "", true, false, Some(&LINUX)), None);
    }
    /// The decision setup makes for each computer and each KiCad it finds: (os, arch, status, install state) -> action.
    #[test] fn setup_decides_by_computer_and_what_it_found() {
        use Action::*;
        let table: &[(&str, &str, &str, &str, Action)] = &[
            ("linux", "x86_64", "missing", "", Auto), ("macos", "aarch64", "missing", "", Auto), ("windows", "x86_64", "missing", "", Auto),
            ("linux", "x86_64", "ok", "", Nothing), ("macos", "aarch64", "ok", "done", Nothing), ("windows", "x86_64", "ok", "", Nothing),
            ("linux", "x86_64", "stock", "", Offer), ("macos", "aarch64", "old", "", Offer), ("windows", "x86_64", "broken", "", Offer),
            ("linux", "x86_64", "missing", "failed", Offer), ("linux", "x86_64", "missing", "running", Wait), ("linux", "x86_64", "stock", "running", Wait),
            ("linux", "x86_64", "checking", "", Wait), ("linux", "x86_64", "", "", Wait),
            ("linux", "aarch64", "missing", "", NoPackage), ("macos", "x86_64", "missing", "", NoPackage), ("linux", "aarch64", "stock", "", NoPackage), ("macos", "x86_64", "stock", "", NoPackage),
            ("linux", "aarch64", "ok", "", Nothing), ("linux", "aarch64", "checking", "", Wait),
        ];
        for (os, arch, status, state, want) in table { assert_eq!(decide(pick(os, arch), status, "", state), *want, "{os} {arch} {status} {state}"); }
    }
    /// An older build of the pinned line (bp.4 when bp.8 is pinned) is offered the update; the same or a newer one, or stock, is not.
    #[test] fn an_older_backplane_kicad_is_offered_the_update() {
        let at = |v: &'static str| Package { version: v, ..LINUX };
        assert!(older("10.99.0-bp.12", &at("10.99.0-bp.12.1")) && older("10.99.0-bp.11", &at("10.99.0-bp.12.1")) && !older("10.99.0-bp.12.1", &at("10.99.0-bp.12.1")));
        assert!(!older("10.99.0-bp.12.1", &at("10.99.0-bp.12")) && older("10.99.0-bp.12.1", &at("10.99.0-bp.13")) && !older("10.99.0-bp.12-851cbefda6", &at("10.99.0-bp.13")));
        assert!(older("10.99.0-bp.4", &LINUX) && older("10.99.0-bp.8", &LINUX) && older("10.99.0-bp.9", &LINUX) && !older("10.99.0-bp.10", &LINUX) && !older("10.99.0-bp.11", &LINUX) && !older("10.0.7", &LINUX) && !older("", &LINUX));
        assert_eq!(decide(Some(&LINUX), "ok", "10.99.0-bp.4", ""), Action::Offer);
        assert_eq!(decide(Some(&LINUX), "ok", "10.99.0-bp.4", "failed"), Action::Nothing, "a failed update leaves the working one alone");
        assert_eq!(decide(Some(&LINUX), "ok", "10.99.0-bp.10", ""), Action::Nothing);
        assert_eq!(refusal("/k/bin/kicad-cli", "ok", "10.99.0-bp.4", true, false, Some(&LINUX)), None);
    }

    /// A disposable package for the real script: its archive and SHA256SUMS, served by a stand-in `curl` on PATH.
    #[cfg(unix)] fn fixture(r: &Path, bad_manifest: bool) -> (Package, PathBuf) {
        let (v, plat) = ("0.0.1-bp.1", "linux-x86_64"); let root = format!("backplane-kicad-{plat}-{v}");
        let src = r.join("src").join(&root); std::fs::create_dir_all(src.join("bin")).unwrap(); std::fs::create_dir_all(src.join("share/backplane")).unwrap();
        exe(&src.join("bin/kicad-cli"), "#!/bin/sh\necho 0.0.1-bp.1\n");
        std::fs::write(src.join("manifest.json"), format!(r#"{{"version": "{}", "platform": "{plat}"}}"#, if bad_manifest { "9.9" } else { v })).unwrap();
        std::fs::write(src.join("share/backplane/api.desc"), "x").unwrap();
        let serve = r.join("serve"); std::fs::create_dir_all(&serve).unwrap(); let asset = format!("{root}.tar.gz");
        assert!(std::process::Command::new("tar").arg("-czf").arg(serve.join(&asset)).arg("-C").arg(r.join("src")).arg(&root).status().unwrap().success());
        let sum = String::from_utf8(std::process::Command::new("sha256sum").arg(serve.join(&asset)).output().unwrap().stdout).unwrap().split(' ').next().unwrap().to_string();
        std::fs::write(serve.join("SHA256SUMS"), format!("{sum}  {asset}\n")).unwrap();
        let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        // curl -fsSL -o OUT URL: the file of that name from serve/ (fails like curl -f for anything else)
        exe(&bin.join("curl"), &format!("#!/bin/sh\nwhile [ $# -gt 1 ]; do [ \"$1\" = -o ] && out=$2; shift; done\nf={}/$(basename \"$1\")\n[ -f \"$f\" ] || exit 22\ncp \"$f\" \"$out\"\n", serve.display()));
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        (Package { platform: plat, version: v, release: leak(format!("backplane-kicad-v{v}")), asset: leak(asset), sha256: leak(sum), format: "tar.gz", cli: "bin/kicad-cli" }, bin)
    }
    #[cfg(unix)] fn script(bin: &Path, parent: &Path, p: &Package) -> (bool, String) {
        let o = std::process::Command::new("sh").arg("-c").arg(SCRIPT).arg("sh").args(args(parent, p)).env("PATH", format!("{}:/usr/bin:/bin", bin.display())).output().unwrap();
        (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
    }
    #[cfg(unix)] #[test] fn the_real_script_installs_a_checked_package_and_says_each_step() {
        let r = tmp("ok"); let (p, bin) = fixture(&r, false); let parent = r.join("home/kicad");
        let (ok, out) = script(&bin, &parent, &p);
        assert!(ok, "{out}");
        assert_eq!(out.lines().collect::<Vec<_>>(), [format!("Downloading {}", p.asset).as_str(), "Checking the checksum", "Unpacking", &format!("Installed in {}/0.0.1-bp.1", parent.display())]);
        assert!(parent.join("0.0.1-bp.1/bin/kicad-cli").is_file());
        let left: Vec<_> = std::fs::read_dir(&parent).unwrap().flatten().map(|e| e.file_name()).collect(); assert_eq!(left.len(), 1, "no stage or lock is left: {left:?}");
        // ipc::runtime finds it there (newest_in the hub's kicad folder reads its manifest)
        let env = crate::files::kicad::ipc::read_runtime(&parent.join("0.0.1-bp.1/bin/kicad-cli")); assert_eq!(env.version, "0.0.1-bp.1");
        // a second run keeps what is there
        let (ok, out) = script(&bin, &parent, &p); assert!(!ok && out.contains("already exists; the installation there is kept"), "{out}");
    }
    /// A PATH with this machine's programs but curl and wget, and a stand-in `wget -q --https-only -O OUT URL` serving the fixture's files
    /// (`wget`: whether to put it there).
    #[cfg(unix)] fn no_curl(r: &Path, fixture_bin: &Path, wget: bool) -> PathBuf {
        let b = r.join(if wget { "wget-only" } else { "neither" }); std::fs::create_dir_all(&b).unwrap();
        for d in ["/usr/local/bin", "/usr/bin", "/bin"] { for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let n = e.file_name(); if n == "curl" || n == "wget" || b.join(&n).exists() { continue } let _ = std::os::unix::fs::symlink(e.path(), b.join(&n));
        } }
        let serve = std::fs::read_to_string(fixture_bin.join("curl")).unwrap().split("f=").nth(1).unwrap().split("/$(").next().unwrap().to_string();
        if wget { exe(&b.join("wget"), &format!("#!/bin/sh
echo \"$*\" >> {}/wget.args
while [ $# -gt 1 ]; do [ \"$1\" = -O ] && out=$2; shift; done
f={serve}/$(basename \"$1\")
[ -f \"$f\" ] || exit 8
cp \"$f\" \"$out\"
", r.display())); }
        b
    }
    #[cfg(unix)] #[test] fn with_no_curl_the_script_downloads_with_wget_and_with_neither_says_so() {
        let r = tmp("wget"); let (p, bin) = fixture(&r, false); let parent = r.join("home/kicad");
        let only = |b: &Path| { let o = std::process::Command::new("sh").arg("-c").arg(SCRIPT).arg("sh").args(args(&parent, &p)).env("PATH", b).output().unwrap(); (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))) };
        let (ok, out) = only(&no_curl(&r, &bin, false));
        assert!(!ok && out.trim_end().ends_with("curl or wget is needed to download KiCad, and neither was found"), "{out}");
        let (ok, out) = only(&no_curl(&r, &bin, true));
        assert!(ok, "{out}"); assert!(parent.join("0.0.1-bp.1/bin/kicad-cli").is_file());
        let a = std::fs::read_to_string(r.join("wget.args")).unwrap(); assert!(a.lines().count() == 2 && a.lines().all(|l| l.starts_with("-q --https-only -O ")), "{a}");
    }
    #[cfg(unix)] #[test] fn the_real_script_refuses_a_digest_that_is_not_pinned_or_a_wrong_manifest_and_leaves_nothing() {
        let r = tmp("bad"); let (mut p, bin) = fixture(&r, false); let parent = r.join("home/kicad");
        p.sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
        let (ok, out) = script(&bin, &parent, &p); assert!(!ok && out.trim_end().ends_with("KiCad archive checksum mismatch"), "{out}");
        assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0, "the stage and the lock are gone");
        let r = tmp("man"); let (p, bin) = fixture(&r, true); let parent = r.join("home/kicad");
        let (ok, out) = script(&bin, &parent, &p); assert!(!ok && out.contains("KiCad manifest version mismatch"), "{out}");
        assert!(!parent.join("0.0.1-bp.1").exists());
    }

    #[cfg(unix)] #[test] fn a_standin_installer_that_succeeds_is_reported_with_its_steps() {
        let r = tmp("sok"); let s = r.join("standin");
        // the installer goes on once the first step has been heard (a go file the step callback makes), never after a pause: a loaded machine can let it finish between two looks at its log
        let go = r.join("go");
        exe(&s, &format!("#!/bin/sh\necho \"Downloading $5\"\ni=0; while [ ! -e '{}' ] && [ $i -lt 1200 ]; do sleep 0.05; i=$((i+1)); done\necho Unpacking\nmkdir -p \"$1/$3/bin\" && touch \"$1/$3/$8\"\necho \"Installed in $1/$3\"\n", go.display()));
        let parent = r.join("kicad"); let mut steps = vec![];
        let got = rt().block_on(run(Some(&s), &parent, &LINUX, &r.join("log"), |l| { if steps.is_empty() { let _ = std::fs::write(&go, ""); } steps.push(l) }));
        assert_eq!(got, Ok(parent.join("10.99.0-bp.10").to_string_lossy().to_string()));
        assert!(steps.first().unwrap().starts_with("Downloading backplane-kicad-linux") && steps.last().unwrap().starts_with("Installed in"), "{steps:?}");
    }
    #[cfg(unix)] #[test] fn a_standin_installer_that_fails_is_reported_in_its_own_words() {
        let r = tmp("sfail"); let s = r.join("standin");
        exe(&s, "#!/bin/sh\necho Downloading\necho 'Could not download https://x/y' >&2\nexit 1\n");
        let got = rt().block_on(run(Some(&s), &r.join("kicad"), &LINUX, &r.join("log"), |_| {}));
        assert_eq!(got, Err("Could not download https://x/y".into()));
        // ending well without placing KiCad is a failure too
        exe(&s, "#!/bin/sh\necho done\n");
        let got = rt().block_on(run(Some(&s), &r.join("kicad"), &LINUX, &r.join("log"), |_| {}));
        assert!(got.unwrap_err().starts_with("the installer ended without"));
    }

    // ---- the real thing, for CI (windows-latest) and any machine with a network: `cargo test real_ -- --ignored --nocapture --test-threads=1` ----
    /// A hub on a temp home whose "user folder" is a temp folder with a space in its name (so Windows' %LOCALAPPDATA% and the home hold one too).
    async fn real_hub(name: &str) -> (Arc<Hub>, crate::plat::tmp::TempDir) {
        let root = crate::plat::tmp::dir(&format!("bp real {name}"));
        let user = root.join("Some Name"); let local = user.join("AppData").join("Local"); std::fs::create_dir_all(&local).unwrap();
        // SAFETY of the process env: these tests run one at a time (--test-threads=1) and nothing else reads them
        for (k, v) in [("HOME", &user), ("USERPROFILE", &user), ("LOCALAPPDATA", &local), ("XDG_DATA_HOME", &user.join(".local/share"))] { std::env::set_var(k, v); }
        std::env::set_var("BACKPLANE_KICAD_NO_SYSTEM", "1");
        let home = root.join("hub home"); std::fs::create_dir_all(&home).unwrap();
        let os = match std::env::consts::OS { "macos" => "darwin", o => o };
        let arch = match std::env::consts::ARCH { "x86_64" => "x64", "aarch64" => "arm64", a => a };
        let cfg = crate::hub::Cfg { home: home.clone(), version: "0.0.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: os.into(), arch: arch.into() };
        (Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap(), root)
    }
    async fn wait_state(hub: &Arc<Hub>, key: &str, tool: Option<&str>, secs: u64) -> (String, String) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        loop {
            let v: serde_json::Value = serde_json::from_str(hub.info_now()[key].as_str().unwrap_or("")).unwrap_or_default();
            let v = match tool { Some(t) => v[t].clone(), None => v };
            let (s, n) = (v["state"].as_str().unwrap_or("").to_string(), v["note"].as_str().unwrap_or("").to_string());
            if s == "done" || s == "failed" || std::time::Instant::now() > end { return (s, n) }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
    /// kicad.install through the hub's own path with the real download and both checksums, then the hub reports status ok and the version.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)] #[ignore]
    async fn real_kicad_install_ends_with_status_ok() {
        let (hub, root) = real_hub("kicad").await;
        super::super::ipc::publish_cad(&hub).await;
        assert_eq!(hub.info_now()["kicad.status"], "missing", "a clean machine");
        let p = package().expect("a package for this runner");
        let parent = parent(&hub.cfg.home);
        if cfg!(windows) { assert!(parent.to_string_lossy().contains("Some Name") && parent.ends_with("Backplane/kicad"), "{parent:?}"); }
        install(&hub, true).await.expect("started");
        let (state, note) = wait_state(&hub, "kicad.install", None, 25 * 60).await;
        assert_eq!(state, "done", "{note}");
        let info = hub.info_now();
        assert_eq!(info["kicad.status"], "ok", "{info}"); assert_eq!(info["kicad.version"], p.version);
        assert!(parent.join(p.version).join(p.cli).is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Claude Code and Codex through setup's own installers; each then answers --version from where the hub looks for it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)] #[ignore]
    async fn real_agent_installs_answer_version() {
        let (hub, root) = real_hub("agents").await;
        for tool in ["claude", "codex"] {
            crate::settings::tools_install::start(&hub, tool, false, false, |_| Box::pin(async {})).await.expect("started");
            let (state, note) = wait_state(&hub, "onboard.installs", Some(tool), 15 * 60).await;
            assert_eq!(state, "done", "{tool}: {note}");
            let found = crate::agents::proc::Cli::from_env().resolve(tool).unwrap_or_else(|e| panic!("{tool}: {e}"));
            let out = std::process::Command::new(&found).arg("--version").output().unwrap();
            assert!(out.status.success(), "{tool} --version at {found:?}: {}", String::from_utf8_lossy(&out.stderr));
            println!("{tool}: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
