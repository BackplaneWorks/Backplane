//! Message attachments and Claude stream-json input.
use serde_json::{json, Value};

// ---- attachments ----
pub const INLINE_MAX: u64 = 3_932_160;
fn is_att(l: &str) -> bool { let t = l.trim(); t.starts_with("[attachment: ") && t.ends_with(']') }
fn att_path(l: &str) -> String { let t = l.trim(); t[13..t.len() - 1].trim().to_string() }
/// Att.paths: the attachment lines at the end of a message (blank lines between them allowed).
pub fn att_paths(text: &str) -> Vec<String> {
    let ls: Vec<&str> = text.split('\n').collect(); let mut out = vec![]; let mut i = ls.len();
    while i > 0 { let l = ls[i - 1]; if is_att(l) { out.push(att_path(l)); i -= 1 } else if l.trim().is_empty() { i -= 1 } else { break } }
    out.reverse(); out
}
pub fn mime(path: &str) -> &'static str { match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() { "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "webp" => "image/webp", _ => "" } }
pub fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"; let mut o = String::with_capacity(bytes.len() / 3 * 4 + 4);
    for c in bytes.chunks(3) { let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(A[(n >> 18) as usize & 63] as char); o.push(A[(n >> 12) as usize & 63] as char); o.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' }); o.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' }) }
    o
}
/// Media.img: an attached image small enough to send inline (1 byte up to INLINE_MAX); bigger ones Claude reads by path.
pub fn read_images(text: &str) -> Vec<(String, String)> {
    att_paths(text).into_iter().filter(|p| !mime(p).is_empty()).filter_map(|p| { let n = std::fs::metadata(&p).ok()?.len(); if n == 0 || n > INLINE_MAX { return None } Some((mime(&p).to_string(), b64(&std::fs::read(&p).ok()?))) }).collect()
}
/// Att.claude + Claude.tag: the stream-json user line, images first, then the text; the command's uuid first.
pub fn claude_line(text: &str, imgs: &[(String, String)], uuid: &str) -> String { claude_line_p(text, imgs, uuid, false) }
/// `now`: the CLI's `priority: "now"`: a person's message into a running turn interrupts it and is taken at once (a steer), instead of waiting for the turn's end.
pub fn claude_line_p(text: &str, imgs: &[(String, String)], uuid: &str, now: bool) -> String {
    let mut content: Vec<Value> = imgs.iter().map(|(m, d)| json!({"type":"image","source":{"type":"base64","media_type":m,"data":d}})).collect();
    content.push(json!({"type":"text","text":text}));
    let mut o = serde_json::Map::new(); if !uuid.is_empty() { o.insert("uuid".into(), json!(uuid)); }
    if now { o.insert("priority".into(), json!("now")); }
    o.insert("type".into(), json!("user")); o.insert("message".into(), json!({"role":"user","content":content})); Value::Object(o).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("in-{n}")) }
    #[test] fn a_steering_line_carries_priority_now_and_a_plain_one_does_not() {
        let l: Value = serde_json::from_str(&claude_line_p("x", &[], "U", true)).unwrap(); assert_eq!(l["priority"], "now");
        let l: Value = serde_json::from_str(&claude_line("x", &[], "U")).unwrap(); assert!(l.get("priority").is_none());
    }
    #[test] fn attachment_lines_at_the_end_are_the_attachments() {
        assert_eq!(att_paths("Look\n\n[attachment: /a/x.png]\n\n[attachment: /a/y.pdf]"), ["/a/x.png", "/a/y.pdf"]); assert!(att_paths("no\n[attachment: /a/x.png]\nmore").is_empty()); assert_eq!(mime("/a/X.JPEG"), "image/jpeg"); assert_eq!(mime("/a/x.pdf"), "");
    }
    #[test] fn base64_matches_the_standard() { assert_eq!(b64(b""), ""); assert_eq!(b64(b"f"), "Zg=="); assert_eq!(b64(b"fo"), "Zm8="); assert_eq!(b64(b"foo"), "Zm9v"); assert_eq!(b64(b"foobar"), "Zm9vYmFy"); }
    #[test] fn images_go_first_in_the_line_and_big_ones_stay_paths() {
        let r = tmp("img"); let ok = r.join("a.png"); std::fs::write(&ok, [1u8, 2, 3]).unwrap(); let big = r.join("b.png"); std::fs::write(&big, vec![0u8; INLINE_MAX as usize + 1]).unwrap(); let empty = r.join("c.png"); std::fs::write(&empty, b"").unwrap();
        let text = format!("hi\n\n[attachment: {}]\n[attachment: {}]\n[attachment: {}]", ok.display(), big.display(), empty.display());
        let imgs = read_images(&text); assert_eq!(imgs, vec![("image/png".to_string(), "AQID".to_string())]);
        let l: Value = serde_json::from_str(&claude_line(&text, &imgs, "U")).unwrap(); assert_eq!(l["uuid"], "U"); assert_eq!(l["message"]["content"][0]["type"], "image"); assert_eq!(l["message"]["content"][1]["text"], text.as_str());
    }
}
