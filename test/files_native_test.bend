import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/app/nui.bend as N

def head(origin: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"since", J.Num{"0"}}, J.KV{"origin", J.Str{origin}}, J.KV{"info", J.Json.obj(Nil{})}, J.KV{"items", J.Json.arr(Nil{})}])

def reply(+id: Nat, path: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{Nat.show(id)}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"fmDir", J.Str{path}}, J.KV{"fmEntries", J.Str{""}}])}])

def cmds(s: N.Step) -> List<&2, N.NCmd>:
  match s:
    case N.Step{_, cs, _}:
      cs

def texts(cs: List<&2, N.NCmd>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case N.NSend{t} <> rest:
      Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}) <> texts(rest)
    case _ <> rest:
      texts(rest)

def only.keep(keep: Bool, j: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      j <> rest
    case False{}:
      rest

def only(xs: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +j <> rest:
      only.keep(String.eq(J.Json.get_str(j, "m", ""), "files.dir"), j, only(rest))

def files(s: N.Step) -> String:
  J.Json.show(J.Json.arr(only(texts(cmds(s)))))

# In-process windows receive a read model once on joining, through
# Nui.adopt.step. Ordinary updates and replies still use Nui.hear.
def base() -> U.Ui:
  U.Ui.select.k(U.Ui.info_set(U.Ui.fold([
    M.ProjectCreated{"p1", "Example", "/tmp/files-native-example", 1n},
    M.ThreadCreated{"t1", "p1", "Files", M.Local{}, "", "", "claude", "", 2n}], U.Ui.init(10n)), "origin", "files-native-origin"), "t1")

def loaded(+path: String) -> U.Ui:
  U.Ui.recv(U.Act.ui(U.Fm.ask(base(), path)), reply(0n, path))

def ui(s: N.Step) -> U.Ui:
  N.Nui.ui(N.Step.nui(s))

def adopt(path: String) -> N.Step:
  N.Nui.adopt.step(N.Nui.init(loaded(path), 800, 600), U.Ui.state.of(base()), head("files-native-origin"))

def methods(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case j <> rest:
      J.Json.get_str(j, "m", "") <> methods(rest)

# Closed scenarios keep large client states out of IO continuation
# captures in the native backend; every assertion still uses the same
# state and receive sequence.
def Scenario.root() -> N.Step:
  adopt("")

def Scenario.child() -> N.Step:
  adopt("src")

def Scenario.answered() -> N.Step:
  N.Nui.hear(N.Step.nui(Scenario.root()), reply(1n, ""))

def Scenario.changes() -> N.Step:
  N.Nui.hear(N.Step.nui(Scenario.answered()), J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr(Nil{})}]))

def Scenario.info() -> N.Step:
  N.Nui.hear(N.Step.nui(Scenario.answered()), J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj(Nil{})}]))

def Scenario.queued() -> U.Ui:
  U.Act.ui(U.Ui.rpc.keep(loaded(""), "turn.start", [J.KV{"thread", J.Str{"t1"}}, J.KV{"text", J.Str{"queued"}}, J.KV{"msg", J.Str{"offline-1"}}]))

def Scenario.repeated() -> N.Step:
  N.Nui.adopt.step(N.Step.nui(Scenario.root()), U.Ui.state.of(base()), head("files-native-origin"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("native adoption refreshes a retained project root", files(Scenario.root()), "[{\"id\":1,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"\"}}]")
    C.Check.str("native adoption refreshes a retained child folder", files(Scenario.child()), "[{\"id\":1,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"src\"}}]")
    C.Check.str("native adoption waits for the new listing", Bool.show(U.Fm.ready(ui(Scenario.root()))), "False")
    C.Check.str("native adoption keeps the new request owner", U.Ui.Scenario.info()(ui(Scenario.root()), "fm.req:1"), "t1")
    C.Check.str("native adoption advances the request counter", Nat.show(U.Ui.rid(ui(Scenario.root()))), "2")
    C.Check.str("native adopted root accepts an empty reply", Bool.show(U.Fm.ready(ui(Scenario.answered()))), "True")
    C.Check.str("native adopted child accepts an empty reply", Bool.show(U.Fm.ready(ui(N.Nui.hear(N.Step.nui(Scenario.child()), reply(1n, "src"))))), "True")
    C.Check.str("native Files reply causes no relist loop", files(Scenario.answered()), "[]")
    C.Check.str("native ordinary changes cause no relist loop", files(Scenario.changes()), "[]")
    C.Check.str("native info update causes no relist loop", files(Scenario.info()), "[]")
    C.Check.str("native unused Files tab is not requested", files(N.Nui.adopt.step(N.Nui.init(base(), 800, 600), U.Ui.state.of(base()), head("files-native-origin"))), "[]")
    C.Check.str("native different origin does not inherit Files requests", files(N.Nui.adopt.step(N.Nui.init(loaded(""), 800, 600), U.Ui.state.of(base()), head("another-origin"))), "[]")
    C.Check.str("native rejoin uses a fresh Files request id", files(Scenario.repeated()), "[{\"id\":2,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"\"}}]")
    C.Check.str("native adoption replays outbox before new Files requests", String.join(methods(texts(cmds(N.Nui.adopt.step(N.Nui.init(Scenario.queued(), 800, 600), U.Ui.state.of(base()), head("files-native-origin"))))), "|"), "turn.start|files.dir")
