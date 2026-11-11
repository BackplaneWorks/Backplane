//! The rows of a thread held beyond the hub's newest page. The hub draws at most one page of a thread at a time and a view stays on the page
//! it asked for, so the window keeps what it has read here: `merge` lays each new picture of the newest page over the rows held, `splice` lays a
//! page read for history into them, and `changed` says which rows the timeline's list must measure again. Pages overlap by half, so a page finds
//! its place by the rows it shares. Pure: no window, no hub.
use serde_json::Value;
use std::{collections::{HashMap, HashSet}, ops::Range};

/// The most rows held for one thread. The list draws only what is on screen, so this bounds memory, not frame time.
pub const MAX_ROWS: usize = 8192;
/// While the reader's top row is within this many rows of the first one held, a page that lands asks for the next at once (glue.rs `older_landed`).
pub const RUNWAY_ROWS: usize = 40;
/// Where a row read for history remembers its page, so a fold opened in it can be read again. Not the hub's: `changed` ignores it.
pub const PAGE_KEY: &str = "historyPage";

fn id(row: &Value) -> &str { row["id"].as_str().unwrap_or("") }

/// A row that names a place in the thread. The hub's live rows (`@working:<thread>`, `@stream:<thread>`) keep one id whatever the thread
/// has become, so they say nothing about where a page sits: a picture of the newest page found its place at the "Working" row of an old
/// stretch, every newer row above that row in the picture went as a "cut head", and Bub's thread stayed at 01:47 for hours (Nov 11).
fn anchor(row: &Value) -> bool { let i = id(row); !i.is_empty() && !i.starts_with('@') }
/// The first row of `page` that `rows` also holds: (its index in `rows`, its index in `page`). Live rows are no anchor.
fn first_shared(rows: &[Value], page: &[Value]) -> Option<(usize, usize)> {
    let at: HashMap<&str, usize> = rows.iter().enumerate().filter(|(_, r)| anchor(r)).map(|(i, r)| (id(r), i)).collect();
    page.iter().enumerate().filter(|(_, r)| anchor(r)).find_map(|(j, r)| at.get(id(r)).map(|i| (*i, j)))
}
/// The most rows at the top of a newest page that may be missing from the rows held and still be only the cut head of a run held whole
/// (a fold's id changes with where the page cuts it). More, and the rows held are not the stretch just above the page: they are not merged.
pub const CUT_HEAD: usize = 2;

/// How far up the rows held a picture of the newest page may find its place: the newest page sits at the end of what is held, so a row it
/// shares further up (a run's fold id seen again, a row an older page brought back above) is not where it starts. Matching it there cut every
/// row read for history away, the list asked for those pages again, the next picture cut them again: the thread flashed between its newest
/// page and its history many times a second (Nov 10, both recordings of opening Bub).
pub fn tail_from(rows: &[Value], fresh: &[Value]) -> usize { rows.len().saturating_sub(fresh.len() * 2 + 16) }
/// The first row of `fresh` that the tail of `rows` (from `tail_from`) also holds: (its index in `rows`, its index in `fresh`).
/// None too when the page's first shared row comes after more than CUT_HEAD rows the tail does not hold: merging there would drop them.
fn first_shared_tail(rows: &[Value], fresh: &[Value]) -> Option<(usize, usize)> {
    let from = tail_from(rows, fresh);
    first_shared(&rows[from..], fresh).map(|(i, j)| (i + from, j)).filter(|(_, j)| *j <= CUT_HEAD)
}
/// Where a picture of the newest page starts in the rows held (None: it shares nothing with them).
pub fn merge_point(rows: &[Value], fresh: &[Value]) -> Option<usize> { first_shared_tail(rows, fresh).map(|(i, _)| i) }

