import Base

# Invitations are private process memory; device hashes live in a separate
# atomic registry. Never replace the hub token or touch its initialization.
def Pairing.start(home: String, root: String, host: String, port: U32) -> IO(String):
  import "./effects/pairing.js"

def Pairing.resolve(token: String, client: U32, route: String) -> IO(String):
  import "./effects/pairing.js"

def Pairing.request(op: String, secret: String, name: String, authority: String, https: Bool) -> IO(String):
  import "./effects/pairing.js"

def Pairing.forget(client: U32) -> IO(Unit):
  import "./effects/pairing.js"

def Pairing.ready(ok: Bool, message: String) -> IO(Unit):
  match ok:
    case True{}: IO.pure(Unit, Unit{})
    case False{}: IO.die(Unit, 1, "backplane: " ++ message)

# Re-check queued socket admissions after any revocation, before publishing a
# joined client. A revoke racing the websocket handshake must also win.
def Pairing.active(client: U32) -> IO(Bool):
  import "./effects/pairing.js"

# Listener-owned reachable origins, never the root credential itself.
def Pairing.address(link: String) -> IO(Unit):
  import "./effects/pairing.js"

# The owner's machines this hub found (Machines.ask's JSON) and the urls of
# the links it verified as the owner's: what POST /auth/known compares a
# pairing link's origin with (effects/pairing.js's pairingKnown).
def Pairing.known(machines: String, peers: String) -> IO(Unit):
  import "./effects/pairing.js"

# Whether the local network listener opened as `base` is still wanted
# (server.bend's Lan.loop ends once it is not: turned off, or opened again).
def Pairing.lan(ip: String, base: U32) -> IO(Bool):
  import "./effects/pairing.js"
