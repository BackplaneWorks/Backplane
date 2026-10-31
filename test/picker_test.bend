import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/fuzzy.bend as FZ
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as H

def labels(ps: List<&2, U.PItem>) -> String:
  match ps:
    case Nil{}:
      ""
    case U.PItem{l, _, _} <> t:
      l ++ "|" ++ labels(t)

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def listing(+id: String, +dir: String) -> J.Json:
  json("{\"t\":\"reply\",\"id\":" ++ id ++ ",\"ok\":true,\"result\":{\"dir\":\"" ++ dir ++ "\",\"entries\":[\"Development\",\"docs\",\".cache\",\"dev-notes\"]}}")

# the picker opened (its fs.list is request 0, its fs.find 1), that
# listing arrived, and it went to the folders
def listed() -> U.Ui:
  U.Ui.scratch_set(U.Ui.recv(ui(U.Ui.act(U.Ui.init(0n), "proj-open", "")), listing("0", "/home/h")), "@proj.mode", "dir")

def at(+q: String) -> String:
  labels(U.Proj.items(U.Proj.set(listed(), q, 0n)))

# a live project with two threads, one removed, one with none
def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "board", "/home/h/board", 1n}, M.ThreadCreated{"t0", "p1", "a", M.Local{}, "", "", "claude", "", 10n},
   M.ThreadCreated{"t1", "p1", "b", M.Local{}, "", "", "claude", "", 20n}, M.ProjectCreated{"p2", "old", "/home/h/old", 2n},
   M.ProjectRemoved{"p2", 3n}, M.ProjectCreated{"p3", "empty", "/home/h/empty", 4n}]

def found() -> J.Json:
  json("{\"t\":\"reply\",\"id\":1,\"ok\":true,\"result\":{\"home\":\"/home/h\",\"found\":[\"/home/h/board\",\"/home/h/kart\",\"/home/h/old\"]}}")

# the picker open on the projects, what the hub found in
def projects() -> U.Ui:
  U.Ui.recv(ui(U.Ui.act(U.Ui.fold(log(), U.Ui.init(0n)), "proj-open", "")), found())

def rows(+q: String) -> String:
  labels(U.Proj.items(U.Proj.set(projects(), q, 0n)))

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> r:
      J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}), "m", "") ++ ";" ++ sent(r)
    case _ <> r:
      sent(r)