/// A new picture of the newest page over the rows held: the rows above it stay, and from the first row they share the picture is the truth, so a
/// reply that ended, a run that shut or a row the hub took away goes with it. The page's rows before that first shared row are only the cut head of
/// a run the rows held have whole. Shares nothing: the thread moved on by more than a page, and the picture stands alone.
pub fn merge(mut rows: Vec<Value>, fresh: Vec<Value>) -> Vec<Value> {
    match first_shared_tail(&rows, &fresh) {
        Some((i, j)) => { rows.truncate(i); rows.extend(fresh.into_iter().skip(j)); rows }
        None => fresh,
    }
}

/// A page read for history laid into the rows held. Rows it shares are replaced by the page's, its rows above them go on top, and the rows held below
/// the last row it shares stay. A top row held before the first shared one is the cut head of a run the older page has whole. A page with nothing
/// shared sits above everything: it is the page just older than the rows held.
pub fn splice(rows: Vec<Value>, page: Vec<Value>) -> Vec<Value> {
    let Some((first, j)) = first_shared(&rows, &page) else { return page.into_iter().chain(rows).collect() };
    let (keep_top, last, last_in_page) = splice_plan(&rows, &page, first, j);
    let tail = rows[last + 1..].to_vec();
    let mut out: Vec<Value> = rows[..keep_top].to_vec();
    out.extend(page.into_iter().take(last_in_page + 1));
    out.extend(tail);
    out
}
/// Where a page goes: the rows held kept on top (`..keep_top`), the last held row it replaces (`last`), and its own last row that goes in (`last_in_page`).
fn splice_plan(rows: &[Value], page: &[Value], first: usize, j: usize) -> (usize, usize, usize) {
    let shared: HashSet<&str> = rows.iter().map(id).collect();
    let last_in_page = page.iter().rposition(|r| shared.contains(id(r))).unwrap_or(j);
    let last = rows.iter().rposition(|r| id(r) == id(&page[last_in_page])).unwrap_or(first);
    (if j > 0 && first <= 1 { 0 } else { first }, last, last_in_page)
}
/// `splice` and what it changed (as `changed(&rows, &spliced, 0)` says), without copying the rows held: they are moved, and only the stretch the
/// page replaces is compared. A long thread read far back holds thousands of rows; each page that lands used to copy and compare all of them.
pub fn splice_changed(mut rows: Vec<Value>, page: Vec<Value>) -> (Vec<Value>, Range<usize>, Range<usize>) {
    let Some((first, j)) = first_shared(&rows, &page) else {
        let (n, held) = (page.len(), rows.len());
        let (a, b) = if n == 0 { (held..held, held..held) } else { (0..0, 0..n) };
        return (page.into_iter().chain(rows).collect(), a, b);
    };
    let (keep_top, last, last_in_page) = splice_plan(&rows, &page, first, j);
    if last + 1 < keep_top { let spliced = splice(rows.clone(), page); let (a, b) = changed(&rows, &spliced, 0); return (spliced, a, b); } // (no such page from the hub)
    let (a, b) = changed(&rows[keep_top..=last], &page[..=last_in_page], 0);
    let tail = rows.split_off(last + 1); rows.truncate(keep_top);
    rows.extend(page.into_iter().take(last_in_page + 1)); rows.extend(tail);
    (rows, a.start + keep_top..a.end + keep_top, b.start + keep_top..b.end + keep_top)
}

fn same_row(a: &Value, b: &Value) -> bool {
    match (a.as_object(), b.as_object()) {
        (Some(x), Some(y)) => x.iter().filter(|(k, _)| *k != PAGE_KEY).eq(y.iter().filter(|(k, _)| *k != PAGE_KEY)),
        _ => a == b,
    }
}

/// The rows that differ: `old[r.0]` was replaced by `new[r.1]`, the rows around them unchanged. `from`: the rows before it are known to be the same.
pub fn changed(old: &[Value], new: &[Value], from: usize) -> (Range<usize>, Range<usize>) {
    let limit = old.len().min(new.len());
    let mut start = from.min(limit);
    while start < limit && same_row(&old[start], &new[start]) { start += 1; }
    let (mut a, mut b) = (old.len(), new.len());
    while a > start && b > start && same_row(&old[a - 1], &new[b - 1]) { a -= 1; b -= 1; }
    (start..a, start..b)
}

