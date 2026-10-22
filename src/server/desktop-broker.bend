import Base
import ../core/json.bend as J
import ../core/desktop-browser.bend as D
import ./perf.bend as PF

type Message is Data:
  Refresh{}
  Link{client: U32, kind: String, text: String, out: Chan(String), epoch: String, lease: String, owner: String, known: Bool, drive: String, recording: Bool}
  Policy{owner: String, known: Bool, drive: String, recording: Bool}
  Call{op: String, args: J.Json, reply: Chan(String)}
  Deadline{id: String}
  CaptureCommit{candidate: J.Json, permit: Chan(J.Json), released: Chan(Unit)}

type Pending is Data:
  Pending{id: String, owner: String, target: J.Json, revision: Nat, reply: Chan(String)}

type Broker is Data:
  Broker{client: U32, epoch: String, lease: String, out: Maybe<&2, Chan(String)>, meta: J.Json, pending: List<&2, Pending>, next: Nat}

def Broker.empty() -> Broker:
  Broker{0, "", "", None{}, J.Json.obj(Nil{}), Nil{}, 1n}

def Broker.meta(b: Broker) -> J.Json:
  match b: case Broker{_, _, _, _, m, _, _}: m

def Broker.pending_owner(ps: List<&2, Pending>, +owner: String) -> Bool:
  match ps:
    case Nil{}: False{}
    case Pending{_, o, _, _, _} <> t: Bool.or(String.eq(o, owner), Broker.pending_owner(t, owner))

def Broker.pending_id(ps: List<&2, Pending>, +id: String) -> Bool:
  match ps:
    case Nil{}: False{}
    case Pending{p, _, _, _, _} <> t: Bool.or(String.eq(p, id), Broker.pending_id(t, id))

def Broker.with_meta(b: Broker, m: J.Json) -> Broker:
  match b: case Broker{c, e, l, o, _, p, n}: Broker{c, e, l, o, m, p, n}

def Broker.owner(+b: Broker, owner: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "owners"), owner)

def Broker.effective_drive(+b: Broker, +owner: String) -> String:
  Bool.pick(String, J.Json.get_bool(Broker.owner(b, owner), "barrier", False{}), "none", J.Json.get_str(Broker.owner(b, owner), "drive", "none"))

def Broker.unknown_owner(+b: Broker, +owner: String) -> Bool:
  Bool.or(J.Json.get_bool(Broker.owner(b, owner), "unknown", False{}), J.Json.get_bool(J.Json.get_obj(Broker.meta(b), "unknownOwners"), owner, False{}))

def Broker.empty_latched(meta: J.Json) -> Broker:
  Broker{0, "", "", None{}, J.Json.obj([J.KV{"unknownOwners", J.Json.get_obj(meta, "unknownOwners")}]), Nil{}, 1n}

def Broker.target(+b: Broker, owner: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(Broker.owner(b, owner), "tabId", ""))

def Broker.public_target_if(present: Bool, t: J.Json) -> J.Json:
  match present:
    case True{}: D.Target.json(D.Target.decode(t))
    case False{}: J.Null{}

def Broker.public_target(+t: J.Json) -> J.Json:
  Broker.public_target_if(Bool.not(String.is_empty(J.Json.get_str(t, "tabId", ""))), t)

def Broker.set_owner(+b: Broker, +owner: String, value: J.Json) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "owners", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "owners"), owner, value)))

def Broker.set_target(+b: Broker, +target: J.Json) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "targets", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(target, "tabId", ""), target)))

def Broker.preparation(+b: Broker, owner: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "preparations"), owner)

def Broker.preparing(+b: Broker, owner: String) -> Bool:
  Bool.not(String.is_empty(J.Json.get_str(Broker.preparation(b, owner), "prepareId", "")))

def Broker.preparation_put(+b: Broker, owner: String, value: J.Json) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "preparations", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "preparations"), owner, value)))

def Broker.is_preparation(target: J.Json) -> Bool:
  J.Json.get_bool(target, "preparation", False{})

def Broker.send_checked(valid: Bool, m: Maybe<&2, Chan(String)>, text: String) -> IO(Unit):
  match valid m:
    case True{} Some{c}:
      do IO<Unit>:
        sent : Result<&1, &1, String, Unit> <- Chan.send(String, c, text)
        return Unit{}
    case False{} Some{c}: Chan.close(String, c)
    case _ None{}: IO.pure(Unit, Unit{})

# Two queued <=1MiB payloads plus one active writer and one blocked broker
# sender bound outbound payload backlog to 4MiB. This is not a heap-byte bound.
# Oversize closes, never truncates content.
def Broker.send(m: Maybe<&2, Chan(String)>, +text: String) -> IO(Unit):
  Broker.send_checked(D.Desktop.bytes_within(text, 1048512n), m, text)

def Broker.message(b: Broker, op: String, id: String, payload: J.Json) -> String:
  match b:
    case Broker{_, e, l, _, _, _, _}:
      J.Json.show(J.Json.obj([J.KV{"v", J.Num{"1"}}, J.KV{"op", J.Str{op}}, J.KV{"id", J.Str{id}}, J.KV{"serviceEpoch", J.Str{e}}, J.KV{"controllerLease", J.Str{l}}, J.KV{"payload", payload}]))

def Broker.nonce() -> IO(String):
  do IO<String>:
    a : U32 <- IO.try(U32, IO.random_u32())
    b : U32 <- IO.try(U32, IO.random_u32())
    c : U32 <- IO.try(U32, IO.random_u32())
    d : U32 <- IO.try(U32, IO.random_u32())
    return D.Desktop.hex(a) ++ D.Desktop.hex(b) ++ D.Desktop.hex(c) ++ D.Desktop.hex(d)

def Broker.emit(+b: Broker, op: String, +id: String, payload: J.Json) -> IO(Unit):
  match b:
    case Broker{c, e, l, +out, m, p, n}:
      do IO<Unit>:
        +fresh : String <- Broker.nonce()
        Broker.send(out, Broker.message(Broker{c, e, l, out, m, p, n}, op, Bool.pick(String, String.is_empty(id), fresh, id), payload))

def Broker.fail(ps: List<&2, Pending>, +why: String) -> IO(Unit):
  match ps:
    case Nil{}: IO.pure(Unit, Unit{})
    case Pending{id, _, +target, _, +reply} <> t:
      do IO<Unit>:
        Broker.send(Some{reply}, D.Desktop.error(id, why, Bool.not(Broker.is_preparation(target))))
        Broker.fail(t, why)

def Broker.close_output(out: Maybe<&2, Chan(String)>) -> IO(Unit):
  match out:
    case Some{c}: Chan.close(String, c)
    case None{}: IO.pure(Unit, Unit{})

def Broker.pending_latch_one(dispatched: Bool, +meta: J.Json, owner: String) -> J.Json:
  match dispatched:
    case False{}: meta
    case True{}: D.Desktop.put(meta, "unknownOwners", D.Desktop.put(J.Json.get_obj(meta, "unknownOwners"), owner, J.Flag{True{}}))

def Broker.pending_latches(ps: List<&2, Pending>, +meta: J.Json) -> J.Json:
  match ps:
    case Nil{}: meta
    case Pending{_, owner, +target, _, _} <> t:
      Broker.pending_latches(t, Broker.pending_latch_one(Bool.or(Bool.not(Broker.is_preparation(target)), J.Json.get_bool(target, "cancelling", False{})), meta, owner))

# every close says why, once, at the default log level (the live hub's
# refusals were silent)
def Broker.close(b: Broker, +why: String) -> IO(Broker):
  match b:
    case Broker{_, _, _, +out, meta, +ps, _}:
      do IO<Broker>:
        PF.Log.warn("desktop", "browser channel closed: " ++ why)
        Broker.fail(ps, why)
        Broker.send(out, "{\"v\":1,\"op\":\"closed\"}")
        Broker.close_output(out)
        return Broker.empty_latched(Broker.pending_latches(ps, meta))

def Broker.timeout_if(active: Bool, b: Broker) -> IO(Broker):
  match active:
    case False{}: IO.pure(Broker, b)
    case True{}: Broker.close(b, "guest command deadline; unresolved dispatch, no handback or automatic retry")

def Broker.operations(drive: String, recording: Bool) -> List<&2, String>:
  match drive:
    case "agent": ["list", "select", "present", "hide", "close", "cdp"]
    case "person": ["list", "select", "present", "hide", "close", "focus", "direct-input", "navigate"]
    case _: ["list", "hide", "close"]

def Broker.methods(drive: String) -> List<&2, String>:
  match drive:
    case "agent": D.Desktop.methods()
    case _: Nil{}

# Capture authority has an independent canonical-policy lineage. Mutation
# permission changes during an approved same-WC navigation cannot create a
# gap between own preload-ready and the first page script.
def Broker.capture_revision(+b: Broker, +owner: String) -> Nat:
  J.Json.get_nat(Broker.owner(b, owner), "captureRevision", J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))

# Owner list renewal must not resurrect a consumed create correlation.
def Broker.owner_payload_create(create: Bool, +b: Broker, +owner: String) -> J.Json:
  match create:
    case True{}:
      J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}},
        J.KV{"ttlMs", J.Num{"3000"}}, J.KV{"createId", J.Str{J.Json.get_str(Broker.owner(b, owner), "createId", "")}}, J.KV{"operations", J.Json.arr([J.Str{"create"}, J.Str{"list"}])}])
    case False{}:
      J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}},
        J.KV{"ttlMs", J.Num{"3000"}}, J.KV{"operations", J.Json.arr([J.Str{"list"}])}])

def Broker.owner_payload(+b: Broker, +owner: String) -> J.Json:
  Broker.owner_payload_create(Bool.not(String.is_empty(J.Json.get_str(Broker.owner(b, owner), "createId", ""))), b, owner)

def Broker.bump_owner(+b: Broker, +owner: String) -> Broker:
  Broker.set_owner(b, owner, D.Desktop.put(D.Desktop.put(Broker.owner(b, owner), "captureRevision", J.Num{Nat.show(Broker.capture_revision(b, owner))}), "revision", J.Num{Nat.show(Nat.add(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n), 1n))}))

# Permission fields are exact: no ambient known/ready/recording metadata is a
# substitute for document registration or the separate capture grant.
def Broker.target_payload_live(+b: Broker, +owner: String) -> J.Json:
  J.Json.obj([J.KV{"target", Broker.public_target(Broker.target(b, owner))}, J.KV{"ttlMs", J.Num{"3000"}},
    J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}},
    J.KV{"drive", J.Str{Broker.effective_drive(b, owner)}},
    J.KV{"operations", J.Json.arr(D.Desktop.strings("initialize" <> Broker.operations(Broker.effective_drive(b, owner), J.Json.get_bool(Broker.owner(b, owner), "recording", False{}))))},
    J.KV{"cdpMethods", J.Json.arr(D.Desktop.strings(Broker.methods(Broker.effective_drive(b, owner))))},
    J.KV{"initializationMethods", J.Json.arr([J.Str{"Runtime.enable"}, J.Str{"Runtime.addBinding"}, J.Str{"Runtime.removeBinding"}, J.Str{"Page.enable"}, J.Str{"Page.addScriptToEvaluateOnNewDocument"}, J.Str{"Page.removeScriptToEvaluateOnNewDocument"}])}])

def Broker.target_payload_choose(preparing: Bool, +b: Broker, +owner: String) -> J.Json:
  match preparing:
    case False{}: Broker.target_payload_live(b, owner)
    case True{}:
      J.Json.obj([J.KV{"target", Broker.public_target(Broker.target(b, owner))}, J.KV{"ttlMs", J.Num{"3000"}},
        J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}}, J.KV{"drive", J.Str{"none"}},
        J.KV{"operations", J.Json.arr([J.Str{"list"}, J.Str{"hide"}, J.Str{"close"}, J.Str{"initialize"}])},
        J.KV{"cdpMethods", J.Json.arr(Nil{})},
        J.KV{"initializationMethods", J.Json.arr(List.append(&2, J.Json, J.Json.elems(J.Json.get_obj(Broker.target_payload_live(b, owner), "initializationMethods")), [J.Str{"Page.setInterceptFileChooserDialog"}]))}])

def Broker.revoke(+b: Broker, +owner: String) -> IO(Unit):
  Broker.emit(b, "grant.revoke", "", J.Json.obj([J.KV{"scope", J.Str{"mutation"}}, J.KV{"owner", J.Str{owner}}, J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}}]))

def Broker.revoke_all(+b: Broker, +owner: String) -> IO(Unit):
  Broker.emit(b, "grant.revoke", "", J.Json.obj([J.KV{"scope", J.Str{"all"}}, J.KV{"owner", J.Str{owner}},
    J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}},
    J.KV{"captureRevision", J.Num{Nat.show(Broker.capture_revision(b, owner))}}]))

def Broker.fenced(b: Broker, +o: J.Json) -> Bool:
  match b:
    case Broker{+c, +e, +l, _, m, _, _}:
      Bool.and(Bool.not(U32.is_eq(c, 0)), Bool.and(J.Json.get_bool(m, "ready", False{}),
        Bool.and(String.eq(e, J.Json.get_str(o, "serviceEpoch", "")), String.eq(l, J.Json.get_str(o, "controllerLease", "")))))

def Broker.hello_payload_raw(b: Broker, +info: String) -> J.Json:
  match b:
    case Broker{_, e, l, _, _, _, _}:
      J.Json.obj([J.KV{"protocol", J.Num{"1"}}, J.KV{"version", J.Str{J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), "version", info)}},
        J.KV{"hub", J.Str{J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), "hub", "")}},
        J.KV{"serviceEpoch", J.Str{e}}, J.KV{"controllerLease", J.Str{l}}, J.KV{"limits", J.Json.obj([J.KV{"frameBytes", J.Num{"1048576"}}, J.KV{"backlogBytes", J.Num{"4194304"}}, J.KV{"pending", J.Num{"32"}}, J.KV{"grantTtlMs", J.Num{"3000"}}])}])


# The accepted connection is bound to the hub actor's current canonical identity,
# not the startup snapshot captured before a new home's first machine.id event.
def Broker.connection_info(+b: Broker, fallback: String) -> String:
  J.Json.get_str(Broker.meta(b), "connectionInfo", fallback)

def Broker.hello_payload(+b: Broker, info: String) -> J.Json:
  Broker.hello_payload_raw(b, Broker.connection_info(b, info))

def Broker.reject(+b: Broker, id: String, why: String) -> IO(Broker):
  do IO<Broker>:
    Broker.emit(b, "result", id, J.Json.obj([J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{why}}, J.KV{"dispatched", J.Flag{False{}}}]))
    return b

def Broker.accept_hello(ok: Bool, +b: Broker, id: String, version: String) -> IO(Broker):
  match ok:
    case False{}: Broker.reject(b, id, "invalid or repeated desktop hello")
    case True{}:
      do IO<Broker>:
        Broker.emit(b, "hello.accept", id, Broker.hello_payload(b, version))
        return Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "ready", J.Flag{True{}}))

def Broker.target_same(+b: Broker, +p: J.Json) -> Bool:
  +t = J.Json.get_obj(p, "target")
  Bool.and(D.Target.valid(t), D.Target.eq(D.Target.decode(t), D.Target.decode(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(t, "tabId", "")))))

def Broker.request_owner(ok: Bool, +b: Broker, +owner: String, +id: String) -> IO(Broker):
  match ok:
    case False{}: Broker.reject(b, id, "unknown canonical browser owner")
    case True{}:
      do IO<Broker>:
        Broker.emit(b, "owner.grant", id, Broker.owner_payload(b, owner))
        return b

def Broker.command(+b: Broker, id: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "commands"), id)

def Broker.command_put(+b: Broker, id: String, command: J.Json) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "commands", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "commands"), id, command)))

