//! Self-hosted SolidCraft Web (`solidcraft-web` WASM): pinned release fetch/verify, streaming HTTP serving,
//! pre-seeded embedded preferences, OPFS/VFS file bridge, and guarded save-back to the project folder.
use super::runtime;
use crate::{
    api::{Caller, Fut, HttpReply, RpcHandler},
    auth::Req,
    files::{fm, kicad::install as ki, ops},
    hub::Hub,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Pinned `solidcraft-web` release asset from `tangency-labs/solidcraft` v0.50.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPin {
    pub version: &'static str,
    pub asset: &'static str,
    pub root: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

pub const WEB_PIN: WebPin = WebPin {
    version: runtime::VERSION,
    asset: "solidcraft-web-0.50.0.zip",
    root: "solidcraft-web-0.50.0",
    sha256: "22fde12d3d98110b997f30e888f604609a70f1a4ebaf488c5d8c31097d3fe328",
    size: 93_115_471,
};

/// Unpacked bundle cap (the 0.50.0 bundle unpacks to ~248 MB).
pub const WEB_UNZIP_CAP: u64 = 512 << 20;

/// Where managed web bundles live: `<home>/solidcraft/web` (or `%LOCALAPPDATA%\Backplane\solidcraft\web` on Windows).
pub fn web_parent(home: &Path) -> PathBuf {
    runtime::parent(home).join("web")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebBundle {
    pub dir: PathBuf,
    pub version: String,
    pub source: &'static str,
    pub js: String,
    pub wasm: String,
    pub css: String,
}

/// Discovers the hashed entry JS, WASM, and optional loading CSS inside an unpacked `solidcraft-web` folder.
pub fn discover_assets(dir: &Path) -> Option<(String, String, String)> {
    if !dir.join("index.html").is_file() {
        return None;
    }
    let mut js = Vec::new();
    let mut wasm = Vec::new();
    let mut css = Vec::new();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_file() {
            continue;
        }
        let Ok(md) = entry.metadata() else { continue };
        if md.len() == 0 {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("solidcraft-web-") && name.ends_with(".js") {
            js.push(name);
        } else if name.starts_with("solidcraft-web-") && name.ends_with("_bg.wasm") {
            wasm.push(name);
        } else if name.starts_with("loading-") && name.ends_with(".css") {
            css.push(name);
        }
    }
    js.sort();
    wasm.sort();
    css.sort();
    if js.len() != 1 || wasm.len() != 1 {
        return None;
    }
    Some((
        js.into_iter().next()?,
        wasm.into_iter().next()?,
        css.into_iter().next().unwrap_or_default(),
    ))
}

#[cfg(test)]
pub(crate) fn test_over() -> &'static Mutex<HashMap<PathBuf, PathBuf>> {
    static OVER: OnceLock<Mutex<HashMap<PathBuf, PathBuf>>> = OnceLock::new();
    OVER.get_or_init(Default::default)
}

/// Resolves an installed `solidcraft-web` bundle for `home` (`BACKPLANE_SOLIDCRAFT_WEB`, then `<web_parent>/<version>`).
pub fn find(home: &Path) -> Option<WebBundle> {
    #[cfg(test)]
    let over = test_over()
        .lock()
        .unwrap()
        .get(home)
        .cloned()
        .or_else(|| std::env::var_os("BACKPLANE_SOLIDCRAFT_WEB").filter(|v| !v.is_empty()).map(PathBuf::from));
    #[cfg(not(test))]
    let over = std::env::var_os("BACKPLANE_SOLIDCRAFT_WEB")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    if let Some(dir) = over {
        if let Some((js, wasm, css)) = discover_assets(&dir) {
            return Some(WebBundle {
                dir,
                version: WEB_PIN.version.to_string(),
                source: "env",
                js,
                wasm,
                css,
            });
        }
    }
    let parent = web_parent(home);
    let versions: Vec<String> = std::fs::read_dir(&parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    for v in runtime::newest_first(versions) {
        let dir = parent.join(&v);
        if let Some((js, wasm, css)) = discover_assets(&dir) {
            return Some(WebBundle {
                dir,
                version: v,
                source: "managed",
                js,
                wasm,
                css,
            });
        }
    }
    None
}

#[derive(Clone, Debug, Default)]
struct InstallState {
    running: bool,
    state: String,
    note: String,
    done: u64,
    total: u64,
}

fn installs() -> &'static Mutex<HashMap<PathBuf, InstallState>> {
    static MAP: OnceLock<Mutex<HashMap<PathBuf, InstallState>>> = OnceLock::new();
    MAP.get_or_init(Default::default)
}

fn get_install_state(home: &Path) -> InstallState {
    installs()
        .lock()
        .unwrap()
        .get(home)
        .cloned()
        .unwrap_or_else(|| InstallState {
            total: WEB_PIN.size,
            ..Default::default()
        })
}

fn set_install_state(home: &Path, next: InstallState) {
    installs().lock().unwrap().insert(home.to_path_buf(), next);
}

pub fn running(home: &Path) -> bool {
    installs()
        .lock()
        .unwrap()
        .get(home)
        .is_some_and(|s| s.running)
}

/// JSON picture returned by `solidcraft.web.status` and published to `solidcraft.web`.
pub fn status_json(home: &Path) -> Value {
    let found = find(home);
    let st = get_install_state(home);
    let folder = web_parent(home);
    json!({
        "installed": found.is_some(),
        "version": found.as_ref().map_or(WEB_PIN.version, |b| b.version.as_str()),
        "source": found.as_ref().map_or("", |b| b.source),
        "available": true,
        "size": WEB_PIN.size,
        "from": runtime::FROM,
        "folder": folder.to_string_lossy(),
        "installing": st.running,
        "copy": crate::copy::on(),
        "install": {
            "state": st.state,
            "note": st.note,
            "done": st.done,
            "total": if st.total == 0 { WEB_PIN.size } else { st.total },
        }
    })
}

pub async fn publish(hub: &Arc<Hub>) -> Value {
    let home = hub.cfg.home.clone();
    let v = tokio::task::spawn_blocking(move || status_json(&home))
        .await
        .unwrap_or_else(|_| json!({"installed": false}));
    hub.set_info("solidcraft.web", &v.to_string()).await;
    v
}

// ---- Download, verify, unpack, place --------------------------------------------------------------------------------------------

pub fn check_pin(p: &WebPin) -> Result<(), String> {
    let name = |s: &str| {
        !s.is_empty()
            && s.len() < 100
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    };
    if !name(p.asset)
        || !name(p.root)
        || !p.asset.contains(p.version)
        || !p.asset.ends_with(".zip")
    {
        return Err("Invalid SolidCraft Web archive name".into());
    }
    if p.sha256.len() != 64
        || !p
            .sha256
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err("Invalid SolidCraft Web pinned digest".into());
    }
    if p.size == 0 {
        return Err("Invalid SolidCraft Web archive size".into());
    }
    Ok(())
}

pub fn refusal(consent: bool, is_running: bool, installed: bool, copy: bool) -> Option<String> {
    if copy {
        return Some("This is a copy of a hub: it installs nothing".into());
    }
    if !consent {
        return Some("Installing SolidCraft Web needs the person's consent".into());
    }
    if is_running {
        return Some("SolidCraft Web is already being installed".into());
    }
    if installed {
        return Some("SolidCraft Web is already installed".into());
    }
    None
}

fn sc(e: String) -> String {
    e.replace("Backplane KiCad", "SolidCraft Web")
        .replace("KiCad", "SolidCraft Web")
}

/// Validates an unpacked `solidcraft-web` stage directory and moves it atomically into `<parent>/<version>`.
pub fn place_web(unpack: &Path, parent: &Path, p: &WebPin) -> Result<PathBuf, String> {
    let root = unpack.join(p.root);
    if !root.is_dir() {
        return Err("SolidCraft Web archive layout is not the expected one".into());
    }
    ki::links_inside(unpack, &root).map_err(sc)?;
    if discover_assets(&root).is_none() {
        return Err("SolidCraft Web archive is incomplete (index/js/wasm missing)".into());
    }
    let dir = parent.join(p.version);
    if dir.symlink_metadata().is_ok() {
        return Err(format!(
            "{} already exists; the installation there is kept",
            dir.display()
        ));
    }
    std::fs::rename(&root, &dir)
        .map_err(|e| format!("Could not move SolidCraft Web into {}: {e}", dir.display()))?;
    Ok(dir)
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Downloads, verifies against `WEB_PIN.sha256` and `SHA256SUMS.txt`, unpacks, and places `solidcraft-web`.
pub async fn fetch(
    base: &str,
    parent: &Path,
    p: &WebPin,
    mut say: impl FnMut(&str, u64, u64),
) -> Result<PathBuf, String> {
    check_pin(p)?;
    let t = ki::tools().map_err(sc)?;
    std::fs::create_dir_all(parent.join(".partial"))
        .map_err(|e| format!("Could not make {}: {e}", parent.display()))?;
    let log = parent.join(".install.log");
    let stage = Stage(parent.join(format!(
        ".install-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    )));
    std::fs::create_dir(&stage.0).map_err(|e| e.to_string())?;
    let pkg = parent.join(".partial").join(p.asset);
    let gp = match &t.get {
        ki::Getter::Curl(x) | ki::Getter::Wget(x) => x.clone(),
    };
    let have = |f: &Path| std::fs::metadata(f).map_or(0, |m| m.len());
    let args = |url: &str, to: &Path| {
        let a = ki::get_args(&t.get, url, to);
        if url.starts_with("http://") {
            a.into_iter()
                .filter(|x| !matches!(x.as_str(), "--proto" | "=https" | "--tlsv1.2" | "--https-only"))
                .collect()
        } else {
            a
        }
    };
    say("Downloading", have(&pkg), p.size);
    if have(&pkg) != p.size {
        if have(&pkg) > p.size {
            let _ = std::fs::remove_file(&pkg);
        }
        let mut ok = Err(String::new());
        for _ in 0..3 {
            ok = ki::tool(&gp, &args(&format!("{base}{}", p.asset), &pkg), &log, || {
                say("Downloading", have(&pkg), p.size)
            })
            .await;
            if ok.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        if let Err(why) = ok {
            return Err(format!(
                "The download stopped ({})",
                if why.is_empty() {
                    "no answer".to_string()
                } else {
                    why
                }
            ));
        }
    }
    let sums = stage.0.join("SUMS");
    ki::tool(&gp, &args(&format!("{base}SHA256SUMS.txt"), &sums), &log, || {})
        .await
        .map_err(|_| format!("Could not download {base}SHA256SUMS.txt"))?;
    say("Checking", p.size, p.size);
    let sums_text = std::fs::read_to_string(&sums).unwrap_or_default();
    let want = ki::sum_for(&sums_text, p.asset).unwrap_or_default();
    let pk = pkg.to_path_buf();
    let got = tokio::task::spawn_blocking(move || ki::sha256_file(&pk))
        .await
        .map_err(|e| e.to_string())??;
    if want != got || got != p.sha256 {
        let _ = std::fs::remove_file(&pkg);
        return Err("SolidCraft Web archive checksum mismatch (the download was removed)".into());
    }
    say("Unpacking", p.size, p.size);
    let unpack = stage.0.join("unpack");
    std::fs::create_dir(&unpack).map_err(|e| e.to_string())?;
    let (z, r, u) = (pkg.to_path_buf(), p.root.to_string(), unpack.clone());
    tokio::task::spawn_blocking(move || ki::unzip_capped(&z, &r, &u, WEB_UNZIP_CAP))
        .await
        .map_err(|e| e.to_string())?
        .map_err(sc)?;
    let (u, par) = (unpack, parent.to_path_buf());
    let pin = p.clone();
    let dir = tokio::task::spawn_blocking(move || place_web(&u, &par, &pin))
        .await
        .map_err(|e| e.to_string())??;
    let _ = std::fs::remove_file(&pkg);
    let _ = std::fs::remove_dir(parent.join(".partial"));
    Ok(dir)
}

async fn say_install(hub: &Arc<Hub>, running_now: bool, state: &str, note: &str, done: u64, total: u64) {
    set_install_state(
        &hub.cfg.home,
        InstallState {
            running: running_now,
            state: state.to_string(),
            note: note.to_string(),
            done,
            total,
        },
    );
    hub.set_info(
        "solidcraft.web.install",
        &json!({"state": state, "note": note, "version": WEB_PIN.version, "done": done, "total": total}).to_string(),
    )
    .await;
    publish(hub).await;
}

pub async fn install(hub: &Arc<Hub>, consent: bool) -> Result<Value, String> {
    let home = hub.cfg.home.clone();
    let installed = {
        let h = home.clone();
        tokio::task::spawn_blocking(move || find(&h).is_some())
            .await
            .unwrap_or(false)
    };
    let already_running = {
        let mut map = installs().lock().unwrap();
        let entry = map.entry(home.clone()).or_insert_with(|| InstallState {
            total: WEB_PIN.size,
            ..Default::default()
        });
        if let Some(why) = refusal(consent, entry.running, installed, crate::copy::on()) {
            return Err(why);
        }
        entry.running = true;
        entry.state = "running".into();
        entry.note = "Downloading".into();
        entry.done = 0;
        entry.total = WEB_PIN.size;
        false
    };
    let _ = already_running;
    say_install(hub, true, "running", "Downloading", 0, WEB_PIN.size).await;
    let hub2 = hub.clone();
    tokio::spawn(async move {
        let par = web_parent(&hub2.cfg.home);
        let base = runtime::base_url(std::env::var("BACKPLANE_SOLIDCRAFT_FEED").ok().as_deref());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, u64, u64)>();
        let h_prog = hub2.clone();
        let teller = tokio::spawn(async move {
            let mut last = (String::new(), 0u64, std::time::Instant::now());
            while let Some((s, d, t)) = rx.recv().await {
                if s != last.0 || (d != last.1 && last.2.elapsed().as_millis() >= 700) {
                    say_install(&h_prog, true, "running", &s, d, t).await;
                    last = (s, d, std::time::Instant::now());
                }
            }
        });
        let r = fetch(&base, &par, &WEB_PIN, |s, d, t| {
            let _ = tx.send((s.to_string(), d, t));
        })
        .await;
        drop(tx);
        let _ = teller.await;
        match r {
            Ok(dir) => {
                eprintln!("solidcraft-web: installed in {}", dir.display());
                say_install(&hub2, false, "done", "", WEB_PIN.size, WEB_PIN.size).await;
            }
            Err(why) => {
                eprintln!("solidcraft-web: install failed: {why}");
                say_install(&hub2, false, "failed", &why, 0, WEB_PIN.size).await;
            }
        }
    });
    Ok(status_json(&hub.cfg.home))
}

// ---- Self-hosted HTML + JS bridge -----------------------------------------------------------------------------------------------

pub const SC_WEB_SECURITY_HEADERS: &str = concat!(
    "Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; ",
    "style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; font-src 'self' data:; ",
    "connect-src 'self' blob: data:; worker-src 'self' blob:; child-src 'self' blob:; ",
    "object-src 'none'; frame-ancestors 'self'; base-uri 'self'; form-action 'self'\r\n",
    "X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n",
    "Cross-Origin-Resource-Policy: same-origin\r\n",
);

fn html_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&#39;".into(),
            _ => c.to_string(),
        })
        .collect()
}

/// Self-hosted HTML shell for `solidcraft-web`: zero `solidcraft.io` calls, no upstream `sw.js`, no inline script.
pub fn render_index_html(b: &WebBundle) -> String {
    let css_link = if b.css.is_empty() {
        String::new()
    } else {
        format!(
            "  <link rel=\"stylesheet\" href=\"./{}\">\n",
            html_escape(&b.css)
        )
    };
    let js_esc = html_escape(&b.js);
    format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no\">\n\
  <meta name=\"referrer\" content=\"no-referrer\">\n\
  <title>SolidCraft Web (Self-Hosted)</title>\n\
{css_link}\
  <style>\n\
    html, body {{ margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #262930; }}\n\
    #solidcraft_canvas {{ width: 100%; height: 100%; display: block; touch-action: none; }}\n\
  </style>\n\
  <link rel=\"modulepreload\" href=\"./{js_esc}\">\n\
</head>\n\
<body>\n\
  <canvas id=\"solidcraft_canvas\"></canvas>\n\
  <div id=\"solidcraft_loading\" role=\"status\"><img src=\"./loading-logo.svg\" alt=\"\"><span class=\"sr-only\">Loading SolidCraft&hellip;</span></div>\n\
  <script type=\"module\" src=\"./bp-boot.js\"></script>\n\
</body>\n\
</html>\n"
    )
}

/// Generates `/solidcraft-web/bp-boot.js`: pre-seeds prefs, bridges `/files/get` & `/solidcraft-web/save` via OPFS/localStorage,
/// and communicates with Web UI (`postMessage`) and Mobile WebViews (`webkit`/`BackplaneBridge`).
pub fn render_boot_js(b: &WebBundle) -> String {
    let js_json = serde_json::to_string(&format!("./{}", b.js)).unwrap_or_else(|_| "\"./solidcraft-web.js\"".into());
    let wasm_json = serde_json::to_string(&format!("./{}", b.wasm)).unwrap_or_else(|_| "\"./solidcraft-web_bg.wasm\"".into());
    format!(
        r#"const JS_ENTRY = {js_json};
const WASM_ENTRY = {wasm_json};
const DESIGN_EXTS = new Set([".sc", ".solidcraft"]);
const CAD_EXTS = new Set([".sc", ".solidcraft", ".step", ".stp", ".stl", ".3mf", ".iges", ".igs", ".obj", ".glb"]);

const params = new URLSearchParams(window.location.search);
let currentThread = params.get("thread") || "";
let currentPath = params.get("path") || "";
const initialTheme = params.get("theme") || "";
const readOnly = params.get("readonly") === "1";
const INSTANCE_ID = Math.random().toString(36).slice(2, 10);
const STALE_BRIDGE_MS = 60 * 60 * 1000;

function bridgeTs(name, prefix) {{
  if (!name.startsWith(prefix)) return 0;
  const rest = name.slice(prefix.length);
  const ts = parseInt(rest.split("-")[0], 10);
  return Number.isFinite(ts) ? ts : 0;
}}

function notifyHost(msg) {{
  try {{
    if (window.parent && window.parent !== window) {{
      window.parent.postMessage(msg, window.location.origin);
    }}
  }} catch {{}}
  try {{
    window.webkit?.messageHandlers?.backplane?.postMessage(msg);
  }} catch {{}}
  try {{
    window.BackplaneBridge?.postMessage(JSON.stringify(msg));
  }} catch {{}}
}}

function seedPrefs(theme) {{
  try {{
    const raw = window.localStorage.getItem("solidcraft.prefs");
    let parsed = {{}};
    if (raw) {{
      try {{ parsed = JSON.parse(raw) || {{}}; }} catch {{ parsed = {{}}; }}
    }}
    const prev = (parsed && typeof parsed.preferences === "object" && parsed.preferences) || {{}};
    parsed.preferences = {{
      ...prev,
      ask_import_options: false,
      show_discord_button: false,
      show_donate_button: false,
      check_updates: false,
      usage_statistics: false,
      show_home_on_startup: false,
      recover_after_browser_leave: false,
      keep_unsaved_in_browser: false,
      ...(theme === "light" || theme === "dark" ? {{ viewport_theme: theme }} : {{}}),
    }};
    window.localStorage.setItem("solidcraft.prefs", JSON.stringify(parsed));
    window.localStorage.setItem("usage_statistics", JSON.stringify({{ install_id: "", notice_ok: true, days: {{}}, forget: [] }}));
  }} catch {{}}
}}

function extOf(p) {{
  const i = p.lastIndexOf(".");
  return i >= 0 ? p.slice(i).toLowerCase() : "";
}}

function baseOf(p) {{
  const s = p.replace(/\\/g, "/");
  const i = s.lastIndexOf("/");
  return i >= 0 ? s.slice(i + 1) : s;
}}

function safeOpfsName(prefix, path) {{
  const base = baseOf(path) || "part.sc";
  const ext = extOf(base);
  const stem = (ext ? base.slice(0, -ext.length) : base).replace(/[^a-zA-Z0-9._-]/g, "_").slice(0, 48) || "part";
  return `${{prefix}}${{stem}}${{ext || ".sc"}}`;
}}

function encodeLocalBytes(bytes) {{
  try {{
    const text = new TextDecoder("utf-8", {{ fatal: true }}).decode(bytes);
    return "t" + text;
  }} catch {{
    let hex = "x";
    for (let i = 0; i < bytes.length; i++) {{
      hex += bytes[i].toString(16).padStart(2, "0");
    }}
    return hex;
  }}
}}

function decodeLocalBytes(raw) {{
  if (typeof raw !== "string" || !raw) return null;
  if (raw[0] === "t") return new TextEncoder().encode(raw.slice(1));
  if (raw[0] === "x") {{
    const hex = raw.slice(1);
    const out = new Uint8Array(Math.floor(hex.length / 2));
    for (let i = 0; i < out.length; i++) {{
      out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16) || 0;
    }}
    return out;
  }}
  return null;
}}

async function opfsRoot() {{
  if (!navigator.storage || typeof navigator.storage.getDirectory !== "function") return null;
  try {{
    return await navigator.storage.getDirectory();
  }} catch {{
    return null;
  }}
}}

async function clearBridgeFiles() {{
  const cutoff = Date.now() - STALE_BRIDGE_MS;
  const isStaleBridge = (n) => {{
    const t = bridgeTs(n, "bp-active-") || bridgeTs(n, "bp-save-");
    return t > 0 && t < cutoff;
  }};
  const dir = await opfsRoot();
  if (dir) {{
    try {{
      for await (const [name] of dir.entries()) {{
        if (isStaleBridge(name)) {{
          await dir.removeEntry(name).catch(() => {{}});
        }}
      }}
    }} catch {{}}
  }}
  try {{
    const toRemove = [];
    for (let i = 0; i < window.localStorage.length; i++) {{
      const k = window.localStorage.key(i);
      if (!k || !k.startsWith("solidcraft.file.")) continue;
      const name = k.slice("solidcraft.file.".length);
      if (isStaleBridge(name)) toRemove.push(k);
    }}
    for (const k of toRemove) window.localStorage.removeItem(k);
  }} catch {{}}
}}

async function writeBridgeFile(name, bytes) {{
  const dir = await opfsRoot();
  if (dir) {{
    try {{
      const fh = await dir.getFileHandle(name, {{ create: true }});
      if (typeof fh.createWritable === "function") {{
        const w = await fh.createWritable();
        await w.write(bytes);
        await w.close();
        return;
      }}
    }} catch {{}}
  }}
  window.localStorage.setItem("solidcraft.file." + name, encodeLocalBytes(bytes));
}}

async function readBridgeEntry(name) {{
  const dir = await opfsRoot();
  if (dir) {{
    try {{
      const fh = await dir.getFileHandle(name, {{ create: false }});
      const file = await fh.getFile();
      const buf = await file.arrayBuffer();
      return {{ bytes: new Uint8Array(buf), stamp: file.lastModified || 0 }};
    }} catch {{}}
  }}
  const raw = decodeLocalBytes(window.localStorage.getItem("solidcraft.file." + name));
  return raw ? {{ bytes: raw, stamp: 0 }} : null;
}}

async function removeBridgeFile(name) {{
  const dir = await opfsRoot();
  if (dir) {{
    await dir.removeEntry(name).catch(() => {{}});
  }}
  try {{
    window.localStorage.removeItem("solidcraft.file." + name);
  }} catch {{}}
}}

async function fetchProjectFile(thread, path) {{
  const url = `/files/get?thread=${{encodeURIComponent(thread)}}&path=${{encodeURIComponent(path)}}`;
  const resp = await fetch(url, {{ credentials: "same-origin" }});
  if (!resp.ok) {{
    throw new Error(`Could not load ${{path}} (HTTP ${{resp.status}})`);
  }}
  return new Uint8Array(await resp.arrayBuffer());
}}

async function waitForApi(timeoutMs = 30000) {{
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {{
    if (window.solidcraft && typeof window.solidcraft.execute === "function" && typeof window.solidcraft.request === "function") {{
      return window.solidcraft;
    }}
    await new Promise((r) => setTimeout(r, 50));
  }}
  throw new Error("SolidCraft Web API timed out");
}}

async function openStagedOpfs(api, opfsName, timeoutMs = 15000) {{
  const deadline = performance.now() + timeoutMs;
  let lastErr = null;
  while (performance.now() < deadline) {{
    try {{
      await api.execute("doc.open", {{ path: `/opfs/${{opfsName}}` }});
      return;
    }} catch (err) {{
      lastErr = err;
      await new Promise((r) => setTimeout(r, 100));
    }}
  }}
  throw lastErr || new Error("Timed out opening staged CAD file");
}}

async function applyEmbeddedPrefs(api, theme) {{
  const prefs = {{
    ask_import_options: false,
    show_discord_button: false,
    show_donate_button: false,
    check_updates: false,
    usage_statistics: false,
    show_home_on_startup: false,
  }};
  if (theme === "light" || theme === "dark") prefs.viewport_theme = theme;
  await api.request("ui.prefs", prefs).catch(() => {{}});
}}

async function waitForExportedBytes(saveName, timeoutMs = 10000) {{
  const deadline = performance.now() + timeoutMs;
  let lastLen = -1;
  let lastStamp = -1;
  while (performance.now() < deadline) {{
    const entry = await readBridgeEntry(saveName);
    if (entry && entry.bytes.byteLength > 0) {{
      if (entry.bytes.byteLength === lastLen && entry.stamp === lastStamp) {{
        return entry.bytes;
      }}
      lastLen = entry.bytes.byteLength;
      lastStamp = entry.stamp;
      await new Promise((r) => setTimeout(r, 150));
      continue;
    }}
    await new Promise((r) => setTimeout(r, 80));
  }}
  const finalEntry = await readBridgeEntry(saveName);
  return finalEntry && finalEntry.bytes.byteLength > 0 && finalEntry.bytes.byteLength === lastLen ? finalEntry.bytes : null;
}}

let saving = false;

async function saveCurrent() {{
  if (saving) return;
  if (readOnly) {{
    notifyHost({{ type: "bp:solidcraft:error", op: "save", error: "Read-only copy: edits cannot be saved." }});
    return;
  }}
  if (!currentThread || !currentPath) {{
    notifyHost({{ type: "bp:solidcraft:error", op: "save", error: "No project CAD file is open." }});
    return;
  }}
  const ext = extOf(currentPath);
  if (!CAD_EXTS.has(ext)) {{
    notifyHost({{ type: "bp:solidcraft:error", op: "save", error: `Unsupported CAD extension: ${{ext}}` }});
    return;
  }}
  saving = true;
  notifyHost({{ type: "bp:solidcraft:saving", thread: currentThread, path: currentPath }});
  const saveName = safeOpfsName(`bp-save-${{Date.now()}}-${{INSTANCE_ID}}-`, currentPath);
  try {{
    const api = await waitForApi(5000);
    let rev = null;
    try {{
      const r = await api.request("document.revision", {{}});
      if (r && typeof r.revision === "number") rev = r.revision;
    }} catch {{}}
    if (DESIGN_EXTS.has(ext)) {{
      await api.execute("file.save_as", {{ path: `/opfs/${{saveName}}`, overwrite: true }});
    }} else {{
      await api.execute("file.export", {{ path: `/opfs/${{saveName}}` }});
    }}
    let bytes = await waitForExportedBytes(saveName, 10000);
    if (!bytes && DESIGN_EXTS.has(ext)) {{
      const exp = await api.request("document.export", {{}});
      if (exp && typeof exp.text === "string") {{
        bytes = new TextEncoder().encode(exp.text);
        if (typeof exp.revision === "number") rev = exp.revision;
      }}
    }}
    if (!bytes || bytes.byteLength === 0) {{
      throw new Error("SolidCraft produced an empty file");
    }}
    const url = `/solidcraft-web/save?thread=${{encodeURIComponent(currentThread)}}&path=${{encodeURIComponent(currentPath)}}`;
    const resp = await fetch(url, {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Content-Type": "application/octet-stream" }},
      body: bytes,
    }});
    let payload = null;
    try {{ payload = await resp.json(); }} catch {{}}
    if (!resp.ok || (payload && payload.ok === false)) {{
      throw new Error(payload?.error || `Save failed (HTTP ${{resp.status}})`);
    }}
    if (DESIGN_EXTS.has(ext) && typeof rev === "number") {{
      await api.request("document.saved", {{ revision: rev }}).catch(() => {{}});
    }}
    notifyHost({{ type: "bp:solidcraft:saved", thread: currentThread, path: currentPath, size: bytes.byteLength }});
  }} catch (err) {{
    notifyHost({{ type: "bp:solidcraft:error", op: "save", error: err instanceof Error ? err.message : String(err) }});
  }} finally {{
    saving = false;
    await removeBridgeFile(saveName);
  }}
}}

async function openAnother(thread, path) {{
  if (!thread || !path) return;
  const nextUrl = new URL(window.location.href);
  nextUrl.searchParams.set("thread", thread);
  nextUrl.searchParams.set("path", path);
  window.location.replace(nextUrl.toString());
}}

window.__bpSolidCraft = {{
  save: saveCurrent,
  fit: async () => {{
    try {{
      const api = await waitForApi(5000);
      await api.request("ui.view", {{ view: "fit", animate: false }});
    }} catch {{}}
  }},
  setTheme: async (light) => {{
    const theme = light ? "light" : "dark";
    seedPrefs(theme);
    try {{
      const api = await waitForApi(5000);
      await api.request("ui.prefs", {{ viewport_theme: theme }});
    }} catch {{}}
  }},
  open: openAnother,
}};

window.addEventListener("message", (ev) => {{
  if (ev.origin !== window.location.origin) return;
  const d = ev.data;
  if (!d || typeof d !== "object") return;
  switch (d.type) {{
    case "bp:solidcraft:save":
      void saveCurrent();
      break;
    case "bp:solidcraft:fit":
      void window.__bpSolidCraft.fit();
      break;
    case "bp:solidcraft:theme":
      void window.__bpSolidCraft.setTheme(Boolean(d.light));
      break;
    case "bp:solidcraft:open":
      if (typeof d.thread === "string" && typeof d.path === "string") {{
        void openAnother(d.thread, d.path);
      }}
      break;
  }}
}});

window.addEventListener("keydown", (ev) => {{
  if ((ev.ctrlKey || ev.metaKey) && !ev.altKey && ev.key.toLowerCase() === "s") {{
    ev.preventDefault();
    ev.stopPropagation();
    void saveCurrent();
  }}
}}, true);

(async () => {{
  const t0 = performance.now();
  seedPrefs(initialTheme);
  await clearBridgeFiles();

  let stagedName = "";
  let preloadError = "";
  if (currentThread && currentPath) {{
    try {{
      const bytes = await fetchProjectFile(currentThread, currentPath);
      stagedName = safeOpfsName(`bp-active-${{Date.now()}}-${{INSTANCE_ID}}-`, currentPath);
      await writeBridgeFile(stagedName, bytes);
    }} catch (err) {{
      preloadError = err instanceof Error ? err.message : String(err);
    }}
  }}

  try {{
    const mod = await import(JS_ENTRY);
    const wasm = await mod.default({{ module_or_path: WASM_ENTRY }});
    window.wasmBindings = mod;
    window.dispatchEvent(new CustomEvent("TrunkApplicationStarted", {{ detail: {{ wasm }} }}));
    const api = await waitForApi(30000);
    await applyEmbeddedPrefs(api, initialTheme);
    if (stagedName) {{
      await openStagedOpfs(api, stagedName, 15000);
      await api.request("ui.view", {{ view: "fit", animate: false }}).catch(() => {{}});
      await removeBridgeFile(stagedName);
    }} else if (preloadError) {{
      notifyHost({{ type: "bp:solidcraft:error", op: "open", error: preloadError }});
    }}
    notifyHost({{
      type: "bp:solidcraft:ready",
      thread: currentThread,
      path: currentPath,
      loadMs: Math.round(performance.now() - t0),
    }});
  }} catch (err) {{
    notifyHost({{
      type: "bp:solidcraft:error",
      op: "boot",
      error: err instanceof Error ? err.message : String(err),
    }});
  }}
}})();
"#
    )
}

