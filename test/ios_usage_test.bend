import Base
import ./check.bend as C
import ./hubs_test.bend as F
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/mobile/hubs.bend as H

def machines(+on: Bool) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(H.Hubs.screen(H.Hubs.local(F.two(), "usage", Bool.pick(String, on, "1", ""))), "usageMachines"))

def at(xs: List<&2, J.Json>, n: Nat) -> J.Json:
  Maybe.default(&2, J.Json, List.get(&2, J.Json, xs, n), J.Null{})

def count(j: J.Json) -> Nat:
  List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(j, "rows")))

def duplicate() -> List<&2, J.Json>:
  H.Hubs.usage.machines([H.Shown{"a:3787", "desk", J.Json.obj([J.KV{"online", J.Flag{True{}}}])},
    H.Shown{"b:3787", "desk", J.Json.obj([J.KV{"online", J.Flag{False{}}}])}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("every paired machine", Nat.show(List.length(&2, J.Json, machines(True{}))), "2")
    C.Check.str("first exact identity", J.Json.get_str(at(machines(True{}), 0n), "key", ""), "100.1.1.1:3787")
    C.Check.str("second exact identity", J.Json.get_str(at(machines(True{}), 1n), "key", ""), "box:3787")
    C.Check.str("first name", J.Json.get_str(at(machines(True{}), 0n), "name", ""), "lap")
    C.Check.str("fallback machine name", J.Json.get_str(at(machines(True{}), 1n), "name", ""), "box")
    C.Check.str("usage outside settings absent", Nat.show(count(at(machines(False{}), 0n))), "0")
    C.Check.str("usage requested for first", F.yes(Nat.is_gt(count(at(machines(True{}), 0n)), 0n)), "yes")
    C.Check.str("usage requested for second", F.yes(Nat.is_gt(count(at(machines(True{}), 1n)), 0n)), "yes")
    C.Check.str("usage closes", Nat.show(count(at(machines(False{}), 1n))), "0")
    C.Check.str("duplicate names retain both", F.strs(duplicate(), "key"), "a:3787;b:3787;")
    C.Check.str("offline stays offline", F.yes(J.Json.get_bool(at(duplicate(), 1n), "online", True{})), "no")
    C.Check.str("online stays online", F.yes(J.Json.get_bool(at(duplicate(), 0n), "online", False{})), "yes")
    C.Check.str("refresh routes to owning machine", F.cmds(H.Hubs.cmds(H.Hubs.act(F.two(), "usage-refresh", "box:3787|"))), "send@box:3787 ")
    C.Check.str("refresh preserves focus", H.Hubs.cur(H.Hubs.after(H.Hubs.act(F.two(), "usage-refresh", "box:3787|"))), H.Hubs.cur(F.two()))
