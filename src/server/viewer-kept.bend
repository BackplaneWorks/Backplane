import Base

# A thread's viewer reads kept hub-wide (effects/viewer_kept.js): the last
# answer per thread and key ("fm.kept:<t>\n<dir>": a Files folder's
# listing, client.bend's Fm.kept), for the folder the thread works in. A
# thin view is given its shown thread's (server.bend's Kept.info), so a
# Files tab, a reload or another window draws a folder seen before at the
# press while it is listed again. Derived data only, bounded, never written.
def ViewerKept.put(thread: String, cwd: String, key: String, value: String) -> IO(String):
  import "./effects/viewer_kept.js"

def ViewerKept.drop(thread: String, key: String) -> IO(String):
  import "./effects/viewer_kept.js"

# (only what changed after since, the "kept.seq:<t>" the view was last
# given: a view's actor keeps what it was given; "" for all of it), as keys
# and values in turn
def ViewerKept.info(thread: String, cwd: String, since: String) -> IO(List<&2, String>):
  import "./effects/viewer_kept.js"

# "1": nothing kept under key (or kept longer ago than maxMs milliseconds)
# and no read of it on its way; the caller reads it now
def ViewerKept.claim(thread: String, cwd: String, key: String, maxMs: String) -> IO(String):
  import "./effects/viewer_kept.js"
