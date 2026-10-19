import ../src/core/i18n.bend as I
import ../src/mobile/view.bend as MV
import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# The viewer belongs to a thread: a viewer_show lands in the viewer of the
# thread that asked, and each thread keeps its own, open or shut, with its
# own browser page.

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def view(+th: String, +path: String) -> J.Json:
  json("{\"t\":\"view\",\"thread\":\"" ++ th ++ "\",\"path\":\"" ++ path ++ "\",\"kind\":\"\"}")

def on(u: U.Ui, +id: String) -> U.Ui:
  ui(U.Ui.act(u, "select", id))

def log(+origin: String, +since: String) -> J.Json:
  json("{\"t\":\"log\",\"since\":" ++ since ++ ",\"origin\":\"" ++ origin ++ "\",\"info\":{},\"items\":[]}")

def diffs() -> J.Json:
  json("{\"t\":\"reply\",\"id\":1}")

def sends(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{t} <> _}:
      t
    case _:
      ""

def main() -> IO(Unit):
  +a = on(U.Ui.init(0n), "a")
  +shown = U.Ui.recv(a, view("a", "/p/a.kicad_pcb"))
  +other = U.Ui.recv(a, view("b", "/p/b.kicad_sch"))
  +both = U.Ui.recv(shown, view("b", "/p/b.kicad_sch"))
  +old = U.Ui.recv(a, json("{\"t\":\"view\",\"path\":\"/p/x.glb\",\"kind\":\"\"}"))
  +picked = U.Ui.view_pick(both, "schematic")
  +joined = U.Ui.recv(on(U.Ui.recv(U.Ui.init(0n), log("O", "0")), "a"), view("a", "/p/a.kicad_pcb"))
  +hist = U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(joined, "diff:a", "x.diff"), "hist.at:a", "3"), "fm.list:a", "stale")
  +shut = ui(U.Ui.act(joined, "panel", ""))
  do IO<Unit>:
    C.Check.str("full reconnect keeps the viewer", U.Ui.view_get(U.Ui.recv(joined, log("O", "0")), "view.path"), "/p/a.kicad_pcb")
    C.Check.str("incremental reconnect keeps the viewer", U.Ui.view_get(U.Ui.recv(joined, log("O", "4")), "view.path"), "/p/a.kicad_pcb")
    C.Check.str("reconnect keeps the panel open", Bool.show(U.Ui.panel(U.Ui.recv(joined, log("O", "4")))), "True")
    C.Check.str("reconnect keeps the diff", U.Ui.info(U.Ui.recv(hist, log("O", "4")), "diff:a"), "x.diff")
    C.Check.str("reconnect keeps the history step", U.Ui.info(U.Ui.recv(hist, log("O", "4")), "hist.at:a"), "3")
    C.Check.str("the relist names the folder", Bool.show(String.contains(sends(U.Ui.recv_act(U.Ui.info_set(hist, "fm.want:a", "src"), log("O", "4"))), "files.dir")), "True")
    C.Check.str("no folder, no relist", sends(U.Ui.recv_act(hist, log("O", "4"))), "")
    C.Check.str("reconnect drops a stale listing", U.Ui.info(U.Ui.recv(hist, log("O", "4")), "fm.list:a"), "")
    C.Check.str("a hand-shut panel stays shut after reconnect", Bool.show(U.Ui.panel(U.Ui.recv(shut, log("O", "4")))), "False")
    C.Check.str("an auto view after reconnect respects the hand", Bool.show(U.Ui.panel(U.Ui.recv(U.Ui.recv(shut, log("O", "4")), json("{\"t\":\"view\",\"thread\":\"a\",\"path\":\"/p/b.kicad_pcb\",\"kind\":\"\",\"auto\":true}")))), "False")
    C.Check.str("another hub's log drops the viewer", U.Ui.view_get(U.Ui.recv(joined, log("P", "0")), "view.path"), "")
    C.Check.str("own thread shows", U.Ui.view_get(shown, "view.path"), "/p/a.kicad_pcb")
    C.Check.str("own thread opens", Bool.show(U.Ui.panel(shown)), "True")
    C.Check.str("other thread waits", U.Ui.view_get(other, "view.path"), "")
    C.Check.str("other thread stays shut", Bool.show(U.Ui.panel(other)), "False")
    C.Check.str("each keeps its own", U.Ui.view_get(both, "view.path"), "/p/a.kicad_pcb")
    C.Check.str("switching shows the other's", U.Ui.view_get(on(both, "b"), "view.path"), "/p/b.kicad_sch")
    C.Check.str("switching back", U.Ui.view_get(on(on(both, "b"), "a"), "view.path"), "/p/a.kicad_pcb")
    C.Check.str("unnamed goes to selected", U.Ui.view_get(old, "view.path"), "/p/x.glb")
    C.Check.str("picked kind clears path", U.Ui.view_get(picked, "view.kind") ++ "|" ++ U.Ui.view_get(picked, "view.path"), "schematic|")
    C.Check.str("picked for this thread only", U.Ui.view_get(on(picked, "b"), "view.path"), "/p/b.kicad_sch")
    C.Check.str("asking again counts", U.Ui.view_get(U.Ui.recv(shown, view("a", "/p/a.kicad_pcb")), "view.n"), "2")
    C.Check.str("panel shuts going to a thread without one", Bool.show(U.Ui.panel(on(shown, "c"))), "False")
    C.Check.str("panel comes back with its thread", Bool.show(U.Ui.panel(on(on(shown, "c"), "a"))), "True")
    C.Check.str("closed stays closed for its thread", Bool.show(U.Ui.panel(on(on(ui(U.Ui.act(shown, "panel", "")), "c"), "a"))), "False")
    C.Check.str("a request waits open in its thread", Bool.show(U.Ui.panel(on(other, "b"))), "True")
    C.Check.str("each thread has its own page", U.Ui.page(on(a, "b")), "b")
    C.Check.str("thread input targets its own page", U.Teach.owner(on(a, "b")), "b")
    C.Check.str("thread takeover uses that page", J.Json.get_str(J.Json.get_obj(json(sends(U.Ui.act(on(a, "b"), "tk-takeover", ""))), "p"), "bot", ""), "b")
    C.Check.str("thread typing uses that page", J.Json.get_str(J.Json.get_obj(json(sends(U.Ui.act(on(a, "b"), "bb-type", "hello"))), "p"), "bot", ""), "b")
    C.Check.str("bot input keeps bot ownership", U.Teach.owner(U.Ui.scratch_set(a, "@bot", "bot-x")), "bot-x")
    C.Check.str("phone browser is visible", J.Json.get_str(MV.View.json(I.En{}, ui(MV.View.open.k(on(a, "b"), "browser")), J.Null{}), "open", ""), "browser")
    C.Check.str("phone browser targets thread", J.Json.get_str(J.Json.get_obj(MV.View.json(I.En{}, ui(MV.View.open.k(on(a, "b"), "browser")), J.Null{}), "browser"), "url", ""), "/bots/b/screen.jpg")
