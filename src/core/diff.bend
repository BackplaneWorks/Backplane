import Base

# Unified diffs (`git diff` output) as files, hunks and numbered lines. One
# walk over the lines; every recursion here is a tail call, so a big diff
# costs time in proportion to its size and no stack.
#
# A DFile names its paths without the a/ b/ prefixes; both `old` and `new`
# hold a path even for an added or deleted file (the status says which).
# `status` is "modified", "added", "deleted", "renamed" or "copied". A binary
# file has one hunk with an empty header holding git's "Binary files ..."
# line as a meta line (Diff.binary).

type DLine is Data:
  DLine{kind: U32, text: String, old_no: Nat, new_no: Nat}

type DHunk is Data:
  DHunk{header: String, lines: List<&2, DLine>}

type DFile is Data:
  DFile{old: String, new: String, status: String, hunks: List<&2, DHunk>}

type DStats is Data:
  DStats{files: Nat, adds: Nat, dels: Nat}

# Lines
# -----

type LS is Data:
  LS{cur: String, acc: List<&2, String>}

def Diff.lines.put(nl: Bool, c: Char, cur: String, acc: List<&2, String>) -> LS:
  match nl:
    case True{}:
      LS{SNil{}, String.reverse(cur) <> acc}
    case False{}:
      LS{SCon{c, cur}, acc}

def Diff.lines.step(+c: Char, st: LS) -> LS:
  match st:
    case LS{cur, acc}:
      Diff.lines.put(Char.is_eq(c, '\n'), c, cur, acc)

def Diff.lines.go(s: String, st: LS) -> LS:
  match s:
    case SNil{}:
      st
    case SCon{c, t}:
      Diff.lines.go(t, Diff.lines.step(c, st))

def Diff.lines.fin(st: LS) -> List<&2, String>:
  match st:
    case LS{cur, acc}:
      List.reverse(&2, String, String.reverse(cur) <> acc)

# the lines of a text, split on "\n" (tail-recursive)
def Diff.lines(s: String) -> List<&2, String>:
  Diff.lines.fin(Diff.lines.go(s, LS{SNil{}, Nil{}}))

def Diff.first.add(nl: Bool, c: Char, acc: String) -> String:
  match nl:
    case True{}:
      acc
    case False{}:
      SCon{c, acc}

def Diff.first.go(s: String, acc: String, done: Bool) -> String:
  match s done:
    case _ True{}:
      String.reverse(acc)
    case SNil{} False{}:
      String.reverse(acc)
    case SCon{+c, t} False{}:
      Diff.first.go(t, Diff.first.add(Char.is_eq(c, '\n'), c, acc), Char.is_eq(c, '\n'))

# a text's first line, in one tail-recursive walk that stops at the first
# line break (String.lines recurses once per character, outside a tail
# call: a fork's handed-over context, thousands of characters, overflowed
# the browser's stack and the web stopped drawing the fork)
def Diff.first(s: String) -> String:
  Diff.first.go(s, SNil{}, False{})

# Line tags
# ---------

type LT is Data:
  TCtx{}
  TAdd{}
  TDel{}
  TMeta{}
  TDiff{}
  THunk{}
  TOld{}
  TNew{}
  TAdded{}
  TDeleted{}
  TRenFrom{}
  TRenTo{}
  TCopyFrom{}
  TCopyTo{}
  TBin{}
  TSkip{}

type Tag is Data:
  Tag{pre: String, tag: LT}

# lines inside a hunk that still expects lines
def Diff.tags.body() -> List<&2, Tag>:
  [Tag{" ", TCtx{}}, Tag{"+", TAdd{}}, Tag{"-", TDel{}}, Tag{"\\", TMeta{}}]

# everything else
def Diff.tags.head() -> List<&2, Tag>:
  [Tag{"diff --git ", TDiff{}}, Tag{"@@ ", THunk{}}, Tag{"\\", TMeta{}},
   Tag{"--- ", TOld{}}, Tag{"+++ ", TNew{}},
   Tag{"new file mode", TAdded{}}, Tag{"deleted file mode", TDeleted{}},
   Tag{"rename from ", TRenFrom{}}, Tag{"rename to ", TRenTo{}},
   Tag{"copy from ", TCopyFrom{}}, Tag{"copy to ", TCopyTo{}},
   Tag{"Binary files ", TBin{}}, Tag{"GIT binary patch", TBin{}}]

