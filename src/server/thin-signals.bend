import Base
import ../core/model.bend as M
import ../core/signal.bend as SG
import ../core/json.bend as J

# Retention only, never a transport/body limit. One scalar slot and one text
# slot per actor, each at most 512 KiB of UTF-8 keys/derived values. Scalar
# values additionally cost eight bytes each; entry caps independently bound
# trie/list overhead (this is not a JS heap-size estimate). Thread metadata
# strings in SideIndex remain shared canonical data under its existing cap.
# The bounded prefix is accounting only: an accepted item is wholly present;
# oversized inputs use the unchanged fresh projection, never truncated text.
def CacheBudget.item(s: String) -> Nat:
  J.Json.bytes.str(String.take(s, 1025n), 0n)

def CacheBudget.scalars(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, n: Nat, +remaining: Nat, valid: Bool) -> Bool:
  match xs n valid:
    case _ _ False{}: False{}
    case Nil{} _ True{}: True{}
    case _ <> _ 0n True{}: False{}
    case (key, _) <> rest 1n+left True{}:
      +bytes = CacheBudget.item(key)
      CacheBudget.scalars(rest, left, Nat.sub(remaining, Nat.add(bytes, 8n)),
        Bool.and(Nat.is_le(bytes, 1024n), Nat.is_le(Nat.add(bytes, 8n), remaining)))

def CacheBudget.strings(xs: List<&2, Sigma<&2, &2, String, _ => String>>, n: Nat, +remaining: Nat, valid: Bool) -> Bool:
  match xs n valid:
    case _ _ False{}: False{}
    case Nil{} _ True{}: True{}
    case _ <> _ 0n True{}: False{}
    case (key, value) <> rest 1n+left True{}:
      +kb = CacheBudget.item(key)
      +vb = CacheBudget.item(value)
      CacheBudget.strings(rest, left, Nat.sub(remaining, Nat.add(kb, vb)),
        Bool.and(Bool.and(Nat.is_le(kb, 1024n), Nat.is_le(vb, 1024n)), Nat.is_le(Nat.add(kb, vb), remaining)))

# One private canonical-derived value on a surviving thin actor. No UI,
# recipient info, clock, draft, command or rendered node is retained here.
# The epoch and last signal-dependent canonical sequence are server inputs,
# never renderer acknowledgements. Legacy callers use the full state sequence.
type SignalCache is Data:
  SignalCache{epoch: String, seq: Nat, signals: Map<&2, Nat>, bounded: Bool}

def SignalCache.signals(c: SignalCache) -> Map<&2, Nat>:
  match c:
    case SignalCache{_, _, signals, _}: signals

def SignalCache.bounded(c: SignalCache) -> Bool:
  match c:
    case SignalCache{_, _, _, bounded}: bounded

def SignalCache.build(epoch: String, seq: Nat, +signals: Map<&2, Nat>) -> SignalCache:
  SignalCache{epoch, seq, signals, CacheBudget.scalars(Map.to_list(&2, Nat, signals), 8192n, 524288n, True{})}

def SignalCache.hit(c: SignalCache, +epoch: String, +seq: Nat) -> Bool:
  match c:
    case SignalCache{saved, n, _, bounded}:
      Bool.and(bounded, Bool.and(String.eq(saved, epoch), Nat.is_eq(n, seq)))

def SignalCache.choose(hit: Bool, c: SignalCache, epoch: String, seq: Nat, st: M.State) -> SignalCache:
  match hit:
    case True{}: c
    case False{}: SignalCache.build(epoch, seq, SG.Signal.index(st))

def SignalCache.obtain.revision(+epoch: String, +revision: Nat, st: M.State, saved: Maybe<&2, SignalCache>) -> SignalCache:
  match saved:
    case None{}: SignalCache.build(epoch, revision, SG.Signal.index(st))
    case Some{+c}: SignalCache.choose(SignalCache.hit(c, epoch, revision), c, epoch, revision, st)

# Compatibility entry is deliberately conservative for callers without a
# hub-owned dependency revision. No request field can opt into a cache hit.
def SignalCache.obtain(epoch: String, +st: M.State, saved: Maybe<&2, SignalCache>) -> SignalCache:
  SignalCache.obtain.revision(epoch, M.State.seq(st), st, saved)
