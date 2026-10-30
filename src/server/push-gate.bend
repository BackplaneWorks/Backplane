import Base

# A bounded process-local circuit. Its only retained data are opaque hashes,
# counters and deadlines, never notification content, credentials or Ui state.
# A registration timestamp changes its identity. A grant is an opaque lease;
# an empty answer coalesces an activity, a '-' answer reports a delivery hold.
def PushGate.claim(url: String, topic: String, kind: String, registration: String, now: Nat) -> IO(String):
  import "./effects/push-gate.js"

def PushGate.finish(lease: String, code: String, reason: String, now: Nat) -> IO(Unit):
  import "./effects/push-gate.js"
