import Base
import ./model.bend as M
import ./json.bend as J
import ./locks.bend as LK
import ./mail.bend as ML
import ./bot.bend as BT

# Live hub policy. No IO, wall clock reads, transcript mutation or agent
# decisions: the caller supplies the state snapshot and time. Historical
# Hub.settle/Shelve helpers remain fixtures for the previous policy's laws.
# All production automatic settlement goes through Auto.changes.

def Auto.allow(turn: M.TurnState, work: Bool, ask: Bool, unread: Bool, delivered: Bool, quiet: Bool, shelf: M.Shelf, override: M.Override) -> Bool:
  match turn work ask unread delivered quiet shelf override:
    case M.Completed{} False{} False{} False{} True{} True{} M.Active{} M.NoOverride{}: True{}
    case _ _ _ _ _ _ _ _: False{}

def Auto.num(s: M.State, +key: String, fallback: Nat) -> Nat:
  Maybe.default(&2, Nat, Nat.read(M.Setting.get(s, key, "")), fallback)

def Auto.seconds.bounded(+minutes: Nat) -> Nat:
  Nat.mul(Bool.pick(Nat, Nat.is_gt(minutes, 525600n), 525600n, minutes), 60n)

def Auto.seconds(s: M.State, +key: String, fallback: Nat) -> Nat:
  Auto.seconds.bounded(Auto.num(s, key, fallback))

def Auto.get(m: Map<&2, Nat>, +id: String) -> Nat:
  M.Work.kids.at.k(Map.get(Nat, 0n, m, id))

def Auto.put(yes: Bool, +m: Map<&2, Nat>, +id: String, +at: Nat) -> Map<&2, Nat>:
  match yes:
    case False{}: m
    case True{}: Map.set(&2, Nat, m, id, M.Time.max(at, Auto.get(m, id)))

# One transient index of tasks, lock owners/waiters, asks and delivered
# mail, shared by every candidate in the pass (never per-thread scans).
type Index is Data:
  Index{workers: Map<&2, Nat>, blocked: Map<&2, Nat>, reports: Map<&2, Nat>, owned: Map<&2, Nat>}

def Auto.tasks(xs: List<&2, M.Task>, workers: Map<&2, Nat>, blocked: Map<&2, Nat>) -> Map<&2, Nat> & Map<&2, Nat>:
  match xs:
    case Nil{}: (workers, blocked)
    case M.Task{_, +p, +c, _, +state, _, +delivered, _, _} <> rest:
      Auto.tasks(rest, Auto.put(True{}, workers, c, 1n),
        Auto.put(Bool.not(Bool.and(M.TaskState.terminal(state), delivered)), Auto.put(Bool.not(Bool.and(M.TaskState.terminal(state), delivered)), blocked, c, 1n), p, 1n))

