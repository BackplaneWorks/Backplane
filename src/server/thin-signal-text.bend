import Base
import ../core/signal.bend as SG
import ./thin-signals.bend as K

# One canonical-derived text map, never recipient info, Ui, session, draft,
# history, clock, node or command. Online visibility is an exact key input.
# Values are the original Signal.cache() applied to an empty info map.
type SignalTextCache is Data:
  SignalTextCache{epoch: String, revision: Nat, online: Bool, values: Map<&2, String>, bounded: Bool}

def SignalTextCache.values(c: SignalTextCache) -> Map<&2, String>:
  match c:
    case SignalTextCache{_, _, _, values, _}: values

def SignalTextCache.bounded(c: SignalTextCache) -> Bool:
  match c:
    case SignalTextCache{_, _, _, _, bounded}: bounded

def SignalTextCache.build(epoch: String, revision: Nat, +online: Bool, signals: Map<&2, Nat>) -> SignalTextCache:
  +values = SG.Signal.cache(online, signals, Map.new(&2, String))
  SignalTextCache{epoch, revision, online, values, K.CacheBudget.strings(Map.to_list(&2, String, values), 8193n, 524288n, True{})}

def SignalTextCache.online(saved: Bool, online: Bool) -> Bool:
  match saved:
    case True{}: online
    case False{}: Bool.not(online)

def SignalTextCache.hit(c: SignalTextCache, +epoch: String, +revision: Nat, +online: Bool) -> Bool:
  match c:
    case SignalTextCache{saved, n, on, _, bounded}:
      Bool.and(bounded, Bool.and(String.eq(saved, epoch), Bool.and(Nat.is_eq(n, revision), SignalTextCache.online(on, online))))

def SignalTextCache.choose(hit: Bool, c: SignalTextCache, epoch: String, revision: Nat, online: Bool, signals: Map<&2, Nat>) -> SignalTextCache:
  match hit:
    case True{}: c
    case False{}: SignalTextCache.build(epoch, revision, online, signals)

def SignalTextCache.obtain(+epoch: String, +revision: Nat, +online: Bool, signals: Map<&2, Nat>, saved: Maybe<&2, SignalTextCache>) -> SignalTextCache:
  match saved:
    case None{}: SignalTextCache.build(epoch, revision, online, signals)
    case Some{+c}: SignalTextCache.choose(SignalTextCache.hit(c, epoch, revision, online), c, epoch, revision, online, signals)
