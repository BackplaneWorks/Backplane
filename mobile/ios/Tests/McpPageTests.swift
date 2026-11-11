// McpPage (Backplane/McpPage.swift) against the rows the hub's Settings > MCP sends (native/host/src/mcpx/page.rs). Compiled with McpPage.swift
// alone, outside the app (as scripts/mobile/ios-unit-tests.sh runs the others).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

let json = #"""
[{"kind":"head","label":"MCP servers","note":"3 servers","buttons":[{"label":"Add","action":"mcp-view","value":"add","on":false},{"label":"Import","action":"mcp-view","value":"import","on":false}],"primary":0},
 {"kind":"group","label":"Servers","note":"","buttons":[]},
 {"kind":"card","id":"fetch","label":"fetch","note":"Ready","title":"fetch","sub":"uvx mcp-server-fetch","icon":{"text":"F","bg":"#d97757"},"pill":{"text":"Ready","tone":"ok"},"meta":"stdio · 1 tool","open":true,"badge":"Community",
  "press":{"action":"mcp-view","value":""},"toggle":{"on":true,"action":"mcp-toggle","value":"fetch"},
  "buttons":[{"label":"on","action":"mcp-toggle","value":"fetch","on":true},{"label":"Close","action":"mcp-view","value":"","on":true},{"label":"Docs","action":"open-url","value":"https://x.example","on":false}]},
 {"label":"Authorization","note":"This server needs it.","fields":[{"name":"msec","hint":"Authorization","secret":true,"vault":true,"value":""}],"buttons":[{"label":"Save to vault","action":"secret-save","value":"mcp-gh-authorization\u001fMCP server gh: Authorization\u001f","on":true}],"in":true},
 {"kind":"tool","label":"fetch","note":"Fetches","toggle":{"on":false,"action":"mcp-tool","value":"fetch|fetch"},"buttons":[],"in":true},
 {"kind":"actions","label":"","note":"","buttons":[{"label":"Test","action":"mcp-test","value":"fetch","on":false}],"in":true},
 {"kind":"card","id":"x","label":"x","title":"","toggle":{"on":true,"action":"mcp-skip","value":"0","style":"check"},"buttons":[{"label":"add","action":"mcp-skip","value":"0","on":true}],"compact":true},
 {"kind":"paste","label":"Paste","note":"A snippet","fields":[{"name":"mpaste","hint":"{}","secret":false,"value":"","lines":7}],"buttons":[]},
 {"kind":"wobble","label":"Later kind","note":"","buttons":[]},
 {"kind":"group","label":"Servers","note":"","buttons":[]}]
"""#
let raw = try! JSONSerialization.jsonObject(with: Data(json.utf8))
let rows = McpRow.parse(raw)
check(rows.map(\.kind) == [.head, .group, .card, .plain, .tool, .actions, .card, .paste, .plain, .group], "kinds, an unknown one plain", rows.map(\.kind))
check(rows[0].primary == 0 && rows[0].buttons.count == 2, "head: its primary button")
let card = rows[2]
check(card.cardId == "fetch" && card.iconText == "F" && card.iconBg == "#d97757" && card.pillText == "Ready" && card.pillTone == "ok" && card.open && card.badge == "Community", "card fields")
check(card.toggle?.on == true && card.toggle?.check == false && card.toggle?.press.action == "mcp-toggle" && card.press?.action == "mcp-view" && card.press?.value == "", "card switch and press")
check(card.cardButtons.map(\.label) == ["Docs"], "the switch and the press are not drawn twice", card.cardButtons.map(\.label))
check(rows[6].toggle?.check == true && rows[6].cardButtons.isEmpty && rows[6].shownTitle == "x" && rows[6].compact, "a pick: a tick box, no stand-in button")
check(rows[3].inside && rows[3].fields.first?.vault == true && rows[3].fields.first?.secret == true, "a vault field inside the card")
check(rows[7].fields.first?.lines == 7, "the paste box's lines")
check(rows[1].id != rows[9].id && Set(rows.map(\.id)).count == rows.count, "ids are unique", rows.map(\.id))
check(McpRow.parse(raw).map(\.id) == rows.map(\.id), "ids hold across a refresh")
check(McpRow.hex("#24292f") == 0x24292f && McpRow.hex("24292f") == nil && McpRow.hex("#zzz000") == nil, "tile colours")

let blocks = McpBlock.of(rows)
check(blocks.count == 7 && blocks[2].row.cardId == "fetch" && blocks[2].inner.map(\.kind) == [.plain, .tool, .actions], "rows with in are drawn inside the card above", blocks.map { $0.inner.count })

// the page state
var st = McpState()
st.typed = ["mpaste": "{…}", "mname": "a", "msec": "sk-live-SECRET"]
check(st.sent(vault: ["msec"]) == ["mpaste": "{…}", "mname": "a"], "a vault box never goes to the hub")
st.acted("mcp-mode", "command", ui: McpState.strings(["view": "add", "mode": "command", "n": 3]))
check(st.ui == ["view": "add", "mode": "command"] && st.typed["mpaste"] == "{…}", "the hub's ui is kept, strings only; what is typed stays", st.ui)
st.acted("mcp-view", "add", ui: ["view": "add"])
check(st.typed["mname"] == nil && st.typed["mpaste"] == "{…}", "back to Add: the form boxes are emptied, the paste stays")
st.acted("mcp-add", "", ui: ["view": "open:a"])
check(st.typed.isEmpty && st.ui == ["view": "open:a"], "after Add every box is empty")
st.typed = ["mname": "b"]; st.acted("mcp-toggle", "a", ui: ["view": "open:a"])
check(st.typed == ["mname": "b"], "a switch keeps what is typed")
for a in ["mcp-import", "mcp-gallery", "mcp-save", "mcp-remove", "mcp-dup"] { var s = McpState(); s.typed = ["mgarg": "x"]; s.acted(a, "", ui: [:]); check(s.typed.isEmpty, "\(a) clears the boxes") }

// the vault's Save
check(mcpVaultSave(packed: "mcp-gh-authorization\u{1f}MCP server gh: Authorization\u{1f}", value: " ghp_123 ") == ["name": "mcp-gh-authorization", "purpose": "MCP server gh: Authorization", "thread": "", "value": "ghp_123"], "packed name and purpose")
check(mcpVaultSave(packed: "n\u{1f}p\u{1f}", value: "  ") == nil && mcpVaultSave(packed: "\u{1f}p", value: "v") == nil, "nothing without a name and a value")

print("mcp-page: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
