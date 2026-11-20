//! Files of the person's projects for every client, local or remote (docs/file-links.md).
//!
//! - `check`: the one access rule. A path (absolute or `~/`) is served only when its real file (every link followed) is a regular file under a live
//!   project's folder, a live thread's worktree or another allowed root (`<home>/attachments`, `<home>/far-files`, the setting `files.share.roots`),
//!   and nothing on its way from that root is hidden (a dot name other than `.backplane` and `.github`) or names a secret (`.env`, keys, credentials,
//!   a vault). Nothing under a Backplane home is served except those two folders of this hub's own.
//! - `/file?path=<p>` (credential: the hub token or a paired device's, as every route): the file, streamed straight off the disk, with one
//!   `bytes=a-b` range, `ETag` and `If-Range`, so a client resumes a big download where it stopped. `HEAD` answers the headers only.
//!   `thread=<link>~<id>`: a thread of another of the owner's machines; the file is read there in pieces over the link (`file.piece`) and streamed on.
//! - `/s/<token>/<name>`: a share an agent made with the MCP tool `file_share`: the token is its own credential, it expires, and Settings lists and
//!   revokes it (`shares.list`, `shares.revoke`). Only the main listener answers a share by its token alone; the web ingress answers it to a
//!   paired browser only. A share with `tailnet` also gets the hub's own tailnet address (its existing `tailscale serve` https address, or its
//!   MagicDNS name when it listens on the tailnet); the hub makes no serve mapping for it.
//! - `files` on an assistant entry (`linkable`): the files its text names that `check` allows, so every client draws them as links.
use crate::{api::{Caller, Fut, RpcHandler, Ticker}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::{Arc, LazyLock, RwLock}};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

/// How much of a file is read and written at a time.
const BUF: usize = 256 << 10;
/// The largest piece one `file.piece` carries between machines (base64 in JSON).
pub const FAR_PIECE: u64 = 4 << 20;
/// The longest a share lives, and its default.
pub const MAX_LIFE_S: u64 = 30 * 86400;
pub const DEFAULT_LIFE_S: u64 = 86400;

// ---- the access rule ----

/// The folders files are served from, real paths. Kept fresh by `Fresh` (every project and thread change); read by the presenter, which is synchronous.
/// By hub home (one hub a process in use; tests run several).
/// One snapshot each: every root, and the browser's (made together, from the same model, so the two always agree).
#[derive(Clone, Default)]
struct Snap { all: Vec<PathBuf>, browser: Vec<PathBuf> }
static ROOTS: LazyLock<RwLock<std::collections::HashMap<PathBuf, Snap>>> = LazyLock::new(Default::default);
/// The roots of this process's hub (the presenter's).
pub fn roots() -> Vec<PathBuf> { roots_at(&HOME.read().unwrap().clone()) }
pub fn roots_at(home: &Path) -> Vec<PathBuf> { ROOTS.read().unwrap().get(home).map(|s| s.all.clone()).unwrap_or_default() }
fn known(home: &Path) -> bool { ROOTS.read().unwrap().contains_key(home) }

/// The live projects' folders and the live threads' worktrees, then the hub's own allowed folders and the setting's, each its real path.
pub fn roots_of(m: &crate::model::Model, home: &Path, extra: &str) -> Vec<PathBuf> {
    let mut v: Vec<String> = m.projects.values().filter(|p| !p.removed && !p.root.is_empty()).map(|p| p.root.clone()).collect();
    v.extend(m.threads.values().filter(|t| !t.deleted && !t.worktree.is_empty()).map(|t| t.worktree.clone()));
    v.extend([home.join("attachments"), home.join("far-files")].iter().map(|p| p.to_string_lossy().into_owned()));
    v.extend(extra.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string));
    let mut out: Vec<PathBuf> = vec![];
    for r in v { if let Ok(c) = crate::plat::fs::canonicalize(Path::new(&r)) { if c.is_dir() && !out.contains(&c) && !too_wide(&c) { out.push(c) } } }
    out
}
/// A paired browser's roots: live projects, live worktrees and the hub's attachments; never the far copies or the setting's extra roots.
pub fn browser_roots_of(m: &crate::model::Model, home: &Path) -> Vec<PathBuf> {
    let mut v = roots_of(m, home, "");
    let far = crate::plat::fs::canonicalize(home.join("far-files")).ok();
    v.retain(|r| far.as_ref().map_or(true, |f| !r.starts_with(f)));
    v
}
/// A root that would open everything: the filesystem's root or the user's home itself.
fn too_wide(p: &Path) -> bool {
    p.parent().is_none() || crate::plat::fs::canonicalize(Path::new(&crate::plat::dirs::home_or_empty())).map_or(false, |h| h == p)
}

