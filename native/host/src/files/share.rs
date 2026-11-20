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
//!   revokes it (`shares.list`, `shares.revoke`). A share with `tailnet` also has an https address on the tailnet (the hub's `tailscale serve`, made
//!   for it when the hub has none, and taken down when the last such share ends).
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
static ROOTS: LazyLock<RwLock<Vec<PathBuf>>> = LazyLock::new(Default::default);
pub fn roots() -> Vec<PathBuf> { ROOTS.read().unwrap().clone() }

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
    Ok((real, md))
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
        if events.iter().any(|e| e["$"].as_str().map_or(false, |k| k.starts_with("Project") || k.starts_with("Thread") || k == "SettingChanged" || k == "WorktreeSet")) { refresh_soon(hub) }
    }
}
fn refresh_soon(hub: &Arc<Hub>) {
    let h = hub.clone();
    tokio::spawn(async move { refresh(&h).await });
}
pub async fn refresh(hub: &Arc<Hub>) {
    let extra = hub.setting("files.share.roots").await.unwrap_or_default();
    let r = { let m = hub.model().await; roots_of(&m, &hub.cfg.home, &extra) };
    *ROOTS.write().unwrap() = r;
}

// ---- streaming ----

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
fn head_bytes(status: &str, ctype: &str, len: u64, extra: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\nCache-Control: private, no-cache\r\nAccept-Ranges: bytes\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n{extra}\r\n").into_bytes()
}
pub fn refusal(status: &str, why: &str) -> Vec<u8> {
    let mut v = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n", why.len()).into_bytes();
    v.extend_from_slice(why.as_bytes()); v
}
/// The status a refusal of `check` answers with.
fn status_of(why: &str) -> &'static str { if why == "no such file" || why == "not a file" { "404 Not Found" } else { "403 Forbidden" } }

