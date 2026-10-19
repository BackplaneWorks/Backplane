import Base
import ./cad.bend as C
import ./json.bend as J

# Releases (pure): a git tag's name, the next tag to propose, the plan of
# commands that build a release's outputs from a CAD tool's templates
# (core/cad.bend), the release folder, its manifest and notes, and the
# `gh release create` command. The server runs what these return.

# Tag names
# ---------

# a character git refuses in a ref name, or whitespace and control characters
def Rel.badc(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_le(c, 32), U32.is_eq(c, 127)), Bool.or(Bool.or(U32.is_eq(c, 126), U32.is_eq(c, 94)), Bool.or(Bool.or(U32.is_eq(c, 58), U32.is_eq(c, 63)), Bool.or(Bool.or(U32.is_eq(c, 42), U32.is_eq(c, 91)), U32.is_eq(c, 92)))))

# no bad character anywhere in s
def Rel.clean(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{+c}, t}:
      Rel.clean(t, Bool.and(ok, Bool.not(Rel.badc(c))))

def Rel.has(+s: String, +p: String) -> Bool:
  String.contains(s, p)

# Is this a name git accepts for a tag? Not empty; no whitespace, control or
# ~ ^ : ? * [ \ characters; no "..", "@{", "//"; not "@"; no leading '-', '/'
# or '.'; no trailing ".lock", '.' or '/'; no component starting with '.'
def Release.valid(+t: String) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.not(String.is_empty(t)), Rel.clean(t, True{})), Bool.and(Bool.not(Rel.has(t, "..")), Bool.not(Rel.has(t, "@{")))), Bool.and(Bool.and(Bool.not(Rel.has(t, "//")), Bool.not(Rel.has(t, "/."))), Bool.and(Bool.and(Bool.not(String.eq(t, "@")), Bool.not(String.starts_with(t, "-"))), Bool.and(Bool.and(Bool.not(String.starts_with(t, "/")), Bool.not(String.starts_with(t, "."))), Bool.and(Bool.not(String.ends_with(t, ".lock")), Bool.and(Bool.not(String.ends_with(t, ".")), Bool.not(String.ends_with(t, "/"))))))))

# Next tag
# --------

def Rel.isdigit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def Rel.isalpha(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)))

def Rel.inc.n(nine: Bool, +c: U32, t: String, rec: String) -> String:
  match nine:
    case True{}:
      SCon{Chr{48}, rec}
    case False{}:
      SCon{Chr{(c + 1 : U32)}, t}

def Rel.inc.d(dig: Bool, nine: Bool, +c: U32, t: String, rec: String, s: String) -> String:
  match dig:
    case False{}:
      "1" ++ s
    case True{}:
      Rel.inc.n(nine, c, t, rec)

# a reversed digit run plus one, still reversed: 9 carries; past the run a
# new leading 1 (reversed: appended)
def Rel.inc(+s: String) -> String:
  match s:
    case SNil{}:
      "1"
    case SCon{Chr{+c}, +t}:
      Rel.inc.d(Rel.isdigit(c), U32.is_eq(c, 57), c, t, Rel.inc(t), s)

def Rel.letter.k(wrap: Bool, +c: U32, t: String) -> String:
  match wrap:
    case True{}:
      SCon{Chr{(c - 25 : U32)}, SCon{Chr{(c - 25 : U32)}, t}}
    case False{}:
      SCon{Chr{(c + 1 : U32)}, t}

# the letter after c: A..Y, a..y; Z and z give AA and aa (reversed already)
def Rel.letter(+c: U32, t: String) -> String:
  Rel.letter.k(Bool.or(U32.is_eq(c, 90), U32.is_eq(c, 122)), c, t)

def Rel.bump.l(lone: Bool, +c: U32, t: String, s: String) -> String:
  match lone:
    case True{}:
      Rel.letter(c, t)
    case False{}:
      s

# the char before the letter (next in the reversed string) is not a letter
def Rel.lone(t: String) -> Bool:
  match t:
    case SNil{}:
      True{}
    case SCon{Chr{c}, _}:
      Bool.not(Rel.isalpha(c))

def Rel.bump.a(alpha: Bool, +c: U32, +t: String, s: String) -> String:
  match alpha:
    case True{}:
      Rel.bump.l(Rel.lone(t), c, t, s)
    case False{}:
      s

