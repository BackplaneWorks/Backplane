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
            // the template's globals are already in a var section: a var section's own keyword is dropped, const/type keep theirs
            if is_w(t, "var") { decls.push_str(sec[3..].trim()); } else { decls.push_str(sec); }
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

/// Template.of: the whole run file for `src`, writing its lines to `out` and, when it has ended, `done`; loops end after `guard_s` seconds.
pub fn build(src: &str, out: &str, done: &str, guard_s: u32) -> String {
    // guards first, on the whole script, so a guard names the script's own line
    let p = split(&track_undo(&guard_loops(src)));
    let fix = |s: &str| s.to_string();
    format!(r#"// Backplane run (altium/live.rs): the agent's script wrapped; Emit(S) writes a line of output.
var
    BpLines : TStringList;
    BpOutPath, BpDonePath : String;
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
    end;
end;
"#, decls = indent(&p.decls), routines = fix(&p.routines), body = indent(&fix(&p.body)), outp = pas_str(out), donep = pas_str(done))
}
fn indent(s: &str) -> String { s.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n") }

/// What a run came to.
#[derive(Debug, Default)]
pub struct Ran { pub ok: bool, pub output: String, pub error: String, pub dialogs: Vec<String>, pub secs: f32 }
impl Ran {
    pub fn json(&self) -> Value { json!({"ok": self.ok, "output": self.output, "error": self.error, "dialogs": self.dialogs, "seconds": (self.secs * 10.0).round() / 10.0}) }
}

/// Altium.run: run `script` in the running Altium and wait up to `timeout` seconds. Err: Altium could not be asked at all.
pub async fn run(script: &str, timeout: u32) -> Result<Ran, String> {
    #[cfg(windows)] { win::run(script, timeout).await }
    #[cfg(not(windows))] { let _ = (script, timeout); Err(NOT_HERE.into()) }
}
pub const NOT_HERE: &str = "Altium Designer runs on Windows: this hub is not on Windows, so it cannot reach Altium. The viewers still draw the project's .SchDoc and .PcbDoc files.";

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
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE, WM_COMMAND};

    fn wide(v: &[u16]) -> String { String::from_utf16_lossy(&v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())]) }

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

    struct Dlg { hwnd: HWND, class: String, text: String }
    unsafe extern "system" fn child_text(h: HWND, l: LPARAM) -> BOOL {
        let acc = &mut *(l as *mut Vec<String>);
        let mut b = [0u16; 1024]; let n = GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
        if n > 0 { acc.push(String::from_utf16_lossy(&b[..n as usize])) }
        1
    }
    unsafe extern "system" fn top(h: HWND, l: LPARAM) -> BOOL {
        let (pid, acc) = &mut *(l as *mut (u32, Vec<Dlg>));
        let mut p = 0u32; GetWindowThreadProcessId(h, &mut p);
        if p != *pid || IsWindowVisible(h) == 0 { return 1 }
        let mut c = [0u16; 256]; let n = GetClassNameW(h, c.as_mut_ptr(), c.len() as i32);
        let class = String::from_utf16_lossy(&c[..n.max(0) as usize]);
        if !matches!(class.as_str(), "#32770" | "TMessageForm") { return 1 }
        let mut t = [0u16; 512]; let tn = GetWindowTextW(h, t.as_mut_ptr(), t.len() as i32);
        let mut parts = vec![String::from_utf16_lossy(&t[..tn.max(0) as usize])];
        EnumChildWindows(h, Some(child_text), &mut parts as *mut Vec<String> as LPARAM);
        acc.push(Dlg { hwnd: h, class, text: parts.into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(" | ") });
        1
    }
    /// Altium's visible dialog boxes (message boxes and its own message forms)
    fn dialogs(pid: u32) -> Vec<Dlg> {
        let mut st: (u32, Vec<Dlg>) = (pid, vec![]);
        unsafe { EnumWindows(Some(top), &mut st as *mut (u32, Vec<Dlg>) as LPARAM); }
        st.1
    }
    /// A box a failed script raised: answered Cancel (it stops the script), a message form closed.
    fn answer(d: &Dlg) {
        unsafe {
            if d.class == "#32770" { PostMessageW(d.hwnd, WM_COMMAND, 2 as WPARAM, 0); } // IDCANCEL
            else { PostMessageW(d.hwnd, WM_CLOSE, 0, 0); }
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
        let guard = timeout.saturating_sub(3).max(2);
        std::fs::write(&pas, super::build(script, &out.to_string_lossy(), &done.to_string_lossy(), guard)).map_err(|e| e.to_string())?;
        let before: Vec<isize> = dialogs(pid).iter().map(|x| x.hwnd as isize).collect();
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
            let mut stopped = false;
            for x in dialogs(pid) {
                if before.contains(&(x.hwnd as isize)) { continue }
                ran.dialogs.push(x.text.clone()); answer(&x); stopped = true;
            }
            if stopped {
                // a compile error or an error outside try: the box is answered, the debugger stopped; the run is over
                let _ = crate::proc::tokio_cmd(crate::proc::Kind::Local, &exe).arg("-REditScript:Stop").spawn();
                tokio::time::sleep(Duration::from_millis(400)).await;
                if !done.exists() { ran.ok = false; ran.error = format!("Altium stopped the script: {}", ran.dialogs.join(" / ")); break }
            }
            if Instant::now() > end { ran.ok = false; ran.error = format!("no answer from Altium in {timeout} s (it may be busy, showing a dialog of its own, or still running the script)"); break }
        }
        ran.output = std::fs::read_to_string(&out).unwrap_or_default().trim_end().to_string();
        ran.secs = started.elapsed().as_secs_f32();
        Ok(ran)
    }
    fn rand_u16() -> u16 { (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0) >> 8) as u16 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn splits_var_routines_and_body() {
        let s = "var\n  Board : IPCB_Board;\n  I, N : Integer;\nconst K = 3;\nfunction Twice(X : Integer) : Integer;\nvar Y : Integer;\nbegin\n  Y := X * 2;\n  if Y > 0 then begin Result := Y; end else Result := 0;\nend;\nprocedure Hi;\nbegin\n  try Emit('a;b end'); except end;\nend;\nBoard := PCBServer.GetCurrentPCBBoard;\nEmit(IntToStr(Twice(K)));\n";
        let p = split(s);
        assert_eq!(p.decls, "Board : IPCB_Board;\n  I, N : Integer;\nconst K = 3;");
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
    #[test] fn template_has_parts_and_paths() {
        let b = build("var X : Integer;\nX := 2;\nwhile X > 0 do X := X - 1;\nEmit('done');", r"C:\r\a.out", r"C:\r\a.done", 20);
        assert!(b.contains("X : Integer;") && b.contains("while (X > 0) and BpTick(3) do") && b.contains("BpOutPath := 'C:\\r\\a.out';") && b.contains("/ 86400"));
        assert!(build("Emit('it''s');", "o", "d", 5).contains("Emit('it''s');"));
    }
}
