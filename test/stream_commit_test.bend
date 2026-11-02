import Base
import ./check.bend as C
import ../src/core/stream-commit.bend as SC
import ../src/core/thin-live.bend as TL
import ../src/core/live.bend as LV
import ../src/core/steer.bend as STR
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Constructive identity for ANY change list with no Live.ended result,
# arbitrary final buffers, commit version and existing map. In particular
# this covers metadata without enumerating its constructors in the helper.
def metadataNoEnds(+cs: List<&2, M.Change>, +bs: List<&2, LV.Buf>, +version: Nat, +streams: Map<&2, TL.Stream>, none: {SC.Streams.any(cs) == False{} : Bool}) ->
    {SC.Streams.commit(cs, bs, version, streams) == streams : Map<&2, TL.Stream>}:
  Equal.cong(Bool, Map<&2, TL.Stream>, ending => SC.Streams.commit.when(ending, cs, bs, version, streams),
    SC.Streams.any(cs), False{}, none)

def old() -> List<&2, LV.Buf>:
  [LV.Buf{"a", ["unfinished λ🙂", "first\n"], 19n, False{}, 7n},
    LV.Buf{"b", ["尾🙂", "kept "], 7n, True{}, 9n},
    LV.Buf{"c", Nil{}, 0n, False{}, 11n}]

def existing() -> Map<&2, TL.Stream>:
  TL.Streams.snapshot([LV.Buf{"parked", ["no buffer now"], 13n, False{}, 4n}], 3n,
    TL.Streams.snapshot([LV.Buf{"b", ["尾🙂", "kept "], 7n, True{}, 9n}], 12n,
      TL.Streams.snapshot(old(), 10n, Map.new(&2, TL.Stream))))

def final(+cs: List<&2, M.Change>) -> List<&2, LV.Buf>:
  STR.Steer.keep(cs, old(), LV.Live.ends(cs, old()))

def committed(+cs: List<&2, M.Change>) -> Map<&2, TL.Stream>:
  SC.Streams.commit(cs, final(cs), 99n, existing())

# Inspect generation, scalar size, cut flag, text and version independently
# of Map shape. The full snapshot is the oracle for every affected slot.
def signature(s: TL.Stream) -> String:
  match s:
    case TL.Stream{+gen, +size, +blocks, +cut, +version}:
      J.Json.show(J.Json.arr([J.Json.nat(gen), J.Json.nat(size), J.Json.bool(cut),
        J.Str{TL.Stream.preview(TL.Stream{gen, size, blocks, cut, version})}, J.Json.nat(version)]))

def slot(+streams: Map<&2, TL.Stream>, thread: String) -> String:
  signature(TL.Stream.get(streams, thread))

def has.result(result: Map<&2, TL.Stream> & Bool) -> Bool:
  (_, yes) = result
  yes

def has(streams: Map<&2, TL.Stream>, thread: String) -> Bool:
  has.result(Map.has(&2, TL.Stream, streams, thread))

def affected(cs: List<&2, M.Change>, thread: String) -> Bool:
  SC.Streams.member(Set.has(SC.Streams.affected(cs), thread))

def check(name: String, yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(yes), "True")

def parity.slot(+name: String, +cs: List<&2, M.Change>, +thread: String) -> IO(Unit):
  do IO<Unit>:
    +expected : Map<&2, TL.Stream> = Bool.pick(Map<&2, TL.Stream>, affected(cs, thread),
      TL.Streams.snapshot(final(cs), 99n, Map.new(&2, TL.Stream)), existing())
    C.Check.str(name ++ " " ++ thread, slot(committed(cs), thread), slot(expected, thread))
    C.Check.str(name ++ " " ++ thread ++ " presence", Bool.show(has(committed(cs), thread)), Bool.show(has(expected, thread)))

def parity(+name: String, +cs: List<&2, M.Change>) -> IO(Unit):
  do IO<Unit>:
    parity.slot(name, cs, "a")
    parity.slot(name, cs, "b")
    parity.slot(name, cs, "c")
    parity.slot(name, cs, "parked")
    C.Check.str(name ++ " slot count", Nat.show(Map.size(&2, TL.Stream, committed(cs))),
      Bool.pick(String, affected(cs, "parked"), "3", "4"))

def metadata() -> List<&2, M.Change>:
  [M.SettingSet{"liveness.a", "42", 42n},
    M.ActivityLogged{"b", "act", "tool", "Bash", "working", 42n},
    M.UsageSet{"a", 10n, 20n, 42n},
    M.TurnUsage{"b", "claude", "model", "usage", 1n, 2n, 3n, 4n, 5n, 6n, "known", "reported", 42n},
    M.TurnChanged{"a", M.Running{}, 42n},
    M.MessagePosted{"a", "user", M.User{}, "continue", 42n}]

def ahead() -> List<&2, M.Change>:
  [M.MessagePosted{"a", "user:before", M.Assistant{}, "first\n", 42n},
    M.MessagePosted{"a", "user", M.User{}, "continue", 42n}]