/// A name that is a secret wherever it is.
pub fn secret_name(n: &str) -> bool {
    let l = n.to_ascii_lowercase();
    let exact = [".env", ".netrc", ".npmrc", ".pypirc", ".git-credentials", ".pgpass", "token", "auth.json", "credentials", "settings.local.json", "devices-v1.json", "shares-v1.json"];
    let starts = [".env.", "id_rsa", "id_ecdsa", "id_ed25519", "id_dsa", "credentials.", "secrets", "secret.", "vault"];
    let ends = [".pem", ".key", ".p12", ".pfx", ".keystore", ".jks", ".kdbx", ".ovpn", ".gpg", ".asc", ".secret", ".secrets", ".token"];
    exact.contains(&l.as_str()) || starts.iter().any(|s| l.starts_with(s)) || ends.iter().any(|s| l.ends_with(s))
}
/// A folder on the way that is never opened: hidden ones but the two a project shows, and Backplane's homes.
fn closed_dir(n: &str) -> bool {
    let l = n.to_ascii_lowercase();
    (l.starts_with('.') && l != ".backplane" && l != ".github") || l == "secrets" || l == "vault" || l.starts_with(".backplane-") || l == "node_modules"
}

/// The request's path made into a path on this machine: `file://` dropped, `~/` expanded; None for anything relative, climbing or with a NUL.
pub fn local_path(p: &str) -> Option<PathBuf> {
    let p = p.trim(); let p = p.strip_prefix("file://").unwrap_or(p);
    if p.contains('\0') || !crate::files::mimg::no_climb(p) { return None }
    let full = match p.strip_prefix("~/") { Some(r) => PathBuf::from(crate::plat::dirs::home_or_empty()).join(r), None => PathBuf::from(p) };
    if full.is_absolute() { Some(full) } else { None }
}

/// The rule: the real file and its size, or why not (the words a client shows). `home` is this hub's home.
pub fn check(roots: &[PathBuf], home: &Path, p: &str) -> Result<(PathBuf, std::fs::Metadata), &'static str> {
    let full = local_path(p).ok_or("not a path on this machine")?;
    let real = crate::plat::fs::canonicalize(&full).map_err(|_| "no such file")?;
    let md = std::fs::metadata(&real).map_err(|_| "no such file")?;
    if !md.is_file() { return Err("not a file") }
    rule(roots, home, &real)?;
    Ok((real, md))
}
/// A file opened once and checked as opened: the rule is applied to the path the HANDLE has (the OS's own word for the object opened, whatever a
/// link on the way did between the look and the open), its metadata are the handle's, and it is that handle that is read and streamed.
pub struct Opened { pub file: std::fs::File, pub real: PathBuf, pub md: std::fs::Metadata }
pub fn open_checked(roots: &[PathBuf], home: &Path, p: &str) -> Result<Opened, &'static str> {
    let full = local_path(p).ok_or("not a path on this machine")?;
    let real = crate::plat::fs::canonicalize(&full).map_err(|_| "no such file")?;
    rule(roots, home, &real)?; // refused before anything is opened
    open_real(roots, home, &real)
}
/// `real` already canonical: opened without following a last link and without blocking on a FIFO, then checked by its handle.
pub fn open_real(roots: &[PathBuf], home: &Path, real: &Path) -> Result<Opened, &'static str> {
    use crate::plat::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new().read(true).custom_flags(crate::plat::fs::O_NOFOLLOW | crate::plat::fs::O_NONBLOCK).open(real).map_err(|_| "no such file")?;
    let md = file.metadata().map_err(|_| "no such file")?;
    if !md.is_file() { return Err("not a file") }
    let real = crate::plat::fs::path_of(&file).map_err(|_| "no such file")?;
    rule(roots, home, &real)?;
    Ok(Opened { file, real, md })
}
/// The rule on a real path (every link already resolved).
fn rule(roots: &[PathBuf], home: &Path, real: &Path) -> Result<(), &'static str> {
    let real = real.to_path_buf();
    let home = crate::plat::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    let own = [home.join("attachments"), home.join("far-files")];
    if real.starts_with(&home) && !own.iter().any(|o| real.starts_with(o)) { return Err("that file is not shared") }
    // the deepest root that holds it decides what is "on the way"
    let Some(root) = roots.iter().filter(|r| real.starts_with(r)).max_by_key(|r| r.components().count()) else { return Err("that file is not in a project") };
    let rel = real.strip_prefix(root).map_err(|_| "that file is not in a project")?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let Some((name, dirs)) = parts.split_last() else { return Err("not a file") };
    if dirs.iter().any(|d| closed_dir(d)) || secret_name(name) { return Err("that file is not shared") }
    // a dotfile at any depth (`.git`, `.ssh` contents are already refused as a folder on the way)
    if name.starts_with('.') && !matches!(name.as_str(), ".backplane.json" | ".gitignore") { return Err("that file is not shared") }
    Ok(())
}

