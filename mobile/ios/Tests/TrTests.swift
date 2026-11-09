// Tr (the catalog's ICU subset and the English-to-key match) against hand-made messages and the real catalogs. Compiled with Shared/Tr.swift and
// Shared/TrCatalog.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func r(_ m: String, _ a: [String: String] = [:], _ tag: String = "en") -> String { Tr.render(TrParse.message(m), a, tag) }

// the ICU subset
check(r("Plain") == "Plain", "plain", r("Plain"))
check(r("\\sa\\nb\\\\c") == " a\nb\\c", "escapes", r("\\sa\\nb\\\\c"))
check(r("Delete {title}?", ["title": "x"]) == "Delete x?", "argument", "")
check(r("{title}", [:]) == "{title}", "a missing argument shows its name", r("{title}"))
check(r("{n, number} lines", ["n": "12345"]) == "12,345 lines", "number", r("{n, number} lines", ["n": "12345"]))
let files = "{n, plural, =0 {No files changed} one {# file changed} other {# files changed}}"
check(r(files, ["n": "0"]) == "No files changed", "plural =0", r(files, ["n": "0"]))
check(r(files, ["n": "1"]) == "1 file changed", "plural one", r(files, ["n": "1"]))
check(r(files, ["n": "1200"]) == "1,200 files changed", "plural other", r(files, ["n": "1200"]))
check(r("{n, plural, one {# 件} other {# 件s}}", ["n": "1"], "ja") == "1 件s", "Japanese has no one", r("{n, plural, one {# 件} other {# 件s}}", ["n": "1"], "ja"))
let who = "{who, select, you {You} agent {Agent} other {Someone}}"
check(r(who, ["who": "agent"]) == "Agent" && r(who, ["who": "zz"]) == "Someone", "select", "")
check(r("{a, select, x {{n, plural, one {# x} other {# xs}}} other {none}}", ["a": "x", "n": "2"]) == "2 xs", "nested", "")
check(r("Create ''{name}'' here", ["name": "p"]) == "Create 'p' here", "'' is an apostrophe", r("Create ''{name}'' here", ["name": "p"]))
check(r("don't '{x}' '#'", [:]) == "don't {x} #", "quoting", r("don't '{x}' '#'"))
check(r("# outside", [:]) == "# outside", "# outside a plural", "")
check(Tr.number("-1234567") == "-1,234,567" && Tr.number("abc") == "abc", "grouping", Tr.number("-1234567"))

// the language and its setting
check(Tr.resolve("auto", preferred: ["ja-JP", "en-US"]) == "ja", "auto, Japanese first", "")
check(Tr.resolve("auto", preferred: ["en-US", "ja-JP"]) == "en", "auto, English first", "")
check(Tr.resolve("auto", preferred: []) == "en", "auto, nothing", "")
check(Tr.resolve("ja", preferred: ["en"]) == "ja" && Tr.resolve("en", preferred: ["ja"]) == "en", "chosen", "")

// the real catalogs: every message renders whole in both languages
var bad: [String] = []
for (key, e) in Tr.table.byKey {
    var args: [String: String] = [:]
    for n in TrParse.variants(e.en, fixed: [:], hashArg: nil).flatMap(\.parts) { if case .cap(let a, _) = n { args[a] = "2" } }
    for (tag, m) in [("en", e.en), ("ja", e.ja)] {
        let out = Tr.render(m, args.merging(["plural": "other"]) { a, _ in a }, tag)
        if out.contains("{") && !out.contains("{}") && !key.hasPrefix("i18n.") { bad.append("\(tag) \(key): \(out)") }
    }
}
check(bad.isEmpty, "every message renders without a hole", bad.prefix(5))
check(Tr.table.byKey.count > 1000, "the catalog loaded", Tr.table.byKey.count)

