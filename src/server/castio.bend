import Base
import ./cacheio.bend as CACHE

# Which clients show which browser page (effects/cast.js).

# The cache effect owns only bounded, expiring subscription metadata.
# Client/page selection remains the caller's decision. A disconnected client
# is removed immediately; no raw socket, channel or history lives in a slot.
def CastFx.seen(client: U32, page: String) -> IO(Bool):
  import "./effects/cast.js"

def CastFx.to(page: String) -> IO(List<&2, U32>):
  import "./effects/cast.js"

def CastFx.drop(client: U32) -> IO(Unit):
  import "./effects/cast.js"

# client shows page now: whether the helper's cast lease is due again
def Cast.seen(+client: U32, +page: String) -> IO(Bool):
  do IO<Bool>:
    CACHE.Cache.start()
    CastFx.seen(client, page)

# clients that showed this page in the last three seconds, in stable order
def Cast.to(+page: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    CACHE.Cache.start()
    CastFx.to(page)

def Cast.drop(+client: U32) -> IO(Unit):
  do IO<Unit>:
    CACHE.Cache.start()
    CastFx.drop(client)
