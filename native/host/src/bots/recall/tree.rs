//! The memory tree and its view, after UniiChat's Memory.ts (github.com/VictorTaelin/uniichat): pure functions, no I/O.
//!
//! Message i of a thread's conversation is leaf (0, i); node (l, i) merges (l-1, 2i) and (l-1, 2i+1), so it covers messages
//! i·2^l to (i+1)·2^l - 1. Every node is a line of at most NODE bytes: a leaf whose message fits is its own line, any other
//! node is written by the compactor. A view lists nodes covering the conversation from its start, oldest first: recent
//! messages one line each, older ones covered by broader lines. Between batches it only grows at its end (each message
//! appends its leaf), so a provider's prompt cache keeps all of it but its end. Past `high` bytes a batch merges the most due
//! adjacent pairs whose parent is written (the oldest since its last message, counted in halves: detail fades with age)
//! down to half, and the view is kept as it then stands.
use std::cmp::Ordering;

/// A line's most bytes (about 70 words).
pub const NODE: usize = 512;
/// The view's most bytes: past it a batch merges it down to half, so it is 64 to 128 KB once the conversation is long.
pub const HIGH: usize = 128_000;
/// What a line not written yet says (a merge whose halves the compactor has not reached).
pub const WAIT: &str = "(not summarized yet: zoom it)";

/// A node: (level, index).
pub type Part = (u32, u64);

/// The messages a node covers, first and last.
pub fn span((l, i): Part) -> (u64, u64) { let a = i << l; (a, a + (1u64 << l) - 1) }
/// Its name in the view and for memory_zoom: "id+n".
pub fn name(p: Part) -> String { format!("{}+{}", p.1 << p.0, 1u64 << p.0) }
/// Its two halves (none for a leaf).
pub fn halves((l, i): Part) -> Vec<Part> { if l == 0 { vec![] } else { vec![(l - 1, 2 * i), (l - 1, 2 * i + 1)] } }
/// Its parent.
pub fn parent((l, i): Part) -> Part { (l + 1, i / 2) }
/// The tree's roots over `count` messages: a node per power of two in the count, largest (oldest) first.
pub fn roots(count: u64) -> Vec<Part> {
    let (mut out, mut id) = (vec![], 0u64);
    for l in (0..=40u32).rev() { let n = 1u64 << l; if id + n <= count { out.push((l, id >> l)); id += n } }
    out
}
/// The message after a view's last line.
pub fn end(parts: &[Part]) -> u64 { parts.last().map_or(0, |p| span(*p).1 + 1) }
/// The node `id+n` names in a tree of `count` messages: n a power of two, id a multiple of n, all its messages there.
pub fn part(count: u64, id: u64, n: u64) -> Option<Part> {
    if n == 0 || !n.is_power_of_two() || id % n != 0 || id.checked_add(n)? > count { return None }
    Some((n.trailing_zeros(), id / n))
}

/// A view brought to message `to`: lines past it dropped (the conversation is shorter: never in a log that only grows, kept
/// for safety), then a leaf for each message from its end to `to`.
pub fn reach(parts: &[Part], to: u64) -> Vec<Part> {
    let mut v: Vec<Part> = parts.iter().copied().take_while(|p| span(*p).1 < to).collect();
    for i in end(&v)..to { v.push((0, i)) }
    v
}

/// The nodes the compactor may write now: not written, their sources written (a leaf's is its message), each visited once
/// from the roots down through nodes not written. In the order their messages came (a node is due at its last message;
/// at a tie the lower level first).
pub fn todo(count: u64, built: &dyn Fn(Part) -> bool) -> Vec<Part> {
    fn go(p: Part, built: &dyn Fn(Part) -> bool, out: &mut Vec<Part>) {
        if built(p) { return }
        let hs = halves(p);
        if hs.iter().all(|h| built(*h)) { out.push(p) }
        for h in hs { go(h, built, out) }
    }
    let mut out = vec![];
    for r in roots(count) { go(r, built, &mut out) }
    out.sort_by_key(|p| (span(*p).1, p.0));
    out
}

/// A pair offered to a merge: its due (the bigger the sooner), its place in the view, its two lines.
struct Pair { d: f64, k: usize, a: Part, b: Part }
impl PartialEq for Pair { fn eq(&self, o: &Self) -> bool { self.cmp(o) == Ordering::Equal } }
impl Eq for Pair {}
impl PartialOrd for Pair { fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) } }
/// The most due first; of equals, the oldest.
impl Ord for Pair { fn cmp(&self, o: &Self) -> Ordering { self.d.partial_cmp(&o.d).unwrap_or(Ordering::Equal).then_with(|| o.k.cmp(&self.k)) } }