/// Writes the file (or the range asked) to `w`: the file checked once more as it is opened (the same file the rule allowed, still a regular file).
pub async fn stream<W: AsyncWrite + Unpin>(w: &mut W, req: &Req, real: &Path, md: &std::fs::Metadata, name: &str) -> std::io::Result<()> {
    let tag = etag(md); let size = md.len();
    let Ok((status, a, b)) = plan(req.header("range"), req.header("if-range"), &tag, size) else {
        return w.write_all(&refusal("416 Range Not Satisfiable", "range not satisfiable")).await;
    };
    let len = if size == 0 { 0 } else { b - a + 1 };
    let mut extra = format!("ETag: {tag}\r\n");
    if status.starts_with("206") { extra.push_str(&format!("Content-Range: bytes {a}-{b}/{size}\r\n")) }
    let inline = req.param("inline") == "1" && matches!(ctype(name), t if t.starts_with("image/") || t == "application/pdf" || t.starts_with("text/"));
    extra.push_str(&disposition(name, inline));
    let mut f = match tokio::fs::File::open(real).await { Ok(f) => f, Err(_) => return w.write_all(&refusal("404 Not Found", "no such file")).await };
    match f.metadata().await { Ok(m2) if m2.is_file() && m2.len() == size => {}, _ => return w.write_all(&refusal("409 Conflict", "the file changed; try again")).await }
    w.write_all(&head_bytes(status, ctype(name), len, &extra)).await?;
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
pub async fn serve_file<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, req: &Req) -> std::io::Result<()> {
    if req.method != "GET" && req.method != "HEAD" { return w.write_all(&refusal("405 Method Not Allowed", "GET only")).await }
    let path = crate::files::fm::from_uri(&req.param("path"));
    let thread = crate::files::fm::from_uri(&req.param("thread"));
    if thread.contains('~') { return far_file(hub, w, req, &thread, &path).await }
    if ROOTS.read().unwrap().is_empty() { refresh(hub).await }
    let (home, roots) = (hub.cfg.home.clone(), roots());
    let p2 = path.clone();
    match tokio::task::spawn_blocking(move || check(&roots, &home, &p2)).await {
        Ok(Ok((real, md))) => { let name = name_of(&path); stream(w, req, &real, &md, &name).await }
        Ok(Err(why)) => w.write_all(&refusal(status_of(why), why)).await,
        Err(_) => w.write_all(&refusal("500 Internal Server Error", "cannot read file")).await,
    }
}
/// `/files/get?thread=&path=` (the Files tab's Download): a file of the thread's folder by its path from there, under the Files tab's own
/// containment (`fm::contained`), streamed and ranged like `/file`.
pub async fn serve_files_get<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, req: &Req) -> std::io::Result<()> {
    let (thread, rel) = (crate::files::fm::from_uri(&req.param("thread")), crate::files::fm::from_uri(&req.param("path")));
    if rel.is_empty() || !crate::files::fm::files_safe(&rel) { return w.write_all(&refusal("400 Bad Request", "that path is not in this project")).await }
    if thread.contains('~') { return far_file(hub, w, req, &thread, &rel).await }
    let Some(root) = crate::files::root_of(hub, &thread).await else { return w.write_all(&refusal("404 Not Found", "no such thread")).await };
    let r2 = rel.clone();
    let found = tokio::task::spawn_blocking(move || -> Result<(PathBuf, std::fs::Metadata), &'static str> {
        let real = crate::files::fm::contained(&root, &r2).ok_or("no such file")?;
        let md = std::fs::metadata(&real).map_err(|_| "no such file")?;
        if md.is_file() { Ok((real, md)) } else { Err("not a file") }
    }).await;
    match found {
        Ok(Ok((real, md))) => stream(w, req, &real, &md, &name_of(&rel)).await,
        Ok(Err(why)) => w.write_all(&refusal("404 Not Found", why)).await,
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
            if ROOTS.read().unwrap().is_empty() { refresh(hub).await }
            let asked = p["path"].as_str().unwrap_or("").to_string();
            // a path from the thread's folder (the Files tab's) or an absolute one (a message's)
            let path = if crate::plat::fs::is_abs(&asked) || asked.starts_with("~/") { asked } else if !asked.is_empty() && crate::files::fm::files_safe(&asked) { root.join(&asked).to_string_lossy().into_owned() } else { return fail("not a file of the project") };
            let (home, roots) = (hub.cfg.home.clone(), roots());
            let (at, len) = (p["at"].as_u64().unwrap_or(0), p["len"].as_u64().unwrap_or(FAR_PIECE).min(FAR_PIECE));
            let r = tokio::task::spawn_blocking(move || -> Result<Value, &'static str> {
                use base64::Engine; use std::io::{Read, Seek};
                let (real, md) = check(&roots, &home, &path)?;
                let root = crate::plat::fs::canonicalize(&root).map_err(|_| "no such folder")?;
                if !real.starts_with(&root) { return Err("that file is not in this thread's project") }
                let mut f = std::fs::File::open(&real).map_err(|_| "no such file")?;
                let n = len.min(md.len().saturating_sub(at));
                let mut b = vec![0u8; n as usize];
                f.seek(std::io::SeekFrom::Start(at)).map_err(|_| "cannot read it")?; f.read_exact(&mut b).map_err(|_| "cannot read it")?;
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
async fn far_file<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, req: &Req, thread: &str, path: &str) -> std::io::Result<()> {
    let Some((link, local)) = thread.split_once('~').filter(|(l, t)| !l.is_empty() && !t.is_empty()) else { return w.write_all(&refusal("404 Not Found", "no such thread")).await };
    let first = match far_piece(hub, link, local, path, 0, 1).await { Ok(x) => x, Err(e) => return w.write_all(&refusal("502 Bad Gateway", &e)).await };
    let (size, tag) = (first.1, first.2);
    let Ok((status, a, b)) = plan(req.header("range"), req.header("if-range"), &tag, size) else { return w.write_all(&refusal("416 Range Not Satisfiable", "range not satisfiable")).await };
    let len = if size == 0 { 0 } else { b - a + 1 };
    let name = name_of(path);
    let mut extra = format!("ETag: {tag}\r\n");
    if status.starts_with("206") { extra.push_str(&format!("Content-Range: bytes {a}-{b}/{size}\r\n")) }
    extra.push_str(&disposition(&name, false));
    w.write_all(&head_bytes(status, ctype(&name), len, &extra)).await?;
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
    pub created: u64, pub expires: u64, #[serde(default)] pub tailnet: bool, #[serde(default)] pub url: String, #[serde(default)] pub served_port: u16 }

fn store_path(home: &Path) -> PathBuf { home.join("shares-v1.json") }
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
pub fn load(home: &Path) -> Vec<Share> {
    std::fs::read_to_string(store_path(home)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value::<Vec<Share>>(v["shares"].clone()).ok()).unwrap_or_default()
}
/// The https ports this hub made `tailscale serve` answer on for shares (taken down when no live share uses one).
fn served(home: &Path) -> Vec<u16> {
    std::fs::read_to_string(store_path(home)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value::<Vec<u16>>(v["served"].clone()).ok()).unwrap_or_default()
}
fn save(home: &Path, v: &[Share]) -> std::io::Result<()> { let mut ports = served(home); for s in v { if s.served_port > 0 && !ports.contains(&s.served_port) { ports.push(s.served_port) } } save_with(home, v, &ports) }
fn save_with(home: &Path, v: &[Share], ports: &[u16]) -> std::io::Result<()> {
    let p = store_path(home); let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&json!({"shares": v, "served": ports}))?)?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)); }
    std::fs::rename(tmp, p)
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
    if ROOTS.read().unwrap().is_empty() { refresh(hub).await }
    let (home, roots, p2) = (hub.cfg.home.clone(), roots(), path.clone());
    let (real, md) = tokio::task::spawn_blocking(move || check(&roots, &home, &p2)).await.map_err(|_| "failed".to_string())?.map_err(str::to_string)?;
    let name = name_of(&path);
    let token = random_hex(24);
    let now = now_s();
    let mut sh = Share { id: format!("sh{}", &random_hex(6)), token: token.clone(), path: real.to_string_lossy().into_owned(), name: name.clone(), size: md.len(), thread: thread.into(), created: now, expires: now + life, tailnet: a["tailnet"].as_bool().unwrap_or(false), url: String::new(), served_port: 0 };
    let route = share_route(&token, &name);
    let mut note = String::new();
    if sh.tailnet {
        match tailnet_base(hub).await { Ok((base, port)) => { sh.url = format!("{base}{route}"); sh.served_port = port }, Err(e) => note = e }
    }
    let _g = LOCK.lock().await;
    let mut all = load(&hub.cfg.home); all.retain(|s| s.expires > now); all.push(sh.clone());
    save(&hub.cfg.home, &all).map_err(|e| format!("could not keep the share: {e}"))?;
    hub.bump_views();
    let mut out = json!({"id": sh.id, "link": route, "name": name, "size": sh.size, "expires": sh.expires,
        "note": "Write `link` in your message: every Backplane client (desktop, phone, web) opens it, on this machine or another. It expires; Settings lists and revokes shares."});
    if !sh.url.is_empty() { out["url"] = json!(sh.url); out["note"] = json!("`url` opens in any app on a device of the person's tailnet until it expires (not outside the tailnet). `link` opens in Backplane's clients.") }
    if !note.is_empty() { out["tailnetError"] = json!(note) }
    Ok(out)
}