def Diff.tag.hit(ts: List<&2, Tag>, +l: String) -> Bool:
  match ts:
    case Tag{p, _} <> _:
      String.starts_with(l, p)
    case Nil{}:
      False{}

def Diff.tag.go(ts: List<&2, Tag>, h: Bool, +l: String) -> LT:
  match ts h:
    case Tag{_, t} <> _ True{}:
      t
    case _ <> +rest False{}:
      Diff.tag.go(rest, Diff.tag.hit(rest, l), l)
    case Nil{} _:
      TSkip{}

def Diff.tag(+ts: List<&2, Tag>, +l: String) -> LT:
  Diff.tag.go(ts, Diff.tag.hit(ts, l), l)

def Diff.is_skip(t: LT) -> Bool:
  match t:
    case TSkip{}:
      True{}
    case _:
      False{}

# an unrecognised line in a hunk body: blank is context (some tools strip the
# space), anything else is read as a header line
def Diff.kind.body(miss: Bool, +t: LT, +l: String) -> LT:
  match miss:
    case False{}:
      t
    case True{}:
      Bool.pick(LT, String.is_empty(l), TCtx{}, Diff.tag(Diff.tags.head(), l))

def Diff.kind.room(+t: LT, +l: String) -> LT:
  Diff.kind.body(Diff.is_skip(t), t, l)

def Diff.kind(room: Bool, +l: String) -> LT:
  match room:
    case True{}:
      Diff.kind.room(Diff.tag(Diff.tags.body(), l), l)
    case False{}:
      Diff.tag(Diff.tags.head(), l)

# Paths
# -----

def Diff.untab(+s: String) -> String:
  Bool.pick(String, String.ends_with(s, "\t"), String.reverse(String.drop(String.reverse(s), 1n)), s)

def Diff.is_prefixed(s: String) -> Bool:
  match s:
    case SCon{_, SCon{c, _}}:
      Char.is_eq(c, '/')
    case _:
      False{}

# "a/x" -> "x"
def Diff.unprefix(+s: String) -> String:
  Bool.pick(String, Diff.is_prefixed(s), String.drop(s, 2n), s)

def Diff.path(+s: String) -> String:
  Diff.unprefix(Diff.untab(s))

def Diff.is_null(+s: String) -> Bool:
  String.eq(Diff.untab(s), "/dev/null")

type Two is Data:
  Two{a: String, b: String}

def Diff.halves.k(same: Bool, +x: String, +rest: String) -> Two:
  match same:
    case True{}:
      Two{x, x}
    case False{}:
      Two{rest, rest}

def Diff.halves(+rest: String, +h: Nat) -> Two:
  Diff.halves.k(String.eq(String.take(String.drop(rest, 2n), h), String.drop(rest, Nat.add(h, 5n))), String.take(String.drop(rest, 2n), h), rest)

# "a/P b/P" -> P, P (renames get their names from the rename lines)
def Diff.git_names(+rest: String) -> Two:
  Diff.halves(rest, Nat.div(Nat.sub(String.length(rest), 5n), 2n))

# Hunk headers
# ------------

type Rg is Data:
  Rg{start: Nat, count: Nat}

def Diff.num(s: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(s), 0n)

def Diff.rg.parts(ps: List<&2, String>) -> Rg:
  match ps:
    case a <> b <> _:
      Rg{Diff.num(a), Diff.num(b)}
    case a <> Nil{}:
      Rg{Diff.num(a), 1n}
    case Nil{}:
      Rg{0n, 0n}

# "-12,3" -> 12, 3; "+4" -> 4, 1
def Diff.rg(s: String) -> Rg:
  Diff.rg.parts(String.split(String.drop(s, 1n), ','))

def Diff.rg.at(ws: List<&2, String>, n: Nat) -> Rg:
  match ws:
    case Nil{}:
      Rg{0n, 0n}
    case w <> _:
      Diff.rg(w)

