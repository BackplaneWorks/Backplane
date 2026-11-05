import Base
import ./locks.bend as LK
import ./model.bend as M
import ./subthreads.bend as ST
import ./signal.bend as SG
import ./fold.bend as FO
import ./client.bend as U
import ./diff.bend as DF
import ./wake.bend as WK
import ./json.bend as J
import ./i18n.bend as I
import ./msgs.bend as MSG

# Subagents
# ---------
# What a thread has working for it, as rows every client draws the same
# way: the tasks it delegated (child threads; a click opens one) and the
# agent's own subagents (Claude's Agent calls, model.bend's Bg; a click
# opens or shuts the steps it took). Rows at work come with what they do
# now, and a subagent at work shows its newest steps unless shut, so the
# person sees them go (laws subs_*).
#
# Starts and each call's activity have their own indices. At most
# Subs.window() starts and that many entries per call are read, newest
# first, so another agent's activity cannot erase a row or empty its steps.

type SubStep is Data:
  SubStep{kind: String, text: String}

type SubRow is Data:
  SubRow{key: String, agent: Bool, title: String, who: String, word: String, doing: String, live: Bool, open: Bool, n: Nat, steps: List<&2, SubStep>}

# a subagent as its log tells it: the call that started it, its kind
# (model.bend's Bg.kind), what it was asked, how it ended ("" while it
# runs), how many steps it took and its newest steps (oldest first)
type SubAgg is Data:
  SubAgg{call: String, kind: String, title: String, end: String, n: Nat, steps: List<&2, SubStep>, at: Nat}

# how many log entries a frame reads, and steps a row shows
def Subs.window() -> Nat:
  400n

def Subs.shown() -> Nat:
  6n

# what the hub notes about a subagent beside its steps (hub.bend's
# Sub.about at its Agent call: its type, model and prompt; Bg.outcome at
# its end: what it reported and used), each a JSON object
def Subs.meta(+kind: String) -> Bool:
  Bool.or(String.eq(kind, "about"), String.eq(kind, "outcome"))

# a step's first line, in one walk that stops at the first break (splitting
# a subagent's 20 KB step recursed once per character and overflowed the
# web's and the phones' stacks: the thread never opened)
def Subs.line(+t: String) -> String:
  DF.Diff.first(t)

# Reading the log
# ---------------

def Agg.call(a: SubAgg) -> String:
  match a:
    case SubAgg{c, _, _, _, _, _, _}:
      c

# a step taken, kept while fewer than Subs.shown() are (the walk goes newest
# first, so what it keeps are the newest). A match, not a Bool.pick: the
# pick took the first line of every step a subagent took (up to
# Subs.window() each, a character at a time), only to keep six, at every
# render of the thread, a Subagents fold included (~170 ms on a thread
# with 8 subagents)
def Agg.step.k(keep: Bool, +kind: String, +text: String, ss: List<&2, SubStep>) -> List<&2, SubStep>:
  match keep:
    case True{}:
      SubStep{kind, Subs.line(text)} <> ss
    case False{}:
      ss

def Agg.step(+kind: String, +text: String, +n: Nat, +ss: List<&2, SubStep>) -> List<&2, SubStep>:
  Agg.step.k(Nat.is_lt(n, Subs.shown()), kind, text, ss)

# 0 its start, 1 its end, 3 what the hub noted about it (Subs.meta: what it
# was asked, how it ended), 2 a step it took
# 4 a call's step count where the store read left its older steps out
# (server/subagent-history.bend: only each call's newest Subs.shown() steps
# are read, with this count of the steps a full read would have walked)
def Subs.count.kind() -> String:
  "steps.count"

def Agg.kind(+kind: String) -> Nat:
  Bool.pick(Nat, String.starts_with(kind, "start"), 0n, Bool.pick(Nat, String.eq(kind, "end"), 1n, Bool.pick(Nat, Subs.meta(kind), 3n, Bool.pick(Nat, String.eq(kind, Subs.count.kind()), 4n, 2n))))

# the count's own number, never fewer than the steps already walked
def Subs.count.of(+text: String, +n: Nat) -> Nat:
  +c = Maybe.default(&2, Nat, Nat.read(text), 0n)
  Bool.pick(Nat, Nat.is_gt(c, n), c, n)

def Agg.find(as: List<&2, SubAgg>, +call: String) -> SubAgg:
  match as:
    case Nil{}:
      SubAgg{call, "", "", "", 0n, Nil{}, 0n}
    case +a <> t:
      Bool.pick(SubAgg, String.eq(Agg.call(a), call), a, Agg.find(t, call))

# Rows
# ----

def Subs.fold(+call: String) -> String:
  Bool.pick(String, String.starts_with(call, "subs.done:"), call, "sub:" ++ call)

# whether a subagent's steps show: as the person left them, else while it
# runs
def Subs.open(+info: Map<&2, String>, +call: String, live: Bool) -> Bool:
  FO.Fold.open(FO.Fold.user(info, Subs.fold(call)), live)

# what a click on a subagent's row asks (FO.Fold.ask, the "fold" action)
def Subs.ask(+open: Bool, +call: String) -> String:
  FO.Fold.ask(open, Subs.fold(call))

def Subs.steps(open: Bool, ss: List<&2, SubStep>) -> List<&2, SubStep>:
  match open:
    case True{}:
      ss
    case False{}:
      Nil{}

