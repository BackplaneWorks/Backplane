// CloneModel (Backplane/CloneModel.swift) against the hub's clone.* shapes (native/host/src/commands/clone/mod.rs `Job::json`, `dest_json`;
// gh.rs `repos_wait`, `signin_json`). Compiled with CloneModel.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func dec<T: Decodable>(_ s: String, _ t: T.Type) -> T { try! JSONDecoder().decode(T.self, from: Data(s.utf8)) }

// clone.repos
let r = dec(#"{"repos":[{"name":"o/kit","value":"o/kit","description":"A board","private":true,"account":""},{"name":"org/x","value":"https://sam@github.com/org/x.git","description":null,"private":false,"account":"sam"}],"more":3,"state":"ok","loading":false,"hint":""}"#, DCloneRepos.self)
check(r.repos.count == 2 && r.repos[0].priv && !r.repos[1].priv && r.repos[1].description == "" && r.more == 3, "repos decoded (a null description is empty)", r.repos)
check(r.shownHint == "" && !r.offersSignIn, "ok: no hint")
let so = dec(#"{"repos":[],"more":0,"state":"signedout","loading":false,"hint":"Sign in to GitHub to see your repositories here."}"#, DCloneRepos.self)
check(so.shownHint.hasPrefix("Sign in") && so.offersSignIn, "signedout: hint and Sign in")
let ng = dec(#"{"repos":[],"state":"nogh","hint":"Install the GitHub CLI"}"#, DCloneRepos.self)
check(ng.shownHint == "Install the GitHub CLI" && !ng.offersSignIn, "nogh: hint, no Sign in")

// what clone.start is given
let other = r.repos[1]
check(Clone.repo(typed: "org/x", picked: other) == "https://sam@github.com/org/x.git", "picked row: its value")
check(Clone.repo(typed: " org/x2 ", picked: other) == "org/x2", "edited after picking: the text")
check(Clone.repo(typed: "file:///srv/kit.git", picked: nil) == "file:///srv/kit.git", "typed: the text")

// clone.dest
let d = dec(#"{"ok":false,"error":"~/Documents/kit already exists and isn't empty. Pick another folder or name.","repo":"o/kit","origin":"github.com/o/kit","name":"kit","dest":"/home/user/Documents/kit","destShown":"~/Documents/kit","parent":"/home/user/Documents","folder":"kit","defaultParent":"/home/user/Documents","defaultParentShown":"~/Documents"}"#, DCloneDest.self)
check(!d.ok && d.isRepo && d.destShown == "~/Documents/kit" && d.folder == "kit" && !Clone.canStart(d, busy: false), "refused: Clone disabled", d)
let bad = dec(#"{"ok":false,"error":""}"#, DCloneDest.self)
check(!bad.isRepo, "not an address: no destination step")
var ok = d; ok.ok = true
check(Clone.canStart(ok, busy: false) && !Clone.canStart(ok, busy: true) && !Clone.canStart(nil, busy: false), "canStart")
check(Clone.destParams(repo: "o/kit", parent: "", name: " ") == ["repo": "o/kit"], "default: no folder")
check(Clone.destParams(repo: "o/kit", parent: "/p", name: "kit-2") == ["repo": "o/kit", "parent": "/p", "name": "kit-2"], "chosen folder")

// a job
let run = dec(#"{"id":"c1-1","repo":"o/kit","origin":"github.com/o/kit","dest":"/h/kit","destShown":"~/kit","name":"kit","state":"running","phase":"Receiving objects","pct":42,"detail":"1.2 MiB | 300 KiB/s","error":"","auth":false,"signin":false,"notes":[],"project":"","thread":"","started":1700000000000,"ended":0}"#, DCloneJob.self)
check(run.running && run.fraction == 0.42 && run.phase == "Receiving objects", "running job", run)
let ind = dec(#"{"id":"c1-1","state":"running","phase":"Starting","pct":null}"#, DCloneJob.self)
check(ind.fraction == nil, "pct null: indeterminate")
let fail = dec(#"{"id":"c1-1","state":"failed","error":"GitHub refused access.","auth":true,"signin":true}"#, DCloneJob.self)
check(fail.over && fail.signin, "failed with sign-in")

// the app's own job ids: i<ms>-<random>, [A-Za-z0-9_-], at most 64, known before clone.start goes
let id = Clone.newId(ms: 1700000000123, random: "ab/c d9")
check(id == "i1700000000123-abcd9", "newId: only the allowed characters", id)
check(Clone.newId(ms: 1, random: String(repeating: "x", count: 100)).count == 64, "newId: at most 64")
let ids = Set((0..<200).map { _ in Clone.newId() })
check(ids.count == 200 && ids.allSatisfy { $0.hasPrefix("i") && $0.count <= 64 && $0.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" } }, "newId: fresh each time", ids.count)

// following the job on show, by recency
check(Clone.follow(nil, run, id: "c1-1") == run, "the first push")
check(Clone.follow(run, nil, id: "c1-1") == run, "nothing new: nothing changes")
check(Clone.follow(run, DCloneJob(id: "c9-9"), id: "c1-1") == run, "another job's push is not ours")
check(Clone.follow(fail, run, id: "c1-1") == fail, "an end beats running (a late running answer does not undo it)")
var doneEarly = run; doneEarly.state = "done"; doneEarly.ended = 5
check(Clone.follow(run, doneEarly, id: "c1-1") == doneEarly, "running -> done")
// a fast clone: its done push comes before clone.start's running answer, for the id the app chose and showed first
let mine = DCloneJob(id: "i1-x"); var mineDone = mine; mineDone.state = "done"; mineDone.ended = 9
check(Clone.follow(Clone.follow(mine, mineDone, id: "i1-x"), DCloneJob(id: "i1-x"), id: "i1-x") == mineDone, "done push before the running answer: done stays")
var failOld = fail; failOld.ended = 10; var failNew = fail; failNew.ended = 20; failNew.error = "later"
check(Clone.follow(failNew, failOld, id: "c1-1") == failNew && Clone.follow(failOld, failNew, id: "c1-1") == failNew, "same state: the later ended wins")
var canc = fail; canc.state = "cancelled"; canc.ended = 30
check(Clone.follow(failNew, canc, id: "c1-1") == canc, "two ends: the later ended wins")
let again = DCloneJob(id: "c2-2")
check(Clone.follow(fail, again, id: "c2-2") == again, "Try again: the new id is followed")
check(Clone.follow(run, run, id: "") == run, "no job: nothing changes")

// Try again: clone.start (a new id is added by the caller) for the repository as first given, into the failed job's folder
var gone = fail; gone.dest = "/home/user/Documents/kit-board"; gone.repo = "o/kit"
let rp = Clone.retryParams(gone, repo: "https://sam@github.com/o/kit.git", thread: false, text: "go")
check(rp["repo"] as? String == "https://sam@github.com/o/kit.git" && rp["dest"] as? String == "/home/user/Documents/kit-board" && rp["thread"] as? Bool == false && rp["text"] as? String == "go" && rp["id"] == nil, "retryParams", rp)
check(Clone.retryParams(gone, repo: "", thread: true, text: "")["repo"] as? String == "o/kit" && Clone.retryParams(gone, repo: "", thread: true, text: "")["text"] == nil, "retryParams: the job's repo when none is known")

// after it is done: `hold` says whether the notes are read first (an older hub without it: any note holds)
var done = run; done.state = "done"; done.project = "p1"; done.thread = "t1"
check(Clone.after(done) == .thread("t1"), "done, no notes: its thread at once")
done.notes = ["Found a KiCad project: kit.kicad_pro."]
check(Clone.after(done) == .stay && Clone.after(done, read: true) == .thread("t1"), "no hold field: notes stay until read")
done.hold = false
check(Clone.after(done) == .thread("t1"), "hold false: a note to see, but on at once")
done.hold = true
check(Clone.after(done) == .stay && Clone.after(done, read: true) == .thread("t1"), "hold true: stay until read")
let held = dec(#"{"id":"c1","state":"done","notes":["Found 2 KiCad projects; a.kicad_pro is the one Backplane opens (set in .backplane.json, change it there)."],"hold":true,"thread":"t9"}"#, DCloneJob.self)
check(held.hold == true && Clone.after(held) == .stay, "hold decoded", held)
check(run.hold == nil, "no hold key: nil")
var noThread = done; noThread.thread = ""; noThread.notes = []; noThread.hold = false
check(Clone.after(noThread) == .project("p1"), "no thread: the project")
noThread.project = ""
check(Clone.after(noThread) == .dismiss, "neither: dismiss")
check(Clone.after(run) == .stay && Clone.after(fail) == .stay, "not done: stay")

// sign-in
let s = dec(#"{"state":"waiting","code":"ABCD-1234","url":"https://github.com/login/device","error":""}"#, DCloneSignin.self)
check(s.state == "waiting" && s.code == "ABCD-1234" && s.url.hasSuffix("/login/device"), "signin", s)

print("clone-model: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