# Parser state
# ------------

# the hunk being read: reversed lines, next line numbers, lines still owed
type PH is Data:
  PH{header: String, lines: List<&2, DLine>, o: Nat, n: Nat, ro: Nat, rn: Nat}

# the file being read: reversed hunks
type PF is Data:
  PF{old: String, new: String, status: String, hunks: List<&2, DHunk>}

type St is Data:
  St{files: List<&2, DFile>, file: Maybe<&2, PF>, hunk: Maybe<&2, PH>}

def Diff.ph.close(h: PH) -> DHunk:
  match h:
    case PH{header, lines, _, _, _, _}:
      DHunk{header, List.reverse(&2, DLine, lines)}

def Diff.pf.add(f: PF, h: DHunk) -> PF:
  match f:
    case PF{o, n, s, hs}:
      PF{o, n, s, h <> hs}

# the file with its open hunk (if any) put away
def Diff.pf.flush(f: PF, h: Maybe<&2, PH>) -> PF:
  match h:
    case None{}:
      f
    case Some{ph}:
      Diff.pf.add(f, Diff.ph.close(ph))

def Diff.pf.close(f: PF) -> DFile:
  match f:
    case PF{o, n, s, hs}:
      DFile{o, n, s, List.reverse(&2, DHunk, hs)}

# a hunk opened with no file header: a bare unified diff
def Diff.pf.or_new(f: Maybe<&2, PF>) -> PF:
  match f:
    case None{}:
      PF{"", "", "modified", Nil{}}
    case Some{pf}:
      pf

def Diff.files.flush(fs: List<&2, DFile>, f: Maybe<&2, PF>, h: Maybe<&2, PH>) -> List<&2, DFile>:
  match f:
    case None{}:
      fs
    case Some{pf}:
      Diff.pf.close(Diff.pf.flush(pf, h)) <> fs

def Diff.room.ph(h: Maybe<&2, PH>) -> Bool:
  match h:
    case None{}:
      False{}
    case Some{PH{_, _, _, _, ro, rn}}:
      Nat.is_gt(Nat.add(ro, rn), 0n)

def Diff.room(st: St) -> Bool:
  match st:
    case St{_, _, h}:
      Diff.room.ph(h)

def Diff.pf.named(t: Two) -> PF:
  match t:
    case Two{a, b}:
      PF{a, b, "modified", Nil{}}

# Steps
# -----

def Diff.on_diff(l: String, st: St) -> St:
  match st:
    case St{fs, f, h}:
      St{Diff.files.flush(fs, f, h), Some{Diff.pf.named(Diff.git_names(String.drop(l, 11n)))}, None{}}

def Diff.ph.open.rg(l: String, a: Rg, b: Rg) -> PH:
  match a b:
    case Rg{o, ro} Rg{n, rn}:
      PH{l, Nil{}, o, n, ro, rn}

def Diff.ph.open.k(l: String, +ws: List<&2, String>) -> PH:
  Diff.ph.open.rg(l, Diff.rg.at(List.drop(&2, String, ws, 1n), 0n), Diff.rg.at(List.drop(&2, String, ws, 2n), 0n))

def Diff.ph.open(+l: String) -> PH:
  Diff.ph.open.k(l, String.split(l, ' '))

def Diff.on_hunk(+l: String, st: St) -> St:
  match st:
    case St{fs, f, h}:
      St{fs, Some{Diff.pf.flush(Diff.pf.or_new(f), h)}, Some{Diff.ph.open(l)}}

# change a file's fields
type Edit is Data:
  EOld{v: String}
  ENew{v: String}
  EStatus{v: String}
  ERename{v: String}
  ECopy{v: String}

def Diff.pf.edit(e: Edit, f: PF) -> PF:
  match e f:
    case EOld{v} PF{_, n, s, hs}:
      PF{v, n, s, hs}
    case ENew{v} PF{o, _, s, hs}:
      PF{o, v, s, hs}
    case EStatus{v} PF{o, n, _, hs}:
      PF{o, n, v, hs}
    case ERename{v} PF{_, n, _, hs}:
      PF{v, n, "renamed", hs}
    case ECopy{v} PF{_, n, _, hs}:
      PF{v, n, "copied", hs}

