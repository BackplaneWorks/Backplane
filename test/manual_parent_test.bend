import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/storedb.bend as S

# A hierarchy choice is independent of task delivery and later delegation.
# Empty values are explicit root choices, not absent parent keys.
def task(+parent: String) -> M.Change:
  M.TaskDelegated{"task-" ++ parent, parent, "child", 1n, "claude", "", 2n}

def parent(cs: List<&2, M.Change>) -> String:
  M.Setting.get(M.State.replay(cs, M.State.empty()), "thread.parent.child", "absent")

def moved() -> List<&2, M.Change>:
  [M.ThreadParentSet{"child", "manual", 1n}, task("delegator")]

def detached() -> List<&2, M.Change>:
  [M.ThreadParentSet{"child", "", 1n}, task("delegator")]

def lines(cs: List<&2, M.Change>) -> List<&2, S.LL>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest: S.LL{1n, True{}, Some{c}} <> lines(rest)

def stored.value(r: Map<&2, String> & String) -> String:
  (_, value) = r
  value

def stored(cs: List<&2, M.Change>) -> String:
  stored.value(Map.get(String, "", S.Mat.recs(S.Mat.replay(lines(cs), S.Mat.empty())), S.Tab.key("setting", ["thread.parent.child"])))

def rows(xs: List<&2, S.UpKV>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case S.UpKV{k, v} <> rest: (k ++ "=" ++ v) <> rows(rest)

def upgrade(cs: List<&2, M.Change>) -> String:
  String.join(rows(S.Up.parents(M.State.replay(cs, M.State.empty()))), "|")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("absent relationship is seeded", parent([task("delegator")]), "delegator")
    C.Check.str("manual move before delegation wins", parent(moved()), "manual")
    C.Check.str("manual detach before delegation wins", parent(detached()), "")
    C.Check.str("move after delegation survives another task", parent([task("first"), M.ThreadParentSet{"child", "manual", 3n}, task("second")]), "manual")
    C.Check.str("detach after delegation survives another task", parent([task("first"), M.ThreadParentSet{"child", "", 3n}, task("second")]), "")
    C.Check.str("repeated delegation preserves the conversation seed", parent([task("first"), task("second")]), "first")
    C.Check.str("a later manual move still replaces the earlier choice", parent(List.append(&2, M.Change, moved(), [M.ThreadParentSet{"child", "new-manual", 4n}, task("later")])), "new-manual")
    C.Check.str("direct legacy parent setting wins", parent([M.SettingSet{"thread.parent.child", "legacy", 1n}, task("delegator")]), "legacy")
    C.Check.str("direct legacy empty setting stays detached", parent([M.SettingSet{"thread.parent.child", "", 1n}, task("delegator")]), "")
    C.Check.str("bot work does not seed a relationship", parent([M.BotSet{"b", "Bot", "delegator", 1n, "", 1n}, task("delegator")]), "absent")
    C.Check.str("bot work preserves a manual relationship", parent([M.ThreadParentSet{"child", "manual", 0n}, M.BotSet{"b", "Bot", "delegator", 1n, "", 1n}, task("delegator")]), "manual")
    C.Check.str("empty parent key remains present", Bool.show(M.Setting.has(M.State.replay(detached(), M.State.empty()), "thread.parent.child")), "True")
    C.Check.str("unrelated setting does not block a seed", parent([M.SettingSet{"thread.parentx.child", "unrelated", 1n}, task("delegator")]), "delegator")
    C.Check.str("checkpoint preserves manual move before delegation", stored(moved()), "thread.parent.child\u{1f}manual\u{1f}0")
    C.Check.str("checkpoint preserves detach before delegation", stored(detached()), "thread.parent.child\u{1f}\u{1f}0")
    C.Check.str("checkpoint retains detach through repeated delivery", stored([task("first"), M.ThreadParentSet{"child", "", 3n}, task("second"), M.TaskStateSet{"task-second", M.TaskDone{}, "done", 4n}, M.TaskDelivered{"task-second", 5n}]), "thread.parent.child\u{1f}\u{1f}0")
    C.Check.str("filtered upgrade equals full replay", upgrade(S.Up.keep.all(S.Up.demo())), upgrade(S.Up.demo()))
    C.Check.str("approved upgrade matrix retains all seven expectations", upgrade(S.Up.demo()), "thread.parent.c1=tp|thread.parent.c2=|thread.parent.c3=tq|thread.parent.c4=tx|thread.parent.c5=ty|thread.parent.c6=tz|thread.parent.c7=tb")