// ---- HTTP serving & save-back ---------------------------------------------------------------------------------------------------

fn mime_for(rel: &str) -> (&'static str, bool) {
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".wasm") {
        ("application/wasm", true)
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        ("application/javascript; charset=utf-8", true)
    } else if lower.ends_with(".css") {
        ("text/css; charset=utf-8", true)
    } else if lower.ends_with(".html") {
        ("text/html; charset=utf-8", false)
    } else if lower.ends_with(".svg") {
        ("image/svg+xml", true)
    } else if lower.ends_with(".png") {
        ("image/png", true)
    } else if lower.ends_with(".ttf") {
        ("font/ttf", true)
    } else if lower.ends_with(".otf") {
        ("font/otf", true)
    } else if lower.ends_with(".woff2") {
        ("font/woff2", true)
    } else if lower.ends_with(".json") || lower.ends_with(".webmanifest") {
        ("application/json; charset=utf-8", false)
    } else if lower.ends_with(".txt") || lower.ends_with(".md") {
        ("text/plain; charset=utf-8", false)
    } else {
        ("application/octet-stream", false)
    }
}

async fn write_http_text<W: AsyncWrite + Unpin>(
    w: &mut W,
    status: &str,
    ctype: &str,
    extra: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let payload = if head_only { &[][..] } else { body };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: private, no-store\r\nConnection: close\r\n{SC_WEB_SECURITY_HEADERS}{extra}\r\n",
        body.len()
    );
    w.write_all(head.as_bytes()).await?;
    if !payload.is_empty() {
        w.write_all(payload).await?;
    }
    w.flush().await
}

