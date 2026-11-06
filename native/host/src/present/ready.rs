//! The project card's Ready to order bar: the hub's `fab.look` answer is kept in the session's `fab.*` info
//! (`keep`), and `bar` says what the thread draws above its composer from it. No order is made here: `order.buy` is not ported, so the bar names what there is to order and
//! its price estimate, and puts away (`fab-min`, the setting `fab.min.<project>`) or comes back (`fab-show`).
use crate::state::{State, Turn};
use serde_json::{json, Value};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
/// Order.ordered
fn ordered(st: &str) -> bool { matches!(st, "paid" | "placed" | "shipped" | "delivered") }
fn ordered_word(st: &str) -> String { match st { "paid" => "paid, being placed".into(), "placed" => "in production".into(), o => o.into() } }
/// Card.ready.j with Card.ready.or
fn ready_line(r: &Value) -> String {
    let names: Vec<&str> = r["parts"].as_array().into_iter().flatten().map(|p| s(p, "name").trim()).filter(|n| !n.is_empty()).collect();
    let boards = r["boards"].as_u64().unwrap_or(0);
    let line = if !names.is_empty() { format!("Ready to order · {}", names.join(" · ")) } else if boards == 0 { String::new() } else { format!("Ready to order · {}", if boards == 1 { "the board".to_string() } else { format!("{boards} boards") }) };
    let note = s(r, "estimate_problems");
    let line = if note.is_empty() { line } else { format!("{note}. Check the fab files before ordering.") };
    let err = s(r, "error");
    let line = if err.is_empty() { line } else { format!("Estimate unavailable: {err}{}", if let Some(rest) = line.strip_prefix("Ready to order · ") { format!(" · Parts: {rest}") } else if line.is_empty() { String::new() } else { format!(" · {line}") }) };
    let order = s(r, "order");
    if ordered(order) { format!("Ordered · {}", ordered_word(order)) } else { line }
}
/// Card.action: the button's words ("" for none)
fn buy_label(r: &Value) -> String {
    let (n, b) = (r["parts"].as_array().map_or(0, Vec::len), r["boards"].as_u64().unwrap_or(0));
    let (label, err, on) = (s(r, "label"), s(r, "error"), r["estimates"].as_bool().unwrap_or(true));
    if n + b as usize > 0 && !err.is_empty() { return "Buy it".into() }
    if n == 0 { return if b > 0 && err.is_empty() { "Buy it".into() } else { String::new() } }
    if label.is_empty() { if on { String::new() } else { "Buy it".into() } } else { format!("Buy it · {label}") }
}

/// Card.boards.j: the boards the viewer's picker offers, one path a line (from the project's folder).
fn boards_text(v: &Value) -> String { v.as_array().into_iter().flatten().filter_map(Value::as_str).filter(|p| !p.contains('\n')).collect::<Vec<_>>().join("\n") }
/// The session patch a `boards.list` answer makes (`{thread, board_list, board_default}`): the same two keys the card sets.
pub fn keep_boards(res: &Value) -> Option<Value> {
    let th = s(res, "thread"); if th.is_empty() || !res["board_list"].is_array() { return None }
    Some(json!({"info": {format!("fab.boards:{th}"): boards_text(&res["board_list"]), format!("fab.board:{th}"): s(res, "board_default")}}))
}
/// The session patch a `fab.look` answer makes: the card's fields as `fab.*` info of its thread.
pub fn keep(res: &Value) -> Option<Value> {
    let th = s(res, "thread"); if th.is_empty() || res.get("goal").is_none() { return None }
    let shown = !s(res, "intent").trim().is_empty() || res["parts"].as_array().map_or(0, Vec::len) + res["boards"].as_u64().unwrap_or(0) as usize > 0;
    let goal = s(res, "goal");
    Some(json!({"info": {
        format!("fab.line:{th}"): if goal.is_empty() { "No goal yet. Edit to say what this project is for." } else { goal },
        format!("fab.intent:{th}"): s(res, "intent"), format!("fab.buy:{th}"): buy_label(res), format!("fab.shown:{th}"): if shown { "1" } else { "" },
        format!("fab.boards:{th}"): boards_text(&res["board_files"]), format!("fab.board:{th}"): s(res, "board_default"),
        format!("fab.ready:{th}"): ready_line(res), format!("fab.again:{th}"): if ordered(s(res, "order")) { "1" } else { "" }}}))
}

