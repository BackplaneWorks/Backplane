import Base
import ./json.bend as J
import ./prompt.bend as Q
import ./model.bend as M

# Agent features, pure: user and project skills (SKILL.md frontmatter, the
# skills block Codex, Grok and Muse are told, `$skill` mentions), reasoning
# effort per provider, and the one-shot prompts that name a worktree's
# branch and write commit messages. The server does the IO around these.

# User and project skills
# -----------------------
# `~/.claude/skills/*/SKILL.md` (user scope) and
# `<cwd>/.claude/skills/*/SKILL.md` (project scope). `model`: the model may
# invoke it on its own (not disable-model-invocation); `user`: the user may
# invoke it with `$name` (user-invocable, the default).

type USkill is Data:
  USkill{name: String, description: String, path: String, model: Bool, user: Bool}

type UFront is Data:
  UFront{name: String, description: String, dmi: String, ui: String}

def UFront.step(f: UFront, +line: String) -> UFront:
  match f:
    case UFront{n, d, m, u}:
      UFront{Maybe.default(&2, String, Q.Skill.field(line, "name"), n), Maybe.default(&2, String, Q.Skill.field(line, "description"), d),
        Maybe.default(&2, String, Q.Skill.field(line, "disable-model-invocation"), m), Maybe.default(&2, String, Q.Skill.field(line, "user-invocable"), u)}

# fold frontmatter lines until the closing ---; `stop` is the head's verdict
def UFront.go(ls: List<&2, String>, stop: Bool, f: UFront) -> UFront:
  match ls stop:
    case +l <> +t False{}:
      UFront.go(t, Q.Front.stop(t), UFront.step(f, l))
    case _ _:
      f

def UFront.from(+t: List<&2, String>, +dflt: String) -> UFront:
  UFront.go(t, Q.Front.stop(t), UFront{dflt, "", "", ""})

def UFront.open(open: Bool, +t: List<&2, String>, +dflt: String) -> UFront:
  match open:
    case True{}:
      UFront.from(Q.Front.fold(t), dflt)
    case False{}:
      UFront{dflt, "", "", ""}

def UFront.of(ls: List<&2, String>, +dflt: String) -> UFront:
  match ls:
    case +l <> t:
      UFront.open(String.eq(String.trim(l), "---"), t, dflt)
    case Nil{}:
      UFront{dflt, "", "", ""}

# a YAML boolean, or d when it is neither
def USkill.flag(+v: String, d: Bool) -> Bool:
  Bool.pick(Bool, String.eq(String.to_lower(v), "true"), True{}, Bool.pick(Bool, String.eq(String.to_lower(v), "false"), False{}, d))

def USkill.parse.k(f: UFront, path: String) -> USkill:
  match f:
    case UFront{n, d, +m, +u}:
      USkill{n, d, path, Bool.not(USkill.flag(m, False{})), USkill.flag(u, True{})}

# a SKILL.md: its frontmatter, the directory name when it names nothing
def USkill.parse(text: String, +dflt: String, path: String) -> USkill:
  USkill.parse.k(UFront.of(String.lines(text), dflt), path)

def USkill.name(s: USkill) -> String:
  match s:
    case USkill{n, _, _, _, _}:
      n

def USkill.has(ss: List<&2, USkill>, +n: String) -> Bool:
  match ss:
    case Nil{}:
      False{}
    case USkill{m, _, _, _, _} <> t:
      Bool.or(String.eq(m, n), USkill.has(t, n))

def USkill.unseen.k(seen: Bool, s: USkill, rest: List<&2, USkill>) -> List<&2, USkill>:
  match seen:
    case True{}:
      rest
    case False{}:
      s <> rest

def USkill.unseen(ps: List<&2, USkill>, +us: List<&2, USkill>) -> List<&2, USkill>:
  match ps:
    case Nil{}:
      Nil{}
    case USkill{+n, d, p, m, u} <> t:
      USkill.unseen.k(USkill.has(us, n), USkill{n, d, p, m, u}, USkill.unseen(t, us))

