import Base

# Which clients show which browser page (effects/cast.js).

# client shows page now: whether the helper's cast lease is due again
def Cast.seen(client: U32, page: String) -> IO(Bool):
  import "./effects/cast.js"

# the clients that showed page in the last few seconds
def Cast.to(page: String) -> IO(List<&2, U32>):
  import "./effects/cast.js"
