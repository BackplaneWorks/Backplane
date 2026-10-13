import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/notice.bend as NO
import ../core/desk.bend as DK
import ../core/client.bend as U

# What the phone tells its owner: an alert when a turn ends, and the
# Dynamic Island (iOS Live Activity, Android promoted notification) while
# turns run. The apps only post what these defs return.

# the kind of alert a turn's end raises: "done", "fail", or "" for none.
# Only a turn seen running can alert (law alert_needs_running), a running
# turn never does (law alert_never_running), and a stop the user asked for
# is not news.
def Note.kind(was: Bool, now: M.TurnState) -> String:
  match was now:
    case False{} _:
      ""
    case True{} M.Completed{}:
      "done"
    case True{} M.Idle{}:
      "done"
    case True{} M.Failed{}:
      "fail"
    case True{} _:
      ""

def Note.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

# is any thread running (the island shows, and the Live Activity lives,
# exactly while one is: law island_idle_empty)
def Note.busy(ts: List<&2, M.Thread>) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case M.Thread{_, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Bool.or(Note.running(turn), Note.busy(t))

# which threads were running, taken before a server message is folded in
type Seen is Data:
  Seen{id: String, running: Bool}

def Note.seen(ts: List<&2, M.Thread>) -> List<&2, Seen>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{id, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Seen{id, Note.running(turn)} <> Note.seen(t)

# what was seen of only the threads a frame touches (any other thread's
# turn cannot change, so it raises nothing either way: law
# alerts_ids_same)
def Note.seen.ids(ids: List<&2, String>, +st: M.State) -> List<&2, Seen>:
  match ids:
    case Nil{}:
      Nil{}
    case +id <> t:
      Seen{id, M.State.running.k(M.State.thread.get(st, id))} <> Note.seen.ids(t, st)

def Note.was.k(hit: Bool, r: Bool, acc: Bool) -> Bool:
  match hit:
    case True{}:
      r
    case False{}:
      acc

# (a tail walk that stops at the thread's first entry: once per thread of
# every frame, it was the square of the threads)
def Note.was.go(ss: List<&2, Seen>, +id: String, found: Bool, r: Bool) -> Bool:
  match ss found:
    case _ True{}:
      r
    case Nil{} False{}:
      False{}
    case Seen{i, x} <> t False{}:
      Note.was.go(t, id, String.eq(i, id), x)

def Note.was(ss: List<&2, Seen>, +id: String) -> Bool:
  Note.was.go(ss, id, False{}, False{})

# the newest entry text of a thread (entries are kept newest first):
# its last assistant message, or its last tool action
def Note.pick(hit: Bool, +text: String, +acc: String) -> String:
  match hit:
    case True{}:
      Bool.pick(String, String.is_empty(acc), text, acc)
    case False{}:
      acc

# entries are newest first, so each walk stops at its thread's first
# match (done): walking the whole log per thread made every phone screen
# cost seconds on a hub with a long history
def Note.said(es: List<&2, M.Entry>, +id: String, done: Bool, +acc: String) -> String:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{_, +th, M.Assistant{}, +text, _} <> t False{}:
      Note.said(t, id, String.eq(th, id), Note.pick(String.eq(th, id), text, acc))
    case _ <> t False{}:
      Note.said(t, id, False{}, acc)

def Note.doing(es: List<&2, M.Entry>, +id: String, done: Bool, +acc: String) -> String:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Act{_, +th, _, kind, summary, _} <> t False{}:
      Note.doing(t, id, String.eq(th, id), Note.pick(String.eq(th, id), kind ++ " " ++ summary, acc))
    case _ <> t False{}:
      Note.doing(t, id, False{}, acc)

def Note.clip(+s: String, +n: Nat) -> String:
  Bool.pick(String, Nat.is_gt(String.length(s), n), String.take(s, n) ++ "…", s)

# the rest after c: nothing once the line ends
def Note.cut(c: Char, t: String) -> String:
  Bool.pick(String, Char.is_eq(c, '\n'), SNil{}, t)

# the first line, at most n characters of it, reversed. It reads only
# those, never the whole text: a reply can be megabytes long, and
# splitting it into lines recursed once per character (the phone's
# JavaScriptCore ran out of stack on a real log)
def Note.upto(n: Nat, s: String, acc: String) -> String:
  match n s:
    case 0n _:
      acc
    case 1n+_ SNil{}:
      acc
    case 1n+m SCon{+h, t}:
      Note.upto(m, Note.cut(h, t), SCon{h, acc})

# one line of at most n characters, for a banner or the island
def Note.line(s: String, +n: Nat) -> String:
  Note.clip(String.trim(String.reverse(Note.upto(Nat.add(n, 1n), String.trim_start(s), SNil{}))), n)

def Note.body(+kind: String, +said: String) -> String:
  Bool.pick(String, String.eq(kind, "fail"), "Failed", Bool.pick(String, String.is_empty(said), "Finished", Note.line(said, 180n)))

# the screenshot of a browser task paused on this thread (a hub path), for
# a banner to show; "" when none waits
def Note.shot.k(browser: Bool, +d: String, rest: String) -> String:
  match browser:
    case True{}:
      J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(d), J.Null{}), "shot", "")
    case False{}:
      rest

