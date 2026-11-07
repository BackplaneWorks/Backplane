//! events.jsonl, format unchanged: one JSON change per line. Whole lines only; a torn tail
//! (bytes after the last newline) is never a line and is closed off with a newline before the
//! next write, as hub does (evlog.js "tail's fix"). A write is fsync'd before it is
//! acknowledged.
use anyhow::{Context, Result};
use serde::de::IgnoredAny;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// A lone surrogate escape (\\ud83d with no partner, a pasted half emoji) is not valid JSON text for a Rust
/// parser; hub reads it as U+FFFD (src/web/wire.js json()), and so does this: the line becomes valid.
pub fn clean(line: &str) -> std::borrow::Cow<'_, str> {
    if !line.contains("\\u") { return std::borrow::Cow::Borrowed(line) }
    let b = line.as_bytes();
    let hex4 = |i: usize| -> Option<u32> { b.get(i..i + 4).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u32::from_str_radix(h, 16).ok()) };
    let mut out = String::with_capacity(line.len());
    let (mut i, mut changed) = (0, false);
    while i < b.len() {
        if b[i] != b'\\' { let n = line[i..].chars().next().unwrap().len_utf8(); out.push_str(&line[i..i + n]); i += n; continue }
        match b.get(i + 1) {
            Some(b'u') => match hex4(i + 2) {
                Some(h) if (0xD800..0xDC00).contains(&h) => {
                    if b.get(i + 6) == Some(&b'\\') && b.get(i + 7) == Some(&b'u') && hex4(i + 8).map_or(false, |l| (0xDC00..0xE000).contains(&l)) { out.push_str(&line[i..i + 12]); i += 12 }
                    else { out.push_str("\\ufffd"); i += 6; changed = true }
                }
                Some(h) if (0xDC00..0xE000).contains(&h) => { out.push_str("\\ufffd"); i += 6; changed = true }
                _ => { out.push_str(&line[i..(i + 2).min(line.len())]); i += 2 }
            },
            // (a backslash before a character of several bytes is not valid JSON, and the line is about to be refused for it, not panicked on)
            Some(_) => { let n = 1 + line[i + 1..].chars().next().map_or(0, char::len_utf8); out.push_str(&line[i..i + n]); i += n }
            None => { out.push('\\'); i += 1 }
        }
    }
    if changed { std::borrow::Cow::Owned(out) } else { std::borrow::Cow::Borrowed(line) }
}

