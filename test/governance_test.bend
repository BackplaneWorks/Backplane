import Base
import ./check.bend as C
import ../src/core/governance.bend as G
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/plane.bend as PL

def bool(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def one() -> List<&2, M.Change>:
  [M.MessagePosted{"thread", "message", M.User{}, "never put this in the receipt", 9n}]

def receipt(+cs: List<&2, M.Change>) -> M.Change:
  match cs:
    case _ <> +r <> Nil{}:
      r
    case _:
      M.GovernanceLogged{"", "", "", "", "", "", "", 0n, 0n, "", False{}, False{}, False{}, 0n}

def round(+change: M.Change) -> Bool:
  String.eq(J.Json.show(K.Change.encode(change)),
    J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(change)), M.ProjectRemoved{"bad", 0n}))))

def main() -> IO(Unit):
  do IO<Unit>:
    +audited : List<&2, M.Change> = G.Governance.audit(10n, one())
    +r : M.Change = receipt(audited)
    +json : String = J.Json.show(K.Change.encode(r))
    +st : M.State = M.State.apply(M.State.empty(), r)
    +plane : PL.Plane = PL.Plane.of.st(st, 10n)
    +ledgers : G.Ledgers = G.Governance.ledgers(audited)
    +old : M.Change = M.GovernanceLogged{"gov:old", "BP/GOV/1", "person", "MessagePosted", "persist:MessagePosted", "approved", "", 1n, 1n, "", True{}, True{}, True{}, 8n}
    +oldLedgers : G.Ledgers = G.Governance.ledgers([M.MessagePosted{"thread", "old", M.User{}, "private", 7n}, old])
    C.Check.str("committee protocol is versioned", G.Governance.protocol(), "BP/GOV/3")
    C.Check.str("receipt carries its wire protocol", J.Json.get_str(K.Change.encode(r), "protocol", ""), "BP/GOV/3")
    C.Check.str("one durable batch gets one receipt", Nat.show(List.length(&2, M.Change, audited)), "2")
    C.Check.str("receipt sequence is stable", J.Json.get_str(K.Change.encode(r), "id", ""), "gov:10")
    C.Check.str("receipt identifies first change kind", J.Json.get_str(K.Change.encode(r), "action", ""), "MessagePosted")
    C.Check.str("receipt records person actor", J.Json.get_str(K.Change.encode(r), "actor", ""), "person")
    C.Check.str("receipt policy fields are valid", bool(G.Governance.approved(r)), "true")
    C.Check.str("receipt digest matches its payload", bool(G.Governance.digest.matches(audited)), "true")
    C.Check.str("receipt count and quorum validate", bool(G.Governance.accepted.go(audited, 1n, False{}, 0n, True{})), "true")
    C.Check.str("receipt includes all three approvals", bool(G.Governance.accepted(audited)), "true")
    C.Check.str("legacy wire shape is a known setting event", J.Json.get_str(K.Change.wire(r), "$", ""), "SettingSet")
    C.Check.str("legacy clients receive the latest receipt key", J.Json.get_str(K.Change.wire(r), "key", ""), "gov.receipt.latest")
    C.Check.str("hub projection exposes the same receipt", M.Setting.get(M.State.apply(M.State.empty(), r), "gov.receipt.latest", ""), M.Audit.value(r))
    C.Check.str("latest batch appears in shared status plane", Nat.show(List.length(&2, PL.PRow, PL.Plane.governance(plane))), "1")
    C.Check.str("plane row identifies its batch", PL.PRow.title(Maybe.default(&2, PL.PRow, List.head(&2, PL.PRow, PL.Plane.governance(plane)), PL.PRow{"", "", "", "", "", "", "", "", 0n, False{}})), "Batch gov:10")
    C.Check.str("review section uses the selected locale", PL.Plane.governance.label("ja"), "ガバナンス")
    C.Check.str("agent-readable plane includes governance section", bool(String.contains(PL.Plane.text(plane, "", "here", 10n, False{}), "Governance (1)")), "true")
    C.Check.str("status plane projection has a stable render certificate", Nat.show(String.length(PL.Plane.certificate(plane, 10n, False{}))) ++ bool(String.eq(PL.Plane.certificate(plane, 10n, False{}), PL.Plane.certificate(plane, 10n, False{}))), "64true")
    C.Check.str("audit is idempotent", Nat.show(List.length(&2, M.Change, G.Governance.audit(10n, audited))), "2")
    C.Check.str("receipt excludes message content", bool(Bool.not(String.contains(json, "never put this in the receipt"))), "true")
    C.Check.str("governance ledger contains only the receipt", bool(String.contains(G.Ledgers.governance(ledgers), "GovernanceLogged")) ++ bool(Bool.not(String.contains(G.Ledgers.governance(ledgers), "never put this in the receipt"))), "truetrue")
    C.Check.str("command envelope maps a message to its adapter", bool(String.contains(G.Ledgers.commands(ledgers), "conversation-projection")) ++ bool(String.contains(G.Ledgers.commands(ledgers), "\"actor\":\"person\"")), "truetrue")
    C.Check.str("entity mirror omits message payload", bool(String.contains(G.Ledgers.entities(ledgers), "Conversation")) ++ bool(Bool.not(String.contains(G.Ledgers.entities(ledgers), "never put this in the receipt"))), "truetrue")
    C.Check.str("legacy case and work units receive domain mirrors", bool(String.contains(String.join(G.Governance.entities([M.DecisionLogged{"case", "bot", "conv", "run", "private note", "reason", 9n}, M.WorkSet{"thread", "work", "private rows", 9n}]), "\n"), "Case")) ++ bool(String.contains(String.join(G.Governance.entities([M.DecisionLogged{"case", "bot", "conv", "run", "private note", "reason", 9n}, M.WorkSet{"thread", "work", "private rows", 9n}]), "\n"), "WorkUnit")), "truetrue")
    C.Check.str("projection ledger carries a verified SHA-256 certificate", bool(String.contains(G.Ledgers.projections(ledgers), "\"verified\":true")) ++ Nat.show(String.length(G.Governance.digest(one()))), "true64")
    C.Check.str("migration ledger records the current adapter", bool(String.contains(G.Ledgers.migrations(ledgers), "\"from\":\"BP/GOV/3\"")) ++ bool(String.contains(G.Ledgers.migrations(ledgers), "\"to\":\"BP/GOV/3\"")), "truetrue")
    C.Check.str("v1 certificate digest is recomputed during migration", bool(String.contains(G.Ledgers.projections(oldLedgers), "\"recomputed\":true")) ++ bool(String.contains(G.Ledgers.projections(oldLedgers), "\"verified\":true")), "truetrue")
    C.Check.str("migration ledger upgrades v1 protocol", bool(String.contains(G.Ledgers.migrations(oldLedgers), "\"from\":\"BP/GOV/1\"")) ++ bool(String.contains(G.Ledgers.migrations(oldLedgers), "\"to\":\"BP/GOV/3\"")), "truetrue")
    C.Check.str("receipt round trips through the event codec", bool(round(r)), "true")
    C.Check.str("legacy event still round trips", bool(round(M.ProjectRemoved{"old", 12n})), "true")
    C.Check.str("missing committee vote rejects batch", bool(G.Governance.accepted([M.MessagePosted{"thread", "message", M.User{}, "hi", 9n}, M.GovernanceLogged{"gov:bad", "BP/GOV/3", "person", "MessagePosted", "persist:MessagePosted", "approved", "", 3n, 1n, "", True{}, False{}, True{}, 9n}])), "false")
    C.Check.str("duplicate receipts reject a batch", bool(G.Governance.accepted(List.append(&2, M.Change, audited, [r]))), "false")
    C.Check.str("receipt count must match its batch", bool(G.Governance.accepted([M.MessagePosted{"thread", "message", M.User{}, "hi", 9n}, M.GovernanceLogged{"gov:bad", "BP/GOV/3", "person", "MessagePosted", "persist:MessagePosted", "approved", "", 3n, 2n, "", True{}, True{}, True{}, 9n}])), "false")
    C.Check.str("projection digest binds receipt to batch", bool(String.eq(J.Json.get_str(K.Change.encode(r), "digest", ""), G.Governance.digest(one()))), "true")
    +proposal : G.Proposal = G.Governance.proposal("p-1", "origin", 1n, ["peer-b", "peer-a"], one())
    +ballot : G.Ballot = G.Governance.vote(proposal, "peer-a")
    C.Check.str("proposal roster is frozen, sorted, and unique", String.join(G.Proposal.roster(proposal), ","), "peer-a,peer-b")
    C.Check.str("peer request carries action metadata only", bool(String.contains(J.Json.show(G.Proposal.metadata(proposal)), "MessagePosted")) ++ bool(Bool.not(String.contains(J.Json.show(G.Proposal.metadata(proposal)), "never put this in the receipt"))), "truetrue")
    C.Check.str("proposal digest binds metadata", bool(G.Governance.proposal.valid(proposal)), "true")
    C.Check.str("all three automatic roles approve a valid proposal", bool(G.Ballot.approved(ballot)), "true")
    +revote : G.Proposal = G.Governance.proposal("p-1:roster-1", "origin", 1n, ["origin", "peer-b"], one())
    C.Check.str("a roster revision changes the peer digest", bool(Bool.not(String.eq(G.Proposal.digest(proposal), G.Proposal.digest(revote)))), "true")
    +rejected : List<&2, M.Change> = G.Governance.audit.with("gov:appeal:2", one(), True{}, False{}, True{}, "reliability dissent")
    +rejection : M.Change = receipt(rejected)
    C.Check.str("second reading gets a distinct receipt id", J.Json.get_str(K.Change.encode(rejection), "id", ""), "gov:appeal:2")
    C.Check.str("a dissenting role records a rejected receipt", J.Json.get_str(K.Change.encode(rejection), "policy", ""), "rejected")
    C.Check.str("a dissenting second reading stays rejected", bool(G.Governance.accepted(rejected)), "false")
    +tampered : G.Proposal = G.Proposal{"p-1", "BP/GOV/3", "origin", 1n, G.Proposal.roster(proposal), ["MessagePosted"], 1n, 3n, "wrong"}
    C.Check.str("tampered proposal metadata is rejected", bool(G.Governance.proposal.valid(tampered)), "false")
    +queued : M.State = M.State.setting(st, "gov.queue.latest", "{\"id\":\"p-1\",\"state\":\"pending\",\"depth\":2,\"digest\":\"0123456789abcdef\",\"votes\":\"desk: yes\",\"appeal\":1,\"dissent\":\"\",\"at\":11}")
    +queuedPlane : PL.Plane = PL.Plane.of.st(queued, 11n)
    C.Check.str("pending queue shares the status plane", Nat.show(List.length(&2, PL.PRow, PL.Plane.governance(queuedPlane))), "2")
    C.Check.str("pending vote and appeal details are visible", bool(String.contains(PL.Plane.text(queuedPlane, "", "here", 11n, False{}), "desk: yes · second reading 1")), "true")
