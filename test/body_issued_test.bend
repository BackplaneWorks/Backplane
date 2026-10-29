import Base
import ./check.bend as C
import ./native_sidebar_projection_test.bend as F
import ../src/core/json.bend as J
import ../src/server/thin-actor.bend as A

def part(id: String, version: String, thread: String) -> J.Json:
  J.Json.obj([J.KV{"bodyPage", J.Num{"1"}}, J.KV{"id", J.Str{id}}, J.KV{"version", J.Str{version}}, J.KV{"thread", J.Str{thread}}])

def result(parts: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"window", J.Json.obj([J.KV{"bodyPages", J.Flag{True{}}}, J.KV{"partial", J.Json.arr(parts)}])}])

def request(id: String, version: String, thread: String) -> J.Json:
  J.Json.obj([J.KV{"entryId", J.Str{id}}, J.KV{"bodyVersion", J.Str{version}}, J.KV{"bodyThread", J.Str{thread}}])

def actor() -> A.Actor:
  A.Actor.make(F.base(), "view", J.Json.get_obj(F.request(), "target"))

def mark(xs: List<&2, A.Actor>) -> List<&2, A.Actor>:
  match xs:
    case Nil{}: Nil{}
    case a <> rest: A.Actor.mark_dirty(a) <> rest

def many(+n: Nat) -> List<&2, J.Json>:
  match n:
    case 0n: Nil{}
    case 1n+k: part(Nat.show(n), "7", "t1") <> many(k)

def main() -> IO(Unit):
  do IO<Unit>:
    +p : J.Json = request("message", "7", "t1")
    C.Check.str("forged ordinal cannot authorize an unissued body", Bool.show(A.Bodies.allowed([actor()], p)), "False")
    +issued : List<&2, A.Actor> = A.Bodies.record([actor()], result([part("message", "7", "t1")]))
    C.Check.str("published body metadata authorizes its exact source", Bool.show(A.Bodies.allowed(issued, p)), "True")
    C.Check.str("same identity in another owner is denied", Bool.show(A.Bodies.allowed(issued, request("message", "7", "t2"))), "False")
    C.Check.str("another version is denied", Bool.show(A.Bodies.allowed(issued, request("message", "8", "t1"))), "False")
    C.Check.str("another entry is denied", Bool.show(A.Bodies.allowed(issued, request("other", "7", "t1"))), "False")
    C.Check.str("picture invalidation retains an issued reading continuation", Bool.show(A.Bodies.allowed(mark(issued), p)), "True")
    +updated : List<&2, A.Actor> = A.Bodies.record(issued, result([part("message", "8", "t1")]))
    C.Check.str("new authoritative version retires its old source", Bool.show(A.Bodies.allowed(updated, p)), "False")
    C.Check.str("new authoritative version is accessible", Bool.show(A.Bodies.allowed(updated, request("message", "8", "t1"))), "True")
    C.Check.str("issued body ledger is bounded across adjacent pages", Nat.show(List.length(&2, J.Json, A.Bodies.add(many(160n), Nil{}))), "128")
    C.Check.str("unnegotiated reply cannot issue body authority", Bool.show(A.Bodies.allowed(A.Bodies.record([actor()], J.Json.obj(Nil{})), p)), "False")