def batch() -> List<&2, M.Change>:
  [M.TurnChanged{"b", M.Failed{}, 42n},
    M.MessagePosted{"a", "done", M.Assistant{}, "first\n", 42n},
    M.SettingSet{"liveness.c", "42", 42n},
    M.TurnChanged{"a", M.Completed{}, 42n},
    M.TurnChanged{"c", M.Interrupted{}, 42n}]

def main() -> IO(Unit):
  do IO<Unit>:
    parity("empty changes", Nil{})
    parity("metadata preserves every slot and version", metadata())
    check("metadata has no endings", Bool.not(SC.Streams.any(metadata())))
    C.Check.str("metadata affected set empty", Nat.show(Set.size(SC.Streams.affected(metadata()))), "0")
    C.Check.str("metadata does not populate an empty map", Nat.show(Map.size(&2, TL.Stream,
      SC.Streams.commit(metadata(), old(), 99n, Map.new(&2, TL.Stream)))), "0")
    parity("assistant finish clears only its own stream", [M.MessagePosted{"a", "done", M.Assistant{}, "done", 42n}])
    parity("completed clears only its own stream", [M.TurnChanged{"a", M.Completed{}, 42n}])
    parity("failed clears only its own stream", [M.TurnChanged{"a", M.Failed{}, 42n}])
    parity("interrupted clears only its own stream", [M.TurnChanged{"a", M.Interrupted{}, 42n}])
    C.Check.str("terminal stream is empty", TL.Stream.preview(TL.Stream.get(committed([M.TurnChanged{"a", M.Completed{}, 42n}]), "a")), "")
    C.Check.str("terminal stream bumps generation", slot(committed([M.TurnChanged{"a", M.Completed{}, 42n}]), "a"), "[8,0,false,\"\",99]")
    parity("ahead assistant uses final retained tail", ahead())
    C.Check.str("ahead unfinished Unicode tail retained", TL.Stream.preview(TL.Stream.get(committed(ahead()), "a")), "unfinished λ🙂")
    parity("multiple endings including repeat", batch())
    parity("multiple endings in reverse order", List.reverse(&2, M.Change, batch()))
    C.Check.str("repeated endings retain final generation", slot(committed(batch()), "a"), "[9,0,false,\"\",99]")
    C.Check.str("affected IDs deduplicated", Nat.show(Set.size(SC.Streams.affected(batch()))), "3")
    check("affected IDs include assistant", affected(batch(), "a"))
    check("affected IDs include failed", affected(batch(), "b"))
    check("affected IDs include interrupted", affected(batch(), "c"))
    check("affected IDs exclude empty sentinel", Bool.not(affected(batch(), "")))
    check("affected IDs exclude parked", Bool.not(affected(batch(), "parked")))
    parity("absent final buffer deletes affected existing slot", [M.TurnChanged{"parked", M.Completed{}, 42n}])
    check("affected stale slot removed", Bool.not(has(committed([M.TurnChanged{"parked", M.Completed{}, 42n}]), "parked")))
    parity("mixed present and absent affected buffers", [M.TurnChanged{"parked", M.Completed{}, 42n}, M.TurnChanged{"a", M.Failed{}, 42n}])
    parity("absent thread never creates slot", [M.TurnChanged{"missing", M.Completed{}, 42n}])
    check("missing thread has no phantom slot", Bool.not(has(committed([M.TurnChanged{"missing", M.Completed{}, 42n}]), "missing")))
    C.Check.str("no buffers never creates slot", Nat.show(Map.size(&2, TL.Stream,
      SC.Streams.commit([M.TurnChanged{"missing", M.Completed{}, 42n}], Nil{}, 99n, Map.new(&2, TL.Stream)))), "0")
    +without : Map<&2, TL.Stream> = SC.Streams.commit([M.TurnChanged{"a", M.Completed{}, 42n}], Nil{}, 99n, existing())
    check("empty final buffers delete affected slot", Bool.not(has(without, "a")))
    C.Check.str("empty final buffers preserve unrelated slots", Nat.show(Map.size(&2, TL.Stream, without)), "3")
    C.Check.str("unaffected slot without buffer retains version and text", slot(without, "parked"), slot(existing(), "parked"))
    parity("empty thread ending is ignored", [M.TurnChanged{"", M.Completed{}, 42n}])
    check("empty thread ending has no affected ID", Bool.not(SC.Streams.any([M.TurnChanged{"", M.Completed{}, 42n}])))
    +mixed : List<&2, M.Change> = List.append(&2, M.Change, metadata(), ahead())
    parity("metadata before ahead ending", mixed)
    parity("duplicate ahead messages snapshot final buffer once", List.append(&2, M.Change, ahead(), ahead()))
    +kept : Map<&2, TL.Stream> = SC.Streams.commit([M.TurnChanged{"a", M.Completed{}, 42n}],
      LV.Live.add(final([M.TurnChanged{"a", M.Completed{}, 42n}]), "b", "not committed here"), 99n, existing())
    C.Check.str("unaffected final buffer never snapshotted", slot(kept, "b"), slot(existing(), "b"))