def Subs.or(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

# "start:<kind>" names its kind ("start" alone, none)
def Agg.upd(i: Nat, a: SubAgg, +kind: String, +text: String, +at: Nat) -> SubAgg:
  match i a:
    case 0n SubAgg{c, _, _, e, n, ss, +tm}:
      SubAgg{c, String.drop(kind, 6n), Subs.line(text), e, n, ss, Bool.pick(Nat, Nat.is_eq(tm, 0n), at, tm)}
    case 1n SubAgg{c, k, ti, +en, n, ss, +tm}:
      SubAgg{c, k, ti, Subs.or(en, text), n, ss, Bool.pick(Nat, String.is_empty(en), at, tm)}
    case 3n a:
      a
    case 4n SubAgg{c, k, ti, e, +n, ss, tm}:
      SubAgg{c, k, ti, e, Subs.count.of(text, n), ss, tm}
    case _ SubAgg{c, k, ti, e, +n, ss, +tm}:
      SubAgg{c, k, ti, e, 1n+n, Agg.step(kind, text, n, ss), Bool.pick(Nat, Nat.is_eq(tm, 0n), at, tm)}

def Agg.upd.if(hit: Bool, a: SubAgg, +kind: String, +text: String, +at: Nat) -> SubAgg:
  match hit:
    case True{}:
      Agg.upd(Agg.kind(kind), a, kind, text, at)
    case False{}:
      a

def Agg.fresh(done: Bool, +call: String, +kind: String, +text: String, +at: Nat) -> List<&2, SubAgg>:
  match done:
    case True{}:
      Nil{}
    case False{}:
      [Agg.upd(Agg.kind(kind), SubAgg{call, "", "", "", 0n, Nil{}, 0n}, kind, text, at)]

# an entry of call's put to its subagent, or a new one last; once put, the
# rest is as it was (it was walked and rebuilt cell by cell: for each of
# up to Subs.window() entries, every subagent of the thread, every layout)
def Agg.put(as: List<&2, SubAgg>, +call: String, +kind: String, +text: String, +at: Nat, +done: Bool) -> List<&2, SubAgg>:
  match as done:
    case _ True{}:
      as
    case Nil{} False{}:
      Agg.fresh(False{}, call, kind, text, at)
    case +a <> t False{}:
      +hit = String.eq(Agg.call(a), call)
      Agg.upd.if(hit, a, kind, text, at) <> Agg.put(t, call, kind, text, at, hit)

def Agg.entry(e: M.Entry, as: List<&2, SubAgg>) -> List<&2, SubAgg>:
  match e:
    case M.Act{_, _, +call, +kind, +text, at}:
      Agg.put(as, call, kind, text, at, False{})
    case M.Msg{_, _, _, _, _}:
      as

def Agg.walk(k: Nat, es: List<&2, M.Entry>, as: List<&2, SubAgg>) -> List<&2, SubAgg>:
  match k es:
    case 0n _:
      as
    case 1n+_ Nil{}:
      as
    case 1n+j e <> t:
      Agg.walk(j, t, Agg.entry(e, as))

# The same walk with each subagent found by its call in a map, and the
# calls kept in the order first met (the list walk compared every entry's
# call with every subagent's: 15 ms a layout for a thread with 141
# subagents); Subs.aggs's answer is the same, in the same order
type AggS is Data:
  AggS{m: Map<&2, SubAgg>, order: List<&2, String>}

def Agg.blank(+call: String) -> SubAgg:
  SubAgg{call, "", "", "", 0n, Nil{}, 0n}

def Agg.or.k(fresh: Bool, +call: String, a: SubAgg) -> SubAgg:
  match fresh:
    case True{}:
      Agg.blank(call)
    case False{}:
      a

def Agg.order.k(fresh: Bool, +call: String, order: List<&2, String>) -> List<&2, String>:
  match fresh:
    case True{}:
      call <> order
    case False{}:
      order

def Agg.got(r: Map<&2, SubAgg> & SubAgg, order: List<&2, String>, +call: String, +kind: String, +text: String, +at: Nat) -> AggS:
  (m, +a) = r
  AggS{Map.set(&2, SubAgg, m, call, Agg.upd(Agg.kind(kind), Agg.or.k(String.is_empty(Agg.call(a)), call, a), kind, text, at)), Agg.order.k(String.is_empty(Agg.call(a)), call, order)}

def Agg.met.a(s: AggS, +call: String, +kind: String, +text: String, +at: Nat) -> AggS:
  match s:
    case AggS{m, order}:
      Agg.got(Map.get(SubAgg, Agg.blank(""), m, call), order, call, kind, text, at)

def Agg.met(e: M.Entry, s: AggS) -> AggS:
  match e:
    case M.Act{_, _, +call, +kind, +text, +at}:
      Agg.met.a(s, call, kind, text, at)
    case M.Msg{_, _, _, _, _}:
      s

def Agg.find.m(r: Map<&2, SubAgg> & SubAgg) -> SubAgg:
  (_, a) = r
  a

def Agg.list(cs: List<&2, String>, +m: Map<&2, SubAgg>, acc: List<&2, SubAgg>) -> List<&2, SubAgg>:
  match cs:
    case Nil{}:
      acc
    case +c <> t:
      Agg.list(t, m, Agg.find.m(Map.get(SubAgg, Agg.blank(c), m, c)) <> acc)

def Agg.out(s: AggS) -> List<&2, SubAgg>:
  match s:
    case AggS{+m, order}:
      Agg.list(order, m, Nil{})

def Agg.scan(k: Nat, es: List<&2, M.Entry>, s: AggS) -> AggS:
  match k es:
    case 0n _:
      s
    case 1n+_ Nil{}:
      s
    case 1n+j e <> t:
      Agg.scan(j, t, Agg.met(e, s))

def Agg.walk.m(k: Nat, es: List<&2, M.Entry>, s: AggS) -> List<&2, SubAgg>:
  Agg.out(Agg.scan(k, es, s))

def Agg.call.entry(e: M.Entry, a: SubAgg) -> SubAgg:
  match e:
    case M.Act{_, _, _, +kind, +text, +at}: Agg.upd(Agg.kind(kind), a, kind, text, at)
    case _: a

def Agg.call.scan(k: Nat, es: List<&2, M.Entry>, a: SubAgg) -> SubAgg:
  match k es:
    case 0n _: a
    case 1n+_ Nil{}: a
    case 1n+j e <> t: Agg.call.scan(j, t, Agg.call.entry(e, a))

def Agg.replaced(r: Map<&2, SubAgg> & SubAgg, order: List<&2, String>, +a: SubAgg) -> AggS:
  (m, old) = r
  AggS{Map.set(&2, SubAgg, m, Agg.call(a), a), Agg.order.k(String.is_empty(Agg.call(old)), Agg.call(a), order)}

def Agg.recover.entry(e: M.Entry, +st: M.State, s: AggS) -> AggS:
  match e s:
    case M.Act{_, +log, +call, +kind, +text, +at} AggS{m, order}:
      +a = Agg.upd(0n, Agg.call.scan(Subs.window(), M.State.mine(st, M.Idx.sub.call(log, call)), Agg.blank(call)), kind, text, at)
      Agg.replaced(Map.get(SubAgg, Agg.blank(""), m, call), order, a)
    case _ _: s

def Agg.recover(es: List<&2, M.Entry>, +st: M.State, s: AggS) -> AggS:
  match es:
    case Nil{}: s
    case e <> t: Agg.recover(t, st, Agg.recover.entry(e, st, s))

def Subs.starts(+st: M.State, +th: String) -> List<&2, M.Entry>:
  List.take(&2, M.Entry, M.State.mine(st, M.Idx.sub.starts(M.Bg.log(th))), Subs.window())

# a thread's subagents from their log, the most recently busy first
def Subs.aggs(+st: M.State, +th: String) -> List<&2, SubAgg>:
  Agg.out(Agg.recover(Subs.starts(st, th), st, Agg.scan(Subs.window(), M.State.mine(st, M.Bg.log(th)), AggS{Map.new(&2, SubAgg), Nil{}})))

# the word for how a subagent ended
def Subs.ended(+status: String) -> String:
  Bool.pick(String, String.eq(status, "completed"), "done",
    Bool.pick(String, String.is_empty(status), "stopped", status))

# what a row is titled: what it was asked, after its kind unless it is a
# subagent ("shell: scripts/check.sh")
def Subs.named(+kind: String, +title: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(kind), String.eq(kind, "subagent")), title, kind ++ ": " ++ title)

# Untitled live work gets its kind and first known step, never a bare label.
def Subs.first_step(ss: List<&2, SubStep>) -> String:
  match ss:
    case Nil{}:
      ""
    case SubStep{+kind, text} <> _:
      Subs.or(text, kind)

# 24 hours in epoch seconds. Unknown times stay; missing starts are separately hidden.
def Subs.age() -> Nat:
  86400n

def Subs.old(+now: Nat, +at: Nat) -> Bool:
  Bool.and(Nat.is_gt(at, 0n), Nat.is_gt(Nat.sub(now, at), Subs.age()))

def Subs.keep(keep: Bool, row: SubRow, rest: List<&2, SubRow>) -> List<&2, SubRow>:
  match keep:
    case True{}:
      row <> rest
    case False{}:
      rest

def Subs.known(+title: String) -> Bool:
  Bool.not(Bool.or(String.is_empty(title), Bool.or(String.eq(title, "Subagent"), String.eq(title, "subagent"))))

