import Base
import ./check.bend as C
import ../src/core/native-sidebar.bend as N
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/mobile/screen.bend as S
import ../src/mobile/hubs.bend as H

# Many closed rows with actual identities; no source body or fake sentinel.
def add(+n: Nat, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k:
      +id = "t" ++ Nat.show(n)
      +made = M.State.apply(st, M.ThreadCreated{id, "p", "Unrendered row " ++ Nat.show(n), M.Local{}, "", "", "codex", "", 2n})
      +settled = M.State.apply(made, M.ThreadSettled{id, True{}, 3n})
      add(k, M.State.apply(settled, M.ThreadArchived{id, Nat.is_eq(Nat.mod(n, 2n), 0n), 4n}))

def state() -> M.State:
  add(192n, M.State.apply(M.State.empty(), M.ProjectCreated{"p", "Large project", "/tmp/native-page", 1n}))

def base() -> U.Ui:
  U.Ui.scratch_set(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), state()), "t1"), "t1", "Unsent 日本語 IME")

def request() -> J.Json:
  J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"hub", J.Str{"h"}}, J.KV{"generation", J.Str{"1"}}, J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t1"}}])}])

def native(u: U.Ui) -> U.Ui:
  N.Ns.prepare(u, False{}, request(), "epoch", "view", 7n, "00112233445566778899aabbccddeeff")

def screen(u: U.Ui) -> J.Json:
  S.Mob.screen.f(True{}, u)

def project(u: U.Ui) -> J.Json:
  Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(screen(u), "projects"))), J.Null{})

def count(j: J.Json) -> String:
  Nat.show(List.length(&2, J.Json, J.Json.elems(j)))

def open() -> U.Ui:
  native(U.Ui.flag_on(base(), "p"))

def go_next(+u: U.Ui) -> U.Ui:
  N.Ns.accept(u, "h|" ++ J.Json.get_str(J.Json.get_obj(J.Json.get_obj(project(u), "settledPage"), "next"), "value", ""))

def child_state() -> M.State:
  M.State.replay([M.ThreadParentSet{"t1", "t3", 5n}, M.ThreadParentSet{"t3", "t5", 5n}, M.ThreadParentSet{"t5", "t7", 5n}, M.ThreadParentSet{"t7", "t9", 5n}], state())

def child_ui() -> U.Ui:
  native(U.Ui.flag_on(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), child_state()), "t1"), "p"))

def child_page() -> J.Json:
  project(N.Ns.reduce(N.Ns.reduce(N.Ns.reduce(N.Ns.reduce(child_ui())))))

def filtered() -> U.Ui:
  native(U.Ui.scratch_set(U.Ui.adopt.raw(base(), M.State.apply(state(), M.ProjectCreated{"q", "Other match", "/tmp/q", 1n})), "@f.pq", "Other"))

def projects_pager(u: U.Ui) -> J.Json:
  J.Json.get_obj(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(screen(u), "sidebarPages"))), J.Null{}), "projects")

def home(+u: U.Ui) -> U.Ui:
  N.Ns.prepare(u, False{}, N.Obj.put(request(), "target", J.Json.obj([J.KV{"kind", J.Str{"recent"}}, J.KV{"id", J.Str{""}}])), "epoch", "view", 7n, "00112233445566778899aabbccddeeff")

def fresh_home() -> U.Ui:
  home(U.Ui.adopt.raw(U.Ui.init(10n), state()))

def dangling_home() -> U.Ui:
  home(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), state()), "missing-thread"))

def deleted_home() -> U.Ui:
  home(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), M.State.apply(state(), M.ThreadDeleted{"t1", 5n})), "t1"))

