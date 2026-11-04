import Base
import ./json.bend as J
import ./json-object.bend as JO

# The shared install policy, used by every client. An unknown wire phase
# is a visible recoverable error, never a blank row or a disabled action.
type CadPhase is Data:
  CadIdle{}
  CadStarting{}
  CadDownload{}
  CadVerify{}
  CadExtract{}
  CadConfigure{}
  CadCleanup{}
  CadReady{}
  CadFailed{}
  CadCancelled{}
  CadUnavailable{}

def Cad.phase(s: String) -> CadPhase:
  match s:
    case "idle": CadIdle{}
    case "starting": CadStarting{}
    case "download": CadDownload{}
    case "verify": CadVerify{}
    case "extract": CadExtract{}
    case "configure": CadConfigure{}
    case "cleanup": CadCleanup{}
    case "ready": CadReady{}
    case "cancelled": CadCancelled{}
    case "unavailable": CadUnavailable{}
    case _: CadFailed{}

def Cad.busy(p: CadPhase) -> Bool:
  match p:
    case CadStarting{}: True{}
    case CadDownload{}: True{}
    case CadVerify{}: True{}
    case CadExtract{}: True{}
    case CadConfigure{}: True{}
    case CadCleanup{}: True{}
    case _: False{}

def Cad.status(p: CadPhase) -> String:
  match p:
    case CadIdle{}: "Ready to install on this hub"
    case CadStarting{}: "Preparing installation…"
    case CadDownload{}: "Downloading…"
    case CadVerify{}: "Verifying checksum…"
    case CadExtract{}: "Extracting…"
    case CadConfigure{}: "Configuring…"
    case CadCleanup{}: "Cleaning up…"
    case CadReady{}: "Installed"
    case CadFailed{}: "Installation failed"
    case CadCancelled{}: "Installation cancelled"
    case CadUnavailable{}: "No portable package for this hub platform"

def Cad.action(p: CadPhase) -> String:
  match p:
    case CadIdle{}: "cad-install"
    case CadReady{}: "cad-refresh"
    case CadFailed{}: "cad-install"
    case CadCancelled{}: "cad-install"
    case CadUnavailable{}: "cad-guide"
    case _: "cad-cancel"

def Cad.button(p: CadPhase) -> String:
  match p:
    case CadIdle{}: "Install"
    case CadReady{}: "Check installation"
    case CadFailed{}: "Retry"
    case CadCancelled{}: "Retry"
    case CadUnavailable{}: "Installation guide"
    case _: "Cancel"

# The hub applies this accumulator before publishing its shared snapshot.
# New attempts and new phases may reset; an in-phase reading cannot fall.
def Cad.advance(same: Bool, old: Nat, next: Nat) -> Nat:
  match same:
    case True{}: Nat.max(old, next)
    case False{}: next

def Cad.merge(+old: J.Json, +next: J.Json) -> J.Json:
  JO.Obj.put(next, "done", J.Json.nat(Cad.advance(Bool.and(String.eq(J.Json.get_str(old, "attempt", ""), J.Json.get_str(next, "attempt", "")),
    String.eq(J.Json.get_str(old, "phase", ""), J.Json.get_str(next, "phase", ""))), J.Json.get_nat(old, "done", 0n), J.Json.get_nat(next, "done", 0n))))

# A reservation, not a polling watcher, creates the next generation. Retain
# the counter even after terminal dismissal, so captured old messages can
# never replace an accepted retry or resurrect a dismissed result.
def Cad.begin(+old: J.Json) -> J.Json:
  +generation : Nat = Nat.add(1n, J.Json.get_nat(old, "generation", 0n))
  J.Json.obj([J.KV{"attempt", J.Str{Nat.show(generation)}}, J.KV{"generation", J.Json.nat(generation)},
    J.KV{"phase", J.Str{"starting"}}, J.KV{"done", J.Num{"0"}}])

def Cad.dismiss(+old: J.Json) -> J.Json:
  J.Json.obj([J.KV{"attempt", J.Str{J.Json.get_str(old, "attempt", "")}}, J.KV{"generation", J.Json.nat(J.Json.get_nat(old, "generation", 0n))},
    J.KV{"phase", J.Str{"dismissed"}}])

def Cad.terminal(+p: J.Json) -> Bool:
  List.contains(~String, ~String.eq, ["ready", "failed", "cancelled", "unavailable"], J.Json.get_str(p, "phase", ""))

def Cad.expected(+current: J.Json, +attempt: String) -> Bool:
  String.eq(J.Json.get_str(current, "attempt", ""), attempt)

def Cad.accepts(+current: J.Json, +next: J.Json) -> Bool:
  Bool.and(Bool.not(String.is_empty(J.Json.get_str(current, "attempt", ""))),
    Bool.and(Cad.expected(current, J.Json.get_str(next, "attempt", "")),
      Bool.and(Bool.not(String.eq(J.Json.get_str(current, "phase", ""), "dismissed")), Bool.not(Cad.terminal(current)))))

