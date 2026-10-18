import Base
import ./check.bend as C
import ../src/core/kserve.bend as K
import ../src/core/kihealth.bend as KH

# The headless KiCad servers' policy: where a server lives, its command
# line, its ready line, the version check, the restart schedule, which
# server makes room for a third project, and the commands it refuses.

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def some(m: Maybe<&2, String>) -> String:
  match m:
    case Some{s}: "some:" ++ s
    case None{}: "none"

def secs(m: Maybe<&2, Nat>) -> String:
  match m:
    case Some{n}: Nat.show(n)
    case None{}: "none"

def nats(xs: List<&2, Nat>) -> String:
  match xs:
    case Nil{}: ""
    case x <> t: Nat.show(x) ++ "," ++ nats(t)

def nkey(n: Maybe<&2, KH.KNote>) -> String:
  match n:
    case Some{KH.KNote{k, _, _, _}}: k
    case None{}: "none"

def ntitle(n: Maybe<&2, KH.KNote>) -> String:
  match n:
    case Some{KH.KNote{_, t, _, _}}: t
    case None{}: "none"

def nbody(n: Maybe<&2, KH.KNote>) -> String:
  match n:
    case Some{KH.KNote{_, _, b, _}}: b
    case None{}: "none"

def nacts.go(as: List<&2, KH.KAct>) -> String:
  match as:
    case Nil{}: ""
    case KH.KAct{l, a, _} <> t: l ++ "/" ++ a ++ "|" ++ nacts.go(t)

def nacts(n: Maybe<&2, KH.KNote>) -> String:
  match n:
    case Some{KH.KNote{_, _, _, a}}: nacts.go(a)
    case None{}: "none"

def ready(+p: String, +last: Nat) -> K.KSrv:
  K.KS.up(K.KS.new(p, 1n), "/s/" ++ p, "10.99.0-bp.1", last)

def caps_all() -> List<&2, String>:
  ["api-server", "parent-pid", "idle-exit", "shutdown"]

def caps_none() -> List<&2, String>:
  ["api-server", "parent-pid"]

def refused_all(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}: True{}
    case x <> t: Bool.and(K.KS.headless_refused("kiapi.common.commands." ++ x), refused_all(t))

def crashes(n: Nat, +s: K.KSrv, +now: Nat) -> K.KSrv:
  match n:
    case 0n: s
    case 1n+k: crashes(k, K.KS.exited(s, 1, now, "x"), Nat.add(now, 10n))

