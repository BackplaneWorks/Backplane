import Base
import ../../src/core/json.bend as J
import ../../src/server/conversation-frame.bend as F

def packet(+rid: J.Json, shown: J.Json) -> J.Json:
  F.Control.envelope(rid, J.Json.obj([J.KV{"shown", shown}]))

def shell(+rid: J.Json, shown: Maybe<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", rid}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", F.Control.picture(shown, rid)}])

def sentinel() -> J.Json:
  J.Num{"14073748835532914073748835532900"}
