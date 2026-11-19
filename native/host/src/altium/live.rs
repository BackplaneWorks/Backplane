//! Altium Designer, live: a DelphiScript run inside the person's running Altium (X2.EXE), as KiCad's IPC is for KiCad.
//!
//! Altium has no out-of-process API. A running Altium takes a command line, `X2.EXE -RScriptingSystem:RunScriptFile(FileName=..|ProcName=Run)`
//! (one argument, quoted when the path has a space; unquoted, a space starts a second Altium instead):
//! a second X2.EXE hands it to the running one and exits, and the script runs on Altium's UI thread with its whole API (workspace,
//! schematic and PCB servers, DRC, output jobs). The hub writes each run as a file of its own (a script project, or a file name used
//! before, leaves Altium compiling the copy its script editor holds, a broken one after a failure), starts it, and reads what it wrote.
//!
//! The script an agent sends is wrapped in a template (`build`): its `var`/`const` block becomes global, its procedures and functions
//! follow the helpers, its statements become `BpBody`, run inside try/except so an error is a result and not Altium's "Continue
//! execution?" box (which blocks every later run until answered; the hub answers that one, with Cancel, which stops the script). `Emit(S)` adds a
//! line to the output, written through to disk at once so a script that dies still shows how far it got. Every `while` and
//! `repeat` loop also calls `BpTick`: a script cannot be stopped from outside, so a loop that never ends would freeze Altium; past the
//! run's deadline every loop exits. Undo groups (`PCBServer.PreProcess`/`PostProcess`, `SchServer.ProcessControl.PreProcess`/
//! `PostProcess`) are tracked and closed at the end: one left open makes every later edit of that document impossible to undo.
//!
//! Design notes owe much to coffeenmusic/altium-mcp (MIT), whose sandbox found these traps first.
use serde_json::{json, Value};

/// A script split into the parts the template places.
#[derive(Debug, Default, PartialEq)]
pub struct Parts { pub decls: String, pub routines: String, pub body: String }

#[derive(Clone, Debug, PartialEq)]
enum Tok { Word(String), Sym(char), Other }
/// Words (lowercased) and symbols with their byte span and line; strings and comments are skipped.
fn lex(s: &str) -> Vec<(Tok, usize, usize, usize)> {
    let b = s.as_bytes();
    let (mut i, mut line, mut out) = (0usize, 1usize, Vec::new());
    while i < b.len() {
        let c = b[i];
        if c == b'\n' { line += 1; i += 1; continue }
        if c.is_ascii_whitespace() { i += 1; continue }
        if c == b'/' && b.get(i + 1) == Some(&b'/') { while i < b.len() && b[i] != b'\n' { i += 1 } continue }
        if c == b'{' { while i < b.len() && b[i] != b'}' { if b[i] == b'\n' { line += 1 } i += 1 } i += 1; continue }
        if c == b'(' && b.get(i + 1) == Some(&b'*') { i += 2; while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b')') { if b[i] == b'\n' { line += 1 } i += 1 } i += 2; continue }
        if c == b'\'' { let st = i; i += 1; while i < b.len() { if b[i] == b'\'' { if b.get(i + 1) == Some(&b'\'') { i += 2; continue } i += 1; break } if b[i] == b'\n' { line += 1 } i += 1 } out.push((Tok::Other, st, i, line)); continue }
        if c.is_ascii_alphabetic() || c == b'_' {
            let st = i; while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') { i += 1 }
            out.push((Tok::Word(s[st..i].to_ascii_lowercase()), st, i, line)); continue
        }
        if c.is_ascii_digit() { let st = i; while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') { i += 1 } out.push((Tok::Other, st, i, line)); continue }
        out.push((Tok::Sym(c as char), i, i + 1, line)); i += 1;
    }
    out
}
fn is_w(t: &Tok, w: &str) -> bool { matches!(t, Tok::Word(x) if x == w) }
/// Words that open a block closed by `end`.
fn opens(t: &Tok) -> bool { matches!(t, Tok::Word(x) if matches!(x.as_str(), "begin" | "try" | "case" | "record" | "asm")) }

/// Parts.of: split a script. Leading `var`/`const`/`type` sections are declarations; `procedure`/`function` definitions (to the `end;`
/// closing their body) are routines; the rest is the body. A body wrapped whole in `begin ... end` (or `end.`) loses the wrapper.
pub fn split(src: &str) -> Parts {
    let toks = lex(src);
    let (mut decls, mut routines, mut body) = (String::new(), String::new(), String::new());
    let mut k = 0usize;
    let mut body_from: Option<usize> = None;
    while k < toks.len() {
        let t = &toks[k].0;
        if is_w(t, "var") || is_w(t, "const") || is_w(t, "type") {
            // a section: declarations up to the next top-level procedure/function/begin or a statement (a word followed by := ( ; or .)
            let st = toks[k].1;
            let mut j = k + 1;
            while j < toks.len() {
                let tj = &toks[j].0;
                if is_w(tj, "procedure") || is_w(tj, "function") || is_w(tj, "begin") || is_w(tj, "var") || is_w(tj, "const") || is_w(tj, "type") { break }
                // a declaration line starts with a name list then ':' or '='; anything else at a line's start is a statement
                if let Tok::Word(_) = tj {
                    let prev_semi = j == k + 1 || matches!(toks[j - 1].0, Tok::Sym(';'));
                    if prev_semi {
                        let mut m = j; while m < toks.len() && (matches!(toks[m].0, Tok::Word(_)) || matches!(toks[m].0, Tok::Sym(','))) { m += 1 }
                        let decl = matches!(toks.get(m).map(|x| &x.0), Some(Tok::Sym(':'))) && !matches!(toks.get(m + 1).map(|x| &x.0), Some(Tok::Sym('=')))
                            || matches!(toks.get(m).map(|x| &x.0), Some(Tok::Sym('=')));
                        if !decl { break }
                    }
                }
                j += 1;
            }
            let en = toks.get(j).map(|x| x.1).unwrap_or(src.len());
            let sec = src[st..en].trim();
            // every section keeps its keyword (the template's own var section comes first; Delphi takes several sections in any order)
            decls.push_str(sec);
            decls.push('\n');
            k = j; continue
        }
        if is_w(t, "procedure") || is_w(t, "function") {
            // to the first `begin` (its own var/const sections may come first), then to its matching end and the ';' after it
            let st = toks[k].1;
            let mut j = k + 1;
            while j < toks.len() && !is_w(&toks[j].0, "begin") { j += 1 }
            let mut depth = 0i32;
            while j < toks.len() {
                if opens(&toks[j].0) { depth += 1 } else if is_w(&toks[j].0, "end") { depth -= 1; if depth == 0 { break } }
                j += 1;
            }
            j += 1; if matches!(toks.get(j).map(|x| &x.0), Some(Tok::Sym(';'))) { j += 1 }
            let en = toks.get(j).map(|x| x.1).unwrap_or(src.len());
            routines.push_str(src[st..en].trim()); routines.push_str("\n\n");
            k = j; continue
        }
        body_from = Some(toks[k].1);
        break
    }
    if let Some(st) = body_from { body = src[st..].trim().to_string(); }
    // a body that is one begin..end block (a program's main block): its inside
    let bt = lex(&body);
    if bt.first().map_or(false, |x| is_w(&x.0, "begin")) {
        let mut depth = 0i32; let mut close = None;
        for (i, (t, ..)) in bt.iter().enumerate() {
            if opens(t) { depth += 1 } else if is_w(t, "end") { depth -= 1; if depth == 0 { close = Some(i); break } }
        }
        if let Some(c) = close {
            let rest: Vec<&Tok> = bt[c + 1..].iter().map(|x| &x.0).collect();
            if rest.iter().all(|t| matches!(t, Tok::Sym('.') | Tok::Sym(';'))) { body = body[bt[0].2..bt[c].1].trim().to_string(); }
        }
    }
    Parts { decls: decls.trim().to_string(), routines: routines.trim().to_string(), body }
}

/// Loops.guard: every `while C do` becomes `while (C) and BpTick(line) do`, every `until C;` `until (C) or not BpTick(line);`.
pub fn guard_loops(src: &str) -> String {
    let toks = lex(src);
    let mut edits: Vec<(usize, usize, String)> = vec![]; // (at, len to replace, text)
    let mut k = 0;
    while k < toks.len() {
        if is_w(&toks[k].0, "while") {
            let line = toks[k].3;
            let mut j = k + 1; let mut depth = 0i32;
            while j < toks.len() { match &toks[j].0 { Tok::Sym('(') => depth += 1, Tok::Sym(')') => depth -= 1, t if depth == 0 && is_w(t, "do") => break, _ => {} } j += 1 }
            if j < toks.len() && k + 1 < j {
                let (a, b) = (toks[k + 1].1, toks[j - 1].2);
                edits.push((a, 0, "(".into())); edits.push((b, 0, format!(") and BpTick({line})")));
            }
            k = j;
        } else if is_w(&toks[k].0, "until") {
            let line = toks[k].3;
            let mut j = k + 1; let mut depth = 0i32;
            while j < toks.len() { match &toks[j].0 { Tok::Sym('(') => depth += 1, Tok::Sym(')') => depth -= 1, Tok::Sym(';') if depth == 0 => break, t if depth == 0 && (is_w(t, "end") || is_w(t, "else")) => break, _ => {} } j += 1 }
            if k + 1 < j {
                let (a, b) = (toks[k + 1].1, toks[j - 1].2);
                edits.push((a, 0, "(".into())); edits.push((b, 0, format!(") or not BpTick({line})")));
            }
            k = j;
        } else { k += 1 }
    }
    let mut out = src.to_string();
    edits.sort_by(|x, y| y.0.cmp(&x.0));
    for (at, len, t) in edits { out.replace_range(at..at + len, &t) }
    out
}

/// Undo.track: the server calls that open and close undo groups, routed through the template's trackers (case as written does not matter).
pub fn track_undo(src: &str) -> String {
    let pats = [("schserver.processcontrol.preprocess", "BpSchPre"), ("schserver.processcontrol.postprocess", "BpSchPost"),
        ("pcbserver.preprocess", "BpPcbPre"), ("pcbserver.postprocess", "BpPcbPost")];
    let low = src.to_ascii_lowercase();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    'scan: while i < src.len() {
        for (p, r) in pats {
            if low[i..].starts_with(p) && (i == 0 || !low.as_bytes()[i - 1].is_ascii_alphanumeric() && low.as_bytes()[i - 1] != b'.') {
                out.push_str(r); i += p.len(); continue 'scan
            }
        }
        let ch = src[i..].chars().next().unwrap(); out.push(ch); i += ch.len_utf8();
    }
    out
}

fn pas_str(s: &str) -> String { format!("'{}'", s.replace('\'', "''")) }

/// Source.ascii: the run file as Altium reads it. Altium reads a script in the Windows ANSI code page, a UTF-8 byte-order mark
/// or not (tested on 26.11: `'Ü'` saved as UTF-8 came back as two characters), so a path or a name in another script than the
/// code page's would arrive garbled. Every character past ASCII inside a string literal becomes a character code (`'a'#220'b'`,
/// which DelphiScript joins, in a `const` too); one in a comment or anywhere else becomes `?`.
pub fn ascii_pas(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let code = |out: &mut String, ch: char| { let mut b = [0u16; 2]; for u in ch.encode_utf16(&mut b) { out.push_str(&format!("#{u}")) } };
    while i < c.len() {
        let ch = c[i];
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' { out.push(if c[i].is_ascii() { c[i] } else { '?' }); i += 1 }
            continue
        }
        if ch == '{' || ch == '(' && c.get(i + 1) == Some(&'*') {
            let close: &[char] = if ch == '{' { &['}'] } else { &['*', ')'] };
            while i < c.len() && !c[i..].starts_with(close) { out.push(if c[i].is_ascii() { c[i] } else { '?' }); i += 1 }
            for _ in 0..close.len() { if i < c.len() { out.push(c[i]); i += 1 } }
            continue
        }
        if ch == '\'' {
            out.push('\''); i += 1;
            while i < c.len() {
                let d = c[i];
                if d == '\'' {
                    if c.get(i + 1) == Some(&'\'') { out.push_str("''"); i += 2; continue }
                    out.push('\''); i += 1; break
                }
                if d.is_ascii() { out.push(d) } else { out.push('\''); code(&mut out, d); out.push('\'') }
                i += 1;
            }
            continue
        }
        out.push(if ch.is_ascii() { ch } else { '?' });
        i += 1;
    }
    out
}