/// What a client draws for a file an assistant message names: `{path, name, size, open}`; `open` is "view" when Backplane's viewers draw it
/// (a board, schematic, 3D model, image, PDF or text note) and "download" otherwise. In the message's spelling.
pub fn linkable(text: &str) -> Vec<Value> {
    // the presenter draws often: what a text names is looked at again at most every LOOK
    use std::{collections::HashMap, hash::{Hash, Hasher}, sync::Mutex, time::{Duration, Instant}};
    const LOOK: Duration = Duration::from_secs(3);
    static SEEN: LazyLock<Mutex<HashMap<u64, (Instant, Vec<Value>)>>> = LazyLock::new(Default::default);
    if !text.contains('/') { return vec![] }
    let key = { let mut h = std::collections::hash_map::DefaultHasher::new(); text.hash(&mut h); h.finish() };
    if let Some((at, v)) = SEEN.lock().unwrap().get(&key) { if at.elapsed() < LOOK { return v.clone() } }
    let v = linkable_now(text);
    let mut g = SEEN.lock().unwrap();
    if g.len() > 2048 { g.retain(|_, (at, _)| at.elapsed() < LOOK) }
    g.insert(key, (Instant::now(), v.clone()));
    v
}
fn linkable_now(text: &str) -> Vec<Value> {
    let roots = roots(); if roots.is_empty() { return vec![] }
    let home = HOME.read().unwrap().clone();
    let mut out = vec![];
    for p in candidates(text) {
        if out.len() >= 32 { break }
        if let Ok((_, md)) = check(&roots, &home, &p) {
            let name = p.rsplit(['/', '\\']).next().unwrap_or(&p).to_string();
            let open = if crate::files::fm::preview(&name).is_empty() { "download" } else { "view" };
            out.push(json!({"path": p, "name": name, "size": md.len(), "open": open}));
        }
    }
    out
}
static HOME: LazyLock<RwLock<PathBuf>> = LazyLock::new(Default::default);

/// Every absolute or `~/` path in a text (code spans and blocks included: an agent writes paths in backticks), each once, at most 1024 chars, with
/// what a sentence puts after it (`.`, `:`, `,`, `)`) dropped.
pub fn candidates(text: &str) -> Vec<String> {
    let sep = |c: char| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '`' | '"' | '\'' | ',' | ';' | '|' | '*');
    let mut out: Vec<String> = vec![];
    for w in text.split(sep) {
        let w = w.strip_prefix("file://").unwrap_or(w);
        let w = w.trim_end_matches(['.', ':', '!', '?']);
        let w = w.split_once(":L").map_or(w, |(a, b)| if b.chars().next().map_or(false, |c| c.is_ascii_digit()) { a } else { w });
        // a `path:line` reference
        let w = match w.rsplit_once(':') { Some((a, b)) if !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()) => a, _ => w };
        if w.len() < 2 || w.len() > 1024 || !(crate::plat::fs::is_abs(w) || w.starts_with("~/")) || w.ends_with('/') { continue }
        if !out.iter().any(|x| x == w) { out.push(w.to_string()) }
        if out.len() >= 64 { break }
    }
    out
}

/// Keeps `ROOTS` fresh: at start, and after any batch that adds, moves or removes a project or a thread's worktree.
struct Fresh;
impl crate::api::Observer for Fresh {
    fn committed(&self, hub: &Arc<Hub>, events: &[Value], _first: u32) {
        if events.iter().any(|e| e["$"].as_str().map_or(false, |k| k.starts_with("Project") || k.starts_with("Thread") || k == "SettingSet" || k == "WorktreeSet")) { refresh_soon(hub) }
    }
}
fn refresh_soon(hub: &Arc<Hub>) {
    let h = hub.clone();
    tokio::spawn(async move { refresh(&h).await });
}
pub async fn refresh(hub: &Arc<Hub>) {
    let extra = hub.setting("files.share.roots").await.unwrap_or_default();
    let (all, browser) = { let m = hub.model().await; (roots_of(&m, &hub.cfg.home, &extra), browser_roots_of(&m, &hub.cfg.home)) };
    ROOTS.write().unwrap().insert(hub.cfg.home.clone(), Snap { all, browser });
}

// ---- streaming ----

/// Who a file is answered to: the headers and the folders depend on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Who {
    /// a native client (the desktop, a phone) with the hub token or a paired device's
    Native,
    /// a paired browser on the web ingress: the app's own origin, so a file is never inline unless it is a picture, a PDF or plain text,
    /// never stored, and only under projects, worktrees and the hub's attachments
    Browser,
    /// a share's token on the main listener: no other credential, never stored
    Share,
}
/// The folders `Who` reaches: a browser not the far copies or the setting's extra roots.
fn roots_for(who: Who, home: &Path) -> Vec<PathBuf> {
    let g = ROOTS.read().unwrap(); let s = g.get(home).cloned().unwrap_or_default();
    if who == Who::Browser { s.browser } else { s.all }
}

