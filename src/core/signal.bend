import Base
import ./model.bend as M
import ./i18n.bend as I
import ./liveness.bend as L

# The complete typed input to the pure index. Timeline, clock, UI info,
# device/world state, projects and canonical sequence cannot be read by
# the calculation below. Revision exemptions must preserve this value.
type SignalInput is Data:
  SignalInput{threads: Map<&2, M.Thread>, tasks: Map<&2, M.Task>, settings: Map<&2, String>, queued: Map<&2, List<&2, M.QItem>>, asks: Map<&2, M.Ask>}

def SignalInput.of(st: M.State) -> SignalInput:
  match st:
    case M.State{_, ts, _, _, tk, sg, qu, ak, _, _, _}: SignalInput{ts, tk, sg, qu, ak}

def SignalInput.setting(st: SignalInput, +key: String) -> String:
  match st:
    case SignalInput{_, _, sg, _, _}: M.State.get.k(Map.get(String, "", sg, key))

def SignalInput.queue(st: SignalInput, +thread: String) -> List<&2, M.QItem>:
  match st:
    case SignalInput{_, _, _, qu, _}: M.Queue.items.k(Map.get(List<&2, M.QItem>, Nil{}, qu, thread))

def SignalInput.asks(st: SignalInput) -> List<&2, M.Ask>:
  match st:
    case SignalInput{_, _, _, _, asks}: Map.values(&2, M.Ask, asks)

# Visible activity, separate from persisted turn state and readiness. A result
# owed to an agent is not running work. No clock, transcript, notices, provider
# calls or cache readiness enter this projection. Bits 1/2 record actual work /
# unresolved interaction; 4 is settled only when neither bit is present, and
# 5 finished: neither bit, not settled, and its last turn completed (done,
# for the person to look at; Signal.done). A stopped or idle thread is 0.
# 6 and 7 waiting: its own turn is not running but work it started is
# out. 6: active (bit 1, Signal.bits) only through confirmed owned work or
# its children's (Signal.waits); 7: finished or idle with something still
# out that the hub waits on but none of it confirmed at work (Signal.out:
# M.Work.busy's delegated tasks, background rows, watched work), so no
# activity bit (the island, the plane's run, offline hiding and its
# parents read it as before). Both are named "waiting", never "finished".
# The index is transient: do not persist it or change the event/state codecs.

def Signal.compose(blocked: Bool, active: Bool, settled: Bool) -> Nat:
  match blocked active settled:
    case True{} True{} _: 3n
    case True{} False{} _: 2n
    case False{} True{} _: 1n
    case False{} False{} True{}: 4n
    case False{} False{} False{}: 0n

# a thread whose last turn completed and that has nothing else to show
# (no work, no ask, nothing queued, not settled) is finished, apart from a
# stopped or idle one
def Signal.done(finished: Bool, +n: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(finished, Nat.is_eq(n, 0n)), 5n, n)

def Signal.bits(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(n, 4n), n, Bool.pick(Nat, Nat.is_eq(n, 6n), 1n, 0n))

# either waiting value (6 or 7)
def Signal.waits.is(+n: Nat) -> Bool:
  Bool.or(Nat.is_eq(n, 6n), Nat.is_eq(n, 7n))

def Signal.active(+n: Nat) -> Bool:
  Nat.is_eq(Nat.mod(Signal.bits(n), 2n), 1n)

def Signal.blocked(+n: Nat) -> Bool:
  Nat.is_ge(Signal.bits(n), 2n)

def Signal.merge(+a: Nat, +b: Nat) -> Nat:
  Nat.add(Bool.pick(Nat, Bool.or(Signal.active(a), Signal.active(b)), 1n, 0n),
    Bool.pick(Nat, Bool.or(Signal.blocked(a), Signal.blocked(b)), 2n, 0n))

