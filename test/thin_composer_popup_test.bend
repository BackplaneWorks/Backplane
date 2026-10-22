import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/server/thin-actor.bend as TA

# The composer's popup in a thin view: the hub renders it from the draft the
# request brought (view.update carries it), with no Ui.draft having run.
# Each trigger offers rows: @ bots, > threads, % projects, # rooms, $ the
# thread's skills (the hub lists them into the info: server.bend's
# Hub.thin.info), / slash commands. A fresh actor (a hot boot's first view)
# and one refreshed from an earlier draft both offer them.

def obj( +s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def check( +name: String, +yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(yes), "True")

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "Alpha board", "/w/alpha", 1n}, M.ProjectCreated{"p2", "Beta enclosure", "/w/beta", 1n},
    M.ThreadCreated{"t1", "p1", "Route the USB connector", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"t2", "p1", "Fix the power rail", M.Local{}, "", "", "claude", "", 3n},
    M.ThreadCreated{"t3", "p2", "Lid hinge", M.Local{}, "", "", "claude", "", 4n},
    M.BotSet{"ba", "Alpha", "t2", 0n, "", 5n}, M.RoomSet{"ra", "Design room", "ba", 6n}], M.State.empty())

def request( +draft: String) -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"hub"}}, J.KV{"cid", J.Str{"device"}}, J.KV{"target", obj("{\"kind\":\"thread\",\"id\":\"t1\"}")},
    J.KV{"session", J.Json.obj([J.KV{"sel", J.Str{"t1"}}, J.KV{"rid", J.Json.nat(7n)}, J.KV{"drafts", J.Json.obj([J.KV{"t1", J.Str{draft}}])},
      J.KV{"info", obj("{}")}, J.KV{"outbox", obj("{}")}])}])

# the hub's info as Hub.thin.info passes it when the draft wants skills
def info( +draft: String) -> J.Json:
  J.Json.obj(List.append(&2, J.KV, [J.KV{"origin", J.Str{"epoch"}}],
    Bool.pick(List<&2, J.KV>, U.Skills.wants(draft), [J.KV{"skills", J.Str{"[{\"name\":\"review\",\"description\":\"Review the board\"}]"}}, J.KV{"skills.for", J.Str{"t1"}}], Nil{})))

def fresh( +draft: String) -> TA.Actor:
  TA.Actor.refresh(Nil{}, True{}, state(), info(draft), obj("{}"), 100n, request(draft), "view-a")

def again( +before: String, +draft: String) -> TA.Actor:
  TA.Actor.refresh([fresh(before)], True{}, state(), info(draft), obj("{}"), 101n, request(draft), "view-a")

def names(rs: List<&2, U.SkillRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.SkillRow{+n, _, _} <> t: n ++ " " ++ names(t)

def offered( +a: TA.Actor) -> String:
  names(U.Ui.skills(TA.Actor.ui(a)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("@ offers bots", offered(fresh("@")), "@alpha ")
    C.Check.str("> offers threads", Bool.show(String.contains(offered(fresh(">")), ">fix-the-power-rail")), "True")
    C.Check.str("% offers projects", Bool.show(Bool.and(String.contains(offered(fresh("%")), "%alpha-board"), String.contains(offered(fresh("%")), "%beta-enclosure"))), "True")
    C.Check.str("# offers rooms", offered(fresh("#")), "#design-room ")
    C.Check.str("$ offers the thread's skills", offered(fresh("$")), "$review ")
    C.Check.str("/ offers slash commands", Bool.show(String.contains(offered(fresh("/")), "/compact")), "True")
    C.Check.str("a word after text: % still offers", Bool.show(String.contains(offered(fresh("see %be")), "%beta-enclosure")), "True")
    C.Check.str("refreshed actor: the new draft's rows, not the old cache", offered(again("@", "#")), "#design-room ")
    C.Check.str("refreshed actor: text with no trigger offers nothing", offered(again("%", "hello ")), "")
    check("Skills.wants: $name and a slash command, not text", Bool.and(Bool.and(U.Skills.wants("$re"), U.Skills.wants("/co")), Bool.not(U.Skills.wants("hello %p"))))