/// Streams `/solidcraft-web` and `/solidcraft-web/*` directly off disk (so the 216 MB `.wasm` is never buffered in RAM).
pub async fn serve_asset<W: AsyncWrite + Unpin>(
    hub: &Arc<Hub>,
    w: &mut W,
    req: &Req,
) -> std::io::Result<()> {
    let head_only = req.method == "HEAD";
    if req.method != "GET" && !head_only {
        return write_http_text(
            w,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "",
            b"GET or HEAD only",
            false,
        )
        .await;
    }
    let route = req.route();
    if route == "/solidcraft-web" {
        let query = req
            .target
            .split_once('?')
            .map(|(_, q)| format!("?{q}"))
            .unwrap_or_default();
        let head = format!(
            "HTTP/1.1 302 Found\r\nLocation: /solidcraft-web/{query}\r\nContent-Length: 0\r\nCache-Control: private, no-store\r\nConnection: close\r\n{SC_WEB_SECURITY_HEADERS}\r\n"
        );
        w.write_all(head.as_bytes()).await?;
        return w.flush().await;
    }
    let home = hub.cfg.home.clone();
    let Some(bundle) = tokio::task::spawn_blocking(move || find(&home))
        .await
        .ok()
        .flatten()
    else {
        return write_http_text(
            w,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "",
            b"SolidCraft Web is not installed on this hub",
            head_only,
        )
        .await;
    };
    let rel = route.strip_prefix("/solidcraft-web/").unwrap_or("");
    if rel.is_empty() || rel == "index.html" {
        let html = render_index_html(&bundle);
        return write_http_text(
            w,
            "200 OK",
            "text/html; charset=utf-8",
            "",
            html.as_bytes(),
            head_only,
        )
        .await;
    }
    if rel == "bp-boot.js" {
        let js = render_boot_js(&bundle);
        return write_http_text(
            w,
            "200 OK",
            "application/javascript; charset=utf-8",
            "",
            js.as_bytes(),
            head_only,
        )
        .await;
    }
    if rel == "sw.js" {
        return write_http_text(
            w,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "",
            b"not found",
            head_only,
        )
        .await;
    }
    if !fm::files_safe(rel) || rel.split('/').any(|p| p.starts_with('.')) {
        return write_http_text(
            w,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            "",
            b"invalid asset path",
            head_only,
        )
        .await;
    }
    let dir = bundle.dir.clone();
    let rel_owned = rel.to_string();
    let found = tokio::task::spawn_blocking(move || -> Option<(PathBuf, u64, String)> {
        let real = fm::contained(&dir, &rel_owned)?;
        let root_real = crate::plat::fs::canonicalize(&dir).ok()?;
        if !real.starts_with(&root_real) {
            return None;
        }
        let lm = std::fs::symlink_metadata(&real).ok()?;
        if !lm.file_type().is_file() {
            return None;
        }
        let mtime = lm
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let etag = format!("\"sc-{}-{}-{mtime}\"", bundle.version, lm.len());
        Some((real, lm.len(), etag))
    })
    .await
    .ok()
    .flatten();
    let Some((real, len, etag)) = found else {
        return write_http_text(
            w,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "",
            b"not found",
            head_only,
        )
        .await;
    };
    let (ctype, immutable) = mime_for(rel);
    let cache = if immutable {
        "Cache-Control: private, max-age=31536000, immutable\r\n"
    } else {
        "Cache-Control: private, no-cache\r\n"
    };
    if req.header("if-none-match").trim() == etag {
        let head = format!(
            "HTTP/1.1 304 Not Modified\r\nETag: {etag}\r\n{cache}Connection: close\r\n{SC_WEB_SECURITY_HEADERS}\r\n"
        );
        w.write_all(head.as_bytes()).await?;
        return w.flush().await;
    }
    let mut file = match tokio::fs::File::open(&real).await {
        Ok(f) => f,
        Err(_) => {
            return write_http_text(
                w,
                "404 Not Found",
                "text/plain; charset=utf-8",
                "",
                b"not found",
                head_only,
            )
            .await
        }
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\nETag: {etag}\r\n{cache}Connection: close\r\n{SC_WEB_SECURITY_HEADERS}\r\n"
    );
    w.write_all(head.as_bytes()).await?;
    if !head_only && len > 0 {
        let mut buf = vec![0u8; 64 * 1024];
        let mut left = len;
        while left > 0 {
            let take = left.min(buf.len() as u64) as usize;
            let n = file.read(&mut buf[..take]).await?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).await?;
            left -= n as u64;
        }
    }
    w.flush().await
}

