import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/mobile/view.bend as MV

# The board picker reads the project's boards again each time it opens
# (client.bend's Bd.look, "board-list"; the web's selector runs it as it
# opens, a phone as its viewer opens on a board): the card, which used to
# be the picker's only source, is asked again only when the thread or its
# turn moves on, so a board added or removed meanwhile never showed. The
# hub's answer (boards.list, the job "boards": core/order.bend's
# Boards.list) is kept where the card keeps it; the same answer again
# changes nothing.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "multi", "/m", 100n},
   M.ThreadCreated{"a", "p1", "Boards", M.Local{}, "", "", "claude", "", 300n}]

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.all(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> r:
      t ++ "\n" ++ sent.all(r)
    case _ <> r:
      sent.all(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.all(cs)

def has(+a: U.Act, +s: String) -> String:
  Bool.show(String.contains(sent(a), s))

def opened() -> U.Ui:
  U.Bots.sel.set(U.Ui.fold(log(), U.Ui.init(1000n)), "a")

def strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Str{x} <> strs(t)

# the hub's answer to boards.list
def answer(+th: String, xs: List<&2, String>, +bd: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"7"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{th}}, J.KV{"board_list", J.Json.arr(strs(xs))}, J.KV{"board_default", J.Str{bd}}])}])

def two() -> U.Ui:
  U.Ui.reply(opened(), answer("a", ["boards/alpha/alpha.kicad_pcb", "boards/beta/beta.kicad_pcb"], "boards/alpha/alpha.kicad_pcb"))

def three() -> U.Ui:
  U.Ui.reply(two(), answer("a", ["boards/alpha/alpha.kicad_pcb", "boards/beta/beta.kicad_pcb", "boards/gamma/gamma.kicad_pcb"], "boards/alpha/alpha.kicad_pcb"))

def files(+u: U.Ui) -> String:
  String.join(U.Bd.files(u), "|")

# what the viewer shows, and what the picker offers
def seen(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.kind") ++ "|" ++ U.Ui.view_get(u, "view.path") ++ "|" ++ U.Ui.view_get(u, "view.n") ++ "|" ++ files(u) ++ "|" ++ U.Ui.info(u, "fab.board:a")

def picked() -> U.Ui:
  ui(U.Ui.act(three(), "board-pick", "boards/beta/beta.kicad_pcb"))

# the card's answer (fab.look), whose boards are its own "board_files"
def card() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"8"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{"a"}}, J.KV{"goal", J.Str{""}}, J.KV{"board_files", J.Json.arr(strs(["x/x.kicad_pcb", "y/y.kicad_pcb"]))}, J.KV{"board_default", J.Str{"x/x.kicad_pcb"}}])}])

def out.one(o: H.Out) -> String:
  match o:
    case H.Git{_, _, job, th, cwd, _, _}:
      "git:" ++ job ++ ":" ++ th ++ "@" ++ cwd
    case _:
      "other"

def outs(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}:
      ""
    case o <> t:
      out.one(o) ++ " " ++ outs(t)

def hub(+text: String) -> String:
  String.trim(outs(H.Step.outs_of(H.Hub.request_text(M.State.replay(log(), M.State.empty()), 400n, 1, text))))

def labels(js: List<&2, J.Json>) -> String:
  match js:
    case Nil{}:
      ""
    case +j <> t:
      J.Json.get_str(j, "label", "") ++ Bool.pick(String, J.Json.get_bool(j, "on", False{}), "*", "") ++ "|" ++ labels(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("opening the picker asks for the boards", has(U.Ui.act(opened(), "board-list", ""), "\"m\":\"boards.list\",\"p\":{\"thread\":\"a\"}"), "True")
    C.Check.str("each time it opens", has(U.Ui.act(two(), "board-list", ""), "boards.list"), "True")
    C.Check.str("never for a thread elsewhere", has(U.Ui.act(U.Bots.sel.set(opened(), "mate~a"), "board-list", ""), "boards.list"), "False")
    C.Check.str("nor with none chosen", has(U.Ui.act(U.Bots.sel.set(opened(), ""), "board-list", ""), "boards.list"), "False")
    C.Check.str("the answer is what the picker offers", files(two()), "boards/alpha/alpha.kicad_pcb|boards/beta/beta.kicad_pcb")
    C.Check.str("a board added shows", files(three()), "boards/alpha/alpha.kicad_pcb|boards/beta/beta.kicad_pcb|boards/gamma/gamma.kicad_pcb")
    C.Check.str("a board removed goes", files(U.Ui.reply(three(), answer("a", ["boards/alpha/alpha.kicad_pcb", "boards/gamma/gamma.kicad_pcb"], "boards/alpha/alpha.kicad_pcb"))), "boards/alpha/alpha.kicad_pcb|boards/gamma/gamma.kicad_pcb")
    C.Check.str("the same answer again changes nothing", seen(U.Ui.reply(picked(), answer("a", ["boards/alpha/alpha.kicad_pcb", "boards/beta/beta.kicad_pcb", "boards/gamma/gamma.kicad_pcb"], "boards/alpha/alpha.kicad_pcb"))), seen(picked()))
    C.Check.str("the board picked stays on show when another is added", U.Ui.view_get(U.Ui.reply(picked(), answer("a", ["boards/alpha/alpha.kicad_pcb", "boards/beta/beta.kicad_pcb", "boards/delta/delta.kicad_pcb"], "boards/alpha/alpha.kicad_pcb")), "view.path"), "/m/boards/beta/beta.kicad_pcb")
    C.Check.str("another thread's answer is kept for it", files(U.Ui.reply(two(), answer("b", ["z/z.kicad_pcb"], ""))), "boards/alpha/alpha.kicad_pcb|boards/beta/beta.kicad_pcb")
    C.Check.str("the card's answer is the card's", files(U.Ui.reply(opened(), card())), "x/x.kicad_pcb|y/y.kicad_pcb")
    C.Check.str("the hub reads them in the thread's folder", hub("{\"id\":1,\"m\":\"boards.list\",\"p\":{\"thread\":\"a\"}}"), "git:boards:a@/m")
    C.Check.str("a phone's viewer opened on a board asks", has(MV.View.open.k(opened(), "board"), "boards.list"), "True")
    C.Check.str("and on a schematic", has(MV.View.open.k(opened(), "schematic"), "boards.list"), "True")
    C.Check.str("a tab after it is the same opening", has(MV.View.open.k(ui(MV.View.open.k(opened(), "board")), "schematic"), "boards.list"), "False")
    C.Check.str("opened again after Files asks again", has(MV.View.open.k(ui(MV.View.open.k(ui(MV.View.open.k(opened(), "board")), "files")), "board"), "boards.list"), "True")
    C.Check.str("Mech lists no boards", has(MV.View.open.k(opened(), "mech"), "boards.list"), "False")
    C.Check.str("the phone's menu offers the boards found", labels(MV.View.board.list(ui(MV.View.open.k(three(), "board")))), "alpha*|beta|gamma|")
