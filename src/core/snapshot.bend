import Base
import ./json.bend as J
import ./model.bend as M
import ./codec.bend as C
import ./bot.bend as BT
import ./mail.bend as ML
import ./store.bend as ST
import ./storedb.bend as SDB

# The hot snapshot (bp-coord/store/S1-SCHEMA.md, "Agreed with S3")
# ----------------------------------------------------------------
# store.db's record rows read back into a read model, so the hub starts
# from them and folds only the log's tail after applied_off, instead of
# the whole log. Pure: the server's hotboot.bend runs the queries and
# hands their rows here as a map from a table's name to its rows, each
# row its columns in Sdb.cols order (obj rows by domain: "obj.<domain>").
# Each record is decoded from its json column, the inverse of storedb's
# Row.* (laws snap_*_rt). What the fold derives (the entries filed by
# thread, the mail box's orders, counts, live runs, ids and headers) is
# made again from the records and the facts here, never stored.
#
# Entries are whatever rows "entry" holds: the hub loads a recent window
# of the open threads only (hotboot.bend), so M.State.mine is that window.
# One fold step reads a thread's entries (ProviderSet: Switch.count); the
# tail's is given the store's own count ("count" rows: thread, n).

# Numbers and JSON
# ----------------

def Snap.dig(skip: Bool, +c: U32, +acc: Nat) -> Nat:
  match skip:
    case True{}:
      acc
    case False{}:
      Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32)))

# the digits of a text as a number, anything else skipped (a column bound
# as an integer reads back as its digits; the pure store keeps "#<digits>")
def Snap.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Snap.num(t, Snap.dig(Bool.or(U32.is_lt(c, 48), U32.is_gt(c, 57)), c, acc))