def Rel.bump.c(dig: Bool, alpha: Bool, +c: U32, +t: String, s: String) -> String:
  match dig:
    case True{}:
      Rel.inc(s)
    case False{}:
      Rel.bump.a(alpha, c, t, s)

# on the reversed tag: a trailing number is incremented, a trailing single
# letter (after a non-letter) goes to the next; anything else is left alone
def Rel.bump.r(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, +t}:
      Rel.bump.c(Rel.isdigit(c), Rel.isalpha(c), c, t, s)

# the tag one step on: v1.2 -> v1.3, rev-A -> rev-B, r09 -> r10, r9 -> r10;
# a tag with no number or single letter at its end is returned as it is
def Release.bump(+t: String) -> String:
  String.reverse(Rel.bump.r(String.reverse(t)))

# can the tag be bumped (it ends in a digit or a single letter)
def Release.bumpable(+t: String) -> Bool:
  Bool.and(Release.valid(t), Bool.not(String.eq(Release.bump(t), t)))

# a tag as pieces: digit runs as numbers, the text between as text
type Piece is Data:
  PNum{n: Nat}
  PTxt{s: String}

# number run so far or text run so far, and the pieces finished (newest first)
type Ps is Data:
  Ps{num: Bool, n: Nat, txt: String, out: List<&2, Piece>}

def Rel.flush.t(none: Bool, +txt: String, out: List<&2, Piece>) -> List<&2, Piece>:
  match none:
    case True{}:
      out
    case False{}:
      PTxt{txt} <> out

def Rel.flush.k(num: Bool, n: Nat, +txt: String, out: List<&2, Piece>) -> List<&2, Piece>:
  match num:
    case True{}:
      PNum{n} <> out
    case False{}:
      Rel.flush.t(String.is_empty(txt), txt, out)

def Rel.flush(st: Ps) -> List<&2, Piece>:
  match st:
    case Ps{num, n, txt, out}:
      Rel.flush.k(num, n, txt, out)

# a digit: extends a number run, or closes the text run and starts a number
def Rel.pstep.d(num: Bool, n: Nat, +txt: String, out: List<&2, Piece>, d: Nat) -> Ps:
  match num:
    case True{}:
      Ps{True{}, Nat.add(Nat.mul(n, 10n), d), "", out}
    case False{}:
      Ps{True{}, d, "", Rel.flush.t(String.is_empty(txt), txt, out)}

# not a digit: extends a text run, or closes the number run
def Rel.pstep.x(num: Bool, n: Nat, +txt: String, out: List<&2, Piece>, +c: U32) -> Ps:
  match num:
    case True{}:
      Ps{False{}, 0n, SCon{Chr{c}, SNil{}}, PNum{n} <> out}
    case False{}:
      Ps{False{}, 0n, txt ++ SCon{Chr{c}, SNil{}}, out}

def Rel.pstep.k(dig: Bool, num: Bool, n: Nat, txt: String, out: List<&2, Piece>, +c: U32) -> Ps:
  match dig:
    case True{}:
      Rel.pstep.d(num, n, txt, out, U32.to_nat((c - 48 : U32)))
    case False{}:
      Rel.pstep.x(num, n, txt, out, c)

def Rel.pstep(+c: U32, st: Ps) -> Ps:
  match st:
    case Ps{num, n, txt, out}:
      Rel.pstep.k(Rel.isdigit(c), num, n, txt, out, c)

def Rel.pcs(s: String, st: Ps) -> Ps:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Rel.pcs(t, Rel.pstep(c, st))

def Rel.pieces(+t: String) -> List<&2, Piece>:
  List.reverse(&2, Piece, Rel.flush(Rel.pcs(t, Ps{False{}, 0n, "", Nil{}})))

# numbers before text; numbers by value, text by String.cmp
def Rel.pcmp(a: Piece, b: Piece) -> Cmp:
  match a b:
    case PNum{x} PNum{y}:
      Nat.cmp(x, y)
    case PTxt{x} PTxt{y}:
      String.order(x, y)
    case PNum{_} PTxt{_}:
      LT{}
    case PTxt{_} PNum{_}:
      GT{}

def Rel.then(c: Cmp, rest: Cmp) -> Cmp:
  match c:
    case EQ{}:
      rest
    case _:
      c

