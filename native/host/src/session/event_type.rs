//! Its own file because every fold reads log lines through it: a change here changes what they produce, so it is an input of their checkpoint stamps (`fold_inputs.rs`).
/// The event type of a log line: `{"$":"Type",…}`, tolerant of JSON whitespace (`{ "$" : "Type"`), as the JSON parse of hub and `Model` are. A line
/// that is not an object starting with a `$` string field gives None. The fast path is the compact form the hub writes itself; hand-written logs
/// (Python's json.dumps puts a space after the colon) took the strict `strip_prefix` filters of agents/bots/commands to "no such event".
pub fn event_type(line: &str) -> Option<&str> {
    if let Some(r) = line.strip_prefix("{\"$\":\"") { return r.split('"').next() }
    let r = line.trim_start().strip_prefix('{')?.trim_start().strip_prefix("\"$\"")?.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    r.split('"').next()
}
