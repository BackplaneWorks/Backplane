import Base
import ./model.bend as M
import ./json.bend as J

# One owner uses a hub home (paired phones and trusted desktop/web clients
# are that owner's surfaces, not distinct application accounts). Keep the
# bounded ledger here, never in telemetry, device storage or peer sync.
# Old logs start at zero: replaying arbitrary User messages would count
# agent mail, routine wakes and historical retries as human prompts.
def Star.key() -> String:
  "star.nudge.v1"

def Star.url() -> String:
  "https://github.com/BackplaneWorks/Backplane"

def Star.body() -> String:
  "Enjoying Backplane? A star on GitHub helps others find us."

def Star.read(raw: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(raw), J.Null{})

def Star.state(s: M.State) -> J.Json:
  Star.read(M.Setting.get(s, Star.key(), ""))

def Star.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case J.Str{x} <> rest: x <> Star.strs(rest)
    case _ <> rest: Star.strs(rest)

def Star.ids(o: J.Json) -> List<&2, String>:
  Star.strs(J.Json.elems(J.Json.get_obj(o, "ids")))

def Star.has(xs: List<&2, String>, +id: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case x <> rest: Bool.or(String.eq(x, id), Star.has(rest, id))

def Star.count(o: J.Json) -> Nat:
  List.length(&2, String, Star.ids(o))

def Star.token(o: J.Json) -> String:
  J.Json.get_str(o, "token", "")

def Star.valid.ids(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}: True{}
    case +id <> +rest: Bool.and(Bool.not(String.is_empty(id)), Bool.and(Bool.not(Star.has(rest, id)), Star.valid.ids(rest)))

def Star.valid.counting(+o: J.Json) -> Bool:
  +ids = Star.ids(o)
  +n = List.length(&2, String, ids)
  Bool.and(String.eq(J.Json.get_str(o, "status", ""), "counting"),
    Bool.and(Bool.and(Nat.is_gt(n, 0n), Nat.is_lt(n, 5n)),
      Bool.and(Star.valid.ids(ids), Bool.and(Nat.is_eq(n, List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(o, "ids")))),
        String.eq(Star.token(o), Maybe.default(&2, String, List.last(&2, String, ids), ""))))))

def Star.sent(o: J.Json) -> Bool:
  String.eq(J.Json.get_str(o, "status", ""), "sent")

# An absent key is a pre-feature installation. Unknown or damaged saved
# state fails closed; it must never silently reset a previous decision.
def Star.counting(+raw: String) -> Bool:
  Bool.or(String.is_empty(raw), Star.valid.counting(Star.read(raw)))

def Star.strings(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> rest: J.Str{x} <> Star.strings(rest)

def Star.json(+ids: List<&2, String>, +now: Nat, +token: String) -> J.Json:
  J.Json.obj([J.KV{"status", J.Str{Bool.pick(String, Nat.is_ge(List.length(&2, String, ids), 5n), "sent", "counting")}},
    J.KV{"ids", J.Json.arr(Star.strings(ids))},
    J.KV{"token", J.Str{token}}, J.KV{"eligibleAt", J.Json.nat(Bool.pick(Nat, Nat.is_ge(List.length(&2, String, ids), 5n), now, 0n))},
    J.KV{"sentAt", J.Json.nat(Bool.pick(Nat, Nat.is_ge(List.length(&2, String, ids), 5n), now, 0n))}])

# The caller supplies an accepted message id, never an RPC id or a text
# hash. Remember at most five ids so even a paged/deleted message retry
# cannot inflate the threshold. No work or writes after the one offer.
def Star.accept.k(ok: Bool, +raw: String, +id: String, +now: Nat) -> List<&2, M.Change>:
  match ok:
    case False{}: Nil{}
    case True{}:
      [M.SettingSet{Star.key(), J.Json.show(Star.json(List.take(&2, String, List.append(&2, String, Star.ids(Star.read(raw)), [id]), 5n), now, id)), now}]

def Star.accept(+s: M.State, +id: String, +now: Nat) -> List<&2, M.Change>:
  +raw = M.Setting.get(s, Star.key(), "")
  Star.accept.k(Bool.and(Star.counting(raw), Bool.and(Bool.not(String.is_empty(id)), Bool.and(Nat.is_lt(Star.count(Star.read(raw)), 5n), Bool.not(Star.has(Star.ids(Star.read(raw)), id))))), raw, id, now)

# Both actions are terminal, and only the offered token can decide. A
# stale client's dismissal/click cannot replace an already saved decision.
def Star.decide.k(ok: Bool, +o: J.Json, +token: String, +action: String, +now: Nat) -> List<&2, M.Change>:
  match ok:
    case False{}: Nil{}
    case True{}:
      [M.SettingSet{Star.key(), J.Json.show(J.Json.obj([J.KV{"status", J.Str{Bool.pick(String, String.eq(action, "open"), "opened", "dismissed")}},
        J.KV{"ids", J.Json.get_obj(o, "ids")}, J.KV{"token", J.Str{token}}, J.KV{"eligibleAt", J.Json.get_obj(o, "eligibleAt")},
        J.KV{"sentAt", J.Json.get_obj(o, "sentAt")}, J.KV{"decidedAt", J.Json.nat(now)}])), now}]

def Star.decide(+s: M.State, +token: String, +action: String, +now: Nat) -> List<&2, M.Change>:
  +o = Star.state(s)
  Star.decide.k(Bool.and(Star.sent(o), Bool.and(String.eq(Star.token(o), token), Bool.and(Bool.not(String.is_empty(token)), Bool.or(String.eq(action, "dismiss"), String.eq(action, "open"))))), o, token, action, now)
