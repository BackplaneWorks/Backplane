import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/deploy.bend as DP
import ../src/core/ship.bend as SH

# deploy_main (core/ship.bend, hub.bend's Ship.*): dedupe, coalescing,
# reporting and restart recovery.

def cs(st: H.Step) -> List<&2, M.Change>:
  H.Queue.changes(st)

def again(+s: M.State, st: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(st), s)

def res.body(r: MC.Res) -> String:
  match r:
    case MC.Res{_, b, _}:
      J.Json.show(b)

def res.step(r: MC.Res) -> H.Step:
  match r:
    case MC.Res{_, _, st}:
      st

def res.state(r: MC.Res) -> String:
  match r:
    case MC.Res{_, b, _}:
      J.Json.get_str(b, "state", "")

# the text of the first message a step posts into th
def said(xs: List<&2, M.Change>, +th: String) -> String:
  match xs:
    case Nil{}:
      ""
    case M.MessagePosted{+t, _, _, +text, _} <> rest:
      Bool.pick(String, String.eq(t, th), text, said(rest, th))
    case _ <> rest:
      said(rest, th)

def look(+head: String, +ins: String) -> String:
  "HEAD " ++ head ++ "\n" ++ ins

def phase(+s: M.State) -> String:
  SH.Ship.phase(H.Ship.get(s))

def round(c: M.Change) -> String:
  J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(c)), M.QueueTaken{"x", 0n})))

