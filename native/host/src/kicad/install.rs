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

// ---- The fetch in Rust (Linux and macOS; Windows next): the same steps as SCRIPT, with no shell. The system's own downloader (curl, else wget;
// %SystemRoot%\System32\curl.exe on Windows) and tar are the only programs started, through proc.rs; the digest, the SHA256SUMS line, the
// archive's member names, links and the runtime's manifest are checked here. A lock folder with the hub's pid; a lock whose holder is gone (a hub
// restarted mid-install) is taken over, its stage removed. Nothing is ever unpacked over an existing runtime; the last step is one rename.

/// The downloader the fetch uses.
#[derive(Debug, Clone, PartialEq)]
pub enum Getter { Curl(PathBuf), Wget(PathBuf) }
/// The programs the fetch starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Tools { pub get: Getter, pub tar: PathBuf }
/// Where the fetch finds its programs: Windows' own System32 curl.exe (Windows 10 1803 and later; its zip is unpacked in-process), elsewhere the
/// first curl (else wget) and tar on `path` (the hub's PATH). Err says what is missing.
pub fn tools_in(windows: bool, system_root: Option<&Path>, path: &[PathBuf]) -> Result<Tools, String> {
    if windows {
        let s = system_root.map(|r| r.join("System32")).ok_or("SystemRoot is not set")?;
        let (curl, tar) = (s.join("curl.exe"), s.join("tar.exe")); // tar: never started for a zip
        if !curl.is_file() { return Err(format!("Windows' own curl.exe was not found in {}", s.display())) }
        return Ok(Tools { get: Getter::Curl(curl), tar })
    }
    let find = |n: &str| path.iter().map(|d| d.join(n)).find(|f| f.is_file());
    let get = find("curl").map(Getter::Curl).or_else(|| find("wget").map(Getter::Wget)).ok_or("curl or wget is needed to download KiCad, and neither was found")?;
    let tar = find("tar").ok_or("tar is needed to unpack KiCad and was not found")?;
    Ok(Tools { get, tar })
}
pub fn tools() -> Result<Tools, String> {
    let path: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).filter(|d| !d.as_os_str().is_empty()).collect();
    tools_in(cfg!(windows), std::env::var_os("SystemRoot").map(PathBuf::from).as_deref(), &path)
}
/// The arguments for one download of `url` into `to`, resuming what a cut earlier try left there and retrying cut connections itself.
pub fn get_args(g: &Getter, url: &str, to: &Path) -> Vec<String> {
    let to = to.to_string_lossy().into_owned();
    match g {
        Getter::Curl(_) => ["--proto", "=https", "--tlsv1.2", "-fsSL", "--retry", "3", "--retry-delay", "2", "--retry-connrefused", "-C", "-", "-o", &to, url].iter().map(|s| s.to_string()).collect(),
        Getter::Wget(_) => ["-q", "--https-only", "-c", "--tries=3", "--waitretry=2", "-O", &to, url].iter().map(|s| s.to_string()).collect(),
    }
}
/// The package's fields as SCRIPT checks them (they come only from the catalog, but a pin is a one-line change and is checked here too).
pub fn check_package(p: &Package) -> Result<(), String> {
    if p.version.is_empty() || !p.version.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') { return Err("Invalid runtime version".into()) }
    if p.release != format!("backplane-kicad-v{}", p.version) { return Err("Invalid runtime release".into()) }
    if p.asset != format!("backplane-kicad-{}-{}.{}", p.platform, p.version, p.format) { return Err("Invalid runtime archive".into()) }
    if p.sha256.len() != 64 || !p.sha256.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) { return Err("Invalid pinned digest".into()) }
    if !matches!((p.platform, p.format), ("linux-x86_64" | "darwin-arm64", "tar.gz") | ("windows-x86_64", "zip")) { return Err("No published Backplane KiCad package for this hub platform".into()) }
    Ok(())
}
/// The digest SHA256SUMS lists for `asset` ("<hex>  <name>", the name maybe after `*` or `./`).
pub fn sum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|l| { let mut it = l.split_whitespace(); let (h, f) = (it.next()?, it.next()?); let f = f.trim_start_matches('*').trim_start_matches("./"); (f == asset).then(|| h.to_ascii_lowercase()) })
}
/// Every member of the archive under `root`, none climbing out (`..`, an absolute path, a drive, a backslash path).
pub fn members_ok(list: &str, root: &str) -> Result<(), String> {
    let mut n = 0;
    for m in list.lines().map(str::trim_end).filter(|m| !m.is_empty()) {
        let m = m.strip_prefix("./").unwrap_or(m);
        if m.starts_with('/') || m.contains('\\') || m.contains(':') || m.split('/').any(|c| c == ".." || c == ".") { return Err("Unsafe runtime archive path".into()) }
        if m.trim_end_matches('/') != root && !m.starts_with(&format!("{root}/")) { return Err("Unexpected runtime archive root".into()) }
        n += 1;
    }
    if n == 0 { Err("The KiCad archive is empty".into()) } else { Ok(()) }
}
/// Every link under `dir` points inside `inside` (a link out of the runtime would let a later write land elsewhere).
pub fn links_inside(dir: &Path, inside: &Path) -> Result<(), String> {
    let inside = inside.canonicalize().map_err(|e| e.to_string())?;
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.flatten() {
            let (p, t) = (e.path(), e.file_type().map_err(|e| e.to_string())?);
            if t.is_symlink() {
                let target = std::fs::read_link(&p).map_err(|e| e.to_string())?;
                // the link's target as a path, resolved lexically from the link's folder (it need not exist)
                let mut at = if target.is_absolute() { PathBuf::new() } else { d.canonicalize().map_err(|e| e.to_string())? };
                for c in target.components() { match c { std::path::Component::ParentDir => { at.pop(); } std::path::Component::CurDir => {} c => at.push(c) } }
                if !at.starts_with(&inside) { return Err(format!("Unsafe link in the KiCad archive: {}", p.strip_prefix(dir).unwrap_or(&p).display())) }
            } else if t.is_dir() { todo.push(p) }
        }
    }
    Ok(())
}
/// The most a KiCad zip may unpack to (the Windows runtime is about 3 GB unpacked): a bound on a zip bomb, checked before anything is written.
pub const UNZIP_CAP: u64 = 12 << 30;
/// A name Windows will not make as asked: a device (CON, PRN, AUX, NUL, COM1-9, LPT1-9, with any extension: "nul.txt" is NUL too), or one
/// ending in a dot or a space (Windows drops them, so it would land on another name).
pub fn windows_reserved(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let dev = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && ["\u{b9}", "\u{b2}", "\u{b3}"].iter().any(|d| stem[3..] == **d));
    dev || component.ends_with('.') || component.ends_with(' ')
}
/// A zip's member names (a backslash read as `/`, any trailing `/` dropped) checked for Windows: no reserved component, and no two paths
/// (a member or a folder one implies) that differ only in case (on Windows one would land on the other).
pub fn names_ok_for_windows(names: &[String]) -> Result<(), String> {
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for n in names {
        let n = n.strip_prefix("./").unwrap_or(n).trim_end_matches('/');
        let parts: Vec<&str> = n.split('/').filter(|c| !c.is_empty()).collect();
        if let Some(c) = parts.iter().find(|c| windows_reserved(c)) { return Err(format!("A name Windows reserves in the KiCad archive: {c} (in {n})")) }
        for k in 1..=parts.len() {
            let path = parts[..k].join("/");
            match seen.insert(path.to_lowercase(), path.clone()) { Some(was) if was != path => return Err(format!("Two names in the KiCad archive differ only in case: {was} and {path}")), _ => {} }
        }
    }
    Ok(())
}
/// A zip archive unpacked in-process into `into` (Windows' package; no tar or PowerShell): every member under `root` and none climbing out
/// (`members_ok`, with a Windows backslash read as a separator), no name Windows reserves or two differing only in case, no links, at most
/// `UNZIP_CAP` bytes in all (declared sizes summed first; each file then read no further than its declared size), each file written new
/// and its CRC checked as it is read.
pub fn unzip(zip: &Path, root: &str, into: &Path) -> Result<(), String> { unzip_capped(zip, root, into, UNZIP_CAP) }
pub fn unzip_capped(zip: &Path, root: &str, into: &Path, cap: u64) -> Result<(), String> {
    let f = std::fs::File::open(zip).map_err(|e| format!("Could not read the KiCad archive: {e}"))?;
    let mut a = zip::ZipArchive::new(std::io::BufReader::new(f)).map_err(|e| format!("Could not read the KiCad archive: {e}"))?;
    let names: Vec<String> = (0..a.len()).map(|i| a.name_for_index(i).unwrap_or("").replace('\\', "/")).collect();
    members_ok(&names.join("\n"), root)?;
    names_ok_for_windows(&names)?;
    let mut total = 0u64;
    for i in 0..a.len() { total = total.saturating_add(a.by_index_raw(i).map_err(|e| format!("Could not read the KiCad archive: {e}"))?.size()); }
    if total > cap { return Err(format!("The KiCad archive would unpack to {} MB, more than the {} MB allowed", total >> 20, cap >> 20)) }
    for (i, n) in names.iter().enumerate() {
        let mut e = a.by_index(i).map_err(|e| format!("Could not read the KiCad archive: {e}"))?;
        if e.is_symlink() { return Err(format!("Unsafe link in the KiCad archive: {n}")) }
        let to = n.strip_prefix("./").unwrap_or(n).split('/').filter(|c| !c.is_empty()).fold(into.to_path_buf(), |p, c| p.join(c));
        if e.is_dir() || n.ends_with('/') { std::fs::create_dir_all(&to).map_err(|e| format!("Could not unpack KiCad: {e}"))?; continue }
        if let Some(d) = to.parent() { std::fs::create_dir_all(d).map_err(|e| format!("Could not unpack KiCad: {e}"))?; }
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&to).map_err(|e| format!("Could not unpack KiCad ({n}): {e}"))?;
        let size = e.size();
        let wrote = std::io::copy(&mut std::io::Read::take(&mut e, size + 1), &mut out).map_err(|e| format!("Could not unpack KiCad ({n}): {e}"))?;
        if wrote != size { return Err(format!("Could not unpack KiCad ({n}): {wrote} bytes where the archive says {size}")) }
        #[cfg(unix)] if let Some(m) = e.unix_mode() { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&to, std::fs::Permissions::from_mode(m & 0o755)); }
    }
    Ok(())
}
/// The runtime's manifest says this version and platform.
pub fn manifest_ok(text: &str, p: &Package) -> Result<(), String> {
    let m: serde_json::Value = serde_json::from_str(text).map_err(|_| "KiCad manifest is not JSON".to_string())?;
    if m["version"] != p.version { return Err("KiCad manifest version mismatch".into()) }
    if m["platform"] != p.platform { return Err("KiCad manifest platform mismatch".into()) }
    Ok(())
}
fn sha256_file(f: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new(); let mut r = std::fs::File::open(f).map_err(|e| e.to_string())?; let mut b = vec![0u8; 1 << 20];
    loop { let n = std::io::Read::read(&mut r, &mut b).map_err(|e| e.to_string())?; if n == 0 { break } h.update(&b[..n]) }
    Ok(hex::encode(h.finalize()))
}
/// One program to its end (through proc.rs, in its own group, its output into the log); Err with its last line when it fails.
async fn tool(prog: &Path, args: &[String], log: &Path, mut tick: impl FnMut()) -> Result<String, String> {
    let out = std::fs::OpenOptions::new().create(true).append(true).open(log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, prog);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::from(err));
    crate::plat::proc::own_group_tokio(&mut c);
    let from = std::fs::metadata(log).map_or(0, |m| m.len());
    let mut child = c.spawn().map_err(|e| format!("could not start {}: {e}", prog.display()))?;
    let mut so = child.stdout.take();
    let read = tokio::spawn(async move { let mut s = String::new(); if let Some(o) = so.as_mut() { let _ = tokio::io::AsyncReadExt::read_to_string(o, &mut s).await; } s });
    let status = loop { match tokio::time::timeout(std::time::Duration::from_secs(1), child.wait()).await { Ok(r) => break r.map_err(|e| e.to_string())?, Err(_) => tick() } };
    let text = read.await.unwrap_or_default(); drop(out);
    if status.success() { return Ok(text) }
    // what it said itself (not an earlier step's line), else what its exit code means
    let said = std::fs::read(log).ok().map(|b| String::from_utf8_lossy(b.get(from as usize..).unwrap_or(&[])).into_owned()).unwrap_or_default();
    let said = last_line(&said);
    let name = prog.file_stem().map_or(String::new(), |n| n.to_string_lossy().to_lowercase());
    Err(if !said.is_empty() { said } else { exit_words(&name, status.code()) })
}
/// A downloader's exit code in the person's words (curl's and wget's documented codes), for one that stopped without a word.
pub fn exit_words(prog: &str, code: Option<i32>) -> String {
    let w = match (prog, code) {
        ("curl", Some(6)) => "no internet connection: the address did not resolve",
        ("curl", Some(7)) => "could not connect to the server",
        ("curl", Some(18)) => "the connection ended before the file was whole",
        ("curl", Some(22)) => "the server refused the file",
        ("curl", Some(28)) => "the connection timed out",
        ("curl", Some(35 | 60)) => "the secure connection could not be made (check the computer's clock and any proxy)",
        ("curl", Some(23)) => "the disk refused the download (is it full?)",
        ("curl", Some(56)) => "the connection was cut",
        ("wget", Some(3)) => "the disk refused the download (is it full?)",
        ("wget", Some(4)) => "the network failed",
        ("wget", Some(5)) => "the secure connection could not be made (check the computer's clock and any proxy)",
        ("wget", Some(8)) => "the server refused the file",
        _ => "",
    };
    if w.is_empty() { format!("{prog} stopped ({})", code.map_or("killed".into(), |c| format!("exit {c}"))) } else { w.to_string() }
}
/// A download's progress: bytes so far, the whole (0: not known yet), bytes a second and seconds left (0: not known).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bytes { pub done: u64, pub total: u64, pub rate: u64, pub eta: u64 }
impl Bytes {
    /// What setup says: "312 of 971 MB, about 40 s left"; "312 MB so far" while the whole is not known.
    pub fn words(&self) -> String {
        let mb = |b: u64| (b + (1 << 19)) >> 20;
        if self.total == 0 { return format!("{} MB so far", mb(self.done)) }
        let left = match self.eta { 0 => String::new(), s if s < 60 => format!(", about {} s left", ((s + 4) / 5 * 5).max(5)), s => format!(", about {} min left", (s + 30) / 60) };
        format!("{} of {} MB{left}", mb(self.done).min(mb(self.total)), mb(self.total))
    }
    /// The share done, 0 to 1 (0 while the whole is not known).
    pub fn share(&self) -> f64 { if self.total == 0 { 0. } else { (self.done as f64 / self.total as f64).min(1.) } }
}
/// Rate and time left from the last few seconds' samples (a moving window, so a stall shows quickly and a burst does not promise too much).
#[derive(Default)]
struct Meter { samples: std::collections::VecDeque<(std::time::Instant, u64)> }
impl Meter {
    fn at(&mut self, now: std::time::Instant, done: u64, total: u64) -> Bytes {
        self.samples.push_back((now, done));
        while self.samples.len() > 2 && now.duration_since(self.samples[0].0).as_secs_f64() > 6. { self.samples.pop_front(); }
        let (t0, d0) = self.samples[0]; let dt = now.duration_since(t0).as_secs_f64();
        let rate = if dt >= 0.9 { (done.saturating_sub(d0) as f64 / dt) as u64 } else { 0 };
        let eta = if rate > 0 && total > done { (total - done).div_ceil(rate) } else { 0 };
        Bytes { done, total, rate, eta }
    }
}
/// Where a download is kept until it is installed: it survives a failure and a hub restart, and the next try continues it (`curl -C -`,
/// `wget -c`). A wrong digest removes it.
pub fn partial(parent: &Path, p: &Package) -> PathBuf { parent.join(".partial").join(p.asset) }
/// The size of `url` from its headers (a HEAD through the same downloader, redirects followed: the last Content-Length); 0 when not said.
async fn length(g: &Getter, url: &str) -> u64 {
    let (prog, args): (&Path, Vec<&str>) = match g { Getter::Curl(x) => (x, vec!["--proto", "=https", "--tlsv1.2", "-fsSIL", "--max-time", "20", url]), Getter::Wget(x) => (x, vec!["--spider", "-S", "--https-only", "--timeout=20", "--tries=1", url]) };
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, prog); c.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let Ok(Ok(o)) = tokio::time::timeout(std::time::Duration::from_secs(25), c.output()).await else { return 0 };
    content_length(&format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}
