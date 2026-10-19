import Base
import ../core/msgs.bend as MSG
import ../core/json.bend as J
import ../core/i18n.bend as I
import ../core/model.bend as M
import ../core/client.bend as U
import ../core/codec.bend as C
import ../core/bot.bend as BT
import ../core/botui.bend as B
import ../core/cron.bend as CR
import ../core/sched.bend as SC
import ../core/cat.bend as CT
import ./space.bend as MS
import ../core/task.bend as TK

# Bots on the phones (docs/bots.md): the cats in the list with their
# moods, the rooms, the new bot and room forms, and what a bot, a room or
# a bot on a linked machine shows, as JSON. Each tab's content is made
# only while that tab is shown. The apps draw it and send actions back.

# a cat's rig (parts with path data, fill, one animation each). Several KB
# each, so the screen names a cat by its key ("look:mood") and the apps
# ask the bridge for each rig once (app.bend's cat)
def MBots.rig(+look: Nat, +mood: String) -> J.Json:
  CT.Cat.rig.json(look, mood)

def MBots.cat(+look: Nat, +mood: String) -> J.Json:
  J.Json.str(Nat.show(look) ++ ":" ++ mood)

# since when ("" for never)
def MBots.ago.loc(+lc: I.Loc, none: Bool, +now: Nat, +at: Nat) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      I.I18n.ago(lc, now, at)

# The list
# --------

# a cat in the list; "act" is what a tap sends with its id (BU.act: "bot"
# for a bot with a thread, here or mirrored, "remote" for one only
# reported; "remote" stays for apps that pick it themselves), "title" its
# name as shown (with its machine when elsewhere)
# ios-snappy: a bot's page is its thread, or a reported conversation.

def MBots.ago(none: Bool, +now: Nat, +at: Nat) -> String:
  MBots.ago.loc(I.En{}, none, now, at)

def MBots.nav.k(none: Bool, +id: String, +thread: String) -> String:
  match none:
    case True{}:
      "@remote:" ++ id
    case False{}:
      thread

def MBots.nav(+r: B.BRow) -> String:
  match r:
    case B.BRow{+id, _, +thread, _, _, _, _, _, _, _}:
      MBots.nav.k(String.is_empty(thread), id, thread)

def MBots.nav.room(+id: String) -> String:
  "@room:" ++ id

def MBots.row(+lc: I.Loc, +r: B.BRow) -> J.Json:
  match r:
    case B.BRow{+i, +n, _, +lk, +m, _, sel, +rem, peer, +l}:
      J.Json.obj([J.KV{"id", J.Json.str(i)}, J.KV{"name", J.Json.str(n)}, J.KV{"title", J.Json.str(B.BU.row.title(r))},
        J.KV{"act", J.Json.str(B.BU.act(r))}, J.KV{"look", J.Json.nat(lk)},
        J.KV{"mood", J.Json.str(m)}, J.KV{"note", J.Json.str(B.Mood.note.loc(lc, m))}, J.KV{"sel", J.Json.bool(sel)},
        J.KV{"remote", J.Json.bool(rem)}, J.KV{"peer", J.Json.str(peer)}, J.KV{"cat", MBots.cat(lk, m)},
        J.KV{"key", J.Json.str(B.BU.key.k(rem, i, n, l))}, J.KV{"link", J.Json.str(l)}, J.KV{"nav", J.Json.str(MBots.nav(r))}])

