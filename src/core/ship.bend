import Base
import ./json.bend as J

# Deploy main (docs/deploy.md, "Deploying origin/main"): the hub builds
# origin/main of the source checkout named by the setting deploy.repo and
# stages it through the safe restart (core/deploy.bend). Pure: what git says
# (origin/main's commit, which commits contain it) comes in as text the
# server read; the state is the setting deploy.ship, one JSON line, folded
# from the events DeployAsked and DeployBuilt (model.bend).
#
# Phases: idle; asked (a follow-up build waits for origin/main to be read);
# building C; staged C (built, the restart waits); live C; failed C.
#
# - A request for T is skipped when T is covered by the commit being built,
#   the one staged or the one live: T is that commit or an ancestor of it
#   (laws ship_covered_*).
# - One build at a time (law ship_busy_never_builds).
# - A request during a build that the build does not cover makes one
#   follow-up, of the newest origin/main, however many came (laws
#   ship_coalesce_*).
# - Everyone who asked since the last report is told the outcome once (laws
#   ship_told_*).
# - deploy.follow is off unless set "on" (law ship_follow_off_default).

type Ship is Data:
  Ship{phase: String, commit: String, staged: String, live: String, job: String, askers: List<&2, String>, next: List<&2, String>, more: Bool}

# the asker who is no thread (the person, a script on loopback)
def Ship.person() -> String:
  "-"

def Ship.idle() -> Ship:
  Ship{"idle", "", "", "", "", Nil{}, Nil{}, False{}}

def Ship.phase(+sh: Ship) -> String:
  match sh:
    case Ship{p, _, _, _, _, _, _, _}:
      p

def Ship.commit(+sh: Ship) -> String:
  match sh:
    case Ship{_, c, _, _, _, _, _, _}:
      c

def Ship.staged(+sh: Ship) -> String:
  match sh:
    case Ship{_, _, s, _, _, _, _, _}:
      s

def Ship.live(+sh: Ship) -> String:
  match sh:
    case Ship{_, _, _, l, _, _, _, _}:
      l

def Ship.job(+sh: Ship) -> String:
  match sh:
    case Ship{_, _, _, _, j, _, _, _}:
      j

def Ship.askers(+sh: Ship) -> List<&2, String>:
  match sh:
    case Ship{_, _, _, _, _, a, _, _}:
      a

def Ship.next(+sh: Ship) -> List<&2, String>:
  match sh:
    case Ship{_, _, _, _, _, _, n, _}:
      n

def Ship.more(+sh: Ship) -> Bool:
  match sh:
    case Ship{_, _, _, _, _, _, _, m}:
      m

# The line
# --------

def Ship.ids(+s: String) -> List<&2, String>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{_, _}:
      String.split(s, ',')

def Ship.show(+sh: Ship) -> String:
  match sh:
    case Ship{p, c, s, l, j, a, n, m}:
      J.Json.show(J.Json.obj([J.KV{"phase", J.Str{p}}, J.KV{"commit", J.Str{c}}, J.KV{"staged", J.Str{s}}, J.KV{"live", J.Str{l}},
        J.KV{"job", J.Str{j}}, J.KV{"askers", J.Str{String.join(a, ",")}}, J.KV{"next", J.Str{String.join(n, ",")}}, J.KV{"more", J.Flag{m}}]))

def Ship.of(+o: J.Json) -> Ship:
  Ship{J.Json.get_str(o, "phase", "idle"), J.Json.get_str(o, "commit", ""), J.Json.get_str(o, "staged", ""), J.Json.get_str(o, "live", ""),
    J.Json.get_str(o, "job", ""), Ship.ids(J.Json.get_str(o, "askers", "")), Ship.ids(J.Json.get_str(o, "next", "")), J.Json.get_bool(o, "more", False{})}

def Ship.read.k(m: Maybe<&2, J.Json>) -> Ship:
  match m:
    case Some{+o}:
      Ship.of(o)
    case None{}:
      Ship.idle()

# the setting's line ("" is idle)
def Ship.read(+line: String) -> Ship:
  Ship.read.k(J.Json.parse(line))

# Deciding
# --------

def Ship.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +y <> t:
      Bool.or(String.eq(y, x), Ship.has(t, x))