def Broker.select_op(+op: String, args: J.Json) -> Bool:
  Bool.or(String.eq(op, "select"), Bool.and(String.eq(op, "tabs"), Bool.not(String.is_empty(J.Json.get_str(args, "selectId", "")))))

def Broker.close_op(+op: String, args: J.Json) -> Bool:
  Bool.or(Bool.or(String.eq(op, "close"), String.eq(op, "close_page")), Bool.and(String.eq(op, "tabs"), Bool.not(String.is_empty(J.Json.get_str(args, "closeId", "")))))

def Broker.new_op(+op: String, +args: J.Json) -> Bool:
  Bool.or(String.eq(op, "newtab"), Bool.and(String.eq(op, "tabs"), Bool.or(J.Json.get_bool(args, "new", False{}), Bool.not(String.is_empty(J.Json.get_str(args, "new", ""))))))

def Broker.navigation_op(op: String, args: J.Json) -> Bool:
  match op:
    case "open": True{}
    case "navigate": True{}
    case "goto": True{}
    case "reload": True{}
    case "back": True{}
    case "forward": True{}
    case "newtab": True{}
    case "click": True{}
    case "type": True{}
    case "press": True{}
    case "select": True{}
    case "fill": True{}
    case "check": True{}
    case "hover": True{}
    case "evaluate": True{}
    case "tabs": Broker.new_op("tabs", args)
    case "cdp": D.Desktop.member(["Page.navigate", "Page.reload", "Page.navigateToHistoryEntry", "Runtime.evaluate", "Runtime.callFunctionOn", "Input.dispatchMouseEvent", "Input.dispatchKeyEvent", "Input.insertText", "Input.imeSetComposition", "DOM.focus"], J.Json.get_str(args, "method", ""))
    case _: False{}

def Broker.request_tab(+op: String, +args: J.Json) -> String:
  Bool.pick(String, Broker.select_op(op, args), J.Json.get_str(args, "selectId", J.Json.get_str(J.Json.get_obj(args, "target"), "tabId", "")),
    Bool.pick(String, Broker.close_op(op, args), J.Json.get_str(args, "closeId", J.Json.get_str(J.Json.get_obj(args, "target"), "tabId", "")), ""))

def Broker.request_target_named(named: Bool, +b: Broker, +args: J.Json, id: String) -> J.Json:
  match named:
    case False{}: Broker.public_target(Broker.target(b, J.Json.get_str(args, "page", "")))
    case True{}: Broker.public_target(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), id))

def Broker.request_target(+b: Broker, +op: String, +args: J.Json) -> J.Json:
  Broker.request_target_named(Bool.not(String.is_empty(Broker.request_tab(op, args))), b, args, Broker.request_tab(op, args))

def Broker.command_initial(+owner: String, +target: J.Json, +revision: Nat, +op: String, +args: J.Json) -> J.Json:
  J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"originalTarget", target}, J.KV{"originalRevision", J.Num{Nat.show(revision)}},
    J.KV{"expectedTarget", target}, J.KV{"expectedRevision", J.Num{Nat.show(revision)}}, J.KV{"valid", J.Flag{True{}}},
    J.KV{"navigates", J.Flag{Broker.navigation_op(op, args)}}, J.KV{"selects", J.Flag{Broker.select_op(op, args)}}, J.KV{"closes", J.Flag{Broker.close_op(op, args)}}, J.KV{"op", J.Str{op}}])

# The original execution fence never changes. Only an explicit correlated
# same-WC navigation may advance the expected OUTCOME target/revision.
def Broker.command_transition_valid(+b: Broker, +p: J.Json, +command: J.Json) -> Bool:
  +owner = J.Json.get_str(command, "owner", "")
  Bool.and(J.Json.get_bool(command, "valid", False{}), Bool.and(J.Json.get_bool(command, "navigates", False{}),
    Bool.and(Bool.not(Broker.unknown_owner(b, owner)), Bool.and(Bool.not(J.Json.get_bool(Broker.owner(b, owner), "barrier", False{})),
    Bool.and(Nat.is_eq(J.Json.get_nat(command, "expectedRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "dispatchRevision", 0n), J.Json.get_nat(command, "originalRevision", 1n)),
    Bool.and(D.Target.valid(J.Json.get_obj(p, "fromTarget")),
      D.Target.eq(D.Target.decode(J.Json.get_obj(p, "fromTarget")), D.Target.decode(J.Json.get_obj(command, "expectedTarget"))))))))))

def Broker.command_generation_one(ownerMatch: Bool, +id: String, +command: J.Json, +before: Broker, +after: Broker, +p: J.Json) -> J.Json:
  match ownerMatch:
    case False{}: command
    case True{}:
      D.Desktop.put(D.Desktop.put(D.Desktop.put(command, "valid", J.Flag{Bool.and(String.eq(J.Json.get_str(p, "dispatchId", ""), id), Broker.command_transition_valid(before, p, command))}),
        "expectedTarget", J.Json.get_obj(p, "target")), "expectedRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(after, J.Json.get_str(command, "owner", "")), "revision", 0n))})

def Broker.command_generation_fields(fields: J.Json, +before: Broker, +after: Broker, +p: J.Json) -> J.Json:
  match fields:
    case J.Obj{fs}: J.Obj{Broker.command_generation_fields(fs, before, after, p)}
    case J.Field{+id, +command, rest}:
      J.Field{id, Broker.command_generation_one(String.eq(J.Json.get_str(command, "owner", ""), J.Json.get_str(J.Json.get_obj(p, "target"), "owner", "")), id, command, before, after, p), Broker.command_generation_fields(rest, before, after, p)}
    case _: J.End{}

def Broker.command_generation(+before: Broker, +after: Broker, p: J.Json) -> Broker:
  Broker.with_meta(after, D.Desktop.put(Broker.meta(after), "commands", Broker.command_generation_fields(J.Json.get_obj(Broker.meta(before), "commands"), before, after, p)))

def Broker.result_valid(+b: Broker, +owner: String, +target: J.Json, rev: Nat, +p: J.Json) -> Bool:
  Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}),
    Bool.and(Nat.is_eq(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n), rev),
    Bool.and(Broker.target_same(b, p), Bool.and(J.Json.get_bool(Broker.target(b, owner), "ready", False{}), Bool.and(String.eq(J.Json.get_str(J.Json.get_obj(p, "target"), "owner", ""), owner),
      D.Target.eq(D.Target.decode(target), D.Target.decode(J.Json.get_obj(p, "target"))))))))

def Broker.result_predispatch(+p: J.Json) -> Bool:
  +reply = J.Json.get_obj(p, "reply")
  Bool.and(Bool.not(J.Json.get_bool(reply, "ok", False{})), Bool.not(J.Json.get_bool(reply, "dispatched", True{})))

def Broker.command_result_current_live(+b: Broker, +owner: String, +p: J.Json, +command: J.Json) -> Bool:
  Bool.and(J.Json.get_bool(command, "valid", False{}), Bool.and(Broker.target_same(b, p),
    Bool.and(J.Json.get_bool(Broker.target(b, owner), "ready", False{}), Bool.and(Bool.not(J.Json.get_bool(Broker.target(b, owner), "retired", False{})),
    Bool.and(Nat.is_eq(J.Json.get_nat(command, "expectedRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "resultRevision", 0n), J.Json.get_nat(command, "expectedRevision", 1n)),
    Bool.and(D.Target.eq(D.Target.decode(J.Json.get_obj(command, "expectedTarget")), D.Target.decode(J.Json.get_obj(p, "target"))),
      Bool.and(String.eq(J.Json.get_str(Broker.target(b, owner), "tabId", ""), J.Json.get_str(J.Json.get_obj(p, "target"), "tabId", "")),
        String.eq(J.Json.get_str(Broker.owner(b, owner), "activeTabId", J.Json.get_str(Broker.owner(b, owner), "tabId", "")), J.Json.get_str(J.Json.get_obj(p, "target"), "tabId", ""))))))))))

def Broker.result_receipt_action(op: String) -> Bool:
  D.Desktop.member(["back", "forward", "reload"], op)

# Native history/reload APIs report finite dispatch acceptance, explicitly NOT
# page-load completion. A later document event cannot settle this receipt again.
def Broker.command_dispatch_receipt(+b: Broker, +owner: String, +p: J.Json, +command: J.Json) -> Bool:
  +receipt = J.Json.get_obj(J.Json.get_obj(p, "reply"), "result")
  Bool.and(Broker.result_receipt_action(J.Json.get_str(command, "op", "")),
    Bool.and(J.Json.get_bool(J.Json.get_obj(p, "reply"), "ok", False{}),
    Bool.and(J.Json.get_bool(receipt, "dispatched", False{}), Bool.and(Bool.not(J.Json.get_bool(receipt, "loadComplete", True{})),
    Bool.and(String.eq(J.Json.get_str(receipt, "action", ""), J.Json.get_str(command, "op", "-")),
    Bool.and(D.Target.valid(J.Json.get_obj(receipt, "target")),
    Bool.and(D.Target.eq(D.Target.decode(J.Json.get_obj(receipt, "target")), D.Target.decode(J.Json.get_obj(command, "originalTarget"))),
    Bool.and(D.Target.eq(D.Target.decode(J.Json.get_obj(p, "target")), D.Target.decode(J.Json.get_obj(command, "originalTarget"))),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "resultRevision", 0n), J.Json.get_nat(command, "originalRevision", 1n)),
    Bool.and(J.Json.get_bool(command, "valid", False{}),
      Nat.is_eq(J.Json.get_nat(command, "expectedRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n))))))))))))

def Broker.command_predispatch(+p: J.Json, +command: J.Json) -> Bool:
  Bool.and(Broker.result_predispatch(p), Bool.and(D.Target.valid(J.Json.get_obj(p, "target")),
    Bool.and(D.Target.eq(D.Target.decode(J.Json.get_obj(p, "target")), D.Target.decode(J.Json.get_obj(command, "originalTarget"))),
      Nat.is_eq(J.Json.get_nat(p, "resultRevision", 0n), J.Json.get_nat(command, "originalRevision", 1n)))))

def Broker.command_result_closed(+b: Broker, +owner: String, +p: J.Json, +command: J.Json) -> Bool:
  +registered = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(J.Json.get_obj(command, "expectedTarget"), "tabId", ""))
  Bool.and(J.Json.get_bool(command, "valid", False{}), Bool.and(J.Json.get_bool(command, "closes", False{}),
    Bool.and(J.Json.get_bool(command, "sawClosed", False{}), Bool.and(J.Json.get_bool(registered, "retired", False{}),
    Bool.and(Nat.is_eq(J.Json.get_nat(command, "expectedRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "resultRevision", 0n), J.Json.get_nat(command, "expectedRevision", 1n)),
      D.Target.eq(D.Target.decode(J.Json.get_obj(command, "expectedTarget")), D.Target.decode(J.Json.get_obj(p, "target")))))))))

def Broker.command_result_current(+b: Broker, +owner: String, +p: J.Json, +command: J.Json) -> Bool:
  Bool.or(Broker.command_result_current_live(b, owner, p, command), Broker.command_result_closed(b, owner, p, command))

def Broker.command_result_valid(+b: Broker, +owner: String, +target: J.Json, +rev: Nat, +p: J.Json, +command: J.Json) -> Bool:
  Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}), Bool.and(Bool.not(Broker.unknown_owner(b, owner)),
    Bool.and(String.eq(J.Json.get_str(command, "owner", ""), owner),
    Bool.and(Nat.is_eq(rev, J.Json.get_nat(command, "originalRevision", 0n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "grantRevision", 0n), rev),
    Bool.and(D.Target.valid(J.Json.get_obj(p, "dispatchTarget")),
    Bool.and(D.Target.eq(D.Target.decode(target), D.Target.decode(J.Json.get_obj(p, "dispatchTarget"))),
    Bool.and(D.Target.eq(D.Target.decode(target), D.Target.decode(J.Json.get_obj(command, "originalTarget"))),
      Bool.or(Broker.command_predispatch(p, command), Bool.or(Broker.command_dispatch_receipt(b, owner, p, command), Broker.command_result_current(b, owner, p, command)))))))))))

def Broker.result_for_request(registered: Bool, +b: Broker, id: String, owner: String, target: J.Json, rev: Nat, p: J.Json) -> Bool:
  match registered:
    case False{}: Broker.result_valid(b, owner, target, rev, p)
    case True{}: Broker.command_result_valid(b, owner, target, rev, p, Broker.command(b, id))

def Broker.result_valid_request(+b: Broker, +id: String, owner: String, target: J.Json, rev: Nat, p: J.Json) -> Bool:
  Broker.result_for_request(Bool.not(String.is_empty(J.Json.get_str(Broker.command(b, id), "owner", ""))), b, id, owner, target, rev, p)

def Broker.result_send(valid: Bool, +reply: Chan(String), +id: String, p: J.Json) -> IO(Unit):
  match valid:
    case True{}: Broker.send(Some{reply}, J.Json.show(J.Json.get_obj(p, "reply")))
    case False{}: Broker.send(Some{reply}, D.Desktop.error(id, "guest target or authority changed after dispatch", True{}))

def Broker.result_keep(hit: Bool, item: Pending, rest: List<&2, Pending>, b: Broker, +p: J.Json) -> IO(List<&2, Pending>):
  match hit item:
    case True{} Pending{+id, +owner, target, rev, +reply}:
      do IO<List<&2, Pending>>:
        Broker.result_send(Broker.result_valid_request(b, id, owner, target, rev, p), reply, id, p)
        return rest
    case False{} _:
      IO.pure(List<&2, Pending>, item <> rest)

def Broker.result(ps: List<&2, Pending>, +b: Broker, +id: String, +p: J.Json) -> IO(List<&2, Pending>):
  match ps:
    case Nil{}: IO.pure(List<&2, Pending>, Nil{})
    case Pending{+pid, owner, +target, rev, reply} <> t:
      do IO<List<&2, Pending>>:
        rest : List<&2, Pending> <- Broker.result(t, b, id, p)
        Broker.result_keep(Bool.and(String.eq(pid, id), Bool.not(Broker.is_preparation(target))), Pending{pid, owner, target, rev, reply}, rest, b, p)

# Result validation and operation dispatch are deliberately centralized below;
# no page/CDP event is a public UI event just because the main process sent it.

def Broker.replace_pending(b: Broker, ps: List<&2, Pending>) -> Broker:
  match b: case Broker{c, e, l, o, m, _, n}: Broker{c, e, l, o, m, ps, n}

def Broker.doc_key(+doc: J.Json) -> String:
  J.Json.as_raw(J.Json.get(doc, "webContentsId"), "") ++ ":" ++ J.Json.as_raw(J.Json.get(doc, "frameTreeNodeId"), "")

def Broker.doc_valid(+doc: J.Json) -> Bool:
  Bool.and(Bool.not(String.is_empty(J.Json.get_str(doc, "documentNonce", ""))),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(doc, "frameToken", ""))),
    Bool.and(Nat.is_gt(J.Json.get_nat(doc, "webContentsId", 0n), 0n), D.Target.number(Maybe.default(&2, J.Json, J.Json.get(doc, "frameTreeNodeId"), J.Null{})))))

# Exact private document identity, including its main target incarnation.
def Broker.doc_same(+a: J.Json, +b: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(a, "documentNonce", ""), J.Json.get_str(b, "documentNonce", "-")),
    Bool.and(String.eq(J.Json.get_str(a, "frameToken", ""), J.Json.get_str(b, "frameToken", "-")),
    Bool.and(Nat.is_eq(J.Json.get_nat(a, "webContentsId", 0n), J.Json.get_nat(b, "webContentsId", 1n)),
      Nat.is_eq(J.Json.get_nat(a, "frameTreeNodeId", 0n), J.Json.get_nat(b, "frameTreeNodeId", 1n)))))

def Broker.doc_json(+doc: J.Json) -> J.Json:
  J.Json.obj([J.KV{"documentNonce", J.Str{J.Json.get_str(doc, "documentNonce", "")}},
    J.KV{"frameToken", J.Str{J.Json.get_str(doc, "frameToken", "")}},
    J.KV{"frameTreeNodeId", J.Num{Nat.show(J.Json.get_nat(doc, "frameTreeNodeId", 0n))}},
    J.KV{"webContentsId", J.Num{Nat.show(J.Json.get_nat(doc, "webContentsId", 0n))}}])

def Broker.retired_key(+e: J.Json) -> String:
  J.Json.show(J.Json.arr([Broker.public_target(e), Broker.doc_json(J.Json.get_obj(e, "document"))]))

def Broker.document_retired(+b: Broker, e: J.Json) -> Bool:
  J.Json.get_bool(J.Json.get_obj(Broker.meta(b), "retiredDocuments"), Broker.retired_key(e), False{})

def Broker.doc_bound(+b: Broker, +e: J.Json) -> Bool:
  Bool.and(Broker.doc_valid(J.Json.get_obj(e, "document")), Nat.is_eq(J.Json.get_nat(J.Json.get_obj(e, "document"), "webContentsId", 0n),
    J.Json.get_nat(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(e, "tabId", "")), "webContentsId", 0n)))

def Broker.ready_valid_core(+b: Broker, +p: J.Json) -> Bool:
  Bool.and(Bool.not(Broker.document_retired(b, D.Desktop.put(J.Json.get_obj(p, "target"), "document", J.Json.get_obj(p, "document")))), Bool.and(Broker.target_same(b, p), Bool.and(J.Json.get_bool(p, "recorderReady", False{}),
    Bool.and(String.eq(J.Json.get_str(p, "recorderSourceSha256", ""), "2e4b03765cc4ae6595e32048555ffe27e750994a85c18b89027064f3c0c15350"),
    Broker.doc_bound(b, D.Desktop.put(J.Json.get_obj(p, "target"), "document", J.Json.get_obj(p, "document")))))))

def Broker.document_save(+b: Broker, +e: J.Json) -> Broker:
  +doc = D.Desktop.put(D.Desktop.put(J.Json.get_obj(e, "document"), "target", Broker.public_target(e)), "mainFrame", J.Flag{J.Json.get_bool(e, "mainFrame", False{})})
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "documents", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "documents"), Broker.doc_key(doc), doc)))