# user skills, then the project's; the user's wins a name both use
def USkill.merge(+us: List<&2, USkill>, ps: List<&2, USkill>) -> List<&2, USkill>:
  List.append(&2, USkill, us, USkill.unseen(ps, us))

def USkill.line.k(keep: Bool, +n: String, +d: String, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      ("- " ++ n ++ ": " ++ d ++ " Read \"" ++ p ++ "\"") <> rest
    case False{}:
      rest

def USkill.lines(ss: List<&2, USkill>) -> List<&2, String>:
  match ss:
    case Nil{}:
      Nil{}
    case USkill{+n, +d, +p, m, _} <> t:
      USkill.line.k(m, n, d, p, USkill.lines(t))

def USkill.block(+ls: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, ls), "",
    "\n\n<skills>\nThese skills are installed (Claude Code SKILL.md format). When a task matches a skill's description, read that skill's complete SKILL.md before starting and follow it. Resolve paths it mentions relative to its directory.\n"
    ++ String.join(ls, "\n") ++ "\n</skills>")

# the skills block for providers that do not read skills themselves
# (Codex, Grok): the skills the model may invoke, with their paths; "" for
# none
def USkill.prompt(ss: List<&2, USkill>) -> String:
  USkill.block(USkill.lines(ss))

def USkill.user_only.k(keep: Bool, s: USkill, rest: List<&2, USkill>) -> List<&2, USkill>:
  match keep:
    case True{}:
      s <> rest
    case False{}:
      rest

# the skills a user may name with `$`
def USkill.user_only(ss: List<&2, USkill>) -> List<&2, USkill>:
  match ss:
    case Nil{}:
      Nil{}
    case USkill{n, d, p, m, +u} <> t:
      USkill.user_only.k(u, USkill{n, d, p, m, u}, USkill.user_only(t))

def USkill.json.go(ss: List<&2, USkill>) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case USkill{n, d, _, _, _} <> t:
      J.Json.obj([J.KV{"name", J.Str{n}}, J.KV{"description", J.Str{d}}]) <> USkill.json.go(t)

# what the composer offers after `$`: name and description of each skill
# the user may invoke
def USkill.json(ss: List<&2, USkill>) -> J.Json:
  J.Json.arr(USkill.json.go(USkill.user_only(ss)))

# `$skill` mentions
# ----------------

# a character of a skill name: letters, digits, - _ :
def Mention.namec(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)), U32.is_eq(c, 58))))

def Mention.stop(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, _}:
      Bool.not(Mention.namec(c))

def Mention.take.go(s: String, stop: Bool, acc: String) -> String:
  match s stop:
    case _ True{}:
      String.reverse(acc)
    case SNil{} False{}:
      String.reverse(acc)
    case SCon{c, +t} False{}:
      Mention.take.go(t, Mention.stop(t), SCon{c, acc})

# the skill a word names: "$pcb-review," -> "pcb-review"; "" for a word
# that is no mention
def Mention.name(+w: String) -> String:
  Bool.pick(String, String.starts_with(w, "$"), Mention.take.go(String.drop(w, 1n), Mention.stop(String.drop(w, 1n)), ""), "")

def Mention.find(ss: List<&2, USkill>, +n: String) -> Maybe<&2, USkill>:
  match ss:
    case Nil{}:
      None{}
    case USkill{+m, d, p, mo, +u} <> t:
      Bool.pick(Maybe<&2, USkill>, Bool.and(u, String.eq(m, n)), Some{USkill{m, d, p, mo, u}}, Mention.find(t, n))

def Mention.slash.k(m: Maybe<&2, USkill>, +w: String) -> String:
  match m:
    case Some{_}:
      "/" ++ String.drop(w, 1n)
    case None{}:
      w

# for Claude: `$name` of a known skill becomes `/name`, Claude Code's own
# skill invocation; any other `$word` stays
def Mention.slash(+ss: List<&2, USkill>, +w: String) -> String:
  Bool.pick(String, String.is_empty(Mention.name(w)), w, Mention.slash.k(Mention.find(ss, Mention.name(w)), w))