# the word for a value, which is also its colour everywhere (style.css's
# --st-*, the phones' PhaseColor): blocked red (an approval, a question, a
# failure: it wants the person), active blue (working), waiting violet
# (its turn is over, work it started is not: --waiting), finished green
# (done), settled and inactive grey (stopped, idle, put away)
def Signal.name(+n: Nat) -> String:
  Bool.pick(String, Signal.blocked(n), "blocked", Bool.pick(String, Signal.waits.is(n), "waiting", Bool.pick(String, Signal.active(n), "active", Bool.pick(String, Nat.is_eq(n, 4n), "settled",
    Bool.pick(String, Nat.is_eq(n, 5n), "finished", "inactive")))))

def Signal.label.loc(lc: I.Loc, +name: String) -> String:
  match lc:
    case I.Ja{}:
      Bool.pick(String, String.eq(name, "stalled"), "進捗停止・実行中の作業は継続しています", Bool.pick(String, String.eq(name, "blocked"), "対応が必要です", Bool.pick(String, String.eq(name, "active"), "作業中", Bool.pick(String, String.eq(name, "settled"), "完了済み",
        Bool.pick(String, String.eq(name, "waiting"), "サブエージェントや開始した作業の完了待ちです", Bool.pick(String, String.eq(name, "finished"), "完了しました", "非稼働・実行中の作業は未確認です"))))))
    case _:
      Bool.pick(String, String.eq(name, "stalled"), "Stalled: no recent progress; busy work continues", Bool.pick(String, String.eq(name, "blocked"), "Blocked: needs interaction", Bool.pick(String, String.eq(name, "active"), "Working: agent or owned work active", Bool.pick(String, String.eq(name, "settled"), "Settled",
        Bool.pick(String, String.eq(name, "waiting"), "Waiting: on subagents or work it started", Bool.pick(String, String.eq(name, "finished"), "Finished: ready for you to look at", "Inactive: no confirmed active work"))))))

def Signal.label(+lc: I.Loc, +n: Nat) -> String:
  Signal.label.loc(lc, Signal.name(n)) ++ Bool.pick(String, Bool.and(Signal.blocked(n), Signal.active(n)),
    Bool.pick(String, String.eq(I.I18n.tag(lc), "ja"), "（実行中の作業もあります）", "; work also active"), "")

# What a thread's State cell says (the web's top bar and work line): the
# signal's name, or how its last turn ended when that says more. A turn
# the person stopped, with nothing else going on, is "stopped" (grey, as
# inactive); one whose agent failed, with no approval or question waiting
# (pending), is "failed" (red, as blocked), not "needs you". The dot keeps
# the signal's colour (Signal.name).
def Signal.shown.k(+name: String, turn: M.TurnState, +pending: Nat) -> String:
  match turn:
    case M.Interrupted{}:
      Bool.pick(String, String.eq(name, "inactive"), "stopped", name)
    case M.Failed{}:
      Bool.pick(String, Bool.and(String.eq(name, "blocked"), Nat.is_eq(pending, 0n)), "failed", name)
    case _:
      name

def Signal.shown(+n: Nat, turn: M.TurnState, +pending: Nat) -> String:
  Signal.shown.k(Signal.name(n), turn, pending)

# the cell's sentence: a failure says why (the failure's own words, why)
def Signal.label.shown(+lc: I.Loc, +n: Nat, +shown: String, +why: String) -> String:
  Bool.pick(String, String.eq(shown, "stopped"), Bool.pick(String, String.eq(I.I18n.tag(lc), "ja"), "停止しました・実行中の作業はありません", "Stopped: nothing is running"),
    Bool.pick(String, String.eq(shown, "failed"),
      Bool.pick(String, String.eq(I.I18n.tag(lc), "ja"), "失敗しました", "Failed") ++ Bool.pick(String, String.is_empty(why), "", ": " ++ why)
        ++ Bool.pick(String, Signal.active(n), Bool.pick(String, String.eq(I.I18n.tag(lc), "ja"), "（実行中の作業もあります）", "; work also active"), ""),
      Signal.label(lc, n)))

