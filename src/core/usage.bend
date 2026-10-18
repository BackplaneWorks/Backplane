import Base
import ./model.bend as M
import ./json.bend as J

# Usage
# -----
# What Settings > Usage says, the same rows in every client. A thread's
# reading (the setting usage.<thread>, from UsageSet) is how much of its
# model's context window the session holds now: a reading, never a total,
# so readings are listed, not summed. Only Claude reports one; a thread on
# another provider says so ("ctx n/a") instead of showing nothing.

type UseT is Data:
  UseT{id: String, title: String, provider: String, model: String, open: Bool, used: Nat, window: Nat}

# a row: its label, its note, and an action for a Refresh button ("" none)
type URow is Data:
  URow{label: String, note: String, act: String}

# whether a provider reports its context (only Claude's stream does)
def Use.reports(+provider: String) -> Bool:
  String.eq(provider, "claude")

def Use.pct(+used: Nat, +window: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(window, 0n), 0n, Nat.div(Nat.mul(used, 100n), window))

def Use.tokens(+n: Nat) -> String:
  Bool.pick(String, Nat.is_ge(n, 1000000n), Nat.show(Nat.div(n, 1000000n)) ++ "M",
    Bool.pick(String, Nat.is_ge(n, 1000n), Nat.show(Nat.div(n, 1000n)) ++ "k", Nat.show(n)))

def Use.tail.k(has: Bool, +provider: String, +text: String) -> String:
  match has:
    case True{}:
      text
    case False{}:
      Bool.pick(String, Use.reports(provider), "", " · ctx n/a")

# the model chip's tail: the reading's text, "ctx n/a" for a provider that
# never reports one, nothing before a reporting provider's first reading
def Use.tail(+provider: String, +used: Nat, +text: String) -> String:
  Use.tail.k(Nat.is_gt(used, 0n), provider, text)

def Use.of.k(t: M.Thread, +v: String) -> UseT:
  match t:
    case M.Thread{+id, _, +ti, _, _, _, _, ar, _, _, _, _, _, _, _, pr, mo, _, _, _, de}:
      UseT{id, Bool.pick(String, String.is_empty(ti), id, ti), pr, mo, Bool.not(Bool.or(ar, de)), M.Usage.used(v), M.Usage.window(v)}

def Use.of(+st: M.State, +t: M.Thread) -> UseT:
  Use.of.k(t, M.Setting.get(st, M.Usage.key(M.Thread.id(t)), ""))

def Use.all(+st: M.State, ts: List<&2, M.Thread>) -> List<&2, UseT>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      Use.of(st, t) <> Use.all(st, r)

def UseT.open(u: UseT) -> Bool:
  match u:
    case UseT{_, _, _, _, o, _, _}:
      o

def UseT.held(u: UseT) -> Bool:
  match u:
    case UseT{_, _, _, _, o, n, _}:
      Bool.and(o, Nat.is_gt(n, 0n))

def UseT.used(u: UseT) -> Nat:
  match u:
    case UseT{_, _, _, _, _, n, _}:
      n

# fuller first
def UseT.before(a: UseT, b: UseT) -> Bool:
  Nat.is_ge(UseT.used(a), UseT.used(b))

def Use.count(us: List<&2, UseT>, +acc: Nat) -> Nat:
  match us:
    case Nil{}:
      acc
    case _ <> r:
      Use.count(r, Nat.add(acc, 1n))

def Use.held.note(+used: Nat, +window: Nat) -> String:
  Bool.pick(String, Nat.is_eq(window, 0n), Use.tokens(used) ++ " held, window unknown",
    Use.tokens(used) ++ " of " ++ Use.tokens(window) ++ " held (" ++ Nat.show(Use.pct(used, window)) ++ "%)")

def Use.row(u: UseT) -> URow:
  match u:
    case UseT{_, ti, _, +mo, _, n, w}:
      URow{ti, Bool.pick(String, String.is_empty(mo), "default", mo) ++ " · " ++ Use.held.note(n, w), ""}

def Use.rows.t(us: List<&2, UseT>) -> List<&2, URow>:
  match us:
    case Nil{}:
      Nil{}
    case u <> r:
      Use.row(u) <> Use.rows.t(r)

def Use.summary(+held: Nat, +open: Nat) -> URow:
  URow{"Context now", Nat.show(held) ++ " of " ++ Nat.show(open) ++ " open threads have a reading. Only Claude reports one; others show ctx n/a.", ""}

# the fullest open threads' readings, at most `n`
def Use.top(+n: Nat, us: List<&2, UseT>) -> List<&2, URow>:
  Use.rows.t(List.take(&2, UseT, List.sort(~UseT, ~UseT.before, List.filter(~UseT, ~UseT.held, us)), n))

def Use.totals() -> URow:
  URow{"Tokens and cost", "Not recorded yet. Readings above are held now, never summed.", ""}

