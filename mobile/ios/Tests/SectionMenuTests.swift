// SectionMenu (Backplane/SectionMenu.swift) against the hub's project row menu (native/host/src/present/sections.rs `project_menu`). Compiled with
// SectionMenu.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh. 11 Nov: the long press had lost Remove, Rename and Archive.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

let json = """
[{"id":"pin","label":"Pin project","action":"project-pin","value":"k1"},{"id":"swarm","label":"New swarm","action":"swarm-new","value":"p1"},
 {"id":"rename","label":"Rename","action":"proj-rename","value":"p1"},{"id":"badge","label":"Badge","action":"look","value":"p1"},
 {"id":"open","label":"Open folder","action":"open-folder","value":"/w/p1"},
 {"id":"move","label":"Move to group","action":"","value":"","sub":[{"id":"group","label":"Nordic","action":"group-move","value":"k1\\u001ffolder:/w/n"},{"id":"new-group","label":"New group…","action":"group-new-start","value":"k1"}]},
 {"id":"archive","label":"Archive","action":"project-archive","value":"k1"},{"id":"remove","label":"Remove project","action":"proj-remove","value":"p1","confirm":true}]
"""
let menu = (try? JSONDecoder().decode([DMenuItem].self, from: Data(json.utf8))) ?? []
check(DMenuItem.phone(menu, swarms: true).map(\.id) == ["pin", "swarm", "rename", "badge", "move", "archive", "remove"], "the hub's menu without Open folder", DMenuItem.phone(menu, swarms: true).map(\.id))
check(DMenuItem.phone(menu, swarms: false).map(\.id) == ["pin", "rename", "badge", "move", "archive", "remove"], "no New swarm with swarms off")
check(menu.last?.confirm == true && menu.last?.value == "p1" && menu.first?.confirm == false, "Remove asks first")
let sub = menu.first { $0.id == "move" }?.sub ?? []
check(sub.map(\.label) == ["Nordic", "New group…"], "Move to group: the other groups, then New group…", sub.map(\.label))
check(sub.first?.value == "k1\u{1f}folder:/w/n", "a group's value is the project key and the group", sub.first?.value ?? "")
check(((try? JSONDecoder().decode([DMenuItem].self, from: Data("[{\"id\":\"pin\"}]".utf8))) ?? []).first?.label == "", "missing fields are empty")

// a chat in no project: its row's menu has Move to project…, a second list of the hub's projects (each a `row-move-project`)
let chatJson = """
[{"id":"pin","label":"Pin","action":"row-pin","value":"c1"},{"id":"settle","label":"Settle","action":"row-settle","value":"c1"},
 {"id":"move","label":"Move to project…","action":"","value":"","sub":[{"id":"project","label":"amp","action":"row-move-project","value":"c1\\u001fp1"}]}]
"""
let chatMenu = DMenuItem.thread(try! JSONDecoder().decode([DMenuItem].self, from: Data(chatJson.utf8)))
check(chatMenu.map(\.id) == ["pin", "settle", "move"], "a chat's menu: pin, settle, move", chatMenu.map(\.id))
check(chatMenu[2].isMove && !chatMenu[0].isMove && chatMenu[2].sub[0].action == DMenuItem.moveProject && chatMenu[2].sub[0].value == "c1\u{1f}p1", "Move to project lists the projects", chatMenu[2].sub)
check(DMenuItem.thread([DMenuItem(id: "move", label: "Move to project…")]).isEmpty, "no project to move to: no item")

print("section-menu: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
