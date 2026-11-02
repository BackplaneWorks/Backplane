import Base
import ./json.bend as J
import ./fallback.bend as F
import ./model.bend as M

# The hub stores this snapshot in an ask. Any accepted write to either
# setting closes open proposals in the same serialized event batch, including
# a write that later restores a value; the answer path checks the snapshot.
type Proposal is Data:
  Proposal{old_on: String, old_chain: String, new_on: String, new_chain: String, expires: Nat}

def Proposal.on.valid(+s: String) -> Bool:
  Bool.or(String.eq(s, "on"), String.eq(s, "off"))

def Proposal.changed(p: Proposal) -> Bool:
  match p:
    case Proposal{old_on, old_chain, new_on, new_chain, _}:
      Bool.or(Bool.not(String.eq(old_on, new_on)), Bool.not(String.eq(old_chain, new_chain)))

def Proposal.chain.valid(+chain: String, +caps: J.Json) -> Bool:
  +targets = F.Chain.parse(chain)
  Bool.and(Nat.is_le(String.length(chain), 2000n), Bool.and(Nat.is_le(List.length(&2, F.Target, targets), 16n), F.Chain.valid(targets, caps)))

# Only explicitly supported settings enter this type. Other settings,
# accounts, credentials, and arbitrary agent-supplied metadata do not.
def Proposal.valid(+p: Proposal, +caps: J.Json) -> Bool:
  match p:
    case Proposal{old_on, +old_chain, +new_on, +new_chain, _}:
      Bool.and(Proposal.on.valid(old_on), Bool.and(Proposal.on.valid(new_on),
        Bool.and(Proposal.chain.valid(old_chain, caps), Bool.and(Proposal.chain.valid(new_chain, caps),
          Bool.or(String.eq(new_on, "off"), Bool.not(String.is_empty(new_chain)))))))

def Proposal.snapshot(p: Proposal, +now: Nat, +on: String, +chain: String) -> Bool:
  match p:
    case Proposal{old_on, old_chain, _, _, expires}:
      Bool.and(Nat.is_lt(now, expires), Bool.and(String.eq(old_on, on), String.eq(old_chain, chain)))

# The record is deliberately closed: it contains only the two supported
# settings and their guard, never a free-form rationale or credential field.
def Proposal.json(p: Proposal) -> J.Json:
  match p:
    case Proposal{old_on, old_chain, new_on, new_chain, expires}:
      J.Json.obj([J.KV{"old_on", J.Str{old_on}}, J.KV{"old_chain", J.Str{old_chain}},
        J.KV{"new_on", J.Str{new_on}}, J.KV{"new_chain", J.Str{new_chain}},
        J.KV{"expires", J.Json.nat(expires)}])

def Proposal.from(+o: J.Json) -> Proposal:
  Proposal{J.Json.get_str(o, "old_on", ""), J.Json.get_str(o, "old_chain", ""),
    J.Json.get_str(o, "new_on", ""), J.Json.get_str(o, "new_chain", ""), J.Json.get_nat(o, "expires", 0n)}

def Proposal.detail(p: Proposal) -> String:
  match p:
    case Proposal{old_on, old_chain, new_on, new_chain, expires}:
      "Hub-wide provider fallback proposal\nEnabled: " ++ old_on ++ " → " ++ new_on ++
      "\nHierarchy: " ++ old_chain ++ " → " ++ new_chain ++
      "\nValues are effective settings; an unset switch defaults to off and an unset hierarchy is empty. Unchanged settings are left untouched.\nExpires at Unix time " ++ Nat.show(expires) ++ ".\nWhen a provider limit is detected, active threads and bots may switch to a later eligible provider/model; an exhausted hierarchy holds work. This change does not itself start or interrupt a conversation."

# Called by the serialized event writer for every accepted fallback write,
# including a write that changes a value back to an earlier value. Closure is
# durable in the same event batch as the setting change. An ask already
# closed in that batch is left alone so its decision is never overwritten.
def Proposal.touched(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}: False{}
    case M.SettingSet{+key, _, _} <> rest:
      Bool.or(Bool.or(String.eq(key, "fallback.on"), String.eq(key, "fallback.chain")), Proposal.touched(rest))
    case _ <> rest: Proposal.touched(rest)

def Proposal.closed(cs: List<&2, M.Change>, +id: String) -> Bool:
  match cs:
    case Nil{}: False{}
    case M.AskClosed{+other, _, _} <> rest:
      Bool.or(String.eq(id, other), Proposal.closed(rest, id))
    case _ <> rest: Proposal.closed(rest, id)

def Proposal.invalidate.go(as: List<&2, M.Ask>, +cs: List<&2, M.Change>, +now: Nat) -> List<&2, M.Change>:
  match as:
    case Nil{}: Nil{}
    case M.Ask{+id, _, _, +tool, _, _, +open, _} <> rest:
      List.append(&2, M.Change,
        Bool.pick(List<&2, M.Change>, Bool.and(open, Bool.and(String.eq(tool, "settings_proposal"), Bool.not(Proposal.closed(cs, id)))),
          [M.AskClosed{id, "stale", now}], Nil{}), Proposal.invalidate.go(rest, cs, now))

def Proposal.invalidate(+cs: List<&2, M.Change>, as: List<&2, M.Ask>, now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Proposal.touched(cs), Proposal.invalidate.go(as, cs, now), Nil{})
