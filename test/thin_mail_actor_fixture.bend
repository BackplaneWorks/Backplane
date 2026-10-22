import Base
import ../src/server/thin-actor.bend as TA
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/mailview.bend as MV

# Synthetic source fixtures; no IO, providers, mailbox files or live state.
# Authored against thin-actor.bend e20ff9c7ad8eec6ac9d741e240e8d2cec88b6330d4aa54622f301815f6416226.
def obj(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def state() -> M.State:
  M.State.replay([M.ThreadCreated{"a", "", "Public A", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"b", "", "Public B", M.Local{}, "", "", "codex", "", 1n},
    M.BotSet{"ba", "Alpha", "a", 0n, "", 1n}, M.BotSet{"bb", "Beta", "b", 0n, "", 1n},
    M.UberSet{"ba", "public-uber", 1n}, M.SettingSet{"mail.on", "off", 1n},
    M.RoomSet{"ra", "Public room A", "ba", 1n}, M.RoomSet{"rb", "Public room B", "bb", 1n}], M.State.empty())

def target(+bot: String) -> J.Json:
  V.Target.json("bot", bot)

def request(+bot: String, +rid: Nat) -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"public-hub"}}, J.KV{"cid", J.Str{"public-device"}}, J.KV{"target", target(bot)},
    J.KV{"session", J.Json.obj([J.KV{"rid", J.Json.nat(rid)}, J.KV{"drafts", obj("{}")}, J.KV{"info", obj("{}")}, J.KV{"outbox", obj("{}")}])}])

def info() -> J.Json:
  obj("{\"origin\":\"public-epoch\",\"local.mailroom\":\"on\"}")

def prepared(+u: U.Ui, +conv: String) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@btab", "mail"), "@mr.conv", conv), "@mr.to", conv), "@mr.bot", U.Mailv.owner(u))

def actor(+bot: String, +view: String, +conv: String, +rid: Nat) -> TA.Actor:
  TA.Actor.make(prepared(TA.Actor.ui(TA.Actor.refresh(Nil{}, True{}, state(), info(), obj("{}"), 100n, request(bot, rid), view)), conv), view, target(bot))

def base() -> TA.Actor:
  actor("ba", "view-a", "room:ra", 100n)

def action(+a: TA.Actor, +name: String, +value: String) -> V.ActResult:
  V.Action.run(True{}, TA.Actor.ui(a), J.Json.obj([J.KV{"action", J.Str{name}}, J.KV{"value", J.Str{value}}]), 100n)

def step(+a: TA.Actor, +name: String, +value: String) -> TA.Actor:
  TA.Actor.accept(a, action(a, name, value), False{})

def field(a: TA.Actor, text: String) -> TA.Actor:
  step(a, "bfield", "mrreply" ++ SCon{Chr{31}, SNil{}} ++ text)

def sent() -> TA.Actor:
  step(field(base(), "captured é🙂\nsecond line"), "mail-send", "")

def refresh(+a: TA.Actor, p: J.Json, view: String) -> TA.Actor:
  TA.Actor.refresh([a], True{}, state(), info(), obj("{}"), 101n, p, view)

def with_ui(a: TA.Actor, next: U.Ui) -> TA.Actor:
  match a:
    case TA.Actor{_, view, target, pending, reads, dirty, memo}: TA.Actor{next, view, target, pending, reads, dirty, memo}

def commands(r: V.ActResult) -> List<&2, J.Json>:
  match r:
    case V.ActResult{_, cs}: cs

def command(+method: String, +id: String) -> J.Json:
  J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{J.Json.show(J.Json.obj([J.KV{"id", J.Num{id}}, J.KV{"m", J.Str{method}}, J.KV{"p", obj("{}")}]))}}])

def find(cs: List<&2, J.Json>, +method: String) -> J.Json:
  match cs:
    case Nil{}: J.Null{}
    case +c <> tail: Bool.pick(J.Json, String.eq(J.Json.get_str(TA.Actor.rpc(c), "m", ""), method), TA.Actor.rpc(c), find(tail, method))

def reply(+id: String, +ok: Bool, result: J.Json) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{ok}}, J.KV{"result", result}, J.KV{"error", J.Str{"public synthetic failure"}}])

def read(+a: TA.Actor) -> TA.Actor:
  TA.Actor.accept(a, V.ActResult.web(U.Mailv.read(TA.Actor.ui(a), "")), False{})

def body(+text: String) -> J.Json:
  J.Json.obj([J.KV{"conv", obj("{\"id\":\"room:ra\",\"parts\":[\"person\",\"bot:ba\"]}")},
    J.KV{"mails", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{"public-filed"}}, J.KV{"from", J.Str{"person"}}, J.KV{"to", J.Str{"room:ra"}},
      J.KV{"kind", J.Str{"room"}}, J.KV{"want", J.Str{"fyi"}}, J.KV{"at", J.Num{"102"}}, J.KV{"text", J.Str{text}}])])}, J.KV{"more", J.Flag{False{}}}])

def bubbles(bs: List<&2, MV.Bub>) -> String:
  match bs:
    case Nil{}: ""
    case MV.Bub{_, _, _, _, _, _, text, _} <> tail: text ++ bubbles(tail)

def methods(cs: List<&2, J.Json>) -> String:
  match cs:
    case Nil{}: ""
    case +c <> tail: J.Json.get_str(TA.Actor.rpc(c), "m", "") ++ "|" ++ methods(tail)

def only_reads(cs: List<&2, J.Json>) -> Bool:
  match cs:
    case Nil{}: True{}
    case c <> tail: Bool.and(TA.Actor.read_only(c), only_reads(tail))
