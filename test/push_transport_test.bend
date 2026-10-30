import Base
import ./check.bend as C
import ../src/server/push.bend as P

def posts(+dir: String) -> IO(Unit):
  do IO<Unit>:
    first : Bool <- P.Push.post(dir, "same", "https://synthetic.invalid/device/synthetic", ["apns-topic: synthetic", "authorization: synthetic-only"], "{}", "activity", "1")
    C.Check.str("permanent response retires exact registration", Bool.show(first), "True")
    suppressed : Bool <- P.Push.post(dir, "same", "https://synthetic.invalid/device/synthetic", ["apns-topic: synthetic"], "{}", "activity", "1")
    C.Check.str("later demand sends no permanent retry", Bool.show(suppressed), "False")
    fresh : Bool <- P.Push.post(dir, "same", "https://synthetic.invalid/device/synthetic", ["apns-topic: synthetic"], "{}", "activity", "2")
    C.Check.str("fresh registration has its own delivery", Bool.show(fresh), "True")
    alert : Bool <- P.Push.post(dir, "same", "https://synthetic.invalid/device/synthetic", ["apns-topic: synthetic"], "{}", "alert", "1")
    C.Check.str("temporary alert failure never retires source token", Bool.show(alert), "False")
    held : Bool <- P.Push.post(dir, "same", "https://synthetic.invalid/device/synthetic", ["apns-topic: synthetic"], "{}", "alert", "1")
    C.Check.str("failed alert has bounded later demands", Bool.show(held), "False")

def fixture(ready: Bool, dir: String) -> IO(Unit):
  match ready:
    case True{}: posts(dir)
    case False{}: C.Check.str("synthetic transport fixture required", "missing", "required")

def main() -> IO(Unit):
  do IO<Unit>:
    +dir : String <- P.Push.env("PUSH_FIXTURE_DIR")
    fixture(Bool.not(String.is_empty(dir)), dir)