/// Output.text: what the run's files hold, as text. The template writes ASCII only (`BpEsc`: `\\` for a backslash, `\uXXXX` for
/// every character past ASCII), because Altium saves a TStringList in the ANSI code page and has no TEncoding; anything else is
/// read as Windows-1252, never dropped.
pub fn decode_out(b: &[u8]) -> String {
    let s = match std::str::from_utf8(b) { Ok(s) => s.to_string(), Err(_) => b.iter().map(|&x| cp1252(x)).collect() };
    unescape(&s)
}
fn cp1252(b: u8) -> char {
    const HI: [u16; 32] = [0x20AC, 0x81, 0x201A, 0x192, 0x201E, 0x2026, 0x2020, 0x2021, 0x2C6, 0x2030, 0x160, 0x2039, 0x152, 0x8D, 0x17D, 0x8F,
        0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x2DC, 0x2122, 0x161, 0x203A, 0x153, 0x9D, 0x17E, 0x178];
    if (0x80..0xA0).contains(&b) { char::from_u32(HI[(b - 0x80) as usize] as u32).unwrap_or('?') } else { b as char }
}
/// `\\` and `\uXXXX` (UTF-16 units, pairs joined) back to text; any other backslash stays as it is.
pub fn unescape(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut units: Vec<u16> = Vec::with_capacity(c.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' && c.get(i + 1) == Some(&'\\') { units.push('\\' as u16); i += 2; continue }
        if c[i] == '\\' && c.get(i + 1) == Some(&'u') && i + 6 <= c.len() {
            let h: String = c[i + 2..i + 6].iter().collect();
            if let Ok(u) = u16::from_str_radix(&h, 16) { units.push(u); i += 6; continue }
        }
        let mut b = [0u16; 2]; units.extend_from_slice(c[i].encode_utf16(&mut b)); i += 1;
    }
    String::from_utf16_lossy(&units)
}

/// Error.where: an error naming a line of the run file ("at Line:54, Char:19") gets that line quoted, so the agent sees which of its
/// own statements it was (the run file wraps the script, so its line numbers are not the script's).
pub fn locate(err: &str, run_file: &str) -> String {
    let Some(i) = err.find("Line:") else { return err.to_string() };
    let n: usize = err[i + 5..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
    match run_file.lines().nth(n.wrapping_sub(1)) { Some(l) if n > 0 && !l.trim().is_empty() => format!("{err}\n(that line of the script: `{}`)", l.trim()), _ => err.to_string() }
}
/// Run.files: a run's files beside its output `x.out`: `x.exp` (BpExpect), `x.run` (written when the script starts), `x.cancel`
/// (written by the hub when it gave up before the script started: a run Altium starts late then does nothing)
pub fn expect_path(out: &str) -> String { sibling(out, "exp") }
pub fn sibling(out: &str, ext: &str) -> String { format!("{}.{ext}", out.strip_suffix(".out").unwrap_or(out)) }
/// Template.of: the whole run file for `src`, writing its lines to `out` and, when it has ended, `done`; loops end after `guard_s` seconds.
/// ASCII only (`ascii_pas`).
pub fn build(src: &str, out: &str, done: &str, guard_s: u32) -> String {
    // guards first, on the whole script, so a guard names the script's own line
    let p = split(&track_undo(&guard_loops(src)));
    ascii_pas(&format!(r#"// Backplane run (altium/live.rs): the agent's script wrapped; Emit(S) writes a line of output.
var
    BpLines : TStringList;
    BpOutPath, BpDonePath, BpExpPath, BpFailMsg : String;
    BpExps : TStringList;
    BpUntil : Double;
    BpTripped, BpFailed : Boolean;
    BpSchDocs : TList;
    BpPcbOpen : Boolean;
{decls}

// the run's files are ASCII: a backslash doubled, every other character past ASCII as \uXXXX (the hub turns them back)
function BpEsc(S : String) : String;
var I, C : Integer;
begin
    Result := '';
    for I := 1 to Length(S) do
    begin
        C := Ord(S[I]);
        if C = 92 then Result := Result + '\\'
        else if (C > 126) or ((C < 32) and (C <> 9)) then Result := Result + '\u' + IntToHex(C, 4)
        else Result := Result + S[I];
    end;
end;

procedure Emit(S : String);
begin
    BpLines.Add(BpEsc(S));
    BpLines.SaveToFile(BpOutPath);
end;

// the run ends not ok with this message (the lines emitted are kept): a partial result, a check that failed
procedure BpFail(Msg : String);
begin
    if BpFailed then BpFailMsg := BpFailMsg + '; ' + Msg else BpFailMsg := Msg;
    BpFailed := True;
    Emit('BP-FAIL ' + Msg);
end;

// numbers in the script and its output with a decimal point whatever the Windows locale (a comma locale reads '1.5' as an error)
function BpSwap(S, A, B : String) : String;
var I : Integer;
begin
    Result := '';
    for I := 1 to Length(S) do
        if S[I] = A then Result := Result + B else Result := Result + S[I];
end;
function BpNum(S : String) : Double;
begin
    if DecimalSeparator = '.' then Result := StrToFloat(Trim(S)) else Result := StrToFloat(BpSwap(Trim(S), '.', DecimalSeparator));
end;
function BpFmt(Fmt : String; X : Double) : String;
begin
    Result := FormatFloat(Fmt, X);
    if DecimalSeparator <> '.' then Result := BpSwap(Result, DecimalSeparator, '.');
end;

function BpTick(Line : Integer) : Boolean;
begin
    if (not BpTripped) and (Now > BpUntil) then
    begin
        BpTripped := True;
        Emit('BP-LOOP-GUARD: the loop at line ' + IntToStr(Line) + ' still ran after {guard_s} s; every loop was cut short from here on');
    end;
    Result := not BpTripped;
end;

// a window the script will open and how the hub is to work it, in a file of its own (the hub reads it only while a new window
// is up, so it never holds the output file Emit rewrites); BpMayExpect: one that may not come (the run is fine without it)
procedure BpExpect(Title, Steps : String);
begin
    BpExps.Add('BP-EXPECT ' + BpEsc(Title) + ' => ' + BpEsc(Steps));
    BpExps.SaveToFile(BpExpPath);
end;
procedure BpMayExpect(Title, Steps : String);
begin
    BpExps.Add('BP-EXPECT? ' + BpEsc(Title) + ' => ' + BpEsc(Steps));
    BpExps.SaveToFile(BpExpPath);
end;

procedure BpSchPre(Doc : IDispatch; Param : String);
begin
    SchServer.ProcessControl.PreProcess(Doc, Param);
    BpSchDocs.Add(Doc);
end;
procedure BpSchPost(Doc : IDispatch; Param : String);
var I : Integer;
begin
    SchServer.ProcessControl.PostProcess(Doc, Param);
    I := BpSchDocs.IndexOf(Doc);
    if I >= 0 then BpSchDocs.Delete(I);
end;
procedure BpPcbPre;
begin
    PCBServer.PreProcess;
    BpPcbOpen := True;
end;
procedure BpPcbPost;
begin
    PCBServer.PostProcess;
    BpPcbOpen := False;
end;

{routines}

procedure BpBody;
begin
{body}
end;

procedure Run;
var
    Done : TStringList;
    I : Integer;
begin
    BpOutPath := {outp};
    BpDonePath := {donep};
    BpExpPath := {expp};
    Done := TStringList.Create;
    // the hub gave up on this run before Altium started it: nothing is done
    if FileExists({cancelp}) then
    begin
        Done.Add('cancelled');
        Done.SaveToFile(BpDonePath);
        Done.Free;
        Exit;
    end;
    Done.Add('started');
    Done.SaveToFile({runp});
    Done.Clear;
    BpExps := TStringList.Create;
    BpLines := TStringList.Create;
    BpSchDocs := TList.Create;
    BpPcbOpen := False;
    BpTripped := False;
    BpFailed := False;
    BpFailMsg := '';
    BpUntil := Now + {guard_s} / 86400;
    try
        try
            BpBody;
            if BpFailed then
            begin
                Done.Add('error');
                Done.Add(BpEsc(BpFailMsg));
            end
            else Done.Add('ok');
        except
            Done.Add('error');
            Done.Add(BpEsc(ExceptionMessage));
        end;
    finally
        for I := BpSchDocs.Count - 1 downto 0 do
            SchServer.ProcessControl.PostProcess(BpSchDocs.Items(I), '');
        if BpPcbOpen then PCBServer.PostProcess;
        BpLines.SaveToFile(BpOutPath);
        Done.SaveToFile(BpDonePath);
        Done.Free;
        BpLines.Free;
        BpSchDocs.Free;
        BpExps.Free;
    end;
end;
"#, decls = indent(&p.decls), routines = p.routines, body = indent(&p.body), outp = pas_str(out), donep = pas_str(done), expp = pas_str(&expect_path(out)),
        runp = pas_str(&sibling(out, "run")), cancelp = pas_str(&sibling(out, "cancel"))))
}
fn indent(s: &str) -> String { s.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n") }

/// One thing to do in a window a script expects (`BpExpect`).
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// click the button with this caption (`&` and a trailing `...` ignored, any case; Altium's own translations of it count),
    /// waiting up to 30 s for it to be there and enabled
    Click(String),
    /// wait this many seconds
    Wait(f32),
    /// set the text of the window's n-th (from 1) edit box
    Text(usize, String),
    /// from here on, act in the window whose title has this in it (waiting up to 30 s for it to open)
    Window(String),
    /// wait until the window acted in has closed (at most this many seconds)
    Gone(f32),
}
/// A window the script said it will open and how to work it. `optional`: `BpMayExpect`, a window that comes only sometimes.
#[derive(Clone, Debug, PartialEq)]
pub struct Expect { pub title: String, pub steps: Vec<Step>, pub optional: bool }

/// Expect.steps: `click Validate Changes; wait 2; text 1 = C:\x.csv; window Open; gone 60` (steps between `;`; a value cannot hold one).
pub fn parse_steps(s: &str) -> Result<Vec<Step>, String> {
    let mut v = vec![];
    for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (verb, rest) = part.split_once(char::is_whitespace).map(|(a, b)| (a, b.trim())).unwrap_or((part, ""));
        let secs = |r: &str| r.parse::<f32>().ok().filter(|x| x.is_finite() && *x >= 0.0).ok_or_else(|| format!("`{part}`: seconds expected"));
        v.push(match verb.to_ascii_lowercase().as_str() {
            "click" if !rest.is_empty() => Step::Click(rest.to_string()),
            "wait" => Step::Wait(secs(rest)?.min(600.0)),
            "gone" => Step::Gone(secs(rest)?.min(3600.0)),
            "window" if !rest.is_empty() => Step::Window(rest.to_string()),
            "text" => {
                let (n, val) = rest.split_once('=').ok_or_else(|| format!("`{part}`: text N = value"))?;
                let n: usize = n.trim().parse().ok().filter(|n| *n > 0).ok_or_else(|| format!("`{part}`: the edit box's number (from 1) expected"))?;
                Step::Text(n, val.trim().to_string())
            }
            _ => return Err(format!("`{part}`: steps are click <caption>, wait <s>, text <n> = <value>, window <title>, gone <s>")),
        });
    }
    Ok(v)
}
/// The `BP-EXPECT <title> => <steps>` lines (`BP-EXPECT?` for BpMayExpect) a script wrote, in order; a line that does not parse
/// is an Err naming it.
pub fn expectations(out: &str) -> Vec<Result<Expect, String>> {
    out.lines().filter_map(|l| l.strip_prefix("BP-EXPECT? ").map(|r| (r, true)).or_else(|| l.strip_prefix("BP-EXPECT ").map(|r| (r, false)))).map(|(l, optional)| {
        let (t, s) = l.split_once("=>").ok_or_else(|| format!("BpExpect `{l}`: no steps"))?;
        let title = t.trim().to_string();
        if title.is_empty() { return Err(format!("BpExpect `{l}`: no window title")) }
        Ok(Expect { title, steps: parse_steps(s)?, optional })
    }).collect()
}
/// Altium's main window, once it is up ("Home Page - Altium Designer (26.11.1)", "... - Altium Designer Develop (26.x)"): its
/// main form's class, or a title naming Altium, never its splash or a dialog
pub fn is_main_window(class: &str, title: &str) -> bool {
    class == "TDocumentForm" || title.contains("Altium") && !title.to_lowercase().contains("splash")
}
/// Words.same: a caption or title as compared: `&` accelerators and a translation's `(&X)`, a trailing `...`, case and spaces aside.
pub fn norm(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let Some(i) = t.rfind("(&") { if t[i..].len() <= 5 && t[i..].ends_with(')') { t.replace_range(i..i + t[i..].find(')').unwrap() + 1, "") } }
    t.replace('&', "").trim().trim_end_matches("...").trim_end_matches('…').trim().to_lowercase()
}
/// A window title the script named: part of the window's title, any case, in English or in any of Altium's translations of it.
/// A short word (Japanese Altium calls the ECO dialog just "ECO") must be the whole title, not part of one.
pub fn title_matches(want: &str, title: &str, tr: &[String]) -> bool {
    let t = norm(title);
    std::iter::once(want).chain(tr.iter().map(String::as_str)).any(|w| { let w = norm(w); !w.is_empty() && if w.chars().count() <= 4 { t == w } else { t.contains(&w) } })
}
/// A button caption the script named: the same words (`norm`), or one of Altium's translations of them; a trailing `*` matches
/// the caption's start (`Generate all footprints in*`, whose caption ends in a path).
pub fn caption_matches(want: &str, caption: &str, tr: &[String]) -> bool {
    let one = |w: &str| match w.trim().strip_suffix('*') {
        Some(p) => !p.trim().is_empty() && norm(caption).starts_with(&norm(p)),
        None => !norm(w).is_empty() && norm(w) == norm(caption),
    };
    one(want) || tr.iter().any(|t| one(&if want.trim().ends_with('*') { format!("{t}*") } else { t.clone() }))
}

