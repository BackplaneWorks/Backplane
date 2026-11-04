import Base
import ./json.bend as J
import ./json-object.bend as JO

# The shared install policy, used by every client. An unknown wire phase
# is a visible recoverable error, never a blank row or a disabled action.
type CadPhase is Data:
  CadIdle{}
  CadDownload{}
  CadVerify{}
  CadExtract{}
  CadConfigure{}
  CadReady{}
  CadFailed{}
  CadCancelled{}
  CadUnavailable{}

def Cad.phase(s: String) -> CadPhase:
  match s:
    case "idle": CadIdle{}
    case "download": CadDownload{}
    case "verify": CadVerify{}
    case "extract": CadExtract{}
    case "configure": CadConfigure{}
    case "ready": CadReady{}
    case "cancelled": CadCancelled{}
    case "unavailable": CadUnavailable{}
    case _: CadFailed{}

def Cad.busy(p: CadPhase) -> Bool:
  match p:
    case CadDownload{}: True{}
    case CadVerify{}: True{}
    case CadExtract{}: True{}
    case CadConfigure{}: True{}
    case _: False{}

def Cad.status(p: CadPhase) -> String:
  match p:
    case CadIdle{}: "Ready to install on this hub"
    case CadDownload{}: "Downloading…"
    case CadVerify{}: "Verifying checksum…"
    case CadExtract{}: "Extracting…"
    case CadConfigure{}: "Configuring…"
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