# an asker added once
def Ship.add.k(has: Bool, xs: List<&2, String>, +x: String) -> List<&2, String>:
  match has:
    case True{}:
      xs
    case False{}:
      List.append(&2, String, xs, [x])

def Ship.add(+xs: List<&2, String>, +x: String) -> List<&2, String>:
  Ship.add.k(Ship.has(xs, x), xs, x)

# a build is on its way: asked (its commit not read yet) or building
def Ship.busy.p(+phase: String) -> Bool:
  Bool.or(String.eq(phase, "asked"), String.eq(phase, "building"))

def Ship.busy(+sh: Ship) -> Bool:
  Ship.busy.p(Ship.phase(sh))

# commit c holds head: c is head, or head is an ancestor of c (within: the
# commits git says contain head)
def Ship.holds(+c: String, +head: String, within: List<&2, String>) -> Bool:
  Bool.and(Bool.not(String.is_empty(c)), Bool.or(String.eq(c, head), Ship.has(within, c)))

def Ship.cover.k(b: Bool, s: Bool, l: Bool) -> String:
  match b s l:
    case True{} _ _:
      "building"
    case False{} True{} _:
      "staged"
    case False{} False{} True{}:
      "live"
    case False{} False{} False{}:
      ""

# what covers a request for head: "building", "staged", "live" or ""
def Ship.cover(+sh: Ship, +head: String, +within: List<&2, String>) -> String:
  match sh:
    case Ship{+p, +c, +s, +l, _, _, _, _}:
      Ship.cover.k(Bool.and(String.eq(p, "building"), Ship.holds(c, head, within)), Ship.holds(s, head, within), Ship.holds(l, head, within))

# the covering commit for a cover word
def Ship.by(+sh: Ship, +cover: String) -> String:
  match sh:
    case Ship{_, +c, +s, +l, _, _, _, _}:
      Bool.pick(String, String.eq(cover, "building"), c, Bool.pick(String, String.eq(cover, "staged"), s, Bool.pick(String, String.eq(cover, "live"), l, "")))

# a build starts only when nothing is on its way and nothing covers it
def Ship.start.k(+cover: String, busy: Bool) -> Bool:
  match busy:
    case True{}:
      False{}
    case False{}:
      String.is_empty(cover)

def Ship.start(+sh: Ship, +head: String, +within: List<&2, String>) -> Bool:
  Ship.start.k(Ship.cover(sh, head, within), Ship.busy(sh))

# the staged commit forgotten (no deploy waits for it any more: cancelled)
def Ship.unstage(+sh: Ship) -> Ship:
  match sh:
    case Ship{p, c, _, l, j, a, n, m}:
      Ship{p, c, "", l, j, a, n, m}

def Ship.cand(+c: String, rest: List<&2, String>) -> List<&2, String>:
  match c:
    case SNil{}:
      rest
    case SCon{_, _}:
      c <> rest

# the commits a look asks git about: the one building, staged and live
def Ship.cands(+sh: Ship) -> List<&2, String>:
  match sh:
    case Ship{+p, c, s, l, _, _, _, _}:
      Ship.cand(Bool.pick(String, String.eq(p, "building"), c, ""), Ship.cand(s, Ship.cand(l, Nil{})))

# Folding
# -------

def Ship.asked.n(busy: Bool, +sh: Ship, +by: String) -> Ship:
  match busy sh:
    case True{} Ship{p, c, s, l, j, a, +n, _}:
      Ship{p, c, s, l, j, a, Ship.add(n, by), True{}}
    case False{} Ship{p, c, s, l, j, +a, n, m}:
      Ship{p, c, s, l, j, Ship.add(a, by), n, m}

def Ship.pos(+cover: String) -> Nat:
  Bool.pick(Nat, String.eq(cover, "building"), 0n, Bool.pick(Nat, String.eq(cover, "staged"), 1n, Bool.pick(Nat, String.eq(cover, "live"), 2n, 3n)))

def Ship.join(+sh: Ship, +by: String) -> Ship:
  match sh:
    case Ship{p, c, s, l, j, +a, n, m}:
      Ship{p, c, s, l, j, Ship.add(a, by), n, m}

def Ship.asked.k(i: Nat, +sh: Ship, +by: String) -> Ship:
  match i:
    case 0n:
      Ship.join(sh, by)
    case 1n:
      sh
    case 2n:
      sh
    case _:
      Ship.asked.n(Ship.busy(sh), sh, by)

