//! Where a download goes, what it is called, and how a file of the hub's comes down (docs/file-links.md). Kept out of the embedded browser
//! (feature `cef`) so the default test run, which CI and the gate build without the Chromium distribution, covers it.
//!
//! `fetch`: a GET on the hub with the client's credential, written straight to `<Downloads>/<name>.part` (never whole in memory), keeping the
//! answer's `ETag`. An error or a body that ends short asks again with `Range: bytes=<have>-` and `If-Range: <etag>`: a 206 appends, a 200 starts
//! over. When the length is complete the `.part` file is renamed to a free name in the Downloads folder. The decisions are pure (`plan`, `head`,
//! `next`) and tested; the window's side is screens/dlbar.rs.
use std::path::{Path, PathBuf};

/// The person's Downloads folder: BACKPLANE_GPUI_DOWNLOADS (a test rig's), XDG_DOWNLOAD_DIR, else ~/Downloads.
pub fn downloads_folder() -> PathBuf {
    std::env::var_os("BACKPLANE_GPUI_DOWNLOADS").filter(|v| !v.is_empty()).map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from))
        .or_else(|| crate::plat::user_home().map(|h| h.join("Downloads"))).unwrap_or_else(std::env::temp_dir) // Windows: %USERPROFILE%\Downloads (no HOME there)
}
/// A finished download at `file` (named by the engine, e.g. a guid in a temp folder) moved into the Downloads folder as `suggested`; the file already there, or a failure, leaves `file` as it was.
#[cfg_attr(not(feature = "cef"), allow(dead_code))]
pub fn bring_to_downloads(file: &Path, suggested: &str) -> PathBuf { bring_to(&downloads_folder(), file, suggested) }
#[cfg_attr(not(feature = "cef"), allow(dead_code))]
fn bring_to(dir: &Path, file: &Path, suggested: &str) -> PathBuf {
    if file.parent() == Some(dir) || suggested.trim().is_empty() || !file.is_file() { return file.to_path_buf() }
    let _ = std::fs::create_dir_all(dir);
    let to = unique_path(dir, &safe_name(suggested));
    if std::fs::rename(file, &to).is_ok() || std::fs::copy(file, &to).is_ok() { to } else { file.to_path_buf() }
}
pub fn safe_name(n: &str) -> String { let n: String = n.chars().map(|c| if c == '/' || c == '\\' || c.is_control() { '_' } else { c }).collect(); let n = n.trim_matches('.').trim().to_string(); if n.is_empty() { "download".into() } else { n } }
/// `dir/name`, or `dir/name (2)`... when that file exists
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name); if !first.exists() { return first }
    let (stem, ext) = match name.rsplit_once('.') { Some((a, b)) if !a.is_empty() => (a.to_string(), format!(".{b}")), _ => (name.to_string(), String::new()) };
    /* i18n-ok: a file name pattern */ (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap()
}
/// Where a download is written while it comes: `<name>.part` beside where it will be, one no other download holds.
pub fn part_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(format!("{name}.part")); if !first.exists() { return first }
    (2..).map(|n| dir.join(format!("{name}.{n}.part"))).find(|p| !p.exists()).unwrap()
}

/// A query value percent-encoded (everything but unreserved characters).
pub fn pct(s: &str) -> String { s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect() }
/// `%XX` decoded; a bad escape is kept as it is.
pub fn unpct(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    while i < b.len() { if b[i] == b'%' && i + 3 <= b.len() { if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) { out.push(h * 16 + l); i += 3; continue } } out.push(b[i]); i += 1 }
    String::from_utf8_lossy(&out).into_owned()
}

/// What one request asks for: the whole file, or the rest of it after `have` bytes while the file is still the one tagged `etag`. Without a tag
/// a partial file cannot be trusted to be the same file: it starts over.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan { Whole, Rest { from: u64, etag: String } }
pub fn plan(have: u64, etag: &str) -> Plan { if have > 0 && !etag.is_empty() { Plan::Rest { from: have, etag: etag.to_string() } } else { Plan::Whole } }
/// The request: GET with the credential as a bearer (never in the address, so it is in no log) and, for the rest of a file, its range.
pub fn request(addr: &str, token: &str, url: &str, p: &Plan) -> String {
    let range = match p { Plan::Whole => String::new(), Plan::Rest { from, etag } => format!("Range: bytes={from}-\r\nIf-Range: {etag}\r\n") }; // i18n-ok: English kept, drawn through trs; or HTTP
    let auth = if token.is_empty() { String::new() } else { format!("Authorization: Bearer {token}\r\n") }; // i18n-ok: English kept, drawn through trs; or HTTP
    /* i18n-ok: an HTTP request */ format!("GET {url} HTTP/1.1\r\nHost: {addr}\r\n{auth}{range}Accept-Encoding: identity\r\nConnection: close\r\n\r\n")
}

