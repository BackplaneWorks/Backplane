// dataBadgeSymbol against the hub's icon list: every name the hub may send (native/host/src/present/badge.rs ICONS) has its own SF Symbol, the symbol exists,
// and any other name is the folder. Compiled with Backplane/BadgeSymbols.swift alone on a Mac (AppKit's symbol catalog is the system's): scripts/mobile/ios-badge-symbols-test.sh.
import AppKit
var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name)") } }
let src = try! String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let body = src.components(separatedBy: "pub const ICONS: &[&str] = &[")[1].components(separatedBy: "];")[0]
let names = body.split(separator: "\n").map { $0.components(separatedBy: "//")[0] }.joined(separator: " ").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).trimmingCharacters(in: CharacterSet(charactersIn: "\"")) }.filter { !$0.isEmpty }
check(names.count == 20, "the hub lists 20 icons: \(names.count)")
var seen = Set<String>()
for n in names {
    let s = dataBadgeSymbol(n)
    check(n == "folder" || s != "folder", "\(n) has its own symbol")
    check(NSImage(systemSymbolName: s, accessibilityDescription: nil) != nil, "\(n): SF Symbol \(s) exists")
    seen.insert(s)
}
check(seen.count == names.count, "every icon draws differently")
check(dataBadgeSymbol("skull") == "folder" && dataBadgeSymbol("") == "folder", "an unknown name is the folder")
print("badge symbols: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
