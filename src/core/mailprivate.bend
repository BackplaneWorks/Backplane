import Base
import ./model.bend as M
import ./bot.bend as BT
import ./uber.bend as UB
import ./mail.bend as ML
import ./json.bend as J

# A coordinator's back office is a different persisted thread and provider
# session. Its final/streamed output can never be the person's chat output.
# Settings keep the existing event and mobile kept-state schemas unchanged.
def Private.thread(bot: String) -> String:
  "mailcoord-" ++ bot

def Private.key(thread: String) -> String:
  "mail.private.owner." ++ thread

def Private.owner.got(+s: M.State, +thread: String, +bot: String) -> String:
  Bool.pick(String, Bool.and(UB.Uber.is(s, bot), String.starts_with(thread, Private.thread(bot) ++ "-")), bot, "")

def Private.owner(+s: M.State, +thread: String) -> String:
  Private.owner.got(s, thread, M.Setting.get(s, Private.key(thread), ""))

def Private.is(+s: M.State, +thread: String) -> Bool:
  Bool.not(String.is_empty(Private.owner(s, thread)))

def Private.live(+s: M.State, +thread: String) -> Bool:
  Bool.and(Private.is(s, thread), M.Work.turn.runs(M.Thread.turn(M.State.thread.get(s, thread))))

def Private.target(+s: M.State, +thread: String) -> String:
  BT.Bot.thread(BT.World.bot(M.State.bots(s), Private.owner(s, thread)))

def Private.bot.got(r: Maybe<&2, ML.Run>, +s: M.State, thread: String) -> String:
  match r:
    case Some{r}: ML.Run.bot(r)
    case None{}: BT.Bot.id(BT.World.bot(M.State.bots(s), Private.owner(s, thread)))

def Private.caller.got(b: Maybe<&2, BT.Bot>, +s: M.State, +thread: String) -> String:
  match b:
    case Some{b}: BT.Bot.id(b)
    case None{}: Private.bot.got(ML.Box.run_of(M.State.mail(s), thread), s, thread)

def Private.caller(+s: M.State, +thread: String) -> String:
  Private.caller.got(BT.World.of_thread(M.State.bots(s), thread), s, thread)

def Private.readable(+s: M.State, +caller: String, +target: String) -> Bool:
  Bool.or(Bool.not(Private.is(s, target)), String.eq(Private.caller(s, caller), Private.owner(s, target)))

def Private.told.key(+bot: String, +key: String) -> String:
  "mail.private.told." ++ J.Json.show(J.Json.arr([J.Str{bot}, J.Str{key}]))

def Private.fields(j: J.Json) -> Bool:
  match j:
    case J.End{}: True{}
    case J.Field{+k, J.Str{_}, tail}:
      Bool.and(ML.Parts.has(["text", "key"], k, False{}), Private.fields(tail))
    case _: False{}

def Private.shape(j: J.Json) -> Bool:
  match j:
    case J.Obj{f}: Private.fields(f)
    case _: False{}

def Private.args(+a: J.Json) -> Bool:
  +text = String.trim(J.Json.get_str(a, "text", ""))
  +key = String.trim(J.Json.get_str(a, "key", ""))
  Bool.and(Private.shape(a), Bool.and(Bool.not(String.is_empty(text)), Bool.and(Nat.is_le(String.length(text), 32000n),
    Bool.and(Bool.not(String.is_empty(key)), Nat.is_le(String.length(key), 200n)))))

# Named publication/history grants only; no shell, approval or product-action
# grant. Existing owner runtime/sandbox and source approvals remain unchanged.
def Private.codex(on: Bool) -> List<&2, String>:
  match on:
    case False{}: Nil{}
    case True{}: ["-c", "mcp_servers.backplane.tools.tell_person.approval_mode=\"approve\"", "-c", "mcp_servers.backplane.tools.mail_context.approval_mode=\"approve\""]

# This is a runtime contract as well as an agent instruction: a completed
# private turn owes no visible reply. Only the explicit tell_person tool
# publishes into the original conversation. No final-text parsing is used.
def Private.brief() -> String:
  "\n<private_mailroom_turn allow_empty_output=\"true\">\nYou are the owning coordinator in its private back office, not the person's conversation. Resolve mail and coordinate within existing authorization using your normal tools, owner memory and mail_context. All commentary, tool activity and final text stay in this private lane. Empty final output is successful: do not retry or re-prompt merely to produce visible output. Routine acknowledgments and already-told items need no user message. Only when you explicitly choose to tell the person, call tell_person(text, key) with your OWN contextual message and actionable source links. This is the only chat-publication path; a final response, escalation or decision is not publication. Use a stable exact event/version key to avoid duplicate tells. Actual source approvals remain pending; no message or decision grants consent. Never answer an approval, resume a hold, or infer authorization from a report.\n</private_mailroom_turn>\n"
