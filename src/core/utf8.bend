import Base

# UTF-8 between byte lists (0..255 per cell) and Strings. Bytes arrive from
# sockets in arbitrary chunks, so decoding happens here, after framing, and
# never splits a character. Malformed input decodes to U+FFFD per bad byte.

# the code point of a lead byte and how many continuation bytes follow
type Lead is Data:
  Lead{bits: U32, more: Nat}

def Utf8.lead.k(a: Bool, b: Bool, c: Bool, d: Bool, +x: U32) -> Lead:
  match a b c d:
    case True{} _ _ _:
      Lead{x, 0n}
    case False{} True{} _ _:
      Lead{U32.and(x, 31), 1n}
    case False{} False{} True{} _:
      Lead{U32.and(x, 15), 2n}
    case False{} False{} False{} True{}:
      Lead{U32.and(x, 7), 3n}
    case False{} False{} False{} False{}:
      Lead{65533, 0n}

def Utf8.lead(+x: U32) -> Lead:
  Utf8.lead.k(U32.is_lt(x, 128), U32.is_eq(U32.and(x, 224), 192), U32.is_eq(U32.and(x, 240), 224), U32.is_eq(U32.and(x, 248), 240), x)

# decoding state: the code point so far and the continuation bytes owed
type Dec is Data:
  Dec{acc: String, cp: U32, owe: Nat}

def Utf8.emit(acc: String, +cp: U32) -> String:
  SCon{Chr{cp}, acc}

def Utf8.start(acc: String, l: Lead) -> Dec:
  match l:
    case Lead{+b, 0n}:
      Dec{Utf8.emit(acc, b), 0, 0n}
    case Lead{b, 1n+m}:
      Dec{acc, b, 1n+m}

# a decoded code point, or U+FFFD when it is no character (a surrogate, or
# past U+10FFFF: a 4-byte lead can spell up to U+1FFFFF), so no String
# holds a value a client cannot encode (a binary webhook body, say)
def Utf8.scalar(+cp: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_gt(cp, 1114111), Bool.and(U32.is_ge(cp, 55296), U32.is_le(cp, 57343))), 65533, cp)

def Utf8.cont.k(ok: Bool, acc: String, +cp: U32, owe: Nat, +x: U32) -> Dec:
  match ok owe:
    case True{} 1n+0n:
      Dec{Utf8.emit(acc, Utf8.scalar(U32.or(U32.shln(cp, 6n), U32.and(x, 63)))), 0, 0n}
    case True{} 1n+m:
      Dec{acc, U32.or(U32.shln(cp, 6n), U32.and(x, 63)), m}
    case True{} 0n:
      Utf8.start(acc, Utf8.lead(x))
    case False{} _:
      Utf8.start(Utf8.emit(acc, 65533), Utf8.lead(x))

def Utf8.step(d: Dec, +x: U32) -> Dec:
  match d:
    case Dec{acc, +cp, 0n}:
      Utf8.start(acc, Utf8.lead(x))
    case Dec{acc, +cp, 1n+m}:
      Utf8.cont.k(U32.is_eq(U32.and(x, 192), 128), acc, cp, 1n+m, x)

def Utf8.go(bs: List<&2, U32>, d: Dec) -> Dec:
  match bs:
    case Nil{}:
      d
    case x <> t:
      Utf8.go(t, Utf8.step(d, x))

def Utf8.fin(d: Dec) -> String:
  match d:
    case Dec{acc, _, 0n}:
      String.reverse(acc)
    case Dec{acc, _, 1n+m}:
      String.reverse(Utf8.emit(acc, 65533))

# the text of a UTF-8 byte list
def Utf8.decode(bs: List<&2, U32>) -> String:
  Utf8.fin(Utf8.go(bs, Dec{SNil{}, 0, 0n}))

def Utf8.put.k(a: Bool, b: Bool, c: Bool, +cp: U32, rest: List<&2, U32>) -> List<&2, U32>:
  match a b c:
    case True{} _ _:
      cp <> rest
    case False{} True{} _:
      U32.or(192, U32.shrn(cp, 6n)) <> U32.or(128, U32.and(cp, 63)) <> rest
    case False{} False{} True{}:
      U32.or(224, U32.shrn(cp, 12n)) <> U32.or(128, U32.and(U32.shrn(cp, 6n), 63)) <> U32.or(128, U32.and(cp, 63)) <> rest
    case False{} False{} False{}:
      U32.or(240, U32.shrn(cp, 18n)) <> U32.or(128, U32.and(U32.shrn(cp, 12n), 63)) <> U32.or(128, U32.and(U32.shrn(cp, 6n), 63)) <> U32.or(128, U32.and(cp, 63)) <> rest

def Utf8.put(+cp: U32, rest: List<&2, U32>) -> List<&2, U32>:
  Utf8.put.k(U32.is_lt(cp, 128), U32.is_lt(cp, 2048), U32.is_lt(cp, 65536), cp, rest)

# the UTF-8 bytes of a String, last first, onto acc: a tail call per
# character (the walk that put each one in front of the rest's overflowed
# the JS runtime's stack on an MCP answer of some 25,000 characters)
def Utf8.encode_rev(s: String, acc: List<&2, U32>) -> List<&2, U32>:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      Utf8.encode_rev(t, List.reverse.go(&2, U32, Utf8.put(c, Nil{}), acc))

# the UTF-8 bytes of a String, in front of rest
def Utf8.encode_onto(s: String, rest: List<&2, U32>) -> List<&2, U32>:
  List.reverse.go(&2, U32, Utf8.encode_rev(s, Nil{}), rest)

def Utf8.encode(s: String) -> List<&2, U32>:
  Utf8.encode_onto(s, Nil{})

# Line framing
# ------------

# A stream split on "\n": the partial line (reversed bytes) carried between
# chunks, and the finished lines (newest first) awaiting pickup.
type Lines is Data:
  Lines{part: List<&2, U32>, done: List<&2, String>}

def Lines.new() -> Lines:
  Lines{Nil{}, Nil{}}

def Lines.step.k(nl: Bool, +x: U32, part: List<&2, U32>, done: List<&2, String>) -> Lines:
  match nl:
    case True{}:
      Lines{Nil{}, Utf8.decode(List.reverse(&2, U32, part)) <> done}
    case False{}:
      Lines{x <> part, done}

def Lines.step(l: Lines, +x: U32) -> Lines:
  match l:
    case Lines{part, done}:
      Lines.step.k(U32.is_eq(x, 10), x, part, done)

def Lines.go(bs: List<&2, U32>, l: Lines) -> Lines:
  match bs:
    case Nil{}:
      l
    case x <> t:
      Lines.go(t, Lines.step(l, x))

def Lines.split(r: Lines) -> Lines & List<&2, String>:
  match r:
    case Lines{part, done}:
      (Lines{part, Nil{}}, List.reverse(&2, String, done))

# feed a chunk; answers the carry and the lines it completed, oldest first
def Lines.feed(l: Lines, bs: List<&2, U32>) -> Lines & List<&2, String>:
  Lines.split(Lines.go(bs, l))

# what is left of a line not yet ended ("" for none): at the stream's end
# it is the last line, read as one (law final_partial_line_flushed)
def Lines.rest(l: Lines) -> String:
  match l:
    case Lines{part, _}:
      Utf8.decode(List.reverse(&2, U32, part))

def Lines.after(r: Lines & List<&2, String>) -> Lines:
  (l, _) = r
  l
