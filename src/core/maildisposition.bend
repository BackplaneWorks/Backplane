import Base
import ./mailprivate.bend as PM
import ./model.bend as M
import ./mail.bend as ML
import ./mailcontinuity.bend as MC
import ./json.bend as J
import ./mailtriage.bend as TRI

def Runs.on(+s: M.State, +bot: String) -> Bool:
  Bool.or(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), TRI.Triage.owner(s, bot))

def Runs.keep(on: Bool, r: ML.Run) -> Maybe<&2, ML.Run>:
  match on:
    case True{}: Some{r}
    case False{}: None{}

def Runs.gate(+s: M.State, r: Maybe<&2, ML.Run>) -> Maybe<&2, ML.Run>:
  match r:
    case Some{+r}: Runs.keep(Bool.and(Runs.on(s, ML.Run.bot(r)), Bool.not(PM.Private.is(s, ML.Run.thread(r)))), r)
    case None{}: None{}

# A disposition settles ONE received notice, never the underlying work,
# conversation status, permission, provider, or notification policy.
def Disposition.key(+thread: String, +input: String) -> String:
  "run.disposition." ++ J.Json.show(J.Json.arr([J.Str{thread}, J.Str{input}]))

def Disposition.pending(+thread: String, +input: String) -> String:
  "run.disposition.pending." ++ J.Json.show(J.Json.arr([J.Str{thread}, J.Str{input}]))

def Disposition.current.k(triage: Bool, +s: M.State, +r: ML.Run) -> String:
  match triage:
    case True{}: TRI.Triage.current(s, r)
    case False{}: MC.Continuity.latest(ML.Box.headers(M.State.mail(s), ML.Run.conv(r)), ML.Run.bot(r), ML.Run.mail(r))

def Disposition.current(+s: M.State, +r: ML.Run) -> String:
  Disposition.current.k(TRI.Triage.owner(s, ML.Run.bot(r)), s, r)

def Disposition.handled(+s: M.State, +r: ML.Run) -> Bool:
  Bool.or(Bool.and(TRI.Triage.owner(s, ML.Run.bot(r)), String.is_empty(Disposition.current(s, r))), Bool.not(String.is_empty(M.Setting.get(s, Disposition.key(ML.Run.thread(r), Disposition.current(s, r)), ""))))

def Disposition.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}: True{}
    case _: False{}

def Disposition.live(+s: M.State, +r: ML.Run) -> Bool:
  Bool.and(Bool.and(Runs.on(s, ML.Run.bot(r)), Bool.not(PM.Private.is(s, ML.Run.thread(r)))),
    Bool.and(ML.Run.live(r), Disposition.running(M.Thread.turn(M.State.thread.get(s, ML.Run.thread(r))))))

def Disposition.matches(live: Bool, +input: String, +current: String) -> Bool:
  Bool.and(live, Bool.and(Bool.not(String.is_empty(input)), String.eq(current, input)))

def Disposition.valid(+s: M.State, +r: ML.Run, +input: String) -> Bool:
  Disposition.matches(Disposition.live(s, r), input, Disposition.current(s, r))

def Disposition.mode(+mode: String) -> Bool:
  Bool.or(String.eq(mode, "done"), Bool.or(String.eq(mode, "defer"), String.eq(mode, "reply")))

# Reject additional fields even if a client ignores inputSchema. This is
# not a general mail/decide/escalate/shell endpoint in disguise.
def Disposition.fields(j: J.Json) -> Bool:
  match j:
    case J.End{}: True{}
    case J.Field{+k, J.Str{_}, tail}:
      Bool.and(ML.Parts.has(["input", "mode", "reason", "text"], k, False{}), Disposition.fields(tail))
    case _: False{}

def Disposition.shape(j: J.Json) -> Bool:
  match j:
    case J.Obj{fields}: Disposition.fields(fields)
    case _: False{}

def Disposition.args(+a: J.Json) -> Bool:
  +reason = String.trim(J.Json.get_str(a, "reason", ""))
  +text = J.Json.get_str(a, "text", "")
  Bool.and(Disposition.shape(a), Bool.and(Disposition.mode(J.Json.get_str(a, "mode", "")),
    Bool.and(Bool.not(String.is_empty(reason)), Bool.and(Nat.is_le(String.length(reason), 2000n),
      Bool.and(Nat.is_le(String.length(text), 4000n), Bool.or(Bool.not(String.eq(J.Json.get_str(a, "mode", ""), "reply")), Bool.not(String.is_empty(String.trim(text)))))))))

def Disposition.receipt(+r: ML.Run, +input: String, +mode: String, +reason: String, +now: Nat) -> M.Change:
  M.SettingSet{Disposition.key(ML.Run.thread(r), input), J.Json.show(J.Json.obj([
    J.KV{"bot", J.Str{ML.Run.bot(r)}}, J.KV{"conv", J.Str{ML.Run.conv(r)}}, J.KV{"input", J.Str{input}},
    J.KV{"mode", J.Str{mode}}, J.KV{"reason", J.Str{reason}}])), now}

# Codex's supported PER-TOOL grant is emitted only for a live mail run.
# No server-wide, shell, sandbox, external-mail or product-action grant.
def Disposition.codex(on: Bool) -> List<&2, String>:
  match on:
    case False{}: Nil{}
    case True{}: ["-c", "mcp_servers.backplane.tools.mail_dispose.approval_mode=\"approve\""]

# Evidence reads and private journal/escalation tools for the coordinator's
# live triage ONLY. Keep the legacy single-disposition grant unchanged.
# No server default, shell, product action, external send or approval tool.
def Disposition.triage.tools() -> List<&2, String>:
  ["thread_read", "thread_list", "mail_context", "told_check", "memory_recall", "memory_list", "mail_list", "mail_read", "decide", "escalate"]

def Disposition.triage.flags(names: List<&2, String>) -> List<&2, String>:
  match names:
    case Nil{}: Nil{}
    case name <> rest:
      "-c" <> ("mcp_servers.backplane.tools." ++ name ++ ".approval_mode=\"approve\"") <> Disposition.triage.flags(rest)

def Disposition.codex.triage(on: Bool) -> List<&2, String>:
  match on:
    case False{}: Nil{}
    case True{}: Disposition.triage.flags(Disposition.triage.tools())

def Disposition.launch.got(r: Maybe<&2, ML.Run>, +s: M.State) -> Bool:
  match r:
    case None{}: False{}
    case Some{+r}: Disposition.live(s, r)

def Disposition.launch(+s: M.State, +thread: String) -> Bool:
  Disposition.launch.got(ML.Box.run_of(M.State.mail(s), thread), s)

# Never confer these extra tool grants on ordinary, retired or foreign runs.
def Disposition.triage.launch(+s: M.State, +thread: String) -> Bool:
  Bool.and(Disposition.launch(s, thread), TRI.Triage.is(s, thread))