def main() -> IO(Unit):
  do IO<Unit>:
    +a : K.KSrv = ready("A", 5n)
    +b : K.KSrv = ready("B", 3n)
    +c : K.KSrv = K.KS.new("C", 1n)
    C.Check.str("digest", K.KS.digest("/p/amp.kicad_pro"), "c9df9285f4d7c2a9")
    C.Check.str("digest of another project", K.KS.digest("/p/other.kicad_pro"), "8eba26912d55a575")
    C.Check.str("dir in the runtime dir", K.KS.dir("/run/user/1", "/home/x", "/p/amp.kicad_pro"), "/run/user/1/backplane/kicad/c9df9285f4d7c2a9")
    C.Check.str("dir without a runtime dir", K.KS.dir("", "/home/x", "/p/amp.kicad_pro"), "/home/x/run/kicad/c9df9285f4d7c2a9")
    C.Check.str("argv with idle-exit", String.join(K.KS.argv("kc", "/p/a.kicad_pro", "/d", 42, caps_all()), " "), "kc api-server /p/a.kicad_pro --socket /d/api.sock --parent-pid 42 --idle-exit 600")
    C.Check.str("argv without idle-exit", String.join(K.KS.argv("kc", "/p/a.kicad_pro", "/d", 42, caps_none()), " "), "kc api-server /p/a.kicad_pro --socket /d/api.sock --parent-pid 42")
    C.Check.str("ready line", some(K.KS.ready("KiCad API server listening at /d/api-77.sock")), "some:/d/api-77.sock")
    C.Check.str("ready line with ipc://", some(K.KS.ready("KiCad API server listening at ipc:///d/api.sock\n")), "some:/d/api.sock")
    C.Check.str("another line is not ready", some(K.KS.ready("loading the project")), "none")
    C.Check.str("ready line without a path", some(K.KS.ready("KiCad API server listening at ")), "none")
    C.Check.str("schema only with api-descriptor", K.KS.desc("/r", ["idle-exit", "api-descriptor"]), "/r/share/backplane/api.desc")
    C.Check.str("no schema without it (bp.1)", K.KS.desc("/r", ["idle-exit", "shutdown"]), "")
    C.Check.str("version exact", yes(K.KS.version_ok("10.99.0-bp.1", "10.99.0-bp.1")), "yes")
    C.Check.str("version with a git-describe part", yes(K.KS.version_ok("10.99.0-unknown-bp.1", "10.99.0-bp.1")), "yes")
    C.Check.str("another build", yes(K.KS.version_ok("10.99.0-bp.2", "10.99.0-bp.1")), "no")
    C.Check.str("a longer build number", yes(K.KS.version_ok("10.99.0-bp.11", "10.99.0-bp.1")), "no")
    C.Check.str("not a fork build", yes(K.KS.version_ok("10.99.0", "10.99.0-bp.1")), "no")
    C.Check.str("manifest without a suffix", yes(K.KS.version_ok("10.0.6", "10.0.6")), "yes")
    C.Check.str("first failure", secs(K.KS.retry([100n], 110n)), "1")
    C.Check.str("second failure", secs(K.KS.retry([101n, 100n], 110n)), "5")
    C.Check.str("third failure", secs(K.KS.retry([102n, 101n, 100n], 110n)), "30")
    C.Check.str("fourth failure gives up", secs(K.KS.retry([103n, 102n, 101n, 100n], 110n)), "none")
    C.Check.str("old failures do not count", secs(K.KS.retry([990n, 102n, 101n, 100n], 1000n)), "1")
    C.Check.str("a failure just inside the window counts", secs(K.KS.retry([701n, 702n, 703n, 704n], 1000n)), "none")
    C.Check.str("a failure just outside does not", secs(K.KS.retry([700n, 702n, 703n, 704n], 1000n)), "30")
    C.Check.str("a failure is recorded", nats(K.KS.fails.add(1, [5n], 10n)), "10,5,")
    C.Check.str("a clean exit is not", nats(K.KS.fails.add(0, [5n], 10n)), "5,")
    C.Check.str("recorded failures are those of the window", nats(K.KS.fails.add(1, [5n], 1000n)), "1000,")
    C.Check.str("clean exit stops", K.KS.state(K.KS.exited(a, 0, 20n, "")), "stopped")
    C.Check.str("clean exit forgets nothing and adds nothing", nats(K.KS.fails.of(K.KS.exited(a, 0, 20n, ""))), "")
    C.Check.str("a crash restarts", K.KS.state(K.KS.exited(a, 1, 20n, "boom")), "starting")
    C.Check.str("a crash says why", K.KS.why.of(K.KS.exited(a, 1, 20n, "boom")), "restarting after a crash: boom")
    C.Check.str("three crashes still restart", K.KS.state(crashes(3n, a, 100n)), "starting")
    C.Check.str("the fourth gives up", K.KS.state(crashes(4n, a, 100n)), "failed")
    C.Check.str("failed says why", K.KS.why.of(crashes(4n, a, 100n)), "stopped four times in five minutes: x")
    C.Check.str("clean exit after crashes is stopped", K.KS.state(K.KS.exited(crashes(2n, a, 100n), 0, 150n, "")), "stopped")
    C.Check.str("clean exit after crashes keeps the count", nats(K.KS.fails.of(K.KS.exited(crashes(2n, a, 100n), 0, 150n, ""))), "110,100,")
    C.Check.str("retry forgets the failures", nats(K.KS.fails.of(K.KS.cleared(crashes(4n, a, 100n)))), "")
    C.Check.str("retry leaves it stopped", K.KS.state(K.KS.cleared(crashes(4n, a, 100n))), "stopped")
    C.Check.str("a new record starts", K.KS.state(c), "starting")
    C.Check.str("up is ready", K.KS.state(a), "ready")
    C.Check.str("up keeps the socket", K.KS.sock(a), "/s/A")
    C.Check.str("touch sets last", Nat.show(K.KS.last.of(K.KS.touch(a, 99n))), "99")
    C.Check.str("two ready, count", Nat.show(K.KS.count([a, b])), "2")
    C.Check.str("failed and stopped do not count", Nat.show(K.KS.count([K.KS.set(a, "failed", "x"), K.KS.set(b, "stopped", ""), c])), "1")
    C.Check.str("evict the least recently used", K.KS.evict([a, b]), "B")
    C.Check.str("evict skips a starting one", K.KS.evict([a, c]), "A")
    C.Check.str("nothing to evict with room", K.KS.evict([a]), "")
    C.Check.str("a held server (unsaved changes) is not evicted", K.KS.evict([a, K.KS.hold(b)]), "A")
    C.Check.str("both held: nothing to evict", K.KS.evict([K.KS.hold(a), K.KS.hold(b)]), "")
    C.Check.str("asked for again: no longer held", K.KS.evict([a, K.KS.touch(K.KS.hold(b), 1n)]), "B")
    C.Check.str("idle-exit dropped below API 2", String.join(K.KS.caps.safe(caps_all(), 1n), ","), "api-server,parent-pid,shutdown")
    C.Check.str("idle-exit kept from API 2", String.join(K.KS.caps.safe(caps_all(), 2n), ","), "api-server,parent-pid,idle-exit,shutdown")
    C.Check.str("nothing ready to evict", K.KS.evict([c, K.KS.new("D", 1n)]), "")
    C.Check.str("plan: first project starts", K.KS.plan([], "C", caps_all()), "start")
    C.Check.str("plan: a running server is used", K.KS.plan([a, b], "A", caps_all()), "ready")
    C.Check.str("plan: a starting one is waited for", K.KS.plan([a, c], "C", caps_all()), "wait")
    C.Check.str("plan: a third project retires the oldest", K.KS.plan([a, b], "C", caps_all()), "evict:B")
    C.Check.str("plan: without Shutdown it is busy", K.KS.plan([a, b], "C", caps_none()), "busy")
    C.Check.str("plan: nothing ready to retire is busy", K.KS.plan([c, K.KS.new("D", 1n)], "E", caps_all()), "busy")
    C.Check.str("plan: a failed server stays failed", K.KS.plan([K.KS.set(a, "failed", "x")], "A", caps_all()), "failed")
    C.Check.str("plan: a failed one makes room", K.KS.plan([a, K.KS.set(b, "failed", "x")], "C", caps_all()), "start")
    C.Check.str("plan: a stopped server starts again", K.KS.plan([K.KS.set(a, "stopped", "")], "A", caps_all()), "start")
    C.Check.str("plan: a stopped server needs room too", K.KS.plan([K.KS.set(a, "stopped", ""), b, ready("D", 4n)], "A", caps_all()), "evict:B")
    C.Check.str("eighteen refused commands", Nat.show(List.length(&2, String, K.KS.refused())), "18")
    C.Check.str("every one is refused", yes(refused_all(K.KS.refused())), "yes")
    C.Check.str("a board command is refused", yes(K.KS.headless_refused("kiapi.board.commands.SetVisibleLayers")), "yes")
    C.Check.str("a bare name is refused", yes(K.KS.headless_refused("RunAction")), "yes")
    C.Check.str("GetItems is not refused", yes(K.KS.headless_refused("kiapi.board.commands.GetItems")), "no")
    C.Check.str("BeginCommit is not refused", yes(K.KS.headless_refused("kiapi.common.commands.BeginCommit")), "no")
    C.Check.str("SketchRoute is not refused", yes(K.KS.headless_refused("kiapi.backplane.SketchRoute")), "no")
    C.Check.str("a name that only ends alike is not refused", yes(K.KS.headless_refused("kiapi.board.commands.GetSelectionX")), "no")
    C.Check.str("refusal says what to do", K.KS.refusal(), "not available in headless KiCad: pass explicit item IDs (get them with GetItems)")
    C.Check.str("busy", K.KS.busy(), "IPC busy: 2 KiCad servers already run for other projects")
    C.Check.str("old runtime", K.KS.why.health("old", "10.0.6", ""), "Backplane KiCad is too old (10.0.6, needs " ++ KH.KH.min_version() ++ ")")
    C.Check.str("stock runtime", K.KS.why.health("stock", "10.0.6", ""), "this is stock KiCad 10.0.6, not Backplane KiCad")
    C.Check.str("ok runtime has no reason", K.KS.why.health("ok", "10.99.0-bp.1", ""), "")
    C.Check.str("the setting off", K.KS.gate("ok", "10.99.0-bp.1", "", "off"), "IPC is turned off in Settings")
    C.Check.str("the setting on, runtime ok", K.KS.gate("ok", "10.99.0-bp.1", "", "on"), "")
    C.Check.str("the setting on, runtime old", K.KS.gate("old", "10.0.6", "", "on"), "Backplane KiCad is too old (10.0.6, needs " ++ KH.KH.min_version() ++ ")")
    C.Check.str("fallback names the cli", K.KS.unavailable("it is off", "/k/bin/kicad-cli"), "KiCad IPC unavailable: it is off. Fallback: edit the file directly or use `/k/bin/kicad-cli pcb …`; re-read the file after.")
    C.Check.str("fallback without a cli", yes(String.contains(K.KS.unavailable("x", ""), "use `kicad-cli pcb")), "yes")
    C.Check.str("activity line", K.KS.activity("x"), "KiCad: fell back to files: x")
    C.Check.str("info lines", K.KS.lines([a, K.KS.set(b, "failed", "a|b\nc")]), "A|ready|\nB|failed|a b c")
    C.Check.str("info lines have no pids", yes(String.contains(K.KS.lines([a]), "0")), "no")
    C.Check.str("project name", K.KS.name("/p/amp.kicad_pro"), "amp")
    C.Check.str("note ready", K.KS.note("/p/amp.kicad_pro|ready|"), "KiCad server: ready for amp")
    C.Check.str("note failed", K.KS.note("/p/amp.kicad_pro|failed|no socket"), "KiCad server: failed for amp: no socket")
    C.Check.str("note nothing", K.KS.note(""), "")
    C.Check.str("failed project", K.KS.failed.project("/p/a.kicad_pro|ready|\n/p/b.kicad_pro|failed|boom"), "/p/b.kicad_pro")
    C.Check.str("failed why", K.KS.failed.why("/p/a.kicad_pro|ready|\n/p/b.kicad_pro|failed|boom"), "boom")
    C.Check.str("no failed project", K.KS.failed.project("/p/a.kicad_pro|ready|"), "")
    C.Check.str("ipc notice key", nkey(KH.KH.ipc.notice("/p/b.kicad_pro", "b", "boom")), "ipc-failed:/p/b.kicad_pro")
    C.Check.str("ipc notice title", ntitle(KH.KH.ipc.notice("/p/b.kicad_pro", "b", "boom")), "Headless KiCad stopped for b")
    C.Check.str("ipc notice body", nbody(KH.KH.ipc.notice("/p/b.kicad_pro", "b", "boom")), "boom")
    C.Check.str("ipc notice acts", nacts(KH.KH.ipc.notice("/p/b.kicad_pro", "b", "boom")), "Retry/kicad-retry|Settings/kicad-path|")
    C.Check.str("no failed project, no notice", nkey(KH.KH.ipc.notice("", "", "")), "none")