def Broker.document_ready(valid: Bool, +b: Broker, +e: J.Json, id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "unregistered document readiness")
    case True{}: IO.pure(Broker, Broker.document_save(b, e))

def saved_target(j: J.Json) -> J.Json:
  J.Json.get_obj(j, "target")

def Broker.main_document_ready(fs: J.Json, +b: Broker, +target: J.Json) -> Bool:
  match fs:
    case J.Obj{fields}: Broker.main_document_ready(fields, b, target)
    case J.Field{_, +doc, rest}:
      Bool.or(Bool.and(J.Json.get_bool(doc, "mainFrame", False{}), Bool.and(Bool.not(J.Json.get_bool(doc, "retired", False{})),
        Bool.and(D.Target.eq(D.Target.decode(J.Json.get_obj(doc, "target")), D.Target.decode(target)),
          Bool.and(Bool.not(Broker.document_retired(b, D.Desktop.put(target, "document", doc))), Broker.doc_bound(b, D.Desktop.put(target, "document", doc)))))),
        Broker.main_document_ready(rest, b, target))
    case _: False{}

# Keep the original revision, document and step through the hub queue. This is
# only a candidate: it cannot enter Teach through the legacy record path.
def Broker.capture_candidate(b: Broker, p: J.Json, id: String) -> J.Json:
  match b:
    case Broker{_, epoch, lease, _, _, _, _}:
      J.Json.obj([J.KV{"event", J.Str{"desktop-record-candidate"}}, J.KV{"id", J.Str{id}},
        J.KV{"serviceEpoch", J.Str{epoch}}, J.KV{"controllerLease", J.Str{lease}}, J.KV{"payload", p}])

def Broker.fields(j: J.Json) -> J.Json:
  match j:
    case J.Obj{fs}: fs
    case _: J.End{}

def Broker.capture_drop(+b: Broker, +id: String) -> Broker:
  +cs = J.Json.get_obj(Broker.meta(b), "captures")
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "captures", J.Obj{D.Desktop.drop(Broker.fields(cs), id)}))

def Broker.record_room(room: Bool, +b: Broker, +p: J.Json, +ev: Chan(String), +id: String) -> IO(Broker):
  match room:
    case False{}: Broker.close(b, "pending recorder candidate limit exceeded")
    case True{}:
      do IO<Broker>:
        +candidate : J.Json = Broker.capture_candidate(b, p, id)
        +b2 : Broker = Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "captures", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "captures"), id, candidate)))
        Broker.send(Some{ev}, J.Json.show(candidate))
        return b2

def Broker.record(allowed: Bool, +b: Broker, p: J.Json, ev: Chan(String), id: String) -> IO(Broker):
  match allowed:
    case False{}: Broker.reject(b, id, "stale document or recording authority")
    case True{}: Broker.record_room(Nat.is_lt(D.Target.fields(J.Json.get_obj(Broker.meta(b), "captures")), 128n), b, p, ev, id)

def Broker.capture_matches(+candidate: J.Json, +e: J.Json) -> Bool:
  +old = J.Json.get_obj(J.Json.get_obj(candidate, "payload"), "event")
  Bool.and(D.Target.eq(D.Target.decode(old), D.Target.decode(e)), Broker.doc_same(J.Json.get_obj(old, "document"), J.Json.get_obj(e, "document")))

def Broker.capture_keep(drop: Bool, key: String, v: J.Json, rest: J.Json) -> J.Json:
  match drop:
    case True{}: rest
    case False{}: J.Field{key, v, rest}

def Broker.capture_cancel(fields: J.Json, +e: J.Json) -> J.Json:
  match fields:
    case J.Obj{fs}: J.Obj{Broker.capture_cancel(fs, e)}
    case J.Field{k, +v, t}: Broker.capture_keep(Broker.capture_matches(v, e), k, v, Broker.capture_cancel(t, e))
    case _: J.End{}

def Broker.retirement_valid(+b: Broker, +e: J.Json) -> Bool:
  +saved = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "documents"), Broker.doc_key(J.Json.get_obj(e, "document")))
  Bool.and(Broker.target_same(b, J.Json.obj([J.KV{"target", Broker.public_target(e)}])),
    Bool.and(Broker.doc_bound(b, e), Bool.and(Broker.doc_same(J.Json.get_obj(e, "document"), saved),
    Bool.and(D.Target.eq(D.Target.decode(saved_target(saved)), D.Target.decode(e)),
    Bool.and(Bool.not(J.Json.get_bool(e, "mainFrame", True{})), Bool.and(Bool.not(J.Json.get_bool(saved, "mainFrame", True{})),
      Bool.or(String.eq(J.Json.get_str(e, "reason", ""), "subframe-navigation"), String.eq(J.Json.get_str(e, "reason", ""), "subframe-navigation-unresolved"))))))))

def Broker.retirement_apply(+b: Broker, +e: J.Json) -> Broker:
  +doc = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "documents"), Broker.doc_key(J.Json.get_obj(e, "document")))
  +m = D.Desktop.put(Broker.meta(b), "documents", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "documents"), Broker.doc_key(doc), D.Desktop.put(doc, "retired", J.Flag{True{}})))
  +m2 = D.Desktop.put(m, "captures", Broker.capture_cancel(J.Json.get_obj(m, "captures"), e))
  Broker.with_meta(b, D.Desktop.put(m2, "retiredDocuments", D.Desktop.put(J.Json.get_obj(m2, "retiredDocuments"), Broker.retired_key(e), J.Flag{True{}})))

def Broker.retirement_room(room: Bool, b: Broker, e: J.Json) -> IO(Broker):
  match room:
    case False{}: Broker.close(b, "document retirement ledger full; controller must reconnect")
    case True{}: IO.pure(Broker, Broker.retirement_apply(b, e))

def Broker.retirement(valid: Bool, +b: Broker, +e: J.Json, id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "unregistered subframe retirement")
    case True{}: Broker.retirement_room(Bool.or(Broker.document_retired(b, e), Nat.is_lt(D.Target.fields(J.Json.get_obj(Broker.meta(b), "retiredDocuments")), 1024n)), b, e)

def Broker.capture_released(done: Maybe<&1, Unit>, b: Broker) -> IO(Broker):
  match done:
    case Some{_}: IO.pure(Broker, b)
    case None{}: Broker.close(b, "recorder commit owner disappeared; controller outcome unknown")

def Broker.capture_send(sent: Result<&1, &1, J.Json, Unit>, b: Broker, released: Chan(Unit)) -> IO(Broker):
  match sent:
    case Done{_}: IO.bind(Maybe<&1, Unit>, Broker, Chan.recv(Unit, released), done => Broker.capture_released(done, b))
    case Fail{_}: Broker.close(b, "recorder commit permit channel closed")

def Broker.capture_permit(valid: Bool, +b: Broker, +candidate: J.Json, +permit: Chan(J.Json), released: Chan(Unit)) -> IO(Broker):
  match valid:
    case False{}:
      do IO<Broker>:
        sent : Result<&1, &1, J.Json, Unit> <- Chan.send(J.Json, permit, J.Null{})
        return Broker.capture_drop(b, J.Json.get_str(candidate, "id", ""))
    case True{}:
      do IO<Broker>:
        +b2 : Broker = Broker.capture_drop(b, J.Json.get_str(candidate, "id", ""))
        sent : Result<&1, &1, J.Json, Unit> <- Chan.send(J.Json, permit, candidate)
        Broker.capture_send(sent, b2, released)

def Broker.owner_has_dispatch(ps: List<&2, Pending>, +owner: String) -> Bool:
  match ps:
    case Nil{}: False{}
    case Pending{_, o, target, _, _} <> rest:
      Bool.or(Bool.and(String.eq(o, owner), Bool.not(Broker.is_preparation(target))), Broker.owner_has_dispatch(rest, owner))

def Broker.closed_valid_core(+b: Broker, +e: J.Json) -> Bool:
  +old = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(e, "tabId", ""))
  Bool.and(D.Target.valid(Broker.public_target(e)), Bool.and(String.eq(J.Json.get_str(e, "owner", ""), J.Json.get_str(old, "owner", "-")),
    Bool.and(Bool.not(J.Json.get_bool(old, "retired", False{})),
      Bool.or(Nat.is_eq(J.Json.get_nat(e, "generation", 0n), J.Json.get_nat(old, "generation", 1n)),
        Bool.and(String.eq(J.Json.get_str(e, "type", ""), "crashed"), Nat.is_eq(J.Json.get_nat(e, "generation", 0n), Nat.add(J.Json.get_nat(old, "generation", 0n), 1n)))))))

def Broker.closed_valid(+b: Broker, +e: J.Json) -> Bool:
  Bool.and(D.Desktop.member(["closed", "crashed", "debugger-detached"], J.Json.get_str(e, "type", "")), Broker.closed_valid_core(b, e))

def Broker.closed_correlated(+b: Broker, +p: J.Json, +e: J.Json, +command: J.Json) -> Bool:
  +owner = J.Json.get_str(e, "owner", "")
  Bool.and(String.eq(J.Json.get_str(e, "type", ""), "closed"), Bool.and(J.Json.get_bool(command, "closes", False{}),
    Bool.and(J.Json.get_bool(command, "valid", False{}), Bool.and(String.eq(J.Json.get_str(command, "owner", ""), owner),
    Bool.and(Bool.not(Broker.unknown_owner(b, owner)), Bool.and(Bool.not(J.Json.get_bool(Broker.owner(b, owner), "barrier", False{})),
    Bool.and(Nat.is_eq(J.Json.get_nat(command, "expectedRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "grantRevision", 0n), J.Json.get_nat(command, "expectedRevision", 1n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "dispatchRevision", 0n), J.Json.get_nat(command, "originalRevision", 1n)),
      D.Target.eq(D.Target.decode(J.Json.get_obj(command, "expectedTarget")), D.Target.decode(e)))))))))))

def Broker.pending(b: Broker) -> List<&2, Pending>:
  match b: case Broker{_, _, _, _, _, ps, _}: ps

def Broker.policy_changed(changed: Bool, +b: Broker, +owner: String, known: Bool, drive: String, recording: Bool) -> Broker:
  match changed:
    case False{}: b
    case True{}:
      Broker.set_owner(b, owner, J.Json.obj([J.KV{"known", J.Flag{known}}, J.KV{"drive", J.Str{drive}}, J.KV{"recording", J.Flag{recording}},
        J.KV{"revision", J.Num{Nat.show(Nat.add(1n, J.Json.get_nat(Broker.owner(b, owner), "revision", 0n)))}},
        J.KV{"captureRevision", J.Num{Nat.show(Nat.add(1n, Broker.capture_revision(b, owner)))}},
        J.KV{"createId", J.Str{J.Json.get_str(Broker.owner(b, owner), "createId", "")}}, J.KV{"tabId", J.Str{J.Json.get_str(Broker.owner(b, owner), "tabId", "")}},
        J.KV{"unknown", J.Flag{Broker.unknown_owner(b, owner)}},
        J.KV{"barrier", J.Flag{Bool.or(Broker.unknown_owner(b, owner), Broker.pending_owner(Broker.pending(b), owner))}}]))

def Broker.policy(+b: Broker, +owner: String, +known: Bool, +drive: String, +recording: Bool) -> Broker:
  Broker.policy_changed(Bool.not(Bool.and(D.Desktop.bool_eq(known, J.Json.get_bool(Broker.owner(b, owner), "known", False{})),
    Bool.and(String.eq(drive, J.Json.get_str(Broker.owner(b, owner), "drive", "none")), D.Desktop.bool_eq(recording, J.Json.get_bool(Broker.owner(b, owner), "recording", False{}))))), b, owner, known, drive, recording)

# An exact per-lease sequence watermark replaces lifetime accumulation of IDs.
# Replaying an old frame never gets a fresh admission; reconnect has a new lease.
def Broker.client_sequence_valid(+b: Broker, +o: J.Json) -> Bool:
  Bool.and(D.Target.number(Maybe.default(&2, J.Json, J.Json.get(o, "clientSequence"), J.Null{})),
    Bool.and(Nat.is_le(J.Json.get_nat(o, "clientSequence", 0n), 4294967295n),
      Nat.is_eq(J.Json.get_nat(o, "clientSequence", 0n), Nat.add(J.Json.get_nat(Broker.meta(b), "clientSequence", 0n), 1n))))

def Broker.sequence_apply(+b: Broker, o: J.Json) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "clientSequence", J.Num{Nat.show(J.Json.get_nat(o, "clientSequence", 0n))}))

def Broker.pending_owner_hit(hit: Bool, owner: String, rest: String) -> String:
  match hit:
    case True{}: owner
    case False{}: rest

def Broker.pending_original_owner(ps: List<&2, Pending>, +id: String) -> String:
  match ps:
    case Nil{}: ""
    case Pending{request, owner, target, _, _} <> t: Broker.pending_owner_hit(Bool.and(String.eq(request, id), Bool.not(Broker.is_preparation(target))), owner, Broker.pending_original_owner(t, id))

def Broker.result_unknown(+p: J.Json) -> Bool:
  +reply = J.Json.get_obj(p, "reply")
  Bool.and(Bool.not(J.Json.get_bool(reply, "ok", False{})), J.Json.get_bool(reply, "dispatched", True{}))

