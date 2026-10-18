import Base

# East Asian cell widths match Vt.wide. Keep these ranges in step with
# the terminal, including supplementary ideographs and wide emoji.
def Cjk.in(+cp: U32, +a: U32, +b: U32) -> Bool:
  U32.is_ge(cp, a) && U32.is_le(cp, b)

def Cjk.wide(+cp: U32) -> Bool:
  Cjk.in(cp, 4352, 4447) || Cjk.in(cp, 11904, 12350) || Cjk.in(cp, 12353, 13311) || Cjk.in(cp, 13312, 19903) || Cjk.in(cp, 19968, 40959) || Cjk.in(cp, 40960, 42191) || Cjk.in(cp, 44032, 55203) || Cjk.in(cp, 63744, 64255) || Cjk.in(cp, 65072, 65103) || Cjk.in(cp, 65280, 65376) || Cjk.in(cp, 65504, 65510) || Cjk.in(cp, 127744, 128591) || Cjk.in(cp, 128640, 128767) || Cjk.in(cp, 129280, 129535) || Cjk.in(cp, 131072, 262141)

def Cjk.no_start(+cp: U32) -> Bool:
  String.contains("、。，．・：；？！ー～）」』】〕〉》｝］｠〙〗’”ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ々〻‐゠–〜)]},.:;!?", SCon{Chr{cp}, ""})

def Cjk.no_end(+cp: U32) -> Bool:
  String.contains("（「『【〔〈《｛［｟〘〖‘“([{", SCon{Chr{cp}, ""})

def Cjk.can_break.k(wide: Bool, +a: U32, +b: U32) -> Bool:
  match wide:
    case False{}:
      False{}
    case True{}:
      Bool.not(Cjk.no_end(a)) && Bool.not(Cjk.no_start(b))

def Cjk.can_break(+a: U32, +b: U32) -> Bool:
  Cjk.can_break.k(Cjk.wide(a) || Cjk.wide(b), a, b)

# Test once per paragraph; ordinary Latin text keeps the old word path.
def Cjk.has.go(s: String, hit: Bool) -> Bool:
  match s hit:
    case _ True{}:
      True{}
    case SNil{} False{}:
      False{}
    case SCon{Chr{cp}, t} False{}:
      Cjk.has.go(t, U32.is_ge(cp, 11904))

def Cjk.has(s: String) -> Bool:
  Cjk.has.go(s, False{})

# Each gap is exactly one original space. Empty segments preserve a
# leading, repeated or trailing space. Characters in rev are newest first.
type Seg is Data:
  Seg{text: String, gap: Bool}

type SegAcc is Data:
  SegAcc{done: List<&2, Seg>, rev: String, gap: Bool, prev: U32, have: Bool}

def Cjk.rev(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      acc
    case SCon{c, t}:
      Cjk.rev(t, SCon{c, acc})

def Cjk.step.k(space: Bool, cut: Bool, +cp: U32, a: SegAcc) -> SegAcc:
  match space cut a:
    case True{} _ SegAcc{ls, rev, gap, _, _}:
      SegAcc{Seg{Cjk.rev(rev, ""), gap} <> ls, "", True{}, 0, False{}}
    case False{} True{} SegAcc{ls, rev, gap, _, _}:
      SegAcc{Seg{Cjk.rev(rev, ""), gap} <> ls, SCon{Chr{cp}, ""}, False{}, cp, True{}}
    case False{} False{} SegAcc{ls, rev, gap, _, _}:
      SegAcc{ls, SCon{Chr{cp}, rev}, gap, cp, True{}}

def Cjk.step(+cp: U32, a: SegAcc) -> SegAcc:
  match a:
    case SegAcc{ls, rev, gap, +prev, +have}:
      Cjk.step.k(U32.is_eq(cp, 32), Bool.and(have, Cjk.can_break(prev, cp)), cp, SegAcc{ls, rev, gap, prev, have})

def Cjk.segs.go(s: String, a: SegAcc) -> List<&2, Seg>:
  match s a:
    case SNil{} SegAcc{ls, rev, gap, _, _}:
      List.reverse(&2, Seg, Seg{Cjk.rev(rev, ""), gap} <> ls)
    case SCon{Chr{cp}, t} a:
      Cjk.segs.go(t, Cjk.step(cp, a))

def Cjk.segs(s: String) -> List<&2, Seg>:
  Cjk.segs.go(s, SegAcc{Nil{}, "", False{}, 0, False{}})

# Reconstruction also uses reversed accumulators, with no recursive
# result appended outside the tail call.
def Cjk.join.go(ss: List<&2, Seg>, +acc: String) -> String:
  match ss:
    case Nil{}:
      Cjk.rev(acc, "")
    case Seg{text, gap} <> t:
      Cjk.join.go(t, Cjk.rev(text, Bool.pick(String, gap, SCon{' ', acc}, acc)))

def Cjk.join(ss: List<&2, Seg>) -> String:
  Cjk.join.go(ss, "")