def Auto.parties(xs: List<&2, LK.LockParty>, blocked: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: blocked
    case p <> rest: Auto.parties(rest, Auto.put(True{}, blocked, LK.Lock.thread(p), 1n))

def Auto.locks(names: List<&2, String>, +s: M.State, blocked: Map<&2, Nat>) -> Map<&2, Nat>:
  match names:
    case Nil{}: blocked
    case +name <> rest:
      Auto.locks(rest, s, Auto.parties(List.append(&2, LK.LockParty, LK.Lock.holder.list(LK.Lock.holder(M.Locks.get(s, name))), LK.Lock.waiters(M.Locks.get(s, name))), blocked))

def Auto.asks(xs: List<&2, M.Ask>, blocked: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: blocked
    case M.Ask{_, th, _, _, _, _, open, _} <> rest: Auto.asks(rest, Auto.put(open, blocked, th, 1n))

def Auto.mail(xs: List<&2, ML.Mail>, reports: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: reports
    case +m <> rest:
      Auto.mail(rest, Auto.put(String.eq(ML.Mail.kind(m), "launch"), reports, M.Link.from(ML.Mail.id(m)) ++ "\t" ++ ML.Mail.to(m), ML.Mail.at(m)))

def Auto.mails(xs: List<&2, List<&2, ML.Mail>>, reports: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: reports
    case ms <> rest: Auto.mails(rest, Auto.mail(ms, reports))

def Auto.addr.bot(b: Maybe<&2, BT.Bot>, +th: String) -> String:
  match b:
    case None{}: ML.Addr.of(th)
    case Some{bot}: "bot:" ++ BT.Bot.id(bot)

def Auto.addr(+s: M.State, +th: String) -> String:
  Auto.addr.bot(BT.World.of_thread(M.State.bots(s), th), th)

# This is the candidate's own resident history, including outgoing links,
# not the global log. It is visited once, after the cheap shelf/turn gates.
type Summary is Data:
  Summary{quiet: Nat, input: Bool, personal: Bool, output: String, output_at: Nat, report: Nat}

def Auto.direct(+id: String, +text: String) -> Bool:
  Bool.and(String.is_empty(M.Link.from(id)), Bool.or(Bool.not(String.starts_with(text, "[")), String.starts_with(text, "[message from person@")))

def Auto.user(role: M.Role) -> Bool:
  match role:
    case M.User{}: True{}
    case _: False{}

def Auto.assistant(role: M.Role) -> Bool:
  match role:
    case M.Assistant{}: True{}
    case _: False{}

def Auto.summary(xs: List<&2, M.Entry>, +th: String, +by: String, a: Summary) -> Summary:
  match xs a:
    case Nil{} result: result
    case M.Msg{+id, +t, +role, +text, +at} <> rest Summary{q, +found, person, +output, oa, report}:
      +own = String.eq(t, th)
      +user = Bool.and(own, Auto.user(role))
      Auto.summary(rest, th, by, Summary{M.Time.max(q, at), Bool.or(found, user),
        Bool.pick(Bool, Bool.and(user, Bool.not(found)), Auto.direct(id, text), person),
        Bool.pick(String, Bool.and(own, Bool.and(Auto.assistant(role), String.is_empty(output))), id, output),
        M.Time.max(oa, Bool.pick(Nat, Bool.and(own, Auto.assistant(role)), at, 0n)),
        M.Time.max(report, Bool.pick(Nat, Bool.and(String.eq(M.Link.from(id), th), String.eq(t, by)), at, 0n))})
    case M.Act{_, _, _, _, _, at} <> rest Summary{q, f, p, o, oa, r}:
      Auto.summary(rest, th, by, Summary{M.Time.max(q, at), f, p, o, oa, r})

def Auto.activity(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, at, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: at

def Auto.worker(+s: M.State, +th: String, workers: Map<&2, Nat>) -> Bool:
  Bool.or(Bool.or(Bool.not(String.is_empty(M.Launch.by(s, th))), Bool.not(String.is_empty(M.Setting.get(s, "thread.parent." ++ th, "")))), Bool.or(Nat.is_gt(Auto.get(workers, th), 0n),
    Bool.or(String.starts_with(th, "run"), String.starts_with(th, "mailcoord-"))))

def Auto.output(+s: M.State, +th: String, fallback: String) -> String:
  M.Setting.get(s, "autosettle.output." ++ th, fallback)

def Auto.unread(+s: M.State, +th: String, +output: String, personal: Bool, worker: Bool) -> Bool:
  Bool.and(Bool.or(personal, Bool.not(worker)), Bool.and(Bool.not(String.is_empty(output)), Bool.not(String.eq(output, M.Setting.get(s, "autosettle.read." ++ th, "")))))

def Auto.report.address(+addr: String, +s: M.State, +th: String, +report: Nat, +after: Nat, reports: Map<&2, Nat>) -> Bool:
  Nat.is_ge(M.Time.max(report, M.Time.max(Auto.get(reports, th ++ "\t" ++ addr), Auto.num(s, "autosettle.delivery." ++ th ++ "\t" ++ addr, 0n))), after)

def Auto.report.received(received: Bool, +s: M.State, +th: String, by: String, +report: Nat, +after: Nat, reports: Map<&2, Nat>) -> Bool:
  match received:
    case True{}: True{}
    case False{}: Auto.report.address(Auto.addr(s, by), s, th, report, after, reports)

def Auto.report.required(required: Bool, +s: M.State, +th: String, +by: String, +report: Nat, +after: Nat, reports: Map<&2, Nat>) -> Bool:
  match required:
    case False{}: True{}
    case True{}:
      Auto.report.received(Bool.and(Bool.not(String.is_empty(M.Setting.get(s, "autosettle.report." ++ th, ""))),
        Nat.is_ge(Auto.num(s, "autosettle.report." ++ th, 0n), after)), s, th, by, report, after, reports)

def Auto.report(+s: M.State, +th: String, +by: String, +report: Nat, +output_at: Nat, +activity: Nat, reports: Map<&2, Nat>) -> Bool:
  Auto.report.required(Bool.and(Bool.not(String.is_empty(by)), M.Launch.reports(s, th)), s, th, by, report, M.Time.max(activity, output_at), reports)

def Auto.window(+s: M.State, worker: Bool, read: Bool) -> Nat:
  Bool.pick(Nat, worker, Auto.seconds(s, "settle.idle.minutes", 15n),
    Bool.pick(Nat, Bool.and(read, M.Lifecycle.on(s)), Auto.seconds(s, "settle.idle.minutes", 15n), Auto.seconds(s, "settle.personal.minutes", 240n)))

def Auto.due.summary(+s: M.State, +t: M.Thread, +now: Nat, ix: Index, a: Summary) -> Bool:
  match ix a:
    case Index{+workers, _, reports, owned} Summary{+q, +found, +personal, recent, +oa, report}:
      +th = M.Thread.id(t)
      +worker = Auto.worker(s, th, workers)
      +output = Auto.output(s, th, recent)
      +audience = M.Setting.get(s, "autosettle.audience." ++ th, "")
      +person = Bool.or(personal, Bool.or(String.eq(audience, "person"), Bool.and(Bool.not(found), String.is_empty(audience))))
      +unread = Auto.unread(s, th, output, person, worker)
      +window = Auto.window(s, worker, Bool.not(unread))
      Auto.allow(M.Thread.turn(t), Nat.is_gt(Auto.get(owned, th), 0n),
        Nat.is_gt(M.Lifecycle.pending(t), 0n), unread,
        Auto.report(s, th, M.Launch.by(s, th), report, oa, Auto.activity(t), reports),
        Bool.and(Nat.is_gt(window, 0n), M.Settle.stale(now, M.Time.max(M.Time.max(q, Auto.activity(t)),
          M.Time.max(Auto.num(s, "autosettle.quiet." ++ th, 0n), M.Nav.until(M.Nav.get(M.State.navs(s), th)))), window)),
        M.Shelf.of(t), M.Lifecycle.override(t))

def Auto.due.gated(turn: M.TurnState, shelf: M.Shelf, override: M.Override, +s: M.State, +t: M.Thread, +now: Nat, ix: Index) -> Bool:
  match turn shelf override:
    case M.Completed{} M.Active{} M.NoOverride{}:
      Auto.due.summary(s, t, now, ix, Auto.summary(M.State.mine(s, M.Thread.id(t)), M.Thread.id(t), M.Launch.by(s, M.Thread.id(t)), Summary{0n, False{}, False{}, "", 0n, 0n}))
    case _ _ _: False{}

def Auto.due(+s: M.State, +t: M.Thread, +now: Nat, ix: Index) -> Bool:
  Auto.due.gated(M.Thread.turn(t), M.Shelf.of(t), M.Lifecycle.override(t), s, t, now, ix)

def Auto.emit(yes: Bool, th: String, at: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match yes:
    case False{}: rest
    case True{}: M.ThreadSettled{th, False{}, at} <> rest

def Auto.go.acc(ts: List<&2, M.Thread>, +s: M.State, +now: Nat, +ix: Index, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match ts:
    case Nil{}: List.reverse(&2, M.Change, acc)
    case +t <> rest:
      Auto.go.acc(rest, s, now, ix, Auto.emit(Auto.due(s, t, now, ix), M.Thread.id(t), now, acc))

def Auto.go(ts: List<&2, M.Thread>, +s: M.State, +now: Nat, +ix: Index) -> List<&2, M.Change>:
  Auto.go.acc(ts, s, now, ix, Nil{})

def Auto.read.summary(summary: Summary, +s: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  match summary:
    case Summary{_, _, _, recent, _, _}:
      +output = Auto.output(s, th, recent)
      Bool.pick(List<&2, M.Change>, Bool.and(Bool.not(String.is_empty(output)), Bool.not(String.eq(output, M.Setting.get(s, "autosettle.read." ++ th, "")))),
        [M.SettingSet{"autosettle.read." ++ th, output, now}], Nil{})

# Completion of owned work starts a fresh grace period too. These small
# durable watermarks are added at the ordinary commit boundary, not on a
# timer. A repeated identical work observation does not postpone settling.
def Auto.read(+s: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  Auto.read.summary(Auto.summary(M.State.mine(s, th), th, "", Summary{0n, False{}, False{}, "", 0n, 0n}), s, th, now)

def Auto.quiet.ask(xs: List<&2, M.Ask>, +id: String, +at: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: m
    case M.Ask{+a, th, _, _, _, _, _, _} <> rest:
      Auto.quiet.ask(rest, id, at, Auto.put(String.eq(a, id), m, th, at))

def Auto.quiet.parties(xs: List<&2, LK.LockParty>, +at: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: m
    case p <> rest: Auto.quiet.parties(rest, at, Auto.put(True{}, m, LK.Lock.thread(p), at))

def Auto.quiet.lock(+l: LK.NamedLock, +at: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  Auto.quiet.parties(List.append(&2, LK.LockParty, LK.Lock.holder.list(LK.Lock.holder(l)), LK.Lock.waiters(l)), at, m)

def Auto.old.reports.needed(needed: Bool, s: M.State) -> Map<&2, Nat>:
  match needed:
    case False{}: Map.new(&2, Nat)
    case True{}: Auto.mails(Map.values(&2, List<&2, ML.Mail>, ML.Box.mails(M.State.mail(s))), Map.new(&2, Nat))

def Auto.old.reports(+s: M.State) -> Map<&2, Nat>:
  Auto.old.reports.needed(Bool.not(String.eq(M.Setting.get(s, "autosettle.version", ""), "1")), s)

def Auto.receipt.relevant(sh: M.Shelf) -> Bool:
  match sh:
    case M.Active{}: True{}
    case M.Pinned{}: True{}
    case M.Snoozed{}: True{}
    case _: False{}

def Auto.receipt.keep(keep: Bool, +key: String, at: Nat, +now: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match keep:
    case False{}: acc
    case True{}: M.SettingSet{"autosettle.delivery." ++ key, Nat.show(at), now} <> acc

def Auto.old.receipts.acc(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +now: Nat, +s: M.State, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}: List.reverse(&2, M.Change, acc)
    case (+key, at) <> rest:
      Auto.old.receipts.acc(rest, now, s, Auto.receipt.keep(Auto.receipt.relevant(M.Shelf.of(M.State.thread.get(s, M.Bg.field(key, 0n)))), key, at, now, acc))

def Auto.old.receipts(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +now: Nat, +s: M.State) -> List<&2, M.Change>:
  Auto.old.receipts.acc(xs, now, s, Nil{})

def Auto.upgrade.index(+ix: Index, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match ix s:
    case Index{_, _, reports, _} M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      List.append(&2, M.Change, Auto.go(Map.values(&2, M.Thread, ts), s, now, ix),
        List.append(&2, M.Change, Auto.old.receipts(Map.to_list(&2, Nat, reports), now, s), [M.SettingSet{"autosettle.version", "1", now}]))

# Match before entering the archive walk. Bool.pick is eager in Bend.
# No mail traversal or upgrade planning runs on an ordinary version-1 tick.
def Auto.secrets(keys: List<&2, String>, +s: M.State, blocked: Map<&2, Nat>) -> Map<&2, Nat>:
  match keys:
    case Nil{}: blocked
    case key <> rest:
      Auto.secrets(rest, s, Auto.put(True{}, blocked,
        J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, key, "")), J.Null{}), "thread", ""), 1n))

def Auto.task.parents(xs: List<&2, M.Task>, m: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: m
    case M.Task{_, p, c, _, _, _, _, _, _} <> rest: Auto.task.parents(rest, Map.set(&2, String, m, c, p))

def Auto.parent.got(pair: Map<&2, String> & String) -> String:
  (_, p) = pair
  p

def Auto.parent(+s: M.State, +parents: Map<&2, String>, +th: String) -> String:
  +by = M.Launch.by(s, th)
  Bool.pick(String, String.is_empty(by), Auto.parent.got(Map.get(String, "", parents, th)), by)

# Only blocked roots and their ownership chains are traversed. This is a
# settlement index, not a second full sidebar status calculation. Fuel
# bounds malformed/cyclic ownership; an already marked ancestor stops it.
def Auto.ancestor.next(more: Bool, +s: M.State, +parents: Map<&2, String>, +th: String) -> String:
  match more:
    case False{}: ""
    case True{}: Auto.parent(s, parents, th)

def Auto.ancestor(+s: M.State, +parents: Map<&2, String>, fuel: Nat, +th: String, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match fuel th:
    case 0n _: m
    case _ SNil{}: m
    case 1n+k SCon{c, rest}:
      +id : String = SCon{c, rest}
      +fresh = Nat.is_eq(Auto.get(m, id), 0n)
      Auto.ancestor(s, parents, k, Auto.ancestor.next(fresh, s, parents, id), Auto.put(fresh, m, id, 1n))

def Auto.ancestors(xs: List<&2, String>, +s: M.State, +parents: Map<&2, String>, +fuel: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: m
    case th <> rest: Auto.ancestors(rest, s, parents, fuel, Auto.ancestor(s, parents, fuel, Auto.parent(s, parents, th), m))

def Auto.running(ts: List<&2, M.Thread>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ts:
    case Nil{}: m
    case M.Thread{id, _, _, _, _, +turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> rest:
      Auto.running(rest, Auto.put(M.Thread.busy(turn, pending), m, id, 1n))

def Auto.queued(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.QItem>>>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: m
    case (th, qs) <> rest: Auto.queued(rest, Auto.put(Bool.not(List.is_empty(&2, M.QItem, qs)), m, th, 1n))

def Auto.wakes(keys: List<&2, String>, +s: M.State, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match keys:
    case Nil{}: m
    case +key <> rest: Auto.wakes(rest, s, Auto.put(Bool.not(String.is_empty(M.Setting.get(s, key, ""))), m, String.drop(key, 9n), 1n))

def Auto.rows(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match xs:
    case Nil{}: m
    case (th, n) <> rest: Auto.rows(rest, Auto.put(Nat.is_gt(n, 0n), m, th, 1n))

def Auto.index.blocked(+workers: Map<&2, Nat>, +blocked: Map<&2, Nat>, +s: M.State) -> Index:
  match s:
    case M.State{_, ts, _, _, tasks, _, _, _, _, _, _}:
      Index{workers, blocked, Auto.old.reports(s), Auto.ancestors(Map.keys(&2, Nat, blocked), s,
        Auto.task.parents(Map.values(&2, M.Task, tasks), Map.new(&2, String)), List.length(&2, M.Thread, Map.values(&2, M.Thread, ts)), blocked)}

def Auto.index.tasks(pair: Map<&2, Nat> & Map<&2, Nat>, +s: M.State) -> Index:
  (workers, blocked) = pair
  match s:
    case M.State{_, ts, _, _, _, _, qs, _, _, _, _}:
      Auto.index.blocked(workers, Auto.rows(Map.to_list(&2, Nat, M.Work.rows.map(s)), Auto.running(Map.values(&2, M.Thread, ts),
        Auto.queued(Map.to_list(&2, List<&2, M.QItem>, qs), Auto.wakes(M.Setting.keys.pre(s, "wakes.by."), s,
          Auto.secrets(M.Setting.keys.pre(s, "secret.ask."), s, Auto.asks(M.Asks.all(s), Auto.locks(M.Locks.names(s), s, blocked))))))), s)

def Auto.index(+s: M.State) -> Index:
  match s:
    case M.State{_, _, _, _, tasks, _, _, _, _, _, _}:
      Auto.index.tasks(Auto.tasks(Map.values(&2, M.Task, tasks), Map.new(&2, Nat), Map.new(&2, Nat)), s)

def Auto.changes(+s: M.State, now: Nat) -> List<&2, M.Change>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Auto.go(Map.values(&2, M.Thread, ts), s, now, Auto.index(s))

# A receipt names the actual assistant message, rather than a timestamp:
# two replies in the same second cannot acknowledge each other.
def Auto.upgrade.needed(needed: Bool, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match needed:
    case False{}: Nil{}
    case True{}: Auto.upgrade.index(Auto.index(s), s, now)

def Auto.upgrade(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Auto.upgrade.needed(Bool.not(String.eq(M.Setting.get(s, "autosettle.version", ""), "1")), s, now)

def Auto.quiet.task.got(pair: Map<&2, M.Task> & M.Task, +at: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  (_, task) = pair
  match task:
    case M.Task{_, +p, c, _, _, _, _, _, _}: Auto.put(True{}, Auto.put(True{}, m, p, at), c, at)
def Auto.quiet.task(+s: M.State, +id: String, +at: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match s:
    case M.State{_, _, _, _, tasks, _, _, _, _, _, _}:
      Auto.quiet.task.got(Map.get(M.Task, M.Task.blank(id), tasks, id), at, m)

def Auto.quiet.one(c: M.Change, +s: M.State, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match c:
    case M.MessagePosted{th, _, _, _, at}: Auto.put(True{}, m, th, at)
    case M.WorkSet{+th, +kind, +rows, at}:
      Auto.put(Bool.not(String.eq(rows, M.Setting.get(s, kind ++ "." ++ th, ""))), m, th, at)
    case M.TaskStateSet{id, _, _, at}: Auto.quiet.task(s, id, at, m)
    case M.TaskDelivered{id, at}: Auto.quiet.task(s, id, at, m)
    case M.AskClosed{id, _, at}: Auto.quiet.ask(M.Asks.all(s), id, at, m)
    case M.PendingChanged{th, _, at}: Auto.put(True{}, m, th, at)
    case M.WakeDropped{+id, at}: Auto.put(True{}, m, J.Json.get_str(M.Wake.get(s, id), "by", ""), at)
    case M.LockChanged{+name, l, +at}: Auto.quiet.lock(l, at, Auto.quiet.lock(M.Locks.get(s, name), at, m))
    case _: m

def Auto.quiet.collect(cs: List<&2, M.Change>, +s: M.State, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match cs:
    case Nil{}: m
    case c <> rest: Auto.quiet.collect(rest, s, Auto.quiet.one(c, s, m))


# Audience is sticky once the person joins a worker's conversation.
# Reply identity is durable even when the store keeps only a recent window.
def Auto.message.metadata(c: M.Change, +s: M.State) -> List<&2, M.Change>:
  match c:
    case M.MessagePosted{+th, +id, M.User{}, +text, +at}:
      Bool.pick(List<&2, M.Change>, Bool.or(Auto.direct(id, text), String.is_empty(M.Setting.get(s, "autosettle.audience." ++ th, ""))),
        [M.SettingSet{"autosettle.audience." ++ th, Bool.pick(String, Auto.direct(id, text), "person", "worker"), at}], Nil{})
    case M.MessagePosted{th, id, M.Assistant{}, _, at}:
      [M.SettingSet{"autosettle.output." ++ th, id, at}]
    case _: Nil{}

def Auto.messages.metadata.acc(cs: List<&2, M.Change>, +s: M.State, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}: List.reverse(&2, M.Change, acc)
    case c <> rest:
      +one = Auto.message.metadata(c, s)
      Auto.messages.metadata.acc(rest, M.State.replay(one, s), List.append(&2, M.Change, List.reverse(&2, M.Change, one), acc))

def Auto.messages.metadata(cs: List<&2, M.Change>, +s: M.State) -> List<&2, M.Change>:
  Auto.messages.metadata.acc(cs, s, Nil{})

def Auto.emit.quiet(yes: Bool, +th: String, +at: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match yes:
    case False{}: acc
    case True{}: M.SettingSet{"autosettle.quiet." ++ th, Nat.show(at), at} <> acc
def Auto.quiet.changes.acc(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +s: M.State, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}: List.reverse(&2, M.Change, acc)
    case (+th, +at) <> rest:
      Auto.quiet.changes.acc(rest, s, Auto.emit.quiet(Bool.and(Bool.not(String.is_empty(th)), Nat.is_gt(at, Auto.num(s, "autosettle.quiet." ++ th, 0n))), th, at, acc))

def Auto.quiet.changes(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +s: M.State) -> List<&2, M.Change>:
  Auto.quiet.changes.acc(xs, s, Nil{})
