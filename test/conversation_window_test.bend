import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/client.bend as U
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as W
import ../src/web/view.bend as V
import ../src/server/conversation-window.bend as F
import ../src/server/conversation-frame.bend as CF
import ../src/server/thin-view.bend as TV
import ../src/web/sidebar-slot.bend as SLOT


def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/tmp/window", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "codex", "", 1n}, M.BotSet{"bot", "Bot", "t", 0n, "", 1n}], M.State.empty())

def ui(st: M.State) -> U.Ui:
  U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(1n), st), "t")

def projected(+u: U.Ui) -> J.Json:
  F.Window.make(J.Json.obj([J.KV{"shown", TV.Render.web(u, J.Json.obj(Nil{}))}]))

def painted(+u: U.Ui) -> String:
  J.Json.show(W.Node.write(W.Node.read.first(V.View.expand([W.Node.read(J.Json.get_obj(J.Json.get_obj(projected(u), "windowProjection"), "root"))]))))

# Exercise the actual rendered sidebar, not a hand-built navigation stub.
# Both negotiated and legacy framing must retain its rows and action targets.
def navigation(+u: U.Ui, +regions: Bool) -> Bool:
  +shown = TV.Render.web(u, J.Json.obj(Nil{}))
  +prepared = F.Window.prepare(regions, J.Json.obj([J.KV{"shown", shown}]))
  +after = Bool.pick(J.Json, regions, J.Json.get_obj(J.Json.get_obj(prepared, "windowProjection"), "root"), J.Json.get_obj(prepared, "shown"))
  C.Check.same(J.Json.show(SLOT.Slot.sidebar(shown)), J.Json.show(SLOT.Slot.sidebar(after)))

def main() -> IO(Unit):
  do IO<Unit>:
    +projects : M.State = M.State.replay([M.ProjectCreated{"other-project", "Other open project", "/tmp/other", 1n}, M.ThreadCreated{"other-thread", "other-project", "Other open thread", M.Local{}, "", "", "codex", "", 1n}], state())
    +nav : U.Ui = U.Ui.flag_on(U.Ui.flag_on(ui(projects), "p"), "other-project")
    +canonical : String = J.Json.show(SLOT.Slot.sidebar(TV.Render.web(nav, J.Json.obj(Nil{}))))
    C.Check.str("navigation fixture includes other project", Bool.show(String.contains(canonical, "Other open project")), "True")
    C.Check.str("navigation fixture includes other thread", Bool.show(String.contains(canonical, "Other open thread")), "True")
    C.Check.str("framed conversation retains complete sidebar", Bool.show(navigation(nav, True{})), "True")
    C.Check.str("legacy conversation retains complete sidebar", Bool.show(navigation(nav, False{})), "True")
    C.Check.str("active sidebar survives framing", Bool.show(navigation(U.Ui.adopt.raw(nav, M.State.setting(projects, "sidebar.view", "active")), True{})), "True")
    C.Check.str("filtered sidebar survives framing", Bool.show(navigation(U.Ui.scratch_set(nav, "@f.pq", "Other"), True{})), "True")
    +person : M.State = M.State.replay([M.MessagePosted{"t", "person", M.User{}, "Real user text", 2n}], state())
    +blank : M.State = M.State.replay([M.MessagePosted{"t", "blank", M.Assistant{}, " \n\t", 2n}], state())
    +internal : M.State = M.State.replay([M.ActivityLogged{"t", "ride:receipt", "info", "ride", "Hidden receipt", 3n}], state())
    C.Check.str("pressure preserves a real user role", Bool.show(String.contains(painted(ui(person)), "msg user")), "True")
    C.Check.str("pressure omits blank assistant rows", Bool.show(String.contains(painted(ui(blank)), "data-id")), "False")
    C.Check.str("pressure omits internal bot receipts", Bool.show(String.contains(painted(ui(internal)), "Hidden receipt")), "False")
    +st : M.State = M.State.replay([M.MessagePosted{"t", "person", M.User{}, "Run", 2n}, M.TurnChanged{"t", M.Running{}, 3n}, M.ActivityLogged{"t", "tool-first", "tool", "shell", "SECRET TOOL BODY", 4n}, M.ActivityLogged{"t", "tool-last", "tool", "read", "SECOND SECRET BODY", 5n}], state())
    +index : T.Index = T.Index.post(T.Index.post(T.Index.empty(), M.Act{"tool-first", "t", "tool", "shell", "SECRET TOOL BODY", 4n}), M.Act{"tool-last", "t", "tool", "read", "SECOND SECRET BODY", 5n})
    +closed : String = painted(ui(st))
    C.Check.str("collapsed tools do not materialize their body", Bool.show(String.contains(closed, "SECRET TOOL BODY")), "False")
    C.Check.str("visible live fold keeps its real first id", Bool.show(String.contains(closed, "1tool-first")), "True")
    C.Check.str("running fold remains live", Bool.show(String.contains(closed, "fold live")), "True")
    +open : U.Ui = U.Ui.info_set(ui(st), "fold:tool-first", "1")
    C.Check.str("explicitly opened fold retains lazy body", Bool.show(String.contains(painted(open), "SECRET TOOL BODY")), "True")
    +viewer : U.Ui = U.Act.ui(U.Ui.toggle_panel(U.Ui.view_get.set(U.Ui.view_get.set(ui(state()), "view.kind", "3d"), "view.path", "/tmp/window/assembly.step")))
    +panel : String = J.Json.show(J.Json.get_obj(J.Json.get_obj(projected(viewer), "windowProjection"), "root"))
    C.Check.str("pressure retains requested viewer", Bool.show(String.contains(panel, "viewer")), "True")
    C.Check.str("pressure retains canonical solid workbench", Bool.show(String.contains(panel, "solid-workbench")), "True")
    C.Check.str("pressure retains canonical solid canvas", Bool.show(String.contains(panel, "\"solid\"")), "True")
    +packet : J.Json = projected(viewer)
    C.Check.str("requested viewer floor remains below transport cap", Bool.show(Nat.is_le(J.Json.bytes(CF.Control.envelope(J.Num{"1"}, packet)), 262144n)), "True")
    C.Check.str("navigation ACK never acquires a synthetic picture", Bool.show(Maybe.is_some(&2, J.Json, J.Json.get(F.Window.prepare(True{}, J.Json.obj([J.KV{"nextTarget", J.Flag{True{}}}])), "shown"))), "False")