def Broker.latch_unknown(+b: Broker, +owner: String) -> Broker:
  +b2 = Broker.bump_owner(Broker.set_owner(b, owner, D.Desktop.put(D.Desktop.put(D.Desktop.put(D.Desktop.put(Broker.owner(b, owner), "unknown", J.Flag{True{}}), "barrier", J.Flag{True{}}), "recording", J.Flag{False{}}), "captureRevision", J.Num{Nat.show(Nat.add(Broker.capture_revision(b, owner), 1n))})), owner)
  Broker.with_meta(b2, D.Desktop.put(Broker.meta(b2), "unknownOwners", D.Desktop.put(J.Json.get_obj(Broker.meta(b2), "unknownOwners"), owner, J.Flag{True{}})))

def Broker.closed_unexpected(pending: Bool, b: Broker, owner: String) -> Broker:
  match pending:
    case True{}: Broker.latch_unknown(b, owner)
    case False{}: b

def Broker.closed_command(expected: Bool, +before: Broker, +after: Broker, +p: J.Json, +owner: String) -> Broker:
  match expected:
    case True{}:
      Broker.command_put(after, J.Json.get_str(p, "dispatchId", ""), D.Desktop.put(D.Desktop.put(Broker.command(before, J.Json.get_str(p, "dispatchId", "")), "sawClosed", J.Flag{True{}}),
        "expectedRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(after, owner), "revision", 0n))}))
    case False{}: Broker.closed_unexpected(Broker.owner_has_dispatch(Broker.pending(before), owner), after, owner)

def Broker.connect(available: Bool, b: Broker, client: U32, +out: Chan(String), epoch: String, lease: String) -> IO(Broker):
  match available:
    case True{}: IO.pure(Broker, Broker{client, epoch, lease, Some{out}, J.Json.obj([J.KV{"unknownOwners", J.Json.get_obj(Broker.meta(b), "unknownOwners")}]), Nil{}, 1n})
    case False{}:
      do IO<Broker>:
        Broker.send(Some{out}, "{\"v\":1,\"op\":\"error\",\"error\":\"controller_busy\"}")
        Chan.close(String, out)
        return b


def Broker.with_connection_info(+b: Broker, info: String) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "connectionInfo", J.Str{info}))

# Only a newly accepted transport may install server-owned identity. A rejected
# competing controller cannot replace the active connection's hello identity.
def Broker.connect_info(available: Bool, b: Broker, client: U32, +out: Chan(String), epoch: String, lease: String, info: String) -> IO(Broker):
  match available:
    case True{}:
      IO.bind(Broker, Broker, Broker.connect(True{}, b, client, out, epoch, lease), fresh => IO.pure(Broker, Broker.with_connection_info(fresh, info)))
    case False{}: Broker.connect(False{}, b, client, out, epoch, lease)

def Broker.client(b: Broker) -> U32:
  match b: case Broker{c, _, _, _, _, _, _}: c

def Broker.with_next(b: Broker, n: Nat) -> Broker:
  match b: case Broker{c, e, l, o, m, p, _}: Broker{c, e, l, o, m, p, n}

def Broker.deadline(+q: Chan(Message), id: String) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(15000)
    sent : Result<&1, &1, Message, Unit> <- Chan.send(Message, q, Deadline{id})
    return Unit{}

def Broker.lease_timer(+q: Chan(Message), id: String) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(3000)
    sent : Result<&1, &1, Message, Unit> <- Chan.send(Message, q, Deadline{id})
    return Unit{}

def Broker.lease_tick_if(refresh: Bool, +b: Broker, +q: Chan(Message), +id: String) -> IO(Broker):
  match refresh:
    case False{}: IO.pure(Broker, b)
    case True{}:
      do IO<Broker>:
        +timer : String = "lease:" ++ id
        IO.spawn(Unit, Broker.lease_timer(q, timer))
        return Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "leaseTimer", J.Str{timer}))

def Broker.lease_tick(+b: Broker, +q: Chan(Message), +text: String) -> IO(Broker):
  +o = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
  Broker.lease_tick_if(Bool.and(Broker.fenced(b, o), String.eq(J.Json.get_str(o, "op", ""), "heartbeat")), b, q, J.Json.get_str(o, "id", ""))

def Broker.initial_lease(start: Bool, +b: Broker, +q: Chan(Message), +epoch: String) -> IO(Broker):
  match start:
    case False{}: IO.pure(Broker, b)
    case True{}:
      do IO<Broker>:
        +timer : String = "lease:initial:" ++ epoch
        IO.spawn(Unit, Broker.lease_timer(q, timer))
        return Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "leaseTimer", J.Str{timer}))

def Broker.expire(current: Bool, b: Broker) -> IO(Broker):
  match current:
    case True{}: Broker.close(b, "desktop controller lease expired; outcome unknown")
    case False{}: IO.pure(Broker, b)

# Original trusted actor is retained in preparation args; main never supplies it.
def Broker.intent_allowed(+b: Broker, +owner: String, args: J.Json) -> Bool:
  Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}), Bool.and(Bool.not(Broker.unknown_owner(b, owner)),
    Bool.and(Bool.not(J.Json.get_bool(Broker.owner(b, owner), "barrier", False{})),
      Bool.or(Bool.not(String.eq(J.Json.get_str(args, "__desktopActor", "service"), "agent")), String.eq(Broker.effective_drive(b, owner), "agent")))))

def Broker.pending_remove_one(hit: Bool, item: Pending, rest: List<&2, Pending>) -> List<&2, Pending>:
  match hit:
    case True{}: rest
    case False{}: item <> rest

def Broker.pending_remove(ps: List<&2, Pending>, +id: String) -> List<&2, Pending>:
  match ps:
    case Nil{}: Nil{}
    case Pending{+pid, owner, target, rev, reply} <> t:
      Broker.pending_remove_one(String.eq(pid, id), Pending{pid, owner, target, rev, reply}, Broker.pending_remove(t, id))

def Broker.prepared_pick(hit: Bool, item: Pending, rest: Maybe<&2, Pending>) -> Maybe<&2, Pending>:
  match hit:
    case False{}: rest
    case True{}: Some{item}

def Broker.prepared_find(ps: List<&2, Pending>, +prepare: String) -> Maybe<&2, Pending>:
  match ps:
    case Nil{}: None{}
    case Pending{+id, o, +target, r, reply} <> t:
      Broker.prepared_pick(Bool.and(String.eq(id, prepare), Broker.is_preparation(target)), Pending{id, o, target, r, reply}, Broker.prepared_find(t, prepare))

def Broker.pending_owner_id(ps: List<&2, Pending>, +owner: String) -> String:
  match ps:
    case Nil{}: ""
    case Pending{id, o, _, _, _} <> t: Broker.pending_owner_hit(String.eq(o, owner), id, Broker.pending_owner_id(t, owner))

def Broker.start_deadline_id(id: String, q: Chan(Message)) -> IO(Unit):
  match id:
    case "": IO.pure(Unit, Unit{})
    case _: IO.spawn(Unit, Broker.deadline(q, id))

def Broker.start_owner_deadline(ps: List<&2, Pending>, q: Chan(Message), owner: String) -> IO(Unit):
  Broker.start_deadline_id(Broker.pending_owner_id(ps, owner), q)

def Broker.new_dispatch_deadline_if(changed: Bool, b: Broker, q: Chan(Message), owner: String) -> IO(Unit):
  match changed:
    case False{}: IO.pure(Unit, Unit{})
    case True{}: Broker.start_owner_deadline(Broker.pending(b), q, owner)

# The message actor installs a timer only when a link frame has produced a new
# ordinary dispatch. Preparation has its own ID/timer; neither timer is reused.
def Broker.new_dispatch_deadline(+before: Broker, +after: Broker, q: Chan(Message), +owner: String) -> IO(Unit):
  Broker.new_dispatch_deadline_if(Bool.not(String.eq(Broker.pending_owner_id(Broker.pending(before), owner), Broker.pending_owner_id(Broker.pending(after), owner))), after, q, owner)

def Broker.command_drop(+b: Broker, id: String) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "commands", J.Obj{D.Desktop.drop(Broker.fields(J.Json.get_obj(Broker.meta(b), "commands")), id)}))

def Broker.checked_pick(known: Bool, valid: Bool, +p: J.Json) -> J.Json:
  match known valid:
    case True{} False{}: D.Desktop.put(p, "reply", J.Json.obj([J.KV{"ok", J.Flag{False{}}}, J.KV{"dispatched", J.Flag{True{}}}, J.KV{"error", J.Str{"invalid result lineage; original operation outcome unknown"}}]))
    case _ _: p

def Broker.checked_result(ps: List<&2, Pending>, +b: Broker, +id: String, +p: J.Json) -> J.Json:
  match ps:
    case Nil{}: p
    case Pending{+request, +owner, +target, +rev, _} <> t:
      Broker.checked_pick(Bool.and(String.eq(request, id), Bool.not(Broker.is_preparation(target))), Broker.result_valid_request(b, request, owner, target, rev, p), Broker.checked_result(t, b, id, p))

def Broker.registry_row_if(valid: Bool, +target: J.Json, active: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match valid:
    case False{}: rest
    case True{}:
      D.Desktop.put(D.Desktop.put(D.Desktop.put(Broker.public_target(target), "url", J.Str{J.Json.get_str(target, "url", "about:blank")}), "title", J.Str{J.Json.get_str(target, "title", "")}), "active", J.Flag{String.eq(active, J.Json.get_str(target, "tabId", ""))}) <> rest

# Registry snapshots contain only normal tab controls; no private recorder/CDP
# metadata is copied into public UI state or used to create authority.
def Broker.registry_rows(fields: J.Json, +owner: String, +active: String) -> List<&2, J.Json>:
  match fields:
    case J.Obj{fs}: Broker.registry_rows(fs, owner, active)
    case J.Field{_, +target, rest}:
      Broker.registry_row_if(Bool.and(String.eq(J.Json.get_str(target, "owner", ""), owner), Bool.not(J.Json.get_bool(target, "retired", False{}))), target, active, Broker.registry_rows(rest, owner, active))
    case _: Nil{}

def Broker.registry(+b: Broker, +owner: String) -> List<&2, J.Json>:
  Broker.registry_rows(J.Json.get_obj(Broker.meta(b), "targets"), owner, J.Json.get_str(Broker.owner(b, owner), "activeTabId", J.Json.get_str(Broker.owner(b, owner), "tabId", "")))

def Broker.json_text(j: J.Json) -> Bool:
  match j:
    case J.Str{_}: True{}
    case _: False{}

def Broker.json_flag(j: J.Json) -> Bool:
  match j:
    case J.Flag{_}: True{}
    case _: False{}

def Broker.registry_entry_valid(+b: Broker, +owner: String, +row: J.Json) -> Bool:
  Bool.and(Nat.is_eq(D.Target.fields(row), 6n), Bool.and(String.eq(J.Json.get_str(row, "owner", ""), owner),
    Bool.and(Broker.target_same(b, J.Json.obj([J.KV{"target", Broker.public_target(row)}])),
    Bool.and(Bool.not(J.Json.get_bool(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(row, "tabId", "")), "retired", False{})),
    Bool.and(Broker.json_text(Maybe.default(&2, J.Json, J.Json.get(row, "url"), J.Null{})),
    Bool.and(Broker.json_text(Maybe.default(&2, J.Json, J.Json.get(row, "title"), J.Null{})),
    Bool.and(Broker.json_flag(Maybe.default(&2, J.Json, J.Json.get(row, "active"), J.Null{})),
    Bool.and(D.Desktop.bytes_within(J.Json.get_str(row, "url", ""), 32768n), D.Desktop.bytes_within(J.Json.get_str(row, "title", ""), 4096n)))))))))

def Broker.registry_entries_valid(rows: List<&2, J.Json>, +b: Broker, +owner: String, +seen: List<&2, String>) -> Bool:
  match rows:
    case Nil{}: True{}
    case +h <> t:
      Bool.and(Broker.registry_entry_valid(b, owner, h), Bool.and(Bool.not(D.Desktop.member(seen, J.Json.get_str(h, "tabId", ""))), Broker.registry_entries_valid(t, b, owner, J.Json.get_str(h, "tabId", "") <> seen)))

def Broker.registry_active_count(rows: List<&2, J.Json>) -> Nat:
  match rows:
    case Nil{}: 0n
    case h <> t: Nat.add(Bool.pick(Nat, J.Json.get_bool(h, "active", False{}), 1n, 0n), Broker.registry_active_count(t))

def Broker.registry_active(rows: List<&2, J.Json>) -> String:
  match rows:
    case Nil{}: ""
    case +h <> t: Broker.pending_owner_hit(J.Json.get_bool(h, "active", False{}), J.Json.get_str(h, "tabId", ""), Broker.registry_active(t))

def Broker.registry_valid_core(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(p, "owner", "")
  +rows = J.Json.elems(J.Json.get_obj(p, "tabs"))
  Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}), Bool.and(Bool.not(Broker.unknown_owner(b, owner)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "grantRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
    Bool.and(Nat.is_le(List.length(&2, J.Json, rows), 32n), Bool.and(Nat.is_eq(List.length(&2, J.Json, rows), List.length(&2, J.Json, Broker.registry(b, owner))),
    Bool.and(Broker.registry_entries_valid(rows, b, owner, Nil{}), Nat.is_eq(Broker.registry_active_count(rows), Bool.pick(Nat, List.is_empty(&2, J.Json, rows), 0n, 1n))))))))

def Broker.json_array(j: J.Json) -> Bool:
  match j:
    case J.Arr{_}: True{}
    case _: False{}

def Broker.registry_save(rows: List<&2, J.Json>, +b: Broker) -> Broker:
  match rows:
    case Nil{}: b
    case +h <> t:
      Broker.registry_save(t, Broker.set_target(b, D.Desktop.put(D.Desktop.put(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(h, "tabId", "")), "url", J.Str{J.Json.get_str(h, "url", "")}), "title", J.Str{J.Json.get_str(h, "title", "")})))

# Creation cancellation is independent of ordinary dispatched-command outcome.
# Its fresh ID cannot settle a command or abort a completed module capability.
def Broker.cancelling(+b: Broker, owner: String) -> Bool:
  String.eq(J.Json.get_str(Broker.preparation(b, owner), "phase", ""), "cancelling")