/// The last Content-Length in a run of response headers (after redirects, the file's own).
pub fn content_length(headers: &str) -> u64 {
    headers.lines().filter_map(|l| { let l = l.trim(); let (k, v) = l.split_once(':')?; k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse::<u64>().ok()).flatten() }).last().unwrap_or(0)
}
/// The lock folder: `<parent>/.install-<version>.lock` holding the hub's pid. Dropping it removes the stage and the lock.
struct Held { lock: PathBuf, stage: Option<PathBuf> }
impl Drop for Held { fn drop(&mut self) { if let Some(s) = &self.stage { let _ = std::fs::remove_dir_all(s); } let _ = std::fs::remove_dir_all(&self.lock); } }
fn take_lock(parent: &Path, version: &str) -> Result<Held, String> {
    let lock = parent.join(format!(".install-{version}.lock"));
    for _ in 0..2 {
        match std::fs::create_dir(&lock) {
            Ok(()) => { let _ = std::fs::write(lock.join("pid"), std::process::id().to_string()); return Ok(Held { lock, stage: None }) }
            Err(_) => {
                // a holder that is gone (a hub restarted mid-install, or 0.14's script) leaves its lock: take it over with its stage
                let pid: u32 = std::fs::read_to_string(lock.join("pid")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                if pid != 0 && crate::plat::proc::alive(pid) { return Err("Another KiCad installation is active; retry when it finishes".into()) }
                if pid == 0 && std::fs::metadata(&lock).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(false, |a| a.as_secs() < 3600) { return Err("Another KiCad installation is active; retry when it finishes".into()) }
                for e in std::fs::read_dir(parent).into_iter().flatten().flatten() { let n = e.file_name().to_string_lossy().into_owned(); if n.starts_with(".install-") && !n.ends_with(".lock") && e.path().is_dir() { let _ = std::fs::remove_dir_all(e.path()); } }
                let _ = std::fs::remove_dir_all(&lock);
            }
        }
    }
    Err("Another KiCad installation is active; retry when it finishes".into())
}
/// The fetch, step by step; each step's words go to `step` and the log, as SCRIPT's lines did. Ok(the installed folder).
pub async fn fetch(t: &Tools, base: &str, parent: &Path, p: &Package, log: &Path, step: &mut impl FnMut(String), bytes: &mut impl FnMut(Bytes)) -> Result<PathBuf, String> {
    check_package(p)?;
    let say = |step: &mut dyn FnMut(String), s: String| { if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) { let _ = std::io::Write::write_all(&mut f, format!("{s}\n").as_bytes()); } step(s) };
    let dir = parent.join(p.version);
    if dir.exists() || dir.symlink_metadata().is_ok() { return Err(format!("{} already exists; the installation there is kept. Set its path in Settings, CAD, or remove it and try again", dir.display())) }
    std::fs::create_dir_all(parent).map_err(|e| format!("Could not make {}: {e}", parent.display()))?;
    let mut held = take_lock(parent, p.version)?;
    let stage = parent.join(format!(".install-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
    std::fs::create_dir(&stage).map_err(|e| e.to_string())?; held.stage = Some(stage.clone());
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o700)); }
    let (pkg, sums) = (partial(parent, p), stage.join("SUMS"));
    std::fs::create_dir_all(pkg.parent().unwrap_or(parent)).map_err(|e| e.to_string())?;
    let gp = match &t.get { Getter::Curl(x) | Getter::Wget(x) => x.clone() };
    let url = format!("{base}{}", p.asset);
    let have = |f: &Path| std::fs::metadata(f).map_or(0, |m| m.len());
    let total = length(&t.get, &url).await;
    let before = have(&pkg);
    say(step, if before > 0 { format!("Downloading {} (continuing from {} MB)", p.asset, before >> 20) } else { format!("Downloading {}", p.asset) });
    let mut meter = Meter::default();
    bytes(meter.at(std::time::Instant::now(), before, total));
    // a download already whole (an earlier try ended after it) is not asked for again: a server answers a range past the end with an error
    if !(total > 0 && before == total) {
        let mut ok = Err(String::new());
        for _ in 0..3 { ok = tool(&gp, &get_args(&t.get, &url, &pkg), log, || bytes(meter.at(std::time::Instant::now(), have(&pkg), total))).await; if ok.is_ok() { break } tokio::time::sleep(std::time::Duration::from_secs(2)).await }
        if let Err(why) = ok {
            let got = have(&pkg);
            let at = if got == 0 { String::new() } else { format!(" at {}", Bytes { done: got, total, rate: 0, eta: 0 }.words().replace(" so far", "")) };
            return Err(format!("The download stopped{at} ({})", if why.is_empty() { "no answer".to_string() } else { why }))
        }
    }
    let got = have(&pkg); bytes(Bytes { done: got, total: total.max(got), rate: 0, eta: 0 });
    tool(&gp, &get_args(&t.get, &format!("{base}SHA256SUMS"), &sums), log, || {}).await.map_err(|_| format!("Could not download {base}SHA256SUMS"))?;
    say(step, "Checking the checksum".into());
    let want = sum_for(&std::fs::read_to_string(&sums).unwrap_or_default(), p.asset).unwrap_or_default();
    let pk = pkg.clone(); let got = tokio::task::spawn_blocking(move || sha256_file(&pk)).await.map_err(|e| e.to_string())??; // the digest
    if want != got || got != p.sha256 { let _ = std::fs::remove_file(&pkg); if let Some(d) = pkg.parent() { let _ = std::fs::remove_dir(d); } return Err("KiCad archive checksum mismatch (the download was removed; Try again downloads it anew)".into()) }
    let mut magic = [0u8; 4]; let _ = std::io::Read::read(&mut std::fs::File::open(&pkg).map_err(|e| e.to_string())?, &mut magic);
    let fmt_ok = match p.format { "tar.gz" => magic[..3] == [0x1f, 0x8b, 0x08], "zip" => magic == [0x50, 0x4b, 0x03, 0x04], _ => false };
    if !fmt_ok { return Err("KiCad archive header does not match its platform format".into()) }
    say(step, "Unpacking".into());
    let root = format!("backplane-kicad-{}-{}", p.platform, p.version);
    let unpack = stage.join("unpack"); std::fs::create_dir(&unpack).map_err(|e| e.to_string())?;
    if p.format == "zip" {
        let (z, r, u) = (pkg.clone(), root.clone(), unpack.clone());
        tokio::task::spawn_blocking(move || unzip(&z, &r, &u)).await.map_err(|e| e.to_string())??;
    } else {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let members = tool(&t.tar, &s(&["-tzf", &pkg.to_string_lossy()]), log, || {}).await.map_err(|e| format!("Could not read the KiCad archive: {e}"))?;
        members_ok(&members, &root)?;
        tool(&t.tar, &s(&["-xzf", &pkg.to_string_lossy(), "-C", &unpack.to_string_lossy()]), log, || {}).await.map_err(|e| format!("Could not unpack KiCad: {e}"))?;
    }
    let runtime = unpack.join(&root);
    links_inside(&unpack, &runtime)?;
    let cli = runtime.join(p.cli);
    let has = |f: &Path| std::fs::metadata(f).map_or(false, |m| m.is_file() && m.len() > 0);
    if !cli.is_file() || !has(&runtime.join("manifest.json")) || !has(&runtime.join("share/backplane/api.desc")) { return Err("KiCad archive runtime layout is incomplete".into()) }
    if p.platform == "windows-x86_64" {
        let mut mz = [0u8; 2]; let _ = std::io::Read::read(&mut std::fs::File::open(&cli).map_err(|e| e.to_string())?, &mut mz);
        if mz != *b"MZ" { return Err("Windows KiCad CLI is not a PE executable".into()) }
    } else {
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; if std::fs::metadata(&cli).map_or(0, |m| m.permissions().mode()) & 0o111 == 0 { return Err("KiCad launcher is not executable".into()) } }
    }
    manifest_ok(&std::fs::read_to_string(runtime.join("manifest.json")).unwrap_or_default(), p)?;
    if dir.exists() || dir.symlink_metadata().is_ok() { return Err("Runtime destination appeared; existing installation preserved".into()) }
    std::fs::rename(&runtime, &dir).map_err(|e| format!("Could not move KiCad into {}: {e}", dir.display()))?;
    let _ = std::fs::remove_file(&pkg); if let Some(d) = pkg.parent() { let _ = std::fs::remove_dir(d); }
    say(step, format!("Installed in {}", dir.display()));
    drop(held);
    Ok(dir)
}
/// Where the fetch downloads from (the script's base).
pub fn base_url(p: &Package) -> String { format!("https://{FROM}/releases/download/{}/", p.release) }
/// Every system uses the fetch in Rust (Windows too: System32's curl.exe and the zip unpacked here, so no Git for Windows is needed); SCRIPT
/// runs only as the stand-in path's reference and in its own tests.
pub fn rust_fetch() -> bool { true }

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
pub async fn run(standin: Option<&Path>, parent: &Path, p: &Package, log: &Path, mut step: impl FnMut(String), mut bytes: impl FnMut(Bytes)) -> Result<String, String> {
    std::fs::create_dir_all(parent).map_err(|e| format!("could not make {}: {e}", parent.display()))?;
    if standin.is_none() && rust_fetch() {
        let _ = std::fs::write(log, "");
        let t = tools()?;
        return fetch(&t, &base_url(p), parent, p, log, &mut step, &mut bytes).await.map(|d| d.to_string_lossy().into_owned());
    }
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
async fn say(hub: &Arc<Hub>, state: &str, version: &str, note: &str) { say_bytes(hub, state, version, note, None).await }
/// The same with the download's progress: `done`, `total` (bytes; 0 not known), `rate` (bytes a second), `eta` (seconds; 0 not known).
async fn say_bytes(hub: &Arc<Hub>, state: &str, version: &str, note: &str, b: Option<Bytes>) {
    let mut v = json!({"state": state, "version": version, "note": note});
    if let Some(b) = b { v["phase"] = json!("download"); v["done"] = json!(b.done); v["total"] = json!(b.total); v["rate"] = json!(b.rate); v["eta"] = json!(b.eta); }
    hub.set_info("kicad.install", &v.to_string()).await; hub.bump_views();
}
/// Whether an install runs in this hub now (a "running" left in `kicad.install` by a hub that stopped is not).
pub fn running() -> bool { RUNNING.load(Ordering::SeqCst) }
/// A download an earlier try left (`partial`), with its size: the next install continues it.
pub fn partial_bytes(home: &Path, p: &Package) -> u64 { std::fs::metadata(partial(&parent(home), p)).map_or(0, |m| m.len()) }

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
        // the steps' words and, while it downloads, its bytes (at most twice a second to the views)
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<String, Bytes>>();
        let h2 = h.clone();
        let relay = tokio::spawn(async move {
            let (mut note, mut last, mut seen) = (String::new(), std::time::Instant::now() - std::time::Duration::from_secs(1), Bytes::default());
            while let Some(m) = rx.recv().await { match m {
                Ok(l) => { let dl = l.starts_with("Downloading"); note = l.clone(); if !dl { say(&h2, "running", p.version, &l).await } else { say_bytes(&h2, "running", p.version, &l, Some(Bytes::default())).await } }
                Err(b) => { seen = b; if last.elapsed().as_millis() >= 500 || (b.total > 0 && b.done >= b.total) { last = std::time::Instant::now(); say_bytes(&h2, "running", p.version, &note, Some(b)).await } }
            } }
            seen
        });
        let tx2 = tx.clone();
        let r = run(standin.as_deref(), &parent, p, &parent.join(".install.log"), |l| { let _ = tx.send(Ok(l)); }, |b| { let _ = tx2.send(Err(b)); }).await;
        drop(tx2);
        drop(tx); let seen = relay.await.unwrap_or_default();
        RUNNING.store(false, Ordering::SeqCst);
        // Settings > CAD names the new KiCad (and its Install row goes) before the install says it is done: whoever reads "done" reads the new status with it
        super::ipc::publish_cad(&h).await;
        match &r {
            Ok(dir) => { eprintln!("kicad: installed Backplane KiCad {} in {dir}", p.version); say(&h, "done", p.version, &format!("Installed Backplane KiCad {} in {dir}", p.version)).await }
            // a stopped download is not a failure for good: its own words, and its bytes (the bar stays where it got to)
            Err(why) if why.starts_with("The download stopped") => { eprintln!("kicad: download stopped: {why}"); say_bytes(&h, "failed", p.version, why, Some(Bytes { rate: 0, eta: 0, ..seen })).await }
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
        // Linux to the hotfix: an installed bp.10, bp.11 or bp.12 is offered the update; bp.12.1 itself and anything newer are left alone
        let hot = Package { version: "10.99.0-bp.12.1", release: "backplane-kicad-v10.99.0-bp.12.1", ..LINUX };
        for v in ["10.99.0-bp.10", "10.99.0-bp.11", "10.99.0-bp.12"] { assert_eq!(decide(Some(&hot), "ok", v, ""), Action::Offer, "{v}"); assert_eq!(refusal("/k/bin/kicad-cli", "ok", v, true, false, Some(&hot)), None, "{v}"); }
        for v in ["10.99.0-bp.12.1", "10.99.0-bp.13"] { assert_eq!(decide(Some(&hot), "ok", v, ""), Action::Nothing, "{v}"); }
        // Windows pinned behind Linux: its bp.10 is current for it while Linux's would be offered the hotfix
        assert_eq!(decide(Some(&WINDOWS), "ok", "10.99.0-bp.10", ""), Action::Nothing);
        assert_eq!(refusal("/k/bin/kicad-cli", "ok", "10.99.0-bp.4", true, false, Some(&LINUX)), None);
    }

    /// A disposable package for the real script: its archive and SHA256SUMS, served by a stand-in `curl` on PATH.
    #[cfg(unix)] fn fixture(r: &Path, bad_manifest: bool) -> (Package, PathBuf) { fixture_v(r, bad_manifest, "0.0.1-bp.1") }
    #[cfg(unix)] fn fixture_v(r: &Path, bad_manifest: bool, v: &'static str) -> (Package, PathBuf) {
        let plat = "linux-x86_64"; let root = format!("backplane-kicad-{plat}-{v}");
        let src = r.join("src").join(&root); std::fs::create_dir_all(src.join("bin")).unwrap(); std::fs::create_dir_all(src.join("share/backplane")).unwrap();
        exe(&src.join("bin/kicad-cli"), &format!("#!/bin/sh\necho {v}\n"));
        std::fs::write(src.join("manifest.json"), format!(r#"{{"version": "{}", "platform": "{plat}"}}"#, if bad_manifest { "9.9" } else { v })).unwrap();
        std::fs::write(src.join("share/backplane/api.desc"), "x").unwrap();
        let serve = r.join("serve"); std::fs::create_dir_all(&serve).unwrap(); let asset = format!("{root}.tar.gz");
        assert!(std::process::Command::new("tar").arg("-czf").arg(serve.join(&asset)).arg("-C").arg(r.join("src")).arg(&root).status().unwrap().success());
        let sum = String::from_utf8(std::process::Command::new("sha256sum").arg(serve.join(&asset)).output().unwrap().stdout).unwrap().split(' ').next().unwrap().to_string();
        std::fs::write(serve.join("SHA256SUMS"), format!("{sum}  {asset}\n")).unwrap();
        let bin = r.join("bin"); std::fs::create_dir_all(&bin).unwrap();
        // curl -fsSL -o OUT URL: the file of that name from serve/ (fails like curl -f for anything else)
        // a HEAD (-fsSIL) answers with the file's size; a serve/FAILAT file (<bytes>) stops a download there (a partial file and exit 18, as curl does)
        exe(&bin.join("curl"), &format!("#!/bin/sh\nhead=; case \"$*\" in *-fsSIL*) head=1;; esac\nwhile [ $# -gt 1 ]; do [ \"$1\" = -o ] && out=$2; shift; done\nf={}/$(basename \"$1\")\n[ -f \"$f\" ] || exit 22\nif [ -n \"$head\" ]; then echo 'HTTP/2 302'; echo 'content-length: 0'; echo; echo 'HTTP/2 200'; echo \"Content-Length: $(wc -c <\"$f\")\"; exit 0; fi\necho x >>\"$f.calls\"\nif [ -f {0}/FAILAT ] && [ \"$(basename \"$f\")\" != SHA256SUMS ]; then head -c \"$(cat {0}/FAILAT)\" \"$f\" >\"$out\"; exit 18; fi\ncp \"$f\" \"$out\"\n", serve.display()).replace("{0}", &serve.display().to_string()));
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
    // ---- the fetch in Rust ----
    #[cfg(unix)] fn sysdirs(bin: &Path) -> Vec<PathBuf> { vec![bin.to_path_buf(), "/usr/bin".into(), "/bin".into()] }
    #[cfg(unix)] fn fetch_run(r: &Path, p: &Package, dirs: &[PathBuf]) -> (Result<PathBuf, String>, Vec<String>) {
        let t = match tools_in(false, None, dirs) { Ok(t) => t, Err(e) => return (Err(e), vec![]) };
        let mut steps = vec![];
        let got = rt().block_on(fetch(&t, "https://x/r/", &r.join("home/kicad"), p, &r.join("log"), &mut |l| steps.push(l), &mut |_| {}));
        (got, steps)
    }
    #[cfg(unix)] #[test] fn the_rust_fetch_installs_a_checked_package_and_says_each_step() {
        let r = tmp("rf-ok"); let (p, bin) = fixture(&r, false);
        let (got, steps) = fetch_run(&r, &p, &sysdirs(&bin));
        let parent = r.join("home/kicad");
        assert_eq!(got, Ok(parent.join("0.0.1-bp.1")));
        assert_eq!(steps.iter().filter(|s| !s.contains(": ")).cloned().collect::<Vec<_>>(), [format!("Downloading {}", p.asset), "Checking the checksum".into(), "Unpacking".into(), format!("Installed in {}/0.0.1-bp.1", parent.display())]);
        assert!(parent.join("0.0.1-bp.1/bin/kicad-cli").is_file());
        let left: Vec<_> = std::fs::read_dir(&parent).unwrap().flatten().map(|e| e.file_name()).collect(); assert_eq!(left.len(), 1, "no stage or lock is left: {left:?}");
        assert_eq!(crate::files::kicad::ipc::read_runtime(&parent.join("0.0.1-bp.1/bin/kicad-cli")).version, "0.0.1-bp.1");
        let (again, _) = fetch_run(&r, &p, &sysdirs(&bin)); assert!(again.unwrap_err().contains("already exists; the installation there is kept"));
        assert!(std::fs::read_to_string(r.join("log")).unwrap().contains("Checking the checksum"), "the steps are in the log as before");
    }
    // ---- Windows' zip, unpacked in-process ----
    /// A zip of `entries` (name, bytes, a link when Some(target)); stored (not deflated) when `stored`, so a test can corrupt a file's bytes.
    fn zip_of(to: &Path, entries: &[(&str, &[u8])], links: &[(&str, &str)], stored: bool) {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::fs::File::create(to).unwrap());
        let o = zip::write::SimpleFileOptions::default().compression_method(if stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated });
        for (n, b) in entries { if n.ends_with('/') { w.add_directory(*n, o).unwrap() } else { w.start_file(*n, o).unwrap(); w.write_all(b).unwrap() } }
        for (n, t) in links { w.add_symlink(*n, *t, o).unwrap() }
        w.finish().unwrap();
    }
    fn win_runtime(root: &str, v: &str) -> Vec<(String, Vec<u8>)> {
        vec![(format!("{root}/"), vec![]), (format!("{root}/bin/kicad-cli.exe"), b"MZ\x90\0 a PE stand-in".to_vec()),
             (format!("{root}/manifest.json"), format!(r#"{{"version": "{v}", "platform": "windows-x86_64"}}"#).into_bytes()),
             (format!("{root}/share/backplane/api.desc"), b"x".to_vec()), (format!("{root}/share/kicad/big.bin"), vec![7u8; 3 << 20])]
    }
    #[test] fn a_zip_unpacks_here_and_anything_climbing_out_or_linking_is_refused() {
        let r = tmp("rf-zip"); let root = "backplane-kicad-windows-x86_64-0.0.1-bp.1";
        let ents = win_runtime(root, "0.0.1-bp.1"); let e: Vec<(&str, &[u8])> = ents.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        zip_of(&r.join("ok.zip"), &e, &[], false);
        let out = r.join("out"); std::fs::create_dir(&out).unwrap();
        assert_eq!(unzip(&r.join("ok.zip"), root, &out), Ok(()));
        assert_eq!(std::fs::read(out.join(root).join("share/kicad/big.bin")).unwrap(), vec![7u8; 3 << 20]);
        assert!(std::fs::read(out.join(root).join("bin/kicad-cli.exe")).unwrap().starts_with(b"MZ"));
        // each refused before anything is written
        let bad = |name: &str, extra: &[(&str, &[u8])], links: &[(&str, &str)], why: &str| {
            let z = r.join(format!("{name}.zip")); let mut e2 = e.clone(); e2.extend_from_slice(extra); zip_of(&z, &e2, links, false);
            let o = r.join(name); std::fs::create_dir(&o).unwrap();
            let got = unzip(&z, root, &o).unwrap_err(); assert!(got.contains(why), "{name}: {got}");
            assert!(!r.join("evil").exists(), "{name}: nothing outside");
        };
        bad("dotdot", &[(&format!("{root}/../../evil"), b"x")], &[], "Unsafe runtime archive path");
        bad("backslash", &[(&format!("{root}\\..\\..\\evil"), b"x")], &[], "Unsafe runtime archive path");
        bad("drive", &[("C:/evil", b"x")], &[], "Unsafe runtime archive path");
        bad("absolute", &[("/evil", b"x")], &[], "Unsafe runtime archive path");
        bad("root", &[("another-root/evil", b"x")], &[], "Unexpected runtime archive root");
        bad("link", &[], &[(&format!("{root}/bin/out"), "../../../evil")], "Unsafe link in the KiCad archive");
        // Windows: two names one file there, a device name, a name Windows trims
        bad("case", &[(&format!("{root}/BIN/kicad-cli.exe"), b"x")], &[], "differ only in case");
        bad("case2", &[(&format!("{root}/Manifest.json"), b"x")], &[], "differ only in case");
        for (k, dev) in ["CON", "nul.txt", "com1", "LPT9.dll", "aux.", "x "].iter().enumerate() {
            bad(&format!("dev{k}"), &[(&format!("{root}/share/{dev}"), b"x")], &[], "A name Windows reserves");
        }
        bad("devdir", &[(&format!("{root}/Prn/a"), b"x")], &[], "A name Windows reserves");
        assert!(!windows_reserved("console.txt") && !windows_reserved("com10") && !windows_reserved("nullable.h") && !windows_reserved("lpt"));
        // more than the cap, refused before anything is written
        let o = r.join("cap"); std::fs::create_dir(&o).unwrap();
        assert!(unzip_capped(&r.join("ok.zip"), root, &o, 1 << 20).unwrap_err().contains("more than the 1 MB allowed"));
        assert_eq!(std::fs::read_dir(&o).unwrap().count(), 0, "nothing written");
        // a file whose bytes changed after it was written (its CRC no longer matches) is not taken
        zip_of(&r.join("crc.zip"), &[(&format!("{root}/manifest.json"), b"0123456789abcdef")], &[], true);
        let mut b = std::fs::read(r.join("crc.zip")).unwrap(); let at = b.windows(16).position(|w| w == b"0123456789abcdef").unwrap(); b[at] = b'X'; std::fs::write(r.join("crc.zip"), b).unwrap();
        let o = r.join("crc"); std::fs::create_dir(&o).unwrap();
        assert!(unzip(&r.join("crc.zip"), root, &o).unwrap_err().contains("Could not unpack KiCad"));
    }
    /// The Windows package through the whole Rust fetch (the stand-in curl serves it): checked, unzipped here, its .exe a PE, its manifest read.
    #[cfg(unix)] #[test] fn the_rust_fetch_installs_a_windows_zip_without_tar() {
        let r = tmp("rf-winzip"); let (_, bin) = fixture(&r, false); let v = "0.0.1-bp.1"; let root = format!("backplane-kicad-windows-x86_64-{v}");
        let asset = format!("{root}.zip"); let ents = win_runtime(&root, v);
        zip_of(&r.join("serve").join(&asset), &ents.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect::<Vec<_>>(), &[], false);
        let sum = sha256_file(&r.join("serve").join(&asset)).unwrap(); std::fs::write(r.join("serve/SHA256SUMS"), format!("{sum}  {asset}\n")).unwrap();
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let p = Package { platform: "windows-x86_64", version: v, release: "backplane-kicad-v0.0.1-bp.1", asset: leak(asset), sha256: leak(sum), format: "zip", cli: "bin/kicad-cli.exe" };
        // no tar anywhere: the fetch is given only the stand-in curl's folder and a tar that fails if it is ever started
        let nt = r.join("notar"); std::fs::create_dir_all(&nt).unwrap(); exe(&nt.join("tar"), "#!/bin/sh\necho tar was started >&2; exit 9\n");
        let (got, steps) = fetch_run(&r, &p, &[bin.clone(), nt]);
        let parent = r.join("home/kicad");
        assert_eq!(got, Ok(parent.join(v)), "{steps:?}");
        assert_eq!(std::fs::read(parent.join(v).join("share/kicad/big.bin")).unwrap().len(), 3 << 20);
        let left: Vec<_> = std::fs::read_dir(&parent).unwrap().flatten().map(|e| e.file_name()).collect(); assert_eq!(left.len(), 1, "no stage, partial or lock is left: {left:?}");
    }
    /// The Linux bump to a hotfix: a four-part version installs beside the release it fixes, checked the same, and is the runtime taken.
    #[cfg(unix)] #[test] fn the_rust_fetch_installs_a_hotfix_beside_its_release() {
        let r = tmp("rf-hot"); let (p, bin) = fixture_v(&r, false, "10.99.0-bp.12.1"); assert_eq!(check_package(&p), Ok(()));
        let parent = r.join("home/kicad"); let old = parent.join("10.99.0-bp.12/bin"); std::fs::create_dir_all(&old).unwrap(); exe(&old.join("kicad-cli"), "#!/bin/sh\necho 10.99.0-bp.12\n");
        let (got, _) = fetch_run(&r, &p, &sysdirs(&bin));
        assert_eq!(got, Ok(parent.join("10.99.0-bp.12.1")));
        assert!(old.join("kicad-cli").is_file(), "the release it replaces is left where it is");
        assert_eq!(crate::files::kicad::ipc::read_runtime(&parent.join("10.99.0-bp.12.1/bin/kicad-cli")).version, "10.99.0-bp.12.1");
        assert!(older("10.99.0-bp.12", &p) && !older("10.99.0-bp.12.1", &p));
    }
    #[cfg(unix)] #[test] fn the_rust_fetch_refuses_a_wrong_digest_or_manifest_and_leaves_nothing() {
        let r = tmp("rf-bad"); let (mut p, bin) = fixture(&r, false); p.sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
        let (got, _) = fetch_run(&r, &p, &sysdirs(&bin)); assert_eq!(got, Err("KiCad archive checksum mismatch (the download was removed; Try again downloads it anew)".into()));
        assert_eq!(std::fs::read_dir(r.join("home/kicad")).unwrap().count(), 0, "the stage and the lock are gone");
        let r = tmp("rf-man"); let (p, bin) = fixture(&r, true);
        let (got, _) = fetch_run(&r, &p, &sysdirs(&bin)); assert_eq!(got, Err("KiCad manifest version mismatch".into()));
        assert!(!r.join("home/kicad/0.0.1-bp.1").exists());
    }
    #[cfg(unix)] #[test] fn the_rust_fetch_downloads_with_wget_without_curl_and_says_what_is_missing() {
        let r = tmp("rf-wget"); let (p, bin) = fixture(&r, false);
        let w = no_curl(&r, &bin, true); let (got, _) = fetch_run(&r, &p, &[w]); assert!(got.is_ok(), "{got:?}");
        let a = std::fs::read_to_string(r.join("wget.args")).unwrap(); assert!(a.lines().filter(|l| !l.starts_with("--spider")).all(|l| l.starts_with("-q --https-only -c ")), "resumes: {a}");
        let n = no_curl(&r, &bin, false); assert_eq!(tools_in(false, None, &[n]).unwrap_err(), "curl or wget is needed to download KiCad, and neither was found");
    }
    #[test] fn a_downloader_that_stops_without_a_word_is_said_in_plain_words() {
        assert_eq!(exit_words("curl", Some(18)), "the connection ended before the file was whole");
        assert_eq!(exit_words("wget", Some(4)), "the network failed");
        assert_eq!(exit_words("curl", Some(99)), "curl stopped (exit 99)"); assert_eq!(exit_words("tar", None), "tar stopped (killed)");
    }
    #[test] fn progress_is_said_in_megabytes_and_time_left() {
        let mb = 1u64 << 20;
        assert_eq!(Bytes { done: 312 * mb, total: 971 * mb, rate: 16 * mb, eta: 41 }.words(), "312 of 971 MB, about 45 s left");
        assert_eq!(Bytes { done: 12 * mb, total: 971 * mb, rate: 2 * mb, eta: 480 }.words(), "12 of 971 MB, about 8 min left");
        assert_eq!(Bytes { done: 312 * mb, total: 0, rate: 0, eta: 0 }.words(), "312 MB so far");
        assert_eq!(Bytes { done: 971 * mb, total: 971 * mb, rate: 0, eta: 0 }.words(), "971 of 971 MB");
        assert_eq!(Bytes { done: 1, total: 4, ..Default::default() }.share(), 0.25);
        let t0 = std::time::Instant::now(); let mut m = Meter::default();
        m.at(t0, 0, 100 * mb); let b = m.at(t0 + std::time::Duration::from_secs(2), 20 * mb, 100 * mb);
        assert_eq!((b.rate, b.eta), (10 * mb, 8));
        assert_eq!(content_length("HTTP/2 302\r\ncontent-length: 0\r\n\r\nHTTP/2 200\r\nContent-Length: 1018212352\r\n"), 1018212352);
        assert_eq!(content_length("  HTTP/1.1 200 OK\n  Content-Length: 42\n"), 42, "wget -S indents its headers");
        assert_eq!(content_length("HTTP/2 200\n"), 0);
    }
    /// A download that stops keeps what it has and says how far it got; the next try continues it; one already whole is not asked for again.
    #[cfg(unix)] #[test] fn a_stopped_download_is_kept_said_and_continued() {
        let r = tmp("rf-stop"); let (p, bin) = fixture(&r, false);
        let t = tools_in(false, None, &sysdirs(&bin)).unwrap(); let parent = r.join("home/kicad");
        let mut seen: Vec<Bytes> = vec![];
        std::fs::write(r.join("serve/FAILAT"), "100").unwrap();
        let got = rt().block_on(fetch(&t, "https://x/r/", &parent, &p, &r.join("log"), &mut |_| {}, &mut |b| seen.push(b)));
        std::fs::remove_file(r.join("serve/FAILAT")).unwrap();
        let e = got.unwrap_err(); assert!(e.starts_with("The download stopped at 0 of 0 MB") && e.ends_with(")"), "{e}");
        assert_eq!(std::fs::metadata(partial(&parent, &p)).unwrap().len(), 100, "the partial file is kept");
        assert!(seen.first().map_or(false, |b| b.total > 0), "the size is known from the start: {seen:?}");
        // the next try: the partial is continued (curl -C -), checked, installed, and removed
        let (got, steps) = fetch_run(&r, &p, &sysdirs(&bin));
        assert!(got.is_ok(), "{got:?}"); assert!(steps[0].contains("(continuing from 0 MB)"), "{steps:?}");
        assert!(!partial(&parent, &p).exists() && !parent.join(".partial").exists());
        // a whole download left by an earlier try is not downloaded again
        let r = tmp("rf-whole"); let (p, bin) = fixture(&r, false); let parent = r.join("home/kicad");
        std::fs::create_dir_all(parent.join(".partial")).unwrap(); std::fs::copy(r.join("serve").join(p.asset), partial(&parent, &p)).unwrap();
        let (got, _) = fetch_run(&r, &p, &sysdirs(&bin)); assert!(got.is_ok(), "{got:?}");
        assert!(!r.join("serve").join(format!("{}.calls", p.asset)).exists(), "not downloaded again");
    }
    #[test] fn a_download_resumes_and_retries() {
        let a = get_args(&Getter::Curl("/c".into()), "https://h/f", Path::new("/t/f")).join(" ");
        assert!(a.contains("-C - ") && a.contains("--retry 3") && a.contains("--proto =https") && a.ends_with("-o /t/f https://h/f"), "{a}");
        assert_eq!(get_args(&Getter::Wget("/w".into()), "https://h/f", Path::new("/t/f")).join(" "), "-q --https-only -c --tries=3 --waitretry=2 -O /t/f https://h/f");
    }
    #[test] fn windows_uses_its_own_curl_and_unzips_itself() {
        let r = tmp("rf-win"); let s = r.join("System32"); std::fs::create_dir_all(&s).unwrap();
        assert!(tools_in(true, Some(&r), &[]).unwrap_err().contains("curl.exe was not found"));
        std::fs::write(s.join("curl.exe"), "").unwrap();
        assert_eq!(tools_in(true, Some(&r), &[PathBuf::from("/elsewhere")]), Ok(Tools { get: Getter::Curl(s.join("curl.exe")), tar: s.join("tar.exe") }));
        assert_eq!(tools_in(true, None, &[]).unwrap_err(), "SystemRoot is not set");
    }
    #[test] fn the_sums_line_and_the_members_are_read_here() {
        assert_eq!(sum_for("abc  other.tgz\nDEF *./a.tar.gz\n", "a.tar.gz").as_deref(), Some("def"));
        assert_eq!(sum_for("abc  a.tar.gz.sig\n", "a.tar.gz"), None);
        assert_eq!(members_ok("r/\nr/bin/x\n./r/lib/y\n", "r"), Ok(()));
        for (bad, why) in [("r/../etc/x", "Unsafe"), ("/r/x", "Unsafe"), ("r\\..\\x", "Unsafe"), ("C:/r/x", "Unsafe"), ("other/x", "Unexpected"), ("rr/x", "Unexpected"), ("r/./x", "Unsafe")] {
            assert!(members_ok(&format!("r/\n{bad}\n"), "r").unwrap_err().starts_with(why), "{bad}");
        }
        assert!(members_ok("", "r").is_err());
        for p in [&LINUX, &MAC, &WINDOWS] { assert_eq!(check_package(p), Ok(()), "{}", p.version) }
    }
    #[cfg(unix)] #[test] fn a_link_out_of_the_runtime_is_refused() {
        let r = tmp("rf-link"); let rt_ = r.join("u/root"); std::fs::create_dir_all(rt_.join("lib")).unwrap();
        std::os::unix::fs::symlink("libx.so.1", rt_.join("lib/libx.so")).unwrap(); std::os::unix::fs::symlink("lib", rt_.join("bin")).unwrap(); std::os::unix::fs::symlink("../lib/libx.so.1", rt_.join("lib/up")).unwrap();
        assert_eq!(links_inside(&r.join("u"), &rt_), Ok(()));
        std::os::unix::fs::symlink("../../../etc", rt_.join("lib/out")).unwrap();
        assert!(links_inside(&r.join("u"), &rt_).unwrap_err().starts_with("Unsafe link"));
    }
    #[cfg(unix)] #[test] fn a_lock_left_by_a_gone_hub_is_taken_over_and_a_live_one_is_not() {
        let r = tmp("rf-lock"); let parent = r.join("k"); let lock = parent.join(".install-1.lock"); std::fs::create_dir_all(&lock).unwrap();
        std::fs::write(lock.join("pid"), std::process::id().to_string()).unwrap();
        assert!(take_lock(&parent, "1").err().unwrap().starts_with("Another KiCad installation is active"));
        std::fs::write(lock.join("pid"), "999999999").unwrap(); std::fs::create_dir_all(parent.join(".install-old")).unwrap();
        let h = take_lock(&parent, "1").expect("a dead holder's lock is taken over");
        assert!(!parent.join(".install-old").exists(), "its stage is gone"); drop(h); assert!(!lock.exists());
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
        let got = rt().block_on(run(Some(&s), &parent, &LINUX, &r.join("log"), |l| { if steps.is_empty() { let _ = std::fs::write(&go, ""); } steps.push(l) }, |_| {}));
        assert_eq!(got, Ok(parent.join("10.99.0-bp.10").to_string_lossy().to_string()));
        assert!(steps.first().unwrap().starts_with("Downloading backplane-kicad-linux") && steps.last().unwrap().starts_with("Installed in"), "{steps:?}");
    }
    #[cfg(unix)] #[test] fn a_standin_installer_that_fails_is_reported_in_its_own_words() {
        let r = tmp("sfail"); let s = r.join("standin");
        exe(&s, "#!/bin/sh\necho Downloading\necho 'Could not download https://x/y' >&2\nexit 1\n");
        let got = rt().block_on(run(Some(&s), &r.join("kicad"), &LINUX, &r.join("log"), |_| {}, |_| {}));
        assert_eq!(got, Err("Could not download https://x/y".into()));
        // ending well without placing KiCad is a failure too
        exe(&s, "#!/bin/sh\necho done\n");
        let got = rt().block_on(run(Some(&s), &r.join("kicad"), &LINUX, &r.join("log"), |_| {}, |_| {}));
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
