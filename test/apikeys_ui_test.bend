import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/mobile/screen.bend as MB
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as WP

# Settings > API keys (client.bend's ApiKeys): Delete asks first (the same
# click again deletes, as Delete thread does), and a key kept shows in the
# list at once, by name only.

def t_ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def cmds(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{t} <> rest: "send " ++ t ++ ";" ++ cmds(rest)
    case _ <> rest: "other;" ++ cmds(rest)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}: cmds(cs)

# two keys kept, as the hub's status answers them
def kept() -> U.Ui:
  U.Ui.scratch_set(U.Ui.flag_on(U.Ui.init(1n), "keys"), "@keys", "OPENAI_API_KEY\nE2E_KEY")

def text(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

def phone(+u: U.Ui) -> String:
  J.Json.show(J.Json.arr(MB.Mob.keys.list.loc(I.En{}, True{}, u)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Delete once: nothing is sent", sent(U.Ui.act(kept(), "key-del", "E2E_KEY")), "")
    C.Check.str("Delete once: it asks", Bool.show(U.ApiKeys.arming(t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY")), "E2E_KEY")), "True")
    C.Check.str("Delete again: the key goes", Bool.show(String.contains(sent(U.Ui.act(t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY")), "key-del", "E2E_KEY")), "\"op\":\"api-del\",\"name\":\"E2E_KEY\"")), "True")
    C.Check.str("Delete again: asked no more", Bool.show(U.ApiKeys.arming(t_ui(U.Ui.act(t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY")), "key-del", "E2E_KEY")), "E2E_KEY")), "False")
    C.Check.str("another key's Delete asks for that one", sent(U.Ui.act(t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY")), "key-del", "OPENAI_API_KEY")), "")
    C.Check.str("another key's Delete: it is the one asked", Bool.show(U.ApiKeys.arming(t_ui(U.Ui.act(t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY")), "key-del", "OPENAI_API_KEY")), "OPENAI_API_KEY")), "True")
    C.Check.str("web: Delete asks to be clicked again", Bool.show(String.contains(text(V.View.keys.set(I.En{}, t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY")))), "Click again to delete")), "True")
    C.Check.str("web: only the key pressed asks", Bool.show(String.contains(text(V.View.keys.set(I.En{}, kept())), "Click again to delete")), "False")
    C.Check.str("phone: Delete asks to be pressed again", Bool.show(String.contains(phone(t_ui(U.Ui.act(kept(), "key-del", "E2E_KEY"))), "Click again to delete")), "True")
    # Save: the form shuts and the list opens, so the new name shows
    +form : U.Ui = U.Ui.flag_off(U.Ui.flag_on(U.Ui.init(1n), "keyform"), "keys")
    +saved : U.Ui = t_ui(U.Ui.act(form, "sealed-sent", "key"))
    C.Check.str("saved: the form shuts", Bool.show(U.Ui.flag(saved, "keyform")), "False")
    C.Check.str("saved: the list opens", Bool.show(U.Ui.flag(saved, "keys")), "True")
    C.Check.str("saved: the names are asked for", Bool.show(String.contains(sent(U.Ui.act(form, "sealed-sent", "key")), "\"op\":\"status\"")), "True")
    C.Check.str("saved: the new name listed once the hub answers", Bool.show(String.contains(text(V.View.keys.set(I.En{}, U.Ui.scratch_set(saved, "@keys", "E2E_KEY"))), "E2E_KEY")), "True")
    C.Check.str("a phone's Save opens the list too", Bool.show(U.Ui.flag(t_ui(U.Ui.act(form, "key-add", "")), "keys")), "True")
