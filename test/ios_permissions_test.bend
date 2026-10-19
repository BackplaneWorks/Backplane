import Base
import ./check.bend as C
import ./hubs_test.bend as F
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/codec.bend as K
import ../src/core/i18n.bend as I
import ../src/mobile/permissions.bend as P
import ../src/mobile/hubs.bend as H

def selected() -> H.Hubs:
  F.act(F.two(), "select", "box:3787|t2")

def request(cs: List<&2, H.HCmd>) -> J.Json:
  match cs:
    case H.HCmd{_, U.Send{text}} <> _:
      Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    case _: J.Null{}

def changed() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(selected(), "box:3787", F.changes([M.ModesSet{"t2", M.Supervised{}, M.DefaultMode{}, "", 200n}])))

def thread.mode(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, r, _, _, _}: K.Runtime.name(r)

def mode(+h: H.Hubs, +key: String, id: String) -> String:
  thread.mode(U.Ui.thread(U.Ui.state(H.Hubs.ui(h, key)), id))

def command(+value: String) -> J.Json:
  request(H.Hubs.cmds(H.Hubs.act(selected(), "thread-permissions", value)))

def choices(lc: I.Loc) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(P.Permissions.json(lc, M.Supervised{}), "choices"))

def displayed(+value: String) -> J.Json:
  H.Q.all(J.Json.obj([J.KV{"permissions", P.Permissions.json(I.En{}, M.FullAccess{})}]), "box:3787")

def displayed.choice() -> J.Json:
  Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(displayed(""), "permissions"), "choices"))), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("screen qualifier preserves permission protocol token", J.Json.get_str(displayed.choice(), "value", ""), "approval-required")
    C.Check.str("displayed permission choice produces real RPC", J.Json.get_str(command(J.Json.get_str(displayed.choice(), "value", "")), "m", ""), "thread.modes")
    C.Check.str("normal thread modes method", J.Json.get_str(command("approval-required"), "m", ""), "thread.modes")
    C.Check.str("selected thread only", J.Json.get_str(J.Json.get_obj(command("approval-required"), "p"), "thread", ""), "t2")
    C.Check.str("exact selected mode", J.Json.get_str(J.Json.get_obj(command("approval-required"), "p"), "runtime", ""), "approval-required")
    C.Check.str("owning hub routing", F.cmds(H.Hubs.cmds(H.Hubs.act(selected(), "thread-permissions", "auto"))), "send@box:3787 ")
    C.Check.str("unknown mode rejected", F.cmds(H.Hubs.cmds(H.Hubs.act(selected(), "thread-permissions", "superuser"))), "")
    C.Check.str("empty mode rejected", F.cmds(H.Hubs.cmds(H.Hubs.act(selected(), "thread-permissions", ""))), "")
    C.Check.str("no selected thread rejected", Nat.show(List.length(&2, U.Cmd, U.Act.cmds(P.Permissions.set(U.Ui.init(0n), "auto")))), "0")
    C.Check.str("auto edits exact", J.Json.get_str(J.Json.get_obj(command("auto-accept-edits"), "p"), "runtime", ""), "auto-accept-edits")
    C.Check.str("full access exact", J.Json.get_str(J.Json.get_obj(command("full-access"), "p"), "runtime", ""), "full-access")
    C.Check.str("default remains full access", mode(selected(), "box:3787", "t2"), "full-access")
    C.Check.str("no optimistic mode change", mode(H.Hubs.after(H.Hubs.act(selected(), "thread-permissions", "approval-required")), "box:3787", "t2"), "full-access")
    C.Check.str("persisted event shown", mode(changed(), "box:3787", "t2"), "approval-required")
    C.Check.str("other thread and machine unchanged", mode(changed(), "100.1.1.1:3787", "t1"), "full-access")
    C.Check.str("navigation keeps stored mode", mode(F.act(F.act(changed(), "select", "100.1.1.1:3787|t1"), "select", "box:3787|t2"), "box:3787", "t2"), "approval-required")
    C.Check.str("four existing choices", Nat.show(List.length(&2, J.Json, choices(I.En{}))), "4")
    C.Check.str("only persisted choice selected", J.Json.get_str(P.Permissions.json(I.En{}, M.Supervised{}), "current", ""), "Permissions: Approval required")
    C.Check.str("Japanese selected label translated", J.Json.get_str(P.Permissions.json(I.Ja{}, M.Supervised{}), "current", ""), "権限：承認が必要")
