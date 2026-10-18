import Base
import ./links.bend as LK
import ./ref.bend as R
import ./refview.bend as RE
import ./model.bend as M
import ./eq.bend as Q

# Chips in the window's timeline (pure). A Backplane ref in a message that
# resolves is drawn as a chip: the card's icon and line, then the machine
# tag, muted. It is decided once per entry, at a memo miss, from the state
# of that moment (layout.bend's TL.memo); drawing only reads the line.
#
# The paragraph is wrapped with the chip's label in place of the ref: a
# slot for the icon, the text and the tag, joined by underscores (no spaces) so a
# wrap never cuts a chip. The chip travels in the line's copy field
# beside the links (LK.Link.enc): one record per chip, label US uri US icon
# US text US machine, ended by RS. Link.dec skips a record of five fields.

type Chip is Data:
  Chip{label: String, uri: String, icon: String, text: String, machine: String}

# a piece of a drawn line
type Run is Data:
  RText{text: String}
  RLink{p: LK.Piece}
  RChip{c: Chip}

def Chip.max() -> Nat:
  48n

def Chip.nbsp() -> Char:
  Chr{95}

# control characters would break the encoding below: they become spaces
def Chip.clean(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, +t}:
      SCon{Bool.pick(Char, U32.is_lt(LK.Link.code(c), 32), ' ', c), Chip.clean(t)}

# the text cut to 48 characters, with an ellipsis when it was longer
def Chip.short.k(+long: Bool, +s: String) -> String:
  match long:
    case True{}:
      String.take(s, 48n) ++ "…"
    case False{}:
      s

def Chip.short(+s: String) -> String:
  Chip.short.k(Nat.is_gt(String.length(s), 48n), s)

# A space that does not wrap, and a middle dot, as ASCII: the paragraph
# measures the label (text.bend's glyph table is for ASCII; any other
# character costs a map lookup, 2000 of them for 200 chips). The label is
# only measured and matched, never drawn: the chip draws its own text.
def Chip.nb.c(+u: U32, +c: Char) -> Char:
  Bool.pick(Char, U32.is_eq(u, 32), Chip.nbsp(), Bool.pick(Char, U32.is_eq(u, 183), '.', c))

def Chip.nb(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, +t}:
      SCon{Chip.nb.c(LK.Link.code(c), c), Chip.nb(t)}

# the icon's slot
def Chip.pad() -> String:
  SCon{Chip.nbsp(), SCon{Chip.nbsp(), SCon{Chip.nbsp(), SNil{}}}}

def Chip.tag(+m: String) -> String:
  Bool.pick(String, String.is_empty(m), "", Chip.nb(" @" ++ m))

# what the paragraph wraps in place of the ref
def Chip.label(+text: String, +machine: String) -> String:
  Chip.pad() ++ Chip.nb(text) ++ Chip.tag(machine) ++ SCon{Chip.nbsp(), SNil{}}

def Chip.make(+uri: String, +icon: String, +text: String, +machine: String) -> Chip:
  Chip{Chip.label(text, machine), uri, icon, text, machine}

# The card's line (title, sub, status) in 48 characters: when it is
# longer the TITLE is shortened, with an ellipsis; sub and status stay whole.
# Only when they alone leave no room is the whole line cut.
def Chip.fit.r(+room: Bool, +t: String, +r: String, +lr: Nat) -> String:
  match room:
    case True{}:
      String.take(t, Nat.sub(47n, lr)) ++ "…" ++ r
    case False{}:
      Chip.short(t ++ r)

def Chip.fit.k(+fits: Bool, +room: Bool, +t: String, +r: String, +lr: Nat) -> String:
  match fits:
    case True{}:
      t ++ r
    case False{}:
      Chip.fit.r(room, t, r, lr)

def Chip.fit(+t: String, +r: String) -> String:
  Chip.fit.k(Nat.is_le(Nat.add(String.length(t), String.length(r)), 48n), Nat.is_le(String.length(r), 46n), t, r, String.length(r))

# a card as a chip: its line without the machine tag, fitted; the tag apart
def Chip.of(+c: RE.Card) -> Chip:
  match c:
    case RE.Card{+u, _, +i, +t, +s, +state, +m}:
      Chip.make(u, i, Chip.clean(Chip.fit(t, RE.Card.part(s, " · ") ++ RE.Card.part(state, " · "))), Chip.clean(String.take(m, 24n)))

def Chip.res(+m: Maybe<&2, RE.Card>, +label: String) -> Run:
  match m:
    case Some{+c}:
      RChip{Chip.of(c)}
    case None{}:
      RText{label}

