//! The mail journal (mailstore.bend `MailStore.append`): one file per conversation, `<home>/mail/<pct(conversation)>.jsonl`, a body per line. An append
//! first looks at the end of the file: an incomplete last line (a crash mid-write) is cut off, a complete last line must be a valid body, and only then the new
//! body is added. Process-crash consistency only: no fsync, as in Bend.
use super::{body_line, body_parse, pct, Body};
use std::{fs::OpenOptions, io::{Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}};

pub fn path(home: &Path, conv: &str) -> PathBuf { home.join("mail").join(format!("{}.jsonl", pct(conv))) }

/// the bytes from the start of the last complete line (or of the file) to its end, and where the file's last newline is: scans back in chunks
fn tail(f: &mut std::fs::File, size: u64) -> std::io::Result<(Vec<u8>, u64, u64)> {
    // (bytes from the start of the second-to-last line start, offset of those bytes, offset just after the last newline)
    let mut buf: Vec<u8> = vec![]; let mut at = size; let chunk = 65536u64;
    loop {
        let from = at.saturating_sub(chunk); let mut part = vec![0u8; (at - from) as usize];
        f.seek(SeekFrom::Start(from))?; f.read_exact(&mut part)?; part.extend_from_slice(&buf); buf = part; at = from;
        let nl = buf.iter().filter(|b| **b == b'\n').count();
        if nl >= 2 || at == 0 { let after = buf.iter().rposition(|b| *b == b'\n').map_or(0, |p| p as u64 + 1) + at; return Ok((buf, at, after)) }
    }
}
/// Append a body to its conversation's file. Ok(true): an incomplete last line was cut off first.
pub fn append(home: &Path, b: &Body) -> Result<bool, String> {
    let p = path(home, &b.conv.id); let err = |e: std::io::Error| format!("{}: {e}", p.display());
    std::fs::create_dir_all(home.join("mail")).map_err(err)?;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = OpenOptions::new().read(true).append(true).create(true).mode(0o600).open(&p).map_err(err)?;
    let size = f.metadata().map_err(err)?.len(); let mut repaired = false;
    if size > 0 {
        let (buf, off, after) = tail(&mut f, size).map_err(err)?;
        let cut = after; // everything before `cut` is complete lines
        if cut != size { repaired = true }
        if cut > 0 { // the last complete line must be a valid body
            let end = (cut - off) as usize; let body = &buf[..end.saturating_sub(1)]; let start = body.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
            let line = String::from_utf8_lossy(&body[start..]).into_owned();
            if body_parse(&line).is_none() { return Err(format!("{}: malformed final completed body record", p.display())) }
        }
        if repaired { f.set_len(cut).map_err(err)?; }
    }
    f.write_all(body_line(b).as_bytes()).map_err(err)?; f.flush().map_err(err)?;
    Ok(repaired)
}
/// Every body of a file, in order; a malformed or incomplete line is skipped
pub fn read(path: &Path) -> Vec<Body> {
    let Ok(t) = std::fs::read_to_string(path) else { return vec![] };
    t.lines().filter_map(body_parse).collect()
}