def asks(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent(cs)

def tags(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.ProjectCreated{_, _, _, _} <> t:
      "ProjectCreated " ++ tags(t)
    case M.ThreadCreated{_, p, _, _, _, _, _, _, _} <> t:
      "ThreadCreated:" ++ p ++ " " ++ tags(t)
    case _ <> t:
      "? " ++ tags(t)

def step.tags(st: H.Step) -> String:
  match st:
    case H.Step{cs, _}:
      tags(cs)

def made(+root: String) -> String:
  step.tags(H.Hub.project_add(M.State.replay(log(), M.State.empty()), 50n, 1, "2", json("{\"path\":\"" ++ root ++ "\"}")))

def main() -> IO(Unit):
  +cd = ui(U.Ui.act(listed(), "proj-cd", "/home/h/docs"))
  +late = U.Ui.recv(cd, listing("0", "/somewhere/old"))
  +adding = ui(U.Ui.act(listed(), "proj-add", "/home/h"))
  +refused = U.Ui.recv(adding, json("{\"t\":\"reply\",\"id\":2,\"ok\":false,\"error\":\"already a project: /home/h\"}"))
  +added = U.Ui.recv(adding, json("{\"t\":\"reply\",\"id\":2,\"ok\":true,\"result\":{}}"))
  +missing = U.Ui.recv(cd, json("{\"t\":\"reply\",\"id\":2,\"ok\":false,\"result\":{\"error\":\"no such folder: /home/h/docs\"}}"))
  +gone = U.Ui.act(projects(), "proj-pick", "/home/h/board")
  +browse = ui(U.Ui.act(projects(), "proj-browse", ""))
  +fresh = {M.ThreadCreated{"t9", "p9", "New thread", M.Local{}, "", "", "claude", "", 60n} : M.Change}
  do IO<Unit>:
    C.Check.str("no filter", at(""), "Add /home/h as a project|..|Development/|docs/|dev-notes/|")
    C.Check.str("filter ranks", at("dev"), "Development/|dev-notes/|Create project 'dev' here|Create folder 'dev' here|Add /home/h as a project|")
    C.Check.str("existing name is not created", at("docs"), "docs/|Add /home/h as a project|")
    C.Check.str("hidden on dot", at(".ca"), ".cache/|Create project '.ca' here|Create folder '.ca' here|Add /home/h as a project|")
    C.Check.str("name trimmed", at(" new "), "Create project 'new' here|Create folder 'new' here|Add /home/h as a project|")
    C.Check.str("path", at("~/x"), "Add ~/x as a project|Go to ~/x|Create project ~/x|")
    C.Check.str("path to go into", at("~/x/"), "Go to ~/x/|Add ~/x/ as a project|Create project ~/x/|")
    # Enter on a folder typed in full adds it; on one ending in a slash it goes in
    C.Check.str("enter adds a typed folder", asks(U.Ui.act(U.Proj.set(listed(), "/tmp/carrier", 0n), "proj-enter", "")), "project.add;")
    C.Check.str("enter goes into a folder/", asks(U.Ui.act(U.Proj.set(listed(), "/tmp/carrier/", 0n), "proj-enter", "")), "fs.list;")
    C.Check.str("slash name", at("a/b"), "Add /home/h as a project|")
    C.Check.str("parent", U.Proj.parent("/home/h"), "/home")
    C.Check.str("parent of top", U.Proj.parent("/home"), "/")
    C.Check.str("no match", Nat.show(FZ.Fuzzy.score("docs", "x")), "0")
    C.Check.str("late listing dropped", U.Proj.dir(late), "/home/h")
    C.Check.str("refused shows in picker", U.Ui.info(refused, "proj.err"), "already a project: /home/h")
    C.Check.str("refused stays open", Bool.show(U.Ui.flag(refused, "proj")), "True")
    C.Check.str("added closes", Bool.show(U.Ui.flag(added, "proj")), "False")
    C.Check.str("added goes to its thread", U.Ui.sel(U.Ui.recv(U.Ui.fold([fresh], adding), json("{\"t\":\"reply\",\"id\":2,\"ok\":true,\"thread\":\"t9\",\"result\":{}}"))), "t9")
    C.Check.str("an older hub's plain answer selects nothing", U.Ui.sel(U.Ui.recv(U.Ui.fold([fresh], adding), json("{\"t\":\"reply\",\"id\":2,\"ok\":true,\"result\":{}}"))), "")
    C.Check.str("another's new thread selects nothing", U.Ui.sel(U.Ui.fold([fresh], adding)), "")
    C.Check.str("refused waits for no thread", U.Ui.sel(U.Ui.fold([fresh], refused)), "")
    C.Check.str("added waits no more", U.Ui.sel(U.Ui.fold([fresh], added)), "")
    C.Check.str("new project starts a thread", made("/home/h/fan"), "ProjectCreated ThreadCreated:p50-6 ")
    C.Check.str("brought back starts none", made("/home/h/old"), "ProjectCreated ")
    C.Check.str("server error text", U.Ui.info(missing, "proj.err"), "no such folder: /home/h/docs")
    C.Check.str("opens on projects", Bool.show(U.Proj.browsing(projects())), "False")
    C.Check.str("open asks", asks(U.Ui.act(U.Ui.init(0n), "proj-open", "")), "fs.list;fs.find;")
    C.Check.str("projects", rows(""), "Add a project from a folder|board|empty|Add kart|Add old|")
    C.Check.str("projects filtered", rows("ol"), "Add old|Add a project from a folder|")
    C.Check.str("projects path", rows("~/x"), "Add ~/x as a project|Go to ~/x|Create project ~/x|")
    C.Check.str("path beside", U.Proj.detail(projects(), U.PItem{"board", "proj-pick", "/home/h/board"}), "~/board")
    C.Check.str("no path beside", U.Proj.detail(projects(), U.PItem{"..", "proj-cd", "/home"}), "")
    C.Check.str("go selects latest", U.Ui.sel(ui(gone)), "t1")
    C.Check.str("go closes", Bool.show(U.Ui.flag(ui(gone), "proj")), "False")
    C.Check.str("go to empty starts a thread", asks(U.Ui.act(projects(), "proj-pick", "/home/h/empty")), "thread.create;")
    C.Check.str("found adds", asks(U.Ui.act(projects(), "proj-found", "/home/h/kart")), "project.add;")
    C.Check.str("browse", Bool.show(U.Proj.browsing(browse)), "True")
    C.Check.str("back", Bool.show(U.Proj.browsing(ui(U.Ui.act(browse, "proj-back", "")))), "False")
    C.Check.str("cd browses", Bool.show(U.Proj.browsing(ui(U.Ui.act(projects(), "proj-cd", "/tmp")))), "True")
    C.Check.str("find error kept quiet", U.Ui.info(U.Ui.recv(ui(U.Ui.act(U.Ui.init(0n), "proj-open", "")), json("{\"t\":\"reply\",\"id\":1,\"ok\":false,\"error\":\"unknown\"}")), "proj.err"), "")
    C.Check.str("pasted path trims only its edges", U.Proj.query(U.Ui.scratch_set(U.Ui.init(0n), "@f.proj", "  /tmp/Plain notes  ")), "/tmp/Plain notes")
    C.Check.str("pasted home path", U.Proj.query(U.Ui.scratch_set(U.Ui.init(0n), "@f.proj", "  ~/Plain notes/  ")), "~/Plain notes/")