def Agg.kept(a: SubAgg, +now: Nat) -> Bool:
  match a:
    case SubAgg{_, _, +title, _, _, _, at}:
      Bool.and(Subs.known(Subs.line(title)), Bool.not(Subs.old(now, at)))

def Subs.asked(+title: String, +logged: String) -> String:
  Bool.pick(String, Subs.known(Subs.line(title)), title, logged)

def Subs.title(+kind: String, +title: String, +ss: List<&2, SubStep>, +doing: String) -> String:
  Bool.pick(String, Bool.not(Subs.known(Subs.line(title))), Subs.or(kind, "subagent") ++ ": " ++ Subs.or(Subs.first_step(ss), Subs.or(Subs.line(doing), "working")), Subs.named(kind, Subs.line(title)))

def Subs.agg.row(a: SubAgg, +info: Map<&2, String>, +kind: String, +title: String, +doing: String, +live: Bool) -> SubRow:
  match a:
    case SubAgg{+call, +k, ti, +en, n, +ss, _}:
      +open = Subs.open(info, call, live)
      +kd = Subs.or(kind, k)
      SubRow{call, True{}, Subs.title(kd, Subs.asked(title, ti), ss, doing), Subs.or(kd, "subagent"), Bool.pick(String, live, "running", Subs.ended(en)),
        doing, live, open, n, Subs.steps(open, ss)}

# what an ended row (M.Bg.ended: finished, its outcome not yet handed to
# the agent) shows in place of what it does
def Subs.reporting() -> String:
  "finished, reporting"

def Subs.word.k(ended: Bool, r: SubRow) -> SubRow:
  match ended r:
    case True{} SubRow{k, a, ti, who, _, _, live, o, n, ss}:
      SubRow{k, a, ti, who, "reporting", Subs.reporting(), live, o, n, ss}
    case False{} _:
      r

# a subagent at work (a line of the thread's Bg); an ended one still
# counts, "reporting"
def Subs.bg.row(+l: String, +as: List<&2, SubAgg>, +info: Map<&2, String>) -> SubRow:
  Subs.word.k(M.Bg.ended(l), Subs.agg.row(Agg.find(as, Subs.or(M.Bg.call(l), M.Bg.field(l, 0n))), info, M.Bg.kind.of(l), M.Bg.title(l), M.Bg.doing(l), True{}))

def Subs.bg.rows(ls: List<&2, String>, +as: List<&2, SubAgg>, +info: Map<&2, String>) -> List<&2, SubRow>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Subs.bg.row(l, as, info) <> Subs.bg.rows(t, as, info)

