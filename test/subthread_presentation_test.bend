import Base
import ./check.bend as C
import ../src/core/subs.bend as SB
import ../src/core/model.bend as M
import ../src/core/prompt.bend as P
import ../src/core/order.bend as O
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/mobile/screen.bend as MS
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/app/layout.bend as L

def row(+id: String, agent: Bool, who: String, word: String, live: Bool) -> SB.SubRow:
  SB.SubRow{id, agent, id, who, word, "", live, False{}, 0n, Nil{}}

def mixed() -> List<&2, SB.SubRow>:
  [row("shell", True{}, "shell", "running", True{}), row("child", False{}, "codex", "running", True{}),
    row("legacy", True{}, "subagent", "running", True{}), row("wake", True{}, "wake", "waiting", True{}),
    row("lock", True{}, "lock", "waiting", True{}), SB.Subs.done.toggle("t", 2n, True{}),
    row("old-shell", True{}, "shell", "done", False{}), row("old-child", False{}, "codex", "done", False{})]

def ids(rs: List<&2, SB.SubRow>) -> String:
  match rs:
    case Nil{}: ""
    case r <> rest: SB.Subs.key(r) ++ ";" ++ ids(rest)

def without.headings(rs: List<&2, SB.SubRow>) -> List<&2, SB.SubRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest: SB.Subs.keep(Bool.not(SB.Subs.display.heading(r)), r, without.headings(rest))

def header() -> SB.SubRow:
  SB.SubRow{"subs.section:1", True{}, "", "section", "", "", False{}, False{}, 0n, Nil{}}

def ordered() -> List<&2, SB.SubRow>:
  [row("child", False{}, "codex", "running", True{}), row("shell", True{}, "shell", "running", True{}),
    row("legacy", True{}, "subagent", "running", True{}), row("wake", True{}, "wake", "waiting", True{}),
    row("lock", True{}, "lock", "waiting", True{}), SB.Subs.done.toggle("t", 2n, True{}),
    row("old-child", False{}, "codex", "done", False{}), row("old-shell", True{}, "shell", "done", False{})]

def fields(rs: List<&2, SB.SubRow>) -> String:
  match rs:
    case Nil{}: ""
    case r <> rest: J.Json.show(MS.Mob.sub(r)) ++ "\n" ++ fields(rest)

def tag(n: D.Node) -> String:
  match n:
    case D.El{t, _, _, _}: t
    case D.Txt{_}: "text"

def truth(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def main() -> IO(Unit):
  do IO<Unit>:
    +info : Map<&2, String> = Map.new(&2, String)
    C.Check.str("shared groups retain every worker and finished toggle", ids(SB.Subs.display(mixed())),
      "subs.section:0;child;subs.section:1;shell;legacy;wake;lock;subs.done:t;subs.section:3;old-child;subs.section:4;old-shell;")
    C.Check.str("grouping preserves every action and row field", fields(without.headings(SB.Subs.display(mixed()))), fields(ordered()))
    C.Check.str("empty panel adds no groups", ids(SB.Subs.display(Nil{})), "")
    C.Check.str("window does not leave a heading without its worker at the bottom", ids(L.Sa.fit(SB.Subs.display([row("child", False{}, "codex", "running", True{})]), 30, False{})), "")
    C.Check.str("window first worker remains reachable in a short viewport", ids(L.Sa.fit(SB.Subs.display([row("child", False{}, "codex", "running", True{})]), 30, True{})), "subs.section:0;child;")
    C.Check.str("headers do not alter active count", Nat.show(SB.Subs.live(SB.Subs.display(mixed()))), Nat.show(SB.Subs.live(mixed())))
    C.Check.str("headers do not alter the finished count", Nat.show(SB.Subs.finished_n(SB.Subs.display(mixed()))), Nat.show(SB.Subs.finished_n(mixed())))
    C.Check.str("current badge wording preserved", SB.Subs.badge(0n) ++ "|" ++ SB.Subs.badge(1n) ++ "|" ++ SB.Subs.badge(3n), "|1 agent|3 agents")
    C.Check.str("background heading Japanese", SB.Subs.display.label(I.Ja{}, header()), "バックグラウンド")
    +wire : J.Json = MS.Mob.sub.display(True{}, I.En{}, header(), info)
    C.Check.str("phones recognize an inert heading", truth(J.Json.get_bool(wire, "heading", False{})), "yes")
    C.Check.str("heading has no action or cancel", J.Json.get_str(wire, "act", "missing") ++ J.Json.get_str(wire, "cancel", "missing"), "")
    C.Check.str("phone conversation still selects its own identity", J.Json.get_str(MS.Mob.sub(row("child", False{}, "codex", "running", True{})), "value", ""), "child")
    C.Check.str("native history still folds its own log", J.Json.get_str(MS.Mob.sub(row("legacy", True{}, "subagent", "running", True{})), "value", ""), "1sub:legacy")
    C.Check.str("web section is a heading instead of a button", tag(V.View.sub.signals(I.En{}, header(), info, M.State.empty(), "t", 1n)), "h3")
    +args : String = String.join(P.Claude.args("sonnet", "resume-id", M.AcceptEdits{}, M.DefaultMode{}, "SYS", Nil{}), " ")
    C.Check.str("Claude blocks both native spawning names on resume", truth(String.contains(args, "--disallowedTools Agent,Task")), "yes")
    C.Check.str("Claude resume contract preserved", truth(String.contains(args, "--resume resume-id")), "yes")
    C.Check.str("delegation keeps both tools", truth(Bool.and(String.contains(P.Prompt.orchestration(True{}, "models"), "`thread_launch`"), String.contains(P.Prompt.orchestration(True{}, "models"), "`delegate_task`"))), "yes")
    C.Check.str("depth limit still forbids more delegation", truth(String.contains(P.Prompt.orchestration(False{}, ""), "do not delegate further")), "yes")
    C.Check.str("intent instructions no longer request native Agent", truth(String.contains(O.Order.prompt.intent(), "Agent tool")), "no")