def Use.totals.if(none: Bool) -> List<&2, URow>:
  match none:
    case True{}:
      [Use.totals()]
    case False{}:
      Nil{}

def Use.rows.k(+st: M.State, +us: List<&2, UseT>) -> List<&2, URow>:
  Use.summary(Use.count(List.filter(~UseT, ~UseT.held, us), 0n), Use.count(List.filter(~UseT, ~UseT.open, us), 0n)) <>
    List.append(&2, URow, Use.top(8n, us), Use.totals.if(String.is_empty(M.Setting.get(st, M.Tok.pre(st) ++ "since", ""))))

# Settings > Usage, model side: a summary, the fullest threads, and what is
# not known
def Use.rows(+st: M.State, ts: List<&2, M.Thread>) -> List<&2, URow>:
  Use.rows.k(st, Use.all(st, ts))

def URow.label(r: URow) -> String:
  match r:
    case URow{l, _, _}:
      l

def URow.note(r: URow) -> String:
  match r:
    case URow{_, n, _}:
      n

def URow.act(r: URow) -> String:
  match r:
    case URow{_, _, a}:
      a

# Boat
# ----
# Boat's side of Settings > Usage. The account comes from Boat's CLI
# (`boat limits --json --no-update`, read-only), which the hub runs at most
# every Bj.every() seconds while Settings is open, or at once from Refresh
# (not again within Bj.again() seconds), and keeps as a cache: when it last
# tried, when it last got an answer, the error if the last try failed, and
# the last answer, so a failure shows the last known values as stale.
# What Backplane itself started comes from the Boat lead's job status
# (bp-coord/usage/DATA-CONTRACT.md); its ledger is the only one. The
# account's numbers (every sandbox on it) stay apart from Backplane's.
# Money is credit-seconds, $1 = 100,000; a reservation is a job's worst
# case, never a charge. A number Boat did not give shows "?", never 0.

def Bj.every() -> Nat:
  300n

def Bj.again() -> Nat:
  10n

# a number as Boat gave it, "" when absent or null
def Bj.rawd(+o: J.Json, +k: String) -> String:
  J.Json.as_raw(J.Json.get(o, k), "")

def Bj.raw.k(+r: String) -> String:
  Bool.pick(String, String.is_empty(r), "?", r)

# a number as Boat gave it, "?" when absent or null
def Bj.raw(+o: J.Json, +k: String) -> String:
  Bj.raw.k(Bj.rawd(o, k))

def Bj.has(+o: J.Json, +k: String) -> Bool:
  Bool.not(String.is_empty(Bj.rawd(o, k)))

def Bj.nat(+o: J.Json, +k: String) -> Nat:
  J.Json.get_nat(o, k, 0n)

def Bj.pad5.k(+s: String) -> String:
  String.repeat("0", Nat.sub(5n, String.length(s))) ++ s

def Bj.pad5(+s: String) -> String:
  Bool.pick(String, Nat.is_ge(String.length(s), 5n), s, Bj.pad5.k(s))

# credit-seconds as dollars, exact ($0.00158 for 158)
def Bj.usd(+cs: Nat) -> String:
  "$" ++ Nat.show(Nat.div(cs, 100000n)) ++ "." ++ Bj.pad5(Nat.show(Nat.mod(cs, 100000n)))

def Bj.usd.k(known: Bool, +cs: Nat) -> String:
  match known:
    case True{}:
      Bj.usd(cs)
    case False{}:
      "?"

def Bj.usd.of(+o: J.Json, +k: String) -> String:
  Bj.usd.k(Bj.has(o, k), Bj.nat(o, k))

# seconds of compute: minutes under an hour, else hours to a tenth
def Bj.hours(+s: Nat) -> String:
  Bool.pick(String, Nat.is_lt(s, 3600n), Nat.show(Nat.div(s, 60n)) ++ " min",
    Nat.show(Nat.div(s, 3600n)) ++ "." ++ Nat.show(Nat.div(Nat.mod(s, 3600n), 360n)) ++ " h")

def Bj.hours.k(known: Bool, +s: Nat) -> String:
  match known:
    case True{}:
      Bj.hours(s)
    case False{}:
      "?"

def Bj.hours.of(+o: J.Json, +k: String) -> String:
  Bj.hours.k(Bj.has(o, k), Bj.nat(o, k))

def Bj.ago(+now: Nat, +at: Nat) -> String:
  Bool.pick(String, Nat.is_lt(Nat.sub(now, at), 60n), "just now",
    Bool.pick(String, Nat.is_lt(Nat.sub(now, at), 3600n), Nat.show(Nat.div(Nat.sub(now, at), 60n)) ++ " min ago",
      Bool.pick(String, Nat.is_lt(Nat.sub(now, at), 86400n), Nat.show(Nat.div(Nat.sub(now, at), 3600n)) ++ " h ago",
        Nat.show(Nat.div(Nat.sub(now, at), 86400n)) ++ " d ago")))

