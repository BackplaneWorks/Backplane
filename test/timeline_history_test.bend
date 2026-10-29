import Base
import ./check.bend as C
import ../src/web/timeline-history.bend as H
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as W
import ../src/core/json.bend as J

def row(+id: String, text: String) -> D.Node:
  D.El{"div", id, [D.At{"data-id", id}, D.At{"class", "msg"}], [D.Txt{text}]}

def fold(+state: String) -> D.Node:
  D.El{"button", "", [D.On{"click", "fold"}, D.At{"data-value", state ++ "run"}], [D.Txt{state}]}

def tl(xs: List<&2, D.Node>) -> D.Node:
  D.El{"div", "timeline", [D.At{"id", "timeline"}], xs}

def page(+label: String, xs: List<&2, D.Node>) -> D.Node:
  D.El{"main", label, [D.At{"class", label}], [D.Dom.span("sidebar", label), tl(xs), D.Dom.span("composer", label)]}

def same(+name: String, got: D.Node, want: D.Node) -> IO(Unit):
  C.Check.str(name, J.Json.show(W.Node.write(got)), J.Json.show(W.Node.write(want)))

def headed(+id: String, stable: String) -> D.Node:
  D.El{"button", stable, [D.At{"data-value", "1" ++ id}, D.On{"click", "fold"}], Nil{}}

def source() -> D.Node:
  D.Dom.button("link in", "select", "sender", "", "from sender")

def sourced(+entry: D.Node) -> D.Node:
  D.El{"div", H.key(entry), [D.At{"class", "timeline-entry"}, D.At{"data-timeline-entry", H.key(entry)}], [source(), entry]}