def Broker.preparation_current(+b: Broker, +owner: String) -> Bool:
  Bool.and(Bool.not(Broker.cancelling(b, owner)), Bool.and(Broker.preparing(b, owner), Bool.and(J.Json.get_bool(Broker.preparation(b, owner), "valid", False{}),
    Bool.and(Nat.is_eq(J.Json.get_nat(Broker.preparation(b, owner), "revision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
      Bool.and(Bool.not(Broker.unknown_owner(b, owner)), Bool.not(J.Json.get_bool(Broker.owner(b, owner), "barrier", False{})))))))

# Only explicit preparation transitions update the expected revision. Policy
# changes cannot repair a stale preparation merely by restoring its labels.
def Broker.preparation_advance(+before: Broker, +after: Broker, +owner: String) -> Broker:
  Broker.preparation_put(after, owner, D.Desktop.put(D.Desktop.put(D.Desktop.put(Broker.preparation(before, owner), "target", Broker.public_target(Broker.target(after, owner))), "valid", J.Flag{Broker.preparation_current(before, owner)}),
    "revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(after, owner), "revision", 0n))}))

def Broker.generation_preparation(preparing: Bool, before: Broker, after: Broker, owner: String) -> Broker:
  match preparing:
    case False{}: after
    case True{}: Broker.preparation_advance(before, after, owner)

def Broker.offer_preparation_valid(+b: Broker, +owner: String, +p: J.Json) -> Bool:
  Bool.and(Broker.preparation_current(b, owner), Bool.and(String.eq(owner, J.Json.get_str(J.Json.get_obj(p, "target"), "owner", "")),
    Bool.and(String.eq(J.Json.get_str(p, "prepareId", ""), J.Json.get_str(Broker.preparation(b, owner), "prepareId", "-")),
      Nat.is_eq(J.Json.get_nat(p, "grantRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)))))

def Broker.preparation_ready_valid(+b: Broker, +owner: String, +p: J.Json) -> Bool:
  Bool.or(Bool.not(Broker.preparing(b, owner)), Bool.and(Broker.offer_preparation_valid(b, owner, p),
    String.eq(J.Json.get_str(p, "createId", ""), J.Json.get_str(Broker.preparation(b, owner), "createId", "-"))))

def Broker.prepared_valid(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(p, "owner", "")
  Bool.and(Broker.preparation_current(b, owner), Bool.and(Broker.target_same(b, p),
    Bool.and(J.Json.get_bool(Broker.target(b, owner), "ready", False{}), Bool.and(Broker.offer_preparation_valid(b, owner, p),
      Bool.and(String.eq(J.Json.get_str(p, "intentId", ""), J.Json.get_str(Broker.preparation(b, owner), "intentId", "-")),
        Broker.intent_allowed(b, owner, J.Json.get_obj(Broker.preparation(b, owner), "args")))))))

def Broker.cancel_key(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(p, "owner", "")
  Bool.and(Broker.cancelling(b, owner), Bool.and(String.eq(J.Json.get_str(p, "phase", ""), "preparation"),
    Bool.and(String.eq(J.Json.get_str(p, "prepareId", ""), J.Json.get_str(Broker.preparation(b, owner), "prepareId", "-")),
    Bool.and(String.eq(J.Json.get_str(p, "createId", ""), J.Json.get_str(Broker.preparation(b, owner), "createId", "-")),
      String.eq(J.Json.get_str(p, "cancelId", ""), J.Json.get_str(Broker.preparation(b, owner), "cancelId", "-"))))))

def Broker.cancel_destroyed(+p: J.Json) -> Bool:
  +r = J.Json.get_obj(p, "receipt")
  Bool.and(J.Json.get_bool(r, "nativeDestroyed", False{}), Bool.not(J.Json.get_bool(r, "closeUnknown", False{})))

def Broker.cancel_retire(known: Bool, +b: Broker, target: J.Json) -> Broker:
  match known:
    case False{}: b
    case True{}:
      Broker.set_target(b, D.Desktop.put(D.Desktop.put(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(target, "tabId", "")), "retired", J.Flag{True{}}), "ready", J.Flag{False{}}))

def Broker.cancel_finished(+b: Broker, +owner: String) -> Broker:
  +prep = Broker.preparation(b, owner)
  +b1 = Broker.cancel_retire(D.Target.valid(J.Json.get_obj(prep, "target")), b, J.Json.get_obj(prep, "target"))
  +active = J.Json.get_str(Broker.owner(b1, owner), "activeTabId", "")
  +activeTarget = J.Json.get_obj(J.Json.get_obj(Broker.meta(b1), "targets"), active)
  +route = Bool.pick(String, Bool.and(String.eq(J.Json.get_str(activeTarget, "owner", ""), owner), Bool.not(J.Json.get_bool(activeTarget, "retired", False{}))), active, "")
  +b2 = Broker.set_owner(b1, owner, D.Desktop.put(D.Desktop.put(Broker.owner(b1, owner), "createId", J.Str{""}), "tabId", J.Str{route}))
  Broker.replace_pending(Broker.preparation_put(b2, owner, J.Json.obj(Nil{})), Broker.pending_remove(Broker.pending(b2), J.Json.get_str(prep, "cancelId", "")))

# Caller receives a known pre-dispatch failure once; cleanup has a different ID,
# reply channel and deadline. No grant is needed for exact scoped safety abort.
def Broker.cancel_start(item: Maybe<&2, Pending>, +b: Broker, +why: String) -> IO(Broker):
  match item:
    case None{}: IO.pure(Broker, b)
    case Some{Pending{+prepare, +owner, _, _, reply}}:
      do IO<Broker>:
        +cancel : String <- Broker.nonce()
        +sink : Chan(String) <- Chan.new(String, 1)
        +prep : J.Json = Broker.preparation(b, owner)
        +b1 : Broker = Broker.bump_owner(Broker.set_owner(b, owner, D.Desktop.put(D.Desktop.put(D.Desktop.put(Broker.owner(b, owner), "createId", J.Str{""}), "barrier", J.Flag{True{}}), "captureRevision", J.Num{Nat.show(Nat.add(Broker.capture_revision(b, owner), 1n))})), owner)
        +p2 : J.Json = D.Desktop.put(D.Desktop.put(D.Desktop.put(prep, "phase", J.Str{"cancelling"}), "valid", J.Flag{False{}}), "cancelId", J.Str{cancel})
        +b2 : Broker = Broker.preparation_put(b1, owner, p2)
        +b3 : Broker = Broker.replace_pending(b2, Pending{cancel, owner, J.Json.obj([J.KV{"preparation", J.Flag{True{}}}, J.KV{"cancelling", J.Flag{True{}}}]), J.Json.get_nat(Broker.owner(b2, owner), "revision", 0n), sink} <> Broker.pending_remove(Broker.pending(b2), prepare))
        Broker.send(Some{reply}, D.Desktop.error(prepare, why, False{}))
        Broker.revoke_all(b, owner)
        Broker.emit(b3, "browser.cancel", cancel, J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"phase", J.Str{"preparation"}}, J.KV{"prepareId", J.Str{prepare}}, J.KV{"createId", J.Str{J.Json.get_str(prep, "createId", "")}}, J.KV{"cancelId", J.Str{cancel}}, J.KV{"target", Maybe.default(&2, J.Json, J.Json.get(prep, "target"), J.Null{})}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b3, owner), "revision", 0n))}}]))
        return b3

def Broker.prepare_failed_valid(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(p, "owner", "")
  Bool.and(Broker.preparing(b, owner), Bool.and(Bool.not(Broker.cancelling(b, owner)),
    Bool.and(String.eq(J.Json.get_str(p, "phase", ""), "preparation"),
    Bool.and(String.eq(J.Json.get_str(p, "prepareId", ""), J.Json.get_str(Broker.preparation(b, owner), "prepareId", "-")),
      String.eq(J.Json.get_str(p, "createId", ""), J.Json.get_str(Broker.preparation(b, owner), "createId", "-"))))))

def Broker.prepare_failed_if(valid: Bool, +b: Broker, +p: J.Json, id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "stale creation failure cannot cancel current or completed guest")
    case True{}: Broker.cancel_start(Broker.prepared_find(Broker.pending(b), J.Json.get_str(p, "prepareId", "")), b, "guest preparation failed; original operation not dispatched; cleanup pending")

def Broker.prepare_failed(+b: Broker, +p: J.Json, id: String) -> IO(Broker):
  Broker.prepare_failed_if(Broker.prepare_failed_valid(b, p), b, p, id)

def Broker.timeout_preparation(cancelling: Bool, item: Pending, +b: Broker) -> IO(Broker):
  match cancelling item:
    case False{} _: Broker.cancel_start(Some{item}, b, "guest preparation deadline; original operation not dispatched; cleanup pending")
    case True{} Pending{_, owner, _, _, _}: Broker.close(Broker.latch_unknown(b, owner), "creation cleanup deadline; completion unresolved; no force or retry")

def Broker.timeout_pending(item: Maybe<&2, Pending>, +b: Broker, id: String) -> IO(Broker):
  match item:
    case None{}: Broker.timeout_if(Broker.pending_id(Broker.pending(b), id), b)
    case Some{Pending{+pid, owner, +target, rev, reply}}:
      Broker.timeout_preparation(J.Json.get_bool(target, "cancelling", False{}), Pending{pid, owner, target, rev, reply}, b)

def Broker.timeout(+b: Broker, +id: String) -> IO(Broker):
  Broker.timeout_pending(Broker.prepared_find(Broker.pending(b), id), b, id)

def Broker.deadline_message(lease: Bool, +b: Broker, +id: String) -> IO(Broker):
  match lease:
    case True{}: Broker.expire(String.eq(J.Json.get_str(Broker.meta(b), "leaseTimer", ""), id), b)
    case False{}: Broker.timeout(b, id)

def Broker.pending_any_owner(ps: List<&2, Pending>, +id: String) -> String:
  match ps:
    case Nil{}: ""
    case Pending{request, owner, _, _, _} <> t: Broker.pending_owner_hit(String.eq(request, id), owner, Broker.pending_any_owner(t, id))

def Broker.deadline_step(+b: Broker, +q: Chan(Message), +id: String, k: Broker -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    +owner : String = Broker.pending_any_owner(Broker.pending(b), id)
    +b2 : Broker <- Broker.deadline_message(String.starts_with(id, "lease:"), b, id)
    Broker.new_dispatch_deadline(b, b2, q, owner)
    k(b2)

def Broker.command_preparation(command: J.Json, +prepare: String, +create: String) -> J.Json:
  D.Desktop.put(D.Desktop.put(D.Desktop.put(command, "prepareId", J.Str{prepare}), "createId", J.Str{create}), "initialSelection", J.Flag{Bool.and(Bool.not(String.is_empty(prepare)), Bool.not(String.is_empty(create)))})

def Broker.dispatch_fresh(+b: Broker, +op: String, +args: J.Json, +reply: Chan(String), +intent: String, +prepare: String, +create: String) -> IO(Broker):
  do IO<Broker>:
    +owner : String = J.Json.get_str(args, "page", "")
    +id : String <- Broker.nonce()
    +freshIntent : String <- Broker.nonce()
    +actualIntent : String = Bool.pick(String, String.is_empty(intent), freshIntent, intent)
    +t : J.Json = Broker.request_target(b, op, args)
    +rev : Nat = J.Json.get_nat(Broker.owner(b, owner), "revision", 0n)
    +b1 : Broker = Broker.command_put(b, id, Broker.command_preparation(Broker.command_initial(owner, t, rev, op, args), prepare, create))
    Broker.emit(b1, "browser.call", id, J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"intentId", J.Str{actualIntent}}, J.KV{"prepareId", J.Str{prepare}}, J.KV{"createId", J.Str{create}}, J.KV{"target", t}, J.KV{"grantRevision", J.Num{Nat.show(rev)}}, J.KV{"op", J.Str{op}}, J.KV{"args", args}]))
    return Broker.replace_pending(b1, Pending{id, owner, t, rev, reply} <> Broker.pending(b1))

def Broker.call_ready(ready: Bool, +b: Broker, +op: String, +args: J.Json, reply: Chan(String)) -> IO(Broker):
  match ready:
    case True{}: Broker.dispatch_fresh(b, op, args, reply, "", "", "")
    case False{}:
      do IO<Broker>:
        Broker.send(Some{reply}, D.Desktop.error("", "registered guest is not ready; no automatic replacement", False{}))
        return b

def Broker.initial_selection_valid(+command: J.Json, +p: J.Json, active: String) -> Bool:
  Bool.and(J.Json.get_bool(command, "initialSelection", False{}),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(command, "prepareId", ""))),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(command, "createId", ""))),
    Bool.and(String.eq(J.Json.get_str(p, "prepareId", ""), J.Json.get_str(command, "prepareId", "-")),
    Bool.and(String.eq(J.Json.get_str(p, "createId", ""), J.Json.get_str(command, "createId", "-")),
      String.eq(active, J.Json.get_str(J.Json.get_obj(command, "originalTarget"), "tabId", "")))))))

def Broker.registry_selection_valid(+command: J.Json, +before: Broker, +p: J.Json, +active: String) -> Bool:
  Bool.and(J.Json.get_bool(command, "valid", False{}),
    Bool.and(String.eq(J.Json.get_str(p, "dispatchId", ""), J.Json.get_str(command, "id", "-")),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "dispatchRevision", 0n), J.Json.get_nat(command, "originalRevision", 1n)),
    Bool.and(Nat.is_eq(J.Json.get_nat(command, "expectedRevision", 0n), J.Json.get_nat(Broker.owner(before, J.Json.get_str(command, "owner", "")), "revision", 1n)),
      Bool.or(Broker.initial_selection_valid(command, p, active), Bool.or(Bool.and(J.Json.get_bool(command, "selects", False{}), String.eq(J.Json.get_str(J.Json.get_obj(command, "expectedTarget"), "tabId", ""), active)), J.Json.get_bool(command, "sawClosed", False{})))))))

def Broker.registry_selection(+command: J.Json, before: Broker, +after: Broker, p: J.Json, active: String) -> J.Json:
  +valid = Broker.registry_selection_valid(command, before, p, active)
  D.Desktop.put(D.Desktop.put(D.Desktop.put(command, "initialSelection", J.Flag{False{}}), "valid", J.Flag{valid}), "expectedRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(after, J.Json.get_str(command, "owner", "")), "revision", 0n))})

def Broker.registry_command_if(sameOwner: Bool, command: J.Json, before: Broker, after: Broker, p: J.Json, active: String) -> J.Json:
  match sameOwner:
    case False{}: command
    case True{}: Broker.registry_selection(command, before, after, p, active)

def Broker.registry_commands(fields: J.Json, +before: Broker, +after: Broker, +p: J.Json, +active: String) -> J.Json:
  match fields:
    case J.Obj{fs}: J.Obj{Broker.registry_commands(fs, before, after, p, active)}
    case J.Field{+id, +command, rest}:
      J.Field{id, Broker.registry_command_if(String.eq(J.Json.get_str(command, "owner", ""), J.Json.get_str(p, "owner", "")), D.Desktop.put(command, "id", J.Str{id}), before, after, p, active), Broker.registry_commands(rest, before, after, p, active)}
    case _: J.End{}

# Rejected/closed roots cannot be offered again. Bounded lifetime IDs prevent
# old document identities or queued callbacks from acquiring a new root.
def Broker.target_count(fields: J.Json, +owner: String, +global: Bool) -> Nat:
  match fields:
    case J.Obj{fs}: Broker.target_count(fs, owner, global)
    case J.Field{_, +target, rest}:
      Nat.add(Bool.pick(Nat, Bool.and(Bool.not(J.Json.get_bool(target, "retired", False{})), Bool.or(global, String.eq(J.Json.get_str(target, "owner", ""), owner))), 1n, 0n), Broker.target_count(rest, owner, global))
    case _: 0n

def Broker.creation_room(+b: Broker, +owner: String) -> Bool:
  Bool.and(Nat.is_lt(Broker.target_count(J.Json.get_obj(Broker.meta(b), "targets"), owner, True{}), 128n),
    Bool.and(Nat.is_lt(Broker.target_count(J.Json.get_obj(Broker.meta(b), "targets"), owner, False{}), 32n),
      Nat.is_lt(D.Target.fields(J.Json.get_obj(Broker.meta(b), "targetIds")), 1024n)))

def Broker.offer_slot(+b: Broker, +owner: String, target: J.Json) -> Bool:
  Bool.and(Broker.creation_room(b, owner), Bool.not(J.Json.get_bool(J.Json.get_obj(Broker.meta(b), "targetIds"), J.Json.get_str(target, "tabId", ""), False{})))

def Broker.remember_target(+b: Broker, id: String) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "targetIds", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "targetIds"), id, J.Flag{True{}})))

def Broker.target_pending(ps: List<&2, Pending>, +id: String) -> Bool:
  match ps:
    case Nil{}: False{}
    case Pending{_, _, target, _, _} <> t:
      Bool.or(String.eq(J.Json.get_str(target, "tabId", ""), id), Broker.target_pending(t, id))

def Broker.prune_row(keep: Bool, id: String, target: J.Json, rest: J.Json) -> J.Json:
  match keep:
    case False{}: rest
    case True{}: J.Field{id, target, rest}

def Broker.prune_fields(fields: J.Json, +ps: List<&2, Pending>, +preparations: J.Json) -> J.Json:
  match fields:
    case J.Obj{fs}: J.Obj{Broker.prune_fields(fs, ps, preparations)}
    case J.Field{+id, +target, rest}:
      Broker.prune_row(Bool.or(Bool.not(J.Json.get_bool(target, "retired", False{})), Bool.or(Broker.target_pending(ps, id), String.eq(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(preparations, J.Json.get_str(target, "owner", "")), "target"), "tabId", ""), id))), id, target, Broker.prune_fields(rest, ps, preparations))
    case _: J.End{}

