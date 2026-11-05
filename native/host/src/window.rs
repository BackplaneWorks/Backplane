//! A thread page's `window` object, computed by native Bend (native/bend/window.bend over
//! src/server/thin-window.bend, equal to the JS hub's on the six reference threads of
//! test/gpui/hub/fixtures/windows.json). Rust reads the page out of its own index and hands over rows and counts;
//! the fold/context logic (60+ laws) stays in Bend. One process per request for now.
use crate::model::{Page, Row};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static N: AtomicU64 = AtomicU64::new(0);

fn esc(s: &str, out: &mut String) { for c in s.chars() { match c { '\\' => out.push_str("\\\\"), '\t' => out.push_str("\\t"), '\n' => out.push_str("\\n"), c => out.push(c) } } }

/// The request file window.bend reads.
pub fn request_text(thread: &str, epoch: &str, before: u64, page: &Page, ctx: &[Row], p: &Value) -> String {
    let mut t = format!("window\t{thread}\t{epoch}\t{before}\t{}\t{}\t{}\t{}\n", page.below, page.above, ctx.len(), p);
    for r in ctx.iter().chain(page.rows.iter()) {
        for (i, c) in [r.seq.to_string(), r.ord.to_string(), r.id.clone(), r.thread.clone(), r.src.clone(), r.kind.to_string(), r.role.clone(), r.akind.clone(), r.text.clone(), r.at.to_string()].iter().enumerate() {
            if i > 0 { t.push('\t') } esc(c, &mut t);
        }
        t.push('\n');
    }
    t
}

pub async fn compute(bin: &Path, tmp: &Path, thread: &str, epoch: &str, before: u64, page: &Page, ctx: &[Row], p: &Value) -> Option<Value> {
    let file: PathBuf = tmp.join(format!("win-{}-{}", std::process::id(), N.fetch_add(1, Relaxed)));
    tokio::fs::write(&file, request_text(thread, epoch, before, page, ctx, p)).await.ok()?;
    let out = crate::proc::tokio_cmd(crate::proc::Kind::Local, bin).arg(&file).output().await.ok();
    let _ = tokio::fs::remove_file(&file).await;
    serde_json::from_slice(&out?.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_request_file_escapes_cells_and_carries_the_counts() {
        let row = Row { seq: 5, ord: 2, id: "m\t1".into(), thread: "t1".into(), src: String::new(), kind: "msg", role: "user".into(), akind: String::new(), text: "a\nb\\c".into(), at: 9 };
        let t = request_text("t1", "77", 0, &Page { ids: vec![], below: 3, above: 0, rows: vec![row] }, &[], &serde_json::json!({"target":{"kind":"thread","id":"t1"}}));
        let mut ls = t.lines();
        assert_eq!(ls.next().unwrap(), "window\tt1\t77\t0\t3\t0\t0\t{\"target\":{\"kind\":\"thread\",\"id\":\"t1\"}}");
        assert_eq!(ls.next().unwrap(), "5\t2\tm\\t1\tt1\t\tmsg\tuser\t\ta\\nb\\\\c\t9");
    }
}
