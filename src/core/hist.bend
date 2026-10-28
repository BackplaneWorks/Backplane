import Base
import ./board.bend as B
import ./diff.bend as DF
import ./json.bend as J
import ./model.bend as M

# A design's history in the viewer. After each tool call that changed a
# .kicad_sch or .kicad_pcb, the hub snapshots the thread's KiCad files as
# a commit on a hidden ref (a step, server git.bend's Git.step) and logs it
# in the thread as an Act of tone "step" whose summary is the step's record
# (Hist.record). Here: the records read back, the steps that touched a
# board or a schematic (a track), the viewer's paths for a version and for
# a comparison of two, and the comparison itself: items marked removed,
# changed or added, the rest dimmed (Hist.mark).

# Versions
# --------
# The viewer's path names a live file, a file in a commit, or a
# comparison:
#   /abs/file.kicad_pcb
#   git\t<git dir>\t<commit>\t<length of top>\t<top>/<rel>
#   diff\n<old>\n<new>            (old "" when the file is new)
# A version's last field is the live file's absolute path, so a sheet's
# child resolves beside it (hier.bend's Hier.resolve) and stays in the
# same commit, and every version of a file shares its family: moving
# between them merges the scene, so only what changed fades in.

def Hist.git(+g: String, +c: String, +top: String, +rel: String) -> String:
  "git\t" ++ g ++ "\t" ++ c ++ "\t" ++ Nat.show(String.length(top)) ++ "\t" ++ top ++ "/" ++ rel

def Hist.diff(+a: String, +b: String) -> String:
  "diff\n" ++ a ++ "\n" ++ b

# 0 a live file, 1 a file in a commit, 2 a comparison
def Hist.kind(+p: String) -> Nat:
  Bool.pick(Nat, String.starts_with(p, "git\t"), 1n, Bool.pick(Nat, String.starts_with(p, "diff\n"), 2n, 0n))

