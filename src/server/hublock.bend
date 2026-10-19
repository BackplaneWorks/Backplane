import Base

# One hub per home: the flock on <home>/hub.lock (effects/hublock.c; the
# answers are core/onehub.bend's).

# take the lock for this process and write "<pid> <port> flock" into the
# file: "ok", "held <pid> <port> flock" or "fail <why>"; taking it again
# (or after an exec that handed it over) answers "ok"
def HubLock.take(path: String, port: String) -> IO(String):
  import "./effects/hublock.c"
  import "./effects/hublock.js"

# "" when no other process holds the lock, else what the holder wrote
def HubLock.held(path: String) -> IO(String):
  import "./effects/hublock.c"
  import "./effects/hublock.js"
