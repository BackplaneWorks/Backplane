//! Backplane's CBOR subset: byte-exact encoder and a bounded decoder.
//! Keys in KEYS travel as their index, string values in WORDS as tag 6 over their index,
//! integers in 32 bits as major 0/1, any other number as tag 7 over its raw text.
use anyhow::{bail, ensure, Result};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

include!("wire_dict.rs");

fn index(list: &'static [&'static str], cell: &'static OnceLock<HashMap<&'static str, u32>>) -> &'static HashMap<&'static str, u32> {
    cell.get_or_init(|| list.iter().enumerate().map(|(i, s)| (*s, i as u32)).collect())
}
static K: OnceLock<HashMap<&'static str, u32>> = OnceLock::new();
static W: OnceLock<HashMap<&'static str, u32>> = OnceLock::new();

fn head(out: &mut Vec<u8>, major: u8, n: u32) {
    let mb = major << 5;
    if n < 24 { out.push(mb | n as u8) }
    else if n < 256 { out.extend([mb | 24, n as u8]) }
    else if n < 65536 { out.push(mb | 25); out.extend((n as u16).to_be_bytes()) }
    else { out.push(mb | 26); out.extend(n.to_be_bytes()) }
}
fn text(out: &mut Vec<u8>, s: &str) { head(out, 3, s.len() as u32); out.extend(s.as_bytes()) }

// digits only, 1..10 of them, no leading zero (unless exactly "0"), below 2^32
fn small(digits: &str) -> Option<u32> {
    if digits.is_empty() || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) { return None }
    if digits.len() > 1 && digits.starts_with('0') { return None }
    let v: u64 = digits.parse().ok()?;
    u32::try_from(v).ok()
}
fn number(out: &mut Vec<u8>, raw: &str) {
    if let Some(v) = small(raw) { head(out, 0, v); return }
    if let Some(d) = raw.strip_prefix('-') {
        if let Some(v) = small(d).filter(|v| *v > 0) { head(out, 1, v - 1); return }
    }
    out.push(199); text(out, raw)
}

pub fn encode(value: &Value) -> Vec<u8> {
    let (keys, words) = (index(KEYS, &K), index(WORDS, &W));
    fn go(v: &Value, out: &mut Vec<u8>, keys: &HashMap<&str, u32>, words: &HashMap<&str, u32>) {
        match v {
            Value::Null => out.push(246),
            Value::Bool(b) => out.push(if *b { 245 } else { 244 }),
            Value::Number(n) => number(out, &n.to_string()),
            Value::String(s) => match words.get(s.as_str()) { Some(i) => { out.push(198); head(out, 0, *i) } None => text(out, s) },
            Value::Array(xs) => { head(out, 4, xs.len() as u32); for x in xs { go(x, out, keys, words) } }
            Value::Object(o) => {
                head(out, 5, o.len() as u32);
                for (k, x) in o { match keys.get(k.as_str()) { Some(i) => head(out, 0, *i), None => text(out, k) } go(x, out, keys, words) }
            }
        }
    }
    let mut out = Vec::new(); go(value, &mut out, keys, words); out
}

