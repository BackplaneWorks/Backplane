import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/subs.bend as SB
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/presentation.bend as P
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

# What a Subagents row says beside its name (subs.bend's SubInfo): where it
# runs, its model, its steps, what it used, how long, what it was asked and
# what it reported. All of it from the log; what the log has not got is
# left out.

def about() -> String:
  "{\"type\":\"Explore\",\"model\":\"haiku\",\"prompt\":\"Read the board files.\\nKeep it short.\"}"

def outcome() -> String:
  "{\"summary\":\"Found 3 nets without labels\",\"usage\":{\"total_tokens\":18250,\"tool_uses\":2,\"duration_ms\":240000}}"

def log() -> List<&2, M.Change>:
  [M.ActivityLogged{"@sub:t1", "c1/about", "c1", "about", about(), 100n},
   M.ActivityLogged{"@sub:t1", "c1/start:subagent", "c1", "start:subagent", "Check the nets", 100n},
   M.ActivityLogged{"@sub:t1", "c1/u1", "c1", "Bash", "grep -n net board.kicad_pcb", 130n},
   M.ActivityLogged{"@sub:t1", "c1/m1", "c1", "say", "Three nets have no label.\nMore below.", 200n},
   M.ActivityLogged{"@sub:t1", "c1/u2", "c1", "Read", "board.kicad_pcb", 250n},
   M.ActivityLogged{"@sub:t1", "c1/end", "c1", "end", "completed", 340n},
   M.ActivityLogged{"@sub:t1", "c1/outcome", "c1", "outcome", outcome(), 340n}]

def st() -> M.State:
  M.State.replay(log(), M.State.empty())

def live() -> M.State:
  M.State.replay([M.SettingSet{"bg.t1", "a\tReading the board\tc2\tCheck the parts\tsubagent", 1n},
    M.ActivityLogged{"@sub:t1", "c2/start:subagent", "c2", "start:subagent", "Check the parts", 100n},
    M.ActivityLogged{"@sub:t1", "c2/u1", "c2", "Bash", "ls parts", 110n}], M.State.empty())

# Claude Code's task_progress for c2, with what it has used
def progress() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a\",\"description\":\"Reading parts\",\"usage\":{\"total_tokens\":4200}}"), J.Null{})

def info(+s: M.State, +call: String) -> SB.SubInfo:
  SB.Subs.info(s, "t1", SB.SubRow{call, True{}, "x", "subagent", "done", "", False{}, False{}, 0n, Nil{}})

def runs(i: SB.SubInfo) -> String:
  match i:
    case SB.SubInfo{w, m, si, u, n, tk, SB.SubStep{lk, lt}, a, o}:
      w ++ "|" ++ m ++ "|" ++ Nat.show(si) ++ "|" ++ Nat.show(u) ++ "|" ++ Nat.show(n) ++ "|" ++ Nat.show(tk) ++ "|" ++ lk ++ " " ++ lt ++ "|" ++ a ++ "|" ++ o

def steps(as: List<&2, SB.SubAgg>) -> String:
  match as:
    case SB.SubAgg{_, _, ti, _, n, _, _} <> _: ti ++ " " ++ Nat.show(n)
    case Nil{}: "none"

# the finished ones shown and c1 open (its steps and what it was asked)
def opened() -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "fold:sub:c1", "1"), "fold:subs.done:t1", "1")

def page(+s: M.State, +now: Nat) -> String:
  J.Json.show(P.Node.write(D.El{"div", "", Nil{}, V.View.subs.signals.at(I.En{}, SB.Subs.rows(s, opened(), "t1", now), opened(), s, "t1", now)}))

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def agent(+id: String, +input: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"tool_use\",\"id\":\"" ++ id ++ "\",\"name\":\"Agent\",\"input\":" ++ input ++ "}"), J.Null{})

