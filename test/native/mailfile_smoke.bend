import Base
import ../../src/core/hub.bend as H
import ../../src/core/mail.bend as ML
import ../../src/core/mailbody.bend as MB
import ../../src/core/model.bend as M
import ../../src/server/mailstore.bend as MS
import ../../src/server/argv.bend as A
import ../../src/server/host.bend as FX
import ../check.bend as C

def stored(+r: MS.StoreResult<Bool>) -> String:
  match r:
    case MS.Stored{_}: "stored"
    case MS.Missing{_}: "missing"
    case MS.StoreError{_, +reason}: reason

def text(+r: MS.StoreResult<List<&2, MB.Body>>) -> String:
  match r:
    case MS.Stored{+b <> _}: MB.Body.text(b)
    case _: "error"

def headers(+r: MS.StoreResult<List<&2, M.Change>>) -> String:
  match r:
    case MS.Stored{+cs}: Nat.show(List.length(&2, M.Change, cs))
    case _: "error"

def smoke(+home: String) -> IO(Unit):
  do IO<Unit>:
    # host.c's join helpers need the CBOR buffer emitted in small native
    # targets too; exercise it in memory, as native/mailstore_test does.
    frame : List<&2, U32> <- FX.Json.cbor_frame("null", "", "")
    C.Check.str("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    +body : MB.Body = H.Mail.body(M.State.empty(), 1n, ML.Origin{"thread:a", "thread", ""}, "bot:b", "hé🙂 body", "smoke", "pair:bot:b|thread:a", "Smoke", "")
    wrote : MS.StoreResult<Bool> <- MS.MailStore.append(home, body)
    C.Check.str("body append succeeds", stored(wrote), "stored")
    +got : MS.StoreResult<List<&2, MB.Body>> <- MS.MailStore.read(home, "pair:bot:b|thread:a", "", 50n)
    C.Check.str("body read exact", text(got), "hé🙂 body")
    recovered : MS.StoreResult<List<&2, M.Change>> <- MS.MailStore.recover(home, ML.Box.empty())
    C.Check.str("recover journal before header commit", headers(recovered), "2")

def args(xs: List<String>) -> IO(Unit):
  match xs:
    case +home <> _: smoke(home)
    case _: IO.die(Unit, 1, "pass a temporary /tmp home")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), args)