/// The bar above the composer (Fab.ready): None while ordering is off, the thread is a buy thread, the agent works, nothing was said yet, or there is nothing to order.
/// `min`: the bar is put away (its header button, "Buy it", brings it back).
pub fn bar(st: &State, thread: &str, session: &Value) -> Option<Value> {
    if st.settings.get("order.enabled").map_or(false, |v| v == "off") { return None }
    let t = st.threads.get(thread)?; if t.project.is_empty() { return None }
    let info = |k: &str| s(&session["info"], &format!("{k}:{thread}")).to_string();
    let (buy, line) = (info("fab.buy"), info("fab.ready"));
    if buy.is_empty() || st.settings.get(&format!("buy.{thread}")).map_or(false, |v| !v.is_empty()) { return None }
    let fresh = t.turn == Turn::Idle && t.activity <= t.created;
    if t.turn == Turn::Running || fresh { return None }
    let line = if line.is_empty() { "Ready to order".to_string() } else { line };
    let min = st.settings.get(&format!("fab.min.{}", t.project)).map_or(false, |v| v == "1");
    let again = info("fab.again") == "1";
    Some(json!({"line": line, "buy": if again { "Buy again".to_string() } else { buy }, "min": min, "again": again, "thread": format!("{thread}")}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(parts: &[&str], label: &str, err: &str) -> Value { json!({"thread":"t1","intent":"# Goal\nlamp","goal":"lamp","parts":parts.iter().map(|n| json!({"name":n})).collect::<Vec<_>>(),"boards":0,"label":label,"estimate_problems":"","error":err,"estimates":true,"order":""}) }
    #[test] fn the_card_names_its_parts_and_the_price() {
        let c = card(&["LED board", "Lamp shade"], "from $5", "");
        assert_eq!(ready_line(&c), "Ready to order · LED board · Lamp shade"); assert_eq!(buy_label(&c), "Buy it · from $5");
        let k = keep(&c).unwrap(); assert_eq!(k["info"]["fab.line:t1"], "lamp"); assert_eq!(k["info"]["fab.shown:t1"], "1");
    }
    #[test] fn an_unreachable_server_still_allows_buying_and_says_so() {
        let c = card(&["LED board"], "", "the order server is not reachable");
        assert_eq!(buy_label(&c), "Buy it"); assert_eq!(ready_line(&c), "Estimate unavailable: the order server is not reachable · Parts: LED board");
        assert_eq!(buy_label(&card(&[], "", "")), "");
    }
    #[test] fn an_ordered_project_reads_ordered_and_buy_again() {
        let mut c = card(&["A"], "from $5", ""); c["order"] = json!("placed");
        assert_eq!(ready_line(&c), "Ordered · in production"); assert_eq!(keep(&c).unwrap()["info"]["fab.again:t1"], "1");
    }
    #[test] fn the_bar_waits_while_the_agent_works_or_nothing_was_said() {
        let mut st = State::default();
        st.apply(&json!({"$":"ProjectCreated","id":"p1","title":"p","root":"/p","at":1})); st.apply(&json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"x","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}));
        let mut sess = json!({"info": {}}); crate::session::patch::merge(&mut sess, &keep(&card(&["A"], "from $5", "")).unwrap());
        assert!(bar(&st, "t1", &sess).is_none(), "a thread nothing was said in");
        st.apply(&json!({"$":"TurnChanged","thread":"t1","state":"completed","at":9}));
        let b = bar(&st, "t1", &sess); assert!(b.is_some_and(|b| b["line"] == "Ready to order · A" && b["buy"] == "Buy it · from $5"), "{:?}", st.threads.get("t1").map(|t| (t.turn.word(), t.activity, t.created)));
    }
}
