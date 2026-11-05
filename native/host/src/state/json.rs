//! Bend's JSON, as the fold uses it (src/core/json.bend): lenient readers with defaults, and a compact printer that escapes
//! exactly what `Json.show` does (`"` `\` and controls below 32: \n \r \t, the rest \u00xx in lower-case hex; everything else raw,
//! so \b and \f are \u0008 and \u000c, unlike serde_json's own printer). Numbers keep their source text (arbitrary_precision).
use serde_json::Value;

/// Json.parse: None for text that is not JSON (Bend's callers read that as Null or as "no value")
pub fn parse(text: &str) -> Option<Value> { serde_json::from_str(text).ok() }
pub fn parse_or_null(text: &str) -> Value { parse(text).unwrap_or(Value::Null) }

/// Json.get_str: the string at `key`, else `d` (a number or object there is the default too)
pub fn gstr<'a>(o: &'a Value, k: &str, d: &'a str) -> &'a str { o.get(k).and_then(|v| v.as_str()).unwrap_or(d) }
/// Json.as_raw: a number's text or a string's
fn raw(v: Option<&Value>) -> Option<String> {
    match v { Some(Value::Number(n)) => Some(n.to_string()), Some(Value::String(s)) => Some(s.clone()), _ => None }
}
/// Nat.read of a decimal text: all digits, else None (a sign, a point or an exponent is no Nat)
pub fn nat_read(s: &str) -> Option<u64> { if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) { s.parse().ok() } else { None } }
/// Json.get_nat: a number or a numeric string, else `d`
pub fn gnat(o: &Value, k: &str, d: u64) -> u64 { raw(o.get(k)).and_then(|r| nat_read(&r)).unwrap_or(d) }
pub fn gbool(o: &Value, k: &str, d: bool) -> bool { o.get(k).and_then(|v| v.as_bool()).unwrap_or(d) }
/// Json.get_obj: the value at `key`, Null when absent
pub fn gobj<'a>(o: &'a Value, k: &str) -> &'a Value { static NULL: Value = Value::Null; o.get(k).unwrap_or(&NULL) }
/// Json.elems: the elements of an array, else none
pub fn elems(v: &Value) -> &[Value] { match v { Value::Array(a) => a, _ => &[] } }
/// Ub.nat / Switch.num: decimal digits read one character at a time (a non-digit wraps as `c - 48` in U32, as in Bend)
pub fn num(s: &str) -> u64 { s.chars().fold(0u64, |acc, c| acc.wrapping_mul(10).wrapping_add((c as u32).wrapping_sub(48) as u64)) }
/// Ub.nat: the number at `key` read through its raw text with `num`
pub fn unat(o: &Value, k: &str) -> u64 { num(&raw(o.get(k)).unwrap_or_default()) }

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"), '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
fn emit(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"), Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()), Value::String(s) => quote(s, out),
        Value::Array(a) => { out.push('['); for (i, x) in a.iter().enumerate() { if i > 0 { out.push(','); } emit(x, out); } out.push(']'); }
        Value::Object(m) => { out.push('{'); for (i, (k, x)) in m.iter().enumerate() { if i > 0 { out.push(','); } quote(k, out); out.push(':'); emit(x, out); } out.push('}'); }
    }
}
/// Json.show: compact text
pub fn show(v: &Value) -> String { let mut s = String::new(); emit(v, &mut s); s }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn json_show_escapes_what_bend_escapes() {
        // rust json_show_escapes_what_bend_escapes: \b and \f are \u0008 and \u000c in Bend; non-ASCII stays raw
        assert_eq!(show(&json!("a\u{8}b\u{c}c\nd\te\u{1}\"\\é")), "\"a\\u0008b\\u000cc\\nd\\te\\u0001\\\"\\\\é\"");
    }
    #[test] fn readers_default_like_bend() {
        let o = parse_or_null(r#"{"s":"x","n":12,"m":"7","f":1.5,"b":true,"a":[1]}"#);
        assert_eq!((gstr(&o, "s", "d"), gstr(&o, "n", "d"), gstr(&o, "zz", "d")), ("x", "d", "d"));
        assert_eq!((gnat(&o, "n", 0), gnat(&o, "m", 0), gnat(&o, "f", 9), gnat(&o, "s", 9), gnat(&o, "zz", 5)), (12, 7, 9, 9, 5));
        assert_eq!(unat(&o, "m"), 7); assert!(gbool(&o, "b", false)); assert_eq!(elems(gobj(&o, "a")).len(), 1); assert!(elems(gobj(&o, "s")).is_empty());
    }
    #[test] fn numbers_keep_their_text() { assert_eq!(show(&parse_or_null(r#"{"a":1.0,"b":10000000000000000000000}"#)), r#"{"a":1.0,"b":10000000000000000000000}"#); }
}