/// Altium's translations of its forms (`System\Resources\<language>\forms.fdb`, one per UI language it ships: German, French,
/// Japanese, Simplified and Traditional Chinese, Korean, Russian in 26.11): each control's `Caption.'<English>'` followed by the
/// translated caption, as length-prefixed UTF-16 strings. English (`norm`) to every translation, so a script's `click Validate
/// Changes` finds "Änderungen bestätigen (&V)" in a German Altium. Read from the installed Altium, never shipped with Backplane.
pub fn fdb_captions(b: &[u8], into: &mut std::collections::HashMap<String, Vec<String>>) {
    let pat: Vec<u8> = "Caption.'".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let str_at = |at: usize, n: usize| -> Option<String> {
        let e = at.checked_add(2 * n)?; if e > b.len() { return None }
        let u: Vec<u16> = b[at..e].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16(&u).ok()
    };
    let mut i = 0;
    while let Some(k) = b[i..].windows(pat.len()).position(|w| w == pat.as_slice()).map(|k| k + i) {
        i = k + 2;
        if k == 0 { continue }
        let n = b[k - 1] as usize;
        let Some(key) = str_at(k, n) else { continue };
        let Some(en) = key.strip_prefix("Caption.'").and_then(|s| s.strip_suffix('\'')) else { continue };
        let j = k + 2 * n;
        let Some(&m) = b.get(j) else { continue };
        let Some(tr) = str_at(j + 1, m as usize) else { continue };
        let (en, tr) = (norm(&en.replace("''", "'")), tr.trim().to_string());
        if en.is_empty() || tr.is_empty() || norm(&tr) == en { continue }
        let v = into.entry(en).or_default();
        if !v.contains(&tr) { v.push(tr) }
    }
}

/// Box.script_error: the box Altium's script engine raises when a script does not compile or fails outside the template's
/// try: a Windows message box titled with the program's name ("X2"), its text "<error> at Line:N, Char:M ... Continue
/// execution?", with a Cancel button (IDCANCEL). Cancel stops the script. This is the only box the hub answers on its own:
/// every other box or window is the person's to answer, whoever raised it (the hub cannot tell a script's box from theirs).
/// In a localized Altium the words may differ ("Zeile"); the line and column numbers stay, so two numbers are enough.
pub fn is_script_error(class: &str, title: &str, exe_stem: &str, text: &str, has_cancel: bool) -> bool {
    let numbers = text.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).count();
    class == "#32770" && has_cancel && title.eq_ignore_ascii_case(exe_stem) && (text.contains("Line:") && text.contains("Char:") || numbers >= 2)
}
/// Box.is: a window that waits for an answer, worth naming in a run's result: Windows' message boxes and dialogs, Delphi's
/// message forms, an owned WPF window (Altium's extensions' boxes: "Unsaved Changes", ActiveRoute's "Information").
pub fn is_box(class: &str, owned: bool) -> bool { matches!(class, "#32770" | "TMessageForm") || owned && class.starts_with("HwndWrapper[") }

/// What a run left on screen for the person: the hub answers none of these.
#[cfg_attr(not(windows), allow(dead_code))]
pub const LEFT: &str = "Altium is showing a window Backplane does not answer on its own (it may be the person's, or one your script opened without BpExpect)";
/// Run.started_and_stopped: what a run whose script the engine stopped tells the agent.
#[cfg_attr(not(windows), allow(dead_code))]
pub const STOPPED: &str = "The script did not finish: before anything else, check with a short script that every document you were editing is still open (Client.GetDocumentByPath) and save it (save_document.pas). If one is gone, redo its change yourself by script (pcb_update_from_sch.pas for the ECO); do not ask the person to redo it.";

/// Integrity.check: Altium more privileged than the hub (Altium "Run as administrator", the hub not) cannot be driven: Windows
/// (UIPI) drops the hub's clicks and keys to it and may drop the command that hands it a script. `None` for Altium's level: its
/// token could not be read, which for the same user means it is elevated. Levels: 0x2000 medium, 0x3000 high.
pub fn privilege_problem(hub: u32, altium: Option<u32>) -> Option<String> {
    let alt = altium.unwrap_or(0x3000);
    (alt > hub).then(|| "Altium Designer is running as administrator and Backplane is not: Windows does not let a program without administrator rights drive one with them. Close Altium and start it normally (not \"Run as administrator\"); or run Backplane as administrator too (not recommended).".to_string())
}
/// Installs.newest: of the Altium installs found (version as Altium writes it, `26.11.1.5`; the X2.EXE), the newest version.
pub fn newest(mut v: Vec<(String, std::path::PathBuf)>) -> Option<std::path::PathBuf> {
    let key = |s: &str| s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    v.sort_by(|a, b| key(&a.0).cmp(&key(&b.0)));
    v.pop().map(|x| x.1)
}

/// What a run came to.
#[derive(Debug, Default)]
pub struct Ran { pub ok: bool, pub output: String, pub error: String, pub dialogs: Vec<String>, pub driven: Vec<String>, pub secs: f32 }
impl Ran {
    pub fn json(&self) -> Value {
        let mut v = json!({"ok": self.ok, "output": self.output, "error": self.error, "dialogs": self.dialogs, "seconds": (self.secs * 10.0).round() / 10.0});
        if !self.driven.is_empty() { v["driven"] = json!(self.driven) }
        v
    }
}
/// The output the agent sees: the BpExpect lines are the hub's, reported as `driven`.
pub fn shown_output(out: &str) -> String { out.lines().filter(|l| !l.starts_with("BP-EXPECT")).collect::<Vec<_>>().join("\n").trim_end().to_string() }
/// Run.verdict: a run whose script said it would open windows is ok only if each was worked as it said (a step it could not do,
/// or a window that never came, unless BpMayExpect said it may not)
pub fn expect_failures(driven: &[(bool, String)]) -> Vec<String> { driven.iter().filter(|(ok, _)| !ok).map(|x| x.1.clone()).collect() }

/// One Altium at a time: every run (status, scripts, the write tool's close/write/reopen, Open in Altium) holds this, in this
/// hub (a mutex) and across hubs on the machine (a lock file in the runs folder), so two runs never work the same windows.
pub struct Session { _g: tokio::sync::MutexGuard<'static, ()>, _f: Option<std::fs::File> }
static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Altium.session: wait (up to `wait_s`) for Altium to be free of other runs.
pub async fn session(wait_s: u64) -> Result<Session, String> {
    let busy = "Altium is busy with another Backplane script (another agent's, or the write tool's); try again when it is done.";
    let g = tokio::time::timeout(std::time::Duration::from_secs(wait_s), GATE.lock()).await.map_err(|_| busy.to_string())?;
    let f = runs_dir().and_then(|d| { std::fs::create_dir_all(&d).ok()?; std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(d.join(".lock")).ok() });
    if let Some(f) = &f {
        let t = std::time::Instant::now();
        loop {
            match f.try_lock() { Ok(()) => break, Err(std::fs::TryLockError::WouldBlock) if t.elapsed().as_secs() < wait_s => tokio::time::sleep(std::time::Duration::from_millis(250)).await, Err(_) => return Err(busy.into()) }
        }
    }
    Ok(Session { _g: g, _f: f })
}
/// Runs.dir: `%LOCALAPPDATA%\Backplane\altium\runs`, the person's own (a path with spaces or any script is fine: the hub hands
/// it to Altium as one quoted argument, tested with `C:\...\Ωμέγα 中文 test`); off Windows, none.
pub fn runs_dir() -> Option<std::path::PathBuf> {
    if !cfg!(windows) { return None }
    let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).or_else(|| std::env::var_os("PUBLIC").map(std::path::PathBuf::from))?;
    Some(base.join("Backplane").join("altium").join("runs"))
}

/// Altium.run: run `script` in the running Altium and wait up to `timeout` seconds. Err: Altium could not be asked at all.
pub async fn run(script: &str, timeout: u32) -> Result<Ran, String> {
    let s = session(timeout as u64 + 60).await?;
    run_in(&s, script, timeout).await
}
/// A run inside a session the caller holds (the write tool's close, write, reopen hold one session throughout).
pub async fn run_in(_s: &Session, script: &str, timeout: u32) -> Result<Ran, String> {
    #[cfg(windows)] { win::run(script, timeout).await }
    #[cfg(not(windows))] { let _ = (script, timeout); Err(NOT_HERE.into()) }
}
#[cfg_attr(windows, allow(dead_code))]
pub const NOT_HERE: &str = "Altium Designer runs on Windows: this hub is not on Windows, so it cannot reach Altium. The viewers still draw the project's .SchDoc and .PcbDoc files.";

