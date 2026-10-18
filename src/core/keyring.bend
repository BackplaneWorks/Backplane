import Base

# The desktop's login keyring (pure)
# ==================================
# Some agent CLIs read their credentials (an MCP server's OAuth token, a
# login) from the Secret Service's login collection. While it is locked
# such a CLI dies at start or hangs, writing nothing useful. The server
# asks whether it is locked (busctl, read only: nothing is ever unlocked;
# BACKPLANE_KEYRING_LOCKED=1/0 overrides it for tests) and:
# - a provider that needs it (Keyring.needs) is not started while it is
#   locked: its turn fails at once saying so (law keyring_locked_never_starts);
# - any agent that fails while it is locked has the hint added to its
#   failure (Keyring.say).
# A state is 0 unknown (no bus, no service: never an error), 1 unlocked,
# 2 locked.

# the providers whose CLI reads credentials from the keyring
def Keyring.needs(+provider: String) -> Bool:
  String.eq(provider, "codex")

# the state busctl's answer gives ("b true" / "b false"; else unknown)
def Keyring.of(+out: String) -> Nat:
  Bool.pick(Nat, String.eq(String.trim(out), "b true"), 2n, Bool.pick(Nat, String.eq(String.trim(out), "b false"), 1n, 0n))

# the test override ("1" locked, "0" unlocked; else none: 3)
def Keyring.env(+v: String) -> Nat:
  Bool.pick(Nat, String.eq(v, "1"), 2n, Bool.pick(Nat, String.eq(v, "0"), 1n, 3n))

def Keyring.locked(+state: Nat) -> Bool:
  Nat.is_eq(state, 2n)

# whether a provider's process may start in this state
def Keyring.start(+provider: String, +state: Nat) -> Bool:
  Bool.not(Bool.and(Keyring.needs(provider), Keyring.locked(state)))

def Keyring.unlock() -> String:
  "the login keyring is locked; unlock it (log in to the desktop session, or open Passwords and Keys)"

# why a provider was not started
def Keyring.refusal(+provider: String) -> String:
  provider ++ " was not started: " ++ Keyring.unlock() ++ " — " ++ provider ++ " reads credentials from it"

# a failure's words, with the hint when the keyring is locked (an agent
# that reads credentials from it dies at start while it is)
def Keyring.say(+msg: String, +state: Nat) -> String:
  Bool.pick(String, Keyring.locked(state), msg ++ " — note: " ++ Keyring.unlock() ++ "; an agent that reads credentials from it fails until then", msg)