def Rel.lcmp(a: List<&2, Piece>, b: List<&2, Piece>) -> Cmp:
  match a b:
    case Nil{} Nil{}:
      EQ{}
    case Nil{} y <> ys:
      LT{}
    case x <> xs Nil{}:
      GT{}
    case +x <> xs +y <> ys:
      Rel.then(Rel.pcmp(x, y), Rel.lcmp(xs, ys))

def Rel.newer.k(c: Cmp) -> Bool:
  match c:
    case GT{}:
      True{}
    case _:
      False{}

def Rel.newer(+a: String, +b: String) -> Bool:
  Rel.newer.k(Rel.lcmp(Rel.pieces(a), Rel.pieces(b)))

def Rel.max.k(newer: Bool, +t: String, +best: String) -> String:
  match newer:
    case True{}:
      t
    case False{}:
      best

# the newest bumpable tag; "" when none
def Rel.max(ts: List<&2, String>, +best: String) -> String:
  match ts:
    case Nil{}:
      best
    case +t <> r:
      Rel.max(r, Rel.max.k(Bool.and(Release.bumpable(t), Bool.or(String.is_empty(best), Rel.newer(t, best))), t, best))

def Rel.next.k(none: Bool, +best: String) -> String:
  match none:
    case True{}:
      "v1"
    case False{}:
      Release.bump(best)

def Rel.next.k2(+best: String) -> String:
  Rel.next.k(String.is_empty(best), best)

# the tag to propose after the existing ones: the newest one bumped (v1.2 ->
# v1.3, rev-A -> rev-B, r7 -> r8), "v1" when there is none
def Release.next(ts: List<&2, String>) -> String:
  Rel.next.k2(Rel.max(ts, ""))

# The plan
# --------

# <repo>/.backplane/releases/<tag>
def Release.dir(+repo: String, +tag: String) -> String:
  repo ++ "/.backplane/releases/" ++ tag

# the file name without its folder
def Rel.base(ps: List<&2, String>, last: String) -> String:
  match ps:
    case Nil{}:
      last
    case +p <> t:
      Rel.base(t, p)

def Rel.first(ps: List<&2, String>) -> String:
  match ps:
    case h <> _:
      h
    case Nil{}:
      ""

def Rel.init(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case _ <> Nil{}:
      Nil{}
    case +h <> t:
      h <> Rel.init(t)

def Rel.stem.k(+ps: List<&2, String>) -> String:
  match ps:
    case +a <> _ <> _:
      String.join(Rel.init(ps), ".")
    case _:
      Rel.first(ps)

# the file name without folder or extension ("/r/a/board.kicad_pcb" -> "board")
def Release.stem(+path: String) -> String:
  Rel.stem.k(String.split(Rel.base(String.split(path, '/'), ""), '.'))

# one command of the plan
type Step is Data:
  Step{id: String, label: String, argv: List<&2, String>}

type Plan is Data:
  Plan{tag: String, dir: String, steps: List<&2, Step>}

def Rel.in(xs: List<&2, String>, +w: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, w), Rel.in(t, w))

def Rel.keep(on: Bool, +i: String, rest: List<&2, String>) -> List<&2, String>:
  match on:
    case True{}:
      i <> rest
    case False{}:
      rest