def MBots.rows(+lc: I.Loc, rs: List<&2, B.BRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      MBots.row(lc, r) <> MBots.rows(lc, t)

def MBots.room(r: B.RRow) -> J.Json:
  match r:
    case B.RRow{+i, n, m, sel}:
      J.Json.obj([J.KV{"id", J.Json.str(i)}, J.KV{"name", J.Json.str(n)}, J.KV{"members", J.Json.nat(m)}, J.KV{"sel", J.Json.bool(sel)}, J.KV{"nav", J.Json.str(MBots.nav.room(i))}])

def MBots.rooms(rs: List<&2, B.RRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      MBots.room(r) <> MBots.rooms(t)

# Forms
# -----

def MBots.providers(ps: List<&2, String>, +cur: String) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      J.Json.obj([J.KV{"label", J.Json.str(U.Chip.name(p))}, J.KV{"provider", J.Json.str(U.Chip.name(p))},
        J.KV{"on", J.Json.bool(String.eq(U.Chip.name(p), cur))}]) <> MBots.providers(t, cur)

# the provider a new bot gets: the one picked, else the hub's default
def MBots.provider(+u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(B.BU.field(u, "provider")), M.Setting.get(U.Ui.state(u), "agent.provider", "claude"), B.BU.field(u, "provider"))

def MBots.new.k(+lc: I.Loc, open: Bool, +u: U.Ui) -> J.Json:
  match open:
    case False{}:
      J.Null{}
    case True{}:
      J.Json.obj([J.KV{"name", J.Json.str(B.BU.field(u, "name"))}, J.KV{"hint", J.Json.str(U.Bots.name.hint.loc(lc, u))}, J.KV{"persona", J.Json.str(B.BU.field(u, "persona"))},
        J.KV{"provider", J.Json.str(MBots.provider(u))}, J.KV{"providers", J.Json.arr(MBots.providers(U.Chip.providers(), MBots.provider(u)))}])

# the new bot form ("bot-new" opens it, "bfield" fills it, "bot-create"),
# or null
def MBots.new(+lc: I.Loc, +u: U.Ui) -> J.Json:
  MBots.new.k(lc, B.BU.open(u, "@bnew"), u)

def MBots.picks(ps: List<&2, B.Pick>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case B.Pick{i, n, on} <> t:
      J.Json.obj([J.KV{"id", J.Json.str(i)}, J.KV{"name", J.Json.str(n)}, J.KV{"on", J.Json.bool(on)}]) <> MBots.picks(t)

def MBots.rnew.k(open: Bool, +u: U.Ui) -> J.Json:
  match open:
    case False{}:
      J.Null{}
    case True{}:
      J.Json.obj([J.KV{"name", J.Json.str(B.BU.field(u, "rname"))}, J.KV{"picks", J.Json.arr(MBots.picks(B.BU.picks(u)))}])

# the new room form ("room-new", "bfield" rname, "room-pick" a bot id,
# "room-save"), or null
def MBots.rnew(+u: U.Ui) -> J.Json:
  MBots.rnew.k(B.BU.open(u, "@rnew"), u)

# A bot's tabs
# ------------

def MBots.tabs(+lc: I.Loc, ts: List<&2, String>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      J.Json.obj([J.KV{"id", J.Json.str(t)}, J.KV{"label", J.Json.str(B.BU.tab.label(lc, t))}]) <> MBots.tabs(lc, r)

def MBots.mem.loc(+lc: I.Loc, m: BT.Mem, +now: Nat) -> J.Json:
  match m:
    case BT.Mem{_, k, kind, text, tags, _, +up}:
      J.Json.obj([J.KV{"key", J.Json.str(k)}, J.KV{"kind", J.Json.str(kind)}, J.KV{"text", J.Json.str(text)},
        J.KV{"tags", J.Json.str(tags)}, J.KV{"updated", J.Json.str(I.I18n.ago(lc, now, up))}])

def MBots.mem(m: BT.Mem, +now: Nat) -> J.Json:
  MBots.mem.loc(I.En{}, m, now)

def MBots.mems.loc(+lc: I.Loc, ms: List<&2, BT.Mem>, +now: Nat) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      MBots.mem.loc(lc, m, now) <> MBots.mems.loc(lc, t, now)

def MBots.mems(ms: List<&2, BT.Mem>, +now: Nat) -> List<&2, J.Json>:
  MBots.mems.loc(I.En{}, ms, now)

def MBots.routine.row.loc(+lc: I.Loc, w: SC.SRow, +r: BT.Routine, +now: Nat) -> J.Json:
  match w:
    case SC.SRow{kind, label, when, next, rel, zone, done, dtext}:
      J.Json.obj([J.KV{"id", J.Json.str(BT.Routine.id(r))}, J.KV{"name", J.Json.str(BT.Routine.name(r))}, J.KV{"cron", J.Json.str(BT.Routine.cron(r))},
        J.KV{"when", J.Json.str(when)}, J.KV{"prompt", J.Json.str(BT.Routine.prompt(r))}, J.KV{"on", J.Json.bool(BT.Routine.on(r))},
        J.KV{"last", J.Json.str(MBots.ago.loc(lc, Nat.is_eq(BT.Routine.last(r), 0n), now, BT.Routine.last(r)))},
        J.KV{"kind", J.Json.str(kind)}, J.KV{"label", J.Json.str(label)}, J.KV{"next", J.Json.str(next)}, J.KV{"rel", J.Json.str(rel)},
        J.KV{"zone", J.Json.str(zone)}, J.KV{"done", J.Json.bool(done)}, J.KV{"doneText", J.Json.str(dtext)}])

def MBots.routine.row(w: SC.SRow, +r: BT.Routine, +now: Nat) -> J.Json:
  MBots.routine.row.loc(I.En{}, w, r, now)

def MBots.routine(+lc: I.Loc, +u: U.Ui, +r: BT.Routine, +now: Nat) -> J.Json:
  MBots.routine.row.loc(lc, SC.Sched.row(lc, U.Ui.state(u), r, now, 0n, True{}), r, now)

def MBots.routines(+lc: I.Loc, +u: U.Ui, rs: List<&2, BT.Routine>, +now: Nat) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      MBots.routine(lc, u, r, now) <> MBots.routines(lc, u, t, now)

# the routine form: open ("routine-edit" fills it, "routine-save"), its
# fields ("bfield" oname, ocron, oprompt); id is "" for a new one
def MBots.routine.form(+lc: I.Loc, +u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"open", J.Json.bool(B.BU.open(u, "@onew"))}, J.KV{"id", J.Json.str(B.BU.field(u, "oid"))},
    J.KV{"name", J.Json.str(B.BU.field(u, "oname"))}, J.KV{"cron", J.Json.str(B.BU.field(u, "ocron"))},
    J.KV{"prompt", J.Json.str(B.BU.field(u, "oprompt"))}, J.KV{"hint", J.Json.str(U.Routine.hint.loc(lc, u))}])

def MBots.hook.loc(+lc: I.Loc, h: BT.Hook, +now: Nat) -> J.Json:
  match h:
    case BT.Hook{+i, _, n, _, _, +last, c}:
      J.Json.obj([J.KV{"id", J.Json.str(i)}, J.KV{"name", J.Json.str(n)}, J.KV{"path", J.Json.str("/hook/" ++ i)},
        J.KV{"count", J.Json.nat(c)}, J.KV{"last", J.Json.str(MBots.ago.loc(lc, Nat.is_eq(last, 0n), now, last))}])

def MBots.hook(h: BT.Hook, +now: Nat) -> J.Json:
  MBots.hook.loc(I.En{}, h, now)

def MBots.hooks.loc(+lc: I.Loc, hs: List<&2, BT.Hook>, +now: Nat) -> List<&2, J.Json>:
  match hs:
    case Nil{}:
      Nil{}
    case h <> t:
      MBots.hook.loc(lc, h, now) <> MBots.hooks.loc(lc, t, now)

def MBots.hooks(hs: List<&2, BT.Hook>, +now: Nat) -> List<&2, J.Json>:
  MBots.hooks.loc(I.En{}, hs, now)

def MBots.peer(p: BT.Peer) -> J.Json:
  match p:
    case BT.Peer{i, n, url, _, _}:
      J.Json.obj([J.KV{"id", J.Json.str(i)}, J.KV{"name", J.Json.str(n)}, J.KV{"url", J.Json.str(url)}])

def MBots.peers(ps: List<&2, BT.Peer>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      MBots.peer(p) <> MBots.peers(t)

def MBots.str(+u: U.Ui, +k: String) -> J.KV:
  J.KV{k, J.Json.str(B.BU.field(u, k))}

# Google's sign-in: its state as the hub said it, the link to open, the
# OAuth client's fields ("bfield" gid, gsecret) and the pasted redirect
# (gpaste), and the accounts signed in, the default first; "google" sends
# an op (status, configure, begin, connect, finish, disconnect, and
# "signout:<address>" for one account)
def MBots.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.str(x) <> MBots.strs(t)

# each account's signature, in the order of the accounts
def MBots.sigs(xs: List<&2, String>, +u: U.Ui) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.str(U.Bots.sig.of(u, x)) <> MBots.sigs(t, u)

def MBots.google(+u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"status", J.Json.str(U.Ui.scratch(u, "@google"))}, J.KV{"url", J.Json.str(U.Ui.scratch(u, "@google.url"))},
    MBots.str(u, "gid"), MBots.str(u, "gsecret"), MBots.str(u, "gpaste"), J.KV{"accounts", J.Json.arr(MBots.strs(U.Bots.google.accts(u)))},
    J.KV{"sigFor", J.Json.str(U.Bots.sig.editing(u))}, J.KV{"sigText", J.Json.str(B.BU.field(u, "gsig"))}, J.KV{"sigs", J.Json.arr(MBots.sigs(U.Bots.google.accts(u), u))}])

# the persona (and the field that changes it, "bfield" persona then
# "bot-edit"), Google, and the linked machines ("peer-invite" with purl,
# "peer-join" with invite, "peer-revoke" id)
def MBots.settings(+u: U.Ui, +persona: String) -> J.Json:
  J.Json.obj([J.KV{"persona", J.Json.str(persona)}, J.KV{"personaField", J.Json.str(B.BU.field(u, "persona"))},
    J.KV{"google", MBots.google(u)}, J.KV{"peers", J.Json.arr(MBots.peers(B.BU.peers(u)))},
    J.KV{"invite", J.Json.str(U.Ui.scratch(u, "@peer.invite"))}, MBots.str(u, "purl"), J.KV{"join", J.Json.str(B.BU.field(u, "invite"))}])

# a button of the browser strip, a task's card or a paused run's card; it
# sends its own action with its value
def MBots.tbtns(bs: List<&2, U.TBtn>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case U.TBtn{+label, +action, +value, +primary} <> t:
      J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)}, J.KV{"primary", J.Json.bool(primary)}]) <> MBots.tbtns(t)

