import Base

# The record of agent processes the hub started, kept through a restart
# in place (effects/adopt.c; the decisions are core/adopt.bend's).

# the record's file; the record starts empty
def Adopt.record(path: String) -> IO(Unit):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# a process started for a thread, with its socket (answered back)
def Adopt.keep(thread: String, pid: U32, s: Socket) -> IO(Socket):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# a per-turn process started for a thread, recorded with no descriptor:
# never adopted, stopped by the next image if it still runs
def Adopt.note(thread: String, pid: U32) -> IO(Unit):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# a process that ended leaves the record
def Adopt.drop(thread: String, pid: U32) -> IO(Unit):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# a pid's start identity now ("" when it is gone)
def Adopt.ident(pid: U32) -> IO(String):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# a descriptor an exec kept, as a socket again
def Adopt.take(fd: U32) -> IO(Result<&1, &1, U32 & String, Socket>):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# SIGTERM to pid if it still is the process of that start identity (never
# 0, 1, the hub or its ancestors); whether it was signalled
def Adopt.stop(pid: U32, ident: String) -> IO(Bool):
  import "./effects/adopt.c"
  import "./effects/adopt.js"

# waits, without blocking, for the children the last exec handed over that
# no one else waits for (effects/adopt.c); how many are left
def Adopt.reap() -> IO(U32):
  import "./effects/adopt.c"
  import "./effects/adopt.js"
