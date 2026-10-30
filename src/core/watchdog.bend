import Base
import ./model.bend as M
import ./hub.bend as B
import ./bothub.bend as BH
import ./bot.bend as BT
import ./json.bend as J
import ./liveness.bend as L
import ./restartretry.bend as RR

# Persisted progress and notice decisions share the hub's atomic step. A
# synthetic nudge, lease renewal or watchdog tick is never progress itself.
def Watchdog.live(+s: M.State, +th: String) -> L.Live:
  L.Liveness.read(M.Setting.get(s, L.Liveness.key(th), ""))

def Watchdog.active.t(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}: Bool.not(Bool.or(ar, de))

# A quota hold without a reset needs attention; known reset times already
# have an automatic continuation and are allowed to wait for that time.
def Watchdog.provider.wait(+s: M.State, +th: String) -> Bool:
  +record = M.Setting.get(s, "provider.resume." ++ th, "")
  Bool.and(Bool.not(String.is_empty(record)), Nat.is_eq(J.Json.get_nat(Maybe.default(&2, J.Json, J.Json.parse(record), J.Null{}), "reset", 0n), 0n))

def Watchdog.active.ix(+kids: Map<&2, Nat>, +s: M.State, +t: M.Thread) -> Bool:
  Bool.and(Watchdog.active.t(t), Bool.or(M.State.running.k(t), Bool.or(M.Queue.starting(s, M.Thread.id(t)),
    Bool.or(Nat.is_gt(M.Work.busy.at(kids, s, M.Thread.id(t)), 0n), Bool.or(Nat.is_gt(B.Hub.pending(t), 0n),
      Bool.or(Watchdog.provider.wait(s, M.Thread.id(t)), Bool.not(List.is_empty(&2, String, M.Wake.ids(s, "wakes.by." ++ M.Thread.id(t))))))))))

def Watchdog.active(+s: M.State, +t: M.Thread) -> Bool:
  Watchdog.active.ix(M.Work.kids.map(s), s, t)

def Watchdog.progress.k(yes: Bool, +s: M.State, +now: Nat, +th: String, +reason: String) -> B.Step:
  match yes:
    case False{}: B.Step.none()
    case True{}:
      B.Step.cat(B.Step{List.append(&2, M.Change, [M.SettingSet{L.Liveness.key(th), L.Liveness.write(L.Liveness.progress(Watchdog.live(s, th), now, String.take(reason, 240n))), now}],
        Bool.pick(List<&2, M.Change>, Bool.or(Bool.not(String.is_empty(M.Setting.get(s, "restart.retry." ++ th, ""))), Nat.is_gt(M.Switch.num(M.Setting.get(s, "restart.retry.n." ++ th, "0"), 0n), 0n)),
          [M.SettingSet{"restart.retry." ++ th, "", now}, M.SettingSet{"restart.retry.n." ++ th, "0", now}], Nil{})), Nil{}}, B.Locks.heartbeat(s, now, th))

def Watchdog.progress.one(+s: M.State, +now: Nat, +th: String, reason: String) -> B.Step:
  Watchdog.progress.k(Bool.or(Nat.is_gt(now, L.Liveness.last(Watchdog.live(s, th))), Nat.is_gt(L.Liveness.nudged(Watchdog.live(s, th)), 0n)), s, now, th, reason)

def Watchdog.task.parent(m: Maybe<&2, M.Task>) -> String:
  match m:
    case Some{M.Task{_, parent, _, _, state, _, _, _, _}}: Bool.pick(String, M.TaskState.terminal(state), "", parent)
    case None{}: ""

def Watchdog.parent(+s: M.State, +th: String) -> String:
  +by = M.Launch.by(s, th)
  Bool.pick(String, String.is_empty(by), Watchdog.task.parent(B.Task.of_child(s, th)), by)

def Watchdog.ancestors(fuel: Nat, +s: M.State, +now: Nat, +th: String, +reason: String, step: B.Step) -> B.Step:
  match fuel:
    case 0n: step
    case 1n+n:
      Watchdog.ancestors(n, s, now, Watchdog.parent(s, th), reason,
        B.Step.cat(step, Watchdog.progress.k(Bool.and(Bool.not(String.is_empty(th)), Watchdog.active(s, B.Hub.thread(s, th))), s, now, th, reason)))

def Watchdog.progress(+s: M.State, +now: Nat, +th: String, +reason: String) -> B.Step:
  Watchdog.ancestors(4n, s, now, Watchdog.parent(s, th), "owned thread " ++ th ++ ": " ++ reason, Watchdog.progress.one(s, now, th, reason))