# The hub's cache
# ---------------

def Bj.parse(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

# an answer worth keeping: an object with the account's limits in it
def Bj.valid(+o: J.Json) -> Bool:
  Bj.has(o, "maxActiveSandboxes")

def Bj.cache.k(ok: Bool, +old: J.Json, nw: J.Json, +now: Nat) -> String:
  match ok:
    case True{}:
      J.Json.show(J.Json.obj([J.KV{"at", J.Num{Nat.show(now)}}, J.KV{"ok_at", J.Num{Nat.show(now)}}, J.KV{"err", J.Str{""}}, J.KV{"limits", nw}]))
    case False{}:
      J.Json.show(J.Json.obj([J.KV{"at", J.Num{Nat.show(now)}}, J.KV{"ok_at", J.Num{Nat.show(Bj.nat(old, "ok_at"))}}, J.KV{"err", J.Str{"Boat's CLI did not answer"}},
        J.KV{"limits", J.Json.get_obj(old, "limits")}]))

# the cache after a try: the CLI's output when it is an answer, else the
# last answer kept, with the error and the try's time
def Bj.cache.next(+old: String, +out: String, +now: Nat) -> String:
  Bj.cache.k(Bj.valid(Bj.parse(out)), Bj.parse(old), Bj.parse(out), now)

# whether to ask Boat again: at once when it never was; Refresh (force)
# waits Bj.again() since the last try, an open Settings Bj.every()
def Bj.due(+old: String, force: Bool, +now: Nat) -> Bool:
  Bool.or(Nat.is_eq(Bj.nat(Bj.parse(old), "at"), 0n),
    Nat.is_ge(Nat.sub(now, Bj.nat(Bj.parse(old), "at")), Bool.pick(Nat, force, Bj.again(), Bj.every())))

# what clients get (info "boat.usage"): the cache and the job status
def Bj.info(+cache: String, +status: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"cache", Bj.parse(cache)}, J.KV{"status", Bj.parse(status)}]))

# Rows
# ----

# how fresh the account's numbers are
def Bj.fresh.k(+err: String, +ok: Nat, +now: Nat) -> String:
  Bool.pick(String, Nat.is_eq(ok, 0n), Bool.pick(String, String.is_empty(err), "", " · " ++ err),
    Bool.pick(String, String.is_empty(err),
      Bool.pick(String, Nat.is_gt(Nat.sub(now, ok), 900n), " · stale, read ", " · read ") ++ Bj.ago(now, ok),
      " · " ++ err ++ "; last read " ++ Bj.ago(now, ok)))

def Bj.fresh(+c: J.Json, +now: Nat) -> String:
  Bj.fresh.k(J.Json.get_str(c, "err", ""), Bj.nat(c, "ok_at"), now)

def Bj.renews(+l: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(l, "subscriptionCurrentPeriodEnd", "")), "",
    Bool.pick(String, J.Json.get_bool(l, "subscriptionCancelAtPeriodEnd", False{}), " · ends ", " · renews ") ++
      String.take(J.Json.get_str(l, "subscriptionCurrentPeriodEnd", ""), 10n))

def Bj.plan(+l: J.Json) -> String:
  J.Json.get_str(l, "activeSandboxPlanKey", J.Json.get_str(l, "sandboxPlanKey", "?")) ++ " · $" ++ Bj.raw(l, "sandboxPlanDollars") ++ "/month · " ++
    J.Json.get_str(l, "subscriptionStatus", J.Json.get_str(l, "billingStatus", "?")) ++ Bj.renews(l)

def Bj.mine(+s: J.Json) -> String:
  Bool.pick(String, Bj.has(s, "running"), Bj.raw(s, "running") ++ " running" ++
    Bool.pick(String, Nat.is_gt(Bj.nat(s, "orphans"), 0n), ", " ++ Bj.raw(s, "orphans") ++ " NOT DELETED", ""), "?")

def Bj.blocked(+l: J.Json) -> String:
  Bool.pick(String, J.Json.get_bool(l, "canStart", True{}), "", " · can't start now: " ++ J.Json.get_str(l, "startBlockedReason", "no reason given"))

def Bj.win(+o: J.Json, +name: String) -> String:
  Bj.raw(o, "used") ++ "/" ++ Bj.raw(o, "limit") ++ " " ++ name ++ " (" ++ Bj.raw(o, "remaining") ++ " left)"

def Bj.starts(+s: J.Json) -> String:
  Bj.win(J.Json.get_obj(s, "minute"), "this minute") ++ " · " ++ Bj.win(J.Json.get_obj(s, "hour"), "this hour") ++ " · " ++ Bj.win(J.Json.get_obj(s, "day"), "today")

def Bj.size.k(have: Bool, want: Bool, +t: String) -> String:
  match have want:
    case True{} True{}:
      t
    case False{} False{}:
      t
    case _ _:
      ""

