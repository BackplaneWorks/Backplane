import Base
import ./json.bend as J
import ./utf8.bend as U

# A compact binary form of the JSON tree (an RFC 8949 CBOR subset), for every
# client/server message on slow links. Scalars: 0xF4 false, 0xF5 true, 0xF6
# null; integers that fit 32 bits as major 0/1; other numbers as tag 7 over
# their raw text; strings as major 3 UTF-8; arrays and maps with definite
# lengths. Map keys found in Cbor.keys() are sent as their index (major 0);
# string values found in Cbor.words() as tag 6 over their index.
# Arguments use the 1-, 2- and 4-byte forms; 8-byte and indefinite lengths
# decode to None.

# Dictionaries
# ------------

# Map keys sent as their index. APPEND-ONLY FOREVER: the order is part of the
# wire format; never reorder, remove or rename an entry.
def Cbor.keys() -> List<&2, String>:
  ["$", "t", "id", "m", "p", "ok", "error", "items", "thread", "msg", "role", "text", "at", "state", "project", "title", "kind", "summary", "tone", "act", "session", "model", "provider", "runtime", "interaction", "since", "seq", "version", "info", "path", "value", "key", "task", "parent", "child", "depth", "branch", "env", "worktree", "root", "pinned", "settled", "name", "result", "content", "type", "cid", "count", "manual", "archived"]

# String values sent as tag 6 over their index. APPEND-ONLY FOREVER, like
# Cbor.keys(). The first 24 cost two bytes, so the busiest words lead.
def Cbor.words() -> List<&2, String>:
  ["MessagePosted", "TurnChanged", "ActivityLogged", "changes", "delta", "reply", "user", "assistant", "running", "completed", "idle", "info", "tool", "error", "log", "update", "view", "hello", "system", "interrupted", "failed", "PendingChanged", "ThreadSettled", "ThreadUnsettled", "ProjectCreated", "ProjectRemoved", "ThreadCreated", "ThreadRenamed", "ThreadArchived", "ThreadPinned", "ThreadDeleted", "WorktreeSet", "SessionSet", "ModesSet", "TaskDelegated", "TaskStateSet", "TaskDelivered", "SettingSet", "local", "worktree", "approval-required", "auto-accept-edits", "auto", "full-access", "default", "plan", "queued", "done", "cancelled",
   "AskOpened", "AskClosed", "device.register", "device.drop", "DeviceRegistered", "DeviceDropped", "ios", "android", "alert", "activity", "sandbox", "production"]

def Cbor.hit(names: List<&2, String>, +s: String) -> Bool:
  match names:
    case n <> _:
      String.eq(n, s)
    case Nil{}:
      False{}

def Cbor.find.go(names: List<&2, String>, hit: Bool, +i: U32, +s: String) -> Maybe<&2, U32>:
  match names hit:
    case _ <> _ True{}:
      Some{i}
    case _ <> +t False{}:
      Cbor.find.go(t, Cbor.hit(t, s), (i + 1 : U32), s)
    case Nil{} _:
      None{}

# the index of s in a dictionary
def Cbor.find(+names: List<&2, String>, +s: String) -> Maybe<&2, U32>:
  Cbor.find.go(names, Cbor.hit(names, s), 0, s)

def Cbor.nth.go(names: List<&2, String>, zero: Bool, +i: U32) -> Maybe<&2, String>:
  match names zero:
    case h <> _ True{}:
      Some{h}
    case _ <> t False{}:
      Cbor.nth.go(t, U32.is_eq(i, 1), (i - 1 : U32))
    case Nil{} _:
      None{}

# the entry at index i of a dictionary
def Cbor.nth(names: List<&2, String>, +i: U32) -> Maybe<&2, String>:
  Cbor.nth.go(names, U32.is_eq(i, 0), i)

# The dictionaries as balanced search trees, so a string costs about six
# comparisons instead of one per entry (the encoder's lookup was half the JS
# hub's CPU). Generated from the lists by scripts/cbor-tree.py; law
# cbor_trees_exact holds them to the lists (sorted, every entry at its index,
# nothing else), so a lookup in a tree is Cbor.find in its list.
type CTree is Data:
  CLeaf{}
  CNode{l: CTree, w: String, i: U32, r: CTree}

def CTree.cmp(t: CTree, +s: String) -> Cmp:
  match t:
    case CLeaf{}:
      EQ{}
    case CNode{_, w, _, _}:
      String.order(s, w)

def CTree.go(t: CTree, o: Cmp, +s: String) -> Maybe<&2, U32>:
  match t o:
    case CLeaf{} _:
      None{}
    case CNode{_, _, i, _} EQ{}:
      Some{i}
    case CNode{+l, _, _, _} LT{}:
      CTree.go(l, CTree.cmp(l, s), s)
    case CNode{_, _, _, +r} GT{}:
      CTree.go(r, CTree.cmp(r, s), s)

# the index of s in the dictionary the tree holds
def CTree.find(+t: CTree, +s: String) -> Maybe<&2, U32>:
  CTree.go(t, CTree.cmp(t, s), s)

