//! The mail journal (mailstore.bend `MailStore.append`, mailstore.js `mi_open`): one file per conversation, `<home>/mail/<pct(conversation)>.jsonl`, a body per line.
//! An append first looks at the end of the file: an incomplete last line (a crash mid-write) is cut off, a complete last line must be a valid body, and only then
//! the new body is added. Process-crash consistency only: no fsync, as in Bend. Files are opened without following links: the directory must be a real
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
fn open(p: &Path, write: bool) -> Result<std::fs::File, StoreError> {
    let mut o = OpenOptions::new(); o.read(true).custom_flags(libc::O_NOFOLLOW);
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
/// Every body of a file, in order; a malformed or incomplete line is skipped. (Whole bodies: for tests and small readers; start-up uses `headers`.)
pub fn read(path: &Path) -> Vec<Body> {
    let Ok(f) = open(path, false) else { return vec![] };
    std::io::BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| body_parse(&l)).collect()
}
/// The conversation and header of every body of a file, one line at a time: the text of a body is dropped as soon as its line is read
pub fn headers(path: &Path) -> Vec<(Conv, Hdr)> {
    let Ok(f) = open(path, false) else { return vec![] };
    std::io::BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| body_parse(&l)).map(|b| (b.conv, b.mail)).collect()
}