def Bj.size(+m: J.Json, want: Bool) -> String:
  Bj.size.k(J.Json.get_bool(m, "available", False{}), want, J.Json.get_str(m, "type", "?"))

def Bj.join(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(b), a, Bool.pick(String, String.is_empty(a), b, a ++ ", " ++ b))

def Bj.sizes(ms: List<&2, J.Json>, +want: Bool, +acc: String) -> String:
  match ms:
    case Nil{}:
      acc
    case m <> t:
      Bj.sizes(t, want, Bj.join(acc, Bj.size(m, want)))

def Bj.sizes.all(+l: J.Json) -> String:
  Bj.sizes(J.Json.elems(J.Json.get_obj(l, "machineTypes")), True{}, "") ++
    Bool.pick(String, String.is_empty(Bj.sizes(J.Json.elems(J.Json.get_obj(l, "machineTypes")), False{}, "")), "",
      " · not offered: " ++ Bj.sizes(J.Json.elems(J.Json.get_obj(l, "machineTypes")), False{}, ""))

def Bj.sizes.row(+l: J.Json) -> String:
  Bj.raw.k(Bj.sizes.all(l))

# the account, as Boat's CLI last said it
def Bj.account(+l: J.Json, +s: J.Json) -> List<&2, URow>:
  [URow{"Boat sandboxes", Bj.raw(l, "activeSandboxes") ++ " of " ++ Bj.raw(l, "maxActiveSandboxes") ++ " active on the account · Backplane's: " ++ Bj.mine(s) ++ Bj.blocked(l), ""},
   URow{"Boat starts", "Rolling: " ++ Bj.starts(J.Json.get_obj(l, "starts")), ""},
   URow{"Boat compute", Bj.hours.of(l, "subscriptionRemainingSeconds") ++ " of " ++ Bj.hours.of(l, "subscriptionQuotaSeconds") ++ " left this period · " ++
     Bj.hours.of(l, "last24hUsageSeconds") ++ " used in the last 24 h", ""},
   URow{"Boat sizes", Bj.sizes.row(l), ""}]

def Bj.account.k(known: Bool, +l: J.Json, +s: J.Json) -> List<&2, URow>:
  match known:
    case True{}:
      Bj.account(l, s)
    case False{}:
      Nil{}

def Bj.plan.row(+c: J.Json, +now: Nat) -> URow:
  URow{"Boat plan", Bool.pick(String, Bj.valid(J.Json.get_obj(c, "limits")), Bj.plan(J.Json.get_obj(c, "limits")), "Not read yet") ++ Bj.fresh(c, now), "usage-refresh"}

# what Backplane's own jobs cost: today's and this month's actual spend,
# and what open jobs hold in reserve
# the shared daily cap and what is left of it today (spent and held both
# count against it), when the status names one; nothing without it
def Bj.cap.left(+b: J.Json, +t: J.Json) -> String:
  " · daily cap " ++ Bj.usd.of(b, "cs") ++ " (" ++ J.Json.get_str(b, "source", "?") ++ ") · " ++
    Bj.usd(Nat.sub(Bj.nat(b, "cs"), Nat.add(Bj.nat(t, "cost_cs"), Bj.nat(t, "reserved_open_cs")))) ++ " left"

def Bj.cap.of(+s: J.Json) -> String:
  Bool.pick(String, Bj.has(J.Json.get_obj(s, "budget"), "cs"), Bj.cap.left(J.Json.get_obj(s, "budget"), J.Json.get_obj(s, "today")), "")

def Bj.spend(+s: J.Json) -> URow:
  URow{"Boat for Backplane", "Today " ++ Bj.usd.of(J.Json.get_obj(s, "today"), "cost_cs") ++ Bj.cap.of(s) ++
    Bool.pick(String, Bj.has(J.Json.get_obj(s, "month"), "cost_cs"), " · this month " ++ Bj.usd.of(J.Json.get_obj(s, "month"), "cost_cs"), "") ++
    " · " ++ Bj.usd.of(J.Json.get_obj(s, "today"), "reserved_open_cs") ++ " held for open jobs (worst case, not spent)", ""}

def UseT.title(u: UseT) -> String:
  match u:
    case UseT{_, ti, _, _, _, _, _}:
      ti

def Bj.thread.k(r: Map<&2, M.Thread> & M.Thread) -> M.Thread:
  (_, t) = r
  t

def Bj.thread(s: M.State, +id: String) -> M.Thread:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Bj.thread.k(Map.get(M.Thread, M.Thread.blank(id), ts, id))

def Bj.who(+st: M.State, +th: String) -> String:
  Bool.pick(String, String.is_empty(th), "started by hand", UseT.title(Use.of(st, Bj.thread(st, th))))

