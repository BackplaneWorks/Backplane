import Base

# Hub-only local diagnostics, separate from free-form logs and telemetry.
# start AFTER acquiring the home lock and setting BACKPLANE_HOME. The effect
# binds that home once; no RPC can supply a location. A local byte remembers
# enable/disable. Every result is bounded JSON with closed status/error fields.
# See docs/diagnostics-backend.md for the integration and schemas.
def Diagnostics.start() -> IO(String):
  import "./effects/diagnostics.js"

def Diagnostics.status() -> IO(String):
  import "./effects/diagnostics.js"

# Call on the hub's existing one-second Tick; no new timer or actor.
def Diagnostics.tick() -> IO(Unit):
  import "./effects/diagnostics.js"

def Diagnostics.flush() -> IO(String):
  import "./effects/diagnostics.js"

# Only closed event/detail pairs are accepted; never pass error messages,
# RPC parameters, connection IDs or a user's text as either argument.
def Diagnostics.event(event: String, detail: String, value: U32) -> IO(Unit):
  import "./effects/diagnostics.js"

# on | off | clear; anything else fails closed.
# A failed durable control change returns ok:false and a fixed error code.
def Diagnostics.control(action: String) -> IO(String):
  import "./effects/diagnostics.js"

# The sanitized JSON document itself; no output path and no upload.
def Diagnostics.export() -> IO(String):
  import "./effects/diagnostics.js"
