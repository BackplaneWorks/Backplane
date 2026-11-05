//! test/vt_test.bend, line for line (same names, same expected strings).
use super::*;

const E: &str = "\x1b";
fn feed(mut v: Vt, s: &str) -> Vt { v.feed(s.as_bytes()); v }
fn run(c: usize, r: usize, s: &str) -> Vt { feed(Vt::new(c, r), s) }
fn text(l: &VLine) -> String { l.runs.iter().map(|r| r.text.as_str()).collect() }
fn screen(v: &Vt) -> String { v.lines(0).iter().map(text).collect::<Vec<_>>().join("|") }
fn scrolled(v: &Vt, n: usize) -> String { v.lines(n).iter().map(text).collect::<Vec<_>>().join("|") }
fn row0(v: &Vt) -> String { v.lines(0)[0].runs.iter().map(|r| format!("[{} {} {} {}{}]", r.text, r.fg, r.bg, r.bold as u8, r.underline as u8)).collect() }
fn cur(v: &Vt) -> String { let c = v.cursor(); format!("{},{}{}", c.x, c.y, c.visible as u8) }
fn bytes(b: &[u8]) -> String { b.iter().map(|&x| if x == 27 { "^[".to_string() } else if x < 32 || x >= 127 { format!("<{x}>") } else { (x as char).to_string() }).collect() }
fn eq(name: &str, got: String, want: &str) { assert_eq!(got, want, "{name}") }