fn save_reply(status: &'static str, v: Value) -> HttpReply {
    HttpReply {
        status,
        ctype: "application/json".into(),
        extra: format!("Cache-Control: private, no-store\r\n{SC_WEB_SECURITY_HEADERS}"),
        body: v.to_string().into_bytes(),
    }
}

/// Saves edited CAD bytes back to `<design_root>/<path>` with `BACKPLANE_COPY`, path containment,
/// CAD extension check, and `Agents::files_write_guard`.
pub async fn handle_save(hub: &Arc<Hub>, req: &Req, body: Vec<u8>) -> HttpReply {
    let ag = crate::agents::handle();
    handle_save_with(hub, ag.as_ref(), req, body).await
}

pub(crate) async fn handle_save_with(
    hub: &Arc<Hub>,
    ag: Option<&Arc<crate::agents::Agents>>,
    req: &Req,
    body: Vec<u8>,
) -> HttpReply {
    if crate::copy::on() {
        return save_reply(
            "403 Forbidden",
            json!({"ok": false, "error": crate::copy::HOME_ONLY}),
        );
    }
    if body.is_empty() {
        return save_reply(
            "400 Bad Request",
            json!({"ok": false, "error": "empty CAD payload"}),
        );
    }
    if body.len() > ops::MAX_WRITE {
        return save_reply(
            "413 Payload Too Large",
            json!({"ok": false, "error": format!("file exceeds {} MB limit", ops::MAX_WRITE >> 20)}),
        );
    }
    let thread = fm::from_uri(&req.param("thread"));
    let rel = fm::from_uri(&req.param("path"));
    if thread.is_empty() || rel.is_empty() || !fm::files_safe(&rel) {
        return save_reply(
            "400 Bad Request",
            json!({"ok": false, "error": "that path is not in this project"}),
        );
    }
    let last = fm::last(&rel).to_string();
    let parent_rel = fm::parent(&rel);
    if !ops::plain_name(&last)
        || parent_rel.split('/').any(|p| p.starts_with('.'))
        || ops::zip_within(&rel)
        || !super::is_cad(&rel)
    {
        return save_reply(
            "400 Bad Request",
            json!({"ok": false, "error": "that file is not an editable CAD path"}),
        );
    }
    let Some(raw_root) = crate::files::root_of(hub, &thread).await else {
        return save_reply(
            "404 Not Found",
            json!({"ok": false, "error": "no such thread"}),
        );
    };
    let root = PathBuf::from(crate::files::kicad::canonical::design_root(
        &raw_root.to_string_lossy(),
    ));
    if let Some(ag) = ag {
        if let Err(why) = ag.files_write_guard(&thread, &[&rel]).await {
            return save_reply("403 Forbidden", json!({"ok": false, "error": why}));
        }
    }
    let size = body.len();
    let rel_clone = rel.clone();
    let write_res = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let dir = fm::contained(&root, &parent_rel).ok_or_else(|| "folder is not in this project".to_string())?;
        let root_real = crate::plat::fs::canonicalize(&root).map_err(|_| "project folder unavailable".to_string())?;
        if !dir.starts_with(&root_real) {
            return Err("that path is not in this project".into());
        }
        let target = dir.join(&last);
        if let Ok(lm) = std::fs::symlink_metadata(&target) {
            if !lm.file_type().is_file() {
                return Err("target is not a regular file".into());
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let tmp = dir.join(format!(".bp-sc-save-{}-{nonce}", std::process::id()));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        let write_ok = std::io::Write::write_all(&mut f, &body).and_then(|()| f.sync_all());
        drop(f);
        if let Err(e) = write_ok {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if let Err(e) = crate::plat::fs::rename(&tmp, &target) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(())
    })
    .await;
    match write_res {
        Ok(Ok(())) => save_reply("200 OK", json!({"ok": true, "path": rel_clone, "size": size})),
        Ok(Err(why)) => save_reply("403 Forbidden", json!({"ok": false, "error": why})),
        Err(_) => save_reply(
            "500 Internal Server Error",
            json!({"ok": false, "error": "cannot write file"}),
        ),
    }
}

// ---- RPC handler ----------------------------------------------------------------------------------------------------------------

pub struct WebRpc;

impl RpcHandler for WebRpc {
    fn methods(&self) -> &'static [&'static str] {
        &["solidcraft.web.status", "solidcraft.web.install"]
    }
    fn call<'a>(
        &'a self,
        hub: &'a Arc<Hub>,
        _who: &'a Caller,
        id: &'a Value,
        method: &'a str,
        p: &'a Value,
    ) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            match method {
                "solidcraft.web.status" => {
                    let home = hub.cfg.home.clone();
                    let v = tokio::task::spawn_blocking(move || status_json(&home))
                        .await
                        .unwrap_or_else(|_| json!({"installed": false}));
                    vec![json!({"t": "reply", "id": id, "ok": true, "result": v})]
                }
                "solidcraft.web.install" => {
                    let consent = p["consent"].as_bool().unwrap_or(false);
                    match install(hub, consent).await {
                        Ok(v) => vec![json!({"t": "reply", "id": id, "ok": true, "result": v})],
                        Err(why) => vec![json!({"t": "reply", "id": id, "ok": false, "error": why})],
                    }
                }
                _ => vec![json!({"t": "reply", "id": id, "ok": false, "error": "unknown method"})],
            }
        })
    }
}