def removed_project_home() -> U.Ui:
  home(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), M.State.apply(state(), M.ProjectRemoved{"p", 5n})), "t1"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("legacy renderer remains unnegotiated", J.Json.show(J.Json.get_obj(screen(base()), "nativeSidebar")), "null")
    C.Check.str("new native marker explicit", Nat.show(J.Json.get_nat(screen(native(base())), "nativeSidebar", 0n)), "1")
    C.Check.str("closed settled retains authoritative96 count", Nat.show(J.Json.get_nat(project(native(base())), "settledCount", 0n)), "96")
    C.Check.str("closed archive retains authoritative96 count", Nat.show(J.Json.get_nat(project(native(base())), "archivedCount", 0n)), "96")
    C.Check.str("closed settled constructs no row JSON", count(J.Json.get_obj(project(native(base())), "settled")), "0")
    C.Check.str("closed archive constructs no row JSON", count(J.Json.get_obj(project(native(base())), "archived")), "0")
    C.Check.str("closed settled pager is null", J.Json.show(J.Json.get_obj(project(native(base())), "settledPage")), "null")
    C.Check.str("closed archive pager is null", J.Json.show(J.Json.get_obj(project(native(base())), "archivedPage")), "null")
    C.Check.str("closed projection has no hidden title work", Bool.show(String.contains(J.Json.show(J.Json.get_obj(screen(native(base())), "projects")), "Unrendered row")), "False")
    C.Check.str("open shelf stays globally bounded with selected extra", Bool.show(Nat.is_le(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(project(open()), "settled"))), 49n)), "True")
    C.Check.str("open shelf offers actual real rows", Bool.show(String.contains(J.Json.show(J.Json.get_obj(project(open()), "settled")), "Unrendered row")), "True")
    C.Check.str("next shelf page moves forward without selection mutation", U.Ui.sel(go_next(open())), "t1")
    C.Check.str("next shelf page preserves local IME draft", U.Ui.scratch(go_next(open()), "t1"), "Unsent 日本語 IME")
    C.Check.str("next shelf page has positive source offset", Bool.show(Nat.is_gt(J.Json.get_nat(J.Json.get_obj(project(go_next(open())), "settledPage"), "offset", 0n), 0n)), "True")
    C.Check.str("private HMAC nonce never reaches screen", Bool.show(String.contains(J.Json.show(screen(open())), "00112233445566778899aabbccddeeff")), "False")
    C.Check.str("read-only page routing added after47 existing routes", Nat.show(H.Hubs.routed("native-sidebar-page")), "47")
    C.Check.str("route cardinality includes new read-only action", Nat.show(H.Hubs.routes()), "48")
    C.Check.str("selected descendant outside ordinary range stays present", Bool.show(String.contains(J.Json.show(J.Json.get_obj(child_page(), "settled")), "\"id\":\"t1\"")), "True")
    C.Check.str("selected descendant exception is declared", Bool.show(J.Json.get_bool(J.Json.get_obj(child_page(), "settledPage"), "selectedOutside", False{})), "True")
    C.Check.str("pressure keeps truthful total and coherent small range", Nat.show(J.Json.get_nat(J.Json.get_obj(child_page(), "settledPage"), "limit", 0n)), "3")
    C.Check.str("selected nonmatching project does not inflate filtered total", Nat.show(J.Json.get_nat(projects_pager(filtered()), "total", 0n)), "1")
    C.Check.str("selected nonmatching project is declared separate exception", Bool.show(J.Json.get_bool(projects_pager(filtered()), "selectedOutside", False{})), "True")
    C.Check.str("filtered ordinary and selected project both reachable", count(J.Json.get_obj(screen(filtered()), "projects")), "2")
    C.Check.str("zero-match filter keeps empty project source", count(J.Json.get_obj(screen(native(U.Ui.scratch_set(base(), "@f.pq", "no matching title"))), "projects")), "0")
    C.Check.str("fresh recent Home never promises nonexistent selected project", Bool.show(J.Json.get_bool(projects_pager(fresh_home()), "selectedOutside", False{})), "False")
    C.Check.str("fresh Home exact project array matches ordinary count", count(J.Json.get_obj(screen(fresh_home()), "projects")), Nat.show(Nat.min(J.Json.get_nat(projects_pager(fresh_home()), "limit", 0n), J.Json.get_nat(projects_pager(fresh_home()), "total", 0n))))
    C.Check.str("dangling selection never promises nonexistent selected project", Bool.show(J.Json.get_bool(projects_pager(dangling_home()), "selectedOutside", False{})), "False")
    C.Check.str("deleted selection never promises nonexistent selected project", Bool.show(J.Json.get_bool(projects_pager(deleted_home()), "selectedOutside", False{})), "False")
    C.Check.str("removed canonical project never promises selected exception", Bool.show(J.Json.get_bool(projects_pager(removed_project_home()), "selectedOutside", False{})), "False")