#[test] fn vt_text_and_wrap() {
    let d40 = "0123456789012345678901234567890123456789"; let d100 = format!("{d40}{d40}01234567890123456789");
    eq("vt text", screen(&run(10, 3, "hi")), "hi||");
    let wrap = run(10, 3, "hello world!");
    eq("vt wrap", screen(&wrap), "hello worl|d!|"); eq("vt wrap cursor", cur(&wrap), "2,11");
    eq("vt pending wrap", cur(&run(5, 2, "abcde")), "4,01");
    eq("vt pending wrap cr", screen(&run(5, 2, "abcde\r\nf")), "abcde|f");
    eq("vt long wrap", screen(&run(40, 3, &d100)), &format!("{d40}|{d40}|01234567890123456789"));
    eq("vt long wrap cursor", cur(&run(40, 3, &d100)), "20,21");
    eq("vt long scroll", screen(&run(40, 2, &d100)), &format!("{d40}|01234567890123456789"));
    eq("vt long after wrap", screen(&feed(run(40, 2, d40), d40)), &format!("{d40}|{d40}"));
    eq("vt long nowrap", screen(&run(40, 2, &format!("{E}[?7l{d40}abcdefghij"))), "012345678901234567890123456789012345678j|");
    eq("vt long insert", screen(&run(40, 1, &format!("abc\r{E}[4h{d40}"))), d40);
    eq("vt long insert keeps", screen(&run(40, 1, &format!("abc\r{E}[4h0123456789012345678901234567890123"))), "0123456789012345678901234567890123abc");
    eq("vt long mid row", screen(&run(40, 2, &format!("ab{E}[31m{d40}"))), "ab01234567890123456789012345678901234567|89");
    eq("vt crlf", screen(&run(10, 3, "ab\r\ncd")), "ab|cd|"); eq("vt lf keeps column", screen(&run(10, 3, "ab\ncd")), "ab|  cd|");
    eq("vt backspace", screen(&run(10, 1, "abc\u{8}\u{8}X")), "aXc"); eq("vt tab", screen(&run(20, 1, "a\tb")), "a       b");
}
#[test] fn vt_cursor_and_erase() {
    eq("vt cup", screen(&run(5, 3, &format!("{E}[2;3HX"))), "|  X|");
    eq("vt cursor moves", screen(&run(6, 3, &format!("{E}[3;3H{E}[AU{E}[2DL{E}[BD{E}[CR"))), "| LU|  D R");
    eq("vt cha vpa", screen(&run(6, 3, &format!("{E}[4GA{E}[3dB"))), "   A||    B");
    eq("vt cursor clamps", cur(&run(5, 3, &format!("{E}[99;99H"))), "4,21");
    eq("vt el 0", screen(&run(10, 1, &format!("abcdef{E}[1;3H{E}[K"))), "ab");
    eq("vt el 1", screen(&run(10, 1, &format!("abcdef{E}[1;3H{E}[1K"))), "   def");
    eq("vt el 2", screen(&run(10, 1, &format!("abcdef{E}[2K"))), "");
    eq("vt ed 0", screen(&run(5, 3, &format!("aaa\r\nbbb\r\nccc{E}[2;2H{E}[J"))), "aaa|b|");
    eq("vt ed 1", screen(&run(5, 3, &format!("aaa\r\nbbb\r\nccc{E}[2;2H{E}[1J"))), "|  b|ccc");
    eq("vt ed 2", screen(&run(5, 3, &format!("aaa\r\nbbb{E}[2J"))), "||");
    eq("vt ich", screen(&run(10, 1, &format!("abcdef\r{E}[2@"))), "  abcdef");
    eq("vt dch", screen(&run(10, 1, &format!("abcdef\r{E}[2P"))), "cdef");
    eq("vt ech", screen(&run(10, 1, &format!("abcdef\r{E}[2X"))), "  cdef");
    eq("vt il", screen(&run(5, 3, &format!("a\r\nb\r\nc{E}[2;1H{E}[L"))), "a||b");
    eq("vt dl", screen(&run(5, 3, &format!("a\r\nb\r\nc{E}[1;1H{E}[M"))), "b|c|");
    eq("vt scroll", screen(&run(5, 3, "1\r\n2\r\n3\r\n4")), "2|3|4");
    eq("vt ri", screen(&run(5, 3, &format!("1\r\n2{E}[H{E}M0"))), "0|1|2");
}
#[test] fn vt_colors() {
    eq("vt sgr red", row0(&run(10, 1, &format!("{E}[31mR{E}[0mn"))), "[R 14707829 855826 00][n 14277092 855826 00]");
    eq("vt sgr bold underline", row0(&run(10, 1, &format!("{E}[1;4mB"))), "[B 14277092 855826 11]");
    eq("vt sgr bright bg", row0(&run(10, 1, &format!("{E}[92;104mG"))), "[G 11921552 8570111 00]");
    eq("vt sgr 256", row0(&run(10, 1, &format!("{E}[38;5;196;48;5;244mX"))), "[X 16711680 8421504 00]");
    eq("vt sgr truecolor", row0(&run(10, 1, &format!("{E}[38;2;1;2;3;48;2;255;128;0mT"))), "[T 66051 16744448 00]");
    eq("vt sgr inverse", row0(&run(10, 1, &format!("{E}[7mI{E}[27mJ"))), "[I 855826 14277092 00][J 14277092 855826 00]");
    eq("vt sgr default", row0(&run(10, 1, &format!("{E}[31;42mA{E}[39mB{E}[49mC"))), "[A 14707829 10011513 00][B 14277092 10011513 00][C 14277092 855826 00]");
    eq("vt erase uses bg", row0(&run(4, 1, &format!("{E}[41m{E}[2K"))), "[     14277092 14707829 00]");
}
#[test] fn vt_regions_and_screens() {
    let rs = run(10, 5, &format!("a\r\nb\r\nc\r\nd\r\ne{E}[2;4r{E}[4;1H\n\nX")); eq("vt scroll region", screen(&rs), "a|d||X|e");
    eq("vt su", screen(&run(5, 3, &format!("1\r\n2\r\n3{E}[S"))), "2|3|"); eq("vt sd", screen(&run(5, 3, &format!("1\r\n2\r\n3{E}[T"))), "|1|2");
    let top = run(5, 4, &format!("{E}[1;3ra\r\nb\r\nc\r\nd")); eq("vt region at top", screen(&top), "b|c|d|"); eq("vt region at top history", top.history().to_string(), "1");
    eq("vt origin mode", screen(&run(5, 4, &format!("{E}[2;3r{E}[?6h{E}[1;1HX"))), "|X||");
    eq("vt cnl", screen(&run(5, 2, &format!("ab{E}[Ec"))), "ab|c");
    eq("vt alt 47", screen(&run(5, 1, &format!("m{E}[?47hx{E}[?47l"))), "m");
    eq("vt da2", bytes(run(10, 1, &format!("{E}[>c")).replies()), "^[[>1;10;0c");
    let alt1 = run(10, 3, &format!("main{E}[?1049h{E}[2;2Halt")); let alt2 = feed(run(10, 3, &format!("main{E}[?1049h{E}[2;2Halt")), &format!("{E}[?1049l"));
    eq("vt alt screen", screen(&alt1), "| alt|"); eq("vt alt restore", screen(&alt2), "main||"); eq("vt alt cursor", cur(&alt2), "4,01"); assert!(alt1.alt());
}
#[test] fn vt_text_encodings_and_replies() {
    let mut split = Vt::new(10, 2); split.feed(&[97, 226, 156]); split.feed(&[147, 98]); eq("vt utf8 split", screen(&split), "a✓b|");
    let mut bad = Vt::new(5, 1); bad.feed(&[97, 255, 98]); eq("vt utf8 bad", screen(&bad), "a\u{FFFD}b");
    eq("vt wide", cur(&run(10, 1, "中a")), "3,01"); eq("vt wide text", screen(&run(10, 1, "中a")), "中a");
    eq("vt dec graphics", screen(&run(10, 1, &format!("{E}(0lqk{E}(Bq"))), "┌─┐q");
    let mut dsr = run(10, 5, &format!("{E}[3;4H{E}[6n")); eq("vt dsr", bytes(dsr.replies()), "^[[3;4R"); dsr.drain(); eq("vt drain", bytes(dsr.replies()), "");
    eq("vt da", bytes(run(10, 1, &format!("{E}[c")).replies()), "^[[?1;2c");
    eq("vt title bel", run(10, 1, &format!("{E}]0;hello\u{7}x")).title().to_string(), "hello");
    eq("vt title st", run(10, 1, &format!("{E}]2;wörld{E}\\x")).title().to_string(), "wörld");
    eq("vt title text", screen(&run(10, 1, &format!("{E}]2;t{E}\\x"))), "x");
    eq("vt osc split", feed(run(10, 1, &format!("{E}]0;ab")), "c\u{7}").title().to_string(), "abc");
    eq("vt unknown csi", screen(&run(10, 1, &format!("a{E}[?1000;1006h{E}[>4;1m{E}[5 qb"))), "ab");
    eq("vt dcs skipped", screen(&run(10, 1, &format!("a{E}P+q544e{E}\\b"))), "ab");
    eq("vt cursor hide", cur(&run(10, 1, &format!("{E}[?25l"))), "0,00");
    eq("vt save restore", cur(&run(10, 3, &format!("{E}[2;5H{E}7{E}[H{E}8"))), "4,11");
    eq("vt reset", screen(&run(10, 2, &format!("abc{E}c"))), "|");
}
#[test] fn vt_scrollback_and_resize() {
    let sb = run(6, 3, "1\r\n2\r\n3\r\n4\r\n5");
    eq("vt scrollback", scrolled(&sb, 1), "2|3|4"); eq("vt scrollback all", scrolled(&sb, 9), "1|2|3"); eq("vt history", sb.history().to_string(), "2");
    let mut v = run(10, 2, "abcdefgh"); v.resize(4, 2); eq("vt resize narrow", screen(&v), "abcd|");
    let mut v = run(4, 3, "a\r\nb\r\nc"); v.resize(4, 2); eq("vt resize short", screen(&v), "b|c"); eq("vt resize short history", scrolled(&v, 1), "a|b");
    let mut v = run(4, 2, "ab"); v.resize(6, 3); eq("vt resize grow", screen(&v), "ab||");
}
#[test] fn vt_keys_and_paste() {
    let plain = Vt::new(10, 2); let app = run(10, 2, &format!("{E}[?1h{E}[?2004h"));
    let k = |v: &Vt, ks, t, m| bytes(&v.key(ks, t, m));
    eq("vt key kp enter", k(&plain, 65421, 13, 0), "<13>"); eq("vt key return", k(&plain, 65293, 13, 0), "<13>"); eq("vt key backspace", k(&plain, 65288, 8, 0), "<127>");
    eq("vt key up", k(&plain, 65362, 0, 0), "^[[A"); eq("vt key up app", k(&app, 65362, 0, 0), "^[OA"); eq("vt key shift up", k(&plain, 65362, 0, 1), "^[[1;2A");
    eq("vt key ctrl right", k(&plain, 65363, 0, 4), "^[[1;5C"); eq("vt key ctrl c", k(&plain, 99, 99, 4), "<3>"); eq("vt key ctrl c text", k(&plain, 99, 3, 4), "<3>");
    eq("vt key alt x", k(&plain, 120, 120, 8), "^[x"); eq("vt key text", k(&plain, 233, 233, 0), "<195><169>");
    eq("vt key f1", k(&plain, 65470, 0, 0), "^[OP"); eq("vt key f5", k(&plain, 65474, 0, 0), "^[[15~"); eq("vt key delete", k(&plain, 65535, 0, 0), "^[[3~");
    eq("vt key pgup ctrl", k(&plain, 65365, 0, 4), "^[[5;5~"); eq("vt key backtab", k(&plain, 65056, 0, 1), "^[[Z"); eq("vt key tab", k(&plain, 65289, 9, 0), "<9>");
    eq("vt key escape", k(&plain, 65307, 27, 0), "^["); eq("vt key shift", k(&plain, 65505, 0, 1), "");
    eq("vt paste", bytes(&plain.paste("a\nb\r\nc")), "a<13>b<13>c"); eq("vt paste bracketed", bytes(&app.paste("x\x1by")), "^[[200~xy^[[201~");
}
#[test] fn bounded() {
    let mut v = Vt::new(100000, 100000); assert!(v.cols() <= MAX_COLS && v.rows() <= MAX_ROWS);
    for _ in 0..5000 { v.feed(b"line\r\n") } assert!(v.history() <= SCROLLBACK);
    v.feed(format!("{E}]0;{}\x07", "x".repeat(100000)).as_bytes()); assert!(v.title().len() <= OSC_MAX);
    for _ in 0..5000 { v.feed(format!("{E}[6n").as_bytes()) } assert!(v.replies().len() <= 4096);
}