def Snap.js(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def Snap.n(+o: J.Json, +k: String) -> Nat:
  Snap.num(J.Json.as_raw(J.Json.get(o, k), ""), 0n)

def Snap.s(+o: J.Json, +k: String) -> String:
  J.Json.get_str(o, k, "")

def Snap.b(+o: J.Json, +k: String) -> Bool:
  J.Json.get_bool(o, k, False{})

def Snap.strs.go(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{s} <> t:
      s <> Snap.strs.go(t)
    case _ <> t:
      Snap.strs.go(t)

def Snap.strs(+o: J.Json, +k: String) -> List<&2, String>:
  Snap.strs.go(J.Json.elems(J.Json.get_obj(o, k)))

def Snap.str.j(o: J.Json) -> String:
  match o:
    case J.Str{s}:
      s
    case _:
      ""

def Snap.nat.j(o: J.Json) -> Nat:
  match o:
    case J.Num{r}:
      Snap.num(r, 0n)
    case _:
      0n

# a row's column (counted from 0), "" past its end
def Snap.col(xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

# a record row's json: its last column
def Snap.last(xs: List<&2, String>) -> String:
  Maybe.default(&2, String, List.last(&2, String, xs), "")

# an op's json column (the laws' round trips)
def Snap.op.json(o: SDB.Op) -> String:
  match o:
    case SDB.Put{_, cols}:
      Snap.last(cols)
    case SDB.Del{_, _}:
      ""

# Records
# -------

def Snap.override(+s: String) -> M.Override:
  Bool.pick(M.Override, String.eq(s, "settled"), M.ForcedSettled{}, Bool.pick(M.Override, String.eq(s, "active"), M.ForcedActive{}, M.NoOverride{}))

def Snap.project.j(+o: J.Json) -> M.Project:
  M.Project{Snap.s(o, "id"), Snap.s(o, "title"), Snap.s(o, "root"), Snap.n(o, "created"), Snap.b(o, "removed")}

def Snap.project(+js: String) -> M.Project:
  Snap.project.j(Snap.js(js))

def Snap.thread.j(+o: J.Json) -> M.Thread:
  M.Thread{Snap.s(o, "id"), Snap.s(o, "project"), Snap.s(o, "title"), Snap.n(o, "created"), Snap.n(o, "activity"), C.Turn.of(Snap.s(o, "turn")),
    Snap.n(o, "pending"), Snap.b(o, "archived"), Snap.override(Snap.s(o, "override")), Snap.n(o, "settled_at"), Snap.n(o, "unsettled_at"),
    Snap.b(o, "pinned"), C.Env.of(Snap.s(o, "env")), Snap.s(o, "branch"), Snap.s(o, "worktree"), Snap.s(o, "provider"), Snap.s(o, "model"),
    C.Runtime.of(Snap.s(o, "runtime")), C.Interaction.of(Snap.s(o, "interaction")), Snap.s(o, "session"), Snap.b(o, "deleted")}

def Snap.thread(+js: String) -> M.Thread:
  Snap.thread.j(Snap.js(js))

def Snap.task.j(+o: J.Json) -> M.Task:
  M.Task{Snap.s(o, "id"), Snap.s(o, "parent"), Snap.s(o, "child"), Snap.n(o, "depth"), C.TaskSt.of(Snap.s(o, "state")), Snap.s(o, "summary"),
    Snap.b(o, "delivered"), Snap.s(o, "provider"), Snap.s(o, "model")}

def Snap.task(+js: String) -> M.Task:
  Snap.task.j(Snap.js(js))

def Snap.ask.j(+o: J.Json) -> M.Ask:
  M.Ask{Snap.s(o, "id"), Snap.s(o, "thread"), Snap.s(o, "kind"), Snap.s(o, "tool"), Snap.s(o, "detail"), Snap.s(o, "options"), Snap.b(o, "open"), Snap.s(o, "answer")}

def Snap.ask(+js: String) -> M.Ask:
  Snap.ask.j(Snap.js(js))

def Snap.qitems(xs: List<&2, J.Json>) -> List<&2, M.QItem>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      M.QItem{Snap.s(x, "msg"), Snap.s(x, "text")} <> Snap.qitems(t)

def Snap.queue(+js: String) -> List<&2, M.QItem>:
  Snap.qitems(J.Json.elems(Snap.js(js)))

def Snap.device.j(+o: J.Json) -> M.Device:
  M.Device{Snap.s(o, "token"), Snap.s(o, "platform"), Snap.s(o, "kind"), Snap.s(o, "thread"), Snap.s(o, "env"), Snap.s(o, "bundle"), Snap.n(o, "at")}

def Snap.device(+js: String) -> M.Device:
  Snap.device.j(Snap.js(js))

def Snap.nav.j(+o: J.Json) -> M.Nav:
  M.Nav{Snap.n(o, "until"), Snap.s(o, "pin"), Snap.s(o, "act")}

def Snap.nav(+js: String) -> M.Nav:
  Snap.nav.j(Snap.js(js))

def Snap.bot.j(+o: J.Json) -> BT.Bot:
  BT.Bot{Snap.s(o, "id"), Snap.s(o, "name"), Snap.s(o, "thread"), Snap.n(o, "look"), Snap.s(o, "persona"), Snap.n(o, "created"), Snap.b(o, "deleted")}

def Snap.bot(+js: String) -> BT.Bot:
  Snap.bot.j(Snap.js(js))

def Snap.room.j(+o: J.Json) -> BT.Room:
  BT.Room{Snap.s(o, "id"), Snap.s(o, "name"), Snap.strs(o, "members"), Snap.n(o, "created"), Snap.b(o, "deleted")}

def Snap.room(+js: String) -> BT.Room:
  Snap.room.j(Snap.js(js))

def Snap.routine.j(+o: J.Json) -> BT.Routine:
  BT.Routine{Snap.s(o, "id"), Snap.s(o, "bot"), Snap.s(o, "name"), Snap.s(o, "cron"), Snap.s(o, "prompt"), Snap.b(o, "on"), Snap.n(o, "last"),
    Snap.n(o, "created"), Snap.b(o, "deleted"), Snap.n(o, "once"), Snap.s(o, "tz"), Snap.s(o, "kind"), Snap.b(o, "done"), Snap.n(o, "slot")}

def Snap.routine(+js: String) -> BT.Routine:
  Snap.routine.j(Snap.js(js))

def Snap.hook.j(+o: J.Json) -> BT.Hook:
  BT.Hook{Snap.s(o, "id"), Snap.s(o, "bot"), Snap.s(o, "name"), Snap.n(o, "created"), Snap.b(o, "revoked"), Snap.n(o, "last"), Snap.n(o, "count")}

def Snap.hook(+js: String) -> BT.Hook:
  Snap.hook.j(Snap.js(js))

def Snap.mem.j(+o: J.Json) -> BT.Mem:
  BT.Mem{Snap.s(o, "bot"), Snap.s(o, "key"), Snap.s(o, "kind"), Snap.s(o, "text"), Snap.s(o, "tags"), Snap.n(o, "created"), Snap.n(o, "updated")}

def Snap.mem(+js: String) -> BT.Mem:
  Snap.mem.j(Snap.js(js))

def Snap.peer.j(+o: J.Json) -> BT.Peer:
  BT.Peer{Snap.s(o, "id"), Snap.s(o, "name"), Snap.s(o, "url"), Snap.n(o, "created"), Snap.b(o, "revoked")}

def Snap.peer(+js: String) -> BT.Peer:
  Snap.peer.j(Snap.js(js))

def Snap.page.j(+o: J.Json) -> BT.Page:
  BT.Page{Snap.s(o, "html"), Snap.n(o, "at")}

def Snap.page(+js: String) -> BT.Page:
  Snap.page.j(Snap.js(js))

def Snap.hops.j(+o: J.Json) -> BT.Hops:
  BT.Hops{Snap.n(o, "now"), Snap.n(o, "wait"), Snap.b(o, "held")}

def Snap.hops(+js: String) -> BT.Hops:
  Snap.hops.j(Snap.js(js))

# a json text that is one string (space, skill, charter, mailname)
def Snap.str(+js: String) -> String:
  Snap.str.j(Snap.js(js))

# a json text that is one number (talk, state.seq)
def Snap.nat(+js: String) -> Nat:
  Snap.nat.j(Snap.js(js))

def Snap.conv.j(+o: J.Json) -> ML.Conv:
  ML.Conv{Snap.s(o, "id"), Snap.s(o, "subject"), Snap.s(o, "kind"), Snap.strs(o, "parts"), Snap.s(o, "project"), Snap.strs(o, "tags"),
    Snap.n(o, "made"), Snap.n(o, "last"), Snap.n(o, "n")}

def Snap.conv(+js: String) -> ML.Conv:
  Snap.conv.j(Snap.js(js))

def Snap.mark.j(+o: J.Json) -> ML.Mark:
  ML.Mark{Snap.s(o, "bot"), Snap.s(o, "conv"), Snap.s(o, "status"), Snap.n(o, "seen"), Snap.n(o, "until"), Snap.strs(o, "tags")}

def Snap.mark(+js: String) -> ML.Mark:
  Snap.mark.j(Snap.js(js))

def Snap.run.j(+o: J.Json) -> ML.Run:
  ML.Run{Snap.s(o, "thread"), Snap.s(o, "bot"), Snap.s(o, "conv"), Snap.s(o, "mail"), Snap.n(o, "at"), Snap.b(o, "live")}

def Snap.run(+js: String) -> ML.Run:
  Snap.run.j(Snap.js(js))

def Snap.origin.j(+o: J.Json) -> ML.Origin:
  ML.Origin{Snap.s(o, "from"), Snap.s(o, "kind"), Snap.s(o, "ref")}

def Snap.origin(+js: String) -> ML.Origin:
  Snap.origin.j(Snap.js(js))

def Snap.decisions.go(xs: List<&2, J.Json>) -> List<&2, ML.Decision>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      ML.Decision{Snap.s(o, "id"), Snap.s(o, "bot"), Snap.s(o, "conv"), Snap.s(o, "run"), Snap.s(o, "text"), Snap.s(o, "reason"), Snap.n(o, "at")} <> Snap.decisions.go(t)

def Snap.decisions(+js: String) -> List<&2, ML.Decision>:
  Snap.decisions.go(J.Json.elems(Snap.js(js)))

# Facts
# -----

def Snap.entry.k(msg: Bool, id: String, th: String, r: String, ak: String, text: String, +at: Nat) -> M.Entry:
  match msg:
    case True{}:
      M.Msg{id, th, C.Role.of(r), text, at}
    case False{}:
      M.Act{id, th, r, ak, text, at}

# an entry row: seq, ord, id, thread, src, kind, role, akind, text, at
def Snap.entry(+r: List<&2, String>) -> M.Entry:
  Snap.entry.k(String.eq(Snap.col(r, 5n), "msg"), Snap.col(r, 2n), Snap.col(r, 3n), Snap.col(r, 6n), Snap.col(r, 7n), Snap.col(r, 8n), Snap.num(Snap.col(r, 9n), 0n))

# a mail row: seq, id, conv, src, dst, kind, ref, want, head, size, at
def Snap.mail(+r: List<&2, String>) -> ML.Mail:
  ML.Mail{Snap.col(r, 1n), Snap.col(r, 2n), Snap.col(r, 3n), Snap.col(r, 4n), Snap.col(r, 5n), Snap.col(r, 6n), Snap.col(r, 7n), Snap.col(r, 8n),
    Snap.num(Snap.col(r, 9n), 0n), Snap.num(Snap.col(r, 10n), 0n)}

# a post row: seq, id, room, src, text, hop, at
def Snap.post(+r: List<&2, String>) -> BT.Post:
  BT.Post{Snap.col(r, 1n), Snap.col(r, 2n), Snap.col(r, 3n), Snap.col(r, 4n), Snap.num(Snap.col(r, 5n), 0n), Snap.num(Snap.col(r, 6n), 0n)}

# Tables into maps (one walk each: a walk cannot take the decoder as an
# argument, a function is not Data)
# ---------------------------------------------------------------------

type SRows is Data:
  SRows{t: Map<&2, List<&2, List<&2, String>>>}

def SRows.get.k(r: Map<&2, List<&2, List<&2, String>>> & List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  (_, v) = r
  v

# a table's rows ("obj.<domain>" for a domain of obj), in the order given
def SRows.of(+rs: SRows, +name: String) -> List<&2, List<&2, String>>:
  match rs:
    case SRows{+t}:
      SRows.get.k(Map.get(List<&2, List<&2, String>>, Nil{}, t, name))

def Fill.projects(rows: List<&2, List<&2, String>>, m: Map<&2, M.Project>) -> Map<&2, M.Project>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.projects(t, Map.set(&2, M.Project, m, Snap.col(r, 0n), Snap.project(Snap.last(r))))

def Fill.threads(rows: List<&2, List<&2, String>>, m: Map<&2, M.Thread>) -> Map<&2, M.Thread>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.threads(t, Map.set(&2, M.Thread, m, Snap.col(r, 0n), Snap.thread(Snap.last(r))))

def Fill.tasks(rows: List<&2, List<&2, String>>, m: Map<&2, M.Task>) -> Map<&2, M.Task>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.tasks(t, Map.set(&2, M.Task, m, Snap.col(r, 0n), Snap.task(Snap.last(r))))

def Fill.asks(rows: List<&2, List<&2, String>>, m: Map<&2, M.Ask>) -> Map<&2, M.Ask>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.asks(t, Map.set(&2, M.Ask, m, Snap.col(r, 0n), Snap.ask(Snap.last(r))))

def Fill.queues(rows: List<&2, List<&2, String>>, m: Map<&2, List<&2, M.QItem>>) -> Map<&2, List<&2, M.QItem>>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.queues(t, Map.set(&2, List<&2, M.QItem>, m, Snap.col(r, 0n), Snap.queue(Snap.last(r))))

# setting rows: k, v (, cold)
def Fill.settings(rows: List<&2, List<&2, String>>, m: Map<&2, String>) -> Map<&2, String>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.settings(t, Map.set(&2, String, m, Snap.col(r, 0n), Snap.col(r, 1n)))

# obj rows: domain, key, json
def Fill.devices(rows: List<&2, List<&2, String>>, m: Map<&2, M.Device>) -> Map<&2, M.Device>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.devices(t, Map.set(&2, M.Device, m, Snap.col(r, 1n), Snap.device(Snap.last(r))))

def Fill.navs(rows: List<&2, List<&2, String>>, m: Map<&2, M.Nav>) -> Map<&2, M.Nav>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.navs(t, Map.set(&2, M.Nav, m, Snap.col(r, 1n), Snap.nav(Snap.last(r))))

def Fill.bots(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Bot>) -> Map<&2, BT.Bot>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.bots(t, Map.set(&2, BT.Bot, m, Snap.col(r, 0n), Snap.bot(Snap.last(r))))

def Fill.rooms(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Room>) -> Map<&2, BT.Room>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.rooms(t, Map.set(&2, BT.Room, m, Snap.col(r, 0n), Snap.room(Snap.last(r))))

def Fill.routines(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Routine>) -> Map<&2, BT.Routine>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.routines(t, Map.set(&2, BT.Routine, m, Snap.col(r, 0n), Snap.routine(Snap.last(r))))

def Fill.hooks(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Hook>) -> Map<&2, BT.Hook>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.hooks(t, Map.set(&2, BT.Hook, m, Snap.col(r, 1n), Snap.hook(Snap.last(r))))

def Fill.mems(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Mem>) -> Map<&2, BT.Mem>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.mems(t, Map.set(&2, BT.Mem, m, Snap.col(r, 1n), Snap.mem(Snap.last(r))))

def Fill.peers(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Peer>) -> Map<&2, BT.Peer>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.peers(t, Map.set(&2, BT.Peer, m, Snap.col(r, 1n), Snap.peer(Snap.last(r))))

def Fill.pages(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Page>) -> Map<&2, BT.Page>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.pages(t, Map.set(&2, BT.Page, m, Snap.col(r, 1n), Snap.page(Snap.last(r))))

def Fill.hopss(rows: List<&2, List<&2, String>>, m: Map<&2, BT.Hops>) -> Map<&2, BT.Hops>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.hopss(t, Map.set(&2, BT.Hops, m, Snap.col(r, 1n), Snap.hops(Snap.last(r))))

# obj rows whose json is one string
def Fill.strs(rows: List<&2, List<&2, String>>, m: Map<&2, String>) -> Map<&2, String>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.strs(t, Map.set(&2, String, m, Snap.col(r, 1n), Snap.str(Snap.last(r))))

# obj rows whose json is one number
def Fill.nats(rows: List<&2, List<&2, String>>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.nats(t, Map.set(&2, Nat, m, Snap.col(r, 1n), Snap.nat(Snap.last(r))))

def Fill.origins(rows: List<&2, List<&2, String>>, m: Map<&2, ML.Origin>) -> Map<&2, ML.Origin>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.origins(t, Map.set(&2, ML.Origin, m, Snap.col(r, 1n), Snap.origin(Snap.last(r))))

def Fill.decisions(rows: List<&2, List<&2, String>>, m: Map<&2, List<&2, ML.Decision>>) -> Map<&2, List<&2, ML.Decision>>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.decisions(t, Map.set(&2, List<&2, ML.Decision>, m, Snap.col(r, 1n), Snap.decisions(Snap.last(r))))

# "count" rows: thread, how many entries it has in the store
def Fill.counts(rows: List<&2, List<&2, String>>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Fill.counts(t, Map.set(&2, Nat, m, Snap.col(r, 0n), Snap.num(Snap.col(r, 1n), 0n)))

# room posts, oldest row first: the world keeps them newest first
def Fill.posts(rows: List<&2, List<&2, String>>, acc: List<&2, BT.Post>) -> List<&2, BT.Post>:
  match rows:
    case Nil{}:
      acc
    case +r <> t:
      Fill.posts(t, Snap.post(r) <> acc)

# Entries
# -------

type EB is Data:
  EB{es: List<&2, M.Entry>, by: Map<&2, List<&2, M.Entry>>}

# entry rows, oldest first: the state's list (newest first) and the
# entries filed by thread, as the fold files them (Idx.post)
def Fill.entries(rows: List<&2, List<&2, String>>, es: List<&2, M.Entry>, by: Map<&2, List<&2, M.Entry>>) -> EB:
  match rows:
    case Nil{}:
      EB{es, by}
    case +r <> t:
      +e = Snap.entry(r)
      Fill.entries(t, e <> es, M.Idx.post(by, e))

# The mail box
# ------------
# Records (conversations, marks, runs, origins, charters, decisions,
# names) as they are; then what the fold derives from them: a mark's
# unread count and snooze (Box.mark), a conversation's orders (Box.conv_put,
# after the marks, so it makes none), a live run's slot and count
# (Box.run_store), and each header filed newest first (Headers.add in the
# order the log has them) with its id.

def Mbox.marks(rows: List<&2, List<&2, String>>, b: ML.Box) -> ML.Box:
  match rows:
    case Nil{}:
      b
    case +r <> t:
      Mbox.marks(t, ML.Box.mark(b, Snap.mark(Snap.last(r))))

def Mbox.convs(rows: List<&2, List<&2, String>>, b: ML.Box) -> ML.Box:
  match rows:
    case Nil{}:
      b
    case +r <> t:
      Mbox.convs(t, ML.Box.conv_put(b, Snap.conv(Snap.last(r))))

def Mbox.run.k(live: Bool, +b: ML.Box, +r: ML.Run) -> ML.Box:
  match live:
    case True{}:
      ML.Box.run_store(b, r)
    case False{}:
      ML.Box.with_runs(b, Map.set(&2, ML.Run, ML.Box.runs(b), ML.Run.thread(r), r))

def Mbox.runs(rows: List<&2, List<&2, String>>, b: ML.Box) -> ML.Box:
  match rows:
    case Nil{}:
      b
    case +r <> t:
      +run = Snap.run(Snap.last(r))
      Mbox.runs(t, Mbox.run.k(ML.Run.live(run), b, run))

def Mbox.header(+b: ML.Box, +m: ML.Mail) -> ML.Box:
  ML.Box.with_ids(ML.Box.with_mails(b, Map.set(&2, List<&2, ML.Mail>, ML.Box.mails(b), ML.Mail.conv(m), ML.Headers.add(m, ML.Box.headers(b, ML.Mail.conv(m))))),
    Map.set(&2, Bool, ML.Box.ids(b), ML.Mail.id(m), True{}))

def Mbox.headers(rows: List<&2, List<&2, String>>, b: ML.Box) -> ML.Box:
  match rows:
    case Nil{}:
      b
    case +r <> t:
      Mbox.headers(t, Mbox.header(b, Snap.mail(r)))

def Snap.box(+rs: SRows) -> ML.Box:
  +b0 = ML.Box.with_names(ML.Box.with_decision_log(ML.Box.with_charters(ML.Box.with_origins(ML.Box.empty(),
    Fill.origins(SRows.of(rs, "obj.origin"), Map.new(&2, ML.Origin))),
    Fill.strs(SRows.of(rs, "obj.charter"), Map.new(&2, String))),
    Fill.decisions(SRows.of(rs, "obj.decision"), Map.new(&2, List<&2, ML.Decision>))),
    Fill.strs(SRows.of(rs, "obj.mailname"), Map.new(&2, String)))
  Mbox.headers(SRows.of(rs, "mail"), Mbox.runs(SRows.of(rs, "obj.run"), Mbox.convs(SRows.of(rs, "conv"), Mbox.marks(SRows.of(rs, "mark"), b0))))

def Snap.world(+rs: SRows) -> BT.World:
  BT.World{Fill.bots(SRows.of(rs, "bot"), Map.new(&2, BT.Bot)), Fill.rooms(SRows.of(rs, "room"), Map.new(&2, BT.Room)), Fill.posts(SRows.of(rs, "post"), Nil{}),
    Fill.hooks(SRows.of(rs, "obj.hook"), Map.new(&2, BT.Hook)), Fill.routines(SRows.of(rs, "routine"), Map.new(&2, BT.Routine)),
    Fill.mems(SRows.of(rs, "obj.mem"), Map.new(&2, BT.Mem)), Fill.peers(SRows.of(rs, "obj.peer"), Map.new(&2, BT.Peer)),
    Fill.strs(SRows.of(rs, "obj.space"), Map.new(&2, String)), Fill.hopss(SRows.of(rs, "obj.hops"), Map.new(&2, BT.Hops)),
    Fill.nats(SRows.of(rs, "obj.talk"), Map.new(&2, Nat)), Fill.pages(SRows.of(rs, "obj.page"), Map.new(&2, BT.Page)),
    Fill.strs(SRows.of(rs, "obj.skill"), Map.new(&2, String))}

def Snap.seq.k(xs: List<&2, List<&2, String>>) -> Nat:
  match xs:
    case r <> _:
      Snap.nat(Snap.last(r))
    case Nil{}:
      0n

def Snap.state.k(+rs: SRows, eb: EB) -> M.State:
  match eb:
    case EB{es, by}:
      M.State{Fill.projects(SRows.of(rs, "project"), Map.new(&2, M.Project)), Fill.threads(SRows.of(rs, "thread"), Map.new(&2, M.Thread)), es,
        Snap.seq.k(SRows.of(rs, "obj.state")), Fill.tasks(SRows.of(rs, "task"), Map.new(&2, M.Task)), Fill.settings(SRows.of(rs, "setting"), Map.new(&2, String)),
        Fill.queues(SRows.of(rs, "queue"), Map.new(&2, List<&2, M.QItem>)), Fill.asks(SRows.of(rs, "ask"), Map.new(&2, M.Ask)),
        Fill.devices(SRows.of(rs, "obj.device"), Map.new(&2, M.Device)), Fill.navs(SRows.of(rs, "obj.nav"), Map.new(&2, M.Nav)),
        [M.Ext{Snap.world(rs), by, [Snap.box(rs)]}]}

# the read model the rows hold (entries: the rows of "entry" there are)
def Snap.state(+rs: SRows) -> M.State:
  Snap.state.k(rs, Fill.entries(SRows.of(rs, "entry"), Nil{}, Map.new(&2, List<&2, M.Entry>)))

# each thread's entry count in the store
def Snap.counts(+rs: SRows) -> Map<&2, Nat>:
  Fill.counts(SRows.of(rs, "count"), Map.new(&2, Nat))

# The tail
# --------
# The log after applied_off folded onto the loaded state as the full replay
# folds it, but for ProviderSet, whose count of the thread's entries is the
# store's count plus what the tail posted since (s0: the state as loaded).

def Snap.get.n(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

def Snap.count(+st: M.State, +s0: M.State, +cs: Map<&2, Nat>, +th: String) -> Nat:
  Nat.sub(Nat.add(M.Switch.count(st, th), Snap.get.n(Map.get(Nat, 0n, cs, th))), M.Switch.count(s0, th))

def Snap.apply.c(+c: M.Change, +s0: M.State, +cs: Map<&2, Nat>, +st: M.State) -> M.State:
  match c:
    case M.ProviderSet{+id, +p, m, _}:
      M.State.tick(M.State.switch.c(st, id, p, m, Snap.count(st, s0, cs, id)))
    case _:
      M.State.apply(st, c)

def Snap.apply(m: Maybe<&2, M.Change>, +s0: M.State, +cs: Map<&2, Nat>, +st: M.State) -> M.State:
  match m:
    case Some{c}:
      Snap.apply.c(c, s0, cs, st)
    case None{}:
      st

# changes onto the loaded state (the laws and the pure harness)
def Snap.replay(xs: List<&2, M.Change>, +s0: M.State, +cs: Map<&2, Nat>, st: M.State) -> M.State:
  match xs:
    case Nil{}:
      st
    case c <> t:
      Snap.replay(t, s0, cs, Snap.apply(Some{c}, s0, cs, st))

def Snap.step(m: Maybe<&2, J.Json>, l: String, +s0: M.State, +cs: Map<&2, Nat>, a: ST.Loaded) -> ST.Loaded:
  match m a:
    case Some{+o} ST.Loaded{st, ws}:
      ST.Loaded{Snap.apply(C.Change.decode(o), s0, cs, st), C.Change.wire_line(o, l) <> ws}
    case None{} _:
      a

# log lines, oldest first, as ST.Store.load reads them (the wire lines
# clients see go on the front of a's, newest first)
def Snap.tail(ls: List<&2, String>, +s0: M.State, +cs: Map<&2, Nat>, a: ST.Loaded) -> ST.Loaded:
  match ls:
    case Nil{}:
      a
    case +l <> t:
      Snap.tail(t, s0, cs, Snap.step(J.Json.parse(l), l, s0, cs, a))

# The pure store as rows (the laws and test/snap_equiv_test.bend)
# ----------------------------------------------------------------
# The pure store's maps ("<table>\x1f<key>" to a row's columns joined by
# \x1f) as the rows hotboot.bend's queries answer: facts by seq, obj by
# domain, and each thread's entry count.

def Snap.pad(+s: String) -> String:
  SDB.Tab.pad.go(Nat.sub(20n, String.length(s)), s)

def Snap.tname(+t: String, +cols: List<&2, String>) -> String:
  Bool.pick(String, String.eq(t, "obj"), "obj." ++ Snap.col(cols, 0n), t)

def Snap.rekey.k(+t: String, +k: String, +cols: List<&2, String>) -> String:
  Snap.tname(t, cols) ++ "\u{1f}" ++ Bool.pick(String, SDB.Sdb.fact(t), Snap.pad(Nat.show(Snap.num(Snap.col(cols, 0n), 0n))), k)

def Snap.rekey(xs: List<&2, Sigma<&2, &2, String, _ => String>>, m: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}:
      m
    case (+k, +v) <> t:
      Snap.rekey(t, Map.set(&2, String, m, Snap.rekey.k(Snap.col(String.split(k, '\u{1f}'), 0n), k, String.split(v, '\u{1f}')), v))

def Snap.add.k(r: Map<&2, List<&2, List<&2, String>>> & List<&2, List<&2, String>>, +name: String, row: List<&2, String>) -> Map<&2, List<&2, List<&2, String>>>:
  (m, cur) = r
  Map.set(&2, List<&2, List<&2, String>>, m, name, row <> cur)

def Snap.add(m: Map<&2, List<&2, List<&2, String>>>, +name: String, row: List<&2, String>) -> Map<&2, List<&2, List<&2, String>>>:
  Snap.add.k(Map.get(List<&2, List<&2, String>>, Nil{}, m, name), name, row)

# newest key first, so each table's rows come out oldest first
def Snap.group(xs: List<&2, Sigma<&2, &2, String, _ => String>>, m: Map<&2, List<&2, List<&2, String>>>) -> Map<&2, List<&2, List<&2, String>>>:
  match xs:
    case Nil{}:
      m
    case (+k, +v) <> t:
      Snap.group(t, Snap.add(m, Snap.col(String.split(k, '\u{1f}'), 0n), String.split(v, '\u{1f}')))

def Snap.bump(r: Map<&2, Nat> & Nat, +th: String) -> Map<&2, Nat>:
  (m, v) = r
  Map.set(&2, Nat, m, th, 1n+v)

def Snap.tally(rows: List<&2, List<&2, String>>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      +th = Snap.col(r, 3n)
      Snap.tally(t, Snap.bump(Map.get(Nat, 0n, m, th), th))

def Snap.count.rows(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}:
      Nil{}
    case (k, v) <> t:
      [k, Nat.show(v)] <> Snap.count.rows(t)

def Snap.with.counts(+m: Map<&2, List<&2, List<&2, String>>>) -> SRows:
  +rs : SRows = SRows{m}
  SRows{Map.set(&2, List<&2, List<&2, String>>, m, "count", Snap.count.rows(Map.to_list(&2, Nat, Snap.tally(SRows.of(rs, "entry"), Map.new(&2, Nat)))))}

# the pure store's facts and records as rows
def Snap.rows(fs: Map<&2, String>, rs: Map<&2, String>) -> SRows:
  Snap.with.counts(Snap.group(List.reverse(&2, Sigma<&2, &2, String, _ => String>, Map.to_list(&2, String, Snap.rekey(Map.to_list(&2, String, rs), Snap.rekey(Map.to_list(&2, String, fs), Map.new(&2, String))))),
    Map.new(&2, List<&2, List<&2, String>>)))

# booted from the store made of a log's first lines, then its other changes
def Snap.boot(m: SDB.Mat, tail: List<&2, M.Change>) -> M.State:
  match m:
    case SDB.Mat{_, fs, rs, _, _, _}:
      +rows = Snap.rows(fs, rs)
      +s0 = Snap.state(rows)
      Snap.replay(tail, s0, Snap.counts(rows), s0)

# Records for the round-trip laws (snap_*_rt): every field away from its
# default, a text with a quote, a newline and a non-ASCII letter
# ---------------------------------------------------------------------

def Snap.demo.text() -> String:
  "a \"q\"\nü"

def Snap.demo.project() -> M.Project:
  M.Project{"p1", Snap.demo.text(), "/r", 7n, True{}}

def Snap.demo.thread() -> M.Thread:
  M.Thread{"t1", "p1", Snap.demo.text(), 1n, 2n, M.Running{}, 3n, True{}, M.ForcedSettled{}, 4n, 5n, True{}, M.Worktree{}, "br", "/w", "codex", "gpt", M.AcceptEdits{}, M.PlanMode{}, "se", True{}}

def Snap.demo.task() -> M.Task:
  M.Task{"k1", "t1", "t2", 2n, M.TaskFailed{}, Snap.demo.text(), True{}, "claude", "opus"}

def Snap.demo.ask() -> M.Ask:
  M.Ask{"a1", "t1", "input", "Bash", Snap.demo.text(), "yes\nno", True{}, "yes"}

def Snap.demo.queue() -> List<&2, M.QItem>:
  [M.QItem{"q1", Snap.demo.text()}, M.QItem{"q2", ""}]

def Snap.demo.room() -> BT.Room:
  BT.Room{"r1", "Room", ["b1", "x@peer"], 3n, True{}}

def Snap.demo.routine() -> BT.Routine:
  BT.Routine{"ro1", "b1", "n", "0 9 * * *", Snap.demo.text(), True{}, 5n, 6n, False{}, 7n, "UTC", "person", True{}, 8n}

def Snap.demo.conv() -> ML.Conv:
  ML.Conv{"c1", Snap.demo.text(), "pair", ["bot:b1", "person"], "p1", ["x", "y"], 1n, 2n, 3n}

def Snap.demo.mark() -> ML.Mark:
  ML.Mark{"b1", "c1", "snoozed", 4n, 9n, ["t"]}

def Snap.demo.run() -> ML.Run:
  ML.Run{"t1", "b1", "c1", "m1", 5n, True{}}

def Snap.demo.decisions() -> List<&2, ML.Decision>:
  [ML.Decision{"d1", "b1", "c1", "t1", Snap.demo.text(), "why", 3n}]
