//! Terminals in the chat (src/core/emb.bend, pure): keys, sizes, the entry's summary, the screen as text and as bytes, the command script,
//! the done mark, what the agent reads, and live or inactive. Laws emb_* map to the tests below.
use super::vt::{Vt, DEFAULT_BG, DEFAULT_FG};

pub const TONE: &str = "term";
pub const KIND: &str = "terminal";
/// 30 minutes.
pub const IDLE: u64 = 1800;

/// Emb.key: the name a chat terminal's bytes travel under.
pub fn key(thread: &str, id: &str) -> String { format!("x:{thread}:{id}") }
pub fn is_key(k: &str) -> bool { k.starts_with("x:") }
/// Et.id: the id the agent knows a terminal by (the key's last part).
pub fn id_of(key: &str) -> &str { key.rsplit(':').next().unwrap_or("") }
pub fn pin_key(key: &str) -> String { format!("emb.pin:{key}") }
pub fn at_key(key: &str) -> String { format!("emb.at:{key}") }

/// A key as a file name under <home>/terms: no separators, no dots at the start, bounded.
pub fn key_ok(k: &str) -> bool { !k.is_empty() && k.len() <= 256 && !k.starts_with('.') && !k.contains("..") && k.bytes().all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b)) }

/// Emb.num: digits only, capped (anything else skipped).
pub fn num(s: &str) -> u32 { s.bytes().filter(u8::is_ascii_digit).fold(0u32, |a, b| (a * 10 + (b - b'0') as u32).min(100000)) }
fn clamp(n: u32, lo: u32, hi: u32, d: u32) -> u32 { if n == 0 { d } else { n.clamp(lo, hi) } }
/// 0 (not given) is the default: 80 columns, 24 rows.
pub fn cols(n: u32) -> u32 { clamp(n, 20, 200, 80) }
pub fn rows(n: u32) -> u32 { clamp(n, 3, 60, 24) }

/// The entry's summary: "title\nCOLSxROWS\ncommand".
pub fn summary(title: &str, cols: u32, rows: u32, cmd: &str) -> String { format!("{title}\n{cols}x{rows}\n{cmd}") }
pub struct Summary { pub title: String, pub cols: u32, pub rows: u32, pub cmd: String }
pub fn parse_summary(s: &str) -> Summary {
    let mut it = s.split('\n'); let title = it.next().unwrap_or("").to_string(); let size = it.next().unwrap_or("");
    let mut xy = size.split('x'); let c = cols(num(xy.next().unwrap_or(""))); let r = rows(num(xy.next().unwrap_or("")));
    Summary { title, cols: c, rows: r, cmd: it.collect::<Vec<_>>().join("\n") }
}
/// Mcp.term.title: the title, else the command's first 60 characters.
pub fn title_of(title: &str, cmd: &str) -> String { let t = title.trim(); if t.is_empty() { cmd.trim().chars().take(60).collect() } else { t.to_string() } }

/// Emb.text: what the screen shows, blank rows at the bottom left out.
pub fn text(v: &Vt) -> String {
    let mut ls: Vec<String> = v.lines(0).iter().map(|l| l.runs.iter().map(|r| r.text.as_str()).collect()).collect();
    while ls.last().map_or(false, |l| l.is_empty()) { ls.pop(); }
    ls.join("\n")
}

fn rgb(c: u32) -> String { format!("{};{};{}", (c >> 16) & 255, (c >> 8) & 255, c & 255) }
/// Emb.snap: bytes that draw v's screen again on a blank terminal of its size (alternate screen, each row's runs, the modes keys
/// depend on, the cursor and the title; the scrollback is left behind).
pub fn snap(v: &Vt) -> Vec<u8> {
    let mut s = String::new();
    if v.alt() { s += "\x1b[?1049h" }
    s += "\x1b[H\x1b[2J";
    for (y, l) in v.lines(0).iter().enumerate() {
        s += &format!("\x1b[{};1H", y + 1);
        for r in &l.runs {
            s += "\x1b[0";
            if r.fg != DEFAULT_FG { s += &format!(";38;2;{}", rgb(r.fg)) }
            if r.bg != DEFAULT_BG { s += &format!(";48;2;{}", rgb(r.bg)) }
            if r.bold { s += ";1" }
            if r.underline { s += ";4" }
            s += "m"; s += &r.text;
        }
    }
    s += "\x1b[0m";
    if v.appk() { s += "\x1b[?1h" }
    if v.bpaste() { s += "\x1b[?2004h" }
    if !v.title().is_empty() { s += &format!("\x1b]2;{}\x07", v.title()) }
    let c = v.cursor();
    if !c.visible { s += "\x1b[?25l" }
    s += &format!("\x1b[{};{}H", c.y + 1, c.x + 1);
    s.into_bytes()
}

/// Emb.script: the command runs in the user's shell, then a mark the screen ignores (OSC 7799) and an interactive shell in its place.
pub fn script(cmd: &str, shell: &str) -> String { format!("{cmd}\nprintf '\\033]7799;done %s\\007' \"$?\"\nexec '{shell}' -l") }
/// Emb.done: the status a mark among `bs` carries ("" when there is none).
pub fn done(bs: &[u8]) -> String {
    const P: &[u8] = b"\x1b]7799;done ";
    match bs.windows(P.len()).position(|w| w == P) { Some(i) => bs[i + P.len()..].iter().take_while(|&&b| b != 7).map(|&b| b as char).collect(), None => String::new() }
}
/// Emb.after: a terminal's state once bytes came.
pub fn after(state: &str, bs: &[u8]) -> String { let d = done(bs); if d.is_empty() { state.to_string() } else { format!("done {d}") } }

