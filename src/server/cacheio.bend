import Base

# Derived data only. Eviction and expiry always mean a miss; these slots
# must never own durable model facts, credentials or event history.
type CacheSlot is Data:
  BrowserSeen{}
  BrowserLease{}

type CacheBudget is Data:
  CacheBudget{entries: U32, bytes: U32, age_ms: U32}

def CacheSlot.code(slot: CacheSlot) -> U32:
  match slot:
    case BrowserSeen{}: 0
    case BrowserLease{}: 1

# One shared policy for browser subscription metadata. The effect charges
# keys, primitive values and fixed per-entry overhead, rather than payload
# bytes alone. These are charged-byte bounds, not a whole-process RSS bound.
def Cache.budget() -> CacheBudget:
  CacheBudget{4096, 524288, 3001}

def Cache.global_bytes() -> U32:
  8388608

def CacheFx.ready(budget: CacheBudget, total_bytes: U32) -> IO(Unit):
  import "./effects/bounded_cache.js"

def CacheFx.get(slot: U32, key: String) -> IO(Maybe<&2, String>):
  import "./effects/bounded_cache.js"

def CacheFx.put(slot: U32, key: String, value: String) -> IO(Bool):
  import "./effects/bounded_cache.js"

def CacheFx.remove(slot: U32, key: String) -> IO(Unit):
  import "./effects/bounded_cache.js"

# Bounded numeric counters and registered slot names only, never keys/values.
def CacheFx.stats() -> IO(String):
  import "./effects/bounded_cache.js"

def Cache.start() -> IO(Unit):
  CacheFx.ready(Cache.budget(), Cache.global_bytes())

def Cache.get(+slot: CacheSlot, +key: String) -> IO(Maybe<&2, String>):
  do IO<Maybe<&2, String>>:
    Cache.start()
    CacheFx.get(CacheSlot.code(slot), key)

def Cache.put(+slot: CacheSlot, +key: String, +value: String) -> IO(Bool):
  do IO<Bool>:
    Cache.start()
    CacheFx.put(CacheSlot.code(slot), key, value)

def Cache.remove(+slot: CacheSlot, +key: String) -> IO(Unit):
  do IO<Unit>:
    Cache.start()
    CacheFx.remove(CacheSlot.code(slot), key)

def Cache.stats() -> IO(String):
  do IO<String>:
    Cache.start()
    CacheFx.stats()
