import Base
import ./check.bend as C
import ./telemetry_cases.bend as F
import ../src/core/telemetry-choice.bend as P
import ../src/core/telemetry.bend as T
import ../src/core/client.bend as U
import ../src/core/onboard.bend as O
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/json.bend as J
import ../src/server/telemetry.bend as TI
import ../src/mobile/screen.bend as MOB
import ../src/core/i18n.bend as I
import ../src/core/diskq.bend as DQ
import ../src/core/telemetry-usage.bend as TUS

def Choice.check(name: String, b: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(b), "True")

def Choice.state(share: String, asked: String, usage: String) -> M.State:
  M.State.replay([M.SettingSet{"telemetry.share", share, 1n}, M.SettingSet{"telemetry.asked", asked, 1n}, M.SettingSet{"telemetry.usage.asked", usage, 1n}], M.State.empty())

def Choice.ui(s: M.State) -> U.Ui:
  U.Ui.scratch_set(U.Ui.info_set(U.Ui.adopt.raw(U.Ui.init(1n), M.State.apply(s, M.ProjectCreated{"p", "Fixture", "/private", 1n})), "telemetry.live", "1"), "@sync", "1")

def Choice.fresh() -> U.Ui:
  Choice.ui(M.State.empty())

def Choice.changes(step: B.Step) -> List<&2, M.Change>:
  match step:
    case B.Step{cs, _}: cs

# one client request; `decision` is the click's own id (a retry repeats it)
def Choice.decide.d(+st: M.State, value: String, expected: String, popup: Bool, decision: String) -> M.State:
  M.State.replay(Choice.changes(B.Hub.request(st, 2n, 1, J.Json.obj([J.KV{"id", J.Num{"1"}}, J.KV{"m", J.Str{"telemetry.choice"}}, J.KV{"p", J.Json.obj([J.KV{"value", J.Str{value}}, J.KV{"expected", J.Str{expected}}, J.KV{"popup", J.Flag{popup}}, J.KV{"decision", J.Str{decision}}])}]))), st)

# a distinct human click for every distinct (value, expected) request
def Choice.decide(+st: M.State, +value: String, +expected: String, +popup: Bool) -> M.State:
  Choice.decide.d(st, value, expected, popup, "d:" ++ value ++ ":" ++ expected ++ ":" ++ Bool.show(popup))

def Choice.on() -> M.State:
  Choice.decide(M.State.empty(), "on", T.Tm.choice.token(M.State.empty()), True{})

def Choice.off() -> M.State:
  Choice.decide(Choice.on(), "off", T.Tm.choice.token(Choice.on()), False{})

def Choice.plan(go: Bool, st: T.TmState) -> T.TmState:
  F.Cases.plan(T.Tm.runtime.plan(go, st, F.Cases.view(True{}, True{}, "0", "0.12.0", "2026-10-14", 900000n), F.Cases.id2(), ""))

def Choice.params(value: String, expected: String, popup: Bool) -> J.Json:
  J.Json.obj([J.KV{"value", J.Str{value}}, J.KV{"expected", J.Str{expected}}, J.KV{"popup", J.Flag{popup}}])

def Choice.step(s: M.State, value: String, expected: String, popup: Bool) -> B.Step:
  B.Hub.telemetry.choice(s, 3n, 1, "1", Choice.params(value, expected, popup))

def Choice.reply(os: List<&2, B.Out>) -> J.Json:
  match os:
    case Nil{}: J.Null{}
    case B.SavedSend{_, text} <> _: Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    case _ <> rest: Choice.reply(rest)

def Choice.ack(step: B.Step) -> J.Json:
  match step:
    case B.Step{_, os}: Choice.reply(os)

def Choice.sent(cs: List<&2, U.Cmd>) -> J.Json:
  match cs:
    case Nil{}: J.Null{}
    case U.Send{text} <> _: Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    case _ <> rest: Choice.sent(rest)

def Choice.request(a: U.Act) -> J.Json:
  match a:
    case U.Act{_, cs}: Choice.sent(cs)

def Choice.legacy(+s: M.State, key: String, value: String) -> M.State:
  M.State.replay(Choice.changes(B.Hub.request(s, 4n, 1, J.Json.obj([J.KV{"id", J.Num{"1"}}, J.KV{"m", J.Str{"setting.set"}}, J.KV{"p", J.Json.obj([J.KV{"key", J.Str{key}}, J.KV{"value", J.Str{value}}])}]))), s)

