//! The mail journal: one file per conversation, `<home>/mail/<pct(conversation)>.jsonl`, a body per line.
//! An append first looks at the end of the file: an incomplete last line (a crash mid-write) is cut off, a complete last line must be a valid body, and only then
//! the new body is added. Process-crash consistency only: no fsync, as in . Files are opened without following links: the directory must be a real
//! directory, the file a regular file, and a file written to has exactly one link (a link at `<home>/mail/x.jsonl` would send mail text somewhere else).
//! A failed commit after an append leaves a body whose retry appends it again; recovery and the box (by id) make that harmless.
//! Errors say what kind of failure it was, never which conversation: a conversation id is an address.
use super::{body_line, body_parse, pct, Body, Conv, Hdr};
use std::{fs::OpenOptions, io::{BufRead, Read, Seek, SeekFrom, Write}, os::unix::fs::{MetadataExt, OpenOptionsExt}, path::{Path, PathBuf}};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StoreError { Io, Link, NotRegular, Hardlink, Malformed }
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { StoreError::Io => "mail journal i/o failed", StoreError::Link => "mail journal path is a link", StoreError::NotRegular => "mail journal is not a regular file",
            StoreError::Hardlink => "mail journal has more than one link", StoreError::Malformed => "malformed final completed body record" })
    }
}
impl std::error::Error for StoreError {}

pub fn path(home: &Path, conv: &str) -> PathBuf { home.join("mail").join(format!("{}.jsonl", pct(conv))) }

/// `<home>/mail` as a real directory (made if absent); a link in its place is refused
fn dir(home: &Path) -> Result<PathBuf, StoreError> {
    let d = home.join("mail");
    match std::fs::symlink_metadata(&d) {
        Ok(m) if m.file_type().is_symlink() => return Err(StoreError::Link),
        Ok(m) if !m.is_dir() => return Err(StoreError::NotRegular),
        Ok(_) => {}
        Err(_) => { std::fs::create_dir_all(&d).map_err(|_| StoreError::Io)?; if std::fs::symlink_metadata(&d).map_or(true, |m| m.file_type().is_symlink() || !m.is_dir()) { return Err(StoreError::Link) } }
    }
    Ok(d)
}
/// open a journal without following a link; a regular file, and for a write exactly one link
pub fn open(p: &Path, write: bool) -> Result<std::fs::File, StoreError> {
    let mut o = OpenOptions::new(); o.read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    if write { o.append(true).create(true).mode(0o600); }
    let f = o.open(p).map_err(|e| match e.raw_os_error() { Some(libc::ELOOP) => StoreError::Link, Some(libc::EISDIR) => StoreError::NotRegular, _ => StoreError::Io })?;
    let m = f.metadata().map_err(|_| StoreError::Io)?;
    if !m.is_file() { return Err(StoreError::NotRegular) }
    if write && m.nlink() != 1 { return Err(StoreError::Hardlink) }
    Ok(f)
}

