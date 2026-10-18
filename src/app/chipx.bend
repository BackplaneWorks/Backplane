import Base
import ../core/chip.bend as CH
import ../core/eq.bend as Q
import ../core/model.bend as M
import ./layout.bend as L

# What a memo of chips must be true to (laws "links (window chips)"):
# equality of what two states make of an entry's refs, exact like core's
# Eq.* (laws eq_*_sound), and the premise "every card an entry shows that
# a change may have altered is among the touched ids".

def Eq.cp(+a: L.Cp, +b: L.Cp) -> Bool:
  L.Cp.eq(a, b)

def Eq.cps(a: List<&2, L.Cp>, b: List<&2, L.Cp>) -> Bool:
  L.Cp.eqs(a, b)

def Eq.rz(a: List<&2, List<&2, L.Cp>>, b: List<&2, List<&2, L.Cp>>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case +x <> +xt +y <> +yt:
      Bool.and(Eq.cps(x, y), Eq.rz(xt, yt))
    case _ _:
      False{}

# the two states make the same of an entry's refs
def Same.entry(+st0: M.State, +st: M.State, +e: M.Entry) -> Bool:
  Eq.rz(L.Rz.entry(st0, e), L.Rz.entry(st, e))

# an entry's chips are right under st when what made them under st0 is
# among the touched ids, or they come out the same
def Cov.entry(+st0: M.State, +st: M.State, +dirty: List<&2, String>, +e: M.Entry) -> Bool:
  Bool.or(L.TL.dirty.hit(dirty, L.Rz.deps(L.Rz.entry(st0, e))), Same.entry(st0, st, e))

# the same of a list of entries (those with no chips need nothing)
def Cov.all(+st0: M.State, +st: M.State, +dirty: List<&2, String>, +sel: String, es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}:
      True{}
    case +e <> +t:
      Bool.and(Bool.or(Bool.not(L.TL.cm.chip(e, sel)), Cov.entry(st0, st, dirty, e)), Cov.all(st0, st, dirty, sel, t))

# no entry of the list has chips (a thread without refs)
def Plain.all(+sel: String, es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}:
      True{}
    case +e <> +t:
      Bool.and(Bool.not(L.TL.cm.chip(e, sel)), Plain.all(sel, t))

# The timeline stamp stays first; following ids are dependencies gathered
# only when an entry memo changes. A normal frame reads the saved ids.
def Cache.deps(marks: List<&2, String>) -> List<&2, String>:
  match marks:
    case Nil{}: Nil{}
    case _ <> ids: ids

def Cache.hit.k(all: Bool, +dirty: List<&2, String>, deps: List<&2, String>) -> Bool:
  match all:
    case True{}: Bool.not(List.is_empty(&2, String, dirty))
    case False{}: L.TL.dirty.hit(dirty, deps)

def Cache.hit(dirty: List<&2, String>, +deps: List<&2, String>) -> Bool:
  match dirty:
    case Nil{}: False{}
    case h <> t: Cache.hit.k(L.TL.dirty.in(deps, "*"), h <> t, deps)

def Cache.same(+key: String, marks: List<&2, String>, +dirty: List<&2, String>) -> Bool:
  match marks:
    case Nil{}: False{}
    case +old <> +ids:
      Bool.and(Bool.and(Bool.not(String.is_empty(key)), String.eq(key, old)), Bool.not(Cache.hit(dirty, ids)))

# The cap counts distinct objects, not occurrences of their links. A
# repeated target must not turn a precise summary into a global one.
def Cache.has.go(xs: List<&2, String>, +id: String, found: Bool) -> Bool:
  match xs found:
    case _ True{}: True{}
    case Nil{} False{}: False{}
    case +h <> t False{}:
      Cache.has.go(t, id, Bool.or(String.eq(h, id), String.eq(h, "*")))

def Cache.add.full(full: Bool, id: String, acc: List<&2, String>) -> List<&2, String>:
  match full:
    case True{}: ["*"]
    case False{}: id <> acc

def Cache.add.k(found: Bool, id: String, +acc: List<&2, String>) -> List<&2, String>:
  match found:
    case True{}: acc
    case False{}: Cache.add.full(Nat.is_ge(List.length(&2, String, acc), 64n), id, acc)

def Cache.add(+id: String, +acc: List<&2, String>) -> List<&2, String>:
  Cache.add.k(Cache.has.go(acc, id, False{}), id, acc)

def Cache.ids(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: acc
    case h <> t: Cache.ids(t, Cache.add(h, acc))

type DepSet is Data:
  DepSet{seen: Map<&2, Bool>, count: Nat, full: Bool}

def Cache.set.put(full: Bool, +id: String, seen: Map<&2, Bool>, count: Nat) -> DepSet:
  match full:
    case True{}: DepSet{seen, count, True{}}
    case False{}: DepSet{Map.set(&2, Bool, seen, id, True{}), 1n+count, False{}}

def Cache.set.add.k(found: Bool, +id: String, seen: Map<&2, Bool>, +count: Nat) -> DepSet:
  match found:
    case True{}: DepSet{seen, count, False{}}
    case False{}: Cache.set.put(Bool.or(String.eq(id, "*"), Nat.is_ge(count, 64n)), id, seen, count)

def Cache.set.has(r: Map<&2, Bool> & Bool, id: String, count: Nat) -> DepSet:
  (seen, found) = r
  Cache.set.add.k(found, id, seen, count)

def Cache.set.add(+id: String, s: DepSet) -> DepSet:
  match s:
    case DepSet{seen, count, True{}}: DepSet{seen, count, True{}}
    case DepSet{seen, count, False{}}:
      Cache.set.has(Map.has(&2, Bool, seen, id), id, count)

def Cache.set.ids(xs: List<&2, String>, s: DepSet) -> DepSet:
  match xs:
    case Nil{}: s
    case h <> t: Cache.set.ids(t, Cache.set.add(h, s))

def Cache.set.items(xs: List<&2, L.TLE>, s: DepSet) -> DepSet:
  match xs:
    case Nil{}: s
    case L.TLE{_, p} <> t:
      Cache.set.items(t, Cache.set.ids(L.TL.part.deps(p), s))

def Cache.set.out(s: DepSet) -> List<&2, String>:
  match s:
    case DepSet{_, _, True{}}: ["*"]
    case DepSet{seen, _, False{}}: Map.keys(&2, Bool, seen)

def Cache.items(xs: List<&2, L.TLE>, acc: List<&2, String>) -> List<&2, String>:
  Cache.set.out(Cache.set.items(xs, Cache.set.ids(acc, DepSet{Map.new(&2, Bool), 0n, False{}})))

def Cache.memo(m: L.TLMemo) -> List<&2, String>:
  match m:
    case L.TLMemo{_, _, xs, _}: Cache.items(xs, Nil{})

def Cache.marks.k(same: Bool, key: String, ms: List<&2, L.TLMemo>, old: List<&2, String>) -> List<&2, String>:
  match same:
    case True{}: old
    case False{}: key <> Cache.memo(L.TL.memos.now(ms))

def Cache.marks(visible: Bool, same: Bool, key: String, ms: List<&2, L.TLMemo>, old: List<&2, String>) -> List<&2, String>:
  match visible:
    case True{}: Cache.marks.k(same, key, ms, old)
    case False{}: Nil{}
