import Base
import ./model.bend as M
import ./mail.bend as ML
import ./bot.bend as BT
import ./json.bend as J
import ./maildisposition.bend as MD

# Shared lookup: runs use their owner's memory and prompt, never a FOH
# session. Ordinary threads keep the pre-mail lookup.
def Run.bot.got(r: Maybe<&2, ML.Run>, +s: M.State, +thread: String) -> Maybe<&2, BT.Bot>:
  match r:
    case Some{+r}: BT.World.find.id(BT.World.bot(M.State.bots(s), ML.Run.bot(r)))
    case None{}: BT.World.of_thread(M.State.bots(s), thread)

def Run.of(+s: M.State, +thread: String) -> Maybe<&2, ML.Run>:
  MD.Runs.gate(s, ML.Box.run_of(M.State.mail(s), thread))

def Run.bot(+s: M.State, +thread: String) -> Maybe<&2, BT.Bot>:
  Run.bot.got(Run.of(s, thread), s, thread)

def Run.is.got(r: Maybe<&2, ML.Run>) -> Bool:
  match r:
    case Some{_}: True{}
    case None{}: False{}

def Run.is(+s: M.State, +thread: String) -> Bool:
  Run.is.got(Run.of(s, thread))

def Run.settings(s: M.State) -> List<&2, String>:
  match s:
    case M.State{_, _, _, _, _, sg, _, _, _, _, _}: Map.keys(&2, String, sg)

def Run.board.one(+s: M.State, +bot: String, +key: String) -> String:
  +j = Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, key, "")), J.Null{})
  Bool.pick(String, String.eq(J.Json.get_str(j, "owner", ""), bot), "[" ++ J.Json.get_str(j, "status", "") ++ "] " ++ J.Json.get_str(j, "title", "") ++ "\n", "")

def Run.board.key(on: Bool, +s: M.State, +bot: String, +key: String) -> String:
  match on:
    case True{}: Run.board.one(s, bot, key)
    case False{}: ""

def Run.board(+ks: List<&2, String>, +s: M.State, +bot: String) -> String:
  match ks:
    case Nil{}: ""
    case +k <> +t: Run.board.key(String.starts_with(k, "board."), s, bot, k) ++ Run.board(t, s, bot)

def Run.decisions(+ds: List<&2, ML.Decision>) -> String:
  match ds:
    case Nil{}: ""
    case +d <> +t: ML.Decision.text(d) ++ " — " ++ ML.Decision.reason(d) ++ "\n" ++ Run.decisions(t)

def Run.state.one(+t: M.Thread) -> String:
  match t:
    case M.Thread{+id, _, +title, _, _, M.Running{}, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: id ++ " running: " ++ title ++ "\n"
    case M.Thread{+id, _, +title, _, _, M.Failed{}, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: id ++ " failed: " ++ title ++ "\n"
    case _: ""

def Run.state(+ts: List<&2, M.Thread>) -> String:
  match ts:
    case Nil{}: ""
    case +h <> +t: Run.state.one(h) ++ Run.state(t)

def Run.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Map.values(&2, M.Thread, ts)

def Run.asks(+xs: List<&2, M.Ask>) -> String:
  match xs:
    case Nil{}: ""
    case M.Ask{_, +th, _, +tool, +detail, _, True{}, _} <> +t: th ++ " needs you: " ++ tool ++ " " ++ String.take(detail, 160n) ++ "\n" ++ Run.asks(t)
    case _ <> +t: Run.asks(t)

# 40k chars of fixed sections; bodies get 8k, newest first, then the
# triggering mail. Each section's oldest entries are discarded first.
def Run.disposition.got(r: Maybe<&2, ML.Run>, +s: M.State) -> String:
  match r:
    case None{}: ""
    case Some{+r}: "Current actionable input: " ++ MD.Disposition.current(s, r) ++ "\nFor a routine status, duplicate notice, or already-known hold needing no new action, use mail_dispose(input, done/defer, reason). It acknowledges only that notice, never permission or completion of the underlying objective. For a local FYI reply use mode reply with text; pending/unknown is not delivery and must not be resent. Use reviewed decide/escalate/mail for substantive decisions, new actions or external targets. Reading mail or final response text is not a durable disposition. A newer incoming actionable input needs its own exact ID.\n"

def Run.brief(+s: M.State, +bot: String, +conv: String) -> String:
  +c = ML.Box.conv(M.State.mail(s), conv)
  "<charter>\n" ++ ML.Box.charter(M.State.mail(s), bot) ++ "\n</charter>\n" ++
  String.take(BT.Mem.block(BT.World.mems(M.State.bots(s), bot), ML.Conv.subject(c), ML.Conv.last(c)), 8000n) ++
  "\n<task_board>\n" ++ String.take(Run.board(List.reverse(&2, String, Run.settings(s)), s, bot), 6000n) ++ "</task_board>\n" ++
  "<live_state>\n" ++ String.take(Run.asks(M.Asks.all(s)) ++ Run.state(List.reverse(&2, M.Thread, Run.threads(s))), 6000n) ++ "</live_state>\n" ++
  "<decisions>\n" ++ String.take(Run.decisions(ML.Box.decisions(M.State.mail(s), bot, 30n)), 6000n) ++ "</decisions>\n" ++
  "<conversation>\n" ++ conv ++ "\n" ++ ML.Conv.subject(c) ++ "\n" ++ String.join(ML.Conv.parts(c), ", ") ++ "\n</conversation>\n" ++
  "You are " ++ BT.Bot.name(BT.World.bot(M.State.bots(s), bot)) ++ "'s mailroom for this conversation. Act within the charter. Record each decision with `decide`. Anything outside the charter: `escalate`. Reply in the conversation with `mail`. End your turn when done.\n" ++
  Run.disposition.got(ML.Box.run_of(M.State.mail(s), ML.Box.live(M.State.mail(s), bot, conv)), s)

# Routing has one work action at most, irrespective of the mail source.
type MailRunPlan is Data:
  FileOnly{}
  NewRun{}
  IntoRun{thread: String}

def Run.plan.live(+live: String) -> MailRunPlan:
  match live:
    case SNil{}: NewRun{}
    case SCon{+c, +t}: IntoRun{SCon{c, t}}

def Run.plan.want(act: Bool, +live: String) -> MailRunPlan:
  match act:
    case False{}: FileOnly{}
    case True{}: Run.plan.live(live)

def Run.plan(+want: String, +live: String) -> MailRunPlan:
  Run.plan.want(String.eq(want, "act"), live)

def Run.actions(p: MailRunPlan) -> Nat:
  match p:
    case FileOnly{}: 0n
    case _: 1n

def Run.posts(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{_, _, _, _, _} <> +t: 1n+Run.posts(t)
    case _ <> +t: Run.posts(t)