pub const MAX_FRAME: usize = 524288;
pub fn decode(bytes: &[u8]) -> Result<Value> {
    ensure!(bytes.len() <= MAX_FRAME, "frame exceeds 512 KiB");
    struct R<'a> { b: &'a [u8], at: usize, nodes: usize }
    impl R<'_> {
        fn byte(&mut self) -> Result<u8> { let x = *self.b.get(self.at).ok_or_else(|| anyhow::anyhow!("truncated CBOR"))?; self.at += 1; Ok(x) }
        fn arg(&mut self, ai: u8) -> Result<usize> {
            Ok(match ai { 0..=23 => ai as usize, 24 => self.byte()? as usize,
                25 => (self.byte()? as usize) << 8 | self.byte()? as usize,
                26 => { let mut n = 0usize; for _ in 0..4 { n = n << 8 | self.byte()? as usize } n }
                _ => bail!("unsupported CBOR width") })
        }
        fn item(&mut self, depth: usize) -> Result<Value> {
            self.nodes += 1; ensure!(depth < 64 && self.nodes < 50000, "CBOR nesting/node budget");
            let h = self.byte()?; let (major, ai) = (h >> 5, h & 31);
            if major == 7 { return Ok(match ai { 20 => Value::Bool(false), 21 => Value::Bool(true), 22 => Value::Null, _ => bail!("unsupported scalar") }) }
            if major == 6 {
                ensure!(ai == 6 || ai == 7, "unknown tag");
                let v = self.item(depth + 1)?;
                return match ai {
                    6 => Ok(Value::String(WORDS.get(v.as_u64().ok_or_else(|| anyhow::anyhow!("bad word"))? as usize).ok_or_else(|| anyhow::anyhow!("unknown word"))?.to_string())),
                    _ => { let s = v.as_str().ok_or_else(|| anyhow::anyhow!("bad number text"))?; Ok(Value::Number(s.parse::<Number>()?)) }
                };
            }
            let n = self.arg(ai)?; ensure!(major < 3 || n <= MAX_FRAME, "item budget");
            Ok(match major {
                0 => Value::from(n as u64), 1 => Value::from(-1 - (n as i64)),
                3 => { let end = self.at.checked_add(n).ok_or_else(|| anyhow::anyhow!("length"))?; let t = std::str::from_utf8(self.b.get(self.at..end).ok_or_else(|| anyhow::anyhow!("truncated text"))?)?; self.at = end; Value::String(t.to_string()) }
                4 => { let mut a = Vec::new(); for _ in 0..n { a.push(self.item(depth + 1)?) } Value::Array(a) }
                5 => { let mut o = Map::new(); for _ in 0..n {
                    let k = match self.item(depth + 1)? { Value::String(s) => s, Value::Number(i) => KEYS.get(i.as_u64().ok_or_else(|| anyhow::anyhow!("bad key"))? as usize).ok_or_else(|| anyhow::anyhow!("unknown key"))?.to_string(), _ => bail!("bad key") };
                    let v = self.item(depth + 1)?; o.insert(k, v) ; } Value::Object(o) }
                _ => bail!("unsupported major type"),
            })
        }
    }
    let mut r = R { b: bytes, at: 0, nodes: 0 };
    let v = r.item(0)?; ensure!(r.at == bytes.len(), "trailing CBOR"); Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn round_trip_with_dictionaries() {
        let v = json!({"t":"reply","id":140737488355429_u64,"ok":true,"result":{"x":-5,"y":"日本語🙂","n":null,"f":1.5,"big":4294967296_u64}});
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }
    #[test] fn hello_matches_the_js_hub_bytes() {
        // captured from the real hub 0.14.0: {"backplane":"0.14.0","busy":0,"commit":"94532cfb30a6a6b2579268424e03733c9b1cfe64"}
        let v = json!({"backplane":"0.14.0","busy":0,"commit":"94532cfb30a6a6b2579268424e03733c9b1cfe64"});
        let mut want = vec![0xa3u8, 0x69]; want.extend(b"backplane"); want.extend([0x66]); want.extend(b"0.14.0");
        want.extend([0x64]); want.extend(b"busy"); want.push(0x00); want.extend([0x66]); want.extend(b"commit"); want.extend([0x78, 0x28]);
        want.extend(b"94532cfb30a6a6b2579268424e03733c9b1cfe64");
        assert_eq!(encode(&v), want);
    }
    #[test] fn small_numbers_and_text_numbers() {
        assert_eq!(encode(&json!(0)), vec![0]); assert_eq!(encode(&json!(24)), vec![24, 24]);
        assert_eq!(encode(&json!(-1)), vec![0x20]); assert_eq!(encode(&json!(4294967295_u64)), vec![0x1a, 255, 255, 255, 255]);
        assert_eq!(encode(&json!(4294967296_u64)), { let mut x = vec![199u8, 0x6a]; x.extend(b"4294967296"); x });
    }
    // law cbor_trees_exact: the encoder's lookup holds exactly the append-only dictionaries, each word at its list index
    #[test] fn cbor_trees_exact() {
        for (list, map) in [(KEYS, index(KEYS, &K)), (WORDS, index(WORDS, &W))] {
            assert_eq!(map.len(), list.len(), "no duplicates in a dictionary");
            for (i, w) in list.iter().enumerate() { assert_eq!(map.get(w), Some(&(i as u32))); }
        }
        let mut sorted = KEYS.to_vec(); sorted.sort(); sorted.dedup(); assert_eq!(sorted.len(), KEYS.len());
    }
    #[test] fn reject_malformed() { for b in [&[0x7f][..], &[0x63, 0x61][..], &[0xf6, 0][..]] { assert!(decode(b).is_err()) } }
}
