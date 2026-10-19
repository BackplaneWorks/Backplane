import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/app/layout.bend as L
import ../../src/app/nui.bend as N

def ui(+text: String) -> U.Ui:
  U.Ui.info_set(U.Ui.init(10n), "models", text)

def count(o: L.Out) -> Nat:
  match o:
    case L.Out{_, hs}: List.length(&2, N.Hit, hs)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- IO.args()
    +text : String = Maybe.default(&1, String, List.head(&1, String, args), "{}")
    IO.print(Nat.show(List.length(&2, U.AgRow, U.Fallbackv.rows(ui(text)))))
