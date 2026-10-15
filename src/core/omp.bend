import Base
import ./model.bend as M
import ./json.bend as J
import ./acp.bend as AC

# oh-my-pi (omp, core/omp.bend): the approval modes it takes on its ACP
# command line, the thinking levels it offers, the chat models it lists
# from `omp models --json` and its install line. One process
# per turn: `omp acp --approval-mode <mode>`. Pure: no IO here.

# the --approval-mode argument for a runtime mode
def Omp.approval(r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}:
      "always-ask"
    case M.AcceptEdits{}:
      "write"
    case M.AutoMode{}:
      "write"
    case M.FullAccess{}:
      "yolo"

# the argument vector after the `omp` command
def Omp.args(r: M.RuntimeMode) -> List<&2, String>:
  ["acp", "--approval-mode", Omp.approval(r)]

# the thinking levels omp accepts: the union of every model's (an omp model
# takes a subset, see Omp.efforts)
def Omp.levels() -> List<&2, String>:
  ["off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"]

# one entry's selector ahead of the rest, when it is a chat model
def Omp.chat.one(on: Bool, +m: J.Json, +rest: List<&2, String>) -> List<&2, String>:
  match on:
    case True{}:
      J.Json.get_str(m, "selector", "") <> rest
    case False{}:
      rest