def Watchdog.observed(rows: List<&2, J.Json>, +s: M.State, +now: Nat, step: B.Step) -> B.Step:
  match rows:
    case Nil{}: step
    case +row <> rest:
      Watchdog.observed(rest, s, now, B.Step.cat(step, Watchdog.progress(s, J.Json.get_nat(row, "at", now), J.Json.get_str(row, "thread", ""), J.Json.get_str(row, "reason", "file or work output"))))

def Watchdog.source(c: M.Change) -> String:
  match c:
    case M.MessagePosted{th, +msg, M.User{}, +text, _}: Bool.pick(String, Bool.or(B.Lr.is(msg), Bool.or(String.starts_with(text, "[Backplane]"), B.Carry.is(text))), "", th)
    case M.AskOpened{_, th, _, _, _, _, _}: th
    case M.WakeSet{_, by, _, _, _, _, _, _}: by
    case M.ActivityLogged{th, _, +tone, +kind, _, _}: Bool.pick(String, Bool.and(Bool.not(Bool.or(String.eq(kind, "restart"), String.eq(kind, "watchdog"))), Bool.or(String.eq(kind, "provider-wait"), Bool.or(String.eq(tone, "tool"), Bool.or(String.eq(tone, "error"), Bool.or(String.eq(tone, "progress"), String.eq(tone, "say")))))), th, "")
    case M.MessagePosted{th, _, M.Assistant{}, _, _}: th
    case _: ""

def Watchdog.reason(c: M.Change) -> String:
  match c:
    case M.ActivityLogged{_, _, _, kind, text, _}: kind ++ ": " ++ String.take(text, 180n)
    case M.AskOpened{_, _, kind, _, detail, _, _}: "waiting for user (" ++ kind ++ "): " ++ detail
    case M.WakeSet{_, _, kind, target, _, _, _, _}: "waiting for " ++ kind ++ " " ++ target
    case _: "agent output"

def Watchdog.owner(+th: String) -> String:
  Bool.pick(String, String.starts_with(th, "@sub:"), String.drop(th, 5n), th)

def Watchdog.change(yes: Bool, +s: M.State, +now: Nat, +c: M.Change) -> B.Step:
  match yes:
    case False{}: B.Step.none()
    case True{}: Watchdog.progress(s, now, Watchdog.owner(Watchdog.source(c)), Watchdog.reason(c))

# Streamed deltas and provider task progress can bypass persisted messages.
# Sample their real output at five-second precision, never a ping or quota
# polling line. An overdue episode is cleared on its first real output.
def Watchdog.block.content(+block: J.Json) -> Bool:
  Bool.or(Bool.not(String.is_empty(J.Json.get_str(block, "text", ""))), Bool.or(Bool.not(String.is_empty(J.Json.get_str(block, "content", ""))),
    Bool.or(Bool.not(List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(block, "content")))), Bool.not(String.is_empty(J.Json.get_str(block, "name", ""))))))

def Watchdog.blocks(xs: List<&2, J.Json>, +user: Bool) -> Bool:
  match xs:
    case Nil{}: False{}
    case +block <> rest:
      Bool.or(Bool.and(Bool.pick(Bool, user, String.eq(J.Json.get_str(block, "type", ""), "tool_result"), True{}), Watchdog.block.content(block)), Watchdog.blocks(rest, user))

def Watchdog.output.kind(kind: String, +o: J.Json) -> Bool:
  match kind:
    case "assistant": Watchdog.blocks(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), False{})
    case "user": Watchdog.blocks(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), True{})
    case "stream_event":
      +delta = J.Json.get_obj(J.Json.get_obj(o, "event"), "delta")
      Bool.or(Bool.not(String.is_empty(J.Json.get_str(delta, "text", ""))), Bool.or(Bool.not(String.is_empty(J.Json.get_str(delta, "thinking", ""))), Bool.not(String.is_empty(J.Json.get_str(delta, "partial_json", "")))))
    case "item.delta": Bool.not(String.is_empty(J.Json.get_str(o, "text", "")))
    case "item.started": Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(o, "item"), "command", "")))
    case "item.completed":
      +item = J.Json.get_obj(o, "item")
      Bool.or(Bool.not(String.is_empty(J.Json.get_str(item, "text", ""))), Bool.or(Bool.not(String.is_empty(J.Json.get_str(item, "aggregated_output", ""))), Bool.not(String.is_empty(J.Json.get_str(item, "command", "")))))
    case "item.updated": Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(o, "item"), "text", "")))
    case _: False{}

def Watchdog.output(+o: J.Json) -> Bool:
  Watchdog.output.kind(J.Json.get_str(o, "type", ""), o)