/// The tailnet https base for shares: the hub's own `tailscale serve` address, else one made for shares on a free port (8443-8499), recorded so it is
/// taken down with the last tailnet share. Returns (base, the port this hub made, 0 when it was there already).
async fn tailnet_base(hub: &Arc<Hub>) -> Result<(String, u16), String> {
    let port: u16 = hub.cfg.listen.rsplit(':').next().and_then(|p| p.parse().ok()).ok_or("the hub's port is unknown")?;
    if std::env::var("BACKPLANE_NO_TAILSCALE").is_ok() { return Err("tailscale is off for this hub".into()) }
    let have = crate::net::botnet::served_url(port).await;
    if !have.is_empty() { return Ok((have, 0)) }
    let dns = crate::net::botnet::tailscale_dns().await;
    if dns.is_empty() { return Err("this machine is not on a tailnet".into()) }
    let taken: Vec<u16> = served(&hub.cfg.home);
    if let Some(p) = taken.first() { return Ok((format!("https://{dns}:{p}"), *p)) }
    for p in 8443u16..8500 {
        let out = tokio::time::timeout(std::time::Duration::from_secs(15), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale")
            .args(["serve", "--bg", &format!("--https={p}"), &format!("http://127.0.0.1:{port}")]).output()).await;
        match out { Ok(Ok(o)) if o.status.success() => return Ok((format!("https://{dns}:{p}"), p)), Ok(Ok(o)) if String::from_utf8_lossy(&o.stderr).contains("already") => continue, Ok(Ok(o)) => return Err(format!("tailscale serve failed: {}", String::from_utf8_lossy(&o.stderr).trim())), _ => return Err("tailscale did not answer".into()) }
    }
    Err("no free https port for tailscale serve".into())
}
async fn unserve(port: u16) {
    let _ = tokio::time::timeout(std::time::Duration::from_secs(15), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale").args(["serve", &format!("--https={port}"), "off"]).output()).await;
}

/// Drops ended shares; a tailnet address this hub made goes when no live share uses it. Returns what is left.
pub async fn sweep(hub: &Arc<Hub>, revoke: Option<&str>) -> Vec<Share> {
    let _g = LOCK.lock().await;
    let now = now_s();
    let all = load(&hub.cfg.home);
    let (keep, gone): (Vec<Share>, Vec<Share>) = all.into_iter().partition(|s| s.expires > now && Some(s.id.as_str()) != revoke);
    let ports = served(&hub.cfg.home);
    let (used, idle): (Vec<u16>, Vec<u16>) = ports.iter().partition(|p| keep.iter().any(|s| s.served_port == **p));
    if !gone.is_empty() || !idle.is_empty() {
        for p in &idle { unserve(*p).await }
        let _ = save_with(&hub.cfg.home, &keep, &used);
        hub.bump_views();
    }
    keep
}

/// Settings → Devices (the presenter, synchronous): the live shares, and Revoke. A tailnet address left with no share is taken down by the next sweep.
pub fn listed_now() -> Vec<Value> { let h = HOME.read().unwrap().clone(); if h.as_os_str().is_empty() { return vec![] } let now = now_s(); listed(&load(&h).into_iter().filter(|s| s.expires > now).collect::<Vec<_>>()) }
pub fn revoke_now(id: &str) {
    let h = HOME.read().unwrap().clone(); if h.as_os_str().is_empty() { return }
    let all = load(&h); let keep: Vec<Share> = all.iter().filter(|s| s.id != id).cloned().collect();
    if keep.len() != all.len() { let _ = save_with(&h, &keep, &served(&h)); }
}
/// `/s/<token>/<name>`: no other credential; an unknown, ended or revoked token is 404, and the file is checked again as it is now.
pub async fn serve_share<W: AsyncWrite + Unpin>(hub: &Arc<Hub>, w: &mut W, req: &Req) -> std::io::Result<()> {
    if req.method != "GET" && req.method != "HEAD" { return w.write_all(&refusal("405 Method Not Allowed", "GET only")).await }
    let token = req.route().strip_prefix("/s/").unwrap_or("").split('/').next().unwrap_or("").to_string();
    let now = now_s();
    let found = if token.len() == 48 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        load(&hub.cfg.home).into_iter().find(|s| s.expires > now && s.token.len() == token.len() && s.token.bytes().zip(token.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0)
    } else { None };
    let Some(sh) = found else { return w.write_all(&refusal("404 Not Found", "this link has ended or never was")).await };
    if ROOTS.read().unwrap().is_empty() { refresh(hub).await }
    let (home, roots, p) = (hub.cfg.home.clone(), roots(), sh.path.clone());
    match tokio::task::spawn_blocking(move || check(&roots, &home, &p)).await {
        Ok(Ok((real, md))) => stream(w, req, &real, &md, &sh.name).await,
        Ok(Err(why)) => w.write_all(&refusal(status_of(why), why)).await,
        Err(_) => w.write_all(&refusal("500 Internal Server Error", "cannot read file")).await,
    }
}

/// Settings: `shares.list` → `{shares: [{id, name, path, size, created, expires, link, url, thread}]}`; `shares.revoke {id}` → the list after it.
struct Rpc;
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["shares.list", "shares.revoke"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let rid = p["id"].as_str().unwrap_or("");
            if m == "shares.revoke" && !load(&hub.cfg.home).iter().any(|s| s.id == rid) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"no such share"})] }
            let left = sweep(hub, if m == "shares.revoke" { Some(rid) } else { None }).await;
            vec![json!({"t":"reply","id":id,"ok":true,"result":{"shares": listed(&left)}})]
        })
    }
}
pub fn listed(v: &[Share]) -> Vec<Value> {
    let mut v: Vec<&Share> = v.iter().collect(); v.sort_by(|a, b| b.created.cmp(&a.created));
    v.iter().map(|s| json!({"id": s.id, "name": s.name, "path": s.path, "size": s.size, "created": s.created, "expires": s.expires, "link": share_route(&s.token, &s.name), "url": s.url, "thread": s.thread})).collect()
}

struct Sweep;
impl Ticker for Sweep {
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, _now: u64) -> Fut<'a, Vec<Value>> { Box::pin(async move { sweep(hub, None).await; refresh(hub).await; vec![] }) }
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
