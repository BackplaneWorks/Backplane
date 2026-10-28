import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# An open Diff follows its thread (client.bend's Dv.*): a diff asked for
# by hand opens the viewer and notes what it was asked at; Diff asks again
# by itself only once the thread moved on, and its own answer only puts
# the text in place; a folder that is no git repository is named.

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def on(u: U.Ui, +id: String) -> U.Ui:
  ui(U.Ui.act(u, "select", id))

def reply(+diff: String, +nogit: String) -> J.Json:
  json("{\"t\":\"reply\",\"id\":7,\"ok\":true,\"result\":{\"diff\":\"" ++ diff ++ "\",\"thread\":\"a\",\"nogit\":\"" ++ nogit ++ "\"}}")

def sends(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{t} <> _}:
      t
    case _:
      ""

def follow(+u: U.Ui) -> U.Act:
  U.Dv.sync(U.Act{u, Nil{}})

def main() -> IO(Unit):
  +a = on(U.Ui.init(0n), "a")
  +shown = U.Ui.recv(a, reply("x", ""))
  +same = U.Ui.info_set(shown, "x", "")
  +moved = U.Ui.info_set(shown, "diff.at:a", "running|0")
  +asked = ui(follow(moved))
  +shut = ui(U.Ui.act(asked, "panel", ""))
  +quiet = U.Ui.recv(shut, reply("y", ""))
  do IO<Unit>:
    C.Check.str("a hand diff opens its viewer", Bool.show(U.Ui.panel(shown)) ++ "|" ++ U.Ui.view_get(shown, "view.kind") ++ "|" ++ U.Ui.view_get(shown, "diff"), "True|diff|x")
    C.Check.str("it notes what it was asked at", Bool.show(String.is_empty(U.Ui.info(shown, "diff.at:a"))), "False")
    C.Check.str("nothing moved: no ask", sends(follow(same)), "")
    C.Check.str("the thread moved on: Diff asks again", Bool.show(String.contains(sends(follow(moved)), "git.diff")), "True")
    C.Check.str("its own ask is marked", U.Ui.info(asked, "diff.auto:a"), "1")
    C.Check.str("asked once", sends(follow(asked)), "")
    C.Check.str("its answer leaves a shut panel shut", Bool.show(U.Ui.panel(quiet)), "False")
    C.Check.str("its answer puts the text in place", U.Ui.info(quiet, "diff:a") ++ "|" ++ U.Ui.info(quiet, "diff.auto:a"), "y|")
    C.Check.str("a shut panel asks nothing", sends(follow(U.Ui.info_set(shut, "diff.at:a", "old"))), "")
    C.Check.str("a hand ask clears the mark", U.Ui.info(ui(U.Ui.act(asked, "diff", "")), "diff.auto:a"), "")
    C.Check.str("no repository: the folder is named", U.Ui.info(U.Ui.recv(a, reply("", "/p")), "diff.none:a") ++ "|" ++ Bool.show(U.Ui.panel(U.Ui.recv(a, reply("", "/p")))), "/p|True")
    C.Check.str("a repository again clears it", U.Ui.info(U.Ui.recv(U.Ui.recv(a, reply("", "/p")), reply("z", "")), "diff.none:a"), "")
