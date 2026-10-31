import Base
import ./json.bend as J
import ./model.bend as M
import ./codec.bend as C
import ./bot.bend as BT
import ./mail.bend as ML
import ./ubop.bend as UO
import ./thin.bend as THIN
import ./locks.bend as LK
import ./sha1.bend as SH

# The indexed store (store.db, docs: bp-coord/store/S1-SCHEMA.md)
# ---------------------------------------------------------------
# events.jsonl stays the truth and is never rewritten; store.db is made
# from it, one line at a time, and can be deleted and made again. Pure:
# the server's store.bend runs the SQL these rows become.
#
# Two kinds of row. Facts are a function of one log line alone (its
# number, offset and length, and the change): the line itself, a timeline
# entry and its search terms, a room post, a mail header. They never
# change, so writing one twice is harmless. Records are the fold's current
# value of a key, read from the state after the change: a change names
# the records it may touch (Sdb.keys, a superset is fine) and each is
# written whole, or deleted when the fold has none. The store is never
# folded in SQL, so after any log it holds what the fold holds (laws
# store_*).

# one row written or deleted. A Put's first columns are its key (Tab.keyn)
type Op is Data:
  Put{table: String, cols: List<&2, String>}
  Del{table: String, key: List<&2, String>}

# a record a change may touch: its domain and key
type RK is Data:
  RK{dom: String, key: String}

# Schema
# ------

# bumped on any change to the tables or to a record's columns or json
# (a different number in meta.codec: the store is made again).
# Version 5 also repairs TurnUsage record coverage: daily dimensions,
# their first accepted turn, and the thread's last turn must survive boot.
def Sdb.codec() -> Nat:
  5n

def Sdb.schema() -> String:
  String.join([
    "CREATE TABLE IF NOT EXISTS meta(k TEXT PRIMARY KEY, v TEXT)",
    "CREATE TABLE IF NOT EXISTS line(seq INTEGER PRIMARY KEY, off INTEGER, len INTEGER, wire TEXT)",
    "CREATE TABLE IF NOT EXISTS entry(seq INTEGER PRIMARY KEY, ord INTEGER UNIQUE, id TEXT, thread TEXT, src TEXT, kind TEXT, role TEXT, akind TEXT, text TEXT, at INTEGER)",
    "CREATE INDEX IF NOT EXISTS entry_thread ON entry(thread, seq)",
    "CREATE INDEX IF NOT EXISTS entry_src ON entry(src, seq) WHERE src <> ''",
    "CREATE INDEX IF NOT EXISTS entry_id ON entry(id)",
    "CREATE INDEX IF NOT EXISTS entry_user ON entry(thread, at) WHERE kind = 'msg' AND role = 'user'",
    "CREATE INDEX IF NOT EXISTS entry_tone ON entry(thread, role, seq) WHERE kind = 'act'",
    "CREATE VIRTUAL TABLE IF NOT EXISTS entry_fts USING fts5(text, content='entry', content_rowid='seq', tokenize='unicode61 remove_diacritics 0')",
    "CREATE TRIGGER IF NOT EXISTS entry_fts_add AFTER INSERT ON entry BEGIN INSERT INTO entry_fts(rowid, text) VALUES (new.seq, new.text); END",
    "CREATE TABLE IF NOT EXISTS term(word TEXT, ord INTEGER, PRIMARY KEY(word, ord)) WITHOUT ROWID",
    "CREATE TABLE IF NOT EXISTS post(seq INTEGER PRIMARY KEY, id TEXT, room TEXT, src TEXT, text TEXT, hop INTEGER, at INTEGER)",
    "CREATE INDEX IF NOT EXISTS post_room ON post(room, seq)",
    "CREATE INDEX IF NOT EXISTS post_room_id ON post(room, id)",
    "CREATE TABLE IF NOT EXISTS mail(seq INTEGER PRIMARY KEY, id TEXT, conv TEXT, src TEXT, dst TEXT, kind TEXT, ref TEXT, want TEXT, head TEXT, size INTEGER, at INTEGER)",
    "CREATE INDEX IF NOT EXISTS mail_conv ON mail(conv, seq)",
    "CREATE INDEX IF NOT EXISTS mail_newest ON mail(conv, at, seq)",
    "CREATE INDEX IF NOT EXISTS mail_id ON mail(id)",
    "CREATE TABLE IF NOT EXISTS project(id TEXT PRIMARY KEY, title TEXT, root TEXT, created INTEGER, removed INTEGER, json TEXT)",
    "CREATE TABLE IF NOT EXISTS thread(id TEXT PRIMARY KEY, project TEXT, title TEXT, created INTEGER, activity INTEGER, turn TEXT, pending INTEGER, archived INTEGER, pinned INTEGER, settled_at INTEGER, provider TEXT, model TEXT, session TEXT, deleted INTEGER, json TEXT)",
    "CREATE INDEX IF NOT EXISTS thread_project ON thread(project, activity)",
    "CREATE TABLE IF NOT EXISTS task(id TEXT PRIMARY KEY, parent TEXT, child TEXT, state TEXT, delivered INTEGER, json TEXT)",
    "CREATE INDEX IF NOT EXISTS task_parent ON task(parent)",
    "CREATE INDEX IF NOT EXISTS task_child ON task(child)",
    "CREATE TABLE IF NOT EXISTS ask(id TEXT PRIMARY KEY, thread TEXT, open INTEGER, json TEXT)",
    "CREATE INDEX IF NOT EXISTS ask_thread ON ask(thread)",
    "CREATE TABLE IF NOT EXISTS queue(thread TEXT PRIMARY KEY, n INTEGER, json TEXT)",
    "CREATE TABLE IF NOT EXISTS setting(k TEXT PRIMARY KEY, v TEXT, cold INTEGER)",
    "CREATE VIEW IF NOT EXISTS work AS SELECT substr(k, 1, instr(k, '.') - 1) AS kind, substr(k, instr(k, '.') + 1) AS thread, v AS rows FROM setting WHERE k GLOB 'bg.*' OR k GLOB 'watch.*'",
    "CREATE VIEW IF NOT EXISTS lock AS SELECT substr(k, 6) AS name, v AS json FROM setting WHERE k GLOB 'lock.*'",
    "CREATE TABLE IF NOT EXISTS bot(id TEXT PRIMARY KEY, name TEXT, thread TEXT, deleted INTEGER, json TEXT)",
    "CREATE TABLE IF NOT EXISTS room(id TEXT PRIMARY KEY, name TEXT, deleted INTEGER, json TEXT)",
    "CREATE TABLE IF NOT EXISTS routine(id TEXT PRIMARY KEY, bot TEXT, on_ INTEGER, deleted INTEGER, json TEXT)",
    "CREATE INDEX IF NOT EXISTS routine_bot ON routine(bot)",
    "CREATE TABLE IF NOT EXISTS conv(id TEXT PRIMARY KEY, kind TEXT, project TEXT, last INTEGER, want TEXT, parts TEXT, json TEXT)",
    "CREATE TABLE IF NOT EXISTS mark(k TEXT PRIMARY KEY, bot TEXT, conv TEXT, status TEXT, json TEXT)",
    "CREATE INDEX IF NOT EXISTS mark_bot ON mark(bot)",
    "CREATE TABLE IF NOT EXISTS far_line(seq INTEGER PRIMARY KEY, scope TEXT)",
    "CREATE INDEX IF NOT EXISTS far_line_scope ON far_line(scope, seq)",
    "CREATE TABLE IF NOT EXISTS obj(domain TEXT, key TEXT, json TEXT, PRIMARY KEY(domain, key)) WITHOUT ROWID"
  ], ";\n") ++ ";"

# a table's columns, key first (obj: anything not listed)
def Sdb.cols.k(i: Nat) -> List<&2, String>:
  match i:
    case 0n:
      ["seq", "off", "len", "wire"]
    case 1n:
      ["seq", "ord", "id", "thread", "src", "kind", "role", "akind", "text", "at"]
    case 2n:
      ["word", "ord"]
    case 3n:
      ["seq", "id", "room", "src", "text", "hop", "at"]
    case 4n:
      ["seq", "id", "conv", "src", "dst", "kind", "ref", "want", "head", "size", "at"]
    case 5n:
      ["id", "title", "root", "created", "removed", "json"]
    case 6n:
      ["id", "project", "title", "created", "activity", "turn", "pending", "archived", "pinned", "settled_at", "provider", "model", "session", "deleted", "json"]
    case 7n:
      ["id", "parent", "child", "state", "delivered", "json"]
    case 8n:
      ["id", "thread", "open", "json"]
    case 9n:
      ["thread", "n", "json"]
    case 10n:
      ["k", "v", "cold"]
    case 11n:
      ["id", "name", "thread", "deleted", "json"]
    case 12n:
      ["id", "name", "deleted", "json"]
    case 13n:
      ["id", "bot", "on_", "deleted", "json"]
    case 14n:
      ["id", "kind", "project", "last", "want", "parts", "json"]
    case 15n:
      ["k", "bot", "conv", "status", "json"]
    case _:
      ["domain", "key", "json"]

def Sdb.cols(+t: String) -> List<&2, String>:
  Sdb.cols.k(C.Codec.pos(["line", "entry", "term", "post", "mail", "project", "thread", "task", "ask", "queue", "setting", "bot", "room", "routine", "conv", "mark"], t))

# how many of a table's first columns are its key
def Tab.keyn(+t: String) -> Nat:
  Bool.pick(Nat, Bool.or(String.eq(t, "obj"), String.eq(t, "term")), 2n, 1n)