/// An answer's head: status, `Content-Length`, `ETag`, `Content-Range` (first byte, total), and whether the body is chunked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Head { pub status: u16, pub length: Option<u64>, pub etag: String, pub range: Option<(Option<u64>, Option<u64>)>, pub chunked: bool }
/// The head of an answer, None until `\r\n\r\n` has come; with it, where the body starts in `b`.
pub fn head(b: &[u8]) -> Option<(Head, usize)> {
    let end = b.windows(4).position(|w| w == b"\r\n\r\n")?;
    let text = String::from_utf8_lossy(&b[..end]);
    let mut lines = text.split("\r\n");
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let mut h = Head { status, ..Default::default() };
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue }; let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => h.length = v.parse().ok(),
            "etag" => h.etag = v.to_string(),
            "transfer-encoding" => h.chunked = v.to_ascii_lowercase().contains("chunked"),
            // `bytes a-b/total` or `bytes */total`
            "content-range" => { let r = v.strip_prefix("bytes ").unwrap_or(v); let (span, total) = r.split_once('/').unwrap_or((r, "*"));
                h.range = Some((span.split_once('-').and_then(|(a, _)| a.trim().parse().ok()), total.trim().parse().ok())); }
            _ => {}
        }
    }
    Some((h, end + 4))
}

/// What to do with an answer to a request made by `p`.
#[derive(Clone, Debug, PartialEq)]
pub enum Next {
    /// Write the body from byte `at` of the file (0: over whatever was there); the whole file is `total` bytes when known.
    Write { at: u64, total: Option<u64>, etag: String },
    /// Nothing more to fetch: the file already has every byte (`have` was all of it).
    Complete,
    /// Ask again for the whole file (the range no longer fits the file).
    Restart,
    /// Stop: the hub refused (`retry` false) or failed in a way another try may not (`retry` true).
    Fail { why: String, retry: bool },
}
pub fn next(p: &Plan, h: &Head) -> Next {
    /* i18n-ok: English kept, drawn through trs */
    match (h.status, p) {
        _ if h.chunked => Next::Fail { why: "the hub sent the file in a form this client does not read".into(), retry: false }, // i18n-ok: English kept, drawn through trs; or HTTP
        (200, _) => Next::Write { at: 0, total: h.length, etag: h.etag.clone() },
        (206, Plan::Rest { from, etag }) => match h.range {
            Some((Some(a), total)) if a == *from => Next::Write { at: a, total, etag: if h.etag.is_empty() { etag.clone() } else { h.etag.clone() } },
            _ => Next::Restart,
        },
        (206, Plan::Whole) => Next::Restart,
        (416, Plan::Rest { from, .. }) => if h.range.and_then(|r| r.1) == Some(*from) { Next::Complete } else { Next::Restart },
        (408 | 429 | 500..=599, _) => Next::Fail { why: format!("the hub answered {}", h.status), retry: true }, // i18n-ok: English kept, drawn through trs; or HTTP
        (404, _) => Next::Fail { why: "that file is not there any more".into(), retry: false }, // i18n-ok: English kept, drawn through trs; or HTTP
        (401 | 403, _) => Next::Fail { why: "the hub does not share that file".into(), retry: false }, // i18n-ok: English kept, drawn through trs; or HTTP
        (s, _) => Next::Fail { why: format!("the hub answered {s}"), retry: false }, // i18n-ok: English kept, drawn through trs; or HTTP
    }
}

/// Percent done, when the size is known.
pub fn percent(got: u64, total: Option<u64>) -> Option<u32> { total.filter(|t| *t > 0).map(|t| ((got.min(t) as f64 / t as f64) * 100.).floor() as u32) }

