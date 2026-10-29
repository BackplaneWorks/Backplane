import Base
import ../core/view-page.bend as P
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/subthreads.bend as ST
import ../core/subs.bend as SB
import ../core/client.bend as U
import ./dom.bend as D

# A list's counts and source stay whole. Only its bounded selected range
# constructs nodes; the current selected item remains reachable separately.
def Lists.button(+show: Bool, +scope: String, +offset: Nat, +label: String) -> List<&2, D.Node>:
  match show:
    case False{}: Nil{}
    case True{}: [D.Dom.button("shelf", "list-page", J.Json.show(J.Json.obj([J.KV{"scope", J.Str{scope}}, J.KV{"offset", J.Json.nat(offset)}])), label, label)]

def Lists.pager(+p: P.Page) -> List<&2, D.Node>:
  match p:
    case P.Page{+scope, +total, +start, +limit, _}:
      List.append(&2, D.Node, Lists.button(Bool.and(Bool.not(String.is_empty(scope)), Nat.is_gt(start, 0n)), scope, Nat.sub(start, limit), "Previous"),
        Lists.button(Bool.and(Bool.not(String.is_empty(scope)), Nat.is_lt(Nat.add(start, limit), total)), scope, Nat.add(start, limit), "Next · " ++ Nat.show(total) ++ " total"))

def Lists.keep(-A: Data, +show: Bool, +item: A, +rest: List<&2, A>) -> List<&2, A>:
  match show:
    case True{}: item <> rest
    case False{}: rest

def Lists.tree(+rs: List<&2, ST.TRow>, +ids: List<&2, String>) -> List<&2, ST.TRow>:
  match rs:
    case Nil{}: Nil{}
    case ST.TRow{+t, d, o, c} <> rest:
      Lists.keep(ST.TRow, P.Page.has(ids, M.Thread.id(t)), ST.TRow{t, d, o, c}, Lists.tree(rest, ids))

def Lists.project.id(+r: U.SRow) -> String:
  match r:
    case U.SRow{_, M.Project{id, _, _, _, _}, _}: id

def Lists.projects(+rs: List<&2, U.SRow>, +ids: List<&2, String>) -> List<&2, U.SRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest: Lists.keep(U.SRow, P.Page.has(ids, Lists.project.id(r)), r, Lists.projects(rest, ids))

def Lists.subs(+rs: List<&2, SB.SubRow>, +ids: List<&2, String>) -> List<&2, SB.SubRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest: Lists.keep(SB.SubRow, P.Page.has(ids, SB.Subs.key(r)), r, Lists.subs(rest, ids))
