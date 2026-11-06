#!/bin/sh
# The phones' fold labels against test/mobile/fold_labels.json (the table the hub's test holds present/thread.rs to). Swift on the Mac; Android's is a Gradle unit test.
# usage: scripts/mobile/fold-check.sh       (needs the ssh wrapper build/gpui/mac-ssh of the main checkout, or M=<ssh command>)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=bp-fold-check
$M hs-mac-mini "mkdir -p $D"
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/DataModels.swift $ROOT/mobile/ios/Backplane/DataFold.swift $ROOT/test/mobile/fold_labels.json hs-mac-mini:$D/
$M hs-mac-mini "cd $D && cat > main.swift" <<'SW'
import Foundation
struct Row: Decodable { var step: Bool?; var name: String; var text: String }
struct Case: Decodable { var live: Bool; var rows: [Row]; var label: String }
let cases = try JSONDecoder().decode([Case].self, from: Data(contentsOf: URL(fileURLWithPath: "fold_labels.json")))
var bad = 0
for c in cases {
    let run: [DEntry] = c.rows.enumerated().map { i, r in
        var e = DEntry(id: "a\(i)", kind: r.step == true ? "step" : "tool", text: r.text, at: 1)
        if r.step != true { e.tool = DTool(name: r.name, summary: r.text, state: "done") }
        return e
    }
    let got = DataFold.label(run, live: c.live)
    if got != c.label { print("want \(c.label)\n got \(got)"); bad += 1 }
}
let u = DEntry(id: "u", kind: "user", text: "x", at: 1), t = { (i: String) -> DEntry in var e = DEntry(id: i, kind: "tool", text: "ls", at: 1); e.tool = DTool(name: "Bash", summary: "ls", state: "done"); return e }
let rows = DataFold.rows([t("1"), t("2"), u, t("3")], running: true)
if rows.map(\.id) != ["fold:1", "u", "fold:3"] { print("rows", rows.map(\.id)); bad += 1 }
if case .fold(let f) = rows[0], f.live { print("an earlier run is not live"); bad += 1 }
if case .fold(let f) = rows[2], !f.live { print("the last run of a running turn is live"); bad += 1 }
print("\(cases.count) labels, \(bad) wrong"); exit(bad == 0 ? 0 : 1)
SW
$M hs-mac-mini "cd $D && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun swiftc -o fold DataModels.swift DataFold.swift main.swift 2>&1 | grep -v warning | head -20; ./fold"