# facts are never replaced: a line written again keeps its row
def Sdb.fact(+t: String) -> Bool:
  Bool.or(Bool.or(String.eq(t, "line"), String.eq(t, "entry")), Bool.or(Bool.or(String.eq(t, "term"), String.eq(t, "post")), String.eq(t, "mail")))

# Parameters
# ----------
# D.Db binds a parameter that reads "#<digits>" as an integer, so a value
# written as it is would lose a person's "#42" (a title, a setting, a
# message) to the number 42. Every value goes as "v" ++ value and the
# statement takes substr(?, 2): no parameter ever starts with '#'. Numbers
# go as plain digits and the INTEGER columns' affinity stores them as
# integers (law-free, checked by test/storedb_test.bend and the commit e2e).

def Sql.p() -> String:
  "substr(?, 2)"

def Sql.vals(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case x <> t:
      Sql.vals(t, ("v" ++ x) <> acc)

# one parameter per column
def Sql.marks(cs: List<&2, String>) -> String:
  match cs:
    case Nil{}:
      ""
    case _ <> Nil{}:
      Sql.p()
    case _ <> t:
      Sql.p() ++ ", " ++ Sql.marks(t)

def Sql.eqs(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> Nil{}:
      x ++ " = " ++ Sql.p()
    case x <> t:
      x ++ " = " ++ Sql.p() ++ " AND " ++ Sql.eqs(t)

def Sql.put.k(fact: Bool, +t: String, +cs: List<&2, String>) -> String:
  match fact:
    case True{}:
      "INSERT INTO " ++ t ++ "(" ++ String.join(cs, ", ") ++ ") VALUES (" ++ Sql.marks(cs) ++ ") ON CONFLICT DO NOTHING"
    case False{}:
      "INSERT OR REPLACE INTO " ++ t ++ "(" ++ String.join(cs, ", ") ++ ") VALUES (" ++ Sql.marks(cs) ++ ")"

# an op as a statement and its parameters (each "v" ++ value, Sql.p). The
# SQL depends on the table alone.
def Sql.of(o: Op) -> List<&2, String>:
  match o:
    case Put{+t, cols}:
      Sql.put.k(Sdb.fact(t), t, Sdb.cols(t)) <> Sql.vals(cols, Nil{})
    case Del{+t, key}:
      ("DELETE FROM " ++ t ++ " WHERE " ++ Sql.eqs(List.take(&2, String, Sdb.cols(t), Tab.keyn(t)))) <> Sql.vals(key, Nil{})

# Columns
# -------

# a number as its column holds it (plain digits: Sql.p)
def S.n(+n: Nat) -> String:
  Nat.show(n)

def S.b(b: Bool) -> String:
  match b:
    case True{}:
      "1"
    case False{}:
      "0"

def S.j(kvs: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj(kvs))

def S.strs.go(xs: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case x <> t:
      S.strs.go(t, J.Str{x} <> acc)

def S.strs(xs: List<&2, String>) -> J.Json:
  J.Json.arr(S.strs.go(xs, Nil{}))

def S.override(o: M.Override) -> String:
  match o:
    case M.NoOverride{}:
      "none"
    case M.ForcedSettled{}:
      "settled"
    case M.ForcedActive{}:
      "active"

# Records as rows (S3's snapshot decodes the json column back: Sdb.codec)
# -------------------------------------------------------------------------

def Row.project(p: M.Project) -> Op:
  match p:
    case M.Project{+id, +ti, +rt, +c, +rm}:
      Put{"project", [id, ti, rt, S.n(c), S.b(rm), S.j([J.KV{"id", J.Str{id}}, J.KV{"title", J.Str{ti}}, J.KV{"root", J.Str{rt}}, J.KV{"created", C.Codec.nat(c)}, J.KV{"removed", J.Flag{rm}}])]}

def Row.thread(t: M.Thread) -> Op:
  match t:
    case M.Thread{+id, +p, +ti, +c, +a, +tu, +pe, +ar, o, +sa, ua, +pi, en, br, wt, +pr, +mo, rm, im, +se, +de}:
      Put{"thread", [id, p, ti, S.n(c), S.n(a), C.Turn.name(tu), S.n(pe), S.b(ar), S.b(pi), S.n(sa), pr, mo, se, S.b(de),
        S.j([J.KV{"id", J.Str{id}}, J.KV{"project", J.Str{p}}, J.KV{"title", J.Str{ti}}, J.KV{"created", C.Codec.nat(c)}, J.KV{"activity", C.Codec.nat(a)},
          J.KV{"turn", J.Str{C.Turn.name(tu)}}, J.KV{"pending", C.Codec.nat(pe)}, J.KV{"archived", J.Flag{ar}}, J.KV{"override", J.Str{S.override(o)}},
          J.KV{"settled_at", C.Codec.nat(sa)}, J.KV{"unsettled_at", C.Codec.nat(ua)}, J.KV{"pinned", J.Flag{pi}}, J.KV{"env", J.Str{C.Env.name(en)}},
          J.KV{"branch", J.Str{br}}, J.KV{"worktree", J.Str{wt}}, J.KV{"provider", J.Str{pr}}, J.KV{"model", J.Str{mo}},
          J.KV{"runtime", J.Str{C.Runtime.name(rm)}}, J.KV{"interaction", J.Str{C.Interaction.name(im)}}, J.KV{"session", J.Str{se}}, J.KV{"deleted", J.Flag{de}}])]}

def Row.task(t: M.Task) -> Op:
  match t:
    case M.Task{+id, +pa, +ch, d, +st, su, +dl, pr, mo}:
      Put{"task", [id, pa, ch, C.TaskSt.name(st), S.b(dl),
        S.j([J.KV{"id", J.Str{id}}, J.KV{"parent", J.Str{pa}}, J.KV{"child", J.Str{ch}}, J.KV{"depth", C.Codec.nat(d)}, J.KV{"state", J.Str{C.TaskSt.name(st)}},
          J.KV{"summary", J.Str{su}}, J.KV{"delivered", J.Flag{dl}}, J.KV{"provider", J.Str{pr}}, J.KV{"model", J.Str{mo}}])]}

def Row.ask(a: M.Ask) -> Op:
  match a:
    case M.Ask{+id, +th, k, tl, d, o, +op, an}:
      Put{"ask", [id, th, S.b(op),
        S.j([J.KV{"id", J.Str{id}}, J.KV{"thread", J.Str{th}}, J.KV{"kind", J.Str{k}}, J.KV{"tool", J.Str{tl}}, J.KV{"detail", J.Str{d}},
          J.KV{"options", J.Str{o}}, J.KV{"open", J.Flag{op}}, J.KV{"answer", J.Str{an}}])]}

# (walked into acc reversed, then turned back: a tail call per item)
def Row.qitems(qs: List<&2, M.QItem>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match qs:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case M.QItem{m, t} <> rest:
      Row.qitems(rest, J.Json.obj([J.KV{"msg", J.Str{m}}, J.KV{"text", J.Str{t}}]) <> acc)

def Row.queue(+th: String, +qs: List<&2, M.QItem>) -> Op:
  Put{"queue", [th, S.n(List.length(&2, M.QItem, qs)), J.Json.show(J.Json.arr(Row.qitems(qs, Nil{})))]}

def Row.obj(+dom: String, +key: String, j: J.Json) -> Op:
  Put{"obj", [dom, key, J.Json.show(j)]}

def Row.device(d: M.Device) -> Op:
  match d:
    case M.Device{+tok, p, k, th, e, b, at}:
      Row.obj("device", tok, J.Json.obj([J.KV{"token", J.Str{tok}}, J.KV{"platform", J.Str{p}}, J.KV{"kind", J.Str{k}}, J.KV{"thread", J.Str{th}},
        J.KV{"env", J.Str{e}}, J.KV{"bundle", J.Str{b}}, J.KV{"at", C.Codec.nat(at)}]))

def Row.nav(+id: String, n: M.Nav) -> Op:
  match n:
    case M.Nav{u, p, a}:
      Row.obj("nav", id, J.Json.obj([J.KV{"until", C.Codec.nat(u)}, J.KV{"pin", J.Str{p}}, J.KV{"act", J.Str{a}}]))

def Row.bot(b: BT.Bot) -> Op:
  match b:
    case BT.Bot{+id, +n, +th, lk, pe, c, +de}:
      Put{"bot", [id, n, th, S.b(de), S.j([J.KV{"id", J.Str{id}}, J.KV{"name", J.Str{n}}, J.KV{"thread", J.Str{th}}, J.KV{"look", C.Codec.nat(lk)},
        J.KV{"persona", J.Str{pe}}, J.KV{"created", C.Codec.nat(c)}, J.KV{"deleted", J.Flag{de}}])]}

def Row.room(r: BT.Room) -> Op:
  match r:
    case BT.Room{+id, +n, ms, c, +de}:
      Put{"room", [id, n, S.b(de), S.j([J.KV{"id", J.Str{id}}, J.KV{"name", J.Str{n}}, J.KV{"members", S.strs(ms)}, J.KV{"created", C.Codec.nat(c)}, J.KV{"deleted", J.Flag{de}}])]}

def Row.routine(r: BT.Routine) -> Op:
  match r:
    case BT.Routine{+id, +b, n, cr, p, +on, l, c, +de, once, tz, k, dn, sl}:
      Put{"routine", [id, b, S.b(on), S.b(de), S.j([J.KV{"id", J.Str{id}}, J.KV{"bot", J.Str{b}}, J.KV{"name", J.Str{n}}, J.KV{"cron", J.Str{cr}},
        J.KV{"prompt", J.Str{p}}, J.KV{"on", J.Flag{on}}, J.KV{"last", C.Codec.nat(l)}, J.KV{"created", C.Codec.nat(c)}, J.KV{"deleted", J.Flag{de}},
        J.KV{"once", C.Codec.nat(once)}, J.KV{"tz", J.Str{tz}}, J.KV{"kind", J.Str{k}}, J.KV{"done", J.Flag{dn}}, J.KV{"slot", C.Codec.nat(sl)}])]}

def Row.hook(h: BT.Hook) -> Op:
  match h:
    case BT.Hook{+id, b, n, c, r, l, k}:
      Row.obj("hook", id, J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"bot", J.Str{b}}, J.KV{"name", J.Str{n}}, J.KV{"created", C.Codec.nat(c)},
        J.KV{"revoked", J.Flag{r}}, J.KV{"last", C.Codec.nat(l)}, J.KV{"count", C.Codec.nat(k)}]))

def Row.mem(+key: String, m: BT.Mem) -> Op:
  match m:
    case BT.Mem{b, k, kd, t, tg, c, u}:
      Row.obj("mem", key, J.Json.obj([J.KV{"bot", J.Str{b}}, J.KV{"key", J.Str{k}}, J.KV{"kind", J.Str{kd}}, J.KV{"text", J.Str{t}},
        J.KV{"tags", J.Str{tg}}, J.KV{"created", C.Codec.nat(c)}, J.KV{"updated", C.Codec.nat(u)}]))

def Row.peer(p: BT.Peer) -> Op:
  match p:
    case BT.Peer{+id, n, u, c, r}:
      Row.obj("peer", id, J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"name", J.Str{n}}, J.KV{"url", J.Str{u}}, J.KV{"created", C.Codec.nat(c)}, J.KV{"revoked", J.Flag{r}}]))

