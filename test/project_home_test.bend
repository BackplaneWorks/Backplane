import Base
import ./check.bend as C
import ../src/core/project-home.bend as PH
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/subthreads.bend as ST
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/project-home.bend as W
import ../src/web/presentation.bend as WP
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as MB
import ../src/mobile/thin.bend as MT
import ../src/server/thin-actor.bend as A

# Pure decisions only: no installed hub, provider, filesystem or CAD process.
def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Spec project", "/tmp/project-home-test", 1n},
    M.ProjectCreated{"q", "Software", "/tmp/project-home-other", 1n}], M.State.empty())

def ui() -> U.Ui:
  U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(30n), state()), PH.Home.owner("p"))

def params() -> J.Json:
  J.Json.obj([J.KV{"project", J.Str{"p"}}, J.KV{"msg", J.Str{"message-1"}}, J.KV{"text", J.Str{"Exact first message 日本語"}}])

def created() -> H.Step:
  H.Home.start(state(), 31n, 0, "request", params())

def after() -> M.State:
  M.State.replay(H.Queue.changes(created()), state())

def posted(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{_, _, M.User{}, _, _} <> rest: (1n + posted(rest) : Nat)
    case _ <> rest: posted(rest)

def web(+u: U.Ui) -> String:
  J.Json.show(WP.Node.write(W.HomeView.node(u, Nil{})))

def snapshot(+u: U.Ui, project: String, artifacts: List<&2, J.Json>) -> U.Ui:
  U.Ui.info_set(u, "home.snapshot", J.Json.show(J.Json.obj([J.KV{"project_home", J.Str{project}}, J.KV{"intent", J.Str{"Saved intent"}}, J.KV{"artifacts", J.Json.arr(artifacts)}])))

def artifact(+kind: String, path: String) -> J.Json:
  J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"path", J.Str{path}}, J.KV{"title", J.Str{kind}}, J.KV{"summary", J.Str{"Saved text"}}])

def home() -> U.Ui:
  snapshot(ui(), "p", [artifact("board", "/tmp/p.kicad_pcb"), artifact("schematic", "/tmp/p.kicad_sch"), artifact("mech", "/tmp/mech/a.step"), artifact("intent", "/tmp/.backplane/intent.md")])

def first_send(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{t} <> _: t
    case _ <> rest: first_send(rest)

def sent(a: U.Act) -> J.Json:
  match a:
    case U.Act{_, cs}: Maybe.default(&2, J.Json, J.Json.parse(first_send(cs)), J.Null{})

def param(+action: String, value: String, key: String) -> String:
  J.Json.get_str(J.Json.get_obj(sent(U.Ui.act(home(), action, value)), "p"), key, "")

def number(action: String, value: String, key: String) -> String:
  J.Json.show(J.Json.get_obj(J.Json.get_obj(sent(U.Ui.act(home(), action, value)), "p"), key))

def naming(kind: String, id: String) -> U.Ui:
  U.Act.ui(U.Ui.act(U.Ui.scratch_set(home(), "project-home-p", "Keep my unsent question"), "home-rename", J.Json.show(J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"id", J.Str{id}}]))))

def renamed(+u: U.Ui, title: String) -> J.Json:
  sent(U.Ui.act(u, "rename-save", U.Ren.on(u) ++ "\u{1f}" ++ title))

