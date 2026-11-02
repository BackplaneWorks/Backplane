import Base
import ./check.bend as C
import ../src/core/settingsproposal.bend as P
import ../src/core/json.bend as J
import ../src/core/model.bend as M

def asks() -> List<&2, M.Ask>:
  [M.Ask{"p1", "t1", "approval", "settings_proposal", "", "", True{}, ""},
   M.Ask{"p2", "t2", "approval", "settings_proposal", "", "", True{}, ""},
   M.Ask{"ordinary", "t1", "approval", "Bash", "", "", True{}, ""}]

def invalidated(cs: List<&2, M.Change>) -> String:
  Nat.show(List.length(&2, M.Change, P.Proposal.invalidate(cs, asks(), 10n)))

def caps() -> J.Json:
  J.Json.obj([J.KV{"claude", J.Json.arr([J.Str{"sonnet"}])}, J.KV{"codex", J.Json.arr([J.Str{"model"}])}])

def proposal() -> P.Proposal:
  P.Proposal{"off", "claude/sonnet", "on", "claude/sonnet,codex/model", 100n}

def current(now: Nat, on: String, chain: String) -> String:
  Bool.show(P.Proposal.snapshot(proposal(), now, on, chain))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("current snapshot before deadline", current(99n, "off", "claude/sonnet"), "True")
    C.Check.str("deadline is exclusive", current(100n, "off", "claude/sonnet"), "False")
    C.Check.str("changed switch rejects", current(50n, "on", "claude/sonnet"), "False")
    C.Check.str("changed hierarchy rejects", current(50n, "off", "codex/model"), "False")
    C.Check.str("missing capability rejects", Bool.show(P.Proposal.valid(proposal(), J.Null{})), "False")
    C.Check.str("duplicate target rejects", Bool.show(P.Proposal.valid(P.Proposal{"off", "", "on", "codex/model,codex/model", 100n}, caps())), "False")
    C.Check.str("empty enabled hierarchy rejects", Bool.show(P.Proposal.valid(P.Proposal{"off", "", "on", "", 100n}, caps())), "False")
    C.Check.str("unchanged values are not a proposal", Bool.show(P.Proposal.changed(P.Proposal{"off", "", "off", "", 100n})), "False")
    C.Check.str("proposal round-trip preserves snapshot", Bool.show(P.Proposal.snapshot(P.Proposal.from(P.Proposal.json(proposal())), 50n, "off", "claude/sonnet")), "True")
    C.Check.str("malformed record cannot be accepted", Bool.show(P.Proposal.snapshot(P.Proposal.from(J.Null{}), 50n, "off", "claude/sonnet")), "False")
    C.Check.str("fallback write invalidates both proposals only", invalidated([M.SettingSet{"fallback.chain", "codex/model", 10n}]), "2")
    C.Check.str("unrelated setting preserves proposals", invalidated([M.SettingSet{"sidebar.view", "active", 10n}]), "0")
    C.Check.str("decided proposal retains its decision", invalidated([M.AskClosed{"p1", "accept", 10n}, M.SettingSet{"fallback.on", "off", 10n}]), "1")