# a job that needs a look, listed first and never left out: a sandbox whose
# delete was never verified (orphan), a create Boat answered unclearly
# (ambiguous: its slot and worst case stay held until it is reconciled), or
# one still unclear past that with look-alike sandboxes up (unresolved: new
# creates are refused until a person clears it). Reconciled ones are closed.
def Bj.warn(+s: String) -> Bool:
  Bool.or(String.eq(s, "orphan"), Bool.or(String.eq(s, "ambiguous"), String.eq(s, "unresolved")))

# a job running now (at most Bj.most() rows of these)
def Bj.shown(+s: String) -> Bool:
  Bool.or(String.eq(s, "running"), String.eq(s, "starting"))

# how long a job has run: a running one from its start to now (the status
# file's runtime is as of when it was written), else as the status says
def Bj.runtime(+j: J.Json, +now: Nat) -> String:
  Bool.pick(String, Bool.and(String.eq(J.Json.get_str(j, "state", ""), "running"), Bj.has(j, "started_at")),
    Nat.show(Nat.sub(now, Bj.nat(j, "started_at"))) ++ " s", Bj.raw(j, "runtime_s") ++ " s")

def Bj.label(+state: String) -> String:
  Bool.pick(String, String.eq(state, "orphan"), "Boat sandbox NOT deleted",
    Bool.pick(String, String.eq(state, "ambiguous"), "Boat create unclear",
      Bool.pick(String, String.eq(state, "unresolved"), "Boat create UNRESOLVED, creates blocked", "Boat job " ++ state)))

def Bj.unclear(+state: String) -> Bool:
  Bool.or(String.eq(state, "ambiguous"), String.eq(state, "unresolved"))

def Bj.job(+st: M.State, +j: J.Json, +now: Nat) -> URow:
  URow{Bj.label(J.Json.get_str(j, "state", "")) ++ " · " ++ J.Json.get_str(j, "size", "?"),
    Bj.who(st, J.Json.get_str(j, "thread", "")) ++ Bool.pick(String, String.is_empty(J.Json.get_str(j, "machine", "")), "", " on " ++ J.Json.get_str(j, "machine", "")) ++
      Bool.pick(String, Bj.unclear(J.Json.get_str(j, "state", "")), " · " ++ J.Json.get_str(j, "why", "no reason given") ++ " · check boat list",
        " · " ++ Bj.runtime(j, now) ++ " · reserved " ++ Bj.usd.of(j, "reserved_cs") ++ " (worst case)"), ""}

def Bj.jobs.k(keep: Bool, +st: M.State, +j: J.Json, +now: Nat, rest: List<&2, URow>) -> List<&2, URow>:
  match keep:
    case True{}:
      Bj.job(st, j, now) <> rest
    case False{}:
      rest

# which jobs: 0 those that need a look, 1 those running
def Bj.wants(+mode: Nat, +s: String) -> Bool:
  Bool.pick(Bool, Nat.is_eq(mode, 0n), Bj.warn(s), Bj.shown(s))

def Bj.jobs(js: List<&2, J.Json>, +mode: Nat, +st: M.State, +now: Nat) -> List<&2, URow>:
  match js:
    case Nil{}:
      Nil{}
    case +j <> t:
      Bj.jobs.k(Bj.wants(mode, J.Json.get_str(j, "state", "")), st, j, now, Bj.jobs(t, mode, st, now))

# at most Bj.most() job rows; the rest as one row saying how many
def Bj.most() -> Nat:
  6n

def Bj.cap(+rs: List<&2, URow>) -> List<&2, URow>:
  Bool.pick(List<&2, URow>, Nat.is_gt(List.length(&2, URow, rs), Bj.most()),
    List.append(&2, URow, List.take(&2, URow, rs, Bj.most()), [URow{"More Boat jobs", Nat.show(Nat.sub(List.length(&2, URow, rs), Bj.most())) ++ " more not listed", ""}]), rs)

# creates still unclear past their time (status "unresolved"), which block
# creates; and the other creates Boat answered unclearly (status
# "ambiguous" counts both, so these are ambiguous less unresolved): a
# sandbox may exist that Backplane does not own. Each job is said once.
def Bj.unresolved(+n: Nat, +blocked: Nat) -> List<&2, URow>:
  List.append(&2, URow,
    Bool.pick(List<&2, URow>, Nat.is_eq(blocked, 0n), Nil{}, [URow{"Boat creates blocked", "Unclear create unresolved, check boat list", ""}]),
    Bool.pick(List<&2, URow>, Nat.is_eq(Nat.sub(n, blocked), 0n), Nil{},
      [URow{"Boat warning", Nat.show(Nat.sub(n, blocked)) ++ Bool.pick(String, Nat.is_eq(Nat.sub(n, blocked), 1n), " Boat create", " Boat creates") ++ " unclear: check boat list", ""}]))

