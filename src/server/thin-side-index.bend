import Base
import ../core/client.bend as U
import ../core/model.bend as M
import ./thin-signals.bend as K

# One actor-private metadata projection, fenced by the server epoch and
# dependency revision. The transient SidebarInput is never retained. This
# cache has no history, recipient info, clock, UI/session or rendered nodes.
# The cap bounds retained map/list entries (including grouped asks), not
# bytes of canonical strings: immutable Thread/Ask strings are shared.
type IndexCache is Data:
  IndexCache{epoch: String, revision: Nat, index: U.SIx, bounded: Bool}

def IndexCache.index(c: IndexCache) -> U.SIx:
  match c:
    case IndexCache{_, _, ix, _}: ix

def IndexCache.bounded(c: IndexCache) -> Bool:
  match c:
    case IndexCache{_, _, _, bounded}: bounded

# The cap is checked only on misses. Counting shared metadata nodes never
# reads the text of titles, ask details, sessions or paths.
def Size.threads(xs: List<&2, M.Thread>, n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case _ <> rest: Size.threads(rest, Nat.add(n, 1n))

def Size.asks(xs: List<&2, M.Ask>, n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case _ <> rest: Size.asks(rest, Nat.add(n, 1n))

def Size.by(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Thread>>>, n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case (_, ts) <> rest: Size.by(rest, Size.threads(ts, Nat.add(n, 1n)))

def Size.open(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Ask>>>, n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case (_, asks) <> rest: Size.open(rest, Size.asks(asks, Nat.add(n, 1n)))

def IndexCache.fits(ix: U.SIx) -> Bool:
  match ix:
    case U.SIx{by, asks, kids, work}:
      Nat.is_le(Nat.add(Nat.add(Size.by(Map.to_list(&2, List<&2, M.Thread>, by), 0n),
        Size.open(Map.to_list(&2, List<&2, M.Ask>, asks), 0n)),
        Nat.add(List.length(&2, String, Map.keys(&2, Nat, kids)), List.length(&2, String, Map.keys(&2, Nat, work)))), K.CacheBudget.entries())

def IndexCache.build(epoch: String, revision: Nat, +ix: U.SIx) -> IndexCache:
  IndexCache{epoch, revision, ix, IndexCache.fits(ix)}

def IndexCache.hit(c: IndexCache, +epoch: String, +revision: Nat) -> Bool:
  match c:
    case IndexCache{saved, n, _, bounded}: Bool.and(bounded, Bool.and(String.eq(saved, epoch), Nat.is_eq(n, revision)))

def IndexCache.choose(hit: Bool, c: IndexCache, epoch: String, revision: Nat, st: M.State) -> IndexCache:
  match hit:
    case True{}: c
    case False{}: IndexCache.build(epoch, revision, U.Side.ix.inputs(U.SidebarInput.of(st)))

def IndexCache.obtain.revision(+epoch: String, +revision: Nat, st: M.State, saved: Maybe<&2, IndexCache>) -> IndexCache:
  match saved:
    case None{}: IndexCache.build(epoch, revision, U.Side.ix.inputs(U.SidebarInput.of(st)))
    case Some{+c}: IndexCache.choose(IndexCache.hit(c, epoch, revision), c, epoch, revision, st)

# Legacy callers without a trusted dependency revision stay conservative.
def IndexCache.obtain(epoch: String, +st: M.State, saved: Maybe<&2, IndexCache>) -> IndexCache:
  IndexCache.obtain.revision(epoch, M.State.seq(st), st, saved)
