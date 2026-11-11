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
t=$(mktemp); trap 'rm -f "$t"' 0
echo "Downloading the installer"
curl --proto '=https' --tlsv1.2 -fsSL -o "$t" "$1" || { echo "Could not download $1" >&2; exit 1; }
echo "Running the installer"
bash "$t"
"#;

/// Codex's standalone binary: the digest GitHub lists for the release asset, the download from that tag, the check, then the move into `$1` (a folder
/// the hub puts on its PATH). Arguments: folder, asset, name to give it, kind (tar.gz or raw). Nothing is placed unless the digest matches.
pub const CODEX_SH: &str = r#"set -eu
dest=$1; asset=$2; name=$3; kind=$4
fail() { echo "$*" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || fail 'curl is needed to download Codex and was not found'
if command -v cygpath >/dev/null 2>&1; then dest=$(cygpath -u "$dest"); fi
case "$asset" in codex-*) ;; *) fail 'Invalid Codex asset' ;; esac
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' 0
echo "Looking up the latest Codex release"
curl --proto '=https' --tlsv1.2 -fsSL -o "$tmp/rel.json" https://api.github.com/repos/openai/codex/releases/latest || fail 'Could not read the Codex release list'
# the API answers on one line: one field per line, then read the tag and the digest that follows the asset's name
tr ',{}' '\n\n\n' < "$tmp/rel.json" | tr -d ' \r' > "$tmp/fields"
tag=$(sed -n 's/^"tag_name":"\([^"]*\)".*/\1/p' "$tmp/fields" | head -n 1)
want=$(awk -v n="\"name\":\"$asset\"" '$0==n { f=1 } f && index($0, "\"digest\":\"sha256:") == 1 { sub(/^"digest":"sha256:/, ""); sub(/".*/, ""); print; exit }' "$tmp/fields")
[ -n "$tag" ] && [ "${#want}" -eq 64 ] || fail "The Codex release lists no checksum for $asset"
echo "Downloading $asset"
curl --proto '=https' --tlsv1.2 -fsSL -o "$tmp/pkg" "https://github.com/openai/codex/releases/download/$tag/$asset" || fail "Could not download $asset"
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
        "freecad" => match os { "windows" if winget => Some(Plan::Winget("FreeCAD.FreeCAD")), _ => Some(Plan::Link("https://www.freecad.org/downloads.php")) },
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

static STATE: Mutex<BTreeMap<String, (String, String)>> = Mutex::new(BTreeMap::new());
fn snapshot() -> String { let m = STATE.lock().unwrap(); Value::Object(m.iter().map(|(k, (s, n))| (k.clone(), json!({"state": s, "note": n}))).collect()).to_string() }
async fn say(hub: &Arc<Hub>, tool: &str, state: &str, note: &str) {
    STATE.lock().unwrap().insert(tool.into(), (state.into(), note.into()));
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
        Plan::Link(_) => None,
    }
}

/// `onboard.install {tool}`: started, or refused with the reason. How it goes is the info key `onboard.installs`; `after` runs the checks again at the end.
pub async fn start(hub: &Arc<Hub>, tool: &str, winget: bool, node: bool, after: impl FnOnce(Arc<Hub>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + 'static) -> Result<String, String> {
    if crate::copy::on() { return Err("a copy of the log installs nothing".into()) }
    let Some(p) = plan(tool, &hub.cfg.os, &hub.cfg.arch, winget, node) else { return Err(format!("{tool} has no install button; follow its guide")) };
    let Some((prog, args)) = command(&p) else { return Err("open its download page".into()) };
    if STATE.lock().unwrap().get(tool).map_or(false, |(s, _)| s == "running") { return Err(format!("{tool} is being installed already")) }
    let tool = tool.to_string(); let name = tool.clone();
    say(hub, &tool, "running", "Starting.").await;
    let h = hub.clone();
    let log = hub.cfg.home.join("installs"); let _ = std::fs::create_dir_all(&log); let log = log.join(format!("{tool}.log"));
    tokio::spawn(async move {
        let r = run(&h, &tool, &prog, &args, &log).await;
        match r { Ok(()) => say(&h, &tool, "done", "Installed.").await, Err(why) => say(&h, &tool, "failed", &why).await }
        after(h).await;
    });
    Ok(format!("installing {name}"))
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
            ("freecad", "windows", "x64", false, false, |p| matches!(p, Plan::Link(u) if u.contains("freecad.org"))),
            ("freecad", "linux", "x64", true, false, |p| matches!(p, Plan::Link(_))),
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
    #[cfg(unix)] fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-tools-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
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
    #[test] fn the_last_line_is_what_the_row_says() { assert_eq!(last_line("a\n\n b \n\n"), "b"); assert_eq!(last_line(""), ""); }
}
