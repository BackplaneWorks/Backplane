import Base
import ./fixtures/workspace-rail.bend as F
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/server/thin-view.bend as S

def scenario.ui(step: Nat, u: U.Ui) -> U.Ui:
  match step:
    case 0n: u
    case 1n: F.stream(u, "Checking the layout…")
    case 2n: U.Ui.fold([M.TurnChanged{"t", M.Idle{}, 21n}], u)
    case 3n: F.step(u, "bot-tab", "memory")
    case 4n: F.step(u, "bot-tab", "chat")
    case 5n: F.step(u, "view-kind", "board")
    case _: F.step(u, "draft", "Keep this unsent draft")

# Production bot DOM across the changes that rebuild the chat around its
# chrome. The driver also exercises the production DOM patcher.
def scenario(+step: Nat, rail: Bool) -> String:
  +u = S.Session.renderer(True{}, F.start(True{}, ""), J.Json.obj([
    J.KV{"workspaceLayout", J.Str{Bool.pick(String, rail, "rail-v1", "legacy")}},
    J.KV{"workspaceChrome", J.Str{"top-v1"}}]))
  F.frame(False{}, True{}, scenario.ui(step, u))