def Mention.words(ws: List<&2, String>, +ss: List<&2, USkill>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      Mention.slash(ss, w) <> Mention.words(t, ss)

def Mention.lines(ls: List<&2, String>, +ss: List<&2, USkill>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      String.join(Mention.words(String.split(l, ' '), ss), " ") <> Mention.lines(t, ss)

def Mention.claude(+text: String, +ss: List<&2, USkill>) -> String:
  String.join(Mention.lines(String.lines(text), ss), "\n")

def Mention.note(s: USkill) -> String:
  match s:
    case USkill{n, _, p, _, _}:
      "The user invoked the $" ++ n ++ " skill: read \"" ++ p ++ "\" completely before anything else and follow its instructions for this request."

def Mention.seen(xs: List<&2, String>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, n), Mention.seen(t, n))

def Mention.seen.add(m: Maybe<&2, USkill>, +n: String, +seen: List<&2, String>) -> List<&2, String>:
  match m:
    case Some{_}:
      n <> seen
    case None{}:
      seen

def Mention.notes.one(m: Maybe<&2, USkill>, +n: String, +seen: List<&2, String>, +rest: List<&2, String>) -> List<&2, String>:
  match m:
    case Some{s}:
      Bool.pick(List<&2, String>, Mention.seen(seen, n), rest, Mention.note(s) <> rest)
    case None{}:
      rest

# the note for each skill the words name, once each, in order
def Mention.notes(ws: List<&2, String>, +ss: List<&2, USkill>, +seen: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      Mention.notes.one(Mention.find(ss, Mention.name(w)), Mention.name(w), seen, Mention.notes(t, ss, Mention.seen.add(Mention.find(ss, Mention.name(w)), Mention.name(w), seen)))

def Mention.all_words(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, String, String.split(l, ' '), Mention.all_words(t))

def Mention.others.k(+notes: List<&2, String>, +text: String) -> String:
  Bool.pick(String, List.is_empty(&2, String, notes), text, "<skill_invocations>\n" ++ String.join(notes, "\n") ++ "\n</skill_invocations>\n\n" ++ text)

# for providers without skills of their own: the text as written, after
# one instruction per named skill to read its SKILL.md
def Mention.others(+text: String, +ss: List<&2, USkill>) -> String:
  Mention.others.k(Mention.notes(Mention.all_words(String.lines(text)), ss, Nil{}), text)

def Mention.expand.k(has: Bool, claude: Bool, +text: String, +ss: List<&2, USkill>) -> String:
  match has claude:
    case False{} _:
      text
    case True{} True{}:
      Mention.claude(text, ss)
    case True{} False{}:
      Mention.others(text, ss)

# a user message with its `$skill` mentions made real for the provider
def Mention.expand(+provider: String, +text: String, +ss: List<&2, USkill>) -> String:
  Mention.expand.k(String.contains(text, "$"), String.eq(provider, "claude"), text, ss)

# Reasoning effort
# ----------------
# Per thread, in the settings map under effort.<thread>; "" is the
# provider's default. Claude: --effort; Codex: model_reasoning_effort;
# Grok: --reasoning-effort; Muse: turn/start's reasoningEffort (msp.bend);
# Pi: --thinking; Oh My Pi: the session's "thinking" option (omp.bend).
# Which levels a model takes comes with the models (about.efforts).

def Effort.key(+thread: String) -> String:
  "effort." ++ thread

def Effort.levels(+provider: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.eq(provider, "claude"), ["low", "medium", "high", "xhigh", "max"],
    Bool.pick(List<&2, String>, String.eq(provider, "codex"), ["low", "medium", "high", "xhigh"],
      Bool.pick(List<&2, String>, String.eq(provider, "grok"), ["low", "medium", "high"],
        Bool.pick(List<&2, String>, String.eq(provider, "muse"), ["minimal", "low", "medium", "high", "xhigh", "max"],
          Bool.pick(List<&2, String>, String.eq(provider, "pi"), ["off", "minimal", "low", "medium", "high", "xhigh", "max"],
            Bool.pick(List<&2, String>, String.eq(provider, "omp"), ["off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"], Nil{}))))))

def Effort.in(xs: List<&2, String>, +e: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, e), Effort.in(t, e))

def Effort.keep.k(keep: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      x <> rest
    case False{}:
      rest

# the levels in both lists (in the first one's order)
def Effort.keep(xs: List<&2, String>, +ys: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Effort.keep.k(Effort.in(ys, x), x, Effort.keep(t, ys))

# the effort a provider takes: e when it offers it, else its default ("")
def Effort.of(+provider: String, +e: String) -> String:
  Bool.pick(String, Effort.in(Effort.levels(provider), e), e, "")

def Effort.args.k(i: Nat, +e: String) -> List<&2, String>:
  match i:
    case 0n:
      ["--effort", e]
    case 1n:
      ["-c", "model_reasoning_effort=\"" ++ e ++ "\""]
    case 2n:
      ["--reasoning-effort", e]
    case 4n:
      ["--thinking", e]
    case _:
      Nil{}

def Effort.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Effort.pos(t, x, 1n+i))

# the flags that set a provider's effort (none for its default)
def Effort.args(+provider: String, +e: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(Effort.of(provider, e)), Nil{},
    Effort.args.k(Effort.pos(["claude", "codex", "grok", "muse", "pi"], provider, 0n), Effort.of(provider, e)))

# One-shot writing: thread titles, branch names and commit messages
# -----------------------------------------------------------------
# A light model with no tools, no MCP and no session, from the text
# generation settings: text.provider (claude, codex, grok or muse; claude
# for anything else) and text.model ("" or unset: the provider's light
# model). Claude and Codex read the text on stdin under a system prompt;
# Grok's and Muse's headless modes take no system prompt, so they read both.

type Tx is Data:
  Tx{provider: String, model: String}

def Text.providers() -> List<&2, String>:
  ["claude", "codex", "grok", "muse"]

def Text.norm.k(i: Nat) -> String:
  match i:
    case 1n:
      "codex"
    case 2n:
      "grok"
    case 3n:
      "muse"
    case _:
      "claude"

# the provider named, or claude
def Text.norm(+p: String) -> String:
  Text.norm.k(Effort.pos(Text.providers(), p, 0n))

def Text.light.k(i: Nat) -> String:
  match i:
    case 1n:
      "gpt-6-luna"
    case 2n:
      "grok-4.7-build-fast"
    case 3n:
      "muse-spark-1.3"
    case _:
      "haiku"

# a provider's light model, used when text.model is empty
def Text.light(+provider: String) -> String:
  Text.light.k(Effort.pos(Text.providers(), provider, 0n))

def Text.digit(m: Maybe<&2, Char>) -> Bool:
  match m:
    case Some{Chr{+c}}:
      Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))
    case None{}:
      False{}

