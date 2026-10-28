import Base
import ./check.bend as C
import ../src/core/modelpick.bend as MP
import ../src/core/json.bend as J

def steps(ss: List<&2, MP.MpStep>) -> String:
  match ss:
    case Nil{}: ""
    case MP.MpStep{l, on} <> t: "[" ++ l ++ Bool.pick(String, on, "", "x") ++ "]" ++ steps(t)

def bars(bs: List<&2, MP.MpBar>) -> String:
  match bs:
    case Nil{}: ""
    case MP.MpBar{l, _, lit, fill, _} <> t: Bool.pick(String, lit, "*", Bool.pick(String, fill, "+", ".")) ++ l ++ " " ++ bars(t)

def reqs(rs: List<&2, MP.MpReq>) -> String:
  match rs:
    case Nil{}: ""
    case MP.MpReq{m, args} <> t: m ++ J.Json.show(J.Json.obj(args)) ++ ";" ++ reqs(t)

def cur(c: MP.MpCur) -> String:
  MP.ModelPick.show("t", c)

def claude() -> List<&2, String>:
  ["", "low", "medium", "high", "xhigh", "max"]

def codex() -> List<&2, String>:
  ["", "low", "medium", "high", "xhigh"]

def agents() -> List<&2, String>:
  ["claude", "codex", "grok"]

def models() -> List<&2, String>:
  ["", "fable", "opus", "sonnet"]

def now() -> MP.MpCur:
  MP.MpCur{1n, "claude", "opus", "high"}

