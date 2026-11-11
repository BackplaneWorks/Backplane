//! A small TOML reader, enough for a Codex `config.toml` and the `[mcp_servers.*]` snippets vendor docs give: tables, array tables, dotted and
//! quoted keys, strings (basic, literal, multi-line), numbers, booleans, arrays and inline tables. Dates are kept as their text. Into JSON.
use serde_json::{Map, Value};

pub fn parse(src: &str) -> Result<Value, String> {
    let mut p = P { s: src.as_bytes(), i: 0, line: 1 };
    let mut root = Map::new();
    let mut cur: Vec<String> = vec![];
    loop {
        p.ws_nl();
        if p.eof() { break }
        if p.peek() == b'[' {
            let arr = p.s.get(p.i + 1) == Some(&b'[');
            p.i += if arr { 2 } else { 1 };
            p.ws(); let keys = p.keys()?; p.ws();
            if !p.eat(b']') || (arr && !p.eat(b']')) { return Err(p.err("a table header ends with ]")) }
            p.eol()?;
            if arr {
                let parent = table_at(&mut root, &keys[..keys.len() - 1], p.line)?;
                let a = parent.entry(keys.last().unwrap().clone()).or_insert_with(|| Value::Array(vec![]));
                let Value::Array(a) = a else { return Err(format!("line {}: {} is not an array of tables", p.line, keys.join("."))) };
                a.push(Value::Object(Map::new()));
                cur = keys; cur.push(format!("\u{0}{}", a.len() - 1));
            } else { table_at(&mut root, &keys, p.line)?; cur = keys; }
            continue
        }
        let keys = p.keys()?; p.ws();
        if !p.eat(b'=') { return Err(p.err("a key is followed by =")) }
        p.ws(); let v = p.value()?; p.eol()?;
        let t = table_at(&mut root, &cur, p.line)?;
        let t = table_at(t, &keys[..keys.len() - 1], p.line)?;
        let k = keys.last().unwrap().clone();
        if t.contains_key(&k) { return Err(format!("line {}: {k} is set twice", p.line)) }
        t.insert(k, v);
    }
    Ok(Value::Object(root))
}

/// The table at `keys` under `t`, made when missing; a `\0n` key is the n-th of an array of tables.
fn table_at<'a>(t: &'a mut Map<String, Value>, keys: &[String], line: usize) -> Result<&'a mut Map<String, Value>, String> {
    let mut t = t;
    let mut i = 0;
    while i < keys.len() {
        let k = &keys[i];
        let next = t.entry(k.clone()).or_insert_with(|| Value::Object(Map::new()));
        let next = match next {
            Value::Array(a) => {
                let n = keys.get(i + 1).and_then(|x| x.strip_prefix('\u{0}')).and_then(|n| n.parse::<usize>().ok());
                let idx = match n { Some(n) => { i += 1; n } None => a.len().saturating_sub(1) };
                a.get_mut(idx).ok_or_else(|| format!("line {line}: {k} has no table"))?
            }
            v => v,
        };
        t = match next { Value::Object(o) => o, _ => return Err(format!("line {line}: {k} is a value, not a table")) };
        i += 1;
    }
    Ok(t)
}