/// A view's most due pairs merged down to `high`/2, as far as written parents allow: a pair whose parent is written, most due
/// first ((T + 1) / 2^l - i, T the view's end: its age from its last message, in halves), the oldest of equals. Answers the new
/// lines and their bytes.
pub fn merge(parts: &[Part], size: &dyn Fn(Part) -> usize, built: &dyn Fn(Part) -> bool, high: usize) -> (Vec<Part>, usize) {
    let (t, low) = (end(parts), high / 2);
    let len = parts.len();
    let mut view: Vec<Option<Part>> = parts.iter().copied().map(Some).collect();
    let mut next: Vec<usize> = (1..=len).collect();
    let mut prev: Vec<Option<usize>> = (0..len).map(|k| k.checked_sub(1)).collect();
    let mut heap = std::collections::BinaryHeap::new();
    let offer = |k: Option<usize>, view: &Vec<Option<Part>>, next: &Vec<usize>, heap: &mut std::collections::BinaryHeap<Pair>| {
        let Some(k) = k else { return };
        let (Some(a), Some(Some(b))) = (view[k], view.get(next[k])) else { return };
        if a.1 % 2 != 0 || b.0 != a.0 || b.1 != a.1 + 1 || !built(parent(a)) { return }
        heap.push(Pair { d: (t + 1) as f64 / (1u64 << a.0) as f64 - a.1 as f64, k, a, b: *b });
    };
    let mut total: usize = parts.iter().map(|p| size(*p)).sum();
    for k in 0..len { offer(Some(k), &view, &next, &mut heap) }
    while total > low {
        let Some(Pair { k, a, b, .. }) = heap.pop() else { break };
        let j = next[k];
        if view[k] != Some(a) || view.get(j).copied().flatten() != Some(b) { continue }
        let up = parent(a);
        total = total + size(up) - size(a) - size(b);
        view[k] = Some(up); view[j] = None;
        next[k] = next[j]; if next[j] < len { prev[next[j]] = Some(k) }
        offer(prev[k], &view, &next, &mut heap); offer(Some(k), &view, &next, &mut heap);
    }
    (view.into_iter().flatten().collect(), total)
}

/// The view a turn reads: the kept one brought to message `to`, and, when that is over `high`, a batch merged; true when the
/// batch changed it (to be kept), false when the kept one, reached, is the view.
pub fn fit(kept: &[Part], to: u64, size: &dyn Fn(Part) -> usize, built: &dyn Fn(Part) -> bool, high: usize) -> (Vec<Part>, bool) {
    let v = reach(kept, to);
    let total: usize = v.iter().map(|p| size(*p)).sum();
    if total <= high { return (v, false) }
    // a batch is kept only when it brings the view under `high`: while the compactor is behind (a long history it is still writing) the
    // view stays as it is, its oldest lines left out (`block`), rather than a new view kept at every turn
    let (m, after) = merge(&v, size, built, high);
    let changed = m.len() != v.len() && after <= high;
    (if changed { m } else { v }, changed)
}

