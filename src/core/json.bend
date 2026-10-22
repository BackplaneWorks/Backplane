import Base

# JSON as one self-recursive tree, so every walk is plain structural
# recursion (Bend forbids mutual recursion). Arrays and objects hold a chain
# of Item / Field cells ending in End. Numbers keep their source text, so a
# parse and a print never lose digits.

type Json is Data:
  Null{}
  Flag{value: Bool}
  Num{raw: String}
  Str{text: String}
  Arr{items: Json}
  Obj{fields: Json}
  Item{head: Json, tail: Json}
  Field{key: String, value: Json, tail: Json}
  End{}

# one key/value pair of an object under construction
type KV is Data:
  KV{key: String, val: Json}

# Building
# --------

def Json.str(s: String) -> Json:
  Str{s}

def Json.u32(n: U32) -> Json:
  Num{U32.show(n)}

def Json.nat(n: Nat) -> Json:
  Num{Nat.show(n)}

def Json.bool(b: Bool) -> Json:
  Flag{b}

# Lists and chains convert through a reversed accumulator: tail calls
# only, since a log can hold thousands of items and a frame per item
# overflowed the phone's JavaScriptCore

def Json.items.go(xs: List<&2, Json>, acc: Json) -> Json:
  match xs:
    case Nil{}:
      acc
    case h <> t:
      Json.items.go(t, Item{h, acc})

def Json.items(xs: List<&2, Json>) -> Json:
  Json.items.go(List.reverse(&2, Json, xs), End{})

def Json.arr(xs: List<&2, Json>) -> Json:
  Arr{Json.items(xs)}

def Json.fields.go(kvs: List<&2, KV>, acc: Json) -> Json:
  match kvs:
    case Nil{}:
      acc
    case KV{k, v} <> t:
      Json.fields.go(t, Field{k, v, acc})

def Json.fields(kvs: List<&2, KV>) -> Json:
  Json.fields.go(List.reverse(&2, KV, kvs), End{})

def Json.obj(kvs: List<&2, KV>) -> Json:
  Obj{Json.fields(kvs)}

# Reading
# -------

# the head's key hit, computed by the caller, rides along as `hit`
def Json.find.hit(fs: Json, +key: String) -> Bool:
  match fs:
    case Field{k, _, _}:
      String.eq(k, key)
    case _:
      False{}

def Json.find.go(fs: Json, hit: Bool, +key: String) -> Maybe<&2, Json>:
  match fs hit:
    case Field{_, v, _} True{}:
      Some{v}
    case Field{_, _, +t} False{}:
      Json.find.go(t, Json.find.hit(t, key), key)
    case _ _:
      None{}

# find a key in a field chain; first match wins
def Json.find(+fs: Json, +key: String) -> Maybe<&2, Json>:
  Json.find.go(fs, Json.find.hit(fs, key), key)

# the value at `key` of an object, or None
def Json.get(o: Json, +key: String) -> Maybe<&2, Json>:
  match o:
    case Obj{fs}:
      Json.find(fs, key)
    case _:
      None{}

def Json.as_str(m: Maybe<&2, Json>, d: String) -> String:
  match m:
    case Some{Str{s}}:
      s
    case _:
      d

def Json.as_raw(m: Maybe<&2, Json>, d: String) -> String:
  match m:
    case Some{Num{r}}:
      r
    case Some{Str{s}}:
      s
    case _:
      d

def Json.as_bool(m: Maybe<&2, Json>, d: Bool) -> Bool:
  match m:
    case Some{Flag{b}}:
      b
    case _:
      d

# the string at `key`, or d
def Json.get_str(o: Json, +key: String, d: String) -> String:
  Json.as_str(Json.get(o, key), d)

def Json.get_u32(o: Json, +key: String, d: U32) -> U32:
  Maybe.default(&2, U32, U32.read(Json.as_raw(Json.get(o, key), "")), d)

def Json.get_nat(o: Json, +key: String, d: Nat) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Json.as_raw(Json.get(o, key), "")), d)