def Text.is_claude(+m: String) -> Bool:
  Bool.or(String.starts_with(m, "claude-"), Bool.or(Bool.or(String.eq(m, "haiku"), String.eq(m, "sonnet")), Bool.or(String.eq(m, "opus"), String.eq(m, "fable"))))

def Text.is_codex(+m: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(m, "gpt-"), String.starts_with(m, "codex-")), Bool.and(String.starts_with(m, "o"), Text.digit(String.get(m, 1n))))

def Text.family.k(claude: Bool, codex: Bool, grok: Bool, muse: Bool) -> String:
  match claude codex grok muse:
    case True{} _ _ _:
      "claude"
    case False{} True{} _ _:
      "codex"
    case False{} False{} True{} _:
      "grok"
    case False{} False{} False{} True{}:
      "muse"
    case False{} False{} False{} False{}:
      ""

# whose model a name is: claude (haiku, sonnet, opus, fable, claude-*),
# codex (gpt-*, codex-*, o3...), grok (grok-*), muse (muse-*), or "" when
# it is not known
def Text.family(+m: String) -> String:
  Text.family.k(Text.is_claude(m), Text.is_codex(m), String.starts_with(m, "grok-"), String.starts_with(m, "muse-"))

# a model the provider can run: its own, or one not known to be another's
# (a text.model left over from another provider does not fit)
def Text.fits(+provider: String, +model: String) -> Bool:
  Bool.or(String.is_empty(Text.family(model)), String.eq(Text.family(model), provider))