pub fn start(hub: &Arc<Hub>) {
    hub.register_rpc(Arc::new(WebRpc));
    let hub = hub.clone();
    tokio::spawn(async move {
        publish(&hub).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{agents::Agents, hub::Cfg, presenter::Canned};

    fn make_bundle(dir: &Path) {
        std::fs::create_dir_all(dir.join("fonts")).unwrap();
        std::fs::write(dir.join("index.html"), "<html></html>").unwrap();
        std::fs::write(dir.join("solidcraft-web-abc123.js"), "export default async function(){}").unwrap();
        std::fs::write(dir.join("solidcraft-web-abc123_bg.wasm"), b"\x00asm\x01\x00\x00\x00").unwrap();
        std::fs::write(dir.join("loading-111.css"), "body{}").unwrap();
        std::fs::write(dir.join("loading-logo.svg"), "<svg></svg>").unwrap();
        std::fs::write(dir.join("sw.js"), "// upstream sw").unwrap();
    }

    #[test]
    fn pin_and_refusals_and_discovery() {
        check_pin(&WEB_PIN).unwrap();
        let bad = WebPin { sha256: "xyz", ..WEB_PIN.clone() };
        assert!(check_pin(&bad).is_err());
        let bad2 = WebPin { asset: "../x-0.50.0.zip", ..WEB_PIN.clone() };
        assert!(check_pin(&bad2).is_err());
        assert!(refusal(false, false, false, false).unwrap().contains("consent"));
        assert!(refusal(true, true, false, false).unwrap().contains("already being"));
        assert!(refusal(true, false, true, false).unwrap().contains("already installed"));
        assert!(refusal(true, false, false, true).unwrap().contains("copy"));
        assert_eq!(refusal(true, false, false, false), None);

        let d = crate::plat::tmp::dir("sc-web-find");
        let home = d.join("home");
        let unpack = d.join("unpack");
        assert!(find(&home).is_none());
        let root = unpack.join(WEB_PIN.root);
        std::fs::create_dir_all(&root).unwrap();
        assert!(place_web(&unpack, &web_parent(&home), &WEB_PIN).is_err());
        make_bundle(&root);
        std::fs::create_dir_all(web_parent(&home)).unwrap();
        let placed = place_web(&unpack, &web_parent(&home), &WEB_PIN).unwrap();
        assert_eq!(placed, web_parent(&home).join(WEB_PIN.version));
        let found = find(&home).unwrap();
        assert_eq!((found.source, found.version.as_str(), found.js.as_str(), found.wasm.as_str()), ("managed", "0.50.0", "solidcraft-web-abc123.js", "solidcraft-web-abc123_bg.wasm"));
        assert!(place_web(&unpack, &web_parent(&home), &WEB_PIN).is_err());

        // Symlink escape rejected
        #[cfg(unix)]
        {
            let un2 = d.join("unpack2");
            let r2 = un2.join(WEB_PIN.root);
            make_bundle(&r2);
            std::os::unix::fs::symlink("/etc/passwd", r2.join("leak")).unwrap();
            assert!(place_web(&un2, &d.join("m2"), &WEB_PIN).is_err());
        }
    }

    #[test]
    fn self_hosted_html_and_boot_have_zero_external_calls() {
        let b = WebBundle {
            dir: PathBuf::from("/tmp/none"),
            version: "0.50.0".into(),
            source: "managed",
            js: "solidcraft-web-abc123.js".into(),
            wasm: "solidcraft-web-abc123_bg.wasm".into(),
            css: "loading-111.css".into(),
        };
        let html = render_index_html(&b);
        let boot = render_boot_js(&b);
        assert!(!html.contains("solidcraft.io") && !html.contains("sw.js"));
        assert!(!boot.contains("solidcraft.io") && !boot.contains("sw.js"));
        assert!(html.contains("./solidcraft-web-abc123.js") && html.contains("./bp-boot.js"));
        assert!(boot.contains("ask_import_options: false") && boot.contains("usage_statistics: false"));
        assert!(SC_WEB_SECURITY_HEADERS.contains("'wasm-unsafe-eval'"));
        assert!(SC_WEB_SECURITY_HEADERS.contains("connect-src 'self' blob: data:"));
    }

    #[tokio::test]
    async fn serve_asset_streams_wasm_with_mime_and_blocks_traversal_and_sw() {
        let d = crate::plat::tmp::dir("sc-web-serve");
        let home = d.join("home");
        let bundle_dir = web_parent(&home).join("0.50.0");
        make_bundle(&bundle_dir);
        let hub = Hub::start(
            Cfg { home: home.clone(), version: "0.1".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() },
            Arc::new(Canned { reply: json!({}) }),
        ).await.unwrap();

        let ask = |target: String, hdrs: Vec<(String, String)>| {
            let hub = hub.clone();
            async move {
                let (mut client, mut server) = tokio::io::duplex(64 * 1024);
                let req = Req::new("GET", &target, hdrs);
                let srv = tokio::spawn(async move { serve_asset(&hub, &mut server, &req).await });
                let mut resp = Vec::new();
                client.read_to_end(&mut resp).await.unwrap();
                srv.await.unwrap().unwrap();
                resp
            }
        };

        let r302 = String::from_utf8_lossy(&ask("/solidcraft-web?thread=t1".into(), vec![]).await).into_owned();
        assert!(r302.starts_with("HTTP/1.1 302 Found") && r302.contains("Location: /solidcraft-web/?thread=t1"));

        let index = String::from_utf8_lossy(&ask("/solidcraft-web/?thread=t1".into(), vec![]).await).into_owned();
        assert!(index.starts_with("HTTP/1.1 200 OK") && index.contains("solidcraft-web-abc123.js") && index.contains("bp-boot.js") && !index.contains("solidcraft.io"));
        let boot = String::from_utf8_lossy(&ask("/solidcraft-web/bp-boot.js".into(), vec![]).await).into_owned();
        assert!(boot.starts_with("HTTP/1.1 200 OK") && boot.contains("solidcraft-web-abc123_bg.wasm") && boot.contains("waitForExportedBytes"));

        let wasm_raw = ask("/solidcraft-web/solidcraft-web-abc123_bg.wasm".into(), vec![]).await;
        let split = wasm_raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let wasm_head = String::from_utf8_lossy(&wasm_raw[..split]);
        assert!(wasm_head.starts_with("HTTP/1.1 200 OK"), "{wasm_head}");
        assert!(wasm_head.contains("Content-Type: application/wasm"), "{wasm_head}");
        assert_eq!(&wasm_raw[split + 4..], b"\x00asm\x01\x00\x00\x00");

        let etag = wasm_head.lines().find_map(|l| l.strip_prefix("ETag: ")).unwrap().trim().to_string();
        let r304 = String::from_utf8_lossy(&ask("/solidcraft-web/solidcraft-web-abc123_bg.wasm".into(), vec![("If-None-Match".into(), etag)]).await).into_owned();
        assert!(r304.starts_with("HTTP/1.1 304 Not Modified"), "{r304}");

        assert!(String::from_utf8_lossy(&ask("/solidcraft-web/sw.js".into(), vec![]).await).starts_with("HTTP/1.1 404"));
        assert!(String::from_utf8_lossy(&ask("/solidcraft-web/../index.html".into(), vec![]).await).starts_with("HTTP/1.1 400"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn handle_save_writes_atomically_and_obeys_write_guard() {
        let d = crate::plat::tmp::dir("sc-web-save");
        let (home, proj) = (d.join("home"), d.join("proj"));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(proj.join("mech")).unwrap();
        std::fs::write(proj.join("mech/part.sc"), b"old").unwrap();
        let hub = Hub::start(
            Cfg { home: home.clone(), version: "0.1".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() },
            Arc::new(Canned { reply: json!({}) }),
        ).await.unwrap();
        let ag = Agents::start(hub.clone()).await.unwrap();
        hub.commit_many(&[
            json!({"$": "ProjectCreated", "id": "p1", "title": "P", "root": proj, "at": 1}),
            json!({"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "T", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": 2}),
        ]).await.unwrap();

        let req = Req::new("POST", "/solidcraft-web/save?thread=t1&path=mech%2Fpart.sc", vec![]);
        let ok_rep = handle_save_with(&hub, Some(&ag), &req, b"new-sc-bytes".to_vec()).await;
        assert_eq!(ok_rep.status, "200 OK");
        assert_eq!(std::fs::read(proj.join("mech/part.sc")).unwrap(), b"new-sc-bytes");

        // Non-CAD extension & traversal refused
        let bad_ext = handle_save_with(&hub, Some(&ag), &Req::new("POST", "/solidcraft-web/save?thread=t1&path=run.sh", vec![]), b"echo".to_vec()).await;
        assert_eq!(bad_ext.status, "400 Bad Request");
        let bad_trav = handle_save_with(&hub, Some(&ag), &Req::new("POST", "/solidcraft-web/save?thread=t1&path=..%2Fescape.sc", vec![]), b"x".to_vec()).await;
        assert_eq!(bad_trav.status, "400 Bad Request");

        // Review mode blocks design write via files_write_guard
        ag.review_set("t1", true).await.unwrap();
        let rev_rep = handle_save_with(&hub, Some(&ag), &req, b"blocked".to_vec()).await;
        assert_eq!(rev_rep.status, "403 Forbidden");
        assert_eq!(std::fs::read(proj.join("mech/part.sc")).unwrap(), b"new-sc-bytes");
    }
}