def Json.get_bool(o: Json, +key: String, d: Bool) -> Bool:
  Json.as_bool(Json.get(o, key), d)

def Json.get_obj(o: Json, +key: String) -> Json:
  Maybe.default(&2, Json, Json.get(o, key), Null{})

def Json.to_list.go(xs: Json, acc: List<&2, Json>) -> List<&2, Json>:
  match xs:
    case Item{h, t}:
      Json.to_list.go(t, h <> acc)
    case _:
      acc

def Json.to_list(xs: Json) -> List<&2, Json>:
  List.reverse(&2, Json, Json.to_list.go(xs, Nil{}))

# the elements of an array, or []
def Json.elems(a: Json) -> List<&2, Json>:
  match a:
    case Arr{xs}:
      Json.to_list(xs)
    case _:
      Nil{}

# Printing
# --------

def Json.hex.off(lt: Bool) -> U32:
  match lt:
    case True{}:
      48
    case False{}:
      87

def Json.hex(+n: U32) -> Char:
  Chr{U32.add(n, Json.hex.off(U32.is_lt(n, 10)))}

def Json.esc.u(+c: U32, rest: String) -> String:
  "\\u00" ++ SCon{Json.hex(U32.shrn(c, 4n)), SCon{Json.hex(U32.and(c, 15)), rest}}

def Json.esc.ctl.k(n: Bool, r: Bool, t: Bool, +c: U32, rest: String) -> String:
  match n r t:
    case True{} _ _:
      SCon{'\\', SCon{'n', rest}}
    case False{} True{} _:
      SCon{'\\', SCon{'r', rest}}
    case False{} False{} True{}:
      SCon{'\\', SCon{'t', rest}}
    case False{} False{} False{}:
      Json.esc.u(c, rest)

def Json.esc.ctl(+c: U32, rest: String) -> String:
  Json.esc.ctl.k(U32.is_eq(c, 10), U32.is_eq(c, 13), U32.is_eq(c, 9), c, rest)

def Json.esc.chr.k(ctl: Bool, q: Bool, bs: Bool, +c: U32, rest: String) -> String:
  match ctl q bs:
    case True{} _ _:
      Json.esc.ctl(c, rest)
    case False{} True{} _:
      SCon{'\\', SCon{'"', rest}}
    case False{} False{} True{}:
      SCon{'\\', SCon{'\\', rest}}
    case False{} False{} False{}:
      SCon{Chr{c}, rest}

def Json.esc.chr(+c: U32, rest: String) -> String:
  Json.esc.chr.k(U32.is_lt(c, 32), U32.is_eq(c, 34), U32.is_eq(c, 92), c, rest)