def Note.shot.of(as: List<&2, M.Ask>) -> String:
  match as:
    case Nil{}:
      ""
    case M.Ask{_, _, +k, _, +d, _, _, _} <> t:
      Note.shot.k(String.eq(k, "browser"), d, Note.shot.of(t))

def Note.alert.k(+kind: String, +key: String, +id: String, +title: String, +said: String, +image: String, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  J.Json.obj([J.KV{"thread", J.Json.str(id)}, J.KV{"title", J.Json.str(title)}, J.KV{"kind", J.Json.str(kind)},
    J.KV{"body", J.Json.str(Note.body(kind, said))}, J.KV{"key", J.Json.str(key)}, J.KV{"image", J.Json.str(image)}]) <> rest

# a turn that ended: core/notice.bend says whether it is news (a room
# exchange alerts once, on whichever paired hub's bot answered first) and
# under which key the apps show it; what the thread last said is read only
# for a thread that alerts
def Note.alert.n.k(quiet: Bool, +st: M.State, +kind: String, +key: String, +id: String, +title: String, +es: List<&2, M.Entry>, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match quiet:
    case True{}:
      rest
    case False{}:
      Note.alert.k(kind, key, id, title, Note.said(es, id, False{}, ""), Note.shot.of(U.Asks.open(st, id)), rest)

def Note.alert.n(n: NO.Notice, +st: M.State, +id: String, +title: String, +es: List<&2, M.Entry>, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match n:
    case NO.Notice{+kind, +key}:
      Note.alert.n.k(String.is_empty(kind), st, kind, key, id, title, es, rest)

# a thread a desk (the window, a web page) is watching raises no alert
# here: the person is looking at it (core/desk.bend; law alert_watched_quiet)
def Note.alert.w(watched: Bool, +kind: String, +id: String, +title: String, +st: M.State, +es: List<&2, M.Entry>, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match watched:
    case True{}:
      rest
    case False{}:
      Note.alert.n(NO.Notice.of(st, id, String.eq(kind, "fail"), False{}), st, id, title, es, rest)

def Note.alert.q(none: Bool, +kind: String, +id: String, +title: String, +st: M.State, +es: List<&2, M.Entry>, +desk: String, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match none:
    case True{}:
      rest
    case False{}:
      Note.alert.w(DK.Desk.watched(desk, id), kind, id, title, st, es, rest)

def Note.alert(+kind: String, +id: String, +title: String, +st: M.State, +es: List<&2, M.Entry>, +desk: String, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  Note.alert.q(String.is_empty(kind), kind, id, title, st, es, desk, rest)

# the alerts a server message raised: every thread seen running before it
# that has now finished or failed, and is news (st is the hub's state;
# desk the threads a desk watches, the hub's info "desk")
def Note.alerts(ts: List<&2, M.Thread>, +seen: List<&2, Seen>, +st: M.State, +es: List<&2, M.Entry>, +desk: String) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, _, title, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Note.alert(Note.kind(Note.was(seen, id), turn), id, title, st, es, desk, Note.alerts(t, seen, st, es, desk))

def Note.doing.line(+d: String) -> String:
  Note.line(Bool.pick(String, String.is_empty(d), "Working", d), 80n)

# one running thread's island line; an idle thread reads no entries
def Note.one(running: Bool, +id: String, +title: String, +es: List<&2, M.Entry>) -> List<&2, J.Json>:
  match running:
    case True{}:
      [J.Json.obj([J.KV{"thread", J.Json.str(id)}, J.KV{"title", J.Json.str(Note.line(title, 60n))},
        J.KV{"doing", J.Json.str(Note.doing.line(Note.doing(es, id, False{}, "")))}])]
    case False{}:
      Nil{}

# the island: every running thread with what it is doing now
def Note.lines(ts: List<&2, M.Thread>, +es: List<&2, M.Entry>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, _, title, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      List.append(&2, J.Json, Note.one(Note.running(turn), id, title, es), Note.lines(t, es))

def Note.headline(n: Nat, +first: String) -> String:
  match n:
    case 0n:
      "All agents idle"
    case 1n:
      first
    case _:
      Nat.show(n) ++ " agents working"

def Note.first(+ls: List<&2, J.Json>) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, ls), J.Null{}), "title", "")

def Note.island.k(+ls: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"running", J.Json.nat(List.length(&2, J.Json, ls))},
    J.KV{"headline", J.Json.str(Note.headline(List.length(&2, J.Json, ls), Note.first(ls)))},
    J.KV{"lines", J.Json.arr(ls)}])

# the same with a machine's name after the headline ("" for none)
def Note.named(+h: String, +name: String) -> String:
  Bool.pick(String, String.is_empty(name), h, h ++ " · " ++ name)

# (a Live Activity's: every thread counted, the first NO.Notice.island.max()
# listed)
def Note.island.k.named(+ls: List<&2, J.Json>, +name: String) -> J.Json:
  J.Json.obj([J.KV{"running", J.Json.nat(List.length(&2, J.Json, ls))},
    J.KV{"headline", J.Json.str(Note.named(Note.headline(List.length(&2, J.Json, ls), Note.first(ls)), name))},
    J.KV{"lines", J.Json.arr(List.take(&2, J.Json, ls, NO.Notice.island.max()))}])

def Note.island(ts: List<&2, M.Thread>, +es: List<&2, M.Entry>) -> J.Json:
  Note.island.k(Note.lines(ts, es))

# The same from the read model: each thread's own entries (M.State.mine),
# never the whole log per thread. Its entries include what it sent other
# threads, and the walks keep their thread check, so the text is the same
# (law island_doing_mine_same)
def Note.lines.st(ts: List<&2, M.Thread>, +st: M.State) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, _, title, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      List.append(&2, J.Json, Note.one(Note.running(turn), id, title, M.State.mine(st, id)), Note.lines.st(t, st))

def Note.island.st(ts: List<&2, M.Thread>, +st: M.State) -> J.Json:
  Note.island.k(Note.lines.st(ts, st))

def Note.alerts.st(ts: List<&2, M.Thread>, +seen: List<&2, Seen>, +st: M.State, +desk: String) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, _, title, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Note.alert(Note.kind(Note.was(seen, id), turn), id, title, st, M.State.mine(st, id), desk, Note.alerts.st(t, seen, st, desk))

# The island with the work threads wait on (model.bend's Work.busy): a
# thread whose turn ended with a subagent, background shell or watched
# work still out stays on it, "waiting on N", so the Live Activity (iOS)
# and the live service (Android) last until the work is done (law
# island_busy_shows; Note.busy and island_idle_empty keep the running-only
# rule they state)
def Note.waiting(+n: Nat) -> String:
  "waiting on " ++ Nat.show(n)

def Note.one.w(running: Bool, +busy: Nat, +id: String, +title: String, +es: List<&2, M.Entry>) -> List<&2, J.Json>:
  match running:
    case True{}:
      Note.one(True{}, id, title, es)
    case False{}:
      Bool.pick(List<&2, J.Json>, Nat.is_gt(busy, 0n),
        [J.Json.obj([J.KV{"thread", J.Json.str(id)}, J.KV{"title", J.Json.str(Note.line(title, 60n))}, J.KV{"doing", J.Json.str(Note.waiting(busy))}])], Nil{})

# an archived or deleted thread is never on the island (law
# island_gone_hidden), as on the hub's pushed one (push.bend's Island.keep)
def Note.one.g(gone: Bool, running: Bool, +kids: Map<&2, Nat>, +rows: Map<&2, Nat>, +st: M.State, +id: String, +title: String) -> List<&2, J.Json>:
  match gone:
    case True{}:
      Nil{}
    case False{}:
      Note.one.w(running, M.Work.busy.m(kids, rows, id), id, title, M.State.mine(st, id))

# (kids: M.Work.kids.map, the child tasks counted once per screen)
# (rows: M.Work.rows.map, every thread's own rows found in one pass)
def Note.lines.r(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +rows: Map<&2, Nat>, +st: M.State) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, _, title, _, _, turn, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de} <> t:
      List.append(&2, J.Json, Note.one.g(Bool.or(ar, de), Note.running(turn), kids, rows, st, id, title), Note.lines.r(t, kids, rows, st))

def Note.lines.w(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +st: M.State) -> List<&2, J.Json>:
  Note.lines.r(ts, kids, M.Work.rows.map(st), st)

def Note.island.w(ts: List<&2, M.Thread>, +st: M.State) -> J.Json:
  Note.island.k(Note.lines.r(ts, M.Work.kids.map(st), M.Work.rows.map(st), st))

# A hub the phone no longer reaches keeps its last state in the phone, and
# a thread running then reads running forever (a hub stopped, or paired at
# an address it left). Its threads stay on the island while it answers and
# for as long after it was last heard as the hub's own pushed island holds
# (push.bend's Island.fresh), then go (laws island_unheard_*)
def Note.fresh() -> Nat:
  10800n

def Note.heard.in(online: Bool, +now: Nat, +heard: Nat, +fresh: Nat) -> Bool:
  match online:
    case True{}:
      True{}
    case False{}:
      Nat.is_lt(Nat.sub(now, heard), fresh)

def Note.heard(online: Bool, +now: Nat, +heard: Nat) -> Bool:
  Note.heard.in(online, now, heard, Note.fresh())

def Note.island.h(heard: Bool, ts: List<&2, M.Thread>, +st: M.State) -> J.Json:
  match heard:
    case True{}:
      Note.island.w(ts, st)
    case False{}:
      Note.island.k(Nil{})

def Note.busy.k(running: Bool, +busy: Nat) -> Bool:
  match running:
    case True{}:
      True{}
    case False{}:
      Nat.is_gt(busy, 0n)

# whether any thread runs or waits on work
def Note.busy.st(ts: List<&2, M.Thread>, +st: M.State) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case M.Thread{+id, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Bool.or(Note.busy.k(Note.running(turn), M.Work.busy(st, id)), Note.busy.st(t, st))