/// Content-Type by name: what a browser or phone opens it with. Anything unknown is bytes.
pub fn ctype(name: &str) -> &'static str {
    let l = name.to_ascii_lowercase(); let e = l.rsplit('.').next().unwrap_or("");
    match e {
        "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "webp" => "image/webp", "bmp" => "image/bmp",
        "pdf" => "application/pdf", "zip" => "application/zip", "csv" => "text/csv; charset=utf-8", "json" => "application/json",
        "txt" | "md" | "log" | "yaml" | "yml" | "gbr" | "drl" | "xln" | "gbrjob" => "text/plain; charset=utf-8",
        "step" | "stp" => "model/step", "glb" => "model/gltf-binary", "stl" => "model/stl",
        _ => "application/octet-stream",
    }
}
fn disposition(name: &str, inline: bool) -> String {
    let ascii: String = name.chars().map(|c| if c.is_ascii_graphic() && !matches!(c, '"' | '\\' | ';') || c == ' ' { c } else { '_' }).collect();
    let pct: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    format!("Content-Disposition: {}; filename=\"{ascii}\"; filename*=UTF-8''{pct}\r\n", if inline { "inline" } else { "attachment" })
}
pub fn etag(md: &std::fs::Metadata) -> String {
    let ns = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    format!("\"{:x}-{:x}\"", md.len(), ns)
}
/// What one request asks for: (status, first, last) or 416; a range a stale `If-Range` names is the whole file.
pub fn plan(range: &str, if_range: &str, tag: &str, size: u64) -> Result<(&'static str, u64, u64), ()> {
    let range = if !if_range.is_empty() && if_range.trim() != tag { "" } else { range };
    match super::serve::range(range, size)? {
        Some((a, b)) => Ok(("206 Partial Content", a, b)),
        None => Ok(("200 OK", 0, size.saturating_sub(1))),
    }
}
/// The headers of every file answer. Never cached by anything but the asking client (a browser's and a share's not at all, so a revoked browser
/// or an ended share is asked again and refused); sandboxed (no script runs with the origin's cookie even if a browser renders it); no Referer.
pub fn head_bytes(who: Who, status: &str, ctype: &str, len: u64, extra: &str) -> Vec<u8> {
    let cache = if who == Who::Native { "private, no-cache" } else { "private, no-store" };
    format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\nCache-Control: {cache}\r\nAccept-Ranges: bytes\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\nCross-Origin-Resource-Policy: same-origin\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n{extra}\r\n").into_bytes()
}
/// The types a file may be shown inline as (`inline=1`): pictures, PDF and plain text only; never HTML, SVG or anything a browser runs.
pub fn inline_ok(ctype: &str) -> bool { matches!(ctype, "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp" | "application/pdf" | "text/plain; charset=utf-8") }
pub fn refusal(status: &str, why: &str) -> Vec<u8> {
    let mut v = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: private, no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; sandbox\r\nCross-Origin-Resource-Policy: same-origin\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n", why.len()).into_bytes();
    v.extend_from_slice(why.as_bytes()); v
}
/// The status a refusal of `check` answers with.
fn status_of(why: &str) -> &'static str { if why == "no such file" || why == "not a file" { "404 Not Found" } else { "403 Forbidden" } }

/// Writes the opened file (or the range asked) to `w`: the handle `open_checked` allowed, never the path again.
pub async fn stream<W: AsyncWrite + Unpin>(w: &mut W, who: Who, req: &Req, o: Opened, name: &str) -> std::io::Result<()> {
    let Opened { file, md, .. } = o;
    let tag = etag(&md); let size = md.len();
    let Ok((status, a, b)) = plan(req.header("range"), req.header("if-range"), &tag, size) else {
        return w.write_all(&refusal("416 Range Not Satisfiable", "range not satisfiable")).await;
    };
    let len = if size == 0 { 0 } else { b - a + 1 };
    let mut extra = format!("ETag: {tag}\r\n");
    if status.starts_with("206") { extra.push_str(&format!("Content-Range: bytes {a}-{b}/{size}\r\n")) }
    let inline = req.param("inline") == "1" && inline_ok(ctype(name));
    extra.push_str(&disposition(name, inline));
    let mut f = tokio::fs::File::from_std(file);
    w.write_all(&head_bytes(who, status, ctype(name), len, &extra)).await?;
    if req.method == "HEAD" || len == 0 { return w.flush().await }
    f.seek(std::io::SeekFrom::Start(a)).await?;
    let mut left = len; let mut buf = vec![0u8; BUF];
    while left > 0 {
        let n = f.read(&mut buf[..(left.min(BUF as u64) as usize)]).await?;
        if n == 0 { break } // the file shrank: the client sees a short body and asks again from where it got
        w.write_all(&buf[..n]).await?; left -= n as u64;
    }
    w.flush().await
}