/// Altium.open: a document or project opened in the person's Altium: a running Altium opens and shows it by script (and says it
/// did); with none running, Altium starts with it.
pub async fn open_file(path: &str) -> Result<String, String> {
    #[cfg(windows)] {
        let file = path.replace('/', "\\");
        if win::altium()?.is_none() { return win::start_with(&file) }
        let s = session(5).await.map_err(|_| "Altium is busy with an agent's script; try again in a moment.".to_string())?;
        let r = run_in(&s, &open_script(&file), 60).await?;
        if r.ok && r.output.lines().any(|l| l == "opened") { Ok(format!("altium {file}")) } else { Err(format!("Altium did not open {file}: {} {}", r.error, r.output).trim().to_string()) }
    }
    #[cfg(not(windows))] { let _ = path; Err(NOT_HERE.into()) }
}
/// Opens a document (or a project) in Altium and shows it.
pub fn open_script(file: &str) -> String {
    let p = pas_str(file);
    match doc_kind(std::path::Path::new(file)) {
        Some(k) => format!("var\n    SD : IServerDocument;\nSD := Client.OpenDocument('{k}', {p});\nif SD = nil then Emit('could not open') else\nbegin\n    Client.ShowDocument(SD);\n    Emit('opened');\nend;\n"),
        None => format!("var\n    Prj : IProject;\nPrj := GetWorkspace.DM_OpenProject({p}, True);\nif Prj = nil then Emit('could not open') else Emit('opened');\n"),
    }
}
/// Before `altium_write` replaces files: what the running Altium has open of them. A document open with unsaved edits stops the
/// write (Err names it: overwriting it raises Altium's modal "has been modified... Reload?" box, and the person's edits are theirs);
/// the others open in Altium are closed (they hold nothing unsaved) and reopened by `after_write`. Off Windows, or with Altium
/// not running, nothing is open. The caller holds `s` from before this to after `after_write`.
pub async fn before_write(s: &Session, paths: &[std::path::PathBuf]) -> Result<Open, String> {
    #[cfg(windows)] {
        let docs: Vec<String> = paths.iter().filter(|p| doc_kind(p).is_some()).map(|p| p.to_string_lossy().into_owned()).collect();
        if win::altium()?.is_none() || docs.is_empty() { return Ok(Open::default()) }
        let r = run_in(s, &close_script(&docs), 60).await?;
        if !r.ok { return Err(format!("could not check what Altium has open: {} {}", r.error, r.output)) }
        let open = Open::of(&r.output);
        if !open.modified.is_empty() { return Err(format!("open in Altium with unsaved changes: {}. Save or close it there first (save_document.pas / close_document.pas, or ask the person); nothing was written.", open.modified.join(", "))) }
        return Ok(open)
    }
    #[cfg(not(windows))] { let _ = (s, paths); Ok(Open::default()) }
}
/// What `before_write` found and closed.
#[derive(Debug, Default, PartialEq)]
pub struct Open { pub focused: String, pub closed: Vec<String>, pub modified: Vec<String> }
impl Open {
    pub fn of(out: &str) -> Open {
        let mut o = Open::default();
        for l in out.lines() {
            match l.split_once('|') { Some(("focused", p)) => o.focused = p.to_string(), Some(("closed", p)) => o.closed.push(p.to_string()), Some(("modified", p)) => o.modified.push(p.to_string()), _ => {} }
        }
        o
    }
}
/// After `altium_write` wrote files: the documents `before_write` closed are opened again (and shown, the person's focused
/// document last), and a project Altium has open gets the new documents added there too (it does not reload a changed
/// .PrjPcb by itself, and saving the project there would drop them). Off Windows, or with Altium not running, the result
/// goes back as it is.
#[cfg_attr(not(windows), allow(unused_mut))]
pub async fn after_write(s: &Session, mut v: Value, open: &Open, added: &[(String, Vec<String>)]) -> Value {
    #[cfg(windows)] {
        if !matches!(win::altium(), Ok(Some(_))) || (open.closed.is_empty() && added.is_empty()) { return v }
        let mut notes = vec![];
        if !open.closed.is_empty() {
            match run_in(s, &reopen_script(&open.closed, &open.focused), 60).await {
                Ok(r) => notes.extend(r.output.lines().map(|l| l.replace('|', ": ")).chain((!r.ok).then(|| format!("reopening failed: {}", r.error)))),
                Err(e) => notes.push(format!("reopening failed: {e}")),
            }
        }
        for (prj, docs) in added {
            match run_in(s, &add_docs_script(prj, docs), 60).await {
                Ok(r) => notes.extend(r.output.lines().map(|l| l.replace('|', ": ")).chain((!r.ok).then(|| format!("adding to the open project failed: {}", r.error)))),
                Err(e) => notes.push(format!("adding to the open project failed: {e}")),
            }
        }
        v["altium"] = json!(notes);
    }
    #[cfg(not(windows))] { let _ = (s, open, added); }
    v
}
fn doc_kind(p: &std::path::Path) -> Option<&'static str> {
    use crate::files::altium::{kind, Kind};
    match kind(&p.to_string_lossy())? { Kind::Schematic => Some("SCH"), Kind::SchLib => Some("SCHLIB"), Kind::PcbLib => Some("PCBLIB"), Kind::Board => Some("PCB"), Kind::Project => None }
}
/// Reports the focused document and the given ones open with unsaved edits; when none has any, closes those open.
pub fn close_script(docs: &[String]) -> String {
    let calls = |f: &str| docs.iter().map(|d| format!("    {f}({});\n", pas_str(d))).collect::<String>();
    format!("var\n    SD : IServerDocument;\n    Dirty : Boolean;\nprocedure BpCheck(P : String);\nbegin\n    SD := Client.GetDocumentByPath(P);\n    if SD = nil then Exit;\n    if SD.Modified <> False then begin Dirty := True; Emit('modified|' + P); end;\nend;\nprocedure BpClose(P : String);\nbegin\n    SD := Client.GetDocumentByPath(P);\n    if SD = nil then Exit;\n    Client.CloseDocument(SD);\n    Emit('closed|' + P);\nend;\nDirty := False;\nif GetWorkspace.DM_FocusedDocument <> nil then Emit('focused|' + GetWorkspace.DM_FocusedDocument.DM_FullPath);\n{}if not Dirty then\nbegin\n{}end;\n", calls("BpCheck"), calls("BpClose"))
}
/// Opens the closed documents again and shows them, then shows the document that was focused.
pub fn reopen_script(docs: &[String], focused: &str) -> String {
    let mut calls = String::new();
    for d in docs { if let Some(k) = doc_kind(std::path::Path::new(d)) { calls.push_str(&format!("BpOpen({}, '{k}');\n", pas_str(d))) } }
    format!("var\n    SD : IServerDocument;\nprocedure BpOpen(P : String; K : String);\nbegin\n    SD := Client.OpenDocument(K, P);\n    if SD = nil then begin Emit('could not reopen in Altium|' + P); Exit; end;\n    Client.ShowDocument(SD);\n    Emit('reopened in Altium|' + P);\nend;\n{calls}if {f} <> '' then\nbegin\n    SD := Client.GetDocumentByPath({f});\n    if SD <> nil then Client.ShowDocument(SD);\nend;\n", f = pas_str(focused))
}
/// Adds documents to a project Altium has open (nothing when it is not open).
pub fn add_docs_script(prj: &str, docs: &[String]) -> String {
    let calls: String = docs.iter().map(|d| format!("    BpAdd({});\n", pas_str(d))).collect();
    format!("var\n    WS : IWorkspace;\n    Prj : IProject;\n    I : Integer;\nprocedure BpAdd(D : String);\nvar\n    J : Integer;\nbegin\n    for J := 0 to Prj.DM_LogicalDocumentCount - 1 do\n        if SameText(Prj.DM_LogicalDocuments(J).DM_FullPath, D) then Exit;\n    Prj.DM_AddSourceDocument(D);\n    Emit('added to the project open in Altium (unsaved there)|' + D);\nend;\nWS := GetWorkspace;\nPrj := nil;\nfor I := 0 to WS.DM_ProjectCount - 1 do\n    if SameText(WS.DM_Projects(I).DM_ProjectFullPath, {p}) then Prj := WS.DM_Projects(I);\nif Prj <> nil then\nbegin\n{calls}end;\n", p = pas_str(prj))
}

/// Altium.status: whether Altium runs, where, and its open projects and documents (a run of a canned script).
pub const STATUS_SCRIPT: &str = r#"var
    WS : IWorkspace;
    P : IProject;
    D : IDocument;
    I, J : Integer;
WS := GetWorkspace;
Emit('version ' + GetCurrentProductBuild);
for I := 0 to WS.DM_ProjectCount - 1 do
begin
    P := WS.DM_Projects(I);
    if P.DM_ProjectFullPath <> 'Free Documents' then Emit('project ' + P.DM_ProjectFullPath);
    for J := 0 to P.DM_LogicalDocumentCount - 1 do
    begin
        D := P.DM_LogicalDocuments(J);
        Emit('  ' + D.DM_DocumentKind + ' ' + D.DM_FullPath);
    end;
end;
if WS.DM_FocusedDocument <> nil then Emit('focused ' + WS.DM_FocusedDocument.DM_FullPath);
"#;

