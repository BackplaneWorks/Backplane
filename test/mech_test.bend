import Base
import ./check.bend as C
import ../src/core/mech.bend as MC
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# A configured model is only a default: opening Mech uses it, while an
# explicit selection and a different thread's viewer keep their own files.
def listing() -> String:
  "1 9 mech/bracket.step\n2 9 mech/case-assembly.step\n3 9 mech/lid.glb\n4 9 mech/renders/lid.iso.png"

def parts() -> List<&2, MC.MPart>:
  MC.Mech.parts(listing())

def path(m: Maybe<&2, MC.MPart>) -> String:
  match m:
    case Some{MC.MPart{p, _, _}}:
      p
    case None{}:
      ""

def preferred(+p: String) -> String:
  path(MC.Mech.preferred(parts(), p))

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, a, v))

def base() -> U.Ui:
  act(U.Ui.fold([M.ProjectCreated{"p", "Parts", "/p", 1n},
    M.ThreadCreated{"a", "p", "First", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"b", "p", "Second", M.Local{}, "", "", "claude", "", 3n}], U.Ui.init(10n)), "select", "a")

def result(+th: String, +primary: String) -> J.Json:
  J.Json.obj([J.KV{"mech", J.Str{listing()}}, J.KV{"root", J.Str{"/p"}}, J.KV{"thread", J.Str{th}}, J.KV{"primary", J.Str{primary}}])

def reply(+u: U.Ui, +r: J.Json) -> U.Ui:
  U.Ui.recv(u, J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"1"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", r}]))

def listed(+u: U.Ui, +th: String, +primary: String) -> U.Ui:
  reply(u, result(th, primary))

def shown(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.path")

def main() -> IO(Unit):
  +waiting = act(base(), "mech", "")
  +opened = listed(waiting, "a", "mech/lid.glb")
  +picked = act(opened, "mech-part", "mech/bracket.step")
  +known = listed(base(), "a", "mech/lid.glb")
  +old = J.Json.obj([J.KV{"mech", J.Str{listing()}}, J.KV{"root", J.Str{"/p"}}, J.KV{"thread", J.Str{"a"}}])
  +other = listed(act(waiting, "select", "b"), "a", "mech/lid.glb")
  +agent = U.Ui.recv(waiting, J.Json.obj([J.KV{"t", J.Str{"view"}}, J.KV{"thread", J.Str{"a"}},
    J.KV{"kind", J.Str{"mech"}}, J.KV{"path", J.Str{"/p/vendor/spacer.step"}}]))
  do IO<Unit>:
    C.Check.str("config coexists with board settings", MC.Mech.config("{\"pcb\":\"board.kicad_pcb\",\"mech\":\"mech/lid.glb\"}"), "mech/lid.glb")
    C.Check.str("missing config", MC.Mech.config(""), "")
    C.Check.str("malformed config", MC.Mech.config("{broken"), "")
    C.Check.str("wrong config type", MC.Mech.config("{\"mech\":42}"), "")
    C.Check.str("no primary keeps the assembly default", preferred(""), "mech/case-assembly.step")
    C.Check.str("primary overrides assembly heuristic", preferred("mech/lid.glb"), "mech/lid.glb")
    C.Check.str("missing primary falls back", preferred("mech/missing.step"), "mech/case-assembly.step")
    C.Check.str("a render cannot become the model", preferred("mech/renders/lid.iso.png"), "mech/case-assembly.step")
    C.Check.str("a config path must be project relative", preferred("/p/mech/lid.glb"), "mech/case-assembly.step")
    C.Check.str("a config path cannot escape the list", preferred("../other.step"), "mech/case-assembly.step")
    C.Check.str("no assembly falls back to first", path(MC.Mech.preferred(MC.Mech.parts("1 9 mech/a.stp\n2 9 mech/b.step"), "missing.step")), "mech/a.stp")
    C.Check.str("no parts remains empty", path(MC.Mech.preferred(Nil{}, "mech/lid.glb")), "")
    C.Check.str("relative selection wins", path(MC.Mech.choose(parts(), "mech/bracket.step", "/p", "mech/lid.glb")), "mech/bracket.step")
    C.Check.str("absolute selection wins", path(MC.Mech.choose(parts(), "/p/mech/bracket.step", "/p", "mech/lid.glb")), "mech/bracket.step")
    C.Check.str("cold Mech waits for its list", U.Ui.view_get(waiting, "view.kind"), "mech")
    C.Check.str("cold Mech opens the primary model", shown(opened), "/p/mech/lid.glb")
    C.Check.str("known listing opens the primary model", shown(act(known, "mech", "")), "/p/mech/lid.glb")
    C.Check.str("render page uses the primary model", U.Mpage.cur.path(known), "mech/lid.glb")
    C.Check.str("old hub falls back to assembly", shown(reply(waiting, old)), "/p/mech/case-assembly.step")
    C.Check.str("refresh does not replace the selected part", shown(listed(picked, "a", "mech/lid.glb")), "/p/mech/bracket.step")
    C.Check.str("reopening keeps a selected part", shown(act(picked, "mech", "")), "/p/mech/bracket.step")
    C.Check.str("render replies retain primary", U.Ui.info(reply(opened, J.Json.obj([J.KV{"mech", J.Str{listing()}}, J.KV{"root", J.Str{"/p"}},
      J.KV{"thread", J.Str{"a"}}, J.KV{"primary", J.Str{"mech/lid.glb"}}, J.KV{"rendered", J.Str{"mech/lid.glb"}}, J.KV{"error", J.Str{""}}])), "mech.primary:a"), "mech/lid.glb")
    C.Check.str("removing config clears the stored primary", U.Ui.info(listed(opened, "a", ""), "mech.primary:a"), "")
    C.Check.str("another thread's reply leaves this viewer alone", shown(other), "")
    C.Check.str("the waiting thread still gets its model", shown(act(other, "select", "a")), "/p/mech/lid.glb")
    C.Check.str("late list does not override an agent's model", shown(listed(agent, "a", "mech/lid.glb")), "/p/vendor/spacer.step")
    C.Check.str("late list does not return from another tab", U.Ui.view_get(listed(act(waiting, "fm", ""), "a", "mech/lid.glb"), "view.kind"), "files")
