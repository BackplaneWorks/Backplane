import Base
import ../core/wire-retention.bend as WRET

# A replay page is transient and oldest first. The SQLite handle and home
# are hub-owned, never request parameters. Failure carries no partial page:
# callers never acknowledge a cursor past an unread or truncated event.
type WirePage is Data:
  WirePage{lines: List<&2, String>, error: String}

def WirePage.lines(p: WirePage) -> List<&2, String>:
  match p:
    case WirePage{xs, _}: xs

def WirePage.ok(p: WirePage) -> Bool:
  match p:
    case WirePage{_, err}: String.is_empty(err)

def WireLog.page(h: String, home: String, since: Nat, through: Nat) -> IO(WirePage):
  import "./effects/wire_log.js"

# Disk-full commits remain visible, but their public suffix is not indexed
# until recovery. Read one durable page first; subsequent credits cross
# into the bounded queued references without a gap or duplicate.
def WireLog.replay.page(pending: Bool, db: String, home: String, n: Nat, at: Nat, durable: Nat, queued: List<&2, String>) -> IO(WirePage):
  match pending:
    case True{}: IO.pure(WirePage, WirePage{WRET.WireRetention.pending(n, at, queued), ""})
    case False{}: WireLog.page(db, home, at, durable)

def WireLog.replay(+db: String, +home: String, +n: Nat, +at: Nat, +queued: List<&2, String>) -> IO(WirePage):
  +durable = WRET.WireRetention.durable(n, queued)
  WireLog.replay.page(Nat.is_ge(at, durable), db, home, n, at, durable, queued)
