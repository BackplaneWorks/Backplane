import Base
import ./check.bend as C
import ../src/core/history-window.bend as H

def show(ids: List<&2, String>) -> String:
  String.join(ids, ",")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("page adopts disjoint older rows with retained viewport", show(H.History.page(["a", "b"], ["p", "q"], ["a", "b"], ["a"], True{})), "p,q,a,b")
    C.Check.str("retention row boundary is accepted", Bool.show(H.History.within(260n, 1048576n)), "True")
    C.Check.str("retention row overflow parks", Bool.show(H.History.within(261n, 1n)), "False")
    C.Check.str("retention byte overflow parks", Bool.show(H.History.within(1n, 1048577n)), "False")
    C.Check.str("older page keeps outgoing anchor and tail", show(H.History.project(["a", "b", "c"], ["p", "a"], ["a", "b", "c"], True{})), "p,a,b,c")
    C.Check.str("older page keeps visible rows beyond overlap", show(H.History.project(["a", "b", "c", "d"], ["p", "a", "b"], ["b", "c", "d"], True{})), "p,a,b,c,d")
    C.Check.str("newer page keeps outgoing visible prefix", show(H.History.project(["p", "a", "b"], ["b", "c", "d"], ["p", "a", "b"], False{})), "p,a,b,c,d")
    C.Check.str("offscreen far tail can retire", show(H.History.project(["a", "b", "c", "d"], ["p", "a"], ["a", "b"], True{})), "p,a,b")
    C.Check.str("reheaded visible fold keeps one stable key", show(H.History.project(["fold:old", "m", "tail"], ["older", "fold:new", "m"], ["fold:old", "m", "tail"], True{})), "older,fold:old,m,tail")
    C.Check.str("unrelated fold does not acquire a bridge key", show(H.History.project(["fold:old", "old-message", "m"], ["fold:new", "m"], ["m"], True{})), "fold:new,m")
    C.Check.str("disjoint older page keeps visible range", show(H.History.project(["a", "b"], ["p", "q"], ["a", "b"], True{})), "p,q,a,b")
    C.Check.str("disjoint newer page keeps visible range", show(H.History.project(["p", "q"], ["a", "b"], ["p", "q"], False{})), "p,q,a,b")
    C.Check.str("shared anchor not duplicated", show(H.History.project(["a", "b"], ["p", "a", "b"], ["a", "b"], True{})), "p,a,b")
    C.Check.str("empty page keeps protected rows", show(H.History.project(["a", "b", "c"], Nil{}, ["b", "c"], True{})), "b,c")
    C.Check.str("no retention crosses an empty previous picture", show(H.History.project(Nil{}, ["a", "b"], Nil{}, True{})), "a,b")
    C.Check.str("page adoption cannot shrink old window", show(H.History.page(["a", "b", "c", "d"], ["p", "a"], ["a", "b"], ["a", "b"], True{})), "p,a,b,c,d")
    C.Check.str("reordered candidate cannot replace visible span", show(H.History.adopt(["a", "b", "c"], ["b", "a", "c"], ["a", "b"])), "a,b,c")
    C.Check.str("missing anchor cannot replace visible span", show(H.History.adopt(["a", "b"], ["p", "q"], ["a"])), "a,b")
    C.Check.str("same-key viewport endpoints are valid", Bool.show(H.History.viewport(["a"], "a", "a", "a")), "True")
    C.Check.str("reversed viewport endpoints rejected", Bool.show(H.History.viewport(["a", "b"], "b", "a", "a")), "False")
