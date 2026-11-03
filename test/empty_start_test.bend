import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as WP

# The main pane with nothing picked (web/view.bend's View.empty): with no
# project yet it offers Add project as a button (the folder picker), not
# only a hint to find the sidebar's +; once there are projects it does not.

def html(n: D.Node) -> String:
  J.Json.show(WP.Node.write(n))

def offers(+n: Nat) -> Bool:
  String.contains(html(V.View.empty(n, Map.new(&2, String), M.State.empty())), "proj-open")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("no project: an Add project button", Bool.show(offers(0n)), "True")
    C.Check.str("projects: no Add project button", Bool.show(offers(2n)), "False")
    C.Check.str("agent setup stays", Bool.show(String.contains(html(V.View.empty(0n, Map.new(&2, String), M.State.empty())), "provider-setup-open")), "True")
