import Base
import ./search-fold.bend as SF

# Fuzzy matching for pickers: every character of the query appears in the
# candidate in order (any case). A match scores higher for a prefix, for
# characters that follow each other, and for characters that start a word;
# 0 means no match. An empty query matches everything (law
# fuzzy_empty_matches).

def Fuzzy.lower(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

# a word starts after a separator (space - _ . /) or at a lower→upper step
def Fuzzy.sep(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 45)), Bool.or(Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 46)), U32.is_eq(c, 47)))

def Fuzzy.hit(s: String, q: String) -> Bool:
  match s q:
    case SCon{Chr{c}, _} SCon{Chr{d}, _}:
      U32.is_eq(Fuzzy.lower(c), Fuzzy.lower(d))
    case _ _:
      False{}

def Fuzzy.upper(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))

def Fuzzy.bonus(+run: Bool, +first: Bool, +word: Bool) -> Nat:
  Nat.add(Nat.add(Bool.pick(Nat, run, 5n, 0n), Bool.pick(Nat, first, 15n, 0n)), Bool.pick(Nat, word, 8n, 1n))

# walk the candidate: `hit` says whether its head matches the query's
# head; prev is the previous candidate character, run whether it matched
def Fuzzy.go(s: String, q: String, hit: Bool, +prev: U32, +run: Bool, +first: Bool, +acc: Nat) -> Nat:
  match s q hit:
    case _ SNil{} _:
      1n+acc
    case SNil{} SCon{_, _} _:
      0n
    case SCon{Chr{+c}, +t} SCon{+h0, +qt} True{}:
      Fuzzy.go(t, qt, Fuzzy.hit(t, qt), c, True{}, False{},
        Nat.add(acc, Fuzzy.bonus(run, first, Bool.or(first, Bool.or(Fuzzy.sep(prev), Bool.and(Fuzzy.upper(c), Bool.not(Fuzzy.upper(prev))))))))
    case SCon{Chr{+c}, +t} SCon{+h, +qt} False{}:
      Fuzzy.go(t, SCon{h, qt}, Fuzzy.hit(t, SCon{h, qt}), c, False{}, False{}, acc)

def Fuzzy.normalized(+s: String, +q: String) -> Nat:
  Fuzzy.go(s, q, Fuzzy.hit(s, q), 32, False{}, True{}, 0n)

# An empty query keeps every candidate without walking its title.
def Fuzzy.score.k(empty: Bool, s: String, q: String) -> Nat:
  match empty:
    case True{}: 1n
    case False{}: Fuzzy.normalized(SF.Fold.title(s), q)

def Fuzzy.score.query(s: String, +q: String) -> Nat:
  Fuzzy.score.k(String.is_empty(q), s, q)

# how well q matches s (0: not at all)
def Fuzzy.score(s: String, q: String) -> Nat:
  Fuzzy.score.query(s, SF.Fold.text(q))
