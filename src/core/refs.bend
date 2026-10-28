import Base
import ./model.bend as M
import ./bot.bend as BT
import ./fuzzy.bend as F
import ./json.bend as J

# References in a message
# -----------------------
# A word that starts with a sigil names something: `@bot` a bot (here, or
# `@name@machine` on a linked machine), `>thread` a thread, `%project` a
# project, `#room` a group chat. After the sigil
# comes a slug: letters, digits, '-' and '_' (Slug.of a title: lower case,
# every other run of characters one '-', at most 40 long). A thread whose
# slug another thread shares is named `slug-<seq>`, the number its id ends
# with. The composer offers them as they are typed (Refs.offer, ranked:
# how well the query matches, the same project, recent work); the hub
# resolves them in a sent message (Refs.found): a mentioned bot is woken, a
# mentioned room hears the message, and the agent gets a block naming each
# one (Refs.block). Summaries come from the text generation model
# (Sum.*), kept as settings summary.<id>.

# x before rest when keep
def Refs.cons(-A: Data, keep: Bool, x: A, rest: List<&2, A>) -> List<&2, A>:
  match keep:
    case True{}:
      x <> rest
    case False{}:
      rest

# Slugs
# -----

def Slug.alnum(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)))

# a character a slug may hold (as typed: any case)
def Slug.c(+c: U32) -> Bool:
  Bool.or(Slug.alnum(c), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)))

def Slug.low(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

# the slug so far, reversed; dash: the last thing put was a '-' (or none yet)
type SAcc is Data:
  SAcc{dash: Bool, acc: String}

def Slug.step.k(al: Bool, dash: Bool, +c: U32, acc: String) -> SAcc:
  match al dash:
    case True{} _:
      SAcc{False{}, SCon{Chr{Slug.low(c)}, acc}}
    case False{} True{}:
      SAcc{True{}, acc}
    case False{} False{}:
      SAcc{True{}, SCon{Chr{45}, acc}}

def Slug.step(+c: U32, st: SAcc) -> SAcc:
  match st:
    case SAcc{dash, acc}:
      Slug.step.k(Slug.alnum(c), dash, c, acc)

def Slug.go(s: String, st: SAcc) -> SAcc:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Slug.go(t, Slug.step(c, st))

def Slug.chop.k(dash: Bool, t: String, +c: U32) -> String:
  match dash:
    case True{}:
      t
    case False{}:
      SCon{Chr{c}, t}

# a reversed slug without its last '-'
def Slug.chop(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Slug.chop.k(U32.is_eq(c, 45), t, c)

def Slug.cut(+s: String) -> String:
  Bool.pick(String, String.ends_with(s, "-"), String.take(s, Nat.sub(String.length(s), 1n)), s)

def Slug.fin(st: SAcc) -> String:
  match st:
    case SAcc{_, acc}:
      Slug.cut(String.take(String.reverse(Slug.chop(acc)), 40n))

# a name as a slug ("Fix the perf lag!" is "fix-the-perf-lag")
def Slug.of(s: String) -> String:
  Slug.fin(Slug.go(s, SAcc{True{}, ""}))

# whether a string starts with a slug character
def Slug.head(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{Chr{+c}, _}:
      Slug.c(c)

# the leading slug characters of a word (what follows a sigil), reversed
def Slug.take.go(s: String, ok: Bool, +acc: String) -> String:
  match s ok:
    case SCon{Chr{+c}, +t} True{}:
      Slug.take.go(t, Slug.head(t), SCon{Chr{c}, acc})
    case _ _:
      acc

def Slug.take(+s: String) -> String:
  String.reverse(Slug.take.go(s, Slug.head(s), ""))

# whether every character may be in a slug ("" too)
def Slug.all(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Slug.c(c), Slug.all(t))

# Sigils
# ------

# the kind a sigil names: "bot" '@', "thread" '>', "project" '%', "room" '#'
def Refs.kind.c(+c: U32) -> String:
  Bool.pick(String, U32.is_eq(c, 64), "bot", Bool.pick(String, U32.is_eq(c, 62), "thread",
    Bool.pick(String, U32.is_eq(c, 37), "project", Bool.pick(String, U32.is_eq(c, 35), "room", ""))))

# the kind a word's sigil names ("" for none)
def Refs.kind(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, _}:
      Refs.kind.c(c)

def Refs.sigil(+kind: String) -> String:
  Bool.pick(String, String.eq(kind, "bot"), "@", Bool.pick(String, String.eq(kind, "thread"), ">",
    Bool.pick(String, String.eq(kind, "project"), "%", Bool.pick(String, String.eq(kind, "room"), "#", ""))))

# the draft's last word
# Words, walked once and tail-recursively (String.lines and String.split
# recurse once per character: a long pasted draft overflowed the web's
# stack at every key)
def Refs.sep(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 10)), Bool.or(U32.is_eq(c, 9), U32.is_eq(c, 13)))

def Refs.cur(sep: Bool, +c: U32, cur: String) -> String:
  match sep:
    case True{}:
      SNil{}
    case False{}:
      SCon{Chr{c}, cur}

def Refs.last.go(s: String, cur: String) -> String:
  match s:
    case SNil{}:
      String.reverse(cur)
    case SCon{Chr{+c}, t}:
      Refs.last.go(t, Refs.cur(Refs.sep(c), c, cur))

# the word being typed: the draft's last, after its last space or line break
def Refs.token(+draft: String) -> String:
  Refs.last.go(draft, SNil{})

# a bot on a linked machine: what follows the sigil is `name@machine`.
# The machine a word names ("" for a bot here); its name comes first
def Refs.far.after(+a: String, +r: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(a)), String.starts_with(r, "@")), Slug.take(String.drop(r, 1n)), "")