def Diff.on_edit(e: Edit, st: St) -> St:
  match st:
    case St{fs, f, h}:
      St{fs, Some{Diff.pf.edit(e, Diff.pf.or_new(f))}, h}

def Diff.on_edit.if(ok: Bool, e: Edit, st: St) -> St:
  match ok:
    case True{}:
      Diff.on_edit(e, st)
    case False{}:
      st

# "--- a/x" / "+++ b/x": a path unless /dev/null
def Diff.on_old(+l: String, st: St) -> St:
  Diff.on_edit.if(Bool.not(Diff.is_null(String.drop(l, 4n))), EOld{Diff.path(String.drop(l, 4n))}, st)

def Diff.on_new(+l: String, st: St) -> St:
  Diff.on_edit.if(Bool.not(Diff.is_null(String.drop(l, 4n))), ENew{Diff.path(String.drop(l, 4n))}, st)

def Diff.on_bin(l: String, st: St) -> St:
  match st:
    case St{fs, f, h}:
      St{fs, Some{Diff.pf.add(Diff.pf.flush(Diff.pf.or_new(f), h), DHunk{"", [DLine{3, l, 0n, 0n}]})}, None{}}

def Diff.ph.push(t: LT, l: String, h: PH) -> PH:
  match t h:
    case TCtx{} PH{hd, ls, +o, +n, ro, rn}:
      PH{hd, DLine{0, String.drop(l, 1n), o, n} <> ls, 1n+o, 1n+n, Nat.sub(ro, 1n), Nat.sub(rn, 1n)}
    case TAdd{} PH{hd, ls, o, +n, ro, rn}:
      PH{hd, DLine{1, String.drop(l, 1n), 0n, n} <> ls, o, 1n+n, ro, Nat.sub(rn, 1n)}
    case TDel{} PH{hd, ls, +o, n, ro, rn}:
      PH{hd, DLine{2, String.drop(l, 1n), o, 0n} <> ls, 1n+o, n, Nat.sub(ro, 1n), rn}
    case _ PH{hd, ls, o, n, ro, rn}:
      PH{hd, DLine{3, l, 0n, 0n} <> ls, o, n, ro, rn}

def Diff.on_line.ph(t: LT, l: String, h: Maybe<&2, PH>) -> Maybe<&2, PH>:
  match h:
    case None{}:
      None{}
    case Some{ph}:
      Some{Diff.ph.push(t, l, ph)}

def Diff.on_line(t: LT, l: String, st: St) -> St:
  match st:
    case St{fs, f, h}:
      St{fs, f, Diff.on_line.ph(t, l, h)}

def Diff.step(t: LT, +l: String, st: St) -> St:
  match t:
    case TDiff{}:
      Diff.on_diff(l, st)
    case THunk{}:
      Diff.on_hunk(l, st)
    case TOld{}:
      Diff.on_old(l, st)
    case TNew{}:
      Diff.on_new(l, st)
    case TAdded{}:
      Diff.on_edit(EStatus{"added"}, st)
    case TDeleted{}:
      Diff.on_edit(EStatus{"deleted"}, st)
    case TRenFrom{}:
      Diff.on_edit(ERename{String.drop(l, 12n)}, st)
    case TRenTo{}:
      Diff.on_edit(ENew{String.drop(l, 10n)}, st)
    case TCopyFrom{}:
      Diff.on_edit(ECopy{String.drop(l, 10n)}, st)
    case TCopyTo{}:
      Diff.on_edit(ENew{String.drop(l, 8n)}, st)
    case TBin{}:
      Diff.on_bin(l, st)
    case TSkip{}:
      st
    case _:
      Diff.on_line(t, l, st)

def Diff.feed(+l: String, +st: St) -> St:
  Diff.step(Diff.kind(Diff.room(st), l), l, st)

def Diff.go(ls: List<&2, String>, st: St) -> St:
  match ls:
    case Nil{}:
      st
    case l <> t:
      Diff.go(t, Diff.feed(l, st))

