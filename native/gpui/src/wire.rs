//! Backplane's bounded CBOR subset; dictionaries generated from at build time.
use anyhow::{bail, ensure, Result};
use serde_json::{Map, Value, Number};
include!("wire_dict.rs");
/// A frame the window takes: 16 MiB. It was 512 KiB, and a long thread's view on a tall window went over it (Nov 16): the window dropped the connection and reconnected forever.
const MAX_FRAME: usize = 16 << 20;
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
/// Values a frame may hold: every value takes at least a byte, so the frame's own cap (MAX_FRAME) already bounds it; this only stops a malformed frame early.
/// It was 50 000, and a long thread's view (Nov 16: the Thalassa thread, under 512 KiB) went over it: the window dropped the connection and reconnected forever.
pub const NODE_BUDGET: usize = MAX_FRAME + 1;
pub fn decode(bytes: &[u8]) -> Result<Value> {
    /* i18n-ok: decoder errors, logged */ ensure!(bytes.len() <= MAX_FRAME, "hub frame exceeds 16 MiB");
    struct Reader<'a> { bytes: &'a [u8], at: usize, nodes: usize }
    impl Reader<'_> {
        /* i18n-ok: decoder errors, logged */ fn byte(&mut self) -> Result<u8> { let b = *self.bytes.get(self.at).ok_or_else(|| anyhow::anyhow!("truncated CBOR"))?; self.at += 1; Ok(b) }
        /* i18n-ok: decoder errors, logged */ fn arg(&mut self, ai:u8) -> Result<usize> { Ok(match ai {0..=23=>ai as usize,24=>self.byte()? as usize,25=>(self.byte()? as usize)<<8|self.byte()? as usize,26=>{let mut n=0;for _ in 0..4 {n=(n<<8)|self.byte()? as usize;}n},_=>bail!("unsupported CBOR width")}) }
        fn item(&mut self, depth:usize) -> Result<Value> {
            /* i18n-ok: decoder errors, logged */ self.nodes+=1;ensure!(depth<64 && self.nodes<=NODE_BUDGET,"CBOR nesting/node budget");
            let h=self.byte()?;let major=h>>5;let ai=h&31;
            /* i18n-ok: decoder errors, logged */ if major==7 {return Ok(match ai {20=>Value::Bool(false),21=>Value::Bool(true),22=>Value::Null,_=>bail!("unsupported CBOR scalar")});}
            /* i18n-ok: decoder errors, logged */ if major==6 {ensure!(ai==6||ai==7,"unknown CBOR tag");let v=self.item(depth+1)?;return match ai {6=>Ok(Value::String(WORDS.get(v.as_u64().ok_or_else(||anyhow::anyhow!("invalid word"))? as usize).ok_or_else(||anyhow::anyhow!("unknown word"))?.to_string())),7=>Ok(Value::Number(v.as_str().ok_or_else(||anyhow::anyhow!("invalid number"))?.parse::<Number>()?)),_=>unreachable!()};}
            /* i18n-ok: decoder errors, logged */ let n=self.arg(ai)?;ensure!(major < 3 || n<=MAX_FRAME,"CBOR item budget");
            Ok(match major {
                0=>Value::from(n as u64),1=>Value::from(-1-(n as i64)),
                /* i18n-ok: decoder errors, logged */ 3=>{let end=self.at.checked_add(n).ok_or_else(||anyhow::anyhow!("length overflow"))?;let text=std::str::from_utf8(self.bytes.get(self.at..end).ok_or_else(||anyhow::anyhow!("truncated text"))?)?;self.at=end;Value::String(text.to_string())},
                4=>{let mut a=Vec::new();for _ in 0..n {a.push(self.item(depth+1)?);}Value::Array(a)},
                /* i18n-ok: decoder errors, logged */ 5=>{let mut o=Map::new();for _ in 0..n {let k=self.item(depth+1)?;let k=match k {Value::String(s)=>s,Value::Number(i)=>KEYS.get(i.as_u64().ok_or_else(||anyhow::anyhow!("invalid key"))? as usize).ok_or_else(||anyhow::anyhow!("unknown key"))?.to_string(),_=>bail!("invalid map key")};let v=self.item(depth+1)?;o.insert(k,v);}Value::Object(o)},
                /* i18n-ok: decoder errors, logged */ _=>bail!("unsupported CBOR major type")
            })
        }
    }
    /* i18n-ok: decoder errors, logged */ let mut r=Reader{bytes,at:0,nodes:0};let value=r.item(0)?;ensure!(r.at==bytes.len(),"trailing CBOR");Ok(value)
}
/// Maximum decompressed frame size for `?view=1&z=deflate` (matches the plot cap; `decode` still enforces `MAX_FRAME` for CBOR).
pub const MAX_INFLATED: usize = 32 << 20;

