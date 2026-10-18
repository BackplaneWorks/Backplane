import Base
import ./json.bend as J

# The may grammar is deliberately small: one `may: tool:NAME` or
# `may: bash:COMMAND PREFIX` per line. No globs, implicit grants or aliases.
def Charter.default() -> String:
  "may: file, tag, snooze, answer status questions, relaunch a failed launched thread once\nescalate: everything else to the bot; money, external or irreversible actions to the person."

def Charter.any(+xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +h <> +t: Bool.or(String.contains(s, h), Charter.any(t, s))

def Charter.prefix(+s: String, +p: String) -> Bool:
  Bool.or(String.eq(s, p), String.starts_with(s, p ++ " "))

# Strip a single leading `cd DIR &&`; shell composition after that is
# conservatively risky and cannot be granted by a bash prefix. Wrappers
# and substitutions are risky too (a prefix must never authorize a tail).
def Charter.composed(+s: String) -> Bool:
  Charter.any([";", "&", "||", "|", "\n", "`", "$", "(", ")", "<", ">", "\\"], s)

def Charter.cd.pick(ok: Bool, +s: String, +tail: List<&2, String>) -> String:
  match ok:
    case True{}: String.trim(String.join(tail, "&"))
    case False{}: s

def Charter.after_cd(+xs: List<&2, String>, +s: String) -> String:
  match xs:
    case +dir <> SNil{} <> +tail: Charter.cd.pick(Bool.not(Charter.composed(dir)), s, tail)
    case _: s

def Charter.command.k(cd: Bool, +s: String) -> String:
  match cd:
    case False{}: s
    case True{}: Charter.after_cd(String.split(s, '&'), s)

def Charter.command(+input: J.Json) -> String:
  +s = String.trim(J.Json.get_str(input, "command", J.Json.get_str(input, "cmd", "")))
  Charter.command.k(String.starts_with(s, "cd "), s)

def Charter.bash_risky(+s: String) -> Bool:
  Bool.or(Charter.composed(s), Bool.or(
    Charter.any(["git push", "gh pr merge", "scripts/dev.sh install", "rm -", "git branch -D", " delete", "gh pr comment", "gh issue create", "thread delete", "curl ", "wget ", "boat new", "boat create", "order", "purchase"], s),
    Charter.any(["bash ", "sh ", "zsh ", "eval ", "exec ", "sudo ", "env ", "python", "node ", "bun ", "ruby ", "perl "], s)))

def Charter.bash(+tool: String) -> Bool:
  Charter.any(["Bash", "bash", "exec_command", "shell", "work_run", "terminal_open", "terminal_send"], tool)

def Charter.risky(+tool: String, +input: J.Json) -> Bool:
  Bool.or(Charter.any(["gmail_send", "gmail_reply", "send_email", "reply_email", "deploy_restart", "deploy_main", "purchase", "order", "store_", "sandbox_create", "boat_new", "boat_create", "delete", "webhook"], tool),
    Bool.or(Bool.and(Charter.bash(tool), Charter.bash_risky(Charter.command(input))),
      Bool.and(Charter.any(["room_post", "mail", "thread_send"], tool), Charter.any(["~", "peer:", ":room:", "https://", "http://"], J.Json.get_str(input, "to", J.Json.get_str(input, "room", ""))))))

def Charter.grant(+line: String, +tool: String, +input: J.Json) -> Bool:
  Bool.or(String.eq(line, "tool:" ++ tool), Bool.and(Charter.bash(tool), Bool.and(String.starts_with(line, "bash:"),
    Bool.and(Bool.not(Charter.composed(Charter.command(input))), Bool.and(Bool.not(String.is_empty(String.trim(String.drop(line, 5n)))), Charter.prefix(Charter.command(input), String.trim(String.drop(line, 5n))))))))

def Charter.lines(+xs: List<&2, String>, +tool: String, +input: J.Json) -> String:
  match xs:
    case Nil{}: ""
    case +h <> +t:
      Bool.pick(String, Bool.and(String.starts_with(String.trim(h), "may:"), Charter.grant(String.trim(String.drop(String.trim(h), 4n)), tool, input)), String.trim(h), Charter.lines(t, tool, input))

def Charter.line(+text: String, +tool: String, +input: J.Json) -> String:
  Charter.lines(String.split(text, '\n'), tool, input)

def Charter.allows(+text: String, +tool: String, +input: J.Json) -> Bool:
  Bool.not(String.is_empty(Charter.line(text, tool, input)))

def Charter.gate(+text: String, +tool: String, +input: J.Json) -> Bool:
  Bool.or(Bool.not(Charter.risky(tool, input)), Charter.allows(text, tool, input))