fn quarantine_dir(log: &Path) -> Option<PathBuf> { log.parent().map(|d| d.join("quarantine")) }
/// The lines earlier starts quarantined.
fn held_lines(log: &Path) -> std::collections::HashSet<usize> {
    let Some(q) = quarantine_dir(log) else { return Default::default() };
    std::fs::read_to_string(q.join("lines.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).filter_map(|v| v["line"].as_u64()).filter(|n| *n > 0).map(|n| n as usize - 1).collect()
}
/// A line that is not JSON: it stays in the log, skipped as it always was, and is now also written down, once, with where it is, so what it held can be looked at.
fn note_bad(log: &Path, at: u64, body: &[u8]) {
    let Some(q) = quarantine_dir(log) else { return };
    let f = q.join("bad-lines.jsonl");
    if std::fs::read_to_string(&f).unwrap_or_default().contains(&format!("\"offset\":{at},")) { return }
    let _ = std::fs::create_dir_all(&q);
    let head: String = String::from_utf8_lossy(&body[..body.len().min(4096)]).into_owned();
    if let Ok(mut w) = OpenOptions::new().create(true).append(true).open(&f) { let _ = writeln!(w, "{}", serde_json::json!({"offset": at, "len": body.len(), "head": head})); }
    eprintln!("store: a line at byte {at} of the log is not JSON: skipped, noted in quarantine/bad-lines.jsonl");
}

/// The log is never held as text: an index of (offset, length) per whole line, lines read back on demand.
/// Cheap to clone and safe to share: readers (a join, a page) take a brief read lock on the index and `pread`
/// the bytes; they never wait for a writer's fsync.
#[derive(Clone)]
pub struct Store { inner: std::sync::Arc<Inner>, pub torn_bytes: usize, pub bad: usize }
struct Inner { path: PathBuf, read: File, w: std::sync::Mutex<W>, idx: std::sync::RwLock<Vec<(u64, u32)>>, held: std::sync::RwLock<std::collections::HashSet<usize>> }
/// What a quarantined line reads as: an event no fold knows, so every fold passes over it. The log itself is never changed: the line keeps its place and its bytes.
pub const QUARANTINED: &str = "{\"$\":\"Quarantined\"}";
/// Tells the person a line was quarantined (set by guard.rs in the hub).
pub static ON_QUARANTINE: std::sync::OnceLock<fn(usize)> = std::sync::OnceLock::new();
/// Lines read and folded so far: while it moves, a long start is working, not hung (the guard reads the age of `guard/phase`, which guard.rs keeps fresh from this).
pub static PROGRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// (`BACKPLANE_FAULT=panic-line:<text>`, for test/guard/faults.sh: a fold panics on a line holding the text.)
fn fault_line() -> Option<&'static str> { static F: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new(); F.get_or_init(|| std::env::var("BACKPLANE_FAULT").ok().and_then(|v| v.strip_prefix("panic-line:").map(str::to_string))).as_deref() }
struct W { file: File, size: u64, torn: bool }

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        // the saved index (ckpt/index.bin) covers the first part of the log; only the bytes past it are scanned
        let (mut idx, mut size, mut bad) = Self::load_index(path).unwrap_or_default();
        let mut torn_bytes = 0usize;
        if path.exists() {
            let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
            { use std::io::Seek; f.seek(std::io::SeekFrom::Start(size))?; }
            let mut r = BufReader::with_capacity(1 << 20, f);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                let n = r.read_until(b'\n', &mut buf)?;
                if n == 0 { break }
                if buf.last() != Some(&b'\n') { torn_bytes = n; size += n as u64; break } // after the last newline: not a line
                let body = &buf[..n - 1];
                PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if !body.is_empty() {
                    if body.len() > u32::MAX as usize || serde_json::from_str::<IgnoredAny>(&clean(&String::from_utf8_lossy(body))).is_err() { bad += 1; note_bad(path, size, body); } else { idx.push((size, body.len() as u32)) }
                }
                size += n as u64;
            }
        }
        let file = OpenOptions::new().create(true).read(true).append(true).open(path).with_context(|| format!("append {}", path.display()))?;
        let read = File::open(path)?;
        let held = held_lines(path);
        if !held.is_empty() { eprintln!("store: {} log line(s) are quarantined (see quarantine/ in the home)", held.len()); }
        Ok(Store { inner: std::sync::Arc::new(Inner { path: path.to_path_buf(), read, w: std::sync::Mutex::new(W { file, size, torn: torn_bytes > 0 }), idx: std::sync::RwLock::new(idx), held: std::sync::RwLock::new(held) }), torn_bytes, bad })
    }
    /// Run a fold over line `at` (0-based). A line that panics its fold is quarantined and the fold goes on: one bad record must not stop every start from here on, and this holds for
    /// every fold of the log, whichever thread it is on.
    pub fn fold(&self, at: usize, line: &str, f: impl FnOnce(&str)) {
        PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { if fault_line().map_or(false, |t| line.contains(t)) { panic!("BACKPLANE_FAULT: a fold panicked on line {}", at + 1) } f(line) }));
        if r.is_err() { self.quarantine(at, line); }
    }
    /// From now on this line reads as `QUARANTINED`, here and in every later start. A copy of it goes to `quarantine/lines.jsonl`; the log keeps the original.
    pub fn quarantine(&self, at: usize, line: &str) {
        if !self.inner.held.write().unwrap_or_else(|e| e.into_inner()).insert(at) { return }
        eprintln!("store: line {} of the log broke a fold: quarantined, the fold goes on", at + 1);
        let Some(q) = quarantine_dir(&self.inner.path) else { return };
        let _ = std::fs::create_dir_all(&q);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(q.join("lines.jsonl")) { let _ = writeln!(f, "{}", serde_json::json!({"line": at + 1, "text": line})); }
        if let Some(f) = ON_QUARANTINE.get() { f(at + 1) }
    }
    fn is_held(&self, i: usize) -> bool { let h = self.inner.held.read().unwrap_or_else(|e| e.into_inner()); !h.is_empty() && h.contains(&i) }

    fn index_path(log: &Path) -> PathBuf { crate::ckpt::dir(log).join("index.bin") }
    /// The saved index, when it is the index of this log's first bytes: its head and the bytes it ended on hash as they did, and the log is not shorter.
    /// File: one JSON header line, then the offsets (u64 LE) and the lengths (u32 LE) of every line.
    fn load_index(log: &Path) -> Option<(Vec<(u64, u32)>, u64, usize)> {
        let raw = std::fs::read(Self::index_path(log)).ok()?;
        let nl = raw.iter().position(|b| *b == b'\n')?;
        let h: serde_json::Value = serde_json::from_slice(&raw[..nl]).ok()?;
        let (lines, scanned, bad) = (h["lines"].as_u64()? as usize, h["scanned"].as_u64()?, h["bad"].as_u64()? as usize);
        if h["v"].as_u64()? != crate::ckpt::VERSION { return None }
        let body = &raw[nl + 1..];
        if Some(body.len()) != lines.checked_mul(12) { return None }
        // a bit flipped in the middle of the file would point every read at the wrong bytes; files from before the sum still have to pass the walk below
        if let Some(sum) = h["sum"].as_u64() { if crate::ckpt::fnv(body) != sum { return None } }
        let f = File::open(log).ok()?;
        if f.metadata().ok()?.len() < scanned { return None }
        let take = |at: u64, n: usize| -> Option<Vec<u8>> { let mut b = vec![0u8; n]; f.read_exact_at(&mut b, at).ok()?; Some(b) };
        let k = scanned.min(4096) as usize;
        if crate::ckpt::fnv(&take(0, k)?) != h["head"].as_u64()? || crate::ckpt::fnv(&take(scanned - k as u64, k)?) != h["tail"].as_u64()? { return None }
        let (offs, lens) = body.split_at(lines * 8);
        let idx: Vec<(u64, u32)> = (0..lines).map(|i| (crate::ckpt::le64(offs, i * 8), crate::ckpt::le32(lens, i * 4))).collect();
        let mut end = 0u64;
        for (o, l) in &idx { if *o < end || o.checked_add(*l as u64 + 1).map_or(true, |e| e > scanned) { return None } end = o + *l as u64 + 1; }
        Some((idx, scanned, bad))
    }
    /// Save the index of the log as it is now (nothing when a torn tail waits for the next write). Appends wait only while the index is copied.
    pub fn save_index(&self) -> Result<usize> {
        let (idx, size) = {
            let w = self.inner.w.lock().unwrap();
            if w.torn { return Ok(0) }
            (self.inner.idx.read().unwrap().clone(), w.size)
        };
        let k = size.min(4096) as usize;
        let take = |at: u64| -> Result<Vec<u8>> { let mut b = vec![0u8; k]; self.inner.read.read_exact_at(&mut b, at)?; Ok(b) };
        let (head, tail) = (crate::ckpt::fnv(&take(0)?), crate::ckpt::fnv(&take(size - k as u64)?));
        let bad = self.bad;
        let (mut offs, mut lens) = (Vec::with_capacity(idx.len() * 8), Vec::with_capacity(idx.len() * 4));
        for (o, l) in &idx { offs.extend(o.to_le_bytes()); lens.extend(l.to_le_bytes()); }
        let sum = crate::ckpt::fnv(&[&offs[..], &lens[..]].concat());
        let header = format!("{{\"v\":{},\"lines\":{},\"scanned\":{},\"bad\":{},\"head\":{},\"tail\":{},\"sum\":{}}}\n", crate::ckpt::VERSION, idx.len(), size, bad, head, tail, sum);
        crate::ckpt::write_atomic(&Self::index_path(&self.inner.path), &[header.as_bytes(), &offs, &lens])?;
        Ok(idx.len())
    }
    pub fn len(&self) -> usize { self.inner.idx.read().unwrap().len() }
    pub fn path(&self) -> &Path { &self.inner.path }
    pub fn get(&self, i: usize) -> Result<String> {
        let (off, len) = *self.inner.idx.read().unwrap().get(i).ok_or_else(|| anyhow::anyhow!("no line {i}"))?;
        if self.is_held(i) { return Ok(QUARANTINED.to_string()) }
        let mut b = vec![0u8; len as usize]; self.inner.read.read_exact_at(&mut b, off)?;
        Ok(clean(&String::from_utf8_lossy(&b)).into_owned())
    }
    /// Lines a..b (clamped), read in one pass over their byte range.
    pub fn range(&self, a: usize, b: usize) -> Result<Vec<String>> {
        let spans: Vec<(u64, u32)> = { let idx = self.inner.idx.read().unwrap(); let b = b.min(idx.len()); if a >= b { return Ok(vec![]) } idx[a..b].to_vec() };
        let (start, last) = (spans[0].0, *spans.last().unwrap());
        let mut buf = vec![0u8; (last.0 + last.1 as u64 - start) as usize]; self.inner.read.read_exact_at(&mut buf, start)?;
        Ok(spans.iter().enumerate().map(|(k, (off, len))| if self.is_held(a + k) { QUARANTINED.to_string() } else { clean(&String::from_utf8_lossy(&buf[(off - start) as usize..(off - start) as usize + *len as usize])).into_owned() }).collect())
    }
    pub fn all(&self) -> Vec<String> { self.range(0, self.len()).unwrap_or_default() }
    /// Durable before it returns; blocking (an fsync): call it from a blocking thread. Returns the index of the
    /// first line appended. One writer at a time; readers are not held up.
    pub fn append_many(&self, lines: &[&str]) -> Result<usize> {
        let mut w = self.inner.w.lock().unwrap();
        let mut buf = Vec::with_capacity(lines.iter().map(|l| l.len() + 1).sum::<usize>() + 1);
        if w.torn { buf.push(b'\n'); }
        let mut at = w.size + buf.len() as u64;
        let mut spans = Vec::with_capacity(lines.len());
        for l in lines { debug_assert!(!l.contains('\n')); spans.push((at, l.len() as u32)); buf.extend(l.as_bytes()); buf.push(b'\n'); at += l.len() as u64 + 1; }
        // A write that fails part way (ENOSPC, EFBIG, EIO) leaves a fragment: cut the file back to the last whole line before answering, or the
        // next append would glue onto the fragment and lose an acknowledged write. If even the cut fails, the next append starts with a newline.
        let wrote = w.file.write_all(&buf).and_then(|_| w.file.sync_data());
        if let Err(e) = wrote {
            let size = w.size;
            if w.file.set_len(size).is_err() { w.torn = true; }
            return Err(e.into());
        }
        w.size += buf.len() as u64; w.torn = false;
        let mut idx = self.inner.idx.write().unwrap(); let first = idx.len(); idx.extend(spans);
        Ok(first)
    }
    pub fn append(&self, line: &str) -> Result<()> { self.append_many(&[line]).map(|_| ()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-store-{}-{}", std::process::id(), name)); let _ = std::fs::remove_file(&p); p }
    #[test] fn reads_whole_lines_and_ignores_a_torn_tail() {
        let p = tmp("torn"); std::fs::write(&p, "{\"$\":\"A\",\"at\":1}\n{\"$\":\"B\",\"at\":2}\n{\"$\":\"C\",\"a").unwrap();
        let s = Store::open(&p).unwrap(); assert_eq!(s.len(), 2); assert!(s.torn_bytes > 0);
    }
    #[test] fn a_torn_tail_is_closed_off_before_the_next_write() {
        let p = tmp("close"); std::fs::write(&p, "{\"$\":\"A\",\"at\":1}\n{\"$\":\"B\"").unwrap();
        let s = Store::open(&p).unwrap(); s.append("{\"$\":\"D\",\"at\":3}").unwrap();
        let again = Store::open(&p).unwrap(); assert_eq!(again.len(), 2); assert_eq!(again.bad, 1);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{\"$\":\"A\",\"at\":1}\n{\"$\":\"B\"\n{\"$\":\"D\",\"at\":3}\n");
    }
    // a backslash before a multi-byte character used to panic here, in the first scan of the log, on every start
    #[test] fn a_corrupt_line_cannot_panic_the_scan() {
        for bad in ["\\é \\u0041", "\\", "\\u", "\\ud83d\\", "\\ud83dé", "é\\", "\\🙂\\u0041"] { let _ = clean(bad); }
        let p = tmp("corrupt"); std::fs::write(&p, "{\"$\":\"A\",\"at\":1}\n{\"x\":\"\\é \\u0041\"}\n\0\0\0\0\n{\"$\":\"B\",\"at\":2}\n").unwrap();
        let s = Store::open(&p).unwrap(); assert_eq!(s.len(), 2, "the good lines"); assert_eq!(s.bad, 2);
    }
    #[test] fn a_flipped_index_is_not_trusted() {
        let d = std::env::temp_dir().join(format!("bp-store-idx-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        let p = d.join("events.jsonl"); let s = Store::open(&p).unwrap(); for i in 0..200 { s.append(&format!("{{\"$\":\"X\",\"at\":{i}}}")).unwrap(); } s.save_index().unwrap();
        let f = crate::ckpt::dir(&p).join("index.bin"); let mut b = std::fs::read(&f).unwrap(); let n = b.len(); b[n - 900] ^= 0x40; std::fs::write(&f, b).unwrap();
        let again = Store::open(&p).unwrap(); assert_eq!(again.len(), 200); assert_eq!(again.get(150).unwrap(), "{\"$\":\"X\",\"at\":150}", "scanned from the log itself");
        std::fs::write(&f, "not an index at all").unwrap(); assert_eq!(Store::open(&p).unwrap().len(), 200);
    }
    #[test] fn a_line_that_breaks_a_fold_is_quarantined_for_good_and_the_log_keeps_it() {
        let d = std::env::temp_dir().join(format!("bp-store-q-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        let p = d.join("events.jsonl"); std::fs::write(&p, "{\"$\":\"A\"}\n{\"$\":\"Poison\"}\n{\"$\":\"C\"}\n").unwrap();
        let before = std::fs::read(&p).unwrap();
        let s = Store::open(&p).unwrap(); let mut seen = vec![];
        for (i, l) in s.range(0, 3).unwrap().iter().enumerate() { s.fold(i, l, |l| { if l.contains("Poison") { panic!("no") } seen.push(l.to_string()) }); }
        assert_eq!(seen.len(), 2, "the fold went on past it");
        assert_eq!(s.get(1).unwrap(), QUARANTINED); assert_eq!(s.range(0, 3).unwrap()[1], QUARANTINED);
        let again = Store::open(&p).unwrap(); assert_eq!(again.len(), 3); assert_eq!(again.get(1).unwrap(), QUARANTINED, "a later start knows it");
        assert_eq!(std::fs::read(&p).unwrap(), before, "not a byte of the log moved");
        assert!(std::fs::read_to_string(d.join("quarantine/lines.jsonl")).unwrap().contains("Poison"), "and a copy is kept");
    }
    #[test] fn appended_lines_survive_reopen() {
        let p = tmp("dur"); let s = Store::open(&p).unwrap(); for i in 0..50 { s.append(&format!("{{\"$\":\"X\",\"at\":{i}}}")).unwrap(); }
        assert_eq!(Store::open(&p).unwrap().len(), 50);
    }
    // law store_torn_tail_newline: a torn tail is closed with a newline; a log ending in one, or empty, needs none
    #[test] fn store_torn_tail_newline() {
        for (bytes, closes) in [(&b"{\""[..], true), (&b"{\n"[..], false), (&b""[..], false)] {
            let p = tmp(&format!("nl{}", bytes.len())); std::fs::write(&p, bytes).unwrap();
            let s = Store::open(&p).unwrap(); s.append("{\"$\":\"Z\"}").unwrap();
            let text = String::from_utf8(std::fs::read(&p).unwrap()).unwrap();
            assert_eq!(text.starts_with(std::str::from_utf8(bytes).unwrap()) && text[bytes.len()..].starts_with('\n') && closes || !closes, true);
            assert_eq!(text.ends_with("{\"$\":\"Z\"}\n"), true);
        }
    }
    // law store_torn_tail_resyncs: the lines after a torn one read as they would without it
    #[test] fn store_torn_tail_resyncs() {
        let rest = ["{\"$\":\"A\",\"at\":1}", "{\"$\":\"B\",\"at\":2}"];
        for torn in ["{\"$\":\"T\",\"a", "{", "{\"$\""] {
            let (p1, p2) = (tmp("rs1"), tmp("rs2"));
            std::fs::write(&p1, format!("{torn}\n{}\n{}\n", rest[0], rest[1])).unwrap(); std::fs::write(&p2, format!("{}\n{}\n", rest[0], rest[1])).unwrap();
            assert_eq!(Store::open(&p1).unwrap().all(), Store::open(&p2).unwrap().all());
        }
    }
    // law store_load_append: a log read in two parts is the log read whole
    #[test] fn store_load_append() {
        let xs: Vec<String> = (0..7).map(|i| format!("{{\"$\":\"X\",\"at\":{i}}}")).collect();
        let ys: Vec<String> = (7..12).map(|i| format!("{{\"$\":\"Y\",\"at\":{i}}}")).collect();
        let whole = tmp("la-whole"); std::fs::write(&whole, xs.iter().chain(&ys).map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        let part = tmp("la-part"); std::fs::write(&part, xs.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        let s = Store::open(&part).unwrap(); for y in &ys { s.append(y).unwrap(); }
        assert_eq!(Store::open(&part).unwrap().all(), Store::open(&whole).unwrap().all());
    }
    fn tmp_dir(name: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-store-ck-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    // the saved index is the index: same lines, and only the bytes past it are scanned
    #[test] fn a_saved_index_is_the_index_and_the_tail_past_it_is_scanned() {
        let d = tmp_dir("idx"); let p = d.join("events.jsonl");
        std::fs::write(&p, "{\"$\":\"A\",\"at\":1}\nnot json\n{\"$\":\"B\",\"at\":2}\n").unwrap();
        let s = Store::open(&p).unwrap(); assert_eq!((s.len(), s.bad), (2, 1)); assert_eq!(s.save_index().unwrap(), 2);
        { use std::io::Write; let mut f = OpenOptions::new().append(true).open(&p).unwrap(); f.write_all(b"{\"$\":\"C\",\"at\":3}\n{\"$\":\"D\"").unwrap(); } // a line, and a torn one
        let again = Store::open(&p).unwrap();
        assert_eq!((again.len(), again.bad), (3, 1), "the saved index is used and the tail added"); assert!(again.torn_bytes > 0);
        assert_eq!(again.all(), vec!["{\"$\":\"A\",\"at\":1}", "{\"$\":\"B\",\"at\":2}", "{\"$\":\"C\",\"at\":3}"]);
        std::fs::remove_file(crate::ckpt::dir(&p).join("index.bin")).unwrap();
        assert_eq!(Store::open(&p).unwrap().all(), again.all(), "with no index file: the same by scanning");
    }
    // an index of another log, or of a log that changed in what it covers, is not used
    #[test] fn a_saved_index_of_another_log_is_ignored() {
        let d = tmp_dir("other"); let p = d.join("events.jsonl");
        std::fs::write(&p, "{\"$\":\"A\",\"at\":1}\n{\"$\":\"B\",\"at\":2}\n").unwrap();
        Store::open(&p).unwrap().save_index().unwrap();
        std::fs::write(&p, "{\"$\":\"X\",\"at\":9}\n{\"$\":\"Y\",\"at\":8}\n{\"$\":\"Z\",\"at\":7}\n").unwrap(); // longer, different
        assert_eq!(Store::open(&p).unwrap().all(), vec!["{\"$\":\"X\",\"at\":9}", "{\"$\":\"Y\",\"at\":8}", "{\"$\":\"Z\",\"at\":7}"]);
        std::fs::write(&p, "{\"$\":\"A\"}\n").unwrap(); // shorter than what the index covers
        assert_eq!(Store::open(&p).unwrap().all(), vec!["{\"$\":\"A\"}"]);
        let ix = crate::ckpt::dir(&p).join("index.bin"); std::fs::write(&ix, b"garbage").unwrap();
        assert_eq!(Store::open(&p).unwrap().len(), 1, "a damaged index file is ignored");
    }
    #[test] fn lone_surrogates_become_replacement_characters() {
        assert_eq!(clean(r#"{"a":"x\ud83dy"}"#), r#"{"a":"x\ufffdy"}"#);   // lone high
        assert_eq!(clean(r#"{"a":"\ude42"}"#), r#"{"a":"\ufffd"}"#);          // lone low
        assert_eq!(clean(r#"{"a":"\ud83d\ude42"}"#), r#"{"a":"\ud83d\ude42"}"#); // a pair stays
        assert_eq!(clean(r#"{"a":"\\ud83d"}"#), r#"{"a":"\\ud83d"}"#);        // an escaped backslash is not an escape
        assert!(serde_json::from_str::<serde_json::Value>(&clean(r#"{"a":"\ud83d\u00e9"}"#)).is_ok());
    }
    // law outbox_fifo: a log goes oldest first from where the client was (OB.Join.from(["d","c","b","a"], 4, 1) == ["b","c","d"])
    #[test] fn outbox_fifo() {
        let p = tmp("fifo"); std::fs::write(&p, "{\"$\":\"a\"}\n{\"$\":\"b\"}\n{\"$\":\"c\"}\n{\"$\":\"d\"}\n").unwrap();
        let s = Store::open(&p).unwrap();
        assert_eq!(s.range(1, 4).unwrap(), vec!["{\"$\":\"b\"}", "{\"$\":\"c\"}", "{\"$\":\"d\"}"]);
        assert_eq!(s.range(4, 9).unwrap(), Vec::<String>::new());
    }
    #[test] fn unicode_and_blank_lines() {
        let p = tmp("uni"); std::fs::write(&p, "{\"$\":\"M\",\"text\":\"日本語🙂\"}\n\n{\"$\":\"N\"}\n").unwrap();
        let s = Store::open(&p).unwrap(); assert_eq!(s.len(), 2); assert!(s.all()[0].contains("日本語🙂"));
    }

    // A write that fails part way must not lose the next acknowledged write (soak finding: ENOSPC glued the next line onto a fragment).
    // RLIMIT_FSIZE makes write_all fail after a partial write; it is process-wide, so the test runs in a child process.
    #[test] fn a_failed_partial_write_is_cut_back_and_the_next_append_is_whole() {
        if std::env::var("BP_STORE_FSIZE_CHILD").is_err() {
            let o = std::process::Command::new(std::env::current_exe().unwrap()).args(["store::tests::a_failed_partial_write_is_cut_back_and_the_next_append_is_whole", "--exact", "--nocapture", "--test-threads=1"])
                .env("BP_STORE_FSIZE_CHILD", "1").output().unwrap();
            assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            return
        }
        unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_IGN); }
        let p = tmp("fsize");
        let s = Store::open(&p).unwrap();
        s.append_many(&["{\"$\":\"A\",\"at\":1}", "{\"$\":\"B\",\"at\":2}"]).unwrap();
        let size = std::fs::metadata(&p).unwrap().len();
        let lim = libc::rlimit { rlim_cur: size + 40, rlim_max: libc::RLIM_INFINITY };
        unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &lim); }
        let big = format!("{{\"$\":\"C\",\"at\":3,\"text\":\"{}\"}}", "x".repeat(500));
        assert!(s.append_many(&[big.as_str()]).is_err(), "the write fails part way");
        assert_eq!(s.len(), 2, "the failed append is not indexed");
        let none = libc::rlimit { rlim_cur: libc::RLIM_INFINITY, rlim_max: libc::RLIM_INFINITY };
        unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &none); }
        s.append_many(&["{\"$\":\"D\",\"at\":4}"]).unwrap();
        assert_eq!(s.len(), 3);
        let again = Store::open(&p).unwrap();
        assert_eq!(again.len(), 3, "the reopened log holds A, B, D");
        assert_eq!((again.bad, again.torn_bytes), (0, 0), "no bad line, no fragment");
        assert_eq!(again.get(2).unwrap(), "{\"$\":\"D\",\"at\":4}", "the acknowledged write is whole");
    }
}
