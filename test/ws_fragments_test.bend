import Base
import ./check.bend as C
import ../src/core/ws-fragments.bend as F
import ../src/core/http.bend as W
import ../src/core/utf8.bend as U
import ../src/core/json.bend as J
import ../src/core/cbor.bend as B

def pending(t: F.Taken) -> F.Pending:
  match t:
    case F.FragHold{p}: p
    case F.FragEmit{p, _}: p
    case F.FragReject{}: F.FragIdle{}

def label(t: F.Taken) -> String:
  match t:
    case F.FragReject{}: "rejected"
    case F.FragHold{_}: "pending"
    case F.FragEmit{_, W.Frame{_, op, bytes}}: U32.show(op) ++ ":" ++ U.Utf8.decode(bytes)

def finish(t: F.Taken, bytes: List<&2, U32>) -> F.Taken:
  F.Fragment.step(pending(t), W.Frame{True{}, 0, bytes})

def first() -> F.Taken:
  F.Fragment.step(F.FragIdle{}, W.Frame{False{}, 2, [65]})

def payload(t: F.Taken) -> List<&2, U32>:
  match t:
    case F.FragEmit{_, W.Frame{_, _, bs}}: bs
    case _: Nil{}

def large() -> String:
  "Exact message 日本語. " ++ String.repeat("Preserve this text 日本語. ", 2400n)

def large_round_trip() -> String:
  +bytes = B.Cbor.encode(J.Str{large()})
  +head = F.Fragment.step(F.FragIdle{}, W.Frame{False{}, 2, List.take(&2, U32, bytes, 7n)})
  +ping = F.Fragment.step(pending(head), W.Frame{True{}, 9, [112]})
  +middle = F.Fragment.step(pending(ping), W.Frame{False{}, 0, List.take(&2, U32, List.drop(&2, U32, bytes, 7n), 65536n)})
  +last = finish(middle, List.drop(&2, U32, bytes, 65543n))
  J.Json.as_str(B.Cbor.decode(payload(last)), "<missing>")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a complete frame is delivered", label(F.Fragment.step(F.FragIdle{}, W.Frame{True{}, 2, [65, 66]})), "2:AB")
    C.Check.str("an unfinished first frame is not decoded", label(first()), "pending")
    C.Check.str("a final continuation emits one complete message", label(finish(first(), [66])), "2:AB")
    C.Check.str("a middle continuation stays pending", label(F.Fragment.step(pending(first()), W.Frame{False{}, 0, [66]})), "pending")
    C.Check.str("a continuation without a message is rejected", label(F.Fragment.step(F.FragIdle{}, W.Frame{True{}, 0, [66]})), "rejected")
    C.Check.str("a second data start cannot replace unfinished bytes", label(F.Fragment.step(pending(first()), W.Frame{True{}, 2, [66]})), "rejected")
    C.Check.str("an interleaved ping is delivered immediately", label(F.Fragment.step(pending(first()), W.Frame{True{}, 9, [112]})), "9:p")
    C.Check.str("an interleaved ping preserves the entire message", label(finish(F.Fragment.step(pending(first()), W.Frame{True{}, 9, [112]}), [66])), "2:AB")
    C.Check.str("a fragmented Unicode CBOR message above 64 KB arrives exactly", large_round_trip(), large())