def Refs.far.m(+rest: String) -> String:
  Refs.far.after(Slug.take(rest), String.drop(rest, String.length(Slug.take(rest))))

# whether what follows a `@` is `name@machine` as far as typed (slug
# characters around one '@', the machine not yet typed is fine)
def Refs.far.typing(+rest: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Slug.take(rest))),
    Bool.and(String.starts_with(String.drop(rest, String.length(Slug.take(rest))), "@"), Slug.all(String.drop(rest, Nat.add(String.length(Slug.take(rest)), 1n)))))

# Threads elsewhere (far.bend's "<link>~<id>", synced from another of the
# owner's machines): written `>machine:slug`, the machine by its name as a
# slug (its link id when the name makes none). `>machine:` alone offers
# only that machine's threads.
def Refs.x.is(+id: String) -> Bool:
  String.contains(id, "~")

def Refs.x.link(+id: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(id, '~')), "")

# the machine a thread elsewhere lives on, as it is called
def Refs.x.name(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(M.Nav.key(M.Nav.get(M.State.navs(s), "far~" ++ Refs.x.link(id)), "pinned")), Refs.x.link(id),
    M.Nav.key(M.Nav.get(M.State.navs(s), "far~" ++ Refs.x.link(id)), "pinned"))

# ... and as a word writes it
def Refs.x.mach(+s: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(Slug.of(Refs.x.name(s, id))), Slug.of(Refs.x.link(id)), Slug.of(Refs.x.name(s, id)))

# what follows `>`: the machine part (before ':', "" for none) and the rest
def Refs.x.m(+w: String) -> String:
  Bool.pick(String, String.contains(w, ":"), Maybe.default(&2, String, List.head(&2, String, String.split(w, ':')), ""), "")

def Refs.x.q(+w: String) -> String:
  Bool.pick(String, String.contains(w, ":"), String.drop(w, Nat.add(String.length(Refs.x.m(w)), 1n)), w)

# whether what follows a `>` is `machine:slug` as far as typed
def Refs.x.typing(+rest: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Slug.take(rest))),
    Bool.and(String.starts_with(String.drop(rest, String.length(Slug.take(rest))), ":"), Slug.all(String.drop(rest, Nat.add(String.length(Slug.take(rest)), 1n)))))

# whether what follows a `>` names a thread elsewhere: `machine:slug`, a
# slug after the ':' (`>fix: it fails` still names the thread fix here)
def Refs.x.word(+rest: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Slug.take(rest))),
    Bool.and(String.starts_with(String.drop(rest, String.length(Slug.take(rest))), ":"), Bool.not(String.is_empty(Slug.take(String.drop(rest, Nat.add(String.length(Slug.take(rest)), 1n)))))))

# whether a word is a reference being typed: a sigil, then slug characters
# only (none yet is fine), a bot's `name@machine`, or a thread's
# `machine:slug`
def Refs.typing(+w: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Refs.kind(w))),
    Bool.or(Slug.all(String.drop(w, 1n)), Bool.or(Bool.and(String.eq(Refs.kind(w), "bot"), Refs.far.typing(String.drop(w, 1n))),
      Bool.and(String.eq(Refs.kind(w), "thread"), Refs.x.typing(String.drop(w, 1n))))))

# Summaries
# ---------
# The text generation model writes one sentence about a thread (from its
# title, the user's latest message and the agent's latest reply) when a
# turn ends, at most every Sum.every() seconds, and about a project from
# its threads when one of them gets a summary and the project's is as old.
# A bot's is its thread's.

def Sum.key(+id: String) -> String:
  "summary." ++ id

def Sum.at.key(+id: String) -> String:
  "summary.at." ++ id

def Sum.every() -> Nat:
  600n

def Sum.of(+s: M.State, +id: String) -> String:
  M.Setting.get(s, Sum.key(id), "")

# digits as a number (not Nat.read: it makes laws crawl)
def Sum.num.go(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Sum.num.go(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc))

def Sum.num(s: String) -> Nat:
  Sum.num.go(s, 0n)

# whether a summary of id is due at now: never made, or made Sum.every()
# seconds ago or more
def Sum.due.k(+at: String, +now: Nat) -> Bool:
  Bool.or(String.is_empty(at), Nat.is_ge(Nat.sub(now, Sum.num(at)), Sum.every()))

def Sum.due(+s: M.State, +id: String, +now: Nat) -> Bool:
  Sum.due.k(M.Setting.get(s, Sum.at.key(id), ""), now)

# the writer's answer as a summary: its first line, spaces collapsed, at
# most 200 characters ("" for none)
def Sum.clean.cut(long: Bool, +l: String) -> String:
  match long:
    case True{}:
      String.take(l, 197n) ++ "..."
    case False{}:
      l

