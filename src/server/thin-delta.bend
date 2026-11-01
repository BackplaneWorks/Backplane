import Base
import ../core/json.bend as J
import ./thin-content-eq.bend as E

# A view's keyed delta (hub view protocol H). The hub keeps the last bodies
# it sent a view ({shown, session, window, overlay}); a client that holds
# one of them (by its token) gets only what changed: a list of ops that
# turns that base into the new body. The client applies them copy on write
# (src/web/view-delta.js), so every untouched subtree stays the base's own
# object. Delta.apply is the oracle the client's applier is tested against
# (test/view_delta_test.bend), and the laws hold it exact.
#
# The walk prunes with ContentEq.same (native on the hub): an equal subtree
# costs one comparison and no op. Objects with the same keys in the same
# order are diffed field by field, any other object is set whole. Arrays
# whose elements keep their identities in place (the "key" of a web node,
# else the "id" of a phone's entry, else none) are diffed element by
# element; otherwise, when every new element has its own identity, the
# array is rebuilt from the old elements by identity (moved rows, rows in
# and out) and each reused one is diffed in its new place; else from the
# common prefix and suffix, the middle new.
#
# Ops, in the order they apply (an array's rebuild before anything inside
# it, paths in the new tree's indexes):
#   ["s", path, value]   set the value at path ([] is the whole body)
#   ["a", path, specs]   rebuild the array at path; a spec is [0, i] (the
#                        base array's i-th element) or [1, value]
#   ["t", path]          set the request's own draft text at path

type Step is Data:
  DKey{name: String}
  DAt{index: Nat}

type Spec is Data:
  DOld{index: Nat}
  DNew{value: J.Json}

type Op is Data:
  DSet{path: List<&2, Step>, value: J.Json}
  DRebuild{path: List<&2, Step>, specs: List<&2, Spec>}

# a pair still to compare; at is its path, innermost step first
type Task is Data:
  DTask{old: J.Json, new: J.Json, at: List<&2, Step>}

type Stepped is Data:
  DStepped{work: List<&2, Task>, ops: List<&2, Op>}

type Pick is Data:
  DPick{spec: Spec, tasks: List<&2, Task>}

# Identity
# --------

def Delta.hit(+fs: J.Json, +key: String) -> Bool:
  match fs:
    case J.Field{+k, _, _}: String.eq(k, key)
    case _: False{}

def Delta.field.go(fs: J.Json, hit: Bool, +key: String) -> J.Json:
  match fs hit:
    case J.Field{_, +v, _} True{}: v
    case J.Field{_, _, +tail} False{}: Delta.field.go(tail, Delta.hit(tail, key), key)
    case _ _: J.Null{}

def Delta.field(+fs: J.Json, +key: String) -> J.Json:
  Delta.field.go(fs, Delta.hit(fs, key), key)

def Delta.str(+j: J.Json) -> String:
  match j:
    case J.Str{+s}: s
    case _: ""

def Delta.id.pick(nokey: Bool, +key: String, +id: String) -> String:
  match nokey:
    case False{}: "k:" ++ key
    case True{}: Bool.pick(String, String.is_empty(id), "", "i:" ++ id)

# "k:<key>", "i:<id>", or "" for an element with neither
def Delta.id(+j: J.Json) -> String:
  match j:
    case J.Obj{+fs}:
      +key = Delta.str(Delta.field(fs, "key"))
      Delta.id.pick(String.is_empty(key), key, Delta.str(Delta.field(fs, "id")))
    case _: ""