/// One line: newlines as spaces.
pub fn flat(text: &str) -> String { if !text.contains(['\n', '\r']) { return text.to_string() } text.split(['\n', '\r']).map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ") }
/// A text's first `n` bytes, cut on a character.
pub fn cut(text: &str, n: usize) -> &str { if text.len() <= n { return text } let mut e = n; while !text.is_char_boundary(e) { e -= 1 } &text[..e] }
/// A line as the view shows it: "id+n|text".
pub fn line(p: Part, text: &str) -> String { format!("{}|{}", name(p), flat(text)) }
/// A message as its own line: who wrote it and its text, one line. Fits: it is its leaf; else the view shows it cut until the
/// compactor writes its line.
pub fn said(who: &str, text: &str) -> String { flat(&format!("{who}: {}", cut(text.trim(), 4 * NODE))) }
/// A leaf too long for a line, cut to one, as the view shows it until it is written.
pub fn clip(said: &str) -> String { if said.len() <= NODE { said.to_string() } else { format!("{} (zoom it)", cut(said, NODE - 12)) } }

/// How the agent reads the view (UniiChat's prompt, for Backplane's tools).
pub const DOC: &str = "# Your memory: one conversation that never ends\n\n\
Each turn starts fresh, and your memory of the whole conversation in this thread is the view below, oldest first, inside <chat> tags, as one-line summaries:\n\n\
  id+n|text   the n messages from id on, summarized (newlines shown as spaces)\n\n\
Each message starts with who wrote it: user (the person, or a report, mail or another bot's message that reached you), you (your replies), tool (your tool calls), note (Backplane's notes). \
The summaries form a binary tree: each message is compressed into a line (a short message is its own line, word for word), then adjacent lines are merged in pairs, again and again. \
Recent lines cover one message each; the older the messages, the more a line covers. A line not summarized yet shows cut, ending \"(zoom it)\", or as \"(not summarized yet: zoom it)\".\n\n\
memory_zoom(id, n) opens line id+n into the two lines it was made from; memory_zoom(id, 1) gives message id whole, with who wrote it and when. \
The view is your memory, and its latest word on a thing is the truth: whenever you need something from earlier, find its latest mention in the view and zoom until you have it whole before you act, guess or ask. \
Summaries keep little of tool output, so say in your reply what you learned that will matter later. Your <memory> notes (memory_save) still hold what must always be at hand.\n";

/// The block a fresh turn starts with: the doc, then the lines in <chat>. Over `high` (a backlog the compactor has not
/// merged yet) the oldest lines are left out, said in one line that never changes.
pub fn block(lines: &[String], high: usize) -> String {
    let mut from = 0; let mut total: usize = lines.iter().map(|l| l.len() + 1).sum();
    while total > high && from < lines.len() { total -= lines[from].len() + 1; from += 1 }
    let left = if from > 0 { "(older lines are left out for size: memory_zoom(0, 0) lists the tree's roots)\n" } else { "" };
    let mut s = String::with_capacity(DOC.len() + total + 64);
    s.push_str(DOC); s.push_str("\n<chat>\n"); s.push_str(left);
    for l in &lines[from..] { s.push_str(l); s.push('\n') }
    s.push_str("</chat>"); s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test] fn roots_spans_and_names() {
        assert_eq!(roots(0), vec![]);
        assert_eq!(roots(13), vec![(3, 0), (2, 2), (0, 12)]);
        assert_eq!(span((3, 1)), (8, 15)); assert_eq!(name((3, 1)), "8+8"); assert_eq!(name((0, 5)), "5+1");
        assert_eq!(part(13, 8, 4), Some((2, 2))); assert_eq!(part(13, 8, 8), None); assert_eq!(part(13, 3, 2), None); assert_eq!(part(13, 12, 1), Some((0, 12))); assert_eq!(part(13, 0, 0), None);
        assert_eq!(halves((2, 3)), vec![(1, 6), (1, 7)]); assert_eq!(parent((1, 7)), (2, 3));
    }
    #[test] fn todo_builds_leaves_then_merges_in_order() {
        // 6 messages, leaf 1 too long (not built): the leaf first, then merges as their halves are written
        let built: HashSet<Part> = [(0, 0), (0, 2), (0, 3), (0, 4), (0, 5)].into();
        let t = todo(6, &|p| built.contains(&p));
        assert_eq!(t, vec![(0, 1), (1, 1), (1, 2)]);
        let built: HashSet<Part> = [(0, 0), (0, 1), (0, 2), (0, 3), (1, 0), (1, 1)].into();
        assert_eq!(todo(4, &|p| built.contains(&p)), vec![(2, 0)]);
        let all: HashSet<Part> = [(0, 0), (0, 1), (1, 0)].into();
        assert!(todo(2, &|p| all.contains(&p)).is_empty());
    }
    #[test] fn reach_appends_leaves_and_keeps_merged_lines() {
        assert_eq!(reach(&[(2, 0), (0, 4)], 7), vec![(2, 0), (0, 4), (0, 5), (0, 6)]);
        assert_eq!(reach(&[(2, 0), (0, 4)], 4), vec![(2, 0)]);
    }
    #[test] fn merge_folds_the_oldest_pairs_first_down_to_half() {
        let v: Vec<Part> = (0..16).map(|i| (0, i)).collect();
        let (m, total) = merge(&v, &|_| 10, &|_| true, 100);
        assert!(total <= 50, "{total}");
        // the oldest messages are covered by the broadest lines, the newest stay leaves
        assert_eq!(m.last(), Some(&(0, 15)));
        assert!(m[0].0 > m[m.len() - 2].0, "{m:?}");
        // contiguous and complete
        let mut at = 0; for p in &m { assert_eq!(span(*p).0, at); at = span(*p).1 + 1 } assert_eq!(at, 16);
    }
    #[test] fn merge_waits_for_parents_not_written() {
        let v: Vec<Part> = (0..8).map(|i| (0, i)).collect();
        let (m, _) = merge(&v, &|_| 10, &|p| p == (1, 1), 20);
        assert_eq!(m, vec![(0, 0), (0, 1), (1, 1), (0, 4), (0, 5), (0, 6), (0, 7)]);
        // and a batch that cannot bring the view under `high` is not kept
        let (v2, changed) = fit(&[], 8, &|_| 10, &|p| p == (1, 1), 60);
        assert!(!changed && v2.len() == 8);
        let (v3, changed) = fit(&[], 8, &|_| 10, &|_| true, 60);
        assert!(changed && v3.len() < 8);
    }
    #[test] fn block_cuts_the_oldest_lines_over_high() {
        let ls: Vec<String> = (0..10).map(|i| format!("{i}+1|{}", "x".repeat(95))).collect();
        let b = block(&ls, 500);
        assert!(b.contains("older lines are left out") && !b.contains("0+1|") && b.contains("9+1|"));
        assert!(b.len() - DOC.len() < 600);
        assert!(!block(&ls, 100_000).contains("left out"));
    }
    #[test] fn said_and_clip_make_one_line() {
        assert_eq!(said("user", "a\nb\r\n c"), "user: a b c");
        let long = said("you", &"é".repeat(400)); assert!(long.len() > NODE);
        let c = clip(&long); assert!(c.len() <= NODE && c.ends_with("(zoom it)"));
        assert_eq!(cut("éé", 3), "é");
    }
}
