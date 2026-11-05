//! events.jsonl, format unchanged: one JSON change per line. Whole lines only; a torn tail
//! (bytes after the last newline) is never a line and is closed off with a newline before the
//! next write, as the Bend hub does (evlog.js "tail's fix"). A write is fsync'd before it is
//! acknowledged.
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct Store { path: PathBuf, file: File, lines: Vec<String>, torn: bool, pub torn_bytes: usize, pub bad: usize }

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let mut bytes = Vec::new();
        if path.exists() { File::open(path).with_context(|| format!("open {}", path.display()))?.read_to_end(&mut bytes)?; }
        let whole = bytes.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
        let mut lines = Vec::new();
        let mut bad = 0;
        for l in bytes[..whole].split(|b| *b == b'\n') {
            if l.is_empty() { continue }
            let s = String::from_utf8_lossy(l).into_owned();
            if serde_json::from_str::<serde_json::Value>(&s).is_err() { bad += 1; continue }
            lines.push(s);
        }
        let torn_bytes = bytes.len() - whole;
        let file = OpenOptions::new().create(true).append(true).open(path).with_context(|| format!("append {}", path.display()))?;
        Ok(Store { path: path.to_path_buf(), file, lines, torn: torn_bytes > 0, torn_bytes, bad })
    }
    pub fn lines(&self) -> &[String] { &self.lines }
    pub fn len(&self) -> usize { self.lines.len() }
    pub fn path(&self) -> &Path { &self.path }
    /// Durable before it returns: the caller acknowledges only after this.
    pub fn append(&mut self, line: &str) -> Result<()> {
        debug_assert!(!line.contains('\n'));
        let mut buf = Vec::with_capacity(line.len() + 2);
        if self.torn { buf.push(b'\n'); }
        buf.extend(line.as_bytes()); buf.push(b'\n');
        self.file.write_all(&buf)?; self.file.sync_data()?;
        self.torn = false; self.lines.push(line.to_string());
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
    #[test] fn unicode_and_blank_lines() {
        let p = tmp("uni"); std::fs::write(&p, "{\"$\":\"M\",\"text\":\"日本語🙂\"}\n\n{\"$\":\"N\"}\n").unwrap();
        let s = Store::open(&p).unwrap(); assert_eq!(s.len(), 2); assert!(s.lines()[0].contains("日本語🙂"));
    }
}