def Sum.words(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +w <> t:
      Refs.cons(String, Bool.not(String.is_empty(w)), w, Sum.words(t))

def Sum.clean(+answer: String) -> String:
  +l = String.join(Sum.words(String.split(String.join(String.split(String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(String.trim(answer))), "")), '\t'), " "), ' ')), " ")
  Sum.clean.cut(Nat.is_gt(String.length(l), 200n), l)

# the newest message of a role in a thread (entries are newest first)
def Sum.last.hit(es: List<&2, M.Entry>, +th: String, +user: Bool) -> Bool:
  match es:
    case M.Msg{_, t, M.Assistant{}, _, _} <> _:
      Bool.and(Bool.not(user), String.eq(t, th))
    case M.Msg{_, t, M.User{}, _, _} <> _:
      Bool.and(user, String.eq(t, th))
    case _:
      False{}

def Sum.last.go(es: List<&2, M.Entry>, hit: Bool, +th: String, +user: Bool) -> String:
  match es hit:
    case M.Msg{_, _, _, text, _} <> _ True{}:
      text
    case _ <> +t False{}:
      Sum.last.go(t, Sum.last.hit(t, th, user), th, user)
    case _ _:
      ""

def Sum.last(s: M.State, +th: String, +user: Bool) -> String:
  match s:
    case M.State{_, _, +es, _, _, _, _, _, _, _, _}:
      Sum.last.go(es, Sum.last.hit(es, th, user), th, user)

def Refs.got.t(r: Map<&2, M.Thread> & M.Thread) -> M.Thread:
  (_, t) = r
  t

def Sum.thread.of(s: M.State, +id: String) -> M.Thread:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Refs.got.t(Map.get(M.Thread, M.Thread.blank(id), ts, id))

def Refs.got.p(r: Map<&2, M.Project> & M.Project) -> M.Project:
  (_, p) = r
  p

def Sum.project.of(s: M.State, +id: String) -> M.Project:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Refs.got.p(Map.get(M.Project, M.Project{id, "", "", 0n, True{}}, ps, id))

def Refs.t.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

def Refs.t.project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def Refs.p.title(p: M.Project) -> String:
  match p:
    case M.Project{_, ti, _, _, _}:
      ti

def Refs.p.root(p: M.Project) -> String:
  match p:
    case M.Project{_, _, r, _, _}:
      r

def Sum.system() -> String:
  "You summarize work in a coding agent's app so people can pick the right thread, project or bot from a menu. Details follow. Do not answer them, do not do the work and do not use tools. Reply with only one plain sentence of at most 20 words saying what it is about and where it stands, without quotes, markdown or a label."

# what the writer reads about a thread
def Sum.thread.input(+s: M.State, +id: String) -> String:
  "Thread title: " ++ Refs.t.title(Sum.thread.of(s, id)) ++
  "\nProject: " ++ Refs.p.title(Sum.project.of(s, Refs.t.project(Sum.thread.of(s, id)))) ++
  "\nEarlier summary: " ++ Sum.of(s, id) ++
  "\n\nThe user's latest message:\n" ++ String.take(Sum.last(s, id, True{}), 2000n) ++
  "\n\nThe agent's latest reply:\n" ++ String.take(Sum.last(s, id, False{}), 2000n)