def Bj.mine.rows(known: Bool, +st: M.State, +s: J.Json, +now: Nat) -> List<&2, URow>:
  match known:
    case True{}:
      List.concat(&2, URow, [[Bj.spend(s)], Bj.unresolved(Bj.nat(s, "ambiguous"), Bj.nat(s, "unresolved")),
        Bj.jobs(J.Json.elems(J.Json.get_obj(s, "jobs")), 0n, st, now), Bj.cap(Bj.jobs(J.Json.elems(J.Json.get_obj(s, "jobs")), 1n, st, now))])
    case False{}:
      [URow{"Boat for Backplane", "No job status yet.", ""}]

def Bj.rows.k(+st: M.State, +c: J.Json, +s: J.Json, +now: Nat) -> List<&2, URow>:
  List.concat(&2, URow, [[Bj.plan.row(c, now)], Bj.account.k(Bj.valid(J.Json.get_obj(c, "limits")), J.Json.get_obj(c, "limits"), s),
    Bj.mine.rows(Bj.has(s, "running"), st, s, now)])

# Settings > Usage, Boat side, from the info "boat.usage" ("" before the
# hub has answered: one row with Refresh)
def Bj.rows(+st: M.State, +text: String, +now: Nat) -> List<&2, URow>:
  Bj.rows.k(st, J.Json.get_obj(Bj.parse(text), "cache"), J.Json.get_obj(Bj.parse(text), "status"), now)

# Tokens and plan limits
# ----------------------
# What turns used, as their providers reported it (model.bend's Tokens):
# today and this month (UTC), by model and by project this month, the
# threads that used most, and since when anything was counted. Claude's
# cost is what Claude reports (API prices), not a bill; Codex reports no
# cost, so its turns say so. Claude's plan windows (limit.claude.*) are the
# share Claude says is used, and when each resets.

def Tk.part(+v: String, +i: Nat) -> Nat:
  M.Tok.part(v, i)

def Tk.pad4(+s: String) -> String:
  Bool.pick(String, Nat.is_ge(String.length(s), 4n), s, String.repeat("0", Nat.sub(4n, String.length(s))) ++ s)

# nano-dollars as dollars: to the cent from $1, else to a hundredth of a cent
def Tk.usd(+c: Nat) -> String:
  Bool.pick(String, Nat.is_ge(c, 1000000000n),
    "$" ++ Nat.show(Nat.div(c, 1000000000n)) ++ "." ++ M.Tok.pad2(Nat.div(Nat.mod(c, 1000000000n), 10000000n)),
    "$0." ++ Tk.pad4(Nat.show(Nat.div(c, 100000n))))

# turns whose tokens are not in the total (a session already running when
# counting began: its share of the session could not be told)
def Tk.unknown(+v: String) -> String:
  Bool.pick(String, Nat.is_ge(Tk.part(v, 8n), Tk.part(v, 0n)), "",
    ", tokens unknown for " ++ Nat.show(Nat.sub(Tk.part(v, 0n), Tk.part(v, 8n))))

def Tk.cost(+v: String) -> String:
  Bool.pick(String, Nat.is_eq(Tk.part(v, 7n), 0n), "cost not reported",
    Tk.usd(Tk.part(v, 6n)) ++ Bool.pick(String, Nat.is_eq(Tk.part(v, 7n), Tk.part(v, 0n)), "", " for " ++ Nat.show(Tk.part(v, 7n)) ++ " of them"))

# a total: "12 turns · 40k in, 1M cache read, 20k cache write · 9k out (2k reasoning) · $0.4210"
def Tk.note(+v: String) -> String:
  Nat.show(Tk.part(v, 0n)) ++ Bool.pick(String, Nat.is_eq(Tk.part(v, 0n), 1n), " turn", " turns") ++ " · " ++
    Use.tokens(Tk.part(v, 1n)) ++ " in, " ++ Use.tokens(Tk.part(v, 2n)) ++ " cache read, " ++ Use.tokens(Tk.part(v, 3n)) ++ " cache write · " ++
    Use.tokens(Tk.part(v, 4n)) ++ " out (" ++ Use.tokens(Tk.part(v, 5n)) ++ " reasoning)" ++ Tk.unknown(v) ++ " · " ++ Tk.cost(v)

def Tk.row(+label: String, +v: String) -> List<&2, URow>:
  Bool.pick(List<&2, URow>, String.is_empty(v), Nil{}, [URow{label, Tk.note(v), ""}])

def Tk.models(ms: List<&2, String>, +st: M.State, +mon: String) -> List<&2, URow>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      List.append(&2, URow, Tk.row(String.join(String.split(m, '|'), " · ") ++ " this month", M.Setting.get(st, M.Tok.pre(st) ++ "mm." ++ mon ++ "|" ++ m, "")), Tk.models(t, st, mon))