# Reconstruct from known fields only. This also bounds snapshots arriving at
# the consumer; producer-side clamping alone cannot constrain a queue input.
# Error text: <=256 scalar values, <=1536 escaped UTF-8 bytes in JSON.
def Cad.clean(+current: J.Json, +next: J.Json) -> J.Json:
  J.Json.obj([J.KV{"attempt", J.Str{String.take(J.Json.get_str(current, "attempt", ""), 64n)}},
    J.KV{"generation", J.Json.nat(J.Json.get_nat(current, "generation", 0n))},
    J.KV{"phase", J.Str{String.take(J.Json.get_str(next, "phase", "failed"), 16n)}},
    J.KV{"done", J.Json.nat(Nat.min(Nat.mul(8n, 1073741824n), J.Json.get_nat(next, "done", 0n)))},
    J.KV{"total", J.Json.nat(Nat.min(Nat.mul(8n, 1073741824n), J.Json.get_nat(next, "total", 0n)))},
    J.KV{"speed", J.Json.nat(Nat.min(Nat.mul(8n, 1073741824n), J.Json.get_nat(next, "speed", 0n)))},
    J.KV{"eta", J.Json.nat(Nat.min(Nat.mul(8n, 1073741824n), J.Json.get_nat(next, "eta", 0n)))},
    J.KV{"known", J.Flag{J.Json.get_bool(next, "known", False{})}},
    J.KV{"error", J.Str{String.take(J.Json.get_str(next, "error", ""), 256n)}}])

def Cad.receive.k(accepted: Bool, +current: J.Json, next: J.Json) -> J.Json:
  match accepted:
    case False{}: current
    case True{}: Cad.merge(current, Cad.clean(current, next))

def Cad.receive(+current: J.Json, +next: J.Json) -> J.Json:
  Cad.receive.k(Cad.accepts(current, next), current, next)

def Cad.can_start(+current: J.Json, +expected: String) -> Bool:
  Bool.and(Bool.not(Cad.busy(Cad.phase(J.Json.get_str(current, "phase", "idle")))), Bool.or(String.is_empty(expected), Cad.expected(current, expected)))

def Cad.can_cancel(+current: J.Json, expected: String) -> Bool:
  Bool.and(Cad.busy(Cad.phase(J.Json.get_str(current, "phase", "idle"))), Cad.expected(current, expected))

def Cad.can_dismiss(+current: J.Json, expected: String) -> Bool:
  Bool.and(Cad.terminal(current), Cad.expected(current, expected))

def Cad.clear.k(allowed: Bool, +current: J.Json) -> J.Json:
  match allowed:
    case False{}: current
    case True{}: Cad.dismiss(current)

def Cad.clear(+current: J.Json, expected: String) -> J.Json:
  Cad.clear.k(Cad.can_dismiss(current, expected), current)

def Cad.tool(+tool: String) -> Bool:
  Bool.or(String.eq(tool, "kicad"), String.eq(tool, "freecad"))

def Cad.mb(n: Nat) -> String:
  Nat.show(Nat.div(n, 1000000n))

def Cad.transfer(+p: J.Json) -> String:
  "Downloading " ++ Cad.mb(J.Json.get_nat(p, "done", 0n)) ++ Bool.pick(String, Nat.is_gt(J.Json.get_nat(p, "total", 0n), 0n), " / " ++ Cad.mb(J.Json.get_nat(p, "total", 0n)), "") ++ " MB"

def Cad.rate(+p: J.Json) -> String:
  +rate = J.Json.get_nat(p, "speed", 0n)
  Bool.pick(String, Nat.is_gt(rate, 0n), " · " ++ Nat.show(Nat.div(rate, 1000n)) ++ " kB/s" ++ Bool.pick(String, J.Json.get_bool(p, "known", False{}), " · " ++ Nat.show(J.Json.get_nat(p, "eta", 0n)) ++ " s left", ""), " · Measuring speed…")

def Cad.note(+p: J.Json) -> String:
  Bool.pick(String, String.eq(J.Json.get_str(p, "phase", "idle"), "download"), Cad.transfer(p) ++ Cad.rate(p),
    Cad.status(Cad.phase(J.Json.get_str(p, "phase", "idle"))) ++ Bool.pick(String, String.is_empty(J.Json.get_str(p, "error", "")), "", ": " ++ J.Json.get_str(p, "error", "")))

def Cad.fraction(+p: J.Json) -> Nat:
  Nat.min(1000n, Nat.div(Nat.mul(J.Json.get_nat(p, "done", 0n), 1000n), Nat.max(1n, J.Json.get_nat(p, "total", 0n))))

def Cad.known(+p: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(p, "phase", ""), "download"), Nat.is_gt(J.Json.get_nat(p, "total", 0n), 0n))

# Exhaustive visible-state and reachable-control inventory for the law.
def Cad.covered(+p: CadPhase) -> Bool:
  Bool.and(Bool.not(String.is_empty(Cad.status(p))), List.contains(~String, ~String.eq, ["cad-install", "cad-cancel", "cad-refresh", "cad-guide"], Cad.action(p)))

def Cad.counter(+p: J.Json) -> Nat:
  Bool.pick(Nat, Cad.known(p), Cad.fraction(p), Nat.div(J.Json.get_nat(p, "done", 0n), 1000000n))

def Cad.changed(+old: J.Json, +next: J.Json) -> Bool:
  Bool.not(Bool.and(String.eq(J.Json.get_str(old, "attempt", ""), J.Json.get_str(next, "attempt", "")),
    Bool.and(String.eq(J.Json.get_str(old, "phase", ""), J.Json.get_str(next, "phase", "")), Nat.is_eq(Cad.counter(old), Cad.counter(next)))))