def Omp.chat.rows(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +m <> t:
      Omp.chat.one(String.eq(J.Json.get_str(m, "kind", ""), "chat"), m, Omp.chat.rows(t))

# the parsed `omp models --json` ("null" when the text is not JSON)
def Omp.catalog(+text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

# the selectors of the chat models in a parsed catalog
def Omp.models.j(+doc: J.Json) -> List<&2, String>:
  Omp.chat.rows(J.Json.elems(J.Json.get_obj(doc, "models")))

# the selectors of the chat models in `omp models --json` ("" when the text
# is not JSON or has no models)
def Omp.models(+text: String) -> List<&2, String>:
  Omp.models.j(Omp.catalog(text))

# one chat model's thinking levels as omp's ACP takes them: off and auto,
# then the model's own list; a model without reasoning takes none
def Omp.efforts.k(reasoning: Bool, +m: J.Json) -> J.Json:
  match reasoning:
    case True{}:
      J.Json.arr(List.append(&2, J.Json, [J.Json.str("off"), J.Json.str("auto")], J.Json.elems(Maybe.default(&2, J.Json, J.Json.get(m, "thinking"), J.Null{}))))
    case False{}:
      J.Json.arr(Nil{})

def Omp.efforts.one(on: Bool, +m: J.Json, +rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match on:
    case True{}:
      J.KV{J.Json.get_str(m, "selector", ""), Omp.efforts.k(J.Json.get_bool(m, "reasoning", False{}), m)} <> rest
    case False{}:
      rest

def Omp.efforts.rows(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}:
      Nil{}
    case +m <> t:
      Omp.efforts.one(String.eq(J.Json.get_str(m, "kind", ""), "chat"), m, Omp.efforts.rows(t))

# the thinking levels of each chat model in a parsed catalog, by selector,
# and "default" (the provider's own model: none sent, so no levels)
def Omp.efforts.j(+doc: J.Json) -> List<&2, J.KV>:
  J.KV{"default", J.Json.arr(Nil{})} <> Omp.efforts.rows(J.Json.elems(J.Json.get_obj(doc, "models")))

def Omp.efforts(+text: String) -> List<&2, J.KV>:
  Omp.efforts.j(Omp.catalog(text))

def Omp.install() -> String:
  "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`."

# A turn
# ------
# initialize (1), then session/new (2) or session/load (2), then the config
# steps: the model (session/set_config_option, configId "model", id 4) when
# one is chosen, then the thinking level ("thinking", id 5) when one is
# chosen, then the prompt (3). A session/load that the agent cannot answer
# opens a new session (session/new, id 7) with a note in the turn; its
# config steps use ids 14 and 15, so the prompt knows the session is new
# and gets Backplane's instructions. Updates and permission requests are
# Grok's (AC.Acp.line). The model and the thinking level come with the turn
# (POmp, msp.bend), not through AcpTurn.

type OmpTurn is Data:
  OmpTurn{t: AC.AcpTurn, model: String, effort: String}

def Omp.acp(+t: OmpTurn) -> AC.AcpTurn:
  match t:
    case OmpTurn{a, _, _}:
      a

def Omp.model(+t: OmpTurn) -> String:
  match t:
    case OmpTurn{_, m, _}:
      m

def Omp.effort(+t: OmpTurn) -> String:
  match t:
    case OmpTurn{_, _, e}:
      e

# a turn with no session yet: its session is new
def Omp.fresh(+t: OmpTurn) -> Bool:
  String.is_empty(AC.Acp.session(Omp.acp(t)))

def Omp.sid(a: AC.Acp) -> String:
  match a:
    case AC.Acp{s, _, _}:
      s

def Omp.with_sid(a: AC.Acp, +sid: String) -> AC.Acp:
  match a:
    case AC.Acp{_, b, d}:
      AC.Acp{sid, b, d}

def Omp.set.rpc(+id: Nat, +sid: String, +option: String, +value: String) -> String:
  AC.Acp.rpc(id, "session/set_config_option", J.Json.obj([J.KV{"sessionId", J.Str{sid}}, J.KV{"configId", J.Str{option}}, J.KV{"value", J.Str{value}}]))

# the prompt: a new session gets Backplane's instructions first
def Omp.prompt(fresh: Bool, +t: OmpTurn, +sid: String) -> String:
  AC.Acp.say.k(fresh, Omp.acp(t), sid)

# the thinking level (config id base + 1) when one is chosen, else the prompt
def Omp.think.k(noeffort: Bool, fresh: Bool, +base: Nat, +t: OmpTurn, +sid: String) -> List<&2, String>:
  match noeffort:
    case True{}:
      [Omp.prompt(fresh, t, sid)]
    case False{}:
      [Omp.set.rpc(Nat.add(base, 1n), sid, "thinking", Omp.effort(t))]

def Omp.think(fresh: Bool, +base: Nat, +t: OmpTurn, +sid: String) -> List<&2, String>:
  Omp.think.k(String.is_empty(Omp.effort(t)), fresh, base, t, sid)

# the first config step once the session is open: the model (config id
# base) when one is chosen, else the thinking level
def Omp.start.k(nomodel: Bool, fresh: Bool, +base: Nat, +t: OmpTurn, +sid: String) -> List<&2, String>:
  match nomodel:
    case True{}:
      Omp.think(fresh, base, t, sid)
    case False{}:
      [Omp.set.rpc(base, sid, "model", Omp.model(t))]

def Omp.start(fresh: Bool, +base: Nat, +t: OmpTurn, +sid: String) -> List<&2, String>:
  Omp.start.k(String.is_empty(Omp.model(t)), fresh, base, t, sid)

# the session is open (new, resumed or renewed): note it, then the config steps
def Omp.opened(fresh: Bool, +base: Nat, a: AC.Acp, +t: OmpTurn, +sid: String) -> AC.AcpOut:
  AC.AcpOut{Omp.with_sid(a, sid), Omp.start(fresh, base, t, sid), [AC.Acp.event("thread.started", [J.KV{"thread_id", J.Str{sid}}])], Nil{}}

# the model is set: the thinking level, else the prompt
def Omp.model.done(fresh: Bool, +base: Nat, +a: AC.Acp, +t: OmpTurn) -> AC.AcpOut:
  AC.AcpOut{a, Omp.think(fresh, base, t, Omp.sid(a)), Nil{}, Nil{}}

# the thinking level is set: the prompt
def Omp.thinking.done(fresh: Bool, +a: AC.Acp, +t: OmpTurn) -> AC.AcpOut:
  AC.AcpOut{a, [Omp.prompt(fresh, t, Omp.sid(a))], Nil{}, Nil{}}

def Omp.renew.rpc(t: AC.AcpTurn) -> String:
  match t:
    case AC.AcpTurn{cwd, mcp, _, _, _, _}:
      AC.Acp.rpc(7n, "session/new", J.Json.obj([J.KV{"cwd", J.Str{cwd}}, J.KV{"mcpServers", AC.Acp.mcp(mcp)}]))

def Omp.note() -> String:
  "The earlier session could not be resumed, so this turn starts a new one."

# a session/load the agent cannot answer: a new session, noted in the turn
def Omp.renew(+a: AC.Acp, +t: OmpTurn) -> AC.AcpOut:
  AC.AcpOut{a, [Omp.renew.rpc(Omp.acp(t))], [AC.Acp.event("item.completed", [J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"backplane_note"}}, J.KV{"text", J.Str{Omp.note()}}])}])], Nil{}}