/// `/file`: the credential was checked by the caller. A thread elsewhere is read there.
/// `thread` (a thread of this machine) narrows it: the file must be inside that thread's folder.
pub async fn serve_file<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, who: Who, req: &Req) -> std::io::Result<()> {
    if req.method != "GET" && req.method != "HEAD" { return w.write_all(&refusal("405 Method Not Allowed", "GET only")).await }
    let path = crate::files::fm::from_uri(&req.param("path"));
    let thread = crate::files::fm::from_uri(&req.param("thread"));
    if thread.contains('~') { return far_file(hub, w, who, req, &thread, &path).await }
    let within = if thread.is_empty() { None } else { match crate::files::root_of(hub, &thread).await { Some(r) => Some(r), None => return w.write_all(&refusal("404 Not Found", "no such thread")).await } };
    if !known(&hub.cfg.home) { refresh(hub).await }
    let (home, roots) = (hub.cfg.home.clone(), roots_for(who, &hub.cfg.home));
    let p2 = path.clone();
    match tokio::task::spawn_blocking(move || open_checked(&roots, &home, &p2).and_then(|o| match within {
        Some(r) if !crate::plat::fs::canonicalize(&r).map_or(false, |r| o.real.starts_with(r)) => Err("that file is not in this thread's project"),
        _ => Ok(o) })).await {
        Ok(Ok(o)) => { let name = name_of(&path); stream(w, who, req, o, &name).await }
        Ok(Err(why)) => w.write_all(&refusal(status_of(why), why)).await,
        Err(_) => w.write_all(&refusal("500 Internal Server Error", "cannot read file")).await,
    }
}
/// `/files/get?thread=&path=` (the Files tab's Download): a file of the thread's folder by its path from there, under the Files tab's own
/// containment (`fm::contained`), streamed and ranged like `/file`.
pub async fn serve_files_get<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, who: Who, req: &Req) -> std::io::Result<()> {
    let (thread, rel) = (crate::files::fm::from_uri(&req.param("thread")), crate::files::fm::from_uri(&req.param("path")));
    if rel.is_empty() || !crate::files::fm::files_safe(&rel) { return w.write_all(&refusal("400 Bad Request", "that path is not in this project")).await }
    if thread.contains('~') { return far_file(hub, w, who, req, &thread, &rel).await }
    let Some(root) = crate::files::root_of(hub, &thread).await else { return w.write_all(&refusal("404 Not Found", "no such thread")).await };
    if !known(&hub.cfg.home) { refresh(hub).await }
    let (home, roots, r2) = (hub.cfg.home.clone(), roots_for(who, &hub.cfg.home), rel.clone());
    // the Files tab's containment, then the same rule as /file (secrets, hidden folders, the caller's roots) on the handle opened
    let found = tokio::task::spawn_blocking(move || -> Result<Opened, &'static str> {
        let real = crate::files::fm::contained(&root, &r2).ok_or("no such file")?;
        let o = open_real(&roots, &home, &real)?;
        let root = crate::plat::fs::canonicalize(&root).map_err(|_| "no such file")?;
        if !o.real.starts_with(&root) { return Err("that file is not in this thread's project") }
        Ok(o)
    }).await;
    match found {
        Ok(Ok(o)) => stream(w, who, req, o, &name_of(&rel)).await,
        Ok(Err(why)) => w.write_all(&refusal(status_of(why), why)).await,
        Err(_) => w.write_all(&refusal("500 Internal Server Error", "cannot read file")).await,
    }
}
fn name_of(p: &str) -> String { p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("file").to_string() }

// ---- a file of another machine's thread ----

/// The owning side of `file.piece {thread, path, at, len}` → `{b64, size, etag}`: the path must pass `check` and be inside that thread's folder.
struct Piece;
impl RpcHandler for Piece {
    fn methods(&self) -> &'static [&'static str] { &["file.piece"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let fail = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            let th = p["thread"].as_str().unwrap_or("");
            let Some(root) = crate::files::root_of(hub, th).await else { return fail("This thread has no project folder.") };
            if !known(&hub.cfg.home) { refresh(hub).await }
            let asked = p["path"].as_str().unwrap_or("").to_string();
            // a path from the thread's folder (the Files tab's) or an absolute one (a message's)
            let path = if crate::plat::fs::is_abs(&asked) || asked.starts_with("~/") { asked } else if !asked.is_empty() && crate::files::fm::files_safe(&asked) { root.join(&asked).to_string_lossy().into_owned() } else { return fail("not a file of the project") };
            let (home, roots) = (hub.cfg.home.clone(), roots_at(&hub.cfg.home));
            let (at, len) = (p["at"].as_u64().unwrap_or(0), p["len"].as_u64().unwrap_or(FAR_PIECE).min(FAR_PIECE));
            let r = tokio::task::spawn_blocking(move || -> Result<Value, &'static str> {
                use base64::Engine; use std::io::{Read, Seek};
                let Opened { file: mut f, real, md } = open_checked(&roots, &home, &path)?;
                let root = crate::plat::fs::canonicalize(&root).map_err(|_| "no such folder")?;
                if !real.starts_with(&root) { return Err("that file is not in this thread's project") }
                let n = len.min(md.len().saturating_sub(at));
                let mut b = vec![0u8; n as usize];
                f.seek(std::io::SeekFrom::Start(at)).map_err(|_| "cannot read it")?; f.read_exact(&mut b).map_err(|_| "the file changed; ask again")?;
                Ok(json!({"b64": base64::engine::general_purpose::STANDARD.encode(b), "size": md.len(), "etag": etag(&md)}))
            }).await;
            match r { Ok(Ok(v)) => vec![json!({"t":"reply","id":id,"ok":true,"result":v})], Ok(Err(e)) => fail(e), Err(_) => fail("cannot read it") }
        })
    }
}

