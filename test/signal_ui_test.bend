import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/signal.bend as S
import ../src/core/subs.bend as SB
import ../src/core/plane.bend as P
import ../src/core/notice.bend as NO
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/mobile/screen.bend as MB
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"project", "Test", "/tmp/stoplight", 1n},
    M.ThreadCreated{"p", "project", "Parent", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"c", "project", "Child", M.Local{}, "", "", "claude", "", 1n},
    M.TaskDelegated{"task", "p", "c", 1n, "claude", "", 2n}]

def ui(cs: List<&2, M.Change>, on: Bool) -> U.Ui:
  U.Ui.online(U.Ui.select.k(U.Ui.fold(List.append(&2, M.Change, log(), cs), U.Ui.init(20n)), "p"), on)

def child() -> List<&2, M.Change>:
  [M.TurnChanged{"p", M.Completed{}, 3n}, M.TurnChanged{"c", M.Running{}, 4n}]

def screen(+u: U.Ui) -> String:
  J.Json.get_str(J.Json.get_obj(MB.Mob.screen(u), "thread"), "phase", "missing")

def working(+u: U.Ui) -> String:
  J.Json.get_str(J.Json.get_obj(MB.Mob.screen(u), "thread"), "working", "missing")

def attrs(n: D.Node) -> List<&2, D.Attr>:
  match n:
    case D.El{_, _, as, _}: as
    case _: Nil{}

def key(n: D.Node) -> String:
  match n:
    case D.El{_, k, _, _}: k
    case _: ""

def sub(+word: String, who: String) -> SB.SubRow:
  SB.SubRow{"w", True{}, "Build", who, word, "details", True{}, True{}, 0n, Nil{}}

def field(j: J.Json, +name: String) -> String:
  J.Json.get_str(j, name, "missing")

def island(+u: U.Ui) -> Nat:
  NO.Island.compact.count(MB.Mob.signal.compact(True{}, U.Ui.threads(U.Ui.state(u)), U.Ui.state(u), U.Ui.infos(u)))