def Omp.failed(a: AC.Acp, +o: J.Json) -> AC.AcpOut:
  AC.Acp.fail(a, "omp login", AC.Acp.error.named(o, "omp"))

# a failed session/load of a resumed session opens afresh; a new session
# that fails ends the turn
def Omp.reopen(fresh: Bool, a: AC.Acp, +t: OmpTurn, +o: J.Json) -> AC.AcpOut:
  match fresh:
    case True{}:
      Omp.failed(a, o)
    case False{}:
      Omp.renew(a, t)

def Omp.reply.sid(+o: J.Json, +t: OmpTurn) -> String:
  J.Json.get_str(J.Json.get_obj(o, "result"), "sessionId", AC.Acp.session(Omp.acp(t)))

# replies to our requests, by id: the session (2, or 7 renewed), each config
# step (4 and 5; 14 and 15 renewed), else Grok's own handling (1 and 3)
def Omp.reply.k(id: Nat, ok: Bool, a: AC.Acp, +t: OmpTurn, +line: String, +o: J.Json) -> AC.AcpOut:
  match id ok:
    case 2n True{}:
      Omp.opened(Omp.fresh(t), 4n, a, t, Omp.reply.sid(o, t))
    case 2n False{}:
      Omp.reopen(Omp.fresh(t), a, t, o)
    case 7n True{}:
      Omp.opened(True{}, 14n, a, t, Omp.reply.sid(o, t))
    case 4n True{}:
      Omp.model.done(Omp.fresh(t), 4n, a, t)
    case 14n True{}:
      Omp.model.done(True{}, 14n, a, t)
    case 5n True{}:
      Omp.thinking.done(Omp.fresh(t), a, t)
    case 15n True{}:
      Omp.thinking.done(True{}, a, t)
    case _ False{}:
      Omp.failed(a, o)
    case _ _:
      AC.Acp.line.login(a, Omp.acp(t), "omp login", line)

def Omp.line.k(req: Bool, a: AC.Acp, +t: OmpTurn, +line: String, +o: J.Json) -> AC.AcpOut:
  match req:
    case True{}:
      AC.Acp.line.login(a, Omp.acp(t), "omp login", line)
    case False{}:
      Omp.reply.k(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(o, "id"), "0")), 0n), String.eq(J.Json.show(J.Json.get_obj(o, "error")), "null"), a, t, line, o)

def Omp.line.parsed(m: Maybe<&2, J.Json>, a: AC.Acp, +t: OmpTurn, +line: String) -> AC.AcpOut:
  match m:
    case Some{+o}:
      Omp.line.k(Bool.not(String.is_empty(J.Json.get_str(o, "method", ""))), a, t, line, o)
    case None{}:
      AC.Acp.line.login(a, Omp.acp(t), "omp login", line)

# one line from the agent
def Omp.line(a: AC.Acp, +t: OmpTurn, +line: String) -> AC.AcpOut:
  Omp.line.parsed(J.Json.parse(line), a, t, line)
