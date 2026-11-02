import Base
import ./model.bend as M
import ./usage.bend as UZ

# Presentation over the existing TokensSet rollups, never transcript history.
# Rolling periods sum UTC daily counters; model breakdowns exist only by month.
# Reasoning is already part of output. A missing reported cost is unknown.

type UpControl is Data:
  UpControl{label: String, action: String, value: String, selected: Bool}

# Keep "month" as the existing calendar view; new/default ranges roll daily.
def Up.period(+s: String) -> String:
  Bool.pick(String, M.Tok.has(["today", "week", "2weeks", "4weeks", "30days", "month"], s), s, "30days")

def Up.metric(+s: String) -> String:
  Bool.pick(String, String.eq(s, "cost"), "cost", Bool.pick(String, String.eq(s, "limits"), "limits", "tokens"))

def Up.period.label(+period: String) -> String:
  Bool.pick(String, String.eq(period, "today"), "Today (UTC)",
    Bool.pick(String, String.eq(period, "week"), "1 week",
      Bool.pick(String, String.eq(period, "2weeks"), "2 weeks",
        Bool.pick(String, String.eq(period, "4weeks"), "4 weeks",
          Bool.pick(String, String.eq(period, "month"), "This calendar month (UTC)", "Month (30 days)")))))

def Up.periods() -> List<&2, String>:
  ["today", "week", "2weeks", "4weeks", "30days", "month"]

def Up.period.controls(ps: List<&2, String>, +period: String) -> List<&2, UpControl>:
  match ps:
    case Nil{}: Nil{}
    case +p <> rest: UpControl{Up.period.label(p), "usage-period", p, String.eq(Up.period(period), p)} <> Up.period.controls(rest, period)

def Up.period.size() -> Nat:
  List.length(&2, String, Up.periods())

def Up.controls(+period: String, +metric: String) -> List<&2, UpControl>:
  List.append(&2, UpControl, Up.period.controls(Up.periods(), period),
    [UpControl{"Tokens", "usage-metric", "tokens", String.eq(Up.metric(metric), "tokens")},
     UpControl{"Reported cost", "usage-metric", "cost", String.eq(Up.metric(metric), "cost")},
     UpControl{"Limits", "usage-metric", "limits", String.eq(Up.metric(metric), "limits")}])

def Up.month.offset(now: Nat) -> Nat:
  Nat.sub(M.Switch.num(String.drop(M.Tok.day(now), 8n), 0n), 1n)

def Up.offset(+now: Nat, +period: String) -> Nat:
  Bool.pick(Nat, String.eq(period, "today"), 0n,
    Bool.pick(Nat, String.eq(period, "week"), 6n,
      Bool.pick(Nat, String.eq(period, "2weeks"), 13n,
        Bool.pick(Nat, String.eq(period, "4weeks"), 27n,
          Bool.pick(Nat, String.eq(period, "month"), Up.month.offset(now), 29n)))))

def Up.start(+now: Nat, period: String) -> Nat:
  Nat.sub(Nat.mul(Nat.div(now, 86400n), 86400n), Nat.mul(Up.offset(now, Up.period(period)), 86400n))

def Up.count(+now: Nat, +period: String) -> Nat:
  Nat.add(Nat.div(Nat.sub(now, Up.start(now, period)), 86400n), 1n)

def Up.range(+now: Nat, period: String) -> String:
  M.Tok.day(Up.start(now, period)) ++ " – " ++ M.Tok.day(now) ++ " (UTC)"

# Add all nine counters, including coverage, without inventing a reading for
# an empty range. A sum of daily rollups is not another single turn.
def Up.sum.parts(n: Nat, +i: Nat, +a: String, +b: String) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: Nat.show(Nat.add(M.Tok.part(a, i), M.Tok.part(b, i))) <> Up.sum.parts(k, Nat.add(i, 1n), a, b)

def Up.sum.k(empty: Bool, +a: String, +b: String) -> String:
  match empty:
    case True{}: ""
    case False{}: String.join(Up.sum.parts(9n, 0n, a, b), "/")

def Up.sum(+a: String, +b: String) -> String:
  Up.sum.k(Bool.and(String.is_empty(a), String.is_empty(b)), a, b)