struct P<'a> { s: &'a [u8], i: usize, line: usize }
impl<'a> P<'a> {
    fn eof(&self) -> bool { self.i >= self.s.len() }
    fn peek(&self) -> u8 { *self.s.get(self.i).unwrap_or(&0) }
    fn eat(&mut self, c: u8) -> bool { if self.peek() == c { self.i += 1; true } else { false } }
    fn err(&self, m: &str) -> String { format!("line {}: {m}", self.line) }
    fn ws(&mut self) { while matches!(self.peek(), b' ' | b'\t') { self.i += 1 } }
    fn comment(&mut self) { if self.peek() == b'#' { while !self.eof() && self.peek() != b'\n' { self.i += 1 } } }
    /// spaces, comments and line ends
    fn ws_nl(&mut self) { loop { self.ws(); self.comment(); match self.peek() { b'\n' => { self.i += 1; self.line += 1 } b'\r' => self.i += 1, _ => break } } }
    fn eol(&mut self) -> Result<(), String> { self.ws(); self.comment(); self.eat(b'\r'); if self.eof() || self.eat(b'\n') { self.line += 1; Ok(()) } else { Err(self.err("one key = value on a line")) } }
    fn keys(&mut self) -> Result<Vec<String>, String> {
        let mut v = vec![self.key()?];
        loop { self.ws(); if self.eat(b'.') { self.ws(); v.push(self.key()?) } else { break } }
        Ok(v)
    }
    fn key(&mut self) -> Result<String, String> {
        match self.peek() {
            b'"' => self.basic(), b'\'' => self.literal(),
            _ => { let st = self.i; while matches!(self.peek(), b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-') { self.i += 1 } if st == self.i { return Err(self.err("a key")) } Ok(String::from_utf8_lossy(&self.s[st..self.i]).into_owned()) }
        }
    }
    fn starts(&self, t: &str) -> bool { self.s[self.i..].starts_with(t.as_bytes()) }
    fn value(&mut self) -> Result<Value, String> {
        if self.starts("\"\"\"") { return self.multi(true).map(Value::String) }
        if self.starts("'''") { return self.multi(false).map(Value::String) }
        match self.peek() {
            b'"' => self.basic().map(Value::String),
            b'\'' => self.literal().map(Value::String),
            b'[' => { self.i += 1; let mut a = vec![]; loop { self.ws_nl(); if self.eat(b']') { break } a.push(self.value()?); self.ws_nl(); if self.eat(b',') { continue } self.ws_nl(); if self.eat(b']') { break } return Err(self.err("an array's values are split by commas")) } Ok(Value::Array(a)) }
            b'{' => { self.i += 1; let mut m = Map::new(); self.ws(); if self.eat(b'}') { return Ok(Value::Object(m)) }
                loop { self.ws(); let ks = self.keys()?; self.ws(); if !self.eat(b'=') { return Err(self.err("an inline table's key is followed by =")) } self.ws(); let v = self.value()?;
                    let t = table_at(&mut m, &ks[..ks.len() - 1], self.line)?; t.insert(ks.last().unwrap().clone(), v); self.ws();
                    if self.eat(b',') { continue } if self.eat(b'}') { break } return Err(self.err("an inline table ends with }")) }
                Ok(Value::Object(m)) }
            _ => {
                let st = self.i; while !self.eof() && !matches!(self.peek(), b',' | b']' | b'}' | b'\n' | b'\r' | b'#' | b' ' | b'\t') { self.i += 1 }
                let raw = String::from_utf8_lossy(&self.s[st..self.i]).trim().to_string();
                match raw.as_str() { "true" => return Ok(Value::Bool(true)), "false" => return Ok(Value::Bool(false)), "" => return Err(self.err("a value")), _ => {} }
                let n = raw.replace('_', "");
                if let Ok(i) = n.parse::<i64>() { return Ok(Value::from(i)) }
                if let Some(h) = n.strip_prefix("0x") { if let Ok(i) = i64::from_str_radix(h, 16) { return Ok(Value::from(i)) } }
                if let Ok(f) = n.parse::<f64>() { if f.is_finite() { if let Some(x) = serde_json::Number::from_f64(f) { return Ok(Value::Number(x)) } } }
                if raw.chars().next().map_or(false, |c| c.is_ascii_digit()) { return Ok(Value::String(raw)) } // a date or a time
                Err(self.err(&format!("{raw:?} is not a TOML value (strings are quoted)")))
            }
        }
    }
    fn literal(&mut self) -> Result<String, String> {
        self.i += 1; let st = self.i;
        while !self.eof() && self.peek() != b'\'' { if self.peek() == b'\n' { return Err(self.err("a string ends on its line")) } self.i += 1 }
        if !self.eat(b'\'') { return Err(self.err("a string ends with '")) }
        Ok(String::from_utf8_lossy(&self.s[st..self.i - 1]).into_owned())
    }
    fn basic(&mut self) -> Result<String, String> {
        self.i += 1; let mut out: Vec<u8> = vec![];
        loop {
            if self.eof() || self.peek() == b'\n' { return Err(self.err("a string ends with \"")) }
            let c = self.peek(); self.i += 1;
            match c { b'"' => break, b'\\' => self.escape(&mut out)?, _ => out.push(c) }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let e = self.peek(); self.i += 1;
        let ch = match e { b'n' => '\n', b't' => '\t', b'r' => '\r', b'"' => '"', b'\\' => '\\', b'b' => '\u{8}', b'f' => '\u{c}', b'e' => '\u{1b}',
            b'u' | b'U' => { let n = if e == b'u' { 4 } else { 8 }; let h = String::from_utf8_lossy(self.s.get(self.i..self.i + n).ok_or_else(|| self.err("a short \\u"))?).into_owned(); self.i += n;
                char::from_u32(u32::from_str_radix(&h, 16).map_err(|_| self.err("a bad \\u"))?).ok_or_else(|| self.err("a bad \\u"))? }
            _ => return Err(self.err("an unknown escape")) };
        let mut b = [0u8; 4]; out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes()); Ok(())
    }
    fn multi(&mut self, basic: bool) -> Result<String, String> {
        let q: &[u8] = if basic { b"\"\"\"" } else { b"'''" };
        self.i += 3; if self.eat(b'\n') { self.line += 1 } else if self.starts("\r\n") { self.i += 2; self.line += 1 }
        let mut out: Vec<u8> = vec![];
        loop {
            if self.eof() { return Err(self.err("a multi-line string is not closed")) }
            if self.s[self.i..].starts_with(q) { self.i += 3; while self.peek() == q[0] { out.push(q[0]); self.i += 1 } break }
            let c = self.peek(); self.i += 1; if c == b'\n' { self.line += 1 }
            if basic && c == b'\\' {
                if matches!(self.peek(), b'\n' | b' ' | b'\t' | b'\r') { while matches!(self.peek(), b'\n' | b' ' | b'\t' | b'\r') { if self.peek() == b'\n' { self.line += 1 } self.i += 1 } continue }
                self.escape(&mut out)?; continue
            }
            out.push(c);
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn a_codex_config_reads_as_codex_reads_it() {
        let t = r#"
# my config
model = "gpt-6"
approval_policy = 'never'   # trailing
[mcp_servers.fetch]
command = "uvx"
args = ["mcp-server-fetch", "--ignore-robots-txt",]
startup_timeout_sec = 20

[mcp_servers.github]
url = "https://api.githubcopilot.com/mcp/"
bearer_token_env_var = "GITHUB_PAT"
http_headers = { "X-Mode" = "ro", n = 1.5 }

[mcp_servers.fetch.env]
LOG = "debug"
"WITH SPACE" = "a\tbé"

[mcp_servers."st.docs"]
command = 'npx'
args = [
  "-y",   # comment in an array
  "@x/y",
]
enabled = false
[[profiles.list]]
a = 1
[[profiles.list]]
a = 2
when = 1979-05-27T07:32:00Z
"#;
        let v = parse(t).unwrap();
        assert_eq!(v["model"], "gpt-6"); assert_eq!(v["approval_policy"], "never");
        assert_eq!(v["mcp_servers"]["fetch"]["args"], json!(["mcp-server-fetch", "--ignore-robots-txt"])); assert_eq!(v["mcp_servers"]["fetch"]["startup_timeout_sec"], 20);
        assert_eq!(v["mcp_servers"]["fetch"]["env"], json!({"LOG":"debug","WITH SPACE":"a\tbé"}));
        assert_eq!(v["mcp_servers"]["github"]["http_headers"]["X-Mode"], "ro"); assert_eq!(v["mcp_servers"]["github"]["http_headers"]["n"], 1.5);
        assert_eq!(v["mcp_servers"]["st.docs"]["args"], json!(["-y", "@x/y"])); assert_eq!(v["mcp_servers"]["st.docs"]["enabled"], false);
        assert_eq!(v["profiles"]["list"][1]["a"], 2); assert_eq!(v["profiles"]["list"][1]["when"], "1979-05-27T07:32:00Z");
    }
    #[test] fn dotted_keys_multiline_strings_and_errors() {
        let v = parse("mcp_servers.a.command = \"x\"\nmcp_servers.a.env.K = \"v\"\ns = \"\"\"\nline1\nline2\"\"\"\nl = '''raw\\n'''\n").unwrap();
        assert_eq!(v["mcp_servers"]["a"]["env"]["K"], "v"); assert_eq!(v["s"], "line1\nline2"); assert_eq!(v["l"], "raw\\n");
        assert!(parse("a = \n").is_err()); assert!(parse("a = bare\n").unwrap_err().contains("quoted")); assert!(parse("a = 1\na = 2\n").unwrap_err().contains("twice"));
        assert!(parse("[a\n").is_err()); assert!(parse("a = \"open\n").is_err()); assert!(parse("a = 1 b = 2\n").is_err());
        assert_eq!(parse("").unwrap(), json!({}));
    }
}