/// The next older page to ask for after one landed: none at the start of the thread, none past the cap, and never a page already read for the
/// rows held (`seen`, by `page_mark`): a hub answering the same cursor, or two cursors that lead to each other, would otherwise be asked for
/// ever, and every page that lands moves the rows on screen (Nov 10: a 27-row and a 2-row page in turn, 40 a second).
pub fn next_older(read: &str, older: &str, has_more: bool, rows: usize, seen: &HashSet<String>) -> String {
    if !has_more || older == read || rows >= MAX_ROWS || seen.contains(&page_mark(older)) { String::new() } else { older.to_string() }
}
/// The page to ask for next after a picture of the newest page was laid over the rows held. `at`: where it went in them (rows held above it
/// keep the cursor they were read to, `held`). With none above, the newest window's own cursor, unless that page was read already: then the
/// cursor the reading got to stands. (Setting it to none there stopped history for good: a page of only hidden rows adds nothing above, the
/// next picture went at 0, and Bub's thread could not be scrolled up at all, Nov 11.)
pub fn older_after_picture(at: usize, held: &str, older: &str, has_more: bool, seen: &HashSet<String>) -> String {
    if at > 0 { return held.to_string(); }
    if !has_more || older.is_empty() { return String::new(); }
    if seen.contains(&page_mark(older)) { held.to_string() } else { older.to_string() }
}
/// What names a page whatever else its cursor carries: its `before` (the hub's epoch and target change nothing about which rows it holds).
pub fn page_mark(cursor: &str) -> String {
    match serde_json::from_str::<Value>(cursor) { Ok(c) if !c["before"].is_null() => c["before"].as_str().map_or_else(|| c["before"].to_string(), str::to_string), _ => cursor.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;
    fn rows(ids: &[&str]) -> Vec<Value> { ids.iter().map(|i| json!({"id": i, "text": i})).collect() }
    fn ids(r: &[Value]) -> Vec<&str> { r.iter().map(id).collect() }
    #[test] fn a_new_picture_keeps_the_rows_above_it_and_takes_the_rest() {
        assert_eq!(ids(&merge(rows(&["a", "b", "c", "d"]), rows(&["c", "d", "e"]))), ["a", "b", "c", "d", "e"]);
    }
    #[test] fn a_window_that_slid_past_a_row_does_not_lose_it() {
        // the hub's page moved down by two: what fell off its top is still held
        assert_eq!(ids(&merge(rows(&["a", "b", "c", "d"]), rows(&["c", "d", "e", "f"]))), ["a", "b", "c", "d", "e", "f"]);
    }
    #[test] fn a_row_the_hub_took_away_from_the_newest_page_goes() {
        assert_eq!(ids(&merge(rows(&["a", "b", "@live:t"]), rows(&["a", "b", "m"]))), ["a", "b", "m"]);
    }
    #[test] fn the_cut_head_of_a_run_in_a_new_picture_gives_way_to_the_whole_run() {
        assert_eq!(ids(&merge(rows(&["fold:1", "m", "n"]), rows(&["fold:7", "m", "n", "o"]))), ["fold:1", "m", "n", "o"]);
    }
    #[test] fn a_picture_that_shares_nothing_stands_alone() { assert_eq!(ids(&merge(rows(&["a"]), rows(&["x", "y"]))), ["x", "y"]); }
    #[test] fn an_older_page_goes_on_top_once_each() {
        assert_eq!(ids(&splice(rows(&["c", "d", "e"]), rows(&["a", "b", "c", "d"]))), ["a", "b", "c", "d", "e"]);
    }
    #[test] fn an_older_page_replaces_the_cut_head_of_a_run() {
        assert_eq!(ids(&splice(rows(&["fold:7", "m", "n"]), rows(&["a", "fold:1", "m"]))), ["a", "fold:1", "m", "n"]);
    }
    #[test] fn splice_changed_says_what_splice_and_changed_say() {
        let row = |id: &str, t: &str| serde_json::json!({"id": id, "text": t});
        let held = |v: &[(&str, &str)]| v.iter().map(|(i, t)| row(i, t)).collect::<Vec<_>>();
        let cases: Vec<(Vec<Value>, Vec<Value>)> = vec![
            (held(&[("c", ""), ("d", ""), ("e", "")]), held(&[("a", ""), ("b", ""), ("c", ""), ("d", "")])),
            (held(&[("c", ""), ("d", ""), ("e", "")]), held(&[("a", ""), ("b", ""), ("c", "new"), ("d", "")])),
            (held(&[("fold:7", ""), ("m", ""), ("n", "")]), held(&[("a", ""), ("fold:1", ""), ("m", "")])),
            (held(&[("c", "")]), held(&[("a", ""), ("b", "")])),
            (held(&[("c", "")]), vec![]),
            (held(&[("a", ""), ("fold:1", ""), ("m", ""), ("z", "")]), held(&[("fold:1", ""), ("t1", ""), ("t2", ""), ("m", "")])),
            (held(&[("a", ""), ("fold:1", ""), ("t1", ""), ("t2", ""), ("m", ""), ("z", "")]), held(&[("fold:1", ""), ("m", "")])),
            (held(&[("x", ""), ("y", ""), ("p", ""), ("q", ""), ("r", "")]), held(&[("o", ""), ("p", "x"), ("q", "")])),
            (vec![], held(&[("a", "")])),
        ];
        for (rows, page) in cases {
            let want = splice(rows.clone(), page.clone()); let (a, b) = changed(&rows, &want, 0);
            let (got, x, y) = splice_changed(rows.clone(), page.clone());
            assert_eq!(ids(&got), ids(&want), "{rows:?} + {page:?}"); assert_eq!(got, want); assert_eq!((x, y), (a, b), "{rows:?} + {page:?}");
        }
    }
    #[test] fn an_older_page_with_no_overlap_sits_above() { assert_eq!(ids(&splice(rows(&["c"]), rows(&["a", "b"]))), ["a", "b", "c"]); }
    #[test] fn a_page_read_again_opens_a_run_in_the_middle_and_leaves_the_rest() {
        assert_eq!(ids(&splice(rows(&["a", "fold:1", "m", "z"]), rows(&["fold:1", "t1", "t2", "m"]))), ["a", "fold:1", "t1", "t2", "m", "z"]);
        assert_eq!(ids(&splice(rows(&["a", "fold:1", "t1", "t2", "m", "z"]), rows(&["fold:1", "m"]))), ["a", "fold:1", "m", "z"], "shutting it takes its rows away");
    }
    #[test] fn only_the_rows_that_differ_are_measured_again() {
        assert_eq!(changed(&rows(&["a", "b", "c"]), &rows(&["a", "x", "c"]), 0), (1..2, 1..2));
        assert_eq!(changed(&rows(&["c", "d"]), &rows(&["a", "b", "c", "d"]), 0), (0..0, 0..2));
        assert_eq!(changed(&rows(&["a", "b"]), &rows(&["a", "b", "c"]), 2), (2..2, 2..3));
        assert_eq!(changed(&rows(&["a"]), &rows(&["a"]), 0), (1..1, 1..1));
    }
    #[test] fn a_rows_page_mark_is_no_change() {
        let mut marked = rows(&["a"]); marked[0][PAGE_KEY] = json!("p1");
        assert_eq!(changed(&rows(&["a"]), &marked, 0), (1..1, 1..1));
    }
    #[test] fn paging_ends_at_the_start_at_the_cap_and_on_a_repeated_cursor() {
        let none = HashSet::new();
        assert_eq!(next_older("p1", "p2", true, 10, &none), "p2");
        assert_eq!(next_older("p1", "p2", false, 10, &none), ""); assert_eq!(next_older("p1", "p1", true, 10, &none), ""); assert_eq!(next_older("p1", "p2", true, MAX_ROWS, &none), "");
    }
    fn cur(before: u64, epoch: &str) -> String { json!({"version": 1, "epoch": epoch, "target": {"kind": "thread", "id": "t"}, "query": "", "before": before.to_string()}).to_string() }
    #[test] fn two_cursors_that_lead_to_each_other_are_read_once_each() {
        // the Nov 10 loop: A's page names B, B's names A again; each read once, then paging stops
        let (a, b) = (cur(900, "1"), cur(880, "1"));
        let mut seen = HashSet::new();
        seen.insert(page_mark(&a)); assert_eq!(next_older(&a, &b, true, 30, &seen), b);
        seen.insert(page_mark(&b)); assert_eq!(next_older(&b, &a, true, 32, &seen), "", "A again");
        assert_eq!(next_older(&b, &cur(900, "2"), true, 32, &seen), "", "the same page under a new epoch is the same page");
        assert_eq!(page_mark(&a), "900"); assert_eq!(page_mark("plain"), "plain");
    }
    #[test] fn the_working_row_is_no_place_and_a_picture_far_ahead_stands_alone() {
        // Nov 11: rows held stopped at 01:47 with the hub's "Working" row; every newer picture shared only that row
        let held = rows(&["a", "b", "m147", "@working:t"]);
        let fresh = rows(&["n1", "n2", "n3", "n4", "@working:t"]);
        assert_eq!(merge_point(&held, &fresh), None, "nothing real shared: the picture stands alone");
        assert_eq!(ids(&merge(held.clone(), fresh.clone())), ["n1", "n2", "n3", "n4", "@working:t"]);
        // shared, but after three rows the rows held do not have: not the stretch above the page
        let held = rows(&["a", "b", "c"]);
        assert_eq!(merge_point(&held, &rows(&["x", "y", "z", "c", "d"])), None);
        assert_eq!(ids(&merge(held, rows(&["x", "y", "z", "c", "d"]))), ["x", "y", "z", "c", "d"]);
        // a new message at the bottom while a turn works: appended, the working row follows
        assert_eq!(ids(&merge(rows(&["a", "b", "@working:t"]), rows(&["a", "b", "c", "@working:t"]))), ["a", "b", "c", "@working:t"]);
    }
    #[test] fn a_picture_over_rows_with_nothing_above_keeps_the_reading_going() {
        let (a, b) = (cur(900, "1"), cur(880, "1"));
        let mut seen = HashSet::new();
        assert_eq!(older_after_picture(0, "", &a, true, &seen), a, "nothing read yet: the newest window's page");
        seen.insert(page_mark(&a));
        assert_eq!(older_after_picture(0, &b, &a, true, &seen), b, "A read and it added no row: B, where the reading got to, stands");
        assert_eq!(older_after_picture(3, &b, &a, true, &seen), b);
        assert_eq!(older_after_picture(0, &b, "", false, &seen), "", "the thread starts here");
    }
    #[test] fn a_picture_finds_its_place_in_the_tail_and_never_cuts_the_history_above() {
        // an older page brought a row back above everything ("m", a run's fold seen again); the newest picture also has it: before, merge cut at
        // index 0 and every row read for history went
        let mut held: Vec<String> = vec!["m".into()]; for i in 0..200 { held.push(format!("h{i}")); } for x in ["x", "y", "z"] { held.push(x.into()); }
        let held: Vec<&str> = held.iter().map(String::as_str).collect();
        let got = merge(rows(&held), rows(&["m", "y", "z", "n"]));
        assert_eq!(got.len(), 205, "{:?}", ids(&got)[..3].to_vec());
        assert_eq!(ids(&got)[..2], ["m", "h0"]); assert_eq!(ids(&got)[got.len() - 3..], ["y", "z", "n"]);
        assert_eq!(merge_point(&rows(&held), &rows(&["m", "y", "z", "n"])), Some(202));
    }
}