def Delta.ids(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> tail: Delta.id(x) <> Delta.ids(tail)

def Delta.ids.same(a: List<&2, String>, b: List<&2, String>, ok: Bool) -> Bool:
  match a b ok:
    case +x <> +at +y <> +bt True{}: Delta.ids.same(at, bt, String.eq(x, y))
    case Nil{} Nil{} True{}: True{}
    case _ _ _: False{}

def Delta.has(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +x <> tail: Bool.or(String.eq(x, s), Delta.has(tail, s))

# every id there and none twice
def Delta.ids.keyed(xs: List<&2, String>, +seen: List<&2, String>, ok: Bool) -> Bool:
  match xs ok:
    case +x <> +tail True{}:
      Delta.ids.keyed(tail, x <> seen, Bool.and(Bool.not(String.is_empty(x)), Bool.not(Delta.has(seen, x))))
    case Nil{} True{}: True{}
    case _ _: False{}

def Delta.find(xs: List<&2, String>, +s: String, +i: Nat, found: Bool) -> Maybe<&2, Nat>:
  match xs found:
    case _ True{}: Some{Nat.sub(i, 1n)}
    case +x <> +tail False{}: Delta.find(tail, s, Nat.add(i, 1n), String.eq(x, s))
    case _ False{}: None{}

# Objects
# -------

def Delta.keys.same(a: J.Json, b: J.Json, ok: Bool) -> Bool:
  match a b ok:
    case J.Field{+k, _, +at} J.Field{+l, _, +bt} True{}: Delta.keys.same(at, bt, String.eq(k, l))
    case J.End{} J.End{} True{}: True{}
    case _ _ _: False{}

def Delta.fields(a: J.Json, b: J.Json, +at: List<&2, Step>) -> List<&2, Task>:
  match a b:
    case J.Field{+k, +x, +xt} J.Field{_, +y, +yt}: DTask{x, y, DKey{k} <> at} <> Delta.fields(xt, yt, at)
    case _ _: Nil{}

def Delta.obj(same: Bool, +a: J.Json, +b: J.Json, +at: List<&2, Step>, +rest: List<&2, Task>, +ops: List<&2, Op>) -> Stepped:
  match same:
    case True{}: DStepped{List.append(&2, Task, Delta.fields(a, b, at), rest), ops}
    case False{}: DStepped{rest, DSet{List.reverse(&2, Step, at), J.Obj{b}} <> ops}

# Arrays
# ------

def Delta.zip(a: List<&2, J.Json>, b: List<&2, J.Json>, +at: List<&2, Step>, +i: Nat) -> List<&2, Task>:
  match a b:
    case +x <> +xt +y <> +yt: DTask{x, y, DAt{i} <> at} <> Delta.zip(xt, yt, at, Nat.add(i, 1n))
    case _ _: Nil{}

def Delta.pick(found: Maybe<&2, Nat>, +x: J.Json, +olds: List<&2, J.Json>, +j: Nat, +at: List<&2, Step>) -> Pick:
  match found:
    case Some{+i}: DPick{DOld{i}, [DTask{Maybe.default(&2, J.Json, List.get(&2, J.Json, olds, i), J.Null{}), x, DAt{j} <> at}]}
    case None{}: DPick{DNew{x}, Nil{}}

def Delta.picks(news: List<&2, J.Json>, +j: Nat, +oldids: List<&2, String>, +olds: List<&2, J.Json>, +at: List<&2, Step>) -> List<&2, Pick>:
  match news:
    case Nil{}: Nil{}
    case +x <> tail:
      Delta.pick(Delta.find(oldids, Delta.id(x), 0n, False{}), x, olds, j, at) <> Delta.picks(tail, Nat.add(j, 1n), oldids, olds, at)

def Delta.picks.specs(ps: List<&2, Pick>) -> List<&2, Spec>:
  match ps:
    case Nil{}: Nil{}
    case DPick{+s, _} <> tail: s <> Delta.picks.specs(tail)

def Delta.picks.tasks(ps: List<&2, Pick>, +rest: List<&2, Task>) -> List<&2, Task>:
  match ps:
    case Nil{}: rest
    case DPick{_, +ts} <> tail: List.append(&2, Task, ts, Delta.picks.tasks(tail, rest))

def Delta.head.eq(+a: List<&2, J.Json>, +b: List<&2, J.Json>) -> Bool:
  match a b:
    case +x <> _ +y <> _: E.ContentEq.same(x, y)
    case _ _: False{}

def Delta.prefix(a: List<&2, J.Json>, b: List<&2, J.Json>, same: Bool, +n: Nat) -> Nat:
  match a b same:
    case _ <> +at _ <> +bt True{}: Delta.prefix(at, bt, Delta.head.eq(at, bt), Nat.add(n, 1n))
    case _ _ _: n

def Delta.olds(n: Nat, +from: Nat) -> List<&2, Spec>:
  match n:
    case 0n: Nil{}
    case 1n+k: DOld{from} <> Delta.olds(k, Nat.add(from, 1n))

def Delta.news(xs: List<&2, J.Json>) -> List<&2, Spec>:
  match xs:
    case Nil{}: Nil{}
    case +x <> tail: DNew{x} <> Delta.news(tail)

def Delta.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

# the common prefix and suffix kept, the middle new
def Delta.unkeyed(+olds: List<&2, J.Json>, +news: List<&2, J.Json>) -> List<&2, Spec>:
  +lo = List.length(&2, J.Json, olds)
  +ln = List.length(&2, J.Json, news)
  +p = Delta.prefix(olds, news, Delta.head.eq(olds, news), 0n)
  +ro = List.reverse(&2, J.Json, olds)
  +rn = List.reverse(&2, J.Json, news)
  +s = Delta.min(Delta.prefix(ro, rn, Delta.head.eq(ro, rn), 0n), Delta.min(Nat.sub(lo, p), Nat.sub(ln, p)))
  List.append(&2, Spec, Delta.olds(p, 0n), List.append(&2, Spec,
    Delta.news(List.take(&2, J.Json, List.drop(&2, J.Json, news, p), Nat.sub(Nat.sub(ln, p), s))), Delta.olds(s, Nat.sub(lo, s))))

def Delta.arr.k(pairwise: Bool, keyed: Bool, +olds: List<&2, J.Json>, +news: List<&2, J.Json>, +oldids: List<&2, String>,
    +at: List<&2, Step>, +rest: List<&2, Task>, +ops: List<&2, Op>) -> Stepped:
  match pairwise keyed:
    case True{} _: DStepped{List.append(&2, Task, Delta.zip(olds, news, at, 0n), rest), ops}
    case False{} True{}:
      +ps = Delta.picks(news, 0n, oldids, olds, at)
      DStepped{Delta.picks.tasks(ps, rest), DRebuild{List.reverse(&2, Step, at), Delta.picks.specs(ps)} <> ops}
    case False{} False{}:
      DStepped{rest, DRebuild{List.reverse(&2, Step, at), Delta.unkeyed(olds, news)} <> ops}

def Delta.arr(+olds: List<&2, J.Json>, +news: List<&2, J.Json>, +at: List<&2, Step>, +rest: List<&2, Task>, +ops: List<&2, Op>) -> Stepped:
  +oldids = Delta.ids(olds)
  +newids = Delta.ids(news)
  Delta.arr.k(Delta.ids.same(oldids, newids, True{}), Delta.ids.keyed(newids, Nil{}, True{}), olds, news, oldids, at, rest, ops)

# The walk
# --------

def Delta.kind(+o: J.Json, +n: J.Json, +at: List<&2, Step>, +rest: List<&2, Task>, +ops: List<&2, Op>) -> Stepped:
  match o n:
    case J.Obj{+a} J.Obj{+b}: Delta.obj(Delta.keys.same(a, b, True{}), a, b, at, rest, ops)
    case J.Arr{+a} J.Arr{+b}: Delta.arr(J.Json.elems(J.Arr{a}), J.Json.elems(J.Arr{b}), at, rest, ops)
    case _ _: DStepped{rest, DSet{List.reverse(&2, Step, at), n} <> ops}

def Delta.node(same: Bool, +o: J.Json, +n: J.Json, +at: List<&2, Step>, +rest: List<&2, Task>, +ops: List<&2, Op>) -> Stepped:
  match same:
    case True{}: DStepped{rest, ops}
    case False{}: Delta.kind(o, n, at, rest, ops)

def Delta.step(s: Stepped) -> Stepped:
  match s:
    case DStepped{Nil{}, +ops}: DStepped{Nil{}, ops}
    case DStepped{DTask{+o, +n, +at} <> +rest, +ops}: Delta.node(E.ContentEq.same(o, n), o, n, at, rest, ops)

def Delta.done(+s: Stepped) -> Bool:
  match s:
    case DStepped{Nil{}, _}: True{}
    case _: False{}

def Delta.ops(+s: Stepped) -> List<&2, Op>:
  match s:
    case DStepped{_, +ops}: List.reverse(&2, Op, ops)

# None: the walk ran out of fuel (send the body whole)
def Delta.go(fuel: Nat, +s: Stepped, done: Bool) -> Maybe<&2, List<&2, Op>>:
  match fuel done:
    case _ True{}: Some{Delta.ops(s)}
    case 0n False{}: None{}
    case 1n+k False{}:
      +next = Delta.step(s)
      Delta.go(k, next, Delta.done(next))

def Delta.fuel() -> Nat:
  4000000n

def Delta.walk(+old: J.Json, +new: J.Json) -> Maybe<&2, List<&2, Op>>:
  +s : Stepped = DStepped{[DTask{old, new, Nil{}}], Nil{}}
  Delta.go(Delta.fuel(), s, Delta.done(s))

# no ops only when the bodies are the same: a walk of unequal bodies that
# found nothing (it cannot) sends the body whole
def Delta.guard(m: Maybe<&2, List<&2, Op>>) -> Maybe<&2, List<&2, Op>>:
  match m:
    case Some{Nil{}}: None{}
    case Some{+op <> +tail}: Some{op <> tail}
    case None{}: None{}

def Delta.empty(m: Maybe<&2, List<&2, Op>>) -> Bool:
  match m:
    case Some{Nil{}}: True{}
    case _: False{}

def Delta.diff.k(same: Bool, +old: J.Json, +new: J.Json) -> Maybe<&2, List<&2, Op>>:
  match same:
    case True{}: Some{Nil{}}
    case False{}: Delta.guard(Delta.walk(old, new))

# the ops from old to new; Some{[]} exactly when they are equal (laws
# view_delta_same, view_delta_empty_same); None: send new whole
def Delta.diff(+old: J.Json, +new: J.Json) -> Maybe<&2, List<&2, Op>>:
  Delta.diff.k(E.ContentEq.same(old, new), old, new)

# The wire
# --------

def Delta.path.json(ps: List<&2, Step>) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case DKey{+k} <> tail: J.Str{k} <> Delta.path.json(tail)
    case DAt{+i} <> tail: J.Json.nat(i) <> Delta.path.json(tail)

def Delta.spec.json(s: Spec) -> J.Json:
  match s:
    case DOld{+i}: J.Json.arr([J.Json.nat(0n), J.Json.nat(i)])
    case DNew{+v}: J.Json.arr([J.Json.nat(1n), v])

def Delta.specs.json(ss: List<&2, Spec>) -> List<&2, J.Json>:
  match ss:
    case Nil{}: Nil{}
    case +s <> tail: Delta.spec.json(s) <> Delta.specs.json(tail)

# a set of the very draft the request carried goes as ["t", path]: the
# client puts back the text it sent, so a draft echo carries none of it
def Delta.set.json(is_draft: Bool, +p: List<&2, Step>, +v: J.Json) -> J.Json:
  match is_draft:
    case True{}: J.Json.arr([J.Str{"t"}, J.Json.arr(Delta.path.json(p))])
    case False{}: J.Json.arr([J.Str{"s"}, J.Json.arr(Delta.path.json(p)), v])

def Delta.is_draft(+v: J.Json, +draft: String) -> Bool:
  match v:
    case J.Str{+s}: Bool.and(Bool.not(String.is_empty(draft)), String.eq(s, draft))
    case _: False{}

def Delta.op.json(op: Op, +draft: String) -> J.Json:
  match op:
    case DSet{+p, +v}: Delta.set.json(Delta.is_draft(v, draft), p, v)
    case DRebuild{+p, +ss}: J.Json.arr([J.Str{"a"}, J.Json.arr(Delta.path.json(p)), J.Json.arr(Delta.specs.json(ss))])

def Delta.json(ops: List<&2, Op>, +draft: String) -> List<&2, J.Json>:
  match ops:
    case Nil{}: Nil{}
    case +op <> tail: Delta.op.json(op, draft) <> Delta.json(tail, draft)

# A whole body without the client's own draft
# ---------------------------------------------
# A big draft rides in a body twice (the session's drafts, the composer),
# so a 200 KB paste made a whole view too big to send. A whole body for a
# client that offered the delta goes as ops from no base: the body with
# every copy of the request's draft blanked, then ["t", path] for each.

def Delta.undraft(j: J.Json, +draft: String) -> J.Json:
  match j:
    case J.Str{+s}: Bool.pick(J.Json, String.eq(s, draft), J.Str{""}, J.Str{s})
    case J.Arr{x}: J.Arr{Delta.undraft(x, draft)}
    case J.Obj{x}: J.Obj{Delta.undraft(x, draft)}
    case J.Item{h, t}: J.Item{Delta.undraft(h, draft), Delta.undraft(t, draft)}
    case J.Field{+k, v, t}: J.Field{k, Delta.undraft(v, draft), Delta.undraft(t, draft)}
    case _: j

def Delta.drafted.hit(hit: Bool, +at: List<&2, Step>, acc: List<&2, List<&2, Step>>) -> List<&2, List<&2, Step>>:
  match hit:
    case True{}: List.reverse(&2, Step, at) <> acc
    case False{}: acc

# where the draft is, each path outermost first (i: an Item's index)
def Delta.drafted(j: J.Json, +at: List<&2, Step>, +i: Nat, +draft: String, acc: List<&2, List<&2, Step>>) -> List<&2, List<&2, Step>>:
  match j:
    case J.Str{+s}: Delta.drafted.hit(String.eq(s, draft), at, acc)
    case J.Arr{x}: Delta.drafted(x, at, 0n, draft, acc)
    case J.Obj{x}: Delta.drafted(x, at, 0n, draft, acc)
    case J.Item{h, t}: Delta.drafted(t, at, Nat.add(i, 1n), draft, Delta.drafted(h, DAt{i} <> at, 0n, draft, acc))
    case J.Field{+k, v, t}: Delta.drafted(t, at, 0n, draft, Delta.drafted(v, DKey{k} <> at, 0n, draft, acc))
    case _: acc

def Delta.refs(ps: List<&2, List<&2, Step>>) -> List<&2, J.Json>:
  match ps:
    case Nil{}: Nil{}
    case +p <> tail: J.Json.arr([J.Str{"t"}, J.Json.arr(Delta.path.json(p))]) <> Delta.refs(tail)

# drafts this long go by reference (shorter ones cost less than finding them)
def Delta.long.go(s: String, n: Nat) -> Bool:
  match s n:
    case _ 0n: True{}
    case SNil{} _: False{}
    case SCon{_, tail} 1n+k: Delta.long.go(tail, k)

def Delta.draft.long(+draft: String) -> Bool:
  Delta.long.go(draft, 1024n)

# the ops that make body from nothing, the draft by reference
def Delta.whole(+body: J.Json, +draft: String) -> List<&2, J.Json>:
  J.Json.arr([J.Str{"s"}, J.Json.arr(Nil{}), Delta.undraft(body, draft)]) <> List.reverse(&2, J.Json, Delta.refs(Delta.drafted(body, Nil{}, 0n, draft, Nil{})))

# The oracle: ops applied to the base
# -----------------------------------

def Delta.put.go(fs: J.Json, hit: Bool, +key: String, +v: J.Json) -> J.Json:
  match fs hit:
    case J.Field{_, _, +tail} True{}: J.Field{key, v, tail}
    case J.Field{+k, +val, +tail} False{}: J.Field{k, val, Delta.put.go(tail, Delta.hit(tail, key), key, v)}
    case _ _: fs

def Delta.put(+j: J.Json, +key: String, +v: J.Json) -> J.Json:
  match j:
    case J.Obj{+fs}: J.Obj{Delta.put.go(fs, Delta.hit(fs, key), key, v)}
    case _: j

def Delta.get(+j: J.Json, +key: String) -> J.Json:
  match j:
    case J.Obj{+fs}: Delta.field(fs, key)
    case _: J.Null{}

def Delta.seti.go(xs: J.Json, +i: Nat, +v: J.Json) -> J.Json:
  match xs i:
    case J.Item{_, +tail} 0n: J.Item{v, tail}
    case J.Item{+h, +tail} 1n+k: J.Item{h, Delta.seti.go(tail, k, v)}
    case _ _: xs

def Delta.seti(+j: J.Json, +i: Nat, +v: J.Json) -> J.Json:
  match j:
    case J.Arr{+xs}: J.Arr{Delta.seti.go(xs, i, v)}
    case _: j

def Delta.geti(+j: J.Json, +i: Nat) -> J.Json:
  Maybe.default(&2, J.Json, List.get(&2, J.Json, J.Json.elems(j), i), J.Null{})

def Delta.build(ss: List<&2, Spec>, +olds: List<&2, J.Json>) -> List<&2, J.Json>:
  match ss:
    case Nil{}: Nil{}
    case DOld{+i} <> tail: Maybe.default(&2, J.Json, List.get(&2, J.Json, olds, i), J.Null{}) <> Delta.build(tail, olds)
    case DNew{+v} <> tail: v <> Delta.build(tail, olds)

def Delta.leaf(op: Op, +j: J.Json) -> J.Json:
  match op:
    case DSet{_, +v}: v
    case DRebuild{_, +ss}: J.Json.arr(Delta.build(ss, J.Json.elems(j)))

def Delta.at(path: List<&2, Step>, +j: J.Json, +op: Op) -> J.Json:
  match path:
    case Nil{}: Delta.leaf(op, j)
    case DKey{+k} <> tail: Delta.put(j, k, Delta.at(tail, Delta.get(j, k), op))
    case DAt{+i} <> tail: Delta.seti(j, i, Delta.at(tail, Delta.geti(j, i), op))

def Delta.path(+op: Op) -> List<&2, Step>:
  match op:
    case DSet{+p, _}: p
    case DRebuild{+p, _}: p

def Delta.apply(ops: List<&2, Op>, +j: J.Json) -> J.Json:
  match ops:
    case Nil{}: j
    case +op <> tail: Delta.apply(tail, Delta.at(Delta.path(op), j, op))

# the new body from the base and its diff (a walk out of fuel: the new body)
def Delta.round.k(d: Maybe<&2, List<&2, Op>>, +old: J.Json, +new: J.Json) -> J.Json:
  match d:
    case Some{+ops}: Delta.apply(ops, old)
    case None{}: new

def Delta.round(+old: J.Json, +new: J.Json) -> J.Json:
  Delta.round.k(Delta.diff(old, new), old, new)
