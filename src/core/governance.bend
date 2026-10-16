import Base
import ./model.bend as M
import ./codec.bend as C
import ./json.bend as J
import ./sha256.bend as SHA
import ./utf8.bend as U8
import ./governance_schema.bend as GS

type Ledgers is Data:
  Ledgers{governance: String, projections: String, migrations: String, commands: String, entities: String}

type Command is Data:
  Command{protocol: String, actor: String, action: String, entity: String, target: String, effect: String, digest: String, at: Nat}

type Mirror is Data:
  Mirror{protocol: String, source: String, entity: String, id: String, at: Nat}

# The peer ballot deliberately contains only proposal metadata. The original
# change batch stays on its owning hub until every frozen roster member votes.
type Proposal is Data:
  Proposal{id: String, protocol: String, origin: String, sequence: Nat, roster: List<&2, String>, tags: List<&2, String>, count: Nat, schema: Nat, digest: String}

type Ballot is Data:
  Ballot{peer: String, architecture: Bool, reliability: Bool, experience: Bool, dissent: String}

def Ledgers.governance(l: Ledgers) -> String:
  match l:
    case Ledgers{g, _, _, _, _}:
      g

def Ledgers.projections(l: Ledgers) -> String:
  match l:
    case Ledgers{_, p, _, _, _}:
      p

def Ledgers.migrations(l: Ledgers) -> String:
  match l:
    case Ledgers{_, _, m, _, _}:
      m

def Ledgers.commands(l: Ledgers) -> String:
  match l:
    case Ledgers{_, _, _, c, _}:
      c

def Ledgers.entities(l: Ledgers) -> String:
  match l:
    case Ledgers{_, _, _, _, e}:
      e

# BP/GOV/3 binds a content-free proposal to a frozen peer roster. Its digest
# covers metadata only; it never fingerprints event bodies sent to a peer.
def Governance.protocol() -> String:
  GS.GovSchema.protocol()

def Governance.queue_limit() -> Nat:
  GS.GovSchema.queue_limit()

def Governance.tag(c: M.Change) -> String:
  J.Json.get_str(C.Change.encode(c), "$", "unknown")

def Governance.at.go(+cs: List<&2, M.Change>, +at: Nat) -> Nat:
  match cs:
    case Nil{}:
      at
    case c <> t:
      Governance.at.go(t, J.Json.get_nat(C.Change.encode(c), "at", at))

def Governance.at(+cs: List<&2, M.Change>) -> Nat:
  Governance.at.go(cs, 0n)

def Governance.actor(c: M.Change) -> String:
  match c:
    case M.MessagePosted{_, _, M.User{}, _, _}:
      "person"
    case M.MessagePosted{_, _, M.Assistant{}, _, _}:
      "agent"
    case _:
      "system"

