import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/cbor.bend as B
import ../src/core/utf8.bend as U

def bytes.go(bs: List<&2, U32>, acc: String) -> String:
  match bs:
    case Nil{}:
      acc
    case b <> t:
      bytes.go(t, acc ++ "," ++ U32.show(b))

# "a,b,c" for a byte list
def bytes(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      ""
    case b <> t:
      bytes.go(t, U32.show(b))

def show(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      "<none>"
    case Some{j}:
      J.Json.show(j)

def parse(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

# encode then decode, shown as JSON text
def rt(+j: J.Json) -> String:
  show(B.Cbor.decode(B.Cbor.encode(j)))

def enc(s: String) -> String:
  bytes(B.Cbor.encode(parse(s)))

def dec(bs: List<&2, U32>) -> String:
  show(B.Cbor.decode(bs))

def u32len(xs: List<&2, U32>, acc: U32) -> U32:
  match xs:
    case Nil{}:
      acc
    case _ <> t:
      u32len(t, (acc + 1 : U32))

# one event of a realistic change stream, by position
def event.k(k: Nat, +i: Nat, +at: String, +th: String) -> String:
  match k:
    case 0n:
      "{\"$\":\"MessagePosted\"," ++ th ++ ",\"msg\":\"m" ++ at ++ "-" ++ Nat.show(i) ++ "\",\"role\":\"user\",\"text\":\"Route the USB differential pair on layer 2 and keep it away from the buck converter.\",\"at\":" ++ at ++ "}"
    case 1n:
      "{\"$\":\"TurnChanged\"," ++ th ++ ",\"state\":\"running\",\"at\":" ++ at ++ "}"
    case 2n:
      "{\"$\":\"ActivityLogged\"," ++ th ++ ",\"act\":\"a" ++ at ++ "-" ++ Nat.show(i) ++ "\",\"tone\":\"tool\",\"kind\":\"edit\",\"summary\":\"Edited board/main.kicad_pcb\",\"at\":" ++ at ++ "}"
    case _:
      "{\"$\":\"MessagePosted\"," ++ th ++ ",\"msg\":\"m" ++ at ++ "-" ++ Nat.show(i) ++ "\",\"role\":\"assistant\",\"text\":\"Done. The pair now runs on In1.Cu with 90 ohm impedance; clearance to the switch node is 2.5 mm.\",\"at\":" ++ at ++ "}"

def event(+i: Nat) -> String:
  event.k(Nat.mod(i, 4n), i, Nat.show(Nat.add(1727000000n, i)), "\"thread\":\"t1727000000-3\"")

def events(n: Nat, +i: Nat) -> String:
  match n:
    case 0n:
      ""
    case 1n+0n:
      event(i)
    case 1n+m:
      event(i) ++ "," ++ events(m, 1n+i)

def changes(n: Nat) -> String:
  "{\"t\":\"changes\",\"items\":[" ++ events(n, 0n) ++ "]}"

def size(name: String, +j: J.Json) -> IO(Unit):
  IO.print("size " ++ name ++ ": json " ++ U32.show(u32len(U.Utf8.encode(J.Json.show(j)), 0)) ++ " bytes, cbor " ++ U32.show(u32len(B.Cbor.encode(j), 0)) ++ " bytes")

# the tree's answer for each probe equals Cbor.find's in the list
def same.find(xs: List<&2, String>, +names: List<&2, String>, +t: B.CTree, ok: Bool) -> Bool:
  match xs:
    case Nil{}:
      ok
    case +x <> rest:
      same.find(rest, names, t, Bool.and(ok, U32.is_eq(Maybe.default(&2, U32, B.Cbor.find(names, x), 999), Maybe.default(&2, U32, B.CTree.find(t, x), 999))))

def probes() -> List<&2, String>:
  List.append(&2, String, List.append(&2, String, B.Cbor.keys(), B.Cbor.words()),
    ["", "a", "zz", "User", "user ", "Thread", "ThreadZ", "~", "\u{1F600}", "itemsx", "$$", "assistan", "assistantt", "Message", "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"])

# the one-walk Enc.text against the two walks it replaced
def text.old(+s: String) -> List<&2, U32>:
  B.Enc.utf8(s, B.Enc.head(96, B.Enc.blen(s, 0), [7]))

def text.same(xs: List<&2, String>, ok: Bool) -> Bool:
  match xs:
    case Nil{}:
      ok
    case +x <> t:
      text.same(t, Bool.and(ok, String.eq(bytes(List.reverse(&2, U32, B.Enc.text(x, [7]))), bytes(List.reverse(&2, U32, text.old(x))))))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Enc.text in one walk is the two walks' bytes", Bool.show(text.same(["", "a", "hello world", "é", "日本語", "😀x", "a\"b\\c\n", "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", "mixed é 日 😀 end"], True{})), "True")
    C.Check.str("trees hold the lists", Bool.show(Bool.and(B.CTree.ok(B.Cbor.keys(), B.Cbor.keys.tree()), B.CTree.ok(B.Cbor.words(), B.Cbor.words.tree()))), "True")
    C.Check.str("tree finds as the list (keys)", Bool.show(same.find(probes(), B.Cbor.keys(), B.Cbor.keys.tree(), True{})), "True")
    C.Check.str("tree finds as the list (words)", Bool.show(same.find(probes(), B.Cbor.words(), B.Cbor.words.tree(), True{})), "True")
    C.Check.str("null", enc("null"), "246")
    C.Check.str("true", enc("true"), "245")
    C.Check.str("false", enc("false"), "244")
    C.Check.str("uint small", enc("23"), "23")
    C.Check.str("uint 1 byte", enc("24"), "24,24")
    C.Check.str("uint 2 bytes", enc("1000"), "25,3,232")
    C.Check.str("uint 4 bytes", enc("1727000000"), "26,102,239,237,192")
    C.Check.str("uint max", enc("4294967295"), "26,255,255,255,255")
    C.Check.str("neg", enc("-1"), "32")
    C.Check.str("neg 1 byte", enc("-500"), "57,1,243")
    C.Check.str("float as tag 7", enc("1.5"), "199,99,49,46,53")
    C.Check.str("text", enc("\"hi\""), "98,104,105")
    C.Check.str("word", enc("\"assistant\""), "198,7")
    C.Check.str("reply", enc("{\"t\":\"reply\",\"id\":3,\"ok\":true}"), "163,1,198,5,2,3,5,245")
    C.Check.str("plain key", enc("{\"zz\":[]}"), "161,98,122,122,128")
    C.Check.str("rt scalars", rt(parse("[null,true,false,0,7,4294967295,-1,-4294967296,\"\",\"x\"]")), "[null,true,false,0,7,4294967295,-1,-4294967296,\"\",\"x\"]")
    C.Check.str("rt odd nums", rt(parse("[1.5,-2.5e3,4294967296,-4294967297,99999999999999999999,-0,007,1e10]")), "[1.5,-2.5e3,4294967296,-4294967297,99999999999999999999,-0,007,1e10]")
    C.Check.str("rt odd raw", rt(J.Json.arr([J.Num{""}, J.Num{"-"}, J.Num{"12a"}])), "[,-,12a]")
    C.Check.str("rt words", rt(parse("[\"MessagePosted\",\"completed\",\"cancelled\",\"user \",\"User\"]")), "[\"MessagePosted\",\"completed\",\"cancelled\",\"user \",\"User\"]")
    C.Check.str("rt keys", rt(parse("{\"$\":1,\"archived\":2,\"zz\":3,\"text\":4,\"\":5}")), "{\"$\":1,\"archived\":2,\"zz\":3,\"text\":4,\"\":5}")
    C.Check.str("rt nested", rt(parse("{\"a\":[[],{},[{\"b\":[1,[2,[3]]]}],{\"c\":{\"d\":{}}}],\"e\":null}")), "{\"a\":[[],{},[{\"b\":[1,[2,[3]]]}],{\"c\":{\"d\":{}}}],\"e\":null}")
    C.Check.str("rt unicode", rt(J.Json.str("héllo → 世界 🎛")), "\"héllo → 世界 🎛\"")
    C.Check.str("unicode len", enc("\"é\""), "98,195,169")
    C.Check.str("rt long text", rt(J.Json.str(String.repeat("x", 300n))), J.Json.show(J.Json.str(String.repeat("x", 300n))))
    C.Check.str("rt changes", rt(parse(changes(20n))), J.Json.show(parse(changes(20n))))
    C.Check.str("empty", dec([]), "<none>")
    C.Check.str("truncated arg", dec([25, 3]), "<none>")
    C.Check.str("truncated text", dec([99, 104, 105]), "<none>")
    C.Check.str("truncated array", dec([130, 1]), "<none>")
    C.Check.str("truncated map", dec([161, 1]), "<none>")
    C.Check.str("8-byte arg", dec([27, 0, 0, 0, 0, 0, 0, 0, 1]), "<none>")
    C.Check.str("indefinite", dec([159, 1, 255]), "<none>")
    C.Check.str("float16", dec([249, 60, 0]), "<none>")
    C.Check.str("bytes major", dec([66, 1, 2]), "<none>")
    C.Check.str("trailing", dec([1, 2]), "<none>")
    C.Check.str("bad word", dec([198, 24, 200]), "<none>")
    C.Check.str("bad key", dec([161, 24, 200, 1]), "<none>")
    C.Check.str("array key", dec([161, 128, 1]), "<none>")
    C.Check.str("tag 7 on int", dec([199, 1]), "<none>")
    C.Check.str("other tag", dec([193, 1]), "<none>")
    size("changes x20", parse(changes(20n)))