def Hist.at(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def Hist.field(+p: String, +i: Nat) -> String:
  Hist.at(String.split(p, '\t'), i)

# a comparison's old and new versions
def Hist.old(+p: String) -> String:
  Hist.at(String.split(p, '\n'), 1n)

def Hist.new(+p: String) -> String:
  Hist.at(String.split(p, '\n'), 2n)

def Hist.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Hist.num(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

def Hist.abs.k(k: Nat, +p: String) -> String:
  match k:
    case 1n:
      Hist.field(p, 4n)
    case _:
      p

# the live file a version is of
def Hist.abs(+p: String) -> String:
  Hist.abs.k(Hist.kind(p), p)

def Hist.family.k(k: Nat, +p: String) -> String:
  match k:
    case 2n:
      Hist.abs(Hist.new(p))
    case _:
      Hist.abs(p)

# the live file a version or a comparison is of: moving between them the
# viewer merges its scene (Vw.board.k: an unchanged item does not fade
# again) and keeps its view (no refit)
def Hist.family(+p: String) -> String:
  Hist.family.k(Hist.kind(p), p)

# a source key's family ("kind|root|path" with the path's family)
def Hist.key.family(+key: String) -> String:
  Hist.at(String.split(key, '|'), 0n) ++ "|" ++ Hist.at(String.split(key, '|'), 1n) ++ "|" ++ Hist.family(Hist.at(String.split(key, '|'), 2n))

# Paths inside a commit are relative to its top, "a/./b" and "a/x/../b"
# folded (a sheet names its children from its own folder)
def Hist.norm.step(+x: String, acc: List<&2, String>) -> List<&2, String>:
  match acc:
    case Nil{}:
      Bool.pick(List<&2, String>, Bool.or(String.is_empty(x), String.eq(x, ".")), Nil{}, [x])
    case +h <> +t:
      Bool.pick(List<&2, String>, Bool.or(String.is_empty(x), String.eq(x, ".")), h <> t,
        Bool.pick(List<&2, String>, Bool.and(String.eq(x, ".."), Bool.not(String.eq(h, ".."))), t, x <> h <> t))

def Hist.norm.go(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Hist.norm.go(t, Hist.norm.step(x, acc))

def Hist.norm(+p: String) -> String:
  String.join(List.reverse(&2, String, Hist.norm.go(String.split(p, '/'), Nil{})), "/")

def Hist.rebase.k(k: Nat, +p: String, +abs: String) -> String:
  match k:
    case 1n:
      "git\t" ++ Hist.field(p, 1n) ++ "\t" ++ Hist.field(p, 2n) ++ "\t" ++ Hist.field(p, 3n) ++ "\t" ++ abs
    case _:
      abs

# another live file (abs) in the same commit as version p (a schematic's
# root sheet for the sheet tree); abs itself when p is live
def Hist.rebase(+p: String, +abs: String) -> String:
  Hist.rebase.k(Hist.kind(p), p, abs)

# a file in a commit: its git dir, commit, and path from the top
def Hist.git.dir(+p: String) -> String:
  Hist.field(p, 1n)

def Hist.git.commit(+p: String) -> String:
  Hist.field(p, 2n)

def Hist.git.rel(+p: String) -> String:
  Hist.norm(String.drop(Hist.field(p, 4n), 1n+Hist.num(Hist.field(p, 3n), 0n)))

# how `git show` names it
def Hist.git.obj(+p: String) -> String:
  Hist.git.commit(p) ++ ":" ++ Hist.git.rel(p)

# Kinds of file
# -------------

# 0 a board, 1 a schematic, 2 neither
def Hist.ext(+path: String) -> Nat:
  Bool.pick(Nat, String.ends_with(path, ".kicad_pcb"), 0n, Bool.pick(Nat, String.ends_with(path, ".kicad_sch"), 1n, 2n))

# Steps
# -----

# a file a step touched: A added, M modified, D deleted; rel from the top
type SFile is Data:
  SFile{st: String, rel: String}

# id: its Act's; entry: the tool call (or message) it followed, "" when
# the turn's end took it; prev: the step before ("" for the first)
type Step is Data:
  Step{id: String, entry: String, commit: String, prev: String, git: String, top: String, files: List<&2, SFile>, at: Nat}

def Step.tone() -> String:
  "step"

def SFile.of(+l: String) -> SFile:
  SFile{Hist.at(String.split(l, '\t'), 0n), Hist.at(String.split(l, '\t'), 1n)}

def SFile.line(f: SFile) -> String:
  match f:
    case SFile{+st, +rel}:
      st ++ "\t" ++ rel

def SFile.lines(fs: List<&2, SFile>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case f <> t:
      J.Str{SFile.line(f)} <> SFile.lines(t)

def SFile.keep(blank: Bool, +x: String, rest: List<&2, SFile>) -> List<&2, SFile>:
  match blank:
    case True{}:
      rest
    case False{}:
      SFile.of(x) <> rest

def SFile.parse(xs: List<&2, String>) -> List<&2, SFile>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      SFile.keep(String.is_empty(String.trim(x)), x, SFile.parse(t))

def SFile.strs(xs: List<&2, J.Json>) -> List<&2, SFile>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+x} <> t:
      SFile.of(x) <> SFile.strs(t)
    case _ <> t:
      SFile.strs(t)

# git's -z output: status, path, status, path... (a NUL after each)
def SFile.pairs(xs: List<&2, String>) -> List<&2, SFile>:
  match xs:
    case +st <> +p <> t:
      SFile.keep(Bool.or(String.is_empty(st), String.is_empty(p)), st ++ "\t" ++ p, SFile.pairs(t))
    case _:
      Nil{}

def Hist.record.k(+ls: List<&2, String>, +entry: String, +ws: List<&2, J.KV>) -> String:
  Bool.pick(String, String.is_empty(String.trim(Hist.at(ls, 0n))), "",
    J.Json.show(J.Json.obj(List.append(&2, J.KV, [J.KV{"e", J.Str{entry}}, J.KV{"c", J.Str{String.trim(Hist.at(ls, 0n))}}, J.KV{"p", J.Str{String.trim(Hist.at(ls, 1n))}},
      J.KV{"g", J.Str{String.trim(Hist.at(ls, 2n))}}, J.KV{"t", J.Str{String.trim(Hist.at(ls, 3n))}},
      J.KV{"f", J.Json.arr(SFile.lines(SFile.pairs(String.split(String.join(List.drop(&2, String, ls, 4n), "\n"), Chr{0}))))}], ws))))

# the capture script's output (commit, previous commit, git dir and top a
# line each, then git's NUL-separated status and path of each file
# changed) as a step's record; "" when nothing changed
def Hist.record(+out: String, +entry: String) -> String:
  Hist.record.k(String.split(out, '\n'), entry, Nil{})

# the same, saying when it was taken ("w": "start" at a turn's start,
# before the agent runs; "" after a tool call or at the turn's end), so
# the design as it was before is told from a first edit (Step.base)
def Hist.record.at(+out: String, +entry: String, +when: String) -> String:
  Hist.record.k(String.split(out, '\n'), entry, [J.KV{"w", J.Str{when}}])

def Step.read(+id: String, +at: Nat, +o: J.Json) -> Step:
  Step{id, J.Json.get_str(o, "e", ""), J.Json.get_str(o, "c", ""), J.Json.get_str(o, "p", ""), J.Json.get_str(o, "g", ""), J.Json.get_str(o, "t", ""),
    SFile.strs(J.Json.elems(J.Json.get_obj(o, "f"))), at}

def Step.parsed(m: Maybe<&2, J.Json>, +id: String, +at: Nat) -> Maybe<&2, Step>:
  match m:
    case Some{+o}:
      Some{Step.read(id, at, o)}
    case None{}:
      None{}

def Step.of.k(step: Bool, +id: String, +summary: String, +at: Nat) -> Maybe<&2, Step>:
  match step:
    case True{}:
      Step.parsed(J.Json.parse(summary), id, at)
    case False{}:
      None{}

# a step's Act read back (None for any other entry)
def Step.of(e: M.Entry) -> Maybe<&2, Step>:
  match e:
    case M.Act{+id, _, +tone, _, +summary, +at}:
      Step.of.k(String.eq(tone, Step.tone()), id, summary, at)
    case M.Msg{_, _, _, _, _}:
      None{}

def Step.is(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      String.eq(tone, Step.tone())
    case M.Msg{_, _, _, _, _}:
      False{}

def Step.put(m: Maybe<&2, Step>, acc: List<&2, Step>) -> List<&2, Step>:
  match m:
    case Some{s}:
      s <> acc
    case None{}:
      acc

def Step.all(es: List<&2, M.Entry>, acc: List<&2, Step>) -> List<&2, Step>:
  match es:
    case Nil{}:
      acc
    case e <> t:
      Step.all(t, Step.put(Step.of(e), acc))

# a thread's steps, oldest first
def Hist.steps(+s: M.State, +th: String) -> List<&2, Step>:
  Step.all(M.State.steps(s, th), Nil{})

def Step.id(s: Step) -> String:
  match s:
    case Step{i, _, _, _, _, _, _, _}:
      i

def Step.entry(s: Step) -> String:
  match s:
    case Step{_, e, _, _, _, _, _, _}:
      e

def Step.files(s: Step) -> List<&2, SFile>:
  match s:
    case Step{_, _, _, _, _, _, fs, _}:
      fs

def Step.first(s: Step) -> Bool:
  match s:
    case Step{_, _, _, +p, _, _, _, _}:
      String.is_empty(p)

def SFile.rel(f: SFile) -> String:
  match f:
    case SFile{_, r}:
      r

def SFile.gone(f: SFile) -> Bool:
  match f:
    case SFile{+st, _}:
      String.eq(st, "D")

def SFile.of_kind(+f: SFile, +kind: Nat) -> Bool:
  Bool.and(Nat.is_eq(Hist.ext(SFile.rel(f)), kind), Bool.not(SFile.gone(f)))

def SFile.any(fs: List<&2, SFile>, +kind: Nat) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case +f <> t:
      Bool.or(SFile.of_kind(f, kind), SFile.any(t, kind))

# whether a step changed a file of the kind (a deletion shows nothing)
def Step.has(+s: Step, +kind: Nat) -> Bool:
  SFile.any(Step.files(s), kind)

def Hist.track.k(has: Bool, s: Step, rest: List<&2, Step>) -> List<&2, Step>:
  match has:
    case True{}:
      s <> rest
    case False{}:
      rest

# the steps that changed a board (kind 0) or a schematic (1), oldest first
def Hist.track(ss: List<&2, Step>, +kind: Nat) -> List<&2, Step>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Hist.track.k(Step.has(s, kind), s, Hist.track(t, kind))

def Step.abs(+s: Step, +rel: String) -> String:
  match s:
    case Step{_, _, _, _, _, top, _, _}:
      top ++ "/" ++ rel

def SFile.pick.go(fs: List<&2, SFile>, +kind: Nat, +want: String, +top: String, +first: String) -> String:
  match fs:
    case Nil{}:
      first
    case +f <> t:
      Bool.pick(String, Bool.and(SFile.of_kind(f, kind), String.eq(top ++ "/" ++ SFile.rel(f), want)), SFile.rel(f),
        SFile.pick.go(t, kind, want, top, Bool.pick(String, Bool.and(String.is_empty(first), SFile.of_kind(f, kind)), SFile.rel(f), first)))

# the file of the kind a step shows: the one on show (want, its live
# path) when the step changed it, else the first it changed
def Step.file(+s: Step, +kind: Nat, +want: String) -> String:
  match s:
    case Step{_, _, _, _, _, +top, fs, _}:
      SFile.pick.go(fs, kind, want, top, "")

def Step.spec.at(+s: Step, +c: String, +rel: String) -> String:
  match s:
    case Step{_, _, _, _, +g, +top, _, _}:
      Bool.pick(String, String.is_empty(c), "", Hist.git(g, c, top, rel))

def Step.commit(s: Step) -> String:
  match s:
    case Step{_, _, c, _, _, _, _, _}:
      c

def Step.prev(s: Step) -> String:
  match s:
    case Step{_, _, _, p, _, _, _, _}:
      p

# the viewer's path for a step: the file as the step left it, or (diff)
# compared with the step before (a file the step made compares with none)
def Step.spec.k(diff: Bool, +s: Step, +rel: String) -> String:
  match diff:
    case True{}:
      Hist.diff(Step.spec.at(s, Step.prev(s), rel), Step.spec.at(s, Step.commit(s), rel))
    case False{}:
      Step.spec.at(s, Step.commit(s), rel)

def Step.spec(+s: Step, +kind: Nat, +want: String, diff: Bool) -> String:
  Step.spec.k(diff, s, Step.file(s, kind, want))

# Positions
# ---------
# The track is numbered from 1; 0 is the live file.

# Walks that stop at a hit: the head's test is computed before the step
# (a helper that recursed back would be mutual recursion)

def Hist.at_id(es: List<&2, M.Entry>, +id: String) -> Bool:
  match es:
    case +e <> _:
      String.eq(M.Idx.id(e), id)
    case Nil{}:
      False{}

def Hist.at_step(es: List<&2, M.Entry>) -> Bool:
  match es:
    case +e <> _:
      Step.is(e)
    case Nil{}:
      False{}

def Hist.find.go(es: List<&2, M.Entry>, hit: Bool, +id: String) -> Maybe<&2, M.Entry>:
  match es hit:
    case Nil{} _:
      None{}
    case e <> +r True{}:
      Some{e}
    case +x <> +t False{}:
      Hist.find.go(t, Hist.at_id(t, id), id)

def Hist.find(+es: List<&2, M.Entry>, +id: String) -> Maybe<&2, M.Entry>:
  Hist.find.go(es, Hist.at_id(es, id), id)

# the first entry that is not a step
def Hist.anchor.next.go(es: List<&2, M.Entry>, step: Bool) -> String:
  match es step:
    case Nil{} _:
      ""
    case +x <> +t True{}:
      Hist.anchor.next.go(t, Hist.at_step(t))
    case e <> +r False{}:
      M.Idx.id(e)

def Hist.anchor.next(+es: List<&2, M.Entry>) -> String:
  Hist.anchor.next.go(es, Hist.at_step(es))

# past the step's own Act, then the first entry that is not a step
def Hist.anchor.find.go(es: List<&2, M.Entry>, hit: Bool, +id: String) -> String:
  match es hit:
    case Nil{} _:
      ""
    case +x <> +t True{}:
      Hist.anchor.next(t)
    case +x <> +t False{}:
      Hist.anchor.find.go(t, Hist.at_id(t, id), id)

def Hist.anchor.find(+es: List<&2, M.Entry>, +id: String) -> String:
  Hist.anchor.find.go(es, Hist.at_id(es, id), id)

def Hist.at_sid(ss: List<&2, Step>, +id: String) -> Bool:
  match ss:
    case +s <> _:
      String.eq(Step.id(s), id)
    case Nil{}:
      False{}

def Hist.index.go(ss: List<&2, Step>, hit: Bool, +id: String, +i: Nat) -> Nat:
  match ss hit:
    case Nil{} _:
      0n
    case +x <> +r True{}:
      i
    case +x <> +t False{}:
      Hist.index.go(t, Hist.at_sid(t, id), id, 1n+i)

def Hist.index(+ss: List<&2, Step>, +id: String) -> Nat:
  Bool.pick(Nat, String.is_empty(id), 0n, Hist.index.go(ss, Hist.at_sid(ss, id), id, 1n))

def Hist.nth(ss: List<&2, Step>, +i: Nat) -> Maybe<&2, Step>:
  match i:
    case 0n:
      None{}
    case 1n+j:
      List.head(&2, Step, List.drop(&2, Step, ss, j))

def Hist.nth.id(m: Maybe<&2, Step>) -> String:
  match m:
    case Some{s}:
      Step.id(s)
    case None{}:
      ""

# the step at position i ("" for live, or past either end)
def Hist.id_at(+ss: List<&2, Step>, +i: Nat) -> String:
  Hist.nth.id(Hist.nth(ss, i))

# Where the arrows go: from live, back is the last step and forward stays
# live; from the last step, forward is live
def Hist.back(+n: Nat, +at: Nat) -> Nat:
  match at:
    case 0n:
      n
    case 1n:
      1n
    case 1n+p:
      p

def Hist.fwd(+n: Nat, +at: Nat) -> Nat:
  Bool.pick(Nat, Bool.or(Nat.is_eq(at, 0n), Nat.is_ge(at, n)), 0n, 1n+at)

# Visual diff
# -----------
# Two versions of a file, each item keyed as the viewer keys it: an item
# only the old one has is drawn on the "removed" layer, one only the new
# one has on "added", one whose signature changed twice (as it was, on
# "removed", and as it is, on "changed"); an unchanged item keeps its
# layer, and the viewer dims those (Look.of with a diff kind, look.bend).

def Mark.removed() -> U32:
  26

def Mark.changed() -> U32:
  27

def Mark.added() -> U32:
  28

def Mark.is(+layer: U32) -> Bool:
  Bool.and(U32.is_ge(layer, Mark.removed()), U32.is_le(layer, Mark.added()))

def Mark.key.cmp.fin(r: (String & String) & Cmp) -> Cmp:
  (_, c) = r
  c

def Mark.key.cmp(a: B.Item, b: B.Item) -> Cmp:
  match a b:
    case B.Item{ka, _, _, _, _} B.Item{kb, _, _, _, _}:
      Mark.key.cmp.fin(String.cmp(ka, kb))

def Mark.key.le(a: B.Item, b: B.Item) -> Bool:
  Cmp.is_le(Mark.key.cmp(a, b))

def Mark.sort(+xs: List<&2, B.Item>) -> List<&2, B.Item>:
  List.sort(~B.Item, ~Mark.key.le, xs)

# a marked item takes its layer's colour: a tinted box becomes a plain fill
def Mark.prim(p: B.Prim) -> B.Prim:
  match p:
    case B.PTint{ps, _, _}:
      B.PPoly{ps, 0.0, True{}}
    case _:
      p

def Mark.on(+layer: U32, +suffix: String, i: B.Item) -> B.Item:
  match i:
    case B.Item{k, s, _, p, info}:
      B.Item{k ++ suffix, s, layer, Mark.prim(p), info}

# an unchanged tinted box dims like its layer
def Mark.dim.prim(p: B.Prim) -> B.Prim:
  match p:
    case B.PTint{ps, c, +a}:
      B.PTint{ps, c, (a / 3 : U32)}
    case _:
      p

def Mark.dim(i: B.Item) -> B.Item:
  match i:
    case B.Item{k, s, l, p, info}:
      B.Item{k, s, l, Mark.dim.prim(p), info}

# every item dimmed: a version compared with itself (law hist_mark_self)
def Mark.dims(xs: List<&2, B.Item>) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Mark.dim(x) <> Mark.dims(t)

def Mark.heads(os: List<&2, B.Item>, ns: List<&2, B.Item>) -> Cmp:
  match os ns:
    case +o <> _ +n <> _:
      Mark.key.cmp(o, n)
    case _ _:
      EQ{}

def Mark.all(+layer: U32, xs: List<&2, B.Item>) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Mark.on(layer, "", x) <> Mark.all(layer, t)

def Mark.same.k(same: Bool, o: B.Item, n: B.Item, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match same:
    case True{}:
      Mark.dim(n) <> rest
    case False{}:
      Mark.on(Mark.removed(), "~was", o) <> Mark.on(Mark.changed(), "", n) <> rest

def Mark.sig(i: B.Item) -> U32:
  match i:
    case B.Item{_, s, _, _, _}:
      s

# both lists by key; `c` is the order of their heads
def Mark.go(+os: List<&2, B.Item>, ns: List<&2, B.Item>, c: Cmp) -> List<&2, B.Item>:
  match os ns c:
    case Nil{} _ _:
      Mark.all(Mark.added(), ns)
    case _ Nil{} _:
      Mark.all(Mark.removed(), os)
    case _ <> +ot +n <> nt GT{}:
      Mark.on(Mark.added(), "", n) <> Mark.go(os, nt, Mark.heads(os, nt))
    case +o <> +ot +n <> +nt LT{}:
      Mark.on(Mark.removed(), "", o) <> Mark.go(ot, n <> nt, Mark.heads(ot, n <> nt))
    case +o <> +ot +n <> nt EQ{}:
      Mark.same.k(U32.is_eq(Mark.sig(o), Mark.sig(n)), o, n, Mark.go(ot, nt, Mark.heads(ot, nt)))

# the comparison of an old version's items with a new one's
def Hist.mark(+olds: List<&2, B.Item>, +news: List<&2, B.Item>) -> List<&2, B.Item>:
  Mark.go(Mark.sort(olds), Mark.sort(news), Mark.heads(Mark.sort(olds), Mark.sort(news)))

def Mark.add(hit: Bool, +n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

def Mark.count(xs: List<&2, B.Item>) -> Nat:
  match xs:
    case Nil{}:
      0n
    case B.Item{_, _, +l, _, _} <> t:
      Mark.add(Mark.is(l), Mark.count(t))

# how many items a comparison marks on each of its layers
def Mark.tally(xs: List<&2, B.Item>, +layer: U32) -> Nat:
  match xs:
    case Nil{}:
      0n
    case B.Item{_, _, +l, _, _} <> t:
      Mark.add(U32.is_eq(l, layer), Mark.tally(t, layer))

# Where a step came from
# ----------------------

def Hist.entry.id(e: M.Entry) -> String:
  M.Idx.id(e)

def Hist.anchor.k(named: Bool, +es: List<&2, M.Entry>, +s: Step) -> String:
  match named:
    case True{}:
      Step.entry(s)
    case False{}:
      Hist.anchor.find(es, Step.id(s))

def Hist.anchor(+es: List<&2, M.Entry>, +s: Step) -> String:
  Hist.anchor.k(Bool.not(String.is_empty(Step.entry(s))), es, s)

# an entry in a line: a tool call's name and summary, a message's first line
# (one walk to the first break: splitting a long message recursed once per
# character and overflowed the phones' engine, so its thread never opened)
def Hist.line(s: String) -> String:
  String.trim(DF.Diff.first(s))

def Hist.label.of(m: Maybe<&2, M.Entry>) -> String:
  match m:
    case Some{M.Act{_, _, _, +kind, +summary, _}}:
      kind ++ "  " ++ Hist.line(summary)
    case Some{M.Msg{_, _, _, +text, _}}:
      Hist.line(text)
    case None{}:
      ""

def Hist.label(+es: List<&2, M.Entry>, +id: String) -> String:
  Hist.label.of(Hist.find(es, id))

# what the model said before an entry, in the same turn: the entry itself
# when it is an assistant message, else the newest assistant message
# older than it ("" when the turn's message comes first). One pass: mode 0
# looks for the entry, 1 for what was said, 2 is done.
type Said is Data:
  Said{mode: Nat, text: String}

def Said.at(+e: M.Entry, +hit: Bool) -> Said:
  match e:
    case M.Msg{_, _, M.Assistant{}, +text, _}:
      Bool.pick(Said, hit, Said{2n, text}, Said{0n, ""})
    case _:
      Bool.pick(Said, hit, Said{1n, ""}, Said{0n, ""})

def Said.after(+e: M.Entry) -> Said:
  match e:
    case M.Msg{_, _, M.Assistant{}, +text, _}:
      Said{2n, text}
    case M.Msg{_, _, M.User{}, _, _}:
      Said{2n, ""}
    case _:
      Said{1n, ""}

def Said.step(+e: M.Entry, st: Said, +id: String) -> Said:
  match st:
    case Said{0n, _}:
      Said.at(e, String.eq(M.Idx.id(e), id))
    case Said{1n, _}:
      Said.after(e)
    case Said{m, t}:
      Said{m, t}

def Said.done(+st: Said) -> Bool:
  match st:
    case Said{2n, _}:
      True{}
    case _:
      False{}

def Said.text(st: Said) -> String:
  match st:
    case Said{_, t}:
      t

def Hist.said.go(es: List<&2, M.Entry>, fin: Bool, +st: Said, +id: String) -> String:
  match es fin:
    case _ True{}:
      Said.text(st)
    case Nil{} False{}:
      Said.text(st)
    case +e <> t False{}:
      Hist.said.go(t, Said.done(Said.step(e, st, id)), Said.step(e, st, id), id)

def Hist.said(+es: List<&2, M.Entry>, +id: String) -> String:
  Hist.said.go(es, False{}, Said{0n, ""}, id)

# Comparing with other versions
# -----------------------------
# The hub lists what a file may be compared with (history.refs): lines of
# "kind\tlabel\tcommit\ttime", kind head, branch, commit or turn.

type Ref is Data:
  Ref{kind: String, label: String, commit: String}

def Ref.of(+l: String) -> Ref:
  Ref{Hist.at(String.split(l, '\t'), 0n), Hist.at(String.split(l, '\t'), 1n), Hist.at(String.split(l, '\t'), 2n)}

def Ref.keep(blank: Bool, +l: String, rest: List<&2, Ref>) -> List<&2, Ref>:
  match blank:
    case True{}:
      rest
    case False{}:
      Ref.of(l) <> rest

def Hist.refs.go(ls: List<&2, String>) -> List<&2, Ref>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Ref.keep(String.is_empty(Hist.at(String.split(l, '\t'), 2n)), l, Hist.refs.go(t))

def Hist.refs(+text: String) -> List<&2, Ref>:
  Hist.refs.go(String.split(text, '\n'))

# A step's row in the thread
# --------------------------

def Hist.base(+p: String) -> String:
  Hist.at(List.reverse(&2, String, String.split(p, '/')), 0n)

def SFile.names(fs: List<&2, SFile>) -> List<&2, String>:
  match fs:
    case Nil{}:
      Nil{}
    case SFile{+st, +rel} <> t:
      (Hist.base(rel) ++ Bool.pick(String, String.eq(st, "D"), " (removed)", Bool.pick(String, String.eq(st, "A"), " (new)", ""))) <> SFile.names(t)

def Step.row.k(m: Maybe<&2, Step>) -> String:
  match m:
    case Some{+s}:
      Bool.pick(String, Step.first(s), "Design before the edits · ", "Design change · ") ++ String.join(SFile.names(Step.files(s)), ", ")
    case None{}:
      "Design change"

# "Design change · board.kicad_pcb, power.kicad_sch" (a thread's first
# step is the design as it was before)
def Hist.row(e: M.Entry) -> String:
  Step.row.k(Step.of(e))

# The design as it was before
# ---------------------------
# A thread's first step (no step before it) taken at a turn's start is
# the design as the agent found it: every file shows as new, yet nothing
# changed. It stays a version for the viewer's history to compare with,
# but it is no design change: the timeline leaves it out (Hist.seen) and
# the top bar's count does not count it (Hist.changes). A first step taken
# after a tool call (the agent made the first board) is a change. Records
# from before "w" was written say only that they are first.

def Step.base.k(first: Bool, +summary: String) -> Bool:
  match first:
    case True{}:
      Bool.or(String.contains(summary, "\"w\":\"start\""), Bool.not(String.contains(summary, "\"w\":")))
    case False{}:
      False{}

def Step.base(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, +summary, _}:
      Bool.and(String.eq(tone, Step.tone()), Step.base.k(String.contains(summary, "\"p\":\"\""), summary))
    case M.Msg{_, _, _, _, _}:
      False{}

def Hist.seen.k(base: Bool, e: M.Entry, rest: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match base:
    case True{}:
      rest
    case False{}:
      e <> rest

# entries without the design as it was before (in order)
def Hist.seen(es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Hist.seen.k(Step.base(e), e, Hist.seen(t))

def Hist.changes.go(es: List<&2, M.Entry>, +n: Nat) -> Nat:
  match es:
    case Nil{}:
      n
    case +e <> t:
      Hist.changes.go(t, Bool.pick(Nat, Step.base(e), n, 1n+n))

# how many design changes a thread's steps (M.State.steps) are
def Hist.changes(es: List<&2, M.Entry>) -> Nat:
  Hist.changes.go(es, 0n)

# the same for a viewer's track (Hist.track: the steps of the kind on
# show), so the history bar's "Live · N changes" agrees with the top bar
# and the timeline: every step but the design as it was before, told by
# the thread's step entries (M.State.steps) (test/hist_test.bend)
def Hist.base.ids.k(base: Bool, +id: String, rest: List<&2, String>) -> List<&2, String>:
  match base:
    case True{}:
      id <> rest
    case False{}:
      rest

def Hist.base.ids(es: List<&2, M.Entry>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Hist.base.ids.k(Step.base(e), M.Idx.id(e), Hist.base.ids(t))

def Hist.has.id(xs: List<&2, String>, +id: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, id), Hist.has.id(t, id))

def Hist.track.changes.go(tr: List<&2, Step>, +bs: List<&2, String>, +n: Nat) -> Nat:
  match tr:
    case Nil{}:
      n
    case +s <> t:
      Hist.track.changes.go(t, bs, Bool.pick(Nat, Hist.has.id(bs, Step.id(s)), n, 1n+n))

def Hist.track.changes(tr: List<&2, Step>, es: List<&2, M.Entry>) -> Nat:
  Hist.track.changes.go(tr, Hist.base.ids(es), 0n)

# Comparing by layer and by sheet (docs/design-review.md)
# --------------------------------------------------------
# A comparison may name the layers it is about: a fourth line, their bits
# (layers.bend's Lyr.kept for the active layer), "diff\n<old>\n<new>\n<bits>".
# Only items on those layers are marked; the rest of the new version is
# drawn dimmed, as unchanged. No fourth line (or 0) marks every layer.

def Hist.diff.on(+a: String, +b: String, +bits: U32) -> String:
  Bool.pick(String, U32.is_eq(bits, 0), Hist.diff(a, b), Hist.diff(a, b) ++ "\n" ++ U32.show(bits))

# a comparison's layers (0 all)
def Hist.bits(+p: String) -> U32:
  Maybe.default(&2, U32, U32.read(Hist.at(String.split(p, '\n'), 3n)), 0)

def Mark.in(+bits: U32, +l: U32) -> Bool:
  Bool.not(U32.is_eq(U32.and(bits, U32.shln(1, U32.to_nat(U32.min(l, 31)))), 0))

def Mark.keep.k(hit: Bool, x: B.Item, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match hit:
    case True{}:
      x <> rest
    case False{}:
      rest

# the items on (want True) or off (False) the layers in bits, in order
def Mark.only(xs: List<&2, B.Item>, +bits: U32, +want: Bool, acc: List<&2, B.Item>) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      List.reverse(&2, B.Item, acc)
    case B.Item{+k, +s, +l, p, i} <> t:
      Mark.only(t, bits, want, Mark.keep.k(Bool.not(Bool.xor(Mark.in(bits, l), want)), B.Item{k, s, l, p, i}, acc))

def Hist.mark.k(all: Bool, +bits: U32, +olds: List<&2, B.Item>, +news: List<&2, B.Item>) -> List<&2, B.Item>:
  match all:
    case True{}:
      Hist.mark(olds, news)
    case False{}:
      List.append(&2, B.Item, Hist.mark(Mark.only(olds, bits, True{}, Nil{}), Mark.only(news, bits, True{}, Nil{})),
        Mark.dims(Mark.only(news, bits, False{}, Nil{})))

# the comparison of two versions on some layers (0 every layer)
def Hist.mark.on(+bits: U32, +olds: List<&2, B.Item>, +news: List<&2, B.Item>) -> List<&2, B.Item>:
  Hist.mark.k(U32.is_eq(bits, 0), bits, olds, news)

# another sheet of the same versions: a file in a commit stays in its
# commit, a comparison compares that sheet in both of its versions (an
# old side that is "" (the file new) stays ""), the live file is the sheet
def Hist.side.to(+p: String, +abs: String) -> String:
  Bool.pick(String, String.is_empty(p), "", Hist.rebase(p, abs))

def Hist.sheet.k(k: Nat, +p: String, +abs: String) -> String:
  match k:
    case 2n:
      Hist.diff.on(Hist.side.to(Hist.old(p), abs), Hist.side.to(Hist.new(p), abs), Hist.bits(p))
    case _:
      Hist.rebase(p, abs)

def Hist.sheet(+p: String, +file: String) -> String:
  Hist.sheet.k(Hist.kind(p), p, Hist.abs(file))
