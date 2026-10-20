import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./model.bend as M

# Tool calls folded
# -----------------
# A run of consecutive tool calls (Act entries of tone "tool") shows as one
# row, "6 tool calls · shell, edit, read"; a click opens it. Errors,
# messages and any other activity are never folded and end a run.
# Grouping loses no entry and keeps the order (law fold_keeps_entries), and
# a run holds only tool calls (law fold_runs_only_tools).
#
# A run is open when the user opened it ("fold:<first id>" is "1" in the
# client's info map), shut when they shut it ("0"), and otherwise open only
# while it is the live one: the last thing in a thread whose turn runs.
#
# Only tool calls fold: a turn's messages always show (law
# fold_roles_message_shown). The turn fold below ("Worked for …", Work.*)
# is no longer applied; it hid the agent's messages between tool runs.

type Grp is Data:
  GOne{e: M.Entry}
  GRun{es: List<&2, M.Entry>}

# a tool call, or a design step one made (core/hist.bend: it folds with
# the calls around it)
def Fold.tool(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      Bool.or(String.eq(tone, "tool"), String.eq(tone, "step"))
    case M.Msg{_, _, _, _, _}:
      False{}

def Fold.step(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      String.eq(tone, "step")
    case M.Msg{_, _, _, _, _}:
      False{}

def Fold.steps(es: List<&2, M.Entry>) -> Nat:
  match es:
    case Nil{}:
      0n
    case +e <> t:
      Nat.add(Bool.pick(Nat, Fold.step(e), 1n, 0n), Fold.steps(t))

def Fold.calls.k(step: Bool, e: M.Entry, rest: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match step:
    case True{}:
      rest
    case False{}:
      e <> rest

# the tool calls of a run, its steps left out
def Fold.calls(es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Fold.calls.k(Fold.step(e), e, Fold.calls(t))

# an entry in front of the groups after it: a tool call joins the run that
# starts there
def Fold.push.k(b: Bool, e: M.Entry, gs: List<&2, Grp>) -> List<&2, Grp>:
  match b gs:
    case True{} GRun{xs} <> t:
      GRun{e <> xs} <> t
    case True{} _:
      GRun{[e]} <> gs
    case False{} _:
      GOne{e} <> gs

def Fold.push(+e: M.Entry, gs: List<&2, Grp>) -> List<&2, Grp>:
  Fold.push.k(Fold.tool(e), e, gs)

# a thread's entries (oldest first) as groups
def Fold.groups(es: List<&2, M.Entry>) -> List<&2, Grp>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Fold.push(e, Fold.groups(t))

# the entries back, in order
def Fold.flat(gs: List<&2, Grp>) -> List<&2, M.Entry>:
  match gs:
    case Nil{}:
      Nil{}
    case GOne{e} <> t:
      e <> Fold.flat(t)
    case GRun{xs} <> t:
      List.append(&2, M.Entry, xs, Fold.flat(t))

# how many entries the groups hold
def Fold.count(gs: List<&2, Grp>) -> Nat:
  List.length(&2, M.Entry, Fold.flat(gs))

# every entry that sits in a run
def Fold.runs(gs: List<&2, Grp>) -> List<&2, M.Entry>:
  match gs:
    case Nil{}:
      Nil{}
    case GOne{_} <> t:
      Fold.runs(t)
    case GRun{xs} <> t:
      List.append(&2, M.Entry, xs, Fold.runs(t))

def Fold.all.and(b: Bool, rest: Bool) -> Bool:
  match b:
    case True{}:
      rest
    case False{}:
      False{}

# whether every entry is a tool call
def Fold.all(es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}:
      True{}
    case +e <> t:
      Fold.all.and(Fold.tool(e), Fold.all(t))

# Labels
# ------

def Fold.id(e: M.Entry) -> String:
  match e:
    case M.Msg{i, _, _, _, _}:
      i
    case M.Act{i, _, _, _, _, _}:
      i

def Fold.kind(e: M.Entry) -> String:
  match e:
    case M.Msg{_, _, _, _, _}:
      ""
    case M.Act{_, _, _, k, _, _}:
      k

def Fold.has(xs: List<&2, String>, +k: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, k), Fold.has(t, k))

def Fold.add.k(has: Bool, +k: String, acc: List<&2, String>) -> List<&2, String>:
  match has:
    case True{}:
      acc
    case False{}:
      List.append(&2, String, acc, [k])

# the distinct kinds, in the order first seen
def Fold.kinds(es: List<&2, M.Entry>, +acc: List<&2, String>) -> List<&2, String>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Fold.kinds(t, Fold.add.k(Fold.has(acc, Fold.kind(e)), Fold.kind(e), acc))

def Fold.first(k: Nat, xs: List<&2, String>) -> List<&2, String>:
  match k xs:
    case 0n _:
      Nil{}
    case 1n+j x <> t:
      x <> Fold.first(j, t)
    case 1n+j Nil{}:
      Nil{}

def Fold.calls.label(+l: I.Loc, +n: Nat, +ks: List<&2, String>) -> String:
  MSG.T.fold.calls(l, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other"))
    ++ Bool.pick(String, List.is_empty(&2, String, ks), "", " · " ++ String.join(Fold.first(3n, ks), ", ")
    ++ Bool.pick(String, Nat.is_gt(List.length(&2, String, ks), 3n), MSG.T.fold.more(l), ""))

def Fold.steps.label(+l: I.Loc, +n: Nat) -> String:
  MSG.T.fold.steps(l, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other"))

# "6 tool calls · shell, edit, read" (three kinds at most), and how many
# design changes they made
def Fold.label(+l: I.Loc, +es: List<&2, M.Entry>) -> String:
  +n = List.length(&2, M.Entry, Fold.calls(es))
  +d = Fold.steps(es)
  Bool.pick(String, Nat.is_eq(n, 0n), Fold.steps.label(l, d),
    Fold.calls.label(l, n, Fold.kinds(Fold.calls(es), Nil{})) ++ Bool.pick(String, Nat.is_eq(d, 0n), "", " · " ++ Fold.steps.label(l, d)))

# the run's id: its first entry's
def Fold.run_id(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case e <> _:
      Fold.id(e)

# Open or shut
# ------------

def Fold.key(+id: String) -> String:
  "fold:" ++ id

def Fold.user.k(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

# what the user chose for a run ("" for nothing yet)
def Fold.user(m: Map<&2, String>, +id: String) -> String:
  Fold.user.k(Map.get(String, "", m, Fold.key(id)))

# the user's choice ("1" open, "0" shut) or, with none, whether it is live
def Fold.open(+user: String, live: Bool) -> Bool:
  Bool.pick(Bool, String.eq(user, "1"), True{}, Bool.pick(Bool, String.eq(user, "0"), False{}, live))

# a click's value: the state it asks for, then the run's id
def Fold.ask(+open: Bool, +id: String) -> String:
  Bool.pick(String, open, "0", "1") ++ id

# Roles
# -----
# What each entry shows as, one role per entry in order (Fold.flat), for a
# UI that walks its entries: as it is, as the first of a run (the run's
# row, then the entry itself when open) or as a later one (itself when
# open).

type Role is Data:
  RShow{}
  RHead{id: String, label: String, open: Bool}
  RIn{open: Bool}
  # the first entry of a turn's work: the turn's row, then (when open) the
  # entry as its own role says
  RWork{id: String, label: String, open: Bool, inner: Role}

def Fold.ins(es: List<&2, M.Entry>, +open: Bool) -> List<&2, Role>:
  match es:
    case Nil{}:
      Nil{}
    case _ <> t:
      RIn{open} <> Fold.ins(t, open)

def Fold.run.roles(+l: I.Loc, +es: List<&2, M.Entry>, +open: Bool) -> List<&2, Role>:
  match es:
    case Nil{}:
      Nil{}
    case _ <> t:
      RHead{Fold.run_id(es), Fold.label(l, es), open} <> Fold.ins(t, open)

def Fold.last(gs: List<&2, Grp>) -> Bool:
  match gs:
    case Nil{}:
      True{}
    case _ <> _:
      False{}

def Fold.grp.roles(+l: I.Loc, g: Grp, last: Bool, +info: Map<&2, String>, +running: Bool) -> List<&2, Role>:
  match g:
    case GOne{_}:
      [RShow{}]
    case GRun{+xs}:
      Fold.run.roles(l, xs, Fold.open(Fold.user(info, Fold.run_id(xs)), Bool.and(last, running)))

def Fold.roles.go(+l: I.Loc, gs: List<&2, Grp>, +info: Map<&2, String>, +running: Bool) -> List<&2, Role>:
  match gs:
    case Nil{}:
      Nil{}
    case g <> +t:
      List.append(&2, Role, Fold.grp.roles(l, g, Fold.last(t), info, running), Fold.roles.go(l, t, info, running))

# Turns
# -----
# The entries cut at each user message: a turn is the message and what
# came after it (a page may start mid-turn: that part has no message, and
# never folds).

type Seg is Data:
  Seg{head: List<&2, M.Entry>, body: List<&2, M.Entry>}

def Work.is_user(e: M.Entry) -> Bool:
  match e:
    case M.Msg{_, _, M.User{}, _, _}:
      True{}
    case _:
      False{}

def Work.is_reply(e: M.Entry) -> Bool:
  match e:
    case M.Msg{_, _, M.Assistant{}, _, _}:
      True{}
    case _:
      False{}

def Work.at(e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, _, _, at}:
      at
    case M.Act{_, _, _, _, _, at}:
      at

def Work.push.k(u: Bool, e: M.Entry, ss: List<&2, Seg>) -> List<&2, Seg>:
  match u ss:
    case True{} Seg{Nil{}, b} <> t:
      Seg{[e], b} <> t
    case True{} _:
      Seg{[e], Nil{}} <> ss
    case False{} Seg{Nil{}, b} <> t:
      Seg{Nil{}, e <> b} <> t
    case False{} _:
      Seg{Nil{}, [e]} <> ss

def Work.segs(es: List<&2, M.Entry>) -> List<&2, Seg>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Work.push.k(Work.is_user(e), e, Work.segs(t))

# where a turn's last reply is among what followed the message
def Work.last.k(r: Bool, +i: Nat, m: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match r:
    case True{}:
      Some{i}
    case False{}:
      m

def Work.last(b: List<&2, M.Entry>, +i: Nat, m: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match b:
    case Nil{}:
      m
    case +e <> t:
      Work.last(t, 1n+i, Work.last.k(Work.is_reply(e), i, m))

# what each entry of a turn is to its fold
type WRole is Data:
  WPlain{}
  WHead{id: String, label: String, open: Bool}
  WIn{open: Bool}

def Work.plain(xs: List<&2, M.Entry>) -> List<&2, WRole>:
  match xs:
    case Nil{}:
      Nil{}
    case _ <> t:
      WPlain{} <> Work.plain(t)

def Work.ins(xs: List<&2, M.Entry>, +open: Bool) -> List<&2, WRole>:
  match xs:
    case Nil{}:
      Nil{}
    case _ <> t:
      WIn{open} <> Work.ins(t, open)

def Work.block(xs: List<&2, M.Entry>, +id: String, +label: String, +open: Bool) -> List<&2, WRole>:
  match xs:
    case Nil{}:
      Nil{}
    case _ <> t:
      WHead{id, label, open} <> Work.ins(t, open)

def Work.ago(+a: String) -> String:
  Bool.pick(String, String.eq(a, "now"), "just now", a ++ " ago")

# the same in a language: English exactly as Work.ago always read
def Work.ago.loc(l: I.Loc, +now: Nat, +at: Nat) -> String:
  match l:
    case I.En{}:
      Work.ago(M.Time.ago(now, at))
    case I.Ja{}:
      I.I18n.ago.long(I.Ja{}, now, at)
    case I.Sux{}:
      I.I18n.ago.long(I.Sux{}, now, at)

def Work.id(+u: M.Entry) -> String:
  "work:" ++ Fold.id(u)

# when the reply came (d with none)
def Work.end(m: Maybe<&2, M.Entry>, +d: Nat) -> Nat:
  match m:
    case Some{e}:
      Work.at(e)
    case None{}:
      d

# a finished turn: the work before its last reply folds (a turn with no
# reply, stopped or failed, shows all of it)
def Work.done(+l: I.Loc, +m: Maybe<&2, Nat>, +b: List<&2, M.Entry>, +u: M.Entry, +info: Map<&2, String>, +now: Nat) -> List<&2, WRole>:
  match m:
    case Some{1n+j}:
      +end = Work.end(List.head(&2, M.Entry, List.drop(&2, M.Entry, b, 1n+j)), Work.at(u))
      List.append(&2, WRole, Work.block(List.take(&2, M.Entry, b, 1n+j), Work.id(u),
          MSG.T.fold.worked(l, I.I18n.took(l, Nat.sub(end, Work.at(u))), Work.ago.loc(l, now, end)), Fold.open(Fold.user(info, Work.id(u)), False{})),
        Work.plain(List.drop(&2, M.Entry, b, 1n+j)))
    case _:
      Work.plain(b)

# the turn still running: all of it is work, open unless shut
def Work.turn.loc(+l: I.Loc, live: Bool, +u: M.Entry, +b: List<&2, M.Entry>, +info: Map<&2, String>, +now: Nat) -> List<&2, WRole>:
  match live:
    case True{}:
      WPlain{} <> Work.block(b, Work.id(u), MSG.T.fold.working(l, Work.ago.loc(l, now, Work.at(u))), Fold.open(Fold.user(info, Work.id(u)), True{}))
    case False{}:
      WPlain{} <> Work.done(l, Work.last(b, 0n, None{}), b, u, info, now)

def Work.turn(live: Bool, u: M.Entry, b: List<&2, M.Entry>, info: Map<&2, String>, now: Nat) -> List<&2, WRole>:
  Work.turn.loc(I.En{}, live, u, b, info, now)

def Work.seg(+l: I.Loc, s: Seg, live: Bool, +info: Map<&2, String>, +now: Nat) -> List<&2, WRole>:
  match s:
    case Seg{+u <> _, +b}:
      Work.turn.loc(l, live, u, b, info, now)
    case Seg{Nil{}, +b}:
      Work.plain(b)

def Work.lastseg(ss: List<&2, Seg>) -> Bool:
  match ss:
    case Nil{}:
      True{}
    case _ <> _:
      False{}

def Work.roles.go(+l: I.Loc, ss: List<&2, Seg>, +info: Map<&2, String>, +running: Bool, +now: Nat) -> List<&2, WRole>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> +t:
      List.append(&2, WRole, Work.seg(l, s, Bool.and(Work.lastseg(t), running), info, now), Work.roles.go(l, t, info, running, now))

def Work.hide(open: Bool, r: Role) -> Role:
  match open:
    case True{}:
      r
    case False{}:
      RIn{False{}}

# an entry's turn role over its tool-run role
def Work.with(w: WRole, r: Role) -> Role:
  match w:
    case WPlain{}:
      r
    case WHead{id, label, open}:
      RWork{id, label, open, r}
    case WIn{open}:
      Work.hide(open, r)

def Work.zip(ws: List<&2, WRole>, rs: List<&2, Role>) -> List<&2, Role>:
  match ws rs:
    case w <> wt r <> rt:
      Work.with(w, r) <> Work.zip(wt, rt)
    case Nil{} r:
      r
    case +w <> +wt Nil{}:
      Nil{}

# the time the client last ticked ("clock" in its info map), for "5m ago"
def Fold.now(m: Map<&2, String>) -> Nat:
  M.Switch.num(Fold.user.k(Map.get(String, "", m, "clock")), 0n)

# the roles of a thread's entries, with the client's info map and whether
# its turn runs: tool runs fold, nothing else does
def Fold.roles.loc(+l: I.Loc, +es: List<&2, M.Entry>, +info: Map<&2, String>, +running: Bool) -> List<&2, Role>:
  Fold.roles.go(l, Fold.groups(es), info, running)

def Fold.roles(es: List<&2, M.Entry>, info: Map<&2, String>, running: Bool) -> List<&2, Role>:
  Fold.roles.loc(I.En{}, es, info, running)

# Revealing an entry
# ------------------
# To show an entry (the viewer's history goes back to the tool call a
# design step followed), the run of tool calls it sits in opens. A page
# may start inside the run, making a later entry its first, so every
# entry's key is set: whichever heads the run on show opens it. Entries
# oldest first; the keys to set to "1".

def Reveal.has(es: List<&2, M.Entry>, +id: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case +e <> t:
      Bool.or(String.eq(Fold.id(e), id), Reveal.has(t, id))

def Reveal.keys(es: List<&2, M.Entry>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Fold.key(Fold.id(e)) <> Reveal.keys(t)

def Reveal.run(g: Grp, +id: String) -> List<&2, String>:
  match g:
    case GRun{+es}:
      Bool.pick(List<&2, String>, Reveal.has(es, id), Reveal.keys(es), Nil{})
    case GOne{_}:
      Nil{}

def Reveal.runs(gs: List<&2, Grp>, +id: String) -> List<&2, String>:
  match gs:
    case Nil{}:
      Nil{}
    case g <> t:
      List.append(&2, String, Reveal.run(g, id), Reveal.runs(t, id))

def Fold.reveal(+es: List<&2, M.Entry>, +id: String) -> List<&2, String>:
  Reveal.runs(Fold.groups(es), id)