# s escaped, reversed onto acc (tail-recursive: a string can be megabytes,
# and one stack frame per character overflowed the phone's JavaScriptCore)
def Json.esc.rev(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      Json.esc.rev(t, String.reverse.go(Json.esc.chr(c, SNil{}), acc))

# s with JSON escapes, prepended to rest
def Json.esc(s: String, rest: String) -> String:
  String.reverse.go(Json.esc.rev(s, SNil{}), rest)

def Json.quote(s: String, rest: String) -> String:
  SCon{'"', Json.esc(s, SCon{'"', rest})}

# a comma unless first, pushed onto a reversed accumulator
def Json.sep.rev(first: Bool, acc: String) -> String:
  match first:
    case True{}:
      acc
    case False{}:
      SCon{',', acc}

def Json.quote.rev(s: String, acc: String) -> String:
  SCon{'"', Json.esc.rev(s, SCon{'"', acc})}

law Json.emit.rev:
  for j: Json
  for first: Bool
  for acc: String
  String

# j printed compactly, front to back, reversed onto acc; `first` says
# whether a chain cell is the first of its container (no leading comma).
# A chain's tail is the tail call and strings escape tail-recursively, so
# the stack grows only with nesting, never with the length of an array or
# a string (a frame per item overflowed the phone's JavaScriptCore)
def Json.emit.rev(j, first, acc):
  match j:
    case Null{}:
      String.reverse.go("null", acc)
    case Flag{True{}}:
      String.reverse.go("true", acc)
    case Flag{False{}}:
      String.reverse.go("false", acc)
    case Num{r}:
      String.reverse.go(r, acc)
    case Str{s}:
      Json.quote.rev(s, acc)
    case Arr{xs}:
      SCon{']', Json.emit.rev(xs, True{}, SCon{'[', acc})}
    case Obj{fs}:
      SCon{'}', Json.emit.rev(fs, True{}, SCon{'{', acc})}
    case Item{h, t}:
      Json.emit.rev(t, False{}, Json.emit.rev(h, True{}, Json.sep.rev(first, acc)))
    case Field{k, v, t}:
      Json.emit.rev(t, False{}, Json.emit.rev(v, True{}, SCon{':', Json.quote.rev(k, Json.sep.rev(first, acc))}))
    case End{}:
      acc

# j printed compactly in front of rest
def Json.emit(j: Json, first: Bool, rest: String) -> String:
  String.reverse.go(Json.emit.rev(j, first, SNil{}), rest)

# compact JSON text
def Json.show(j: Json) -> String:
  Json.emit(j, True{}, "")

# The UTF-8 size of Json.show(j), read from the tree: a reply's size is
# checked without printing it (the hub's thin replies go to their writer as
# the tree, core/cbor.bend encodes it). Each case counts what Json.emit.rev
# writes; test/json_bytes_test.bend holds it to the printed text.
def Json.bytes.chr(+c: U32) -> Nat:
  Bool.pick(Nat, U32.is_lt(c, 32), Bool.pick(Nat, Bool.or(U32.is_eq(c, 10), Bool.or(U32.is_eq(c, 13), U32.is_eq(c, 9))), 2n, 6n),
    Bool.pick(Nat, Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 92)), 2n,
      Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)))))

def Json.bytes.raw(+c: U32) -> Nat:
  Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)))

# s escaped (no quotes), added to n
def Json.bytes.esc(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{c}, t}:
      Json.bytes.esc(t, Nat.add(n, Json.bytes.chr(c)))

# s as it is, added to n
def Json.bytes.str(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{c}, t}:
      Json.bytes.str(t, Nat.add(n, Json.bytes.raw(c)))

def Json.bytes.sep(first: Bool, +n: Nat) -> Nat:
  match first:
    case True{}:
      n
    case False{}:
      Nat.add(n, 1n)

law Json.bytes.go:
  for j: Json
  for first: Bool
  for n: Nat
  Nat

# as Json.emit.rev: a chain's tail is the tail call, so only nesting uses stack
def Json.bytes.go(j, first, n):
  match j:
    case Null{}:
      Nat.add(n, 4n)
    case Flag{True{}}:
      Nat.add(n, 4n)
    case Flag{False{}}:
      Nat.add(n, 5n)
    case Num{r}:
      Json.bytes.str(r, n)
    case Str{s}:
      Json.bytes.esc(s, Nat.add(n, 2n))
    case Arr{xs}:
      Nat.add(Json.bytes.go(xs, True{}, n), 2n)
    case Obj{fs}:
      Nat.add(Json.bytes.go(fs, True{}, n), 2n)
    case Item{h, t}:
      Json.bytes.go(t, False{}, Json.bytes.go(h, True{}, Json.bytes.sep(first, n)))
    case Field{k, v, t}:
      Json.bytes.go(t, False{}, Json.bytes.go(v, True{}, Nat.add(Json.bytes.esc(k, Nat.add(Json.bytes.sep(first, n), 2n)), 1n)))
    case End{}:
      n

# the size in UTF-8 bytes of Json.show(j)
def Json.bytes(j: Json) -> Nat:
  Json.bytes.go(j, True{}, 0n)

# Lexing
# ------

type Tok is Data:
  TOpen{obj: Bool}
  TClose{}
  TColon{}
  TComma{}
  TStr{text: String}
  TNum{raw: String}
  TWord{word: String}
  TBad{}