def Governance.is_record(c: M.Change) -> Bool:
  match c:
    case M.GovernanceLogged{_, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      True{}
    case _:
      False{}

def Governance.quorum(architecture: Bool, reliability: Bool, experience: Bool) -> Bool:
  Bool.and(architecture, Bool.and(reliability, experience))

def Governance.has.go(+cs: List<&2, M.Change>, +seq: Nat) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.GovernanceLogged{id, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Bool.or(String.eq(id, "gov:" ++ Nat.show(seq)), Governance.has.go(t, seq))
    case _ <> t:
      Governance.has.go(t, seq)

def Governance.approved.fields(+protocol: String, +policy: String, +dissent: String, +schema: Nat, +changes: Nat,
    architecture: Bool, reliability: Bool, experience: Bool) -> Bool:
  Bool.and(String.eq(protocol, Governance.protocol()), Bool.and(String.eq(policy, "approved"),
    Bool.and(Nat.is_eq(schema, GS.GovSchema.version()), Bool.and(Bool.not(Nat.is_eq(changes, 0n)),
      Bool.and(String.is_empty(dissent), Governance.quorum(architecture, reliability, experience))))))

def Governance.approved(c: M.Change) -> Bool:
  match c:
    case M.GovernanceLogged{_, protocol, _, _, _, policy, dissent, schema, changes, _, architecture, reliability, experience, _}:
      Governance.approved.fields(protocol, policy, dissent, schema, changes, architecture, reliability, experience)
    case _:
      False{}

def Governance.non_audit(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case c <> t:
      Nat.add(Bool.pick(Nat, Governance.is_record(c), 0n, 1n), Governance.non_audit(t))

def Governance.all.rows(+cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(C.Change.encode(c)) <> Governance.all.rows(t)

def Governance.nibble(+n: U32) -> String:
  SCon{J.Json.hex(n), SNil{}}

def Governance.hex.byte(+n: U32) -> String:
  Governance.nibble(U32.shrn(n, 4n)) ++ Governance.nibble(U32.and(n, 15))

def Governance.hex.bytes(xs: List<&2, U32>, +out: String) -> String:
  match xs:
    case Nil{}:
      out
    case x <> t:
      Governance.hex.bytes(t, out ++ Governance.hex.byte(x))

def Governance.digest(+cs: List<&2, M.Change>) -> String:
  Governance.hex.bytes(SHA.Sha256.hash(U8.Utf8.encode(String.join(Governance.all.rows(cs), "\n"))), "")

def Governance.in.go(names: List<&2, String>, +tag: String) -> Bool:
  match names:
    case Nil{}:
      False{}
    case name <> t:
      Bool.or(String.eq(name, tag), Governance.in.go(t, tag))

def Governance.entity(+tag: String) -> String:
  Bool.pick(String, Governance.in.go(["WorkSet", "WorktreeSet", "WorktreeSkipped", "WorktreeRemoved"], tag), "WorkUnit",
    Bool.pick(String, Governance.in.go(["DecisionLogged", "AskOpened", "AskClosed"], tag), "Case",
      Bool.pick(String, Governance.in.go(["ConvSet", "MailPosted", "MailMarked", "MailTagged", "RunSet", "CharterSet"], tag), "Conversation",
        Bool.pick(String, Governance.in.go(["ProjectCreated", "ProjectRemoved", "ProjectOrdered"], tag), "Project",
          Bool.pick(String, Governance.in.go(["ThreadCreated", "ThreadRenamed", "MessagePosted", "ActivityLogged", "TurnChanged", "PendingChanged", "ThreadSettled", "ThreadUnsettled", "ThreadArchived", "ThreadPinned", "ThreadDeleted"], tag), "Conversation", "System")))))

def Governance.effect(+tag: String) -> String:
  Bool.pick(String, Governance.in.go(["WorkSet", "WorktreeSet", "WorktreeSkipped", "WorktreeRemoved"], tag), "work-unit-projection",
    Bool.pick(String, Governance.in.go(["ConvSet", "MailPosted", "MailMarked", "MailTagged", "RunSet", "CharterSet", "DecisionLogged"], tag), "conversation-projection",
      Bool.pick(String, Governance.in.go(["ProjectCreated", "ProjectRemoved", "ProjectOrdered"], tag), "project-projection",
        Bool.pick(String, Governance.in.go(["ThreadCreated", "ThreadRenamed", "MessagePosted", "ActivityLogged", "TurnChanged", "PendingChanged"], tag), "conversation-projection", "state-fold"))))

def Governance.target(+o: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(o, "thread", "")),
    Bool.pick(String, String.is_empty(J.Json.get_str(o, "conv", "")),
      Bool.pick(String, String.is_empty(J.Json.get_str(o, "project", "")), J.Json.get_str(o, "id", ""), J.Json.get_str(o, "project", "")),
      J.Json.get_str(o, "conv", "")), J.Json.get_str(o, "thread", ""))

def Governance.command(+c: M.Change) -> Command:
  +o = C.Change.encode(c)
  +tag = Governance.tag(c)
  Command{Governance.protocol(), Governance.actor(c), tag, Governance.entity(tag), Governance.target(o), Governance.effect(tag), Governance.digest([c]), J.Json.get_nat(o, "at", 0n)}

def Governance.command.row(cmd: Command) -> String:
  match cmd:
    case Command{protocol, actor, action, entity, target, effect, digest, at}:
      J.Json.show(J.Json.obj([J.KV{"protocol", J.Str{protocol}}, J.KV{"actor", J.Str{actor}}, J.KV{"action", J.Str{action}},
        J.KV{"entity", J.Str{entity}}, J.KV{"target", J.Str{target}}, J.KV{"effect", J.Str{effect}},
        J.KV{"digest", J.Str{digest}}, J.KV{"at", J.Num{Nat.show(at)}}]))

def Governance.mirror(+c: M.Change) -> Mirror:
  +o = C.Change.encode(c)
  +tag = Governance.tag(c)
  Mirror{Governance.protocol(), tag, Governance.entity(tag), Governance.target(o), J.Json.get_nat(o, "at", 0n)}

def Governance.mirror.row(m: Mirror) -> String:
  match m:
    case Mirror{protocol, source, entity, id, at}:
      J.Json.show(J.Json.obj([J.KV{"protocol", J.Str{protocol}}, J.KV{"source", J.Str{source}}, J.KV{"entity", J.Str{entity}},
        J.KV{"id", J.Str{id}}, J.KV{"at", J.Num{Nat.show(at)}}]))

def Governance.commands.go(+cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      Governance.command.row(Governance.command(c)) <> Governance.commands.go(t)

def Governance.commands(+cs: List<&2, M.Change>) -> List<&2, String>:
  Governance.commands.go(cs)

def Governance.entities.go(+cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      Governance.mirror.row(Governance.mirror(c)) <> Governance.entities.go(t)

def Governance.entities(+cs: List<&2, M.Change>) -> List<&2, String>:
  Governance.entities.go(cs)

def Governance.payload.go(+cs: List<&2, M.Change>, +out: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      List.reverse(&2, M.Change, out)
    case c <> t:
      Bool.pick(List<&2, M.Change>, Governance.is_record(c), Governance.payload.go(t, out), Governance.payload.go(t, c <> out))

def Governance.digest.matches.go(+cs: List<&2, M.Change>, +payload: List<&2, M.Change>, found: Bool, ok: Bool) -> Bool:
  match cs:
    case Nil{}:
      Bool.and(found, ok)
    case M.GovernanceLogged{_, _, _, _, _, _, _, _, _, digest, _, _, _, _} <> t:
      Governance.digest.matches.go(t, payload, True{}, Bool.and(ok, String.eq(digest, Governance.digest(payload))))
    case c <> t:
      Governance.digest.matches.go(t, payload, found, ok)

def Governance.digest.matches(+cs: List<&2, M.Change>) -> Bool:
  Governance.digest.matches.go(cs, Governance.payload.go(cs, Nil{}), False{}, True{})

def Governance.accepted.go(+cs: List<&2, M.Change>, +expected: Nat, found: Bool, count: Nat, ok: Bool) -> Bool:
  match cs:
    case Nil{}:
      Bool.and(Bool.and(found, Nat.is_eq(count, 1n)), ok)
    case M.GovernanceLogged{_, protocol, _, _, _, policy, dissent, schema, changes, _, architecture, reliability, experience, _} <> t:
      Governance.accepted.go(t, expected, True{}, 1n+count, Bool.and(ok,
        Bool.and(Governance.approved.fields(protocol, policy, dissent, schema, changes, architecture, reliability, experience), Nat.is_eq(changes, expected))))
    case _ <> t:
      Governance.accepted.go(t, expected, found, count, ok)

def Governance.accepted(+cs: List<&2, M.Change>) -> Bool:
  Bool.and(Governance.accepted.go(cs, Governance.non_audit(cs), False{}, 0n, True{}), Governance.digest.matches(cs))

def Governance.audit.with.go(+cs: List<&2, M.Change>, recorded: Bool, +id: String,
    +architecture: Bool, +reliability: Bool, +experience: Bool, +dissent: String) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      Nil{}
    case +first <> _:
      match recorded:
        case True{}:
          cs
        case False{}:
          +action = Governance.tag(first)
          +at = Governance.at(cs)
          +approved = Bool.and(Governance.quorum(architecture, reliability, experience), String.is_empty(dissent))
          record : M.Change = M.GovernanceLogged{id, Governance.protocol(), Governance.actor(first), action, "persist:" ++ action,
            Bool.pick(String, approved, "approved", "rejected"), dissent, GS.GovSchema.version(),
            List.length(&2, M.Change, cs), Governance.digest(cs), architecture, reliability, experience, at}
          List.append(&2, M.Change, cs, [record])

def Governance.has.id.go(+cs: List<&2, M.Change>, +id: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.GovernanceLogged{+got, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Bool.or(String.eq(got, id), Governance.has.id.go(t, id))
    case _ <> t:
      Governance.has.id.go(t, id)

def Governance.audit.with(+id: String, +cs: List<&2, M.Change>, architecture: Bool,
    reliability: Bool, experience: Bool, +dissent: String) -> List<&2, M.Change>:
  Governance.audit.with.go(cs, Governance.has.id.go(cs, id), id, architecture, reliability, experience, dissent)

# Exactly one synthetic review record per persisted batch. Running it twice
# is harmless, which lets recovery and normal commit share this path.
def Governance.audit(+seq: Nat, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  Governance.audit.with("gov:" ++ Nat.show(seq), cs, True{}, True{}, True{}, "")

# The event log remains authoritative. These three newline ledgers are
# rebuildable indexes: review receipts, batch projection digests, and the
# compatibility adapter used to expose BP/GOV/2 to older clients.
def Governance.rows(+cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case M.GovernanceLogged{id, protocol, actor, action, intent, policy, dissent, schema, changes, digest, architecture, reliability, experience, at} <> t:
      J.Json.show(C.Change.encode(M.GovernanceLogged{id, protocol, actor, action, intent, policy, dissent, schema, changes, digest, architecture, reliability, experience, at})) <> Governance.rows(t)
    case _ <> t:
      Governance.rows(t)

def Governance.batch.take(n: Nat, +pending: List<&2, M.Change>, +out: List<&2, M.Change>) -> List<&2, M.Change>:
  match n pending:
    case 0n _:
      List.reverse(&2, M.Change, out)
    case 1n+k c <> t:
      Governance.batch.take(k, t, c <> out)
    case _ Nil{}:
      List.reverse(&2, M.Change, out)

def Governance.cert(id: String, protocol: String, +digest: String, +legacy: Bool, +n: Nat, +pending: List<&2, M.Change>) -> String:
  +batch = Governance.batch.take(n, pending, Nil{})
  +actual = Governance.digest(batch)
  +certdigest = Bool.pick(String, legacy, actual, digest)
  +verified = Bool.and(Nat.is_eq(List.length(&2, M.Change, batch), n), Bool.pick(Bool, legacy, True{}, String.eq(digest, actual)))
  J.Json.show(J.Json.obj([J.KV{"protocol", J.Str{protocol}}, J.KV{"receipt", J.Str{id}}, J.KV{"schema", J.Num{Nat.show(GS.GovSchema.version())}},
    J.KV{"changes", J.Num{Nat.show(n)}}, J.KV{"algorithm", J.Str{"SHA-256"}}, J.KV{"digest", J.Str{certdigest}},
    J.KV{"verified", J.Flag{verified}}, J.KV{"recomputed", J.Flag{legacy}}]))

def Governance.projections.go(+cs: List<&2, M.Change>, +pending: List<&2, M.Change>, +out: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}:
      List.reverse(&2, String, out)
    case M.GovernanceLogged{id, protocol, _, _, _, _, _, _, n, digest, _, _, _, _} <> t:
      Governance.projections.go(t, Nil{}, Governance.cert(id, protocol, digest, String.eq(protocol, "BP/GOV/1"), n, pending) <> out)
    case c <> t:
      Governance.projections.go(t, c <> pending, out)

def Governance.projections(+cs: List<&2, M.Change>) -> List<&2, String>:
  Governance.projections.go(cs, Nil{}, Nil{})

def Governance.migrations.go(+cs: List<&2, M.Change>, +out: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}:
      List.reverse(&2, String, out)
    case M.GovernanceLogged{id, protocol, _, _, _, _, _, schema, _, _, _, _, _, _} <> t:
      +row = J.Json.show(J.Json.obj([J.KV{"receipt", J.Str{id}}, J.KV{"from", J.Str{protocol}},
        J.KV{"to", J.Str{Governance.protocol()}}, J.KV{"adapter", J.Str{"SettingSet:gov.receipt.latest"}},
        J.KV{"fromSchema", J.Num{Nat.show(schema)}}, J.KV{"schema", J.Num{Nat.show(GS.GovSchema.version())}}]))
      Governance.migrations.go(t, row <> out)
    case _ <> t:
      Governance.migrations.go(t, out)

def Governance.migrations(+cs: List<&2, M.Change>) -> List<&2, String>:
  Governance.migrations.go(cs, Nil{})

def Governance.file(+rows: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, rows), "", String.join(rows, "\n") ++ "\n")

def Governance.ledgers(+cs: List<&2, M.Change>) -> Ledgers:
  Ledgers{Governance.file(Governance.rows(cs)), Governance.file(Governance.projections(cs)), Governance.file(Governance.migrations(cs)),
    Governance.file(Governance.commands(cs)), Governance.file(Governance.entities(cs))}


def Governance.roster.cmp(r: (String & String) & Cmp) -> Cmp:
  (_, c) = r
  c

def Governance.roster.lt(a: String, b: String) -> Bool:
  Cmp.is_lt(Governance.roster.cmp(String.cmp(a, b)))

def Governance.json.strings(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.str(x) <> Governance.json.strings(t)

def Governance.roster(+xs: List<&2, String>) -> List<&2, String>:
  List.sort(~String, ~Governance.roster.lt, xs)

def Governance.roster.duplicate.go(xs: List<&2, String>, +seen: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(Governance.in.go(seen, x), Governance.roster.duplicate.go(t, x <> seen))

def Governance.strings.nonempty.go(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case +x <> t:
      Bool.and(Bool.not(String.is_empty(x)), Governance.strings.nonempty.go(t))

def Governance.tags(+cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      Governance.tag(c) <> Governance.tags(t)

def Governance.proposal.metadata(+protocol: String, +origin: String, +sequence: Nat, +roster: List<&2, String>,
    +tags: List<&2, String>, +schema: Nat) -> String:
  protocol ++ "\n" ++ origin ++ "\n" ++ Nat.show(sequence) ++ "\n" ++ Nat.show(schema) ++ "\n"
    ++ String.join(roster, "\n") ++ "\n--\n" ++ String.join(tags, "\n")

def Governance.proposal(+id: String, +origin: String, +sequence: Nat, +roster: List<&2, String>, +cs: List<&2, M.Change>) -> Proposal:
  +members = Governance.roster(roster)
  +tags = Governance.tags(cs)
  +schema = GS.GovSchema.version()
  +meta = Governance.proposal.metadata(Governance.protocol(), origin, sequence, members, tags, schema)
  Proposal{id, Governance.protocol(), origin, sequence, members, tags, List.length(&2, M.Change, cs), schema,
    Governance.hex.bytes(SHA.Sha256.hash(U8.Utf8.encode(meta)), "")}

def Proposal.metadata(p: Proposal) -> J.Json:
  match p:
    case Proposal{id, protocol, origin, sequence, roster, tags, count, schema, digest}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"protocol", J.Str{protocol}}, J.KV{"origin", J.Str{origin}},
        J.KV{"sequence", J.Num{Nat.show(sequence)}}, J.KV{"roster", J.Json.arr(Governance.json.strings(roster))},
        J.KV{"actions", J.Json.arr(Governance.json.strings(tags))}, J.KV{"count", J.Num{Nat.show(count)}},
        J.KV{"schema", J.Num{Nat.show(schema)}}, J.KV{"digest", J.Str{digest}}])

def Governance.proposal.valid(p: Proposal) -> Bool:
  match p:
    case Proposal{+id, +protocol, +origin, +sequence, +roster, +tags, +count, +schema, +digest}:
      +head = Bool.and(String.eq(protocol, Governance.protocol()), Nat.is_eq(schema, GS.GovSchema.version()))
      +identity = Bool.and(Bool.not(String.is_empty(id)), Bool.not(String.is_empty(origin)))
      +size = Bool.and(Nat.is_gt(sequence, 0n), Bool.and(Nat.is_gt(count, 0n), Nat.is_eq(count, List.length(&2, String, tags))))
      +members = Bool.and(Nat.is_gt(List.length(&2, String, roster), 0n), Bool.and(Governance.strings.nonempty.go(roster), Bool.not(Governance.roster.duplicate.go(roster, Nil{}))))
      +actions = Governance.strings.nonempty.go(tags)
      +digest_ok = String.eq(digest, Governance.hex.bytes(SHA.Sha256.hash(U8.Utf8.encode(Governance.proposal.metadata(protocol, origin, sequence, roster, tags, schema))), ""))
      Bool.and(head, Bool.and(identity, Bool.and(size, Bool.and(members, Bool.and(actions, digest_ok)))))

def Proposal.sequence(p: Proposal) -> Nat:
  match p:
    case Proposal{_, _, _, n, _, _, _, _, _}:
      n

def Proposal.count(p: Proposal) -> Nat:
  match p:
    case Proposal{_, _, _, _, _, _, n, _, _}:
      n

def Proposal.roster(p: Proposal) -> List<&2, String>:
  match p:
    case Proposal{_, _, _, _, roster, _, _, _, _}:
      roster

def Proposal.tags(p: Proposal) -> List<&2, String>:
  match p:
    case Proposal{_, _, _, _, _, tags, _, _, _}:
      tags

def Proposal.id(p: Proposal) -> String:
  match p:
    case Proposal{id, _, _, _, _, _, _, _, _}:
      id

def Proposal.protocol(p: Proposal) -> String:
  match p:
    case Proposal{_, protocol, _, _, _, _, _, _, _}:
      protocol

def Proposal.origin(p: Proposal) -> String:
  match p:
    case Proposal{_, _, origin, _, _, _, _, _, _}:
      origin

def Proposal.schema(p: Proposal) -> Nat:
  match p:
    case Proposal{_, _, _, _, _, _, _, schema, _}:
      schema

def Proposal.digest(p: Proposal) -> String:
  match p:
    case Proposal{_, _, _, _, _, _, _, _, digest}:
      digest

def Governance.json.strings.decode(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+s} <> t:
      s <> Governance.json.strings.decode(t)
    case _ <> t:
      "" <> Governance.json.strings.decode(t)

def Governance.proposal.from.json(+o: J.Json) -> Proposal:
  Proposal{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "protocol", ""), J.Json.get_str(o, "origin", ""),
    J.Json.get_nat(o, "sequence", 0n), Governance.json.strings.decode(J.Json.elems(J.Json.get_obj(o, "roster"))),
    Governance.json.strings.decode(J.Json.elems(J.Json.get_obj(o, "actions"))), J.Json.get_nat(o, "count", 0n),
    J.Json.get_nat(o, "schema", 0n), J.Json.get_str(o, "digest", "")}

def Governance.actions.mapped(+tags: List<&2, String>) -> Bool:
  match tags:
    case Nil{}:
      True{}
    case +tag <> t:
      Bool.and(Bool.and(Bool.not(String.is_empty(Governance.entity(tag))), Bool.not(String.is_empty(Governance.effect(tag)))), Governance.actions.mapped(t))

def Governance.vote(+p: Proposal, +peer: String) -> Ballot:
  +valid = Governance.proposal.valid(p)
  +architecture = Bool.and(valid, Bool.and(String.eq(Governance.protocol(), GS.GovSchema.protocol()), Nat.is_eq(GS.GovSchema.version(), 3n)))
  +reliability = Bool.and(valid, Bool.and(Nat.is_gt(Proposal.sequence(p), 0n), Nat.is_eq(Proposal.count(p), List.length(&2, String, Proposal.tags(p)))))
  +experience = Bool.and(valid, Governance.actions.mapped(Proposal.tags(p)))
  +dissent = Bool.pick(String, Governance.quorum(architecture, reliability, experience), "", "policy check failed")
  Ballot{peer, architecture, reliability, experience, dissent}

def Ballot.approved(b: Ballot) -> Bool:
  match b:
    case Ballot{_, architecture, reliability, experience, dissent}:
      Bool.and(Governance.quorum(architecture, reliability, experience), String.is_empty(dissent))

def Ballot.row(b: Ballot) -> String:
  match b:
    case Ballot{peer, architecture, reliability, experience, dissent}:
      J.Json.show(J.Json.obj([J.KV{"peer", J.Str{peer}}, J.KV{"architecture", J.Flag{architecture}},
        J.KV{"reliability", J.Flag{reliability}}, J.KV{"experience", J.Flag{experience}}, J.KV{"dissent", J.Str{dissent}}]))
