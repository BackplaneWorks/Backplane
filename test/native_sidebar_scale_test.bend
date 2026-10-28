import Base
import ./check.bend as C
import ../src/core/native-sidebar.bend as N
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/mobile/screen.bend as S

# Twelve projects, 1440 real metadata rows, four open shelf kinds. Each
# label is large enough that row-count bounds alone exceed the byte gate.
def threads(+n: Nat, +project: String, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k:
      +id = project ++ "t" ++ Nat.show(n)
      +made = M.State.apply(st, M.ThreadCreated{id, project, String.repeat("Long canonical title ", 180n) ++ id, M.Local{}, "", "", "codex", "", 2n})
      +done = M.State.apply(made, M.ThreadSettled{id, Nat.is_lt(Nat.mod(n, 4n), 2n), 3n})
      +archive = M.State.apply(done, M.ThreadArchived{id, Nat.is_eq(Nat.mod(n, 4n), 0n), 4n})
      threads(k, project, M.State.apply(archive, M.ThreadSnoozed{id, Bool.pick(Nat, Nat.is_eq(Nat.mod(n, 4n), 3n), 9999n, 0n), 5n}))

def projects(+n: Nat, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k:
      +id = "p" ++ Nat.show(n)
      projects(k, threads(120n, id, M.State.apply(st, M.ProjectCreated{id, "Project " ++ id, "/tmp/native-scale/" ++ id, 1n})))

def opened(+n: Nat, u: U.Ui) -> U.Ui:
  match n:
    case 0n: u
    case 1n+k:
      +id = "p" ++ Nat.show(n)
      opened(k, U.Ui.flag_on(U.Ui.flag_on(u, id), "arch:" ++ id))

def ui() -> U.Ui:
  N.Ns.prepare(opened(12n, U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), projects(12n, M.State.empty())), "p1t1")), False{},
    J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"hub", J.Str{"h"}}, J.KV{"generation", J.Str{"1"}}, J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"p1t1"}}])}]),
    "epoch", "view", 7n, "00112233445566778899aabbccddeeff")

def screen(u: U.Ui) -> J.Json:
  S.Mob.screen.f(True{}, u)

def rows(ps: List<&2, J.Json>) -> Nat:
  match ps:
    case Nil{}: 0n
    case +p <> rest:
      Nat.add(Nat.add(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(p, "threads"))), List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(p, "snoozed")))),
        Nat.add(Nat.add(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(p, "settled"))), List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(p, "archived")))), rows(rest)))

def physical(+s: J.Json) -> Nat:
  Nat.add(rows(J.Json.elems(J.Json.get_obj(s, "projects"))), Nat.add(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(s, "active"))), List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(s, "legacy")))))

def pressure() -> U.Ui:
  N.Ns.reduce(N.Ns.reduce(N.Ns.reduce(N.Ns.reduce(ui()))))

def main() -> IO(Unit):
  do IO<Unit>:
    +initial : J.Json = screen(ui())
    +fitted : J.Json = screen(pressure())
    C.Check.str("many open lists share global48 rows plus selected exception", Bool.show(Nat.is_le(physical(initial), 49n)), "True")
    C.Check.str("project page and selected exception never exceed9", Bool.show(Nat.is_le(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(initial, "projects"))), 9n)), "True")
    C.Check.str("ordinary long-label projection exceeds duplicated budget", Bool.show(Nat.is_gt(Nat.mul(2n, J.Json.bytes(initial)), 262144n)), "True")
    C.Check.str("coherent adaptive sidebar fits duplicated logical budget", Bool.show(Nat.is_lt(Nat.mul(2n, J.Json.bytes(fitted)), 262144n)), "True")
    C.Check.str("adaptive planner retains real nonempty open lists", Bool.show(Nat.is_gt(physical(fitted), 0n)), "True")
    C.Check.str("adaptive planner retains reachable project intervals", Bool.show(String.contains(J.Json.show(J.Json.get_obj(fitted, "sidebarPages")), "native-sidebar-page")), "True")
