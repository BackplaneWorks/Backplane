import Base
import ./check.bend as C
import ../src/core/agent.bend as A
import ../src/core/json.bend as J

# Skills (SKILL.md frontmatter, scopes, the prompt block), `$skill`
# mentions per provider, reasoning effort flags, and the one-shot answers.

def sk(+n: String, +m: Bool, +u: Bool) -> A.USkill:
  A.USkill{n, "does " ++ n, "/s/" ++ n ++ "/SKILL.md", m, u}

def names(ss: List<&2, A.USkill>) -> List<&2, String>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> t:
      A.USkill.name(s) <> names(t)

def show(s: A.USkill) -> String:
  match s:
    case A.USkill{n, d, p, m, u}:
      n ++ "|" ++ d ++ "|" ++ p ++ "|" ++ Bool.pick(String, m, "model", "-") ++ "|" ++ Bool.pick(String, u, "user", "-")

def tx(t: A.Tx) -> String:
  match t:
    case A.Tx{p, m}:
      p ++ "|" ++ m

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def main() -> IO(Unit):
  +full = A.USkill.parse("---\nname: pcb-review\ndescription: \"Review a board\"\ndisable-model-invocation: true\nuser-invocable: false\n---\nbody", "dir", "/p/SKILL.md")
  +bare = A.USkill.parse("no frontmatter", "dir", "/p/SKILL.md")
  +us = {[sk("a", True{}, True{}), sk("b", True{}, True{})] : List<&2, A.USkill>}
  +ps = {[A.USkill{"b", "project b", "/proj/b", True{}, True{}}, sk("c", False{}, True{}), sk("hidden", True{}, False{})] : List<&2, A.USkill>}
  +all = A.USkill.merge(us, ps)
  +codex = A.Mention.expand("codex", "$a then $c and $a, $nope $hidden", all)
  do IO<Unit>:
    C.Check.str("skill frontmatter", show(full), "pcb-review|Review a board|/p/SKILL.md|-|-")
    C.Check.str("skill defaults", show(bare), "dir||/p/SKILL.md|model|user")
    C.Check.str("user scope wins", String.join(names(all), ","), "a,b,c,hidden")
    C.Check.str("user b kept", yes(String.contains(A.USkill.prompt(all), "does b")), "yes")
    C.Check.str("prompt lists model skills", yes(String.contains(A.USkill.prompt(all), "- a: does a Read \"/s/a/SKILL.md\"")), "yes")
    C.Check.str("prompt skips disabled", yes(String.contains(A.USkill.prompt(all), "- c:")), "no")
    C.Check.str("prompt empty", A.USkill.prompt(Nil{}), "")
    C.Check.str("json offers user skills", J.Json.show(A.USkill.json([sk("a", True{}, True{}), sk("h", True{}, False{})])), "[{\"name\":\"a\",\"description\":\"does a\"}]")
    C.Check.str("mention claude", A.Mention.expand("claude", "$a fix R3\nthen $c, not $HOME or $hidden", all), "/a fix R3\nthen /c, not $HOME or $hidden")
    C.Check.str("mention none", A.Mention.expand("codex", "no skills here", all), "no skills here")
    C.Check.str("mention codex notes", codex,
      "<skill_invocations>\nThe user invoked the $a skill: read \"/s/a/SKILL.md\" completely before anything else and follow its instructions for this request.\nThe user invoked the $c skill: read \"/s/c/SKILL.md\" completely before anything else and follow its instructions for this request.\n</skill_invocations>\n\n$a then $c and $a, $nope $hidden")
    C.Check.str("effort claude", String.join(A.Effort.args("claude", "xhigh"), " "), "--effort xhigh")
    C.Check.str("effort codex", String.join(A.Effort.args("codex", "low"), " "), "-c model_reasoning_effort=\"low\"")
    C.Check.str("effort grok", String.join(A.Effort.args("grok", "high"), " "), "--reasoning-effort high")
    C.Check.str("effort grok no xhigh", String.join(A.Effort.args("grok", "xhigh"), " "), "")
    C.Check.str("effort muse on the wire", String.join(A.Effort.args("muse", "high"), " "), "")
    C.Check.str("effort muse levels", String.join(A.Effort.levels("muse"), ","), "minimal,low,medium,high,xhigh,max")
    C.Check.str("effort omp levels", String.join(A.Effort.levels("omp"), ","), "off,auto,minimal,low,medium,high,xhigh,max")
    C.Check.str("effort omp no argv", String.join(A.Effort.args("omp", "high"), " "), "")
    C.Check.str("effort default", String.join(A.Effort.args("claude", ""), " "), "")
    C.Check.str("effort bogus", String.join(A.Effort.args("codex", "max; rm"), " "), "")
    C.Check.str("temp branch", yes(A.Name.temp("backplane/0a1b2c3d")), "yes")
    C.Check.str("named branch", yes(A.Name.temp("backplane/fix-the-thing")), "no")
    C.Check.str("oneshot result", A.OneShot.result("warning: x\n{\"type\":\"result\",\"is_error\":false,\"result\":\"  Add effort chips \"}\n"), "Add effort chips")
    C.Check.str("oneshot error", A.OneShot.result("{\"type\":\"result\",\"is_error\":true,\"result\":\"Not logged in\"}"), "")
    C.Check.str("oneshot garbage", A.OneShot.result("command not found"), "")
    C.Check.str("name fragment", A.Name.fragment("\"Add Effort Chips\"\nmore"), "add-effort-chips")
    C.Check.str("commit fences", A.Commit.message("```\nAdd effort chips\n\nPass --effort.\n```", "title"), "Add effort chips\n\nPass --effort.")
    C.Check.str("commit fallback", A.Commit.message("  ", "title"), "title")
    C.Check.str("text default", tx(A.Text.pick("", "")), "claude|haiku")
    C.Check.str("text codex light", tx(A.Text.pick("codex", "")), "codex|gpt-6-luna")
    C.Check.str("text grok model", tx(A.Text.pick(" grok ", "grok-4.6")), "grok|grok-4.6")
    C.Check.str("text unknown", tx(A.Text.pick("gemini", "opus")), "claude|opus")
    C.Check.str("text codex leftover haiku", tx(A.Text.pick("codex", "haiku")), "codex|gpt-6-luna")
    C.Check.str("text codex leftover grok", tx(A.Text.pick("codex", "grok-4.6")), "codex|gpt-6-luna")
    C.Check.str("text codex o3", tx(A.Text.pick("codex", "o3")), "codex|o3")
    C.Check.str("text claude leftover gpt", tx(A.Text.pick("claude", "gpt-6-sol")), "claude|haiku")
    C.Check.str("text claude id", tx(A.Text.pick("claude", "claude-opus-5-5")), "claude|claude-opus-5-5")
    C.Check.str("text grok leftover sonnet", tx(A.Text.pick("grok", "sonnet")), "grok|grok-4.7-build-fast")
    C.Check.str("text muse", tx(A.Text.pick("muse", "")), "muse|muse-spark-1.3")
    C.Check.str("text muse model", tx(A.Text.pick("muse", "muse-wonder-1.0")), "muse|muse-wonder-1.0")
    C.Check.str("text muse leftover grok", tx(A.Text.pick("muse", "grok-4.6")), "muse|muse-spark-1.3")
    C.Check.str("text custom kept", tx(A.Text.pick("codex", "my-local-model")), "codex|my-local-model")
    C.Check.str("args claude", String.join(A.OneShot.args(A.Tx{"claude", "haiku"}, "SYS"), " "),
      "60 claude -p --model haiku --output-format json --no-session-persistence --tools  --strict-mcp-config --effort low --system-prompt SYS")
    C.Check.str("args codex", String.join(A.OneShot.args(A.Tx{"codex", "gpt-6-luna"}, "SYS"), " "),
      "60 codex exec --json --ephemeral --skip-git-repo-check --sandbox read-only -c model_reasoning_effort=\"low\" -m gpt-6-luna SYS")
    C.Check.str("args codex default model", String.join(A.OneShot.args(A.Tx{"codex", ""}, "SYS"), " "),
      "60 codex exec --json --ephemeral --skip-git-repo-check --sandbox read-only -c model_reasoning_effort=\"low\" SYS")
    C.Check.str("args grok", String.join(A.OneShot.args(A.Tx{"grok", "grok-4.7-build-fast"}, "SYS"), " "),
      "60 sh -c cat | exec grok --prompt-file /dev/stdin \"$@\" sh --output-format json --no-subagents --disable-web-search --permission-mode dontAsk --max-turns 1 --reasoning-effort low -m grok-4.7-build-fast")
    C.Check.str("input grok", A.OneShot.input("grok", "SYS", "fix it"), "SYS\n\n<input>\nfix it\n</input>")
    C.Check.str("args muse", String.join(A.OneShot.args(A.Tx{"muse", "muse-spark-1.3"}, "SYS"), " "),
      "60 sh -c TMPDIR=/var/tmp exec muse exec \"$@\" -- \"$(cat)\" 2>/dev/null sh --no-session-log --disable-shell --disable-write --disable-web-tools --disable-reminders --approval-mode never --max-model-steps 1 --reasoning-effort low --model muse-spark-1.3")
    C.Check.str("input muse", A.OneShot.input("muse", "SYS", "fix it"), "SYS\n\n<input>\nfix it\n</input>")
    C.Check.str("answer muse", A.OneShot.answer("muse", "Fix Login Bug\n"), "Fix Login Bug")
    C.Check.str("input claude", A.OneShot.input("claude", "SYS", "fix it"), "fix it")
    C.Check.str("answer codex", A.OneShot.answer("codex", "Reading additional input from stdin...\n{\"type\":\"thread.started\",\"thread_id\":\"x\"}\n{\"type\":\"item.completed\",\"item\":{\"id\":\"item_0\",\"type\":\"reasoning\",\"text\":\"hmm\"}}\n{\"type\":\"item.completed\",\"item\":{\"id\":\"item_1\",\"type\":\"agent_message\",\"text\":\" Keep pinned threads at top \"}}\n{\"type\":\"turn.completed\"}\n"), "Keep pinned threads at top")
    C.Check.str("answer codex none", A.OneShot.answer("codex", "{\"type\":\"turn.failed\"}\n"), "")
    C.Check.str("answer grok", A.OneShot.answer("grok", "{\n  \"text\": \"Keep pinned threads on top\",\n  \"stopReason\": \"end_turn\"\n}\n"), "Keep pinned threads on top")
    C.Check.str("answer grok cancelled", A.OneShot.answer("grok", "{\n  \"text\": \"I'll look through the code\",\n  \"stopReason\": \"cancelled\"\n}\n"), "")
    C.Check.str("answer claude", A.OneShot.answer("claude", "{\"type\":\"result\",\"is_error\":false,\"result\":\"Add effort chips\"}"), "Add effort chips")
    C.Check.str("title plain", A.Title.clean("Keep pinned threads at top"), "Keep pinned threads at top")
    C.Check.str("title quotes period", A.Title.clean("  \"Keep pinned   threads at top.\"\nextra line"), "Keep pinned threads at top")
    C.Check.str("title markdown", A.Title.clean("## **Title:** `Fix` the  sidebar\t order"), "Fix the sidebar order")
    C.Check.str("title single quotes", A.Title.clean("'Fix the sidebar's order'"), "Fix the sidebar's order")
    C.Check.str("title ellipsis kept", A.Title.clean("Wait for it..."), "Wait for it...")
    C.Check.str("title empty", A.Title.clean("  \n "), "")
    C.Check.str("title cut", A.Title.clean("Make the very long title that the model wrote go on and on past sixty characters"), "Make the very long title that the model wrote go on and o...")
