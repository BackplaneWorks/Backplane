//! Order keys: a manual place on a shelf is a binary fraction 0.b1b2b3...
//! read with zeros after its last digit; keys travel as hex, four bits a character. Dragging a thread
//! gives the hub the shelf's new order and `plan` says which threads get a new key.

pub type Bits = Vec<bool>;

fn hexval(c: char) -> u32 { match c { '0'..='9' => c as u32 - 48, 'a'..='f' => c as u32 - 87, _ => 0 } }
/// Key.read: "" is no key; a character outside 0-9a-f reads as 0000.
pub fn read(s: &str) -> Bits { s.chars().flat_map(|c| { let v = hexval(c); [v & 8 != 0, v & 4 != 0, v & 2 != 0, v & 1 != 0] }).collect() }
/// Key.show: a trailing partial group is padded with zeros.
pub fn show(b: &[bool]) -> String {
    b.chunks(4).map(|g| { let mut v = 0u32; for (i, x) in g.iter().enumerate() { if *x { v |= 8 >> i } } char::from_digit(v, 16).unwrap() }).collect()
}
pub fn cmp(a: &[bool], b: &[bool]) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let n = a.len().max(b.len());
    for i in 0..n { let (x, y) = (a.get(i).copied().unwrap_or(false), b.get(i).copied().unwrap_or(false)); if x != y { return if x { Greater } else { Less } } }
    Equal
}
pub fn lt(a: &[bool], b: &[bool]) -> bool { cmp(a, b) == std::cmp::Ordering::Less }
pub fn after(s: &[bool]) -> Bits {
    match s.first() { None | Some(false) => vec![true], Some(true) => { let mut r = vec![true]; r.extend(after(&s[1..])); r } }
}
fn midz(t: &[bool]) -> Bits {
    match t.first() { None => vec![true], Some(false) => { let mut r = vec![false]; r.extend(midz(&t[1..])); r } Some(true) => vec![false, true] }
}
pub fn mid(a: &[bool], b: &[bool]) -> Bits {
    let Some(&x) = a.first() else { return midz(b) };
    let s = &a[1..];
    let Some(&y) = b.first() else { return vec![true] };
    let t = &b[1..];
    match (x, y) {
        (false, false) => { let mut r = vec![false]; r.extend(mid(s, t)); r }
        (false, true) => { let mut r = vec![false]; r.extend(after(s)); r }
        (true, false) => vec![true],
        (true, true) => { let mut r = vec![true]; r.extend(mid(s, t)); r }
    }
}

/// Order.plan: `xs` are the shelf's threads in their new order with the keys they have now ("" none);
/// `moved` the one dragged. The result is the (id, new key) changes that put the shelf in this order.
pub fn plan(xs: &[(String, String)], moved: &str) -> Vec<(String, String)> {
    let (mut prev, mut past, mut out): (Bits, bool, Vec<(String, String)>) = (vec![], false, vec![]);
    for (i, (id, k)) in xs.iter().enumerate() {
        let rest = &xs[i + 1..];
        let nxt = rest.first().map(|r| read(&r.1)).unwrap_or_default();
        let kk = read(k);
        let skip = past && k.is_empty() && rest.iter().all(|r| r.1.is_empty());
        let (keyed, nkeyed) = (!kk.is_empty(), !nxt.is_empty());
        let keep = keyed && lt(&prev, &kk) && (!nkeyed || lt(&kk, &nxt));
        let fresh = if nkeyed && lt(&prev, &nxt) { mid(&prev, &nxt) } else { after(&prev) };
        if !skip && !keep { out.push((id.clone(), show(&fresh))) }
        prev = if skip { prev } else if keep { kk } else { fresh };
        past = past || id == moved;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn xs(v: &[(&str, &str)]) -> Vec<(String, String)> { v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }
    // law key_mid_above / key_mid_below: a place between two places, and past any place a later one
    #[test] fn key_mid_and_after_fall_in_order() {
        for (a, b) in [("1", "2"), ("4", "8"), ("0", "1"), ("8", "c"), ("ff", "ffff1")] {
            let (a, b) = (read(a), read(b)); if !lt(&a, &b) { continue }
            let m = mid(&a, &b); assert!(lt(&a, &m) && lt(&m, &b), "{a:?} {m:?} {b:?}");
        }
        for k in ["", "1", "8", "f", "ff", "a5"] { let a = read(k); assert!(lt(&a, &after(&a)) || a.is_empty() && !after(&a).is_empty()); }
    }
    #[test] fn key_show_read_round_trip() { for k in ["8", "c", "ff", "123", "0"] { assert_eq!(show(&read(k)), k) } assert_eq!(show(&[true]), "8"); assert_eq!(show(&[true, true, false, false, true]), "c8"); }
    // Order.plan: unkeyed threads get keys after the keyed one above; keys already in order stay
    #[test] fn plan_keeps_keys_in_order_and_gives_the_rest_one() {
        assert_eq!(plan(&xs(&[("a", ""), ("b", ""), ("c", "")]), "c"), xs(&[("a", "8"), ("b", "c"), ("c", "e")]));
        assert_eq!(plan(&xs(&[("a", "4"), ("b", "8")]), "b"), xs(&[]));
        let p = plan(&xs(&[("b", "8"), ("a", "4")]), "b"); assert_eq!(p, xs(&[("b", "2")]));
    }
    // threads past the moved one with no key, with none below them keyed either, keep none
    #[test] fn plan_leaves_the_unkeyed_tail_alone() {
        assert_eq!(plan(&xs(&[("a", "8"), ("b", ""), ("c", "")]), "a"), xs(&[]));
        assert_eq!(plan(&xs(&[("a", ""), ("b", ""), ("c", "")]), "a"), xs(&[("a", "8")]));
    }
}
