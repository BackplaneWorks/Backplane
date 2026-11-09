//! Gmail (v1), pure, as 0.14's core/google.bend: the request paths, what an agent reads of a list and of a message, and composing mail (RFC 5322 and MIME):
//! replies, attachments, HTML, signatures, the multipart upload, and the words the person approves before a send.
use super::oauth::enc;
use base64::{engine::general_purpose::{STANDARD as B64, URL_SAFE as B64U}, Engine};
use serde_json::{json, Value};

pub const ME: &str = "/gmail/v1/users/me";
pub const BODY_CAP: usize = 20000;
/// the most an attachment, and all of them together, may be (raw bytes; base64 makes them a third more, under Gmail's 25 MB)
pub const ATT_MAX: u64 = 18874368;
pub const ATT_COUNT: usize = 20;

pub fn search_path(q: &str, max: u64) -> String { format!("{ME}/messages?q={}&maxResults={max}", enc(q)) }
pub fn get_path(id: &str, full: bool) -> String {
    format!("{ME}/messages/{}{}", enc(id), if full { "?format=full" } else { "?format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Subject&metadataHeaders=Date" })
}
const META_Q: &str = "format=metadata&metadataHeaders=Message-ID&metadataHeaders=References&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=Reply-To&metadataHeaders=To&metadataHeaders=Cc";
pub fn msg_meta_path(id: &str) -> String { format!("{ME}/messages/{}?{META_Q}", enc(id)) }
pub fn thread_path(id: &str) -> String { format!("{ME}/threads/{}?{META_Q}", enc(id)) }
pub fn draft_path(id: &str) -> String { format!("{ME}/drafts/{}?format=full", enc(id)) }
pub fn draft_send_path() -> String { format!("{ME}/drafts/send") }
pub fn upload_send() -> String { "/upload/gmail/v1/users/me/messages/send?uploadType=multipart".into() }
pub fn upload_draft(id: &str) -> String { if id.is_empty() { "/upload/gmail/v1/users/me/drafts?uploadType=multipart".into() } else { format!("/upload/gmail/v1/users/me/drafts/{}?uploadType=multipart", enc(id)) } }

pub fn ids(j: &Value) -> Vec<String> { j["messages"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect()).unwrap_or_default() }

// ---- reading ----

