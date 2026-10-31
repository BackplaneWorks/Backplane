import Base

# These files are a private journal of the live buffers, not conversation
# history. The pure checkpoint module validates them against the durable log.
def Checkpoint.write(home: String, record: String) -> IO(String):
  import "./effects/live-checkpoint.js"

def Checkpoint.sync(home: String, records: String) -> IO(String):
  import "./effects/live-checkpoint.js"

def Checkpoint.read(home: String) -> IO(String):
  import "./effects/live-checkpoint.js"