async fn far_piece(hub: &Arc<Hub>, link: &str, thread: &str, path: &str, at: u64, len: u64) -> Result<(Vec<u8>, u64, String), String> {
    use base64::Engine;
    let body = json!({"id": 0, "m": "file.piece", "p": {"thread": thread, "path": path, "at": at, "len": len}}).to_string();
    let (code, text) = crate::net::farfiles::ask(hub, link, &body).await.ok_or("This machine is not linked to others.")?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if code == 0 { return Err("That machine is away.".into()) }
    if code != 200 || v["ok"] != true { return Err(v["error"].as_str().unwrap_or("That machine did not answer.").to_string()) }
    let b = base64::engine::general_purpose::STANDARD.decode(v["result"]["b64"].as_str().unwrap_or("")).map_err(|_| "the file came back unreadable".to_string())?;
    Ok((b, v["result"]["size"].as_u64().unwrap_or(0), v["result"]["etag"].as_str().unwrap_or("").to_string()))
}
/// `/file?thread=<link>~<id>&path=`: the first piece says the size and tag; the range asked is then fetched piece by piece and written as it comes.
async fn far_file<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, who: Who, req: &Req, thread: &str, path: &str) -> std::io::Result<()> {
    let Some((link, local)) = thread.split_once('~').filter(|(l, t)| !l.is_empty() && !t.is_empty()) else { return w.write_all(&refusal("404 Not Found", "no such thread")).await };
    let first = match far_piece(hub, link, local, path, 0, 1).await { Ok(x) => x, Err(e) => return w.write_all(&refusal("502 Bad Gateway", &e)).await };
    let (size, tag) = (first.1, first.2);
    let Ok((status, a, b)) = plan(req.header("range"), req.header("if-range"), &tag, size) else { return w.write_all(&refusal("416 Range Not Satisfiable", "range not satisfiable")).await };
    let len = if size == 0 { 0 } else { b - a + 1 };
    let name = name_of(path);
    let mut extra = format!("ETag: {tag}\r\n");
    if status.starts_with("206") { extra.push_str(&format!("Content-Range: bytes {a}-{b}/{size}\r\n")) }
    extra.push_str(&disposition(&name, false));
    w.write_all(&head_bytes(who, status, ctype(&name), len, &extra)).await?;
    if req.method == "HEAD" { return w.flush().await }
    let mut at = a;
    while at < a + len {
        let n = FAR_PIECE.min(a + len - at);
        match far_piece(hub, link, local, path, at, n).await {
            Ok((bytes, _, t)) if t == tag && !bytes.is_empty() => { w.write_all(&bytes).await?; at += bytes.len() as u64 }
            _ => break, // changed or gone: a short body; the client's next range (with If-Range) starts again
        }
    }
    w.flush().await
}