#[cfg(windows)]
mod win {
    use super::{Expect, Ran, Step};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, WPARAM};
    use windows_sys::Win32::Security::{GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel, TOKEN_MANDATORY_LABEL, TOKEN_QUERY};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::Registry::{RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
    use windows_sys::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows, GetClassNameW, GetClientRect, GetDlgCtrlID, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostMessageW, SendMessageTimeoutW, SetForegroundWindow, GWL_STYLE, GW_OWNER, SMTO_ABORTIFHUNG, WM_CLOSE, WM_COMMAND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NULL, WM_SETTEXT, WS_DISABLED};

    fn wide(v: &[u16]) -> String { String::from_utf16_lossy(&v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())]) }
    fn w16(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

    /// Every running X2.EXE's pid
    fn x2_pids() -> Vec<u32> {
        let mut v = vec![];
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap as isize == -1 { return v }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut e) != 0 { loop { if wide(&e.szExeFile).eq_ignore_ascii_case("X2.EXE") { v.push(e.th32ProcessID) } if Process32NextW(snap, &mut e) == 0 { break } } }
            CloseHandle(snap);
        }
        v
    }
    fn session_of(pid: u32) -> Option<u32> { let mut s = 0u32; (unsafe { ProcessIdToSessionId(pid, &mut s) } != 0).then_some(s) }
    fn image(pid: u32) -> String {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() { return String::new() }
            let mut buf = vec![0u16; 1024]; let mut n = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n) != 0;
            CloseHandle(h);
            if ok { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() }
        }
    }
    /// A process's integrity level (0x2000 medium, 0x3000 high); None when its token cannot be read
    fn token_level(h: HANDLE) -> Option<u32> {
        unsafe {
            let mut t: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(h, TOKEN_QUERY, &mut t) == 0 { return None }
            let mut buf = vec![0u8; 256]; let mut n = 0u32;
            let ok = GetTokenInformation(t, TokenIntegrityLevel, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n) != 0;
            CloseHandle(t);
            if !ok { return None }
            let lab = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
            let sid = lab.Label.Sid;
            let cnt = *GetSidSubAuthorityCount(sid);
            if cnt == 0 { return None }
            Some(*GetSidSubAuthority(sid, (cnt - 1) as u32))
        }
    }
    pub fn integrity(pid: u32) -> Option<u32> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() { return None }
            let l = token_level(h);
            CloseHandle(h);
            l
        }
    }
    pub fn own_integrity() -> u32 { unsafe { token_level(GetCurrentProcess()) }.unwrap_or(0x2000) }

    /// A visible top-level window of one of `pids`: (hwnd, pid, class, title, owner)
    #[derive(Clone, Debug)]
    struct Top { h: isize, pid: u32, class: String, title: String, owner: isize }
    fn class_of(h: HWND) -> String { let mut c = [0u16; 256]; let n = unsafe { GetClassNameW(h, c.as_mut_ptr(), c.len() as i32) }; String::from_utf16_lossy(&c[..n.max(0) as usize]) }
    fn text_of(h: HWND) -> String { let mut t = [0u16; 1024]; let n = unsafe { GetWindowTextW(h, t.as_mut_ptr(), t.len() as i32) }; String::from_utf16_lossy(&t[..n.max(0) as usize]) }
    unsafe extern "system" fn top_any(h: HWND, l: LPARAM) -> BOOL {
        let (pids, acc) = &mut *(l as *mut (Vec<u32>, Vec<Top>));
        let mut p = 0u32; GetWindowThreadProcessId(h, &mut p);
        if !pids.contains(&p) || IsWindowVisible(h) == 0 { return 1 }
        acc.push(Top { h: h as isize, pid: p, class: class_of(h), title: text_of(h), owner: GetWindow(h, GW_OWNER) as isize });
        1
    }
    fn tops(pids: &[u32]) -> Vec<Top> {
        let mut st: (Vec<u32>, Vec<Top>) = (pids.to_vec(), vec![]);
        unsafe { EnumWindows(Some(top_any), &mut st as *mut (Vec<u32>, Vec<Top>) as LPARAM); }
        st.1
    }
    fn main_of(pid: u32) -> Option<isize> { tops(&[pid]).into_iter().find(|t| t.owner == 0 && super::is_main_window(&t.class, &t.title) && !t.title.is_empty()).map(|t| t.h) }

    /// The Altium this hub drives: the X2.EXE in the hub's own Windows session that has its main window up (a second X2.EXE that
    /// only hands a command over has none). Err: Altium runs only in another session (another user, Remote Desktop, a service:
    /// Windows does not let a program reach windows there), or with more rights than the hub (`privilege_problem`).
    pub struct Inst { pub pid: u32, pub exe: String }
    pub fn altium() -> Result<Option<Inst>, String> {
        let me = session_of(unsafe { GetCurrentProcessId() });
        let all = x2_pids();
        let here: Vec<u32> = all.iter().copied().filter(|p| me.is_some() && session_of(*p) == me).collect();
        if here.is_empty() {
            if !all.is_empty() { return Err("Altium Designer is running, but in another Windows session (another user signed in, Remote Desktop, or a service), where Backplane cannot reach it. Start Altium in the session Backplane runs in.".into()) }
            return Ok(None)
        }
        let pid = here.iter().copied().find(|p| main_of(*p).is_some()).unwrap_or(here[0]);
        if let Some(why) = super::privilege_problem(own_integrity(), integrity(pid)) { return Err(why) }
        Ok(Some(Inst { pid, exe: image(pid) }))
    }

    /// A window's controls, all levels down: (hwnd, class, text, control id)
    unsafe extern "system" fn child_any(h: HWND, l: LPARAM) -> BOOL {
        let acc = &mut *(l as *mut Vec<(isize, String, String, i32)>);
        acc.push((h as isize, class_of(h), text_of(h), GetDlgCtrlID(h)));
        1
    }
    fn controls(w: isize) -> Vec<(isize, String, String, i32)> {
        let mut v: Vec<(isize, String, String, i32)> = vec![];
        unsafe { EnumChildWindows(w as HWND, Some(child_any), &mut v as *mut Vec<(isize, String, String, i32)> as LPARAM); }
        v
    }
    fn usable(h: isize) -> bool { unsafe { IsWindowVisible(h as HWND) != 0 && (GetWindowLongW(h as HWND, GWL_STYLE) as u32 & WS_DISABLED) == 0 } }
    fn alive(h: isize) -> bool { unsafe { IsWindow(h as HWND) != 0 && IsWindowVisible(h as HWND) != 0 } }
    fn pid_of(h: isize) -> u32 { let mut p = 0u32; unsafe { GetWindowThreadProcessId(h as HWND, &mut p); } p }
    fn is_button(class: &str) -> bool { class == "Button" || class == "TButton" || class.contains("Btn") }
    /// Whether `h` is `root` or owned by it, at any depth
    fn owned_by(h: isize, root: isize) -> bool {
        let mut x = h;
        for _ in 0..12 { if x == root { return true } if x == 0 { return false } x = unsafe { GetWindow(x as HWND, GW_OWNER) } as isize; }
        false
    }
    /// Windows' file box (Open, Save As): a dialog with the file-name box (control 1148); its title and buttons are in the language
    /// of Windows, so `window Open` and `click Open`/`click Save` find it by control (the box itself, IDOK) whatever the language
    fn is_file_box(h: isize) -> bool { class_of(h as HWND) == "#32770" && controls(h).iter().any(|c| c.3 == 1148) }
    fn file_word(w: &str) -> bool { matches!(super::norm(w).as_str(), "open" | "save" | "save as") }
    /// A click on a control as the mouse makes it (posted, so a button that opens a box of its own does not hold the hub): Altium's
    /// own buttons are drawn controls that take no BM_CLICK
    fn click(h: isize) {
        unsafe {
            let mut r: RECT = std::mem::zeroed(); GetClientRect(h as HWND, &mut r);
            let at = (((r.bottom / 2) as u32 & 0xffff) << 16 | ((r.right / 2) as u32 & 0xffff)) as LPARAM;
            PostMessageW(h as HWND, WM_LBUTTONDOWN, 1, at);
            PostMessageW(h as HWND, WM_LBUTTONUP, 0, at);
        }
    }

    /// Altium's translations of its forms, read once from the install the hub drives (`fdb_captions`)
    static TR: OnceLock<Mutex<(String, HashMap<String, Vec<String>>)>> = OnceLock::new();
    pub fn translations(exe: &str, words: &str) -> Vec<String> {
        let m = TR.get_or_init(|| Mutex::new((String::new(), HashMap::new())));
        let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
        if g.0 != exe {
            let mut map = HashMap::new();
            if let Some(dir) = std::path::Path::new(exe).parent().map(|p| p.join("System").join("Resources")) {
                for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    if let Ok(b) = std::fs::read(e.path().join("forms.fdb")) { super::fdb_captions(&b, &mut map) }
                }
            }
            *g = (exe.to_string(), map);
        }
        g.1.get(&super::norm(words.trim().trim_end_matches('*'))).cloned().unwrap_or_default()
    }

    /// While an expected window is worked, a box it raises with a single button (Altium's "Information: Component not found in
    /// available libraries" after Execute Changes, seen on 26.11) blocks it and offers no choice: its button is pressed, and the
    /// run's `driven` says so. Only boxes that came up during the window's steps, owned, of Altium's process; a box with more
    /// than one button is not touched (the step then fails and says what is up).
    fn answer_single(pid: u32, w: isize, known: &mut Vec<isize>, log: &mut Vec<String>) {
        if unsafe { IsWindowEnabled(w as HWND) } != 0 { return }
        for t in tops(&[pid]) {
            if known.contains(&t.h) || t.owner == 0 || !super::is_box(&t.class, true) || !alive(t.h) { continue }
            let bs: Vec<(isize, String, String, i32)> = controls(t.h).into_iter().filter(|c| is_button(&c.1) && usable(c.0)).collect();
            if bs.len() != 1 { continue }
            known.push(t.h);
            if t.class == "#32770" { unsafe { PostMessageW(t.h as HWND, WM_COMMAND, bs[0].3 as WPARAM, 0); } } else { click(bs[0].0) }
            std::thread::sleep(Duration::from_millis(400));
            log.push(format!("answered \"{}\" (its only button, {})", t.title, bs[0].2.replace('&', "")));
        }
    }
    /// Drive.window: do an expected window's steps; a line per step done, Err (with what the window offers) at the first that cannot
    /// be done. Before every step the window is checked again: still there, still Altium's, still the one named.
    fn drive(first: isize, e: &Expect, pid: u32, before: &[isize], until_t: Instant, exe: &str) -> Result<Vec<String>, (Vec<String>, String)> {
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }
        let mut w = first;
        let mut wtitle = e.title.clone();
        let mut log = vec![];
        let mut known: Vec<isize> = tops(&[pid]).into_iter().map(|t| t.h).collect();
        let left = || until_t.saturating_duration_since(Instant::now()).as_secs_f32();
        let until = |secs: f32, f: &mut dyn FnMut() -> bool| { let t = Instant::now(); let secs = secs.min(left()); while t.elapsed().as_secs_f32() < secs { if f() { return true } std::thread::sleep(Duration::from_millis(150)) } f() };
        for st in &e.steps {
            let file_box = |h: isize| is_file_box(h);
            let same = |h: isize, t: &str| alive(h) && pid_of(h) == pid && (super::title_matches(t, &text_of(h as HWND), &translations(exe, t)) || file_word(t) && file_box(h));
            if !matches!(st, Step::Gone(_) | Step::Window(_) | Step::Wait(_)) && !same(w, &wtitle) { return Err((log, format!("the window `{wtitle}` is no longer there"))) }
            if left() <= 0.0 { return Err((log, "the run's time ran out".into())) }
            // a one-button box the last step raised: pressed, then the window is waited for (up to 10 s) to take input again
            if !matches!(st, Step::Wait(_) | Step::Window(_)) {
                let t0 = Instant::now();
                loop {
                    answer_single(pid, w, &mut known, &mut log);
                    if !alive(w) || unsafe { IsWindowEnabled(w as HWND) } != 0 || t0.elapsed().as_secs_f32() > 10.0f32.min(left()) { break }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
            match st {
                Step::Wait(s) => { std::thread::sleep(Duration::from_secs_f32(s.min(left()))); log.push(format!("waited {s} s")) }
                Step::Click(c) => {
                    let tr = translations(exe, c);
                    let mut hit = None;
                    let fb = file_word(c) && file_box(w);
                    if !until(30.0, &mut || { hit = controls(w).into_iter().find(|(h, cl, t, id)| usable(*h) && (if fb { *id == 1 && is_button(cl) } else { super::caption_matches(c, t, &tr) })).map(|x| x.0); hit.is_some() && unsafe { IsWindowEnabled(w as HWND) } != 0 }) {
                        if unsafe { IsWindowEnabled(w as HWND) } == 0 {
                            let up: Vec<String> = tops(&[pid]).into_iter().filter(|t| !known.contains(&t.h) && t.owner != 0).map(|t| format!("\"{}\"", t.title)).collect();
                            return Err((log, format!("the window is blocked by another one ({}) that Backplane does not answer", up.join(", "))))
                        }
                        let offers: Vec<String> = controls(w).into_iter().filter(|(h, _, t, _)| !t.trim().is_empty() && usable(*h)).map(|x| x.2).collect();
                        return Err((log, format!("no enabled `{c}` in the window (it offers: {})", offers.join(", "))))
                    }
                    if !same(w, &wtitle) { return Err((log, format!("the window `{wtitle}` changed before the click"))) }
                    click(hit.unwrap()); std::thread::sleep(Duration::from_millis(300));
                    log.push(format!("clicked {c}"));
                }
                Step::Text(n, val) => {
                    let edits: Vec<isize> = controls(w).into_iter().filter(|(h, cl, _, _)| cl.to_ascii_lowercase().contains("edit") && usable(*h)).map(|x| x.0).collect();
                    let Some(h) = edits.get(n - 1) else { return Err((log, format!("the window has {} edit boxes, not {n}", edits.len()))) };
                    let mut wide: Vec<u16> = val.encode_utf16().collect(); wide.push(0);
                    let mut res = 0usize;
                    unsafe { SendMessageTimeoutW(*h as HWND, WM_SETTEXT, 0, wide.as_ptr() as LPARAM, SMTO_ABORTIFHUNG, 5000, &mut res); }
                    log.push(format!("typed into box {n}"));
                }
                Step::Window(t) => {
                    let tr = translations(exe, t);
                    let mut hit = None;
                    if !until(30.0, &mut || { hit = tops(&[pid]).into_iter().find(|x| !before.contains(&x.h) && x.owner != 0 && (super::title_matches(t, &x.title, &tr) || file_word(t) && is_file_box(x.h))).map(|x| x.h); hit.is_some() }) {
                        return Err((log, format!("no window `{t}` opened")))
                    }
                    w = hit.unwrap(); wtitle = t.clone(); log.push(format!("in {t}"));
                }
                Step::Gone(s) => {
                    if !until(*s, &mut || !alive(w)) { return Err((log, format!("the window was still open after {s} s"))) }
                    log.push("closed".into());
                }
            }
        }
        Ok(log)
    }

    /// The window that was in front before a run, in front again (unless it was Altium's own, or the person has moved on to another).
    /// Windows may refuse (its foreground lock): then Altium stays in front.
    fn give_back(pid: u32, front: isize) {
        if front == 0 { return }
        unsafe {
            let (mut fp, mut np) = (0u32, 0u32);
            GetWindowThreadProcessId(front as HWND, &mut fp);
            let now = GetForegroundWindow();
            GetWindowThreadProcessId(now, &mut np);
            if fp != pid && np == pid { SetForegroundWindow(front as HWND); }
        }
    }

    /// Installs.find: `BACKPLANE_ALTIUM_EXE` if set; else the newest install Altium registered (HKLM\SOFTWARE\Altium\Builds\*:
    /// ProgramsInstallPath, Version; its folder name is not its version: 26.11 installs into AD23 when updated in place), else
    /// the folders under Program Files\Altium.
    pub fn exe() -> Option<String> {
        if let Some(p) = std::env::var_os("BACKPLANE_ALTIUM_EXE").map(PathBuf::from).filter(|p| p.is_file()) { return Some(p.to_string_lossy().into_owned()) }
        let mut found: Vec<(String, PathBuf)> = registered();
        if found.is_empty() {
            let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
            found = std::fs::read_dir(std::path::Path::new(&pf).join("Altium")).into_iter().flatten().flatten()
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().join("X2.EXE"))).filter(|x| x.1.is_file()).collect();
        }
        super::newest(found).map(|p| p.to_string_lossy().into_owned())
    }
    fn reg_str(k: HKEY, sub: &[u16], name: &str) -> Option<String> {
        let mut buf = vec![0u16; 1024]; let mut n = (buf.len() * 2) as u32;
        let ok = unsafe { RegGetValueW(k, sub.as_ptr(), w16(name).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut n) } == 0;
        ok.then(|| wide(&buf))
    }
    fn registered() -> Vec<(String, PathBuf)> {
        let mut v = vec![];
        unsafe {
            let mut k: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, w16(r"SOFTWARE\Altium\Builds").as_ptr(), 0, KEY_READ, &mut k) != 0 { return v }
            for i in 0.. {
                let mut name = vec![0u16; 512]; let mut n = name.len() as u32;
                if RegEnumKeyExW(k, i, name.as_mut_ptr(), &mut n, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) != 0 { break }
                let sub: Vec<u16> = name[..n as usize].iter().copied().chain(std::iter::once(0)).collect();
                if let Some(dir) = reg_str(k, &sub, "ProgramsInstallPath") {
                    let x2 = PathBuf::from(dir).join("X2.EXE");
                    let ver = reg_str(k, &sub, "Version").or_else(|| reg_str(k, &sub, "FullBuild")).unwrap_or_default();
                    if x2.is_file() { v.push((ver, x2)) }
                }
            }
            RegCloseKey(k);
        }
        v
    }

    /// Altium not running: the newest installed one is started (on the hub's desktop) and waited for until its main window is up
    /// and it has had a moment to load its servers (a cold start takes about a minute), so no one has to open it by hand. A
    /// Backplane run as administrator starts it as administrator too.
    async fn start_altium() -> Result<Inst, String> {
        let Some(x2) = exe() else { return Err("Altium Designer is not installed on this machine (or set BACKPLANE_ALTIUM_EXE to its X2.EXE).".into()) };
        let child = crate::proc::std_cmd(crate::proc::Kind::Local, &x2).spawn().map_err(|e| format!("cannot start Altium: {e}"))?;
        let mine = child.id();
        let t0 = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let up = if main_of(mine).is_some() { Some(mine) } else { altium().ok().flatten().map(|a| a.pid).filter(|p| main_of(*p).is_some()) };
            if let Some(pid) = up { tokio::time::sleep(Duration::from_secs(10)).await; return Ok(Inst { pid, exe: image(pid) }) }
            if t0.elapsed() > Duration::from_secs(300) { return Err("Altium Designer was started but its window did not come up within 5 minutes (a licence or sign-in window may be waiting for the person). Check it with altium_status in a minute.".into()) }
        }
    }
    pub fn start_with(file: &str) -> Result<String, String> {
        let Some(x2) = exe() else { return Err("Altium Designer is not installed on this machine".into()) };
        crate::proc::std_cmd(crate::proc::Kind::Local, &x2).arg(file).spawn().map_err(|e| format!("cannot start Altium: {e}"))?;
        Ok(format!("altium {file}"))
    }

    /// A run that ended in a timeout after its script had started: it may still be running in Altium (or waiting on a window). No
    /// new script goes to Altium until it has ended (its .done is there), Altium is answering and free of windows again, or that
    /// Altium is gone.
    struct Pending { done: PathBuf, pid: u32, at: Instant, windows: Vec<isize> }
    static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
    fn idle(pid: u32, windows: &[isize]) -> bool {
        let Some(m) = main_of(pid) else { return false };
        let mut r = 0usize;
        let answers = unsafe { SendMessageTimeoutW(m as HWND, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 1500, &mut r) } != 0;
        answers && unsafe { IsWindowEnabled(m as HWND) } != 0 && !windows.iter().any(|h| alive(*h))
    }
    /// Pending.check: Ok(note) when Altium may take a new script (the note says what became of the earlier one), Err while it may not.
    fn reconcile(pid: u32) -> Result<Option<String>, String> {
        let mut g = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        let Some(p) = g.as_ref() else { return Ok(None) };
        let note = if p.done.exists() {
            let st = super::decode_out(&std::fs::read(&p.done).unwrap_or_default());
            Some(format!("(the earlier script that ran past its timeout has ended: {})", st.lines().collect::<Vec<_>>().join(": ")))
        } else if p.pid != pid || main_of(p.pid).is_none() { Some("(the Altium that was running the earlier script is gone)".into()) }
        else if idle(pid, &p.windows) { Some("(the earlier script that ran past its timeout never reported back; Altium is idle now: check that its work is there)".into()) }
        else {
            return Err(format!("The earlier script, started {} s ago, has not finished: Altium is still running it or waiting on a window. Backplane sends no other script until it ends; look at what it was doing, or ask the person to check Altium.", p.at.elapsed().as_secs()))
        };
        *g = None;
        Ok(note)
    }

    pub async fn run(script: &str, timeout: u32) -> Result<Ran, String> {
        let (started_it, inst) = match altium()? { Some(a) => (false, a), None => (true, start_altium().await?) };
        let (pid, exe) = (inst.pid, inst.exe);
        if exe.is_empty() { return Err("Altium Designer is running but its program path cannot be read.".into()) }
        let stem = std::path::Path::new(&exe).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "X2".into());
        let note = reconcile(pid)?;
        // a modal window already up (the person's, most likely): a script would wait behind it; say so instead of sending one
        if let Some(m) = main_of(pid) {
            if unsafe { IsWindowEnabled(m as HWND) } == 0 {
                let up: Vec<String> = tops(&[pid]).into_iter().filter(|t| t.owner != 0 && !t.title.is_empty()).map(|t| format!("\"{}\"", t.title)).collect();
                return Err(format!("Altium is showing a window that waits for an answer ({}): a script would wait behind it. Nothing was sent; ask the person to finish with it, then run again.", if up.is_empty() { "a dialog".to_string() } else { up.join(", ") }))
            }
        }
        let d = super::runs_dir().ok_or("no folder for Altium runs")?;
        std::fs::create_dir_all(&d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
        // old runs: keep the newest 150 files
        if let Ok(rd) = std::fs::read_dir(&d) {
            let mut v: Vec<_> = rd.flatten().filter(|e| e.file_name() != ".lock").filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
            v.sort(); let n = v.len(); for (_, p) in v.into_iter().take(n.saturating_sub(150)) { let _ = std::fs::remove_file(p); }
        }
        let id = format!("bp{}{:04x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0), rand_u16());
        let (pas, out, done) = (d.join(format!("{id}.pas")), d.join(format!("{id}.out")), d.join(format!("{id}.done")));
        let o = out.to_string_lossy().into_owned();
        let (exp, runm, cancel) = (PathBuf::from(super::expect_path(&o)), PathBuf::from(super::sibling(&o, "run")), PathBuf::from(super::sibling(&o, "cancel")));
        let guard = timeout.saturating_sub(3).max(2);
        let src = super::build(script, &o, &done.to_string_lossy(), guard);
        std::fs::write(&pas, &src).map_err(|e| format!("cannot write {}: {e}", pas.display()))?;
        // every window Altium had before the run: a window of the run's is a new one
        let before: Vec<isize> = tops(&[pid]).iter().map(|x| x.h).collect();
        let (mut announced, mut pending): (usize, Vec<Expect>) = (0, vec![]);
        let mut results: Vec<(bool, String)> = vec![];
        let mut seen: HashMap<isize, Instant> = HashMap::new();
        let mut reported: Vec<isize> = vec![];
        let mut handled: Vec<isize> = vec![];
        let mut stopped_at: Option<(Instant, String)> = None;
        let mut pressed: Option<(Instant, String)> = None;
        // the window in front before the run: Altium comes forward when a script starts; the person's window is given back after
        let front = unsafe { GetForegroundWindow() } as isize;
        let started = Instant::now();
        // one argument: Rust quotes it when the path has a space, which Altium reads (an unquoted path with a space starts a
        // second Altium instead)
        let arg = format!("-RScriptingSystem:RunScriptFile(FileName={}|ProcName=Run)", pas.to_string_lossy());
        let child = crate::proc::tokio_cmd(crate::proc::Kind::Local, &exe).arg(arg).spawn().map_err(|e| format!("cannot start {exe}: {e}"))?;
        let fwd = child.id().unwrap_or(0);
        drop(child);
        let mut ran = Ran::default();
        let end = started + Duration::from_secs(timeout as u64);
        let read = |p: &PathBuf| -> Result<String, String> { match std::fs::read(p) { Ok(b) => Ok(super::decode_out(&b)), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()), Err(e) => Err(format!("cannot read {}: {e}", p.display())) } };
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if done.exists() {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let st = read(&done)?;
                let mut ls = st.lines();
                let first = ls.next().map(str::trim).unwrap_or("");
                ran.ok = first == "ok";
                if !ran.ok { ran.error = ls.collect::<Vec<_>>().join("\n").trim().to_string(); if ran.error.is_empty() { ran.error = if first == "cancelled" { "the run was cancelled before it started".into() } else { "the script failed".into() } } }
                break
            }
            let pids: Vec<u32> = if fwd != 0 && fwd != pid { vec![pid, fwd] } else { vec![pid] };
            let open = tops(&pids);
            let new: Vec<&Top> = open.iter().filter(|t| !before.contains(&t.h)).collect();
            for t in &new { seen.entry(t.h).or_insert_with(Instant::now); }
            // windows the script said it opens (BpExpect, written before the call that opens them), read only while a new window is
            // up (the script is then waiting on it, not writing the file)
            if !new.is_empty() {
                for e in super::expectations(&read(&exp)?).into_iter().skip(announced) { announced += 1; match e { Ok(e) => pending.push(e), Err(m) => results.push((false, m)) } }
            }
            // an expected window: Altium's, new since the run began, owned (a dialog of Altium's, not a top-level window of its own),
            // its title the one the script named
            let hit = pending.iter().enumerate().find_map(|(i, e)| {
                let tr = translations(&exe, &e.title);
                new.iter().find(|t| t.pid == pid && t.owner != 0 && !handled.contains(&t.h) && super::title_matches(&e.title, &t.title, &tr)).map(|t| (i, t.h))
            });
            if let Some((i, w)) = hit {
                let e = pending.remove(i);
                handled.push(w);
                let (title, ba, x) = (e.title.clone(), before.clone(), exe.clone());
                let r = tokio::task::spawn_blocking(move || {
                    let r = drive(w, &e, pid, &ba, end, &x);
                    // steps that could not be done: the window worked is closed, as Cancel would, with the windows it opened (owned by it),
                    // so Altium is not left waiting on a modal window of the script's; nothing else is touched
                    if r.is_err() {
                        for t in tops(&[pid]).into_iter().filter(|t| !ba.contains(&t.h) && t.h != w && owned_by(t.h, w)) { unsafe { PostMessageW(t.h as HWND, WM_CLOSE, 0, 0); } }
                        if alive(w) && pid_of(w) == pid { unsafe { PostMessageW(w as HWND, WM_CLOSE, 0, 0); } }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                    r
                }).await.unwrap_or_else(|x| Err((vec![], x.to_string())));
                results.push(match r { Ok(l) => (true, format!("{title}: {}", l.join(", "))), Err((l, m)) => (false, format!("{title}: {}{m}; closed it", l.iter().map(|x| format!("{x}, ")).collect::<String>())) });
                continue
            }
            for t in &new {
                if handled.contains(&t.h) || !alive(t.h) { continue }
                let cs = controls(t.h);
                // the script engine's own box: the script did not compile or failed outside try. Cancel (IDCANCEL) stops it.
                if t.pid == pid && stopped_at.is_none() {
                    let text: String = cs.iter().filter(|c| c.1 == "Static" && !c.2.trim().is_empty()).map(|c| c.2.trim().replace("\r\n", " ")).collect::<Vec<_>>().join(" ");
                    if super::is_script_error(&t.class, &t.title, &stem, &text, cs.iter().any(|c| c.3 == 2 && is_button(&c.1))) {
                        unsafe { PostMessageW(t.h as HWND, WM_COMMAND, 2 as WPARAM, 0); }
                        handled.push(t.h);
                        ran.dialogs.push(format!("script error: {text} => Cancel (stops the script)"));
                        stopped_at = Some((Instant::now(), text));
                        continue
                    }
                }
                // after that, the engine's one-button "Error" box (a script that did not compile): closed
                if t.pid == pid && t.class == "TMessageForm" && t.owner != 0 && stopped_at.as_ref().is_some_and(|(at, _)| at.elapsed() < Duration::from_secs(4)) && cs.iter().filter(|c| is_button(&c.1)).count() == 1 {
                    unsafe { PostMessageW(t.h as HWND, WM_CLOSE, 0, 0); }
                    handled.push(t.h);
                    ran.dialogs.push(format!("{} (the script engine's, after the error) => closed", t.title));
                    continue
                }
                // a box of the command's own X2.EXE (the hub started it to hand the script over): closed
                if t.pid == fwd && fwd != pid && super::is_box(&t.class, t.owner != 0) {
                    unsafe { PostMessageW(t.h as HWND, WM_CLOSE, 0, 0); }
                    handled.push(t.h);
                    ran.dialogs.push(format!("{} (the command's own box) => closed", t.title));
                    continue
                }
                // a box with a single button that came up while the script was running: Altium's UI thread was running the script, so
                // the person could not have opened it; it is the script's (or Altium's, on the script's behalf: "Access violation in
                // ScriptingSystem.DLL", "Information"), and its only button decides nothing. Pressed, and said.
                if t.pid == pid && t.owner != 0 && matches!(t.class.as_str(), "#32770" | "TMessageForm") && runm.exists() && !done.exists() {
                    let bs: Vec<&(isize, String, String, i32)> = cs.iter().filter(|c| is_button(&c.1) && usable(c.0)).collect();
                    if bs.len() == 1 && seen.get(&t.h).is_some_and(|s| s.elapsed() > Duration::from_millis(600)) {
                        if t.class == "#32770" { unsafe { PostMessageW(t.h as HWND, WM_COMMAND, bs[0].3 as WPARAM, 0); } } else { click(bs[0].0) }
                        handled.push(t.h);
                        ran.dialogs.push(format!("\"{}\" (one button, raised while the script ran) => pressed {}", t.title, bs[0].2.replace('&', "")));
                        pressed = Some((Instant::now(), t.title.clone()));
                        continue
                    }
                }
                // anything else is left as it is; named once it has stayed up for 2 s (a progress window comes and goes)
                if !reported.contains(&t.h) && super::is_box(&t.class, t.owner != 0) && seen.get(&t.h).is_some_and(|s| s.elapsed() > Duration::from_secs(2)) {
                    reported.push(t.h);
                    let buttons: Vec<String> = cs.iter().filter(|c| is_button(&c.1) && !c.2.trim().is_empty()).map(|c| c.2.replace('&', "")).collect();
                    ran.dialogs.push(format!("left for the person: \"{}\"{}", t.title, if buttons.is_empty() { String::new() } else { format!(" [{}]", buttons.join(", ")) }));
                }
            }
            if let Some((at, text)) = &stopped_at {
                if at.elapsed() > Duration::from_millis(2500) && !done.exists() {
                    ran.ok = false;
                    // no start mark: the script never compiled, so nothing of it ran
                    let what = if runm.exists() { format!("Altium stopped the script: {text}. {}", super::STOPPED) } else { format!("The script did not compile, so nothing of it ran: {text}") };
                    ran.error = super::locate(&what, &std::fs::read_to_string(&pas).unwrap_or_default());
                    break
                }
            }
            // a box pressed, and then Altium idle with no word from the script: Altium ended it (an error inside Altium, such as an
            // access violation, ends a script outright, its finally included)
            if let Some((at, title)) = &pressed {
                if at.elapsed() > Duration::from_secs(3) && !done.exists() && idle(pid, &[]) {
                    ran.ok = false;
                    ran.error = format!("Altium ended the script at its \"{title}\" box (an error inside Altium; its text cannot be read): nothing after that point ran, and the script's own clean-up did not run. {}", super::STOPPED);
                    break
                }
            }
            if Instant::now() > end {
                ran.ok = false;
                let left: Vec<String> = ran.dialogs.iter().filter(|d| d.starts_with("left for the person")).cloned().collect();
                let why = if left.is_empty() { String::new() } else { format!(" {}: {}.", super::LEFT, left.join("; ")) };
                if !runm.exists() {
                    // never started: if Altium starts it later, it does nothing
                    let _ = std::fs::write(&cancel, "cancelled");
                    ran.error = format!("Altium did not start the script within {timeout} s (it may be busy, starting up, or showing a window of its own); nothing was done.{why}");
                } else {
                    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Pending { done: done.clone(), pid, at: started, windows: new.iter().map(|t| t.h).collect() });
                    ran.error = format!("The script started but had not finished after {timeout} s: Altium is still running it or waiting on a window.{why} Backplane sends no other script until it ends (the next call says when); narrow the work, or check its results first.");
                }
                break
            }
        }
        ran.output = super::shown_output(&read(&out)?);
        for e in super::expectations(&read(&exp)?).into_iter().skip(announced) { match e { Err(m) => results.push((false, m)), Ok(e) => pending.push(e) } }
        for e in pending { results.push((e.optional, format!("{}: did not open", e.title))) }
        let failed = super::expect_failures(&results);
        ran.driven = results.into_iter().map(|x| x.1).collect();
        if ran.ok && !failed.is_empty() { ran.ok = false; ran.error = format!("windows the script expected were not worked as it said: {}", failed.join("; ")) }
        ran.secs = started.elapsed().as_secs_f32();
        give_back(pid, front);
        let mut pre = vec![];
        if started_it { pre.push("(Altium Designer was not running: Backplane started it; open the project's documents by path before editing.)".to_string()) }
        if let Some(n) = note { pre.push(n) }
        if !pre.is_empty() { ran.output = format!("{}\n{}", pre.join("\n"), ran.output).trim_end().to_string() }
        Ok(ran)
    }
    /// Tests: close Altium's visible boxes titled `title` (a box a live test raised on purpose)
    #[cfg(test)]
    pub fn close_titled(title: &str) -> usize {
        let Ok(Some(a)) = altium() else { return 0 };
        let v: Vec<isize> = tops(&[a.pid]).into_iter().filter(|t| t.title == title && t.owner != 0).map(|t| t.h).collect();
        for h in &v { unsafe { PostMessageW(*h as HWND, WM_CLOSE, 0, 0); } }
        v.len()
    }
    fn rand_u16() -> u16 { (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0) >> 8) as u16 }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// The write tool's scripts split as the bridge splits them (globals, routines, body); BP_ALTIUM_WRITE_SCRIPTS=<dir> writes them
    /// there with paths from BP_ALTIUM_WRITE_DOCS (`;`-separated, the first the focused one) to try through the bridge.
    #[test] fn write_scripts() {
        let docs = vec![r"C:\p\A.SchDoc".to_string(), r"C:\p\it's.PcbLib".to_string()];
        for sc in [close_script(&docs), reopen_script(&docs, r"C:\p\B.PcbDoc"), add_docs_script(r"C:\p\P.PrjPcb", &docs)] {
            let p = split(&sc);
            assert!(p.decls.starts_with("var") && p.routines.starts_with("procedure Bp") && !p.body.is_empty(), "{sc}");
            assert!(sc.contains("'C:\\p\\it''s.PcbLib'"), "{sc}");
        }
        assert!(reopen_script(&docs, "").contains("'SCH'") && reopen_script(&docs, "").contains("'PCBLIB'"));
        assert_eq!(Open::of("focused|C:\\x.PcbDoc\nmodified|C:\\a.SchDoc\nclosed|C:\\b.SchLib\nnoise"), Open { focused: "C:\\x.PcbDoc".into(), closed: vec!["C:\\b.SchLib".into()], modified: vec!["C:\\a.SchDoc".into()] });
        if let (Some(dir), Ok(list)) = (std::env::var_os("BP_ALTIUM_WRITE_SCRIPTS"), std::env::var("BP_ALTIUM_WRITE_DOCS")) {
            let d: Vec<String> = list.split(';').map(String::from).collect();
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(dir.join("close.pas"), close_script(&d)).unwrap();
            std::fs::write(dir.join("reopen.pas"), reopen_script(&d, &d[0])).unwrap();
            let prj = d.iter().find(|x| x.to_lowercase().ends_with(".prjpcb")).cloned().unwrap_or_default();
            std::fs::write(dir.join("add.pas"), add_docs_script(&prj, &d.iter().filter(|x| !x.to_lowercase().ends_with(".prjpcb")).cloned().collect::<Vec<_>>())).unwrap();
        }
    }
    #[test] fn splits_var_routines_and_body() {
        let s = "var\n  Board : IPCB_Board;\n  I, N : Integer;\nconst K = 3;\nfunction Twice(X : Integer) : Integer;\nvar Y : Integer;\nbegin\n  Y := X * 2;\n  if Y > 0 then begin Result := Y; end else Result := 0;\nend;\nprocedure Hi;\nbegin\n  try Emit('a;b end'); except end;\nend;\nBoard := PCBServer.GetCurrentPCBBoard;\nEmit(IntToStr(Twice(K)));\n";
        let p = split(s);
        assert_eq!(p.decls, "var\n  Board : IPCB_Board;\n  I, N : Integer;\nconst K = 3;");
        assert!(p.routines.starts_with("function Twice") && p.routines.contains("procedure Hi;") && p.routines.trim_end().ends_with("end;"), "{}", p.routines);
        assert_eq!(p.body, "Board := PCBServer.GetCurrentPCBBoard;\nEmit(IntToStr(Twice(K)));");
    }
    #[test] fn a_main_block_loses_its_wrapper() {
        assert_eq!(split("var X : Integer;\nbegin\n  X := 1;\n  Emit('x');\nend.").body, "X := 1;\n  Emit('x');");
        assert_eq!(split("begin X := 1; end; Emit('more');").body, "begin X := 1; end; Emit('more');", "two statements stay");
        assert_eq!(split("Emit('just this');").body, "Emit('just this');");
    }
    #[test] fn loops_are_guarded() {
        let g = guard_loops("while I < 10 do I := I + 1;\nrepeat\n  I := I - 1;\nuntil I = 0;\nwhile (A or B) and Ok(X) do begin end;\nEmit('while x do');");
        assert!(g.contains("while (I < 10) and BpTick(1) do"), "{g}");
        assert!(g.contains("until (I = 0) or not BpTick(4);"), "{g}");
        assert!(g.contains("while ((A or B) and Ok(X)) and BpTick(5) do"), "{g}");
        assert!(g.contains("Emit('while x do')"), "strings untouched: {g}");
    }
    #[test] fn undo_groups_are_tracked() {
        let t = track_undo("PCBServer.PreProcess;\nSchServer.ProcessControl.PreProcess(Doc, '');\nschserver.processcontrol.postprocess(Doc, '');\nPCBServer.PostProcess;\nMyPCBServer.PreProcess;");
        assert_eq!(t, "BpPcbPre;\nBpSchPre(Doc, '');\nBpSchPost(Doc, '');\nBpPcbPost;\nMyPCBServer.PreProcess;");
    }
    /// Against a real Altium (Windows, BP_ALTIUM_LIVE=1, run in the desktop session): status, an error, a runaway loop, a compile error.
    #[test] fn live_altium() {
        if std::env::var("BP_ALTIUM_LIVE").as_deref() != Ok("1") { return }
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let st = run(STATUS_SCRIPT, 30).await.expect("altium");
            println!("STATUS ok={} {:.1}s\n{}", st.ok, st.secs, st.output);
            assert!(st.ok && st.output.starts_with("version "));
            let e = run("var X : IPCB_Board;\nX := nil;\nEmit('before');\nEmit(X.FileName);\nEmit('after');", 30).await.unwrap();
            println!("NIL ok={} err={:?} out={:?} dialogs={:?}", e.ok, e.error, e.output, e.dialogs);
            assert!(!e.ok && e.output.contains("before") && !e.output.contains("after"));
            let l = run("var I : Integer;\nI := 0;\nwhile True do I := I + 1;\nEmit('out ' + IntToStr(I));", 8).await.unwrap();
            println!("LOOP ok={} {:.1}s out={:?} err={:?}", l.ok, l.secs, l.output, l.error);
            assert!(l.output.contains("BP-LOOP-GUARD") && l.output.contains("out "));
            let c = run("Emit(NoSuchThing(1));", 20).await.unwrap();
            println!("COMPILE ok={} err={:?} dialogs={:?}", c.ok, c.error, c.dialogs);
            assert!(!c.ok);
            let again = run("Emit('still answering');", 20).await.unwrap();
            println!("AGAIN ok={} out={:?}", again.ok, again.output);
            assert!(again.ok && again.output == "still answering");
        });
    }
    /// A box no one expected (BP_ALTIUM_LIVE=2, Windows): left for the person, the run times out as still running, the next run is
    /// refused until the box is gone, then goes through and says what became of the earlier one.
    #[cfg(windows)]
    #[test] fn live_left_box() {
        if std::env::var("BP_ALTIUM_LIVE").as_deref() != Ok("2") { return }
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let r = run("ShowMessage('a box no one expected');\nEmit('after');", 8).await.unwrap();
            println!("LEFT ok={} err={:?} dialogs={:?}", r.ok, r.error, r.dialogs);
            assert!(!r.ok && r.dialogs.iter().any(|d| d.starts_with("left for the person")) && r.error.contains("had not finished"));
            let again = run("Emit('x');", 10).await;
            println!("REFUSED {:?}", again.as_ref().err());
            assert!(again.as_ref().err().is_some_and(|e| e.contains("earlier script")));
            assert_eq!(win::close_titled("X2"), 1);
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let after = run("Emit('free');", 20).await.unwrap();
            println!("AFTER ok={} out={:?}", after.ok, after.output);
            assert!(after.ok && after.output.contains("free") && after.output.contains("earlier script"));
        });
    }
    /// Integrity levels as the hub reads them (BP_ALTIUM_IL_PID=<pid of a process>, Windows): this process's, that one's, and the verdict
    #[cfg(windows)]
    #[test] fn live_integrity() {
        let Ok(p) = std::env::var("BP_ALTIUM_IL_PID") else { return };
        let pid: u32 = p.parse().unwrap();
        let (own, other) = (win::own_integrity(), win::integrity(pid));
        println!("IL own={own:#x} other={other:x?} verdict={:?}", privilege_problem(own, other));
        let a = win::altium().ok().flatten().map(|a| (a.pid, a.exe));
        println!("ALTIUM {a:?} install {:?}", win::exe());
        if let Some((_, exe)) = a { for w in ["Validate Changes", "Engineering Change Order", "Situs Routing Strategies"] { println!("TR {w}: {:?}", win::translations(&exe, w)) } }
    }
    /// A script file through the bridge (BP_ALTIUM_SCRIPT=<file>, Windows, desktop session): prints the result as JSON. A dev aid.
    #[test] fn live_script_file() {
        let Ok(f) = std::env::var("BP_ALTIUM_SCRIPT") else { return };
        let t: u32 = std::env::var("BP_ALTIUM_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
        let src = std::fs::read_to_string(&f).expect("script");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let r = rt.block_on(run(&src, t));
        println!("BP-RESULT {}", match r { Ok(r) => r.json().to_string(), Err(e) => json!({"error": e}).to_string() });
    }
    #[test] fn errors_quote_their_line() {
        let f = "a\nb\n    Emit('x';\nd";
        assert_eq!(locate(") expected at Line:3, Char:19.", f), ") expected at Line:3, Char:19.\n(that line of the script: `Emit('x';`)");
        assert_eq!(locate("no line here", f), "no line here");
        assert_eq!(locate("at Line:99", f), "at Line:99");
    }
    #[test] fn expected_windows_parse() {
        assert_eq!(parse_steps("click Validate Changes; wait 2;click &Execute Changes ;; gone 30").unwrap(),
            vec![Step::Click("Validate Changes".into()), Step::Wait(2.0), Step::Click("&Execute Changes".into()), Step::Gone(30.0)]);
        assert_eq!(parse_steps("click Add Files; window Open; text 1 = C:\\bp\\a b.csv; click Open").unwrap(),
            vec![Step::Click("Add Files".into()), Step::Window("Open".into()), Step::Text(1, "C:\\bp\\a b.csv".into()), Step::Click("Open".into())]);
        assert!(parse_steps("press OK").unwrap_err().contains("click <caption>"));
        assert!(parse_steps("wait soon").is_err() && parse_steps("text 0 = x").is_err() && parse_steps("text 1 x").is_err() && parse_steps("click").is_err());
        assert_eq!(parse_steps("wait 99999").unwrap(), vec![Step::Wait(600.0)], "a wait is capped");
        let out = "a line\nBP-EXPECT Engineering Change Order => click Validate Changes; click Execute Changes; click Close\nBP-EXPECT  => click x\nBP-EXPECT Situs => jump\nmore";
        let e = expectations(out);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].as_ref().unwrap(), &Expect { title: "Engineering Change Order".into(), steps: vec![Step::Click("Validate Changes".into()), Step::Click("Execute Changes".into()), Step::Click("Close".into())], optional: false });
        assert!(e[1].is_err() && e[2].is_err());
        assert_eq!(shown_output(out), "a line\nmore");
        let m = expectations("BP-EXPECT? Component Links => click Perform Update");
        assert!(m[0].as_ref().unwrap().optional && m[0].as_ref().unwrap().title == "Component Links");
    }
    #[test] fn boxes_include_owned_wpf_windows() {
        assert!(is_box("#32770", false) && is_box("TMessageForm", true));
        assert!(is_box("HwndWrapper[DefaultDomain;;baff34aa]", true), "ActiveRoute's Information box, Unsaved Changes");
        assert!(!is_box("HwndWrapper[DefaultDomain;;baff34aa]", false), "an unowned WPF window is a panel or the app");
        assert!(!is_box("TDocumentForm", true) && !is_box("TApplication", false));
    }
    /// The only box the hub answers on its own: the script engine's (seen on 26.11: #32770 "X2", OK/Cancel, "<error> at Line:5,
    /// Char:25. Continue execution?"). A box of the person's (a save prompt, any other title) is never one.
    #[test] fn only_the_script_engines_box_is_answered() {
        let t = "Undeclared identifier: FileName at Line:5, Char:25.  Continue execution?";
        assert!(is_script_error("#32770", "X2", "X2", t, true));
        assert!(is_script_error("#32770", "x2", "X2", ") or  ] expected at Line:3, Char:18. Continue execution?", true));
        assert!(!is_script_error("#32770", "X2", "X2", t, false), "no Cancel to press");
        assert!(!is_script_error("#32770", "Confirm", "X2", t, true), "another title");
        assert!(!is_script_error("TMessageForm", "X2", "X2", t, true));
        assert!(!is_script_error("#32770", "X2", "X2", "Save changes to board.PcbDoc?", true), "the person's save prompt");
        assert!(is_script_error("#32770", "X2", "X2", "Undeklarierter Bezeichner: FileName in Zeile 5, Zeichen 25. Ausführung fortsetzen?", true), "a localized engine's words");
    }
    #[test] fn altiums_main_window_is_known() {
        assert!(is_main_window("TDocumentForm", "MOP.PrjPcb - Altium Designer Professional (26.11.1)") && is_main_window("X", "board.PcbDoc - Altium Designer Develop (26.12)"));
        assert!(!is_main_window("TMessageForm", "Information") && !is_main_window("TSplash", "") && !is_main_window("X", "Altium Designer splash"));
    }
    #[test] fn titles_and_captions_match_loosely() {
        let no: Vec<String> = vec![];
        assert!(title_matches("engineering change order", "Engineering Change Order", &no));
        assert!(title_matches("Situs Routing Strategies", "Situs Routing Strategies - board.PcbDoc", &no));
        assert!(!title_matches("", "anything", &no) && !title_matches("ECO", "Component Links", &no));
        assert!(caption_matches("Execute Changes", "&Execute Changes", &no) && caption_matches("Route All", "Route All...", &no) && caption_matches("open", "&Open", &no));
        assert!(!caption_matches("Execute", "Execute Changes", &no) && !caption_matches(" ", "", &no));
        assert!(caption_matches("Generate all footprints in*", r"Generate all footprints in C:\p\a.PcbLib", &no) && !caption_matches("*", "x", &no) && !caption_matches("Gen*", "Start", &no));
        // a localized Altium: the caption is one of Altium's translations of the English one
        let de = vec!["Änderungen bestätigen (&V)".to_string(), "Änderungsaufträge (ECO)".to_string()];
        assert!(caption_matches("Validate Changes", "Änderungen bestätigen (&V)", &de) && caption_matches("Validate Changes", "&Änderungen bestätigen", &de));
        assert!(title_matches("Engineering Change Order", "Änderungsaufträge (ECO)", &de) && !caption_matches("Validate Changes", "Abbrechen", &de));
        assert_eq!(norm(" Änderungen ausführen (&E) "), "änderungen ausführen");
        let ja = vec!["ECO".to_string()];
        assert!(title_matches("Engineering Change Order", "ECO", &ja) && !title_matches("Engineering Change Order", "Recorder", &ja), "a short translation is the whole title");
        assert_eq!(norm("&Report Changes..."), "report changes");
    }
    /// Altium's forms.fdb: length-prefixed UTF-16 strings, `Caption.'<English>'` then the translation (the layout of 26.11's files)
    #[test] fn translations_are_read_from_altiums_fdb() {
        let s = |x: &str| { let u: Vec<u16> = x.encode_utf16().collect(); let mut v = vec![u.len() as u8]; v.extend(u.iter().flat_map(|c| c.to_le_bytes())); v };
        let mut b = vec![0x7c, 5, 0, 0];
        for (k, tr) in [("Button_ValidateChanges", None), ("Caption.'Validate Changes'", Some("Änderungen bestätigen")), ("Hint.''", None), ("Caption.'&Execute Changes'", Some("Änderungen ausführen (&E)")), ("Caption.'Close'", Some("Close"))] {
            b.extend(s(k)); if let Some(t) = tr { b.extend(s(t)) } b.extend([2, 0, 0, 0]);
        }
        let mut m = std::collections::HashMap::new();
        fdb_captions(&b, &mut m);
        assert_eq!(m.get("validate changes"), Some(&vec!["Änderungen bestätigen".to_string()]));
        assert_eq!(m.get("execute changes"), Some(&vec!["Änderungen ausführen (&E)".to_string()]));
        assert!(m.get("close").is_none(), "an untranslated caption adds nothing");
        fdb_captions(&b[..b.len() - 9], &mut m); fdb_captions(&[], &mut m); // cut short: no panic
    }
    /// What Altium reads: ASCII, a character past it as a character code inside a string (tested on 26.11: `'a'#220'b'#937'c'#20013`
    /// came back as those six characters, a UTF-8 file with a BOM did not)
    #[test] fn run_files_are_ascii() {
        assert_eq!(ascii_pas("P := 'C:\\bp\\Ünïcode test\\x.PcbDoc';"), "P := 'C:\\bp\\'#220'n'#239'code test\\x.PcbDoc';");
        assert_eq!(ascii_pas("Emit('Ω中'); // Grüße\n{ é } (* ü *) X := 'it''s é';"), "Emit(''#937''#20013''); // Gr??e\n{ ? } (* ? *) X := 'it''s '#233'';");
        assert_eq!(ascii_pas("S := '😀';"), "S := ''#55357#56832'';", "past the BMP: the UTF-16 pair");
        let b = build("Emit('Ünïcode µ');", r"C:\Users\Zoë\out.out", "d", 5);
        assert!(b.is_ascii() && b.contains("Emit(''#220'n'#239'code '#181'');") && b.contains("BpOutPath := 'C:\\Users\\Zo'#235'\\out.out';"), "{b}");
    }
    #[test] fn run_output_is_decoded() {
        assert_eq!(decode_out(br"C:\\bp\\\u00DCn\u00EFcode \u03A9\u4E2D \uD83D\uDE00 \\x"), "C:\\bp\\Ünïcode Ω中 😀 \\x");
        assert_eq!(decode_out(b"4.7\xb5F \x80"), "4.7µF €", "a file Altium wrote in its code page");
        assert_eq!(unescape("\\u12"), "\\u12", "a cut escape stays");
    }
    #[test] fn newest_install_by_version_not_folder() {
        let p = |s: &str| std::path::PathBuf::from(s);
        assert_eq!(newest(vec![("26.11.1.5".into(), p("AD23")), ("25.8.1.18".into(), p("AD25")), ("9.4".into(), p("AD9"))]), Some(p("AD23")));
        assert_eq!(newest(vec![("AD9".into(), p("a")), ("AD24".into(), p("b"))]), Some(p("b")));
        assert_eq!(newest(vec![]), None);
    }
    #[test] fn an_elevated_altium_is_refused_clearly() {
        assert!(privilege_problem(0x2000, Some(0x3000)).unwrap().contains("as administrator"));
        assert!(privilege_problem(0x2000, None).is_some(), "a token that cannot be read is an elevated one");
        assert!(privilege_problem(0x2000, Some(0x2000)).is_none() && privilege_problem(0x3000, Some(0x2000)).is_none() && privilege_problem(0x3000, None).is_none());
    }
    #[test] fn open_scripts_by_kind() {
        assert!(open_script(r"C:\p\a.PcbDoc").contains("Client.OpenDocument('PCB', 'C:\\p\\a.PcbDoc')"));
        assert!(open_script(r"C:\p\a.PrjPcb").contains("DM_OpenProject('C:\\p\\a.PrjPcb', True)"));
        for s in [open_script(r"C:\p\a.SchDoc"), open_script(r"C:\p\P.PrjPcb")] { let p = split(&s); assert!(p.decls.starts_with("var") && p.body.contains("Emit('opened')"), "{s}") }
    }
    #[test] fn a_failed_expectation_fails_the_run() {
        assert!(expect_failures(&[(true, "Component Links: did not open".into()), (true, "ECO: clicked Close".into())]).is_empty());
        assert_eq!(expect_failures(&[(false, "Engineering Change Order: no enabled `Execute Changes`".into())]).len(), 1);
    }
    #[test] fn template_offers_expect() {
        let b = build("BpExpect('Engineering Change Order', 'click Close');\nRunProcess('WorkspaceManager:Compare');", "o", "d", 5);
        assert!(b.contains("procedure BpExpect(Title, Steps : String);") && b.contains("BpExps.Add('BP-EXPECT ' + BpEsc(Title) + ' => ' + BpEsc(Steps));") && b.contains("procedure BpMayExpect("));
        assert!(b.contains("procedure BpFail(Msg : String);") && b.contains("function BpNum(S : String) : Double;") && b.contains("function BpFmt(Fmt : String; X : Double) : String;"));
        assert!(b.contains("if FileExists('o.cancel') then") && b.contains("Done.SaveToFile('o.run');"));
        assert!(build("Emit('x');", r"C:\r\a.out", "d", 5).contains(r"BpExpPath := 'C:\r\a.exp';"));
        assert_eq!(expect_path(r"C:\r\bp1.out"), r"C:\r\bp1.exp");
        let r = Ran { ok: true, driven: vec!["Engineering Change Order: clicked Close".into()], ..Default::default() };
        assert_eq!(r.json()["driven"][0], "Engineering Change Order: clicked Close");
        assert!(Ran::default().json().get("driven").is_none());
    }
    #[test] fn template_has_parts_and_paths() {
        let b = build("var X : Integer;\nX := 2;\nwhile X > 0 do X := X - 1;\nEmit('done');", r"C:\r\a.out", r"C:\r\a.done", 20);
        assert!(b.contains("var X : Integer;") && build("const K = 2;\nvar Y : Integer;\nY := K;", "o", "d", 5).contains("const K = 2;\n    var Y : Integer;") && b.contains("while (X > 0) and BpTick(3) do") && b.contains("BpOutPath := 'C:\\r\\a.out';") && b.contains("/ 86400"));
        assert!(build("Emit('it''s');", "o", "d", 5).contains("Emit('it''s');"));
    }
}
