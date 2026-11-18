//! Altium Designer, live: a DelphiScript run inside the person's running Altium (X2.EXE), as KiCad's IPC is for KiCad.
//!
//! Altium has no out-of-process API. A running Altium takes a command line, `X2.EXE -RScriptingSystem:RunScriptFile(FileName=..|ProcName=Run)`:
//! a second X2.EXE hands it to the running one and exits, and the script runs on Altium's UI thread with its whole API (workspace,
//! schematic and PCB servers, DRC, output jobs). The hub writes each run as a file of its own (a script project, or a file name used
//! before, leaves Altium compiling the copy its script editor holds, a broken one after a failure), starts it, and reads what it wrote.
//!
//! The script an agent sends is wrapped in a template (`build`): its `var`/`const` block becomes global, its procedures and functions
//! follow the helpers, its statements become `BpBody`, run inside try/except so an error is a result and not Altium's "Continue
//! execution?" box (which blocks every later run until answered; the hub answers it and stops the debugger anyway). `Emit(S)` adds a
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

/// Error.where: an error naming a line of the run file ("at Line:54, Char:19") gets that line quoted, so the agent sees which of its
/// own statements it was (the run file wraps the script, so its line numbers are not the script's).
pub fn locate(err: &str, run_file: &str) -> String {
    let Some(i) = err.find("Line:") else { return err.to_string() };
    let n: usize = err[i + 5..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
    match run_file.lines().nth(n.wrapping_sub(1)) { Some(l) if n > 0 && !l.trim().is_empty() => format!("{err}\n(that line of the script: `{}`)", l.trim()), _ => err.to_string() }
}
/// Expect.file: where a run's BpExpect lines go (beside its output: `x.out` -> `x.exp`)
pub fn expect_path(out: &str) -> String { format!("{}.exp", out.strip_suffix(".out").unwrap_or(out)) }
/// Template.of: the whole run file for `src`, writing its lines to `out` and, when it has ended, `done`; loops end after `guard_s` seconds.
pub fn build(src: &str, out: &str, done: &str, guard_s: u32) -> String {
    // guards first, on the whole script, so a guard names the script's own line
    let p = split(&track_undo(&guard_loops(src)));
    let fix = |s: &str| s.to_string();
    format!(r#"// Backplane run (altium/live.rs): the agent's script wrapped; Emit(S) writes a line of output.
var
    BpLines : TStringList;
    BpOutPath, BpDonePath, BpExpPath : String;
    BpExps : TStringList;
    BpUntil : Double;
    BpTripped : Boolean;
    BpSchDocs : TList;
    BpPcbOpen : Boolean;
{decls}

procedure Emit(S : String);
begin
    BpLines.Add(S);
    BpLines.SaveToFile(BpOutPath);
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
// is up, so it never holds the output file Emit rewrites)
procedure BpExpect(Title, Steps : String);
begin
    BpExps.Add('BP-EXPECT ' + Title + ' => ' + Steps);
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
    BpExps := TStringList.Create;
    BpLines := TStringList.Create;
    BpSchDocs := TList.Create;
    BpPcbOpen := False;
    BpTripped := False;
    BpUntil := Now + {guard_s} / 86400;
    Done := TStringList.Create;
    try
        try
            BpBody;
            Done.Add('ok');
        except
            Done.Add('error');
            Done.Add(ExceptionMessage);
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
"#, decls = indent(&p.decls), routines = fix(&p.routines), body = indent(&fix(&p.body)), outp = pas_str(out), donep = pas_str(done), expp = pas_str(&expect_path(out)))
}
fn indent(s: &str) -> String { s.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n") }

/// One thing to do in a window a script expects (`BpExpect`).
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// click the button with this caption (`&` and a trailing `...` ignored, any case), waiting up to 30 s for it to be there and enabled
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
/// A window the script said it will open and how to work it, instead of the hub cancelling it as an unexpected box.
#[derive(Clone, Debug, PartialEq)]
pub struct Expect { pub title: String, pub steps: Vec<Step> }

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
/// The `BP-EXPECT <title> => <steps>` lines a script's BpExpect wrote, in order; a line that does not parse is an Err naming it.
pub fn expectations(out: &str) -> Vec<Result<Expect, String>> {
    out.lines().filter_map(|l| l.strip_prefix("BP-EXPECT ")).map(|l| {
        let (t, s) = l.split_once("=>").ok_or_else(|| format!("BpExpect `{l}`: no steps"))?;
        let title = t.trim().to_string();
        if title.is_empty() { return Err(format!("BpExpect `{l}`: no window title")) }
        Ok(Expect { title, steps: parse_steps(s)? })
    }).collect()
}
/// A window title the script named: part of the window's title, any case.
pub fn title_matches(want: &str, title: &str) -> bool { !want.is_empty() && title.to_lowercase().contains(&want.to_lowercase()) }
/// A button caption the script named: the same words, `&` accelerators, a trailing `...` and case aside; a trailing `*` matches
/// the caption's start (`Generate all footprints in*`, whose caption ends in a path).
pub fn caption_matches(want: &str, caption: &str) -> bool {
    let n = |s: &str| s.replace('&', "").trim().trim_end_matches("...").trim_end_matches('…').trim().to_lowercase();
    match want.trim().strip_suffix('*') {
        Some(p) => !p.trim().is_empty() && n(caption).starts_with(&n(p)),
        None => !want.trim().is_empty() && n(want) == n(caption),
    }
}

/// Box.is: a window of Altium's the hub answers when it comes up unasked: Windows' message boxes, Delphi's message forms, and
/// an owned WPF window (its extensions' boxes, ActiveRoute's "Information" one: no controls to read, closed as Alt+F4 would)
pub fn is_box(class: &str, owned: bool) -> bool { matches!(class, "#32770" | "TMessageForm") || owned && class.starts_with("HwndWrapper[") }

/// What a run the hub had to stop says after the boxes: a document the script was editing may have closed with it.
pub const STOPPED: &str = "The script did not finish: before anything else, check with a short script that every document you were editing is still open (Client.GetDocumentByPath) and save it (save_document.pas). If one is gone, redo its change yourself by script (pcb_update_from_sch.pas for the ECO); do not ask the person to redo it.";
/// Box.answer: the button the hub presses on a box Altium raised unasked (index into `buttons`, captions as read; None:
/// close the window, as Alt+F4). Work is never thrown away. A button that discards (Don't Save, Save None, Discard,
/// Reload, Revert) is never pressed. On a box that asks about saving or losing changes, the hub presses Cancel, or
/// else the answer that keeps the document as it is in Altium: No to discard, reload or revert, Yes to save. Otherwise
/// it presses Cancel (which stops a failed script), No, OK or Close.
/// Altium's own message forms draw their text as a label that cannot be read, so `text` is often only the title.
pub fn pick_answer(text: &str, buttons: &[String]) -> Option<usize> {
    let n = |s: &str| s.replace('&', "").trim().to_lowercase();
    let t = text.to_lowercase();
    let throws = |c: &str| ["don't save", "dont save", "do not save", "save none", "discard", "reload", "revert", "close without saving"].iter().any(|w| c.contains(w));
    let find = |w: &str| buttons.iter().position(|b| n(b) == w);
    let has = |ws: &[&str]| ws.iter().any(|w| t.contains(w));
    let order: &[&str] = if has(&["discard", "lose", "lost", "reload", "revert", "overwrite", "without saving"]) { &["cancel", "no"] }
        else if has(&["save", "modified", "unsaved", "changes"]) { &["cancel", "yes", "save"] }
        else { &["cancel", "no", "ok", "close"] };
    order.iter().find_map(|w| find(w)).or_else(|| buttons.iter().position(|b| !throws(&n(b)) && !matches!(n(b).as_str(), "yes" | "no")))
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
pub fn shown_output(out: &str) -> String { out.lines().filter(|l| !l.starts_with("BP-EXPECT ")).collect::<Vec<_>>().join("\n").trim_end().to_string() }

/// Altium.run: run `script` in the running Altium and wait up to `timeout` seconds. Err: Altium could not be asked at all.
pub async fn run(script: &str, timeout: u32) -> Result<Ran, String> {
    #[cfg(windows)] { win::run(script, timeout).await }
    #[cfg(not(windows))] { let _ = (script, timeout); Err(NOT_HERE.into()) }
}
pub const NOT_HERE: &str = "Altium Designer runs on Windows: this hub is not on Windows, so it cannot reach Altium. The viewers still draw the project's .SchDoc and .PcbDoc files.";

/// Altium.open: a document or project opened in the person's Altium (the running one takes it; else Altium starts with it).
pub fn open_file(path: &str) -> Result<String, String> {
    #[cfg(windows)] { win::open_file(path) }
    #[cfg(not(windows))] { let _ = path; Err(NOT_HERE.into()) }
}
/// Before `altium_write` replaces files: what the running Altium has open of them. A document open with unsaved edits stops the
/// write (Err names it: overwriting it raises Altium's modal "has been modified... Reload?" box, and the person's edits are theirs);
/// the others open in Altium are closed (they hold nothing unsaved) and reopened by `after_write`. Off Windows, or with Altium
/// not running, nothing is open.
pub async fn before_write(paths: &[std::path::PathBuf]) -> Result<Open, String> {
    #[cfg(windows)] {
        let docs: Vec<String> = paths.iter().filter(|p| doc_kind(p).is_some()).map(|p| p.to_string_lossy().into_owned()).collect();
        if win::altium().is_none() || docs.is_empty() { return Ok(Open::default()) }
        let r = run(&close_script(&docs), 60).await?;
        if !r.ok { return Err(format!("could not check what Altium has open: {} {}", r.error, r.output)) }
        let open = Open::of(&r.output);
        if !open.modified.is_empty() { return Err(format!("open in Altium with unsaved changes: {}. Save or close it there first (save_document.pas / close_document.pas, or ask the person); nothing was written.", open.modified.join(", "))) }
        return Ok(open)
    }
    #[cfg(not(windows))] { let _ = paths; Ok(Open::default()) }
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
pub async fn after_write(mut v: Value, open: &Open, added: &[(String, Vec<String>)]) -> Value {
    #[cfg(windows)] {
        if win::altium().is_none() || (open.closed.is_empty() && added.is_empty()) { return v }
        let mut notes = vec![];
        if !open.closed.is_empty() {
            match run(&reopen_script(&open.closed, &open.focused), 60).await {
                Ok(r) => notes.extend(r.output.lines().map(|l| l.replace('|', ": ")).chain((!r.ok).then(|| format!("reopening failed: {}", r.error)))),
                Err(e) => notes.push(format!("reopening failed: {e}")),
            }
        }
        for (prj, docs) in added {
            match run(&add_docs_script(prj, docs), 60).await {
                Ok(r) => notes.extend(r.output.lines().map(|l| l.replace('|', ": ")).chain((!r.ok).then(|| format!("adding to the open project failed: {}", r.error)))),
                Err(e) => notes.push(format!("adding to the open project failed: {e}")),
            }
        }
        v["altium"] = json!(notes);
    }
    #[cfg(not(windows))] { let _ = (open, added); }
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
    use super::Ran;
    use std::time::{Duration, Instant};
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows, GetClassNameW, GetClientRect, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostMessageW, SendMessageTimeoutW, SendMessageW, SetForegroundWindow, GWL_STYLE, GW_OWNER, SMTO_ABORTIFHUNG, WM_CLOSE, WM_COMMAND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_SETTEXT, WS_DISABLED};

    fn wide(v: &[u16]) -> String { String::from_utf16_lossy(&v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())]) }

    /// Every running X2.EXE's pid (Altium itself, and the short-lived second instance a command line starts, whose own boxes
    /// ("another instance is busy") block it until answered)
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
    /// The running X2.EXE's pid and full path (the first found)
    pub fn altium() -> Option<(u32, String)> {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap as isize == -1 { return None }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = None;
            if Process32FirstW(snap, &mut e) != 0 {
                loop {
                    if wide(&e.szExeFile).eq_ignore_ascii_case("X2.EXE") { found = Some(e.th32ProcessID); break }
                    if Process32NextW(snap, &mut e) == 0 { break }
                }
            }
            CloseHandle(snap);
            let pid = found?;
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() { return Some((pid, String::new())) }
            let mut buf = vec![0u16; 1024]; let mut n = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n) != 0;
            CloseHandle(h);
            Some((pid, if ok { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() }))
        }
    }

    struct Dlg { hwnd: HWND, class: String, text: String, buttons: Vec<(HWND, String)> }
    unsafe extern "system" fn child_text(h: HWND, l: LPARAM) -> BOOL {
        let acc = &mut *(l as *mut Vec<String>);
        let mut b = [0u16; 1024]; let n = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
        if n > 0 { acc.push(String::from_utf16_lossy(&b[..n as usize])) }
        1
    }
    unsafe extern "system" fn child_button(h: HWND, l: LPARAM) -> BOOL {
        let acc = &mut *(l as *mut Vec<(HWND, String)>);
        let mut c = [0u16; 128]; let n = GetClassNameW(h, c.as_mut_ptr(), c.len() as i32);
        let class = String::from_utf16_lossy(&c[..n.max(0) as usize]);
        if class == "Button" || class == "TButton" || class.contains("Btn") {
            let mut b = [0u16; 128]; let tn = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
            acc.push((h, String::from_utf16_lossy(&b[..tn.max(0) as usize]).replace('&', "")));
        }
        1
    }
    unsafe extern "system" fn top(h: HWND, l: LPARAM) -> BOOL {
        let (pids, acc) = &mut *(l as *mut (Vec<u32>, Vec<Dlg>));
        let mut p = 0u32; GetWindowThreadProcessId(h, &mut p);
        if !pids.contains(&p) || IsWindowVisible(h) == 0 { return 1 }
        let mut c = [0u16; 256]; let n = GetClassNameW(h, c.as_mut_ptr(), c.len() as i32);
        let class = String::from_utf16_lossy(&c[..n.max(0) as usize]);
        if !super::is_box(&class, !GetWindow(h, GW_OWNER).is_null()) { return 1 }
        let mut t = [0u16; 512]; let tn = GetWindowTextW(h, t.as_mut_ptr(), t.len() as i32);
        let mut parts = vec![String::from_utf16_lossy(&t[..tn.max(0) as usize])];
        EnumChildWindows(h, Some(child_text), &mut parts as *mut Vec<String> as LPARAM);
        let mut buttons: Vec<(HWND, String)> = vec![];
        EnumChildWindows(h, Some(child_button), &mut buttons as *mut Vec<(HWND, String)> as LPARAM);
        acc.push(Dlg { hwnd: h, class, text: parts.into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(" | "), buttons });
        1
    }
    unsafe extern "system" fn top_any(h: HWND, l: LPARAM) -> BOOL {
        let (pids, acc) = &mut *(l as *mut (Vec<u32>, Vec<(isize, String)>));
        let mut p = 0u32; GetWindowThreadProcessId(h, &mut p);
        if !pids.contains(&p) || IsWindowVisible(h) == 0 { return 1 }
        let mut t = [0u16; 512]; let tn = GetWindowTextW(h, t.as_mut_ptr(), t.len() as i32);
        acc.push((h as isize, String::from_utf16_lossy(&t[..tn.max(0) as usize])));
        1
    }
    /// Every visible top-level window of Altium's processes, with its title
    fn windows(pid: u32) -> Vec<(isize, String)> {
        let mut pids = x2_pids(); if !pids.contains(&pid) { pids.push(pid) }
        let mut st: (Vec<u32>, Vec<(isize, String)>) = (pids, vec![]);
        unsafe { EnumWindows(Some(top_any), &mut st as *mut (Vec<u32>, Vec<(isize, String)>) as LPARAM); }
        st.1
    }
    unsafe extern "system" fn child_any(h: HWND, l: LPARAM) -> BOOL {
        let acc = &mut *(l as *mut Vec<(isize, String, String)>);
        let mut c = [0u16; 128]; let n = GetClassNameW(h, c.as_mut_ptr(), c.len() as i32);
        let mut b = [0u16; 512]; let tn = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
        acc.push((h as isize, String::from_utf16_lossy(&c[..n.max(0) as usize]), String::from_utf16_lossy(&b[..tn.max(0) as usize])));
        1
    }
    /// A window's controls, all levels down: (hwnd, class, text)
    fn controls(w: isize) -> Vec<(isize, String, String)> {
        let mut v: Vec<(isize, String, String)> = vec![];
        unsafe { EnumChildWindows(w as HWND, Some(child_any), &mut v as *mut Vec<(isize, String, String)> as LPARAM); }
        v
    }
    fn usable(h: isize) -> bool { unsafe { IsWindowVisible(h as HWND) != 0 && (GetWindowLongW(h as HWND, GWL_STYLE) as u32 & WS_DISABLED) == 0 } }
    fn alive(h: isize) -> bool { unsafe { IsWindow(h as HWND) != 0 && IsWindowVisible(h as HWND) != 0 } }
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
    /// Drive.window: do an expected window's steps; a line per step done, Err (with what the window offers) at the first that cannot be done
    fn drive(first: isize, e: &super::Expect, pid: u32, before: &[isize]) -> Result<Vec<String>, (Vec<String>, String)> {
        use super::Step;
        let mut w = first;
        let mut log = vec![];
        let until = |secs: f32, f: &mut dyn FnMut() -> bool| { let t = Instant::now(); while t.elapsed().as_secs_f32() < secs { if f() { return true } std::thread::sleep(Duration::from_millis(150)) } f() };
        for st in &e.steps {
            match st {
                Step::Wait(s) => { std::thread::sleep(Duration::from_secs_f32(*s)); log.push(format!("waited {s} s")) }
                Step::Click(c) => {
                    let mut hit = None;
                    if !until(30.0, &mut || { hit = controls(w).into_iter().find(|(h, _, t)| super::caption_matches(c, t) && usable(*h)).map(|x| x.0); hit.is_some() }) {
                        let offers: Vec<String> = controls(w).into_iter().filter(|(h, _, t)| !t.trim().is_empty() && usable(*h)).map(|x| x.2).collect();
                        return Err((log, format!("no enabled `{c}` in the window (it offers: {})", offers.join(", "))))
                    }
                    click(hit.unwrap()); std::thread::sleep(Duration::from_millis(300));
                    log.push(format!("clicked {c}"));
                }
                Step::Text(n, val) => {
                    let edits: Vec<isize> = controls(w).into_iter().filter(|(h, cl, _)| cl.to_ascii_lowercase().contains("edit") && usable(*h)).map(|x| x.0).collect();
                    let Some(h) = edits.get(n - 1) else { return Err((log, format!("the window has {} edit boxes, not {n}", edits.len()))) };
                    let mut wide: Vec<u16> = val.encode_utf16().collect(); wide.push(0);
                    let mut res = 0usize;
                    unsafe { SendMessageTimeoutW(*h as HWND, WM_SETTEXT, 0, wide.as_ptr() as LPARAM, SMTO_ABORTIFHUNG, 5000, &mut res); }
                    log.push(format!("typed into box {n}"));
                }
                Step::Window(t) => {
                    let mut hit = None;
                    if !until(30.0, &mut || { hit = windows(pid).into_iter().find(|(h, ti)| !before.contains(h) && super::title_matches(t, ti)).map(|x| x.0); hit.is_some() }) {
                        return Err((log, format!("no window `{t}` opened")))
                    }
                    w = hit.unwrap(); log.push(format!("in {t}"));
                }
                Step::Gone(s) => {
                    if !until(*s, &mut || !alive(w)) { return Err((log, format!("the window was still open after {s} s"))) }
                    log.push("closed".into());
                }
            }
        }
        Ok(log)
    }
    /// Altium's visible dialog boxes (message boxes and its own message forms)
    fn dialogs(pid: u32) -> Vec<Dlg> {
        let mut pids = x2_pids(); if !pids.contains(&pid) { pids.push(pid) }
        let mut st: (Vec<u32>, Vec<Dlg>) = (pids, vec![]);
        unsafe { EnumWindows(Some(top), &mut st as *mut (Vec<u32>, Vec<Dlg>) as LPARAM); }
        st.1
    }
    /// A box Altium raised unasked, answered as `pick_answer` says (never a button that throws work away), and what was done, for
    /// the result's `dialogs`. A Windows box with no readable button takes IDCANCEL; Altium's own message forms take a click on
    /// the button (BM_CLICK) or, for its drawn buttons, Enter sent to the button; WM_CLOSE when there is no button to press.
    fn answer(d: &Dlg) -> String {
        let caps: Vec<String> = d.buttons.iter().map(|(_, t)| t.clone()).collect();
        unsafe {
            match super::pick_answer(&d.text, &caps) {
                Some(i) => {
                    let b = d.buttons[i].0;
                    SendMessageW(b, 0x00F5, 0, 0); // BM_CLICK
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    if IsWindowVisible(d.hwnd) != 0 { PostMessageW(b, 0x0100, 0x0D, 0); PostMessageW(b, 0x0101, 0x0D, 0); } // Enter down/up
                    format!("pressed {}", caps[i])
                }
                None if d.class == "#32770" && caps.is_empty() => { PostMessageW(d.hwnd, WM_COMMAND, 2 as WPARAM, 0); "cancelled".into() } // IDCANCEL
                None => { PostMessageW(d.hwnd, WM_CLOSE, 0, 0); "closed it".into() }
            }
        }
    }

    /// The window that was in front before a run, in front again (unless it was Altium's own, or the person has moved on to another).
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
    /// Runs.dir: %PUBLIC%\Backplane\altium\runs (no spaces in a person's name: Altium's command-line parameters are not quoted)
    fn dir() -> std::path::PathBuf {
        let base = std::env::var_os("PUBLIC").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\Public"));
        base.join("Backplane").join("altium").join("runs")
    }

    pub async fn run(script: &str, timeout: u32) -> Result<Ran, String> {
        let Some((pid, exe)) = altium() else { return Err("Altium Designer is not running. Open it (and the project) first, then ask again.".into()) };
        if exe.is_empty() { return Err("Altium Designer is running but its program path cannot be read.".into()) }
        let d = dir();
        std::fs::create_dir_all(&d).map_err(|e| format!("cannot make {}: {e}", d.display()))?;
        // old runs: keep the newest 50
        if let Ok(rd) = std::fs::read_dir(&d) {
            let mut v: Vec<_> = rd.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
            v.sort(); let n = v.len(); for (_, p) in v.into_iter().take(n.saturating_sub(150)) { let _ = std::fs::remove_file(p); }
        }
        let id = format!("bp{}{:04x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0), rand_u16());
        let (pas, out, done) = (d.join(format!("{id}.pas")), d.join(format!("{id}.out")), d.join(format!("{id}.done")));
        let exp = std::path::PathBuf::from(super::expect_path(&out.to_string_lossy()));
        let guard = timeout.saturating_sub(3).max(2);
        std::fs::write(&pas, super::build(script, &out.to_string_lossy(), &done.to_string_lossy(), guard)).map_err(|e| e.to_string())?;
        let before: Vec<isize> = dialogs(pid).iter().map(|x| x.hwnd as isize).collect();
        // every window Altium had before the run: an expected window is a new one
        let before_all: Vec<isize> = windows(pid).iter().map(|x| x.0).collect();
        let (mut announced, mut pending): (usize, Vec<super::Expect>) = (0, vec![]);
        // the window in front before the run: Altium comes forward when a script starts; the person's window is given back after
        let front = unsafe { GetForegroundWindow() } as isize;
        let started = Instant::now();
        let arg = format!("-RScriptingSystem:RunScriptFile(FileName={}|ProcName=Run)", pas.to_string_lossy());
        crate::proc::tokio_cmd(crate::proc::Kind::Local, &exe).arg(arg).spawn().map_err(|e| format!("cannot start {exe}: {e}"))?;
        let mut ran = Ran::default();
        let end = started + Duration::from_secs(timeout as u64);
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if done.exists() {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let st = std::fs::read_to_string(&done).unwrap_or_default();
                let mut ls = st.lines();
                ran.ok = ls.next().map(str::trim) == Some("ok");
                if !ran.ok { ran.error = ls.collect::<Vec<_>>().join("\n").trim().to_string(); if ran.error.is_empty() { ran.error = "the script failed".into() } }
                break
            }
            // windows the script said it opens (BpExpect, written before the call that opens them): worked as it said, not cancelled
            let open = windows(pid);
            // read only while a new window is up: the script is then waiting on it, not writing the file
            if open.iter().any(|(h, _)| !before_all.contains(h)) {
                let ex = super::expectations(&std::fs::read_to_string(&exp).unwrap_or_default());
                for e in ex.into_iter().skip(announced) { announced += 1; match e { Ok(e) => pending.push(e), Err(m) => ran.driven.push(m) } }
            }
            let hit = pending.iter().enumerate().find_map(|(i, e)| open.iter().find(|(h, t)| !before_all.contains(h) && super::title_matches(&e.title, t)).map(|(h, _)| (i, *h)));
            if let Some((i, w)) = hit {
                let e = pending.remove(i);
                let (title, ba) = (e.title.clone(), before_all.clone());
                let r = tokio::task::spawn_blocking(move || {
                    let at_start: Vec<isize> = windows(pid).into_iter().map(|x| x.0).filter(|h| *h != w).collect();
                    let r = drive(w, &e, pid, &ba);
                    // steps that could not be done leave the window as the script found it: closed, as Cancel would, so Altium is not left
                    // waiting on a modal window (the windows it opened from there too)
                    if r.is_err() {
                        for (h, _) in windows(pid).into_iter().filter(|(h, _)| !at_start.contains(h)) { unsafe { PostMessageW(h as HWND, WM_CLOSE, 0, 0); } }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                    r
                }).await.unwrap_or_else(|x| Err((vec![], x.to_string())));
                ran.driven.push(match r { Ok(l) => format!("{title}: {}", l.join(", ")), Err((l, m)) => format!("{title}: {}{m}; closed it", l.iter().map(|x| format!("{x}, ")).collect::<String>()) });
                continue
            }
            let mut stopped = false;
            for x in dialogs(pid) {
                if before.contains(&(x.hwnd as isize)) { continue }
                if pending.iter().any(|e| super::title_matches(&e.title, &x.text.split(" | ").next().unwrap_or(""))) { continue }
                let did = answer(&x); ran.dialogs.push(format!("{} => {did}", x.text)); stopped = true;
            }
            if stopped {
                // a compile error or an error outside try: "Continue execution?" is answered, then Altium may raise an Error box after it;
                // every new box is answered until none has come for a second, then its debugger is stopped (that raises a box of its own
                // while a run is still paused: answered too)
                settle(pid, &before, &mut ran).await;
                if !done.exists() {
                    let _ = crate::proc::tokio_cmd(crate::proc::Kind::Local, &exe).arg("-REditScript:Stop").spawn();
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    settle(pid, &before, &mut ran).await;
                }
                if !done.exists() {
                    ran.ok = false;
                    let e = format!("Altium stopped the script: {}. {}", ran.dialogs.iter().filter(|d| !d.is_empty()).cloned().collect::<Vec<_>>().join(" / "), super::STOPPED);
                    ran.error = super::locate(&e, &std::fs::read_to_string(&pas).unwrap_or_default());
                    break
                }
            }
            if Instant::now() > end { ran.ok = false; ran.error = format!("no answer from Altium in {timeout} s (it may be busy, showing a dialog of its own, or still running the script)"); break }
        }
        let o = std::fs::read_to_string(&out).unwrap_or_default();
        ran.output = super::shown_output(&o);
        for e in super::expectations(&std::fs::read_to_string(&exp).unwrap_or_default()).into_iter().skip(announced) {
            match e { Err(m) => ran.driven.push(m), Ok(e) => pending.push(e) }
        }
        for e in pending { ran.driven.push(format!("{}: did not open", e.title)) }
        ran.secs = started.elapsed().as_secs_f32();
        give_back(pid, front);
        Ok(ran)
    }
    /// X2.EXE: the running one's, else the newest in Program Files/Altium/AD*
    fn exe() -> Option<String> {
        if let Some((_, p)) = altium().filter(|(_, p)| !p.is_empty()) { return Some(p) }
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
        let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(std::path::Path::new(&pf).join("Altium")).ok()?.flatten().map(|e| e.path().join("X2.EXE")).filter(|p| p.is_file()).collect();
        v.sort(); v.pop().map(|p| p.to_string_lossy().into_owned())
    }
    pub fn open_file(path: &str) -> Result<String, String> {
        let Some(x2) = exe() else { return Err("Altium Designer is not installed on this machine".into()) };
        let file = path.replace('/', "\\");
        crate::proc::std_cmd(crate::proc::Kind::Local, &x2).arg(&file).spawn().map_err(|e| format!("cannot start Altium: {e}"))?;
        Ok(format!("altium {file}"))
    }
    /// Answer every box Altium raises (not one it had before the run) until none has come for a second (at most 6 s)
    async fn settle(pid: u32, before: &[isize], ran: &mut Ran) {
        let (t0, mut quiet) = (Instant::now(), Instant::now());
        while quiet.elapsed() < Duration::from_millis(1000) && t0.elapsed() < Duration::from_secs(6) {
            tokio::time::sleep(Duration::from_millis(120)).await;
            for x in dialogs(pid) {
                if before.contains(&(x.hwnd as isize)) { continue }
                let line = format!("{} => {}", x.text, answer(&x));
                if !ran.dialogs.contains(&line) { ran.dialogs.push(line) }
                quiet = Instant::now();
            }
        }
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
        assert_eq!(e[0].as_ref().unwrap(), &Expect { title: "Engineering Change Order".into(), steps: vec![Step::Click("Validate Changes".into()), Step::Click("Execute Changes".into()), Step::Click("Close".into())] });
        assert!(e[1].is_err() && e[2].is_err());
        assert_eq!(shown_output(out), "a line\nmore");
    }
    #[test] fn boxes_include_owned_wpf_windows() {
        assert!(is_box("#32770", false) && is_box("TMessageForm", true));
        assert!(is_box("HwndWrapper[DefaultDomain;;baff34aa]", true), "ActiveRoute's Information box");
        assert!(!is_box("HwndWrapper[DefaultDomain;;baff34aa]", false), "an unowned WPF window is a panel or the app");
        assert!(!is_box("TDocumentForm", true) && !is_box("TApplication", false));
    }
    #[test] fn boxes_are_answered_without_throwing_work_away() {
        let b = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let at = |t: &str, v: &[&str]| pick_answer(t, &b(v)).map(|i| v[i].to_string());
        assert_eq!(at("Information", &["OK"]).as_deref(), Some("OK"), "the esp_carrier box: its only button");
        assert_eq!(at("Confirm | Save changes to esp_carrier.PcbDoc?", &["&Yes", "&No", "Cancel"]).as_deref(), Some("Cancel"));
        assert_eq!(at("Save changes to esp_carrier.PcbDoc?", &["Yes", "No"]).as_deref(), Some("Yes"), "no Cancel: save, never No");
        assert_eq!(at("The document has been modified outside. Reload?", &["Yes", "No"]).as_deref(), Some("No"), "reload would drop the edits");
        assert_eq!(at("Discard your changes?", &["Yes", "No"]).as_deref(), Some("No"));
        assert_eq!(at("Confirm Save for 1 document", &["Save All", "Save None"]).as_deref(), Some("Save All"), "never Save None");
        assert_eq!(at("Unsaved", &["Don't Save"]), None, "a discard button alone: closed, not pressed");
        assert_eq!(at("Error | Continue execution?", &["Yes", "No", "Cancel"]).as_deref(), Some("Cancel"), "a failed script is still stopped");
        assert_eq!(at("Information", &[]), None);
    }
    #[test] fn titles_and_captions_match_loosely() {
        assert!(title_matches("engineering change order", "Engineering Change Order"));
        assert!(title_matches("Situs Routing Strategies", "Situs Routing Strategies - board.PcbDoc"));
        assert!(!title_matches("", "anything") && !title_matches("ECO", "Component Links"));
        assert!(caption_matches("Execute Changes", "&Execute Changes") && caption_matches("Route All", "Route All...") && caption_matches("open", "&Open"));
        assert!(!caption_matches("Execute", "Execute Changes") && !caption_matches(" ", ""));
        assert!(caption_matches("Generate all footprints in*", r"Generate all footprints in C:\p\a.PcbLib") && !caption_matches("*", "x") && !caption_matches("Gen*", "Start"));
    }
    #[test] fn template_offers_expect() {
        let b = build("BpExpect('Engineering Change Order', 'click Close');\nRunProcess('WorkspaceManager:Compare');", "o", "d", 5);
        assert!(b.contains("procedure BpExpect(Title, Steps : String);") && b.contains("BpExps.Add('BP-EXPECT ' + Title + ' => ' + Steps);"));
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