type Lex is Data:
  LIdle{}
  LStr{acc: String}
  LEsc{acc: String}
  LUni{acc: String, left: Nat, code: U32}
  LAtom{acc: String, num: Bool}

type Cls is Data:
  CSpace{}
  CQuote{}
  COpen{obj: Bool}
  CClose{}
  CColon{}
  CComma{}
  COther{}

def Lex.cls.q.k(q: Bool, col: Bool, com: Bool, sp: Bool) -> Cls:
  match q col com sp:
    case True{} _ _ _:
      CQuote{}
    case _ True{} _ _:
      CColon{}
    case _ _ True{} _:
      CComma{}
    case _ _ _ True{}:
      CSpace{}
    case _ _ _ _:
      COther{}

def Lex.cls.q(+u: U32) -> Cls:
  Lex.cls.q.k(U32.is_eq(u, 34), U32.is_eq(u, 58), U32.is_eq(u, 44), U32.is_le(u, 32))

def Lex.cls.p.k(a: Bool, o: Bool, ca: Bool, co: Bool, +u: U32) -> Cls:
  match a o ca co:
    case True{} _ _ _:
      COpen{False{}}
    case _ True{} _ _:
      COpen{True{}}
    case _ _ True{} _:
      CClose{}
    case _ _ _ True{}:
      CClose{}
    case _ _ _ _:
      Lex.cls.q(u)

def Lex.cls.p(+u: U32) -> Cls:
  Lex.cls.p.k(U32.is_eq(u, 91), U32.is_eq(u, 123), U32.is_eq(u, 93), U32.is_eq(u, 125), u)

# a token for a finished atom (number or bare word)
def Lex.atom(acc: String, num: Bool) -> Tok:
  match num:
    case True{}:
      TNum{String.reverse(acc)}
    case False{}:
      TWord{String.reverse(acc)}

def Lex.start(c: Char, out: List<&2, Tok>) -> Lex & List<&2, Tok>:
  match c:
    case Chr{+u}:
      (LAtom{SCon{Chr{u}, SNil{}}, Bool.or(U32.is_eq(u, 45), Char.is_digit(Chr{u}))}, out)

# the state and the tokens (reversed) after a structural char in LIdle
def Lex.idle(k: Cls, c: Char, out: List<&2, Tok>) -> Lex & List<&2, Tok>:
  match k:
    case CSpace{}:
      (LIdle{}, out)
    case CQuote{}:
      (LStr{SNil{}}, out)
    case COpen{o}:
      (LIdle{}, TOpen{o} <> out)
    case CClose{}:
      (LIdle{}, TClose{} <> out)
    case CColon{}:
      (LIdle{}, TColon{} <> out)
    case CComma{}:
      (LIdle{}, TComma{} <> out)
    case COther{}:
      Lex.start(c, out)

def Lex.esc.b.k(b: Bool, f: Bool, +u: U32, acc: String) -> Lex:
  match b f:
    case True{} _:
      LStr{SCon{Chr{8}, acc}}
    case _ True{}:
      LStr{SCon{Chr{12}, acc}}
    case _ _:
      LStr{SCon{Chr{u}, acc}}

def Lex.esc.k(uu: Bool, n: Bool, t: Bool, r: Bool, +u: U32, acc: String) -> Lex:
  match uu n t r:
    case True{} _ _ _:
      LUni{acc, 4n, 0}
    case _ True{} _ _:
      LStr{SCon{Chr{10}, acc}}
    case _ _ True{} _:
      LStr{SCon{Chr{9}, acc}}
    case _ _ _ True{}:
      LStr{SCon{Chr{13}, acc}}
    case _ _ _ _:
      Lex.esc.b.k(U32.is_eq(u, 98), U32.is_eq(u, 102), u, acc)

def Lex.esc(c: Char, acc: String) -> Lex:
  match c:
    case Chr{+u}:
      Lex.esc.k(U32.is_eq(u, 117), U32.is_eq(u, 110), U32.is_eq(u, 116), U32.is_eq(u, 114), u, acc)

