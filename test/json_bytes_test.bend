import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/cbor.bend as B
import ../src/core/utf8.bend as U

def len(xs: List<&2, U32>, +acc: Nat) -> Nat:
  match xs:
    case Nil{}:
      acc
    case _ <> t:
      len(t, Nat.add(acc, 1n))

def same.bytes(xs: List<&2, U32>, ys: List<&2, U32>, ok: Bool) -> Bool:
  match xs ys:
    case Nil{} Nil{}:
      ok
    case x <> xt y <> yt:
      same.bytes(xt, yt, Bool.and(ok, U32.is_eq(x, y)))
    case _ _:
      False{}

# Json.bytes is the UTF-8 size of the printed text, and the tree's CBOR is
# the CBOR of the printed text parsed again (what the hub sent before)
def one(+name: String, +j: J.Json) -> IO(Unit):
  do IO<Unit>:
    C.Check.str(name ++ " size", Nat.show(J.Json.bytes(j)), Nat.show(len(U.Utf8.encode(J.Json.show(j)), 0n)))
    C.Check.str(name ++ " cbor", Bool.show(same.bytes(B.Cbor.encode(j), B.Cbor.encode(Maybe.default(&2, J.Json, J.Json.parse(J.Json.show(j)), J.Null{})), True{})), "True")

def text(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    one("null", J.Null{})
    one("flags", J.Json.arr([J.Flag{True{}}, J.Flag{False{}}, J.Null{}]))
    one("empty", J.Json.obj([J.KV{"a", J.Json.arr(Nil{})}, J.KV{"b", J.Json.obj(Nil{})}, J.KV{"", J.Str{""}}]))
    one("nums", J.Json.arr([J.Num{"0"}, J.Num{"-12"}, J.Num{"1.5e3"}, J.Json.nat(Nat.mul(65536n, 65536n))]))
    one("escapes", J.Str{"a\"b\\c\nd\re\tf" ++ SCon{Chr{1}, SNil{}} ++ SCon{Chr{31}, SNil{}} ++ "/"})
    one("unicode", J.Json.obj([J.KV{"é ключ", J.Str{"naïve — 日本語 😀 " ++ SCon{Chr{2047}, SNil{}} ++ SCon{Chr{2048}, SNil{}} ++ SCon{Chr{65535}, SNil{}} ++ SCon{Chr{65536}, SNil{}}}}]))
    one("words", J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"140737488355329"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"zz", J.Str{"assistant"}}]))
    one("nested", text("{\"t\":\"reply\",\"result\":{\"shown\":{\"tag\":\"div\",\"attrs\":[[\"class\",\"thread\"],[\"id\",\"timeline\"]],\"kids\":[{\"text\":\"Hello\\nthere \\\"you\\\"\"},[],{}]},\"window\":{\"ids\":[\"1\",\"2\"],\"hasMore\":false,\"partial\":[]},\"cmds\":[],\"remoteReady\":true}}"))
