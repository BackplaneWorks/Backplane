import Base
import ../src/server/desktop-broker.bend as B
import ../src/core/desktop-browser.bend as D
import ../src/core/json.bend as J

def check(name: String, ok: Bool) -> IO(Unit):
  match ok:
    case True{}: IO.print("ok " ++ name)
    case False{}: IO.die(Unit, 1, "FAIL " ++ name)

def obj(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def target() -> J.Json:
  obj("{\"owner\":\"A\",\"tabId\":\"tab-A\",\"generation\":2}")

def document() -> J.Json:
  obj("{\"documentNonce\":\"doc-A\",\"frameToken\":\"frame-A\",\"frameTreeNodeId\":3,\"webContentsId\":4}")

def meta() -> J.Json:
  obj("{\"ready\":true,\"owners\":{\"A\":{\"known\":true,\"drive\":\"agent\",\"recording\":true,\"revision\":7,\"tabId\":\"tab-A\",\"createId\":\"create-A\"}},\"targets\":{\"tab-A\":{\"owner\":\"A\",\"tabId\":\"tab-A\",\"generation\":2,\"webContentsId\":4,\"ready\":true}}}")

def state(+out: Chan(String)) -> B.Broker:
  B.Broker{42, "epoch-A", "lease-A", Some{out}, meta(), Nil{}, 1n}

def event() -> J.Json:
  D.Desktop.put(D.Desktop.put(target(), "document", document()), "mainFrame", J.Flag{True{}})

def result() -> J.Json:
  J.Json.obj([J.KV{"target", target()}, J.KV{"reply", J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"result", J.Str{"actual result"}}])}])

def record() -> J.Json:
  J.Json.obj([J.KV{"captureRevision", J.Num{"7"}}, J.KV{"event", event()}])

def pending(+out: Chan(String)) -> List<&2, B.Pending>:
  [B.Pending{"request-A", "A", target(), 7n, out}]

def text(m: Maybe<&1, String>) -> String:
  Maybe.default(&1, String, m, "")

def sub_event() -> J.Json:
  D.Desktop.put(D.Desktop.put(event(), "document", obj("{\"documentNonce\":\"sub-old\",\"frameToken\":\"sub-frame\",\"frameTreeNodeId\":6,\"webContentsId\":4}")), "mainFrame", J.Flag{False{}})

def retirement() -> J.Json:
  D.Desktop.put(sub_event(), "reason", J.Str{"subframe-navigation"})

def sub_record() -> J.Json:
  J.Json.obj([J.KV{"captureRevision", J.Num{"7"}}, J.KV{"event", D.Desktop.put(sub_event(), "step", obj("{\"do\":\"click\",\"target\":{\"text\":\"public button\"}}"))}])

def finished(+out: Chan(B.Broker), b: B.Broker) -> IO(Unit):
  do IO<Unit>:
    sent : Result<&1, &1, B.Broker, Unit> <- Chan.send(B.Broker, out, b)
    return Unit{}

def state_result(m: Maybe<&1, B.Broker>) -> B.Broker:
  match m:
    case Some{b}: b
    case None{}: B.Broker.empty()