def mutation(cs: List<&2, U.Cmd>) -> Bool:
  match cs:
    case Nil{}: False{}
    case U.Send{t} <> rest:
      +m = J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}), "m", "")
      Bool.or(Bool.or(String.eq(m, "project.rename"), Bool.or(String.eq(m, "thread.rename"), String.eq(m, "setting.set"))), mutation(rest))
    case _ <> rest: mutation(rest)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("project route round trip", PH.Home.project(PH.Home.owner("p")), "p")
    C.Check.str("absent artifact has no panel", Nat.show(List.length(&2, PH.Panel, PH.Home.panel("board", "", "Board", ""))), "0")
    C.Check.str("empty snapshot has no CAD control", Bool.show(String.contains(web(ui()), "Open schematic")), "False")
    C.Check.str("foreign project snapshot is never displayed", Bool.show(String.contains(web(snapshot(ui(), "q", [artifact("schematic", "/tmp/q.kicad_sch")])), "Open schematic")), "False")
    C.Check.str("saved schematic exposes viewer", Bool.show(String.contains(web(snapshot(ui(), "p", [artifact("schematic", "/tmp/p.kicad_sch")])), "Open schematic")), "True")
    C.Check.str("opening home creates no canonical conversation", Nat.show(List.length(&2, M.Change, H.Queue.changes(H.Home.request(state(), 0, "read", params())))), "0")
    C.Check.str("first send creates one exact human message", Nat.show(posted(H.Queue.changes(created()))), "1")
    C.Check.str("first send owns its project", ST.Tree.project(H.Hub.thread(after(), "home-message-1")), "p")
    C.Check.str("retry creates no second message or thread", Nat.show(List.length(&2, M.Change, H.Queue.changes(H.Home.start(after(), 32n, 0, "retry", params())))), "0")
    C.Check.str("removed project cannot start a thread", Nat.show(List.length(&2, M.Change, H.Queue.changes(H.Home.start(M.State.replay([M.ProjectRemoved{"p", 32n}], state()), 33n, 0, "removed", params())))), "0")
    C.Check.str("unsafe durable message key rejected", Bool.show(PH.Home.token("../bad")), "False")
    C.Check.str("native home has explicit owner", J.Json.get_str(MB.Mob.home.shown(True{}, I.En{}, ui()), "owner", ""), "project-home-p")
    C.Check.str("native route uses project target", J.Json.get_str(MT.Target.of(J.Json.obj([J.KV{"sel", J.Str{PH.Home.owner("p")}}])), "kind", ""), "project")
    C.Check.str("native home is not a fabricated thread", J.Json.show(MB.Mob.focus.thread(I.En{}, True{}, ui(), PH.Home.owner("p"))), "null")

    C.Check.str("refresh performs the project contents RPC", J.Json.get_str(sent(U.Ui.act(home(), "home-refresh", "")), "m", ""), "project.home")
    C.Check.str("board control opens exact saved board", U.Ui.view_get(U.Act.ui(U.Ui.act(home(), "home-view", "board")), "view.path"), "/tmp/p.kicad_pcb")
    C.Check.str("schematic control opens exact saved schematic", U.Ui.view_get(U.Act.ui(U.Ui.act(home(), "home-view", "schematic")), "view.path"), "/tmp/p.kicad_sch")
    C.Check.str("Mech control opens exact saved model", U.Ui.view_get(U.Act.ui(U.Ui.act(home(), "home-model", "/tmp/mech/a.step")), "view.path"), "/tmp/mech/a.step")
    C.Check.str("thread composer launches the atomic project RPC", J.Json.get_str(sent(U.Ui.act(U.Ui.scratch_set(home(), "project-home-p", "A new question"), "home-start", "")), "m", ""), "project.start")
    C.Check.str("native send captures exact tap text", param("home-start", "Exact tap text", "text"), "Exact tap text")
    C.Check.str("thread paging changes its bounded window", U.Ui.scratch(U.Act.ui(U.Ui.act(home(), "home-threads", "32")), "@home.threads"), "32")
    C.Check.str("edit opens this owner's field", U.Ui.scratch(U.Act.ui(U.Ui.act(home(), "home-intent", "")), "@home.edit:project-home-p"), "1")
    C.Check.str("Save sends a revision checked intent operation", param("home-save", "Edited goal", "op"), "intent")
    C.Check.str("Cancel closes the editor", U.Ui.scratch(U.Act.ui(U.Ui.act(U.Act.ui(U.Ui.act(home(), "home-intent", "")), "home-cancel", "")), "@home.edit:project-home-p"), "")
    C.Check.str("parts continuation sends its cursor", number("home-more", "26", "after"), "26")
    C.Check.str("discovery continuation sends its cursor", number("home-scan", "256", "scan"), "256")
    C.Check.str("notes continuation sends its byte offset", number("home-edit-page", "8192", "editor"), "8192")
    C.Check.str("document control reads the chosen file", param("home-document", "/tmp/README.md", "path"), "/tmp/README.md")
    C.Check.str("document continuation sends its byte offset", number("home-document-more", "8192", "offset"), "8192")
    C.Check.str("Close document removes its picture", U.Ui.info(U.Act.ui(U.Ui.act(U.Ui.info_set(home(), "home.document", "saved"), "home-document-close", "")), "home.document"), "")
    C.Check.str("price uses the existing estimator path explicitly", param("home-price", "", "op"), "price")

    C.Check.str("project inline rename shares canonical RPC", J.Json.get_str(renamed(naming("project", "p"), "New project"), "m", ""), "project.rename")
    C.Check.str("artifact rename uses existing settings RPC", J.Json.get_str(renamed(naming("artifact", "/tmp/mech/a.step"), "Housing"), "m", ""), "setting.set")
    C.Check.str("unknown artifact cannot be renamed", U.Ren.on(naming("artifact", "/tmp/not-in-project.step")), "")
    C.Check.str("blank name never emits a mutation", Bool.show(mutation(U.Act.cmds(U.Ui.act(naming("project", "p"), "rename-save", "p\u{1f}  ")))), "False")
    C.Check.str("cancel clears independent name editor", U.Ren.on(U.Act.ui(U.Ui.act(naming("project", "p"), "rename-no", ""))), "")
    C.Check.str("rename retains exact composer draft", U.Ui.draft_text(U.Act.ui(U.Ui.act(naming("project", "p"), "rename-save", "p\u{1f}Updated"))), "Keep my unsent question")
    C.Check.str("cancel retains exact composer draft", U.Ui.draft_text(U.Act.ui(U.Ui.act(naming("project", "p"), "rename-no", ""))), "Keep my unsent question")
    C.Check.str("project title upsert retains its canonical root", U.Ui.project_root(M.State.replay(H.Home.rename.project(H.Hub.project(state(), "p"), "Updated"), state()), "p"), "/tmp/project-home-test")
    C.Check.str("project title upsert changes sidebar read model", U.Ui.project_title(M.State.replay(H.Home.rename.project(H.Hub.project(state(), "p"), "Updated"), state()), "p"), "Updated")
    C.Check.str("removed project cannot be resurrected by rename", Nat.show(List.length(&2, M.Change, H.Home.rename.project(H.Hub.project(M.State.replay([M.ProjectRemoved{"p", 2n}], state()), "p"), "Updated"))), "0")
    C.Check.str("artifact names are relative settings, never file operations", J.Json.get_str(J.Json.get_obj(renamed(naming("artifact", "/tmp/mech/a.step"), "Housing"), "p"), "key", ""), "project.artifact.name.p:/tmp/mech/a.step")
    C.Check.str("home header retains project icon control", Bool.show(String.contains(web(home()), "Change project icon")), "True")
    C.Check.str("inline inputs carry Enter and Escape handlers", Bool.show(String.contains(web(naming("project", "p")), "rename-no")), "True")

    C.Check.str("default saved board survives the next stale client panel input", Bool.show(U.Ui.panel(A.Actor.merge(home(), U.Home.viewer.default(U.Ui.info_set(home(), "local.home.renderer", "web")), True{}, "project-home-p", False{}))), "True")
    C.Check.str("common Send in a home starts a real project thread", J.Json.get_str(sent(U.Ui.act(U.Ui.scratch_set(home(), "project-home-p", "Keyboard send"), "send", "")), "m", ""), "project.start")
