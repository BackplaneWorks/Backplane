// What gets a copy button and Run in terminal, and the bytes Run in terminal types: the cases of native/host/src/md/copy.rs and
// native/gpui/src/screens/term.rs, so the phone and the hub agree. Compiled with Backplane/Copyable.swift and Vt.swift: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func P(_ t: String, _ c: Bool) -> DCopyPiece { DCopyPiece(text: t, cmd: c) }

// copy.rs a_plain_posts_copyable_pieces
let v = Copyable.spans("Run `sudo apt-get install -y rootlesskit slirp4netns` then see `foo()` and:\n```bash\nmake\nmake install\n```\nhash `3552c7cfd`.")
check(v == [P("sudo apt-get install -y rootlesskit slirp4netns", true), P("make\nmake install", true), P("3552c7cfd", false)], "a post's pieces", v)
check(Copyable.spans("no code here").isEmpty, "no code, no pieces")
check(Copyable.spans("an `unclosed span").isEmpty, "an unclosed span")
check(Copyable.spans("```rust\nfn main() {}\n```") == [P("fn main() {}", false)], "a block in another language is copyable, not a command")

// copy.rs fenced_blocks_that_hold_commands
let yes: [(String, String)] = [("sudo apt-get install -y rootlesskit slirp4netns", ""), ("cd native/host\ncargo test --offline", "text"), ("# set up\nexport A=1\n\nscripts/land.sh", ""),
    ("docker run \\\n  --rm \\\n  img", ""), ("anything at all", "bash"), ("Get-ChildItem", "powershell"), ("$ git status", ""),
    ("RUST_LOG=debug ./target/fast/backplane-hub --foreground", ""), ("cd a\r\nmake", "")]
for (t, l) in yes { check(Copyable.blockIsCommand(t, lang: l), "command: \(t) [\(l)]") }
let no: [(String, String)] = [("", "sh"), ("fn main() {}", "rust"), ("git status", "rust"), ("{\"a\": 1}", ""), ("git status\nOn branch main", ""), ("Install the tools first.", ""), ("# only a comment", "")]
for (t, l) in no { check(!Copyable.blockIsCommand(t, lang: l), "not a command: \(t) [\(l)]") }

// copy.rs inline_spans_by_kind
let kinds: [(String, String?)] = [("sudo apt-get install -y rootlesskit slirp4netns", "cmd"), ("git status", "cmd"), ("scripts/land.sh --dry", "cmd"), ("git", nil),
    ("https://example.com/a", "url"), ("~/.backplane/home", "path"), ("/usr/bin/env", "path"), ("native/host/src/md/copy.rs", "path"), ("C:\\Users\\h", "path"),
    ("3552c7cfd", "hash"), ("8e41de6a2c1f0a9b8e41de6a2c1f0a9b8e41de6a", "hash"), ("t1794855320-614", "id"), ("0.16.1", "id"), ("Copy", nil), ("foo()", nil), ("a b", nil),
    ("deadbeef", nil), ("", nil)]
for (t, k) in kinds { let got = Copyable.inlineKind(t); check(got == k, "kind of \(t)", got as Any) }

// the exact text: no trailing newline, the rest whole
check(Copyable.exact("make\nmake install\n") == "make\nmake install", "exact drops the trailing newline", Copyable.exact("make\nmake install\n"))
check(Copyable.exact("a\r\n\r\n") == "a", "exact drops CR LF")
check(Copyable.exact("  x  ") == "  x  ", "exact keeps the rest")

// a reply's Markdown: the hub's list decides when it sent one, else the rules
let hub = [P("git status", true), P("make", false)]
check(Copyable.inlinePieces("see `git status` and `foo()` and `/etc/hosts`", hub: nil) == [P("git status", true), P("/etc/hosts", false)], "inline pieces by the rules")
check(Copyable.inlinePieces("see `git status` and `/etc/hosts`", hub: hub) == [P("git status", true)], "inline pieces by the hub's list")
check(Copyable.blockCmd("make", lang: "", hub: hub) == false && Copyable.blockCmd("make", lang: "", hub: nil) == true, "a block's cmd: the hub's word first")
check(Copyable.blockCmd("git status\n", lang: "", hub: hub) == true && Copyable.blockCmd("cargo build", lang: "bash", hub: []) == false, "a block not in the hub's list: no Run")
check(Copyable.list(nil, hubHasCopy: true) == [] && Copyable.list(nil, hubHasCopy: false) == nil && Copyable.list(hub, hubHasCopy: true) == hub, "no key from a hub with copy: nothing to copy")
check(Copyable.pieces(hub: nil, text: "`git log -1`") == [P("git log -1", true)] && Copyable.pieces(hub: [], text: "`git log -1`").isEmpty, "pieces: the hub's, else the rules")

// term.rs run_in_terminal_types_the_command_and_never_enter
let plain = Vt(cols: 80, rows: 24)
check(TermPrefill.bytes("sudo apt-get install -y x\n", vt: plain) == Array("sudo apt-get install -y x".utf8), "one line, no Enter")
let joined = TermPrefill.bytes("cd a \\\n  && make\n# done\nmake install", vt: plain)
check(joined == Array("cd a    && make && make install".utf8), "no bracketed paste: one line", String(decoding: joined, as: UTF8.self))
let b = Vt(cols: 80, rows: 24); b.feed("\u{1b}[?2004h$ ")
let got = TermPrefill.bytes("make\nmake install", vt: b)
check(got == Array("\u{1b}[200~make\rmake install\u{1b}[201~".utf8), "bracketed paste keeps the lines", String(decoding: got, as: UTF8.self))
check(got.last != 13 && joined.last != 13, "never an Enter")
check(!TermPrefill.bytes("echo \u{1b}x", vt: plain).contains(0x1b), "no escape typed")

// term.rs run_in_terminal_waits_for_the_prompt_and_not_into_a_full_screen_program
let w = Vt(cols: 80, rows: 24)
check(TermPrefill.ready(w, waited: 0) == false, "a blank screen: wait")
w.feed("$ "); check(TermPrefill.ready(w, waited: 0) == false && TermPrefill.ready(w, waited: TermPrefill.wait) == true, "a prompt: after the wait")
check(TermPrefill.ready(Vt(cols: 80, rows: 24), waited: TermPrefill.wait * 4) == true, "nothing printed: type after 4 waits")
w.feed("\u{1b}[?2004h"); check(TermPrefill.ready(w, waited: 0) == true, "bracketed paste on: at once")
w.feed("\u{1b}[?1049h"); check(TermPrefill.ready(w, waited: TermPrefill.wait) == nil, "a full-screen program: never")

// term.rs the_terminal_belongs_to_the_chat_on_screen
check(ChatTerm.key(thread: "t1") == "t1" && ChatTerm.key(room: "r1") == "room:r1" && ChatTerm.key(room: "lk~r1") == "" && ChatTerm.key(room: "") == "", "the chat's terminal key")

// the wire: {text, cmd}
let d = try! JSONDecoder().decode([DCopyPiece].self, from: Data(#"[{"text":"git status","cmd":true},{"text":"/etc/hosts"}]"#.utf8))
check(d == [P("git status", true), P("/etc/hosts", false)], "decodes the hub's pieces", d)

print("copyable: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