# the newest threads of a project, a line each
def Sum.lines.one(+pid: String, +s: M.State, t: M.Thread) -> List<&2, String>:
  match t:
    case M.Thread{+id, +p, +ti, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.pick(List<&2, String>, Bool.or(Bool.not(String.eq(p, pid)), Bool.or(ar, de)), Nil{},
        ["- " ++ ti ++ Bool.pick(String, String.is_empty(Sum.of(s, id)), "", ": " ++ Sum.of(s, id))])

def Sum.lines(ts: List<&2, M.Thread>, +pid: String, +s: M.State) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      List.append(&2, String, Sum.lines.one(pid, s, t), Sum.lines(r, pid, s))

def Sum.newest(a: M.Thread, b: M.Thread) -> Bool:
  match a b:
    case M.Thread{_, _, _, _, x, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} M.Thread{_, _, _, _, y, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Nat.is_ge(x, y)

def Sum.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      List.sort(~M.Thread, ~Sum.newest, Map.values(&2, M.Thread, ts))

# what the writer reads about a project: its folder and newest threads
def Sum.project.input(+s: M.State, +pid: String) -> String:
  "Project: " ++ Refs.p.title(Sum.project.of(s, pid)) ++ "\nFolder: " ++ Refs.p.root(Sum.project.of(s, pid)) ++
  "\n\nIts newest threads:\n" ++ String.join(List.take(&2, String, Sum.lines(Sum.threads(s), pid, s), 12n), "\n")

# Offers
# ------
# What the composer offers for the word being typed: at most Refs.max()
# rows, best first. A row's score is how well the query matches its slug
# or name (Fuzzy.score; a query of 3 or more characters found in its
# summary counts too), plus how near it is: a thread in the same project,
# one worked on lately, a pinned or running one; a bot busy lately.

type Cand is Data:
  Cand{token: String, about: String, score: Nat, at: Nat}

def Refs.max() -> Nat:
  6n

def Cand.before(a: Cand, b: Cand) -> Bool:
  match a b:
    case Cand{_, _, +sa, aa} Cand{_, _, +sb, ab}:
      Bool.or(Nat.is_gt(sa, sb), Bool.and(Nat.is_eq(sa, sb), Nat.is_ge(aa, ab)))

# how well q (lower case, no sigil) matches a candidate (0: not at all)
def Refs.score(+slug: String, +name: String, +about: String, +q: String) -> Nat:
  Bool.pick(Nat, String.is_empty(q), 1n,
    Nat.add(Nat.add(F.Fuzzy.score(slug, q), F.Fuzzy.score(name, q)),
      Bool.pick(Nat, Bool.and(Nat.is_ge(String.length(q), 3n), String.contains(String.to_lower(about), q)), 12n, 0n)))

# how recent: within an hour, a day, a week
def Refs.fresh(+now: Nat, +at: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(Nat.sub(now, at), 3600n), 30n, Bool.pick(Nat, Nat.is_le(Nat.sub(now, at), 86400n), 20n, Bool.pick(Nat, Nat.is_le(Nat.sub(now, at), 604800n), 10n, 0n)))

def Refs.keep(+m: Nat, c: Cand, rest: List<&2, Cand>) -> List<&2, Cand>:
  match m:
    case 0n:
      rest
    case _:
      c <> rest

# a thread's seq: what its id ends with after the last '-'
def Refs.seq(+id: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(id, '-')), "")

# a thread's slug, its id's seq when the title has none
def Refs.t.slug(+title: String, +id: String) -> String:
  Bool.pick(String, String.is_empty(Slug.of(title)), Slug.of(id), Slug.of(title))

def Refs.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

def Refs.proj.title(+s: M.State, +pid: String) -> String:
  Refs.p.title(Sum.project.of(s, pid))

# what a thread's row says: its summary, else its title; the project's
# name first when it is another project's
def Refs.t.about(+s: M.State, +same: Bool, +pid: String, +title: String, +sum: String) -> String:
  Bool.pick(String, same, "", Refs.proj.title(s, pid) ++ ": ") ++ title ++ Bool.pick(String, String.is_empty(sum), "", " — " ++ sum)

def Refs.count.of(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

def Refs.count.k(skip: Bool, +k: String, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match skip:
    case True{}:
      m
    case False{}:
      Map.set(&2, Nat, m, k, Nat.add(Refs.count.of(Map.get(Nat, 0n, m, k)), 1n))

# what a thread's slug is counted under: its slug, with its machine for
# one elsewhere ("desk:fix-perf")
def Refs.t.key(+s: M.State, +slug: String, +id: String) -> String:
  Bool.pick(String, Refs.x.is(id), Refs.x.mach(s, id) ++ ":" ++ slug, slug)

# the slugs every live project thread has, counted (for Refs.t.token)
def Refs.counts(ts: List<&2, M.Thread>, +s: M.State, +m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ts:
    case Nil{}:
      m
    case M.Thread{+id, +p, +ti, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de} <> t:
      Refs.counts(t, s, Refs.count.k(Bool.or(String.is_empty(p), Bool.or(ar, de)), Refs.t.key(s, Refs.t.slug(ti, id), id), m))

def Refs.count(+m: Map<&2, Nat>, +k: String) -> Nat:
  Refs.count.of(Map.get(Nat, 0n, m, k))

# a thread's word: its slug, with its seq when another thread shares it,
# after its machine for one elsewhere
def Refs.t.token(+s: M.State, +m: Map<&2, Nat>, +slug: String, +id: String) -> String:
  ">" ++ Bool.pick(String, Refs.x.is(id), Refs.x.mach(s, id) ++ ":", "") ++ Bool.pick(String, Nat.is_gt(Refs.count(m, Refs.t.key(s, slug, id)), 1n), slug ++ "-" ++ Refs.seq(id), slug)

# whether a thread is left out for the machine a word names ("": none named)
def Refs.x.out(+s: M.State, +mach: String, +id: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(mach)), Bool.not(Bool.and(Refs.x.is(id), String.starts_with(Refs.x.mach(s, id), mach))))

# what a row matches on: its title, and its machine's name for one elsewhere
def Refs.t.name(+s: M.State, +id: String, +title: String) -> String:
  Bool.pick(String, Refs.x.is(id), title ++ " " ++ Refs.x.name(s, id), title)

def Refs.t.about.x(+s: M.State, +same: Bool, +id: String, +pid: String, +title: String, +sum: String) -> String:
  Bool.pick(String, Refs.x.is(id), Refs.x.name(s, id) ++ " · " ++ Refs.t.about(s, False{}, pid, title, sum), Refs.t.about(s, same, pid, title, sum))

def Refs.t.cand(+s: M.State, +m: Map<&2, Nat>, +sel: String, +here: String, +now: Nat, +mach: String, +q: String, t: M.Thread, +rest: List<&2, Cand>) -> List<&2, Cand>:
  match t:
    case M.Thread{+id, +p, +ti, _, +act, tu, _, ar, _, _, _, +pin, _, _, _, _, _, _, _, _, de}:
      Bool.pick(List<&2, Cand>, Bool.or(Bool.or(Bool.or(String.is_empty(p), String.eq(id, sel)), Bool.or(ar, de)), Refs.x.out(s, mach, id)), rest,
        Refs.keep(Refs.score(Refs.t.slug(ti, id), Refs.t.name(s, id, ti), Sum.of(s, id), q),
          Cand{Refs.t.token(s, m, Refs.t.slug(ti, id), id), Refs.t.about.x(s, String.eq(p, here), id, p, ti, Sum.of(s, id)),
            Nat.add(Nat.add(Refs.score(Refs.t.slug(ti, id), Refs.t.name(s, id, ti), Sum.of(s, id), q), Refs.fresh(now, act)),
              Nat.add(Bool.pick(Nat, String.eq(p, here), 40n, 0n), Nat.add(Bool.pick(Nat, pin, 10n, 0n), Bool.pick(Nat, Refs.running(tu), 15n, 0n)))), act},
          rest))

def Refs.threads.go(ts: List<&2, M.Thread>, +s: M.State, +m: Map<&2, Nat>, +sel: String, +here: String, +now: Nat, +mach: String, +q: String) -> List<&2, Cand>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      Refs.t.cand(s, m, sel, here, now, mach, q, t, Refs.threads.go(r, s, m, sel, here, now, mach, q))

# (w: what follows the `>`, lower case: `machine:` narrows to that machine)
def Refs.threads(+s: M.State, +sel: String, +now: Nat, +w: String) -> List<&2, Cand>:
  match s:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}:
      Refs.threads.go(Map.values(&2, M.Thread, ts), s, Refs.counts(Map.values(&2, M.Thread, ts), s, Map.new(&2, Nat)), sel,
        Refs.t.project(Sum.thread.of(s, sel)), now, Refs.x.m(w), Refs.x.q(w))

# a bot's row: its summary, else its persona's first line
def Refs.b.first(+line: String) -> String:
  Bool.pick(String, String.is_empty(line), "bot", String.take(line, 160n))

def Refs.b.about(+sum: String, +persona: String) -> String:
  Bool.pick(String, String.is_empty(sum), Refs.b.first(String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(String.trim(persona))), ""))), sum)

