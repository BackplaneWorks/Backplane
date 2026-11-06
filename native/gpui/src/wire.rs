//! Backplane's bounded CBOR subset; dictionaries generated from at build time.
use anyhow::{bail, ensure, Result};
use serde_json::{Map, Value, Number};
include!("wire_dict.rs");
const MAX_FRAME: usize = 524288;
fn arg(out: &mut Vec<u8>, major: u8, n: u32) {
    let h = major << 5;
    match n {
        0..=23 => out.push(h | n as u8),
        24..=255 => out.extend([h | 24, n as u8]),
        256..=65535 => { out.push(h | 25); out.extend((n as u16).to_be_bytes()); },
        _ => { out.push(h | 26); out.extend(n.to_be_bytes()); }
    }
}
pub fn encode(value: &Value) -> Vec<u8> {
    fn go(v: &Value, out: &mut Vec<u8>) {
        match v {
            Value::Null => out.push(246),
            Value::Bool(b) => out.push(if *b {245} else {244}),
            Value::Number(n) => if let Some(n) = n.as_u64().filter(|n| *n <= u32::MAX as u64) { arg(out, 0, n as u32) } else { out.push(199); go(&Value::String(n.to_string()), out); },
            Value::String(s) => { arg(out, 3, s.len() as u32); out.extend(s.as_bytes()); },
            Value::Array(xs) => { arg(out, 4, xs.len() as u32); for x in xs {go(x, out);} },
            Value::Object(xs) => { arg(out, 5, xs.len() as u32); for (k,v) in xs {go(&Value::String(k.clone()),out);go(v,out);} }
        }
    }
    let mut bytes = Vec::new(); go(value, &mut bytes); bytes
}
pub fn decode(bytes: &[u8]) -> Result<Value> {
    ensure!(bytes.len() <= MAX_FRAME, "hub frame exceeds 512 KiB");
    struct Reader<'a> { bytes: &'a [u8], at: usize, nodes: usize }
    impl Reader<'_> {
        fn byte(&mut self) -> Result<u8> { let b = *self.bytes.get(self.at).ok_or_else(|| anyhow::anyhow!("truncated CBOR"))?; self.at += 1; Ok(b) }
        fn arg(&mut self, ai:u8) -> Result<usize> { Ok(match ai {0..=23=>ai as usize,24=>self.byte()? as usize,25=>(self.byte()? as usize)<<8|self.byte()? as usize,26=>{let mut n=0;for _ in 0..4 {n=(n<<8)|self.byte()? as usize;}n},_=>bail!("unsupported CBOR width")}) }
        fn item(&mut self, depth:usize) -> Result<Value> {
            self.nodes+=1;ensure!(depth<64 && self.nodes<50000,"CBOR nesting/node budget");
            let h=self.byte()?;let major=h>>5;let ai=h&31;
            if major==7 {return Ok(match ai {20=>Value::Bool(false),21=>Value::Bool(true),22=>Value::Null,_=>bail!("unsupported CBOR scalar")});}
            if major==6 {ensure!(ai==6||ai==7,"unknown CBOR tag");let v=self.item(depth+1)?;return match ai {6=>Ok(Value::String(WORDS.get(v.as_u64().ok_or_else(||anyhow::anyhow!("invalid word"))? as usize).ok_or_else(||anyhow::anyhow!("unknown word"))?.to_string())),7=>Ok(Value::Number(v.as_str().ok_or_else(||anyhow::anyhow!("invalid number"))?.parse::<Number>()?)),_=>unreachable!()};}
            let n=self.arg(ai)?;ensure!(major < 3 || n<=MAX_FRAME,"CBOR item budget");
            Ok(match major {
                0=>Value::from(n as u64),1=>Value::from(-1-(n as i64)),
                3=>{let end=self.at.checked_add(n).ok_or_else(||anyhow::anyhow!("length overflow"))?;let text=std::str::from_utf8(self.bytes.get(self.at..end).ok_or_else(||anyhow::anyhow!("truncated text"))?)?;self.at=end;Value::String(text.to_string())},
                4=>{let mut a=Vec::new();for _ in 0..n {a.push(self.item(depth+1)?);}Value::Array(a)},
                5=>{let mut o=Map::new();for _ in 0..n {let k=self.item(depth+1)?;let k=match k {Value::String(s)=>s,Value::Number(i)=>KEYS.get(i.as_u64().ok_or_else(||anyhow::anyhow!("invalid key"))? as usize).ok_or_else(||anyhow::anyhow!("unknown key"))?.to_string(),_=>bail!("invalid map key")};let v=self.item(depth+1)?;o.insert(k,v);}Value::Object(o)},
                _=>bail!("unsupported CBOR major type")
            })
        }
    }
    let mut r=Reader{bytes,at:0,nodes:0};let value=r.item(0)?;ensure!(r.at==bytes.len(),"trailing CBOR");Ok(value)
}
#[cfg(test)] mod tests {
    use super::*;use serde_json::json;
    #[test] fn unicode_large_ids_and_dictionaries() {
        let v=json!({"id":140737488355429_u64,"m":"view.open","p":{"text":"日本語🙂","ok":true}});
        assert_eq!(decode(&encode(&v)).unwrap(),v);
        assert_eq!(decode(&[0xa1,1,0xc6,5]).unwrap(),json!({"t":"reply"}));
    }
    #[test] fn reject_malformed() {for b in [&[0x7f][..],&[0x63,0x61][..],&[0xf6,0][..]] {assert!(decode(b).is_err());}}
}