def Diff.fin(st: St) -> List<&2, DFile>:
  match st:
    case St{fs, f, h}:
      List.reverse(&2, DFile, Diff.files.flush(fs, f, h))

# the files of a unified diff, in order
def Diff.parse(text: String) -> List<&2, DFile>:
  Diff.fin(Diff.go(Diff.lines(text), St{Nil{}, None{}, None{}}))

# Stats
# -----

# A capped diff (server/git.bend's Git.diff.cap) still counts what it left
# out: its meta lines "\ Too big to show here: +N -M lines" and
# "\ N more changed files not shown here"
def Diff.word(+ws: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, ws, i), "")

def Diff.count.more(more: Bool, +ws: List<&2, String>, s: DStats) -> DStats:
  match more s:
    case True{} DStats{f, a, d}:
      DStats{Nat.add(f, Diff.num(Diff.word(ws, 1n))), a, d}
    case False{} _:
      s

def Diff.count.meta(big: Bool, +t: String, s: DStats) -> DStats:
  match big s:
    case True{} DStats{f, a, d}:
      DStats{f, Nat.add(a, Diff.num(String.drop(Diff.word(String.split(t, ' '), 6n), 1n))), Nat.add(d, Diff.num(String.drop(Diff.word(String.split(t, ' '), 7n), 1n)))}
    case False{} _:
      Diff.count.more(String.ends_with(t, " more changed files not shown here"), String.split(t, ' '), s)

def Diff.count.line.k(meta: Bool, +k: U32, +t: String, s: DStats) -> DStats:
  match meta s:
    case True{} _:
      Diff.count.meta(String.starts_with(t, "\\ Too big to show here: +"), t, s)
    case False{} DStats{f, a, d}:
      DStats{f, Nat.add(a, U32.to_nat(Bool.to_u32(U32.is_eq(k, 1)))), Nat.add(d, U32.to_nat(Bool.to_u32(U32.is_eq(k, 2))))}

def Diff.count.line(l: DLine, s: DStats) -> DStats:
  match l:
    case DLine{+k, +t, _, _}:
      Diff.count.line.k(U32.is_eq(k, 3), k, t, s)

def Diff.count.lines(ls: List<&2, DLine>, s: DStats) -> DStats:
  match ls:
    case Nil{}:
      s
    case l <> t:
      Diff.count.lines(t, Diff.count.line(l, s))

def Diff.count.hunk(h: DHunk, s: DStats) -> DStats:
  match h:
    case DHunk{_, ls}:
      Diff.count.lines(ls, s)

def Diff.count.hunks(hs: List<&2, DHunk>, s: DStats) -> DStats:
  match hs:
    case Nil{}:
      s
    case h <> t:
      Diff.count.hunks(t, Diff.count.hunk(h, s))

def Diff.count.file(f: DFile, s: DStats) -> DStats:
  match f s:
    case DFile{_, _, _, hs} DStats{n, a, d}:
      Diff.count.hunks(hs, DStats{1n+n, a, d})

def Diff.count.files(fs: List<&2, DFile>, s: DStats) -> DStats:
  match fs:
    case Nil{}:
      s
    case f <> t:
      Diff.count.files(t, Diff.count.file(f, s))

# files changed, lines added, lines deleted
def Diff.stats(fs: List<&2, DFile>) -> DStats:
  Diff.count.files(fs, DStats{0n, 0n, 0n})

def Diff.plural(+n: Nat) -> String:
  Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " file", " files")

def Diff.summary.of(s: DStats) -> String:
  match s:
    case DStats{f, a, d}:
      Diff.plural(f) ++ ", +" ++ Nat.show(a) ++ " −" ++ Nat.show(d)

# "3 files, +12 −4"
def Diff.summary(fs: List<&2, DFile>) -> String:
  Diff.summary.of(Diff.stats(fs))

def Diff.binary.hunks(hs: List<&2, DHunk>) -> Bool:
  match hs:
    case DHunk{hd, _} <> _:
      String.is_empty(hd)
    case Nil{}:
      False{}

# whether git reported the file as binary (no text hunks)
def Diff.binary(f: DFile) -> Bool:
  match f:
    case DFile{_, _, _, hs}:
      Diff.binary.hunks(hs)
