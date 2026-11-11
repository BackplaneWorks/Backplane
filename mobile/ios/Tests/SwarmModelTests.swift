// SwarmModel (Backplane/SwarmModel.swift) against hand-made rows and the hub's swarm.get shape (native/host/src/bots/swarm.rs `board`). Compiled with
// SwarmModel.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

// the project list: only this project's live swarms, in the hub's order
let rows = [DSwarmRow(id: "r3", name: "c", project: "p", state: "paused"), DSwarmRow(id: "r2", name: "b", project: "p", state: "done"),
            DSwarmRow(id: "r1", name: "a", project: "p", state: "active"), DSwarmRow(id: "r0", name: "z", project: "q", state: "active"), DSwarmRow(id: "r9", name: "s", project: "p", state: "stopped")]
check(Swarm.listed(rows, project: "p").map(\.id) == ["r3", "r1"], "listed: active and paused of the project", Swarm.listed(rows, project: "p").map(\.id))
struct T { var id: String; var swarm: String }
check(Swarm.members([T(id: "a", swarm: ""), T(id: "b", swarm: "r1"), T(id: "c", swarm: "r3")], swarm: \.swarm, id: \.id) == ["b", "c"], "members: threads with a swarm are left out")

// the hub's swarm.get, decoded forgivingly
let json = #"""
{"room":{"id":"r1","name":"debounce","kind":"swarm","project":"p","goal":"pick a debounce","state":"paused","reason":"circular","by":"","budget":{"posts":300},"used":{"posts":4}},
 "members":[{"member":"thread:t1","thread":"t1","name":"opus","color":"#3b82f6","role":"","provider":"claude","model":"opus","state":"working","title":"debounce · opus"},
            {"member":"bot1","thread":"","name":"Mochi","color":"#f97316","role":null,"provider":"","model":"","state":"bot","title":""}],
 "posts":[{"id":"p1","from":"opus","thread":"t1","color":"#3b82f6","text":"Idea: an integrator.","tags":["idea"],"reply":"","mentions":[],"files":[],"guard":false,"held":"","at":1700000000},
          {"id":"p2","from":"gemini","thread":"t2","color":"#f97316","text":"I take it.","tags":["claim"],"files":["src/debounce.rs"],"guard":false,"held":"","at":1700000001},
          {"id":"p3","from":"hub","thread":"","text":"The swarm paused: it went in circles.","tags":[],"guard":true,"held":"","at":1700000002},
          {"id":"p4","from":"codex","thread":"t3","text":"again the hall sensor","tags":[],"held":"circular","at":1700000003},
          {"id":"p5","from":"you","thread":"","text":"Use the integrator.","tags":["synthesis"],"at":1700000004}],
 "claims":[{"post":"p2","thread":"t2","files":["src/debounce.rs"],"state":"waiting"}]}
"""#
let b = try! JSONDecoder().decode(DSwarmBoard.self, from: Data(json.utf8))
check(b.room.state == "paused" && b.room.reason == "circular" && b.room.goal == "pick a debounce", "room", b.room)
check(b.members.count == 2 && b.members[0].thread == "t1" && b.members[1].role == "" && b.members[1].state == "bot", "members (a null role is empty)", b.members)
check(b.posts.map(\.id) == ["p1", "p2", "p3", "p4", "p5"], "posts in order", b.posts.map(\.id))
check(b.posts.map(Swarm.look) == [.plain, .plain, .hub, .held, .synthesis], "looks", b.posts.map(Swarm.look))
check(Swarm.labels(b.posts[4]) == [] && Swarm.labels(b.posts[0]) == ["idea"], "synthesis is drawn by its box, not a label")
check(Swarm.waiting(b.posts[1], b.claims) && Swarm.open(b.posts[1], b.claims) && !Swarm.waiting(b.posts[0], b.claims), "a claim's files wait while its claim does")
check(!Swarm.open(b.posts[1], []), "a claim a done answered is no longer open")
check(Swarm.ms(1700000000) == 1700000000000 && Swarm.ms(1700000000000) == 1700000000000 && Swarm.ms(0) == 0, "seconds to ms")

// re-reading with since: new posts at the end, a post already shown replaced in place
check(Swarm.since(b.posts) == "p5" && Swarm.since([]) == "", "since: the newest post shown")
var held = b.posts[0]; held.held = "rate"
let m = Swarm.merge(b.posts, [held, DSwarmPost(id: "p6", text: "next")])
check(m.map(\.id) == ["p1", "p2", "p3", "p4", "p5", "p6"] && m[0].held == "rate", "merge", m.map(\.id))
check(Swarm.merge([], b.posts).map(\.id) == b.posts.map(\.id), "merge into nothing")

// the state line's controls, as the desktop's room page offers them
check(Swarm.actions("active") == ["pause", "stop"] && Swarm.actions("paused") == ["resume", "stop"] && Swarm.actions("done") == [] && Swarm.actions("stopped") == [], "actions")
check(Swarm.canEditMembers("active", count: 3) == (true, true) && Swarm.canEditMembers("paused", count: 8) == (false, true) && Swarm.canEditMembers("done", count: 3) == (false, false), "members can change while it lives")

// New swarm: providers with a model the hub can run, rows, what is sent
let models = try! JSONDecoder().decode(DModelsList.self, from: Data(#"{"models":{"codex":[],"claude":[{"value":"opus","label":"Opus 5.5","ok":true,"why":""}],"grok":[{"value":"g4","label":"g4","ok":false,"why":"no CLI"}],"antigravity":[{"value":"gemini","ok":true}]}}"#.utf8)).models
check(Swarm.providers(models) == ["antigravity", "claude"], "providers: only runnable ones, by name", Swarm.providers(models))
check(models["antigravity"]?[0].label == "gemini", "a model with no label shows its id")
check(Swarm.draft(models).provider == "antigravity" && Swarm.draft([:]).provider == "", "a new row takes the first provider")
let two = [DSwarmMemberDraft(provider: "claude", model: "opus", role: " critic "), DSwarmMemberDraft(provider: "antigravity")]
check(Swarm.canCreate(topic: "debounce", members: two), "a topic and two members")
check(!Swarm.canCreate(topic: "  ", members: two) && !Swarm.canCreate(topic: "x", members: [two[0]]) && !Swarm.canCreate(topic: "x", members: Array(repeating: two[0], count: 9)), "no topic, one or nine members")
check(!Swarm.canCreate(topic: "x", members: [two[0], DSwarmMemberDraft()]), "every member needs a provider")
let p = Swarm.createParams(project: "p", topic: " debounce ", goal: "", seed: "It bounces.", members: two)
check(p["project"] as? String == "p" && p["topic"] as? String == "debounce" && p["goal"] == nil && p["seed"] as? String == "It bounces.", "create params", p)
check((p["members"] as? [[String: String]]) == [["provider": "claude", "model": "opus", "role": "critic"], ["provider": "antigravity"]], "members: empty model and role left out", p["members"] ?? "")

print("swarm-model: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