def Up.total.days(n: Nat, +at: Nat, +st: M.State, acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: Up.total.days(k, Nat.add(at, 86400n), st, Up.sum(acc, M.Setting.get(st, M.Tok.pre(st) ++ "d." ++ M.Tok.day(at), "")))

def Up.total.k(calendar: Bool, +st: M.State, +now: Nat, +period: String) -> String:
  match calendar:
    case True{}: M.Setting.get(st, M.Tok.pre(st) ++ "m." ++ M.Tok.month(now), "")
    case False{}: Up.total.days(Up.count(now, period), Up.start(now, period), st, "")

def Up.total(+st: M.State, +now: Nat, +period: String) -> String:
  Up.total.k(String.eq(Up.period(period), "month"), st, now, period)

def Up.tokens(+v: String) -> Nat:
  Nat.add(Nat.add(M.Tok.part(v, 1n), M.Tok.part(v, 2n)), Nat.add(M.Tok.part(v, 3n), M.Tok.part(v, 4n)))

def Up.coverage(+v: String) -> String:
  Nat.show(M.Tok.part(v, 8n)) ++ " of " ++ Nat.show(M.Tok.part(v, 0n)) ++ " recorded turns supplied token readings. Missing token fields remain unknown."

def Up.usd(+n: Nat) -> String:
  Bool.pick(String, Bool.and(Nat.is_gt(n, 0n), Nat.is_lt(n, 100000n)), "<$0.0001", UZ.Tk.usd(n))

def Up.cost(+v: String) -> String:
  Bool.pick(String, Nat.is_eq(M.Tok.part(v, 7n), 0n), "Unknown: no turn reported a cost.",
    Up.usd(M.Tok.part(v, 6n)) ++ " reported for " ++ Nat.show(M.Tok.part(v, 7n)) ++ " of " ++ Nat.show(M.Tok.part(v, 0n)) ++ " turns" ++
    Bool.pick(String, Nat.is_eq(M.Tok.part(v, 7n), M.Tok.part(v, 0n)), ".", "; remaining costs unknown."))

def Up.summary(+metric: String, +v: String) -> String:
  Bool.pick(String, String.eq(Up.metric(metric), "cost"), Up.cost(v),
    UZ.Use.tokens(Up.tokens(v)) ++ " recorded tokens · " ++ Nat.show(M.Tok.part(v, 0n)) ++ " turns · " ++ Up.coverage(v))

def Up.token.rows(+v: String) -> List<&2, UZ.URow>:
  [UZ.URow{"Recorded tokens", UZ.Use.tokens(Up.tokens(v)) ++ " total · " ++ Up.coverage(v), ""},
   UZ.URow{"Input", UZ.Use.tokens(M.Tok.part(v, 1n)) ++ " recorded uncached input tokens", ""},
   UZ.URow{"Cache", UZ.Use.tokens(M.Tok.part(v, 2n)) ++ " recorded cache read · " ++ UZ.Use.tokens(M.Tok.part(v, 3n)) ++ " recorded cache write", ""},
   UZ.URow{"Output", UZ.Use.tokens(M.Tok.part(v, 4n)) ++ " recorded output · " ++ UZ.Use.tokens(M.Tok.part(v, 5n)) ++ " recorded reasoning (included in output)", ""}]

def Up.measured.rows(cost: Bool, +v: String) -> List<&2, UZ.URow>:
  match cost:
    case True{}: [UZ.URow{"Reported cost", Up.cost(v), ""}]
    case False{}: Up.token.rows(v)

def Up.readings(have: Bool, +metric: String, +v: String) -> List<&2, UZ.URow>:
  match have:
    case True{}: Up.measured.rows(String.eq(Up.metric(metric), "cost"), v)
    case False{}: [UZ.URow{"Recorded usage", "No recorded turns in this period. Earlier or unreported usage is unknown.", ""}]

def Up.since(+st: M.State) -> String:
  M.Setting.get(st, M.Tok.pre(st) ++ "since", "")

def Up.scope(+st: M.State) -> UZ.URow:
  UZ.URow{"Coverage", Bool.pick(String, String.is_empty(Up.since(st)), "Counting has not started.",
    "Counted since " ++ M.Tok.day(M.Switch.num(Up.since(st), 0n)) ++ " (UTC).") ++
    " Only recorded Backplane turns on this machine; turns without usage events are absent. No subscription bill or estimated prices. Zero recorded tokens does not prove a missing field was zero.", ""}

# Return at most twelve rows per breakdown page. Projects and model IDs
# come from the catalog and the current month's rollup index, never entries.
def Up.page.size() -> Nat:
  12n

def Up.models.go(ms: List<&2, String>, +st: M.State, +month: String, +metric: String) -> List<&2, UZ.URow>:
  match ms:
    case Nil{}: Nil{}
    case +m <> rest:
      UZ.URow{String.join(String.split(m, '|'), " · "), Up.summary(metric, M.Setting.get(st, M.Tok.pre(st) ++ "mm." ++ month ++ "|" ++ m, "")), ""} <>
        Up.models.go(rest, st, month, metric)

def Up.model.ids(+st: M.State, +now: Nat) -> List<&2, String>:
  List.filter(~String, ~x => Bool.not(String.is_empty(x)), String.split(M.Setting.get(st, M.Tok.pre(st) ++ "ml." ++ M.Tok.month(now), ""), '\n'))

def Up.models(+st: M.State, +now: Nat, +metric: String, +page: Nat) -> List<&2, UZ.URow>:
  Up.models.go(List.take(&2, String, List.drop(&2, String, Up.model.ids(st, now), Nat.mul(page, Up.page.size())), Up.page.size()), st, M.Tok.month(now), metric)

type UpProject is Data:
  UpProject{title: String, total: String}

def Up.project.add(have: Bool, +title: String, +total: String, rest: List<&2, UpProject>) -> List<&2, UpProject>:
  match have:
    case True{}: UpProject{title, total} <> rest
    case False{}: rest

def Up.projects.all(ps: List<&2, M.Project>, +st: M.State, +month: String) -> List<&2, UpProject>:
  match ps:
    case Nil{}: Nil{}
    case M.Project{+id, title, _, _, _} <> rest:
      +v = M.Setting.get(st, M.Tok.pre(st) ++ "p." ++ id ++ "|" ++ month, "")
      Up.project.add(Bool.not(String.is_empty(v)), title, v, Up.projects.all(rest, st, month))

def Up.projects.go(ps: List<&2, UpProject>, +metric: String) -> List<&2, UZ.URow>:
  match ps:
    case Nil{}: Nil{}
    case UpProject{title, total} <> rest:
      UZ.URow{title, Up.summary(metric, total), ""} <> Up.projects.go(rest, metric)

def Up.projects(+st: M.State, ps: List<&2, M.Project>, +now: Nat, +metric: String, +page: Nat) -> List<&2, UZ.URow>:
  Up.projects.go(List.take(&2, UpProject, List.drop(&2, UpProject, Up.projects.all(ps, st, M.Tok.month(now)), Nat.mul(page, Up.page.size())), Up.page.size()), metric)

def Up.more(+count: Nat, +page: Nat) -> Bool:
  Nat.is_gt(count, Nat.mul(Nat.add(page, 1n), Up.page.size()))

def Up.pager(+name: String, +action: String, +page: Nat, more: Bool) -> List<&2, UpControl>:
  List.append(&2, UpControl,
    Bool.pick(List<&2, UpControl>, Nat.is_gt(page, 0n), [UpControl{"Previous " ++ name, action, "previous", False{}}], Nil{}),
    Bool.pick(List<&2, UpControl>, more, [UpControl{"Next " ++ name, action, "next", False{}}], Nil{}))

def Up.pages(+st: M.State, ps: List<&2, M.Project>, +now: Nat, +modelpage: Nat, +projectpage: Nat) -> List<&2, UpControl>:
  List.append(&2, UpControl,
    Up.pager("models", "usage-model-page", modelpage, Up.more(List.length(&2, String, Up.model.ids(st, now)), modelpage)),
    Up.pager("projects", "usage-project-page", projectpage, Up.more(List.length(&2, UpProject, Up.projects.all(ps, st, M.Tok.month(now))), projectpage)))

def Up.breakdown(month: Bool, +st: M.State, ps: List<&2, M.Project>, +now: Nat, +metric: String, +modelpage: Nat, +projectpage: Nat) -> List<&2, UZ.URow>:
  match month:
    case True{}:
      List.concat(&2, UZ.URow, [[UZ.URow{"Models · " ++ M.Tok.month(now), "Provider and model · page " ++ Nat.show(Nat.add(modelpage, 1n)) ++ " · up to 12 rows", ""}],
        Up.models(st, now, metric, modelpage), [UZ.URow{"Projects · " ++ M.Tok.month(now), "Page " ++ Nat.show(Nat.add(projectpage, 1n)) ++ " · up to 12 rows", ""}],
        Up.projects(st, ps, now, metric, projectpage)])
    case False{}:
      [UZ.URow{"Breakdown", "Model and project breakdowns are recorded by calendar month. Select This calendar month to view them. Rolling project token totals appear above.", ""}]

def Up.limits(+st: M.State, +now: Nat) -> List<&2, UZ.URow>:
  Bool.pick(List<&2, UZ.URow>, List.is_empty(&2, UZ.URow, UZ.Tk.limits(st, now)),
    [UZ.URow{"Plan limits", "No plan limits reported on this machine. Unknown, not unlimited.", ""}], UZ.Tk.limits(st, now))

def Up.rows.k(limits: Bool, +st: M.State, ps: List<&2, M.Project>, +now: Nat, +period: String, +metric: String, +modelpage: Nat, +projectpage: Nat) -> List<&2, UZ.URow>:
  match limits:
    case True{}: Up.limits(st, now)
    case False{}:
      List.concat(&2, UZ.URow, [[UZ.URow{Up.period.label(Up.period(period)),
        Up.range(now, period), ""}],
        Up.readings(Bool.not(String.is_empty(Up.total(st, now, period))), metric, Up.total(st, now, period)), [Up.scope(st)],
        Up.breakdown(String.eq(Up.period(period), "month"), st, ps, now, metric, modelpage, projectpage)])

def Up.rows(+st: M.State, ps: List<&2, M.Project>, +now: Nat, +period: String, +metric: String, +modelpage: Nat, +projectpage: Nat) -> List<&2, UZ.URow>:
  Up.rows.k(String.eq(Up.metric(metric), "limits"), st, ps, now, period, metric, modelpage, projectpage)