def Broker.prune_targets(+b: Broker) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "targets", Broker.prune_fields(J.Json.get_obj(Broker.meta(b), "targets"), Broker.pending(b), J.Json.get_obj(Broker.meta(b), "preparations"))))

def Broker.cancel_no_target(j: J.Json) -> Bool:
  match j:
    case J.Null{}: True{}
    case _: False{}

def Broker.cancel_target_valid(known: Bool, +expected: J.Json, +actual: J.Json, owner: String, notCreated: Bool) -> Bool:
  match known:
    case True{}: Bool.and(D.Target.valid(actual), Bool.and(String.eq(J.Json.get_str(expected, "owner", ""), J.Json.get_str(actual, "owner", "-")), Bool.and(String.eq(J.Json.get_str(expected, "tabId", ""), J.Json.get_str(actual, "tabId", "-")), Nat.is_ge(J.Json.get_nat(actual, "generation", 0n), J.Json.get_nat(expected, "generation", 1n)))))
    case False{}:
      Bool.or(Bool.and(notCreated, Broker.cancel_no_target(actual)), Bool.and(D.Target.valid(actual), String.eq(J.Json.get_str(actual, "owner", ""), owner)))

# Preparation has already selected its canonical root. Physical activation of
# that same root changes no permission and must preserve the saved action fence.
def Broker.registry_activation_needed(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(p, "owner", "")
  +active = Broker.registry_active(J.Json.elems(J.Json.get_obj(p, "tabs")))
  Bool.and(Bool.not(Broker.preparing(b, owner)), Bool.and(Broker.pending_owner(Broker.pending(b), owner),
    Bool.and(String.eq(active, J.Json.get_str(Broker.owner(b, owner), "tabId", "")),
      Bool.not(String.eq(active, J.Json.get_str(Broker.owner(b, owner), "activeTabId", J.Json.get_str(Broker.owner(b, owner), "tabId", "")))))))

def Broker.registry_activation_command(+b: Broker, +p: J.Json) -> J.Json:
  +id = Broker.pending_owner_id(Broker.pending(b), J.Json.get_str(p, "owner", ""))
  D.Desktop.put(Broker.command(b, id), "id", J.Str{id})

def Broker.registry_activation_if(needed: Bool, +b: Broker, +p: J.Json) -> Bool:
  match needed:
    case False{}: True{}
    case True{}: Broker.registry_selection_valid(Broker.registry_activation_command(b, p), b, p, Broker.registry_active(J.Json.elems(J.Json.get_obj(p, "tabs"))))

def Broker.registry_activation_valid(+b: Broker, +p: J.Json) -> Bool:
  Broker.registry_activation_if(Broker.registry_activation_needed(b, p), b, p)

def Broker.registry_valid(+b: Broker, +p: J.Json) -> Bool:
  Bool.and(Broker.json_array(J.Json.get_obj(p, "tabs")), Bool.and(Broker.registry_valid_core(b, p), Broker.registry_activation_valid(b, p)))

def Broker.registry_activation_update(needed: Bool, +before: Broker, +after: Broker, +p: J.Json) -> Broker:
  match needed:
    case False{}: after
    case True{}:
      Broker.with_meta(after, D.Desktop.put(Broker.meta(after), "commands", Broker.registry_commands(J.Json.get_obj(Broker.meta(before), "commands"), before, after, p, Broker.registry_active(J.Json.elems(J.Json.get_obj(p, "tabs"))))))

def Broker.registry_activation_apply(+before: Broker, after: Broker, +p: J.Json) -> Broker:
  Broker.registry_activation_update(Broker.registry_activation_needed(before, p), before, after, p)

# Loss of mandatory file interception is not evidence that native contents
# exited. Keep the registry/root capacity, permanently disable that root and
# preserve narrow safety close. No same-root readiness or reattach can repair it.
def Broker.target_guard_lost(+b: Broker, target: J.Json) -> Bool:
  J.Json.get_bool(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(target, "tabId", "")), "guardLost", False{})

def Broker.capture(+b: Broker, +owner: String, +createId: String, +tab: String) -> IO(Unit):
  Broker.emit(b, "recording.grant", "", J.Json.obj([J.KV{"owner", J.Str{owner}},
    J.KV{"revision", J.Num{Nat.show(Broker.capture_revision(b, owner))}}, J.KV{"ttlMs", J.Num{"3000"}},
    J.KV{"root", Bool.pick(J.Json, String.is_empty(tab), J.Json.obj([J.KV{"createId", J.Str{createId}}]), J.Json.obj([J.KV{"tabId", J.Str{tab}}]))},
    J.KV{"documents", J.Str{"prospective"}}, J.KV{"popupDescendants", J.Flag{False{}}},
    J.KV{"enabled", J.Flag{Bool.and(J.Json.get_bool(Broker.owner(b, owner), "recording", False{}), Bool.not(Broker.target_guard_lost(b, J.Json.obj([J.KV{"tabId", J.Str{tab}}]))))}}]))

def Broker.capture_pending(pending: Bool, +b: Broker, +owner: String) -> IO(Unit):
  match pending:
    case False{}: IO.pure(Unit, Unit{})
    case True{}: Broker.capture(b, owner, J.Json.get_str(Broker.owner(b, owner), "createId", ""), "")

def Broker.request_target_valid_core(+b: Broker, +op: String, +args: J.Json) -> Bool:
  +target = Broker.request_target(b, op, args)
  Bool.and(D.Target.valid(target), Bool.and(String.eq(J.Json.get_str(target, "owner", ""), J.Json.get_str(args, "page", "")),
    Bool.and(Bool.not(J.Json.get_bool(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(target, "tabId", "")), "retired", False{})), Bool.or(Broker.close_op(op, args), Bool.not(Broker.target_guard_lost(b, target))))))

def Broker.request_target_valid(+b: Broker, +op: String, +args: J.Json) -> Bool:
  Bool.and(Broker.request_target_valid_core(b, op, args), Bool.or(Maybe.is_none(&2, J.Json, J.Json.get(args, "target")),
    Bool.and(D.Target.valid(J.Json.get_obj(args, "target")), D.Target.eq(D.Target.decode(J.Json.get_obj(args, "target")), D.Target.decode(Broker.request_target(b, op, args))))))

def Broker.ready_valid(+b: Broker, +p: J.Json) -> Bool:
  Bool.and(Bool.not(J.Json.get_bool(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(J.Json.get_obj(p, "target"), "tabId", "")), "retired", False{})), Bool.and(Bool.not(Broker.target_guard_lost(b, J.Json.get_obj(p, "target"))), Broker.ready_valid_core(b, p)))

def Broker.record_valid(+b: Broker, +p: J.Json, +e: J.Json) -> Bool:
  +owner = J.Json.get_str(e, "owner", "")
  +doc = J.Json.get_obj(e, "document")
  +saved = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "documents"), Broker.doc_key(doc))
  Bool.and(Bool.not(Broker.target_guard_lost(b, e)), Bool.and(Bool.not(Broker.unknown_owner(b, owner)), Bool.and(Bool.not(J.Json.get_bool(Broker.target(b, owner), "retired", False{})),
    Bool.and(Broker.main_document_ready(J.Json.get_obj(Broker.meta(b), "documents"), b, Broker.public_target(e)),
    Bool.and(Broker.target_same(b, J.Json.obj([J.KV{"target", Broker.public_target(e)}])), Bool.and(J.Json.get_bool(Broker.owner(b, owner), "recording", False{}),
    Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "captureRevision", 0n), Broker.capture_revision(b, owner)),
    Bool.and(Broker.doc_valid(doc), Bool.and(Broker.doc_same(doc, saved),
    Bool.and(Bool.not(J.Json.get_bool(saved, "retired", False{})), Bool.and(Bool.not(Broker.document_retired(b, e)),
      D.Target.eq(D.Target.decode(saved_target(saved)), D.Target.decode(e))))))))))))))

# Only the hub can request this internal fence. It checks current canonical
# Teach state before requesting. The actor rechecks epoch, original revision
# and document AFTER queueing; no retirement can be applied during the tiny
# pure Teach-state commit. The release channel has one reserved slot; the hub
# releases before broadcasts, persistence jobs or any call back to this actor.
def Broker.capture_commit_valid(+b: Broker, +candidate: J.Json) -> Bool:
  +id = J.Json.get_str(candidate, "id", "")
  +saved = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "captures"), id)
  +p = J.Json.get_obj(candidate, "payload")
  Bool.and(Broker.fenced(b, candidate), Bool.and(Bool.not(String.is_empty(id)),
    Bool.and(String.eq(J.Json.show(saved), J.Json.show(candidate)), Broker.record_valid(b, p, J.Json.get_obj(p, "event")))))

def Broker.generation_valid(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(J.Json.get_obj(p, "target"), "owner", "")
  Bool.and(Bool.not(Broker.target_guard_lost(b, J.Json.get_obj(p, "target"))), Bool.and(Nat.is_eq(J.Json.get_nat(p, "grantRevision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 1n)),
    Bool.or(Bool.not(Broker.preparing(b, owner)), Broker.offer_preparation_valid(b, owner, p))))

def Broker.target_payload_guard(lost: Bool, +b: Broker, +owner: String) -> J.Json:
  match lost:
    case False{}: Broker.target_payload_choose(Broker.preparing(b, owner), b, owner)
    case True{}:
      J.Json.obj([J.KV{"target", Broker.public_target(Broker.target(b, owner))}, J.KV{"ttlMs", J.Num{"3000"}},
        J.KV{"revision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))}}, J.KV{"drive", J.Str{"none"}},
        J.KV{"operations", J.Json.arr([J.Str{"list"}, J.Str{"hide"}, J.Str{"close"}])}, J.KV{"cdpMethods", J.Json.arr(Nil{})}, J.KV{"initializationMethods", J.Json.arr(Nil{})}])

def Broker.target_payload(+b: Broker, +owner: String) -> J.Json:
  Broker.target_payload_guard(Broker.target_guard_lost(b, Broker.target(b, owner)), b, owner)

def Broker.grant_target_row(valid: Bool, +b: Broker, +owner: String, +target: J.Json) -> IO(Unit):
  match valid:
    case False{}: IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        +scoped : Broker = Broker.set_owner(b, owner, D.Desktop.put(Broker.owner(b, owner), "tabId", J.Str{J.Json.get_str(target, "tabId", "")}))
        Broker.emit(b, "target.grant", "", Broker.target_payload(scoped, owner))
        Broker.capture(b, owner, "", J.Json.get_str(target, "tabId", ""))

def Broker.grant_targets(fields: J.Json, +b: Broker, +owner: String) -> IO(Unit):
  match fields:
    case J.Obj{fs}: Broker.grant_targets(fs, b, owner)
    case J.Field{_, +target, rest}:
      do IO<Unit>:
        Broker.grant_target_row(Bool.and(String.eq(J.Json.get_str(target, "owner", ""), owner), Bool.and(Bool.not(J.Json.get_bool(target, "retired", False{})),
          Bool.or(Bool.not(Broker.preparing(b, owner)), String.eq(J.Json.get_str(target, "tabId", ""), J.Json.get_str(Broker.owner(b, owner), "tabId", ""))))), b, owner, target)
        Broker.grant_targets(rest, b, owner)
    case _: IO.pure(Unit, Unit{})

def Broker.grant_known(known: Bool, +b: Broker, +owner: String) -> IO(Unit):
  match known:
    case False{}: IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        Broker.emit(b, "owner.grant", "", Broker.owner_payload(b, owner))
        Broker.capture_pending(Bool.not(String.is_empty(J.Json.get_str(Broker.owner(b, owner), "createId", ""))), b, owner)
        Broker.grant_targets(J.Json.get_obj(Broker.meta(b), "targets"), b, owner)

def Broker.grant(+b: Broker, +owner: String) -> IO(Unit):
  Broker.grant_known(Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}), Bool.not(Broker.unknown_owner(b, owner))), b, owner)

def Broker.release_allowed(clear: Bool, +b: Broker, +owner: String) -> IO(Broker):
  match clear:
    case False{}: IO.pure(Broker, b)
    case True{}:
      do IO<Broker>:
        +b2 : Broker = Broker.bump_owner(Broker.set_owner(b, owner, D.Desktop.put(Broker.owner(b, owner), "barrier", J.Flag{False{}})), owner)
        Broker.grant(b2, owner)
        return b2

def Broker.release_if(clear: Bool, +b: Broker, +owner: String) -> IO(Broker):
  Broker.release_allowed(Bool.and(clear, Bool.not(Broker.unknown_owner(b, owner))), b, owner)

def Broker.ready(ok: Bool, +b: Broker, +p: J.Json, id: String) -> IO(Broker):
  match ok:
    case False{}: Broker.reject(b, id, "unregistered or stale target ready")
    case True{}:
      do IO<Broker>:
        +t : J.Json = J.Json.get_obj(p, "target")
        +old : J.Json = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(t, "tabId", ""))
        +b2 : Broker = Broker.set_target(b, D.Desktop.put(old, "ready", J.Flag{True{}}))
        +b3 : Broker = Broker.document_save(b2, D.Desktop.put(D.Desktop.put(t, "document", J.Json.get_obj(p, "document")), "mainFrame", J.Flag{True{}}))
        Broker.grant(b3, J.Json.get_str(t, "owner", ""))
        Broker.emit(b3, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"prepareId", J.Str{J.Json.get_str(p, "prepareId", "")}}, J.KV{"target", Broker.public_target(t)}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b3, J.Json.get_str(t, "owner", "")), "revision", 0n))}}]))
        return b3

def Broker.policy_emit(changed: Bool, +before: Broker, +after: Broker, +owner: String) -> IO(Broker):
  match changed:
    case False{}: IO.pure(Broker, after)
    case True{}:
      do IO<Broker>:
        Broker.revoke_all(before, owner)
        Broker.grant(after, owner)
        return after

def Broker.policy_update(+b: Broker, +owner: String, known: Bool, drive: String, recording: Bool) -> IO(Broker):
  +after = Broker.policy(b, owner, known, drive, recording)
  Broker.policy_emit(Bool.and(Nat.is_gt(J.Json.get_nat(Broker.owner(b, owner), "revision", 0n), 0n), Nat.is_gt(J.Json.get_nat(Broker.owner(after, owner), "revision", 0n), J.Json.get_nat(Broker.owner(b, owner), "revision", 0n))), b, after, owner)

def Broker.renew(fields: J.Json, +b: Broker) -> IO(Unit):
  match fields:
    case J.Obj{fs}: Broker.renew(fs, b)
    case J.Field{k, _, t}:
      do IO<Unit>:
        Broker.grant(b, k)
        Broker.renew(t, b)
    case _: IO.pure(Unit, Unit{})

def Broker.closed_apply(valid: Bool, +b: Broker, +p: J.Json, +e: J.Json, +id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "foreign, stale or repeated guest retirement")
    case True{}:
      do IO<Broker>:
        +owner : String = J.Json.get_str(e, "owner", "")
        +old : J.Json = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(e, "tabId", ""))
        +b1 : Broker = Broker.set_target(b, D.Desktop.put(D.Desktop.put(D.Desktop.put(old, "ready", J.Flag{False{}}), "retired", J.Flag{True{}}), "generation", J.Num{Nat.show(J.Json.get_nat(e, "generation", 0n))}))
        +b2 : Broker = Broker.bump_owner(Broker.set_owner(b1, owner, D.Desktop.put(Broker.owner(b1, owner), "captureRevision", J.Num{Nat.show(Nat.add(Broker.capture_revision(b1, owner), 1n))})), owner)
        +b3 : Broker = Broker.closed_command(Broker.closed_correlated(b, p, e, Broker.command(b, J.Json.get_str(p, "dispatchId", ""))), b, b2, p, owner)
        Broker.revoke_all(b, owner)
        Broker.grant(b3, owner)
        Broker.emit(b3, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"target", Broker.public_target(e)}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b3, owner), "revision", 0n))}}]))
        return b3

