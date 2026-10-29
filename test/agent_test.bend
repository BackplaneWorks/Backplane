import Base
import ./check.bend as C
import ../src/core/agent.bend as A
import ../src/core/json.bend as J
import ../src/core/prompt.bend as P
import ../src/server/kicad.bend as K

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

def qshow(s: P.Skill) -> String:
  match s:
    case P.Skill{n, d, p}:
      n ++ "|" ++ d ++ "|" ++ p

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
    # YAML's folded and literal blocks and wrapped values: the text, not ">"
    C.Check.str("skill folded description", show(A.USkill.parse("---\nname: diagnose-crash\ndescription: >\n  Diagnose why a program crashed.\n  Use when a process dumped core.\nuser-invocable: false\n---\n  indented body", "dir", "/p")), "diagnose-crash|Diagnose why a program crashed. Use when a process dumped core.|/p|model|-")
    C.Check.str("skill literal description", show(A.USkill.parse("---\ndescription: |-\n\tOne\n\ttwo\nname: lit\n---\n", "dir", "/p")), "lit|One two|/p|model|user")
    C.Check.str("skill wrapped description", show(A.USkill.parse("---\nname: w\ndescription: \"Place parts\n  where they belong\"\n---\n", "dir", "/p")), "w|Place parts where they belong|/p|model|user")
    C.Check.str("skill nested key not a field", show(A.USkill.parse("---\nname: n\nmetadata:\n  description: inner\n---\n", "dir", "/p")), "n||/p|model|user")
    C.Check.str("skill one-line description", show(A.USkill.parse("---\nname: one\ndescription: Just this.\n---\n  body: x", "dir", "/p")), "one|Just this.|/p|model|user")
    C.Check.str("kistack skill folded description", qshow(P.Skill.parse("---\nname: kicad-layout\ndescription: >-\n  Route a board.\n  Check DRC.\n---\nbody", "dir", "/k")), "kicad-layout|Route a board. Check DRC.|/k")
    # KiStack beside the hub, or the install's for the desktop app's own hub
    C.Check.str("kistack beside the hub", K.KiStack.dir("/home/u/.local/share/backplane"), "/home/u/.local/share/backplane/kistack")
    C.Check.str("kistack for the desktop's hub", K.KiStack.dir("/home/u/.local/share/backplane/desktop/runtime/resources/backplane-service"), "/home/u/.local/share/backplane/kistack")
    C.Check.str("kistack for another service folder", K.KiStack.dir("/opt/x/resources/backplane-service"), "/opt/x/resources/backplane-service/kistack")
    # KiStack synced into the hub's home at each start: where, which wins
    C.Check.str("kistack synced in the home", K.KiStack.home("/h"), "/h/kistack")
    C.Check.str("kistack sync stage in the home", K.KiStack.stage("/h"), "/h/kistack.sync")
    C.Check.str("kistack synced wins", K.KiStack.pick(True{}, "/h", "/b/kistack"), "/h/kistack")
    C.Check.str("kistack bundled until synced", K.KiStack.pick(False{}, "/h", "/b/kistack"), "/b/kistack")
    C.Check.str("kistack claude dirs synced", String.join(K.KiStack.dirs(True{}, "/h", ["/b/kistack"]), ","), "/h/kistack")
    C.Check.str("kistack claude dirs synced, none bundled", String.join(K.KiStack.dirs(True{}, "/h", Nil{}), ","), "/h/kistack")
    C.Check.str("kistack claude dirs bundled", String.join(K.KiStack.dirs(False{}, "/h", ["/b/kistack"]), ","), "/b/kistack")
    C.Check.str("kistack claude dirs none", String.join(K.KiStack.dirs(False{}, "/h", Nil{}), ","), "")
    C.Check.str("kistack manifest is ours", K.KiStack.manifest(), "{\"name\":\"kistack\",\"description\":\"KiStack KiCad skills by American Embedded\"}")
    # only skills/** and the LICENSE: never the repository's manifest, hooks, MCP servers, commands or agents
    C.Check.str("kistack keeps", String.join([yes(K.KiStack.kept("skills/pcb/SKILL.md")), yes(K.KiStack.kept("skills/")), yes(K.KiStack.kept("LICENSE"))], ","), "yes,yes,yes")
    C.Check.str("kistack drops", String.join([yes(K.KiStack.kept(".claude-plugin/plugin.json")), yes(K.KiStack.kept("hooks/hooks.json")), yes(K.KiStack.kept(".mcp.json")),
      yes(K.KiStack.kept("commands/x.md")), yes(K.KiStack.kept("agents/a.md")), yes(K.KiStack.kept("skills")), yes(K.KiStack.kept("skillsx/a")), yes(K.KiStack.kept("README.md")), yes(K.KiStack.kept("LICENSE/x"))], ","),
      "no,no,no,no,no,no,no,no,no")
    C.Check.str("kistack members", String.join(K.KiStack.members("pax_global_header\nkistack-641a/\nkistack-641a/.claude-plugin/\nkistack-641a/.claude-plugin/plugin.json\nkistack-641a/.mcp.json\nkistack-641a/LICENSE\nkistack-641a/hooks/hooks.json\nkistack-641a/skills/\nkistack-641a/skills/pcb/\nkistack-641a/skills/pcb/SKILL.md\n"), ","),
      "kistack-641a/skills,kistack-641a/LICENSE")
    C.Check.str("kistack members, no license", String.join(K.KiStack.members("repo/\nrepo/skills/a/SKILL.md\nrepo/README.md\n"), ","), "repo/skills")
    C.Check.str("kistack members, no skills", String.join(K.KiStack.members("repo/\nrepo/LICENSE\nrepo/.claude-plugin/plugin.json\n"), ","), "")
    C.Check.str("kistack members, skills a link", String.join(K.KiStack.members("repo/\nrepo/skills\nrepo/LICENSE\nrepo/other/a/SKILL.md\n"), ","), "")
    C.Check.str("kistack members, no top folder", String.join(K.KiStack.members("skills/a/SKILL.md\nLICENSE\n"), ","), "")
    C.Check.str("kistack members, none listed", String.join(K.KiStack.members(""), ","), "")
    # REVISION and GitHub's answer: a commit is 40 lowercase hex digits
    C.Check.str("kistack revision", K.KiStack.rev("641ade9041bb0844c4530dfff11010eef30919af\n"), "641ade9041bb0844c4530dfff11010eef30919af")
    C.Check.str("kistack revision trimmed", K.KiStack.rev("  641ade9041bb0844c4530dfff11010eef30919af  \nmore"), "641ade9041bb0844c4530dfff11010eef30919af")
    C.Check.str("kistack revision local", K.KiStack.rev("local\n"), "")
    C.Check.str("kistack revision short", K.KiStack.rev("641ade9041bb0844c4530dfff11010eef30919a"), "")
    C.Check.str("kistack revision upper", K.KiStack.rev("641ADE9041BB0844C4530DFFF11010EEF30919AF"), "")
    C.Check.str("kistack revision junk", K.KiStack.rev("{\"message\":\"API rate limit exceeded\"}"), "")
    C.Check.str("kistack revision none", K.KiStack.rev(""), "")
    C.Check.str("kistack have needs a complete copy", K.KiStack.have(False{}, "641ade9041bb0844c4530dfff11010eef30919af\n"), "")
    C.Check.str("kistack have", K.KiStack.have(True{}, "641ade9041bb0844c4530dfff11010eef30919af\n"), "641ade9041bb0844c4530dfff11010eef30919af")
    C.Check.str("kistack got", K.KiStack.got("noise\ngot 641ade9041bb0844c4530dfff11010eef30919af\n\n"), "641ade9041bb0844c4530dfff11010eef30919af")
    C.Check.str("kistack got local", K.KiStack.got("got local\n"), "local")
    C.Check.str("kistack got junk", K.KiStack.got("got ../x\n"), "")
    C.Check.str("kistack same", K.KiStack.got("up to date at 641ade9041bb0844c4530dfff11010eef30919af\n"), "")
    C.Check.str("kistack failed", K.KiStack.got("got 641ade9041bb0844c4530dfff11010eef30919af\nfail: x\n"), "")
    C.Check.str("kistack timed out", K.KiStack.got(""), "")
    C.Check.str("kistack done", K.KiStack.done("ok 71\n", "641a", "/h/kistack"), "kistack sync: 641a in /h/kistack (71 files)")
    C.Check.str("kistack done failed", K.KiStack.done("fail: a link or special file among the skills: refused\n", "641a", "/h/kistack"), "kistack sync: fail: a link or special file among the skills: refused")
    C.Check.str("kistack stopped", K.KiStack.done("", "641a", "/h/kistack"), "kistack sync: stopped (timed out)")
    C.Check.str("kistack put args", yes(String.ends_with(String.join(K.KiStack.put.args("/h", "641a", ["t/skills", "t/LICENSE"]), "|"),
      "|sh|/h/kistack|/h/kistack.sync|641a|" ++ K.KiStack.manifest() ++ "|t/skills|t/LICENSE")), "yes")
    C.Check.str("kistack get args", yes(String.ends_with(String.join(K.KiStack.get.args("/h", "", "641a"), "|"), "|sh|/h/kistack|/h/kistack.sync||641a|33554432|134217728")), "yes")
    # where it syncs: everywhere but a copied log, a test hub without a source, or when turned off
    C.Check.str("kistack sync on", K.KiStack.off("", False{}, "", ""), "")
    C.Check.str("kistack sync off", K.KiStack.off("0", False{}, "", ""), "BACKPLANE_KISTACK_SYNC=0")
    C.Check.str("kistack sync off source", K.KiStack.off("0", False{}, "/fake", "/src"), "BACKPLANE_KISTACK_SYNC=0")
    C.Check.str("kistack sync copy", K.KiStack.off("1", True{}, "", "/src"), "a hub on a copied log (BACKPLANE_COPY=1) reaches no network")
    C.Check.str("kistack sync test hub", K.KiStack.off("", False{}, "/fake", ""), "stand-in agents (BACKPLANE_AGENTS) and no BACKPLANE_KISTACK_SOURCE")
    C.Check.str("kistack sync test hub source", K.KiStack.off("", False{}, "/fake", "/src"), "")
    C.Check.str("kistack sync test hub asked", K.KiStack.off("1", False{}, "/fake", ""), "")
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