def Text.pick.m(keep: Bool, +provider: String, +model: String) -> String:
  match keep:
    case True{}:
      model
    case False{}:
      Text.light(provider)

def Text.pick.k(+provider: String, +model: String) -> Tx:
  Tx{provider, Text.pick.m(Bool.and(Bool.not(String.is_empty(model)), Text.fits(provider, model)), provider, model)}

# the effective provider and model for a provider and model setting: the
# provider's light model when the model is empty or another provider's
def Text.pick(+provider: String, +model: String) -> Tx:
  Text.pick.k(Text.norm(String.trim(provider)), String.trim(model))

# the text generation model the settings choose
def Text.of(+st: M.State) -> Tx:
  Text.pick(M.Setting.get(st, "text.provider", "claude"), M.Setting.get(st, "text.model", ""))

def Tx.provider(t: Tx) -> String:
  match t:
    case Tx{p, _}:
      p

def Tx.model(t: Tx) -> String:
  match t:
    case Tx{_, m}:
      m

# the effective text.provider and text.model (what a settings UI shows)
def Text.provider(+st: M.State) -> String:
  Tx.provider(Text.of(st))

def Text.model(+st: M.State) -> String:
  Tx.model(Text.of(st))

def OneShot.flag(+flag: String, +model: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(model), Nil{}, [flag, model])

def OneShot.claude(+model: String, +system: String) -> List<&2, String>:
  List.append(&2, String, ["60", "claude", "-p"], List.append(&2, String, OneShot.flag("--model", model),
    ["--output-format", "json", "--no-session-persistence", "--tools", "", "--strict-mcp-config", "--effort", "low", "--system-prompt", system]))

# the prompt is the system text; codex appends stdin as a <stdin> block
def OneShot.codex(+model: String, +system: String) -> List<&2, String>:
  List.append(&2, String, ["60", "codex", "exec", "--json", "--ephemeral", "--skip-git-repo-check", "--sandbox", "read-only", "-c", "model_reasoning_effort=\"low\""],
    List.append(&2, String, OneShot.flag("-m", model), [system]))

# grok reads a prompt file; stdin is a socket, so cat makes it a pipe
def OneShot.grok(+model: String) -> List<&2, String>:
  List.append(&2, String, ["60", "sh", "-c", "cat | exec grok --prompt-file /dev/stdin \"$@\"", "sh", "--output-format", "json", "--no-subagents",
    "--disable-web-search", "--permission-mode", "dontAsk", "--max-turns", "1", "--reasoning-effort", "low"], OneShot.flag("-m", model))

# muse exec takes the prompt as an argument (it refuses a pipe as a prompt
# file) and prints the answer alone; its tool output may not live under
# the workspace, /tmp, so TMPDIR moves
def OneShot.muse(+model: String) -> List<&2, String>:
  List.append(&2, String, ["60", "sh", "-c", "TMPDIR=/var/tmp exec muse exec \"$@\" -- \"$(cat)\" 2>/dev/null", "sh", "--no-session-log", "--disable-shell",
    "--disable-write", "--disable-web-tools", "--disable-reminders", "--approval-mode", "never", "--max-model-steps", "1", "--reasoning-effort", "low"], OneShot.flag("--model", model))

def OneShot.args.k(i: Nat, +model: String, +system: String) -> List<&2, String>:
  match i:
    case 1n:
      OneShot.codex(model, system)
    case 2n:
      OneShot.grok(model)
    case 3n:
      OneShot.muse(model)
    case _:
      OneShot.claude(model, system)

