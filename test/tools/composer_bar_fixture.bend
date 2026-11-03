import Base
import ../../src/core/i18n.bend as I
import ../../src/web/dom.bend as D
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/web/view.bend as V
import ../../src/web/presentation.bend as P

def main() -> IO(Unit):
  +st = M.State.replay([
    M.ProjectCreated{"p", "Composer", "/tmp/composer-fixture", 1n},
    M.ThreadCreated{"t", "p", "Composer", M.Local{}, "", "", "claude", "", 1n},
    M.SettingSet{"allowance.claude", "{\"provider\":\"claude\",\"at\":100,\"state\":\"ok\",\"windows\":[{\"name\":\"5 h\",\"scope\":\"shared\",\"used_permille\":90,\"duration_minutes\":300,\"resets_at\":10000,\"shared\":true}]}", 100n}], M.State.empty())
  +u = U.Ui.info_set(U.Ui.select.k(U.Ui.adopt(U.Ui.init(100n), st), "t"), "clock", "100")
  IO.print(J.Json.show(J.Json.obj([
    J.KV{"composer", P.Node.write(V.View.composer.u(u))},
    J.KV{"star", P.Node.write(D.El{"div", "", Nil{}, V.View.star.k(True{}, "fixture")})},
    J.KV{"reply", P.Node.write(D.El{"div", "", Nil{}, V.View.rpl.chip.k(I.En{}, "message", "A long reply banner repeated: A long reply banner repeated: A long reply banner repeated")})}])))
