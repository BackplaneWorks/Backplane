// CatPick (Backplane/CatPick.swift): the colour a person types, the rig cache key and the preview's question while a name is typed.
// Compiled with CatPick.swift alone, outside the app (as scripts/mobile/ios-unit-tests.sh runs the others).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

// colours: the hub's own rule (bots/cat.rs parse_color)
check(CatPick.hex("#6FB3F2") == "#6fb3f2", "upper case, with #")
check(CatPick.hex(" 6fb3f2 ") == "#6fb3f2", "no #, spaces around")
for bad in ["", "#", "#6fb3f", "#6fb3f2a", "#6fb3fg", "6f b3f2", "##6fb3f2", "red"] { check(CatPick.hex(bad) == nil, "refused: \(bad)") }

// the cache key: the hub's rig for (look, colour, mood)
check(CatPick.key(look: 5, color: "#6FB3F2", mood: "idle") == CatPick.key(look: 5, color: "6fb3f2", mood: "idle"), "one key for one colour however it is written")
check(CatPick.key(look: 5, color: "", mood: "") == "5||idle", "no colour, no mood: the coat's own, idle")
check(CatPick.key(look: 5, color: "nope", mood: "idle") == CatPick.key(look: 5, color: "", mood: "idle"), "a colour that does not parse is the coat's own")
check(CatPick.key(look: 5, color: "", mood: "idle") != CatPick.key(look: 6, color: "", mood: "idle"), "looks differ")
check(CatPick.key(look: 5, color: "", mood: "idle") != CatPick.key(look: 5, color: "", mood: "sleep"), "moods differ")

// picked or the name's
check(!CatPick.picked(rowLook: 12, nameLook: 12), "the name's look: following the name")
check(CatPick.picked(rowLook: 12, nameLook: 40), "another look: picked")
check(!CatPick.picked(rowLook: 12, nameLook: nil), "not known yet: following the name")

// the preview's question
let byName = CatPick.ask(typed: " Mog ", color: "#6fb3f2", rowLook: 12, picked: false)
check(byName["name"] as? String == "Mog" && byName["look"] == nil && byName["color"] as? String == "#6fb3f2" && byName["mood"] as? String == "idle", "following the name: by the name typed", byName)
let byLook = CatPick.ask(typed: "Mog", color: "", rowLook: 12, picked: true)
check(byLook["look"] as? Int == 12 && byLook["name"] == nil, "picked: by look, whatever the name", byLook)
let empty = CatPick.ask(typed: "  ", color: "", rowLook: 7, picked: false)
check(empty["look"] as? Int == 7 && empty["name"] == nil, "an empty field: the bot's own cat", empty)

print("cat-pick: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
