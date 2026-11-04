import Base
import ../src/core/json.bend as J
import ../src/core/desktop-browser.bend as D
import ../src/server/desktop-broker.bend as B
import ../src/server/desktop-info.bend as S

def check(name: String, ok: Bool) -> IO(Unit):
  match ok:
    case True{}: IO.print("ok " ++ name)
    case False{}: IO.die(Unit, 1, "FAIL " ++ name)

def obj(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def text(m: Maybe<&1, String>) -> String:
  Maybe.default(&1, String, m, "")

def stale() -> String:
  "{\"version\":\"fixture-version\",\"hub\":\"\"}"

def main() -> IO(Unit):
  do IO<Unit>:
    +canonical : String = "persisted-machine-A"
    +info : String = S.Desktop.connection_info("connected", "client-text-ignored", "fixture-version", canonical)
    check("connected snapshot uses current canonical identity", String.eq(J.Json.get_str(obj(info), "hub", ""), canonical))
    check("connected snapshot uses configured service version", String.eq(J.Json.get_str(obj(info), "version", ""), "fixture-version"))
    check("connection snapshot cannot use client-supplied identity", Bool.not(String.eq(J.Json.get_str(obj(info), "hub", ""), "client-text-ignored")))
    check("ordinary frame bytes remain unchanged", String.eq(S.Desktop.connection_info("frame", "raw client frame", "version", canonical), "raw client frame"))
    check("close notification bytes remain unchanged", String.eq(S.Desktop.connection_info("closed", "", "version", canonical), ""))
    +out : Chan(String) <- Chan.new(String, 16)
    +ev : Chan(String) <- Chan.new(String, 16)
    +b : B.Broker <- B.Broker.link("connected", B.Broker.empty(), 1, info, out, "epoch-A", "lease-A", "", stale(), ev)
    check("accepted transport stores trusted current snapshot", String.eq(B.Broker.connection_info(b, stale()), info))
    check("hello ignores empty startup identity snapshot", String.eq(J.Json.get_str(B.Broker.hello_payload(b, stale()), "hub", ""), canonical))
    check("hello retains configured version", String.eq(J.Json.get_str(B.Broker.hello_payload(b, stale()), "version", ""), "fixture-version"))
    check("identity snapshot grants no owner", String.is_empty(J.Json.get_str(B.Broker.owner(b, "foreign"), "drive", "")))
    check("identity snapshot grants no readiness", Bool.not(J.Json.get_bool(B.Broker.meta(b), "ready", False{})))
    +ready : B.Broker <- B.Broker.frame(b, "{\"v\":1,\"op\":\"hello\",\"id\":\"request-A\",\"clientSequence\":1,\"payload\":{\"hub\":\"forged\"}}", "", stale(), ev)
    response : Maybe<&1, String> <- Chan.recv(String, out)
    +hello : J.Json = obj(text(response))
    check("actual production hello response accepted", String.eq(J.Json.get_str(hello, "op", ""), "hello.accept"))
    check("actual hello keeps request correlation", String.eq(J.Json.get_str(hello, "id", ""), "request-A"))
    check("actual hello returns canonical hub despite forged payload", String.eq(J.Json.get_str(J.Json.get_obj(hello, "payload"), "hub", ""), canonical))
    check("hello carries exact epoch", String.eq(J.Json.get_str(J.Json.get_obj(hello, "payload"), "serviceEpoch", ""), "epoch-A"))
    check("hello carries exact controller lease", String.eq(J.Json.get_str(J.Json.get_obj(hello, "payload"), "controllerLease", ""), "lease-A"))
    check("hello establishes readiness only after handshake", J.Json.get_bool(B.Broker.meta(ready), "ready", False{}))
    +other : Chan(String) <- Chan.new(String, 16)
    +kept : B.Broker <- B.Broker.link("connected", ready, 2, S.Desktop.connection_info("connected", "", "other-version", "other-machine"), other, "epoch-B", "lease-B", "", stale(), ev)
    refused : Maybe<&1, String> <- Chan.recv(String, other)
    check("competing controller remains refused", String.eq(J.Json.get_str(obj(text(refused)), "error", ""), "controller_busy"))
    check("refused controller cannot replace canonical identity", String.eq(B.Broker.connection_info(kept, stale()), info))
    check("refused controller cannot replace active client", U32.is_eq(B.Broker.client(kept), 1))
    +closed : B.Broker <- B.Broker.close(kept, "fixture transport closed")
    check("closed connection discards its identity snapshot", String.eq(B.Broker.connection_info(closed, "missing"), "missing"))
    +again : Chan(String) <- Chan.new(String, 16)
    +reconnected : B.Broker <- B.Broker.link("connected", closed, 3, info, again, "epoch-C", "lease-C", "", stale(), ev)
    check("reconnect binds same current canonical identity", String.eq(J.Json.get_str(B.Broker.hello_payload(reconnected, stale()), "hub", ""), canonical))
    check("reconnect receives fresh transport epoch", String.eq(J.Json.get_str(B.Broker.hello_payload(reconnected, stale()), "serviceEpoch", ""), "epoch-C"))
    check("reconnect does not revive prior handshake readiness", Bool.not(J.Json.get_bool(B.Broker.meta(reconnected), "ready", False{})))
    +legacy : Chan(String) <- Chan.new(String, 4)
    +old : B.Broker <- B.Broker.connect(True{}, B.Broker.empty(), 4, legacy, "epoch-D", "lease-D")
    check("explicit legacy broker initializer keeps fallback contract", String.eq(J.Json.get_str(B.Broker.hello_payload(old, info), "hub", ""), canonical))
    Chan.close(String, ev)