/// the bytes from the second-to-last line start (or the file start) to the end, where they start, and the offset just after the last newline: scans back in chunks
fn tail(f: &mut std::fs::File, size: u64) -> std::io::Result<(Vec<u8>, u64, u64)> {
    let mut buf: Vec<u8> = vec![]; let mut at = size; let chunk = 65536u64;
    loop {
        let from = at.saturating_sub(chunk); let mut part = vec![0u8; (at - from) as usize];
        f.seek(SeekFrom::Start(from))?; f.read_exact(&mut part)?; part.extend_from_slice(&buf); buf = part; at = from;
        let nl = buf.iter().filter(|b| **b == b'\n').count();
        if nl >= 2 || at == 0 { let after = buf.iter().rposition(|b| *b == b'\n').map_or(0, |p| p as u64 + 1) + at; return Ok((buf, at, after)) }
    }
}
/// Append a body to its conversation's file. Ok(true): an incomplete last line was cut off first.
pub fn append(home: &Path, b: &Body) -> Result<bool, StoreError> {
    let d = dir(home)?; let p = d.join(format!("{}.jsonl", pct(&b.conv.id)));
    let mut f = open(&p, true)?;
    let size = f.metadata().map_err(|_| StoreError::Io)?.len(); let mut repaired = false;
    if size > 0 {
        let (buf, off, cut) = tail(&mut f, size).map_err(|_| StoreError::Io)?;
        if cut != size { repaired = true }
        if cut > 0 { // the last complete line must be a valid body
            let end = (cut - off) as usize; let body = &buf[..end.saturating_sub(1)]; let start = body.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
            if body_parse(&String::from_utf8_lossy(&body[start..])).is_none() { return Err(StoreError::Malformed) }
        }
        if repaired { f.set_len(cut).map_err(|_| StoreError::Io)?; }
    }
    f.write_all(body_line(b).as_bytes()).map_err(|_| StoreError::Io)?; f.flush().map_err(|_| StoreError::Io)?;
    Ok(repaired)
}
/// The lines of a file that are text: a line that is not UTF-8 is skipped (the journal is the person's data; one bad line must not hide the rest)
pub fn text_lines(f: std::fs::File) -> impl Iterator<Item = String> {
    let mut r = std::io::BufReader::new(f); let mut buf = vec![];
    std::iter::from_fn(move || loop { buf.clear(); match std::io::BufRead::read_until(&mut r, b'\n', &mut buf) { Ok(0) | Err(_) => return None, Ok(_) => { if buf.last() == Some(&b'\n') { buf.pop(); } if let Ok(s) = String::from_utf8(buf.clone()) { return Some(s) } } } })
}
/// Every body of a file, in order; a malformed or incomplete line is skipped. (Whole bodies: for tests and small readers; start-up uses `headers`.)
pub fn read(path: &Path) -> Vec<Body> {
    let Ok(f) = open(path, false) else { return vec![] };
    text_lines(f).filter_map(|l| body_parse(&l)).collect()
}
/// The conversation and header of every body of a file, one line at a time: the text of a body is dropped as soon as its line is read
pub fn headers(path: &Path) -> Vec<(Conv, Hdr)> {
    let Ok(f) = open(path, false) else { return vec![] };
    text_lines(f).filter_map(|l| body_parse(&l)).map(|b| (b.conv, b.mail)).collect()
}

/// MailStore.read for a page: the bodies strictly before `before` (all when empty), newest first, at most `n`, and whether older ones remain.
/// One streaming pass over the journal that stops at the cursor, holding n+1 bodies; Err(None) when the cursor is not in the file.
pub fn page(path: &Path, before: &str, n: usize) -> Result<Option<(Vec<Body>, bool)>, PageError> {
    if n == 0 { return Ok(Some((vec![], false))) }
    let f = match open(path, false) { Ok(f) => f, Err(StoreError::Io) if !path.exists() => return Ok(None), Err(e) => return Err(PageError::Store(e)) };
    let mut ring: std::collections::VecDeque<Body> = std::collections::VecDeque::with_capacity(n + 2);
    let mut found = before.is_empty();
    for l in text_lines(f) {
        let Some(b) = body_parse(&l) else { continue };
        if !before.is_empty() && b.mail.id == before { found = true; break }
        ring.push_back(b); if ring.len() > n + 1 { ring.pop_front(); }
    }
    if !found { return Err(PageError::Cursor) }
    let more = ring.len() > n;
    Ok(Some((ring.into_iter().rev().take(n).collect(), more)))
}
#[derive(Debug, PartialEq)]
pub enum PageError { Cursor, Store(StoreError) }

/// The bodies of the named mail ids in a journal, by id, in one streaming pass that stops once all are found (MailStore.exact for a page)
pub fn exact(path: &Path, ids: &std::collections::HashSet<String>) -> std::collections::HashMap<String, Body> {
    let mut out = std::collections::HashMap::new();
    let Ok(f) = open(path, false) else { return out };
    for l in text_lines(f) { if let Some(b) = body_parse(&l) { if ids.contains(&b.mail.id) { out.insert(b.mail.id.clone(), b); if out.len() == ids.len() { break } } } }
    out
}