def Refs.t.act(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      a

def Refs.b.cand(+s: M.State, +sel: String, +now: Nat, +q: String, b: BT.Bot, +rest: List<&2, Cand>) -> List<&2, Cand>:
  match b:
    case BT.Bot{_, +name, +th, _, +persona, _, _}:
      Bool.pick(List<&2, Cand>, String.eq(th, sel), rest,
        Refs.keep(Refs.score(Slug.of(name), name, Refs.b.about(Sum.of(s, th), persona), q),
          Cand{"@" ++ name, Refs.b.about(Sum.of(s, th), persona),
            Nat.add(Refs.score(Slug.of(name), name, Refs.b.about(Sum.of(s, th), persona), q), Refs.fresh(now, Refs.t.act(Sum.thread.of(s, th)))),
            Refs.t.act(Sum.thread.of(s, th))},
          rest))

def Refs.bots.go(bs: List<&2, BT.Bot>, +s: M.State, +sel: String, +now: Nat, +q: String) -> List<&2, Cand>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> r:
      Refs.b.cand(s, sel, now, q, b, Refs.bots.go(r, s, sel, now, q))

def Refs.bots(+s: M.State, +sel: String, +now: Nat, +q: String) -> List<&2, Cand>:
  Refs.bots.go(BT.World.bots.here(M.State.bots(s)), s, sel, now, q)

# Bots on linked machines, as the hub lists them (info "bots.remote":
# [{"name", "peer", "peerName", "state", "seen"}]): a bot's word is
# `@name@machine`, the machine by its name, or by its link id when the
# name could mean another machine or is no slug
type Far is Data:
  Far{name: String, at: String, about: String, seen: Nat}

def Refs.far.at(+ps: List<&2, BT.Peer>, +link: String, +pname: String) -> String:
  Bool.pick(String, Bool.or(BT.Peer.clash(ps, link, pname), Bool.not(Slug.all(pname))), link, pname)

def Refs.far.of(+ps: List<&2, BT.Peer>, +o: J.Json) -> Far:
  Far{J.Json.get_str(o, "name", ""), Refs.far.at(ps, J.Json.get_str(o, "peer", ""), J.Json.get_str(o, "peerName", J.Json.get_str(o, "peer", ""))),
    "bot on " ++ J.Json.get_str(o, "peerName", J.Json.get_str(o, "peer", "")) ++ Bool.pick(String, String.eq(J.Json.get_str(o, "state", ""), "away"), ", away", ""),
    J.Json.get_nat(o, "seen", 0n)}

def Refs.fars.go(os: List<&2, J.Json>, +ps: List<&2, BT.Peer>) -> List<&2, Far>:
  match os:
    case Nil{}:
      Nil{}
    case +o <> t:
      Refs.cons(Far, Bool.and(Slug.all(J.Json.get_str(o, "name", "")), Bool.not(String.is_empty(J.Json.get_str(o, "name", "")))), Refs.far.of(ps, o), Refs.fars.go(t, ps))

# the linked machines' bots a list names (bots whose name is no slug
# cannot be written as a word, so are left out)
def Refs.fars(+dir: String, +ps: List<&2, BT.Peer>) -> List<&2, Far>:
  Refs.fars.go(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(dir), J.Json.arr(Nil{}))), ps)

# the list read only for a `@` word (a phone parses it per keystroke)
def Refs.fars.if(bot: Bool, +dir: String, +ps: List<&2, BT.Peer>) -> List<&2, Far>:
  match bot:
    case True{}:
      Refs.fars(dir, ps)
    case False{}:
      Nil{}

