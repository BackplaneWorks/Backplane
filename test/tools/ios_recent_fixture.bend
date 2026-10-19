import Base
import ../../src/core/model.bend as M
import ../hubs_test.bend as F
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/mobile/hubs.bend as H
import ../../src/mobile/session.bend as S

def render(text: String) -> String:
  J.Json.show(S.Session.render(M.State.apply(U.Ui.state(H.Hubs.ui(F.two(), "box:3787")), M.MessagePosted{"t2", "raw-message", M.Assistant{}, "one paragraph", 200n}),
    J.Json.obj([J.KV{"machines", J.Json.str(F.machines("box"))}]),
    J.Json.obj([J.KV{"t", J.Json.str("live")}, J.KV{"items", J.Json.arr([])}]), 900n,
    Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))