def Choice.partial(raw: String) -> U.Ui:
  U.Ui.scratch_set(Choice.fresh(), "@tour", raw)

def Choice.resume(raw: String, action: String, value: String) -> Nat:
  U.Tour.at(F.Cases.ui(U.Ui.act(Choice.partial(raw), action, value)))

def Choice.cfg() -> List<&2, TI.TmCfg>:
  TI.Tio.plat.cfg(("linux", "x64"), True{}, False{}, "", "", "", 0n, "release")

def Choice.proposal() -> U.Act:
  U.Ui.act(Choice.fresh(), "ob-next", "")

def Choice.accepted() -> B.Step:
  B.Hub.request(M.State.empty(), 3n, 1, Choice.request(Choice.proposal()))

def Choice.failed() -> U.Ui:
  U.Ui.recv.k(3n, F.Cases.ui(Choice.proposal()), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"1"}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"Unable to save preferences"}}]))

# 1. One worker lifecycle as the hub runs it (Hub.telemetry → done → again):
# a start, its completion, and a restart only when the snapshot is stale.
# Counts the jobs started within the bound.
def Choice.jobs(fuel: Nat, +st: M.State, +cfg: List<&2, TI.TmCfg>, run: Bool) -> Nat:
  match fuel run:
    case 0n _: 0n
    case 1n+k False{}: 0n
    case 1n+k True{}:
      +view = TI.Tio.view(st, "0.12.0", 5n, cfg)
      +after = TI.Tio.done(TI.Tio.busy(cfg, 5n), 5n, "")
      Nat.add(1n, Choice.jobs(k, st, after, Bool.and(Bool.not(T.Tm.current(view, st)), T.Tm.start(TI.Tio.can(after, 5n), False{}, T.Tm.allowed(st)))))

def Choice.lifecycle(+st: M.State) -> Nat:
  Choice.jobs(5n, st, Choice.cfg(), T.Tm.start(TI.Tio.can(Choice.cfg(), 5n), False{}, T.Tm.allowed(st)))

def Choice.identity(+st: M.State) -> Bool:
  T.Tm.current(TI.Tio.view(st, "0.12.0", 5n, Choice.cfg()), st)

# 2. A choice whose append failed waits in the real disk queue, folded into
# the hub's state but not on disk (server Hub.commit.held).
def Choice.queued(+before: M.State, +cs: List<&2, M.Change>) -> DQ.Dq:
  DQ.Dq.push(DQ.Dq.empty(), before, M.State.seq(before), ["{}"], cs, Nil{}, Nil{})

def Choice.on.changes() -> List<&2, M.Change>:
  Choice.changes(Choice.step(M.State.empty(), "on", T.Tm.choice.token(M.State.empty()), True{}))

def Choice.off.changes() -> List<&2, M.Change>:
  Choice.changes(Choice.step(Choice.on(), "off", T.Tm.choice.token(Choice.on()), False{}))

# the registration permit (Hub.telemetry.permit) and the aggregate one
# (Hub.telemetry.usage_permit) with the real queue
def Choice.permit(+q: DQ.Dq, +post: M.State) -> Bool:
  T.Tm.permit(True{}, DQ.Dq.some(q), T.Tm.allowed(post), T.Tm.current(TI.Tio.view(post, "0.12.0", 5n, Choice.cfg()), post))

def Choice.usage.permit(+q: DQ.Dq, +post: M.State) -> Bool:
  Bool.and(Choice.permit(q, post), TUS.Usage.current(M.Setting.get(post, "telemetry.usage.generation", ""), post))

# 3. Two people's Off from the same displayed On, with a re-enable between
def Choice.a() -> M.State:
  Choice.decide.d(Choice.on(), "off", T.Tm.choice.token(Choice.on()), False{}, "c1-A")

def Choice.c() -> M.State:
  Choice.decide.d(Choice.a(), "on", T.Tm.choice.token(Choice.a()), False{}, "c3-C")

def Choice.b() -> M.State:
  Choice.decide.d(Choice.c(), "off", T.Tm.choice.token(Choice.on()), False{}, "c2-B")