def Refs.f.cand.k(+sc: Nat, +now: Nat, +word: String, +about: String, +seen: Nat, rest: List<&2, Cand>) -> List<&2, Cand>:
  Refs.keep(sc, Cand{word, about, Nat.add(sc, Refs.fresh(now, seen)), seen}, rest)

def Refs.f.cand(+now: Nat, +q: String, f: Far, rest: List<&2, Cand>) -> List<&2, Cand>:
  match f:
    case Far{+n, +at, +ab, +seen}:
      Refs.f.cand.k(Refs.score(String.to_lower(n) ++ "@" ++ String.to_lower(at), n, ab, q), now, "@" ++ n ++ "@" ++ at, ab, seen, rest)

def Refs.f.cands(fs: List<&2, Far>, +now: Nat, +q: String) -> List<&2, Cand>:
  match fs:
    case Nil{}:
      Nil{}
    case f <> t:
      Refs.f.cand(now, q, f, Refs.f.cands(t, now, q))

def Refs.p.cand(+s: M.State, +here: String, +q: String, p: M.Project, +rest: List<&2, Cand>) -> List<&2, Cand>:
  match p:
    case M.Project{+id, +ti, +root, +at, rm}:
      Bool.pick(List<&2, Cand>, rm, rest,
        Refs.keep(Refs.score(Refs.t.slug(ti, id), ti, Sum.of(s, id), q),
          Cand{"%" ++ Refs.t.slug(ti, id), Bool.pick(String, String.is_empty(Sum.of(s, id)), root, Sum.of(s, id)),
            Nat.add(Refs.score(Refs.t.slug(ti, id), ti, Sum.of(s, id), q), Bool.pick(Nat, String.eq(id, here), 5n, 0n)), at},
          rest))

def Refs.projects.go(ps: List<&2, M.Project>, +s: M.State, +here: String, +q: String) -> List<&2, Cand>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> r:
      Refs.p.cand(s, here, q, p, Refs.projects.go(r, s, here, q))

def Refs.projects(+s: M.State, +sel: String, +q: String) -> List<&2, Cand>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Refs.projects.go(Map.values(&2, M.Project, ps), s, Refs.t.project(Sum.thread.of(s, sel)), q)


# a room's size where the composer offers it: "1 member", "3 members"
def Refs.members(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 1n), "1 member", Nat.show(n) ++ " members")

def Refs.r.cand(+q: String, r: BT.Room, rest: List<&2, Cand>) -> List<&2, Cand>:
  match r:
    case BT.Room{+id, +name, ms, +at, _}:
      Refs.keep(Refs.score(Refs.t.slug(name, id), name, "", q),
        Cand{"#" ++ Refs.t.slug(name, id), Refs.members(List.length(&2, String, ms)), Refs.score(Refs.t.slug(name, id), name, "", q), at},
        rest)

def Refs.rooms.go(rs: List<&2, BT.Room>, +q: String) -> List<&2, Cand>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Refs.r.cand(q, r, Refs.rooms.go(t, q))

def Refs.rooms(+s: M.State, +q: String) -> List<&2, Cand>:
  Refs.rooms.go(BT.World.rooms(M.State.bots(s)), q)

def Refs.cands(i: Nat, +s: M.State, +sel: String, +now: Nat, +q: String, fs: List<&2, Far>) -> List<&2, Cand>:
  match i:
    case 0n:
      List.append(&2, Cand, Refs.bots(s, sel, now, q), Refs.f.cands(fs, now, q))
    case 1n:
      Refs.threads(s, sel, now, q)
    case 2n:
      Refs.projects(s, sel, q)
    case 3n:
      Refs.rooms(s, q)
    case _:
      Nil{}

def Refs.kind.pos(+kind: String) -> Nat:
  Bool.pick(Nat, String.eq(kind, "bot"), 0n, Bool.pick(Nat, String.eq(kind, "thread"), 1n, Bool.pick(Nat, String.eq(kind, "project"), 2n, Bool.pick(Nat, String.eq(kind, "room"), 3n, 4n))))

# the rows offered for a word being typed in thread sel (none for a word
# that is no reference), with the linked machines' bots fs after a `@`
def Refs.offer.with(+s: M.State, +sel: String, +now: Nat, +w: String, fs: List<&2, Far>) -> List<&2, Cand>:
  List.take(&2, Cand, List.sort(~Cand, ~Cand.before, Refs.cands(Bool.pick(Nat, Refs.typing(w), Refs.kind.pos(Refs.kind(w)), 4n), s, sel, now, String.to_lower(String.drop(w, 1n)), fs)), Refs.max())

# the rows offered with this hub's own bots only
def Refs.offer(+s: M.State, +sel: String, +now: Nat, +w: String) -> List<&2, Cand>:
  Refs.offer.with(s, sel, now, w, Nil{})

# Resolving a sent message
# ------------------------

type Hit is Data:
  Hit{kind: String, id: String, name: String, word: String}

def Hit.id(h: Hit) -> String:
  match h:
    case Hit{_, i, _, _}:
      i

# a thread a word names: its slug, or its slug and seq; the same project
# first, then the newest
def Refs.t.named(+w: String, +id: String, +title: String) -> Bool:
  Bool.or(String.eq(Refs.t.slug(title, id), w), String.eq(Refs.t.slug(title, id) ++ "-" ++ Refs.seq(id), w))