def Tk.projects(ps: List<&2, M.Project>, +st: M.State, +mon: String) -> List<&2, URow>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, +ti, _, _, _} <> t:
      List.append(&2, URow, Tk.row(ti ++ " this month", M.Setting.get(st, M.Tok.pre(st) ++ "p." ++ id ++ "|" ++ mon, "")), Tk.projects(t, st, mon))

# a thread's total, with its title, for the threads that used most
type TkT is Data:
  TkT{title: String, v: String}

def TkT.n(t: TkT) -> Nat:
  match t:
    case TkT{_, +v}:
      Nat.add(Nat.add(Tk.part(v, 1n), Tk.part(v, 2n)), Nat.add(Tk.part(v, 3n), Tk.part(v, 4n)))

def TkT.before(a: TkT, b: TkT) -> Bool:
  Nat.is_ge(TkT.n(a), TkT.n(b))

def TkT.some(t: TkT) -> Bool:
  match t:
    case TkT{_, v}:
      Bool.not(String.is_empty(v))

def TkT.row(t: TkT) -> URow:
  match t:
    case TkT{ti, +v}:
      URow{ti ++ " so far", Tk.note(v), ""}

def Tk.threads.all(ts: List<&2, M.Thread>, +st: M.State) -> List<&2, TkT>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      TkT{UseT.title(Use.of(st, t)), M.Setting.get(st, M.Tok.pre(st) ++ "t." ++ M.Thread.id(t), "")} <> Tk.threads.all(r, st)

def Tk.threads.rows(ts: List<&2, TkT>) -> List<&2, URow>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      TkT.row(t) <> Tk.threads.rows(r)

def Tk.threads(ts: List<&2, M.Thread>, +st: M.State) -> List<&2, URow>:
  Tk.threads.rows(List.take(&2, TkT, List.sort(~TkT, ~TkT.before, List.filter(~TkT, ~TkT.some, Tk.threads.all(ts, st))), 5n))

def Tk.since(+st: M.State) -> List<&2, URow>:
  Bool.pick(List<&2, URow>, String.is_empty(M.Setting.get(st, M.Tok.pre(st) ++ "since", "")), Nil{},
    [URow{"Counted since", M.Tok.day(M.Switch.num(M.Setting.get(st, M.Tok.pre(st) ++ "since", ""), 0n)) ++ " (UTC); earlier turns were not recorded. Costs are Claude's own figures at API prices, not a bill; Codex reports none.", ""}])

# a time from now: "resets in 2 h 10 min", "resets in 3 d"
def Tk.until(+now: Nat, +at: Nat) -> String:
  Bool.pick(String, Nat.is_ge(now, at), "reset since, not read again",
    Bool.pick(String, Nat.is_lt(Nat.sub(at, now), 3600n), "resets in " ++ Nat.show(Nat.div(Nat.sub(at, now), 60n)) ++ " min",
      Bool.pick(String, Nat.is_lt(Nat.sub(at, now), 86400n), "resets in " ++ Nat.show(Nat.div(Nat.sub(at, now), 3600n)) ++ " h " ++ Nat.show(Nat.div(Nat.mod(Nat.sub(at, now), 3600n), 60n)) ++ " min",
        "resets in " ++ Nat.show(Nat.div(Nat.sub(at, now), 86400n)) ++ " d")))

def Tk.window(+name: String, +v: String, +now: Nat) -> String:
  Bool.pick(String, String.is_empty(v), name ++ " ?",
    name ++ " " ++ Nat.show(Nat.div(Tk.part(v, 0n), 10n)) ++ "% used, " ++ Tk.until(now, Tk.part(v, 1n)))

def Tk.nth(ps: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, ps, i)), "")

def Tk.limits(+st: M.State, +now: Nat) -> List<&2, URow>:
  Bool.pick(List<&2, URow>, Bool.and(String.is_empty(M.Setting.get(st, "limit.claude.five_hour", "")), String.is_empty(M.Setting.get(st, "limit.claude.seven_day", ""))), Nil{},
    [URow{"Claude plan", Tk.window("5 h:", M.Setting.get(st, "limit.claude.five_hour", ""), now) ++ " · " ++ Tk.window("7 d:", M.Setting.get(st, "limit.claude.seven_day", ""), now) ++
      " · " ++ Bj.raw.k(Tk.nth(String.split(M.Setting.get(st, "limit.claude.five_hour", ""), '/'), 2n)) ++ " (as Claude last said)", ""}])

# Settings > Usage, tokens side
def Tk.rows(+st: M.State, ts: List<&2, M.Thread>, ps: List<&2, M.Project>, +now: Nat) -> List<&2, URow>:
  List.concat(&2, URow, [Tk.limits(st, now), Tk.row("Today (UTC)", M.Setting.get(st, M.Tok.pre(st) ++ "d." ++ M.Tok.day(now), "")),
    Tk.row("This month", M.Setting.get(st, M.Tok.pre(st) ++ "m." ++ M.Tok.month(now), "")),
    Tk.models(String.split(M.Setting.get(st, M.Tok.pre(st) ++ "ml." ++ M.Tok.month(now), ""), '\n'), st, M.Tok.month(now)),
    Tk.projects(ps, st, M.Tok.month(now)), Tk.threads(ts, st), Tk.since(st)])