/// Mail.hdr: a header of a part (any case), or ""
pub fn hdr(part: &Value, name: &str) -> String {
    part["headers"].as_array().and_then(|hs| hs.iter().find(|h| h["name"].as_str().map_or(false, |n| n.eq_ignore_ascii_case(name)))).and_then(|h| h["value"].as_str()).unwrap_or("").to_string()
}
fn unread(m: &Value) -> &'static str { if m["labelIds"].as_array().map_or(false, |a| a.iter().any(|x| x == "UNREAD")) { " [unread]" } else { "" } }
/// Google.msg_line
pub fn msg_line(m: &Value) -> String {
    let p = &m["payload"];
    format!("- id {}{}  {}\n  From: {}\n  Subject: {}\n  {}", m["id"].as_str().unwrap_or(""), unread(m), hdr(p, "date"), hdr(p, "from"), hdr(p, "subject"), entities(m["snippet"].as_str().unwrap_or("")))
}
/// Google.msg_list
pub fn msg_list(ms: &[Value]) -> String { if ms.is_empty() { "No messages.".into() } else { format!("{} messages:\n{}", ms.len(), ms.iter().map(msg_line).collect::<Vec<_>>().join("\n")) } }
/// Mime.parts: the payload and every part under it, in document order
pub fn parts(p: &Value) -> Vec<Value> { let mut v = vec![]; fn go(p: &Value, v: &mut Vec<Value>) { if p.is_object() { v.push(p.clone()); for c in p["parts"].as_array().into_iter().flatten() { go(c, v) } } } go(p, &mut v); v }
fn data(p: &Value) -> &str { p["body"]["data"].as_str().unwrap_or("") }
fn is(p: &Value, want: &str) -> bool { p["mimeType"] == want && !data(p).is_empty() && p["filename"].as_str().unwrap_or("").is_empty() }
/// Gb64.decode: base64 or base64url, padded or not, junk skipped
pub fn b64_any(s: &str) -> Vec<u8> {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || "+/-_".contains(*c)).map(|c| match c { '+' => '-', '/' => '_', c => c }).collect();
    let pad = (4 - t.len() % 4) % 4; B64U.decode(format!("{t}{}", "=".repeat(pad))).unwrap_or_default()
}
/// Mime.text: a part's text, its transfer encoding undone and its charset read (Latin-1 and Windows-1252 as Latin-1, else UTF-8)
fn text(p: &Value) -> String {
    let bs = b64_any(data(p)); let ct = hdr(p, "content-type").to_lowercase();
    if ct.contains("8859-1") || ct.contains("1252") { bs.iter().map(|b| *b as char).collect() } else { String::from_utf8_lossy(&bs).into_owned() }
}
/// Mime.body: its text/plain part, else its HTML as text, else the snippet
pub fn body(ps: &[Value], snippet: &str) -> String {
    if let Some(p) = ps.iter().find(|p| is(p, "text/plain")) { return text(p).replace("\r\n", "\n") }
    if let Some(p) = ps.iter().find(|p| is(p, "text/html")) { return html_text(&text(p)) }
    entities(snippet)
}
fn atts(ps: &[Value]) -> Vec<String> {
    ps.iter().filter(|p| !p["filename"].as_str().unwrap_or("").is_empty()).map(|p| format!("{} ({}, {} bytes)", p["filename"].as_str().unwrap_or(""), p["mimeType"].as_str().unwrap_or(""), p["body"]["size"].as_u64().map_or("?".to_string(), |n| n.to_string()))).collect()
}
fn line(name: &str, v: &str) -> String { if v.is_empty() { String::new() } else { format!("{name}: {v}\n") } }
/// Str.cap
pub fn cap(s: &str, n: usize) -> String { if s.chars().count() > n { format!("{}\n[... truncated]", s.chars().take(n).collect::<String>()) } else { s.into() } }
/// Google.msg_full: a whole message (format=full) as text: headers, labels, attachments, body
pub fn msg_full(m: &Value) -> String {
    let p = &m["payload"]; let ps = parts(p);
    let labels = m["labelIds"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    format!("id: {}\nthread: {}\n{}{}{}{}{}{}{}\n{}", m["id"].as_str().unwrap_or(""), m["threadId"].as_str().unwrap_or(""), line("From", &hdr(p, "from")), line("To", &hdr(p, "to")), line("Cc", &hdr(p, "cc")),
        line("Date", &hdr(p, "date")), line("Subject", &hdr(p, "subject")), line("Labels", &labels), line("Attachments", &atts(&ps).join("; ")), cap(&body(&ps, m["snippet"].as_str().unwrap_or("")), BODY_CAP))
}

// ---- HTML to text ----

/// Html.entities: &amp; &lt; &gt; &quot; &nbsp; &#39; &#x27; decoded; unknown ones kept
pub fn entities(s: &str) -> String {
    let mut o = String::new(); let mut rest = s;
    while let Some(i) = rest.find('&') {
        o.push_str(&rest[..i]); let r = &rest[i..];
        let end = r[1..].find(|c: char| c == ';' || c == '&' || c.is_whitespace()).map(|j| j + 1).filter(|j| *j <= 12 && r[*j..].starts_with(';'));
        match end {
            Some(j) => {
                let name = &r[1..j];
                let v = match name { "nbsp" => Some(' '), "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''),
                    n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32), n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32), _ => None };
                match v { Some(c) => { o.push(c); rest = &r[j + 1..] } None => { o.push('&'); rest = &r[1..] } }
            }
            None => { o.push('&'); rest = &r[1..] }
        }
    }
    o.push_str(rest); o
}
/// Html.to_text: tags dropped (style and script with their content), breaks at br, /p, /div, /tr, li, /h*, entities decoded, lines trimmed, blank runs as one
pub fn html_text(s: &str) -> String {
    let (mut out, mut tag, mut intag, mut skip) = (String::new(), String::new(), false, false);
    for c in s.chars() {
        if intag {
            if c == '>' {
                intag = false; let n = tag.trim().to_lowercase();
                if n.starts_with("style") || n.starts_with("script") { skip = true } else if n.starts_with("/style") || n.starts_with("/script") { skip = false }
                if !skip && (n.starts_with("br") || n.starts_with("/p") || n.starts_with("/div") || n.starts_with("/tr") || n.starts_with("li") || n.starts_with("/h") || n.starts_with("p ") || n == "p") { out.push('\n') }
            } else { tag.push(c) }
        } else if c == '<' { intag = true; tag.clear() } else if !skip { out.push(c) }
    }
    squeeze(&entities(&out))
}
fn squeeze(s: &str) -> String {
    let mut v: Vec<&str> = vec![]; let mut blank = false;
    for l in s.lines().map(str::trim) { if l.is_empty() { if !blank && !v.is_empty() { v.push("") } blank = true } else { v.push(l); blank = false } }
    while v.last() == Some(&"") { v.pop(); } v.join("\n")
}
pub fn html_esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

