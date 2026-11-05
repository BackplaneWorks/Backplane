//! events.jsonl, format unchanged: one JSON change per line. Whole lines only; a torn tail
//! (bytes after the last newline) is never a line and is closed off with a newline before the
//! next write, as the Bend hub does (evlog.js "tail's fix"). A write is fsync'd before it is
//! acknowledged.
use anyhow::{Context, Result};
use serde::de::IgnoredAny;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// A lone surrogate escape (\\ud83d with no partner, a pasted half emoji) is not valid JSON text for a Rust
/// parser; the Bend hub reads it as U+FFFD (src/web/wire.js json()), and so does this: the line becomes valid.
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
            Some(_) => { out.push_str(&line[i..i + 2]); i += 2 }
            None => { out.push('\\'); i += 1 }
        }
    }
    if changed { std::borrow::Cow::Owned(out) } else { std::borrow::Cow::Borrowed(line) }
}

/// The log is never held as text: an index of (offset, length) per whole line, lines read back on demand.
pub struct Store { path: PathBuf, file: File, idx: Vec<(u64, u32)>, size: u64, torn: bool, pub torn_bytes: usize, pub bad: usize }

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let mut idx = Vec::new();
        let (mut size, mut bad, mut torn_bytes) = (0u64, 0usize, 0usize);
        if path.exists() {
            let mut r = BufReader::with_capacity(1 << 20, File::open(path).with_context(|| format!("open {}", path.display()))?);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                let n = r.read_until(b'\n', &mut buf)?;
                if n == 0 { break }
                if buf.last() != Some(&b'\n') { torn_bytes = n; size += n as u64; break } // after the last newline: not a line
                let body = &buf[..n - 1];
                if !body.is_empty() {
                    if serde_json::from_str::<IgnoredAny>(&clean(&String::from_utf8_lossy(body))).is_err() { bad += 1 } else { idx.push((size, body.len() as u32)) }
                }
                size += n as u64;
            }
        }
        let file = OpenOptions::new().create(true).read(true).append(true).open(path).with_context(|| format!("append {}", path.display()))?;
        Ok(Store { path: path.to_path_buf(), file, idx, size, torn: torn_bytes > 0, torn_bytes, bad })
    }
    pub fn len(&self) -> usize { self.idx.len() }
    pub fn path(&self) -> &Path { &self.path }
    pub fn get(&self, i: usize) -> Result<String> {
        let (off, len) = self.idx[i];
        let mut b = vec![0u8; len as usize]; self.file.read_exact_at(&mut b, off)?;
        Ok(clean(&String::from_utf8_lossy(&b)).into_owned())
    }
    /// Lines a..b (clamped), read in one pass over their byte range.
    pub fn range(&self, a: usize, b: usize) -> Result<Vec<String>> {
        let b = b.min(self.idx.len()); if a >= b { return Ok(vec![]) }
        let (start, last) = (self.idx[a].0, self.idx[b - 1]);
        let mut buf = vec![0u8; (last.0 + last.1 as u64 - start) as usize]; self.file.read_exact_at(&mut buf, start)?;
        Ok(self.idx[a..b].iter().map(|(off, len)| clean(&String::from_utf8_lossy(&buf[(off - start) as usize..(off - start) as usize + *len as usize])).into_owned()).collect())
    }
    pub fn all(&self) -> Vec<String> { self.range(0, self.len()).unwrap_or_default() }
    /// Durable before it returns: the caller acknowledges only after this.
    pub fn append(&mut self, line: &str) -> Result<()> {
        debug_assert!(!line.contains('\n'));
        let mut buf = Vec::with_capacity(line.len() + 2);
        if self.torn { buf.push(b'\n'); self.size += 1; }
        let off = self.size + 0;
        buf.extend(line.as_bytes()); buf.push(b'\n');
        self.file.write_all(&buf)?; self.file.sync_data()?;
        self.idx.push((off, line.len() as u32)); self.size += buf.len() as u64 - if self.torn { 1 } else { 0 };
        self.torn = false;
        Ok(())
    }
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
        let mut s = Store::open(&p).unwrap(); s.append("{\"$\":\"D\",\"at\":3}").unwrap();
        let again = Store::open(&p).unwrap(); assert_eq!(again.len(), 2); assert_eq!(again.bad, 1);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{\"$\":\"A\",\"at\":1}\n{\"$\":\"B\"\n{\"$\":\"D\",\"at\":3}\n");
    }
    #[test] fn appended_lines_survive_reopen() {
        let p = tmp("dur"); let mut s = Store::open(&p).unwrap(); for i in 0..50 { s.append(&format!("{{\"$\":\"X\",\"at\":{i}}}")).unwrap(); }
        assert_eq!(Store::open(&p).unwrap().len(), 50);
    }
    // law store_torn_tail_newline: a torn tail is closed with a newline; a log ending in one, or empty, needs none
    #[test] fn store_torn_tail_newline() {
        for (bytes, closes) in [(&b"{\""[..], true), (&b"{\n"[..], false), (&b""[..], false)] {
            let p = tmp(&format!("nl{}", bytes.len())); std::fs::write(&p, bytes).unwrap();
            let mut s = Store::open(&p).unwrap(); s.append("{\"$\":\"Z\"}").unwrap();
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
        let mut s = Store::open(&part).unwrap(); for y in &ys { s.append(y).unwrap(); }
        assert_eq!(Store::open(&part).unwrap().all(), Store::open(&whole).unwrap().all());
    }
    #[test] fn lone_surrogates_become_replacement_characters() {
        assert_eq!(clean(r#"{"a":"x\ud83dy"}"#), r#"{"a":"x\ufffdy"}"#);   // lone high
        assert_eq!(clean(r#"{"a":"\ude42"}"#), r#"{"a":"\ufffd"}"#);          // lone low
        assert_eq!(clean(r#"{"a":"\ud83d\ude42"}"#), r#"{"a":"\ud83d\ude42"}"#); // a pair stays
        assert_eq!(clean(r#"{"a":"\\ud83d"}"#), r#"{"a":"\\ud83d"}"#);        // an escaped backslash is not an escape
        assert!(serde_json::from_str::<serde_json::Value>(&clean(r#"{"a":"\ud83d\u00e9"}"#)).is_ok());
    }
    #[test] fn unicode_and_blank_lines() {
        let p = tmp("uni"); std::fs::write(&p, "{\"$\":\"M\",\"text\":\"日本語🙂\"}\n\n{\"$\":\"N\"}\n").unwrap();
        let s = Store::open(&p).unwrap(); assert_eq!(s.len(), 2); assert!(s.all()[0].contains("日本語🙂"));
    }
}
