import Base
import ./dom.bend as D

def H.bool.value(r: Map<&2, Bool> & Bool) -> Bool:
  (_, v) = r
  v

def H.has(m: Map<&2, Bool>, +k: String) -> Bool:
  H.bool.value(Map.get(Bool, False{}, m, k))

def H.string.value(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def H.get(m: Map<&2, String>, +k: String) -> String:
  H.string.value(Map.get(String, "", m, k))

def H.attr.hit(hit: Bool, v: String, rest: String) -> String:
  match hit:
    case True{}: v
    case False{}: rest

# Page history is data only. The fresh page owns every non-entry node.
# No text comparisons, DOM state, measurements or scroll positions live here.
def H.attr(xs: List<&2, D.Attr>, +name: String) -> String:
  match xs:
    case Nil{}: ""
    case D.At{+n, v} <> t:
      H.attr.hit(String.eq(n, name), v, H.attr(t, name))
    case D.On{_, _} <> t: H.attr(t, name)

# Attribute lists are tiny. On is also accepted in its transported DOM form.
def H.fold(xs: List<&2, D.Attr>) -> Bool:
  match xs:
    case Nil{}: False{}
    case D.On{+ev, +act} <> t:
      Bool.or(Bool.and(String.eq(ev, "click"), String.eq(act, "fold")), H.fold(t))
    case D.At{+n, +v} <> t:
      Bool.or(Bool.and(String.eq(n, "data-on-click"), String.eq(v, "fold")), H.fold(t))

def H.prefer(a: String, b: String) -> String:
  match a:
    case SNil{}: b
    case SCon{c, t}: SCon{c, t}

def H.fold.key(fold: Bool, +k: String, +as: List<&2, D.Attr>, fallback: String) -> String:
  match fold:
    case True{}: H.prefer(k, "fold:" ++ String.drop(H.attr(as, "data-value"), 1n))
    case False{}: fallback

# Actions distinguish otherwise identical controls without reading their text.
# Accept the VNode event representation and transported data-on-* attributes.
def H.action.attr(event: Bool, name: String, value: String, acc: String) -> String:
  match event:
    case True{}: acc ++ name ++ "=" ++ value ++ ";"
    case False{}: acc

def H.actions(xs: List<&2, D.Attr>, acc: String) -> String:
  match xs:
    case Nil{}: acc
    case D.On{event, action} <> t: H.actions(t, acc ++ "data-on-" ++ event ++ "=" ++ action ++ ";")
    case D.At{+name, value} <> t:
      H.actions(t, H.action.attr(String.starts_with(name, "data-on-"), name, value, acc))

# Explicit identities are unique. Class-based control identities deliberately
# remain control identities: display_key_at adds their sibling index.
# Message and fold identities never depend on their position or their text.
def key(n: D.Node) -> String:
  match n:
    case D.Txt{_}: ""
    case D.El{+tag, +k, +as, _}:
      H.fold.key(H.fold(as), k, as, H.prefer(H.attr(as, "data-id"), H.prefer(k,
        H.prefer(H.attr(as, "id"), "control:" ++ tag ++ ":" ++ H.attr(as, "class") ++ ":" ++ H.attr(as, "data-value") ++ ":" ++ H.actions(as, "")))))

def H.word.go(s: String, +word: String, acc: String) -> Bool:
  match s:
    case SNil{}: String.eq(acc, word)
    case SCon{Chr{32}, t}:
      Bool.or(String.eq(acc, word), H.word.go(t, word, ""))
    case SCon{c, t}: H.word.go(t, word, acc ++ SCon{c, SNil{}})

def H.transient(+k: String, +as: List<&2, D.Attr>) -> Bool:
  Bool.or(Bool.or(String.starts_with(k, "live:"), String.starts_with(k, "@live:")), Bool.or(String.eq(H.attr(as, "fn"), "live"),
    Bool.or(H.word.go(H.attr(as, "class"), "live", ""), H.word.go(H.attr(as, "class"), "sending", ""))))

def H.historical.allowed(blocked: Bool, +tag: String, +k: String, +as: List<&2, D.Attr>) -> Bool:
  match blocked:
    case True{}: False{}
    case False{}:
      Bool.or(Bool.or(Bool.not(String.is_empty(H.attr(as, "data-id"))), Bool.not(String.is_empty(H.attr(as, "data-timeline-entry")))),
        Bool.or(Bool.and(H.fold(as), Bool.not(String.is_empty(String.drop(H.attr(as, "data-value"), 1n)))),
          Bool.and(String.eq(tag, "bp-memo"), Bool.and(Bool.not(String.is_empty(k)),
            Bool.or(String.eq(H.attr(as, "fn"), "msg"), Bool.or(String.eq(H.attr(as, "fn"), "row"), String.eq(H.attr(as, "fn"), "act")))))))

def H.historical(n: D.Node) -> Bool:
  match n:
    case D.Txt{_}: False{}
    case D.El{+tag, +k, +as, _}: H.historical.allowed(H.transient(k, as), tag, k, as)

def H.transient.node(n: D.Node) -> Bool:
  match n:
    case D.Txt{_}: False{}
    case D.El{_, k, as, _}: H.transient(k, as)

def H.display.key(entry: Bool, k: String) -> String:
  match entry:
    case True{}: k
    case False{}: "control:" ++ k

# Bend owns anchor eligibility: entry/live identities remain anchor candidates;
# fixed page controls are marked for exclusion by the virtual layout planner.
def display_key(+n: D.Node) -> String:
  H.display.key(Bool.or(H.historical(n), H.transient.node(n)), key(n))

def H.display.at(control: Bool, k: String, i: Nat) -> String:
  match control:
    case True{}: k ++ ":" ++ Nat.show(i)
    case False{}: k

def H.display.position(+k: String, i: Nat) -> String:
  H.display.at(String.starts_with(k, "control:"), k, i)

# Repeated fresh controls have positional identities; entry and live keys keep
# the same identity when older rows are prepended. JS passes BigInt(index).
def display_key_at(n: D.Node, i: Nat) -> String:
  H.display.position(display_key(n), i)

def H.push(yes: Bool, n: D.Node, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match yes:
    case True{}: n <> acc
    case False{}: acc

def H.rows(xs: List<&2, D.Node>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: List.reverse(&2, D.Node, acc)
    case +h <> t: H.rows(t, H.push(H.historical(h), h, acc))

def H.timeline(n: D.Node) -> Bool:
  match n:
    case D.Txt{_}: False{}
    case D.El{_, _, as, _}: String.eq(H.attr(as, "id"), "timeline")

def H.head(xs: List<&2, D.Node>) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> _: H.timeline(h)

# Stops before reading timeline bodies; sibling loops are tail recursive.
def H.find(xs: List<&2, D.Node>, at: Bool, got: Maybe<&2, List<&2, D.Node>>) -> Maybe<&2, List<&2, D.Node>>:
  match xs at got:
    case _ _ Some{ns}: Some{ns}
    case Nil{} _ None{}: None{}
    case D.El{_, _, _, ks} <> +t True{} None{}: Some{ks}
    case D.El{_, _, _, +ks} <> +t False{} None{}:
      H.find(t, H.head(t), H.find(ks, H.head(ks), None{}))
    case D.Txt{_} <> +t _ None{}: H.find(t, H.head(t), None{})

def H.old(found: Maybe<&2, List<&2, D.Node>>) -> List<&2, D.Node>:
  match found:
    case None{}: Nil{}
    case Some{xs}: H.rows(xs, Nil{})

def H.keys(xs: List<&2, D.Node>, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}: acc
    case h <> t: H.keys(t, Map.set(&2, Bool, acc, key(h), True{}))

type HBounds is Data:
  HBounds{found: Bool, first: Nat, last: Nat}

def H.bound.hit(shared: Bool, +i: Nat, b: HBounds) -> HBounds:
  match shared b:
    case False{} other: other
    case True{} HBounds{False{}, _, _}: HBounds{True{}, i, i}
    case True{} HBounds{True{}, first, _}: HBounds{True{}, first, i}

def H.bounds(xs: List<&2, D.Node>, +fresh: Map<&2, Bool>, +i: Nat, b: HBounds) -> HBounds:
  match xs:
    case Nil{}: b
    case h <> t: H.bounds(t, fresh, Nat.add(i, 1n), H.bound.hit(H.has(fresh, key(h)), i, b))

type HEdges is Data:
  HEdges{prefix: List<&2, D.Node>, suffix: List<&2, D.Node>}

def H.edge.hit(before: Bool, after: Bool, +n: D.Node, e: HEdges) -> HEdges:
  match e:
    case HEdges{p, s}: HEdges{H.push(before, n, p), H.push(after, n, s)}

def H.edge.one(+i: Nat, b: HBounds, +direction: String, n: D.Node, e: HEdges) -> HEdges:
  match b:
    case HBounds{True{}, +first, +last}:
      H.edge.hit(Nat.is_lt(i, first), Nat.is_gt(i, last), n, e)
    case HBounds{False{}, _, _}:
      H.edge.hit(String.eq(direction, "newer"), String.eq(direction, "older"), n, e)

def H.edges(xs: List<&2, D.Node>, +b: HBounds, +direction: String, +i: Nat, e: HEdges) -> HEdges:
  match xs:
    case Nil{}: e
    case h <> t: H.edges(t, b, direction, Nat.add(i, 1n), H.edge.one(i, b, direction, h, e))

# Reverse lists can be put onto an accumulator without non-tail append calls.
def H.onto(xs: List<&2, D.Node>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: acc
    case h <> t: H.onto(t, h <> acc)

def H.count(xs: List<&2, D.Node>, n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case _ <> t: H.count(t, Nat.add(n, 1n))

def H.at.end(last: Bool, suffix: List<&2, D.Node>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match last:
    case True{}: H.onto(List.reverse(&2, D.Node, suffix), acc)
    case False{}: acc

def H.weave.head(xs: List<&2, D.Node>) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> _: H.historical(h)

# prefix and suffix are reversed by H.edges. Fresh controls stay interleaved
# with the same fresh rows, including controls between historical entries.
def H.weave(xs: List<&2, D.Node>, historical: Bool, prefix: List<&2, D.Node>, +suffix: List<&2, D.Node>, +left: Nat, +seen: Map<&2, Bool>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs historical:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case +h <> +t True{}:
      H.weave(t, H.weave.head(t), Nil{}, suffix, Nat.sub(left, 1n), Map.set(&2, Bool, seen, key(h), True{}),
        H.at.end(Nat.is_eq(left, 1n), suffix,
          H.push(Bool.not(H.has(seen, key(h))), h, H.onto(List.reverse(&2, D.Node, prefix), acc))))
    case h <> +t False{}: H.weave(t, H.weave.head(t), prefix, suffix, left, seen, h <> acc)

def H.weave.edges(e: HEdges, +fresh: List<&2, D.Node>, count: Nat) -> List<&2, D.Node>:
  match e:
    case HEdges{p, s}: H.weave(fresh, H.weave.head(fresh), p, s, count, Map.new(&2, Bool), Nil{})

# Retained data can arrive from a restored snapshot, including duplicate old
# rows or a historical identity which the fresh page now renders as live.
# Fresh ownership wins before the assembled timeline is deduplicated.
def H.without(xs: List<&2, D.Node>, +freshKeys: Map<&2, Bool>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: List.reverse(&2, D.Node, acc)
    case +h <> t: H.without(t, freshKeys, H.push(Bool.not(H.has(freshKeys, key(h))), h, acc))

def H.edges.fresh(e: HEdges, +freshKeys: Map<&2, Bool>) -> HEdges:
  match e:
    case HEdges{p, s}: HEdges{H.without(p, freshKeys, Nil{}), H.without(s, freshKeys, Nil{})}

def H.entry(+n: D.Node) -> Bool:
  Bool.or(H.historical(n), H.transient.node(n))

def H.unique.step(entry: Bool, duplicate: Bool, n: D.Node, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match entry duplicate:
    case True{} True{}: acc
    case _ _: n <> acc

def H.unique.seen(entry: Bool, +n: D.Node, seen: Map<&2, Bool>) -> Map<&2, Bool>:
  match entry:
    case True{}: Map.set(&2, Bool, seen, key(n), True{})
    case False{}: seen

# Controls retain their places and multiplicity. Entries are unique across the
# complete prefix + fresh + suffix, rather than only within the fresh window.
def H.unique(xs: List<&2, D.Node>, +seen: Map<&2, Bool>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: List.reverse(&2, D.Node, acc)
    case +h <> t:
      H.unique(t, H.unique.seen(H.entry(h), h, seen), H.unique.step(H.entry(h), H.has(seen, key(h)), h, acc))

def H.merge.rows(+old: List<&2, D.Node>, +fresh: List<&2, D.Node>, +rows: List<&2, D.Node>, direction: String) -> List<&2, D.Node>:
  H.unique(H.weave.edges(H.edges.fresh(H.edges(old, H.bounds(old, H.keys(rows, Map.new(&2, Bool)), 0n, HBounds{False{}, 0n, 0n}), direction, 0n, HEdges{Nil{}, Nil{}}), H.keys(fresh, Map.new(&2, Bool))), fresh, H.count(rows, 0n)), Map.new(&2, Bool), Nil{})

def H.fold.node(n: D.Node) -> Bool:
  match n:
    case D.Txt{_}: False{}
    case D.El{_, _, as, _}: H.fold(as)

def H.fold.raw(n: D.Node) -> String:
  match n:
    case D.Txt{_}: ""
    case D.El{_, _, as, _}: "fold:" ++ String.drop(H.attr(as, "data-value"), 1n)

type HFolds is Data:
  HFolds{ids: Map<&2, String>, successors: Map<&2, String>, pending: String}

# A shared successor identifies a run even when its first entry changed at a
# page boundary. Each run supplies at most its first shared successor.
def H.folds.step(folded: Bool, shared: Bool, +n: D.Node, f: HFolds) -> HFolds:
  match folded shared f:
    case True{} _ HFolds{ids, succ, _}:
      HFolds{Map.set(&2, String, ids, H.fold.raw(n), key(n)), succ, key(n)}
    case False{} True{} HFolds{ids, succ, pending}:
      HFolds{ids, Map.set(&2, String, succ, key(n), pending), ""}
    case False{} False{} other: other

def H.folds(xs: List<&2, D.Node>, +fresh: Map<&2, Bool>, f: HFolds) -> HFolds:
  match xs:
    case Nil{}: f
    case +h <> t:
      H.folds(t, fresh, H.folds.step(H.fold.node(h), H.has(fresh, key(h)), h, f))

def H.rekey(k: String, n: D.Node) -> D.Node:
  match k n:
    case SNil{} other: other
    case SCon{c, t} D.El{tag, _, as, ks}: D.El{tag, SCon{c, t}, as, ks}
    case _ other: other

def H.rekey.same(same: Bool, k: String, n: D.Node) -> D.Node:
  match same:
    case True{}: n
    case False{}: H.rekey(k, n)

def H.rekey.changed(+k: String, +n: D.Node) -> D.Node:
  H.rekey.same(String.eq(k, key(n)), k, n)

def H.normalize.one(folded: Bool, +n: D.Node, +ids: Map<&2, String>, +successors: Map<&2, String>, next: String) -> D.Node:
  match folded:
    case False{}: n
    case True{}:
      H.rekey.changed(H.prefer(H.get(ids, H.fold.raw(n)), H.get(successors, next)), n)

def H.next(folded: Bool, shared: Bool, n: D.Node, next: String) -> String:
  match folded shared:
    case True{} _: ""
    case False{} True{}: key(n)
    case False{} False{}: next

# Walk reversed fresh siblings, retaining the next shared entry, never their
# body text. Same canonical fold id wins over a successor-based reheading.
def H.normalize(xs: List<&2, D.Node>, +oldKeys: Map<&2, Bool>, +ids: Map<&2, String>, +successors: Map<&2, String>, +next: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: acc
    case +h <> t:
      H.normalize(t, oldKeys, ids, successors,
        H.next(H.fold.node(h), Bool.and(H.historical(h), H.has(oldKeys, key(h))), h, next),
        H.normalize.one(H.fold.node(h), h, ids, successors, next) <> acc)

def H.merge.folds(f: HFolds, +old: List<&2, D.Node>, fresh: List<&2, D.Node>, direction: String) -> List<&2, D.Node>:
  match f:
    case HFolds{ids, successors, _}:
      +normalized = H.normalize(List.reverse(&2, D.Node, fresh), H.keys(old, Map.new(&2, Bool)), ids, successors, "", Nil{})
      H.merge.rows(old, normalized, H.rows(normalized, Nil{}), direction)

def H.merge(+old: List<&2, D.Node>, +fresh: List<&2, D.Node>, direction: String) -> List<&2, D.Node>:
  H.merge.folds(H.folds(old, H.keys(H.rows(fresh, Nil{}), Map.new(&2, Bool)), HFolds{Map.new(&2, String), Map.new(&2, String), ""}), old, fresh, direction)

# Only tree depth uses a stack frame; flat sibling lists use an accumulator.
def H.paint(xs: List<&2, D.Node>, at: Bool, +old: List<&2, D.Node>, +direction: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs at:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case D.El{tag, k, as, ks} <> +t True{}:
      H.paint(t, H.head(t), old, direction, D.El{tag, k, as, H.merge(old, ks, direction)} <> acc)
    case D.El{tag, k, as, +ks} <> +t False{}:
      H.paint(t, H.head(t), old, direction, D.El{tag, k, as, H.paint(ks, H.head(ks), old, direction, Nil{})} <> acc)
    case h <> +t _: H.paint(t, H.head(t), old, direction, h <> acc)

def H.first(xs: List<&2, D.Node>, fallback: D.Node) -> D.Node:
  match xs:
    case h <> _: h
    case Nil{}: fallback

# An incoming-message source label belongs to that message, not to the current
# hub page. Keep both in one keyed historical row so page overlap cannot remove
# 32px between already visible messages. Generic page controls stay fresh-only.
def H.source(n: D.Node) -> Bool:
  match n:
    case D.El{_, _, +as, _}: Bool.and(H.word.go(H.attr(as, "class"), "link", ""), H.word.go(H.attr(as, "class"), "in", ""))
    case _: False{}

def H.group(source: Bool, historical: Bool, +label: D.Node, +entry: D.Node) -> List<&2, D.Node>:
  match source historical:
    case True{} True{}:
      [D.El{"div", key(entry), [D.At{"class", "timeline-entry"}, D.At{"data-timeline-entry", key(entry)}], [label, entry]}]
    case _ _: [label, entry]

def H.source.head(xs: List<&2, D.Node>) -> Bool:
  match xs:
    case a <> _: H.source(a)
    case Nil{}: False{}

# Walk one sibling at a time: a source label may follow any ordinary entry.
def H.decorate(xs: List<&2, D.Node>, source: Bool, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs source:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case a <> +b <> +t True{}:
      H.decorate(t, H.source.head(t), H.onto(H.group(True{}, H.historical(b), a, b), acc))
    case a <> +t _: H.decorate(t, H.source.head(t), a <> acc)

def H.prepare(xs: List<&2, D.Node>, timeline: Bool, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs timeline:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case D.El{tag, k, as, +ks} <> +t True{}:
      H.prepare(t, H.head(t), D.El{tag, k, as, H.decorate(ks, H.source.head(ks), Nil{})} <> acc)
    case D.El{tag, k, as, +ks} <> +t False{}:
      H.prepare(t, H.head(t), D.El{tag, k, as, H.prepare(ks, H.head(ks), Nil{})} <> acc)
    case h <> +t _: H.prepare(t, H.head(t), h <> acc)

def H.prepared(+node: D.Node) -> D.Node:
  H.first(H.prepare([node], H.timeline(node), Nil{}), node)

def H.project(reset: Bool, +previous: D.Node, +fresh: D.Node, direction: String) -> D.Node:
  match reset:
    case True{}: fresh
    case False{}:
      H.first(H.paint([fresh], H.timeline(fresh), H.old(H.find([previous], H.timeline(previous), None{})), direction, Nil{}), fresh)

# Public API consumed by host.js, importing this module as History.
def project(previous: D.Node, fresh: D.Node, +direction: String) -> D.Node:
  H.project(Bool.or(String.eq(direction, "reset"), String.eq(direction, "newest")), H.prepared(previous), H.prepared(fresh), direction)
