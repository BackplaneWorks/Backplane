import Base
import ../mailstore_test.bend as T
import ../mailbody_test.bend as B
import ../check.bend as C
import ../../src/server/mailstore.bend as S
import ../../src/server/perf.bend as P
import ../../src/server/host.bend as H
import ../../src/core/mailbody.bend as MB
import ../../src/core/mail.bend as ML

def bench_result(r: S.StoreResult<Bool>, n: Nat, home: String, text: String, k: Unit -> IO(Unit)) -> IO(Unit):
  match r:
    case S.Stored{_}: k(Unit{})
    case _: IO.die(Unit, 1, "benchmark append failed at " ++ Nat.show(n) ++ " in " ++ home ++ " body bytes " ++ Nat.show(String.length(text)))

# Native emission plus a reproducible 10000 x 16 KiB append/read benchmark.
def bench(+n: Nat, +home: String, +text: String) -> IO(Unit):
  match n:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        r : S.StoreResult<Bool> <- S.MailStore.append(home, B.fixture("m" ++ Nat.show(k), "long", text, k))
        bench_result(r, k, home, text, u => bench(k, home, text))

def count(r: S.StoreResult<List<&2, MB.Body>>) -> String:
  match r:
    case S.Stored{bs}: Nat.show(List.length(&2, MB.Body, bs))
    case _: "error"

def first(r: S.StoreResult<List<&2, MB.Body>>) -> String:
  match r:
    case S.Stored{b <> t}: ML.Mail.id(MB.Body.mail(b))
    case _: "error"

def main() -> IO(Unit):
  do IO<Unit>:
    # host.c currently emits join helpers unconditionally, while their CBOR
    # buffer dependency is conditional. Exercise that existing codec here so
    # this small native target includes the same dependency as the real hub.
    # No socket, hub or user data is touched by this in-memory frame check.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    C.Check.str("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    T.main()
    +home : String <- T.home()
    t0 : U32 <- P.Perf.us()
    bench(10000n, home ++ "/bench", B.long(14n, "x"))
    +t1 : U32 <- P.Perf.us()
    +page : S.StoreResult<List<&2, MB.Body>> <- S.MailStore.read(home ++ "/bench", "long", "", 50n)
    +t2 : U32 <- P.Perf.us()
    +deep : S.StoreResult<List<&2, MB.Body>> <- S.MailStore.read(home ++ "/bench", "long", "m200", 50n)
    t3 : U32 <- P.Perf.us()
    C.Check.str("10000 row newest 50", T.status(List<&2, MB.Body>, page), "ok")
    C.Check.str("page five deep", T.status(List<&2, MB.Body>, deep), "ok")
    C.Check.str("newest 50 count", count(page), "50")
    C.Check.str("newest starts at m0", first(page), "m0")
    C.Check.str("deep 50 count", count(deep), "50")
    C.Check.str("deep starts strictly before m200", first(deep), "m201")
    capped : S.StoreResult<List<&2, MB.Body>> <- S.MailStore.read(home ++ "/bench", "long", "", 240n)
    C.Check.str("200 limit spans index blocks", count(capped), "200")
    IO.print("bench rows=10000 body_bytes=16384 append_us=" ++ U32.show(U32.sub(t1, t0)) ++ " newest50_us=" ++ U32.show(U32.sub(t2, t1)) ++ " page5_us=" ++ U32.show(U32.sub(t3, t2)))
