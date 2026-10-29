import Base

# Retained display rows are local, separate from the 32-row wire page.
# A full picture beyond this budget parks until a viewport refresh can retire
# offscreen context; the host never clips the visible span to fit.
def History.within(rows: Nat, bytes: Nat) -> Bool:
  Bool.and(Nat.is_le(rows, 260n), Nat.is_le(bytes, 1048576n))

type Alias is Data:
  Alias{from: String, to: String}

def History.has(xs: List<&2, String>, +id: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +x <> rest: Bool.or(String.eq(x, id), History.has(rest, id))

# Extending a clipped tool run changes its first call, hence its fold id.
# The same immediately following shared entry identifies that run. Keep its
# displayed key while taking the new header's current action and metadata.
def History.previous.k(same: Bool, previous: String, after: String) -> String:
  match same:
    case True{}: previous
    case False{}: after

def History.previous(xs: List<&2, String>, +id: String, previous: String) -> String:
  match xs:
    case Nil{}: ""
    case +x <> rest: History.previous.k(String.eq(x, id), previous, History.previous(rest, id, x))

def History.aliases.k(eligible: Bool, previous: String, was: String, rest: List<&2, Alias>) -> List<&2, Alias>:
  match eligible:
    case True{}: Alias{previous, was} <> rest
    case False{}: rest

def History.aliases.go(xs: List<&2, String>, +old: List<&2, String>, +fresh: List<&2, String>, +previous: String) -> List<&2, Alias>:
  match xs:
    case Nil{}: Nil{}
    case +id <> rest:
      +was = History.previous(old, id, "")
      +eligible = Bool.and(History.has(old, id), Bool.and(String.starts_with(previous, "fold:"),
        Bool.and(String.starts_with(was, "fold:"), Bool.and(Bool.not(History.has(old, previous)), Bool.not(History.has(fresh, was))))))
      History.aliases.k(eligible, previous, was, History.aliases.go(rest, old, fresh, id))

def History.aliases(+old: List<&2, String>, +fresh: List<&2, String>) -> List<&2, Alias>:
  History.aliases.go(fresh, old, fresh, "")

def History.alias.k(same: Bool, to: String, after: String) -> String:
  match same:
    case True{}: to
    case False{}: after

def History.alias(+id: String, xs: List<&2, Alias>) -> String:
  match xs:
    case Nil{}: id
    case Alias{from, to} <> rest: History.alias.k(String.eq(id, from), to, History.alias(id, rest))

def History.renamed(xs: List<&2, String>, +aliases: List<&2, Alias>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case id <> rest: History.alias(id, aliases) <> History.renamed(rest, aliases)

# A retained fold can already have a stable displayed key and a newer
# canonical action target. Reuse that proved identity when the next page
# ends inside the run and supplies no following shared row. Never infer a
# new relationship merely because two fold headers are adjacent.
def History.aliases.count(xs: List<&2, Alias>, +from: String, +to: String) -> Nat:
  match xs:
    case Nil{}: 0n
    case Alias{f, t} <> rest:
      Nat.add(Bool.pick(Nat, Bool.or(String.eq(f, from), String.eq(t, to)), 1n, 0n), History.aliases.count(rest, from, to))

def History.aliases.kept(xs: List<&2, Alias>, +all: List<&2, Alias>, +old: List<&2, String>, +fresh: List<&2, String>) -> List<&2, Alias>:
  match xs:
    case Nil{}: Nil{}
    case Alias{+from, +to} <> rest:
      +eligible = Bool.and(String.starts_with(from, "fold:"), Bool.and(String.starts_with(to, "fold:"),
        Bool.and(History.has(old, to), Bool.and(History.has(fresh, from),
        Bool.and(Bool.not(History.has(old, from)), Bool.and(Bool.not(History.has(fresh, to)),
          Nat.is_eq(History.aliases.count(all, from, to), 1n)))))))
      History.aliases.k(eligible, from, to, History.aliases.kept(rest, all, old, fresh))

def History.aliases.using(+old: List<&2, String>, +fresh: List<&2, String>, +known: List<&2, Alias>) -> List<&2, Alias>:
  +held = History.aliases.kept(known, known, old, fresh)
  List.append(&2, Alias, held, History.aliases(old, History.renamed(fresh, held)))

# A bounded wire page is not the whole visible timeline. The presentation
# host retains rows from the outgoing page until the reader leaves them.
# Only identities are planned here; canonical entries and cursors stay put.

def History.filter.k(kept: Bool, id: String, rest: List<&2, String>) -> List<&2, String>:
  match kept:
    case True{}: id <> rest
    case False{}: rest

def History.filter(xs: List<&2, String>, +keep: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> rest: History.filter.k(History.has(keep, x), x, History.filter(rest, keep))

type Cut is Data:
  Cut{before: List<&2, String>, after: List<&2, String>}

def History.cut.k(same: Bool, rest: List<&2, String>, acc: List<&2, String>, after: Cut) -> Cut:
  match same:
    case True{}: Cut{List.reverse(&2, String, acc), rest}
    case False{}: after

def History.cut.go(xs: List<&2, String>, +id: String, +acc: List<&2, String>) -> Cut:
  match xs:
    case Nil{}: Cut{List.reverse(&2, String, acc), Nil{}}
    case +x <> +rest: History.cut.k(String.eq(x, id), rest, acc, History.cut.go(rest, id, x <> acc))

def Cut.before(cut: Cut) -> List<&2, String>:
  match cut:
    case Cut{before, _}: before

def Cut.after(cut: Cut) -> List<&2, String>:
  match cut:
    case Cut{_, after}: after

def History.merge(fresh: List<&2, String>, +old: List<&2, String>, +keep: List<&2, String>) -> List<&2, String>:
  match fresh:
    case Nil{}: History.filter(old, keep)
    case +id <> rest:
      +cut = History.cut.go(old, id, Nil{})
      +shared = History.has(old, id)
      +prefix = Bool.pick(List<&2, String>, shared, History.filter(Cut.before(cut), keep), Nil{})
      List.append(&2, String, prefix, id <> History.merge(rest, Bool.pick(List<&2, String>, shared, Cut.after(cut), old), keep))

def History.shared(old: List<&2, String>, +fresh: List<&2, String>) -> Bool:
  match old:
    case Nil{}: False{}
    case +id <> rest: Bool.or(History.has(fresh, id), History.shared(rest, fresh))

def History.project.k(shared: Bool, +old: List<&2, String>, +fresh: List<&2, String>, +keep: List<&2, String>, older: Bool) -> List<&2, String>:
  match shared:
    case True{}: History.merge(fresh, old, keep)
    case False{}:
      Bool.pick(List<&2, String>, older, List.append(&2, String, fresh, History.filter(old, keep)), List.append(&2, String, History.filter(old, keep), fresh))

def History.project.planned(+old: List<&2, String>, +fresh: List<&2, String>, keep: List<&2, String>, older: Bool) -> List<&2, String>:
  History.project.k(History.shared(old, fresh), old, fresh, keep, older)

def History.project(+old: List<&2, String>, +fresh: List<&2, String>, keep: List<&2, String>, older: Bool) -> List<&2, String>:
  +named = History.renamed(fresh, History.aliases(old, fresh))
  History.project.planned(old, named, keep, older)

# The visible source span must remain an ordered subsequence, including
# split link rows and fold headers. A rejected plan leaves the picture alone.
def History.ordered.go(rows: List<&2, String>, visible: List<&2, String>) -> Bool:
  match rows visible:
    case _ Nil{}: True{}
    case Nil{} _: False{}
    case x <> rest +id <> +wanted:
      History.ordered.go(rest, Bool.pick(List<&2, String>, String.eq(id, x), wanted, id <> wanted))

def History.ordered(visible: List<&2, String>, rows: List<&2, String>) -> Bool:
  History.ordered.go(rows, visible)

def History.before(+rows: List<&2, String>, +a: String, +b: String) -> Bool:
  Bool.or(String.eq(a, b), History.ordered([a, b], rows))

def History.viewport(+rows: List<&2, String>, +first: String, +last: String, +anchor: String) -> Bool:
  Bool.and(History.has(rows, first), Bool.and(History.has(rows, last), Bool.and(History.has(rows, anchor),
    Bool.and(History.before(rows, first, anchor), History.before(rows, anchor, last)))))

def History.valid(+old: List<&2, String>, +candidate: List<&2, String>, visible: List<&2, String>) -> Bool:
  Bool.and(History.ordered(visible, candidate), Nat.is_ge(List.length(&2, String, candidate), List.length(&2, String, old)))

def History.choose(ok: Bool, old: List<&2, String>, candidate: List<&2, String>) -> List<&2, String>:
  match ok:
    case True{}: candidate
    case False{}: old

def History.adopt(+old: List<&2, String>, +candidate: List<&2, String>, visible: List<&2, String>) -> List<&2, String>:
  History.choose(History.valid(old, candidate, visible), old, candidate)

# Page/refresh consume the exact identities already normalized by the
# caller's returned alias plan. A second alias pass could discover another
# relationship after the first rename that the host was never told about.
# Inputs must already be alias-normalized, as the mobile adapter supplies.
def History.refresh(+old: List<&2, String>, +fresh: List<&2, String>, keep: List<&2, String>, +visible: List<&2, String>, older: Bool) -> List<&2, String>:
  +candidate = History.project.planned(old, fresh, keep, older)
  History.choose(History.ordered(visible, candidate), old, candidate)

def History.page.k(ok: Bool, +old: List<&2, String>, +fresh: List<&2, String>, candidate: List<&2, String>, +visible: List<&2, String>, older: Bool) -> List<&2, String>:
  match ok:
    case True{}: candidate
    case False{}: History.adopt(old, History.project.planned(old, fresh, old, older), visible)

def History.page(+old: List<&2, String>, +fresh: List<&2, String>, keep: List<&2, String>, +visible: List<&2, String>, +older: Bool) -> List<&2, String>:
  +candidate = History.project.planned(old, fresh, keep, older)
  History.page.k(History.valid(old, candidate, visible), old, fresh, candidate, visible, older)