def Choice.retry() -> M.State:
  Choice.decide.d(Choice.c(), "off", T.Tm.choice.token(Choice.on()), False{}, "c1-A")

# 4. Settings with two outstanding choices; replies by request id
def Choice.settled(+st: M.State) -> U.Ui:
  Choice.ui(M.State.apply(st, M.SettingSet{"onboard.done", "1", 1n}))

def Choice.rid(a: U.Act) -> String:
  J.Json.as_raw(J.Json.get(Choice.request(a), "id"), "")

def Choice.refuse(u: U.Ui, rid: String) -> U.Ui:
  U.Ui.recv.k(3n, u, J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{rid}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"Unable to save preferences"}}]))

def Choice.first() -> U.Act:
  U.Ui.act(Choice.settled(Choice.off()), "share", "on")

def Choice.second() -> U.Act:
  U.Ui.act(F.Cases.ui(Choice.first()), "share", "off")

def Choice.both(first_last: Bool) -> U.Ui:
  Bool.pick(U.Ui, first_last,
    Choice.refuse(Choice.refuse(F.Cases.ui(Choice.second()), Choice.rid(Choice.second())), Choice.rid(Choice.first())),
    Choice.refuse(Choice.refuse(F.Cases.ui(Choice.second()), Choice.rid(Choice.first())), Choice.rid(Choice.second())))

# another client's On at the next revision is adopted between the refusals
def Choice.adopted() -> M.State:
  Choice.decide(Choice.off(), "on", T.Tm.choice.token(Choice.off()), False{})

def Choice.interleaved() -> U.Ui:
  Choice.refuse(U.Ui.adopt.raw(Choice.refuse(F.Cases.ui(Choice.second()), Choice.rid(Choice.first())), M.State.apply(Choice.adopted(), M.SettingSet{"onboard.done", "1", 1n})), Choice.rid(Choice.second()))

def Choice.shown(u: U.Ui) -> String:
  M.Setting.get(U.Ui.state(u), "telemetry.share", "")

# 5. The tour around a usage step that is answered or dismissed
def Choice.at(+st: M.State, raw: String, dismissed: Bool) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(Choice.ui(st), "@tour", raw), "@telemetry.dismiss", Bool.pick(String, dismissed, "1", ""))

def Choice.move(u: U.Ui, action: String) -> U.Ui:
  F.Cases.ui(U.Ui.act(u, action, ""))