/// Emb.report: one terminal as the agent reads it.
pub fn report(id: &str, title: &str, state: &str, text: &str) -> String { format!("<terminal id=\"{id}\" title=\"{title}\" state=\"{state}\">\n{text}\n</terminal>\n") }
/// Emb.note: the terminals that changed since the agent last read them ("" when none did).
pub fn note(rs: &[String]) -> String { if rs.is_empty() { String::new() } else { format!("\n\n<terminals>\nWhat the terminals you opened in this chat show now (the person may have used them since):\n{}</terminals>", rs.concat()) } }

/// Emb.live: pinned, or until the person sends a message after it was opened (or last restarted), or for 30 minutes.
pub fn live(pinned: bool, touched: u64, last_user: u64, now: u64) -> bool { pinned || (last_user <= touched && now < touched + IDLE) }
/// Et.stays: what a stopped program prints, and its end, are no news to an agent that has seen it.
pub fn stays(seen: bool, state: &str) -> bool { seen && state == "stopped" }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sizes_are_clamped_with_defaults() { assert_eq!((cols(0), rows(0)), (80, 24)); assert_eq!((cols(5), rows(1)), (20, 3)); assert_eq!((cols(9999), rows(9999)), (200, 60)); assert_eq!(num("x1a2"), 12); assert_eq!(num("99999999999"), 100000) }
    #[test] fn emb_summary_round_trips_and_keeps_command_newlines() {
        let s = summary("Pick", 100, 30, "echo a\necho b"); let p = parse_summary(&s);
        assert_eq!((p.title.as_str(), p.cols, p.rows, p.cmd.as_str()), ("Pick", 100, 30, "echo a\necho b"));
        let q = parse_summary("t\nbad\nc"); assert_eq!((q.cols, q.rows), (80, 24));
    }
    #[test] fn emb_keys_name_thread_and_id() { assert_eq!(key("t1", "x9"), "x:t1:x9"); assert!(is_key("x:t1:x9") && !is_key("t1")); assert_eq!(id_of("x:t1:x9"), "x9"); assert_eq!(pin_key("k"), "emb.pin:k") }
    #[test] fn file_names_cannot_leave_the_terms_folder() { assert!(key_ok("x:t1:x9")); for bad in ["", "../a", "a/b", ".hidden", "a..b", "a b"] { assert!(!key_ok(bad), "{bad}") } assert!(!key_ok(&"a".repeat(300))) }
    #[test] fn emb_done_mark_gives_the_status() {
        assert_eq!(done(b"out\x1b]7799;done 3\x07$ "), "3"); assert_eq!(done(b"nothing"), ""); assert_eq!(after("running", b"\x1b]7799;done 0\x07"), "done 0"); assert_eq!(after("running", b"x"), "running");
        assert!(script("ls", "/bin/sh").ends_with("exec '/bin/sh' -l")); assert!(script("ls", "/bin/sh").starts_with("ls\nprintf '\\033]7799;done %s\\007' \"$?\""));
    }
    #[test] fn emb_text_leaves_blank_rows_out_and_reports_wrap_it() {
        let mut v = Vt::new(20, 5); v.feed(b"hello\r\nworld"); assert_eq!(text(&v), "hello\nworld");
        assert_eq!(report("x1", "T", "running", "hi"), "<terminal id=\"x1\" title=\"T\" state=\"running\">\nhi\n</terminal>\n");
        assert_eq!(note(&[]), ""); assert!(note(&["a".into()]).starts_with("\n\n<terminals>\nWhat the terminals you opened"));
    }
    #[test] fn emb_snap_redraws_the_screen_on_a_blank_terminal() {
        let mut v = Vt::new(20, 3); v.feed(b"\x1b[31mred\x1b[0m ok\x1b]2;ti\x07"); let s = String::from_utf8(snap(&v)).unwrap();
        assert!(s.starts_with("\x1b[H\x1b[2J\x1b[1;1H\x1b[0;38;2;")); assert!(s.contains("red") && s.contains("\x1b]2;ti\x07")); assert!(s.ends_with("\x1b[1;7H"));
        // feeding the snapshot to a blank terminal reproduces the screen
        let mut w = Vt::new(20, 3); w.feed(&snap(&v)); assert_eq!(text(&w), text(&v)); assert_eq!(w.cursor(), v.cursor()); assert_eq!(w.title(), "ti");
        let mut a = Vt::new(20, 3); a.feed(b"\x1b[?1049h\x1b[?1h\x1b[?2004h\x1b[?25l"); let s = String::from_utf8(snap(&a)).unwrap(); assert!(s.starts_with("\x1b[?1049h") && s.contains("\x1b[?1h") && s.contains("\x1b[?2004h") && s.contains("\x1b[?25l"));
    }
    #[test] fn emb_live_rules() {
        assert!(live(true, 0, 99, 99999)); assert!(live(false, 100, 50, 200)); assert!(!live(false, 100, 150, 200), "a message after it was opened"); assert!(!live(false, 100, 50, 100 + IDLE), "30 minutes");
        assert!(stays(true, "stopped") && !stays(false, "stopped") && !stays(true, "running"));
    }
}