def key(+k: String, c: MP.MpCur, +es: List<&2, String>) -> String:
  cur(MP.ModelPick.key(k, c, agents(), models(), MP.ModelPick.steps(es), now()))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("claude: the six steps, all taken", steps(MP.ModelPick.steps(claude())), "[][low][medium][high][xhigh][max]")
    C.Check.str("codex: max drawn, not taken", steps(MP.ModelPick.steps(codex())), "[][low][medium][high][xhigh][maxx]")
    C.Check.str("no effort: six steps, none taken", steps(MP.ModelPick.steps(Nil{})), "[x][lowx][mediumx][highx][xhighx][maxx]")
    C.Check.str("pi: off and minimal in their place", steps(MP.ModelPick.steps(["", "off", "minimal", "low", "medium", "high", "xhigh", "max"])), "[][off][minimal][low][medium][high][xhigh][max]")
    C.Check.str("omp: its own auto too", steps(MP.ModelPick.steps(["", "off", "auto", "minimal", "low", "high"])), "[][off][auto][minimal][low][mediumx][high][xhighx][maxx]")
    C.Check.str("an unknown level goes last", steps(MP.ModelPick.steps(["", "low", "turbo"])), "[][low][mediumx][highx][xhighx][maxx][turbo]")
    C.Check.str("lit: a level taken", MP.ModelPick.lit(MP.ModelPick.steps(claude()), "max"), "max")
    C.Check.str("lit: a level not taken is the default", MP.ModelPick.lit(MP.ModelPick.steps(codex()), "max"), "")
    C.Check.str("lit: no effort lights nothing", MP.ModelPick.lit(MP.ModelPick.steps(Nil{}), ""), "-")
    C.Check.str("bars: below the lit one fill", bars(MP.ModelPick.bars(MP.ModelPick.steps(claude()), "high")), "+ +low +medium *high .xhigh .max ")
    C.Check.str("bars: nothing lit, nothing filled", bars(MP.ModelPick.bars(MP.ModelPick.steps(Nil{}), "high")), ". .low .medium .high .xhigh .max ")
    C.Check.str("tab: model to effort", key("tab", now(), claude()), "t\t2\tclaude\topus\thigh")
    C.Check.str("tab: effort to agent", key("tab", MP.ModelPick.col(now(), 2n), claude()), "t\t0\tclaude\topus\thigh")
    C.Check.str("shift+tab: model to agent", key("stab", now(), claude()), "t\t0\tclaude\topus\thigh")
    C.Check.str("shift+tab: agent to effort", key("stab", MP.ModelPick.col(now(), 0n), claude()), "t\t2\tclaude\topus\thigh")
    C.Check.str("down: the next model", key("down", now(), claude()), "t\t1\tclaude\tsonnet\thigh")
    C.Check.str("down: the last model stays", key("down", MP.ModelPick.model(now(), "sonnet"), claude()), "t\t1\tclaude\tsonnet\thigh")
    C.Check.str("up: the default stays first", key("up", MP.ModelPick.model(now(), ""), claude()), "t\t1\tclaude\t\thigh")
    C.Check.str("up: a model not listed goes to the first", key("up", MP.ModelPick.model(now(), "x"), claude()), "t\t1\tclaude\t\thigh")
    C.Check.str("down on agents: another agent, its default model", key("down", MP.ModelPick.col(now(), 0n), claude()), "t\t0\tcodex\t\thigh")
    C.Check.str("back to the thread's agent: its own model", key("up", MP.MpCur{0n, "codex", "", "high"}, codex()), "t\t0\tclaude\topus\thigh")
    C.Check.str("right: more effort", key("right", now(), claude()), "t\t1\tclaude\topus\txhigh")
    C.Check.str("right: max stays", key("right", MP.ModelPick.effort(now(), "max"), claude()), "t\t1\tclaude\topus\tmax")
    C.Check.str("right: a step not taken is skipped", key("right", MP.MpCur{1n, "codex", "", "xhigh"}, codex()), "t\t1\tcodex\t\txhigh")
    C.Check.str("right: from a level not taken, from the default", key("right", MP.MpCur{1n, "codex", "", "max"}, codex()), "t\t1\tcodex\t\tlow")
    C.Check.str("left: down to auto", key("left", MP.ModelPick.effort(now(), "low"), claude()), "t\t1\tclaude\topus\t")
    C.Check.str("left: auto stays", key("left", MP.ModelPick.effort(now(), ""), claude()), "t\t1\tclaude\topus\t")
    C.Check.str("up in the effort column: more", key("up", MP.ModelPick.col(now(), 2n), claude()), "t\t2\tclaude\topus\txhigh")
    C.Check.str("no effort: left and right stay", key("right", now(), Nil{}), "t\t1\tclaude\topus\thigh")
    C.Check.str("another key leaves it", key("x", now(), claude()), "t\t1\tclaude\topus\thigh")
    C.Check.str("plan: nothing changed", reqs(MP.ModelPick.plan(MP.ModelPick.col(now(), 2n), now())), "")
    C.Check.str("plan: the model", reqs(MP.ModelPick.plan(MP.ModelPick.model(now(), "fable"), now())), "thread.modes{\"model\":\"fable\"};")
    C.Check.str("plan: model and effort", reqs(MP.ModelPick.plan(MP.MpCur{1n, "claude", "", "max"}, now())), "thread.modes{\"model\":\"\"};thread.effort{\"effort\":\"max\"};")
    C.Check.str("plan: another agent, one request", reqs(MP.ModelPick.plan(MP.MpCur{0n, "codex", "gpt-5", "high"}, now())), "thread.provider{\"provider\":\"codex\",\"model\":\"gpt-5\"};")
    C.Check.str("plan: another agent and effort", reqs(MP.ModelPick.plan(MP.MpCur{0n, "codex", "", "low"}, now())), "thread.provider{\"provider\":\"codex\",\"model\":\"\",\"effort\":\"low\"};")
    C.Check.str("kept for this thread", cur(MP.ModelPick.read("t\t2\tcodex\tgpt\tlow", "t", now())), "t\t2\tcodex\tgpt\tlow")
    C.Check.str("kept for another thread: the thread's own", cur(MP.ModelPick.read("u\t2\tcodex\tgpt\tlow", "t", now())), "t\t1\tclaude\topus\thigh")
    C.Check.str("none kept: the thread's own", cur(MP.ModelPick.read("", "t", MP.ModelPick.col(now(), 0n))), "t\t1\tclaude\topus\thigh")
    C.Check.str("kept: this thread", Bool.show(MP.ModelPick.kept("t\t1\tclaude\t\t", "t")), "True")
    C.Check.str("kept: not another", Bool.show(MP.ModelPick.kept("tt\t1\tclaude\t\t", "t")), "False")
    C.Check.str("products", MP.ModelPick.product("claude") ++ "," ++ MP.ModelPick.product("codex") ++ "," ++ MP.ModelPick.product("x"), "Claude Code,Codex CLI,")
    # the heads' keycaps read from the column the keys are in: it says ↑↓,
    # Tab names the next column, ⇧Tab the one before, the effort ←→
    C.Check.str("caps on the model column (it opens there)", MP.ModelPick.cap(0n, 1n) ++ " " ++ MP.ModelPick.cap(1n, 1n) ++ " " ++ MP.ModelPick.cap(2n, 1n), "⇧Tab ↑↓ ←→")
    C.Check.str("caps on the agent column", MP.ModelPick.cap(0n, 0n) ++ " " ++ MP.ModelPick.cap(1n, 0n) ++ " " ++ MP.ModelPick.cap(2n, 0n), "↑↓ Tab ←→")
    C.Check.str("caps on the effort column", MP.ModelPick.cap(0n, 2n) ++ " " ++ MP.ModelPick.cap(1n, 2n) ++ " " ++ MP.ModelPick.cap(2n, 2n), "Tab ⇧Tab ←→")
    C.Check.str("Tab goes where its cap is", Nat.show(MP.MpCur.col(MP.ModelPick.key("tab", MP.ModelPick.col(now(), 0n), agents(), ["", "opus"], MP.ModelPick.steps(claude()), now()))), "1")
