import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/liveness.bend as L
import ../src/core/signal.bend as SG
import ../src/core/client.bend as U
import ../src/core/plane.bend as PL
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/bots.bend as WB
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as MB
import ../src/mobile/plane.bend as MP

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Test", "/tmp/liveness-ui", 1n},
    M.ThreadCreated{"t", "p", "Worker", M.Local{}, "", "", "codex", "", 1n}, M.TurnChanged{"t", M.Running{}, 2n}], M.State.empty())

def stalled() -> M.State:
  M.State.apply(base(), M.SettingSet{L.Liveness.key("t"), L.Liveness.write(L.Live{2n, 1202n, True{}, True{}, "Bash: waiting for fixture"}), 1262n})

def ui() -> U.Ui:
  U.Side.with_signals(U.Ui.online(U.Ui.adopt.raw(U.Ui.init(1300n), stalled()), True{}))

def ix() -> U.SIx:
  U.Side.signals.ix(U.Side.ix(stalled()), ui())

def first(rs: List<&2, PL.PRow>) -> PL.PRow:
  match rs:
    case r <> _: r
    case Nil{}: PL.PRow{"", "", "", "", "", "", "", "", 0n, False{}}

def need() -> PL.PRow:
  first(PL.Plane.need(PL.Plane.signals.of(ui(), 1300n, True{})))

def attrs(n: D.Node) -> List<&2, D.Attr>:
  match n:
    case D.El{_, _, as, _}: as
    case _: Nil{}

def sub(r: PL.PRow) -> String:
  match r:
    case PL.PRow{_, _, _, _, _, s, _, _, _, _}: s

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("numeric activity remains active", Nat.show(SG.Signal.at(SG.Signal.index(stalled()), "t")), "1")
    C.Check.str("display distinctly stalled", SG.Signal.display.name(1n, stalled(), "t"), "stalled")
    C.Check.str("reason shared by every view", SG.Signal.display.note(I.En{}, stalled(), "t", ""), "Stalled: Bash: waiting for fixture")
    C.Check.str("stalled belongs in attention", Nat.show(PL.Plane.sec("stalled")), "0")
    C.Check.str("plane row stalled", PL.PRow.state(need()), "stalled")
    C.Check.str("plane reason visible", Bool.show(String.contains(sub(need()), "Bash: waiting for fixture")), "True")
    C.Check.str("plane warning mark distinct", WB.WB.pactivity("stalled"), "stalled")
    C.Check.str("mobile plane includes reason", Bool.show(String.contains(J.Json.get_str(MP.MPlane.row(I.En{}, need(), 1300n, stalled()), "sub", ""), "Bash: waiting for fixture")), "True")
    +row : J.Json = MB.Mob.row(I.En{}, U.Ui.thread(stalled(), "t"), SG.Signal.display.name(1n, stalled(), "t"), ix(), stalled(), 1300n, 0n)
    C.Check.str("mobile sidebar stalled", J.Json.get_str(row, "status", ""), "stalled")
    C.Check.str("mobile sidebar reason", J.Json.get_str(row, "agents", ""), "Stalled: Bash: waiting for fixture")
    +web : D.Node = V.View.row(I.En{}, U.Ui.thread(stalled(), "t"), ix(), stalled(), "t", 1300n, False{}, False{})
    C.Check.str("web sidebar stalled", V.View.attr(attrs(web), "dot"), "stalled")
    C.Check.str("web sidebar reason", V.View.attr(attrs(web), "badge"), "Stalled: Bash: waiting for fixture")
    C.Check.str("thread status says stalled", V.View.chrome.state.word(I.En{}, SG.Signal.display.shown(1n, M.Running{}, 0n, stalled(), "t")), "stalled")
    C.Check.str("progress clears display", SG.Signal.display.name(1n, M.State.apply(stalled(), M.SettingSet{L.Liveness.key("t"), L.Liveness.write(L.Liveness.progress(SG.Signal.display.live(stalled(), "t"), 1301n, "tool output")), 1301n}), "t"), "active")
    C.Check.str("archived marker never shown", SG.Signal.display.name(1n, M.State.apply(stalled(), M.ThreadArchived{"t", True{}, 1301n}), "t"), "active")
