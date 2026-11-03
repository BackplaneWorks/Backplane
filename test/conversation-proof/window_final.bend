import Base
import ../../src/core/json.bend as J
import ../../src/server/thin-view.bend as V
import ../../src/server/conversation-frame.bend as F
import ../../src/server/conversation-window.bend as W
import ./packet_schema.bend as S
import ./packet_order.bend as O
import ./escaped_cost.bend as E

# Prove metadata algebra over an arbitrary picture/projection, then
# instantiate actual Window.make once. No root-size input is assumed.
def put_get(result: J.Json, +shown: J.Json, +projection: J.Json) ->
  {J.Json.get(V.Obj.put(V.Obj.put(V.Obj.put(result, "shown", shown), "windowProjection", projection), "boundedFrame", J.Num{"1"}), "shown") == Some{shown} : Maybe<&2, J.Json>}:
  match result:
    case J.Obj{_}: {==}
    case J.Null{}: {==}
    case J.Flag{_}: {==}
    case J.Num{_}: {==}
    case J.Str{_}: {==}
    case J.Arr{_}: {==}
    case J.Item{_, _}: {==}
    case J.Field{_, _, _}: {==}
    case J.End{}: {==}

def projection(+result: J.Json) -> J.Json:
  +root = W.Window.root(J.Json.get_obj(result, "shown"))
  J.Json.obj([J.KV{"root", root}, J.KV{"logicalBytes", J.Json.nat(J.Json.bytes(root))}])

def made_shown(+result: J.Json) ->
  {J.Json.get(W.Window.make(result), "shown") == Some{W.Window.picture()} : Maybe<&2, J.Json>}:
  put_get(result, W.Window.picture(), projection(result))

def shape(+rid: J.Json, +result: J.Json) ->
  {F.Control.envelope(rid, W.Window.make(result)) == S.packet(rid, W.Window.picture()) : J.Json}:
  Equal.cong(Maybe<&2, J.Json>, J.Json, m => S.shell(rid, m), J.Json.get(W.Window.make(result), "shown"), Some{W.Window.picture()}, made_shown(result))

# A concrete complete FINAL reply, including both ID occurrences. This is
# checked by Bend, not accepted from a Python calculation or runtime sample.
def reference_packet_bytes() ->
  {J.Json.bytes(S.packet(S.sentinel(), W.Window.picture())) == 244n : Nat}:
  {==}

def producer_bound(+rid: J.Json, +result: J.Json,
  admitted: {Nat.is_le(J.Json.bytes(rid), 32n) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes(F.Control.envelope(rid, W.Window.make(result))), 262144n) == True{} : Bool}:
  %Equal.sym(J.Json, F.Control.envelope(rid, W.Window.make(result)), S.packet(rid, W.Window.picture()), shape(rid, result)) : {Nat.is_le(J.Json.bytes(_), 262144n) == True{} : Bool}
  E.le_trans(J.Json.bytes(S.packet(rid, W.Window.picture())), J.Json.bytes(S.packet(S.sentinel(), W.Window.picture())), 262144n,
    O.envelope_mono(rid, S.sentinel(), W.Window.picture(), admitted), {==})

def producer_ok(+rid: J.Json, result: J.Json) ->
  {J.Json.get_bool(F.Control.envelope(rid, W.Window.make(result)), "ok", False{}) == True{} : Bool}:
  {==}

# The arbitrary canonical region is retained exactly as a control VALUE;
# this theorem makes no aggregate byte bound or browser adoption claim.
def put_projection(result: J.Json, +shown: J.Json, +region: J.Json) ->
  {J.Json.get_obj(V.Obj.put(V.Obj.put(V.Obj.put(result, "shown", shown), "windowProjection", region), "boundedFrame", J.Num{"1"}), "windowProjection") == region : J.Json}:
  match result:
    case J.Obj{_}: {==}
    case J.Null{}: {==}
    case J.Flag{_}: {==}
    case J.Num{_}: {==}
    case J.Str{_}: {==}
    case J.Arr{_}: {==}
    case J.Item{_, _}: {==}
    case J.Field{_, _, _}: {==}
    case J.End{}: {==}

def retained_projection(+result: J.Json) ->
  {J.Json.get_obj(W.Window.make(result), "windowProjection") == projection(result) : J.Json}:
  put_projection(result, W.Window.picture(), projection(result))

def retained_root(+result: J.Json) ->
  {J.Json.get_obj(J.Json.get_obj(W.Window.make(result), "windowProjection"), "root") == W.Window.root(J.Json.get_obj(result, "shown")) : J.Json}:
  Equal.cong(J.Json, J.Json, j => J.Json.get_obj(j, "root"), J.Json.get_obj(W.Window.make(result), "windowProjection"), projection(result), retained_projection(result))
