import Base
import ../src/core/json.bend as J
import ../src/core/cbor.bend as C

# Driven by test/tools/json_native.mjs: wire.js's json() must build exactly
# the tree Json.parse gives for JSON.stringify of the same value.

def parse(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def json_tags() -> J.Json:
  J.Arr{J.Item{J.Null{}, J.Item{J.Flag{False{}}, J.Item{J.Num{"0"}, J.Item{J.Str{""}, J.Item{J.Obj{J.Field{"", J.Null{}, J.End{}}}, J.End{}}}}}}}

def show(j: J.Json) -> String:
  J.Json.show(j)

# the hub's codec: a client's frame (app.bend's wire_out_json) and the hub's
# reading of it, then the line its log would keep and that line read back
def wire_out_json(j: J.Json) -> List<&2, U32>:
  C.Cbor.encode(j)

def wire_in(bs: List<&2, U32>) -> J.Json:
  Maybe.default(&2, J.Json, C.Cbor.decode(bs), J.Null{})