def Lex.hexval.k(d: Bool, up: Bool, +u: U32) -> U32:
  match d up:
    case True{} _:
      (u - 48 : U32)
    case _ True{}:
      (u - 55 : U32)
    case _ _:
      (u - 87 : U32)

def Lex.hexval(+u: U32) -> U32:
  Lex.hexval.k(U32.is_le(u, 57), U32.is_le(u, 70), u)

# one more hex digit of a \uXXXX escape
def Lex.uni(acc: String, left: Nat, +code: U32, c: Char) -> Lex:
  match left c:
    case 1n+0n Chr{+u}:
      LStr{SCon{Chr{U32.or(U32.shln(code, 4n), Lex.hexval(u))}, acc}}
    case 1n+p Chr{+u}:
      LUni{acc, p, U32.or(U32.shln(code, 4n), Lex.hexval(u))}
    case 0n _:
      LStr{acc}

# a finished string read once at its head. In the JS build a string grown
# a character at a time is a rope (one string cell per character) until
# something reads it, and a parsed value folded into the hub's state may
# never be read: a log's settings kept millions of cells alive (the store
# build: 5.4 GB). Matching the head makes the engine flatten it in place,
# once; in C it is one cell. Never a walk per character.
def Lex.flat(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{c, t}:
      SCon{c, t}

def Lex.str.k(q: Bool, bs: Bool, +u: U32, acc: String, out: List<&2, Tok>) -> Lex & List<&2, Tok>:
  match q bs:
    case True{} _:
      (LIdle{}, TStr{Lex.flat(String.reverse(acc))} <> out)
    case _ True{}:
      (LEsc{acc}, out)
    case _ _:
      (LStr{SCon{Chr{u}, acc}}, out)

def Lex.str(c: Char, acc: String, out: List<&2, Tok>) -> Lex & List<&2, Tok>:
  match c:
    case Chr{+u}:
      Lex.str.k(U32.is_eq(u, 34), U32.is_eq(u, 92), u, acc, out)

# an atom ends at any structural char, which is then lexed from LIdle
def Lex.atom.step(k: Cls, c: Char, acc: String, num: Bool, out: List<&2, Tok>) -> Lex & List<&2, Tok>:
  match k:
    case COther{}:
      (LAtom{SCon{c, acc}, num}, out)
    case _:
      Lex.idle(k, c, Lex.atom(acc, num) <> out)

def Lex.step(st: Lex, c: Char, out: List<&2, Tok>) -> Lex & List<&2, Tok>:
  match st c:
    case LIdle{} Chr{+u}:
      Lex.idle(Lex.cls.p(u), Chr{u}, out)
    case LStr{acc} _:
      Lex.str(c, acc, out)
    case LEsc{acc} _:
      (Lex.esc(c, acc), out)
    case LUni{acc, left, code} _:
      (Lex.uni(acc, left, code, c), out)
    case LAtom{acc, num} Chr{+u}:
      Lex.atom.step(Lex.cls.p(u), Chr{u}, acc, num, out)

def Lex.done(st: Lex, out: List<&2, Tok>) -> List<&2, Tok>:
  match st:
    case LIdle{}:
      List.reverse(&2, Tok, out)
    case LAtom{acc, num}:
      List.reverse(&2, Tok, Lex.atom(acc, num) <> out)
    case _:
      List.reverse(&2, Tok, TBad{} <> out)

def Lex.step.pair(c: Char, r: Lex & List<&2, Tok>) -> Lex & List<&2, Tok>:
  (st, out) = r
  Lex.step(st, c, out)

def Lex.go.fin(r: Lex & List<&2, Tok>) -> List<&2, Tok>:
  (st, out) = r
  Lex.done(st, out)

def Lex.go(s: String, r: Lex & List<&2, Tok>) -> List<&2, Tok>:
  match s:
    case SNil{}:
      Lex.go.fin(r)
    case SCon{c, t}:
      Lex.go(t, Lex.step.pair(c, r))

# the tokens of a JSON text
def Json.lex(s: String) -> List<&2, Tok>:
  Lex.go(s, (LIdle{}, Nil{}))

# Parsing
# -------

# An open container on the stack: its cells so far (reversed), and for an
# object the key waiting for its value.
type Frame is Data:
  FArr{acc: List<&2, Json>}
  FObj{acc: List<&2, KV>, key: Maybe<&2, String>}

type Parse is Data:
  PGo{stack: List<&2, Frame>, top: Maybe<&2, Json>}
  PBad{}

def Parse.chain(xs: List<&2, Json>, end: Json) -> Json:
  match xs:
    case Nil{}:
      end
    case h <> t:
      Parse.chain(t, Item{h, end})

def Parse.fchain(xs: List<&2, KV>, end: Json) -> Json:
  match xs:
    case Nil{}:
      end
    case KV{k, v} <> t:
      Parse.fchain(t, Field{k, v, end})

# a string where an object expects a key
def Parse.key(acc: List<&2, KV>, v: Json, rest: List<&2, Frame>) -> Parse:
  match v:
    case Str{k}:
      PGo{FObj{acc, Some{k}} <> rest, None{}}
    case _:
      PBad{}

# a finished value lands in the enclosing frame, or becomes the top
def Parse.put(stack: List<&2, Frame>, v: Json) -> Parse:
  match stack:
    case Nil{}:
      PGo{Nil{}, Some{v}}
    case FArr{acc} <> rest:
      PGo{FArr{v <> acc} <> rest, None{}}
    case FObj{acc, Some{k}} <> rest:
      PGo{FObj{KV{k, v} <> acc, None{}} <> rest, None{}}
    case FObj{acc, None{}} <> rest:
      Parse.key(acc, v, rest)

def Parse.close(stack: List<&2, Frame>) -> Parse:
  match stack:
    case Nil{}:
      PBad{}
    case FArr{acc} <> rest:
      Parse.put(rest, Arr{Parse.chain(acc, End{})})
    case FObj{acc, _} <> rest:
      Parse.put(rest, Obj{Parse.fchain(acc, End{})})

def Parse.word.k(t: Bool, f: Bool) -> Json:
  match t f:
    case True{} _:
      Flag{True{}}
    case _ True{}:
      Flag{False{}}
    case _ _:
      Null{}

def Parse.word(+w: String) -> Json:
  Parse.word.k(String.eq(w, "true"), String.eq(w, "false"))

def Parse.tok(stack: List<&2, Frame>, t: Tok) -> Parse:
  match t:
    case TOpen{True{}}:
      PGo{FObj{Nil{}, None{}} <> stack, None{}}
    case TOpen{False{}}:
      PGo{FArr{Nil{}} <> stack, None{}}
    case TClose{}:
      Parse.close(stack)
    case TColon{}:
      PGo{stack, None{}}
    case TComma{}:
      PGo{stack, None{}}
    case TStr{s}:
      Parse.put(stack, Str{s})
    case TNum{r}:
      Parse.put(stack, Num{r})
    case TWord{w}:
      Parse.put(stack, Parse.word(w))
    case TBad{}:
      PBad{}

def Parse.step(p: Parse, t: Tok) -> Parse:
  match p:
    case PGo{stack, top}:
      Parse.tok(stack, t)
    case PBad{}:
      PBad{}

def Parse.go(ts: List<&2, Tok>, p: Parse) -> Parse:
  match ts:
    case Nil{}:
      p
    case h <> t:
      Parse.go(t, Parse.step(p, h))

def Parse.fin(p: Parse) -> Maybe<&2, Json>:
  match p:
    case PGo{Nil{}, top}:
      top
    case _:
      None{}

# the value of a JSON text, or None when it is malformed
def Json.parse(s: String) -> Maybe<&2, Json>:
  Parse.fin(Parse.go(Json.lex(s), PGo{Nil{}, None{}}))