def Watchdog.task.row(rows: List<&2, String>, +id: String) -> String:
  match rows:
    case Nil{}: ""
    case +row <> rest: Bool.pick(String, String.eq(M.Bg.field(row, 0n), id), M.Bg.field(row, 1n), Watchdog.task.row(rest, id))

def Watchdog.line.k(yes: Bool, +s: M.State, +now: Nat, +th: String) -> B.Step:
  match yes:
    case False{}: B.Step.none()
    case True{}: Watchdog.progress(s, now, th, "agent tool or stream output")

def Watchdog.line(+s: M.State, +now: Nat, +th: String, +o: J.Json) -> B.Step:
  +task = Bool.and(B.Prog.is(o), Bool.not(String.eq(J.Json.get_str(o, "description", ""), Watchdog.task.row(M.Bg.of(s, th), J.Json.get_str(o, "task_id", "")))))
  Watchdog.line.k(Bool.and(Bool.or(Watchdog.output(o), task), Bool.or(Nat.is_ge(now, Nat.add(L.Liveness.last(Watchdog.live(s, th)), 5n)),
    Nat.is_gt(L.Liveness.nudged(Watchdog.live(s, th)), 0n))), s, now, th)

def Watchdog.after.go(cs: List<&2, M.Change>, +s: M.State, +now: Nat, step: B.Step) -> B.Step:
  match cs:
    case Nil{}: step
    case +c <> rest:
      Watchdog.after.go(rest, s, now, B.Step.cat(step, Watchdog.change(Bool.not(String.is_empty(Watchdog.source(c))), s, now, c)))

def Watchdog.synthetic(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}: False{}
    case M.SettingSet{+key, _, _} <> rest: Bool.or(String.starts_with(key, "liveness."), Watchdog.synthetic(rest))
    case _ <> rest: Watchdog.synthetic(rest)

def Watchdog.after.k(synthetic: Bool, +s: M.State, +now: Nat, step: B.Step) -> B.Step:
  match synthetic step:
    case True{} _: step
    case False{} B.Step{+cs, os}: B.Step.cat(B.Step{cs, os}, Watchdog.after.go(cs, s, now, B.Step.none()))

def Watchdog.after(+s: M.State, +now: Nat, +step: B.Step) -> B.Step:
  match step:
    case B.Step{+cs, os}: Watchdog.after.k(Watchdog.synthetic(cs), s, now, B.Step{cs, os})

def Watchdog.bub(b: Maybe<&2, BT.Bot>, +s: M.State, +now: Nat, +th: String, +text: String) -> B.Step:
  match b:
    case None{}: B.Step.none()
    case Some{bot}: BH.Bots.wake(s, now, "", bot, "watchdog:" ++ th ++ ":" ++ Nat.show(L.Liveness.last(Watchdog.live(s, th))), text, 0n)

def Watchdog.launcher(+s: M.State, +th: String) -> String:
  +by = M.Launch.by(s, th)
  Bool.pick(String, String.is_empty(by), M.Setting.get(s, "thread.parent." ++ th, ""), by)