def series(count: Nat, +start: Nat, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match count:
    case 0n: List.reverse(&2, D.Node, acc)
    case 1n+p: series(p, Nat.add(start, 1n), row(Nat.show(start), Nat.show(start)) <> acc)

def main() -> IO(Unit):
  do IO<Unit>:
    +a : D.Node = row("a", "a")
    +b : D.Node = row("b", "old b")
    +c : D.Node = row("c", "old c")
    +d : D.Node = row("d", "d")
    +b1 : D.Node = row("b", "fresh b")
    +c1 : D.Node = row("c", "fresh c")
    same("initial source label grouped with its stable message", H.project(tl([]), tl([a, source(), b, c]), "reset"), tl([a, sourced(b), c]))
    same("retained source label never disappears on older overlap", H.project(tl([source(), b, source(), c]), tl([a, source(), b1]), "older"), tl([a, sourced(b1), sourced(c)]))
    same("source metadata survives newer overlap", H.project(tl([a, source(), b, source(), c]), tl([source(), c1, d]), "newer"), tl([a, sourced(b), sourced(c1), d]))
    same("grouping is idempotent across repeated updates", H.project(tl([sourced(b), c]), tl([source(), b1, c1]), "update"), tl([sourced(b1), c1]))
    C.Check.str("grouped message remains same anchor identity", H.display_key(sourced(b)), H.display_key(b))
    +before : D.Node = D.Dom.button("earlier", "view-page", "older", "", "fresh earlier")
    +between : D.Node = D.Dom.span("middle", "fresh middle")
    +after : D.Node = D.Dom.button("later", "view-page", "newer", "", "fresh later")
    same("canonical overlap keeps both outer intervals and fresh controls", H.project(page("old", [a, b, c, d]), page("fresh", [before, b1, between, c1, after]), "update"), page("fresh", [before, a, b1, between, c1, d, after]))
    same("older disjoint prepends", H.project(tl([c, d]), tl([before, a, b, after]), "older"), tl([before, a, b, c, d, after]))
    same("newer disjoint appends", H.project(tl([a, b]), tl([before, c, d, after]), "newer"), tl([before, a, b, c, d, after]))
    same("unknown disjoint update replaces", H.project(tl([a]), tl([d]), "update"), tl([d]))
    same("reset drops retained pages", H.project(tl([a, b, c]), tl([b1]), "reset"), tl([b1]))
    same("newest drops retained pages", H.project(tl([a, b, c]), tl([b1]), "newest"), tl([b1]))
    same("fold closing replaces old interior", H.project(tl([a, fold("0"), b, c, d]), tl([fold("1"), d]), "update"), tl([a, fold("1"), d]))
    same("fold opening uses same stable head", H.project(tl([a, fold("1"), d]), tl([fold("0"), b, c, d]), "update"), tl([a, fold("0"), b, c, d]))
    same("fresh duplicate shared rows appear once", H.project(tl([a, b, c]), tl([b1, b1, c1]), "older"), tl([a, b1, c1]))
    +memo : D.Node = D.El{"bp-memo", "memo", [D.At{"fn", "msg"}], [D.Txt{"memo text"}]}
    +memoRow : D.Node = D.El{"bp-memo", "memo-row", [D.At{"fn", "row"}], [D.Txt{"row text"}]}
    +live : D.Node = D.El{"bp-memo", "live:t", [D.At{"fn", "live"}], [D.Txt{"old live"}]}
    +sending : D.Node = D.El{"div", "sending", [D.At{"class", "msg sending"}, D.At{"data-id", "pending"}], [D.Txt{"old sending"}]}
    +liveId : D.Node = D.El{"div", "live:t", [D.At{"data-id", "live:t"}], [D.Txt{"live keyed"}]}
    +bodyLive : D.Node = D.El{"bp-memo", "@live:t", [D.At{"fn", "row"}], [D.Txt{"stream preview"}]}
    same("body-window live key never becomes historical overlap", H.project(tl([c, d, bodyLive]), tl([a, b, bodyLive]), "older"), tl([a, b, c, d, bodyLive]))
    same("stream refresh preserves chronological retained suffix", H.project(H.project(tl([c, d, bodyLive]), tl([a, b, bodyLive]), "older"), tl([a, b, bodyLive]), "update"), tl([a, b, c, d, bodyLive]))
    same("memo history retained and old transient/control nodes excluded", H.project(tl([before, memo, memoRow, live, sending, liveId]), tl([a, after]), "newer"), tl([memo, memoRow, a, after]))
    same("accumulates more than two visited pages", H.project(H.project(tl([a]), tl([b]), "newer"), tl([c]), "newer"), tl([a, b, c]))
    same("class timeline alone is not selected", H.project(tl([a]), D.Dom.div("timeline", [b]), "older"), D.Dom.div("timeline", [b]))
    same("missing previous timeline uses fresh", H.project(D.Txt{"old"}, page("fresh", [b]), "older"), page("fresh", [b]))
    C.Check.str("fold key stable across state", H.key(fold("0")), H.key(fold("1")))
    C.Check.str("memo key", H.key(memo), "memo")
    C.Check.str("data-id wins explicit key", H.key(D.El{"div", "wrong", [D.At{"data-id", "right"}], Nil{}}), "right")
    +oldHead : D.Node = tl([headed("B", ""), c, d, row("e", "e")])
    +newHead : D.Node = tl([row("0", "0"), headed("A", ""), c, d])
    +reheaded : D.Node = tl([row("0", "0"), headed("A", "fold:B"), c, d, row("e", "e")])
    same("reheading maps every shared run and removes obsolete head", H.project(oldHead, newHead, "older"), reheaded)
    same("refresh retains a reheaded fold's explicit identity", H.project(reheaded, newHead, "update"), reheaded)
    C.Check.str("reheaded fold key prefers explicit identity", H.key(headed("A", "fold:B")), "fold:B")
    same("unrelated retained run is not reheaded", H.project(tl([headed("B", ""), c, headed("Z", ""), row("z", "z")]), tl([headed("A", ""), c]), "older"), tl([headed("A", "fold:B"), c, headed("Z", ""), row("z", "z")]))
    same("32-entry older overlap deduplicates", H.project(tl(series(32n, 16n, Nil{})), tl(series(32n, 0n, Nil{})), "older"), tl(series(48n, 0n, Nil{})))
    same("32-entry newer overlap retains prefix", H.project(tl(series(32n, 0n, Nil{})), tl(series(32n, 16n, Nil{})), "newer"), tl(series(48n, 0n, Nil{})))
    same("multiple overlapping pages keep all visited data", H.project(H.project(tl(series(32n, 0n, Nil{})), tl(series(32n, 16n, Nil{})), "newer"), tl(series(32n, 32n, Nil{})), "newer"), tl(series(64n, 0n, Nil{})))
    same("short page without a shared rendered row retains old data", H.project(tl([row("z", "z")]), tl([headed("a", "")]), "older"), tl([headed("a", ""), row("z", "z")]))
    same("a second short page retains all visited data", H.project(H.project(tl([row("z", "z")]), tl([headed("a", "")]), "older"), tl([headed("0", "")]), "older"), tl([headed("0", ""), headed("a", ""), row("z", "z")]))
    C.Check.str("display message remains anchor eligible", H.display_key(a), H.key(a))
    C.Check.str("display fold retains reheaded anchor identity", H.display_key(headed("A", "fold:B")), "fold:B")
    C.Check.str("display memo remains anchor eligible", H.display_key(memo), H.key(memo))
    C.Check.str("display live remains anchor eligible", H.display_key(live), H.key(live))
    C.Check.str("display sending remains anchor eligible", H.display_key(sending), H.key(sending))
    C.Check.str("display parent control excluded from anchors", H.display_key(D.El{"div", "parent", Nil{}, Nil{}}), "control:parent")
    C.Check.str("display work header excluded from anchors", H.display_key(D.El{"div", "work", Nil{}, Nil{}}), "control:work")
    C.Check.str("display fab header excluded from anchors", H.display_key(D.El{"div", "fab", Nil{}, Nil{}}), "control:fab")
    C.Check.str("display unkeyed control keeps a control prefix", H.display_key(before), "control:" ++ H.key(before))
    same("duplicate retained prefix entry is emitted once", H.project(tl([a, a, b]), tl([c]), "newer"), tl([a, b, c]))
    same("duplicate retained suffix entry is emitted once", H.project(tl([b, c, c]), tl([a]), "older"), tl([a, b, c]))
    same("duplicate retained reheaded folds are emitted once", H.project(tl([headed("A", "fold:B"), headed("A", "fold:B"), c]), tl([d]), "newer"), tl([headed("A", "fold:B"), c, d]))
    +nowLive : D.Node = D.El{"div", "a", [D.At{"class", "msg live"}, D.At{"data-id", "a"}], [D.Txt{"fresh live"}]}
    same("fresh transient identity wins over retained historical prefix", H.project(tl([a, b]), tl([c, nowLive]), "newer"), tl([b, c, nowLive]))
    same("fresh transient identity wins over retained historical suffix", H.project(tl([a, b]), tl([c, nowLive]), "older"), tl([c, b, nowLive]))
    same("multiple fresh controls keep their multiplicity and positions", H.project(tl([a]), tl([before, before, b, after, after]), "newer"), tl([before, before, a, b, after, after]))
    +memoAct : D.Node = D.El{"bp-memo", "tool", [D.At{"fn", "act"}, D.At{"tone", "tool"}, D.At{"kind", "read"}], [D.Txt{"fresh tool"}]}
    +madeAct : D.Node = D.El{"div", "tool", [D.At{"class", "act tool"}, D.At{"data-id", "tool"}], [D.Txt{"made tool"}]}
    C.Check.str("act display identity is invariant under memo expansion", H.display_key(memoAct), H.display_key(madeAct))
    same("canonical act duplicates deduplicate before expansion", H.project(tl([a]), tl([memoAct, memoAct, b]), "newer"), tl([a, memoAct, b]))
    same("retained canonical acts deduplicate across complete history", H.project(tl([memoAct, memoAct, b]), tl([c]), "newer"), tl([memoAct, b, c]))
    same("fresh expanded act replaces canonical retained act", H.project(tl([a, memoAct, b]), tl([madeAct, b]), "update"), tl([a, madeAct, b]))
    same("fresh canonical act replaces expanded retained act", H.project(tl([a, madeAct, b]), tl([memoAct, b]), "update"), tl([a, memoAct, b]))
    same("canonical and expanded fresh duplicate entry emitted once", H.project(tl([a]), tl([memoAct, madeAct, b]), "newer"), tl([a, memoAct, b]))
    +controlA : D.Node = D.Dom.button("same", "first-action", "same-value", "", "First")
    +controlB : D.Node = D.Dom.button("same", "second-action", "same-value", "", "Second")
    C.Check.str("different control actions yield different display identities", Bool.pick(String, String.eq(H.display_key(controlA), H.display_key(controlB)), "same", "different"), "different")
    +transported : D.Node = D.El{"button", "", [D.At{"class", "same"}, D.At{"data-value", "same-value"}, D.At{"data-on-click", "first-action"}], [D.Txt{"First"}]}
    C.Check.str("transported event has same control identity", H.display_key(transported), H.display_key(controlA))
    same("different-action controls both survive projection", H.project(tl([a]), tl([controlA, b, controlB]), "newer"), tl([controlA, a, b, controlB]))
    C.Check.str("identical control occurrences get distinct positional keys", Bool.pick(String, String.eq(H.display_key_at(before, 1n), H.display_key_at(before, 2n)), "same", "different"), "different")
    C.Check.str("positional control key retains exclusion prefix", H.display_key_at(before, 3n), H.display_key(before) ++ ":3")
    C.Check.str("message display key survives prepend positions", H.display_key_at(a, 1n), H.display_key_at(a, 200n))
    C.Check.str("reheaded fold display key survives prepend positions", H.display_key_at(headed("A", "fold:B"), 300n), "fold:B")
    C.Check.str("live display key survives prepend positions", H.display_key_at(live, 1n), H.display_key_at(live, 200n))
    C.Check.str("canonical act display key survives prepend positions", H.display_key_at(memoAct, 1n), H.display_key_at(madeAct, 200n))
