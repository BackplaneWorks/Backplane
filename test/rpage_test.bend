import Base
import ../src/server/thin-actor.bend as TA
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/board.bend as B
import ../src/core/hist.bend as H
import ../src/core/layers.bend as LY
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# Design review's Review tab (docs/design-review.md, client.bend's Rp): its
# pages, what it asks the hub, the reports it keeps.

def item(+k: String, +sig: U32, +layer: U32) -> B.Item:
  B.Item{k, sig, layer, B.PSeg{B.Pt{0.0, 0.0}, B.Pt{1.0, 1.0}, 0.2}, k}

def Show.items(xs: List<&2, B.Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Item{+k, _, +l, _, _} <> t:
      k ++ "@" ++ U32.show(l) ++ " " ++ Show.items(t)

def olds() -> List<&2, B.Item>:
  [item("a", 1, 12), item("b", 1, 5), item("c", 1, 12)]

def news() -> List<&2, B.Item>:
  [item("a", 2, 12), item("b", 2, 5), item("d", 1, 5)]

def labels(rs: List<&2, H.Ref>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      U.Hs.ref.label(r) <> labels(t)

def recv(+u: U.Ui, +text: String) -> U.Ui:
  U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def act(u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def sends(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      Nat.show(List.length(&2, U.Cmd, cs))

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

# thread t selected, its viewer on the board /r/b.kicad_pcb
def base() -> U.Ui:
  U.Ui.view_put(act(U.Ui.adopt(U.Ui.init(0n), state()), "select", "t"), "board", "/r/b.kicad_pcb")

def path(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.path")

def git(+c: String) -> String:
  H.Hist.git("/r/.git", c, "/r", "b.kicad_pcb")

def tabs(ts: List<&2, U.RpTab>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTab{_, +l, on} <> t:
      (l ++ Bool.pick(String, on, "*", "")) <> tabs(t)

def Show.sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{+x} <> t:
      x ++ Show.sent(t)
    case _ <> t:
      Show.sent(t)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      Show.sent(cs)

def report(+th: String, +md: String, +err: String) -> String:
  "{\"t\":\"reply\",\"id\":1,\"ok\":true,\"result\":{\"report\":{\"thread\":\"" ++ th ++ "\",\"page\":\"bom\",\"md\":\"" ++ md ++ "\",\"err\":\"" ++ err ++ "\"}}}"

def main() -> IO(Unit):
  +u = base()
  +rv = act(u, "rv-page", "bom")
  +cmp = act(u, "cmp-pick", "a\tc1\tv1\t/r/.git\t/r")
  do IO<Unit>:
    C.Check.str("restored report is a follow-up read", Bool.show(TA.Actor.read_only(J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{"{\"m\":\"review.bom\"}"}}]))), "True")
    C.Check.str("releases are never follow-up reads", Bool.show(TA.Actor.read_only(J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{"{\"m\":\"review.release\"}"}}]))), "False")
    C.Check.str("restored BOM asks for report", Bool.show(String.contains(Show.sent(U.Act.cmds(U.Rp.sync(U.Act{U.Ui.view_put(base(), "review", "bom"), Nil{}}))), "review.bom")), "True")
    C.Check.str("restored BOM asks once", Show.sent(U.Act.cmds(U.Rp.sync(U.Act{ui(U.Rp.sync(U.Act{U.Ui.view_put(base(), "review", "bom"), Nil{}})), Nil{}}))), "")
    C.Check.str("closed review does not ask", Show.sent(U.Act.cmds(U.Rp.sync(U.Act{base(), Nil{}}))), "")
    C.Check.str("the tab shows the BOM page", U.Ui.view_get(rv, "view.kind") ++ "|" ++ U.Rp.page(rv), "review|bom")
    C.Check.str("opening it asks the hub for the working copy's BOM", Bool.show(String.contains(sent(U.Ui.act(u, "rv-page", "bom")), "review.bom")), "True")
    C.Check.str("with no versions", Bool.show(String.contains(sent(U.Ui.act(u, "rv-page", "bom")), "\"a\":\"\"")), "True")
    C.Check.str("it is busy until the answer", Bool.show(U.RpView.busy(U.Rp.view(rv))) ++ "|" ++ U.RpView.note(U.Rp.view(rv)), "True|Reading the design…")
    C.Check.str("its title", U.RpView.title(U.Rp.view(rv)), "BOM · Working copy")
    C.Check.str("the answer is kept", U.RpView.md(U.Rp.view(recv(rv, report("t", "3 parts", "")))) ++ "|" ++ Bool.show(U.RpView.busy(U.Rp.view(recv(rv, report("t", "3 parts", ""))))), "3 parts|False")
    C.Check.str("a failure says why", U.RpView.note(U.Rp.view(recv(rv, report("t", "", "This project has no schematic.")))), "This project has no schematic.")
    C.Check.str("another thread's report stays there", U.RpView.md(U.Rp.view(recv(rv, report("x", "3 parts", "")))), "")
    C.Check.str("a comparison makes it a BOM change", U.RpView.title(U.Rp.view(act(cmp, "rv-bom", ""))), "BOM change · v1 → Working copy")
    C.Check.str("of the comparison's versions", Bool.show(String.contains(sent(U.Ui.act(cmp, "rv-bom", "")), "\\tc1\\t")), "True")
    C.Check.str("rv-run asks again", Bool.show(String.contains(sent(U.Ui.act(rv, "rv-run", "")), "review.bom")), "True")
    C.Check.str("no page off the tab", U.Rp.page(u), "")
    C.Check.str("the tabs", String.join(tabs(U.RpView.tabs(U.Rp.view(rv))), ","), "BOM*,Checks,Comments,Release")
    # back from the Review tab to the board: the comparison as it was
    C.Check.str("back to the board keeps the comparison", path(act(act(cmp, "rv-bom", ""), "view-kind", "board")), H.Hist.diff(git("c1"), "/r/b.kicad_pcb"))
    C.Check.str("as Old when Old showed", path(act(act(act(cmp, "rv-as", "old"), "rv-bom", ""), "view-kind", "board")), git("c1"))
    C.Check.str("the schematic tab is another file", path(act(act(cmp, "rv-bom", ""), "view-kind", "schematic")), "")
    C.Check.str("the window's tab too", U.Ui.view_get(U.Ui.view_pick.back(act(cmp, "rv-bom", ""), "board"), "view.path"), H.Hist.diff(git("c1"), "/r/b.kicad_pcb"))
    # the Release page: the next tag proposed from the repository's tags
    C.Check.str("release proposes v1 with no tags", U.Rl.tag(act(u, "rv-page", "release")), "v1")
    C.Check.str("and the next after the newest", U.Rl.tag(recv(act(u, "rv-page", "release"), "{\"t\":\"reply\",\"id\":1,\"ok\":true,\"result\":{\"thread\":\"t\",\"git\":\"/r/.git\",\"top\":\"/r\",\"refs\":\"tag\\tv1.9\\tc1\\ntag\\tv1.10\\tc2\\nbranch\\tmain\\tc3\"}}")), "v1.11")
    C.Check.str("a tag typed in wins", U.Rl.tag(U.Ui.scratch_set(act(u, "rv-page", "release"), "@f.rltag", " rev-B ")), "rev-B")
    C.Check.str("publish is off until asked", Bool.show(U.Rl.on(act(u, "rv-page", "release"), "@rl.pub")) ++ Bool.show(U.Rl.on(act(act(u, "rv-page", "release"), "rv-release-pub", ""), "@rl.pub")), "FalseTrue")
    C.Check.str("making it asks the hub", Bool.show(String.contains(sent(U.Ui.act(act(u, "rv-page", "release"), "rv-release", "")), "review.release")), "True")
    C.Check.str("without publishing", Bool.show(String.contains(sent(U.Ui.act(act(u, "rv-page", "release"), "rv-release", "")), "\"publish\":false")), "True")