def Row.page(+bot: String, p: BT.Page) -> Op:
  match p:
    case BT.Page{h, at}:
      Row.obj("page", bot, J.Json.obj([J.KV{"html", J.Str{h}}, J.KV{"at", C.Codec.nat(at)}]))

def Row.hops(+bot: String, h: BT.Hops) -> Op:
  match h:
    case BT.Hops{n, w, hd}:
      Row.obj("hops", bot, J.Json.obj([J.KV{"now", C.Codec.nat(n)}, J.KV{"wait", C.Codec.nat(w)}, J.KV{"held", J.Flag{hd}}]))

# a conversation, with what its newest mail wants and its members as one
# text ("bot:a bot:b person"), for the mail views' queries
def Row.conv(+b: ML.Box, c: ML.Conv) -> Op:
  match c:
    case ML.Conv{+id, s, +k, +ps, +pj, tg, m, +l, n}:
      Put{"conv", [id, k, pj, S.n(l), ML.Mail.wanted(ML.Box.newest(b, id)), String.join(ps, " "), S.j([J.KV{"id", J.Str{id}}, J.KV{"subject", J.Str{s}}, J.KV{"kind", J.Str{k}}, J.KV{"parts", S.strs(ps)},
        J.KV{"project", J.Str{pj}}, J.KV{"tags", S.strs(tg)}, J.KV{"made", C.Codec.nat(m)}, J.KV{"last", C.Codec.nat(l)}, J.KV{"n", C.Codec.nat(n)}])]}

def Row.mark(+key: String, m: ML.Mark) -> Op:
  match m:
    case ML.Mark{+b, +c, +s, sn, u, tg}:
      Put{"mark", [key, b, c, s, S.j([J.KV{"bot", J.Str{b}}, J.KV{"conv", J.Str{c}}, J.KV{"status", J.Str{s}}, J.KV{"seen", C.Codec.nat(sn)},
        J.KV{"until", C.Codec.nat(u)}, J.KV{"tags", S.strs(tg)}])]}

def Row.run(r: ML.Run) -> Op:
  match r:
    case ML.Run{+th, b, c, m, at, l}:
      Row.obj("run", th, J.Json.obj([J.KV{"thread", J.Str{th}}, J.KV{"bot", J.Str{b}}, J.KV{"conv", J.Str{c}}, J.KV{"mail", J.Str{m}},
        J.KV{"at", C.Codec.nat(at)}, J.KV{"live", J.Flag{l}}]))

def Row.origin(+key: String, o: ML.Origin) -> Op:
  match o:
    case ML.Origin{f, k, r}:
      Row.obj("origin", key, J.Json.obj([J.KV{"from", J.Str{f}}, J.KV{"kind", J.Str{k}}, J.KV{"ref", J.Str{r}}]))