# the save sheet's rows (client.bend's Teach.rows): what a step did and its
# typed value as a chip that "tk-mark" (the step's number) makes a
# parameter, named in the field "tkp.<n>"
def MBots.srows(rs: List<&2, U.TStepRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.TStepRow{+n, +text, _, +chip, +secret, +marked, +pname} <> t:
      J.Json.obj([J.KV{"n", J.Json.nat(n)}, J.KV{"text", J.Json.str(text)}, J.KV{"chip", J.Json.str(chip)}, J.KV{"secret", J.Json.bool(secret)},
        J.KV{"marked", J.Json.bool(marked)}, J.KV{"pname", J.Json.str(pname)}, J.KV{"field", J.Json.str("tkp." ++ Nat.show(n))}]) <> MBots.srows(t)

# the strip above the page and, once a recording is stopped, the save sheet;
# "input" is whether the frame takes the person's taps and keys (bb-click
# "x y" in thousandths, bb-scroll dy, bb-type text, bb-key name, bb-nav with
# the field "bburl", bb-back, bb-reload)
def MBots.strip(+lc: I.Loc, s: U.TStrip, +u: U.Ui, +id: String) -> List<&2, J.KV>:
  match s:
    case U.TStrip{+mode, +title, +clock, +steps, bs}:
      [J.KV{"mode", J.Json.nat(mode)}, J.KV{"title", J.Json.str(title)}, J.KV{"clock", J.Json.str(clock)}, J.KV{"steps", J.Json.nat(steps)},
       J.KV{"btns", J.Json.arr(MBots.tbtns(bs))}, J.KV{"input", J.Json.bool(U.Teach.input(u, id))},
       MBots.str(u, "bburl"), MBots.str(u, "tknote"), MBots.str(u, "tkname"), MBots.str(u, "tkdesc"),
       J.KV{"rows", J.Json.arr(MBots.srows(U.Teach.rows.loc(lc, u, id)))}]

# a browser task paused for help (client.bend's Teach.ask): the same card
# with its screenshot (a hub path, loaded with the pairing token) and the
# person's choices. Answers are "answer" values; Take over is "tk-takeover"
# (the buttons carry their action).
def MBots.ask.loc(+lc: I.Loc, t: U.TAsk) -> J.Json:
  match t:
    case U.TAsk{+id, _, +slug, +n, _, +q, +risky, +shot, bs}:
      J.Json.obj([J.KV{"id", J.Json.str(id)}, J.KV{"kind", J.Json.str("browser")},
        J.KV{"head", J.Json.str(MSG.T.teach.paused(lc, slug, Nat.show(n)))},
        J.KV{"detail", J.Json.str(q)}, J.KV{"blocks", J.Json.arr(Nil{})}, J.KV{"image", J.Json.str(shot)}, J.KV{"risky", J.Json.bool(risky)},
        J.KV{"buttons", J.Json.arr(MBots.tbtns(bs))}])

def MBots.ask(t: U.TAsk) -> J.Json:
  MBots.ask.loc(I.En{}, t)

def MBots.asks.m.loc(+lc: I.Loc, browser: Bool, t: U.TAsk, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match browser:
    case True{}:
      MBots.ask.loc(lc, t) <> rest
    case False{}:
      rest

def MBots.asks.m(browser: Bool, t: U.TAsk, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  MBots.asks.m.loc(I.En{}, browser, t, rest)

def MBots.ask.kind(a: M.Ask) -> String:
  match a:
    case M.Ask{_, _, +k, _, _, _, _, _}:
      k

def MBots.asks.k(+lc: I.Loc, +a: M.Ask, rest: List<&2, J.Json>, +u: U.Ui, +bot: String) -> List<&2, J.Json>:
  MBots.asks.m.loc(lc, String.eq(MBots.ask.kind(a), "browser"), U.Teach.ask.of.loc(lc, a, u, bot), rest)

def MBots.asks.go(+lc: I.Loc, as: List<&2, M.Ask>, +u: U.Ui, +bot: String) -> List<&2, J.Json>:
  match as:
    case Nil{}:
      Nil{}
    case a <> r:
      MBots.asks.k(lc, a, MBots.asks.go(lc, r, u, bot), u, bot)

# the browser's frame: where the hub serves it, and how many it has sent
# (a new n is a new frame), with the strip
def MBots.browser(+lc: I.Loc, +u: U.Ui, +id: String) -> J.Json:
  J.Json.obj(List.append(&2, J.KV, [J.KV{"url", J.Json.str("/bots/" ++ id ++ "/screen.jpg")},
    J.KV{"n", J.Json.str(Bool.pick(String, String.is_empty(U.Ui.info(u, "shot." ++ id)), "0", U.Ui.info(u, "shot." ++ id)))}],
    List.append(&2, J.KV, MBots.strip(lc, U.Teach.strip.loc(lc, u, id), u, id),
      [J.KV{"asks", J.Json.arr(MBots.asks.go(lc, U.Asks.open(U.Ui.state(u), BT.Bot.thread(BT.World.bot(M.State.bots(U.Ui.state(u)), id))), u, id))}])))

# Tasks (client.bend's Teach.tasks): a card each; open, its run form (one
# field "tkrun.<param>" each, then "tk-run" with the slug) and Delete
# ("tk-del", which asks first: "asking")
def MBots.tparams(ps: List<&2, TK.TkParam>, +u: U.Ui) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case TK.TkParam{+name, +label, _, +example} <> t:
      J.Json.obj([J.KV{"field", J.Json.str(U.Teach.pfield(name))}, J.KV{"label", J.Json.str(TK.Tk.first(label, name))},
        J.KV{"example", J.Json.str(example)}, J.KV{"value", J.Json.str(B.BU.field(u, U.Teach.pfield(name)))}]) <> MBots.tparams(t, u)

def MBots.task(t: U.TTask, +u: U.Ui) -> J.Json:
  match t:
    case U.TTask{+slug, +name, +desc, ps, +open, +asking, +last}:
      J.Json.obj([J.KV{"slug", J.Json.str(slug)}, J.KV{"name", J.Json.str(name)}, J.KV{"desc", J.Json.str(desc)},
        J.KV{"open", J.Json.bool(open)}, J.KV{"asking", J.Json.bool(asking)}, J.KV{"last", J.Json.str(last)},
        J.KV{"params", J.Json.arr(MBots.tparams(ps, u))}])

def MBots.tasks(ts: List<&2, U.TTask>, +u: U.Ui) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> r:
      MBots.task(t, u) <> MBots.tasks(r, u)

def MBots.run(r: U.TRun) -> J.Json:
  match r:
    case U.TRun{+run, +slug, +status, _, +summary, +shots}:
      J.Json.obj([J.KV{"run", J.Json.str(run)}, J.KV{"slug", J.Json.str(slug)}, J.KV{"status", J.Json.str(status)},
        J.KV{"summary", J.Json.str(summary)}, J.KV{"shots", J.Json.arr(MBots.strs(shots))}])

def MBots.runs(rs: List<&2, U.TRun>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      MBots.run(r) <> MBots.runs(t)

def MBots.tasks.tab(+u: U.Ui, +id: String) -> J.Json:
  J.Json.obj([J.KV{"items", J.Json.arr(MBots.tasks(U.Teach.tasks(u, id), u))},
    J.KV{"runs", J.Json.arr(MBots.runs(List.take(&2, U.TRun, U.Teach.runs.shown(u, id), 8n)))}])

def MBots.space.k(none: Bool, +at: Nat, +u: U.Ui, b: BT.Bot, +id: String) -> List<&2, J.KV>:
  match none:
    case True{}:
      [J.KV{"space", MS.MSpace.json(B.BU.space(u, b), id)}]
    case False{}:
      [J.KV{"page", J.Json.obj([J.KV{"bot", J.Json.str(id)}, J.KV{"url", J.Json.str("/bots/" ++ id ++ "/space.html")}, J.KV{"n", J.Json.str(Nat.show(at))}])}]

# a bot's space: its page when it wrote one (the url the hub serves it at,
# and n, new when the page is), else its blocks; the apps hand a tapped
# link to "space-link" with bot + "\x1f" + url
def MBots.space(p: BT.Page, +u: U.Ui, b: BT.Bot, +id: String) -> List<&2, J.KV>:
  match p:
    case BT.Page{+html, +at}:
      MBots.space.k(String.is_empty(html), at, u, b, id)

# what tab i (in B.BU.tabs order) adds; the chat is the screen's thread
def MBots.tab.k(+lc: I.Loc, i: Nat, +u: U.Ui, b: BT.Bot, +id: String, +persona: String, +now: Nat) -> List<&2, J.KV>:
  match i:
    case 1n:
      MBots.space(B.BU.page(u, id), u, b, id)
    case 2n:
      [J.KV{"browser", MBots.browser(lc, u, id)}]
    case 3n:
      [J.KV{"tasks", MBots.tasks.tab(u, id)}]
    case 4n:
      [J.KV{"memory", J.Json.arr(MBots.mems.loc(lc, B.BU.mems(u, id), now))}]
    case 5n:
      [J.KV{"routines", J.Json.arr(MBots.routines(lc, u, B.BU.routines(u, id), now))}, J.KV{"routine", MBots.routine.form(lc, u)}]
    case 6n:
      [J.KV{"hooks", J.Json.arr(MBots.hooks.loc(lc, B.BU.hooks(u, id), now))}, J.KV{"secret", J.Json.str(U.Ui.scratch(u, "@hook.secret"))},
       J.KV{"secretFor", J.Json.str(U.Ui.scratch(u, "@hook.id"))}, J.KV{"token", J.Json.str(U.Ui.scratch(u, "@hook.token"))},
       J.KV{"tokenFor", J.Json.str(U.Ui.scratch(u, "@hook.tokfor"))}, J.KV{"tokenUrl", J.Json.str(U.Ui.scratch(u, "@hook.tokurl"))}, MBots.str(u, "hname")]
    case 7n:
      [J.KV{"settings", MBots.settings(u, persona)}]
    case _:
      Nil{}

# a bot's header (BU.head): its name as shown ("title": with its machine
# when mirrored from a linked machine), its mood, its tabs and the one
# shown, and whether its memory and routines may be changed here ("edit")
def MBots.head(+lc: I.Loc, +hd: B.Head) -> List<&2, J.KV>:
  [J.KV{"title", J.Json.str(B.Head.title(hd))}, J.KV{"mood", J.Json.str(B.Head.mood(hd))},
   J.KV{"note", J.Json.str(B.Head.note(hd))}, J.KV{"look", J.Json.nat(B.Head.look(hd))}, J.KV{"cat", MBots.cat(B.Head.look(hd), B.Head.mood(hd))},
   J.KV{"tab", J.Json.str(B.Head.tab(hd))}, J.KV{"tabs", J.Json.arr(MBots.tabs(lc, B.Head.tabs(hd)))},
   J.KV{"edit", J.Json.bool(B.Head.owns(hd))}]

def MBots.bot.m(+lc: I.Loc, +u: U.Ui, b: BT.Bot, +id: String, +name: String, +persona: String, +hd: B.Head) -> J.Json:
  J.Json.obj(List.append(&2, J.KV,
    [J.KV{"kind", J.Json.str("bot")}, J.KV{"id", J.Json.str(id)}, J.KV{"name", J.Json.str(name)}],
    List.append(&2, J.KV, MBots.head(lc, hd), MBots.tab.k(lc, C.Codec.pos(B.BU.tabs(), B.Head.tab(hd)), u, b, id, persona, U.Side.now(u)))))

def MBots.bot(+lc: I.Loc, +u: U.Ui, b: BT.Bot) -> J.Json:
  match b:
    case BT.Bot{+i, +n, +th, +lk, +pe, +c, +d}:
      MBots.bot.m(lc, u, BT.Bot{i, n, th, lk, pe, c, d}, i, n, pe, B.BU.head.bot(u, BT.Bot{i, n, th, lk, pe, c, d}))

# Rooms and bots elsewhere
# ------------------------

def MBots.post.loc(+lc: I.Loc, p: BT.Post, +u: U.Ui, +now: Nat) -> J.Json:
  match p:
    case BT.Post{i, _, +f, text, _, +at}:
      J.Json.obj([J.KV{"id", J.Json.str(i)}, J.KV{"from", J.Json.str(B.BU.member(u, f))}, J.KV{"text", J.Json.str(text)},
        J.KV{"ago", J.Json.str(I.I18n.ago(lc, now, at))}, J.KV{"mine", J.Json.bool(String.eq(f, "you"))}])

def MBots.post(p: BT.Post, +u: U.Ui, +now: Nat) -> J.Json:
  MBots.post.loc(I.En{}, p, u, now)

def MBots.posts.loc(+lc: I.Loc, ps: List<&2, BT.Post>, +u: U.Ui, +now: Nat) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      MBots.post.loc(lc, p, u, now) <> MBots.posts.loc(lc, t, u, now)

# a room: who is in it, its posts, the composer ("bfield" rpost, "room-post")

def MBots.posts(ps: List<&2, BT.Post>, +u: U.Ui, +now: Nat) -> List<&2, J.Json>:
  MBots.posts.loc(I.En{}, ps, u, now)

def MBots.room.view.loc(+lc: I.Loc, +u: U.Ui, r: BT.Room) -> J.Json:
  match r:
    case BT.Room{+i, +n, +m, +c, +d}:
      J.Json.obj([J.KV{"kind", J.Json.str("room")}, J.KV{"id", J.Json.str(i)}, J.KV{"name", J.Json.str(n)},
        J.KV{"members", J.Json.str(B.BU.members(u, BT.Room{i, n, m, c, d}))},
        J.KV{"posts", J.Json.arr(MBots.posts.loc(lc, B.BU.posts(u, i), u, U.Side.now(u)))},
        J.KV{"draft", J.Json.str(B.BU.field(u, "rpost"))}])

def MBots.room.view(+u: U.Ui, r: BT.Room) -> J.Json:
  MBots.room.view.loc(I.En{}, u, r)

def MBots.far.m(+lc: I.Loc, m: B.BRow, +u: U.Ui, +id: String, +hd: B.Head) -> J.Json:
  match m:
    case B.BRow{_, n, _, _, _, _, _, _, peer, _}:
      J.Json.obj(List.append(&2, J.KV, [J.KV{"kind", J.Json.str("remote")}, J.KV{"id", J.Json.str(id)}, J.KV{"name", J.Json.str(n)},
        J.KV{"peer", J.Json.str(peer)}, J.KV{"posts", J.Json.arr(MBots.posts.loc(lc, B.BU.remote.posts(u, id), u, U.Side.now(u)))},
        J.KV{"draft", J.Json.str(B.BU.field(u, "rtext"))}], MBots.head(lc, hd)))

# a bot only reported by a linked machine: the same header as a bot's
# (BU.head: "name · machine", Chat alone) over the person's conversation
# with it ("bfield" rtext, "remote-send")
def MBots.far(+lc: I.Loc, +u: U.Ui, +id: String) -> J.Json:
  MBots.far.m(lc, B.BU.remote.row(u, id), u, id, B.BU.head(u, B.VRemote{id}))

def MBots.view(+lc: I.Loc, v: B.View, +u: U.Ui) -> J.Json:
  match v:
    case B.VThread{}:
      J.Null{}
    case B.VBot{b}:
      MBots.bot(lc, u, b)
    case B.VRoom{r}:
      MBots.room.view.loc(lc, u, r)
    case B.VRemote{id}:
      MBots.far(lc, u, id)

# what the main area shows when it is a bot ("kind" bot), a room or a bot
# elsewhere ("remote"); null for a thread
def MBots.json(+lc: I.Loc, +u: U.Ui) -> J.Json:
  MBots.view(lc, B.BU.view(u), u)