# the tree's words in order
def CTree.list(t: CTree, acc: List<&2, String>) -> List<&2, String>:
  match t:
    case CLeaf{}:
      acc
    case CNode{l, w, _, r}:
      CTree.list(l, w <> CTree.list(r, acc))

def CTree.rising(xs: List<&2, String>, +prev: String, ok: Bool) -> Bool:
  match xs ok:
    case _ False{}:
      False{}
    case Nil{} True{}:
      True{}
    case +x <> t True{}:
      CTree.rising(t, x, String.is_lt(prev, x))

def CTree.all(names: List<&2, String>, +t: CTree, +i: U32, ok: Bool) -> Bool:
  match names ok:
    case _ False{}:
      False{}
    case Nil{} True{}:
      True{}
    case +n <> rest True{}:
      CTree.all(rest, t, (i + 1 : U32), Maybe.is_some(&2, U32, CTree.find(t, n)) && U32.is_eq(Maybe.default(&2, U32, CTree.find(t, n), 0), i))

def CTree.words.ok(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case +x <> t:
      CTree.rising(t, x, True{})

# the tree holds exactly the list: its words strictly rising (so a search
# that misses is right), as many as the list, each at its list index
def CTree.ok(+names: List<&2, String>, +t: CTree) -> Bool:
  Bool.and(CTree.words.ok(CTree.list(t, Nil{})), Bool.and(Nat.is_eq(List.length(&2, String, CTree.list(t, Nil{})), List.length(&2, String, names)), CTree.all(names, t, 0, True{})))

def Cbor.keys.tree() -> CTree:
  CNode{CNode{CNode{CNode{CNode{CNode{CLeaf{}, "$", 0, CLeaf{}}, "act", 19, CNode{CLeaf{}, "archived", 49, CLeaf{}}}, "at", 12, CNode{CNode{CLeaf{}, "branch", 36, CLeaf{}}, "child", 34, CLeaf{}}}, "cid", 46, CNode{CNode{CNode{CLeaf{}, "content", 44, CLeaf{}}, "count", 47, CLeaf{}}, "depth", 35, CNode{CNode{CLeaf{}, "env", 37, CLeaf{}}, "error", 6, CLeaf{}}}}, "id", 2, CNode{CNode{CNode{CNode{CLeaf{}, "info", 28, CLeaf{}}, "interaction", 24, CNode{CLeaf{}, "items", 7, CLeaf{}}}, "key", 31, CNode{CNode{CLeaf{}, "kind", 16, CLeaf{}}, "m", 3, CLeaf{}}}, "manual", 48, CNode{CNode{CNode{CLeaf{}, "model", 21, CLeaf{}}, "msg", 9, CLeaf{}}, "name", 42, CNode{CNode{CLeaf{}, "ok", 5, CLeaf{}}, "p", 4, CLeaf{}}}}}, "parent", 33, CNode{CNode{CNode{CNode{CNode{CLeaf{}, "path", 29, CLeaf{}}, "pinned", 40, CNode{CLeaf{}, "project", 14, CLeaf{}}}, "provider", 22, CNode{CNode{CLeaf{}, "result", 43, CLeaf{}}, "role", 10, CLeaf{}}}, "root", 39, CNode{CNode{CNode{CLeaf{}, "runtime", 23, CLeaf{}}, "seq", 26, CLeaf{}}, "session", 20, CNode{CNode{CLeaf{}, "settled", 41, CLeaf{}}, "since", 25, CLeaf{}}}}, "state", 13, CNode{CNode{CNode{CNode{CLeaf{}, "summary", 17, CLeaf{}}, "t", 1, CLeaf{}}, "task", 32, CNode{CNode{CLeaf{}, "text", 11, CLeaf{}}, "thread", 8, CLeaf{}}}, "title", 15, CNode{CNode{CNode{CLeaf{}, "tone", 18, CLeaf{}}, "type", 45, CLeaf{}}, "value", 30, CNode{CNode{CLeaf{}, "version", 27, CLeaf{}}, "worktree", 38, CLeaf{}}}}}}

def Cbor.words.tree() -> CTree:
  CNode{CNode{CNode{CNode{CNode{CNode{CLeaf{}, "ActivityLogged", 2, CLeaf{}}, "AskClosed", 50, CNode{CLeaf{}, "AskOpened", 49, CLeaf{}}}, "DeviceDropped", 54, CNode{CNode{CLeaf{}, "DeviceRegistered", 53, CLeaf{}}, "MessagePosted", 0, CNode{CLeaf{}, "ModesSet", 33, CLeaf{}}}}, "PendingChanged", 21, CNode{CNode{CNode{CLeaf{}, "ProjectCreated", 24, CLeaf{}}, "ProjectRemoved", 25, CNode{CLeaf{}, "SessionSet", 32, CLeaf{}}}, "SettingSet", 37, CNode{CNode{CLeaf{}, "TaskDelegated", 34, CLeaf{}}, "TaskDelivered", 36, CNode{CLeaf{}, "TaskStateSet", 35, CLeaf{}}}}}, "ThreadArchived", 28, CNode{CNode{CNode{CNode{CLeaf{}, "ThreadCreated", 26, CLeaf{}}, "ThreadDeleted", 30, CNode{CLeaf{}, "ThreadPinned", 29, CLeaf{}}}, "ThreadRenamed", 27, CNode{CNode{CLeaf{}, "ThreadSettled", 22, CLeaf{}}, "ThreadUnsettled", 23, CNode{CLeaf{}, "TurnChanged", 1, CLeaf{}}}}, "WorktreeSet", 31, CNode{CNode{CNode{CLeaf{}, "activity", 58, CLeaf{}}, "alert", 57, CNode{CLeaf{}, "android", 56, CLeaf{}}}, "approval-required", 40, CNode{CNode{CLeaf{}, "assistant", 7, CLeaf{}}, "auto", 42, CLeaf{}}}}}, "auto-accept-edits", 41, CNode{CNode{CNode{CNode{CNode{CLeaf{}, "cancelled", 48, CLeaf{}}, "changes", 3, CNode{CLeaf{}, "completed", 9, CLeaf{}}}, "default", 44, CNode{CNode{CLeaf{}, "delta", 4, CLeaf{}}, "device.drop", 52, CNode{CLeaf{}, "device.register", 51, CLeaf{}}}}, "done", 47, CNode{CNode{CNode{CLeaf{}, "error", 13, CLeaf{}}, "failed", 20, CNode{CLeaf{}, "full-access", 43, CLeaf{}}}, "hello", 17, CNode{CNode{CLeaf{}, "idle", 10, CLeaf{}}, "info", 11, CNode{CLeaf{}, "interrupted", 19, CLeaf{}}}}}, "ios", 55, CNode{CNode{CNode{CNode{CLeaf{}, "local", 38, CLeaf{}}, "log", 14, CNode{CLeaf{}, "plan", 45, CLeaf{}}}, "production", 60, CNode{CNode{CLeaf{}, "queued", 46, CLeaf{}}, "reply", 5, CNode{CLeaf{}, "running", 8, CLeaf{}}}}, "sandbox", 59, CNode{CNode{CNode{CLeaf{}, "system", 18, CLeaf{}}, "tool", 12, CNode{CLeaf{}, "update", 15, CLeaf{}}}, "user", 6, CNode{CNode{CLeaf{}, "view", 16, CLeaf{}}, "worktree", 39, CLeaf{}}}}}}

# Encoding
# --------

# The encoder walks the tree once, front to back, pushing bytes onto a
# reversed accumulator, and reverses once at the end. Every walk along a
# chain or a string is a tail call, so only nesting depth uses stack.

def Enc.head.k(a: Bool, b: Bool, c: Bool, +mb: U32, +n: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match a b c:
    case True{} _ _:
      U32.or(mb, n) <> acc
    case False{} True{} _:
      n <> U32.or(mb, 24) <> acc
    case False{} False{} True{}:
      U32.and(n, 255) <> U32.shrn(n, 8n) <> U32.or(mb, 25) <> acc
    case False{} False{} False{}:
      U32.and(n, 255) <> U32.and(U32.shrn(n, 8n), 255) <> U32.and(U32.shrn(n, 16n), 255) <> U32.shrn(n, 24n) <> U32.or(mb, 26) <> acc

# an item head in shortest form; mb is the major type already shifted (major * 32)
def Enc.head(+mb: U32, +n: U32, acc: List<&2, U32>) -> List<&2, U32>:
  Enc.head.k(U32.is_lt(n, 24), U32.is_lt(n, 256), U32.is_lt(n, 65536), mb, n, acc)

def Enc.cw.k(a: Bool, b: Bool, c: Bool) -> U32:
  match a b c:
    case True{} _ _:
      1
    case False{} True{} _:
      2
    case False{} False{} True{}:
      3
    case False{} False{} False{}:
      4

# the UTF-8 width of a code point (matches Utf8.put)
def Enc.cw(+c: U32) -> U32:
  Enc.cw.k(U32.is_lt(c, 128), U32.is_lt(c, 2048), U32.is_lt(c, 65536))

# the UTF-8 byte length of a string
def Enc.blen(s: String, acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      Enc.blen(t, (acc + Enc.cw(c) : U32))

def Enc.put.k(a: Bool, b: Bool, c: Bool, +cp: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match a b c:
    case True{} _ _:
      cp <> acc
    case False{} True{} _:
      U32.or(128, U32.and(cp, 63)) <> U32.or(192, U32.shrn(cp, 6n)) <> acc
    case False{} False{} True{}:
      U32.or(128, U32.and(cp, 63)) <> U32.or(128, U32.and(U32.shrn(cp, 6n), 63)) <> U32.or(224, U32.shrn(cp, 12n)) <> acc
    case False{} False{} False{}:
      U32.or(128, U32.and(cp, 63)) <> U32.or(128, U32.and(U32.shrn(cp, 6n), 63)) <> U32.or(128, U32.and(U32.shrn(cp, 12n), 63)) <> U32.or(240, U32.shrn(cp, 18n)) <> acc

# the UTF-8 bytes of a code point, reversed onto acc (the same bytes as Utf8.put)
def Enc.put(+cp: U32, acc: List<&2, U32>) -> List<&2, U32>:
  Enc.put.k(U32.is_lt(cp, 128), U32.is_lt(cp, 2048), U32.is_lt(cp, 65536), cp, acc)

def Enc.utf8(s: String, acc: List<&2, U32>) -> List<&2, U32>:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      Enc.utf8(t, Enc.put(c, acc))

# a string's UTF-8 bytes, newest first, and how many: one walk of the
# string (on the JS target each step of a walk slices the string, so
# measuring it and then writing it cost twice that)
type U8 is Data:
  U8{n: U32, rev: List<&2, U32>}

def Enc.u8.put(+c: U32, +n: U32, rev: List<&2, U32>) -> U8:
  U8{(n + Enc.cw(c) : U32), Enc.put(c, rev)}

def Enc.u8(s: String, u: U8) -> U8:
  match s u:
    case SNil{} _:
      u
    case SCon{Chr{c}, t} U8{n, rev}:
      Enc.u8(t, Enc.u8.put(c, n, rev))

# xs pushed onto acc in order (each a tail call)
def Enc.onto(xs: List<&2, U32>, acc: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Enc.onto(t, x <> acc)

def Enc.text.k(u: U8, acc: List<&2, U32>) -> List<&2, U32>:
  match u:
    case U8{n, rev}:
      Enc.onto(List.reverse(&2, U32, rev), Enc.head(96, n, acc))

# a text string item: the same bytes as Enc.utf8 over Enc.head of Enc.blen
# (test/cbor_test.bend), from one walk of the string
def Enc.text(+s: String, acc: List<&2, U32>) -> List<&2, U32>:
  Enc.text.k(Enc.u8(s, U8{0, Nil{}}), acc)

# Numbers: which ones travel as integers
type Sc is Data:
  Sc{ok: Bool, val: U32, len: U32, zero: Bool}

def Enc.sc.step(st: Sc, +c: U32) -> Sc:
  match st:
    case Sc{ok, +v, +l, z}:
      +d = (c - 48 : U32)
      +fits = U32.is_lt(v, 429496729) || (U32.is_eq(v, 429496729) && U32.is_le(d, 5))
      Sc{ok && U32.is_lt(d, 10) && U32.is_lt(l, 10) && fits, (v * 10 + d : U32), (l + 1 : U32), z || (U32.is_eq(l, 0) && U32.is_eq(c, 48))}

def Enc.scan(s: String, st: Sc) -> Sc:
  match s:
    case SNil{}:
      st
    case SCon{Chr{c}, t}:
      Enc.scan(t, Enc.sc.step(st, c))

type NumK is Data:
  KPos{n: U32}
  KNeg{n: U32}
  KTxt{}

def Enc.pos.k(ok: Bool, v: U32) -> NumK:
  match ok:
    case True{}:
      KPos{v}
    case False{}:
      KTxt{}

# digits only, 1..10 of them, no leading zero, below 2^32
def Enc.pos(sc: Sc) -> NumK:
  match sc:
    case Sc{ok, v, +l, z}:
      Enc.pos.k(ok && U32.is_lt(0, l) && (Bool.not(z) || U32.is_eq(l, 1)), v)

def Enc.neg.k(p: NumK) -> NumK:
  match p:
    case KPos{+v}:
      Bool.pick(NumK, U32.is_eq(v, 0), KTxt{}, KNeg{(v - 1 : U32)})
    case _:
      KTxt{}

def Enc.kind.k(neg: Bool, +c: U32, t: String) -> NumK:
  match neg:
    case True{}:
      Enc.neg.k(Enc.pos(Enc.scan(t, Sc{True{}, 0, 0, False{}})))
    case False{}:
      Enc.pos(Enc.scan(SCon{Chr{c}, t}, Sc{True{}, 0, 0, False{}}))

# how a Num's raw text travels: as a non-negative or negative integer, or as text
def Enc.kind(r: String) -> NumK:
  match r:
    case SNil{}:
      KTxt{}
    case SCon{Chr{+c}, t}:
      Enc.kind.k(U32.is_eq(c, 45), c, t)

def Enc.num.put(k: NumK, r: String, acc: List<&2, U32>) -> List<&2, U32>:
  match k:
    case KPos{n}:
      Enc.head(0, n, acc)
    case KNeg{n}:
      Enc.head(32, n, acc)
    case KTxt{}:
      Enc.text(r, 199 <> acc)

def Enc.num(+r: String, acc: List<&2, U32>) -> List<&2, U32>:
  Enc.num.put(Enc.kind(r), r, acc)

def Enc.str.k(m: Maybe<&2, U32>, s: String, acc: List<&2, U32>) -> List<&2, U32>:
  match m:
    case Some{i}:
      Enc.head(0, i, 198 <> acc)
    case None{}:
      Enc.text(s, acc)

def Enc.str(+s: String, +ws: CTree, acc: List<&2, U32>) -> List<&2, U32>:
  Enc.str.k(CTree.find(ws, s), s, acc)

def Enc.key.k(m: Maybe<&2, U32>, k: String, acc: List<&2, U32>) -> List<&2, U32>:
  match m:
    case Some{i}:
      Enc.head(0, i, acc)
    case None{}:
      Enc.text(k, acc)

def Enc.key(+k: String, +ks: CTree, acc: List<&2, U32>) -> List<&2, U32>:
  Enc.key.k(CTree.find(ks, k), k, acc)

# the cells of an Item / Field chain
def Enc.count(xs: J.Json, acc: U32) -> U32:
  match xs:
    case J.Item{_, t}:
      Enc.count(t, (acc + 1 : U32))
    case J.Field{_, _, t}:
      Enc.count(t, (acc + 1 : U32))
    case _:
      acc

# j's bytes, reversed onto acc
def Enc.go(j: J.Json, +ks: CTree, +ws: CTree, acc: List<&2, U32>) -> List<&2, U32>:
  match j:
    case J.Null{}:
      246 <> acc
    case J.Flag{True{}}:
      245 <> acc
    case J.Flag{False{}}:
      244 <> acc
    case J.Num{r}:
      Enc.num(r, acc)
    case J.Str{s}:
      Enc.str(s, ws, acc)
    case J.Arr{+xs}:
      Enc.go(xs, ks, ws, Enc.head(128, Enc.count(xs, 0), acc))
    case J.Obj{+fs}:
      Enc.go(fs, ks, ws, Enc.head(160, Enc.count(fs, 0), acc))
    case J.Item{h, t}:
      Enc.go(t, ks, ws, Enc.go(h, ks, ws, acc))
    case J.Field{k, v, t}:
      Enc.go(t, ks, ws, Enc.go(v, ks, ws, Enc.key(k, ks, acc)))
    case J.End{}:
      acc

# the CBOR bytes (0..255 per cell) of a JSON value
def Cbor.encode(j: J.Json) -> List<&2, U32>:
  +ks = Cbor.keys.tree()
  +ws = Cbor.words.tree()
  List.reverse(&2, U32, Enc.go(j, ks, ws, Nil{}))

# Decoding
# --------

# a pending tag: none, 6 (a word index follows) or 7 (number text follows)
type Tag is Data:
  TNone{}
  TWord{}
  TNum{}

# an open array or map: items still owed, items so far (reversed), and for a
# map the key waiting for its value
type Frame is Data:
  FArr{left: Nat, acc: List<&2, J.Json>}
  FObj{left: Nat, acc: List<&2, J.KV>, key: Maybe<&2, String>}

# The decoder walks the bytes once: waiting for an initial byte, reading an
# argument's bytes, or reading a string's bytes.
type St is Data:
  SHead{stack: List<&2, Frame>, tag: Tag}
  SArg{stack: List<&2, Frame>, tag: Tag, major: U32, need: U32, acc: U32}
  SText{stack: List<&2, Frame>, tag: Tag, need: U32, acc: List<&2, U32>}
  SDone{v: J.Json}
  SBad{}

def Dec.keyidx(m: Maybe<&2, U32>, +ks: List<&2, String>) -> Maybe<&2, String>:
  match m:
    case Some{i}:
      Cbor.nth(ks, i)
    case None{}:
      None{}

def Dec.keyof(v: J.Json, +ks: List<&2, String>) -> Maybe<&2, String>:
  match v:
    case J.Str{k}:
      Some{k}
    case J.Num{r}:
      Dec.keyidx(U32.read(r), ks)
    case _:
      None{}

def Dec.key(m: Maybe<&2, String>, n: Nat, acc: List<&2, J.KV>, rest: List<&2, Frame>) -> St:
  match m:
    case Some{k}:
      SHead{FObj{n, acc, Some{k}} <> rest, TNone{}}
    case None{}:
      SBad{}

# a finished value lands in the open container, closing every one it fills
def Dec.put(stack: List<&2, Frame>, v: J.Json, +ks: List<&2, String>) -> St:
  match stack:
    case Nil{}:
      SDone{v}
    case FArr{1n+0n, acc} <> rest:
      Dec.put(rest, J.Arr{J.Parse.chain(v <> acc, J.End{})}, ks)
    case FArr{1n+m, acc} <> rest:
      SHead{FArr{m, v <> acc} <> rest, TNone{}}
    case FArr{0n, _} <> _:
      SBad{}
    case FObj{n, acc, None{}} <> rest:
      Dec.key(Dec.keyof(v, ks), n, acc, rest)
    case FObj{1n+0n, acc, Some{k}} <> rest:
      Dec.put(rest, J.Obj{J.Parse.fchain(J.KV{k, v} <> acc, J.End{})}, ks)
    case FObj{1n+m, acc, Some{k}} <> rest:
      SHead{FObj{m, J.KV{k, v} <> acc, None{}} <> rest, TNone{}}
    case FObj{0n, _, Some{_}} <> _:
      SBad{}

def Dec.word(m: Maybe<&2, String>, stack: List<&2, Frame>, +ks: List<&2, String>) -> St:
  match m:
    case Some{s}:
      Dec.put(stack, J.Str{s}, ks)
    case None{}:
      SBad{}

def Dec.mk(tag: Tag, s: String) -> J.Json:
  match tag:
    case TNum{}:
      J.Num{s}
    case _:
      J.Str{s}

def Dec.negs.k(top: Bool, +n: U32) -> String:
  match top:
    case True{}:
      "-4294967296"
    case False{}:
      "-" ++ U32.show((n + 1 : U32))

# the text of a major-1 integer, -1 - n
def Dec.negs(+n: U32) -> String:
  Dec.negs.k(U32.is_eq(n, 4294967295), n)

def Dec.text.k(empty: Bool, +n: U32, tag: Tag, stack: List<&2, Frame>, +ks: List<&2, String>) -> St:
  match empty:
    case True{}:
      Dec.put(stack, Dec.mk(tag, ""), ks)
    case False{}:
      SText{stack, tag, n, Nil{}}

def Dec.arr.k(empty: Bool, +n: U32, stack: List<&2, Frame>, +ks: List<&2, String>) -> St:
  match empty:
    case True{}:
      Dec.put(stack, J.Arr{J.End{}}, ks)
    case False{}:
      SHead{FArr{U32.to_nat(n), Nil{}} <> stack, TNone{}}

def Dec.obj.k(empty: Bool, +n: U32, stack: List<&2, Frame>, +ks: List<&2, String>) -> St:
  match empty:
    case True{}:
      Dec.put(stack, J.Obj{J.End{}}, ks)
    case False{}:
      SHead{FObj{U32.to_nat(n), Nil{}, None{}} <> stack, TNone{}}

# an item whose head (major type and argument n) is complete
def Dec.item(major: Nat, tag: Tag, stack: List<&2, Frame>, +n: U32, +ks: List<&2, String>, +ws: List<&2, String>) -> St:
  match major tag:
    case 0n TNone{}:
      Dec.put(stack, J.Num{U32.show(n)}, ks)
    case 0n TWord{}:
      Dec.word(Cbor.nth(ws, n), stack, ks)
    case 1n TNone{}:
      Dec.put(stack, J.Num{Dec.negs(n)}, ks)
    case 3n TNone{}:
      Dec.text.k(U32.is_eq(n, 0), n, TNone{}, stack, ks)
    case 3n TNum{}:
      Dec.text.k(U32.is_eq(n, 0), n, TNum{}, stack, ks)
    case 4n TNone{}:
      Dec.arr.k(U32.is_eq(n, 0), n, stack, ks)
    case 5n TNone{}:
      Dec.obj.k(U32.is_eq(n, 0), n, stack, ks)
    case _ _:
      SBad{}

def Dec.tag.k(w: Bool, n: Bool, stack: List<&2, Frame>) -> St:
  match w n:
    case True{} _:
      SHead{stack, TWord{}}
    case False{} True{}:
      SHead{stack, TNum{}}
    case False{} False{}:
      SBad{}

def Dec.tag(tag: Tag, +ai: U32, stack: List<&2, Frame>) -> St:
  match tag:
    case TNone{}:
      Dec.tag.k(U32.is_eq(ai, 6), U32.is_eq(ai, 7), stack)
    case _:
      SBad{}

def Dec.simple.k(f: Bool, t: Bool, z: Bool, stack: List<&2, Frame>, +ks: List<&2, String>) -> St:
  match f t z:
    case True{} _ _:
      Dec.put(stack, J.Flag{False{}}, ks)
    case False{} True{} _:
      Dec.put(stack, J.Flag{True{}}, ks)
    case False{} False{} True{}:
      Dec.put(stack, J.Null{}, ks)
    case False{} False{} False{}:
      SBad{}

def Dec.simple(tag: Tag, +ai: U32, stack: List<&2, Frame>, +ks: List<&2, String>) -> St:
  match tag:
    case TNone{}:
      Dec.simple.k(U32.is_eq(ai, 20), U32.is_eq(ai, 21), U32.is_eq(ai, 22), stack, ks)
    case _:
      SBad{}

def Dec.head.k(t6: Bool, t7: Bool, lt: Bool, a1: Bool, a2: Bool, a4: Bool, +maj: U32, +ai: U32, stack: List<&2, Frame>, tag: Tag, +ks: List<&2, String>, +ws: List<&2, String>) -> St:
  match t6 t7 lt a1 a2 a4:
    case True{} _ _ _ _ _:
      Dec.tag(tag, ai, stack)
    case False{} True{} _ _ _ _:
      Dec.simple(tag, ai, stack, ks)
    case False{} False{} True{} _ _ _:
      Dec.item(U32.to_nat(maj), tag, stack, ai, ks, ws)
    case False{} False{} False{} True{} _ _:
      SArg{stack, tag, maj, 1, 0}
    case False{} False{} False{} False{} True{} _:
      SArg{stack, tag, maj, 2, 0}
    case False{} False{} False{} False{} False{} True{}:
      SArg{stack, tag, maj, 4, 0}
    case False{} False{} False{} False{} False{} False{}:
      SBad{}

# an initial byte: major type in the top 3 bits, additional info in the low 5
def Dec.head(+maj: U32, +ai: U32, stack: List<&2, Frame>, tag: Tag, +ks: List<&2, String>, +ws: List<&2, String>) -> St:
  Dec.head.k(U32.is_eq(maj, 6), U32.is_eq(maj, 7), U32.is_lt(ai, 24), U32.is_eq(ai, 24), U32.is_eq(ai, 25), U32.is_eq(ai, 26), maj, ai, stack, tag, ks, ws)

def Dec.arg.k(last: Bool, stack: List<&2, Frame>, tag: Tag, +maj: U32, +need: U32, +acc: U32, +ks: List<&2, String>, +ws: List<&2, String>) -> St:
  match last:
    case True{}:
      Dec.item(U32.to_nat(maj), tag, stack, acc, ks, ws)
    case False{}:
      SArg{stack, tag, maj, (need - 1 : U32), acc}

def Dec.txt.k(last: Bool, stack: List<&2, Frame>, tag: Tag, +need: U32, acc: List<&2, U32>, +ks: List<&2, String>) -> St:
  match last:
    case True{}:
      Dec.put(stack, Dec.mk(tag, U.Utf8.decode(List.reverse(&2, U32, acc))), ks)
    case False{}:
      SText{stack, tag, (need - 1 : U32), acc}

def Dec.step(st: St, +b: U32, +ks: List<&2, String>, +ws: List<&2, String>) -> St:
  match st:
    case SHead{stack, tag}:
      Dec.head(U32.shrn(b, 5n), U32.and(b, 31), stack, tag, ks, ws)
    case SArg{stack, tag, maj, +need, acc}:
      Dec.arg.k(U32.is_eq(need, 1), stack, tag, maj, need, U32.or(U32.shln(acc, 8n), b), ks, ws)
    case SText{stack, tag, +need, acc}:
      Dec.txt.k(U32.is_eq(need, 1), stack, tag, need, b <> acc, ks)
    case SDone{_}:
      SBad{}
    case SBad{}:
      SBad{}

def Dec.go(bs: List<&2, U32>, st: St, +ks: List<&2, String>, +ws: List<&2, String>) -> St:
  match bs:
    case Nil{}:
      st
    case b <> t:
      Dec.go(t, Dec.step(st, b, ks, ws), ks, ws)

def Dec.fin(st: St) -> Maybe<&2, J.Json>:
  match st:
    case SDone{v}:
      Some{v}
    case _:
      None{}

# the JSON value of CBOR bytes, or None when they are malformed or truncated
def Cbor.decode(bs: List<&2, U32>) -> Maybe<&2, J.Json>:
  +ks = Cbor.keys()
  +ws = Cbor.words()
  Dec.fin(Dec.go(bs, SHead{Nil{}, TNone{}}, ks, ws))

# Encoding with holes (fast hub R1: a thin reply whose page was encoded
# before): a string value that is a hole's name (they start with "\u{0}",
# which no text the hub writes does) is written as the hole's bytes, each
# segment in turn, exactly as Enc.go would write the value it stands for
# (test/thin_splice_test.bend holds Cbor.encode.holes to Cbor.encode of
# the whole tree).
type Hole is Data:
  Hole{name: String, segs: List<&2, List<&2, U32>>}

def Hole.segs.go(segs: List<&2, List<&2, U32>>, acc: List<&2, U32>) -> List<&2, U32>:
  match segs:
    case Nil{}:
      acc
    case s <> t:
      Hole.segs.go(t, Enc.onto(s, acc))

def Hole.name.is(hs: List<&2, Hole>, +s: String) -> Bool:
  match hs:
    case Hole{+n, _} <> _:
      String.eq(n, s)
    case Nil{}:
      False{}

def Hole.find(hs: List<&2, Hole>, +s: String, +hit: Bool) -> Maybe<&2, List<&2, List<&2, U32>>>:
  match hs hit:
    case Nil{} _:
      None{}
    case Hole{+n, segs} <> +r True{}:
      Some{segs}
    case +x <> t False{}:
      Hole.find(t, s, Hole.name.is(t, s))

def Hole.mark(+s: String) -> Bool:
  match s:
    case SCon{Chr{c}, _}:
      U32.is_eq(c, 0)
    case SNil{}:
      False{}

def Enc.hole.put(m: Maybe<&2, List<&2, List<&2, U32>>>, +s: String, +ws: CTree, acc: List<&2, U32>) -> List<&2, U32>:
  match m:
    case Some{segs}:
      Hole.segs.go(segs, acc)
    case None{}:
      Enc.str(s, ws, acc)

def Enc.hole.str(+marked: Bool, +s: String, +ws: CTree, +hs: List<&2, Hole>, acc: List<&2, U32>) -> List<&2, U32>:
  match marked:
    case True{}:
      Enc.hole.put(Hole.find(hs, s, Hole.name.is(hs, s)), s, ws, acc)
    case False{}:
      Enc.str(s, ws, acc)

# Enc.go with holes
def Enc.hole.go(j: J.Json, +ks: CTree, +ws: CTree, +hs: List<&2, Hole>, acc: List<&2, U32>) -> List<&2, U32>:
  match j:
    case J.Null{}:
      246 <> acc
    case J.Flag{True{}}:
      245 <> acc
    case J.Flag{False{}}:
      244 <> acc
    case J.Num{r}:
      Enc.num(r, acc)
    case J.Str{+s}:
      Enc.hole.str(Hole.mark(s), s, ws, hs, acc)
    case J.Arr{+xs}:
      Enc.hole.go(xs, ks, ws, hs, Enc.head(128, Enc.count(xs, 0), acc))
    case J.Obj{+fs}:
      Enc.hole.go(fs, ks, ws, hs, Enc.head(160, Enc.count(fs, 0), acc))
    case J.Item{h, t}:
      Enc.hole.go(t, ks, ws, hs, Enc.hole.go(h, ks, ws, hs, acc))
    case J.Field{k, v, t}:
      Enc.hole.go(t, ks, ws, hs, Enc.hole.go(v, ks, ws, hs, Enc.key(k, ks, acc)))
    case J.End{}:
      acc

def Cbor.encode.holes(j: J.Json, +hs: List<&2, Hole>) -> List<&2, U32>:
  +ks = Cbor.keys.tree()
  +ws = Cbor.words.tree()
  List.reverse(&2, U32, Enc.hole.go(j, ks, ws, hs, Nil{}))

# A tree's bytes split where a marker string stands (its bytes, once):
# (before, after), forward; None when the marker is not there exactly once
type Cut2 is Data:
  Cut2{before: List<&2, U32>, after: List<&2, U32>}

def Cut.prefix(xs: List<&2, U32>, pat: List<&2, U32>, ok: Bool) -> Bool:
  match xs pat ok:
    case _ _ False{}:
      False{}
    case _ Nil{} True{}:
      True{}
    case Nil{} _ True{}:
      False{}
    case +x <> xt +p <> pt True{}:
      Cut.prefix(xt, pt, U32.is_eq(p, x))

def Cut.drop(xs: List<&2, U32>, n: Nat) -> List<&2, U32>:
  match xs n:
    case +ys 0n:
      ys
    case Nil{} _:
      Nil{}
    case +y <> t 1n+m:
      Cut.drop(t, m)

# walk xs a byte at a time; rev: the bytes before the first match,
# reversed; n: matches so far; skip: the match's bytes still to pass;
# after: what follows the first match
type CutAt is Data:
  CutAt{rev: List<&2, U32>, n: Nat, skip: Nat, after: List<&2, U32>}

def Cut.skip(+t: List<&2, U32>, rev: List<&2, U32>, +n: Nat, +skip: Nat, +after: List<&2, U32>) -> CutAt:
  CutAt{rev, n, Nat.sub(skip, 1n), Bool.pick(List<&2, U32>, Bool.and(Nat.is_eq(n, 1n), Nat.is_eq(skip, 1n)), t, after)}

def Cut.hit(+t: List<&2, U32>, rev: List<&2, U32>, +n: Nat, +after: List<&2, U32>, +plen: Nat) -> CutAt:
  CutAt{rev, Nat.add(n, 1n), Nat.sub(plen, 1n), Bool.pick(List<&2, U32>, Bool.and(Nat.is_eq(n, 0n), Nat.is_le(plen, 1n)), t, after)}

def Cut.plain(+x: U32, +rev: List<&2, U32>, +n: Nat, after: List<&2, U32>) -> CutAt:
  CutAt{Bool.pick(List<&2, U32>, Nat.is_eq(n, 0n), x <> rev, rev), n, 0n, after}

def Cut.step.k(skipping: Bool, hit: Bool, +x: U32, +t: List<&2, U32>, at: CutAt, +plen: Nat) -> CutAt:
  match skipping hit at:
    case True{} _ CutAt{rev, +n, +skip, +after}:
      Cut.skip(t, rev, n, skip, after)
    case False{} True{} CutAt{rev, +n, _, +after}:
      Cut.hit(t, rev, n, after, plen)
    case False{} False{} CutAt{+rev, +n, _, after}:
      Cut.plain(x, rev, n, after)

def Cut.skipping(+at: CutAt) -> Bool:
  match at:
    case CutAt{_, _, skip, _}:
      Nat.is_gt(skip, 0n)

def Cut.find(xs: List<&2, U32>, +pat: List<&2, U32>, +plen: Nat, +at: CutAt, hit: Bool) -> CutAt:
  match xs:
    case Nil{}:
      at
    case +x <> +t:
      Cut.find(t, pat, plen, Cut.step.k(Cut.skipping(at), hit, x, t, at, plen), Cut.prefix(t, pat, True{}))

def Cut.done(at: CutAt) -> Maybe<&2, Cut2>:
  match at:
    case CutAt{+rev, +n, _, +after}:
      Bool.pick(Maybe<&2, Cut2>, Nat.is_eq(n, 1n), Some{Cut2{List.reverse(&2, U32, rev), after}}, None{})

def Cbor.cut(+bs: List<&2, U32>, +pat: List<&2, U32>) -> Maybe<&2, Cut2>:
  Cut.done(Cut.find(bs, pat, List.length(&2, U32, pat), CutAt{Nil{}, 0n, 0n, Nil{}}, Cut.prefix(bs, pat, True{})))