def main() -> IO(Unit):
  do IO<Unit>:
    +out : Chan(String) <- Chan.new(String, 64)
    +reply : Chan(String) <- Chan.new(String, 16)
    +q : Chan(B.Message) <- Chan.new(B.Message, 16)
    +b : B.Broker = state(out)
    check("unchanged canonical policy keeps authority revision", Nat.is_eq(J.Json.get_nat(B.Broker.owner(B.Broker.policy(b, "A", True{}, "agent", True{}), "A"), "revision", 0n), 7n))
    check("canonical takeover increments revision", Nat.is_eq(J.Json.get_nat(B.Broker.owner(B.Broker.policy(b, "A", True{}, "person", True{}), "A"), "revision", 0n), 8n))
    check("person explicit grant excludes CDP", Bool.not(D.Desktop.member(B.Broker.operations("person", False{}), "cdp")))
    check("agent explicit grant excludes direct input", Bool.not(D.Desktop.member(B.Broker.operations("agent", False{}), "direct-input")))
    check("agent explicit grant excludes focus", Bool.not(D.Desktop.member(B.Broker.operations("agent", False{}), "focus")))
    check("barrier excludes input and CDP", Bool.not(D.Desktop.member(B.Broker.operations("none", False{}), "cdp")))
    check("exact registered target accepted", B.Broker.target_same(b, result()))
    check("foreign target rejected", Bool.not(B.Broker.target_same(b, J.Json.obj([J.KV{"target", D.Desktop.put(target(), "owner", J.Str{"B"})}]))))
    check("exact result target and revision accepted", B.Broker.result_valid(b, "A", target(), 7n, result()))
    check("stale result revision rejected", Bool.not(B.Broker.result_valid(b, "A", target(), 6n, result())))
    check("result cannot switch tab", Bool.not(B.Broker.result_valid(b, "A", D.Desktop.put(target(), "tabId", J.Str{"other"}), 7n, result())))
    check("ready hash required", Bool.not(B.Broker.ready_valid(b, J.Json.obj([J.KV{"target", target()}, J.KV{"document", document()}, J.KV{"recorderReady", J.Flag{True{}}}]))))
    check("document WC identity bound to offered root", B.Broker.doc_bound(b, event()))
    check("foreign WC identity refused", Bool.not(B.Broker.doc_bound(b, D.Desktop.put(event(), "document", D.Desktop.put(document(), "webContentsId", J.Num{"5"})))))
    +withDoc : B.Broker = B.Broker.document_save(b, event())
    check("current installed document accepted for capture", B.Broker.record_valid(withDoc, record(), event()))
    check("old capture revision refused", Bool.not(B.Broker.record_valid(withDoc, D.Desktop.put(record(), "captureRevision", J.Num{"6"}), event())))
    check("old document nonce refused", Bool.not(B.Broker.record_valid(withDoc, record(), D.Desktop.put(event(), "document", D.Desktop.put(document(), "documentNonce", J.Str{"old"})))))
    check("foreign document frame token refused", Bool.not(B.Broker.record_valid(withDoc, record(), D.Desktop.put(event(), "document", D.Desktop.put(document(), "frameToken", J.Str{"other"})))))
    +next : B.Broker = B.Broker.set_target(withDoc, D.Desktop.put(D.Desktop.put(target(), "generation", J.Num{"3"}), "ready", J.Flag{False{}}))
    check("old document generation cannot record", Bool.not(B.Broker.record_valid(next, record(), event())))
    +revoked : B.Broker = B.Broker.policy(withDoc, "A", True{}, "person", False{})
    check("Teach stop refuses capture despite installed proof", Bool.not(B.Broker.record_valid(revoked, record(), event())))
    +busy : B.Broker = B.Broker.replace_pending(b, pending(reply))
    +takeover : B.Broker = B.Broker.policy(busy, "A", True{}, "person", False{})
    check("pending command installs revoke barrier", String.eq(B.Broker.effective_drive(takeover, "A"), "none"))
    check("stale epoch frame refused", Bool.not(B.Broker.fenced(b, obj("{\"serviceEpoch\":\"old\",\"controllerLease\":\"lease-A\"}"))))
    check("stale controller frame refused", Bool.not(B.Broker.fenced(b, obj("{\"serviceEpoch\":\"epoch-A\",\"controllerLease\":\"old\"}"))))
    +seen : B.Broker = B.Broker.sequence_apply(b, obj("{\"clientSequence\":1}"))
    check("accepted frame sequence tracked", Nat.is_eq(J.Json.get_nat(B.Broker.meta(seen), "clientSequence", 0n), 1n))
    check("same sequence cannot replay a completed frame", Bool.not(B.Broker.client_sequence_valid(seen, obj("{\"clientSequence\":1}"))))
    ps : List<&2, B.Pending> <- B.Broker.result(pending(reply), b, "request-A", result())
    answer : Maybe<&1, String> <- Chan.recv(String, reply)
    check("actual pending result delivered once", J.Json.get_bool(obj(text(answer)), "ok", False{}))
    check("pending result removed", List.is_empty(&2, B.Pending, ps))
    ps2 : List<&2, B.Pending> <- B.Broker.result(pending(reply), revoked, "request-A", result())
    stale : Maybe<&1, String> <- Chan.recv(String, reply)
    check("revoked in-flight result reports unknown", J.Json.get_bool(obj(text(stale)), "dispatched", False{}))
    check("revoked in-flight completion removed", List.is_empty(&2, B.Pending, ps2))
    +person : B.Broker = B.Broker.policy(b, "A", True{}, "person", False{})
    stopped : B.Broker <- B.Broker.call(person, q, "evaluate", obj("{\"page\":\"A\",\"__desktopActor\":\"agent\"}"), reply)
    refused : Maybe<&1, String> <- Chan.recv(String, reply)
    check("production actor refuses queued agent after takeover", Bool.not(J.Json.get_bool(obj(text(refused)), "ok", True{})))
    check("refused command never enters pending", List.is_empty(&2, B.Pending, B.Broker.pending(stopped)))
    # a thread latched by an unknown outcome is told which thread, why and
    # what clears it; another refusal keeps the old words with its reason
    +latched : B.Broker = B.Broker{42, "epoch-A", "lease-A", Some{out}, D.Desktop.put(meta(), "unknownOwners", obj("{\"A\":true}")), Nil{}, 1n}
    still : B.Broker <- B.Broker.call(latched, q, "snapshot", obj("{\"page\":\"A\",\"__desktopActor\":\"agent\"}"), reply)
    why : Maybe<&1, String> <- Chan.recv(String, reply)
    check("a latched thread hears its thread, the unknown outcome and what clears it", String.eq(J.Json.get_str(obj(text(why)), "error", ""), "the browser is closed to thread A: its last browser command has an unknown outcome, so the browser stays closed to it until the hub restarts or the person resets it"))
    check("a latched thread's call never enters pending", List.is_empty(&2, B.Pending, B.Broker.pending(still)))
    check("another refusal keeps its words and says why", String.starts_with(B.Broker.refusal(person, "evaluate", obj("{\"page\":\"A\",\"__desktopActor\":\"agent\"}")), "direct desktop guest unavailable or owner authority refused: owner A is not driven by its agent"))
    # Real production candidate queue: original revision and document survive.
    +events : Chan(String) <- Chan.new(String, 8)
    +sub : B.Broker = B.Broker.document_save(withDoc, sub_event())
    +queued : B.Broker <- B.Broker.record(True{}, sub, sub_record(), events, "capture-A")
    line : Maybe<&1, String> <- Chan.recv(String, events)
    +candidate : J.Json = obj(text(line))
    check("capture queue retains private document identity", B.Broker.doc_same(J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(candidate, "payload"), "event"), "document"), J.Json.get_obj(sub_event(), "document")))
    check("capture queue retains original revision", Nat.is_eq(J.Json.get_nat(J.Json.get_obj(candidate, "payload"), "captureRevision", 0n), 7n))
    check("candidate cannot enter legacy record path", String.eq(J.Json.get_str(candidate, "event", ""), "desktop-record-candidate"))
    check("current queued candidate valid at commit", B.Broker.capture_commit_valid(queued, candidate))
    check("exact registered subframe may retire", B.Broker.retirement_valid(queued, retirement()))
    check("retirement requires no surviving capture grant", B.Broker.retirement_valid(B.Broker.policy(queued, "A", True{}, "person", False{}), retirement()))
    check("main document cannot retire as a subframe", Bool.not(B.Broker.retirement_valid(queued, D.Desktop.put(event(), "reason", J.Str{"subframe-navigation"}))))
    check("foreign nonce cannot retire current subframe", Bool.not(B.Broker.retirement_valid(queued, D.Desktop.put(retirement(), "document", D.Desktop.put(J.Json.get_obj(sub_event(), "document"), "documentNonce", J.Str{"foreign"})))))
    +retired : B.Broker = B.Broker.retirement_apply(queued, retirement())
    check("applied retirement cancels queued capture", Bool.not(B.Broker.capture_commit_valid(retired, candidate)))
    check("retired old nonce cannot record", Bool.not(B.Broker.record_valid(retired, sub_record(), sub_event())))
    check("retirement preserves main installed proof", B.Broker.record_valid(retired, record(), event()))
    check("retirement preserves target generation", Nat.is_eq(J.Json.get_nat(B.Broker.target(retired, "A"), "generation", 0n), 2n))
    check("retired identity is independent of JSON key order", B.Broker.document_retired(retired, D.Desktop.put(sub_event(), "document", obj("{\"webContentsId\":4,\"frameTreeNodeId\":6,\"frameToken\":\"sub-frame\",\"documentNonce\":\"sub-old\"}"))))
    +newEvent : J.Json = D.Desktop.put(sub_event(), "document", D.Desktop.put(J.Json.get_obj(sub_event(), "document"), "documentNonce", J.Str{"sub-new"}))
    +replacement : B.Broker = B.Broker.document_save(retired, newEvent)
    check("replacement fresh nonce is not tombstoned", Bool.not(B.Broker.document_retired(replacement, newEvent)))
    check("replacement does not revive original candidate", Bool.not(B.Broker.capture_commit_valid(replacement, candidate)))
    check("old retirement cannot delete replacement", Bool.not(B.Broker.retirement_valid(replacement, retirement())))
    +permit : Chan(J.Json) <- Chan.new(J.Json, 1)
    +released : Chan(Unit) <- Chan.new(Unit, 1)
    +done : Chan(B.Broker) <- Chan.new(B.Broker, 1)
    IO.spawn(Unit, IO.bind(B.Broker, Unit, B.Broker.capture_permit(B.Broker.capture_commit_valid(queued, candidate), queued, candidate, permit, released), b2 => finished(done, b2)))
    granted : Maybe<&1, J.Json> <- Chan.recv(J.Json, permit)
    check("commit fence returns exact original candidate", String.eq(J.Json.show(Maybe.default(&1, J.Json, granted, J.Null{})), J.Json.show(candidate)))
    sent : Result<&1, &1, Unit, Unit> <- Chan.send(Unit, released, Unit{})
    completed : Maybe<&1, B.Broker> <- Chan.recv(B.Broker, done)
    check("released commit consumes candidate once", Bool.not(B.Broker.capture_commit_valid(state_result(completed), candidate)))
    +permit2 : Chan(J.Json) <- Chan.new(J.Json, 1)
    +released2 : Chan(Unit) <- Chan.new(Unit, 1)
    denied : B.Broker <- B.Broker.capture_permit(B.Broker.capture_commit_valid(retired, candidate), retired, candidate, permit2, released2)
    refusedCapture : Maybe<&1, J.Json> <- Chan.recv(J.Json, permit2)
    check("retirement before dequeue refuses commit permit", String.eq(J.Json.show(Maybe.default(&1, J.Json, refusedCapture, J.Null{})), "null"))
    Chan.close(String, events)
    Chan.close(J.Json, permit)
    Chan.close(Unit, released)
    Chan.close(B.Broker, done)
    Chan.close(J.Json, permit2)
    Chan.close(Unit, released2)
    Chan.close(String, out)
    Chan.close(String, reply)
    Chan.close(B.Message, q)