/// Per-connection raw DEFLATE stream decoder (RFC 1951, sync-flushed per frame).
pub struct Inflater { inner: flate2::Decompress }
impl Inflater {
    pub fn new() -> Self { Self { inner: flate2::Decompress::new(false) } }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(bytes.len().saturating_mul(3).min(MAX_INFLATED));
        let mut pos = 0usize;
        let mut chunk = [0u8; 16384];
        loop {
            let before_in = self.inner.total_in();
            let before_out = self.inner.total_out();
            let status = self.inner.decompress(&bytes[pos..], &mut chunk, flate2::FlushDecompress::Sync)
                /* i18n-ok: decoder errors, logged */ .map_err(|e| anyhow::anyhow!("deflate frame error: {e}"))?;
            let consumed = (self.inner.total_in() - before_in) as usize;
            let produced = (self.inner.total_out() - before_out) as usize;
            pos += consumed;
            if produced > 0 {
                /* i18n-ok: decoder errors, logged */ ensure!(out.len().saturating_add(produced) <= MAX_INFLATED, "inflated frame exceeds 32 MiB");
                out.extend_from_slice(&chunk[..produced]);
            }
            if status == flate2::Status::StreamEnd { break; }
            if pos >= bytes.len() && produced < chunk.len() { break; }
            if consumed == 0 && produced == 0 {
                /* i18n-ok: decoder errors, logged */ ensure!(pos >= bytes.len(), "deflate frame stalled before end of input");
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)] mod tests {

    #[test] fn inflater_roundtrips_multiple_sync_flushed_frames_with_shared_window() {
        use flate2::{Compress, Compression, FlushCompress};
        let mut comp = Compress::new(Compression::fast(), false);
        let mut inf = super::Inflater::new();
        let f1 = br#"{"t":"reply","id":1,"result":{"rows":[{"id":"a","text":"hello world hello world"}]}}"#;
        let f2 = br#"{"t":"reply","id":2,"result":{"rows":[{"id":"a","text":"hello world hello world"},{"id":"b","text":"again"}]}}"#;
        let compress_frame = |c: &mut Compress, src: &[u8]| -> Vec<u8> {
            let mut out = Vec::new();
            let mut pos = 0;
            let mut buf = [0u8; 4096];
            loop {
                let bi = c.total_in(); let bo = c.total_out();
                c.compress(&src[pos..], &mut buf, FlushCompress::Sync).unwrap();
                pos += (c.total_in() - bi) as usize;
                let n = (c.total_out() - bo) as usize;
                out.extend_from_slice(&buf[..n]);
                if pos >= src.len() && n < buf.len() { break; }
            }
            out
        };
        let c1 = compress_frame(&mut comp, f1);
        let c2 = compress_frame(&mut comp, f2);
        assert_eq!(inf.feed(&c1).unwrap(), f1);
        assert_eq!(inf.feed(&c2).unwrap(), f2);
        // Second frame reuses the first frame's dictionary window, so it compresses much smaller than f2.
        assert!(c2.len() < f2.len() / 2);
        // Corrupt bytes fail cleanly.
        assert!(inf.feed(&[0xff, 0xff, 0xff, 0xff]).is_err());
    }

    #[test] fn a_long_view_under_the_frame_cap_decodes() {
        // an array of 100 000 small integers (one byte each) is 100 KB: over the old 50 000-value budget, well under the frame cap
        let mut b = vec![0x9a, 0x00, 0x01, 0x86, 0xa0]; b.extend(std::iter::repeat(0x01u8).take(100_000));
        assert_eq!(super::decode(&b).unwrap().as_array().map(|a| a.len()), Some(100_000));
    }    use super::*;use serde_json::json;
    #[test] fn unicode_large_ids_and_dictionaries() {
        let v=json!({"id":140737488355429_u64,"m":"view.open","p":{"text":"日本語🙂","ok":true}});
        assert_eq!(decode(&encode(&v)).unwrap(),v);
        assert_eq!(decode(&[0xa1,1,0xc6,5]).unwrap(),json!({"t":"reply"}));
    }
    #[test] fn reject_malformed() {for b in [&[0x7f][..],&[0x63,0x61][..],&[0xf6,0][..]] {assert!(decode(b).is_err());}}
}