// English to Japanese
Tr.lang = "en"
check(Tr.t("Cancel") == "Cancel", "English is unchanged", Tr.t("Cancel"))
check(Tr.k("common.cancel") == "Cancel", "by key, English", Tr.k("common.cancel"))
Tr.lang = "ja"
check(Tr.k("common.cancel") == "キャンセル", "by key, Japanese", Tr.k("common.cancel"))
check(Tr.k("no.such.key") == "no.such.key", "an unknown key", "")
check(Tr.t("Cancel") == "キャンセル", "exact", Tr.t("Cancel"))
check(Tr.t("3 files changed") == "変更ファイル3件", "plural template", Tr.t("3 files changed"))
check(Tr.t("1 file changed") == "変更ファイル1件", "plural one", Tr.t("1 file changed"))
check(Tr.t("No files changed") == "変更なし", "plural =0", Tr.t("No files changed"))
check(Tr.t("1,234 files changed") == "変更ファイル1,234件", "a grouped number", Tr.t("1,234 files changed"))
check(Tr.t("Agent") != "Agent", "select branch", Tr.t("Agent"))
check(Tr.t("12 tool calls").hasPrefix("ツール呼び出し12"), "a select beside a count", Tr.t("12 tool calls"))
check(Tr.t("Delete board.kicad_pcb?") == Tr.thread_delete_ask(title: "board.kicad_pcb"), "a text hole", Tr.t("Delete board.kicad_pcb?"))
check(Tr.t("Create project 'demo' here") == "ここにプロジェクト「demo」を作成", "apostrophes in a template", Tr.t("Create project 'demo' here"))
check(Tr.t(" · 1–20 of 300") == "・300件中1–20件", "leading space", Tr.t(" · 1–20 of 300"))
// (the pieces' own Japanese is the catalog's: more than one key says "{n} tool calls")
let joined = Tr.t("3 tool calls · Read, Edit · 2 design changes")
check(joined.hasPrefix("ツール呼び出し3") && joined.contains(" · Read, Edit · ") && joined.hasSuffix("設計変更2件"), "pieces joined by ·", joined)
check(Tr.t("my file · other words") == "my file · other words", "pieces none of which is the catalog's", "")
check(Tr.t("Unable to decode model — previous saved revision shown") == Tr.ios_plot_previous(why: Tr.k("ios.plot.undecodable")), "a hole that is a catalog message is translated too", Tr.t("Unable to decode model — previous saved revision shown"))
check(Tr.thread_count(n: 4) == "スレッド4件", "typed function", Tr.thread_count(n: 4))
for s in ["board.kicad_pcb", "I changed the footprint of U3.", "", "src/main.rs", "https://example.com", "7", "Fix the bug in parse()"] {
    check(Tr.t(s) == s, "unknown text unchanged: \(s)", Tr.t(s))
}
check(Tr.t("files changed") == "files changed", "a template needs its hole", Tr.t("files changed"))
check(Tr.t("x files changed") == "x files changed", "a number hole takes digits only", Tr.t("x files changed"))
check(Tr.match("3 files changed")?.key == "files.changed" && Tr.match("3 files changed")?.args["n"] == "3", "match", Tr.match("3 files changed") as Any)

// the hub's machine and queue words (feature machines, the queue strip, the find bar)
check(Tr.t("runs on desktop") == "desktopで実行", "a machine's runs line", Tr.t("runs on desktop"))
check(Tr.t("New thread on desktop (away)") == "desktopに新しいスレッド（応答なし）", "the longer template wins", Tr.t("New thread on desktop (away)"))
check(Tr.t("New thread on desktop") == "desktopに新しいスレッド", "new thread on a machine", Tr.t("New thread on desktop"))
check(Tr.t("3 of 12+") == "12+件中3件目", "the find count", Tr.t("3 of 12+"))
check(Tr.t("Queued") == "待機中" && Tr.t("Unsent") == "未送信", "queue tags", Tr.t("Unsent"))
check(Tr.t("fix the board (goes with the next message)") == "fix the board（次のメッセージと一緒に送られます）", "a handoff row", Tr.t("fix the board (goes with the next message)"))
check(Tr.ios_machine_away(label: "desktop") == "desktop · 応答なし", "away", Tr.ios_machine_away(label: "desktop"))

// the cache follows the language
check(Tr.t("Cancel") == "キャンセル", "cached", "")
Tr.lang = "en"
check(Tr.t("Cancel") == "Cancel" && Tr.t("3 files changed") == "3 files changed", "a language change clears the cache", Tr.t("Cancel"))

print("\(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
