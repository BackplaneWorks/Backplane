import Base
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U

# The differential oracle: events.jsonl through Backplane's own State.replay, printed as a
# canonical state JSON. Rust prints the same shape from its fold (see
# test/gpui/parity/oracle/DIGEST.md); the harness sorts both and compares SHA-256.
#   digest CHUNK1 [CHUNK2 ...]   (files of whole lines, folded in order)
# Base IO only.

def Dg.turn(+t: M.TurnState) -> String:
  match t:
    case M.Idle{}: "idle"
    case M.Running{}: "running"
    case M.Completed{}: "completed"
    case M.Interrupted{}: "interrupted"
    case M.Failed{}: "failed"

def Dg.entry.id(+e: M.Entry) -> J.Json:
  match e:
    case M.Msg{id, _, _, _, _}: J.Str{id}
    case M.Act{id, _, _, _, _, _}: J.Str{id}

def Dg.ids(+es: List<&2, M.Entry>) -> List<&2, J.Json>:
  match es:
    case Nil{}: Nil{}
    case e <> t: Dg.entry.id(e) <> Dg.ids(t)

def Dg.project(+p: M.Project) -> J.Json:
  match p:
    case M.Project{id, title, root, _, removed}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"title", J.Str{title}}, J.KV{"root", J.Str{root}}, J.KV{"removed", J.Json.bool(removed)}])

def Dg.projects(+xs: List<&2, Sigma<&2, &2, String, _ => M.Project>>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case (_, p) <> t: Dg.project(p) <> Dg.projects(t)

def Dg.thread(+st: M.State, +t: M.Thread) -> J.Json:
  match t:
    case M.Thread{id, project, title, _, _, turn, pending, archived, _, _, _, pinned, _, _, _, provider, model, _, _, session, deleted}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"project", J.Str{project}}, J.KV{"title", J.Str{title}}, J.KV{"turn", J.Str{Dg.turn(turn)}},
        J.KV{"pending", J.Json.nat(pending)}, J.KV{"archived", J.Json.bool(archived)}, J.KV{"pinned", J.Json.bool(pinned)}, J.KV{"deleted", J.Json.bool(deleted)},
        J.KV{"provider", J.Str{provider}}, J.KV{"model", J.Str{model}}, J.KV{"session", J.Str{session}},
        J.KV{"entries", J.Json.arr(Dg.ids(M.State.mine(st, id)))}])

def Dg.threads(+st: M.State, +xs: List<&2, Sigma<&2, &2, String, _ => M.Thread>>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case (_, t) <> r: Dg.thread(st, t) <> Dg.threads(st, r)

def Dg.settings(+xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case (k, v) <> t: J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"value", J.Str{v}}]) <> Dg.settings(t)

def Dg.state(+st: M.State) -> J.Json:
  match st:
    case M.State{ps, ts, _, seq, _, sg, _, _, _, _, _}:
      J.Json.obj([J.KV{"projects", J.Json.arr(Dg.projects(Map.to_list(&2, M.Project, ps)))},
        J.KV{"threads", J.Json.arr(Dg.threads(st, Map.to_list(&2, M.Thread, ts)))},
        J.KV{"settings", J.Json.arr(Dg.settings(Map.to_list(&2, String, sg)))}, J.KV{"seq", J.Json.nat(seq)}])

# ---- reading ----

def Dg.item(m: Maybe<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case None{}: acc
    case Some{j}: j <> acc

def Dg.items(xs: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case x <> t: Dg.items(t, Dg.item(J.Json.parse(x), acc))

def Dg.apply.d(+d: U.Decoded, +st: M.State) -> M.State:
  match d:
    case U.Decoded{cs, _}: M.State.replay(List.reverse(&2, M.Change, cs), st)

def Dg.apply(+st: M.State, js: List<&2, J.Json>) -> M.State:
  Dg.apply.d(U.Ui.decode.all(js, U.Decoded{Nil{}, 0n}), st)

def Dg.read.k(r: File & Result<&1, &1, U32 & String, String>) -> IO(String):
  (f, res) = r
  do IO<String>:
    File.close(f)
    s : String <- IO.pass(String, res)
    return s

def Dg.read(+path: String) -> IO(String):
  do IO<String>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, String> <- File.read(f, 2000000000)
    Dg.read.k(r)

# the files folded in the order given (the harness cuts a big log on line boundaries)
def Dg.files(+paths: List<&2, String>, +st: M.State) -> IO(M.State):
  match paths:
    case Nil{}: IO.pure(M.State, st)
    case p <> t:
      do IO<M.State>:
        text : String <- Dg.read(p)
        Dg.files(t, Dg.apply(st, Dg.items(String.lines(text), Nil{})))

def Dg.copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> Dg.copy(t)

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<&1, String> <- IO.args()
    +xs : List<&2, String> = Dg.copy(a)
    st : M.State <- Dg.files(List.drop(&2, String, xs, 1n), M.State.empty())
    IO.print(J.Json.show(Dg.state(st)))