def Signal.error(e: M.Entry, +th: String) -> String:
  match e:
    case M.Act{_, +t, +tone, _, +summary, _}:
      Bool.pick(String, Bool.and(String.eq(t, th), String.eq(tone, "error")), String.trim(summary), "")
    case M.Msg{_, _, _, _, _}:
      ""

def Signal.asked(e: M.Entry, +th: String) -> Bool:
  match e:
    case M.Msg{_, +t, M.User{}, _, _}:
      String.eq(t, th)
    case _:
      False{}

# entries newest first: the first error of the thread's last turn (n more
# to look at, 0 once found or at the person's message)
def Signal.why.go(es: List<&2, M.Entry>, +th: String, n: Nat, +found: String) -> String:
  match es n:
    case +_ 0n:
      found
    case Nil{} 1n+_:
      found
    case +e <> t 1n+k:
      Signal.why.go(t, th, Bool.pick(Nat, Bool.or(Signal.asked(e, th), Bool.not(String.is_empty(Signal.error(e, th)))), 0n, k), Signal.error(e, th))

# why the thread's last turn failed: the error it logged ("" for none
# among its newest 400 entries), at most 400 characters. Only for the
# cell's sentence: the index above never reads the timeline.
def Signal.why(+st: M.State, +th: String) -> String:
  String.take(Signal.why.go(M.State.mine(st, th), th, 400n, ""), 400n)

def Signal.why.if(failed: Bool, +st: M.State, +th: String) -> String:
  match failed:
    case True{}:
      Signal.why(st, th)
    case False{}:
      ""

# Liveness is a display overlay. It does not change the signal's work bits,
# readiness, persisted turn state or provider/process lifecycle.
def Signal.display.live(st: M.State, +id: String) -> L.Live:
  L.Liveness.read(M.Setting.get(st, L.Liveness.key(id), ""))

def Signal.display.stalled(+st: M.State, +id: String) -> Bool:
  Bool.and(Bool.not(M.Thread.hidden(M.State.thread.get(st, id))), L.Liveness.stalled(Signal.display.live(st, id)))

def Signal.display.name(n: Nat, +st: M.State, +id: String) -> String:
  Bool.pick(String, Signal.display.stalled(st, id), "stalled", Signal.name(n))

def Signal.display.shown(n: Nat, turn: M.TurnState, pending: Nat, +st: M.State, +id: String) -> String:
  Bool.pick(String, Signal.display.stalled(st, id), "stalled", Signal.shown(n, turn, pending))

def Signal.display.text(lc: I.Loc, +st: M.State, +id: String) -> String:
  Bool.pick(String, String.eq(I.I18n.tag(lc), "ja"), "進捗停止: ", "Stalled: ") ++ L.Liveness.reason(Signal.display.live(st, id))

def Signal.display.note(lc: I.Loc, +st: M.State, +id: String, fallback: String) -> String:
  Bool.pick(String, Signal.display.stalled(st, id), Signal.display.text(lc, st, id), fallback)

def Signal.display.label(+lc: I.Loc, +n: Nat, +shown: String, +st: M.State, +id: String) -> String:
  Signal.display.note(lc, st, id, Signal.label.shown(lc, n, shown, Signal.why.if(String.eq(shown, "failed"), st, id)))

