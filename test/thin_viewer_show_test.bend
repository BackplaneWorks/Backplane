import Base
import ./check.bend as C
import ./thin_terminal_input_test.bend as F
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# A view the hub sends (an agent's viewer_show, the board an agent starting
# in a KiCad project opens) reaches a hub-drawn page's Ui: the panel opens
# on it, and the view is drawn again.

def actor(+u: U.Ui) -> A.Actor:
  A.Actor.make(u, "v", V.Target.json("thread", "a"))

def heard(+u: U.Ui, +msg: String) -> A.Actor:
  A.Actor.message(actor(u), True{}, "h", F.obj(msg))

def shown() -> String: "{\"t\":\"view\",\"thread\":\"a\",\"kind\":\"board\",\"path\":\"/tmp/b.kicad_pcb\"}"
def auto() -> String: "{\"t\":\"view\",\"thread\":\"a\",\"kind\":\"board\",\"path\":\"/tmp/b.kicad_pcb\",\"auto\":true}"

# the page's next request, its viewer shut (the hub's reply tells it otherwise)
def asked() -> J.Json:
  F.obj("{\"target\":{\"kind\":\"thread\",\"id\":\"a\"},\"session\":{\"sel\":\"a\",\"rid\":100,\"drafts\":{},\"open\":{},\"info\":{},\"panel\":false}}")

def again(+a: A.Actor) -> A.Actor:
  A.Actor.refresh([a], True{}, U.Ui.state(F.ui()), F.obj("{}"), F.obj("{}"), 2n, asked(), "v")

def open(+a: A.Actor) -> String:
  Bool.show(U.Ui.panel(A.Actor.ui(a)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("viewer_show opens the panel", Bool.show(U.Ui.panel(A.Actor.ui(heard(F.ui(), shown())))), "True")
    C.Check.str("on that file", U.Ui.info(A.Actor.ui(heard(F.ui(), shown())), "view.path:a"), "/tmp/b.kicad_pcb")
    C.Check.str("and the page is drawn again", Bool.show(A.Actor.dirty(heard(F.ui(), shown()))), "True")
    C.Check.str("an agent's start opens the board", Bool.show(U.Ui.panel(A.Actor.ui(heard(F.ui(), auto())))), "True")
    C.Check.str("but not where the person shut it by hand", Bool.show(U.Ui.panel(A.Actor.ui(heard(U.Act.ui(U.Ui.toggle_panel(U.Act.ui(U.Ui.toggle_panel(F.ui())))), auto())))), "False")
    C.Check.str("the page's next request keeps the viewer the hub opened", open(again(heard(F.ui(), shown()))), "True")
    C.Check.str("once: after that the page's own choice wins", open(again(again(heard(F.ui(), shown())))), "False")
    C.Check.str("with no view from the hub the page's choice stands", open(again(actor(F.ui()))), "False")