def main() -> IO(Unit):
  do IO<Unit>:
    +idle : M.State = H.Ship.demo("")
    # git's answer, read
    C.Check.str("look head", SH.Look.head("HEAD abc\nIN c1\nIN c2\n"), "abc")
    C.Check.str("look within", String.join(SH.Look.within("HEAD abc\nIN c1\nIN c2\n"), ","), "c1,c2")
    C.Check.str("look error", SH.Look.err("ERR git fetch origin main: no network\n"), "git fetch origin main: no network")
    C.Check.str("no output is an error", SH.Look.err(""), "git said nothing")
    # not configured
    C.Check.str("no deploy.repo: not configured", res.state(MC.Mcp.ship.none(True{})), "not configured")
    # 1. idle: one build
    +r1 : MC.Res = MC.Mcp.ship(idle, 10n, "t1", J.Json.obj(Nil{}), look("c1", ""))
    C.Check.str("idle: queued", res.state(r1), "queued")
    C.Check.str("idle: one build starts", Nat.show(H.Ship.demo.builds(res.step(r1))), "1")
    +s1 : M.State = again(idle, res.step(r1))
    C.Check.str("building c1", phase(s1) ++ " " ++ SH.Ship.commit(H.Ship.get(s1)), "building c1")
    C.Check.str("a build holds the restart", Bool.show(H.Ship.busy.s(s1)), "True")
    # 2. the same commit again: skipped, covered by the build, no second build
    +r2 : MC.Res = MC.Mcp.ship(s1, 11n, "t2", J.Json.obj(Nil{}), look("c1", ""))
    C.Check.str("same commit: skipped", res.state(r2), "skipped")
    C.Check.str("skipped names the build", res.body(r2), "{\"state\":\"skipped\",\"head\":\"c1\",\"commit\":\"c1\",\"covered\":\"building\",\"next\":\"origin/main is in the build under way; you will be told when it ends. End your turn.\"}")
    C.Check.str("skipped: no build", Nat.show(H.Ship.demo.builds(res.step(r2))), "0")
    +s2 : M.State = again(s1, res.step(r2))
    C.Check.str("an ancestor of the build is covered too", res.state(MC.Mcp.ship(s2, 12n, "t2", J.Json.obj(Nil{}), look("c0", "IN c1\n"))), "skipped")
    # 3. two newer requests during the build: queued, no build now, one follow-up
    +r3 : MC.Res = MC.Mcp.ship(s2, 13n, "t2", J.Json.obj(Nil{}), look("c2", ""))
    C.Check.str("newer during a build: queued", res.state(r3), "queued")
    C.Check.str("newer during a build: no second build", Nat.show(H.Ship.demo.builds(res.step(r3))), "0")
    +s3 : M.State = again(s2, res.step(r3))
    +r4 : MC.Res = MC.Mcp.ship(s3, 14n, "deploy", J.Json.obj(Nil{}), look("c3", ""))
    +s4 : M.State = again(s3, res.step(r4))
    C.Check.str("follow-up wanted once", Bool.show(SH.Ship.more(H.Ship.get(s4))) ++ " " ++ String.join(SH.Ship.next(H.Ship.get(s4)), ","), "True t2,-")
    # 4. the build ends: t1 and t2 (who asked for c1) told once, the follow-up asked for
    +d1 : H.Step = H.Ship.done(s4, 20n, "c1", "STATUS done 0\nVERSION 0.9.1-dev.1\nSTAGE /h/deploy/c1/pkg\ndev: built 0.9.1-dev.1\n")
    C.Check.str("t1 told", said(cs(d1), "t1"), "[Backplane] deploy_main: origin/main c1 is built and staged (version 0.9.1-dev.1). The hub restarts into it at a safe point (no tool call running, build locks free); every turn it cuts resumes.")
    C.Check.str("t1 told once, t2 once", Nat.show(H.Ship.demo.posts(d1, "t1")) ++ " " ++ Nat.show(H.Ship.demo.posts(d1, "t2")), "1 1")
    C.Check.str("one follow-up read of origin/main", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(d1))), "1")
    +e1 : M.State = again(s4, d1)
    C.Check.str("the restart waits for the staged package", DP.Deploy.stage(DP.Deploy.wait(e1)) ++ " " ++ DP.Deploy.note(DP.Deploy.wait(e1)), "/h/deploy/c1/pkg deploy_main c1")
    C.Check.str("asked for the follow-up, staged c1", phase(e1) ++ " " ++ SH.Ship.staged(H.Ship.get(e1)), "asked c1")
    C.Check.str("a done report for another commit changes nothing", Nat.show(List.length(&2, M.Change, cs(H.Ship.done(e1, 21n, "c1", "STATUS done 0\n")))), "0")
    # 5. the follow-up: newest is c3, one build; its end tells t2 and the person
    +n1 : H.Step = H.Ship.newest(e1, 22n, "next", look("c3", ""))
    C.Check.str("follow-up: one build of the newest", Nat.show(H.Ship.demo.builds(n1)), "1")
    +f1 : M.State = again(e1, n1)
    C.Check.str("follow-up building c3", phase(f1) ++ " " ++ SH.Ship.commit(H.Ship.get(f1)), "building c3")
    +d2 : H.Step = H.Ship.done(f1, 30n, "c3", "STATUS done 2\nVERSION \nSTAGE /h/deploy/c3/pkg\nerror: check failed\n")
    C.Check.str("t2 told the failure", said(cs(d2), "t2"), "[Backplane] deploy_main: the build of origin/main c3 failed:\nerror: check failed")
    C.Check.str("t1 not told again", Nat.show(H.Ship.demo.posts(d2, "t1")), "0")
    C.Check.str("the person alerted", Bool.show(String.starts_with(M.Setting.get(again(f1, d2), "alert.deploy", ""), "note\t")), "True")
    C.Check.str("no follow-up after", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(d2))), "0")
    +g1 : M.State = again(f1, d2)
    C.Check.str("failed c3, c1 still staged", phase(g1) ++ " " ++ SH.Ship.staged(H.Ship.get(g1)), "failed c1")
    C.Check.str("staged c1 covers it", res.state(MC.Mcp.ship(g1, 31n, "t1", J.Json.obj(Nil{}), look("c1", ""))), "skipped")
    C.Check.str("an ancestor of the staged commit: skipped", res.state(MC.Mcp.ship(g1, 31n, "t1", J.Json.obj(Nil{}), look("c0", "IN c1\n"))), "skipped")
    # a cancelled deploy no longer covers
    C.Check.str("a cancelled stage covers nothing", res.state(MC.Mcp.ship(again(g1, H.Deploy.cancel(g1, 32n)), 33n, "t1", J.Json.obj(Nil{}), look("c1", ""))), "queued")
    # 6. restart recovery: a build under way is watched again, not lost
    C.Check.str("at start a build is watched again", Nat.show(H.Ship.demo.builds(H.Ship.recover(f1, 40n))), "1")
    C.Check.str("and nothing is written", Nat.show(List.length(&2, M.Change, cs(H.Ship.recover(f1, 40n)))), "0")
    +lost : H.Step = H.Ship.done(f1, 41n, "c3", "STATUS gone\nVERSION \nSTAGE /h/deploy/c3/pkg\n")
    C.Check.str("gone without an exit status: lost", Bool.show(String.starts_with(said(cs(lost), "t2"), "[Backplane] deploy_main: the build of origin/main c3 failed:\nlost:")), "True")
    C.Check.str("lost is failed", phase(again(f1, lost)), "failed")
    C.Check.str("a follow-up asked for is asked again at start", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(H.Ship.recover(e1, 40n)))), "1")
    # the restart into a staged build: live
    +dn : M.State = again(g1, H.Deploy.go(DP.Deploy.wait(g1), g1, 50n))
    +lv : M.State = again(dn, H.Ship.recover(dn, 51n))
    C.Check.str("after its restart c1 is live", phase(lv) ++ " " ++ SH.Ship.live(H.Ship.get(lv)) ++ " [" ++ SH.Ship.staged(H.Ship.get(lv)) ++ "]", "live c1 []")
    C.Check.str("live covers it", res.state(MC.Mcp.ship(lv, 52n, "t1", J.Json.obj(Nil{}), look("c1", ""))), "skipped")
    # follow: off by default, a look every 5 minutes when on
    C.Check.str("follow off: no look", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(H.Ship.tick(idle, 600n)))), "0")
    +on : M.State = again(idle, H.Step{[M.SettingSet{"deploy.follow", "on", 3n}], Nil{}})
    C.Check.str("follow on: a look when due", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(H.Ship.tick(on, 600n)))), "1")
    C.Check.str("follow on: none between", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(H.Ship.tick(on, 660n)))), "0")
    C.Check.str("follow on: none during a build", Nat.show(H.Ship.demo.nexts(H.Ship.demo.outs(H.Ship.tick(again(s1, H.Step{[M.SettingSet{"deploy.follow", "on", 3n}], Nil{}}), 600n)))), "0")
    C.Check.str("follow: a new commit builds", Nat.show(H.Ship.demo.builds(H.Ship.newest(on, 601n, "follow", look("c9", "")))), "1")
    C.Check.str("follow: a covered one does nothing", Nat.show(List.length(&2, M.Change, cs(H.Ship.newest(lv, 601n, "follow", look("c1", ""))))), "0")
    # the deploy check does not wait for a build (it runs in a scope of its
    # own, holds no build lock, and is re-adopted after the restart)
    C.Check.str("the deploy does not wait for the build", Nat.show(H.Deploy.demo.go(H.Deploy.check(again(s1, H.Deploy.ask(s1, 5n, "", "x", "")), 6n, True{}, Nil{}))), "1")
    # events round trip
    C.Check.str("DeployAsked round trip", round(M.DeployAsked{"t1", "n", "c1", "building", 7n}), "{\"$\":\"DeployAsked\",\"by\":\"t1\",\"note\":\"n\",\"commit\":\"c1\",\"cover\":\"building\",\"at\":7}")
    C.Check.str("DeployBuilt round trip", round(M.DeployBuilt{"c1", "pid", "", "12 b:3", 8n}), "{\"$\":\"DeployBuilt\",\"commit\":\"c1\",\"state\":\"pid\",\"version\":\"\",\"detail\":\"12 b:3\",\"at\":8}")
    C.Check.str("deploy_status has main", J.Json.get_str(J.Json.get_obj(DP.Deploy.status(s1), "main"), "phase", ""), "building")
    C.Check.str("deploy_main is a tool", Nat.show(K.Codec.pos(MC.Mcp.names(), "deploy_main")), "33")