def Signal.at.k(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

def Signal.at(+m: Map<&2, Nat>, +id: String) -> Nat:
  Signal.at.k(Map.get(Nat, 0n, m, id))

def Signal.asks(xs: List<&2, M.Ask>, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: m
    case M.Ask{_, +th, _, _, _, _, open, _} <> rest:
      Signal.asks(rest, Map.set(&2, Nat, m, th, Bool.pick(Nat, open, 2n, Signal.at(m, th))))

def Signal.completed(t: M.TurnState) -> Bool:
  match t:
    case M.Completed{}: True{}
    case _: False{}

def Signal.failed(t: M.TurnState) -> Bool:
  match t:
    case M.Failed{}: True{}
    case _: False{}

# Watching/cancelling confirm an owned process; launching/queued/unknown do
# not. Ended rows stay in Work.rows until delivery but are never active here.
def Signal.watch(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}: False{}
    case +l <> rest:
      Bool.or(Bool.and(Bool.not(M.Bg.ended(l)), Bool.or(String.starts_with(M.Bg.doing(l), "watching"), String.eq(M.Bg.doing(l), "cancelling"))), Signal.watch(rest))

def Signal.held(xs: List<&2, M.QItem>) -> Bool:
  match xs:
    case Nil{}: False{}
    case M.QItem{+id, _} <> rest:
      Bool.or(String.starts_with(id, "fallback:held:"), Signal.held(rest))

def Signal.shelf(s: M.Shelf) -> Bool:
  match s:
    case M.Settled{}: True{}
    case _: False{}

# what the hub still waits on for a thread, confirmed running or not
# (M.Work.busy, from the signal's own input): its delegated tasks not
# finished (kids: M.Work.kids.fold), its background rows (ended ones too:
# their outcome is still owed) and its watched work
def Signal.out(+st: SignalInput, +kids: Map<&2, Nat>, +id: String) -> Bool:
  Bool.or(Nat.is_gt(Signal.at(kids, id), 0n), Bool.or(Nat.is_gt(M.Bg.live(M.Bg.lines(SignalInput.setting(st, M.Bg.key(id)))), 0n),
    Nat.is_gt(M.Bg.live(M.Bg.lines(SignalInput.setting(st, M.Work.watch.key(id)))), 0n)))

# idle (0) or finished (5) with work out waits (7)
def Signal.out.k(out: Bool, +n: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(out, Bool.or(Nat.is_eq(n, 0n), Nat.is_eq(n, 5n))), 7n, n)

def Signal.own(+st: SignalInput, +asks: Map<&2, Nat>, +kids: Map<&2, Nat>, +t: M.Thread) -> Nat:
  match t:
    case M.Thread{+id, _, _, _, _, +turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, deleted}:
      +owned = Bool.or(Nat.is_gt(M.Bg.running(M.Bg.lines(SignalInput.setting(st, M.Bg.key(id)))), 0n), Signal.watch(M.Bg.lines(SignalInput.setting(st, M.Work.watch.key(id)))))
      +blocked = Bool.or(Signal.failed(turn), Bool.or(Nat.is_gt(pending, 0n), Bool.or(Signal.blocked(Signal.at(asks, id)),
        Bool.or(Bool.not(String.is_empty(SignalInput.setting(st, "auth.wait." ++ id))), Signal.held(SignalInput.queue(st, id))))))
      Bool.pick(Nat, deleted, 0n, Signal.out.k(Signal.out(st, kids, id), Signal.done(Bool.and(Signal.completed(turn), List.is_empty(&2, M.QItem, SignalInput.queue(st, id))), Signal.compose(blocked, Bool.or(owned, Bool.and(M.State.running.k(t), Bool.not(blocked))), Signal.shelf(M.Shelf.of(t))))))

def Signal.owns(ts: List<&2, M.Thread>, +st: SignalInput, +asks: Map<&2, Nat>, +kids: Map<&2, Nat>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ts:
    case Nil{}: m
    case +t <> rest:
      Signal.owns(rest, st, asks, kids, Map.set(&2, Nat, m, M.Thread.id(t), Signal.own(st, asks, kids, t)))

def Signal.parents.get.k(r: Map<&2, List<&2, String>> & List<&2, String>) -> List<&2, String>:
  (_, xs) = r
  xs

def Signal.parents.get(+m: Map<&2, List<&2, String>>, +id: String) -> List<&2, String>:
  Signal.parents.get.k(Map.get(List<&2, String>, Nil{}, m, id))

def Signal.has.k(r: Map<&2, Nat> & Bool) -> Bool:
  (_, yes) = r
  yes

def Signal.has(+known: Map<&2, Nat>, +id: String) -> Bool:
  Signal.has.k(Map.has(&2, Nat, known, id))

def Signal.edge(ok: Bool, +child: String, +parent: String, +m: Map<&2, List<&2, String>>) -> Map<&2, List<&2, String>>:
  match ok:
    case False{}: m
    case True{}:
      Map.set(&2, List<&2, String>, m, child, parent <> Signal.parents.get(m, child))

def Signal.task.edges(xs: List<&2, M.Task>, +known: Map<&2, Nat>, m: Map<&2, List<&2, String>>) -> Map<&2, List<&2, String>>:
  match xs:
    case Nil{}: m
    case M.Task{_, +p, +c, _, s, _, _, _, _} <> rest:
      Signal.task.edges(rest, known, Signal.edge(Bool.and(Bool.not(M.TaskState.terminal(s)), Bool.and(Signal.has(known, c), Signal.has(known, p))), c, p, m))

def Signal.launch.edges(xs: List<&2, M.Thread>, +st: SignalInput, +known: Map<&2, Nat>, m: Map<&2, List<&2, String>>) -> Map<&2, List<&2, String>>:
  match xs:
    case Nil{}: m
    case t <> rest:
      +id = M.Thread.id(t)
      +by = String.drop(SignalInput.setting(st, M.Launch.key(id)), 1n)
      Signal.launch.edges(rest, st, known, Signal.edge(Bool.and(Bool.not(String.is_empty(by)), Signal.has(known, by)), id, by, m))

type SignalStep is Data:
  SignalStep{values: Map<&2, Nat>, queue: List<&2, String>}

def Signal.changed(changed: Bool, +id: String, +value: Nat, s: SignalStep) -> SignalStep:
  match changed s:
    case False{} _: s
    case True{} SignalStep{m, q}:
      SignalStep{Map.set(&2, Nat, m, id, value), id <> q}

def Signal.spread(xs: List<&2, String>, +bits: Nat, s: SignalStep) -> SignalStep:
  match xs s:
    case Nil{} _: s
    case +id <> rest SignalStep{+m, +q}:
      +old = Signal.at(m, id)
      +value = Signal.merge(old, bits)
      Signal.spread(rest, bits, Signal.changed(Bool.not(Nat.is_eq(Signal.bits(old), value)), id, value, SignalStep{m, q}))

def Signal.step(s: SignalStep, +parents: Map<&2, List<&2, String>>) -> SignalStep:
  match s:
    case SignalStep{m, Nil{}}: SignalStep{m, Nil{}}
    case SignalStep{+m, +id <> q}:
      Signal.spread(Signal.parents.get(parents, id), Signal.bits(Signal.at(m, id)), SignalStep{m, q})

# Each node can acquire each of two bits only once. Three visits per node
# (the initial queue + two upgrades) suffice, including malformed cycles.
def Signal.drain(fuel: Nat, s: SignalStep, +parents: Map<&2, List<&2, String>>) -> SignalStep:
  match fuel s:
    case 0n _: s
    case 1n+_ SignalStep{m, Nil{}}: SignalStep{m, Nil{}}
    case 1n+k SignalStep{m, id <> q}:
      Signal.drain(k, Signal.step(SignalStep{m, id <> q}, parents), parents)

def Signal.values(s: SignalStep) -> Map<&2, Nat>:
  match s:
    case SignalStep{m, _}: m

def Signal.pending(s: SignalStep) -> Nat:
  match s:
    case SignalStep{_, q}: List.length(&2, String, q)

def Signal.ids(xs: List<&2, M.Thread>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case t <> rest: M.Thread.id(t) <> Signal.ids(rest)

def Signal.index.owns(+st: SignalInput, +ts: List<&2, M.Thread>, tasks: List<&2, M.Task>, +known: Map<&2, Nat>) -> Map<&2, Nat>:
  Signal.values(Signal.drain(Nat.mul(3n, List.length(&2, M.Thread, ts)), SignalStep{known, Signal.ids(ts)},
    Signal.launch.edges(ts, st, known, Signal.task.edges(tasks, known, Map.new(&2, List<&2, String>)))))

# active (1, not blocked) with its own turn not running: waiting (6) on
# the work it started or its children's
def Signal.waits(+n: Nat, running: Bool) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_eq(n, 1n), Bool.not(running)), 6n, n)

def Signal.wait.all(ts: List<&2, M.Thread>, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ts:
    case Nil{}: m
    case +t <> rest:
      Signal.wait.all(rest, Map.set(&2, Nat, m, M.Thread.id(t), Signal.waits(Signal.at(m, M.Thread.id(t)), M.State.running.k(t))))

def Signal.index.k(+st: SignalInput, +ts: List<&2, M.Thread>, +tasks: List<&2, M.Task>) -> Map<&2, Nat>:
  Signal.wait.all(ts, Signal.index.owns(st, ts, tasks, Signal.owns(ts, st, Signal.asks(SignalInput.asks(st), Map.new(&2, Nat)), M.Work.kids.fold(tasks, Map.new(&2, Nat)), Map.new(&2, Nat))))

def Signal.index.inputs(+st: SignalInput) -> Map<&2, Nat>:
  match st:
    case SignalInput{ts, tk, _, _, _}:
      Signal.index.k(st, Map.values(&2, M.Thread, ts), Map.values(&2, M.Task, tk))

def Signal.index(st: M.State) -> Map<&2, Nat>:
  Signal.index.inputs(SignalInput.of(st))

def Signal.of(+st: M.State, t: M.Thread) -> Nat:
  Signal.at(Signal.index(st), M.Thread.id(t))

# Reserved values only in a frame's decorated Ui.info and SIx.work.
def Signal.key(+id: String) -> String:
  "@signal:" ++ id

def Signal.visible(online: Bool, +n: Nat) -> Nat:
  match online:
    case True{}: n
    case False{}: Bool.pick(Nat, Signal.active(n), Bool.pick(Nat, Signal.blocked(n), 2n, 0n), n)

def Signal.cache.go(xs: List<&2, String>, +online: Bool, +signals: Map<&2, Nat>, info: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: info
    case +id <> rest:
      Signal.cache.go(rest, online, signals, Map.set(&2, String, info, Signal.key(id), Nat.show(Signal.visible(online, Signal.at(signals, id)))))

# Values already accompany keys in the map traversal. Keep cache.go above as
# the independent lookup-based reference used by compatibility checks.
def Signal.cache.pairs(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +online: Bool, info: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: info
    case (id, n) <> rest:
      Signal.cache.pairs(rest, online, Map.set(&2, String, info, Signal.key(id), Nat.show(Signal.visible(online, n))))

def Signal.cache(+online: Bool, signals: Map<&2, Nat>, info: Map<&2, String>) -> Map<&2, String>:
  Signal.cache.pairs(Map.to_list(&2, Nat, signals), online, Map.set(&2, String, info, "@signal.online", Bool.pick(String, online, "1", "0")))

# The cached map contains only canonical @signal:* strings and its exact
# online visibility flag. Overlay current recipient info underneath it:
# canonical keys win, while unknown reserved keys and all ordinary values
# remain fresh. The independent cache() above is the full per-frame oracle.
def Signal.cache.present(r: Map<&2, String> & Bool) -> Bool:
  (_, found) = r
  found

def Signal.cache.overlay.put(known: Bool, key: String, value: String, acc: Map<&2, String>) -> Map<&2, String>:
  match known:
    case True{}: acc
    case False{}: Map.set(&2, String, acc, key, value)

def Signal.cache.overlay.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, +canonical: Map<&2, String>, acc: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: acc
    case (+key, value) <> rest:
      Signal.cache.overlay.go(rest, canonical, Signal.cache.overlay.put(Signal.cache.present(Map.has(&2, String, canonical, key)), key, value, acc))

def Signal.cache.overlay(+canonical: Map<&2, String>, current: Map<&2, String>) -> Map<&2, String>:
  Signal.cache.overlay.go(Map.to_list(&2, String, current), canonical, canonical)

def Signal.cached.k(r: Map<&2, String> & String) -> Nat:
  (_, v) = r
  M.Switch.num(v, 0n)

def Signal.cached(+info: Map<&2, String>, +id: String) -> Nat:
  Signal.cached.k(Map.get(String, "0", info, Signal.key(id)))

# whether an open thread waits on work it started, for what its page says
# (the last reply's "Waiting since", stamp.bend): its signal says so when
# the client has it, else its turn is over with work the hub still waits on
def Signal.waiting(+info: Map<&2, String>, +st: M.State, +id: String, running: Bool) -> Bool:
  Bool.or(Signal.waits.is(Signal.cached(info, id)), Bool.and(Bool.not(running), Nat.is_gt(M.Work.busy(st, id), 0n)))

# The server may pass raw bits directly to the display index. Fall back to
# the original info parser if its input was already decorated or has an
# unknown reserved entry, so no legacy value disappears.
def Signal.info.raw.go(xs: List<&2, String>, clean: Bool) -> Bool:
  match xs:
    case Nil{}: clean
    case k <> rest: Signal.info.raw.go(rest, Bool.and(clean, Bool.not(String.starts_with(k, "@signal:"))))

def Signal.info.raw(xs: List<&2, String>) -> Bool:
  Signal.info.raw.go(xs, True{})

def Signal.work.given(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +online: Bool, work: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: work
    case (id, n) <> rest:
      Signal.work.given(rest, online, Map.set(&2, Nat, work, Signal.key(id), Signal.visible(online, n)))

def Signal.cache.put(yes: Bool, +k: String, +info: Map<&2, String>, work: Map<&2, Nat>) -> Map<&2, Nat>:
  match yes:
    case False{}: work
    case True{}:
      Map.set(&2, Nat, work, k, Signal.cached(info, String.drop(k, 8n)))

def Signal.cache.keys(xs: List<&2, String>, +info: Map<&2, String>, work: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: work
    case +k <> rest:
      Signal.cache.keys(rest, info, Signal.cache.put(String.starts_with(k, "@signal:"), k, info, work))

def Signal.cache.work(+info: Map<&2, String>, work: Map<&2, Nat>) -> Map<&2, Nat>:
  Signal.cache.keys(Map.keys(&2, String, info), info, work)

# LiveActivity counts top-level work, not provider process existence. Its
# legacy calculator accepts Running threads, so adapt only its temporary
# input list. The canonical state, events and notification policy stay raw.
def Signal.island.go(ts: List<&2, M.Thread>, +info: Map<&2, String>, +acc: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: List.reverse(&2, M.Thread, acc)
    case +t <> rest:
      Signal.island.go(rest, info, Bool.pick(List<&2, M.Thread>, Signal.active(Signal.cached(info, M.Thread.id(t))),
        M.Thread.turned(M.Running{}, 0n, t) <> acc, acc))

def Signal.island.threads(ts: List<&2, M.Thread>, info: Map<&2, String>) -> List<&2, M.Thread>:
  Signal.island.go(ts, info, Nil{})

# Only these recorded history changes leave every Signal.index input alone.
# New/unknown events invalidate by default; user messages touch the shelf.
# The server also uses this revision for its explicit metadata-only sidebar,
# with exact clock/UI/info/owner fences. Timeline and commands stay current.
def SignalRevision.affects(c: M.Change) -> Bool:
  match c:
    case M.ActivityLogged{_, _, _, _, _, _}: False{}
    case M.MessagePosted{_, _, M.Assistant{}, _, _}: False{}
    case M.MessagePosted{_, _, M.System{}, _, _}: False{}
    case _: True{}

def SignalRevision.changed.go(cs: List<&2, M.Change>, changed: Bool) -> Bool:
  match cs:
    case Nil{}: changed
    case c <> rest: SignalRevision.changed.go(rest, Bool.or(changed, SignalRevision.affects(c)))

def SignalRevision.advance(cs: List<&2, M.Change>, old: Nat, head: Nat) -> Nat:
  Bool.pick(Nat, SignalRevision.changed.go(cs, False{}), head, old)