def Refs.t.hits(ts: List<&2, M.Thread>, +w: String, +here: String) -> List<&2, Cand>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, +p, +ti, _, +act, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de} <> t:
      Refs.cons(Cand, Bool.and(Bool.and(Bool.not(String.is_empty(p)), Bool.not(Bool.or(ar, de))), Refs.t.named(w, id, ti)),
        Cand{id, ti, Bool.pick(Nat, String.eq(p, here), 1n, 0n), act}, Refs.t.hits(t, w, here))

def Refs.hit.of(+kind: String, +word: String, m: Maybe<&2, Cand>) -> List<&2, Hit>:
  match m:
    case Some{Cand{id, name, _, _}}:
      [Hit{kind, id, name, word}]
    case None{}:
      Nil{}

def Refs.find.thread(+s: M.State, +sel: String, +w: String) -> List<&2, Hit>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Refs.hit.of("thread", ">" ++ w, List.head(&2, Cand, List.sort(~Cand, ~Cand.before, Refs.t.hits(Map.values(&2, M.Thread, ts), w, Refs.t.project(Sum.thread.of(s, sel))))))

# a bot the word names, unless it is the bot whose thread it is said in
# (law refs_own_bot_never)
def Refs.b.one(named: Bool, own: Bool, +id: String, +name: String) -> List<&2, Hit>:
  match named own:
    case True{} False{}:
      [Hit{"bot", id, name, "@" ++ name}]
    case True{} True{}:
      Nil{}
    case False{} _:
      Nil{}

# the bots a word names (names are unique, so one at most)
def Refs.b.hits(bs: List<&2, BT.Bot>, +w: String, +sel: String) -> List<&2, Hit>:
  match bs:
    case Nil{}:
      Nil{}
    case BT.Bot{+id, +name, +th, _, _, _, _} <> t:
      List.append(&2, Hit, Refs.b.one(String.eq(Slug.of(name), w), String.eq(th, sel), id, name), Refs.b.hits(t, w, sel))

def Refs.p.hits(ps: List<&2, M.Project>, +w: String) -> List<&2, Hit>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, +ti, _, _, rm} <> t:
      Bool.pick(List<&2, Hit>, Bool.and(Bool.not(rm), String.eq(Refs.t.slug(ti, id), w)), [Hit{"project", id, ti, "%" ++ w}], Refs.p.hits(t, w))

def Refs.find.project(+s: M.State, +w: String) -> List<&2, Hit>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Refs.p.hits(Map.values(&2, M.Project, ps), w)

def Refs.r.hits(rs: List<&2, BT.Room>, +w: String) -> List<&2, Hit>:
  match rs:
    case Nil{}:
      Nil{}
    case BT.Room{+id, +name, _, _, _} <> t:
      Bool.pick(List<&2, Hit>, String.eq(Refs.t.slug(name, id), w), [Hit{"room", id, name, "#" ++ w}], Refs.r.hits(t, w))

def Refs.find.k(i: Nat, +s: M.State, +sel: String, +w: String) -> List<&2, Hit>:
  match i:
    case 0n:
      Refs.b.hits(BT.World.bots.here(M.State.bots(s)), w, sel)
    case 1n:
      Refs.find.thread(s, sel, w)
    case 2n:
      Refs.find.project(s, w)
    case 3n:
      Refs.r.hits(BT.World.rooms(M.State.bots(s)), w)
    case _:
      Nil{}

# a bot on a linked machine the log knows (its id is "name@machine", as
# bothub.bend's Bots.tell takes it)
def Refs.far.hit(m: Maybe<&2, BT.Peer>, +name: String, +at: String) -> List<&2, Hit>:
  match m:
    case Some{_}:
      [Hit{"bot", name ++ "@" ++ at, name, "@" ++ name ++ "@" ++ at}]
    case None{}:
      Nil{}

def Refs.find.far(far: Bool, +s: M.State, +sel: String, +word: String) -> List<&2, Hit>:
  match far:
    case True{}:
      Refs.far.hit(BT.Peer.of(BT.World.peers(M.State.bots(s)), Refs.far.m(String.drop(word, 1n))), Slug.take(String.drop(word, 1n)), Refs.far.m(String.drop(word, 1n)))
    case False{}:
      Refs.find.k(Refs.kind.pos(Refs.kind(word)), s, sel, Slug.of(Slug.take(String.drop(word, 1n))))

# what one word names (nothing, or one hit)
# (a thread elsewhere, `>machine:slug`, is none here: xref.bend finds it
# in the hub's mirror of that machine)
def Refs.find(+s: M.State, +sel: String, +word: String) -> List<&2, Hit>:
  Bool.pick(List<&2, Hit>, Bool.or(String.is_empty(Slug.take(String.drop(word, 1n))), Bool.and(String.eq(Refs.kind(word), "thread"), Refs.x.word(String.drop(word, 1n)))), Nil{},
    Refs.find.far(Bool.and(String.eq(Refs.kind(word), "bot"), Bool.not(String.is_empty(Refs.far.m(String.drop(word, 1n))))), s, sel, word))