// ---- shares ----

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Share { pub id: String, pub token: String, pub path: String, pub name: String, pub size: u64, pub thread: String,
    pub created: u64, pub expires: u64, #[serde(default)] pub tailnet: bool, #[serde(default)] pub url: String }

fn store_path(home: &Path) -> PathBuf { home.join("shares-v1.json") }
/// Every change to the store goes through this (sync: the presenter's Revoke is synchronous; held only for a read and a write of one small file).
static STORE: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Load, change, save under `STORE`; Ok only once the change is on disk.
fn mutate<T>(home: &Path, f: impl FnOnce(&mut Vec<Share>) -> T) -> Result<T, String> {
    let _g = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load(home); let before = all.clone();
    let out = f(&mut all);
    if all != before { save(home, &all).map_err(|e| format!("could not save the shares: {e}"))? }
    Ok(out)
}
pub fn load(home: &Path) -> Vec<Share> {
    std::fs::read_to_string(store_path(home)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value::<Vec<Share>>(v["shares"].clone()).ok()).unwrap_or_default()
}
/// The temp file is made 0600 from its first byte (never readable by others, even for a moment), written, synced, then renamed over the store.
fn save(home: &Path, v: &[Share]) -> std::io::Result<()> {
    use std::io::Write;
    let p = store_path(home); let tmp = p.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut o = std::fs::OpenOptions::new(); o.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    let mut f = o.open(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(&json!({"shares": v}))?)?; f.sync_all()?; drop(f);
    std::fs::rename(&tmp, &p)?;
    let _ = crate::plat::fs::sync_dir(home);
    Ok(())
}
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
fn random_hex(n: usize) -> String { let mut b = vec![0u8; n]; crate::plat::fs::random(&mut b).expect("/dev/urandom"); b.iter().map(|x| format!("{x:02x}")).collect() }

/// `expires`: seconds, or "30m", "12h", "7d"; none is a day; at most 30 days.
pub fn life(v: &Value) -> Result<u64, String> {
    let s = match v { Value::Null => return Ok(DEFAULT_LIFE_S), Value::Number(n) => return n.as_u64().filter(|n| *n > 0).map(|n| n.min(MAX_LIFE_S)).ok_or("expires must be a positive number of seconds".into()), Value::String(s) => s.trim().to_ascii_lowercase(), _ => return Err("expires must be seconds or like 30m, 12h, 7d".into()) };
    if s.is_empty() { return Ok(DEFAULT_LIFE_S) }
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: u64 = num.parse().map_err(|_| "expires must be seconds or like 30m, 12h, 7d".to_string())?;
    let mul = match unit.trim() { "" | "s" => 1, "m" | "min" => 60, "h" => 3600, "d" => 86400, _ => return Err("expires must be seconds or like 30m, 12h, 7d".into()) };
    if n == 0 { return Err("expires must be more than zero".into()) }
    Ok((n * mul).min(MAX_LIFE_S))
}
/// The address part of a share: `/s/<token>/<name>` (its name percent-encoded).
pub fn share_route(token: &str, name: &str) -> String {
    let n: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    format!("/s/{token}/{n}")
}

/// The MCP tool `file_share {path, expires?, tailnet?}`.
pub async fn create(hub: &Arc<Hub>, thread: &str, a: &Value) -> Result<Value, String> {
    let path = a["path"].as_str().unwrap_or("").to_string();
    let life = life(&a["expires"])?;
    if !known(&hub.cfg.home) { refresh(hub).await }
    let (home, roots, p2) = (hub.cfg.home.clone(), roots_at(&hub.cfg.home), path.clone());
    let (real, md) = tokio::task::spawn_blocking(move || check(&roots, &home, &p2)).await.map_err(|_| "failed".to_string())?.map_err(str::to_string)?;
    let name = name_of(&path);
    let token = random_hex(24);
    let now = now_s();
    let mut sh = Share { id: format!("sh{}", &random_hex(6)), token: token.clone(), path: real.to_string_lossy().into_owned(), name: name.clone(), size: md.len(), thread: thread.into(), created: now, expires: now + life, tailnet: a["tailnet"].as_bool().unwrap_or(false), url: String::new() };
    let route = share_route(&token, &name);
    let mut note = String::new();
    if sh.tailnet {
        match tailnet_base(hub).await { Ok(base) => sh.url = format!("{base}{route}"), Err(e) => note = e }
    }
    let keep = sh.clone();
    let home = hub.cfg.home.clone();
    tokio::task::spawn_blocking(move || mutate(&home, |all| { all.retain(|s| s.expires > now); all.push(keep) })).await.map_err(|_| "failed".to_string())?
        .map_err(|e| format!("could not keep the share: {e}"))?;
    hub.bump_views();
    let mut out = json!({"id": sh.id, "link": route, "name": name, "size": sh.size, "expires": sh.expires,
        "note": "Write `link` in your message: every Backplane client (desktop, phone, web) opens it, on this machine or another. It expires; Settings lists and revokes shares."});
    if !sh.url.is_empty() { out["url"] = json!(sh.url); out["note"] = json!("`url` opens in any app on a device of the person's tailnet until it expires (not outside the tailnet). `link` opens in Backplane's clients.") }
    if !note.is_empty() { out["tailnetError"] = json!(note) }
    Ok(out)
}

/// The tailnet address of a share: the hub's own `tailscale serve` https address when the person set one up for it, else its MagicDNS name when
/// the hub listens on the tailnet. The hub never makes a serve mapping of its own for a share: a share is no second ingress, only a path on
/// the hub's main listener (`/s/<token>/<name>`), reached however that listener is.
async fn tailnet_base(hub: &Arc<Hub>) -> Result<String, String> {
    let (host, port) = hub.cfg.listen.rsplit_once(':').ok_or("the hub's address is unknown")?;
    let port: u16 = port.parse().map_err(|_| "the hub's port is unknown")?;
    let have = crate::net::botnet::served_url(port).await;
    if !have.is_empty() { return Ok(have) }
    let dns = crate::net::botnet::tailscale_dns().await;
    if dns.is_empty() { return Err("this machine is not on a tailnet".into()) }
    if host != "0.0.0.0" && !host.starts_with("100.") { return Err("this hub does not answer on the tailnet (it listens on this computer only)".into()) }
    Ok(format!("http://{dns}:{port}"))
}

/// Drops ended (and the revoked) shares, on disk before it answers. `revoke`: Err when there is no such share. Returns what is left.
pub async fn sweep(hub: &Arc<Hub>, revoke: Option<&str>) -> Result<Vec<Share>, String> {
    let (home, rid) = (hub.cfg.home.clone(), revoke.map(str::to_string));
    let r = tokio::task::spawn_blocking(move || drop_shares(&home, rid.as_deref())).await.map_err(|_| "failed".to_string())?;
    if matches!(&r, Ok((_, true))) { hub.bump_views() }
    r.map(|(v, _)| v)
}
/// (what is left, whether anything went)
fn drop_shares(home: &Path, revoke: Option<&str>) -> Result<(Vec<Share>, bool), String> {
    let now = now_s();
    mutate(home, |all| {
        if let Some(id) = revoke { if !all.iter().any(|s| s.id == id) { return Err("no such share".to_string()) } }
        let n = all.len(); all.retain(|s| s.expires > now && Some(s.id.as_str()) != revoke);
        Ok((all.clone(), all.len() != n))
    })?
}

/// Settings → Devices (the presenter, synchronous): the live shares, and Revoke.
pub fn listed_now() -> Vec<Value> { let h = HOME.read().unwrap().clone(); if h.as_os_str().is_empty() { return vec![] } let now = now_s(); listed(&load(&h).into_iter().filter(|s| s.expires > now).collect::<Vec<_>>()) }
/// Ok once the share is gone from disk; Err with the words Settings shows.
pub fn revoke_now(id: &str) -> Result<(), String> {
    let h = HOME.read().unwrap().clone(); if h.as_os_str().is_empty() { return Err("the hub is not ready".into()) }
    drop_shares(&h, Some(id)).map(|_| ())
}
/// `/s/<token>/<name>`: an unknown, ended or revoked token is 404, and the file is checked again as it is now, against what the caller may reach
/// (`Who::Share` on the main listener: the token is the credential; `Who::Browser` on the web ingress, already paired: the share's file AND the
/// browser's roots). `requester`: whose unknown tokens are counted (the peer address; on the ingress, the paired browser).
pub async fn serve_share<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, who: Who, requester: &str, req: &Req) -> std::io::Result<()> {
    if req.method != "GET" && req.method != "HEAD" { return w.write_all(&refusal("405 Method Not Allowed", "GET only")).await }
    let key = requester.to_string();
    if misses(&key, false) >= MISSES { return w.write_all(&refusal("429 Too Many Requests", "too many unknown links; wait a minute")).await }
    let token = req.route().strip_prefix("/s/").unwrap_or("").split('/').next().unwrap_or("").to_string();
    let now = now_s();
    let found = if token.len() == 48 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        load(&hub.cfg.home).into_iter().find(|s| s.expires > now && s.token.len() == token.len() && s.token.bytes().zip(token.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0)
    } else { None };
    let Some(sh) = found else { misses(&key, true); return w.write_all(&refusal("404 Not Found", "this link has ended or never was")).await };
    if !known(&hub.cfg.home) { refresh(hub).await }
    let (home, roots, p) = (hub.cfg.home.clone(), roots_for(who, &hub.cfg.home), sh.path.clone());
    // the share names its real file: opened by that path, checked by its handle, and it must still be that same file
    match tokio::task::spawn_blocking(move || open_checked(&roots, &home, &p).and_then(|o| if o.real == Path::new(&p) { Ok(o) } else { Err("that file is not shared") })).await {
        Ok(Ok(o)) => stream(w, if who == Who::Browser { Who::Browser } else { Who::Share }, req, o, &sh.name).await,
        Ok(Err(why)) => w.write_all(&refusal(status_of(why), why)).await,
        Err(_) => w.write_all(&refusal("500 Internal Server Error", "cannot read file")).await,
    }
}