def Broker.closed_event(+b: Broker, +p: J.Json, id: String) -> IO(Broker):
  Broker.closed_apply(Broker.closed_valid(b, J.Json.get_obj(p, "event")), b, p, J.Json.get_obj(p, "event"), id)

def Broker.result_settle(unknown: Bool, +b: Broker, +owner: String) -> IO(Broker):
  match unknown:
    case True{}:
      do IO<Broker>:
        Broker.revoke_all(b, owner)
        return Broker.latch_unknown(b, owner)
    case False{}: Broker.release_if(Bool.and(J.Json.get_bool(Broker.owner(b, owner), "barrier", False{}), Bool.not(Broker.pending_owner(Broker.pending(b), owner))), b, owner)

def Broker.result_correlated(known: Bool, b: Broker, owner: String, p: J.Json) -> IO(Broker):
  match known:
    case False{}: IO.pure(Broker, b)
    case True{}: Broker.result_settle(Broker.result_unknown(p), b, owner)

def Broker.prepare_start(+b: Broker, +op: String, +args: J.Json, +reply: Chan(String)) -> IO(Broker):
  do IO<Broker>:
    +owner : String = J.Json.get_str(args, "page", "")
    +prepare : String <- Broker.nonce()
    +create : String <- Broker.nonce()
    +intent : String <- Broker.nonce()
    +b1 : Broker = Broker.bump_owner(Broker.set_owner(b, owner, D.Desktop.put(Broker.owner(b, owner), "createId", J.Str{create})), owner)
    +rev : Nat = J.Json.get_nat(Broker.owner(b1, owner), "revision", 0n)
    +prep : J.Json = J.Json.obj([J.KV{"prepareId", J.Str{prepare}}, J.KV{"createId", J.Str{create}}, J.KV{"intentId", J.Str{intent}}, J.KV{"revision", J.Num{Nat.show(rev)}}, J.KV{"valid", J.Flag{True{}}}, J.KV{"phase", J.Str{"preparation"}}, J.KV{"target", J.Null{}}, J.KV{"op", J.Str{op}}, J.KV{"args", args}])
    +b2 : Broker = Broker.preparation_put(b1, owner, prep)
    Broker.revoke(b, owner)
    Broker.grant(b2, owner)
    Broker.emit(b2, "browser.prepare", prepare, J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"intentId", J.Str{intent}}, J.KV{"prepareId", J.Str{prepare}}, J.KV{"createId", J.Str{create}}, J.KV{"grantRevision", J.Num{Nat.show(rev)}}]))
    return Broker.replace_pending(b2, Pending{prepare, owner, J.Json.obj([J.KV{"preparation", J.Flag{True{}}}]), rev, reply} <> Broker.pending(b2))

def Broker.event_generation(ok: Bool, +b: Broker, +p: J.Json, id: String) -> IO(Broker):
  match ok:
    case False{}: Broker.reject(b, id, "unknown or non-increasing target generation")
    case True{}:
      do IO<Broker>:
        +t : J.Json = J.Json.get_obj(p, "target")
        +old : J.Json = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(t, "tabId", ""))
        +b2 : Broker = Broker.set_target(b, D.Desktop.put(D.Desktop.put(old, "generation", J.Num{Nat.show(J.Json.get_nat(t, "generation", 0n))}), "ready", J.Flag{False{}}))
        +owner : String = J.Json.get_str(t, "owner", "")
        +b3 : Broker = Broker.bump_owner(b2, owner)
        +b4 : Broker = Broker.command_generation(b, Broker.generation_preparation(Broker.preparing(b, owner), b, b3, owner), p)
        Broker.revoke(b, owner)
        Broker.grant(b4, owner)
        Broker.emit(b4, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"prepareId", J.Str{J.Json.get_str(Broker.preparation(b4, owner), "prepareId", "")}}, J.KV{"target", Broker.public_target(t)}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b4, owner), "revision", 0n))}}]))
        return b4

def Broker.event(+b: Broker, +p: J.Json, +id: String) -> IO(Broker):
  +t = J.Json.get_obj(p, "target")
  +old = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(t, "tabId", ""))
  Broker.event_generation(Bool.and(D.Target.valid(t), Bool.and(Broker.generation_valid(b, p), Bool.and(String.eq(J.Json.get_str(t, "owner", ""), J.Json.get_str(old, "owner", "")),
    Nat.is_eq(J.Json.get_nat(t, "generation", 0n), Nat.add(J.Json.get_nat(old, "generation", 0n), 1n))))), b, p, id)

def Broker.cancelled_apply(destroyed: Bool, +b: Broker, +p: J.Json, +id: String) -> IO(Broker):
  match destroyed:
    case False{}:
      do IO<Broker>:
        +owner : String = J.Json.get_str(p, "owner", "")
        +b1 : Broker = Broker.latch_unknown(b, owner)
        Broker.revoke_all(b, owner)
        Broker.emit(b1, "result", id, J.Json.obj([J.KV{"ok", J.Flag{False{}}}, J.KV{"cleanupUnknown", J.Flag{True{}}}, J.KV{"error", J.Str{"registry retirement is not native destruction; owner remains UNKNOWN"}}]))
        return b1
    case True{}:
      do IO<Broker>:
        +owner : String = J.Json.get_str(p, "owner", "")
        +b1 : Broker = Broker.cancel_finished(b, owner)
        +b2 : Broker <- Broker.release_if(Bool.not(Broker.pending_owner(Broker.pending(b1), owner)), b1, owner)
        Broker.emit(b2, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"cleanupVerified", J.Flag{True{}}}, J.KV{"unknown", J.Flag{Broker.unknown_owner(b2, owner)}}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b2, owner), "revision", 0n))}}]))
        return b2

def Broker.cancelled_if(valid: Bool, b: Broker, +p: J.Json, id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "foreign, stale or incomplete creation cancellation receipt")
    case True{}: Broker.cancelled_apply(Broker.cancel_destroyed(p), b, p, id)

def Broker.prepared_item(item: Maybe<&2, Pending>, +b: Broker, +owner: String, +prepare: String) -> IO(Broker):
  match item:
    case None{}: IO.pure(Broker, b)
    case Some{Pending{_, _, _, _, reply}}:
      do IO<Broker>:
        +prep : J.Json = Broker.preparation(b, owner)
        +b1 : Broker = Broker.replace_pending(Broker.preparation_put(b, owner, J.Json.obj(Nil{})), Broker.pending_remove(Broker.pending(b), prepare))
        +b2 : Broker = Broker.bump_owner(b1, owner)
        Broker.revoke(b, owner)
        Broker.grant(b2, owner)
        Broker.dispatch_fresh(b2, J.Json.get_str(prep, "op", ""), J.Json.get_obj(prep, "args"), reply, J.Json.get_str(prep, "intentId", ""), prepare, J.Json.get_str(prep, "createId", ""))

def Broker.prepared_if(valid: Bool, +b: Broker, +p: J.Json, id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "stale or unready preparation; original command not dispatched")
    case True{}: Broker.prepared_item(Broker.prepared_find(Broker.pending(b), J.Json.get_str(p, "prepareId", "")), b, J.Json.get_str(p, "owner", ""), J.Json.get_str(p, "prepareId", ""))

def Broker.prepared(+b: Broker, +p: J.Json, id: String) -> IO(Broker):
  Broker.prepared_if(Broker.prepared_valid(b, p), b, p, id)

def Broker.registry_changed(changed: Bool, +before: Broker, +after: Broker, +owner: String, +p: J.Json, +active: String) -> IO(Broker):
  match changed:
    case False{}: IO.pure(Broker, after)
    case True{}:
      do IO<Broker>:
        +b1 : Broker = Broker.bump_owner(Broker.set_owner(after, owner, D.Desktop.put(Broker.owner(after, owner), "tabId", J.Str{active})), owner)
        +b2 : Broker = Broker.with_meta(b1, D.Desktop.put(Broker.meta(b1), "commands", Broker.registry_commands(J.Json.get_obj(Broker.meta(before), "commands"), before, b1, p, active)))
        Broker.revoke(before, owner)
        Broker.grant(b2, owner)
        return b2

def Broker.offer(ok: Bool, +b: Broker, +owner: String, +p: J.Json, id: String) -> IO(Broker):
  match ok:
    case False{}: Broker.reject(b, id, "unknown owner/create or target collision")
    case True{}:
      do IO<Broker>:
        +t : J.Json = J.Json.get_obj(p, "target")
        +b2 : Broker = Broker.set_target(Broker.remember_target(Broker.prune_targets(b), J.Json.get_str(t, "tabId", "")), D.Desktop.put(D.Desktop.put(t, "ready", J.Flag{False{}}), "webContentsId", J.Num{Nat.show(J.Json.get_nat(p, "webContentsId", 0n))}))
        +b3 : Broker = Broker.bump_owner(Broker.set_owner(b2, owner, D.Desktop.put(D.Desktop.put(Broker.owner(b2, owner), "tabId", J.Str{J.Json.get_str(t, "tabId", "")}), "createId", J.Str{""})), owner)
        +b4 : Broker = Broker.preparation_advance(b, b3, owner)
        Broker.revoke(b, owner)
        Broker.grant(b4, owner)
        Broker.emit(b4, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"prepareId", J.Str{J.Json.get_str(Broker.preparation(b4, owner), "prepareId", "")}}, J.KV{"target", Broker.public_target(t)}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b4, owner), "revision", 0n))}}]))
        return b4

def Broker.prepare_capacity(room: Bool, b: Broker, op: String, args: J.Json, reply: Chan(String)) -> IO(Broker):
  match room:
    case True{}: Broker.prepare_start(b, op, args, reply)
    case False{}:
      do IO<Broker>:
        Broker.send(Some{reply}, D.Desktop.error("", "guest capacity exhausted; no creation or manager reset", False{}))
        return b

def Broker.call_existing(create: Bool, +b: Broker, +op: String, +args: J.Json, reply: Chan(String)) -> IO(Broker):
  match create:
    case True{}: Broker.prepare_capacity(Broker.creation_room(b, J.Json.get_str(args, "page", "")), b, op, args, reply)
    case False{}:
      Broker.call_ready(Bool.and(Broker.request_target_valid(b, op, args), Bool.or(Broker.close_op(op, args), J.Json.get_bool(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(Broker.request_target(b, op, args), "tabId", "")), "ready", False{}))), b, op, args, reply)

# which of Broker.call's conditions refused it (the first that fails), for
# the log line
def Broker.refused_why(+b: Broker, +op: String, +args: J.Json) -> String:
  +owner = J.Json.get_str(args, "page", "")
  Bool.pick(String, Bool.not(J.Json.get_bool(Broker.meta(b), "ready", False{})), "no desktop guest is ready (no app attached, or its channel closed)",
  Bool.pick(String, Bool.not(J.Json.get_bool(Broker.owner(b, owner), "known", False{})), "owner " ++ owner ++ " is not known to the desktop",
  Bool.pick(String, Broker.unknown_owner(b, owner), "owner " ++ owner ++ " is marked unknown",
  Bool.pick(String, Broker.pending_owner(Broker.pending(b), owner), "owner " ++ owner ++ " has a command pending",
  Bool.pick(String, Broker.preparing(b, owner), "owner " ++ owner ++ " is being prepared",
  Bool.pick(String, Bool.and(String.eq(J.Json.get_str(args, "__desktopActor", "service"), "agent"), Bool.not(String.eq(J.Json.get_str(Broker.owner(b, owner), "drive", "none"), "agent"))),
    "owner " ++ owner ++ " is not driven by its agent (drive " ++ J.Json.get_str(Broker.owner(b, owner), "drive", "none") ++ ")",
    "too many commands pending")))))) ++ " (" ++ op ++ ")"

# what a refused caller is told: a thread latched by an unknown outcome
# (Broker.close keeps it for the hub's life, never retried) hears which
# thread, why, and what clears it; any other refusal keeps the old words
# with its reason
def Broker.refusal.k(latched: Bool, +owner: String, why: String) -> String:
  match latched:
    case True{}:
      "the browser is closed to thread " ++ owner ++ ": its last browser command has an unknown outcome, so the browser stays closed to it until the hub restarts or the person resets it"
    case False{}:
      "direct desktop guest unavailable or owner authority refused: " ++ why

def Broker.refusal(+b: Broker, +op: String, +args: J.Json) -> String:
  Broker.refusal.k(Broker.unknown_owner(b, J.Json.get_str(args, "page", "")), J.Json.get_str(args, "page", ""), Broker.refused_why(b, op, args))

def Broker.refuse_call(ok: Bool, +b: Broker, +q: Chan(Message), +op: String, +args: J.Json, +reply: Chan(String)) -> IO(Broker):
  match ok:
    case False{}:
      do IO<Broker>:
        PF.Log.warn("desktop", "browser call refused: " ++ Broker.refused_why(b, op, args))
        Broker.send(Some{reply}, D.Desktop.error("", Broker.refusal(b, op, args), False{}))
        return b
    case True{}:
      do IO<Broker>:
        +owner : String = J.Json.get_str(args, "page", "")
        +b2 : Broker <- Broker.call_existing(Bool.or(Broker.new_op(op, args), Bool.and(String.is_empty(Broker.request_tab(op, args)), Bool.and(Maybe.is_none(&2, J.Json, J.Json.get(args, "target")), String.is_empty(J.Json.get_str(Broker.owner(b, owner), "tabId", ""))))), b, op, args, reply)
        Broker.start_owner_deadline(Broker.pending(b2), q, owner)
        return b2

def Broker.call(+b: Broker, q: Chan(Message), op: String, +args: J.Json, reply: Chan(String)) -> IO(Broker):
  +owner = J.Json.get_str(args, "page", "")
  Broker.refuse_call(Bool.and(J.Json.get_bool(Broker.meta(b), "ready", False{}),
    Bool.and(Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}), Bool.and(Bool.not(Broker.unknown_owner(b, owner)), Bool.and(Bool.not(Broker.pending_owner(Broker.pending(b), owner)), Bool.not(Broker.preparing(b, owner))))),
    Bool.and(Bool.or(Bool.not(String.eq(J.Json.get_str(args, "__desktopActor", "service"), "agent")), String.eq(J.Json.get_str(Broker.owner(b, owner), "drive", "none"), "agent")),
      Nat.is_lt(List.length(&2, Pending, Broker.pending(b)), 32n)))), b, q, op, args, reply)

def Broker.registry_apply(valid: Bool, +b: Broker, +p: J.Json, +id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "foreign, stale, incomplete or duplicated guest inventory")
    case True{}:
      do IO<Broker>:
        +owner : String = J.Json.get_str(p, "owner", "")
        +rows : List<&2, J.Json> = J.Json.elems(J.Json.get_obj(p, "tabs"))
        +active : String = Broker.registry_active(rows)
        +b1 : Broker = Broker.registry_activation_apply(b, Broker.registry_save(rows, b), p)
        +b2 : Broker = Broker.set_owner(b1, owner, D.Desktop.put(Broker.owner(b1, owner), "activeTabId", J.Str{active}))
        +b3 : Broker <- Broker.registry_changed(Bool.and(Bool.not(Broker.preparing(b, owner)), Bool.not(String.eq(active, J.Json.get_str(Broker.owner(b, owner), "tabId", "")))), b, b2, owner, p, active)
        Broker.emit(b3, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"target", Broker.public_target(J.Json.get_obj(J.Json.get_obj(Broker.meta(b3), "targets"), active))}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b3, owner), "revision", 0n))}}]))
        return b3