/// A request's endpoint and credential: the hub's address, TLS or not, and its token (none for a share).
#[derive(Clone, Debug)]
pub struct Source { pub addr: String, pub tls: bool, pub token: String }
impl Source { pub fn of(t: &crate::screens::attach::Target) -> Self { Self { addr: t.addr.clone(), tls: t.tls, token: t.token.clone() } } }

/// How a download stands, as it goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Event { Progress { got: u64, total: Option<u64> }, Done { path: PathBuf, bytes: u64 }, Failed { why: String, part: Option<PathBuf>, etag: String, got: u64 } }

/// Tries after the first, and how long each one waits more than the last.
pub const RETRIES: u32 = 4;
const BACKOFF_MS: u64 = 600;
/// The least time between two progress reports (the window repaints on each).
pub const REPORT_MS: u128 = 200;

/// A part file to go on with: where it is and the tag of the file it holds bytes of.
#[derive(Clone, Debug, Default)]
pub struct Resume { pub part: Option<PathBuf>, pub etag: String }

/// Fetch `url` (a path on the hub, `/file?...`, `/files/get?...`, `/s/...`) into the folder `dir` as `name`, resuming after errors. `report`
/// hears progress at most every REPORT_MS and the end once; it is called from this thread. `stop` set: the download ends between two reads,
/// its part file deleted, and nothing more is reported.
pub fn fetch(src: &Source, url: &str, dir: &Path, name: &str, resume: Resume, stop: &std::sync::atomic::AtomicBool, mut report: impl FnMut(Event)) {
    let name = safe_name(name);
    if let Err(e) = std::fs::create_dir_all(dir) { return report(Event::Failed { why: e.to_string(), part: None, etag: String::new(), got: 0 }) }
    let part = resume.part.filter(|p| p.is_file()).unwrap_or_else(|| part_path(dir, &name));
    let mut etag = resume.etag;
    let mut have = std::fs::metadata(&part).map_or(0, |m| m.len());
    if have > 0 && etag.is_empty() { have = 0 }
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut tries = 0u32;
    let mut why = String::new();
    loop {
        let p = plan(have, &etag);
        match once(src, url, &part, &p, &mut have, &mut etag, stop, &mut |got, total| { if last.elapsed().as_millis() >= REPORT_MS { last = std::time::Instant::now(); report(Event::Progress { got, total }) } }) {
            Ok(()) => break,
            Err(Step::Restart) => { have = 0; etag.clear(); }
            Err(Step::Stop(w)) => { why = w; break }
            Err(Step::Cancel) => { let _ = std::fs::remove_file(&part); return }
            Err(Step::Again(w)) => { why = w; if tries >= RETRIES { break } }
        }
        tries += 1;
        if tries > RETRIES + 1 { /* i18n-ok: English kept, drawn through trs */ if why.is_empty() { why = "the hub kept changing the file".into() } break }
        std::thread::sleep(std::time::Duration::from_millis(BACKOFF_MS * tries as u64));
        if stop.load(std::sync::atomic::Ordering::Relaxed) { let _ = std::fs::remove_file(&part); return }
        why.clear();
    }
    if !why.is_empty() { return report(Event::Failed { why, part: Some(part.clone()).filter(|p| p.is_file()), etag, got: have }) }
    let to = unique_path(dir, &name);
    match std::fs::rename(&part, &to) { Ok(()) => report(Event::Done { path: to, bytes: have }), Err(e) => report(Event::Failed { why: e.to_string(), part: Some(part), etag, got: have }) }
}
enum Step { Restart, Again(String), Stop(String), Cancel }
/// One request: the head read, the body written to `part` from where `next` says; `have` and `etag` follow what is on disk.
fn once(src: &Source, url: &str, part: &Path, p: &Plan, have: &mut u64, etag: &mut String, stop: &std::sync::atomic::AtomicBool, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<(), Step> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let again = |e: std::io::Error| Step::Again(e.to_string());
    let mut s = crate::pairing::open(&src.addr, src.tls).map_err(again)?;
    s.set_timeouts(std::time::Duration::from_secs(60)).map_err(again)?;
    s.write_all(request(&src.addr, &src.token, url, p).as_bytes()).map_err(again)?;
    let mut buf = vec![0u8; 256 << 10]; let mut got: Vec<u8> = Vec::new();
    /* i18n-ok: English kept, drawn through trs */
    let (h, at) = loop {
        let n = s.read(&mut buf).map_err(again)?;
        if n == 0 { return Err(Step::Again("the hub closed the connection".into())) } // i18n-ok: English kept, drawn through trs; or HTTP
        got.extend_from_slice(&buf[..n]);
        if let Some(x) = head(&got) { break x }
        if got.len() > 64 << 10 { return Err(Step::Stop("the hub's answer has no end to its head".into())) } // i18n-ok: English kept, drawn through trs; or HTTP
    };
    let (from, total) = match next(p, &h) {
        Next::Write { at, total, etag: tag } => { *etag = tag; (at, total) }
        Next::Complete => return Ok(()),
        Next::Restart => return Err(Step::Restart),
        Next::Fail { why, retry } => return Err(if retry { Step::Again(why) } else { Step::Stop(why) }),
    };
    let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(part).map_err(|e| Step::Stop(e.to_string()))?;
    f.set_len(from).map_err(|e| Step::Stop(e.to_string()))?; f.seek(SeekFrom::Start(from)).map_err(|e| Step::Stop(e.to_string()))?;
    let mut w = std::io::BufWriter::with_capacity(256 << 10, f);
    *have = from;
    let body_len = h.length;
    let mut wrote = 0u64;
    let mut put = |b: &[u8], have: &mut u64, wrote: &mut u64| -> Result<(), Step> { w.write_all(b).map_err(|e| Step::Stop(e.to_string()))?; *have += b.len() as u64; *wrote += b.len() as u64; Ok(()) };
    let first = &got[at..]; let first = &first[..body_len.map_or(first.len(), |l| first.len().min(l as usize))];
    put(first, have, &mut wrote)?;
    progress(*have, total);
    let end = loop {
        if body_len.map_or(false, |l| wrote >= l) { break Ok(()) }
        if stop.load(std::sync::atomic::Ordering::Relaxed) { return Err(Step::Cancel) }
        match s.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => { let n = body_len.map_or(n, |l| n.min((l - wrote) as usize)); put(&buf[..n], have, &mut wrote)?; progress(*have, total); }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => break Err(e),
        }
    };
    w.flush().map_err(|e| Step::Stop(e.to_string()))?;
    let _ = w.get_ref().sync_data();
    end.map_err(again)?;
    /* i18n-ok: English kept, drawn through trs */
    match (total, body_len) {
        (Some(t), _) if *have == t => Ok(()),
        (Some(t), _) if *have < t => Err(Step::Again("the download ended early".into())), // i18n-ok: English kept, drawn through trs; or HTTP
        (Some(_), _) => Err(Step::Restart),
        (None, Some(l)) if wrote < l => Err(Step::Again("the download ended early".into())), // i18n-ok: English kept, drawn through trs; or HTTP
        (None, _) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_finished_download_named_by_the_engine_is_brought_to_downloads_under_its_own_name() {
        let d = crate::tmp::uncreated("dl-bring");
        let (tmp, dl) = (d.join("artifacts"), d.join("Downloads")); std::fs::create_dir_all(&tmp).unwrap();
        let f = tmp.join("461feba6-guid"); std::fs::write(&f, "bytes").unwrap();
        let to = bring_to(&dl, &f, "report.txt"); assert_eq!(to, dl.join("report.txt")); assert_eq!(std::fs::read_to_string(&to).unwrap(), "bytes"); assert!(!f.exists());
        let g = tmp.join("second-guid"); std::fs::write(&g, "again").unwrap(); let to2 = bring_to(&dl, &g, "report.txt"); assert_eq!(to2, dl.join("report (2).txt"), "never over a file");
        assert_eq!(bring_to(&dl, &to, "report.txt"), to, "a file already in Downloads stays");
        assert_eq!(bring_to(&dl, &tmp.join("nothing"), "x.txt"), tmp.join("nothing"), "a missing file is left alone");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn the_rest_of_a_file_is_asked_for_only_with_its_tag() {
        assert_eq!(plan(0, "\"a\""), Plan::Whole); assert_eq!(plan(10, ""), Plan::Whole, "no tag: the bytes cannot be trusted to be the same file");
        assert_eq!(plan(10, "\"a\""), Plan::Rest { from: 10, etag: "\"a\"".into() });
        let r = request("h:1", "tok", "/file?path=%2Fa", &plan(10, "\"a\""));
        assert!(r.starts_with("GET /file?path=%2Fa HTTP/1.1\r\nHost: h:1\r\nAuthorization: Bearer tok\r\nRange: bytes=10-\r\nIf-Range: \"a\"\r\n"), "{r}");
        assert!(!request("h:1", "tok", "/x", &Plan::Whole).contains("Range"));
        assert!(!request("h:1", "", "/s/t/a", &Plan::Whole).contains("Authorization"), "a share needs no credential");
        assert!(!request("h:1", "tok", "/x", &Plan::Whole).contains("token="), "the token is never in the address");
    }
    #[test] fn heads_are_read_with_their_range_and_tag() {
        let raw: &[u8] = b"HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nETag: \"9-1\"\r\nContent-Range: bytes 4-8/9\r\n\r\nhello";
        let (h, at) = head(raw).unwrap();
        assert_eq!((h.status, h.length, h.etag.as_str(), h.range, h.chunked, &raw[at..]), (206, Some(5), "\"9-1\"", Some((Some(4), Some(9))), false, &b"hello"[..]));
        let (h, _) = head(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */9\r\n\r\n").unwrap(); assert_eq!(h.range, Some((None, Some(9))));
        assert!(head(b"HTTP/1.1 200 OK\r\nContent-Le").is_none(), "not yet");
        assert!(head(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap().0.chunked);
    }
    #[test] fn an_answer_says_append_restart_or_stop() {
        let rest = plan(4, "\"t\"");
        let h = |s: u16, range: Option<(Option<u64>, Option<u64>)>, tag: &str| Head { status: s, length: Some(5), etag: tag.into(), range, chunked: false };
        assert_eq!(next(&rest, &h(206, Some((Some(4), Some(9))), "\"t\"")), Next::Write { at: 4, total: Some(9), etag: "\"t\"".into() }, "206: append");
        assert_eq!(next(&rest, &h(200, None, "\"u\"")), Next::Write { at: 0, total: Some(5), etag: "\"u\"".into() }, "200: the file changed, start over with its new tag");
        assert_eq!(next(&rest, &h(206, Some((Some(2), Some(9))), "\"t\"")), Next::Restart, "a range that is not ours");
        assert_eq!(next(&Plan::Whole, &h(206, Some((Some(0), Some(9))), "")), Next::Restart);
        assert_eq!(next(&rest, &h(416, Some((None, Some(4))), "")), Next::Complete, "every byte is already here");
        assert_eq!(next(&rest, &h(416, Some((None, Some(3))), "")), Next::Restart, "the file shrank");
        assert!(matches!(next(&Plan::Whole, &h(503, None, "")), Next::Fail { retry: true, .. }));
        assert!(matches!(next(&Plan::Whole, &h(403, None, "")), Next::Fail { retry: false, .. }), "a refusal is not tried again");
        assert!(matches!(next(&Plan::Whole, &h(404, None, "")), Next::Fail { retry: false, .. }));
        assert!(matches!(next(&Plan::Whole, &Head { chunked: true, ..h(200, None, "") }), Next::Fail { retry: false, .. }));
    }
    #[test] fn names_never_overwrite_and_parts_never_collide() {
        let d = crate::tmp::uncreated("dl-names"); std::fs::create_dir_all(&*d).unwrap();
        assert_eq!(part_path(&d, "a.zip"), d.join("a.zip.part")); std::fs::write(d.join("a.zip.part"), "").unwrap(); assert_eq!(part_path(&d, "a.zip"), d.join("a.zip.2.part"));
        std::fs::write(d.join("a.zip"), "").unwrap(); assert_eq!(unique_path(&d, "a.zip"), d.join("a (2).zip"));
        assert_eq!(safe_name("../x/y"), "_x_y"); assert_eq!(safe_name(".."), "download");
        assert_eq!(pct("/a b/ü.txt"), "%2Fa%20b%2F%C3%BC.txt"); assert_eq!(unpct(&pct("/a b/ü.txt")), "/a b/ü.txt");
        assert_eq!(percent(50, Some(200)), Some(25)); assert_eq!(percent(5, None), None); assert_eq!(percent(9, Some(9)), Some(100));
    }

    /// A hub stand-in: serves `body` with ranges and If-Range as files/share.rs does; the first answer of `cut` stops after that many body bytes.
    fn serve(body: Vec<u8>, tag: &'static str, cut: Option<usize>, answers: usize) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        let h = std::thread::spawn(move || { let mut reqs = vec![]; let mut cut = cut;
            for s in l.incoming().take(answers) { let mut s = s.unwrap(); let mut b = vec![0u8; 4096]; let n = s.read(&mut b).unwrap(); let req = String::from_utf8_lossy(&b[..n]).to_string();
                let from = req.lines().find_map(|l| l.strip_prefix("Range: bytes=")).and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                let same = req.lines().find_map(|l| l.strip_prefix("If-Range: ")).map_or(true, |t| t == tag);
                reqs.push(req);
                let (head, part) = match from.filter(|_| same) {
                    Some(a) if a >= body.len() => (format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\n\r\n", body.len()), &body[0..0]),
                    Some(a) => (format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nETag: {tag}\r\nContent-Range: bytes {a}-{}/{}\r\n\r\n", body.len() - a, body.len() - 1, body.len()), &body[a..]),
                    None => (format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: {tag}\r\n\r\n", body.len()), &body[..]),
                };
                let _ = s.write_all(head.as_bytes());
                let part = match cut.take() { Some(c) => &part[..c.min(part.len())], None => part };
                let _ = s.write_all(part);
            }
            reqs });
        (addr, h)
    }
    fn body(n: usize) -> Vec<u8> { (0..n).map(|i| (i * 31 % 251) as u8).collect() }
    #[test] fn a_body_that_ends_short_is_resumed_from_where_it_stopped() {
        let b = body(3 << 20); let (addr, h) = serve(b.clone(), "\"s-1\"", Some(1 << 20), 2);
        let d = crate::tmp::uncreated("dl-resume");
        let mut events = vec![];
        fetch(&Source { addr, tls: false, token: "tok".into() }, "/file?path=%2Fw%2Fbig.bin", &d, "big.bin", Resume::default(), &Default::default(), |e| events.push(e));
        let reqs = h.join().unwrap();
        assert!(reqs[0].contains("Authorization: Bearer tok") && !reqs[0].contains("Range:"));
        assert!(reqs[1].contains(&format!("Range: bytes={}-", 1 << 20)) && reqs[1].contains("If-Range: \"s-1\""), "{}", reqs[1]);
        let Some(Event::Done { path, bytes }) = events.last() else { panic!("{events:?}") };
        assert_eq!((path.clone(), *bytes), (d.join("big.bin"), b.len() as u64));
        assert_eq!(std::fs::read(path).unwrap(), b, "the same bytes, in order");
        assert!(!d.join("big.bin.part").exists(), "the part file became the file");
    }
    #[test] fn a_file_that_changed_meanwhile_is_fetched_again_whole() {
        let b = body(200_000); let (addr, h) = serve(b.clone(), "\"new\"", None, 1);
        let d = crate::tmp::uncreated("dl-changed"); std::fs::create_dir_all(&*d).unwrap();
        let part = d.join("x.bin.part"); std::fs::write(&part, vec![7u8; 50_000]).unwrap();
        let mut events = vec![];
        fetch(&Source { addr, tls: false, token: "t".into() }, "/file?path=x", &d, "x.bin", Resume { part: Some(part), etag: "\"old\"".into() }, &Default::default(), |e| events.push(e));
        let reqs = h.join().unwrap(); assert!(reqs[0].contains("If-Range: \"old\""));
        assert!(matches!(events.last(), Some(Event::Done { .. })), "{events:?}");
        assert_eq!(std::fs::read(d.join("x.bin")).unwrap(), b, "the old bytes are gone");
    }
    #[test] fn a_refusal_stops_at_once_and_keeps_nothing() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        let h = std::thread::spawn(move || { let (mut s, _) = l.accept().unwrap(); let mut b = [0u8; 2048]; let _ = s.read(&mut b); let _ = s.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 24\r\n\r\nthat file is not shared."); });
        let d = crate::tmp::uncreated("dl-refused"); let mut events = vec![];
        fetch(&Source { addr, tls: false, token: "t".into() }, "/file?path=x", &d, "x.env", Resume::default(), &Default::default(), |e| events.push(e));
        h.join().unwrap();
        assert!(matches!(events.last(), Some(Event::Failed { part: None, .. })), "{events:?}");
        assert!(!d.join("x.env").exists());
    }
}