def kinds(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{th, _, _, k, _, _} <> t: th ++ ":" ++ k ++ ";" ++ kinds(t)
    case _ <> t: kinds(t)

def texts(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{_, _, _, _, x, _} <> t: x ++ ";" ++ texts(t)
    case _ <> t: texts(t)

def notification() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"a\",\"tool_use_id\":\"c1\",\"status\":\"completed\",\"summary\":\"done\",\"usage\":{\"total_tokens\":5}}"), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a finished subagent: where, model, times, steps, tokens, newest step, prompt, report", runs(info(st(), "c1")),
      "Claude Code · Explore|haiku|100|340|3|18250|Read board.kicad_pcb|Read the board files.\nKeep it short.|Found 3 nets without labels")
    C.Check.str("what the hub notes is not a step", steps(SB.Subs.aggs(st(), "t1")), "Check the nets 3")
    C.Check.str("a subagent with no note: the type and model left out", runs(info(live(), "c2")), "Claude Code subagent||100|0|1|0|Bash ls parts||")
    C.Check.str("a delegated thread elsewhere: on its machine", runs(SB.Subs.info(M.State.empty(), "t1", SB.SubRow{"lnk~t9", False{}, "x", "codex", "running", "", True{}, False{}, 0n, Nil{}})), "Backplane thread on lnk||0|0|0|0| ||")
    C.Check.str("a delegated Codex thread here: its agent and model, since it was made", runs(SB.Subs.info(M.State.replay([M.ThreadCreated{"t9", "p", "Child", M.Local{}, "", "", "codex", "gpt-6.1", 50n}], M.State.empty()), "t1", SB.SubRow{"t9", False{}, "x", "codex", "running", "", True{}, False{}, 0n, Nil{}})), "Codex thread|gpt-6.1|50|0|0|0| ||")
    C.Check.str("durations", SB.Subs.dur(45n) ++ " " ++ SB.Subs.dur(240n) ++ " " ++ SB.Subs.dur(3840n) ++ " " ++ SB.Subs.dur(7200n) ++ " " ++ SB.Subs.dur(200000n), "45s 4m 1h 4m 2h 2d")
    C.Check.str("tokens", SB.Subs.tokens(0n) ++ "|" ++ SB.Subs.tokens(812n) ++ "|" ++ SB.Subs.tokens(18250n) ++ "|" ++ SB.Subs.tokens(1450000n) ++ "|" ++ SB.Subs.tokens(240000n), "|812 tokens|18.2k tokens|1.4M tokens|240k tokens")
    C.Check.str("at work: how long so far", SB.Subs.took(True{}, 100n, 0n, 280n), "3m")
    C.Check.str("at work under a minute", SB.Subs.took(True{}, 100n, 0n, 130n) ++ "|" ++ SB.Subs.took(True{}, 100n, 0n, 90n), "<1m|<1m")
    C.Check.str("ended: what it took", SB.Subs.took(False{}, 100n, 340n, 9000n), "4m")
    C.Check.str("no start known: no time", SB.Subs.took(True{}, 0n, 0n, 280n), "")
    +shown : String = page(st(), 400n)
    C.Check.str("the row says where it ran", yes(String.contains(shown, "Claude Code · Explore")), "yes")
    C.Check.str("the row says its model", yes(String.contains(shown, "\"haiku\"")), "yes")
    C.Check.str("the row says its steps and tokens", yes(Bool.and(String.contains(shown, "3 steps"), String.contains(shown, "18.2k tokens"))), "yes")
    C.Check.str("the row says how it ended and what it took", yes(String.contains(shown, "done · 4m")), "yes")
    C.Check.str("the row says what it reported", yes(String.contains(shown, "Found 3 nets without labels")), "yes")
    C.Check.str("open, the row shows what it was asked", yes(String.contains(shown, "Read the board files.")), "yes")
    +running : String = page(live(), 280n)
    C.Check.str("at work: what it does now", yes(String.contains(running, "Reading the board")), "yes")
    C.Check.str("at work: how long so far", yes(String.contains(running, "running · 3m")), "yes")
    +rows : List<&2, String> = H.Bg.progress.o(M.Bg.of(live(), "t1"), progress())
    C.Check.str("a progress line keeps what it does now and the tokens so far", String.join(rows, "\n"), "a\tReading parts\tc2\tCheck the parts\tsubagent\t4200")
    C.Check.str("a progress line without usage leaves the line as it was", String.join(H.Bg.progress.o(M.Bg.of(live(), "t1"), Maybe.default(&2, J.Json, J.Json.parse("{\"task_id\":\"a\",\"description\":\"x\"}"), J.Null{})), "\n"), "a\tx\tc2\tCheck the parts\tsubagent")
    +used : M.State = M.State.replay([M.SettingSet{"bg.t1", String.join(rows, "\n"), 2n}], live())
    C.Check.str("at work: the tokens so far", runs(info(used, "c2")), "Claude Code subagent||100|0|1|4200|Bash ls parts||")
    # the hub notes an Agent call's type, model and prompt in its subagent's log
    +cs : List<&2, M.Change> = H.Claude.block.k(1n, "t1", "m", 0n, 5n, 0n, "", agent("c9", "{\"description\":\"Check\",\"prompt\":\"Look\",\"subagent_type\":\"Explore\",\"model\":\"haiku\"}"), False{})
    C.Check.str("an Agent call is logged and noted", kinds(cs), "t1:Agent;@sub:t1:about;")
    C.Check.str("the note names its type, model and prompt", yes(String.contains(texts(cs), "{\"type\":\"Explore\",\"model\":\"haiku\",\"prompt\":\"Look\"}")), "yes")
    C.Check.str("another tool notes nothing", kinds(H.Claude.block.k(1n, "t1", "m", 0n, 5n, 0n, "", Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"tool_use\",\"id\":\"c8\",\"name\":\"Bash\",\"input\":{}}"), J.Null{}), False{})), "t1:Bash;")
    C.Check.str("an end notes what it reported and used", yes(String.contains(texts(H.Bg.system(M.State.empty(), "t1", 9n, notification())), "{\"summary\":\"done\",\"usage\":{\"total_tokens\":5}}")), "yes")