# the arguments for timeout(1) that run one piece of writing
def OneShot.args(tx: Tx, +system: String) -> List<&2, String>:
  match tx:
    case Tx{+p, +m}:
      OneShot.args.k(Effort.pos(Text.providers(), p, 0n), m, system)

def OneShot.input.k(grok: Bool, +system: String, +text: String) -> String:
  match grok:
    case True{}:
      system ++ "\n\n<input>\n" ++ text ++ "\n</input>"
    case False{}:
      text

# what the writer reads on stdin
def OneShot.input(+provider: String, +system: String, +text: String) -> String:
  OneShot.input.k(Bool.or(String.eq(provider, "grok"), String.eq(provider, "muse")), system, text)

def OneShot.pick.k(m: Maybe<&2, J.Json>, rest: String) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, Bool.or(J.Json.get_bool(o, "is_error", False{}), Bool.not(String.eq(J.Json.get_str(o, "type", ""), "result"))), rest, J.Json.get_str(o, "result", ""))
    case None{}:
      rest

def OneShot.pick(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> +t:
      Bool.pick(String, String.starts_with(String.trim(l), "{"), OneShot.pick.k(J.Json.parse(String.trim(l)), OneShot.pick(t)), OneShot.pick(t))

# the model's answer, or "" when it failed
def OneShot.result(+out: String) -> String:
  String.trim(OneShot.pick(String.lines(out)))

def OneShot.cx.k(m: Maybe<&2, J.Json>, +acc: String) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, Bool.and(String.eq(J.Json.get_str(o, "type", ""), "item.completed"), String.eq(J.Json.get_str(J.Json.get_obj(o, "item"), "type", ""), "agent_message")),
        J.Json.get_str(J.Json.get_obj(o, "item"), "text", ""), acc)
    case None{}:
      acc

# codex exec --json: the last agent message among the JSONL events
def OneShot.cx(ls: List<&2, String>, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      OneShot.cx(t, OneShot.cx.k(J.Json.parse(String.trim(l)), acc))

def OneShot.gk.k(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, String.eq(J.Json.get_str(o, "stopReason", ""), "end_turn"), J.Json.get_str(o, "text", ""), "")
    case None{}:
      ""

def OneShot.answer.k(i: Nat, +out: String) -> String:
  match i:
    case 1n:
      String.trim(OneShot.cx(String.lines(out), ""))
    case 2n:
      String.trim(OneShot.gk.k(J.Json.parse(String.trim(out))))
    case 3n:
      String.trim(out)
    case _:
      OneShot.result(out)

# a provider's answer, or "" when it failed
def OneShot.answer(+provider: String, +out: String) -> String:
  OneShot.answer.k(Effort.pos(Text.providers(), provider, 0n), out)

# Thread titles
# -------------
# A thread's first message titles it at once (its first line, hub.bend's
# Hub.title.of); the writer's title replaces that while it still stands.

def Title.system() -> String:
  "You title threads in a coding agent's sidebar. The user's first message to the agent follows. Do not answer it, do not do the work and do not use tools. Reply with only a title of 3 to 7 words naming the change or question the user wants, in sentence case, without quotes, markdown or a trailing period."

def Title.keep(empty: Bool, w: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      w <> rest

def Title.words(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +w <> t:
      Title.keep(String.is_empty(w), w, Title.words(t))

# leading # and spaces (a markdown heading)
def Title.unhash(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, +t}:
      Bool.pick(String, Bool.or(U32.is_eq(c, 35), U32.is_eq(c, 32)), Title.unhash(t), SCon{Chr{c}, t})

def Title.prefix(+s: String) -> String:
  Bool.pick(String, String.starts_with(String.to_lower(s), "title:"), String.trim(String.drop(s, 6n)), s)

def Title.unquote(+s: String) -> String:
  Bool.pick(String, Bool.and(Nat.is_ge(String.length(s), 2n), Bool.and(String.starts_with(s, "'"), String.ends_with(s, "'"))),
    String.trim(String.take(String.drop(s, 1n), Nat.sub(String.length(s), 2n))), s)

def Title.unperiod(+s: String) -> String:
  Bool.pick(String, Bool.and(String.ends_with(s, "."), Bool.not(String.ends_with(s, ".."))), String.trim(String.take(s, Nat.sub(String.length(s), 1n))), s)

def Title.squash(+s: String) -> String:
  String.join(Title.words(String.split(String.join(String.split(s, '\t'), " "), ' ')), " ")

def Title.bare(+s: String) -> String:
  String.join(String.split(String.join(String.split(String.join(String.split(s, '`'), ""), '"'), ""), '*'), "")

def Title.cut(long: Bool, +line: String) -> String:
  match long:
    case True{}:
      String.take(line, 57n) ++ "..."
    case False{}:
      line

def Title.first(+answer: String) -> String:
  String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(String.trim(answer))), ""))