# a request (cover: what covered it, "" for none): covered by the build it
# joins those told at its end; uncovered during a build it waits for the
# one follow-up; else it is told at the end of the build it starts
def Ship.asked(+sh: Ship, +by: String, +cover: String) -> Ship:
  Ship.asked.k(Ship.pos(cover), sh, by)

# a build ended (staged, failed, skipped): those told are gone; with a
# follow-up wanted, its askers wait for it (phase asked)
def Ship.end.k(m: Bool, +state: String, +commit: String, +s: String, +l: String, +n: List<&2, String>) -> Ship:
  match m:
    case True{}:
      Ship{"asked", commit, s, l, "", n, Nil{}, False{}}
    case False{}:
      Ship{Bool.pick(String, String.eq(state, "skipped"), "idle", state), commit, s, l, "", Nil{}, Nil{}, False{}}

def Ship.end(+sh: Ship, +state: String, +commit: String) -> Ship:
  match sh:
    case Ship{_, _, +s, +l, _, _, +n, m}:
      Ship.end.k(m, state, commit, Bool.pick(String, String.eq(state, "staged"), commit, s), l, n)

def Ship.live.k(busy: Bool, +sh: Ship, +commit: String) -> Ship:
  match busy sh:
    case True{} Ship{p, c, +s, _, j, a, n, m}:
      Ship{p, c, Bool.pick(String, String.eq(s, commit), "", s), commit, j, a, n, m}
    case False{} Ship{_, _, +s, _, _, a, n, m}:
      Ship{"live", commit, Bool.pick(String, String.eq(s, commit), "", s), commit, "", a, n, m}

def Ship.pid.set(+sh: Ship, +detail: String) -> Ship:
  match sh:
    case Ship{p, c, s, l, _, a, n, m}:
      Ship{p, c, s, l, detail, a, n, m}

def Ship.building(+sh: Ship, +commit: String) -> Ship:
  match sh:
    case Ship{_, _, s, l, _, a, n, m}:
      Ship{"building", commit, s, l, "", a, n, m}

def Ship.built.k(i: Nat, +sh: Ship, +commit: String, +state: String, +detail: String) -> Ship:
  match i:
    case 0n:
      Ship.building(sh, commit)
    case 1n:
      Ship.pid.set(sh, detail)
    case 2n:
      Ship.live.k(Ship.busy(sh), sh, commit)
    case _:
      Ship.end(sh, state, commit)

# a build's step (state "building"; "pid" records the running job, detail
# "<pid> <ident>"; "staged", "failed" or "skipped" end it; "live" after the
# restart into it)
def Ship.built(+sh: Ship, +commit: String, +state: String, +detail: String) -> Ship:
  Ship.built.k(Bool.pick(Nat, String.eq(state, "building"), 0n, Bool.pick(Nat, String.eq(state, "pid"), 1n, Bool.pick(Nat, String.eq(state, "live"), 2n, 3n))), sh, commit, state, detail)

# who is told when the build under way ends
def Ship.told(+sh: Ship) -> List<&2, String>:
  Ship.askers(sh)

# The follow setting
# ------------------

def Ship.follow.on(+v: String) -> Bool:
  String.eq(v, "on")

# how often a following hub reads origin/main (seconds)
def Ship.every() -> Nat:
  300n

# the minute's tick that looks: one in every Ship.every() seconds
def Ship.due(+now: Nat) -> Bool:
  Nat.is_lt(Nat.mod(now, Ship.every()), 60n)

# What git said
# -------------
# The look (server.bend's Ship.look.sh) prints "HEAD <sha>", then "IN <c>"
# for each candidate commit that contains it, or "ERR <why>".

def Look.pick(ls: List<&2, String>, +tag: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, String.starts_with(l, tag), String.trim(String.drop(l, String.length(tag))), Look.pick(t, tag))

def Look.keep(hit: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}:
      x <> rest
    case False{}:
      rest

