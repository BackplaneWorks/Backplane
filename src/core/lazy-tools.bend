import Base
import ./model.bend as M
import ./thin.bend as T
import ./fold.bend as F
import ./hist.bend as H
import ./json.bend as J
import ./presentation.bend as P
import ./i18n.bend as I

# This is a body materialization policy, not a new history/cursor unit.
# Keep every raw row and the existing fold context. Only an open tool's
# payload may cross from storage into the selected view's row index.
def Tools.action(fold: Bool, info: Map<&2, String>, +value: String) -> Map<&2, String>:
  match fold:
    case False{}: info
    case True{}: Map.set(&2, String, info, F.Fold.key(String.drop(value, 1n)), String.take(value, 1n))

def Tools.info(+p: J.Json) -> Map<&2, String>:
  +info = P.Session.info.read(J.Json.get_obj(J.Json.get_obj(p, "session"), "info"), Map.new(&2, String))
  Tools.action(String.eq(J.Json.get_str(p, "action", ""), "fold"), info, J.Json.get_str(p, "value", ""))

def Tools.entries(ids: List<&2, String>, +index: T.Index, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match ids:
    case Nil{}: acc
    case id <> rest: Tools.entries(rest, index, T.Row.entry(T.Index.row(index, M.Switch.num(id, 0n))) <> acc)

def Tools.open(role: F.Role) -> Bool:
  match role:
    case F.RShow{}: True{}
    case F.RHead{_, _, open, _}: open
    case F.RIn{open}: open
    case F.RWork{_, _, open, inner}:
      match open:
        case False{}: False{}
        case True{}: Tools.open(inner)

def Tools.names(es: List<&2, M.Entry>, rs: List<&2, F.Role>, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match es rs:
    case e <> tail r <> rest: Tools.names(tail, rest, Map.set(&2, Bool, acc, M.Idx.id(e), Tools.open(r)))
    case _ _: acc

def Tools.context(c: F.Around) -> F.Around:
  match c:
    case F.Around{before, after}: F.Around{H.Hist.seen(before), H.Hist.seen(after)}

def Tools.allowed(+st: M.State, +index: T.Index, +thread: String, +ids: List<&2, String>, p: J.Json) -> Map<&2, Bool>:
  +es = H.Hist.seen(Tools.entries(ids, index, Nil{}))
  +context = Tools.context(F.Fold.context(Nil{}, M.State.mine(st, thread), Nil{}, F.Fold.shown(es)))
  Tools.names(es, F.Fold.roles.all(I.En{}, context, es, Tools.info(p), False{}), Map.new(&2, Bool))

def Tools.tool(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, tone, _, _, _}: String.eq(tone, "tool")
    case _: False{}

def Tools.found(p: Map<&2, Bool> & Bool) -> Bool:
  match p:
    case (_, value): value

def Tools.keep(+e: M.Entry, +allowed: Map<&2, Bool>) -> Bool:
  Bool.or(Bool.not(Tools.tool(e)), Tools.found(Map.get(Bool, False{}, allowed, M.Idx.id(e))))

def Tools.ordinal(open: Bool, id: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match open:
    case False{}: acc
    case True{}: J.Num{id} <> acc

def Tools.ordinals(ids: List<&2, String>, +index: T.Index, +allowed: Map<&2, Bool>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ids:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case +id <> rest:
      +e = T.Row.entry(T.Index.row(index, M.Switch.num(id, 0n)))
      Tools.ordinals(rest, index, allowed, Tools.ordinal(Bool.and(Tools.tool(e), Tools.keep(e, allowed)), id, acc))

def Tools.row(keep: Bool, r: T.Row) -> T.Row:
  match keep r:
    case True{} _: r
    case False{} T.Row{n, e, words, _}: T.Row{n, T.Entry.with_text(e, ""), words, T.Blocks.of("")}

# A cleared row has no detail blocks. Its separately bounded title remains
# available to Fold.what, so a live drawer keeps the same concise label.
def Tools.title.row(keep: Bool, title: String, r: T.Row) -> T.Row:
  match keep r:
    case True{} _: r
    case False{} T.Row{n, e, words, blocks}: T.Row{n, T.Entry.with_text(e, title), words, blocks}

def Tools.metadata.row(+keep: Bool, +r: T.Row) -> T.Row:
  Tools.title.row(keep, F.Fold.line(160n, T.Entry.text(T.Row.entry(r)), ""), Tools.row(keep, r))

def Tools.mask.rows(ids: List<&2, String>, +allowed: Map<&2, Bool>, +thread: String, +index: T.Index) -> T.Index:
  match ids:
    case Nil{}: index
    case id <> rest:
      +r = T.Index.row(index, M.Switch.num(id, 0n))
      Tools.mask.rows(rest, allowed, thread, T.Index.window.put(index, thread, Tools.metadata.row(Tools.keep(T.Row.entry(r), allowed), r)))

def Tools.mask(+st: M.State, +index: T.Index, +thread: String, +ids: List<&2, String>, p: J.Json) -> T.Index:
  Tools.mask.rows(ids, Tools.allowed(st, index, thread, ids, p), thread, index)

def Tools.mask.enabled(on: Bool, st: M.State, index: T.Index, thread: String, ids: List<&2, String>, p: J.Json) -> T.Index:
  match on:
    case False{}: index
    case True{}: Tools.mask(st, index, thread, ids, p)