def Row.decisions.js(ds: List<&2, ML.Decision>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ds:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case ML.Decision{i, b, c, r, t, rs, at} <> rest:
      Row.decisions.js(rest, J.Json.obj([J.KV{"id", J.Str{i}}, J.KV{"bot", J.Str{b}}, J.KV{"conv", J.Str{c}}, J.KV{"run", J.Str{r}}, J.KV{"text", J.Str{t}},
        J.KV{"reason", J.Str{rs}}, J.KV{"at", C.Codec.nat(at)}]) <> acc)

# Reading the state
# -----------------

def St.ps(s: M.State) -> Map<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      ps

def St.ts(s: M.State) -> Map<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      ts

def St.tk(s: M.State) -> Map<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      tk

def St.sg(s: M.State) -> Map<&2, String>:
  match s:
    case M.State{_, _, _, _, _, sg, _, _, _, _, _}:
      sg

def St.qu(s: M.State) -> Map<&2, List<&2, M.QItem>>:
  match s:
    case M.State{_, _, _, _, _, _, qu, _, _, _, _}:
      qu

def St.ak(s: M.State) -> Map<&2, M.Ask>:
  match s:
    case M.State{_, _, _, _, _, _, _, ak, _, _, _}:
      ak

def St.dv(s: M.State) -> Map<&2, M.Device>:
  match s:
    case M.State{_, _, _, _, _, _, _, _, dv, _, _}:
      dv

def W.bots(w: BT.World) -> Map<&2, BT.Bot>:
  match w:
    case BT.World{x, _, _, _, _, _, _, _, _, _, _, _}:
      x

def W.rooms(w: BT.World) -> Map<&2, BT.Room>:
  match w:
    case BT.World{_, x, _, _, _, _, _, _, _, _, _, _}:
      x

def W.posts(w: BT.World) -> List<&2, BT.Post>:
  match w:
    case BT.World{_, _, x, _, _, _, _, _, _, _, _, _}:
      x

def W.hooks(w: BT.World) -> Map<&2, BT.Hook>:
  match w:
    case BT.World{_, _, _, x, _, _, _, _, _, _, _, _}:
      x

def W.routines(w: BT.World) -> Map<&2, BT.Routine>:
  match w:
    case BT.World{_, _, _, _, x, _, _, _, _, _, _, _}:
      x

def W.mems(w: BT.World) -> Map<&2, BT.Mem>:
  match w:
    case BT.World{_, _, _, _, _, x, _, _, _, _, _, _}:
      x

def W.peers(w: BT.World) -> Map<&2, BT.Peer>:
  match w:
    case BT.World{_, _, _, _, _, _, x, _, _, _, _, _}:
      x

def W.spaces(w: BT.World) -> Map<&2, String>:
  match w:
    case BT.World{_, _, _, _, _, _, _, x, _, _, _, _}:
      x

def W.hops(w: BT.World) -> Map<&2, BT.Hops>:
  match w:
    case BT.World{_, _, _, _, _, _, _, _, x, _, _, _}:
      x

def W.talk(w: BT.World) -> Map<&2, Nat>:
  match w:
    case BT.World{_, _, _, _, _, _, _, _, _, x, _, _}:
      x

def W.pages(w: BT.World) -> Map<&2, BT.Page>:
  match w:
    case BT.World{_, _, _, _, _, _, _, _, _, _, x, _}:
      x

def W.skills(w: BT.World) -> Map<&2, String>:
  match w:
    case BT.World{_, _, _, _, _, _, _, _, _, _, _, x}:
      x

def Got(-V: Data, r: Map<&2, V> & Maybe<&2, V>) -> Maybe<&2, V>:
  (_, v) = r
  v

# the value at a key, if the map has one
def Get(-V: Data, m: Map<&2, V>, +k: String) -> Maybe<&2, V>:
  Got(V, Map.pop(&2, V, m, k))

# Records from the state after a change
# -------------------------------------

def Row.or(-V: Data, f: V -> Op, +t: String, +key: List<&2, String>, m: Maybe<&2, V>) -> Op:
  match m:
    case Some{v}:
      f(v)
    case None{}:
      Del{t, key}

def Row.obj.or(-V: Data, f: V -> Op, +dom: String, +key: String, m: Maybe<&2, V>) -> Op:
  Row.or(V, f, "obj", [dom, key], m)

# settings read on demand, never loaded with the hot state: copies of bot
# run inputs (up to 18 KB each, ~35 MB of the 35.5 MB of settings)
def Sdb.cold(+k: String) -> Bool:
  Bool.or(String.starts_with(k, "launch.input.run"), String.starts_with(k, "fallback.input.run"))

# a setting's value as raw text
def Row.set(+k: String, v: String) -> Op:
  Put{"setting", [k, v, S.b(Sdb.cold(k))]}

def Row.setting(+k: String, m: Maybe<&2, String>) -> Op:
  match m:
    case Some{v}:
      Row.set(k, v)
    case None{}:
      Del{"setting", [k]}

def Row.str(+dom: String, +k: String, m: Maybe<&2, String>) -> Op:
  Row.obj.or(String, v => Row.obj(dom, k, J.Str{v}), dom, k, m)

def Row.nat(+dom: String, +k: String, m: Maybe<&2, Nat>) -> Op:
  Row.obj.or(Nat, v => Row.obj(dom, k, C.Codec.nat(v)), dom, k, m)

def Row.decisions(+bot: String, m: Maybe<&2, List<&2, ML.Decision>>) -> Op:
  Row.obj.or(List<&2, ML.Decision>, ds => Row.obj("decision", bot, J.Json.arr(Row.decisions.js(ds, Nil{}))), "decision", bot, m)

def Row.world.k(i: Nat, +w: BT.World, +k: String) -> Op:
  match i:
    case 0n:
      Row.or(BT.Bot, Row.bot, "bot", [k], Get(BT.Bot, W.bots(w), k))
    case 1n:
      Row.or(BT.Room, Row.room, "room", [k], Get(BT.Room, W.rooms(w), k))
    case 2n:
      Row.or(BT.Routine, Row.routine, "routine", [k], Get(BT.Routine, W.routines(w), k))
    case 3n:
      Row.obj.or(BT.Hook, Row.hook, "hook", k, Get(BT.Hook, W.hooks(w), k))
    case 4n:
      Row.obj.or(BT.Mem, m => Row.mem(k, m), "mem", k, Get(BT.Mem, W.mems(w), k))
    case 5n:
      Row.obj.or(BT.Peer, Row.peer, "peer", k, Get(BT.Peer, W.peers(w), k))
    case 6n:
      Row.str("space", k, Get(String, W.spaces(w), k))
    case 7n:
      Row.obj.or(BT.Page, p => Row.page(k, p), "page", k, Get(BT.Page, W.pages(w), k))
    case 8n:
      Row.str("skill", k, Get(String, W.skills(w), k))
    case 9n:
      Row.obj.or(BT.Hops, h => Row.hops(k, h), "hops", k, Get(BT.Hops, W.hops(w), k))
    case _:
      Row.nat("talk", k, Get(Nat, W.talk(w), k))

def Row.mail.k(i: Nat, +b: ML.Box, +k: String) -> Op:
  match i:
    case 0n:
      Row.or(ML.Conv, c => Row.conv(b, c), "conv", [k], Get(ML.Conv, ML.Box.convs(b), k))
    case 1n:
      Row.or(ML.Mark, m => Row.mark(k, m), "mark", [k], Get(ML.Mark, ML.Box.marks(b), k))
    case 2n:
      Row.obj.or(ML.Run, Row.run, "run", k, Get(ML.Run, ML.Box.runs(b), k))
    case 3n:
      Row.obj.or(ML.Origin, o => Row.origin(k, o), "origin", k, Get(ML.Origin, ML.Box.origins(b), k))
    case 4n:
      Row.str("charter", k, Get(String, ML.Box.charters(b), k))
    case 5n:
      Row.decisions(k, Get(List<&2, ML.Decision>, ML.Box.decision_log(b), k))
    case _:
      Row.str("mailname", k, Get(String, ML.Box.names(b), k))

def Row.top.k(i: Nat, +s: M.State, +k: String) -> Op:
  match i:
    case 0n:
      Row.or(M.Project, Row.project, "project", [k], Get(M.Project, St.ps(s), k))
    case 1n:
      Row.or(M.Thread, Row.thread, "thread", [k], Get(M.Thread, St.ts(s), k))
    case 2n:
      Row.or(M.Task, Row.task, "task", [k], Get(M.Task, St.tk(s), k))
    case 3n:
      Row.or(M.Ask, Row.ask, "ask", [k], Get(M.Ask, St.ak(s), k))
    case 4n:
      Row.or(List<&2, M.QItem>, qs => Row.queue(k, qs), "queue", [k], Get(List<&2, M.QItem>, St.qu(s), k))
    case 5n:
      Row.setting(k, Get(String, St.sg(s), k))
    case 6n:
      Row.obj.or(M.Device, Row.device, "device", k, Get(M.Device, St.dv(s), k))
    case 7n:
      Row.obj.or(M.Nav, n => Row.nav(k, n), "nav", k, Get(M.Nav, M.State.navs(s), k))
    case _:
      Row.obj("state", "seq", C.Codec.nat(M.State.seq(s)))

def Dom.top() -> List<&2, String>:
  ["project", "thread", "task", "ask", "queue", "setting", "device", "nav", "state"]

def Dom.world() -> List<&2, String>:
  ["bot", "room", "routine", "hook", "mem", "peer", "space", "page", "skill", "hops", "talk"]

def Dom.mail() -> List<&2, String>:
  ["conv", "mark", "run", "origin", "charter", "decision", "mailname"]

# a domain's place in its group, or the group's length when not in it
def Dom.at(+xs: List<&2, String>, +d: String) -> Nat:
  C.Codec.pos(xs, d)

def Row.of.w(i: Nat, +s: M.State, +d: String, +k: String) -> Op:
  match i:
    case 11n:
      Row.mail.k(Dom.at(Dom.mail(), d), M.State.mail(s), k)
    case _:
      Row.world.k(i, M.State.world.of(s), k)

def Row.of.k(i: Nat, +s: M.State, +d: String, +k: String) -> Op:
  match i:
    case 9n:
      Row.of.w(Dom.at(Dom.world(), d), s, d, k)
    case _:
      Row.top.k(i, s, k)

# the record at a key as the fold has it now: written whole, or deleted
def Row.of(+s: M.State, k: RK) -> Op:
  match k:
    case RK{+d, +key}:
      Row.of.k(Dom.at(Dom.top(), d), s, d, key)

# (records' order is free: each is the state's value at its key)
def Row.ofs(ks: List<&2, RK>, +s: M.State, acc: List<&2, Op>) -> List<&2, Op>:
  match ks:
    case Nil{}:
      acc
    case k <> t:
      Row.ofs(t, s, Row.of(s, k) <> acc)

# Which records a change may touch
# --------------------------------
# A superset is harmless (an untouched record is written as it was); a
# missing key is not, and the laws store_eq_fold_* and the copy harness
# (test/native/store_mat.bend) catch it.

def K.set(+k: String) -> RK:
  RK{"setting", k}

def K.hops.of(m: Maybe<&2, BT.Bot>) -> List<&2, RK>:
  match m:
    case Some{b}:
      [RK{"hops", BT.Bot.id(b)}]
    case None{}:
      Nil{}

def K.bot.mark(bot: Bool, +p: String, +conv: String, rest: List<&2, RK>) -> List<&2, RK>:
  match bot:
    case True{}:
      RK{"mark", String.drop(p, 4n) ++ "/" ++ conv} <> rest
    case False{}:
      rest

def K.bot.marks(ps: List<&2, String>, +conv: String, acc: List<&2, RK>) -> List<&2, RK>:
  match ps:
    case Nil{}:
      acc
    case +p <> t:
      K.bot.marks(t, conv, K.bot.mark(String.starts_with(p, "bot:"), p, conv, acc))

# a conversation that took mail, and every bot in it (Box.received)
def K.conv(+post: M.State, +conv: String) -> List<&2, RK>:
  RK{"conv", conv} <> K.bot.marks(ML.Conv.parts(ML.Box.conv(M.State.mail(post), conv)), conv, Nil{})

# a conversation whose members may change (ML.Box.conv_set: a joining bot
# gets an open mark, one that left loses its mark): the marks of its
# members before and after (found missing by the copy harness, F1)
def K.conv.both(+pre: M.State, +post: M.State, +conv: String) -> List<&2, RK>:
  List.append(&2, RK, K.conv(post, conv), K.conv(pre, conv))

def K.tokens(+pre: M.State, +th: String, +pv: String, +mo: String, +at: Nat) -> List<&2, RK>:
  +x = M.Tok.pre(pre)
  +pm = pv ++ "|" ++ Bool.pick(String, String.is_empty(mo), "default", mo)
  +mon = M.Tok.month(at)
  +day = M.Tok.day(at)
  +pj = M.Tok.project(M.State.thread.get(pre, th))
  +provider = Maybe.default(&2, String, List.head(&2, String, String.split(pm, '|')), "")
  [K.set(x ++ "t." ++ th), K.set(x ++ "d." ++ M.Tok.day(at)), K.set(x ++ "m." ++ mon), K.set(x ++ "mm." ++ mon ++ "|" ++ pm),
   K.set(x ++ "p." ++ pj ++ "|" ++ mon), K.set(x ++ "ml." ++ mon), K.set(x ++ "since"),
   K.set(x ++ "dv." ++ day ++ "|" ++ provider), K.set(x ++ "dp." ++ day ++ "|" ++ pj), K.set(x ++ "daily.since"),
   K.set("tok.key." ++ th), K.set("tok.basis." ++ th), K.set(M.Tok.last.key(th))]

def K.uber.nop(ps: List<&2, String>, +uid: String) -> List<&2, RK>:
  match ps:
    case +o <> +c <> Nil{}:
      [K.set(UO.Ub.key.op(uid, o, M.Switch.num(c, 0n)))]
    case _:
      Nil{}

def K.uber(+pre: M.State, +uid: String, +origin: String, +n: Nat, +op: String) -> List<&2, RK>:
  +key = UO.Ub.op.key(op)
  +gid = UO.Ub.field(UO.Ub.json(op), "routine")
  List.append(&2, RK, [K.set(UO.Ub.key.vv(uid, origin)), K.set(UO.Ub.key.op(uid, origin, n)), K.set(UO.Ub.key.origins(uid)), K.set(UO.Ub.key.hlc(uid)),
    K.set(UO.Ub.key.fired(uid, gid)), K.set(UO.Ub.key.by(uid, gid)), K.set(UO.Ub.key.reg(uid, key)), K.set(UO.Ub.key.stamp(uid, key)), K.set(UO.Ub.key.rn(uid, key))],
    K.uber.nop(String.split(M.Setting.get(pre, UO.Ub.key.rn(uid, key), ""), '.'), uid))

def K.wake.drop(+o: J.Json, +id: String) -> List<&2, RK>:
  [K.set("wakes.poll"), K.set("wakes.by." ++ J.Json.get_str(o, "by", "")), K.set("wakes.on." ++ J.Json.get_str(o, "kind", "") ++ ":" ++ J.Json.get_str(o, "target", "")), K.set("wake." ++ id)]

def K.names(+pre: M.State, +id: String) -> List<&2, RK>:
  [RK{"mailname", "id:" ++ id}, RK{"mailname", "name:" ++ ML.Get.str(ML.Box.names(M.State.mail(pre)), "id:" ++ id)}]

def K.room.mail(off: Bool, +post: M.State, +room: String) -> List<&2, RK>:
  match off:
    case True{}:
      Nil{}
    case False{}:
      K.conv(post, "room:" ++ room)

# the records a change may touch; pre and post: the state before and after
def Sdb.keys(+pre: M.State, +post: M.State, c: M.Change) -> List<&2, RK>:
  match c:
    case M.ProjectCreated{+id, _, _, _}:
      [RK{"project", id}]
    case M.ProjectRemoved{+id, _}:
      [RK{"project", id}]
    case M.ThreadCreated{+id, _, _, _, _, _, _, _, _}:
      [RK{"thread", id}]
    case M.ThreadRenamed{+id, _, _}:
      [RK{"thread", id}]
    case M.MessagePosted{+th, _, _, _, _}:
      [RK{"thread", th}]
    case M.TurnChanged{+th, _, _}:
      [RK{"thread", th}, RK{"run", th}, K.set("todos." ++ th), K.set("todos.base." ++ th), K.set("todos.visible." ++ th), K.set("todos.gen." ++ th)]
    case M.PendingChanged{+th, _, _}:
      [RK{"thread", th}]
    case M.ActivityLogged{_, _, _, _, _, _}:
      Nil{}
    case M.ThreadSettled{+id, _, _}:
      [RK{"thread", id}]
    case M.ThreadUnsettled{+id, _}:
      [RK{"thread", id}]
    case M.ThreadWoke{+id, _}:
      [RK{"thread", id}]
    case M.ThreadArchived{+id, _, _}:
      [RK{"thread", id}]
    case M.ThreadPinned{+id, _, _}:
      [RK{"thread", id}]
    case M.ThreadDeleted{+id, _}:
      [RK{"thread", id}]
    case M.WorktreeSet{+id, _, _, _}:
      [RK{"thread", id}]
    case M.SessionSet{+id, _, _}:
      [RK{"thread", id}]
    case M.ModesSet{+id, _, _, _, _}:
      [RK{"thread", id}]
    case M.TaskDelegated{+id, _, _, _, _, _, _}:
      [RK{"task", id}]
    case M.TaskStateSet{+id, _, _, _}:
      [RK{"task", id}]
    case M.TaskDelivered{+id, _}:
      [RK{"task", id}]
    case M.SettingSet{+k, _, _}:
      [K.set(k)]
    case M.TurnQueued{+th, _, _, _}:
      [RK{"queue", th}]
    case M.QueueTaken{+th, _}:
      RK{"queue", th} <> K.hops.of(BT.World.of_thread(M.State.world.of(pre), th))
    case M.AskOpened{+id, _, _, _, _, _, _}:
      [RK{"ask", id}]
    case M.AskClosed{+id, _, _}:
      [RK{"ask", id}]
    case M.DeviceRegistered{+tok, _, _, _, _, _, _}:
      [RK{"device", tok}]
    case M.DeviceDropped{+tok, _}:
      [RK{"device", tok}]
    case M.ThreadSnoozed{+id, _, _}:
      [RK{"nav", id}]
    case M.ThreadOrdered{+id, _, _, _}:
      [RK{"nav", id}]
    case M.WorktreeSkipped{+id, _}:
      [RK{"thread", id}]
    case M.ThreadMerged{+id, _, _}:
      [RK{"thread", id}, K.set(M.Merge.key(id))]
    case M.WorktreeRemoved{+id, _}:
      [RK{"thread", id}]
    case M.ProjectOrdered{+id, _, _}:
      [RK{"nav", M.Nav.project(id)}]
    case M.QueueEdited{+th, _, _, _}:
      [RK{"queue", th}]
    case M.QueueRemoved{+th, _, _}:
      [RK{"queue", th}]
    case M.QueueMoved{+th, _, _, _}:
      [RK{"queue", th}]
    case M.QueueTakenOne{+th, _, _}:
      [RK{"queue", th}]
    case M.ThreadForked{+id, _, _}:
      [K.set(M.Fork.key(id))]
    case M.ProviderSet{+id, _, _, _}:
      +old = M.Switch.prov(M.State.thread.get(pre, id))
      [RK{"thread", id}, K.set(M.Switch.session.key(id, old)), K.set(M.Switch.seen.key(id, old))]
    case M.TodosChanged{+th, _, _, _}:
      [K.set("todos." ++ th), K.set("todos.base." ++ th), K.set("todos.visible." ++ th)]
    case M.TodosSet{+th, _, _}:
      [K.set("todos." ++ th), K.set("todos.base." ++ th), K.set("todos.visible." ++ th)]
    case M.UsageSet{+th, _, _, _}:
      [K.set(M.Usage.key(th))]
    case M.BotSet{+id, +name, _, _, _, _}:
      RK{"bot", id} <> RK{"mailname", "name:" ++ String.to_lower(name)} <> K.names(pre, id)
    case M.BotDeleted{+id, _}:
      RK{"bot", id} <> K.names(pre, id)
    case M.RoomSet{+id, _, _, _}:
      RK{"room", id} <> K.conv.both(pre, post, "room:" ++ id)
    case M.RoomDeleted{+id, _}:
      [RK{"room", id}]
    case M.RoomPosted{_, +room, +from, _, _, _}:
      RK{"talk", from} <> K.room.mail(String.eq(M.Setting.get(pre, "mail.on", "off"), "on"), post, room)
    case M.HookSet{+id, _, _, _, _}:
      [RK{"hook", id}]
    case M.HookHit{+id, _}:
      [RK{"hook", id}]
    case M.RoutineSet{+id, _, _, _, _, _, _}:
      [RK{"routine", id}]
    case M.RoutineDeleted{+id, _}:
      [RK{"routine", id}]
    case M.RoutineFired{+id, _}:
      [RK{"routine", id}]
    case M.MemorySaved{+bot, +key, _, _, _, _}:
      [RK{"mem", BT.Mem.at(bot, key)}]
    case M.MemoryForgot{+bot, +key, _}:
      [RK{"mem", BT.Mem.at(bot, key)}]
    case M.SpaceSet{+bot, _, _}:
      [RK{"space", bot}]
    case M.PeerSet{+id, _, _, _, _}:
      [RK{"peer", id}]
    case M.BotWoke{+bot, _, _}:
      [RK{"hops", bot}]
    case M.SpacePageSet{+bot, _, _}:
      [RK{"page", bot}]
    case M.WorkSet{+th, +kind, _, _}:
      [K.set(kind ++ "." ++ th)]
    case M.UberSet{_, _, _}:
      [K.set("uber.bot"), K.set("uber.uid")]
    case M.UberOp{+uid, +origin, +n, _, +op, _}:
      K.uber(pre, uid, origin, n, op)
    case M.ThreadParentSet{+id, _, _}:
      [K.set("thread.parent." ++ id)]
    case M.ThreadLaunched{+id, +by, _, _}:
      [K.set(M.Launch.key(id)), K.set(M.Launch.list.key(by))]
    case M.WakeSet{+id, +by, +kind, +target, _, _, _, _}:
      [K.set("wake." ++ id), K.set("wakes.by." ++ by), K.set("wakes.on." ++ kind ++ ":" ++ target), K.set("wakes.poll")]
    case M.WakeDropped{+id, _}:
      K.wake.drop(M.Wake.get(pre, id), id)
    case M.LockChanged{+name, _, _}:
      [K.set("lock." ++ name), K.set("locks.names")]
    case M.RoutinePlan{+id, _, _, _, _}:
      [RK{"routine", id}]
    case M.RoutineSlot{+id, _, _}:
      [RK{"routine", id}]
    case M.RoutineDone{+id, _}:
      [RK{"routine", id}]
    case M.Reminded{_, _, _, _, _}:
      Nil{}
    case M.TaskSet{+bot, +slug, _, _}:
      [RK{"skill", BT.Task.at(bot, slug)}]
    case M.TaskDeleted{+bot, +slug, _}:
      [RK{"skill", BT.Task.at(bot, slug)}]
    case M.MessageFrom{+th, +msg, _, _, _, _}:
      [RK{"origin", th ++ "/" ++ msg}]
    case M.ConvSet{+id, _, _, _, _, _, _}:
      K.conv.both(pre, post, id)
    case M.MailPosted{_, +conv, _, _, _, _, _, _, _, _}:
      K.conv(post, conv)
    case M.MailMarked{+bot, +conv, _, _, _, _}:
      [RK{"mark", bot ++ "/" ++ conv}]
    case M.MailTagged{+bot, +conv, _, _}:
      [RK{"mark", bot ++ "/" ++ conv}]
    case M.RunSet{+th, +bot, +conv, _, _}:
      [RK{"run", th}, RK{"run", ML.Box.live(M.State.mail(pre), bot, conv)}]
    case M.CharterSet{+bot, _, _, _}:
      [RK{"charter", bot}]
    case M.DecisionLogged{_, +bot, _, _, _, _, _}:
      [RK{"decision", bot}]
    case M.TurnUsage{+th, +pv, +mo, _, _, _, _, _, _, _, _, _, +at}:
      K.tokens(pre, th, pv, mo, at)
    case M.DeployAsked{_, _, _, _, _}:
      [K.set("deploy.ship")]
    case M.DeployBuilt{_, _, _, _, _}:
      [K.set("deploy.ship")]

# Facts
# -----

# a line's place in the log: its number (from 1), byte offset and length
# (the newline not counted), and the next entry's ordinal
type At is Data:
  At{seq: Nat, off: Nat, len: Nat, ord: Nat}

def F.terms(ws: List<&2, String>, +ord: Nat, acc: List<&2, Op>) -> List<&2, Op>:
  match ws:
    case Nil{}:
      acc
    case w <> t:
      F.terms(t, ord, Put{"term", [w, S.n(ord)]} <> acc)

def F.entry.cols(e: M.Entry, +seq: String, +ord: String, +id: String, +th: String, +src: String, +text: String) -> List<&2, String>:
  match e:
    case M.Msg{_, _, r, _, at}:
      [seq, ord, id, th, src, "msg", C.Role.name(r), "", text, S.n(at)]
    case M.Act{_, _, tone, kind, _, at}:
      [seq, ord, id, th, src, "act", tone, kind, text, S.n(at)]

# an entry's row and its search terms (THIN.Words.of: what thin search
# matches, so a query is answered exactly)
def F.entry(+e: M.Entry, +a: At) -> List<&2, Op>:
  match a:
    case At{+seq, _, _, +ord}:
      +id = M.Idx.id(e)
      +th = M.Idx.thread(e)
      +from = M.Link.from(id)
      +src = Bool.pick(String, Bool.or(String.is_empty(from), String.eq(from, th)), "", from)
      +text = THIN.Entry.text(e)
      Put{"entry", F.entry.cols(e, S.n(seq), S.n(ord), id, th, src, text)} <> F.terms(Map.keys(&2, Bool, THIN.Words.of(text)), ord, Nil{})

# the entry a change posts (State.post's three producers), if any
def F.posts(c: M.Change) -> Maybe<&2, M.Entry>:
  match c:
    case M.MessagePosted{th, id, role, text, at}:
      Some{M.Msg{id, th, role, text, at}}
    case M.ActivityLogged{th, id, tone, kind, text, at}:
      Some{M.Act{id, th, tone, kind, text, at}}
    case M.Reminded{+id, _, th, text, +at}:
      Some{M.Act{"remind:" ++ id ++ ":" ++ Nat.show(at), th, "remind", "reminder", text, at}}
    case _:
      None{}

def F.mail(fresh: Bool, +seq: Nat, m: ML.Mail) -> List<&2, Op>:
  match fresh m:
    case False{} _:
      Nil{}
    case True{} ML.Mail{i, c, f, t, k, r, w, h, sz, at}:
      [Put{"mail", [S.n(seq), i, c, f, t, k, r, w, h, S.n(sz), S.n(at)]}]

# a room post's mail header, as State.mail_post files it (only with
# mail.on not "on", and not twice: ML.Box.mail)
def F.room.mail(on: Bool, +pre: M.State, +seq: Nat, +id: String, +room: String, +from: String, +text: String, +at: Nat) -> List<&2, Op>:
  match on:
    case True{}:
      Nil{}
    case False{}:
      +b = M.State.mail(pre)
      F.mail(Bool.not(ML.Box.has(b, id)), seq, ML.Mail{id, "room:" ++ room, ML.Box.sender(b, from), "room:" ++ room, "room", room, "fyi", ML.Mail.head(text), ML.Mail.text_size(text), at})

def F.other(+pre: M.State, +seq: Nat, c: M.Change) -> List<&2, Op>:
  match c:
    case M.RoomPosted{+id, +room, +from, +text, +hop, +at}:
      Put{"post", [S.n(seq), id, room, from, text, S.n(hop), S.n(at)]} <> F.room.mail(String.eq(M.Setting.get(pre, "mail.on", "off"), "on"), pre, seq, id, room, from, text, at)
    case M.MailPosted{+id, c2, f, t, k, r, w, h, sz, at}:
      F.mail(Bool.not(ML.Box.has(M.State.mail(pre), id)), seq, ML.Mail{id, c2, f, t, k, r, w, h, sz, at})
    case _:
      Nil{}

def F.entry.of(m: Maybe<&2, M.Entry>, +a: At) -> List<&2, Op>:
  match m:
    case Some{e}:
      F.entry(e, a)
    case None{}:
      Nil{}

# what clients see of a line, when it is not the line itself: a phone's
# push registration goes out without its token (C.Change.wire_line)
def F.wire(c: M.Change) -> String:
  match c:
    case M.DeviceRegistered{tok, p, k, th, e, b, at}:
      J.Json.show(C.Change.wire(M.DeviceRegistered{tok, p, k, th, e, b, at}))
    case M.DeviceDropped{tok, at}:
      J.Json.show(C.Change.wire(M.DeviceDropped{tok, at}))
    case _:
      ""

# a line clients see: its number among them (lines that parse, as the
# hub's log of lines counts them), where it is in the file, and its wire
# text when that is not the line's own
def F.line(+a: At, w: String) -> Op:
  match a:
    case At{+seq, +off, +len, _}:
      Put{"line", [S.n(seq), S.n(off), S.n(len), w]}

def At.seq(a: At) -> Nat:
  match a:
    case At{s, _, _, _}:
      s

# a line's facts: the line, and what it posts
def Sdb.facts(+pre: M.State, +a: At, +c: M.Change) -> List<&2, Op>:
  F.line(a, F.wire(c)) <> List.append(&2, Op, F.entry.of(F.posts(c), a), F.other(pre, At.seq(a), c))

def At.ord(a: At) -> Nat:
  match a:
    case At{_, _, _, o}:
      o

# the next entry's ordinal after a change
def Sdb.ord.k(m: Maybe<&2, M.Entry>, +ord: Nat) -> Nat:
  match m:
    case Some{_}:
      1n+ord
    case None{}:
      ord

def Sdb.ord(+c: M.Change, +ord: Nat) -> Nat:
  Sdb.ord.k(F.posts(c), ord)

# One line applied
# ----------------

# what a line does to the store: the state after it, its facts and the
# records it touched (the state already after it), and the next ordinal
type Step is Data:
  Step{st: M.State, facts: List<&2, Op>, recs: List<&2, Op>, ord: Nat}

def Sdb.step.k(+pre: M.State, +post: M.State, +a: At, +c: M.Change) -> Step:
  Step{post, Sdb.facts(pre, a, c), Row.ofs(RK{"state", "seq"} <> Sdb.keys(pre, post, c), post, Nil{}), Sdb.ord(c, At.ord(a))}

def Sdb.step(+pre: M.State, +a: At, +c: M.Change) -> Step:
  Sdb.step.k(pre, M.State.apply(pre, c), a, c)

# A pure store
# ------------
# The tables as maps from "<table>\x1f<key>" to the row ("\x1f" between
# columns): what store.db holds, for the laws. Facts and records apart.

def Tab.sep() -> String:
  "\u{1f}"

def Tab.key(+t: String, +key: List<&2, String>) -> String:
  t ++ Tab.sep() ++ String.join(key, Tab.sep())

def Tab.op(m: Map<&2, String>, o: Op) -> Map<&2, String>:
  match o:
    case Put{+t, +cols}:
      Map.set(&2, String, m, Tab.key(t, List.take(&2, String, cols, Tab.keyn(t))), String.join(cols, Tab.sep()))
    case Del{+t, key}:
      Map.del(&2, String, m, Tab.key(t, key))

def Tab.ops(os: List<&2, Op>, m: Map<&2, String>) -> Map<&2, String>:
  match os:
    case Nil{}:
      m
    case o <> t:
      Tab.ops(t, Tab.op(m, o))

# a log line as the store reads it: its length in bytes and its change
# (None: a line no change decodes from, which still has its place)
# (parsed False: a line that is not JSON, a torn write's fragment: it takes
# its bytes but no number, as clients never see it; parsed with no change:
# a newer hub's change, seen by clients, folded by no one)
type LL is Data:
  LL{len: Nat, parsed: Bool, change: Maybe<&2, M.Change>}

type Mat is Data:
  Mat{st: M.State, facts: Map<&2, String>, recs: Map<&2, String>, seq: Nat, off: Nat, ord: Nat}

def Mat.empty() -> Mat:
  Mat{M.State.empty(), Map.new(&2, String), Map.new(&2, String), 0n, 0n, 1n}

def Mat.recs(m: Mat) -> Map<&2, String>:
  match m:
    case Mat{_, _, r, _, _, _}:
      r

def Mat.facts(m: Mat) -> Map<&2, String>:
  match m:
    case Mat{_, f, _, _, _, _}:
      f

def Mat.st(m: Mat) -> M.State:
  match m:
    case Mat{s, _, _, _, _, _}:
      s

def Mat.put(s: Step, fs: Map<&2, String>, rs: Map<&2, String>, +seq: Nat, +off: Nat) -> Mat:
  match s:
    case Step{st, f, r, ord}:
      Mat{st, Tab.ops(f, fs), Tab.ops(r, rs), seq, off, ord}

def Mat.step.k(c: Maybe<&2, M.Change>, +st: M.State, fs: Map<&2, String>, rs: Map<&2, String>, +seq: Nat, +off: Nat, +len: Nat, +ord: Nat) -> Mat:
  match c:
    case Some{ch}:
      Mat.put(Sdb.step(st, At{seq, off, len, ord}, ch), fs, rs, seq, Nat.add(off, 1n+len))
    case None{}:
      Mat{st, Tab.op(fs, F.line(At{seq, off, len, ord}, "")), rs, seq, Nat.add(off, 1n+len), ord}

def Mat.step.p(parsed: Bool, c: Maybe<&2, M.Change>, +st: M.State, fs: Map<&2, String>, rs: Map<&2, String>, +n: Nat, +off: Nat, +len: Nat, +ord: Nat) -> Mat:
  match parsed:
    case True{}:
      Mat.step.k(c, st, fs, rs, 1n+n, off, len, ord)
    case False{}:
      Mat{st, fs, rs, n, Nat.add(off, 1n+len), ord}

# one more line: numbered after the last, starting where it ended
def Mat.step(m: Mat, l: LL) -> Mat:
  match m l:
    case Mat{st, fs, rs, +n, +off, +ord} LL{+len, p, c}:
      Mat.step.p(p, c, st, fs, rs, n, off, len, ord)

# the store after a log, oldest line first
def Mat.replay(ls: List<&2, LL>, m: Mat) -> Mat:
  match ls:
    case Nil{}:
      m
    case l <> t:
      Mat.replay(t, Mat.step(m, l))

# Every record of a state
# -----------------------
# What a full rebuild writes after the last line, and what the incremental
# store must equal (laws store_eq_fold_*).

def Dump.d0(xs: List<&2, Sigma<&2, &2, String, _ => M.Project>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d0(t, Row.project(v) <> acc)

def Dump.d1(xs: List<&2, Sigma<&2, &2, String, _ => M.Thread>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d1(t, Row.thread(v) <> acc)

def Dump.d2(xs: List<&2, Sigma<&2, &2, String, _ => M.Task>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d2(t, Row.task(v) <> acc)

def Dump.d3(xs: List<&2, Sigma<&2, &2, String, _ => M.Ask>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d3(t, Row.ask(v) <> acc)

def Dump.d4(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.QItem>>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d4(t, Row.queue(k, v) <> acc)

def Dump.d5(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d5(t, Row.set(k, v) <> acc)

def Dump.d6(xs: List<&2, Sigma<&2, &2, String, _ => M.Device>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d6(t, Row.device(v) <> acc)

def Dump.d7(xs: List<&2, Sigma<&2, &2, String, _ => M.Nav>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d7(t, Row.nav(k, v) <> acc)

def Dump.d8(xs: List<&2, Sigma<&2, &2, String, _ => BT.Bot>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d8(t, Row.bot(v) <> acc)

def Dump.d9(xs: List<&2, Sigma<&2, &2, String, _ => BT.Room>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d9(t, Row.room(v) <> acc)

def Dump.d10(xs: List<&2, Sigma<&2, &2, String, _ => BT.Routine>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d10(t, Row.routine(v) <> acc)

def Dump.d11(xs: List<&2, Sigma<&2, &2, String, _ => BT.Hook>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d11(t, Row.hook(v) <> acc)

def Dump.d12(xs: List<&2, Sigma<&2, &2, String, _ => BT.Mem>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d12(t, Row.mem(k, v) <> acc)

def Dump.d13(xs: List<&2, Sigma<&2, &2, String, _ => BT.Peer>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d13(t, Row.peer(v) <> acc)

def Dump.d14(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d14(t, Row.obj("space", k, J.Str{v}) <> acc)

def Dump.d15(xs: List<&2, Sigma<&2, &2, String, _ => BT.Page>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d15(t, Row.page(k, v) <> acc)

def Dump.d16(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d16(t, Row.obj("skill", k, J.Str{v}) <> acc)

def Dump.d17(xs: List<&2, Sigma<&2, &2, String, _ => BT.Hops>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d17(t, Row.hops(k, v) <> acc)

def Dump.d18(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d18(t, Row.obj("talk", k, C.Codec.nat(v)) <> acc)

def Dump.d19(xs: List<&2, Sigma<&2, &2, String, _ => ML.Conv>>, +b: ML.Box, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d19(t, b, Row.conv(b, v) <> acc)

def Dump.d20(xs: List<&2, Sigma<&2, &2, String, _ => ML.Mark>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d20(t, Row.mark(k, v) <> acc)

def Dump.d21(xs: List<&2, Sigma<&2, &2, String, _ => ML.Run>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (_, v) <> t:
      Dump.d21(t, Row.run(v) <> acc)

def Dump.d22(xs: List<&2, Sigma<&2, &2, String, _ => ML.Origin>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d22(t, Row.origin(k, v) <> acc)

def Dump.d23(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d23(t, Row.obj("charter", k, J.Str{v}) <> acc)

def Dump.d24(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, ML.Decision>>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d24(t, Row.obj("decision", k, J.Json.arr(Row.decisions.js(v, Nil{}))) <> acc)

def Dump.d25(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: List<&2, Op>) -> List<&2, Op>:
  match xs:
    case Nil{}:
      acc
    case (k, v) <> t:
      Dump.d25(t, Row.obj("mailname", k, J.Str{v}) <> acc)

def Dump.ops(+s: M.State) -> List<&2, Op>:
  +w = M.State.world.of(s)
  +b = M.State.mail(s)
  Dump.d25(Map.to_list(&2, String, ML.Box.names(b)),
    Dump.d24(Map.to_list(&2, List<&2, ML.Decision>, ML.Box.decision_log(b)),
    Dump.d23(Map.to_list(&2, String, ML.Box.charters(b)),
    Dump.d22(Map.to_list(&2, ML.Origin, ML.Box.origins(b)),
    Dump.d21(Map.to_list(&2, ML.Run, ML.Box.runs(b)),
    Dump.d20(Map.to_list(&2, ML.Mark, ML.Box.marks(b)),
    Dump.d19(Map.to_list(&2, ML.Conv, ML.Box.convs(b)), b,
    Dump.d18(Map.to_list(&2, Nat, W.talk(w)),
    Dump.d17(Map.to_list(&2, BT.Hops, W.hops(w)),
    Dump.d16(Map.to_list(&2, String, W.skills(w)),
    Dump.d15(Map.to_list(&2, BT.Page, W.pages(w)),
    Dump.d14(Map.to_list(&2, String, W.spaces(w)),
    Dump.d13(Map.to_list(&2, BT.Peer, W.peers(w)),
    Dump.d12(Map.to_list(&2, BT.Mem, W.mems(w)),
    Dump.d11(Map.to_list(&2, BT.Hook, W.hooks(w)),
    Dump.d10(Map.to_list(&2, BT.Routine, W.routines(w)),
    Dump.d9(Map.to_list(&2, BT.Room, W.rooms(w)),
    Dump.d8(Map.to_list(&2, BT.Bot, W.bots(w)),
    Dump.d7(Map.to_list(&2, M.Nav, M.State.navs(s)),
    Dump.d6(Map.to_list(&2, M.Device, St.dv(s)),
    Dump.d5(Map.to_list(&2, String, St.sg(s)),
    Dump.d4(Map.to_list(&2, List<&2, M.QItem>, St.qu(s)),
    Dump.d3(Map.to_list(&2, M.Ask, St.ak(s)),
    Dump.d2(Map.to_list(&2, M.Task, St.tk(s)),
    Dump.d1(Map.to_list(&2, M.Thread, St.ts(s)),
    Dump.d0(Map.to_list(&2, M.Project, St.ps(s)),
    [Row.obj("state", "seq", C.Codec.nat(M.State.seq(s)))]))))))))))))))))))))))))))

# every record of a state, as the pure store holds them
def Tab.dump(+s: M.State) -> Map<&2, String>:
  Tab.ops(Dump.ops(s), Map.new(&2, String))

def Tab.entry.k(msg: Bool, id: String, th: String, r: String, ak: String, text: String, +at: Nat) -> M.Entry:
  match msg:
    case True{}:
      M.Msg{id, th, C.Role.of(r), text, at}
    case False{}:
      M.Act{id, th, r, ak, text, at}

# Reading the pure store's timeline (the hub's store reads mirror these)
# ---------------------------------------------------------------------

# an entry row's fields back as an Entry
def Tab.entry.of(cs: List<&2, String>) -> M.Entry:
  match cs:
    case _ <> _ <> id <> th <> _ <> +k <> r <> ak <> text <> at <> Nil{}:
      Tab.entry.k(String.eq(k, "msg"), id, th, r, ak, text, M.Switch.num(at, 0n))
    case _:
      M.Act{"", "", "", "", "", 0n}

def Tab.mine.k(+cs: List<&2, String>, +th: String, +acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match cs:
    case _ <> _ <> _ <> +t <> +src <> _:
      Bool.pick(List<&2, M.Entry>, Bool.or(String.eq(t, th), String.eq(src, th)), Tab.entry.of(cs) <> acc, acc)
    case _:
      acc

# a thread's entries and the messages it sent, newest first (M.State.mine
# over the store: thread = th or src = th, by ordinal)
def Tab.mine.go(rows: List<&2, String>, +th: String, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match rows:
    case Nil{}:
      acc
    case r <> t:
      Tab.mine.go(t, th, Tab.mine.k(String.split(r, '\u{1f}'), th, acc))

# a row's ordinal, zero-padded so a map keyed by it walks in order
def Tab.pad.go(+n: Nat, s: String) -> String:
  match n:
    case 0n:
      s
    case 1n+k:
      Tab.pad.go(k, "0" ++ s)

def Tab.ord(cs: List<&2, String>) -> String:
  match cs:
    case _ <> +o <> _:
      Tab.pad.go(Nat.sub(20n, String.length(o)), o)
    case _:
      ""

def Tab.by_ord.k(keep: Bool, +v: String, m: Map<&2, String>) -> Map<&2, String>:
  match keep:
    case True{}:
      Map.set(&2, String, m, Tab.ord(String.split(v, '\u{1f}')), v)
    case False{}:
      m

def Tab.by_ord(xs: List<&2, Sigma<&2, &2, String, _ => String>>, m: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}:
      m
    case (+k, +v) <> t:
      Tab.by_ord(t, Tab.by_ord.k(String.starts_with(k, "entry" ++ Tab.sep()), v, m))

# the entry rows by ordinal, oldest first
def Tab.entries(fs: Map<&2, String>) -> List<&2, String>:
  Map.values(&2, String, Tab.by_ord(Map.to_list(&2, String, fs), Map.new(&2, String)))

def Tab.mine(fs: Map<&2, String>, +th: String) -> List<&2, M.Entry>:
  Tab.mine.go(Tab.entries(fs), th, Nil{})

# A log's identity
# ----------------
# store.db belongs to one log: meta log_id is "sha1:" and the hex SHA-1 of
# the log's first line (its bytes before the first newline, at most 4096),
# "" for an empty log. The build and the hub's open both use this one
# definition; a store whose log_id differs from its log's is made again.

def Id.upto.t(nl: Bool, t: List<&2, U32>) -> List<&2, U32>:
  match nl:
    case True{}:
      Nil{}
    case False{}:
      t

def Id.upto.x(nl: Bool, +x: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match nl:
    case True{}:
      acc
    case False{}:
      x <> acc

# the bytes before the first newline, at most n of them
def Id.upto(n: Nat, bs: List<&2, U32>, acc: List<&2, U32>) -> List<&2, U32>:
  match n bs:
    case _ Nil{}:
      List.reverse(&2, U32, acc)
    case 0n +x <> +t:
      List.reverse(&2, U32, acc)
    case 1n+k +x <> +t:
      Id.upto(k, Id.upto.t(U32.is_eq(x, 10), t), Id.upto.x(U32.is_eq(x, 10), x, acc))

def Id.nib(+x: U32) -> String:
  String.take(String.drop("0123456789abcdef", U32.to_nat(x)), 1n)

# (a digest: 20 bytes, so no deeper than that)
def Id.hex(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      ""
    case +b <> t:
      Id.nib(U32.shrn(b, 4n)) ++ Id.nib(U32.and(b, 15)) ++ Id.hex(t)

def Id.k(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      ""
    case +b <> t:
      "sha1:" ++ Id.hex(SH.Sha1.hash(b <> t))

# the identity of a log whose file starts with these bytes
def Sdb.log_id(bs: List<&2, U32>) -> String:
  Id.k(Id.upto(4096n, bs, Nil{}))

# A demo log for the laws
# -----------------------
# One of most kinds of change, so every domain is written incrementally and
# compared with a dump (laws sdb_eq_fold_*).

def Demo.changes() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/p", 1n}, M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 1n},
   M.ThreadCreated{"t2", "p1", "B", M.Local{}, "", "", "claude", "", 1n},
   M.MessagePosted{"t1", "m1", M.User{}, "Hello wörld—x", 2n}, M.ActivityLogged{"t2", "u1", "tool", "Bash", "ls -la", 3n},
   M.MessagePosted{"t2", M.Link.id("t1", "7"), M.User{}, "hi", 4n}, M.MessagePosted{"t2", "m2", M.Assistant{}, "b", 5n},
   M.TurnChanged{"t1", M.Running{}, 6n}, M.TurnChanged{"t1", M.Completed{}, 7n},
   M.SettingSet{"agent.model", "opus", 8n}, M.SettingSet{"launch.input.run1", "big", 8n}, M.SettingSet{"told.x", "1", 8n}, M.SettingSet{"told.x", "", 9n},
   M.TurnQueued{"t1", "q1", "later", 9n}, M.TurnQueued{"t1", "q2", "more", 9n}, M.QueueTakenOne{"t1", "q1", 10n},
   M.AskOpened{"a1", "t1", "approval", "Bash", "rm", "yes,no", 10n}, M.AskClosed{"a1", "yes", 11n},
   M.TaskDelegated{"k1", "t1", "t2", 1n, "claude", "opus", 11n}, M.TaskStateSet{"k1", M.TaskDone{}, "ok", 12n}, M.TaskDelivered{"k1", 12n},
   M.ThreadSnoozed{"t2", 99n, 12n}, M.ProjectOrdered{"p1", "80", 12n}, M.ThreadRenamed{"t2", "Bee", 13n}, M.ThreadPinned{"t2", True{}, 13n},
   M.BotSet{"b1", "Bub", "t2", 1n, "kind", 14n}, M.RoomSet{"r1", "Room", "b1", 14n},
   M.BotSet{"b2", "Two", "t1", 1n, "", 14n}, M.RoomSet{"r2", "Quiet", "b1\nb2", 14n}, M.RoomSet{"r2", "Quiet", "b1", 15n},
   M.ConvSet{"c2", "S2", "pair", "bot:b2\nperson", "p1", "", 15n}, M.ConvSet{"c2", "S2", "pair", "person", "p1", "", 16n}, M.RoomPosted{"rp1", "r1", "b1", "hey room", 0n, 15n},
   M.HookSet{"h1", "b1", "gh", False{}, 15n}, M.HookHit{"h1", 16n},
   M.RoutineSet{"ro1", "b1", "daily", "0 9 * * *", "go", True{}, 16n}, M.RoutineFired{"ro1", 17n}, M.RoutinePlan{"ro1", 0n, "person", "", 17n},
   M.MemorySaved{"b1", "k", "note", "remember", "", 17n}, M.MemorySaved{"b1", "k2", "note", "gone", "", 17n}, M.MemoryForgot{"b1", "k2", 18n},
   M.SpaceSet{"b1", "spec", 18n}, M.SpacePageSet{"b1", "<p>", 18n}, M.PeerSet{"pe1", "far", "http://x", False{}, 18n}, M.BotWoke{"b1", 1n, 19n},
   M.TaskSet{"b1", "s1", "{}", 19n}, M.MessageFrom{"t2", "m2", "bot:b1", "room", "r1", 19n},
   M.ConvSet{"c1", "Subj", "pair", "bot:b1\nperson", "p1", "", 20n},
   M.MailPosted{"ml1", "c1", "person", "bot:b1", "note", "", "fyi", "Subj", 3n, 20n}, M.MailPosted{"ml1", "c1", "person", "bot:b1", "note", "", "fyi", "Subj", 3n, 20n},
   M.MailMarked{"b1", "c1", "read", 20n, 0n, 21n}, M.RunSet{"t2", "b1", "c1", "ml1", 21n}, M.CharterSet{"b1", "be good", "person", 21n},
   M.DecisionLogged{"d1", "b1", "c1", "t2", "did", "because", 21n},
   M.WorkSet{"t1", "bg", "row", 22n}, M.LockChanged{"bend", LK.Lock.empty(), 22n}, M.ThreadLaunched{"t2", "t1", True{}, 22n},
   M.WakeSet{"w1", "t1", "pr", "42", ["merged"], True{}, "n", 22n}, M.WakeSet{"w2", "t1", "file", "x", ["changed"], False{}, "", 22n}, M.WakeDropped{"w1", 23n},
   M.UsageSet{"t1", 100n, 1000n, 23n}, M.TurnUsage{"t1", "claude", "opus", "k", 1n, 2n, 3n, 4n, 5n, 6n, "co", "api", 1790000000n},
   M.ProviderSet{"t1", "codex", "gpt", 24n}, M.TodosSet{"t1", "[]", 24n}, M.ThreadForked{"t3", "t1", 24n},
   M.DeviceRegistered{"tok", "ios", "alert", "", "prod", "app", 25n}, M.Reminded{"rm1", "b1", "t2", "wake up", 25n},
   M.ThreadMerged{"t2", "http://pr", 26n}, M.ThreadDeleted{"t1", 27n}, M.RoomDeleted{"r1", 27n}, M.BotDeleted{"b1", 28n}, M.ProjectRemoved{"p1", 29n}]

# each change as a log line of made-up length, with one torn line and one
# from a newer hub among them
def Demo.lines(cs: List<&2, M.Change>, +i: Nat) -> List<&2, LL>:
  match cs:
    case Nil{}:
      [LL{7n, False{}, None{}}, LL{9n, True{}, None{}}]
    case c <> t:
      LL{Nat.add(10n, i), True{}, Some{c}} <> Demo.lines(t, 1n+i)

def Demo.log() -> List<&2, LL>:
  Demo.lines(Demo.changes(), 0n)

# a few changes for laws the checker normalizes (the wide demo above is
# for test/storedb_test.bend: normalizing it takes the checker minutes)
def Demo.small() -> List<&2, M.Change>:
  [M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 1n}, M.MessagePosted{"t1", "m1", M.User{}, "hi", 2n},
   M.SettingSet{"a", "1", 3n}, M.SettingSet{"a", "", 4n}, M.ThreadPinned{"t1", True{}, 5n}]

def Demo.small.log() -> List<&2, LL>:
  Demo.lines(Demo.small(), 0n)