def Look.ins(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Look.keep(String.starts_with(l, "IN "), String.trim(String.drop(l, 3n)), Look.ins(t))

def Look.head(+out: String) -> String:
  Look.pick(String.lines(out), "HEAD ")

def Look.within(+out: String) -> List<&2, String>:
  Look.ins(String.lines(out))

def Look.err.k(+err: String, +head: String) -> String:
  Bool.pick(String, Bool.and(String.is_empty(err), String.is_empty(head)), "git said nothing", err)

# why origin/main could not be read ("" when it was)
def Look.err(+out: String) -> String:
  Look.err.k(Look.pick(String.lines(out), "ERR "), Look.head(out))

# A build's report (server.bend's Ship.check.sh): "VERSION <v>" and the
# log's last lines
def Ran.version(+out: String) -> String:
  Look.pick(String.lines(out), "VERSION ")

def Ran.tail.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Look.keep(Bool.not(Bool.or(String.starts_with(l, "VERSION "), Bool.or(String.starts_with(l, "STAGE "), String.starts_with(l, "STATUS ")))), l, Ran.tail.go(t))

def Ran.tail(+out: String) -> String:
  String.trim(String.join(Ran.tail.go(String.lines(out)), "\n"))

def Ran.stage(+out: String) -> String:
  Look.pick(String.lines(out), "STAGE ")

# "running", "done <code>" or "gone"
def Ran.status(+out: String) -> String:
  Look.pick(String.lines(out), "STATUS ")

def Ran.ok(+out: String) -> Bool:
  String.eq(Ran.status(out), "done 0")

def Ran.gone(+out: String) -> Bool:
  String.eq(Ran.status(out), "gone")

# Words
# -----

def Ship.short(+c: String) -> String:
  String.take(c, 9n)

# the restart's note, which says which build it carries
def Ship.note(+commit: String) -> String:
  "deploy_main " ++ Ship.short(commit)

def Ship.text.k(i: Nat, +commit: String, +version: String, +detail: String) -> String:
  match i:
    case 0n:
      "[Backplane] deploy_main: origin/main " ++ Ship.short(commit) ++ " is built and staged" ++ Bool.pick(String, String.is_empty(version), "", " (version " ++ version ++ ")") ++
        ". The hub restarts into it at a safe point (no tool call running, build locks free); every turn it cuts resumes."
    case 1n:
      "[Backplane] deploy_main: the build of origin/main " ++ Ship.short(commit) ++ " failed" ++ Bool.pick(String, String.is_empty(detail), ".", ":\n" ++ detail)
    case _:
      "[Backplane] deploy_main: nothing to build; origin/main is already in " ++ Ship.short(commit) ++ Bool.pick(String, String.is_empty(detail), ".", " (" ++ detail ++ ").")

# what the askers are told when a build ends (state staged, failed, skipped)
def Ship.text(+state: String, +commit: String, +version: String, +detail: String) -> String:
  Ship.text.k(Bool.pick(Nat, String.eq(state, "staged"), 0n, Bool.pick(Nat, String.eq(state, "failed"), 1n, 2n)), commit, version, detail)

def Ship.lost() -> String:
  "lost: the build is gone and left no exit status (the hub or the machine restarted under it)"

# deploy_status's part
def Ship.status(+sh: Ship, +follow: Bool, +repo: String) -> J.Json:
  match sh:
    case Ship{p, c, s, l, j, a, n, m}:
      J.Json.obj([J.KV{"configured", J.Flag{Bool.not(String.is_empty(repo))}}, J.KV{"repo", J.Str{repo}}, J.KV{"follow", J.Flag{follow}},
        J.KV{"phase", J.Str{p}}, J.KV{"commit", J.Str{c}}, J.KV{"staged", J.Str{s}}, J.KV{"live", J.Str{l}}, J.KV{"job", J.Str{j}},
        J.KV{"waiting", J.Num{Nat.show(List.length(&2, String, a))}}, J.KV{"followUp", J.Flag{m}}, J.KV{"followUpWaiting", J.Num{Nat.show(List.length(&2, String, n))}}])

# Demos for the laws and tests
# ----------------------------

# building c1 for t1
def Ship.demo.building() -> Ship:
  Ship{"building", "c1", "", "c0", "", ["t1"], Nil{}, False{}}

# t2 and t3 asked for c2 during it, t2 twice
def Ship.demo.queued() -> Ship:
  Ship.asked(Ship.asked(Ship.asked(Ship.demo.building(), "t2", ""), "t3", ""), "t2", "")