def Broker.guard_lost_valid(+b: Broker, +p: J.Json) -> Bool:
  +e = J.Json.get_obj(p, "event")
  +old = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(e, "tabId", ""))
  Bool.and(String.eq(J.Json.get_str(e, "type", ""), "file-guard-lost"),
    Bool.and(Broker.target_same(b, J.Json.obj([J.KV{"target", Broker.public_target(e)}])),
      Bool.and(Bool.not(J.Json.get_bool(old, "retired", False{})), Bool.not(J.Json.get_bool(old, "guardLost", False{})))))

def Broker.guard_lost_apply(+b: Broker, +p: J.Json) -> Broker:
  +e = J.Json.get_obj(p, "event")
  +owner = J.Json.get_str(e, "owner", "")
  +old = J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(e, "tabId", ""))
  +b1 = Broker.set_target(b, D.Desktop.put(D.Desktop.put(old, "guardLost", J.Flag{True{}}), "ready", J.Flag{False{}}))
  +b2 = Broker.bump_owner(Broker.set_owner(b1, owner, D.Desktop.put(Broker.owner(b1, owner), "captureRevision", J.Num{Nat.show(Nat.add(Broker.capture_revision(b1, owner), 1n))})), owner)
  Broker.closed_unexpected(Broker.owner_has_dispatch(Broker.pending(b), owner), b2, owner)

def Broker.guard_lost_if(valid: Bool, +b: Broker, +p: J.Json, id: String) -> IO(Broker):
  match valid:
    case False{}: Broker.reject(b, id, "foreign, stale or repeated mandatory guard loss")
    case True{}:
      do IO<Broker>:
        +owner : String = J.Json.get_str(J.Json.get_obj(p, "event"), "owner", "")
        +b1 : Broker = Broker.guard_lost_apply(b, p)
        Broker.revoke_all(b, owner)
        Broker.grant(b1, owner)
        Broker.emit(b1, "result", id, J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"target", Broker.public_target(J.Json.get_obj(p, "event"))}, J.KV{"grantRevision", J.Num{Nat.show(J.Json.get_nat(Broker.owner(b1, owner), "revision", 0n))}}]))
        return b1

def Broker.guard_lost(+b: Broker, +p: J.Json, id: String) -> IO(Broker):
  Broker.guard_lost_if(Broker.guard_lost_valid(b, p), b, p, id)

def Broker.event_kind(kind: String, +b: Broker, +p: J.Json, +e: J.Json, +id: String, ev: Chan(String)) -> IO(Broker):
  match kind:
    case "generation": Broker.reject(b, id, "generation requires correlated guest.generation pathway")
    case "file-guard-lost": Broker.guard_lost(b, p, id)
    case "recorder-ready": Broker.document_ready(Bool.and(Bool.not(Broker.target_guard_lost(b, e)), Bool.and(Bool.not(Broker.document_retired(b, e)), Bool.and(Broker.target_same(b, J.Json.obj([J.KV{"target", Broker.public_target(e)}])), Broker.doc_bound(b, e)))), b, e, id)
    case "record": Broker.record(Broker.record_valid(b, p, e), b, p, ev, id)
    case "recorder-document-retired": Broker.retirement(Broker.retirement_valid(b, e), b, e, id)
    case "closed": Broker.closed_event(b, p, id)
    case "crashed": Broker.closed_event(b, p, id)
    case "debugger-detached": Broker.closed_event(b, p, id)
    case _: IO.pure(Broker, b)

def Broker.guest_event_if(valid: Bool, b: Broker, p: J.Json, +e: J.Json, id: String, ev: Chan(String)) -> IO(Broker):
  match valid:
    case True{}: Broker.event_kind(J.Json.get_str(e, "type", ""), b, p, e, id, ev)
    case False{}: Broker.reject(b, id, "malformed guest lifecycle identity")

def Broker.guest_event(+b: Broker, +p: J.Json, id: String, ev: Chan(String)) -> IO(Broker):
  +e = J.Json.get_obj(p, "event")
  Broker.guest_event_if(D.Target.number(Maybe.default(&2, J.Json, J.Json.get(e, "generation"), J.Null{})), b, p, e, id, ev)

# Explicit projection into the existing Browser toolbar schema. Never publish
# raw lifecycle/recorder payloads or private grant/command/document metadata.
def Broker.public_row(+row: J.Json, index: Nat) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{J.Json.get_str(row, "tabId", "")}}, J.KV{"index", J.Num{Nat.show(index)}},
    J.KV{"url", J.Str{J.Json.get_str(row, "url", "")}}, J.KV{"title", J.Str{J.Json.get_str(row, "title", "")}}, J.KV{"active", J.Flag{J.Json.get_bool(row, "active", False{})}}])

def Broker.public_rows(rows: List<&2, J.Json>, +index: Nat) -> List<&2, J.Json>:
  match rows:
    case Nil{}: Nil{}
    case h <> t: Broker.public_row(h, index) <> Broker.public_rows(t, Nat.add(index, 1n))

def Broker.public_url_if(active: Bool, row: J.Json, rest: String) -> String:
  match active:
    case True{}: J.Json.get_str(row, "url", "")
    case False{}: rest

def Broker.public_url(rows: List<&2, J.Json>) -> String:
  match rows:
    case Nil{}: ""
    case +h <> t: Broker.public_url_if(J.Json.get_bool(h, "active", False{}), h, Broker.public_url(t))

def Broker.public_tabs(+p: J.Json) -> J.Json:
  +rows = J.Json.elems(J.Json.get_obj(p, "tabs"))
  J.Json.obj([J.KV{"event", J.Str{"tabs"}}, J.KV{"page", J.Str{J.Json.get_str(p, "owner", "")}},
    J.KV{"url", J.Str{Broker.public_url(rows)}}, J.KV{"tabs", J.Json.arr(Broker.public_rows(rows, 0n))}])

def Broker.public_cache(+b: Broker, +event: J.Json) -> Broker:
  Broker.with_meta(b, D.Desktop.put(Broker.meta(b), "publicTabs", D.Desktop.put(J.Json.get_obj(Broker.meta(b), "publicTabs"), J.Json.get_str(event, "page", ""), event)))

def Broker.publish_cached(fields: J.Json, +ev: Chan(String)) -> IO(Unit):
  match fields:
    case J.Obj{fs}: Broker.publish_cached(fs, ev)
    case J.Field{_, event, rest}:
      do IO<Unit>:
        Broker.send(Some{ev}, J.Json.show(event))
        Broker.publish_cached(rest, ev)
    case _: IO.pure(Unit, Unit{})

def Broker.registry_frame_if(valid: Bool, +b: Broker, +p: J.Json, id: String, ev: Chan(String)) -> IO(Broker):
  match valid:
    case False{}: Broker.registry_apply(False{}, b, p, id)
    case True{}:
      do IO<Broker>:
        +b1 : Broker <- Broker.registry_apply(True{}, b, p, id)
        +event : J.Json = Broker.public_tabs(p)
        Broker.send(Some{ev}, J.Json.show(event))
        return Broker.public_cache(b1, event)

def Broker.registry_frame(+b: Broker, +p: J.Json, id: String, ev: Chan(String)) -> IO(Broker):
  Broker.registry_frame_if(Bool.and(Broker.registry_valid(b, p), D.Desktop.bytes_within(J.Json.show(Broker.public_tabs(p)), 1048512n)), b, p, id, ev)

def Broker.cancel_unknown_flag(value: Maybe<&2, J.Json>) -> Bool:
  match value:
    case None{}: True{}
    case Some{j}: Broker.json_flag(j)

def Broker.cancel_receipt_valid(+b: Broker, +p: J.Json) -> Bool:
  +owner = J.Json.get_str(p, "owner", "")
  +r = J.Json.get_obj(p, "receipt")
  +expected = J.Json.get_obj(Broker.preparation(b, owner), "target")
  Bool.and(Broker.cancel_key(b, p), Bool.and(String.eq(J.Json.get_str(r, "owner", ""), owner),
    Bool.and(String.eq(J.Json.get_str(r, "createId", ""), J.Json.get_str(p, "createId", "-")),
    Bool.and(Broker.cancel_unknown_flag(J.Json.get(r, "closeUnknown")), Bool.and(J.Json.get_bool(r, "closed", False{}), Bool.and(Broker.json_flag(Maybe.default(&2, J.Json, J.Json.get(r, "nativeDestroyed"), J.Null{})),
      Broker.cancel_target_valid(D.Target.valid(expected), expected, J.Json.get_obj(r, "target"), owner, J.Json.get_bool(p, "notCreated", False{}))))))))

def Broker.cancelled(+b: Broker, +p: J.Json, id: String) -> IO(Broker):
  Broker.cancelled_if(Broker.cancel_receipt_valid(b, p), b, p, id)

def Broker.frame_op(op: String, +b: Broker, +o: J.Json, +owner: String, version: String, ev: Chan(String)) -> IO(Broker):
  match op:
    case "hello":
      Broker.accept_hello(Bool.and(Bool.not(J.Json.get_bool(Broker.meta(b), "ready", False{})), Nat.is_eq(J.Json.get_nat(o, "v", 0n), 1n)), b, J.Json.get_str(o, "id", ""), version)
    case "owner.request":
      Broker.request_owner(Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}), Bool.not(Broker.unknown_owner(b, owner))), b, owner, J.Json.get_str(o, "id", ""))
    case "target.offer":
      +p = J.Json.get_obj(o, "payload")
      +t = J.Json.get_obj(p, "target")
      Broker.offer(Bool.and(Broker.offer_slot(b, owner, t), Bool.and(Broker.offer_preparation_valid(b, owner, p), Bool.and(D.Target.valid(t), Bool.and(Nat.is_gt(J.Json.get_nat(p, "webContentsId", 0n), 0n), Bool.and(J.Json.get_bool(Broker.owner(b, owner), "known", False{}),
        Bool.and(String.eq(J.Json.get_str(p, "createId", ""), J.Json.get_str(Broker.owner(b, owner), "createId", "-")),
          String.is_empty(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(Broker.meta(b), "targets"), J.Json.get_str(t, "tabId", "")), "owner", "")))))))), b, owner, p, J.Json.get_str(o, "id", ""))
    case "target.ready":
      Broker.ready(Bool.and(Broker.ready_valid(b, J.Json.get_obj(o, "payload")), Broker.preparation_ready_valid(b, owner, J.Json.get_obj(o, "payload"))), b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "browser.prepared": Broker.prepared(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "browser.prepare-failed": Broker.prepare_failed(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "browser.cancelled": Broker.cancelled(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "guest.guard-lost": Broker.guard_lost(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "guest.closed": Broker.closed_event(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "guest.tabs": Broker.registry_frame(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""), ev)
    case "guest.generation": Broker.event(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""))
    case "guest.event": Broker.guest_event(b, J.Json.get_obj(o, "payload"), J.Json.get_str(o, "id", ""), ev)
    case "result":
      do IO<Broker>:
        +request : String = J.Json.get_str(o, "id", "")
        +p : J.Json = Broker.checked_result(Broker.pending(b), b, request, J.Json.get_obj(o, "payload"))
        +actualOwner : String = Broker.pending_original_owner(Broker.pending(b), request)
        ps : List<&2, Pending> <- Broker.result(Broker.pending(b), b, request, p)
        +b2 : Broker = Broker.command_drop(Broker.replace_pending(b, ps), request)
        Broker.result_correlated(Bool.not(String.is_empty(actualOwner)), b2, actualOwner, p)
    case "heartbeat":
      do IO<Broker>:
        Broker.renew(J.Json.get_obj(Broker.meta(b), "owners"), b)
        Broker.emit(b, "result", J.Json.get_str(o, "id", ""), J.Json.obj([J.KV{"ok", J.Flag{True{}}}]))
        return b
    case _: Broker.reject(b, J.Json.get_str(o, "id", ""), "unsupported desktop operation")

def Broker.frame_ok(valid: Bool, +b: Broker, +o: J.Json, owner: String, version: String, ev: Chan(String)) -> IO(Broker):
  match valid:
    case False{}: Broker.close(b, "stale, malformed or replayed desktop frame; no command replay")
    case True{}: Broker.frame_op(J.Json.get_str(o, "op", ""), Broker.sequence_apply(b, o), o, owner, version, ev)

def Broker.frame(+b: Broker, +text: String, +owner: String, version: String, ev: Chan(String)) -> IO(Broker):
  +o = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
  Broker.frame_ok(Bool.and(D.Desktop.unique(o, Nil{}), Bool.and(Nat.is_eq(J.Json.get_nat(o, "v", 0n), 1n),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(o, "id", ""))), Bool.and(Broker.client_sequence_valid(b, o),
      Bool.or(String.eq(J.Json.get_str(o, "op", ""), "hello"), Broker.fenced(b, o)))))), b, o, owner, version, ev)

def Broker.link_current(same: Bool, kind: String, +b: Broker, text: String, owner: String, version: String, ev: Chan(String)) -> IO(Broker):
  match same kind:
    case True{} "closed": Broker.close(b, "desktop disconnected; command outcome unknown, no retry")
    case True{} "frame": Broker.frame(b, text, owner, version, ev)
    case _ _: IO.pure(Broker, b)

def Broker.link(kind: String, +b: Broker, +client: U32, text: String, out: Chan(String), epoch: String, lease: String, owner: String, version: String, ev: Chan(String)) -> IO(Broker):
  match kind:
    case "connected": Broker.connect_info(U32.is_eq(Broker.client(b), 0), b, client, out, epoch, lease, text)
    case _: Broker.link_current(U32.is_eq(client, Broker.client(b)), kind, b, text, owner, version, ev)

def Broker.step(m: Maybe<&1, Message>, +b: Broker, +q: Chan(Message), +version: String, +ev: Chan(String), k: Broker -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{Link{+client, +kind, +text, out, +e, l, +owner, known, drive, recording}}:
      do IO<Unit>:
        +policy : Broker <- Broker.policy_update(b, owner, known, drive, recording)
        +b2 : Broker <- Broker.link(kind, policy, client, text, out, e, l, owner, version, ev)
        Broker.new_dispatch_deadline(policy, b2, q, owner)
        +b3 : Broker <- Broker.lease_tick(b2, q, text)
        b4 : Broker <- Broker.initial_lease(Bool.and(String.eq(kind, "connected"), U32.is_eq(client, Broker.client(b3))), b3, q, e)
        k(b4)
    case Some{Refresh{}}:
      IO.bind(Unit, Unit, Broker.publish_cached(J.Json.get_obj(Broker.meta(b), "publicTabs"), ev), _ => k(b))
    case Some{Policy{owner, known, drive, recording}}:
      IO.bind(Broker, Unit, Broker.policy_update(b, owner, known, drive, recording), k)
    case Some{CaptureCommit{+candidate, permit, released}}:
      IO.bind(Broker, Unit, Broker.capture_permit(Broker.capture_commit_valid(b, candidate), b, candidate, permit, released), k)
    case Some{Call{op, args, reply}}: IO.bind(Broker, Unit, Broker.call(b, q, op, args, reply), k)
    case Some{Deadline{+id}}: Broker.deadline_step(b, q, id, k)
    case None{}: IO.bind(Broker, Unit, Broker.close(b, "desktop broker closed"), _ => IO.pure(Unit, Unit{}))

def Broker.loop(fuel: Nat, +q: Chan(Message), +version: String, +ev: Chan(String), b: Broker) -> IO(Unit):
  match fuel:
    case 0n: IO.bind(Broker, Unit, Broker.close(b, "desktop broker stopped"), _ => IO.pure(Unit, Unit{}))
    case 1n+n:
      IO.bind(Maybe<&1, Message>, Unit, Chan.recv(Message, q), m => Broker.step(m, b, q, version, ev, b2 => Broker.loop(n, q, version, ev, b2)))
