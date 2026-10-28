import Base

# The provider-owned auth lifecycle. Results are bounded public metadata;
# credentials and callback input never leave the private IO actor.
def AccountsFx.request(home: String, request: String) -> IO(String):
  import "./effects/accounts.js"

def AccountsFx.snapshot(home: String) -> IO(String):
  import "./effects/accounts.js"

# Rechecks the selected identity before a new process, using the same store
# and environment as login, status, models and limits. No model turn.
def AccountsFx.spawn(home: String, profile: String, epoch: String, cmd: String, args: List<&2, String>, cwd: String, err: U32) ->
  IO(Result<&1, &1, U32 & String, U32 & Socket>):
  import "./effects/accounts.js"

# Only a current child may publish passive Claude windows; no inference probe.
def AccountsFx.observe(home: String, profile: String, epoch: String, reading: String) -> IO(String):
  import "./effects/accounts.js"

# Existing one-shot text jobs use the identical account store, with a 60s cap.
def AccountsFx.spawn_text(home: String, profile: String, epoch: String, cmd: String, args: List<&2, String>, cwd: String, err: U32) ->
  IO(Result<&1, &1, U32 & String, U32 & Socket>):
  import "./effects/accounts.js"
