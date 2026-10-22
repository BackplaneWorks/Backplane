import Base
import ../../src/server/host.bend as H
import ../../src/core/http.bend as W
import ../../src/core/cbor.bend as CB
import ../../src/server/kicad.bend as K
import ../../src/core/diff.bend as DF
import ../../src/core/outbox.bend as OB
import ../../src/server/argv.bend as AV

# host.c's Json.cbor_frame (the bytes Sock.send_cbor sends) against Bend's
# W.Ws.cbor, the same frame: every line of an event log wrapped as a join
# chunk, and edge cases (numbers at the edges of 32 bits, escapes, words).
# Then Json.items_frames (the bytes Sock.send_items sends) against
# OB.Items.texts.s's frames through W.Ws.cbor: the log's lines in the hub's
# chunks of 64, with the writer's limits and with tight ones that split
# them, the last frame's "boot" both ways.
#   scripts/build-js.sh test/native/cbor_frame_test.bend build/cbor_frame_test
#   build/cbor_frame_test EVENTS.jsonl

def Cf.keys() -> String:
  String.join(CB.Cbor.keys(), "\n")

def Cf.words() -> String:
  String.join(CB.Cbor.words(), "\n")

def Cf.same(a: List<&2, U32>, b: List<&2, U32>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case x <> s y <> t:
      Bool.and(U32.is_eq(x, y), Cf.same(s, t))
    case _ _:
      False{}

def Cf.one(+t: String) -> IO(Nat):
  do IO<Nat>:
    got : List<&2, U32> <- H.Json.cbor_frame(t, Cf.keys(), Cf.words())
    return Bool.pick(Nat, Cf.same(got, W.Ws.cbor(t)), 0n, 1n)

def Cf.report(+bad: Nat, +t: String) -> IO(Unit):
  Bool.pick(IO(Unit), Nat.is_eq(bad, 0n), IO.pure(Unit, Unit{}), IO.print("DIFF " ++ String.take(t, 300n)))

def Cf.then(+b: Nat, +t: String, +bad: Nat, k: Nat -> IO(Nat)) -> IO(Nat):
  IO.bind(Unit, Nat, Cf.report(b, t), u => k(Nat.add(bad, b)))

def Cf.all(ts: List<&2, String>, +bad: Nat) -> IO(Nat):
  match ts:
    case Nil{}:
      IO.pure(Nat, bad)
    case +t <> rest:
      IO.bind(Nat, Nat, Cf.one(t), b => Cf.then(b, t, bad, n => Cf.all(rest, n)))

def Cf.wrap.k(empty: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      ("{\"t\":\"changes\",\"boot\":true,\"items\":[" ++ l ++ "]}") <> rest

def Cf.wrap(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> rest:
      Cf.wrap.k(String.is_empty(l), l, Cf.wrap(rest))

def Cf.edges() -> List<&2, String>:
  ["0", "-0", "-1", "7", "23", "24", "255", "256", "65535", "65536", "4294967295", "4294967296", "-4294967295", "-4294967296", "007", "1.5", "-2.25", "1e3", "12345678901",
   "\"\"", "\"MessagePosted\"", "\"t\"", "\"a\\\"b\\\\c\\/d\\n\\t\\r\\b\\f\"", "\"\\u00e9\\u4e2d\\ud83d\\ude00\"", "\"é中😀\"", "true", "false", "null",
   "[]", "{}", "[1,[2,[3,{}]]]", "{\"$\":\"SettingSet\",\"key\":\"x\",\"value\":\"y\",\"at\":1791104049}", "{\"zzz\":1,\"t\":\"log\"}",
   " { \"a\" : [ 1 , 2 ] } "]

def Cf.path(xs: List<String>) -> String:
  match xs:
    case Nil{}:
      ""
    case +p <> Nil{}:
      p
    case _ <> t:
      Cf.path(t)

def Cf.hd() -> String:
  "{\"t\":\"log\",\"since\":0,\"origin\":\"x\",\"version\":\"v\""

def Cf.flat(fs: List<&2, String>) -> List<&2, U32>:
  match fs:
    case Nil{}:
      Nil{}
    case +t <> rest:
      List.append(&2, U32, W.Ws.cbor(t), Cf.flat(rest))

def Cf.items.one(+xs: List<&2, String>, +ends: Bool, +most: Nat, +bytes: Nat) -> IO(Nat):
  do IO<Nat>:
    got : List<&2, U32> <- H.Json.items_frames(Cf.hd(), ",\"items\":[", ",\"more\":true,\"items\":[", xs, "]}", Bool.pick(U32, ends, 1, 0), U32.from_nat(most), U32.from_nat(bytes), Cf.keys(), Cf.words())
    return Bool.pick(Nat, Cf.same(got, Cf.flat(OB.Items.texts.s(Cf.hd(), ",\"items\":[", ",\"more\":true,\"items\":[", xs, "]}", ends, most, bytes))), 0n, 1n)

def Cf.groups(fuel: Nat, +ls: List<&2, String>, +n: Nat) -> List<&2, List<&2, String>>:
  match fuel ls:
    case _ Nil{}:
      Nil{}
    case 0n _:
      [ls]
    case 1n+f _:
      List.take(&2, String, ls, n) <> Cf.groups(f, List.drop(&2, String, ls, n), n)

def Cf.items.all(gs: List<&2, List<&2, String>>, +i: Nat, +most: Nat, +bytes: Nat, +bad: Nat) -> IO(Nat):
  match gs:
    case Nil{}:
      IO.pure(Nat, bad)
    case +g <> rest:
      IO.bind(Nat, Nat, Cf.items.one(g, Nat.is_eq(Nat.mod(i, 2n), 0n), most, bytes), b => Cf.items.all(rest, 1n+i, most, bytes, Nat.add(bad, b)))

def Cf.solid.k(empty: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      l <> rest

def Cf.solid(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> rest:
      Cf.solid.k(String.is_empty(l), l, Cf.solid(rest))

def Cf.items.report(+what: String, +bad: Nat, +n: Nat) -> IO(Unit):
  IO.print("items, " ++ what ++ ": " ++ Nat.show(bad) ++ " differ of " ++ Nat.show(n))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +path : String = Cf.path(args)
    e : Nat <- Cf.all(Cf.edges(), 0n)
    IO.print("edge cases: " ++ Nat.show(e) ++ " differ of " ++ Nat.show(List.length(&2, String, Cf.edges())))
    +text : String <- K.Io.text(path)
    +ls : List<&2, String> = Cf.wrap(DF.Diff.lines(text))
    d : Nat <- Cf.all(ls, 0n)
    IO.print("log lines: " ++ Nat.show(d) ++ " differ of " ++ Nat.show(List.length(&2, String, ls)))
    +gs : List<&2, List<&2, String>> = Cf.groups(List.length(&2, String, ls), Cf.solid(DF.Diff.lines(text)), 64n)
    a : Nat <- Cf.items.all([Nil{}, Cf.edges(), Cf.edges()], 0n, 3n, 40n, 0n)
    Cf.items.report("edge cases", a, 3n)
    b : Nat <- Cf.items.all(gs, 0n, OB.Join.most(), OB.Join.bytes(), 0n)
    Cf.items.report("log chunks", b, List.length(&2, List<&2, String>, gs))
    c : Nat <- Cf.items.all(gs, 1n, 7n, 4000n, 0n)
    Cf.items.report("log chunks, tight", c, List.length(&2, List<&2, String>, gs))