def Chip.card(+bare: Bool, +st: M.State, +r: R.Ref) -> Maybe<&2, RE.Card>:
  match bare:
    case True{}:
      RE.Card.bare(st, r)
    case False{}:
      RE.Card.of(st, r)

# a piece resolved: what it is drawn as, and the object ids that decided it
type Rv is Data:
  Rv{run: Run, deps: List<&2, String>}

def Chip.resolve.ref(+m: Maybe<&2, R.Ref>, +bare: Bool, +st: M.State, +label: String) -> Rv:
  match m:
    case None{}:
      Rv{RText{label}, Nil{}}
    case Some{+r}:
      Rv{Chip.res(Chip.card(bare, st, r), label), RE.Cards.deps(st, r)}

# A ref that resolves is a chip (a bare id only if it names a thread, bot,
# project, room, routine or task); one that does not is its label, exactly
# the characters written. Links stay links.
def Chip.resolve(+st: M.State, +p: LK.Piece) -> Rv:
  match p:
    case LK.PText{+s}:
      Rv{RText{s}, Nil{}}
    case LK.PLink{+l, +u}:
      Rv{RLink{LK.PLink{l, u}}, Nil{}}
    case LK.PRef{+l, +u, +b}:
      Chip.resolve.ref(R.Ref.parse(u), b, st, l)

def Chip.rv.run(+v: Rv) -> Run:
  match v:
    case Rv{+r, _}:
      r

def Chip.rv.deps(+v: Rv) -> List<&2, String>:
  match v:
    case Rv{_, +d}:
      d

def Chip.piece(+st: M.State, +p: LK.Piece) -> Run:
  Chip.rv.run(Chip.resolve(st, p))

# the object ids a piece's chip depends on, resolved or not, so that an
# object created later turns a bare id into a chip
def Chip.deps(+st: M.State, +p: LK.Piece) -> List<&2, String>:
  Chip.rv.deps(Chip.resolve(st, p))

# Encoding
# --------

def Chip.rec(+c: Chip) -> String:
  match c:
    case Chip{+l, +u, +i, +t, +m}:
      l ++ SCon{Chr{31}, u} ++ SCon{Chr{31}, i} ++ SCon{Chr{31}, t} ++ SCon{Chr{31}, m} ++ SCon{Chr{30}, SNil{}}

def Chip.enc(+cs: List<&2, Chip>) -> String:
  match cs:
    case Nil{}:
      ""
    case +c <> +t:
      Chip.rec(c) ++ Chip.enc(t)

def Chip.dec.one(+xs: List<&2, String>) -> List<&2, Chip>:
  match xs:
    case +l <> +u <> +i <> +t <> +m <> Nil{}:
      [Chip{l, u, i, t, m}]
    case _:
      Nil{}

def Chip.dec.all(+xs: List<&2, String>) -> List<&2, Chip>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> +t:
      List.append(&2, Chip, Chip.dec.one(String.split(x, Chr{31})), Chip.dec.all(t))

def Chip.dec(+s: String) -> List<&2, Chip>:
  Chip.dec.all(String.split(s, Chr{30}))

# Lines
# -----
# A paragraph's chips go to the lines that show them, in order: a chip is
# never cut by a wrap, so each is in exactly one line. Two chips with one
# label then keep their own addresses.

type Cn is Data:
  Cn{got: List<&2, Chip>, rest: List<&2, Chip>}

def Chip.label.of(+c: Chip) -> String:
  match c:
    case Chip{+l, _, _, _, _}:
      l