def Watchdog.report(none: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> B.Step:
  match none:
    case True{}: Watchdog.bub(BT.World.find(M.State.bots(s), "Bub"), s, now, th, text)
    case False{}: B.Lr.send(s, now, "watchdog:" ++ th, Watchdog.launcher(s, th), text)

def Watchdog.notify(yes: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> B.Step:
  match yes:
    case False{}: B.Step.none()
    case True{}:
      Watchdog.report(String.is_empty(Watchdog.launcher(s, th)), s, now, th, text)

def Watchdog.nudge(yes: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> B.Step:
  match yes:
    case False{}: B.Step.none()
    case True{}: B.Lr.send(s, now, "watchdog:" ++ th, th, text)

def Watchdog.decided(+d: L.Decision, +s: M.State, +now: Nat, +th: String) -> B.Step:
  +next = L.Liveness.write(L.Decision.live(d))
  +text = "Stalled thread " ++ th ++ ": no progress since " ++ Nat.show(L.Liveness.last(L.Decision.live(d))) ++ " at " ++ L.Liveness.reason(L.Decision.live(d)) ++ ". Busy work is still running."
  B.Step.cat(B.Step{Bool.pick(List<&2, M.Change>, String.eq(next, M.Setting.get(s, L.Liveness.key(th), "")), Nil{}, [M.SettingSet{L.Liveness.key(th), next, now}]), Nil{}},
    B.Step.cat(Watchdog.nudge(L.Decision.nudge(d), s, now, th, L.Liveness.nudge.text(L.Liveness.reason(L.Decision.live(d)))),
      Watchdog.notify(L.Decision.notify(d), s, now, th, text)))

def Watchdog.ask.reason(xs: List<&2, M.Ask>, +th: String) -> String:
  match xs:
    case Nil{}: "waiting for user"
    case M.Ask{_, +id, kind, _, detail, _, +open, _} <> rest:
      Bool.pick(String, Bool.and(String.eq(id, th), open), "waiting for user (" ++ kind ++ "): " ++ String.take(detail, 180n), Watchdog.ask.reason(rest, th))

def Watchdog.wait.reason(ids: List<&2, String>, +s: M.State) -> String:
  match ids:
    case Nil{}: "turn or owned work"
    case id <> _:
      +r = M.Wake.get(s, id)
      "waiting for " ++ J.Json.get_str(r, "kind", "dependency") ++ " " ++ J.Json.get_str(r, "target", "")

def Watchdog.initial.reason(asking: Bool, +s: M.State, +th: String) -> String:
  match asking:
    case True{}: Watchdog.ask.reason(M.Asks.all(s), th)
    case False{}: Watchdog.wait.reason(M.Wake.ids(s, "wakes.by." ++ th), s)

def Watchdog.one(initial: Bool, active: Bool, +s: M.State, +now: Nat, +t: M.Thread) -> B.Step:
  match initial:
    case True{}: Watchdog.progress(s, now, M.Thread.id(t), Watchdog.initial.reason(Nat.is_gt(B.Hub.pending(t), 0n), s, M.Thread.id(t)))
    case False{}:
      Watchdog.decided(L.Liveness.tick(active, now, Nat.mul(L.Liveness.number(M.Setting.get(s, "watchdog.minutes", "20"), 20n), 60n),
        L.Liveness.number(M.Setting.get(s, "watchdog.grace.seconds", "60"), 60n), Watchdog.live(s, M.Thread.id(t))), s, now, M.Thread.id(t))

def Watchdog.keep(known: Bool, +active: Bool, +s: M.State, +now: Nat, +t: M.Thread) -> B.Step:
  match known:
    case False{}: B.Step.none()
    case True{}: Watchdog.one(Bool.and(active, Nat.is_eq(L.Liveness.last(Watchdog.live(s, M.Thread.id(t))), 0n)), active, s, now, t)

def Watchdog.threads(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat, step: B.Step) -> B.Step:
  match ts:
    case Nil{}: step
    case +t <> rest:
      +active = Watchdog.active.ix(kids, s, t)
      +one = Watchdog.keep(Bool.or(active, Nat.is_gt(L.Liveness.last(Watchdog.live(s, M.Thread.id(t))), 0n)), active, s, now, t)
      Watchdog.threads(rest, kids, M.State.replay(B.Queue.changes(one), s), now, B.Step.cat(step, one))

def Watchdog.all(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Map.values(&2, M.Thread, ts)

def Watchdog.tick.after(+s: M.State, +now: Nat, +step: B.Step) -> B.Step:
  +s2 = M.State.replay(B.Queue.changes(step), s)
  +timeouts = B.WorkTimeout.tick(s2, now)
  +s3 = M.State.replay(B.Queue.changes(timeouts), s2)
  +providers = B.ProviderResume.tick(s3, now)
  B.Step.cat(step, B.Step.cat(timeouts, B.Step.cat(providers, RR.Retry.tick(M.State.replay(B.Queue.changes(providers), s3), now))))

def Watchdog.tick(+s: M.State, +now: Nat) -> B.Step:
  Watchdog.tick.after(s, now, Watchdog.threads(Watchdog.all(s), M.Work.kids.map(s), s, now, B.Step.none()))

def Watchdog.plan.one(active: Bool, +s: M.State, +t: M.Thread, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match active:
    case False{}: rest
    case True{}:
      J.Json.obj([J.KV{"thread", J.Str{M.Thread.id(t)}}, J.KV{"cwd", J.Str{B.Hub.cwd(s, t)}},
        J.KV{"since", J.Json.nat(Nat.mul(L.Liveness.last(Watchdog.live(s, M.Thread.id(t))), 1000n))},
        J.KV{"work", J.Str{M.Bg.show(M.Work.watch(s, M.Thread.id(t)))}}]) <> rest

def Watchdog.plan.go(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +s: M.State) -> List<&2, J.Json>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest: Watchdog.plan.one(Watchdog.active.ix(kids, s, t), s, t, Watchdog.plan.go(rest, kids, s))

def Watchdog.plan(+s: M.State) -> String:
  J.Json.show(J.Json.arr(Watchdog.plan.go(Watchdog.all(s), M.Work.kids.map(s), s)))