# the writer's answer as a title: its first line without quotes, markdown,
# a "Title:" label or a trailing period, spaces collapsed, at most 60
# characters like Hub.title.of; "" for none
def Title.clean(+answer: String) -> String:
  +l = Title.unperiod(Title.squash(Title.unquote(Title.prefix(Title.unhash(Title.bare(Title.first(answer)))))))
  Title.cut(Nat.is_gt(String.length(l), 60n), l)

def Name.system() -> String:
  "You name git branches. The user's first message to a coding agent follows. Reply with only a short branch name for that work: 2 to 5 lowercase English words joined by hyphens, at most 40 characters, no prefix, no quotes, no punctuation, nothing else."

def Name.hex.k(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102)))

def Name.hex(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Name.hex.k(c), Name.hex(t))

# a worktree's temporary branch (backplane/<8 hex>), which the agent renames
def Name.temp(+b: String) -> Bool:
  Bool.and(Bool.and(String.starts_with(b, "backplane/"), Nat.is_eq(String.length(b), 18n)), Name.hex(String.drop(b, 10n)))

def Name.strip(+s: String) -> String:
  String.trim(String.join(String.split(String.join(String.split(String.join(String.split(s, '`'), ""), '"'), ""), '\''), ""))

# the model's answer as a branch fragment: its first line, no quotes, words
# joined by hyphens (git.bend sanitizes what is left); "" for none
def Name.fragment(+answer: String) -> String:
  String.take(String.join(String.split(String.to_lower(Name.strip(Maybe.default(&2, String, List.head(&2, String, String.lines(String.trim(answer))), ""))), ' '), "-"), 48n)

# the branch a named thread moves to
def Name.branch(+fragment: String) -> String:
  "backplane/" ++ fragment

def Btw.system() -> String:
  "You answer a quick side question about a coding agent's thread. The thread so far and the question follow. Answer from what the thread shows, in a few sentences, in plain text. Do not do the work, do not use tools and do not repeat the question. If the thread does not tell, say so."

def Commit.system() -> String:
  "You write git commit messages. A unified diff follows, after the title of the task that produced it. Reply with only the commit message: a subject line in the imperative mood under 72 characters, then, only if the change needs it, a blank line and a short body wrapped at 72 characters. No quotes, no code fences, no trailers."

# the text the commit writer reads: the task, then the diff (cut short)
def Commit.input(+title: String, +diff: String) -> String:
  "Task: " ++ title ++ "\n\n" ++ String.take(diff, 30000n)

def Commit.fence(+l: String) -> Bool:
  String.starts_with(String.trim(l), "```")

def Commit.unfence(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> +t:
      Bool.pick(List<&2, String>, Commit.fence(l), Commit.unfence(t), l <> Commit.unfence(t))

# the model's answer as a commit message, or fallback when it gave none
def Commit.message(+answer: String, +fallback: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(String.join(Commit.unfence(String.lines(answer)), "\n"))), fallback,
    String.trim(String.join(Commit.unfence(String.lines(answer)), "\n")))
