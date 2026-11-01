import Base
import ./dom.bend as D
import ../core/landing.bend as LAND

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

def H.fold.node(n: D.Node) -> Bool:
  match n:
    case D.Txt{_}: False{}
    case D.El{_, _, as, _}: H.fold(as)

def H.fold.raw(n: D.Node) -> String:
  match n:
    case D.Txt{_}: ""
    case D.El{_, _, as, _}: "fold:" ++ String.drop(H.attr(as, "data-value"), 1n)

# A run of tool calls the fresh page shows shut owns its rows. An open
# run's rows say which run they are in (view.bend's View.run.mark,
# data-run); one kept from an earlier picture goes once the fresh page has
# that run's head shut. Else a run open at the page's end (opened, then
# shut) left its rows below the newest entry, headless.
def H.shut.is(n: D.Node) -> Bool:
  match n:
    case D.Txt{_}: False{}
    case D.El{_, _, +as, _}: Bool.and(H.fold(as), String.starts_with(H.attr(as, "data-value"), "1"))

def H.shut.one(shut: Bool, +n: D.Node, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match shut:
    case True{}: Map.set(&2, Bool, Map.set(&2, Bool, acc, H.fold.raw(n), True{}), key(n), True{})
    case False{}: acc

# the heads of the runs shut on the fresh page, by their run ("fold:<id>")
# and by the key they show under
def H.shut(xs: List<&2, D.Node>, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}: acc
    case +h <> t: H.shut(t, H.shut.one(H.shut.is(h), h, acc))

def H.run(n: D.Node) -> String:
  match n:
    case D.Txt{_}: ""
    case D.El{_, _, as, _}: H.attr(as, "data-run")

def H.gone.k(+r: String, +shut: Map<&2, Bool>) -> Bool:
  Bool.and(Bool.not(String.is_empty(r)), H.has(shut, "fold:" ++ r))

# a retained row of a run the fresh page shows shut
def H.gone(+shut: Map<&2, Bool>, n: D.Node) -> Bool:
  H.gone.k(H.run(n), shut)

# Retained data can arrive from a restored snapshot, including duplicate old
# rows or a historical identity which the fresh page now renders as live.
# Fresh ownership wins before the assembled timeline is deduplicated, and
# a shut run's rows go with it (H.shut).
def H.without(xs: List<&2, D.Node>, +freshKeys: Map<&2, Bool>, +shut: Map<&2, Bool>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: List.reverse(&2, D.Node, acc)
    case +h <> t: H.without(t, freshKeys, shut, H.push(Bool.not(Bool.or(H.has(freshKeys, key(h)), H.gone(shut, h))), h, acc))

def H.edges.fresh(e: HEdges, +freshKeys: Map<&2, Bool>, +shut: Map<&2, Bool>) -> HEdges:
  match e:
    case HEdges{p, s}: HEdges{H.without(p, freshKeys, shut, Nil{}), H.without(s, freshKeys, shut, Nil{})}

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
  H.unique(H.weave.edges(H.edges.fresh(H.edges(old, H.bounds(old, H.keys(rows, Map.new(&2, Bool)), 0n, HBounds{False{}, 0n, 0n}), direction, 0n, HEdges{Nil{}, Nil{}}), H.keys(fresh, Map.new(&2, Bool)), H.shut(fresh, Map.new(&2, Bool))), fresh, H.count(rows, 0n)), Map.new(&2, Bool), Nil{})

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

# The same, for a previous page that is project's own result (host.js's
# historyNode): already prepared (its source labels grouped, H.prepare
# again changes nothing, test/timeline_history_test.bend), so it is not
# walked again; a landing then costs the fresh page and the merge.
def project_held(previous: D.Node, fresh: D.Node, +direction: String) -> D.Node:
  H.project(Bool.or(String.eq(direction, "reset"), String.eq(direction, "newest")), previous, H.prepared(fresh), direction)

def prepared(node: D.Node) -> D.Node:
  H.prepared(node)

# Rows held past the cap (thin-host.js's heldTrim): the rows of the pages
# let go leave the page, and their measured height stays as a gap (gaps),
# so the scrollbar and the reader's place do not move (host.js sums the
# measured heights of the ids let go). ids: entry ids,
# one a line. A row goes when it is one of them, the run it heads or the
# run it is in starts with one; controls and live rows always stay.
def H.ids.one(+id: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match id:
    case SNil{}: m
    case SCon{c, t}: Map.set(&2, Bool, Map.set(&2, Bool, m, SCon{c, t}, True{}), "fold:" ++ SCon{c, t}, True{})

def H.ids(s: String, +word: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match s:
    case SNil{}: H.ids.one(word, m)
    case SCon{Chr{10}, t}: H.ids(t, "", H.ids.one(word, m))
    case SCon{c, t}: H.ids(t, word ++ SCon{c, SNil{}}, m)

def H.let.go(+ids: Map<&2, Bool>, +n: D.Node) -> Bool:
  Bool.and(H.historical(n), Bool.or(H.has(ids, key(n)), Bool.and(Bool.not(String.is_empty(H.run(n))), H.has(ids, H.run(n)))))

def H.drop.rows(xs: List<&2, D.Node>, +ids: Map<&2, Bool>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: List.reverse(&2, D.Node, acc)
    case +h <> t: H.drop.rows(t, ids, H.push(Bool.not(H.let.go(ids, h)), h, acc))

def H.drop.paint(xs: List<&2, D.Node>, at: Bool, +ids: Map<&2, Bool>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs at:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case D.El{tag, k, as, ks} <> +t True{}:
      H.drop.paint(t, H.head(t), ids, D.El{tag, k, as, H.drop.rows(ks, ids, Nil{})} <> acc)
    case D.El{tag, k, as, +ks} <> +t False{}:
      H.drop.paint(t, H.head(t), ids, D.El{tag, k, as, H.drop.paint(ks, H.head(ks), ids, Nil{})} <> acc)
    case h <> +t _: H.drop.paint(t, H.head(t), ids, h <> acc)

def drop(+node: D.Node, ids: String) -> D.Node:
  +m = H.ids(ids, "", Map.new(&2, Bool))
  H.first(H.drop.paint([node], H.timeline(node), m, Nil{}), node)

# The height of the rows let go above and below, as empty rows that are
# never an anchor (a control's key): the older one first in the timeline,
# the newer one after its last entry. "0" (or "") adds none.
def H.gap(+side: String, +px: String) -> D.Node:
  D.El{"div", "@gap:" ++ side, [D.At{"class", "timeline-gap"}, D.At{"aria-hidden", "true"}, D.At{"style", "height:" ++ px ++ "px"}], Nil{}}

def H.gap.none(+px: String) -> Bool:
  Bool.or(String.is_empty(px), String.eq(px, "0"))

def H.gap.older(none: Bool, +px: String, xs: List<&2, D.Node>) -> List<&2, D.Node>:
  match none:
    case True{}: xs
    case False{}: H.gap("older", px) <> xs

type HGap is Data:
  HGap{done: Bool, acc: List<&2, D.Node>}

# walked from the end: the newer gap goes right after the last entry
def H.gap.step(entry: Bool, +h: D.Node, +gap: D.Node, g: HGap) -> HGap:
  match entry g:
    case True{} HGap{False{}, acc}: HGap{True{}, h <> gap <> acc}
    case _ HGap{done, acc}: HGap{done, h <> acc}

def H.gap.walk(xs: List<&2, D.Node>, +gap: D.Node, g: HGap) -> HGap:
  match xs:
    case Nil{}: g
    case +h <> t: H.gap.walk(t, gap, H.gap.step(H.historical(h), h, gap, g))

def H.gap.end(+gap: D.Node, g: HGap) -> List<&2, D.Node>:
  match g:
    case HGap{True{}, acc}: acc
    case HGap{False{}, acc}: gap <> acc

def H.gap.newer(none: Bool, +px: String, xs: List<&2, D.Node>) -> List<&2, D.Node>:
  match none:
    case True{}: xs
    case False{}: H.gap.end(H.gap("newer", px), H.gap.walk(List.reverse(&2, D.Node, xs), H.gap("newer", px), HGap{False{}, Nil{}}))

def H.gaps.paint(xs: List<&2, D.Node>, at: Bool, +older: String, +newer: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs at:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case D.El{tag, k, as, ks} <> +t True{}:
      H.gaps.paint(t, H.head(t), older, newer, D.El{tag, k, as, H.gap.older(H.gap.none(older), older, H.gap.newer(H.gap.none(newer), newer, ks))} <> acc)
    case D.El{tag, k, as, +ks} <> +t False{}:
      H.gaps.paint(t, H.head(t), older, newer, D.El{tag, k, as, H.gaps.paint(ks, H.head(ks), older, newer, Nil{})} <> acc)
    case h <> +t _: H.gaps.paint(t, H.head(t), older, newer, h <> acc)

def gaps(+node: D.Node, +older: String, +newer: String) -> D.Node:
  H.first(H.gaps.paint([node], H.timeline(node), older, newer, Nil{}), node)

# What a page reply redraws (core/landing.bend, laws land_*): "page", or the
# names of the regions it wrote into, space-separated. regions: a line each,
# its name then its path in the reply's body, tab-separated; paths: the
# delta's, one a line ("" none; a whole reply: one empty line)
def landing(+regions: String, +paths: String) -> String:
  LAND.Land.of(regions, paths)

# An older page added as a block (host.js's landing): the merge reads the
# held rows only down to the end of the new page's overlap, and the rest of
# them is the very same list, shared. That is the full merge's result when
# the rest holds entry rows alone and no run of tool calls crosses into it:
# the full merge keeps every old entry row outside the fresh page's span as
# it is, in order, and drops only old controls, transient rows, rows the
# fresh page draws, and the rows of runs the fresh page shuts. The held
# timeline says where its last non-entry node is (data-held-clean: the
# count of nodes up to it, set by every merge), so the check costs nothing
# per held row; anything else takes the full merge
# (test/timeline_history_test.bend holds both equal).
def H.kids(found: Maybe<&2, List<&2, D.Node>>) -> List<&2, D.Node>:
  match found:
    case None{}: Nil{}
    case Some{xs}: xs

# (the count H.marked wrote, Nat.show's digits; "" none: unknown)
def H.num.go(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}: acc
    case SCon{Chr{+c}, t}: H.num.go(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

def H.num(s: String) -> Maybe<&2, Nat>:
  match s:
    case SNil{}: None{}
    case SCon{c, t}: Some{H.num.go(SCon{c, t}, 0n)}

# the count of nodes up to and including the last one that is not an entry row
def H.clean.step(row: Bool, +i: Nat, last: Nat) -> Nat:
  match row:
    case True{}: last
    case False{}: Nat.add(i, 1n)

def H.clean(xs: List<&2, D.Node>, +i: Nat, last: Nat) -> Nat:
  match xs:
    case Nil{}: last
    case +h <> t: H.clean(t, Nat.add(i, 1n), H.clean.step(H.historical(h), i, last))

type HSplit is Data:
  HSplit{count: Nat, shared: Bool, rest: List<&2, D.Node>}

# the split stops at the first entry row the fresh page does not draw
def H.stops(xs: List<&2, D.Node>, +fk: Map<&2, Bool>) -> Bool:
  match xs:
    case Nil{}: False{}
    case +h <> _: Bool.and(H.historical(h), Bool.not(H.has(fk, key(h))))

def H.split(xs: List<&2, D.Node>, stop: Bool, +fk: Map<&2, Bool>, +n: Nat, shared: Bool) -> HSplit:
  match xs stop:
    case Nil{} _: HSplit{n, shared, Nil{}}
    case +h <> +t True{}: HSplit{n, shared, h <> t}
    case +h <> +t False{}: H.split(t, H.stops(t, fk), fk, Nat.add(n, 1n), Bool.or(shared, H.has(fk, key(h))))

def H.in.run(xs: List<&2, D.Node>) -> Bool:
  match xs:
    case Nil{}: False{}
    case +h <> _: Bool.not(String.is_empty(H.run(h)))

def H.take(xs: List<&2, D.Node>, n: Nat, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs n:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case +h <> t 0n: List.reverse(&2, D.Node, acc)
    case +h <> t 1n+p: H.take(t, p, h <> acc)

# a merged timeline's rows and where its last non-entry node is
type HMerged is Data:
  HMerged{kids: List<&2, D.Node>, clean: Nat}

def H.merged(+kids: List<&2, D.Node>) -> HMerged:
  HMerged{kids, H.clean(kids, 0n, 0n)}

# merged: the merge of the read part, whose count is the whole's (the
# shared rest after it holds entry rows alone)
def H.onto.rest(+merged: List<&2, D.Node>, rest: List<&2, D.Node>) -> HMerged:
  HMerged{H.onto(List.reverse(&2, D.Node, merged), rest), H.clean(merged, 0n, 0n)}

def H.drop.n(xs: List<&2, D.Node>, n: Nat) -> List<&2, D.Node>:
  match xs n:
    case Nil{} _: Nil{}
    case +h <> +t 0n: h <> t
    case _ <> t 1n+p: H.drop.n(t, p)

def H.shares(xs: List<&2, D.Node>, +fk: Map<&2, Bool>, acc: Bool) -> Bool:
  match xs:
    case Nil{}: acc
    case +h <> t: H.shares(t, fk, Bool.or(acc, Bool.and(H.historical(h), H.has(fk, key(h)))))

# the part read: down to the end of the overlap or past the last non-entry
# node, whichever is further (the full merge drops old controls and
# transient rows, so they are read and dropped here too); both lie in the
# last pages merged, so the part read is a page or two whatever is held
type HCut is Data:
  HCut{head: List<&2, D.Node>, rest: List<&2, D.Node>}

def H.cut(+old: List<&2, D.Node>, +n: Nat) -> HCut:
  HCut{H.take(old, n, Nil{}), H.drop.n(old, n)}

def H.reach(+s: HSplit, +clean: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match s clean:
    case HSplit{n, _, _} Some{c}: Some{Bool.pick(Nat, Nat.is_ge(n, c), n, c)}
    case _ None{}: None{}

def H.cut.ok(+c: HCut, +fk: Map<&2, Bool>) -> Bool:
  match c:
    case HCut{head, rest}: Bool.and(H.shares(head, fk, False{}), Bool.not(H.in.run(rest)))

def H.block.k(ok: Bool, +old: List<&2, D.Node>, +fresh: List<&2, D.Node>, +c: HCut, +direction: String) -> HMerged:
  match ok c:
    case True{} HCut{head, rest}: H.onto.rest(H.merge(H.rows(head, Nil{}), fresh, direction), rest)
    case False{} _: H.merged(H.merge(H.rows(old, Nil{}), fresh, direction))

def H.block.at(+reach: Maybe<&2, Nat>, +old: List<&2, D.Node>, +fresh: List<&2, D.Node>, +fk: Map<&2, Bool>, +direction: String) -> HMerged:
  match reach:
    case None{}: H.merged(H.merge(H.rows(old, Nil{}), fresh, direction))
    case Some{n}: H.block.k(Bool.and(String.eq(direction, "older"), H.cut.ok(H.cut(old, n), fk)), old, fresh, H.cut(old, n), direction)

def H.block(+old: List<&2, D.Node>, +fresh: List<&2, D.Node>, +clean: Maybe<&2, Nat>, +direction: String) -> HMerged:
  +fk = H.keys(H.rows(fresh, Nil{}), Map.new(&2, Bool))
  H.block.at(H.reach(H.split(old, H.stops(old, fk), fk, 0n, False{}), clean), old, fresh, fk, direction)

def H.clean.attr(xs: List<&2, D.Attr>, +n: Nat) -> List<&2, D.Attr>:
  List.append(&2, D.Attr, xs, [D.At{"data-held-clean", Nat.show(n)}])

def H.no.clean(xs: List<&2, D.Attr>, acc: List<&2, D.Attr>) -> List<&2, D.Attr>:
  match xs:
    case Nil{}: List.reverse(&2, D.Attr, acc)
    case D.At{"data-held-clean", _} <> t: H.no.clean(t, acc)
    case +h <> t: H.no.clean(t, h <> acc)

# the merged timeline, marked with where its last non-entry node is
def H.marked(tag: String, k: String, as: List<&2, D.Attr>, m: HMerged) -> D.Node:
  match m:
    case HMerged{kids, n}: D.El{tag, k, H.clean.attr(H.no.clean(as, Nil{}), n), kids}

def H.paint.block(xs: List<&2, D.Node>, at: Bool, +old: List<&2, D.Node>, +clean: Maybe<&2, Nat>, +direction: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs at:
    case Nil{} _: List.reverse(&2, D.Node, acc)
    case D.El{tag, k, as, ks} <> +t True{}:
      H.paint.block(t, H.head(t), old, clean, direction, H.marked(tag, k, as, H.block(old, ks, clean, direction)) <> acc)
    case D.El{tag, k, as, +ks} <> +t False{}:
      H.paint.block(t, H.head(t), old, clean, direction, D.El{tag, k, as, H.paint.block(ks, H.head(ks), old, clean, direction, Nil{})} <> acc)
    case h <> +t _: H.paint.block(t, H.head(t), old, clean, direction, h <> acc)

def H.timeline.attrs(found: Maybe<&2, D.Node>) -> String:
  match found:
    case Some{D.El{_, _, as, _}}: H.attr(as, "data-held-clean")
    case _: ""

def H.find.el(xs: List<&2, D.Node>, at: Bool, got: Maybe<&2, D.Node>) -> Maybe<&2, D.Node>:
  match xs at got:
    case _ _ Some{n}: Some{n}
    case Nil{} _ None{}: None{}
    case +h <> +t True{} None{}: Some{h}
    case D.El{_, _, _, +ks} <> +t False{} None{}:
      H.find.el(t, H.head(t), H.find.el(ks, H.head(ks), None{}))
    case D.Txt{_} <> +t _ None{}: H.find.el(t, H.head(t), None{})

# project_held for an older page, the held rows past the overlap shared
def project_block.k(+previous: D.Node, +ready: D.Node, +direction: String) -> D.Node:
  +old = H.kids(H.find([previous], H.timeline(previous), None{}))
  +clean = H.num(H.timeline.attrs(H.find.el([previous], H.timeline(previous), None{})))
  H.first(H.paint.block([ready], H.timeline(ready), old, clean, direction, Nil{}), ready)

def project_block(+previous: D.Node, fresh: D.Node, +direction: String) -> D.Node:
  project_block.k(previous, H.prepared(fresh), direction)

# whether project_block adds this page as a block, and why not (for the
# tests and the harness): "block", "nomark", "noshare", "run", "newer"
def H.why.parts(+c: HCut, +fk: Map<&2, Bool>) -> String:
  match c:
    case HCut{head, rest}: Bool.pick(String, Bool.not(H.shares(head, fk, False{})), "noshare", Bool.pick(String, H.in.run(rest), "run", "block"))

def H.why.cut(+reach: Maybe<&2, Nat>, +old: List<&2, D.Node>, +fk: Map<&2, Bool>) -> String:
  match reach:
    case None{}: "nomark"
    case Some{n}: H.why.parts(H.cut(old, n), fk)

def why.k(+previous: D.Node, +ready: D.Node, +direction: String) -> String:
  +old = H.kids(H.find([previous], H.timeline(previous), None{}))
  +fk = H.keys(H.rows(H.kids(H.find([ready], H.timeline(ready), None{})), Nil{}), Map.new(&2, Bool))
  Bool.pick(String, String.eq(direction, "older"), H.why.cut(H.reach(H.split(old, H.stops(old, fk), fk, 0n, False{}), H.num(H.timeline.attrs(H.find.el([previous], H.timeline(previous), None{})))), old, fk), "newer")

def why(+previous: D.Node, fresh: D.Node, +direction: String) -> String:
  why.k(previous, H.prepared(fresh), direction)

def blocks(+previous: D.Node, fresh: D.Node, +direction: String) -> Bool:
  String.eq(why(previous, fresh, direction), "block")