# A thread's Boat chip
# --------------------
# Whether a thread has a Boat sandbox now, in its header ("Boat · large ·
# running", "Boat · warm 12m"), from the Boat lead's status.json "threads".
# The hub keeps it as the setting boat.thread.<id> =
# "<state>/<size>/<jobs>/<idle until>" and logs it only when it changes
# (Chip.changes, against the state, so a restart leaves nothing stale);
# clients only format it.

# where the Boat runner keeps its status
def Chip.path(+xs: String, +user: String) -> String:
  Bool.pick(String, String.is_empty(xs), user ++ "/.local/state", xs) ++ "/backplane/boat/status.json"

def Chip.key(+th: String) -> String:
  "boat.thread." ++ th

# whether a starting or running sandbox's news is stale: a running job
# rewrites status.json every 60 s, so older than 3 minutes means its runner
# stopped telling (a warm one goes quiet by right, and ends by idle_until)
def Chip.stale(+state: String, +gen: Nat, +now: Nat) -> Bool:
  Bool.and(Bool.not(String.eq(state, "warm")), Nat.is_gt(Nat.sub(now, gen), 180n))

def Chip.state(+state: String, +gen: Nat, +now: Nat) -> String:
  Bool.pick(String, Chip.stale(state, gen, now), "stale", state)

# a thread's entry in status.json's "threads" as the setting's value
def Chip.value(+e: J.Json, +gen: Nat, +now: Nat) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(e, "state", "")), "",
    Chip.state(J.Json.get_str(e, "state", ""), gen, now) ++ "/" ++ J.Json.get_str(e, "size", "?") ++ "/" ++ Nat.show(J.Json.get_nat(e, "jobs", 1n)) ++ "/" ++ Nat.show(J.Json.get_nat(e, "idle_until", 0n)))

def Chip.one(+th: String, +now: Nat, +cur: String, +nw: String) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.eq(cur, nw), Nil{}, [M.SettingSet{Chip.key(th), nw, now}])

def Chip.go(ts: List<&2, M.Thread>, +st: M.State, +threads: J.Json, +gen: Nat, +now: Nat) -> List<&2, M.Change>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      List.append(&2, M.Change, Chip.one(M.Thread.id(t), now, M.Setting.get(st, Chip.key(M.Thread.id(t)), ""), Chip.value(J.Json.get_obj(threads, M.Thread.id(t)), gen, now)),
        Chip.go(r, st, threads, gen, now))

def Chip.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Chip.changes.k(ok: Bool, +st: M.State, +o: J.Json, +now: Nat) -> List<&2, M.Change>:
  match ok:
    case True{}:
      Chip.go(Chip.threads(st), st, J.Json.get_obj(o, "threads"), Bj.nat(o, "generated_at"), now)
    case False{}:
      Nil{}

# what the status changes in the threads' chips (an unreadable status
# changes nothing; a thread it no longer lists loses its chip)
def Chip.changes(+st: M.State, +status: String, +now: Nat) -> List<&2, M.Change>:
  Chip.changes.k(Bj.has(Bj.parse(status), "generated_at"), st, Bj.parse(status), now)

def Chip.part(ps: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, ps, i)), "")

def Chip.text.w(+state: String, +size: String, +jobs: Nat, +until: Nat, +now: Nat) -> String:
  Bool.pick(String, String.eq(state, "warm"),
    Bool.pick(String, Nat.is_gt(until, now), "Boat · warm " ++ Nat.show(Nat.div(Nat.add(Nat.sub(until, now), 59n), 60n)) ++ "m", ""),
    "Boat · " ++ size ++ " · " ++ Bool.pick(String, Nat.is_gt(jobs, 1n), Nat.show(jobs) ++ " ", "") ++ state)

def Chip.text.k(+state: String, +size: String, +jobs: Nat, +until: Nat, +now: Nat) -> String:
  Bool.pick(String, String.eq(state, "stale"), "Boat · ? (stale)", Chip.text.w(state, size, jobs, until, now))

# the chip's text ("" for none, or a warm sandbox already gone: never "0m")
def Chip.text(+v: String, +now: Nat) -> String:
  Bool.pick(String, String.is_empty(v), "",
    Chip.text.k(Chip.part(String.split(v, '/'), 0n), Chip.part(String.split(v, '/'), 1n), M.Switch.num(Chip.part(String.split(v, '/'), 2n), 0n),
      M.Switch.num(Chip.part(String.split(v, '/'), 3n), 0n), now))

def Chip.of(+st: M.State, +th: String, +now: Nat) -> String:
  Chip.text(M.Setting.get(st, Chip.key(th), ""), now)
