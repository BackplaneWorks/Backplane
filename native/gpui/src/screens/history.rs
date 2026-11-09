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

/// The first row of `page` that `rows` also holds: (its index in `rows`, its index in `page`).
fn first_shared(rows: &[Value], page: &[Value]) -> Option<(usize, usize)> {
    let at: HashMap<&str, usize> = rows.iter().enumerate().map(|(i, r)| (id(r), i)).collect();
    page.iter().enumerate().find_map(|(j, r)| at.get(id(r)).map(|i| (*i, j)))
}

/// Where a picture of the newest page starts in the rows held (None: it shares nothing with them).
pub fn merge_point(rows: &[Value], fresh: &[Value]) -> Option<usize> { first_shared(rows, fresh).map(|(i, _)| i) }

/// A new picture of the newest page over the rows held: the rows above it stay, and from the first row they share the picture is the truth, so a
/// reply that ended, a run that shut or a row the hub took away goes with it. The page's rows before that first shared row are only the cut head of
/// a run the rows held have whole. Shares nothing: the thread moved on by more than a page, and the picture stands alone.
pub fn merge(mut rows: Vec<Value>, fresh: Vec<Value>) -> Vec<Value> {
    match first_shared(&rows, &fresh) {
        Some((i, j)) => { rows.truncate(i); rows.extend(fresh.into_iter().skip(j)); rows }
        None => fresh,
    }
}

/// A page read for history laid into the rows held. Rows it shares are replaced by the page's, its rows above them go on top, and the rows held below
/// the last row it shares stay. A top row held before the first shared one is the cut head of a run the older page has whole. A page with nothing
/// shared sits above everything: it is the page just older than the rows held.
pub fn splice(rows: Vec<Value>, page: Vec<Value>) -> Vec<Value> {
    let Some((first, j)) = first_shared(&rows, &page) else { return page.into_iter().chain(rows).collect() };
    let shared: HashSet<&str> = rows.iter().map(id).collect();
    let last_in_page = page.iter().rposition(|r| shared.contains(id(r))).unwrap_or(j);
    let last = rows.iter().rposition(|r| id(r) == id(&page[last_in_page])).unwrap_or(first);
    let keep_top = if j > 0 && first <= 1 { 0 } else { first };
    let tail = rows[last + 1..].to_vec();
    let mut out: Vec<Value> = rows[..keep_top].to_vec();
    out.extend(page.into_iter().take(last_in_page + 1));
    out.extend(tail);
    out
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

/// The next older page to ask for after one landed: none at the start of the thread, none past the cap, and never the page just read (a hub answering
/// the same cursor would otherwise be asked for ever).
pub fn next_older(read: &str, older: &str, has_more: bool, rows: usize) -> String {
    if !has_more || older == read || rows >= MAX_ROWS { String::new() } else { older.to_string() }
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
        assert_eq!(next_older("p1", "p2", true, 10), "p2");
        assert_eq!(next_older("p1", "p2", false, 10), ""); assert_eq!(next_older("p1", "p1", true, 10), ""); assert_eq!(next_older("p1", "p2", true, MAX_ROWS), "");
    }
}
