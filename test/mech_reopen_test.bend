import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/mobile/view.bend as MV
import ../src/core/mech.bend as MC

# Mech re-checks its parts each time it opens (client.bend's Mpage.open,
# Mpage.to3d, Mpage.sync). Opening it asked for them again, but showed the
# part the last listing gave, so a part that had gone stayed on show; and
# a thread whose viewer was on Mech, chosen again, or the viewer opened
# again by hand, showed the parts it had (the parts were asked for only
# when the thread or its view changed since the last ask). Now what the
# fresh listing gives is shown when it comes, the part on show staying as
# it is while it is still there, and each of those openings lists them.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "proj", "/m", 100n},
   M.ThreadCreated{"a", "p1", "Parts", M.Local{}, "", "", "claude", "", 300n},
   M.ThreadCreated{"b", "p1", "Other", M.Local{}, "", "", "claude", "", 310n}]

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

# how many times the parts are asked for
def asks.go(ls: List<&2, String>) -> Nat:
  match ls:
    case Nil{}:
      0n
    case +l <> t:
      Nat.add(Bool.pick(Nat, String.contains(l, "\"m\":\"mech.list\""), 1n, 0n), asks.go(t))

def asks(+a: U.Act) -> String:
  Nat.show(asks.go(String.lines(sent(a))))

def opened() -> U.Ui:
  U.Bots.sel.set(U.Ui.fold(log(), U.Ui.init(1000n)), "a")

def listing(+th: String, +text: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"1"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{th}}, J.KV{"root", J.Str{"/m"}}, J.KV{"mech", J.Str{text}}])}])

def got(+u: U.Ui, +text: String) -> U.Ui:
  U.Ui.reply(u, listing("a", text))

def ab() -> String:
  "1 9 mech/a.step\n2 9 mech/b.step"

# what the viewer shows
def shows(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.kind") ++ "|" ++ U.Ui.view_get(u, "view.path")

# Mech opened, its parts a and b listed: a in 3D
def first() -> U.Ui:
  got(ui(U.Ui.act(opened(), "mech", "")), ab())

# on b, the viewer shut, then Mech again (the rail's Mech, the viewer shut)
def again() -> U.Act:
  U.Ui.act(ui(U.Ui.act(ui(U.Ui.act(first(), "mech-part", "mech/b.step")), "panel", "")), "mech", "")

def paths(ps: List<&2, MC.MPart>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      MC.Mech.part.path(p) <> paths(t)

def main() -> IO(Unit):
  +a1 = U.Ui.act(opened(), "mech", "")
  +u2 = ui(again())
  do IO<Unit>:
    C.Check.str("Mech asks for its parts", asks(a1), "1")
    C.Check.str("shows the first once they are in", shows(first()), "3d|/m/mech/a.step")
    C.Check.str("opened again it asks again, once", asks(again()), "1")
    C.Check.str("from the Board too", asks(U.Ui.act(ui(U.Ui.act(first(), "view-kind", "board")), "mech", "")), "1")
    C.Check.str("and shows the part on show at once", shows(u2), "3d|/m/mech/b.step")
    C.Check.str("still there: it stays as it is", U.Ui.view_get(got(u2, ab()), "view.n") ++ "|" ++ shows(got(u2, ab())), U.Ui.view_get(u2, "view.n") ++ "|3d|/m/mech/b.step")
    C.Check.str("a new part is listed", String.join(paths(U.Mpage.parts(got(u2, "1 9 mech/a.step\n2 9 mech/b.step\n3 9 mech/c.step"))), "|"), "mech/a.step|mech/b.step|mech/c.step")
    C.Check.str("gone: the assembly or the first shows", shows(got(u2, "1 9 mech/a.step\n3 9 mech/c.step")), "3d|/m/mech/a.step")
    C.Check.str("and is not asked for again for that", asks(U.Ui.act(got(u2, "1 9 mech/a.step"), "fold", "x")), "0")
    C.Check.str("none left: the page says so", shows(got(u2, "")), "mech|")
    C.Check.str("then one comes: it shows", shows(got(got(u2, ""), "4 9 mech/d.step")), "3d|/m/mech/d.step")
    C.Check.str("Renders picked meanwhile stays", shows(got(ui(U.Ui.act(u2, "mech-renders", "mech/b.step")), "1 9 mech/a.step")), "mech|mech/b.step")
    C.Check.str("so does a view picked meanwhile", shows(got(ui(U.Ui.act(u2, "view-kind", "board")), "1 9 mech/a.step")), "board|")
    C.Check.str("an agent's later model is left alone", shows(got(ui(U.Ui.act(got(u2, ab()), "mech-3d", "/m/other/x.step")), "1 9 mech/a.step")), "3d|/m/other/x.step")
    C.Check.str("a thread on Mech chosen again lists its parts", has(U.Ui.act(ui(U.Ui.act(first(), "select", "b")), "select", "a"), "\"m\":\"mech.list\""), "True")
    C.Check.str("the viewer opened again on Mech lists them", has(U.Ui.act(ui(U.Ui.act(first(), "panel", "")), "panel", ""), "\"m\":\"mech.list\""), "True")
    C.Check.str("shut, it asks for nothing", has(U.Ui.act(first(), "panel", ""), "\"m\":\"mech.list\""), "False")
    C.Check.str("nor while it stays on screen", has(U.Ui.act(first(), "fold", "x"), "\"m\":\"mech.list\""), "False")
    C.Check.str("a phone's Mech asks once", asks(MV.View.open(opened(), "mech")), "1")
    C.Check.str("opened again, once", asks(MV.View.open(ui(MV.View.open(got(ui(MV.View.open(opened(), "mech")), ab()), "")), "mech")), "1")
    C.Check.str("a phone watches the part the listing gives", has(MV.View.far(listing("a", "1 9 mech/b.step"), U.Ui.recv_act(ui(MV.View.open(got(ui(MV.View.open(opened(), "mech")), ab()), "mech")), listing("a", "1 9 mech/b.step"))), "/m/mech/b.step"), "True")