# the ids of the outputs on by default
def Release.defaults.go(os: List<&2, C.Out>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case C.Out{+i, _, _, on} <> t:
      Rel.keep(on, i, Release.defaults.go(t))

def Release.defaults(t: C.Tool) -> List<&2, String>:
  match t:
    case C.Tool{_, _, _, _, os}:
      Release.defaults.go(os)

# does a command use a placeholder whose value is missing
def Rel.needs(argv: List<&2, String>, +ph: String, +val: String) -> Bool:
  Bool.and(String.is_empty(val), Rel.in(argv, ph))

def Rel.ok(+argv: List<&2, String>, +pcb: String, +sch: String) -> Bool:
  Bool.not(Bool.or(Rel.needs(argv, "{pcb}", pcb), Rel.needs(argv, "{sch}", sch)))

def Rel.step.k(runs: Bool, +id: String, +label: String, +argv: List<&2, String>, vars: List<&2, C.Var>, rest: List<&2, Step>) -> List<&2, Step>:
  match runs:
    case True{}:
      Step{id, label, C.Cad.fill(argv, vars)} <> rest
    case False{}:
      rest

def Rel.steps(os: List<&2, C.Out>, +ids: List<&2, String>, +pcb: String, +sch: String, +vars: List<&2, C.Var>) -> List<&2, Step>:
  match os:
    case Nil{}:
      Nil{}
    case C.Out{+i, +l, +argv, _} <> t:
      Rel.step.k(Bool.and(Rel.in(ids, i), Rel.ok(argv, pcb, sch)), i, l, argv, vars, Rel.steps(t, ids, pcb, sch, vars))

def Rel.vars(+cli: String, +pcb: String, +sch: String, +out: String, +name: String) -> List<&2, C.Var>:
  [C.Var{"cli", cli}, C.Var{"pcb", pcb}, C.Var{"sch", sch}, C.Var{"out", out}, C.Var{"name", name}]

def Rel.name.k(none: Bool, +pcb: String, +sch: String) -> String:
  match none:
    case True{}:
      Release.stem(sch)
    case False{}:
      Release.stem(pcb)

def Rel.name(+pcb: String, +sch: String) -> String:
  Rel.name.k(String.is_empty(pcb), pcb, sch)

def Rel.plan.k(os: List<&2, C.Out>, ids: List<&2, String>, +tag: String, +dir: String, +cli: String, +pcb: String, +sch: String) -> Plan:
  Plan{tag, dir, Rel.steps(os, ids, pcb, sch, Rel.vars(cli, pcb, sch, dir, Rel.name(pcb, sch)))}

# The commands that make a release from a tool's output templates: those
# whose ids were chosen, in the tool's order, with {cli} {pcb} {sch} {out}
# {name} put in; outputs that need a board or schematic that was not given
# are left out. The caller makes the folder and runs the steps there.
def Release.plan(t: C.Tool, ids: List<&2, String>, +repo: String, +tag: String, +cli: String, +pcb: String, +sch: String) -> Plan:
  match t:
    case C.Tool{_, _, _, _, os}:
      Rel.plan.k(os, ids, tag, Release.dir(repo, tag), cli, pcb, sch)

# The manifest
# ------------

def Rel.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      J.Str{h} <> Rel.strs(t)

# {"tag":..,"commit":..,"date":..,"files":[..]} as JSON text; the date is
# given (ISO 8601), pure code has no clock
def Release.manifest(+tag: String, +commit: String, +date: String, files: List<&2, String>) -> String:
  J.Json.show(J.Json.obj([J.KV{"tag", J.Str{tag}}, J.KV{"commit", J.Str{commit}}, J.KV{"date", J.Str{date}}, J.KV{"files", J.Json.arr(Rel.strs(files))}]))

def Rel.sec.k(none: Bool, +title: String, +body: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      "\n## " ++ title ++ "\n\n" ++ String.trim(body) ++ "\n"

# Notes
# -----

def Rel.sec(+title: String, +body: String) -> String:
  Rel.sec.k(String.is_empty(String.trim(body)), title, body)

def Rel.since.k(none: Bool, +prev: String) -> String:
  match none:
    case True{}:
      "First release."
    case False{}:
      "Changes since " ++ prev ++ "."

def Rel.since(+prev: String) -> String:
  Rel.since.k(String.is_empty(prev), prev)

# the release notes: the tag, the commit, what it is since the previous tag,
# the BOM changes and the checks' summary (each section left out when its
# text is empty)
def Release.notes(+tag: String, +prev: String, +commit: String, +bom: String, +checks: String) -> String:
  "# " ++ tag ++ "\n\nCommit `" ++ commit ++ "`. " ++ Rel.since(prev) ++ "\n" ++ Rel.sec("Checks", checks) ++ Rel.sec("BOM changes", bom)

# the command that publishes the release on GitHub: the tag must already
# exist on the remote (--verify-tag); files are attached
def Release.gh(+tag: String, +notes: String, files: List<&2, String>) -> List<&2, String>:
  List.append(&2, String, ["gh", "release", "create", tag, "--title", tag, "--notes-file", notes, "--verify-tag"], files)

# An export is complete only when its command succeeded. Keep the step's
# name and a bounded diagnostic for the release report.
def Release.command_error(+label: String, code: U32, +output: String) -> String:
  match code:
    case 0:
      ""
    case +other:
      label ++ " failed (exit " ++ U32.show(other) ++ ")" ++ Bool.pick(String, String.is_empty(String.trim(output)), "", ": " ++ String.trim(String.take(output, 300n)))