// ---- composing ----

/// Mail.clean: a header value: every control character a space (a To or Subject cannot add headers)
pub fn clean(s: &str) -> String { s.chars().map(|c| if (c as u32) < 32 || c as u32 == 127 { ' ' } else { c }).collect() }
/// Mail.subject: as is when ASCII, else RFC 2047 encoded words of at most 11 characters, folded
pub fn subject(s: &str) -> String {
    let c = clean(s);
    if c.is_ascii() { return c }
    let cs: Vec<char> = c.chars().collect();
    cs.chunks(11).map(|w| format!("=?UTF-8?B?{}?=", B64.encode(w.iter().collect::<String>().as_bytes()))).collect::<Vec<_>>().join("\r\n ")
}
fn b64lines(bytes: &[u8]) -> String { let e = B64.encode(bytes); let mut o = String::new(); for ch in e.as_bytes().chunks(76) { o.push_str(std::str::from_utf8(ch).unwrap_or("")); o.push_str("\r\n") } o }
fn text_part(ctype: &str, t: &str) -> String { format!("Content-Type: {ctype}; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}", b64lines(t.replace("\r\n", "\n").replace('\n', "\r\n").as_bytes())) }
/// Gmail.signed: the signature after a body: plain text after a blank line; in HTML after two breaks, escaped, its breaks kept
pub fn signed(html: bool, body: &str, sig: &str) -> String {
    if sig.trim().is_empty() { body.into() } else if html { format!("{body}<br><br>{}", html_esc(sig).lines().collect::<Vec<_>>().join("<br>")) } else { format!("{body}\n\n{sig}") }
}
fn body_block(html: bool, body: &str, ba: &str) -> String {
    if html { format!("Content-Type: multipart/alternative; boundary=\"{ba}\"\r\n\r\n--{ba}\r\n{}--{ba}\r\n{}--{ba}--\r\n", text_part("text/plain", &html_text(body)), text_part("text/html", body)) } else { text_part("text/plain", body) }
}
fn hdr_line(name: &str, v: &str) -> String { if v.trim().is_empty() { String::new() } else { format!("{name}: {}\r\n", clean(v)) } }
pub fn basename(p: &str) -> String { p.rsplit('/').next().unwrap_or("").to_string() }
/// Gmail.mime_type: a file's content type from its extension
pub fn mime_type(name: &str) -> &'static str {
    let e = if name.contains('.') { name.rsplit('.').next().unwrap_or("").to_lowercase() } else { String::new() };
    match e.as_str() { "pdf" => "application/pdf", "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "svg" => "image/svg+xml", "txt" => "text/plain", "csv" => "text/csv",
        "html" | "htm" => "text/html", "json" => "application/json", "zip" => "application/zip", "gz" => "application/gzip", "step" | "stp" => "model/step", e if e.starts_with("kicad_") => "text/plain", _ => "application/octet-stream" }
}
fn fallback(s: &str) -> String { s.chars().map(|c| if (c as u32) < 32 || (c as u32) > 126 || c == '"' || c == '\\' { '_' } else { c }).collect() }
fn att_head(b: &str, path: &str) -> String {
    let name = basename(path); let fb = fallback(&name);
    format!("--{b}\r\nContent-Type: {}; name=\"{fb}\"\r\nContent-Disposition: attachment; filename=\"{fb}\"{}\r\nContent-Transfer-Encoding: base64\r\n\r\n", mime_type(&name), if fb == name { String::new() } else { format!("; filename*=UTF-8''{}", enc(&name)) })
}
/// Gmail.paths: a JSON array of paths, or one comma-separated string
pub fn paths(v: Option<&Value>) -> Vec<String> {
    let raw: Vec<String> = match v { Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(), Some(Value::String(s)) => s.split(',').map(str::to_string).collect(), _ => vec![] };
    raw.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// A message to send or save, as the person sees it.
#[derive(Debug, Clone, Default)]
pub struct Msg { pub from: String, pub to: String, pub cc: String, pub bcc: String, pub subject: String, pub html: bool, pub body: String, pub irt: String, pub refs: String, pub thread: String, pub inreply: String, pub paths: Vec<String> }
/// The original a reply answers
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // id: kept as 0.14's Gorig has it
pub struct Orig { pub id: String, pub thread: String, pub msgid: String, pub refs: String, pub subject: String, pub from: String, pub replyto: String }
pub fn orig(m: &Value) -> Orig { let p = &m["payload"]; Orig { id: m["id"].as_str().unwrap_or("").into(), thread: m["threadId"].as_str().unwrap_or("").into(), msgid: hdr(p, "message-id").trim().into(), refs: hdr(p, "references").trim().into(), subject: hdr(p, "subject"), from: hdr(p, "from"), replyto: hdr(p, "reply-to") } }
pub fn orig_thread(j: &Value) -> Orig { orig(j["messages"].as_array().and_then(|a| a.last()).unwrap_or(j)) }
/// Gmail.reply_subject: the one given, else "Re: " the original's (not twice)
pub fn reply_subject(given: &str, orig: &str) -> String { if !given.trim().is_empty() { given.into() } else { let o = orig.trim(); if o.to_lowercase().starts_with("re:") { o.into() } else { format!("Re: {o}") } } }
pub fn references(refs: &str, msgid: &str) -> String { format!("{refs} {msgid}").trim().to_string() }
pub fn reply_to_addr(replyto: &str, from: &str) -> String { if replyto.trim().is_empty() { from.into() } else { replyto.into() } }
pub fn inreply(subject: &str, from: &str) -> String { if subject.trim().is_empty() && from.trim().is_empty() { String::new() } else { format!("\"{}\" from {}", clean(subject), clean(from)) } }

/// The message from a tool's arguments (and the original a reply answers, the account's signature).
pub fn msg_of(a: &Value, from: &str, o: Option<&Orig>, sig: &str) -> Msg {
    let g = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let html = a.get("html").and_then(Value::as_bool).unwrap_or(false);
    let to = if g("to").is_empty() { o.map_or(String::new(), |o| reply_to_addr(&o.replyto, &o.from)) } else { g("to") };
    let subj = a.get("subject").and_then(Value::as_str).unwrap_or("");
    Msg { from: from.into(), to, cc: g("cc"), bcc: g("bcc"), subject: o.map_or(subj.to_string(), |o| reply_subject(subj, &o.subject)), html,
        body: signed(html, a.get("body").and_then(Value::as_str).unwrap_or(""), sig), irt: o.map_or(String::new(), |o| o.msgid.clone()), refs: o.map_or(String::new(), |o| references(&o.refs, &o.msgid)),
        thread: o.map_or(String::new(), |o| o.thread.clone()), inreply: o.map_or(String::new(), |o| inreply(&o.subject, &o.from)), paths: paths(a.get("attachments")) }
}
/// Gmail.content: any argument that makes a message (gmail_send {draft} sends a draft only as it is)
pub fn content(a: &Value) -> bool {
    ["to", "cc", "bcc", "subject", "body", "reply_to", "thread"].iter().any(|k| !a.get(*k).and_then(Value::as_str).unwrap_or("").trim().is_empty()) || a.get("html").and_then(Value::as_bool).unwrap_or(false) || !paths(a.get("attachments")).is_empty()
}
/// Gm.problem: what is wrong before any file is looked at
pub fn problem(m: &Msg, body_given: &str) -> Option<String> {
    if m.to.trim().is_empty() { return Some("to is required (or reply_to / thread to reply)".into()) }
    if body_given.is_empty() && m.paths.is_empty() { return Some("body is required".into()) }
    None
}
/// Gmail.att_err over files read: too many, not absolute, missing, unreadable, too big, too much in all. The files' bytes, or why not.
pub fn read_atts(ps: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if ps.len() > ATT_COUNT { return Err(format!("at most {ATT_COUNT} attachments")) }
    let mut v = vec![]; let mut sum = 0u64;
    for p in ps {
        if !p.starts_with('/') { return Err(format!("{p} is not an absolute path")) }
        let md = std::fs::metadata(p).map_err(|_| format!("{p} is not an existing file"))?;
        if !md.is_file() { return Err(format!("{p} is not an existing file")) }
        if md.len() > ATT_MAX { return Err(format!("{p} is over {}", size(ATT_MAX))) }
        sum += md.len(); if sum > ATT_MAX { return Err(format!("the attachments are over {} in all", size(ATT_MAX))) }
        v.push((p.clone(), std::fs::read(p).map_err(|_| format!("{p} cannot be read"))?));
    }
    Ok(v)
}
/// Gmail.size: 512 B, 182 KB, 2.5 MB, 14 MB
pub fn size(n: u64) -> String {
    if n < 1024 { format!("{n} B") } else if n < 1024000 { format!("{} KB", (n + 512) / 1024) } else { let t = n * 10 / 1048576; if t < 100 { format!("{}.{} MB", t / 10, t % 10) } else { format!("{} MB", t / 10) } }
}
/// The whole message (RFC 5322): headers, the text (and HTML), the attachments; the boundaries from a nonce base64 cannot hold ("=_bp").
pub fn compose(m: &Msg, atts: &[(String, Vec<u8>)], nonce: &str) -> Vec<u8> {
    let head = format!("{}To: {}\r\n{}{}Subject: {}\r\n{}{}MIME-Version: 1.0\r\n", hdr_line("From", &m.from), clean(&m.to), hdr_line("Cc", &m.cc), hdr_line("Bcc", &m.bcc), subject(&m.subject), hdr_line("In-Reply-To", &m.irt), hdr_line("References", &m.refs));
    let blk = body_block(m.html, &m.body, &format!("=_bpa_{nonce}"));
    if atts.is_empty() { return format!("{head}{blk}").into_bytes() }
    let b = format!("=_bp_{nonce}");
    let mut out = format!("{head}Content-Type: multipart/mixed; boundary=\"{b}\"\r\n\r\n--{b}\r\n{blk}").into_bytes();
    for (p, bytes) in atts { out.extend(att_head(&b, p).into_bytes()); out.extend(b64lines(bytes).into_bytes()); }
    out.extend(format!("--{b}--\r\n").into_bytes()); out
}
/// The multipart/related upload: the JSON part (the thread it joins; a draft's id), then the message. (Content-Type header, body)
pub fn upload(json: &Value, message: &[u8], nonce: &str) -> (String, Vec<u8>) {
    let bnd = format!("=_bpu_{nonce}");
    let mut b = format!("--{bnd}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{json}\r\n--{bnd}\r\nContent-Type: message/rfc822\r\n\r\n").into_bytes();
    b.extend_from_slice(message); b.extend(format!("\r\n--{bnd}--\r\n").into_bytes());
    (format!("Content-Type: multipart/related; boundary=\"{bnd}\""), b)
}
pub fn json_send(thread: &str) -> Value { if thread.is_empty() { json!({}) } else { json!({"threadId":thread}) } }
pub fn json_draft(draft: &str, thread: &str) -> Value { let mut j = json!({"message":json_send(thread)}); if !draft.is_empty() { j["id"] = json!(draft) } j }

fn shown(html: bool, body: &str) -> String {
    let capn = |s: &str| { let n = s.chars().count(); if n > BODY_CAP { format!("{}… ({} more characters)", s.chars().take(BODY_CAP).collect::<String>(), n - BODY_CAP) } else { s.to_string() } };
    if html { format!("{}\n\n(sent as HTML)", capn(&html_text(body))) } else { capn(body) }
}
pub fn att_line(atts: &[(String, Vec<u8>)]) -> String { atts.iter().map(|(p, b)| format!("{} ({})", basename(p), size(b.len() as u64))).collect::<Vec<_>>().join(", ") }
/// Gmail.review: what the person approves: every header that matters and the whole text
pub fn review(draft: &str, account: &str, m: &Msg, atts: &str) -> String {
    format!("{}{}{}{}{}{}{}{}{}{}\n{}", line("Draft", draft), line("Account", account), line("From", &m.from), line("To", &clean(&m.to)), line("Cc", &clean(&m.cc)), line("Bcc", &clean(&m.bcc)),
        line("Subject", &clean(&m.subject)), line("In reply to", &m.inreply), line("Attachments", atts), line("Format", if m.html { "HTML" } else { "" }), shown(m.html, &m.body))
}
/// Gmail.draft_review: a drafts.get answer (format=full) as the approval to send it
pub fn draft_review(account: &str, j: &Value) -> String {
    let m = &j["message"]; let p = &m["payload"]; let ps = parts(p);
    let atts: Vec<String> = ps.iter().filter(|p| !p["filename"].as_str().unwrap_or("").is_empty()).map(|p| format!("{} ({})", p["filename"].as_str().unwrap_or(""), size(p["body"]["size"].as_u64().unwrap_or(0)))).collect();
    let msg = Msg { from: hdr(p, "from"), to: hdr(p, "to"), cc: hdr(p, "cc"), bcc: hdr(p, "bcc"), subject: hdr(p, "subject"), body: body(&ps, m["snippet"].as_str().unwrap_or("")), ..Default::default() };
    review(j["id"].as_str().unwrap_or(""), account, &msg, &atts.join(", "))
}
pub fn draft_saved(j: &Value) -> String {
    let id = j["id"].as_str().unwrap_or("");
    format!("Draft saved (draft id {id}, message id {}, thread {}). The person can review it in Gmail; send it with gmail_send {{draft: \"{id}\"}}.", j["message"]["id"].as_str().unwrap_or(""), j["message"]["threadId"].as_str().unwrap_or(""))
}
pub fn sent(j: &Value) -> String { format!("Sent (message id {}, thread {}).", j["id"].as_str().unwrap_or(""), j["threadId"].as_str().unwrap_or("")) }

#[cfg(test)]
mod tests {
    use super::*;
    fn b64u(s: &str) -> String { base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s) }
    #[test] fn a_list_shows_id_date_sender_subject_and_snippet() {
        let m = json!({"id":"m1","labelIds":["UNREAD"],"snippet":"Hi &amp; bye","payload":{"headers":[{"name":"From","value":"A <a@x>"},{"name":"Subject","value":"S"},{"name":"Date","value":"D"}]}});
        assert_eq!(msg_list(&[m]), "1 messages:\n- id m1 [unread]  D\n  From: A <a@x>\n  Subject: S\n  Hi & bye"); assert_eq!(msg_list(&[]), "No messages.");
    }
    #[test] fn a_message_reads_as_headers_then_its_plain_text_else_its_html() {
        let m = json!({"id":"m1","threadId":"t1","labelIds":["INBOX"],"payload":{"mimeType":"multipart/mixed","headers":[{"name":"From","value":"A"},{"name":"Subject","value":"S"}],"parts":[
            {"mimeType":"text/html","body":{"data":b64u("<p>Hello<br>there</p><style>x{}</style>&amp;")}},{"mimeType":"application/pdf","filename":"a.pdf","body":{"size":12}}]}});
        assert_eq!(msg_full(&m), "id: m1\nthread: t1\nFrom: A\nSubject: S\nLabels: INBOX\nAttachments: a.pdf (application/pdf, 12 bytes)\n\nHello\nthere\n&");
        let p = json!({"payload":{"mimeType":"text/plain","headers":[{"name":"Content-Type","value":"text/plain; charset=ISO-8859-1"}],"body":{"data":B64.encode([0x63, 0x61, 0x66, 0xe9])}}});
        assert_eq!(body(&parts(&p["payload"]), ""), "café"); assert!(cap(&"x".repeat(30), 20).ends_with("[... truncated]"));
    }
    #[test] fn entities_decode_and_unknown_ones_stay() { assert_eq!(entities("a&nbsp;b &lt;c&gt; &#39;d&#x27; &bogus; & e"), "a b <c> 'd' &bogus; & e"); }
    #[test] fn headers_cannot_be_injected_and_subjects_are_encoded_words() {
        assert_eq!(clean("a\r\nBcc: x@y"), "a  Bcc: x@y"); assert_eq!(subject("Hi"), "Hi");
        let s = subject("Grüße aus München, wie geht's"); assert!(s.starts_with("=?UTF-8?B?") && s.contains("\r\n ") && !s.contains('ü'));
    }
    #[test] fn a_reply_answers_in_its_conversation() {
        let o = Orig { id: "m1".into(), thread: "t1".into(), msgid: "<a@x>".into(), refs: "<0@x>".into(), subject: "Re: Plan".into(), from: "A <a@x>".into(), replyto: "".into() };
        let m = msg_of(&json!({"body":"ok"}), "me@x", Some(&o), "-- K");
        assert_eq!((m.to.as_str(), m.subject.as_str(), m.refs.as_str(), m.irt.as_str(), m.thread.as_str(), m.body.as_str()), ("A <a@x>", "Re: Plan", "<0@x> <a@x>", "<a@x>", "t1", "ok\n\n-- K"));
        assert_eq!(reply_subject("", "Plan"), "Re: Plan"); assert_eq!(reply_subject("New", "Plan"), "New"); assert_eq!(signed(true, "<b>x</b>", "A & B\nC"), "<b>x</b><br><br>A &amp; B<br>C");
    }
    #[test] fn a_message_with_html_and_an_attachment_is_well_formed() {
        let m = Msg { from: "me@x".into(), to: "a@x".into(), subject: "S".into(), html: true, body: "<p>Hi</p>".into(), ..Default::default() };
        let raw = String::from_utf8(compose(&m, &[("/tmp/ä b.pdf".into(), b"PDF".to_vec())], "N1")).unwrap();
        assert!(raw.starts_with("From: me@x\r\nTo: a@x\r\nSubject: S\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"=_bp_N1\"\r\n\r\n--=_bp_N1\r\nContent-Type: multipart/alternative; boundary=\"=_bpa_N1\""), "{raw}");
        assert!(raw.contains("Content-Type: application/pdf; name=\"_ b.pdf\"\r\nContent-Disposition: attachment; filename=\"_ b.pdf\"; filename*=UTF-8''%C3%A4%20b.pdf"), "{raw}");
        assert!(raw.contains(&B64.encode("PDF")) && raw.ends_with("--=_bp_N1--\r\n"));
        let (ct, up) = upload(&json_send("t1"), raw.as_bytes(), "N1"); assert_eq!(ct, "Content-Type: multipart/related; boundary=\"=_bpu_N1\"");
        assert!(String::from_utf8(up).unwrap().starts_with("--=_bpu_N1\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{\"threadId\":\"t1\"}\r\n--=_bpu_N1\r\nContent-Type: message/rfc822\r\n\r\nFrom:"));
    }
    #[test] fn attachments_are_checked_before_anything_is_sent() {
        assert_eq!(read_atts(&["rel.txt".into()]).unwrap_err(), "rel.txt is not an absolute path"); assert!(read_atts(&["/nonexistent/x".into()]).unwrap_err().contains("not an existing file"));
        assert_eq!(read_atts(&vec!["/x".to_string(); 21]).unwrap_err(), "at most 20 attachments"); assert_eq!(paths(Some(&json!("/a, /b ,"))), vec!["/a", "/b"]);
        assert_eq!((size(512), size(186000), size(2621440), size(14680064)), ("512 B".into(), "182 KB".into(), "2.5 MB".into(), "14 MB".into()));
    }
    #[test] fn the_approval_shows_every_header_and_the_whole_text() {
        let m = Msg { from: "me@x".into(), to: "a@x".into(), cc: "c@x".into(), subject: "S".into(), body: "Body".into(), inreply: "\"P\" from A".into(), ..Default::default() };
        assert_eq!(review("", "me@x", &m, "a.pdf (1 KB)"), "Account: me@x\nFrom: me@x\nTo: a@x\nCc: c@x\nSubject: S\nIn reply to: \"P\" from A\nAttachments: a.pdf (1 KB)\n\nBody");
    }
}
