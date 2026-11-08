//! A terminal's output as a stream with offsets, for clients that come and go (phones; docs/PHONE-DATA.md, Terminals).
//! Each run of a terminal (`gen`: a new process, or a screen read back from disk) counts the bytes it printed (`off`) and keeps the newest RING of
//! them. A client holds a screen up to an offset; asked again with `from` = that offset and the same `gen`, the hub sends only what came after,
//! else (another run, or older than the ring) a snapshot of the screen at `off`. Every live frame says where its bytes start (`o`), so a client
//! takes from each frame only what it does not hold yet and asks again when one starts past what it holds (`merge`).
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bytes of output each terminal keeps for resuming (64 terminals at most: 8 MB in all).
pub const RING: usize = 131072;
static GEN: AtomicU64 = AtomicU64::new(0);

/// A new run's name: the clock in ms, never the same twice in one hub (nor across restarts, as the clock moves on).
pub fn gen() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let mut cur = GEN.load(Ordering::Relaxed);
    loop { let next = now.max(cur + 1); match GEN.compare_exchange(cur, next, Ordering::Relaxed, Ordering::Relaxed) { Ok(_) => return next, Err(c) => cur = c } }
}
/// Keep the newest RING bytes.
pub fn keep(ring: &mut VecDeque<u8>, bs: &[u8]) {
    if bs.len() >= RING { ring.clear(); ring.extend(&bs[bs.len() - RING..]); return }
    let over = (ring.len() + bs.len()).saturating_sub(RING); ring.drain(..over); ring.extend(bs);
}
/// What the hub answers a client that holds run `g` up to `from` (None: it holds nothing): the bytes after `from`, or None when only a snapshot will do.
pub fn since(gen: u64, off: u64, ring: &VecDeque<u8>, g: u64, from: Option<u64>) -> Option<Vec<u8>> {
    let from = from?;
    if g != gen || from > off || off - from > ring.len() as u64 { return None }
    let skip = ring.len() - (off - from) as usize;
    Some(ring.iter().skip(skip).copied().collect())
}
/// A state read back from disk: a command that had ended keeps its code (its shell went with the hub); what was still going is stopped.
pub fn read_back(state: &str) -> String { match state.strip_prefix("done ") { Some(c) => format!("exited {c}"), None if state == "running" || state.is_empty() => "stopped".into(), None => state.into() } }

/// What a client does with a live frame of the run it holds, whose bytes start at `o` and are `len` long, while it holds up to `held`.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Merge {
    /// it holds all of it already
    Skip,
    /// feed the frame's bytes from this index on; it then holds up to `o + len`
    Take(usize),
    /// bytes between are missing: ask `term.read {from: held}` (or a snapshot)
    Gap,
}
pub fn merge(held: u64, o: u64, len: usize) -> Merge {
    let end = o + len as u64;
    if o > held { Merge::Gap } else if end <= held { Merge::Skip } else { Merge::Take((held - o) as usize) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_ring_keeps_the_newest_bytes() {
        let mut r = VecDeque::new(); keep(&mut r, b"abc"); keep(&mut r, &vec![b'x'; RING - 1]); assert_eq!(r.len(), RING); assert_eq!(r[0], b'c');
        keep(&mut r, &vec![b'y'; RING + 5]); assert_eq!(r.len(), RING); assert!(r.iter().all(|&b| b == b'y'));
    }
    #[test] fn a_client_resumes_from_its_offset_or_gets_a_snapshot() {
        let mut r = VecDeque::new(); keep(&mut r, b"hello world"); let off = 11;
        assert_eq!(since(5, off, &r, 5, Some(6)).as_deref(), Some(&b"world"[..]));
        assert_eq!(since(5, off, &r, 5, Some(11)).as_deref(), Some(&b""[..]), "nothing new");
        assert_eq!(since(5, off, &r, 4, Some(6)), None, "another run");
        assert_eq!(since(5, off, &r, 5, Some(12)), None, "an offset this run never reached");
        assert_eq!(since(5, off, &r, 5, None), None, "holds nothing");
        // older than the ring
        let mut big = VecDeque::new(); keep(&mut big, &vec![1u8; RING]); keep(&mut big, b"zz"); let off = RING as u64 + 2;
        assert_eq!(since(1, off, &big, 1, Some(1)), None); assert_eq!(since(1, off, &big, 1, Some(2)).map(|v| v.len()), Some(RING));
    }
    #[test] fn frames_merge_by_offset() {
        assert_eq!(merge(10, 10, 5), Merge::Take(0)); assert_eq!(merge(10, 8, 5), Merge::Take(2)); assert_eq!(merge(10, 3, 5), Merge::Skip);
        assert_eq!(merge(10, 12, 5), Merge::Gap); assert_eq!(merge(10, 10, 0), Merge::Skip);
    }
    #[test] fn a_screen_read_back_from_a_running_terminal_is_stopped() {
        assert_eq!(read_back("running"), "stopped"); assert_eq!(read_back("done 0"), "exited 0"); assert_eq!(read_back("exited 3"), "exited 3"); assert_eq!(read_back("stopped"), "stopped");
    }
    #[test] fn runs_have_new_names() { let a = gen(); let b = gen(); assert!(b > a) }
}