def Refs.has(hs: List<&2, Hit>, +id: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case h <> t:
      Bool.or(String.eq(Hit.id(h), id), Refs.has(t, id))

# hits without repeats, first kept
def Refs.uniq(hs: List<&2, Hit>, +seen: List<&2, Hit>) -> List<&2, Hit>:
  match hs:
    case Nil{}:
      List.reverse(&2, Hit, seen)
    case Hit{+k, +i, +n, +w} <> t:
      Refs.uniq(t, Bool.pick(List<&2, Hit>, Refs.has(seen, i), seen, Hit{k, i, n, w} <> seen))

def Refs.words.go(ws: List<&2, String>, +s: M.State, +sel: String) -> List<&2, Hit>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      List.append(&2, Hit, Bool.pick(List<&2, Hit>, String.is_empty(Refs.kind(w)), Nil{}, Refs.find(s, sel, w)), Refs.words.go(t, s, sel))

# a text's words that start with a sigil (Refs.kind), in order: one walk
type RW is Data:
  RW{cur: String, acc: List<&2, String>}

def Refs.sig.keep.k(none: Bool, +w: String, acc: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      acc
    case False{}:
      w <> acc

def Refs.sig.keep(+w: String, acc: List<&2, String>) -> List<&2, String>:
  Refs.sig.keep.k(String.is_empty(Refs.kind(w)), w, acc)

def Refs.rw.step(sep: Bool, +c: U32, st: RW) -> RW:
  match sep st:
    case True{} RW{cur, acc}:
      RW{SNil{}, Refs.sig.keep(String.reverse(cur), acc)}
    case False{} RW{cur, acc}:
      RW{SCon{Chr{c}, cur}, acc}

def Refs.rw.go(s: String, st: RW) -> List<&2, String>:
  match s st:
    case SNil{} RW{cur, acc}:
      List.reverse(&2, String, Refs.sig.keep(String.reverse(cur), acc))
    case SCon{Chr{+c}, t} _:
      Refs.rw.go(t, Refs.rw.step(Refs.sep(c), c, st))

def Refs.sigil.words(+text: String) -> List<&2, String>:
  Refs.rw.go(text, RW{SNil{}, Nil{}})

# everything a message sent in thread sel names, in order, each once
def Refs.found(+s: M.State, +sel: String, +text: String) -> List<&2, Hit>:
  Refs.uniq(Refs.words.go(Refs.sigil.words(text), s, sel), Nil{})

# the hits of one kind
def Refs.only(hs: List<&2, Hit>, +kind: String) -> List<&2, Hit>:
  match hs:
    case Nil{}:
      Nil{}
    case Hit{+k, +i, +n, +w} <> t:
      Refs.cons(Hit, String.eq(k, kind), Hit{k, i, n, w}, Refs.only(t, kind))

def Refs.line.go(i: Nat, +s: M.State, +id: String, +name: String, +w: String) -> String:
  match i:
    case 0n:
      Bool.pick(String, String.contains(id, "@"),
        w ++ " is the bot \"" ++ name ++ "\" on the linked machine " ++ Refs.far.m(id) ++ "; it has been sent this message, cannot read this thread, and answers in the person's conversation with it.",
        w ++ " is the bot \"" ++ name ++ "\"; it has been sent this message and may answer in this thread.")
    case 1n:
      w ++ " is the thread \"" ++ name ++ "\" (id " ++ id ++ ", project \"" ++ Refs.proj.title(s, Refs.t.project(Sum.thread.of(s, id))) ++ "\")" ++
        Bool.pick(String, String.is_empty(Sum.of(s, id)), "", ": " ++ Sum.of(s, id)) ++ ". Read it with thread_read."
    case 2n:
      w ++ " is the project \"" ++ name ++ "\" (id " ++ id ++ ", folder " ++ Refs.p.root(Sum.project.of(s, id)) ++ ")" ++
        Bool.pick(String, String.is_empty(Sum.of(s, id)), "", ": " ++ Sum.of(s, id)) ++ "."
    case _:
      w ++ " is the group chat \"" ++ name ++ "\"; this message was posted there."

# What the agent reads
# --------------------
# The timeline keeps a message as it was sent; the agent's copy ends with
# a block naming what it references, so it can reach them (thread_read,
# thread_launch's project, a bot by name).

def Refs.line.k(i: Nat, +s: M.State, +h: Hit) -> String:
  match h:
    case Hit{_, +id, +name, +w}:
      Refs.line.go(i, s, id, name, w)

def Refs.hit.kind(h: Hit) -> String:
  match h:
    case Hit{k, _, _, _}:
      k

def Refs.lines(hs: List<&2, Hit>, +s: M.State) -> List<&2, String>:
  match hs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Refs.line.k(Refs.kind.pos(Refs.hit.kind(h)), s, h) <> Refs.lines(t, s)

def Refs.block.k(+hs: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, hs), "", "\n\n<references>\n" ++ String.join(hs, "\n") ++ "\n</references>")

# whether a text may hold a reference at all (a cheap test first)
def Refs.maybe(+text: String) -> Bool:
  Bool.or(Bool.or(String.contains(text, "@"), String.contains(text, ">")), Bool.or(String.contains(text, "%"), String.contains(text, "#")))

# a message as its agent in thread sel reads it
def Refs.agent(+s: M.State, +sel: String, +text: String) -> String:
  text ++ Refs.block.k(Refs.lines(Refs.found(s, sel, text), s))