def Subs.calls(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Subs.or(M.Bg.call(l), M.Bg.field(l, 0n)) <> Subs.calls(t)

def Subs.has.go(xs: List<&2, String>, +x: String, found: Bool) -> Bool:
  match xs found:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case +y <> t False{}:
      Subs.has.go(t, x, String.eq(x, y))

def Subs.has(xs: List<&2, String>, +x: String) -> Bool:
  Subs.has.go(xs, x, False{})

# Finished agents without their start in the bounded window are omitted.
def Subs.done(as: List<&2, SubAgg>, +busy: List<&2, String>, +info: Map<&2, String>, +now: Nat) -> List<&2, SubRow>:
  match as:
    case Nil{}:
      Nil{}
    case +a <> t:
      Subs.keep(Bool.and(Bool.not(Subs.has(busy, Agg.call(a))), Agg.kept(a, now)), Subs.agg.row(a, info, "", "", "", False{}), Subs.done(t, busy, info, now))

# watched work (hub.bend's Watch.*: work_run, work_watch): a row a
# piece, kind "watch", its state the word ("launching", "watching",
# "cancelling", or "unknown" once its outcome cannot be known)
def Subs.watch.word(+l: String) -> String:
  Bool.pick(String, M.Bg.ended(l), M.Bg.status(l), M.Bg.doing(l))

def Subs.watch.row(+l: String, +info: Map<&2, String>) -> SubRow:
  SubRow{M.Bg.field(l, 0n), True{}, "watch: " ++ M.Bg.title(l), "watch", Subs.watch.word(l), M.Bg.clean(M.Bg.dec(M.Bg.field(l, 6n))), True{}, Subs.open(info, M.Bg.field(l, 0n), True{}), 0n, Nil{}}

def Subs.watch.rows(ls: List<&2, String>, +info: Map<&2, String>) -> List<&2, SubRow>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Subs.watch.row(l, info) <> Subs.watch.rows(t, info)

def Subs.key(r: SubRow) -> String:
  match r:
    case SubRow{k, _, _, _, _, _, _, _, _, _}:
      k

# Presentation membership uses every row's actual key. Panel-close
# signatures (Subs.keys) name attention state and omit finished rows.
def Subs.row_keys(rs: List<&2, SubRow>) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case r <> t: Subs.key(r) <> Subs.row_keys(t)

def Subs.word(r: SubRow) -> String:
  match r:
    case SubRow{_, _, _, _, w, _, _, _, _, _}:
      w

# the action a row's Stop sends (with the row's key): watched work still
# at work "work-cancel", a delegated task at work "task-cancel"; "" for
# none (the agent's own subagents stop with its turn)
def Subs.cancel(r: SubRow) -> String:
  match r:
    case SubRow{_, +agent, _, +who, +word, _, +live, _, _, _}:
      Bool.pick(String, String.eq(who, "lock"), Bool.pick(String, String.eq(word, "held"), "lock-release", "lock-cancel"), Bool.pick(String, String.eq(who, "wake"), "wake-cancel", Bool.pick(String, Bool.and(agent, Bool.and(String.eq(who, "watch"), Bool.and(live, Bool.not(Bool.or(String.eq(word, "cancelling"), String.eq(word, "unknown")))))), "work-cancel",
        Bool.pick(String, Bool.and(Bool.not(agent), Bool.and(live, Bool.not(String.starts_with(who, "launched · ")))), "task-cancel", ""))))

def Subs.cancel.word(+act: String) -> String:
  Bool.pick(String, String.eq(act, "lock-release"), "Release", Bool.pick(String, Bool.or(String.eq(act, "lock-cancel"), String.eq(act, "wake-cancel")), "Cancel", "Stop"))

# the agent's own subagents: those at work in the order they started, then
# the work Backplane watches for it, then the newest finished ones
def Subs.agents(+st: M.State, +info: Map<&2, String>, +th: String, +now: Nat) -> List<&2, SubRow>:
  +as = Subs.aggs(st, th)
  +ls = M.Bg.of(st, th)
  List.concat(&2, SubRow, [Subs.bg.rows(ls, as, info), Subs.watch.rows(M.Work.watch(st, th), info), Subs.done(as, Subs.calls(ls), info, now)])

# what a thread did last: its newest entry's first line (a design-history
# step is skipped: its text is the hub's own record, not words)
def Subs.latest.of(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case M.Msg{_, _, _, +text, _} <> _:
      Subs.line(text)
    case M.Act{_, _, "step", _, _, _} <> t:
      Subs.latest.of(t)
    case M.Act{_, _, _, +kind, +text, _} <> _:
      kind ++ "  " ++ Subs.line(text)

def Subs.latest(live: Bool, +st: M.State, +th: String) -> String:
  match live:
    case True{}:
      Subs.latest.of(M.State.mine(st, th))
    case False{}:
      ""

# Ask/approval and monitored work are derived from the same thread phase as the sidebar.
def Subs.needs(p: M.Phase) -> Bool:
  match p:
    case M.PhApproval{}:
      True{}
    case M.PhInput{}:
      True{}
    case _:
      False{}

def Subs.phase.live(p: M.Phase) -> Bool:
  match p:
    case M.PhWorking{}:
      True{}
    case M.PhMonitoring{_}:
      True{}
    case M.PhApproval{}:
      True{}
    case M.PhInput{}:
      True{}
    case M.PhIdle{}:
      True{}
    case _:
      False{}

def Subs.model.first(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{'-', _}:
      String.reverse(acc)
    case SCon{c, t}:
      Subs.model.first(t, SCon{c, acc})

def Subs.model.family(model: String) -> String:
  Subs.model.first(model, "")

def Subs.model(+provider: String, +model: String) -> String:
  +mo = Subs.or(model, "default")
  +short = Bool.pick(String, String.starts_with(mo, "claude-"), String.drop(mo, 7n), mo)
  +family = Bool.pick(String, String.starts_with(mo, "claude-"), Subs.or(Subs.model.family(short), "default"), mo)
  Bool.pick(String, String.eq(provider, "claude"), family, provider ++ " · " ++ mo)

def Subs.launch.who(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, pr, mo, _, _, _, _}:
      "launched · " ++ Subs.model(pr, mo)

def Subs.task(t: M.Task, +st: M.State, +ix: U.SIx) -> SubRow:
  match t:
    case M.Task{_, _, +child, _, +s, _, _, +pr, +mo}:
      +ph = U.Side.phase.ix(ix, st, U.Ui.thread(st, child))
      +needs = Subs.needs(ph)
      +live = Bool.not(M.TaskState.terminal(s))
      SubRow{child, False{}, U.Tasks.title(st, child), Subs.model(pr, mo), Bool.pick(String, needs, "needs you", U.Task.word(s)), Subs.latest(Bool.or(needs, live), st, child), live, False{}, 0n, Nil{}}

# when a task's or launched thread's work last moved: its thread's
# activity (model.bend's Thread.turned/touched: the newest turn change or
# person's message). The log has the time; nothing is folded into settings
# or the state for it, so far sharing, settings sync and the phones' kept
# state are unchanged. 0 (no such thread) is an unknown time, never old.
def Subs.thread.at(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, at, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      at

def Subs.end(+st: M.State, +child: String) -> Nat:
  Subs.thread.at(U.Ui.thread(st, child))

def Subs.task.kept(t: M.Task, +st: M.State, +now: Nat, +ix: U.SIx) -> Bool:
  match t:
    case M.Task{_, _, +child, _, s, _, _, _, _}:
      Bool.or(Bool.or(Bool.not(M.TaskState.terminal(s)), Subs.needs(U.Side.phase.ix(ix, st, U.Ui.thread(st, child)))),
        Bool.not(Subs.old(now, Subs.end(st, child))))

def Subs.tasks(ts: List<&2, M.Task>, +st: M.State, +now: Nat, +ix: U.SIx) -> List<&2, SubRow>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Subs.keep(Subs.task.kept(t, st, now, ix), Subs.task(t, st, ix), Subs.tasks(rest, st, now, ix))

# Launched threads (thread_launch, client.bend's Launched): each a row like
# a delegated task's (a click opens it), "launched" before its model; every
# one at work or waiting on the person, then the newest finished ones
# (law subs_launched_live_kept)
def Subs.launch.word(p: M.Phase, fallback: String) -> String:
  match p:
    case M.PhWorking{}:
      "running"
    case M.PhMonitoring{_}:
      "watching"
    case M.PhApproval{}:
      "needs you"
    case M.PhInput{}:
      "needs you"
    case M.PhIdle{}:
      "launching"
    case _:
      fallback

def Subs.launch.row(+t: M.Thread, +st: M.State, +id: String, +ix: U.SIx) -> SubRow:
  +ph = U.Side.phase.ix(ix, st, t)
  +live = Subs.phase.live(ph)
  SubRow{id, False{}, U.Tasks.title(st, id), Subs.launch.who(t), Subs.launch.word(ph, U.Launched.word(t)), Subs.latest(live, st, id), live, False{}, 0n, Nil{}}

# Newest first, all live/attention rows and every finished row younger than the age limit.
def Subs.launch.go(ids: List<&2, String>, +st: M.State, +now: Nat, +ix: U.SIx) -> List<&2, SubRow>:
  match ids:
    case Nil{}:
      Nil{}
    case +id <> t:
      +thread = U.Ui.thread(st, id)
      Subs.keep(Bool.and(U.Launched.shown(thread, id), Bool.or(Subs.phase.live(U.Side.phase.ix(ix, st, thread)), Bool.not(Subs.old(now, Subs.end(st, id))))),
        Subs.launch.row(thread, st, id, ix), Subs.launch.go(t, st, now, ix))

# The child phases share one index rather than rescanning every ask/task
# for each row. Project rows are unnecessary for phase queries.
def Subs.index(+st: M.State) -> U.SIx:
  U.SIx{Map.new(&2, List<&2, M.Thread>), U.SIx.asks.go(List.reverse(&2, M.Ask, M.Asks.all(st)), Map.new(&2, List<&2, M.Ask>)), M.Work.kids.map(st), M.Work.rows.map(st)}

# Kept for callers that inspect launched rows directly; panel rows supply their clock.
def Subs.launched(+st: M.State, +th: String) -> List<&2, SubRow>:
  Subs.launch.go(List.reverse(&2, String, M.Launch.of(st, th)), st, 0n, Subs.index(st))

# every row of a thread's subagents: its delegated tasks, the threads it
# launched, then the agent's
# A waiting subscription is an openable row without a thread to select.
def Subs.wake.row(+o: J.Json) -> SubRow:
  SubRow{J.Json.get_str(o, "id", ""), True{}, J.Json.get_str(o, "kind", "") ++ " " ++ J.Json.get_str(o, "target", ""), "wake", "waiting on", J.Json.get_str(o, "note", "") ++ " (" ++ String.join(WK.Wake.events(o), ", ") ++ ")", True{}, False{}, 0n, Nil{}}

def Subs.wake.go(ids: List<&2, String>, +st: M.State) -> List<&2, SubRow>:
  match ids:
    case Nil{}:
      Nil{}
    case id <> t:
      Subs.wake.row(WK.Wake.get(st, id)) <> Subs.wake.go(t, st)

def Subs.wakes(+st: M.State, +th: String) -> List<&2, SubRow>:
  Subs.wake.go(WK.Wake.by(st, th), st)

def Subs.lock.row(+name: String, +l: LK.NamedLock, +th: String) -> SubRow:
  +held = String.eq(LK.Lock.owner(l), th)
  SubRow{name, True{}, "lock: " ++ name, "lock", Bool.pick(String, held, "held", "waiting"), Bool.pick(String, held, "Release when done", "position " ++ Nat.show(LK.Lock.position(l, th)) ++ " · held by " ++ LK.Lock.owner(l)), True{}, False{}, 0n, Nil{}}

def Subs.lock.has(+has: Bool, +name: String, +l: LK.NamedLock, +th: String) -> List<&2, SubRow>:
  match has:
    case True{}:
      [Subs.lock.row(name, l, th)]
    case False{}:
      Nil{}

def Subs.lock.of(+name: String, +l: LK.NamedLock, +th: String) -> List<&2, SubRow>:
  Subs.lock.has(LK.Lock.has(l, th), name, l, th)

# an idle lock (M.Locks.idle) has no holder or waiter, so no thread's row
def Subs.lock.if(+idle: Bool, +st: M.State, +name: String, +th: String) -> List<&2, SubRow>:
  match idle:
    case True{}:
      Nil{}
    case False{}:
      Subs.lock.of(name, M.Locks.get(st, name), th)

def Subs.locks.go(names: List<&2, String>, +st: M.State, +th: String) -> List<&2, SubRow>:
  match names:
    case Nil{}:
      Nil{}
    case +name <> t:
      List.append(&2, SubRow, Subs.lock.if(M.Locks.idle(st, name), st, name, th), Subs.locks.go(t, st, th))

# Successful/stopped finishes fold away; failures remain visible as attention.
def Subs.finished(+word: String) -> Bool:
  Subs.has(["done", "completed", "stopped", "cancelled"], word)

def Subs.is_done(r: SubRow) -> Bool:
  match r:
    case SubRow{_, _, _, _, word, _, live, _, _, _}:
      Bool.and(Bool.not(live), Subs.finished(word))

def Subs.attention(r: SubRow) -> Bool:
  match r:
    case SubRow{+key, _, _, +who, +word, _, +live, _, _, _}:
      Bool.and(Bool.not(String.starts_with(key, "subs.done:")),
        Bool.or(Bool.and(Bool.not(live), Bool.not(Subs.finished(word))),
          Bool.or(String.eq(word, "needs you"), Bool.or(String.eq(who, "wake"), Bool.and(String.eq(who, "lock"), String.eq(word, "waiting"))))))

def Subs.is_live(r: SubRow) -> Bool:
  match r:
    case SubRow{_, _, _, _, _, _, live, _, _, _}:
      live

def Subs.partition(rs: List<&2, SubRow>, +done: Bool) -> List<&2, SubRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Subs.keep(Bool.not(Bool.xor(Subs.is_done(r), done)), r, Subs.partition(t, done))

def Subs.done.key(+th: String) -> String:
  "subs.done:" ++ th

def Subs.done.toggle(+th: String, +n: Nat, +open: Bool) -> SubRow:
  SubRow{Subs.done.key(th), True{}, Nat.show(n) ++ " done", "", "", "", False{}, open, n, Nil{}}

def Subs.finished.rows(open: Bool, rs: List<&2, SubRow>) -> List<&2, SubRow>:
  match open:
    case True{}:
      rs
    case False{}:
      Nil{}

def Subs.finish(some: Bool, +info: Map<&2, String>, +th: String, +done: List<&2, SubRow>) -> List<&2, SubRow>:
  match some:
    case False{}:
      Nil{}
    case True{}:
      +open = FO.Fold.open(FO.Fold.user(info, Subs.done.key(th)), False{})
      Subs.done.toggle(th, List.length(&2, SubRow, done), open) <> Subs.finished.rows(open, done)

def Subs.priority(+r: SubRow) -> Nat:
  Bool.pick(Nat, Subs.is_live(r), Bool.pick(Nat, Subs.attention(r), 1n, 0n), Bool.pick(Nat, String.eq(Subs.word(r), "needs you"), 1n, 2n))

def Subs.current(rs: List<&2, SubRow>, +priority: Nat) -> List<&2, SubRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Subs.keep(Bool.and(Bool.not(Subs.is_done(r)), Nat.is_eq(Subs.priority(r), priority)), r, Subs.current(t, priority))

def Subs.group(+rs: List<&2, SubRow>, +info: Map<&2, String>, +th: String) -> List<&2, SubRow>:
  +done = Subs.partition(rs, True{})
  List.concat(&2, SubRow, [Subs.current(rs, 0n), Subs.current(rs, 1n), Subs.current(rs, 2n), Subs.finish(Bool.not(List.is_empty(&2, SubRow, done)), info, th, done)])

# Project rows do not participate in child phases. A supplied sidebar index
# contributes exactly the same asks, children and watched-work maps.
def Subs.index.phase(ix: U.SIx) -> U.SIx:
  match ix:
    case U.SIx{_, asks, kids, work}:
      U.SIx{Map.new(&2, List<&2, M.Thread>), asks, kids, work}

def Subs.index.given(ixs: List<&2, U.SIx>, +st: M.State) -> U.SIx:
  match ixs:
    case Nil{}: Subs.index(st)
    case ix <> _: Subs.index.phase(ix)

def Subs.children.at(some: Bool, +ts: List<&2, M.Task>, +ids: List<&2, String>, +st: M.State, +now: Nat, +ixs: List<&2, U.SIx>) -> List<&2, SubRow>:
  match some:
    case False{}:
      Nil{}
    case True{}:
      +ix = Subs.index.given(ixs, st)
      List.append(&2, SubRow, Subs.tasks(ts, st, now, ix), Subs.launch.go(List.reverse(&2, String, ids), st, now, ix))

def Subs.children(some: Bool, +ts: List<&2, M.Task>, +ids: List<&2, String>, +st: M.State, +now: Nat) -> List<&2, SubRow>:
  Subs.children.at(some, ts, ids, st, now, Nil{})

def Subs.rows.at(+st: M.State, +info: Map<&2, String>, +th: String, +now: Nat, +ixs: List<&2, U.SIx>) -> List<&2, SubRow>:
  +ts = U.Tasks.of(st, th)
  +ids = M.Launch.of(st, th)
  Subs.group(List.concat(&2, SubRow, [Subs.children.at(Bool.or(Bool.not(List.is_empty(&2, M.Task, ts)), Bool.not(List.is_empty(&2, String, ids))), ts, ids, st, now, ixs), Subs.wakes(st, th), Subs.locks.go(M.Locks.names(st), st, th), Subs.agents(st, info, th, now)]), info, th)

def Subs.rows.ix(+st: M.State, +info: Map<&2, String>, +th: String, +now: Nat, ix: U.SIx) -> List<&2, SubRow>:
  Subs.rows.at(st, info, th, now, [ix])

def Subs.rows(+st: M.State, +info: Map<&2, String>, +th: String, +now: Nat) -> List<&2, SubRow>:
  Subs.rows.at(st, info, th, now, Nil{})

def Subs.live(rs: List<&2, SubRow>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case +r <> t:
      Nat.add(Bool.pick(Nat, Bool.and(Subs.is_live(r), Bool.not(Subs.attention(r))), 1n, 0n), Subs.live(t))

def Subs.waiting(rs: List<&2, SubRow>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case +r <> t:
      Nat.add(Bool.pick(Nat, Subs.attention(r), 1n, 0n), Subs.waiting(t))

def Subs.finished_n(rs: List<&2, SubRow>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case SubRow{+key, _, _, _, _, _, _, _, +n, _} <> t:
      Nat.add(Bool.pick(Nat, String.starts_with(key, "subs.done:"), n, 0n), Subs.finished_n(t))

# "3 at work", or "" with none
def Subs.busy(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", Nat.show(n) ++ " at work")

# The panel as a whole
# --------------------
# Open while work needs attention. A manual shut snapshots the current
# work; only a new work/attention signature overrides that choice. The
# device-local choice lives under subs:<thread> in Ui.info.

def Subs.panel.key(+th: String) -> String:
  "subs:" ++ th

# A waiting signature differs from a running signature, so an existing
# child that starts asking for input reopens a shut panel too.
def Subs.signature(+r: SubRow) -> String:
  Bool.pick(String, Subs.attention(r), "attention:", "live:") ++ Subs.key(r)

def Subs.keys(rs: List<&2, SubRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      List.append(&2, String, Bool.pick(List<&2, String>, Bool.or(Subs.is_live(r), Subs.attention(r)), [Subs.signature(r)], Nil{}), Subs.keys(t))

# Closing while a row needs attention also acknowledges its ordinary live
# identity: resolving the ask must not reopen the panel by itself.
def Subs.recorded_keys(rs: List<&2, SubRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      +known = Bool.pick(List<&2, String>, Subs.attention(r), ["live:" ++ Subs.key(r), Subs.signature(r)], [Subs.signature(r)])
      List.append(&2, String, Bool.pick(List<&2, String>, Bool.or(Subs.is_live(r), Subs.attention(r)), known, Nil{}), Subs.recorded_keys(t))

# Tail-recursive scan of current keys; an absent key reopens the panel.
def Subs.new_keys.go(ks: List<&2, String>, +recorded: List<&2, String>, fresh: Bool) -> Bool:
  match ks fresh:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case +k <> t False{}:
      Subs.new_keys.go(t, recorded, Bool.not(Subs.has(recorded, k)))

def Subs.new_keys(ks: List<&2, String>, +recorded: List<&2, String>) -> Bool:
  Subs.new_keys.go(ks, recorded, False{})

# Split snapshots with a tail call per character (Base.split uses the JS stack).
def Subs.saved(s: String, acc: String, keys: List<&2, String>) -> List<&2, String>:
  match s:
    case SNil{}:
      List.reverse(&2, String, String.reverse(acc) <> keys)
    case SCon{'\u{001f}', t}:
      Subs.saved(t, "", String.reverse(acc) <> keys)
    case SCon{c, t}:
      Subs.saved(t, SCon{c, acc}, keys)

def Subs.panel.chosen(+choice: String, +rs: List<&2, SubRow>) -> Bool:
  Bool.pick(Bool, String.eq(choice, "1"), True{},
    Bool.pick(Bool, String.starts_with(choice, "0"), Subs.new_keys(Subs.keys(rs), Subs.saved(String.drop(choice, 1n), "", Nil{})),
      Nat.is_gt(Nat.add(Subs.live(rs), Subs.waiting(rs)), 0n)))

def Subs.panel.open(+info: Map<&2, String>, +th: String, +rs: List<&2, SubRow>) -> Bool:
  Subs.panel.chosen(U.Str.get(info, Subs.panel.key(th)), rs)

def Subs.panel.action(open: Bool) -> String:
  Bool.pick(String, open, "subs-shut", "subs-open")

# all a panel holds is the finished fold ("▸ 2 done"): nothing at work,
# waiting or failed
def Subs.only_done(rs: List<&2, SubRow>) -> Bool:
  match rs:
    case Nil{}:
      True{}
    case +r <> t:
      Bool.and(Bool.or(String.starts_with(Subs.key(r), "subs.done:"), Subs.is_done(r)), Subs.only_done(t))

# shutting records the rows seen; opening a panel that holds only the
# finished fold opens the fold too ("\tdone"), so one click shows which
# ones finished rather than "▸ 2 done" to click again
def Subs.panel.ask(+open: Bool, +th: String, +rs: List<&2, SubRow>) -> String:
  Bool.pick(String, open, th ++ "\t" ++ String.join(Subs.recorded_keys(rs), "\u{001f}"),
    Bool.pick(String, Bool.and(Bool.not(List.is_empty(&2, SubRow, rs)), Subs.only_done(rs)), th ++ "\tdone", th))

# Finished count comes from the toggle, never counting it twice when expanded.
def Subs.panel.word(+rs: List<&2, SubRow>) -> String:
  Bool.pick(String, Nat.is_gt(Subs.waiting(rs), 0n), Nat.show(Subs.waiting(rs)) ++ " waiting" ++ Bool.pick(String, Nat.is_gt(Subs.live(rs), 0n), " · " ++ Subs.busy(Subs.live(rs)), ""),
    Subs.or(Subs.busy(Subs.live(rs)), Nat.show(Subs.finished_n(rs)) ++ " done"))

# the rows a panel shows: all of them open, none shut
def Subs.panel.rows(open: Bool, rs: List<&2, SubRow>) -> List<&2, SubRow>:
  match open:
    case True{}:
      rs
    case False{}:
      Nil{}

# Presentation only. Keep the source rows, work signatures, counts, done
# toggle and every action unchanged. A page groups conversations apart
# from shells, watched work, locks, wakes and legacy native subagents.
# Headers never enter Subs.rows or the lifecycle/badge projection.
def Subs.display.heading(r: SubRow) -> Bool:
  String.starts_with(Subs.key(r), "subs.section:")

def Subs.display.kind(+r: SubRow) -> Nat:
  match r:
    case SubRow{+key, +agent, _, _, _, _, +live, _, _, _}:
      Bool.pick(Nat, String.starts_with(key, "subs.done:"), 2n,
        Nat.add(Bool.pick(Nat, agent, 1n, 0n), Bool.pick(Nat, Subs.is_done(r), 3n, 0n)))

def Subs.display.part(rs: List<&2, SubRow>, +kind: Nat) -> List<&2, SubRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest:
      Subs.keep(Nat.is_eq(Subs.display.kind(r), kind), r, Subs.display.part(rest, kind))

def Subs.display.group(rs: List<&2, SubRow>, +kind: Nat) -> List<&2, SubRow>:
  match rs:
    case Nil{}: Nil{}
    case r <> rest:
      SubRow{"subs.section:" ++ Nat.show(kind), True{}, "", "section", "", "", False{}, False{}, 0n, Nil{}} <> r <> rest

def Subs.display(+rs: List<&2, SubRow>) -> List<&2, SubRow>:
  List.concat(&2, SubRow, [Subs.display.group(Subs.display.part(rs, 0n), 0n),
    Subs.display.group(Subs.display.part(rs, 1n), 1n), Subs.display.part(rs, 2n),
    Subs.display.group(Subs.display.part(rs, 3n), 3n), Subs.display.group(Subs.display.part(rs, 4n), 4n)])

def Subs.display.label(+lc: I.Loc, +r: SubRow) -> String:
  Bool.pick(String, Bool.or(String.eq(Subs.key(r), "subs.section:0"), String.eq(Subs.key(r), "subs.section:3")),
    MSG.T.subagents.group.threads(lc), MSG.T.subagents.group.background(lc))

def Subs.agent_n(ls: List<&2, String>) -> Nat:
  match ls:
    case Nil{}:
      0n
    case +l <> t:
      Nat.add(Bool.pick(Nat, String.eq(M.Bg.kind.of(l), "subagent"), 1n, 0n), Subs.agent_n(t))

# Visible conversations count even when they have no delegated task.
# Existing task counts stay separate; never count one child twice.
def Subs.tree.extra.go(ts: List<&2, M.Thread>, +tasks: Map<&2, Bool>) -> Nat:
  match ts:
    case Nil{}: 0n
    case +t <> rest:
      Nat.add(Bool.pick(Nat, U.Flag.get(tasks, M.Thread.id(t)), 0n, 1n), Subs.tree.extra.go(rest, tasks))

def Subs.tree.extra(+st: M.State, +th: String) -> Nat:
  Subs.tree.extra.go(ST.Tree.children(st, th), U.Kids.of(U.Tasks.of(st, th), M.State.bots(st), Map.new(&2, Bool)))

# how many subagents a thread has at work (its sidebar row's count): what
# it delegated that is not finished, and the agent's own (its shells and
# monitors are not agents)
def Subs.count(+st: M.State, +th: String) -> Nat:
  Nat.add(Subs.tree.extra(st, th), Nat.add(U.Side.live(U.Tasks.of(st, th)), Subs.agent_n(M.Bg.of(st, th))))

# Subs.count from the sidebar's index (law subs_count_ix_eq)
def Subs.count.ix(+ix: U.SIx, +st: M.State, +th: String) -> Nat:
  Nat.add(Subs.tree.extra(st, th), Nat.add(U.SIx.kids(ix, th), Subs.agent_n(M.Bg.of(st, th))))

def Subs.counts(ts: List<&2, M.Thread>, +st: M.State) -> List<&2, Nat>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Subs.count(st, M.Thread.id(t)) <> Subs.counts(rest, st)

def Subs.counts.ix(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State) -> List<&2, Nat>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Subs.count.ix(ix, st, M.Thread.id(t)) <> Subs.counts.ix(rest, ix, st)

# a sidebar row's badge: "1 subthread", "3 subthreads", or "" with none
def Subs.badge(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " subthread", " subthreads"))

# a step as a row shows it: the tool, or "" for what it wrote
# a subagent's step: its tool as the timeline names it (FO.Tool.short: an
# MCP tool without its server prefix), nothing for what it said (laws
# subs_step_label_*)
def Subs.step.label(+kind: String) -> String:
  Bool.pick(String, String.eq(kind, "say"), "", FO.Tool.short(kind))

# Display activity only. Keep the raw live/action flags for cancellation,
# delivery, folding and history compatibility; they include queued/reporting.
def Subs.signal(r: SubRow) -> String:
  match r:
    case SubRow{+key, _, _, +who, +word, _, _, _, _, _}:
      Bool.pick(String, String.starts_with(key, "subs.done:"), "settled",
        Bool.pick(String, String.eq(word, "needs you"), "blocked",
          Bool.pick(String, Bool.or(String.eq(who, "lock"), String.eq(who, "wake")), "inactive",
            Bool.pick(String, Bool.or(String.eq(word, "running"), Bool.or(String.starts_with(word, "watching"), String.eq(word, "cancelling"))), "active",
              Bool.pick(String, Bool.or(Subs.finished(word), String.eq(word, "settled")), "settled", "inactive")))))

def Subs.signal.connected(+info: Map<&2, String>) -> Bool:
  Bool.not(String.eq(U.Str.get(info, "@signal.online"), "0"))

def Subs.signal.of(+info: Map<&2, String>, +r: SubRow) -> String:
  match r:
    case SubRow{+key, agent, _, _, _, _, _, _, _, _}:
      Bool.pick(String, agent,
        Bool.pick(String, Bool.and(String.eq(Subs.signal(r), "active"), Bool.not(Subs.signal.connected(info))), "inactive", Subs.signal(r)),
        SG.Signal.name(SG.Signal.cached(info, key)))

def Subs.active(rs: List<&2, SubRow>, +info: Map<&2, String>) -> Nat:
  match rs:
    case Nil{}: 0n
    case r <> rest:
      Nat.add(Bool.pick(Nat, String.eq(Subs.signal.of(info, r), "active"), 1n, 0n), Subs.active(rest, info))

# More about a row
# ----------------
# What a row says beside its name, worked out only for the rows a page
# shows (view.bend's View.subs.signals.at): where it runs (Claude Code's
# subagent and its type, a background shell, a thread of another agent and
# the machine it is on), its model, when it started and ended, how many
# steps it took, what it used, its newest step, what it was asked and what
# it reported. Nothing here is guessed: a field the log has not got is
# empty (0, "") and the row leaves it out.
type SubInfo is Data:
  SubInfo{runs: String, model: String, since: Nat, until: Nat, steps: Nat, tokens: Nat, last: SubStep, ask: String, outcome: String, shown: List<&2, SubStep>}

def SubInfo.none() -> SubInfo:
  SubInfo{"", "", 0n, 0n, 0n, 0n, SubStep{"", ""}, "", "", Nil{}}

# a subagent's own entries, newest first: its start and end times, its
# step count, newest step and newest Subs.shown() steps (oldest first: the
# page draws them shut too, hidden, so a click shows them at once), the
# newest of what the hub noted
type SubWalk is Data:
  SubWalk{since: Nat, until: Nat, n: Nat, last: SubStep, about: String, outcome: String, ss: List<&2, SubStep>}

def Walk.first(+have: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(have), text, have)

# the newest step's first line, taken once (not for every step: Agg.step.k)
def Walk.last(none: Bool, +kind: String, +text: String, last: SubStep) -> SubStep:
  match none:
    case True{}:
      SubStep{kind, Subs.line(text)}
    case False{}:
      last

def Walk.empty(s: SubStep) -> Bool:
  match s:
    case SubStep{+k, +t}:
      Bool.and(String.is_empty(k), String.is_empty(t))

def Walk.act(i: Nat, w: SubWalk, +kind: String, +text: String, +at: Nat) -> SubWalk:
  match i w:
    case 0n SubWalk{_, u, n, l, ab, oc, ss}:
      SubWalk{at, u, n, l, ab, oc, ss}
    case 1n SubWalk{si, +u, n, l, ab, oc, ss}:
      SubWalk{si, Bool.pick(Nat, Nat.is_eq(u, 0n), at, u), n, l, ab, oc, ss}
    case 3n SubWalk{si, u, n, l, +ab, +oc, ss}:
      SubWalk{si, u, n, l, Bool.pick(String, String.eq(kind, "about"), Walk.first(ab, text), ab), Bool.pick(String, String.eq(kind, "outcome"), Walk.first(oc, text), oc), ss}
    case 4n SubWalk{si, u, +n, l, ab, oc, ss}:
      SubWalk{si, u, Subs.count.of(text, n), l, ab, oc, ss}
    case _ SubWalk{si, u, +n, +l, ab, oc, ss}:
      SubWalk{si, u, 1n+n, Walk.last(Walk.empty(l), kind, text, l), ab, oc, Agg.step(kind, text, n, ss)}

def Walk.entry(e: M.Entry, w: SubWalk) -> SubWalk:
  match e:
    case M.Act{_, _, _, +kind, +text, +at}:
      Walk.act(Agg.kind(kind), w, kind, text, at)
    case M.Msg{_, _, _, _, _}:
      w

def Walk.go(k: Nat, es: List<&2, M.Entry>, w: SubWalk) -> SubWalk:
  match k es:
    case 0n _:
      w
    case 1n+_ Nil{}:
      w
    case 1n+j e <> t:
      Walk.go(j, t, Walk.entry(e, w))

def Subs.walk(+st: M.State, +th: String, +call: String) -> SubWalk:
  Walk.go(Subs.window(), M.State.mine(st, M.Idx.sub.call(M.Bg.log(th), call)), SubWalk{0n, 0n, 0n, SubStep{"", ""}, "", "", Nil{}})

def Subs.json(+text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

# where a subagent of the agent's own runs: Claude Code reports them all
# (hub.bend's Bg.system), as a subagent of a type, a background shell...
def Subs.runs.own(+kind: String, +ty: String) -> String:
  Bool.pick(String, String.eq(kind, "subagent"), "Claude Code" ++ Bool.pick(String, String.is_empty(ty), " subagent", " · " ++ ty),
    Bool.pick(String, String.eq(kind, "shell"), "background shell",
      Bool.pick(String, String.eq(kind, "watch"), "watched by Backplane", Subs.or(kind, "task"))))

def Subs.info.walked(w: SubWalk, +kind: String) -> SubInfo:
  match w:
    case SubWalk{si, u, n, l, +ab, +oc, ss}:
      +a = Subs.json(ab)
      +o = Subs.json(oc)
      SubInfo{Subs.runs.own(kind, J.Json.get_str(a, "type", "")), J.Json.get_str(a, "model", ""), si, u, n, J.Json.get_nat(J.Json.get_obj(o, "usage"), "total_tokens", 0n), l,
        J.Json.get_str(a, "prompt", ""), J.Json.get_str(o, "summary", ""), ss}

def Subs.link(+id: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(id, '~')), "")

# a delegated or launched thread: the agent it runs, and on which machine
# when not this one (far.bend's ids hold a "~")
def Subs.machine(+st: M.State, +id: String) -> String:
  Bool.pick(String, String.contains(id, "~"), " on " ++ Subs.or(M.Nav.key(M.Nav.get(M.State.navs(st), "far~" ++ Subs.link(id)), "pinned"), Subs.link(id)), "")

def Subs.model.of(+pr: String, +mo: String) -> String:
  Bool.pick(String, String.eq(pr, "claude"), Subs.model.family(Bool.pick(String, String.starts_with(mo, "claude-"), String.drop(mo, 7n), mo)), mo)

# the agents' names (modelpick.bend's ModelPick.product, which subs.bend
# cannot import without a cycle through client.bend)
def Subs.product(+agent: String) -> String:
  match agent:
    case "claude": "Claude Code"
    case "codex": "Codex"
    case "grok": "Grok"
    case "muse": "Muse"
    case "antigravity": "Antigravity"
    case "opencode": "OpenCode"
    case "pi": "Pi"
    case "omp": "Oh My Pi"
    case _: ""

def Subs.thread.info(t: M.Thread, +st: M.State, +launched: Bool, live: Bool, +doing: String) -> SubInfo:
  match t:
    case M.Thread{+id, _, _, +created, +activity, _, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      SubInfo{Bool.pick(String, launched, "launched ", "") ++ Subs.or(Subs.or(Subs.product(pr), pr), "Backplane") ++ " thread" ++ Subs.machine(st, id), Subs.model.of(pr, mo), created, Bool.pick(Nat, live, 0n, activity), 0n, 0n, SubStep{"", ""}, "", "", Nil{}}

# digits as a Nat ("" or anything else: 0)
def Subs.num.go(s: String, acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{c, t}:
      Subs.num.go(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((Char.to_u32(c) - 48 : U32))))

def Subs.num(+s: String) -> Nat:
  Bool.pick(Nat, Bool.and(Bool.not(String.is_empty(s)), Nat.is_le(String.length(s), 12n)), Subs.num.go(s, 0n), 0n)

# the tokens a subagent at work has used so far (its Bg line), 0 for none
def Subs.used(ls: List<&2, String>, +call: String) -> Nat:
  match ls:
    case Nil{}:
      0n
    case +l <> t:
      Bool.pick(Nat, String.eq(M.Bg.call(l), call), Subs.num(M.Bg.tokens(l)), Subs.used(t, call))

def Subs.info.tokens(i: SubInfo, +live: Nat) -> SubInfo:
  match i:
    case SubInfo{w, m, si, u, n, +tk, l, a, o, ss}:
      SubInfo{w, m, si, u, n, Bool.pick(Nat, Nat.is_eq(tk, 0n), live, tk), l, a, o, ss}

def Subs.info.k(agent: Bool, +st: M.State, +th: String, r: SubRow) -> SubInfo:
  match agent r:
    case True{} SubRow{+key, _, _, +who, _, _, _, _, _, _}:
      Subs.info.tokens(Subs.info.walked(Subs.walk(st, th, key), who), Subs.used(M.Bg.of(st, th), key))
    case False{} SubRow{+key, _, _, +who, _, +doing, live, _, _, _}:
      Subs.thread.info(U.Ui.thread(st, key), st, String.starts_with(who, "launched"), live, doing)

def Subs.info(+st: M.State, +th: String, r: SubRow) -> SubInfo:
  match r:
    case SubRow{+key, +agent, ti, +who, wo, d, li, o, n, ss}:
      Bool.pick(SubInfo, Bool.or(String.starts_with(key, "subs.done:"), Bool.or(String.eq(who, "lock"), String.eq(who, "wake"))), SubInfo.none(),
        Subs.info.k(agent, st, th, SubRow{key, agent, ti, who, wo, d, li, o, n, ss}))

# "45s", "12m", "1h 4m", "3d"
def Subs.dur(+s: Nat) -> String:
  Bool.pick(String, Nat.is_lt(s, 60n), Nat.show(s) ++ "s",
    Bool.pick(String, Nat.is_lt(s, 3600n), Nat.show(Nat.div(s, 60n)) ++ "m",
      Bool.pick(String, Nat.is_lt(s, 86400n), Nat.show(Nat.div(s, 3600n)) ++ "h" ++ Bool.pick(String, Nat.is_eq(Nat.mod(Nat.div(s, 60n), 60n), 0n), "", " " ++ Nat.show(Nat.mod(Nat.div(s, 60n), 60n)) ++ "m"),
        Nat.show(Nat.div(s, 86400n)) ++ "d")))

# how long: at work so far, or what it took ("" with no start known). At
# work it is told in minutes: the hub's view clock moves in 30 s steps
# (core/viewclock.bend), so seconds would stand still or read 0s
def Subs.so_far(+s: Nat) -> String:
  Bool.pick(String, Nat.is_lt(s, 60n), "<1m", Subs.dur(s))

def Subs.took(+live: Bool, +since: Nat, +until: Nat, +now: Nat) -> String:
  Bool.pick(String, Nat.is_eq(since, 0n), "",
    Bool.pick(String, live, Subs.so_far(Nat.sub(now, since)), Bool.pick(String, Nat.is_eq(until, 0n), "", Subs.dur(Nat.sub(until, since)))))

# a whole and its tenth: "18.2", "240"
def Subs.tenths(+whole: Nat, +tenth: Nat) -> String:
  Nat.show(whole) ++ Bool.pick(String, Nat.is_eq(tenth, 0n), "", "." ++ Nat.show(tenth))

# "812 tokens", "18.2k tokens", "240k tokens", "1.4M tokens"
def Subs.tokens(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "",
    Bool.pick(String, Nat.is_lt(n, 1000n), Nat.show(n) ++ " tokens",
      Bool.pick(String, Nat.is_lt(n, 1000000n), Subs.tenths(Nat.div(n, 1000n), Nat.div(Nat.mod(n, 1000n), 100n)) ++ "k tokens",
        Subs.tenths(Nat.div(n, 1000000n), Nat.div(Nat.mod(n, 1000000n), 100000n)) ++ "M tokens")))

def Subs.steps.word(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " step", " steps"))