/// Unknown share tokens one address may try a minute (192-bit tokens are not guessed; this only stops a scan from costing the hub).
const MISSES: u32 = 30;
/// The misses of `who` in the current minute, one added when `add`.
fn misses(who: &str, add: bool) -> u32 {
    use std::{collections::HashMap, sync::Mutex, time::{Duration, Instant}};
    static M: LazyLock<Mutex<HashMap<String, (Instant, u32)>>> = LazyLock::new(Default::default);
    let mut g = M.lock().unwrap();
    if g.len() > 4096 { g.retain(|_, (t, _)| t.elapsed() < Duration::from_secs(60)) }
    let e = g.entry(who.to_string()).or_insert((Instant::now(), 0));
    if e.0.elapsed() >= Duration::from_secs(60) { *e = (Instant::now(), 0) }
    if add { e.1 += 1 }
    e.1
}

/// Settings: `shares.list` → `{shares: [{id, name, path, size, created, expires, link, url, thread}]}`; `shares.revoke {id}` → the list after it.
struct Rpc;
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["shares.list", "shares.revoke"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let rid = p["id"].as_str().unwrap_or("");
            match sweep(hub, if m == "shares.revoke" { Some(rid) } else { None }).await {
                Ok(left) => vec![json!({"t":"reply","id":id,"ok":true,"result":{"shares": listed(&left)}})],
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
            }
        })
    }
}
pub fn listed(v: &[Share]) -> Vec<Value> {
    let mut v: Vec<&Share> = v.iter().collect(); v.sort_by(|a, b| b.created.cmp(&a.created));
    v.iter().map(|s| json!({"id": s.id, "name": s.name, "path": s.path, "size": s.size, "created": s.created, "expires": s.expires, "link": share_route(&s.token, &s.name), "url": s.url, "thread": s.thread})).collect()
}

struct Sweep;
impl Ticker for Sweep {
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, _now: u64) -> Fut<'a, Vec<Value>> { Box::pin(async move { let _ = sweep(hub, None).await; refresh(hub).await; vec![] }) }
}

/// The MCP tool.
pub struct Tool(pub Arc<Hub>);
impl crate::agents::mcp::ToolSet for Tool {
    fn list(&self, _thread: &str) -> Vec<Value> { vec![json!({"name":"file_share","description":"Share a file of the person's projects so they can open or download it from any of their devices, including a phone, the web app or another machine (a plain path only opens where the hub runs). Answers `link` (write it in your message; Backplane's clients open it) and, with tailnet true, `url`: an https address on the person's tailnet for opening in another app. Shares expire (default 1 day, at most 30 days); the person lists and revokes them in Settings. Secret files (keys, .env, credentials) and files outside the projects are refused.","inputSchema":{"type":"object","properties":{"path":{"type":"string","description":"absolute path of the file"},"expires":{"type":"string","description":"how long it works: seconds, or like 30m, 12h, 7d (default 1d, at most 30d)"},"tailnet":{"type":"boolean","description":"also give an https url on the tailnet, for other apps"}},"required":["path"]}})] }
    fn owns(&self, name: &str) -> bool { name == "file_share" }
    fn call<'a>(&'a self, thread: &'a str, _name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move { match create(&self.0, thread, a).await { Ok(v) => (true, v), Err(e) => (false, json!({"error": e})) } })
    }
}

pub fn start(hub: &Arc<Hub>) {
    *HOME.write().unwrap() = hub.cfg.home.clone();
    hub.register_observer(Arc::new(Fresh));
    hub.register_rpc(Arc::new(Rpc));
    hub.register_rpc(Arc::new(Piece));
    hub.register_ticker(Arc::new(Sweep));
    refresh_soon(hub);
}
pub fn register(hub: &Arc<Hub>, ag: &Arc<crate::agents::Agents>) { ag.add_tools(Arc::new(Tool(hub.clone()))) }

#[cfg(test)]
#[path = "share_tests.rs"]
mod tests;
