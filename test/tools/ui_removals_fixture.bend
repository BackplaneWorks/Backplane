# Production page fixture; timestamps and provider readings are synthetic.
import Base
import ../allowance_ui_test.bend as F
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/web/view.bend as V
import ../../src/web/presentation.bend as P

def base() -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.info_set(U.Ui.info_set(F.ui(),
    "local.workspace.layout", "rail-v1"), "local.workspace.chrome", "top-v1"), "@desktop", "1"), "@desktop.browser", "1")

def scenario(+kind: String) -> U.Ui:
  +u = F.tick(base(), 1793168000n)
  Bool.pick(U.Ui, String.eq(kind, "unknown"), F.model(u, "grok"), u)

def page(+kind: String) -> String:
  +u = scenario(kind)
  J.Json.show(P.Node.write(V.View.page(Bool.pick(U.Ui, String.eq(kind, "browser"), F.act(u, "view-kind", "browser"), u))))
