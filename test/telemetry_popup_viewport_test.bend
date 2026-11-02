import Base
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/web/dom.bend as D
import ../src/web/view.bend as V
import ../src/web/presentation.bend as P

def fixture() -> String:
  +u = U.Ui.scratch_set(U.Ui.info_set(U.Ui.adopt.raw(U.Ui.init(1n), M.State.apply(M.State.empty(), M.ProjectCreated{"p", "Fixture", "/fixture", 1n})), "telemetry.live", "1"), "@sync", "1")
  J.Json.show(P.Node.write(D.Dom.div("app", V.View.tour(I.En{}, u))))
