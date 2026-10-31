import Base

# These files are a private journal of the live buffers, not conversation
# history. The pure checkpoint module validates them against the durable log.
# (effects/live-checkpoint.js: a whole record, then each delta appended)
def Checkpoint.write(home: String, record: String) -> IO(String):
  import "./effects/live-checkpoint.js"

# one delta: "" when appended, "new" when the whole record must be written
def Checkpoint.append(home: String, thread: String, gen: String, text: String) -> IO(String):
  import "./effects/live-checkpoint.js"

def Checkpoint.sync(home: String, records: String) -> IO(String):
  import "./effects/live-checkpoint.js"

# at start: every journal but those of the restored threads (a JSON list) goes
def Checkpoint.keep(home: String, threads: String) -> IO(String):
  import "./effects/live-checkpoint.js"

def Checkpoint.read(home: String) -> IO(String):
  import "./effects/live-checkpoint.js"
