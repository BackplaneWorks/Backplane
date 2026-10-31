import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/core/client.bend as U

# Finding and removing projects: the sidebar's search keeps the projects
# whose name matches, Enter goes to the first; Remove takes two clicks and
# re-adding the folder brings the project back with its threads. A project
# with a live thread (p1) sits above the quiet ones.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "board", "/tmp/board", 100n},
   M.ThreadCreated{"a", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", 300n},
   M.ProjectCreated{"p2", "enclosure", "/tmp/enclosure", 400n},
   M.ProjectCreated{"p3", "firmware", "/tmp/firmware", 500n}]

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> _:
      t
    case _ <> r:
      sent.go(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.go(cs)

def focus.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Focus{f} <> _:
      f
    case _ <> r:
      focus.go(r)

def focus(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      focus.go(cs)

def base() -> U.Ui:
  U.Ui.fold(log(), U.Ui.init(1000n))

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def typed(+q: String) -> U.Ui:
  act(act(base(), "proj-find", ""), "bfield", "pq\u{1f}" ++ q)

def pids(+u: U.Ui) -> String:
  String.join(U.Side.projects(u), ",")

def changes.go(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(K.Change.encode(c)) <> changes.go(t)

def state(es: List<&2, M.Change>) -> M.State:
  M.State.replay(es, M.State.empty())

def step(+es: List<&2, M.Change>, +now: Nat, +text: String) -> H.Step:
  H.Hub.request_text(state(es), now, 1, text)

def after(+es: List<&2, M.Change>, +now: Nat, +text: String) -> List<&2, M.Change>:
  List.append(&2, M.Change, es, H.Queue.changes(step(es, now, text)))

def removed() -> List<&2, M.Change>:
  after(log(), 600n, "{\"id\":0,\"m\":\"project.remove\",\"p\":{\"project\":\"p1\"}}")

def readded() -> List<&2, M.Change>:
  after(removed(), 700n, "{\"id\":2,\"m\":\"project.create\",\"p\":{\"path\":\"/tmp/board/\"}}")

def fresh() -> List<&2, M.Change>:
  after(removed(), 700n, "{\"id\":2,\"m\":\"project.create\",\"p\":{\"path\":\"/tmp/other\"}}")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("all shown", pids(base()), "p1,p3,p2")
    C.Check.str("search focuses", focus(U.Ui.act(base(), "proj-find", "")), "proj-find")
    C.Check.str("empty query keeps all", pids(typed("")), "p1,p3,p2")
    C.Check.str("query filters", pids(typed("bo")), "p1")
    C.Check.str("fuzzy", pids(typed("frw")), "p3")
    C.Check.str("any case", pids(typed("ENC")), "p2")
    C.Check.str("no match", pids(typed("zzz")), "")
    C.Check.str("shut shows all", pids(act(typed("bo"), "proj-find", "")), "p1,p3,p2")
    C.Check.str("shut clears", U.Ui.scratch(act(typed("bo"), "proj-find", ""), "@f.pq"), "")
    C.Check.str("enter goes to top thread", U.Ui.sel(act(typed("bo"), "proj-go", "")), "a")
    C.Check.str("enter shuts", pids(act(typed("bo"), "proj-go", "")), "p1,p3,p2")
    C.Check.str("the web's enter sends what was typed: the top thread", U.Ui.sel(act(typed("bo"), "proj-go", "bo")), "a")
    C.Check.str("and makes no thread", Bool.show(String.contains(sent(U.Ui.act(typed("bo"), "proj-go", "bo")), "thread.create")), "False")
    C.Check.str("a project named by its id: its thread", U.Ui.sel(act(typed(""), "proj-go", "p1")), "a")
    C.Check.str("a project named with no thread: a new one there", sent(U.Ui.act(base(), "proj-go", "p2")), "{\"id\":0,\"m\":\"thread.create\",\"p\":{\"project\":\"p2\",\"title\":\"New thread\"}}")
    C.Check.str("first click arms", sent(U.Ui.act(base(), "proj-remove", "p1")), "")
    C.Check.str("armed", U.Ui.scratch(act(base(), "proj-remove", "p1"), "@rm"), "p1")
    C.Check.str("second click removes", sent(U.Ui.act(act(base(), "proj-remove", "p1"), "proj-remove", "p1")), "{\"id\":0,\"m\":\"project.remove\",\"p\":{\"project\":\"p1\"}}")
    C.Check.str("other project re-arms", sent(U.Ui.act(act(base(), "proj-remove", "p1"), "proj-remove", "p2")), "")
    C.Check.str("editor disarms", U.Ui.scratch(act(act(base(), "proj-remove", "p1"), "look", "p1"), "@rm"), "")
    C.Check.str("removed leaves sidebar", pids(U.Ui.fold(removed(), U.Ui.init(1000n))), "p3,p2")
    C.Check.str("re-add restores id", String.join(changes.go(H.Queue.changes(step(removed(), 700n, "{\"id\":2,\"m\":\"project.create\",\"p\":{\"path\":\"/tmp/board/\"}}"))), " "),
      "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"board\",\"root\":\"/tmp/board\",\"at\":700}")
    C.Check.str("restored with threads", String.join(U.Side.of(U.Ui.fold(readded(), U.Ui.init(1000n)), "p1"), ","), "a")
    C.Check.str("new folder new id", Bool.show(Bool.not(String.eq(U.Side.first(U.Side.projects(U.Ui.fold(fresh(), U.Ui.init(1000n)))), "p1"))), "True")
    C.Check.str("live root still refused", String.join(changes.go(H.Queue.changes(step(log(), 700n, "{\"id\":2,\"m\":\"project.create\",\"p\":{\"path\":\"/tmp/board\"}}"))), " "), "")
    C.Check.str("whitespace query keeps projects", pids(typed("   ")), pids(base()))
    C.Check.str("padded query finds project", pids(typed("  ENC  ")), "p2")