def Chip.head(+l: String, +cs: List<&2, Chip>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case +c <> _:
      String.contains(l, Chip.label.of(c))

# the chips at the front of cs that the line shows
def Chip.take(+cs: List<&2, Chip>, +hit: Bool, +l: String, +got: List<&2, Chip>) -> Cn:
  match cs hit:
    case +c <> +t True{}:
      Chip.take(t, Chip.head(l, t), l, c <> got)
    case +c <> +t False{}:
      Cn{List.reverse(&2, Chip, got), c <> t}
    case Nil{} _:
      Cn{List.reverse(&2, Chip, got), Nil{}}

type En is Data:
  En{rest: List<&2, Chip>, out: List<&2, String>}

def Chip.step.k(+n: Cn, +base: String, +out: List<&2, String>) -> En:
  match n:
    case Cn{+got, +rest}:
      En{rest, (base ++ Chip.enc(got)) <> out}

def Chip.step(+l: String, +base: String, +e: En) -> En:
  match e:
    case En{+cs, +out}:
      Chip.step.k(Chip.take(cs, Chip.head(l, cs), l, Nil{}), base, out)

def Chip.encs.go(+ls: List<&2, String>, +base: String, +e: En) -> En:
  match ls:
    case Nil{}:
      e
    case +l <> +t:
      Chip.encs.go(t, base, Chip.step(l, base, e))

def Chip.encs.out(+e: En) -> List<&2, String>:
  match e:
    case En{_, +out}:
      List.reverse(&2, String, out)

# each wrapped line's copy field: the paragraph's links (base) and the
# chips that line shows
def Chip.encs(+ls: List<&2, String>, +base: String, +cs: List<&2, Chip>) -> List<&2, String>:
  Chip.encs.out(Chip.encs.go(ls, base, En{cs, Nil{}}))

# Drawing
# -------

type Rc is Data:
  Rc{acc: String, skip: Nat, out: List<&2, Run>, cs: List<&2, Chip>}

def Chip.text(+acc: String, +out: List<&2, Run>) -> List<&2, Run>:
  match acc:
    case SNil{}:
      out
    case SCon{+h, +t}:
      RText{String.reverse(SCon{h, t})} <> out

def Chip.at(+cs: List<&2, Chip>, +s: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case +c <> _:
      String.starts_with(s, Chip.label.of(c))

def Chip.cut.hit(+hc: Bool, +hl: Maybe<&2, LK.Piece>, +c: Char, +st: Rc) -> Rc:
  match hc hl st:
    case True{} _ Rc{+acc, _, +out, +ch <> +rest}:
      Rc{SNil{}, Nat.sub(String.length(Chip.label.of(ch)), 1n), RChip{ch} <> Chip.text(acc, out), rest}
    case False{} Some{LK.PRef{+l, +u, +b}} Rc{+acc, _, +out, +cs}:
      Rc{SNil{}, Nat.sub(String.length(l), 1n), RLink{LK.PRef{l, u, b}} <> Chip.text(acc, out), cs}
    case False{} Some{LK.PLink{+l, +u}} Rc{+acc, _, +out, +cs}:
      Rc{SNil{}, Nat.sub(String.length(l), 1n), RLink{LK.PLink{l, u}} <> Chip.text(acc, out), cs}
    case _ _ Rc{+acc, +k, +out, +cs}:
      Rc{SCon{c, acc}, k, out, cs}

def Chip.cut.cs(+st: Rc) -> List<&2, Chip>:
  match st:
    case Rc{_, _, _, +cs}:
      cs

def Chip.cut.step(+skip: Nat, +ls: List<&2, LK.Piece>, +c: Char, +s: String, +st: Rc) -> Rc:
  match skip st:
    case 1n+k Rc{+acc, _, +out, +cs}:
      Rc{acc, k, out, cs}
    case 0n _:
      Chip.cut.hit(Chip.at(Chip.cut.cs(st), s), LK.Link.at(ls, s), c, st)

def Chip.cut.skip(+st: Rc) -> Nat:
  match st:
    case Rc{_, +k, _, _}:
      k

def Chip.cut.fin(+st: Rc) -> List<&2, Run>:
  match st:
    case Rc{+acc, _, +out, _}:
      List.reverse(&2, Run, Chip.text(acc, out))

def Chip.cut.go(+s: String, +ls: List<&2, LK.Piece>, +st: Rc) -> List<&2, Run>:
  match s:
    case SNil{}:
      Chip.cut.fin(st)
    case SCon{+c, +t}:
      Chip.cut.go(t, ls, Chip.cut.step(Chip.cut.skip(st), ls, c, SCon{c, t}, st))

# a drawn line as runs of text, links and chips, from its copy field
def Chip.cut(+line: String, +copy: String) -> List<&2, Run>:
  Chip.cut.go(line, LK.Link.dec(copy), Rc{SNil{}, 0n, Nil{}, Chip.dec(copy)})

# Sameness
# --------
# Exact equality of what two states make of a piece (laws eq_*_sound, so a
# memo never keeps a chip a card change made stale): the run and the ids
# it was decided from.

def Chip.eq.strs(a: List<&2, String>, b: List<&2, String>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case +x <> +xt +y <> +yt:
      Bool.and(Q.Eq.str(x, y), Chip.eq.strs(xt, yt))
    case _ _:
      False{}

def Chip.eq.chip(+a: Chip, +b: Chip) -> Bool:
  match a b:
    case Chip{+a0, +a1, +a2, +a3, +a4} Chip{+b0, +b1, +b2, +b3, +b4}:
      Bool.and(Q.Eq.str(a0, b0), Bool.and(Q.Eq.str(a1, b1), Bool.and(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Q.Eq.str(a4, b4)))))

def Chip.eq.run(+a: Run, +b: Run) -> Bool:
  match a b:
    case RText{+x} RText{+y}:
      Q.Eq.str(x, y)
    case RLink{LK.PLink{+l0, +u0}} RLink{LK.PLink{+l1, +u1}}:
      Bool.and(Q.Eq.str(l0, l1), Q.Eq.str(u0, u1))
    case RChip{+x} RChip{+y}:
      Chip.eq.chip(x, y)
    case _ _:
      False{}

def Chip.eq.rv(+a: Rv, +b: Rv) -> Bool:
  match a b:
    case Rv{+r0, +d0} Rv{+r1, +d1}:
      Bool.and(Chip.eq.run(r0, r1), Chip.eq.strs(d0, d1))

# a piece is made the same by the two states
def Chip.same(+st0: M.State, +st: M.State, +p: LK.Piece) -> Bool:
  Chip.eq.rv(Chip.resolve(st0, p), Chip.resolve(st, p))

# Runs of wrapped lines without links
# -----------------------------------
# For a paragraph with chips and no links (most) the runs of each line are
# found by label directly, with no copy field to build and cut again: the
# chips go to the lines that show them, in order, as Chip.encs does, and
# each line becomes its text runs and chips (the same runs Chip.cut makes
# from the line and its copy field).

# where the label first starts s (from index i), if it does; hit says
# whether it starts s itself
def Chip.find(s: String, +label: String, +i: Nat, +hit: Bool) -> Maybe<&2, Nat>:
  match s hit:
    case _ True{}:
      Some{i}
    case SNil{} False{}:
      None{}
    case SCon{_, +t} False{}:
      Chip.find(t, label, 1n+i, String.starts_with(t, label))

def Chip.find.head(+s: String, +cs: List<&2, Chip>) -> Maybe<&2, Nat>:
  match cs:
    case Nil{}:
      None{}
    case +c <> _:
      Chip.find(s, Chip.label.of(c), 0n, String.starts_with(s, Chip.label.of(c)))

type Lr is Data:
  Lr{rest: List<&2, Chip>, runs: List<&2, Run>}

def Chip.lr.text(+s: String, +r: List<&2, Run>) -> List<&2, Run>:
  match s:
    case SNil{}:
      r
    case SCon{_, _}:
      RText{s} <> r

def Chip.lr.cons(+before: String, +c: Chip, +r: Lr) -> Lr:
  match r:
    case Lr{+rest, +runs}:
      Lr{rest, Chip.lr.text(before, RChip{c} <> runs)}

# the chips at the front of cs that the line shows, in order, and the line
# as runs (m: where the head chip's label is in the line)
def Chip.lr(+cs: List<&2, Chip>, +line: String, +m: Maybe<&2, Nat>) -> Lr:
  match cs m:
    case Nil{} _:
      Lr{Nil{}, Chip.lr.text(line, Nil{})}
    case +c <> +t None{}:
      Lr{c <> t, Chip.lr.text(line, Nil{})}
    case +c <> +t Some{+i}:
      +rest = {String.drop(line, Nat.add(i, String.length(Chip.label.of(c)))) : String}
      Chip.lr.cons(String.take(line, i), c, Chip.lr(t, rest, Chip.find.head(rest, t)))

def Chip.lr.line(+line: String, +cs: List<&2, Chip>) -> Lr:
  Chip.lr(cs, line, Chip.find.head(line, cs))

type Ln is Data:
  Ln{rest: List<&2, Chip>, out: List<&2, List<&2, Run>>}

def Chip.lines.step.k(+r: Lr, +out: List<&2, List<&2, Run>>) -> Ln:
  match r:
    case Lr{+rest, +runs}:
      Ln{rest, runs <> out}

def Chip.lines.step(+line: String, +e: Ln) -> Ln:
  match e:
    case Ln{+cs, +out}:
      Chip.lines.step.k(Chip.lr.line(line, cs), out)

def Chip.lines.go(+ls: List<&2, String>, +e: Ln) -> List<&2, List<&2, Run>>:
  match ls:
    case Nil{}:
      match e:
        case Ln{_, +out}:
          List.reverse(&2, List<&2, Run>, out)
    case +l <> +t:
      Chip.lines.go(t, Chip.lines.step(l, e))

# each wrapped line as its runs
def Chip.lines(+ls: List<&2, String>, +cs: List<&2, Chip>) -> List<&2, List<&2, Run>>:
  Chip.lines.go(ls, Ln{cs, Nil{}})

# whether the runs hold a chip
def Chip.has(+rs: List<&2, Run>) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case RChip{_} <> _:
      True{}
    case _ <> +t:
      Chip.has(t)
