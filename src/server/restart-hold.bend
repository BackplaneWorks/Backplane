import Base

# One bounded private exec request while another turn or tool is active.
# Nothing is persisted, launched, signalled or exposed to a renderer here.
def RestartHold.offer(payload: String) -> IO(Bool):
  import "./effects/restart-hold.js"

def RestartHold.take() -> IO(String):
  import "./effects/restart-hold.js"