def main() -> IO(Unit):
  do IO<Unit>:
    Choice.check("new install default selected On", U.Share.on(M.State.empty()))
    Choice.check("default is not a recorded choice", Bool.not(T.Tm.allowed(M.State.empty())))
    C.Check.str("new first-run shows telemetry before tour", Nat.show(U.Tour.at(Choice.fresh())), Nat.show(O.Ob.share()))
    C.Check.str("popup default is explained in Continue", U.Tour.forward(Choice.fresh()), "Continue with telemetry on")
    C.Check.str("Continue records whole choice", F.Cases.setting(U.Ui.act(Choice.fresh(), "ob-next", ""), "telemetry.usage.asked"), "1")
    C.Check.str("Continue resumes welcome without finishing tour", Nat.show(U.Tour.at(F.Cases.ui(U.Ui.act(Choice.fresh(), "ob-next", "")))), "0")
    C.Check.str("explicit Off records whole choice", F.Cases.setting(U.Ui.act(Choice.fresh(), "share", "off"), "telemetry.share"), "off")
    C.Check.str("dismiss records no choice", F.Cases.setting(U.Ui.act(Choice.fresh(), "ob-skip", ""), "telemetry.asked"), "")
    Choice.check("dismissed remains unable to send", Bool.not(T.Tm.allowed(U.Ui.state(F.Cases.ui(U.Ui.act(Choice.fresh(), "ob-skip", ""))))))
    Choice.check("dismiss defers presentation in this connection", Bool.not(U.Share.popup(F.Cases.ui(U.Ui.act(Choice.fresh(), "ob-skip", "")))))
    Choice.check("next connection still has pending choice", U.Share.popup(Choice.fresh()))
    Choice.check("legacy explicit off without answer stays answered", U.Share.asked(Choice.state("off", "", "")))
    Choice.check("legacy opt-out never sends", Bool.not(T.Tm.allowed(Choice.state("off", "", ""))))
    Choice.check("expanded decline remains Off", Bool.not(U.Share.on(Choice.state("on", "1", "0"))))
    Choice.check("legacy On waits for complete disclosure", Bool.not(T.Tm.allowed(Choice.state("on", "1", ""))))
    Choice.check("complete previous On choice preserved", T.Tm.allowed(Choice.state("on", "1", "1")))
    Choice.check("upgrade does not reopen complete answer", Bool.not(U.Share.popup(Choice.ui(Choice.on()))))
    Choice.check("same-home reinstall preserves opt-out", Bool.not(U.Share.popup(Choice.ui(Choice.off()))))
    Choice.check("other client adopts one shared answer", Bool.not(U.Share.popup(U.Ui.adopt.raw(Choice.fresh(), Choice.off()))))
    Choice.check("atomic hub choice allows all telemetry", T.Tm.allowed(Choice.on()))
    Choice.check("off wins over a stale popup On", Bool.not(T.Tm.allowed(Choice.decide(Choice.off(), "on", T.Tm.choice.token(M.State.empty()), True{}))))
    Choice.check("explicit off still applies from stale popup", Bool.not(T.Tm.allowed(Choice.decide(Choice.on(), "off", T.Tm.choice.token(M.State.empty()), True{}))))
    Choice.check("invalid choice never enables", Bool.not(T.Tm.allowed(Choice.decide(M.State.empty(), "yes", T.Tm.choice.token(M.State.empty()), True{}))))
    Choice.check("Settings may deliberately re-enable after off", T.Tm.allowed(Choice.decide(Choice.off(), "on", T.Tm.choice.token(Choice.off()), False{})))
    Choice.check("unanswered runtime after days never releases queue", List.is_empty(&2, T.TmEv, T.TmState.queue(Choice.plan(False{}, F.Cases.unasked()))))
    Choice.check("disabled runtime drops all old identified categories", List.is_empty(&2, T.TmEv, T.TmState.queue(Choice.plan(False{}, F.Cases.st([F.Cases.ev("install", "0.12.0", True{}), F.Cases.ev("update", "0.12.0", True{}), F.Cases.ev("active", "0.12.0", True{})])))))
    Choice.check("anonymous old queue never revived", List.is_empty(&2, T.TmEv, T.Tm.runtime.queue([F.Cases.ev("install", "0.12.0", False{}), F.Cases.ev("update", "0.12.0", False{})])))
    Choice.check("pending runtime view cannot authorize worker", Bool.not(T.TmView.asked(TI.Tio.view(Choice.state("on", "1", ""), "0.12.0", 2n, Choice.cfg()))))
    Choice.check("mobile uses same disclosure and controls", String.contains(J.Json.show(MOB.Mob.tour(I.En{}, Choice.fresh())), "Continue with telemetry on"))
    Choice.check("disclosure includes all transmission metadata", String.contains(O.Ob.body.share(), "architecture") && String.contains(O.Ob.body.share(), "anonymous install") && String.contains(O.Ob.body.share(), "investor"))
    Choice.check("real configured accepted-On positive control", T.TmView.asked(TI.Tio.view(Choice.on(), "0.12.0", 2n, Choice.cfg())))
    Choice.check("real configured Off closes runtime gate", Bool.not(T.TmView.asked(TI.Tio.view(Choice.off(), "0.12.0", 2n, Choice.cfg()))))
    C.Check.str("auto partial tour resumes after default answer", Nat.show(Choice.resume("3", "ob-next", "")), "3")
    C.Check.str("manual partial tour resumes after opt-out", Nat.show(Choice.resume("m3", "share", "off")), "3")
    C.Check.str("manual marker survives injected answer", U.Ui.scratch(F.Cases.ui(U.Ui.act(Choice.partial("m3"), "ob-next", "")), "@tour"), "m3")
    C.Check.str("raw usage cursor dismiss moves away", Nat.show(Choice.resume("s9", "ob-skip", "")), Nat.show(O.Ob.uber()))
    C.Check.str("manual usage cursor dismiss moves away", Nat.show(Choice.resume("ms9", "ob-skip", "")), Nat.show(O.Ob.uber()))
    C.Check.str("other client answer removes raw usage card", Nat.show(U.Tour.at(U.Ui.adopt.raw(Choice.partial("s9"), Choice.off()))), Nat.show(O.Ob.uber()))
    C.Check.str("other client answer removes manual usage card", Nat.show(U.Tour.at(U.Ui.adopt.raw(Choice.partial("ms9"), Choice.off()))), Nat.show(O.Ob.uber()))
    C.Check.str("other client answer preserves interrupted manual tour", Nat.show(U.Tour.at(U.Ui.adopt.raw(Choice.partial("m3"), Choice.off()))), "3")
    Choice.check("provider setup pauses telemetry presentation", Bool.not(U.Tour.on(U.Ui.flag_on(U.Ui.flag_on(Choice.fresh(), "settings"), "provider-setup"))))
    Choice.check("real reconnect reoffers unanswered dismissal", U.Share.popup(U.Ui.scratch_set(U.Ui.online(F.Cases.ui(U.Ui.act(Choice.fresh(), "ob-skip", "")), True{}), "@sync", "1")))
    Choice.check("current-state synchronization required", Bool.not(U.Share.popup(U.Ui.online(Choice.fresh(), True{}))))
    Choice.check("unbaked capability absent needs no telemetry prompt", Bool.not(U.Share.pending(U.Ui.info_set(Choice.fresh(), "telemetry.live", ""))))
    Choice.check("explicit disabled capability does not prompt", Bool.not(U.Share.pending(U.Ui.info_set(Choice.fresh(), "telemetry.live", "0"))))
    C.Check.str("proposal uses atomic RPC", J.Json.get_str(Choice.request(Choice.proposal()), "m", ""), "telemetry.choice")
    C.Check.str("proposal binds displayed state", J.Json.get_str(J.Json.get_obj(Choice.request(Choice.proposal()), "p"), "expected", ""), T.Tm.choice.token(M.State.empty()))
    Choice.check("proposal alone cannot authorize canonical hub", Bool.not(T.Tm.allowed(M.State.empty())))
    Choice.check("accepted append records canonical permission", T.Tm.allowed(M.State.replay(Choice.changes(Choice.accepted()), M.State.empty())))
    Choice.check("failed append restores unrecorded UI", Bool.not(U.Share.asked(U.Ui.state(Choice.failed()))))
    Choice.check("failed append restores pending popup", U.Share.popup(Choice.failed()))
    C.Check.str("authoritative rejection corrects optimistic On", M.Setting.get(U.Ui.state(U.Ui.recv.k(3n, F.Cases.ui(Choice.proposal()), Choice.ack(Choice.step(Choice.off(), "on", T.Tm.choice.token(M.State.empty()), True{})))), "telemetry.share", ""), "off")
    Choice.check("authoritative rejection shows error", Bool.not(J.Json.get_bool(Choice.ack(Choice.step(Choice.off(), "on", T.Tm.choice.token(M.State.empty()), True{})), "ok", True{})))
    Choice.check("legacy share On cannot bypass saved Off", Bool.not(T.Tm.allowed(Choice.legacy(Choice.off(), "telemetry.share", "on"))))
    Choice.check("legacy asked cannot enable pending hub", Bool.not(T.Tm.allowed(Choice.legacy(Choice.state("on", "", "1"), "telemetry.asked", "1"))))
    Choice.check("legacy expanded marker cannot bypass pending popup", Bool.not(T.Tm.allowed(Choice.legacy(Choice.state("on", "1", ""), "telemetry.usage.asked", "1"))))
    Choice.check("legacy Off remains supported", Bool.not(T.Tm.allowed(Choice.legacy(Choice.on(), "telemetry.share", "off"))))
    C.Check.str("legacy revision writes rejected", M.Setting.get(Choice.legacy(Choice.off(), "telemetry.choice.revision", "0"), "telemetry.choice.revision", ""), M.Setting.get(Choice.off(), "telemetry.choice.revision", ""))
    Choice.check("lost ACK replay does not create second decision", Nat.is_eq(T.Tm.choice.revision(Choice.decide(Choice.on(), "on", T.Tm.choice.token(M.State.empty()), True{})), T.Tm.choice.revision(Choice.on())))
    Choice.check("fresh repeated Off advances revision", Nat.is_gt(T.Tm.choice.revision(Choice.decide(Choice.off(), "off", T.Tm.choice.token(Choice.off()), False{})), T.Tm.choice.revision(Choice.off())))
    Choice.check("repeated Off invalidates queued Settings On without collection", Bool.not(T.Tm.allowed(Choice.decide(Choice.decide(Choice.off(), "off", T.Tm.choice.token(Choice.off()), False{}), "on", T.Tm.choice.token(Choice.off()), False{}))))
    Choice.check("ABA Off-On-Off invalidates old Settings On", Bool.not(T.Tm.allowed(Choice.decide(Choice.decide(Choice.decide(Choice.off(), "on", T.Tm.choice.token(Choice.off()), False{}), "off", T.Tm.choice.token(Choice.off()), False{}), "on", T.Tm.choice.token(Choice.off()), False{}))))
    Choice.check("two simultaneous default answers record once", Nat.is_eq(T.Tm.choice.revision(Choice.decide.d(Choice.on(), "on", T.Tm.choice.token(M.State.empty()), True{}, "second-client")), 1n))
    # 1. current-snapshot identity uses the view's own projection
    Choice.check("identity: configured fresh pending is current", Choice.identity(M.State.empty()))
    Choice.check("identity: accepted On is current", Choice.identity(Choice.on()))
    Choice.check("identity: accepted Off is current", Choice.identity(Choice.off()))
    Choice.check("identity: legacy incomplete On is current", Choice.identity(Choice.state("on", "1", "")))
    Choice.check("identity: legacy expanded decline is current", Choice.identity(Choice.state("on", "1", "0")))
    Choice.check("identity: a changed choice is not current", Bool.not(T.Tm.current(TI.Tio.view(Choice.on(), "0.12.0", 5n, Choice.cfg()), Choice.off())))
    C.Check.str("lifecycle: unchanged Off completion starts no second job", Nat.show(Choice.lifecycle(Choice.off())), "1")
    C.Check.str("lifecycle: unchanged On completion starts no second job", Nat.show(Choice.lifecycle(Choice.on())), "1")
    C.Check.str("lifecycle: pending completion starts no second job", Nat.show(Choice.lifecycle(M.State.empty())), "1")
    C.Check.str("lifecycle: legacy incomplete On starts no second job", Nat.show(Choice.lifecycle(Choice.state("on", "1", ""))), "1")
    C.Check.str("lifecycle: legacy expanded decline starts no second job", Nat.show(Choice.lifecycle(Choice.state("on", "1", "0"))), "1")
    # 2. an unpersisted On authorizes nothing until its append is flushed
    Choice.check("queued On: folded state reads On", T.Tm.allowed(M.State.replay(Choice.on.changes(), M.State.empty())))
    Choice.check("queued On: registration send refused", Bool.not(Choice.permit(Choice.queued(M.State.empty(), Choice.on.changes()), M.State.replay(Choice.on.changes(), M.State.empty()))))
    Choice.check("queued On: aggregate send refused", Bool.not(Choice.usage.permit(Choice.queued(M.State.empty(), Choice.on.changes()), M.State.replay(Choice.on.changes(), M.State.empty()))))
    Choice.check("queued On: no worker starts", Bool.not(T.Tm.start(True{}, DQ.Dq.some(Choice.queued(M.State.empty(), Choice.on.changes())), T.Tm.allowed(M.State.replay(Choice.on.changes(), M.State.empty())))))
    Choice.check("flushed On: registration send permitted", Choice.permit(DQ.Dq.empty(), Choice.on()))
    Choice.check("flushed On: aggregate send permitted", Choice.usage.permit(DQ.Dq.empty(), Choice.on()))
    Choice.check("queued Off: revokes at once", Bool.not(Choice.permit(Choice.queued(Choice.on(), Choice.off.changes()), M.State.replay(Choice.off.changes(), Choice.on()))))
    Choice.check("queued Off: cleanup worker still starts", T.Tm.start(True{}, DQ.Dq.some(Choice.queued(Choice.on(), Choice.off.changes())), T.Tm.allowed(M.State.replay(Choice.off.changes(), Choice.on()))))
    # 3. receipts name one decision
    Choice.check("independent same-token Off after a re-enable applies", Bool.not(T.Tm.allowed(Choice.b())))
    Choice.check("independent Off is acknowledged as a new decision", Nat.is_gt(T.Tm.choice.revision(Choice.b()), T.Tm.choice.revision(Choice.c())))
    Choice.check("lost-ACK retry does not revert the later On", T.Tm.allowed(Choice.retry()))
    Choice.check("lost-ACK retry records nothing", Nat.is_eq(T.Tm.choice.revision(Choice.retry()), T.Tm.choice.revision(Choice.c())))
    Choice.check("lost-ACK retry is acknowledged", J.Json.get_bool(Choice.ack(B.Hub.telemetry.choice(Choice.c(), 3n, 1, "1", J.Json.obj([J.KV{"value", J.Str{"off"}}, J.KV{"expected", J.Str{T.Tm.choice.token(Choice.on())}}, J.KV{"popup", J.Flag{False{}}}, J.KV{"decision", J.Str{"c1-A"}}]))), "ok", False{}))
    Choice.check("Off without a decision id always applies", Bool.not(T.Tm.allowed(Choice.decide.d(Choice.c(), "off", T.Tm.choice.token(Choice.on()), False{}, ""))))
    Choice.check("proposal carries a decision id", Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(Choice.request(Choice.proposal()), "p"), "decision", ""))))
    Choice.check("two clicks carry different decision ids", Bool.not(String.eq(J.Json.get_str(J.Json.get_obj(Choice.request(Choice.first()), "p"), "decision", ""), J.Json.get_str(J.Json.get_obj(Choice.request(Choice.second()), "p"), "decision", ""))))
    # 4. two outstanding Settings choices, both refused
    C.Check.str("two refusals, first then second: canonical Off shown", Choice.shown(Choice.both(False{})), "off")
    C.Check.str("two refusals, second then first: canonical Off shown", Choice.shown(Choice.both(True{})), "off")
    Choice.check("two refusals: nothing outstanding", String.eq(U.Ui.scratch(Choice.both(False{}), "@telemetry.out"), "0"))
    Choice.check("one refusal of two leaves one outstanding", String.eq(U.Ui.scratch(Choice.refuse(F.Cases.ui(Choice.second()), Choice.rid(Choice.first())), "@telemetry.out"), "1"))
    C.Check.str("canonical adoption between refusals is kept", Choice.shown(Choice.interleaved()), "on")
    # 5. Back and Next pass the suppressed usage step in their direction
    C.Check.str("answered: Back from uberbot reaches step 8", Nat.show(U.Tour.at(Choice.move(Choice.at(Choice.off(), "10", False{}), "ob-back"))), "8")
    C.Check.str("answered manual: Back from uberbot reaches step 8", Nat.show(U.Tour.at(Choice.move(Choice.at(Choice.off(), "m10", False{}), "ob-back"))), "8")
    C.Check.str("answered manual: Back keeps the manual marker", U.Ui.scratch(Choice.move(Choice.at(Choice.off(), "m10", False{}), "ob-back"), "@tour"), "m8")
    C.Check.str("dismissed: Back from uberbot reaches step 8", Nat.show(U.Tour.at(Choice.move(Choice.at(M.State.empty(), "10", True{}), "ob-back"))), "8")
    C.Check.str("dismissed raw s9 normalized: Back reaches step 8", Nat.show(U.Tour.at(Choice.move(Choice.at(M.State.empty(), "s9", True{}), "ob-back"))), "8")
    C.Check.str("answered manual ms9 normalized: Back reaches step 8", Nat.show(U.Tour.at(Choice.move(Choice.at(Choice.off(), "ms9", False{}), "ob-back"))), "8")
    C.Check.str("answered: Next from step 8 reaches uberbot", Nat.show(U.Tour.at(Choice.move(Choice.at(Choice.off(), "8", False{}), "ob-next"))), Nat.show(O.Ob.uber()))
    C.Check.str("dismissed: Next from step 8 reaches uberbot", Nat.show(U.Tour.at(Choice.move(Choice.at(M.State.empty(), "8", True{}), "ob-next"))), Nat.show(O.Ob.uber()))
    C.Check.str("usage step not suppressed: Back from uberbot shows it", Nat.show(U.Tour.at(Choice.move(U.Ui.info_set(Choice.at(M.State.empty(), "10", False{}), "telemetry.live", "0"), "ob-back"))), Nat.show(O.Ob.share()))
    C.Check.str("interrupted manual tour still resumes after a choice", Nat.show(Choice.resume("m3", "share", "off")), "3")