def main() -> IO(Unit):
  +u = U.Side.with_signals(ui(child(), True{}))
  +ix = U.Side.signals.ix(U.Side.ix(U.Ui.state(u)), u)
  +t = U.Ui.thread(U.Ui.state(u), "p")
  +info = U.Ui.infos(u)
  +held = sub("held", "lock")
  +unknown = sub("unknown", "watch")
  +running = sub("watching", "watch")
  do IO<Unit>:
    C.Check.str("sidebar index: a parent of an actual child waits", S.Signal.name(U.Side.signal.ix(ix, t)), "waiting")
    C.Check.str("ordinary work count preserved", Nat.show(U.SIx.work(ix, "p")), Nat.show(U.SIx.work(U.Side.ix(U.Ui.state(u)), "p")))
    C.Check.str("display projection appends no events", Nat.show(U.Ui.seq(u)), Nat.show(U.Ui.seq(ui(child(), True{}))))
    C.Check.str("phone selected parent waiting", screen(ui(child(), True{})), "waiting")
    C.Check.str("phone offline active neutralized", screen(ui(child(), False{})), "inactive")
    C.Check.str("phone settled gray", screen(ui([M.ThreadSettled{"p", True{}, 5n}], True{})), "settled")
    C.Check.str("phone interaction red", screen(ui([M.AskOpened{"q", "c", "input", "", "Choose", "", 5n}], True{})), "blocked")
    C.Check.str("offline preserves explicit interaction", screen(ui([M.AskOpened{"q", "c", "input", "", "Choose", "", 5n}], False{})), "blocked")
    C.Check.str("web live parent blue", V.View.attr(attrs(V.View.live.u.loc(I.En{}, ui(child(), True{}))), "class"), "phase active")
    C.Check.str("web offline parent inactive", V.View.attr(attrs(V.View.live.u.loc(I.En{}, ui(child(), False{}))), "class"), "phase inactive")
    C.Check.str("web stable live row identity", key(V.View.live.u.loc(I.En{}, ui(child(), True{}))), "live:p")
    C.Check.str("incremental text delta keeps original memo", V.View.attr(attrs(V.View.live.u.loc(I.En{}, U.Ui.delta(ui(child(), True{}), "p", "stream"))), "fn"), "live")
    C.Check.str("incremental text delta keeps live identity", key(V.View.live.u.loc(I.En{}, U.Ui.delta(ui(child(), True{}), "p", "stream"))), "live:p")
    C.Check.str("nonempty streaming branch preserved", V.View.attr(attrs(V.View.one(V.View.signal.live.loc(I.En{}, False{}, 2n, "stream", "live:p"))), "fn"), "live")
    C.Check.str("blocked accessible label", V.View.attr(attrs(V.View.dot(I.En{}, "blocked")), "aria-label"), "Blocked: needs interaction")
    C.Check.str("active accessible label", V.View.attr(attrs(V.View.dot(I.En{}, "active")), "aria-label"), "Working: agent or owned work active")
    C.Check.str("finished accessible label", V.View.attr(attrs(V.View.dot(I.En{}, "finished")), "aria-label"), "Finished: ready for you to look at")
    C.Check.str("phone finished green", screen(ui([M.TaskStateSet{"task", M.TaskDone{}, "done", 3n}, M.TurnChanged{"p", M.Completed{}, 3n}], True{})), "finished")
    C.Check.str("phone: finished turn, its subagent queued, waits", screen(ui([M.TurnChanged{"p", M.Completed{}, 3n}], True{})), "waiting")
    # a failed turn says failed and why on the phone too, not "Blocked: needs interaction"
    C.Check.str("phone failed says why", working(U.Side.with_signals(ui([M.MessagePosted{"p", "u2", M.User{}, "go", 2n}, M.ActivityLogged{"p", "e3", "error", "codex", "codex exited (exit 127)", 3n}, M.TurnChanged{"p", M.Failed{}, 3n}], True{}))),
      "Failed: codex exited (exit 127)")
    C.Check.str("phone failed still red", screen(ui([M.TurnChanged{"p", M.Failed{}, 3n}], True{})), "blocked")
    C.Check.str("phone stopped gray", screen(ui([M.TaskStateSet{"task", M.TaskCancelled{}, "", 3n}, M.TurnChanged{"p", M.Interrupted{}, 3n}], True{})), "inactive")
    C.Check.str("plane finished row", P.Plane.signal.state(5n), "finished")
    C.Check.str("held lock gray", SB.Subs.signal.of(info, held), "inactive")
    C.Check.str("reporting gray", SB.Subs.signal.of(info, sub("reporting", "subagent")), "inactive")
    C.Check.str("unknown watch gray", SB.Subs.signal.of(info, unknown), "inactive")
    C.Check.str("actual watched work blue", SB.Subs.signal.of(info, running), "active")
    C.Check.str("sub active count excludes reporting/lock/unknown", Nat.show(SB.Subs.active([held, unknown, running, sub("reporting", "subagent")], info)), "1")
    C.Check.str("phone held lock release authority unchanged", field(MB.Mob.sub.signals.loc(I.En{}, held, info), "cancel"), SB.Subs.cancel(held))
    C.Check.str("phone unknown watch cancel authority unchanged", field(MB.Mob.sub.signals.loc(I.En{}, unknown, info), "cancel"), SB.Subs.cancel(unknown))
    C.Check.str("phone actual watch cancel authority unchanged", field(MB.Mob.sub.signals.loc(I.En{}, running, info), "cancel"), SB.Subs.cancel(running))
    C.Check.str("phone sub actual signal field", field(MB.Mob.sub.signals.loc(I.En{}, held, info), "state"), "inactive")
    C.Check.str("phone sub raw explanatory state retained", field(MB.Mob.sub.signals.loc(I.En{}, held, info), "stateLabel"), U.StateWord.label(I.En{}, "held"))
    C.Check.str("plane launched/delegated parent waits on active child", P.Plane.signal(ix, U.Ui.state(u), t), "waiting")
    C.Check.str("new plane places actual parent in run", String.join(P.Plane.ids(P.Plane.run(P.Plane.signals.of(ui(child(), True{}), 20n, True{}))), ","), "p")
    C.Check.str("new plane excludes queued child from run", String.join(P.Plane.ids(P.Plane.run(P.Plane.signals.of(ui(Nil{}, True{}), 20n, True{}))), ","), "")
    C.Check.str("new plane counts actual parent only", Nat.show(P.PTally.run(P.Plane.tally(u, ix, U.Side.rows.ix(U.Ui.state(u), ix, 20n)))), "1")
    C.Check.str("new plane explicit interaction in need", String.join(P.Plane.ids(P.Plane.need(P.Plane.signals.of(ui([M.AskOpened{"q", "c", "input", "", "Choose", "", 5n}], True{}), 20n, True{}))), ","), "p")
    C.Check.str("new plane offline excludes current work claim", String.join(P.Plane.ids(P.Plane.run(P.Plane.signals.of(ui(child(), False{}), 20n, True{}))), ","), "")
    C.Check.str("LiveActivity includes yielded parent actual child", Nat.show(island(u)), "1")
    C.Check.str("LiveActivity excludes queued/inactive work", Nat.show(island(U.Side.with_signals(ui(Nil{}, True{})))), "0")
    C.Check.str("LiveActivity excludes own question-blocked process", Nat.show(island(U.Side.with_signals(ui([M.TurnChanged{"p", M.Running{}, 3n}, M.AskOpened{"q", "p", "input", "", "Choose", "", 5n}], True{})))), "0")
    C.Check.str("LiveActivity counts independent work alongside question", Nat.show(island(U.Side.with_signals(ui([M.TurnChanged{"p", M.Running{}, 3n}, M.AskOpened{"q", "p", "input", "", "Choose", "", 5n}, M.WorkSet{"p", "watch", "w\twatching\t\tbuild\twatch", 6n}], True{})))), "1")
    C.Check.str("LiveActivity offline cannot claim current work", Nat.show(island(U.Side.with_signals(ui(child(), False{})))), "0")
    C.Check.str("LiveActivity adapter preserves canonical completed turn", M.Phase.name(M.Phase.of.t(M.Thread.turn(U.Ui.thread(U.Ui.state(u), "p")), False{})), "complete")
    C.Check.str("plane legacy API unchanged", P.Plane.state(M.PhMonitoring{1n}), "working")
