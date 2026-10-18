import Base

# Input-method composition (pure). A composition ("preedit") is text the
# input method shows while a word is being converted: it belongs to one
# field, sits at that field's caret, and is never part of the field's
# text until the method commits it. Everything here is transient state
# kept beside the client's scratch ("@ime"); none of it is saved, sent or
# put on an undo history.
#
# Positions are counts of characters (code points), as the input method
# reports them, so a wide or multibyte character is one unit. Feedback is
# one character per character of the preedit: '0' plus the XIM bits
# (1 reverse, 2 underline, 4 highlight, ...); a reverse or highlighted
# run is the clause being converted.
#
# Focus rule: a composition is owned by the field (and thread) it began
# in. Any change of owner, a window blur, or an owner mismatch on a new
# update discards it (Ime.sync, Ime.draw): nothing is inserted anywhere
# and a composition never moves to another field.

type Ime is Data:
  Ime{owner: String, text: String, fb: String, caret: Nat}

# a part of the preedit and whether it is the clause being converted
type Run is Data:
  Run{lo: Nat, hi: Nat, hot: Bool}

def Ime.none() -> Ime:
  Ime{"", "", "", 0n}

def Ime.on(+i: Ime) -> Bool:
  match i:
    case Ime{_, t, _, _}:
      Bool.not(String.is_empty(t))

def Ime.text(+i: Ime) -> String:
  match i:
    case Ime{_, t, _, _}:
      t

def Ime.owner(+i: Ime) -> String:
  match i:
    case Ime{o, _, _, _}:
      o

def Ime.len(+i: Ime) -> Nat:
  String.length(Ime.text(i))

def Ime.sep() -> String:
  SCon{Chr{31}, SNil{}}

# the scratch form: owner, caret, feedback, text; text is the last part
# and holds no control characters (Ime.clean), so it never splits
def Ime.show(+i: Ime) -> String:
  match i:
    case Ime{+o, +t, +f, +c}:
      Bool.pick(String, String.is_empty(t), "", o ++ Ime.sep() ++ Nat.show(c) ++ Ime.sep() ++ f ++ Ime.sep() ++ t)

def Ime.nth(+ps: List<&2, String>, +k: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, ps, k), "")

def Ime.read.k(+ps: List<&2, String>) -> Ime:
  Ime{Ime.nth(ps, 0n), Ime.nth(ps, 3n), Ime.nth(ps, 2n), Maybe.default(&2, Nat, Nat.read(Ime.nth(ps, 1n)), 0n)}

def Ime.read(+s: String) -> Ime:
  Bool.pick(Ime, String.is_empty(s), Ime.none(), Ime.read.k(String.split(s, Chr{31})))

# control characters never enter a preedit (no newline, no separator)
def Ime.clean.keep(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 32), U32.is_ne(c, 127))

def Ime.clean(+s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, +t}:
      Bool.pick(String, Ime.clean.keep(c), SCon{Chr{c}, Ime.clean(t)}, Ime.clean(t))

# the feedback fitted to n characters: cut, or padded with '0' (none)
def Ime.fit(+f: String, +n: Nat) -> String:
  String.take(f ++ String.repeat("0", n), n)

# i with the characters [first, first + len) replaced by x (whose
# feedback is f), x and f already fitted
def Ime.draw.k(+i: Ime, +first: Nat, +len: Nat, +x: String, +f: String) -> Ime:
  match i:
    case Ime{+o, +t, +fb, +c}:
      +n = String.length(t)
      +a = Nat.min(first, n)
      +b = Nat.min(Nat.add(a, len), n)
      Ime{o, String.take(t, a) ++ x ++ String.drop(t, b), String.take(fb, a) ++ Ime.fit(f, String.length(x)) ++ String.drop(fb, b), Nat.min(c, Nat.add(Nat.sub(n, Nat.sub(b, a)), String.length(x)))}

# a composition of another owner is dropped first (focus isolation)
def Ime.draw.own(same: Bool, +i: Ime, +owner: String, +first: Nat, +len: Nat, +x: String, +f: String) -> Ime:
  match same:
    case True{}:
      Ime.draw.k(i, first, len, x, f)
    case False{}:
      Ime.draw.k(Ime{owner, "", "", 0n}, first, len, x, f)

def Ime.draw(+i: Ime, +owner: String, +first: Nat, +len: Nat, +x: String, +f: String) -> Ime:
  Ime.draw.own(Bool.and(String.eq(Ime.owner(i), owner), Ime.on(i)), i, owner, first, len, Ime.clean(x), f)

# the feedback of [first, first + len) replaced (the text stays)
def Ime.mark(+i: Ime, +first: Nat, +len: Nat, +f: String) -> Ime:
  match i:
    case Ime{+o, +t, +fb, +c}:
      +n = String.length(t)
      +a = Nat.min(first, n)
      +b = Nat.min(Nat.add(a, len), n)
      Ime{o, t, String.take(fb, a) ++ Ime.fit(f, Nat.sub(b, a)) ++ String.drop(fb, b), c}

# the caret inside the preedit (clamped to its length)
def Ime.at(+i: Ime, +pos: Nat) -> Ime:
  match i:
    case Ime{o, +t, f, _}:
      Ime{o, t, f, Nat.min(pos, String.length(t))}

# the composition ends or is cancelled
def Ime.clear(+i: Ime) -> Ime:
  Ime.none()

# only the owner keeps it
def Ime.sync(+i: Ime, +owner: String) -> Ime:
  Bool.pick(Ime, String.eq(Ime.owner(i), owner), i, Ime.none())

# the field's text as shown: the preedit at position p (the caret, or the
# near end of a selection), and the caret as shown
def Ime.shown(+i: Ime, +s: String, +p: Nat) -> String:
  Bool.pick(String, Ime.on(i), String.take(s, p) ++ Ime.text(i) ++ String.drop(s, p), s)

def Ime.caret(+i: Ime, +p: Nat) -> Nat:
  match i:
    case Ime{_, _, _, +c}:
      Bool.pick(Nat, Ime.on(i), Nat.add(p, c), p)

# where the preedit ends in the shown text
def Ime.end(+i: Ime, +p: Nat) -> Nat:
  Bool.pick(Nat, Ime.on(i), Nat.add(p, Ime.len(i)), p)

# the clause being converted is reversed or highlighted
def Ime.hot(+c: U32) -> Bool:
  U32.is_ne(U32.and(U32.sub(c, 48), 5), 0)

# runs of equal heat in the feedback, from position at (offset by base);
# the run open since start is closed when the heat changes
def Ime.runs.go(+f: String, +at: Nat, +start: Nat, +hot: Bool, +base: Nat, +acc: List<&2, Run>) -> List<&2, Run>:
  match f:
    case SNil{}:
      Bool.pick(List<&2, Run>, Nat.is_eq(at, start), acc, Run{Nat.add(base, start), Nat.add(base, at), hot} <> acc)
    case SCon{Chr{+c}, +t}:
      +h = Ime.hot(c)
      +same = Bool.or(Nat.is_eq(at, start), Bool.not(Bool.xor(h, hot)))
      Ime.runs.go(t, 1n+at, Bool.pick(Nat, same, start, at), h, base, Bool.pick(List<&2, Run>, same, acc, Run{Nat.add(base, start), Nat.add(base, at), hot} <> acc))

def Ime.runs(+i: Ime, +p: Nat) -> List<&2, Run>:
  match i:
    case Ime{_, _, +f, _}:
      List.reverse(&2, Run, Ime.runs.go(f, 0n, 0n, False{}, p, Nil{}))
