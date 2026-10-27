import Base
import ../core/msgs.bend as MSG
import ../core/json.bend as J
import ../core/i18n.bend as I
import ../core/model.bend as M
import ../core/client.bend as U
import ../core/codec.bend as C
import ./screen.bend as S
import ./notify.bend as N
import ../core/notice.bend as NO
import ./view.bend as VW
import ./picker.bend as PK
import ../core/botui.bend as BU
import ../core/bot.bend as BT
import ../core/once.bend as O
import ../core/far.bend as FR
import ../core/plane.bend as PL
import ./plane.bend as MPL
import ./bots.bend as MB
import ./tabs.bend as MT
import ./permissions.bend as PM
import ../core/ref.bend as R
import ../core/refview.bend as RE
import ../web/mail-actions.bend as MA

# The phone keeps every hub it paired with connected at once, each with
# its own client (Ui). A hub's key is its socket's host:port, which the
# app names it by. The screen is all of them in one: every hub's projects
# (tagged with the machine when there are several), alerts and the island
# from all, and the thread of the hub in focus.
#
# Everything a phone sends back names a thread or project by the id the
# screen gave it, and those ids carry their hub: "key|id". Actions on a
# row are routed by that prefix; the rest go to the hub in focus, which
# is the one whose thread was last opened.

type Hub is Data:
  Hub{key: String, ui: U.Ui}

type Hubs is Data:
  Hubs{cid: String, now: Nat, cur: String, hs: List<&2, Hub>}

# a command for one hub's socket
type HCmd is Data:
  HCmd{hub: String, cmd: U.Cmd}

type HAct is Data:
  HAct{hubs: Hubs, cmds: List<&2, HCmd>, alerts: List<&2, J.Json>}

def Hubs.cmds(a: HAct) -> List<&2, HCmd>:
  match a:
    case HAct{_, cs, _}:
      cs

def Hubs.after(a: HAct) -> Hubs:
  match a:
    case HAct{h, _, _}:
      h

def Hub.ui(h: Hub) -> U.Ui:
  match h:
    case Hub{_, u}:
      u

def Hub.key(h: Hub) -> String:
  match h:
    case Hub{k, _}:
      k

# the device's language: every hub's Ui holds it after Hubs.local, so the
# hub in focus (else the first) answers for all, once a screen
def Hubs.loc.pick(hit: Bool, u: U.Ui, fallback: U.Ui) -> U.Ui:
  match hit:
    case True{}: u
    case False{}: fallback

def Hubs.loc.go(hs: List<&2, Hub>, +cur: String, fallback: U.Ui) -> U.Ui:
  match hs:
    case Nil{}: fallback
    case Hub{+k, u} <> t:
      Hubs.loc.pick(String.eq(k, cur), u, Hubs.loc.go(t, cur, fallback))

def Hubs.loc.first(hs: List<&2, Hub>) -> U.Ui:
  match hs:
    case Nil{}: U.Ui.init(0n)
    case Hub{_, u} <> _: u

def Hubs.loc(hs: Hubs) -> I.Loc:
  match hs:
    case Hubs{_, _, cur, +list}: U.Lang.loc(Hubs.loc.go(list, cur, Hubs.loc.first(list)))

def Hubs.init(now: Nat, +cid: String) -> Hubs:
  Hubs{cid, now, "", Nil{}}

# Ids
# ---

# an id of hub k (no id stays none: law hubs_no_id)
def Hubs.id.k(none: Bool, +k: String, +s: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      k ++ "|" ++ s

def Hubs.id(+k: String, +s: String) -> String:
  Hubs.id.k(String.is_empty(s), k, s)

# nav qualifies threads exactly as sel; room/remote ids stay the focused
# hub's, just as the screen's bot.id does (Q leaves bot untouched).
def Hubs.nav.tag(nav: String) -> Bool:
  match nav:
    case SCon{'@', SCon{'r', SCon{'o', SCon{'o', SCon{'m', SCon{':', _}}}}}}:
      True{}
    case SCon{'@', SCon{'r', SCon{'e', SCon{'m', SCon{'o', SCon{'t', SCon{'e', SCon{':', _}}}}}}}}:
      True{}
    case _:
      False{}

def Hubs.nav.qual(+hub: String, +nav: String) -> String:
  Bool.pick(String, Hubs.nav.tag(nav), nav, Hubs.id(hub, nav))

def Hubs.nav.bot(+hub: String, +row: BU.BRow) -> String:
  Hubs.nav.qual(hub, MB.MBots.nav(row))

# the hub a value names ("" when it names none), and the value without it
def Hubs.key_of(+v: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(v, '|')), "")

def Hubs.rest.k(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case _ <> t:
      String.join(t, "|")

def Hubs.rest(+v: String) -> String:
  Hubs.rest.k(String.split(v, '|'))

# what a field of the screen holds: 1 left alone (a thread's entries, the
# viewer's, model picker's and project picker's own values; the bot, room
# or remote bot shown and the bot and room forms, all of the hub in focus,
# whose actions go there), 2 an id of this hub, 3 a destination nav (only
# thread destinations qualify), 0 anything else
# (asked of every field of the screen, thousands a screen: on a phone a
# chain of every name tried for every key was a third of a screen. A hash
# of the key's first two letters turns away two in three keys at once
# (Q.may: a bit a hash, set for the names' first letters), "value" and
# "id", most of the rest, are tried first, and the rare names last)
type QN is Data:
  QN{name: String, kind: Nat}

def Q.names() -> List<&2, QN>:
  [QN{"sel", 2n}, QN{"thread", 2n}, QN{"entries", 1n}, QN{"live", 1n}, QN{"viewer", 1n}, QN{"blocks", 1n}, QN{"bot", 1n},
    QN{"picker", 1n}, QN{"permissions", 1n}, QN{"folders", 1n}, QN{"find", 1n}, QN{"asks", 1n}, QN{"attaching", 1n}, QN{"menu", 1n}, QN{"diff", 1n},
    QN{"term", 1n}, QN{"skills", 1n}, QN{"settings", 1n}, QN{"newBot", 1n}, QN{"newRoom", 1n}, QN{"plane", 1n}]

def Q.hash(+a: U32, +b: U32) -> U32:
  U32.and(U32.shrn(((a * 29 : U32) + b : U32), 2n), 31)

def Q.bit(+a: U32, +b: U32) -> Bool:
  U32.is_eq(U32.and(U32.shrn(3100017390, U32.to_nat(Q.hash(a, b))), 1), 1)

# whether a key could be one of the names (never false for one of them:
# law q_may_names)
def Q.may(+k: String) -> Bool:
  match k:
    case SCon{Chr{+a}, SCon{Chr{+b}, _}}:
      Q.bit(a, b)
    case _:
      False{}

def Q.step.k(hit: Bool, n: QN, st: Nat) -> Nat:
  match hit n:
    case True{} QN{_, kind}:
      kind
    case False{} _:
      st

def Q.step(+st: Nat, n: QN, +k: String) -> Nat:
  match st n:
    case 0n QN{+name, kind}:
      Q.step.k(String.eq(name, k), QN{name, kind}, 0n)
    case 1n+j _:
      1n+j

def Q.find(ns: List<&2, QN>, +k: String, +st: Nat) -> Nat:
  match ns:
    case Nil{}:
      st
    case n <> t:
      Q.find(t, k, Q.step(st, n, k))

def Q.rare(id: Bool, +k: String) -> Nat:
  match id:
    case True{}:
      2n
    case False{}:
      Q.find(Q.names(), k, 0n)

def Q.named(value: Bool, +k: String) -> Nat:
  match value:
    case True{}:
      2n
    case False{}:
      Q.rare(String.eq(k, "id"), k)

def Q.kind.k(may: Bool, +k: String) -> Nat:
  match may:
    case True{}:
      Q.named(String.eq(k, "value"), k)
    case False{}:
      0n

def Q.kind(+k: String) -> Nat:
  Bool.pick(Nat, String.eq(k, "nav"), 3n, Q.kind.k(Q.may(k), k))

# whether every name is its own kind through Q.kind (law q_kind_names)
def Q.own(ns: List<&2, QN>) -> Bool:
  match ns:
    case Nil{}:
      True{}
    case QN{+name, +kind} <> t:
      Bool.and(Nat.is_eq(Q.kind(name), kind), Q.own(t))

# a hub's screen (or alerts) with every id made that hub's
def Q.walk(j: J.Json, +p: String, mode: Nat) -> J.Json:
  match j mode:
    case x 1n:
      x
    case J.Str{s} 2n:
      J.Str{Hubs.id(p, s)}
    case J.Str{s} 3n:
      J.Str{Hubs.nav.qual(p, s)}
    case J.Obj{fs} _:
      J.Obj{Q.walk(fs, p, 0n)}
    case J.Arr{xs} _:
      J.Arr{Q.walk(xs, p, 0n)}
    case J.Item{h, t} _:
      J.Item{Q.walk(h, p, 0n), Q.walk(t, p, 0n)}
    case J.Field{+k, v, t} _:
      J.Field{k, Q.walk(v, p, Q.kind(k)), Q.walk(t, p, 0n)}
    case x _:
      x

def Q.all(j: J.Json, +p: String) -> J.Json:
  Q.walk(j, p, 0n)

# One hub's client
# ----------------

# a bot form shut (only those: the value names a scratch key)
def One.close(+u: U.Ui, +k: String) -> U.Ui:
  Bool.pick(U.Ui, Nat.is_lt(C.Codec.pos(["@bnew", "@rnew", "@onew"], k), 3n), U.Ui.scratch_set(u, k, ""), u)

# an action, as a phone's one client takes it: "view" opens or closes the
# board viewer ("board", "schematic", "3d", ""), "view-pick" inspects a
# tap, "view-unpick" closes the card, "view-mention" puts the inspected
# item into the draft; "picker-open" opens the project picker at ~/,
# "picker-type" is its field's new text (typed, or a folder tapped),
# "picker-mkdir" makes a folder and goes into it; "find-open" opens
# thread search ("search") or the file picker ("files", which asks for the
# thread's files), "find-q" sets its query, "find-close" closes it; "form-close" shuts a bot
# form ("@bnew", "@rnew", "@onew"; the phones' cancel); any action that moves to another project re-aims
# the viewer
def One.find(files: Bool, +u: U.Ui, +mode: String) -> U.Act:
  match files:
    case True{}:
      U.Ui.act(U.Palette.open(u, mode), "files", "")
    case False{}:
      U.Act{U.Palette.open(u, mode), Nil{}}

# Reuse the current durable-body/one-shot guard; phone sends are never outbox retries.
def One.mail.core(i: Nat, +u: U.Ui, action: String, value: String) -> U.Act:
  match i:
    case 0n: U.Mailv.sync(MA.MA.send(u))
    case 1n: U.Mailv.sync(MA.MA.open(u, value))
    case 2n: U.Mailv.sync(MA.MA.mark(u, value))
    case _: PM.Permissions.act(u, action, value)

# the design history's actions run with the phone's board or schematic as
# the client's view (VW.View.hist.act)
def One.act.rest(hist: Bool, +u: U.Ui, +action: String, +value: String) -> U.Act:
  match hist:
    case True{}:
      VW.View.hist.act(u, action, value)
    case False{}:
      VW.View.sync(One.mail.core(C.Codec.pos(["mail-send", "mail-open", "mail-mark"], action), u, action, value))

# The hub's settings page of the Settings tab: "on" opens Settings (the flag
# "settings") and "off" shuts it, each only when it is not so already (a
# second "on" must not close it)
def One.hubset.k(same: Bool, +u: U.Ui) -> U.Act:
  match same:
    case True{}:
      U.Act{u, Nil{}}
    case False{}:
      U.Ui.act(u, "flag", "settings")

def One.hubset(+u: U.Ui, +value: String) -> U.Act:
  One.hubset.k(Bool.pick(Bool, String.eq(value, "on"), U.Ui.flag(u, "settings"), Bool.not(U.Ui.flag(u, "settings"))), u)

def One.act.k(i: Nat, +u: U.Ui, +action: String, +value: String) -> U.Act:
  match i:
    case 0n:
      VW.View.open(u, value)
    case 1n:
      VW.View.mention(u, value)
    case 2n:
      VW.View.pick(u, value)
    case 3n:
      U.Act{VW.Pick.clear(u), Nil{}}
    case 4n:
      PK.MPick.open(u)
    case 5n:
      PK.MPick.type(u, value)
    case 6n:
      PK.MPick.mkdir(u, value)
    case 7n:
      One.find(String.eq(value, "files"), u, value)
    case 8n:
      U.Act{U.Palette.set(u, value, 0n), Nil{}}
    case 9n:
      U.Act{U.Ui.flag_off(u, "palette"), Nil{}}
    case 10n:
      U.Act{One.close(u, value), Nil{}}
    case 11n:
      VW.View.sheet.pick(u, value)
    case 12n:
      VW.View.layer(u, value)
    case 13n:
      VW.View.parts(u)
    case 14n:
      VW.View.mech.act(u, action, value, "mech")
    case 15n:
      VW.View.mech.act(u, action, value, "renders")
    case 16n:
      VW.View.fm.act(u, action, value)
    case 17n:
      VW.View.fm.act(u, action, value)
    case 18n:
      VW.View.fm.act(u, action, value)
    case 19n:
      VW.View.fm.act(u, action, value)
    case 20n:
      VW.View.fm.act(u, action, value)
    case 21n:
      VW.View.layer.pick(u, value)
    case 22n:
      VW.View.layer.mode(u, value)
    case 23n:
      One.hubset(u, value)
    case _:
      One.act.rest(Bool.or(Bool.or(String.starts_with(action, "hist-"), String.starts_with(action, "cmp")), String.starts_with(action, "rv-")), u, action, value)

# (and the board's file asked for, to pin its comments: client.bend's Rc.sync)
# Mailroom viewport intents are rows/IDs, never display pixels. Reuse sync.
def One.mail.window(body: Bool, +u: U.Ui, +value: String) -> U.Act:
  match body:
    case False{}: U.Mailv.sync(U.Act{U.Ui.info_set(u, "pane:mr", Nat.show(M.Switch.num(value, 0n))), Nil{}})
    case True{}: U.Mailv.sync(U.Act{U.Ui.scratch_set(u, "@mr.mobile.anchor", value), Nil{}})

# Send the tap's exact payload without replacing a newer retained draft.
def One.mail.send.restore(a: U.Act, draft: String, recipient: String) -> U.Act:
  match a:
    case U.Act{u, cs}: U.Act{U.Ui.scratch_set(U.Ui.scratch_set(u, "@f.mrreply", draft), "@mr.to", recipient), cs}

def One.mail.send(+u: U.Ui, +j: J.Json) -> U.Act:
  # Capture the tap without overwriting a newer retained edit.
  +draft = U.Ui.scratch(u, "@f.mrreply")
  +recipient = U.Ui.scratch(u, "@mr.to")
  One.mail.send.restore(U.Mailv.sync(MA.MA.send(U.Ui.scratch_set(U.Ui.scratch_set(u, "@mr.to", J.Json.get_str(j, "recipient", "")), "@f.mrreply", J.Json.get_str(j, "draft", "")))), draft, recipient)

def One.act.mail(i: Nat, +u: U.Ui, +action: String, +value: String) -> U.Act:
  match i:
    case 0n: One.mail.window(False{}, u, value)
    case 1n: One.mail.window(True{}, u, value)
    case 2n: One.mail.send(u, Maybe.default(&2, J.Json, J.Json.parse(value), J.Null{}))
    case _: One.act.k(C.Codec.pos(["view", "view-mention", "view-pick", "view-unpick", "picker-open", "picker-type", "picker-mkdir", "find-open", "find-q", "find-close", "form-close", "view-sheet", "view-layer", "view-parts", "mech-3d", "mech-renders", "fm", "fm-cd", "fm-up", "fm-open", "fm-page", "view-layer-pick", "view-layer-mode", "hub-settings"], action), u, action, value)

def One.act(+u: U.Ui, +action: String, +value: String) -> U.Act:
  U.Rc.sync.of(One.act.mail(C.Codec.pos(["mail-window", "mail-body-window", "mailroom-send"], action), u, action, value))

# what a step does to one hub's client
type Op is Data:
  Do{action: String, value: String}
  Got{o: J.Json}
  On{b: Bool}
  Tick{now: Nat}
  Reg{platform: String, token: String, kind: String, thread: String, env: String, bundle: String}

type Did is Data:
  Did{act: U.Act, alerts: List<&2, J.Json>}

def One.got.k(+seen: List<&2, N.Seen>, a: U.Act) -> Did:
  match a:
    case U.Act{+u, cmds}:
      Did{U.Act{u, cmds}, N.Note.alerts.st.loc(U.Ui.threads(S.Mob.st(u)), seen, S.Mob.st(u), U.Ui.info(u, "desk"), U.Lang.loc(u))}

def Did.alerts(d: Did) -> List<&2, J.Json>:
  match d:
    case Did{_, als}:
      als

def One.got.seen(a: U.Act) -> U.Act:
  match a:
    case U.Act{u, cmds}:
      U.Act{PK.MPick.seen(S.Mob.imgs.seen(MA.MA.reconcile(u))), cmds}

# a frame from the hub, and the alerts it raised
# (streamed text ends no turn: it raises none, and nothing is compared;
# every other message, far ones included, is: law alerts_none_for_delta)
def One.got.b2(booting: Bool, +seen: List<&2, N.Seen>, a: U.Act) -> Did:
  match booting:
    case True{}:
      Did{a, Nil{}}
    case False{}:
      One.got.k(seen, a)

# a frame of a log still arriving in chunks (or its last) is history: it
# raises no alert; the next frame compares against all of it (law
# boot_no_alerts)
def One.got.b(booting: Bool, +seen: List<&2, N.Seen>, +a: U.Act) -> Did:
  match booting:
    case True{}:
      Did{a, Nil{}}
    case False{}:
      One.got.b2(U.Boot.on(U.Act.ui(a)), seen, a)

# the threads a frame of log changes touches (each item's "thread", and
# "id" for a thread made)
def One.ids.one(+x: String, +acc: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(x), acc, x <> acc)

def One.ids.k(+t: String, +i: String, +acc: List<&2, String>) -> List<&2, String>:
  One.ids.one(i, One.ids.one(t, acc))

def One.ids(xs: List<&2, J.Json>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case +x <> rest:
      One.ids(rest, One.ids.k(J.Json.get_str(x, "thread", ""), J.Json.get_str(x, "id", ""), acc))

# what was seen before a frame: for log changes only the threads they
# touch (the phone compared every thread after every frame); for anything
# else (a log, a far batch) every thread
def One.seen.k(changes: Bool, +u: U.Ui, +o: J.Json) -> List<&2, N.Seen>:
  match changes:
    case True{}:
      N.Note.seen.ids(One.ids(J.Json.elems(J.Json.get_obj(o, "items")), Nil{}), S.Mob.st(u))
    case False{}:
      N.Note.seen(U.Ui.threads(S.Mob.st(u)))

def One.seen(+u: U.Ui, +o: J.Json) -> List<&2, N.Seen>:
  One.seen.k(String.eq(J.Json.get_str(o, "t", ""), "changes"), u, o)

def One.got.d(delta: Bool, +u: U.Ui, +o: J.Json) -> Did:
  match delta:
    case True{}:
      Did{One.got.seen(VW.View.far(o, U.Ui.recv_act(MA.MA.recv(VW.View.sheets(VW.View.picked(u, o), o), o), o))), Nil{}}
    case False{}:
      One.got.b(U.Boot.on(u), One.seen(u, o), One.got.seen(VW.View.far(o, U.Ui.recv_act(MA.MA.recv(VW.View.sheets(VW.View.picked(u, o), o), o), o))))

# the same comparing every thread (the spec One.got.d keeps to: law
# alerts_ids_same)
def One.got.all(+u: U.Ui, +o: J.Json) -> Did:
  One.got.b(U.Boot.on(u), N.Note.seen(U.Ui.threads(S.Mob.st(u))), One.got.seen(VW.View.far(o, U.Ui.recv_act(MA.MA.recv(VW.View.sheets(VW.View.picked(u, o), o), o), o))))

def One.got(+u: U.Ui, +o: J.Json) -> Did:
  One.got.d(String.eq(J.Json.get_str(o, "t", ""), "delta"), u, o)

# The same by the policy: the alerts a frame raised are the alert records
# it changed (core/notice.bend; N.Note.ralerts)
def One.pol.k(+seen: List<&2, N.RSeen>, a: U.Act) -> Did:
  match a:
    case U.Act{+u, cmds}:
      Did{U.Act{u, cmds}, N.Note.ralerts.loc(U.Ui.threads(S.Mob.st(u)), seen, S.Mob.st(u), U.Ui.info(u, "desk"), U.Lang.loc(u))}

def One.pol.b(booting: Bool, +seen: List<&2, N.RSeen>, +a: U.Act) -> Did:
  match booting:
    case True{}:
      Did{a, Nil{}}
    case False{}:
      One.pol.k(seen, a)

def One.pol.ids.one(+x: String, +acc: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(x), acc, x <> acc)

# (a record's setting names its thread: alert.<id>)
def One.pol.key(+k: String) -> String:
  Bool.pick(String, String.starts_with(k, "alert."), String.drop(k, 6n), "")

def One.pol.ids.k(+t: String, +i: String, +k: String, +acc: List<&2, String>) -> List<&2, String>:
  One.pol.ids.one(One.pol.key(k), One.pol.ids.one(i, One.pol.ids.one(t, acc)))

def One.pol.ids(xs: List<&2, J.Json>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case +x <> rest:
      One.pol.ids(rest, One.pol.ids.k(J.Json.get_str(x, "thread", ""), J.Json.get_str(x, "id", ""), J.Json.get_str(x, "key", ""), acc))

def One.pol.seen.k(changes: Bool, +u: U.Ui, +o: J.Json) -> List<&2, N.RSeen>:
  match changes:
    case True{}:
      N.Note.rseen.ids(One.pol.ids(J.Json.elems(J.Json.get_obj(o, "items")), Nil{}), S.Mob.st(u))
    case False{}:
      N.Note.rseen(U.Ui.threads(S.Mob.st(u)), S.Mob.st(u))

def One.pol.d(delta: Bool, +u: U.Ui, +o: J.Json) -> Did:
  match delta:
    case True{}:
      Did{One.got.seen(VW.View.far(o, U.Ui.recv_act(MA.MA.recv(VW.View.sheets(VW.View.picked(u, o), o), o), o))), Nil{}}
    case False{}:
      One.pol.b(U.Boot.on(u), One.pol.seen.k(String.eq(J.Json.get_str(o, "t", ""), "changes"), u, o), One.got.seen(VW.View.far(o, U.Ui.recv_act(MA.MA.recv(VW.View.sheets(VW.View.picked(u, o), o), o), o))))

# the same comparing every thread (law policy_phone_ids_same)
def One.pol.all(+u: U.Ui, +o: J.Json) -> Did:
  One.pol.b(U.Boot.on(u), N.Note.rseen(U.Ui.threads(S.Mob.st(u)), S.Mob.st(u)), One.got.seen(VW.View.far(o, U.Ui.recv_act(MA.MA.recv(VW.View.sheets(VW.View.picked(u, o), o), o), o))))

# frames that fold nothing into a thread: streamed text, a heartbeat, a
# streamed reply's end. They raise no alert and nothing is compared for
# them (a heartbeat compared every thread: 24 ms on a phone, every few
# seconds; laws alerts_none_for_hb, alerts_none_for_delta_end)
def One.pol.still(+t: String) -> Bool:
  Bool.or(String.eq(t, "delta"), Bool.or(String.eq(t, "hb"), String.eq(t, "delta_end")))

def One.pol(+u: U.Ui, +o: J.Json) -> Did:
  One.pol.d(One.pol.still(J.Json.get_str(o, "t", "")), u, o)

def One.run(op: Op, +u: U.Ui) -> Did:
  match op:
    case Do{+a, +v}:
      Did{One.act(u, a, v), Nil{}}
    case Got{+o}:
      One.pol(u, o)
    case On{+b}:
      Did{VW.View.online(MA.MA.online(S.Mob.heard.set(u), b), b), Nil{}}
    case Tick{now}:
      Did{U.Act{S.Mob.heard.set(U.Ui.tick(u, now)), Nil{}}, Nil{}}
    case Reg{p, t, k, th, e, b}:
      Did{U.Ui.rpc.keep(u, "device.register", [J.KV{"platform", J.Json.str(p)}, J.KV{"token", J.Json.str(t)},
        J.KV{"kind", J.Json.str(k)}, J.KV{"thread", J.Json.str(th)}, J.KV{"env", J.Json.str(e)}, J.KV{"bundle", J.Json.str(b)}]), Nil{}}

# Steps over the hubs
# -------------------

type Step is Data:
  Step{hs: List<&2, Hub>, cmds: List<&2, HCmd>, alerts: List<&2, J.Json>}

def Step.hs(s: Step) -> List<&2, Hub>:
  match s:
    case Step{hs, _, _}:
      hs

def Hubs.tag(cs: List<&2, U.Cmd>, +k: String) -> List<&2, HCmd>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      HCmd{k, c} <> Hubs.tag(t, k)

def Hubs.qual(xs: List<&2, J.Json>, +k: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Q.all(x, k) <> Hubs.qual(t, k)

def Hubs.put(d: Did, +k: String, rest: Step) -> Step:
  match d rest:
    case Did{U.Act{u, cs}, al} Step{hs, cmds, als}:
      Step{Hub{k, u} <> hs, List.append(&2, HCmd, Hubs.tag(cs, k), cmds), List.append(&2, J.Json, Hubs.qual(al, k), als)}

def Hubs.keep(h: Hub, rest: Step) -> Step:
  match rest:
    case Step{hs, cmds, als}:
      Step{h <> hs, cmds, als}

def Hubs.one(h: Hub, +op: Op, rest: Step) -> Step:
  match h:
    case Hub{+k, +u}:
      Hubs.put(One.run(op, u), k, rest)

def Hubs.pick(hit: Bool, h: Hub, +op: Op, rest: Step) -> Step:
  match hit:
    case True{}:
      Hubs.one(h, op, rest)
    case False{}:
      Hubs.keep(h, rest)

# op on hub `key`, or on every hub
def Hubs.run(hs: List<&2, Hub>, +all: Bool, +key: String, +op: Op) -> Step:
  match hs:
    case Nil{}:
      Step{Nil{}, Nil{}, Nil{}}
    case +h <> t:
      Hubs.pick(Bool.or(all, String.eq(Hub.key(h), key)), h, op, Hubs.run(t, all, key, op))

def Hubs.done(+cid: String, +now: Nat, +cur: String, s: Step) -> HAct:
  match s:
    case Step{hs, cmds, als}:
      HAct{Hubs{cid, now, cur, hs}, cmds, als}

def Hubs.step(hs: Hubs, +all: Bool, +key: String, +op: Op) -> HAct:
  match hs:
    case Hubs{+cid, +now, +cur, list}:
      Hubs.done(cid, now, cur, Hubs.run(list, all, key, op))

def Hubs.has(hs: List<&2, Hub>, +key: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(Hub.key(h), key), Hubs.has(t, key))

# The bridge's calls
# ------------------

def Hubs.find(hs: List<&2, Hub>, +key: String) -> Maybe<&2, Hub>:
  match hs:
    case Nil{}:
      None{}
    case +h <> t:
      Bool.pick(Maybe<&2, Hub>, String.eq(Hub.key(h), key), Some{h}, Hubs.find(t, key))

def Hubs.lang.inherit(old: List<&2, Hub>, u: U.Ui) -> U.Ui:
  match old:
    case Nil{}: u
    case Hub{_, +from} <> _:
      U.Lang.apply(U.Ui.scratch_set(u, "@lang.preview", U.Ui.scratch(from, "@lang.preview")), U.Lang.choice(from), U.Ui.scratch(from, "@lang.os"))

def Hubs.made(+m: Maybe<&2, Hub>, +k: String, +cid: String, +now: Nat, +old: List<&2, Hub>) -> Hub:
  match m:
    case Some{h}:
      h
    case None{}:
      Hub{k, Hubs.lang.inherit(old, U.Ui.info_set(U.Ui.set_cid(U.Ui.init(now), cid), "local.fixed", "1"))}

def Hubs.set.go(ks: List<&2, String>, +old: List<&2, Hub>, +cid: String, +now: Nat) -> List<&2, Hub>:
  match ks:
    case Nil{}:
      Nil{}
    case +k <> t:
      Hubs.made(Hubs.find(old, k), k, cid, now, old) <> Hubs.set.go(t, old, cid, now)

def Hubs.first(hs: List<&2, Hub>) -> String:
  match hs:
    case Nil{}:
      ""
    case h <> _:
      Hub.key(h)

def Hubs.set.k(+cid: String, +now: Nat, +cur: String, +hs: List<&2, Hub>) -> Hubs:
  Hubs{cid, now, Bool.pick(String, Hubs.has(hs, cur), cur, Hubs.first(hs)), hs}

# the hubs the phone is paired with, in its order: a new one gets a new
# client, one no longer paired is dropped with all it held, and the focus
# stays where it was while that hub stays
def Hubs.set(hs: Hubs, ks: List<&2, String>) -> Hubs:
  match hs:
    case Hubs{+cid, +now, +cur, +list}:
      Hubs.set.k(cid, now, cur, Hubs.set.go(ks, list, cid, now))

# a hub's client (a fresh one for a key it does not hold)
def Hubs.ui(hs: Hubs, +key: String) -> U.Ui:
  match hs:
    case Hubs{+cid, +now, _, +list}:
      Hub.ui(Hubs.made(Hubs.find(list, key), key, cid, now, list))

def Hubs.recv(hs: Hubs, +key: String, o: J.Json) -> HAct:
  Hubs.step(hs, False{}, key, Got{o})

def Hubs.online(hs: Hubs, +key: String, b: Bool) -> HAct:
  Hubs.step(hs, False{}, key, On{b})

# what a phone shows over its screens (sheets, menus, forms, questions,
# an error) is this device's moment, never its state: the saved copy may
# have been written while one was up (the app goes to the background with
# Find open), and loading it put the sheet back over the app at every
# launch. Each is shut on load: the flags Find, Settings, the pairing QR,
# the key form and the folder picker; the scratch row menu, delete,
# remove and rename questions, the bot, room and routine forms; the error.
def Hubs.calm.flags(fs: List<&2, String>, u: U.Ui) -> U.Ui:
  match fs:
    case Nil{}:
      u
    case f <> t:
      Hubs.calm.flags(t, U.Ui.flag_off(u, f))

def Hubs.calm.keys(ks: List<&2, String>, u: U.Ui) -> U.Ui:
  match ks:
    case Nil{}:
      u
    case k <> t:
      Hubs.calm.keys(t, U.Ui.scratch_set(u, k, ""))

def Hubs.calm.flag_names() -> List<&2, String>:
  ["palette", "settings", "pairqr", "keyform", "proj"]

def Hubs.calm.scratch_names() -> List<&2, String>:
  ["@menu", "@del", "@rm", "@ren", "@bnew", "@rnew", "@onew"]

def Hubs.calm(u: U.Ui) -> U.Ui:
  U.Ui.error_set(Hubs.calm.keys(Hubs.calm.scratch_names(), Hubs.calm.flags(Hubs.calm.flag_names(), u)), "")

def Hubs.stale.go(l: List<&2, Hub>) -> List<&2, Hub>:
  match l:
    case Nil{}:
      Nil{}
    case Hub{+k, +u} <> t:
      Hub{k, U.Ui.unsync(Hubs.calm(u))} <> Hubs.stale.go(t)

# state loaded from the phone's saved copy: no hub has answered yet
def Hubs.stale(hs: Hubs) -> Hubs:
  match hs:
    case Hubs{cid, now, cur, list}:
      Hubs{cid, now, cur, Hubs.stale.go(list)}

def Hubs.now(hs: Hubs, now: Nat) -> Hubs:
  match hs:
    case Hubs{cid, _, cur, list}:
      Hubs{cid, now, cur, list}

# A choice that belongs to this device (`Backplane.local(key, value)`, ios-snappy
# tabs): left in every hub's scratch as @local.<key>, where the screen reads it
# (screen.bend's Mob.view). The apps send it after load/start, after pairing
# and on every change, so a hub added later has it at once.
def Hubs.local.ui(k: Nat, +u: U.Ui, key: String, +value: String) -> U.Ui:
  match k:
    case 0n: U.Lang.apply(u, value, U.Ui.scratch(u, "@lang.os"))
    case 1n: U.Lang.apply(u, U.Lang.choice(u), value)
    case 2n: U.Lang.apply(U.Ui.scratch_set(u, "@lang.preview", value), U.Lang.choice(u), U.Ui.scratch(u, "@lang.os"))
    case 3n: U.Ui.info_set(u, "local.mailroom", Bool.pick(String, Bool.or(String.eq(value, "ios-v1"), String.eq(value, "android-v1")), "on", ""))
    case _: U.Ui.scratch_set(u, "@local." ++ key, value)

def Hubs.local.go(hs: List<&2, Hub>, +key: String, +value: String) -> List<&2, Hub>:
  match hs:
    case Nil{}:
      Nil{}
    case Hub{+k, u} <> t:
      Hub{k, Hubs.local.ui(C.Codec.pos(["lang", "lang-os", "lang-preview", "mailroom-renderer"], key), u, key, value)} <> Hubs.local.go(t, key, value)

def Hubs.local(hs: Hubs, +key: String, +value: String) -> Hubs:
  match hs:
    case Hubs{cid, now, cur, l}:
      Hubs{cid, now, cur, Hubs.local.go(l, key, value)}

def Hubs.tick(hs: Hubs, +now: Nat) -> HAct:
  Hubs.step(Hubs.now(hs, now), True{}, "", Tick{now})

# this phone's push tokens go to every hub it is paired with
def Hubs.register(hs: Hubs, p: String, t: String, k: String, th: String, e: String, b: String) -> HAct:
  Hubs.step(hs, True{}, "", Reg{p, t, k, th, e, b})

# a token for one hub alone: the Live Activity a hub keeps (iOS runs one
# per paired hub, Hubs.islands), so no hub updates or ends another's
def Hubs.register.at(hs: Hubs, +key: String, p: String, t: String, k: String, th: String, e: String, b: String) -> HAct:
  Hubs.step(hs, False{}, key, Reg{p, t, k, th, e, b})

# Actions
# -------

# actions whose value names a row, a project, a bot or room, or (opening
# the project picker or a bot or room form, adding a project) a hub: they
# go to the hub it names; the first thirteen also bring it into focus, so the
# picker's, the bot's and the forms' own actions follow it (the history's
# playback ticks go to the hub that asked: bridge.js names it)
def Hubs.routed(+action: String) -> Nat:
  C.Codec.pos(["select", "new-thread", "picker-open", "bot", "room", "remote", "uber", "uber-bot", "bot-new", "room-new", "proj-go", "hub-settings", "proj-intent",
    "row-settle", "row-unsettle", "row-snooze", "row-delete", "toggle-settled", "add-project", "proj-remove",
    "row-menu", "proj-menu", "row-pin", "row-unpin", "row-archive", "row-unarchive", "hist-tick", "row-rename", "rename-save", "uber-promote", "uber-home", "fab-open", "fab-edit", "buy", "fab-hide", "fab-show", "deploy-cancel", "thread-tree", "thread-child", "thread-detach", "intent-close", "intent-look"], action)

def Hubs.moves() -> Nat:
  13n

def Hubs.routes() -> Nat:
  42n

# answers to what any hub may be showing (its error, a delete or remove
# to confirm); the project search is every hub's at once, since the list
# it filters spans them all ("proj-find", "proj-find-q", unrouted, go to all)
def Hubs.everywhere(+action: String) -> Bool:
  Nat.is_lt(C.Codec.pos(["dismiss", "delete-no", "proj-keep", "proj-find", "proj-find-q", "menu-close", "side-older", "side-settled", "rename-no", "tz-report"], action), 10n)

# The merged phone sidebar has one view preference across paired hubs.
def Hubs.every.op(op: Op) -> Bool:
  match op:
    case Do{+a, +v}:
      Bool.or(Hubs.everywhere(a), Bool.and(String.eq(a, "setting"), String.starts_with(v, "sidebar.view=")))
    case _:
      False{}

# of what the hub let go says, only what it sends (its viewer's watch
# ends); its focus and scroll are for a screen no longer shown
def Hubs.sends(cs: List<&2, HCmd>) -> List<&2, HCmd>:
  match cs:
    case Nil{}:
      Nil{}
    case HCmd{k, U.Send{t}} <> r:
      HCmd{k, U.Send{t}} <> Hubs.sends(r)
    case _ <> r:
      Hubs.sends(r)

def Hubs.then.k(cmds: List<&2, HCmd>, als: List<&2, J.Json>, +cid: String, +now: Nat, +cur: String, s: Step) -> HAct:
  match s:
    case Step{hs2, cmds2, als2}:
      HAct{Hubs{cid, now, cur, hs2}, List.append(&2, HCmd, cmds, cmds2), List.append(&2, J.Json, als, als2)}

def Hubs.then(s: Step, +cid: String, +now: Nat, +cur: String, +key: String, +op: Op) -> HAct:
  match s:
    case Step{hs, cmds, als}:
      Hubs.then.k(Hubs.sends(cmds), als, cid, now, cur, Hubs.run(hs, False{}, key, op))

def Op.action(op: Op) -> String:
  match op:
    case Do{a, _}:
      a
    case _:
      ""

# into another hub's thread: the hub in focus lets go of its own first
# (closing its viewer), then the other takes the action
def Hubs.focus(move: Bool, hs: Hubs, +key: String, +op: Op) -> HAct:
  match move hs:
    case True{} Hubs{+cid, +now, +cur, list}:
      Hubs.then(Hubs.run(list, False{}, cur, Do{"select", ""}), cid, now, key, key, op)
    case False{} Hubs{+cid, +now, +cur, list}:
      Hubs.step(Hubs{cid, now, cur, list}, Hubs.every.op(op), key, op)

# (matches, not Bool.pick: an unrouted value, which may be a big piece of
# an upload, is never split)
def Hubs.act.k(+hs: Hubs, +cur: String, known: Bool, +action: String, +value: String) -> HAct:
  match known:
    case True{}:
      Hubs.focus(Bool.and(Bool.or(Nat.is_lt(Hubs.routed(action), Hubs.moves()), String.eq(action, "thread-child")), Bool.not(String.eq(Hubs.key_of(value), cur))), hs, Hubs.key_of(value), Do{action, Hubs.rest(value)})
    case False{}:
      Hubs.focus(False{}, hs, cur, Do{action, value})

def Hubs.cur(hs: Hubs) -> String:
  match hs:
    case Hubs{_, _, c, _}:
      c

def Hubs.shows.k(focus: Bool, +hs: Hubs, +key: String, +o: J.Json) -> Bool:
  match focus:
    case True{}:
      U.Ui.shows(Hubs.ui(hs, key), o)
    case False{}:
      False{}

# whether a message from hub key can change the screen: a thread's streamed
# text shows only in the thread open on the hub in focus, so any other
# thread's delta (every token of every agent at work, on every hub) builds
# no screen (laws hubs_delta_elsewhere, hubs_delta_other_thread)
def Hubs.shows(+hs: Hubs, +key: String, +o: J.Json) -> Bool:
  Hubs.shows.k(String.eq(key, Hubs.cur(hs)), hs, key, o)

def Hubs.redraw.k(delta: Bool, +hs: Hubs, +key: String, +o: J.Json) -> Bool:
  match delta:
    case True{}:
      Hubs.shows(hs, key, o)
    case False{}:
      True{}

# whether the phone builds a screen for a frame from hub key: any message
# from any paired hub can change the merged sidebar and the island; only
# streamed text is held to Hubs.shows (laws hubs_redraw_*)
def Hubs.redraw.t(beat: Bool, +hs: Hubs, +key: String, +o: J.Json) -> Bool:
  match beat:
    case True{}:
      False{}
    case False{}:
      Hubs.redraw.k(String.eq(J.Json.get_str(o, "t", ""), "delta"), hs, key, o)

# (a heartbeat builds none: law hubs_redraw_hb; nor the end of a streamed
# reply, which every client ignores: law hubs_redraw_delta_end)
def Hubs.redraw.still(+t: String) -> Bool:
  Bool.or(String.eq(t, "hb"), String.eq(t, "delta_end"))

def Hubs.redraw(+hs: Hubs, +key: String, +o: J.Json) -> Bool:
  Hubs.redraw.t(Hubs.redraw.still(J.Json.get_str(o, "t", "")), hs, key, o)

def Hubs.list(hs: Hubs) -> List<&2, Hub>:
  match hs:
    case Hubs{_, _, _, l}:
      l

# A thread named with no hub (a notification's tap: the hub that pushed it
# named it in its own ids, maybe a mirror's `<link>~<id>`) belongs to the
# hub that holds it: the focused one when it does, else the first that does.
def Hub.holds(h: Hub, +id: String) -> Bool:
  match h:
    case Hub{_, +u}:
      FR.Fr.has(Map.has(&2, M.Thread, FR.Fr.st.threads(U.Ui.state(u)), id))

def Hubs.holder(hs: List<&2, Hub>, +id: String) -> String:
  match hs:
    case Nil{}:
      ""
    case +h <> t:
      Bool.pick(String, Hub.holds(h, id), Hub.key(h), Hubs.holder(t, id))

def Hubs.held.k(+hs: Hubs, +cur: String, +other: String, +action: String, +value: String) -> HAct:
  Bool.pick(HAct, String.is_empty(other), Hubs.act.k(hs, cur, False{}, action, value), Hubs.act.k(hs, cur, True{}, action, other ++ "|" ++ value))

def Hubs.held(here: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match here:
    case True{}:
      Hubs.act.k(hs, Hubs.cur(hs), False{}, action, value)
    case False{}:
      Hubs.held.k(hs, Hubs.cur(hs), Hubs.holder(Hubs.list(hs), value), action, value)

def Hubs.act.r.k(known: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match known:
    case True{}:
      Hubs.act.k(hs, Hubs.cur(hs), True{}, action, value)
    case False{}:
      Hubs.held(Bool.or(Bool.not(String.eq(action, "select")), Hub.holds(Hub{Hubs.cur(hs), Hubs.ui(hs, Hubs.cur(hs))}, value)), hs, action, value)

def Hubs.act.r(routed: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match routed:
    case True{}:
      Hubs.act.r.k(Hubs.has(Hubs.list(hs), Hubs.key_of(value)), hs, action, value)
    case False{}:
      Hubs.act.k(hs, Hubs.cur(hs), False{}, action, value)

# Usage controls are local to the named machine. An obsolete qualified
# machine key must never fall through and change the focused machine.
def Hubs.usage.action(+action: String) -> Bool:
  Nat.is_lt(C.Codec.pos(["usage-period", "usage-metric", "usage-model-page", "usage-project-page", "usage-provider"], action), 5n)

def Hubs.usage.target(known: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match known:
    case True{}: Hubs.act.k(hs, Hubs.cur(hs), True{}, action, value)
    case False{}: HAct{hs, Nil{}, Nil{}}

def Hubs.usage.qualified(qualified: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match qualified:
    case True{}: Hubs.usage.target(Hubs.has(Hubs.list(hs), Hubs.key_of(value)), hs, action, value)
    case False{}: Hubs.act.k(hs, Hubs.cur(hs), False{}, action, value)

def Hubs.usage.dispatch(usage: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match usage:
    case True{}: Hubs.usage.qualified(String.contains(value, "|"), hs, action, value)
    case False{}: Hubs.act.r(Bool.or(String.eq(action, "usage-refresh"), Nat.is_lt(Hubs.routed(action), Hubs.routes())), hs, action, value)

def Hubs.act.one(+hs: Hubs, +action: String, +value: String) -> HAct:
  Hubs.usage.dispatch(Hubs.usage.action(action), hs, action, value)

def Hubs.act.then(a: HAct, b: HAct) -> HAct:
  match a b:
    case HAct{_, cs, als} HAct{h, cs2, als2}:
      HAct{h, List.append(&2, HCmd, cs, cs2), List.append(&2, J.Json, als, als2)}

def Hubs.act.go.after(+a: HAct, +action: String, +value: String) -> HAct:
  Hubs.act.then(a, Hubs.act.one(Hubs.after(a), action, value))

def Hubs.act.go.k(go: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match go:
    case True{}:
      Hubs.act.go.after(Hubs.act.one(hs, "proj-find", "off"), action, value)
    case False{}:
      Hubs.act.one(hs, action, value)

# Internal links carry their containing message's hub separately from the URI.
# Never remove a remote link~id prefix or reinterpret a URI as a pairing URL.
def RefOpen.found(m: Maybe<&2, RE.Card>) -> Bool:
  match m:
    case Some{_}: True{}
    case None{}: False{}

def RefOpen.resolves.kind(+u: U.Ui, +r: R.Ref) -> Bool:
  match r:
    case R.Ref{k, _, _}:
      Bool.and(R.Ref.has(["thread", "bot", "room", "project", "machine", "routine", "file", "work", "task", "pr", "mailroom"], k), RefOpen.found(RE.Card.of(U.Ui.state(u), r)))

def RefOpen.resolves(+u: U.Ui, r: R.Ref) -> Bool:
  RefOpen.resolves.kind(u, r)

def RefOpen.first(+found: String, key: String, resolves: Bool) -> String:
  match found resolves:
    case SNil{} True{}: key
    case _ _: found

def RefOpen.owner(hs: List<&2, Hub>, +r: R.Ref, found: String) -> String:
  match hs:
    case Nil{}: found
    case Hub{+key, +u} <> rest:
      RefOpen.owner(rest, r, RefOpen.first(found, key, RefOpen.resolves(u, r)))

def RefOpen.go.known(known: Bool, +hs: Hubs, +key: String, +r: R.Ref) -> HAct:
  match known:
    case False{}: HAct{hs, Nil{}, Nil{}}
    case True{}:
      Hubs.focus(Bool.not(String.eq(Hubs.cur(hs), key)), hs, key, Do{"ref-go", R.Ref.uri(r)})

def RefOpen.go(+hs: Hubs, +key: String, +r: R.Ref) -> HAct:
  RefOpen.go.known(Bool.and(Hubs.has(Hubs.list(hs), key), RefOpen.resolves(Hubs.ui(hs, key), r)), hs, key, r)

def RefOpen.external(here: Bool, +hs: Hubs, +r: R.Ref) -> HAct:
  match here:
    case True{}: RefOpen.go(hs, Hubs.cur(hs), r)
    case False{}: RefOpen.go(hs, RefOpen.owner(Hubs.list(hs), r, ""), r)

def RefOpen.context(+hs: Hubs, +key: String, +r: R.Ref) -> HAct:
  match key:
    case SNil{}: RefOpen.external(RefOpen.resolves(Hubs.ui(hs, Hubs.cur(hs)), r), hs, r)
    case _: RefOpen.go(hs, key, r)

def RefOpen.parsed(m: Maybe<&2, R.Ref>, hs: Hubs, key: String) -> HAct:
  match m:
    case None{}: HAct{hs, Nil{}, Nil{}}
    case Some{r}: RefOpen.context(hs, key, r)

def RefOpen.json(+hs: Hubs, +j: J.Json) -> HAct:
  RefOpen.parsed(R.Ref.parse(J.Json.get_str(j, "uri", "")), hs, J.Json.get_str(j, "hub", ""))

def RefOpen.act(+hs: Hubs, value: String) -> HAct:
  RefOpen.json(hs, Maybe.default(&2, J.Json, J.Json.parse(value), J.Null{}))

# going to a found project shuts the search on every hub, then goes
def Hubs.act.ref(internal: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match internal:
    case True{}: RefOpen.act(hs, value)
    case False{}: Hubs.act.go.k(String.eq(action, "proj-go"), hs, action, value)

# This envelope carries the hub separately from all opaque protocol values.
# In particular a conversation ID may itself contain the route separator.
def Hubs.mail.allowed(+action: String, +value: String) -> Bool:
  Bool.or(Nat.is_lt(C.Codec.pos(["mail-open", "mail-more", "mail-close", "mail-search", "mail-filter", "mail-to", "mail-send", "mail-mark", "mail-window", "mail-body-window"], action), 10n),
    Bool.and(String.eq(action, "bfield"), String.starts_with(value, "mrreply" ++ SCon{Chr{31}, SNil{}})))

def Hubs.mail.apply(valid: Bool, +hs: Hubs, +key: String, action: String, value: String) -> HAct:
  match valid:
    case False{}: HAct{hs, Nil{}, Nil{}}
    case True{}: Hubs.step(hs, False{}, key, Do{action, value})

def Hubs.mail.action(send: Bool, j: J.Json) -> String:
  match send:
    case True{}: "mailroom-send"
    case False{}: J.Json.get_str(j, "action", "")

def Hubs.mail.value(send: Bool, +j: J.Json) -> String:
  match send:
    case True{}: J.Json.show(J.Json.obj([J.KV{"recipient", J.Str{J.Json.get_str(j, "recipient", "")}}, J.KV{"draft", J.Str{J.Json.get_str(j, "draft", "")}}]))
    case False{}: J.Json.get_str(j, "value", "")

def Hubs.mail.context(+hs: Hubs, +j: J.Json, +key: String, +u: U.Ui) -> HAct:
  Hubs.mail.apply(Bool.and(U.Mailv.shown(u), Bool.and(
    String.eq(U.Mailv.owner(u), J.Json.get_str(j, "owner", "")),
    String.eq(U.Ui.scratch(u, "@mr.conv"), J.Json.get_str(j, "conversation", "")))),
    hs, key, Hubs.mail.action(String.eq(J.Json.get_str(j, "action", ""), "mail-send"), j),
    Hubs.mail.value(String.eq(J.Json.get_str(j, "action", ""), "mail-send"), j))

def Hubs.mail.known(known: Bool, +hs: Hubs, +j: J.Json, +key: String) -> HAct:
  match known:
    case False{}: HAct{hs, Nil{}, Nil{}}
    case True{}: Hubs.mail.context(hs, j, key, Hubs.ui(hs, key))

def Hubs.mail.request(+hs: Hubs, +j: J.Json) -> HAct:
  Hubs.mail.known(Bool.and(Hubs.has(Hubs.list(hs), J.Json.get_str(j, "hub", "")),
    Hubs.mail.allowed(J.Json.get_str(j, "action", ""), J.Json.get_str(j, "value", ""))), hs, j, J.Json.get_str(j, "hub", ""))

def Hubs.act.mail(mail: Bool, +hs: Hubs, +action: String, +value: String) -> HAct:
  match mail:
    case True{}: Hubs.mail.request(hs, Maybe.default(&2, J.Json, J.Json.parse(value), J.Null{}))
    case False{}: Hubs.act.ref(String.eq(action, "ref-open"), hs, action, value)

def Hubs.act(+hs: Hubs, +action: String, +value: String) -> HAct:
  Hubs.act.mail(String.eq(action, "mailroom-action"), hs, action, value)

def Hubs.self(+u: U.Ui) -> String:
  J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(U.Ui.info(u, "machines")), J.Null{}), "self"), "name", "")

# The screen
# ----------

# what a hub calls itself, else its host
def Hubs.name(+key: String, +u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(Hubs.self(u)), Maybe.default(&2, String, List.head(&2, String, String.split(key, ':')), key), Hubs.self(u))

# The status plane (plane.bend) is built for every hub's screen while any
# paired hub has an uberbot, so a hub without one still adds its rows
def Hubs.pl.has.k(m: Maybe<&2, BU.BRow>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def Hubs.pl.has(+u: U.Ui) -> Bool:
  Hubs.pl.has.k(BU.BU.uber(u))

def Hubs.pl.any(hs: List<&2, Hub>) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Hub{_, +u} <> t:
      Bool.pick(Bool, Hubs.pl.has(u), True{}, Hubs.pl.any(t))

# a plane row made the hub's own: its id and its value carry the hub (the
# plane is left out of Q.all, which walked every row's every key)
def Hubs.pl.q.fields(fs: J.Json, +from: String) -> J.Json:
  match fs:
    case J.Field{+k, J.Str{+s}, t}:
      J.Field{k, J.Str{Bool.pick(String, String.eq(k, "nav"), Hubs.nav.qual(from, s), Bool.pick(String, Bool.or(String.eq(k, "id"), String.eq(k, "value")), Hubs.id(from, s), s))}, Hubs.pl.q.fields(t, from)}
    case J.Field{k, v, t}:
      J.Field{k, v, Hubs.pl.q.fields(t, from)}
    case x:
      x

def Hubs.pl.q(r: J.Json, +from: String) -> J.Json:
  match r:
    case J.Obj{fs}:
      J.Obj{Hubs.pl.q.fields(fs, from)}
    case x:
      x

def Hubs.pl.q.rows(rs: List<&2, J.Json>, +from: String) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Hubs.pl.q(r, from) <> Hubs.pl.q.rows(t, from)

# a field of the hub's screen given a new value
def Hubs.pl.put.k(hit: Bool, +k: String, v: J.Json, t: J.Json, plane: J.Json, rest: J.Json) -> J.Json:
  match hit:
    case True{}:
      J.Field{k, plane, t}
    case False{}:
      J.Field{k, v, rest}

def Hubs.pl.put(fs: J.Json, +key: String, +plane: J.Json) -> J.Json:
  match fs:
    case J.Field{+k, v, +t}:
      Hubs.pl.put.k(String.eq(k, key), k, v, t, plane, Hubs.pl.put(t, key, plane))
    case x:
      x

def Hubs.pl.null(j: J.Json) -> Bool:
  match j:
    case J.Null{}:
      True{}
    case _:
      False{}

# the uberbot's card with the counts of the plane, when this hub has one
def Hubs.pl.card.k(+lc: I.Loc, m: Maybe<&2, BU.BRow>, +p: PL.Plane, +k: String) -> J.Json:
  match m:
    case Some{r}:
      Hubs.pl.q(MPL.MPlane.card(lc, r, p), k)
    case None{}:
      J.Null{}

def Hubs.pl.fill.p(+lc: I.Loc, +p: PL.Plane, +fs: J.Json, +u: U.Ui, +k: String) -> J.Json:
  J.Obj{Hubs.pl.put(Hubs.pl.put(fs, "plane", MPL.MPlane.with.home(MPL.MPlane.json.loc(lc, p, S.Mob.now(u), U.Ui.state(u)), Hubs.pl.q(MPL.MPlane.home(lc, u), k))), "uber", Hubs.pl.card.k(lc, BU.BU.uber(u), p, k))}

# the card alone (no counts: they come from the plane)
def Hubs.pl.card.n(+lc: I.Loc, m: Maybe<&2, BU.BRow>, +k: String) -> J.Json:
  match m:
    case Some{r}:
      Hubs.pl.q(MPL.MPlane.bare(lc, r), k)
    case None{}:
      J.Null{}

def Hubs.pl.fill.n(+lc: I.Loc, +fs: J.Json, +u: U.Ui, +k: String) -> J.Json:
  J.Obj{Hubs.pl.put(fs, "uber", Hubs.pl.card.n(lc, BU.BU.uber(u), k))}

# The list of a phone with one hub draws the plane without its idle threads,
# made from the index and rows the screen has made (core's Plane.of.l); the
# Status tab and several hubs get the whole plane.
def Hubs.pl.fill.l(+lc: I.Loc, lite: Bool, +fs: J.Json, +u: U.Ui, +k: String, +idle: Bool, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  match lite:
    case True{}:
      Hubs.pl.fill.p(lc, PL.Plane.signals.of(u, S.Mob.now(u), False{}), fs, u, k)
    case False{}:
      Hubs.pl.fill.p(lc, PL.Plane.signals.of(u, S.Mob.now(u), idle), fs, u, k)

def Hubs.pl.fill.w(+lc: I.Loc, want: Bool, +fs: J.Json, +u: U.Ui, +k: String, +idle: Bool, +lite: Bool, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  match want:
    case True{}:
      Hubs.pl.fill.l(lc, lite, fs, u, k, idle, ix, rows)
    case False{}:
      Hubs.pl.fill.n(lc, fs, u, k)

def Hubs.pl.fill.k(+lc: I.Loc, any: Bool, +want: Bool, +j: J.Json, +u: U.Ui, +k: String, +idle: Bool, +lite: Bool, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  match any j:
    case True{} J.Obj{+fs}:
      Hubs.pl.fill.w(lc, want, fs, u, k, idle, lite, ix, rows)
    case _ x:
      x

def Hubs.pl.fill(+lc: I.Loc, +any: Bool, +want: Bool, +j: J.Json, +u: U.Ui, +k: String, +idle: Bool, +lite: Bool, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  Hubs.pl.fill.k(lc, any, want, j, u, k, idle, lite, ix, rows)

# whether the plane is on screen: the focused hub shows its list (nothing
# selected) or the uberbot's Status tab. Every other screen (a thread, a room,
# another bot's tab) gets the card but no plane, which costs the screen nothing.
def Hubs.pl.want.k(v: BU.View, +u: U.Ui) -> Bool:
  match v:
    case BU.VThread{}:
      String.is_empty(U.Ui.sel(u))
    case BU.VBot{+b}:
      String.eq(BU.BU.tab.of(u, BT.Bot.id(b)), "status")
    case _:
      False{}

def Hubs.pl.want(+hs: List<&2, Hub>, +cur: String) -> Bool:
  match hs:
    case Nil{}:
      True{}
    case Hub{+k, +u} <> t:
      Bool.pick(Bool, String.eq(k, cur), Hubs.pl.want.k(BU.BU.view(u), u), Hubs.pl.want(t, cur))

# whether the focused hub shows the uberbot's Status tab (whose idle threads
# every hub then builds)
def Hubs.pl.status.k(v: BU.View, +u: U.Ui) -> Bool:
  match v:
    case BU.VBot{+b}:
      String.eq(BU.BU.tab.of(u, BT.Bot.id(b)), "status")
    case _:
      False{}

def Hubs.pl.status(+hs: List<&2, Hub>, +cur: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Hub{+k, +u} <> t:
      Bool.pick(Bool, String.eq(k, cur), Hubs.pl.status.k(BU.BU.view(u), u), Hubs.pl.status(t, cur))

# the hub's tabs (tabs.bend) as a field of its screen
def Hubs.tabs.put(j: J.Json, t: J.Json) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"tabs", t, fs}}
    case x:
      x

type Shown is Data:
  Shown{key: String, name: String, j: J.Json}

# (a hub out of focus builds no thread, bot or viewer: the merged screen
# reads those from the hub in focus only)
# (the index of the state and the sidebar's rows are made once and shared by
# the screen and the plane)
# Usage is requested by the device only while its usage page is visible.
# No per-thread usage scan is added to chat/streaming screens.
def Hubs.usage.rows(+lc: I.Loc, on: Bool, +u: U.Ui) -> List<&2, J.Json>:
  match on:
    case True{}:
      S.Mob.usage.page(lc, u)
    case False{}:
      Nil{}

def Hubs.usage.put(j: J.Json, rows: List<&2, J.Json>) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{J.Field{"machineUsage", J.Json.arr(rows), fs}}
    case x:
      x

# Bot JSON is opaque to Q.all. Add the owner only to the nested native
# Mailroom projection, leaving every other field and raw protocol ID alone.
def Hubs.mail.online(u: U.Ui) -> Bool:
  match u:
    case U.Ui{_, _, _, _, _, on, _, _, _, _, _, _, _, _, _, _, _}: on

def Hubs.mail.owner.attach(found: Bool, +key: String, +machine: String, +online: Bool, j: J.Json) -> J.Json:
  match found j:
    case True{} J.Obj{fields}: J.Obj{J.Field{"hub", J.Str{key}, J.Field{"machine", J.Str{machine}, J.Field{"online", J.Flag{online}, fields}}}}
    case _ x: x

def Hubs.mail.owner.bot.fields(+key: String, +machine: String, +online: Bool, fs: J.Json) -> J.Json:
  match fs:
    case J.Field{+name, value, tail}:
      J.Field{name, Hubs.mail.owner.attach(String.eq(name, "mailroom"), key, machine, online, value), Hubs.mail.owner.bot.fields(key, machine, online, tail)}
    case x: x

def Hubs.mail.owner.bot(found: Bool, +key: String, +machine: String, +online: Bool, j: J.Json) -> J.Json:
  match found j:
    case True{} J.Obj{fields}: J.Obj{Hubs.mail.owner.bot.fields(key, machine, online, fields)}
    case _ x: x

def Hubs.mail.owner.fields(+key: String, +machine: String, +online: Bool, fs: J.Json) -> J.Json:
  match fs:
    case J.Field{+name, value, tail}:
      J.Field{name, Hubs.mail.owner.bot(String.eq(name, "bot"), key, machine, online, value), Hubs.mail.owner.fields(key, machine, online, tail)}
    case x: x

def Hubs.mail.owner(+key: String, +machine: String, +online: Bool, j: J.Json) -> J.Json:
  match j:
    case J.Obj{fields}: J.Obj{Hubs.mail.owner.fields(key, machine, online, fields)}
    case x: x

def Hubs.shown.r(+lc: I.Loc, +k: String, +u: U.Ui, +cur: String, +any: Bool, +want: Bool, +idle: Bool, +lite: Bool, +ix: U.SIx, +rows: List<&2, U.SRow>) -> J.Json:
  Hubs.usage.put(Hubs.tabs.put(Hubs.pl.fill(lc, any, want, Hubs.mail.owner(k, Hubs.name(k, u), Hubs.mail.online(u), Q.all(S.Mob.screen.ixr(lc, String.eq(k, cur), u, ix, rows), k)), u, k, idle, lite, ix, rows), MT.MTab.json(lc, u, ix, rows)),
    Hubs.usage.rows(lc, String.eq(U.Ui.scratch(u, "@local.usage"), "1"), u))

def Hubs.shown.ix(+lc: I.Loc, +k: String, +u: U.Ui, +cur: String, +any: Bool, +want: Bool, +idle: Bool, +lite: Bool, +ix: U.SIx) -> J.Json:
  Hubs.shown.r(lc, k, u, cur, any, want, idle, lite, ix, U.Side.rows.ix(U.Ui.state(u), ix, S.Mob.now(u)))

def Hubs.shown.go(+lc: I.Loc, hs: List<&2, Hub>, +cur: String, +any: Bool, +want: Bool, +idle: Bool, +lite: Bool) -> List<&2, Shown>:
  match hs:
    case Nil{}:
      Nil{}
    case Hub{+k, +u} <> t:
      Shown{k, Hubs.name(k, u), Hubs.shown.ix(lc, k, u, cur, any, want, idle, lite, U.Side.ix(U.Ui.state(u)))} <> Hubs.shown.go(lc, t, cur, any, want, idle, lite)

def Hubs.shown.l(+lc: I.Loc, +hs: List<&2, Hub>, +cur: String, +want: Bool, +idle: Bool) -> List<&2, Shown>:
  Hubs.shown.go(lc, hs, cur, Hubs.pl.any(hs), want, idle, Bool.and(Bool.not(idle), Nat.is_eq(List.length(&2, Hub, hs), 1n)))

def Hubs.shown.w(+lc: I.Loc, +hs: List<&2, Hub>, +cur: String, +want: Bool) -> List<&2, Shown>:
  Hubs.shown.l(lc, hs, cur, want, Hubs.pl.status(hs, cur))

def Hubs.shown(+lc: I.Loc, +hs: List<&2, Hub>, +cur: String) -> List<&2, Shown>:
  Hubs.shown.w(lc, hs, cur, Hubs.pl.want(hs, cur))

def Hubs.named(ss: List<&2, Shown>, +n: String) -> Nat:
  match ss:
    case Nil{}:
      0n
    case Shown{_, +m, _} <> t:
      Nat.add(Bool.pick(Nat, String.eq(String.to_lower(m), String.to_lower(n)), 1n, 0n), Hubs.named(t, n))

def Hubs.relabel.go(ss: List<&2, Shown>, +all: List<&2, Shown>) -> List<&2, Shown>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+k, +n, j} <> t:
      Shown{k, Bool.pick(String, Nat.is_gt(Hubs.named(all, n), 1n), n ++ " (" ++ k ++ ")", n), j} <> Hubs.relabel.go(t, all)

# two paired machines with one name (any case) are told apart by their key
def Hubs.relabel(+ss: List<&2, Shown>) -> List<&2, Shown>:
  Hubs.relabel.go(ss, ss)

def Shown.j(s: Shown) -> J.Json:
  match s:
    case Shown{_, _, j}:
      j

def Hubs.of(ss: List<&2, Shown>, +key: String) -> J.Json:
  match ss:
    case Nil{}:
      J.Null{}
    case Shown{+k, _, +j} <> t:
      Bool.pick(J.Json, String.eq(k, key), j, Hubs.of(t, key))

def Hubs.all_on(ss: List<&2, Shown>) -> Bool:
  match ss:
    case Nil{}:
      True{}
    case +s <> t:
      Bool.and(J.Json.get_bool(Shown.j(s), "online", False{}), Hubs.all_on(t))

# the first hub's non-empty string at k
def Hubs.said(ss: List<&2, Shown>, +k: String) -> String:
  match ss:
    case Nil{}:
      ""
    case +s <> t:
      Bool.pick(String, String.is_empty(J.Json.get_str(Shown.j(s), k, "")), Hubs.said(t, k), J.Json.get_str(Shown.j(s), k, ""))

def Hubs.nonnull(+j: J.Json) -> Bool:
  match j:
    case J.Null{}:
      False{}
    case _:
      True{}

def Hubs.first_obj(ss: List<&2, Shown>, +k: String) -> J.Json:
  match ss:
    case Nil{}:
      J.Null{}
    case +s <> t:
      Bool.pick(J.Json, Hubs.nonnull(J.Json.get_obj(Shown.j(s), k)), J.Json.get_obj(Shown.j(s), k), Hubs.first_obj(t, k))

# a project row, named by its machine when there are several
def Hubs.machine(p: J.Json, +name: String) -> J.Json:
  match p:
    case J.Obj{fs}:
      J.Obj{J.Field{"machine", J.Str{name}, fs}}
    case x:
      x

def Hubs.machines(ps: List<&2, J.Json>, +name: String) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Hubs.machine(p, name) <> Hubs.machines(t, name)

def Hubs.projects.all(ss: List<&2, Shown>, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{_, +n, +j} <> t:
      List.append(&2, J.Json, Hubs.machines(J.Json.elems(J.Json.get_obj(j, "projects")), Bool.pick(String, many, n, "")), Hubs.projects.all(t, many))

# the projects of every hub are one list: the live ones of all first, then
# the quiet ones, each hub's own order kept; a project is named by its
# machine only when another has the same name
def Hubs.titles(ps: List<&2, J.Json>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      J.Json.get_str(p, "title", "") <> Hubs.titles(t)

def Hubs.count(ts: List<&2, String>, +x: String) -> Nat:
  match ts:
    case Nil{}:
      0n
    case +h <> t:
      Nat.add(Bool.pick(Nat, String.eq(h, x), 1n, 0n), Hubs.count(t, x))

def Hubs.dup(p: J.Json, +keep: Bool) -> J.Json:
  match p keep:
    case J.Obj{fs} False{}:
      J.Obj{J.Field{"machine", J.Str{""}, fs}}
    case x _:
      x

def Hubs.same(a: Bool, b: Bool) -> Bool:
  match a b:
    case True{} True{}:
      True{}
    case False{} False{}:
      True{}
    case _ _:
      False{}

def Hubs.part.k(keep: Bool, p: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      p <> rest
    case False{}:
      rest

def Hubs.part(ps: List<&2, J.Json>, +quiet: Bool, +ts: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Hubs.part.k(Hubs.same(J.Json.get_bool(p, "quiet", False{}), quiet), Hubs.dup(p, Nat.is_gt(Hubs.count(ts, J.Json.get_str(p, "title", "")), 1n)), Hubs.part(t, quiet, ts))

def Hubs.projects.p(+ps: List<&2, J.Json>) -> List<&2, J.Json>:
  List.append(&2, J.Json, Hubs.part(ps, False{}, Hubs.titles(ps)), Hubs.part(ps, True{}, Hubs.titles(ps)))

def Hubs.projects(ss: List<&2, Shown>, +many: Bool) -> List<&2, J.Json>:
  Hubs.projects.p(Hubs.projects.all(ss, many))

# Once a paired owner replaces a mirror, keep its machine visible too.
# A mirror already names its origin in its title; its relay is not its
# origin. Rows from the focused hub retain the usual collision label.
def Hubs.origin.name(ss: List<&2, Shown>, +key: String) -> String:
  match ss:
    case Nil{}:
      key
    case Shown{+k, +n, _} <> t:
      Bool.pick(String, String.eq(k, key), n, Hubs.origin.name(t, key))

def Hubs.origin.project(elsewhere: Bool, p: J.Json, +name: String) -> J.Json:
  match elsewhere:
    case True{}:
      Hubs.machine(p, name)
    case False{}:
      p

def Hubs.origin.projects(ps: List<&2, J.Json>, +ss: List<&2, Shown>, +cur: String) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      +id = J.Json.get_str(p, "id", "")
      Hubs.origin.project(Bool.and(Bool.not(String.eq(Hubs.key_of(id), cur)), Bool.not(FR.Fr.is(Hubs.rest(id)))), p,
        Hubs.origin.name(ss, Hubs.key_of(id))) <> Hubs.origin.projects(t, ss, cur)

# A directly paired owner survives mirror deduplication. Its Active rows
# and focused headers still name their machine, including when its socket
# is offline. Far and reported-only titles already name their origin.
def Hubs.origin.caption(online: Bool, title: String, name: String) -> String:
  match online:
    case True{}:
      title ++ " · " ++ name
    case False{}:
      title ++ " · " ++ name ++ " (away)"

def Hubs.origin.title(mark: Bool, title: String, +name: String, +online: Bool) -> String:
  match mark:
    case False{}:
      title
    case True{}:
      Hubs.origin.caption(online, title, name)

def Hubs.origin.fields(fs: J.Json, +name: String, +online: Bool) -> J.Json:
  match fs:
    case J.Field{+k, J.Str{+v}, t}:
      J.Field{k, J.Str{Hubs.origin.title(String.eq(k, "title"), v, name, online)}, Hubs.origin.fields(t, name, online)}
    case J.Field{k, v, t}:
      J.Field{k, v, Hubs.origin.fields(t, name, online)}
    case x:
      x

def Hubs.origin.object(v: J.Json, +name: String, +online: Bool) -> J.Json:
  match v:
    case J.Obj{fs}:
      J.Obj{Hubs.origin.fields(fs, name, online)}
    case _:
      v

def Hubs.origin.label(mark: Bool, v: J.Json, name: String, online: Bool) -> J.Json:
  match mark:
    case False{}:
      v
    case True{}:
      Hubs.origin.object(v, name, online)

def Hubs.origin.direct(+v: J.Json) -> Bool:
  +id = J.Json.get_str(v, "id", "")
  Bool.not(Bool.or(Bool.or(FR.Fr.is(id), FR.Fr.is(Hubs.rest(id))),
    Bool.or(J.Json.get_bool(v, "remote", False{}), String.eq(J.Json.get_str(v, "kind", ""), "remote"))))

def Hubs.origin.row(+many: Bool, +v: J.Json, +name: String, +online: Bool) -> J.Json:
  Hubs.origin.label(Bool.and(many, Hubs.origin.direct(v)), v, name, online)

def Hubs.origin.rows(rs: List<&2, J.Json>, +many: Bool, +name: String, +online: Bool) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Hubs.origin.row(many, r, name, online) <> Hubs.origin.rows(t, many, name, online)

def Hubs.origin.focus(+ss: List<&2, Shown>, +cur: String, +field: String) -> J.Json:
  Hubs.origin.row(Nat.is_gt(List.length(&2, Shown, ss), 1n), J.Json.get_obj(Hubs.of(ss, cur), field),
    Hubs.origin.name(ss, cur), J.Json.get_bool(Hubs.of(ss, cur), "online", False{}))

# every hub's folded shelf at k ("older": older projects; "settled": the
# active view's settled rows) as one: how many in all, and open when any
# hub's is ("side-older", "side-settled" go to every hub)
def Hubs.older.n(ss: List<&2, Shown>, +k: String, +acc: Nat) -> Nat:
  match ss:
    case Nil{}:
      acc
    case Shown{_, _, +j} <> t:
      Hubs.older.n(t, k, Nat.add(acc, J.Json.get_nat(J.Json.get_obj(j, k), "count", 0n)))

def Hubs.older.open(ss: List<&2, Shown>, +k: String) -> Bool:
  match ss:
    case Nil{}:
      False{}
    case Shown{_, _, +j} <> t:
      Bool.or(J.Json.get_bool(J.Json.get_obj(j, k), "open", False{}), Hubs.older.open(t, k))

def Hubs.older(+ss: List<&2, Shown>, +k: String) -> J.Json:
  Bool.pick(J.Json, Nat.is_eq(Hubs.older.n(ss, k, 0n), 0n), J.Null{},
    J.Json.obj([J.KV{"count", J.Json.nat(Hubs.older.n(ss, k, 0n))}, J.KV{"open", J.Json.bool(Hubs.older.open(ss, k))}]))

# every hub's active rows as one list, in core's order (Active.rows): live
# rows first, by how much they wait on the person, then newest
def Hubs.arow.cmp(+a: J.Json, +b: J.Json) -> Bool:
  +fa = J.Json.get_bool(a, "faded", False{})
  +fb = J.Json.get_bool(b, "faded", False{})
  +ua = J.Json.get_nat(a, "urg", 0n)
  +ub = J.Json.get_nat(b, "urg", 0n)
  Bool.pick(Bool, Hubs.same(fa, fb), Bool.pick(Bool, Nat.is_eq(ua, ub), Nat.is_ge(J.Json.get_nat(a, "at", 0n), J.Json.get_nat(b, "at", 0n)), Nat.is_gt(ua, ub)), fb)

def Hubs.arow.before(a: J.Json, b: J.Json) -> Bool:
  Hubs.arow.cmp(a, b)

def Hubs.active.all(ss: List<&2, Shown>, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{_, +name, +j} <> t:
      List.append(&2, J.Json, Hubs.origin.rows(J.Json.elems(J.Json.get_obj(j, "active")), many, name,
        J.Json.get_bool(j, "online", False{})), Hubs.active.all(t, many))

# Sorting flattened rows independently tears children away from their
# parent. Group each depth-zero root and its visible descendants, order
# those groups by the existing root comparator, then flatten them again.
type ATree is Data:
  ATree{head: J.Json, rev: List<&2, J.Json>}

def Hubs.tree.add(root: Bool, +row: J.Json, acc: List<&2, ATree>) -> List<&2, ATree>:
  match root acc:
    case False{} ATree{head, rev} <> rest:
      ATree{head, row <> rev} <> rest
    case _ trees:
      ATree{row, [row]} <> trees

def Hubs.trees(rs: List<&2, J.Json>, acc: List<&2, ATree>) -> List<&2, ATree>:
  match rs:
    case Nil{}: List.reverse(&2, ATree, acc)
    case +r <> rest:
      Hubs.trees(rest, Hubs.tree.add(Nat.is_eq(J.Json.get_nat(r, "depth", 0n), 0n), r, acc))

def Hubs.tree.before(a: ATree, b: ATree) -> Bool:
  match a b:
    case ATree{ha, _} ATree{hb, _}:
      Hubs.arow.before(ha, hb)

def Hubs.trees.rows(ts: List<&2, ATree>) -> List<&2, J.Json>:
  match ts:
    case Nil{}: Nil{}
    case ATree{_, rev} <> rest:
      List.append(&2, J.Json, List.reverse(&2, J.Json, rev), Hubs.trees.rows(rest))

def Hubs.active(+ss: List<&2, Shown>) -> List<&2, J.Json>:
  Hubs.trees.rows(List.sort(~ATree, ~Hubs.tree.before,
    Hubs.trees(Hubs.active.all(ss, Nat.is_gt(List.length(&2, Shown, ss), 1n)), Nil{})))

# History is outside projects, with the original qualified thread ids.
# Add origin captions exactly as active rows do when several hubs report.
def Hubs.legacy.all(ss: List<&2, Shown>, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}: Nil{}
    case Shown{_, +name, +j} <> rest:
      List.append(&2, J.Json, Hubs.origin.rows(J.Json.elems(J.Json.get_obj(j, "legacy")), many, name,
        J.Json.get_bool(j, "online", False{})), Hubs.legacy.all(rest, many))

# every hub's list at k (bots, rooms), named by machine when there are several
def Hubs.all(ss: List<&2, Shown>, +k: String, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{_, +n, +j} <> t:
      List.append(&2, J.Json, Hubs.machines(J.Json.elems(J.Json.get_obj(j, k)), Bool.pick(String, many, n, "")), Hubs.all(t, k, many))

# rooms keyed by their id as their own hub gives it (a room shared by two
# machines is one room, whichever hub reports it)
def Hubs.rkeyed(rs: List<&2, J.Json>) -> List<&2, BU.RK<J.Json>>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      BU.RK{Hubs.rest(J.Json.get_str(r, "id", "")), r} <> Hubs.rkeyed(t)

# every hub's rooms, each room once (law hubs_rooms_listed_once)
def Hubs.rooms.keyed(ss: List<&2, Shown>, +many: Bool) -> List<&2, BU.RK<J.Json>>:
  BU.BU.once.of(J.Json, Hubs.rkeyed(Hubs.all(ss, "rooms", many)))

def Hubs.rooms(ss: List<&2, Shown>, +many: Bool) -> List<&2, J.Json>:
  BU.BU.once.vals(J.Json, Hubs.rooms.keyed(ss, many))

def Hubs.lines(ss: List<&2, Shown>) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      List.append(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(Shown.j(s), "island"), "lines")), Hubs.lines(t))

# A thread the owner's machine runs and this phone's other hub mirrors is
# one thread: the island lists it once, as its owner's hub gives it (the
# once list keeps the last of a key, so the owner's lines go last), else
# one mirror's (law island_far_once)
# (a line's thread is qualified by its hub, "host:port|id": the key is the
# owner's own id, so a mirror's "<link>~t" and its owner's "t" are one)
def Hubs.line.id(+t: String) -> String:
  Bool.pick(String, String.contains(t, "|"), Hubs.rest(t), t)

def Hubs.line.is.far(+l: J.Json) -> Bool:
  FR.Fr.is(Hubs.line.id(J.Json.get_str(l, "thread", "")))

def Hubs.line.keep(take: Bool, l: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match take:
    case True{}:
      l <> rest
    case False{}:
      rest

# the lines whose thread is elsewhere (far) or here (not far)
def Hubs.lines.part(ls: List<&2, J.Json>, +far: Bool) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Hubs.line.keep(Bool.pick(Bool, far, Hubs.line.is.far(l), Bool.not(Hubs.line.is.far(l))), l, Hubs.lines.part(t, far))

def Hubs.lines.keyed(ls: List<&2, J.Json>) -> List<&2, BU.RK<J.Json>>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      BU.RK{FR.Fr.local(Hubs.line.id(J.Json.get_str(l, "thread", ""))), l} <> Hubs.lines.keyed(t)

def Hubs.lines.once(+ls: List<&2, J.Json>) -> List<&2, J.Json>:
  BU.BU.once.vals(J.Json, BU.BU.once.of(J.Json, Hubs.lines.keyed(List.append(&2, J.Json, Hubs.lines.part(ls, True{}), Hubs.lines.part(ls, False{})))))

# Each hub's own island, for iOS's Live Activity per hub: a hub pushes and
# ends only its own (push.bend), so one machine going idle never ends or
# overwrites another's. With several hubs the headline names the machine.
# Merge counts separately from the one line: a hub may count roots with
# no person message. The screen's islandAt is transient, never kept state.
type HCompact is Data:
  HCompact{lines: List<&2, J.Json>, at: Nat}

def Hubs.compact.lines(c: HCompact) -> List<&2, J.Json>:
  match c:
    case HCompact{ls, _}:
      ls

def Hubs.compact.at(c: HCompact) -> Nat:
  match c:
    case HCompact{_, at}:
      at

def Hubs.compact.pick(hit: Bool, ls: List<&2, J.Json>, at: Nat, old: List<&2, J.Json>, stamp: Nat) -> HCompact:
  match hit:
    case True{}:
      HCompact{ls, at}
    case False{}:
      HCompact{old, stamp}

def Hubs.compact.go.loc(+lc: I.Loc, ss: List<&2, Shown>, +n: Nat, +ls: List<&2, J.Json>, +stamp: Nat) -> J.Json:
  match ss:
    case Nil{}:
      J.Json.obj([J.KV{"running", J.Json.nat(n)}, J.KV{"headline", J.Json.str(N.Note.compact.head(lc, n))}, J.KV{"lines", J.Json.arr(ls)}])
    case Shown{_, _, +j} <> t:
      +i = J.Json.get_obj(j, "island")
      +next = J.Json.elems(J.Json.get_obj(i, "lines"))
      +at = J.Json.get_nat(j, "islandAt", 0n)
      +picked = Hubs.compact.pick(NO.Island.compact.newer(J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, next), J.Null{}), "thread", ""), at,
        J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, ls), J.Null{}), "thread", ""), stamp), next, at, ls, stamp)
      Hubs.compact.go.loc(lc, t, Nat.add(n, J.Json.get_nat(i, "running", 0n)), Hubs.compact.lines(picked), Hubs.compact.at(picked))

def Hubs.compact.go(ss: List<&2, Shown>, +n: Nat, +ls: List<&2, J.Json>, +stamp: Nat) -> J.Json:
  Hubs.compact.go.loc(I.En{}, ss, n, ls, stamp)

def Hubs.island.named(+many: Bool, +name: String, +i: J.Json) -> J.Json:
  J.Json.obj([J.KV{"running", J.Json.get_obj(i, "running")},
    J.KV{"headline", J.Json.str(N.Note.named(J.Json.get_str(i, "headline", ""), Bool.pick(String, many, name, "")))},
    J.KV{"lines", J.Json.get_obj(i, "lines")}])

def Hubs.islands.go(ss: List<&2, Shown>, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+k, +n, +j} <> t:
      J.Json.obj([J.KV{"hub", J.Json.str(k)}, J.KV{"island", Hubs.island.named(many, n, J.Json.get_obj(j, "island"))}]) <> Hubs.islands.go(t, many)

def Hubs.islands(+ss: List<&2, Shown>) -> List<&2, J.Json>:
  Hubs.islands.go(ss, Nat.is_gt(List.length(&2, Shown, ss), 1n))

def Hubs.rows(ss: List<&2, Shown>) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+k, +n, +j} <> t:
      J.Json.obj([J.KV{"key", J.Json.str(k)}, J.KV{"name", J.Json.str(n)}, J.KV{"online", J.Json.bool(J.Json.get_bool(j, "online", False{}))},
        J.KV{"version", J.Json.str(J.Json.get_str(j, "version", ""))}, J.KV{"changes", J.Json.nat(J.Json.get_nat(j, "seq", 0n))}]) <> Hubs.rows(t)

# Keep each paired machine's identity even when it is offline or has the
# same display name as another. Actions stay qualified to the owning hub.
def Hubs.usage.machine(+k: String, +name: String, +j: J.Json) -> J.Json:
  J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"name", J.Str{name}},
    J.KV{"online", J.Json.bool(J.Json.get_bool(j, "online", False{}))},
    J.KV{"rows", J.Json.arr(J.Json.elems(J.Json.get_obj(j, "machineUsage")))}])

def Hubs.usage.machines(ss: List<&2, Shown>) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{k, name, j} <> t:
      Hubs.usage.machine(k, name, j) <> Hubs.usage.machines(t)

# the host:port of a hub's url (http://host:3787 -> host:3787)
def Hubs.auth(+url: String) -> String:
  Bool.pick(String, String.contains(url, "://"),
    Maybe.default(&2, String, List.get(&2, String, String.split(url, '/'), 2n), ""),
    Maybe.default(&2, String, List.head(&2, String, String.split(url, '/')), ""))

# Bots over every paired hub
# --------------------------
# A hub lists its own bots and those of the machines it is linked to, so a
# phone paired with two linked hubs would list each bot twice: from its own
# hub, and as name@machine from the other. Each bot is keyed by what it
# stands for and kept once (law hubs_bots_once): a hub's own bot as
# "key|id"; a bot elsewhere, when another paired hub holds that link (or is
# the machine at its url) and has a bot of that name, as that hub's own
# bot; else by its machine's host:port and its name, so two hubs reporting
# the same machine agree. Every hub's own bots come first, so a bot shows
# as its own hub's. A bot a hub mirrors from a linked machine (its threads
# kept there, far.bend) is dropped when that machine is paired too
# (Hubs.bot.dup): it shows once, as its own hub's.

def Hubs.state(h: Hub) -> M.State:
  U.Ui.state(Hub.ui(h))

def Hubs.bot.id(m: Maybe<&2, BT.Bot>) -> String:
  match m:
    case Some{b}:
      BT.Bot.id(b)
    case None{}:
      ""

def Hubs.peer.url(m: Maybe<&2, BT.Peer>) -> String:
  match m:
    case Some{BT.Peer{_, _, u, _, _}}:
      u
    case None{}:
      ""

def Hubs.twin.k(hit: Bool, +k: String, +id: String, +rest: String) -> String:
  Bool.pick(String, Bool.and(hit, Bool.not(String.is_empty(id))), k ++ "|" ++ id, rest)

# whether hub k (not `from`) is the machine a link or an url leads to
def Hubs.twin.is(+k: String, +st: M.State, +from: String, +link: String, +at: String) -> Bool:
  Bool.and(Bool.not(String.eq(k, from)),
    Bool.or(Bool.and(Bool.not(String.is_empty(link)), BT.Maybe.is(BT.Peer, BT.Peer.by.id(BT.World.peers(M.State.bots(st)), link))),
      Bool.and(Bool.not(String.is_empty(at)), String.eq(k, at))))

# that hub's own bot of the name, as "key|id" ("" when no paired hub is it)
def Hubs.twin(hs: List<&2, Hub>, +from: String, +link: String, +at: String, +name: String) -> String:
  match hs:
    case Nil{}:
      ""
    case +h <> t:
      Hubs.twin.k(Hubs.twin.is(Hub.key(h), Hubs.state(h), from, link, at), Hub.key(h),
        Hubs.bot.id(BT.World.find(M.State.bots(Hubs.state(h)), name)), Hubs.twin(t, from, link, at, name))

def Hubs.of.key(hs: List<&2, Hub>, +k: String) -> M.State:
  match hs:
    case Nil{}:
      M.State.empty()
    case +h <> t:
      Bool.pick(M.State, String.eq(Hub.key(h), k), Hubs.state(h), Hubs.of.key(t, k))

def Hubs.far.k(+twin: String, +at: String, +own: String, +name: String) -> String:
  Bool.pick(String, String.is_empty(twin), Bool.pick(String, String.is_empty(at), own, "@" ++ at ++ "|" ++ String.to_lower(name)), twin)

def Hubs.far.at(+st: M.State, +link: String) -> String:
  Bool.pick(String, String.is_empty(Hubs.peer.url(BT.Peer.by.id(BT.World.peers(M.State.bots(st)), link))), "",
    Hubs.auth(Hubs.peer.url(BT.Peer.by.id(BT.World.peers(M.State.bots(st)), link))))

def Hubs.far.key(+hs: List<&2, Hub>, +from: String, +at: String, +r: J.Json) -> String:
  Hubs.far.k(Hubs.twin(hs, from, J.Json.get_str(r, "link", ""), at, J.Json.get_str(r, "name", "")), at,
    from ++ "|" ++ J.Json.get_str(r, "key", ""), J.Json.get_str(r, "name", ""))

# what a row of hub `from` stands for
def Hubs.bot.key(+hs: List<&2, Hub>, +from: String, +r: J.Json) -> String:
  Bool.pick(String, J.Json.get_bool(r, "remote", False{}),
    Hubs.far.key(hs, from, Hubs.far.at(Hubs.of.key(hs, from), J.Json.get_str(r, "link", "")), r),
    from ++ "|" ++ J.Json.get_str(r, "key", ""))

# whether a paired hub other than `from` holds link l: a link's id is the
# same at both its ends, so that hub is the machine l reaches from `from`
# (or, for a link among more, one that reaches it too)
def Hubs.holds(hs: List<&2, Hub>, +from: String, +l: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(Bool.and(Bool.not(String.eq(Hub.key(h), from)), BT.Maybe.is(BT.Peer, BT.Peer.by.id(BT.World.peers(M.State.bots(Hubs.state(h))), l))),
        Hubs.holds(t, from, l))

# Projects and ordinary threads follow the same ownership rule as bots.
# Drop a mirror only when the paired owner actually has that entity;
# during initial connection its cached mirror must remain visible.
def Hubs.entity.got(-A: Data, r: Map<&2, A> & Bool) -> Bool:
  (_, b) = r
  b

def Hubs.entity.has(+s: M.State, project: Bool, +id: String) -> Bool:
  match s project:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _} True{}:
      Hubs.entity.got(M.Project, Map.has(&2, M.Project, ps, id))
    case M.State{_, ts, _, _, _, _, _, _, _, _, _} False{}:
      Hubs.entity.got(M.Thread, Map.has(&2, M.Thread, ts, id))

def Hubs.entity.owner(hs: List<&2, Hub>, +from: String, +link: String, +at: String, +project: Bool, +id: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(Bool.and(Hubs.twin.is(Hub.key(h), Hubs.state(h), from, link, at), Hubs.entity.has(Hubs.state(h), project, id)),
        Hubs.entity.owner(t, from, link, at, project, id))

def Hubs.entity.dup(+hs: List<&2, Hub>, +from: String, +project: Bool, +id: String) -> Bool:
  Bool.and(FR.Fr.is(id), Hubs.entity.owner(hs, from, FR.Fr.link(id), Hubs.far.at(Hubs.of.key(hs, from), FR.Fr.link(id)), project, FR.Fr.local(id)))

def Hubs.catalog.rows(rs: List<&2, J.Json>, +hs: List<&2, Hub>, +from: String, +project: Bool) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Hubs.part.k(Bool.not(Hubs.entity.dup(hs, from, project, Hubs.rest(J.Json.get_str(r, "id", "")))), r,
        Hubs.catalog.rows(t, hs, from, project))

# Filter only catalog arrays, never timeline text or the focused view.
# A mirrored project contains only mirrored threads, so dropping its
# section also drops every shelf without touching local fold state.
def Hubs.catalog.value(catalog: Bool, v: J.Json, +hs: List<&2, Hub>, +from: String, +project: Bool) -> J.Json:
  match catalog:
    case True{}:
      J.Json.arr(Hubs.catalog.rows(J.Json.elems(v), hs, from, project))
    case False{}:
      v

# Fold counts include hidden rows. Apply the ownership filter to those
# rows too, so pairing the owner does not double Older or Settled.
def Hubs.fold.projects(rs: List<&2, U.SRow>, +hs: List<&2, Hub>, +from: String, acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case U.SRow{_, M.Project{+id, _, _, _, _}, _} <> t:
      Hubs.fold.projects(t, hs, from, Nat.add(acc, Bool.pick(Nat, Hubs.entity.dup(hs, from, True{}, id), 1n, 0n)))

def Hubs.fold.older(g: U.SGroups, +hs: List<&2, Hub>, +from: String) -> Nat:
  match g:
    case U.SGroups{_, _, o}:
      Hubs.fold.projects(o, hs, from, 0n)

def Hubs.fold.rows(rs: List<&2, U.ARow>, +hs: List<&2, Hub>, +from: String, acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case U.ARow{_, _, +t, _, _, _} <> rest:
      Hubs.fold.rows(rest, hs, from, Nat.add(acc, Bool.pick(Nat, Hubs.entity.dup(hs, from, False{}, M.Thread.id(t)), 1n, 0n)))

def Hubs.fold.settled(p: U.AParts, +hs: List<&2, Hub>, +from: String) -> Nat:
  match p:
    case U.AParts{_, f}:
      Hubs.fold.rows(f, hs, from, 0n)

def Hubs.fold.count(older: Bool, h: Maybe<&2, Hub>, +hs: List<&2, Hub>, +from: String) -> Nat:
  match older h:
    case True{} Some{Hub{_, U.Ui{+st, _, +dr, _, +o, _, +now, _, _, _, _, _, _, _, _, _, _}}}:
      Hubs.fold.older(U.Side.groups(U.Side.shown.ix(st, U.Side.ix(st), now, U.Find.q(o, dr)), now), hs, from)
    case False{} Some{Hub{_, U.Ui{+st, _, +dr, _, +o, _, +now, _, _, _, _, _, _, _, _, _, _}}}:
      Hubs.fold.settled(U.Active.parts(U.Active.rows.ix(st, U.Side.ix(st), now, U.Find.q(o, dr))), hs, from)
    case _ None{}:
      0n

def Hubs.fold.json(v: J.Json, +older: Bool, +hs: List<&2, Hub>, +from: String) -> J.Json:
  match v:
    case J.Obj{+fs}:
      S.Mob.older.j(Nat.sub(J.Json.get_nat(J.Obj{fs}, "count", 0n), Hubs.fold.count(older, Hubs.find(hs, from), hs, from)),
        J.Json.get_bool(J.Obj{fs}, "open", False{}), "")
    case x:
      x

def Hubs.catalog.field(kind: Nat, v: J.Json, +hs: List<&2, Hub>, +from: String) -> J.Json:
  match kind:
    case 0n:
      Hubs.catalog.value(True{}, v, hs, from, True{})
    case 1n:
      Hubs.catalog.value(True{}, v, hs, from, False{})
    case 2n:
      Hubs.fold.json(v, True{}, hs, from)
    case 3n:
      Hubs.fold.json(v, False{}, hs, from)
    case _:
      v

def Hubs.catalog.fields(fs: J.Json, +hs: List<&2, Hub>, +from: String) -> J.Json:
  match fs:
    case J.Field{+k, +v, t}:
      J.Field{k, Hubs.catalog.field(C.Codec.pos(["projects", "active", "older", "settled"], k), v, hs, from), Hubs.catalog.fields(t, hs, from)}
    case x:
      x

def Hubs.catalog.shown(ss: List<&2, Shown>, +hs: List<&2, Hub>) -> List<&2, Shown>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+k, n, J.Obj{fs}} <> t:
      Shown{k, n, J.Obj{Hubs.catalog.fields(fs, hs, k)}} <> Hubs.catalog.shown(t, hs)
    case h <> t:
      h <> Hubs.catalog.shown(t, hs)

# a row a hub mirrors from a linked machine (far.bend): a bot elsewhere
# that opens as a bot (BU.act), its threads kept by this hub
def Hubs.mirrored(+r: J.Json) -> Bool:
  Bool.and(J.Json.get_bool(r, "remote", False{}), String.eq(J.Json.get_str(r, "act", ""), "bot"))

# a mirrored bot whose own machine is paired too: that hub lists it as its
# own, so the copy goes (law hubs_bots_mirror_dropped)
def Hubs.bot.dup(+hs: List<&2, Hub>, +from: String, +r: J.Json) -> Bool:
  Bool.and(Hubs.mirrored(r), Bool.and(Bool.not(String.is_empty(J.Json.get_str(r, "link", ""))), Hubs.holds(hs, from, J.Json.get_str(r, "link", ""))))

def Hubs.bot.keep.k(dup: Bool, +far: Bool, +want: Bool, +hs: List<&2, Hub>, +from: String, +r: J.Json, rest: List<&2, O.Ky<J.Json>>) -> List<&2, O.Ky<J.Json>>:
  match dup:
    case True{}:
      rest
    case False{}:
      BT.Post.keep(O.Ky<J.Json>, Bool.pick(Bool, want, far, Bool.not(far)), O.Ky{Hubs.bot.key(hs, from, r), r}, rest)

def Hubs.bot.keep(+far: Bool, +want: Bool, +hs: List<&2, Hub>, +from: String, +r: J.Json, rest: List<&2, O.Ky<J.Json>>) -> List<&2, O.Ky<J.Json>>:
  Hubs.bot.keep.k(Hubs.bot.dup(hs, from, r), far, want, hs, from, r, rest)

def Hubs.bot.rows(rs: List<&2, J.Json>, +want: Bool, +hs: List<&2, Hub>, +from: String) -> List<&2, O.Ky<J.Json>>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Hubs.bot.keep(J.Json.get_bool(r, "remote", False{}), want, hs, from, r, Hubs.bot.rows(t, want, hs, from))

# every hub's own bots (want False) or those elsewhere (want True), keyed
def Hubs.bots.of(ss: List<&2, Shown>, +want: Bool, +hs: List<&2, Hub>, +many: Bool) -> List<&2, O.Ky<J.Json>>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+k, +n, +j} <> t:
      List.append(&2, O.Ky<J.Json>, Hubs.bot.rows(Hubs.machines(J.Json.elems(J.Json.get_obj(j, "bots")), Bool.pick(String, many, n, "")), want, hs, k),
        Hubs.bots.of(t, want, hs, many))

def Hubs.bots.src(+ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool) -> List<&2, O.Ky<J.Json>>:
  List.append(&2, O.Ky<J.Json>, Hubs.bots.of(ss, False{}, hs, many), Hubs.bots.of(ss, True{}, hs, many))

def Hubs.bots.keyed(+ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool) -> List<&2, O.Ky<J.Json>>:
  O.Once.of(J.Json, Hubs.bots.src(ss, hs, many))

# the cats of every paired hub, each bot once
def Hubs.bots(+ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool) -> List<&2, J.Json>:
  O.Once.vals(J.Json, Hubs.bots.keyed(ss, hs, many))

# The status plane over every paired hub (docs/uber.md)
# ------------------------------------------------------
# Each hub's screen carries its own "uber" card and "plane" (plane.bend);
# the phone shows one: the uberbot of the hub in focus, else the first
# hub's, and every hub's rows under it. A row is routed by the hub its ids
# carry. A thread or bot a hub only mirrors from a machine this phone is
# paired with too is dropped, the owner's hub lists it itself (as the
# Active rows and the cats do), and every list shows a row once (law
# hubs_plane_once). A row of a machine other than the one it reports is
# named by it when there are several hubs.

def Hubs.pl.far(+hs: List<&2, Hub>, +from: String, +bot: String) -> Bool:
  Bool.and(FR.Fr.is(bot), Hubs.holds(hs, from, FR.Fr.link(bot)))

# a routine's value is "<bot>|routines" (the plane is qualified by Hubs.pl.q once its rows are kept)
def Hubs.pl.bot.of(+value: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(value, '|')), "")

def Hubs.pl.dup.k(i: Nat, +hs: List<&2, Hub>, +from: String, +id: String, +value: String) -> Bool:
  match i:
    case 0n:
      Hubs.entity.dup(hs, from, False{}, id)
    case 1n:
      Hubs.pl.far(hs, from, id)
    case 2n:
      Hubs.pl.far(hs, from, Hubs.pl.bot.of(value))
    case _:
      False{}

def Hubs.pl.dup(+hs: List<&2, Hub>, +from: String, +r: J.Json) -> Bool:
  Hubs.pl.dup.k(C.Codec.pos(["thread", "bot", "routine"], J.Json.get_str(r, "kind", "")), hs, from, J.Json.get_str(r, "id", ""), J.Json.get_str(r, "value", ""))

def Hubs.pl.label.k(name: Bool, r: J.Json, +machine: String) -> J.Json:
  match name:
    case True{}:
      Hubs.machine(r, machine)
    case False{}:
      r

def Hubs.pl.label(+many: Bool, +machine: String, +r: J.Json) -> J.Json:
  Hubs.pl.label.k(Bool.and(many, String.is_empty(J.Json.get_str(r, "machine", ""))), r, machine)

def Hubs.pl.rows(rs: List<&2, J.Json>, +hs: List<&2, Hub>, +from: String, +many: Bool, +machine: String) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Hubs.part.k(Bool.not(Hubs.pl.dup(hs, from, r)), Hubs.pl.q(Hubs.pl.label(many, machine, r), from), Hubs.pl.rows(t, hs, from, many, machine))

# every hub's rows of the plane's section k
def Hubs.pl.all(ss: List<&2, Shown>, +hs: List<&2, Hub>, +k: String, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+from, +name, +j} <> t:
      List.append(&2, J.Json, Hubs.pl.rows(J.Json.elems(J.Json.get_obj(J.Json.get_obj(j, "plane"), k)), hs, from, many, name), Hubs.pl.all(t, hs, k, many))

# the order of core's sections (Plane.before, Plane.newer, Plane.due.before)
def Hubs.pl.need.before.cmp(+a: J.Json, +b: J.Json) -> Bool:
  +ra = PL.Plane.rank(J.Json.get_str(a, "state", ""))
  +rb = PL.Plane.rank(J.Json.get_str(b, "state", ""))
  Bool.pick(Bool, Nat.is_eq(ra, rb), Nat.is_ge(J.Json.get_nat(a, "at", 0n), J.Json.get_nat(b, "at", 0n)), Nat.is_gt(ra, rb))

def Hubs.pl.newer.cmp(+a: J.Json, +b: J.Json) -> Bool:
  Nat.is_ge(J.Json.get_nat(a, "at", 0n), J.Json.get_nat(b, "at", 0n))

def Hubs.pl.due.before.cmp(+a: J.Json, +b: J.Json) -> Bool:
  +sa = J.Json.get_str(a, "state", "")
  +sb = J.Json.get_str(b, "state", "")
  Bool.pick(Bool, String.eq(sa, sb), PL.Plane.due.le(J.Json.get_nat(a, "at", 0n), J.Json.get_nat(b, "at", 0n)), String.eq(sa, "working"))

def Hubs.pl.need.before(a: J.Json, b: J.Json) -> Bool:
  Hubs.pl.need.before.cmp(a, b)

def Hubs.pl.newer(a: J.Json, b: J.Json) -> Bool:
  Hubs.pl.newer.cmp(a, b)

def Hubs.pl.due.before(a: J.Json, b: J.Json) -> Bool:
  Hubs.pl.due.before.cmp(a, b)

# a row's key: what it stands for (a room is the same on every hub)
def Hubs.pl.key(+r: J.Json) -> String:
  J.Json.get_str(r, "kind", "") ++ ":" ++ Bool.pick(String, String.eq(J.Json.get_str(r, "kind", ""), "room"), Hubs.rest(J.Json.get_str(r, "id", "")), J.Json.get_str(r, "id", ""))

def Hubs.pl.src(rs: List<&2, J.Json>) -> List<&2, O.Ky<J.Json>>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      O.Ky{Hubs.pl.key(r), r} <> Hubs.pl.src(t)

def Hubs.pl.keyed(+rs: List<&2, J.Json>) -> List<&2, O.Ky<J.Json>>:
  O.Once.of(J.Json, Hubs.pl.src(rs))

def Hubs.pl.once(+rs: List<&2, J.Json>) -> List<&2, J.Json>:
  O.Once.vals(J.Json, Hubs.pl.keyed(rs))

def Hubs.pl.idle.n(ss: List<&2, Shown>, +k: String, acc: Nat) -> Nat:
  match ss:
    case Nil{}:
      acc
    case Shown{_, _, +j} <> t:
      Hubs.pl.idle.n(t, k, Nat.add(acc, J.Json.get_nat(J.Json.get_obj(j, "plane"), k, 0n)))

def Hubs.pl.len(ss: List<&2, Shown>, +k: String, acc: Nat) -> Nat:
  match ss:
    case Nil{}:
      acc
    case Shown{_, _, +j} <> t:
      Hubs.pl.len(t, k, Nat.add(acc, List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(j, "plane"), k)))))

# the uberbot's card: the focused hub's, else the first hub's that has one
def Hubs.pl.uber(+ss: List<&2, Shown>, +cur: String) -> J.Json:
  Bool.pick(J.Json, Hubs.nonnull(J.Json.get_obj(Hubs.of(ss, cur), "uber")), J.Json.get_obj(Hubs.of(ss, cur), "uber"), Hubs.first_obj(ss, "uber"))

def Hubs.pl.working.k(hit: Bool, +n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

# the merged bots at work (mood work or talk, as core's Plane.working)
def Hubs.pl.working(rs: List<&2, J.Json>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case +r <> t:
      Hubs.pl.working.k(Bool.or(String.eq(J.Json.get_str(r, "state", ""), "work"), String.eq(J.Json.get_str(r, "state", ""), "talk")), Hubs.pl.working(t))

# the merged need rows that are failed runs (core's Plane.failed)
def Hubs.pl.failed(rs: List<&2, J.Json>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case +r <> t:
      Hubs.pl.working.k(String.eq(J.Json.get_str(r, "state", ""), "failed"), Hubs.pl.failed(t))

def Hubs.pl.json(+lc: I.Loc, +need: List<&2, J.Json>, +run: List<&2, J.Json>, +idle: List<&2, J.Json>, +idle_n: Nat, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool, +uber: J.Json) -> MPL.MUber:
  MPL.MUber{MPL.MPlane.card.k(uber, PL.Plane.counts.of(lc, List.length(&2, J.Json, need), Hubs.pl.failed(need), List.length(&2, J.Json, run))),
    J.Json.obj([J.KV{"need", J.Json.arr(need)}, J.KV{"run", J.Json.arr(run)}, J.KV{"idle", J.Json.arr(List.take(&2, J.Json, idle, PL.Plane.idle.max()))},
      J.KV{"idleN", J.Json.nat(idle_n)},
      J.KV{"working", J.Json.nat(Hubs.pl.working(Hubs.pl.once(Hubs.pl.all(ss, hs, "bots", many))))},
      J.KV{"bots", J.Json.arr(Hubs.pl.once(Hubs.pl.all(ss, hs, "bots", many)))},
      J.KV{"rooms", J.Json.arr(Hubs.pl.once(List.sort(~J.Json, ~Hubs.pl.newer, Hubs.pl.all(ss, hs, "rooms", many))))},
      J.KV{"locks", J.Json.arr(Hubs.pl.once(Hubs.pl.all(ss, hs, "locks", many)))},
      J.KV{"routines", J.Json.arr(Hubs.pl.once(List.sort(~J.Json, ~Hubs.pl.due.before, Hubs.pl.all(ss, hs, "routines", many))))},
      J.KV{"machines", J.Json.get_obj(J.Json.get_obj(Hubs.of(ss, Hubs.key_of(J.Json.get_str(uber, "id", ""))), "plane"), "machines")},
      J.KV{"home", J.Json.get_obj(J.Json.get_obj(Hubs.of(ss, Hubs.key_of(J.Json.get_str(uber, "id", ""))), "plane"), "home")}])}

# (the idle count lost to dropped mirrors comes off the hubs' own counts)
def Hubs.pl.idle.k(+lc: I.Loc, +idle: List<&2, J.Json>, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool, +uber: J.Json) -> MPL.MUber:
  Hubs.pl.json(lc, Hubs.pl.once(List.sort(~J.Json, ~Hubs.pl.need.before, Hubs.pl.all(ss, hs, "need", many))),
    Hubs.pl.once(List.sort(~J.Json, ~Hubs.pl.newer, Hubs.pl.all(ss, hs, "run", many))), idle,
    Nat.sub(Hubs.pl.idle.n(ss, "idleN", 0n), Nat.sub(Hubs.pl.len(ss, "idle", 0n), List.length(&2, J.Json, idle))), ss, hs, many, uber)

def Hubs.pl.build(+lc: I.Loc, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool, +uber: J.Json) -> MPL.MUber:
  Hubs.pl.idle.k(lc, Hubs.pl.once(List.sort(~J.Json, ~Hubs.pl.newer, Hubs.pl.all(ss, hs, "idle", many))), ss, hs, many, uber)

# One paired hub: its plane is the plane (core keeps each row once and in
# order, and no other hub is there to mirror a row), and only its ids need
# the hub's name.
def Hubs.pl.one.fields(fs: J.Json, +from: String) -> J.Json:
  match fs:
    case J.Field{+k, J.Arr{+xs}, t}:
      J.Field{k, J.Json.arr(Hubs.pl.q.rows(J.Json.elems(J.Arr{xs}), from)), Hubs.pl.one.fields(t, from)}
    case J.Field{k, v, t}:
      J.Field{k, v, Hubs.pl.one.fields(t, from)}
    case x:
      x

def Hubs.pl.one.j(plane: J.Json, +from: String) -> J.Json:
  match plane:
    case J.Obj{fs}:
      J.Obj{Hubs.pl.one.fields(fs, from)}
    case x:
      x

def Hubs.pl.one(+ss: List<&2, Shown>, +uber: J.Json) -> MPL.MUber:
  match ss:
    case Shown{+k, _, +j} <> _:
      MPL.MUber{uber, Hubs.pl.one.j(J.Json.get_obj(j, "plane"), k)}
    case Nil{}:
      MPL.MUber{J.Null{}, J.Null{}}

def Hubs.pl.of.m(+lc: I.Loc, many: Bool, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +uber: J.Json) -> MPL.MUber:
  match many:
    case True{}:
      Hubs.pl.build(lc, ss, hs, True{}, uber)
    case False{}:
      Hubs.pl.one(ss, uber)

def Hubs.pl.of.w(+lc: I.Loc, want: Bool, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +uber: J.Json) -> MPL.MUber:
  match want:
    case True{}:
      Hubs.pl.of.m(lc, Nat.is_gt(List.length(&2, Shown, ss), 1n), ss, hs, uber)
    case False{}:
      MPL.MUber{uber, J.Null{}}

def Hubs.pl.of.k(+lc: I.Loc, up: Bool, +want: Bool, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool, +uber: J.Json) -> MPL.MUber:
  match up:
    case True{}:
      Hubs.pl.of.w(lc, want, ss, hs, uber)
    case False{}:
      MPL.MUber{J.Null{}, J.Null{}}

# the card and the plane, both null while no hub has an uberbot
def Hubs.pl.of(+lc: I.Loc, +ss: List<&2, Shown>, +hs: List<&2, Hub>, +cur: String, +many: Bool, +want: Bool) -> MPL.MUber:
  Hubs.pl.of.k(lc, Hubs.nonnull(Hubs.pl.uber(ss, cur)), want, ss, hs, many, Hubs.pl.uber(ss, cur))

def Hubs.peer(skip: Bool, +p: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match skip:
    case True{}:
      rest
    case False{}:
      J.Json.obj([J.KV{"name", J.Json.str(J.Json.get_str(p, "name", ""))}, J.KV{"url", J.Json.str(J.Json.get_str(p, "url", ""))}]) <> rest

def Hubs.known(ks: List<&2, String>, +a: String) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case k <> t:
      Bool.or(String.eq(k, a), Hubs.known(t, a))

# peers a hub knows of, not paired yet: each once, the first hub's first
def Hubs.peers(ps: List<&2, J.Json>, +ks: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Hubs.peer(Bool.or(String.is_empty(J.Json.get_str(p, "url", "")), Hubs.known(ks, Hubs.auth(J.Json.get_str(p, "url", "")))), p,
        Hubs.peers(t, Hubs.auth(J.Json.get_str(p, "url", "")) <> ks))

def Hubs.peer_lists(hs: List<&2, Hub>) -> List<&2, J.Json>:
  match hs:
    case Nil{}:
      Nil{}
    case Hub{_, +u} <> t:
      List.append(&2, J.Json, J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(U.Ui.info(u, "machines")), J.Null{}), "peers")), Hubs.peer_lists(t))

def Hubs.keys(hs: List<&2, Hub>) -> List<&2, String>:
  match hs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Hub.key(h) <> Hubs.keys(t)

# the owner's machines the paired hubs know of that this phone is not
# paired with: one tap adds each
def Hubs.found(+hs: List<&2, Hub>) -> List<&2, J.Json>:
  Hubs.peers(Hubs.peer_lists(hs), Hubs.keys(hs))

def Hubs.empty.loc(+lc: I.Loc, +n: Nat, +m: Nat, +finding: Bool) -> String:
  Bool.pick(String, Nat.is_eq(m, 0n), MSG.T.home.pair(lc),
    Bool.pick(String, Bool.and(finding, Nat.is_eq(n, 0n)), MSG.T.home.no.results(lc), S.Mob.empty.loc(lc, n)))

# the plane is left out (absent, not null) off the list and the Status tab

def Hubs.empty(+n: Nat, +m: Nat, +finding: Bool) -> String:
  Hubs.empty.loc(I.En{}, n, m, finding)

def Hubs.pl.kv(plane: J.Json) -> List<&2, J.KV>:
  match plane:
    case J.Null{}:
      Nil{}
    case x:
      [J.KV{"plane", x}]

# The tabs over every hub (tabs.bend): a badge is the sum of the hubs' (a
# hub whose rows are mirrors another paired hub owns counts nothing twice, as
# its plane's rows are dropped), the routines every hub's rows, each once,
# the soonest due first
def Hubs.tabs.sum(ss: List<&2, Shown>, +tab: String, acc: Nat) -> Nat:
  match ss:
    case Nil{}:
      acc
    case Shown{_, _, +j} <> t:
      Hubs.tabs.sum(t, tab, Nat.add(acc, J.Json.get_nat(J.Json.get_obj(J.Json.get_obj(j, "tabs"), tab), "badge", 0n)))

def Hubs.tabs.rows(ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      Nil{}
    case Shown{+from, +name, +j} <> t:
      List.append(&2, J.Json, Hubs.pl.rows(J.Json.elems(J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(j, "tabs"), "routines"), "rows")), hs, from, many, name), Hubs.tabs.rows(t, hs, many))

def Hubs.tabs(+ss: List<&2, Shown>, +hs: List<&2, Hub>, +many: Bool) -> J.Json:
  J.Json.obj([J.KV{"home", MT.MTab.badge(Hubs.tabs.sum(ss, "home", 0n))}, J.KV{"bots", MT.MTab.badge(Hubs.tabs.sum(ss, "bots", 0n))},
    J.KV{"routines", J.Json.obj([J.KV{"badge", J.Json.nat(Hubs.tabs.sum(ss, "routines", 0n))},
      J.KV{"rows", J.Json.arr(Hubs.pl.once(List.sort(~J.Json, ~Hubs.pl.due.before, Hubs.tabs.rows(ss, hs, many))))}])}])

# Keep one small lease snapshot per paired hub, including background hubs.
# The hosts reset observations at each transport epoch and dedupe by lease.
def Hubs.bub.homes(ss: List<&2, Shown>) -> List<&2, J.Json>:
  match ss:
    case Nil{}: Nil{}
    case Shown{k, _, j} <> rest:
      J.Json.obj([J.KV{"hub", J.Str{k}}, J.KV{"home", J.Json.get_obj(j, "bubHome")}]) <> Hubs.bub.homes(rest)

def Hubs.screen.p.loc(+lc: I.Loc, +pl: MPL.MUber, +ss: List<&2, Shown>, +cur: String, +list: List<&2, Hub>) -> J.Json:
  J.Json.obj(List.append(&2, J.KV, Hubs.pl.kv(MPL.MUber.plane(pl)), [J.KV{"bubHomes", J.Json.arr(Hubs.bub.homes(ss))}, J.KV{"online", J.Json.bool(Hubs.all_on(ss))}, J.KV{"version", J.Json.str(J.Json.get_str(Hubs.of(ss, cur), "version", ""))},
    J.KV{"error", J.Json.str(Hubs.said(ss, "error"))}, J.KV{"note", J.Json.str(Hubs.said(ss, "note"))},
    J.KV{"sel", J.Json.str(J.Json.get_str(Hubs.of(ss, cur), "sel", ""))},
    J.KV{"making", J.Json.bool(J.Json.get_bool(Hubs.of(ss, cur), "making", False{}))},
    J.KV{"lang", J.Json.get_obj(Hubs.of(ss, cur), "lang")},
    J.KV{"theme", J.Json.str(J.Json.get_str(Hubs.of(ss, cur), "theme", ""))},
    J.KV{"empty", J.Json.str(Hubs.empty.loc(lc, List.length(&2, J.Json, Hubs.projects(ss, False{})), List.length(&2, Shown, ss), J.Json.get_bool(Hubs.first_obj(ss, "search"), "open", False{})))},
    J.KV{"projects", J.Json.arr(Hubs.origin.projects(Hubs.projects(ss, Nat.is_gt(List.length(&2, Shown, ss), 1n)), ss, cur))},
    J.KV{"older", Hubs.older(ss, "older")},
    J.KV{"settled", Hubs.older(ss, "settled")},
    J.KV{"view", J.Json.str(J.Json.get_str(Hubs.of(ss, cur), "view", "projects"))},
    J.KV{"active", J.Json.arr(Hubs.active(ss))},
    J.KV{"legacy", J.Json.arr(Hubs.legacy.all(ss, Nat.is_gt(List.length(&2, Shown, ss), 1n)))},
    J.KV{"rowMenu", Hubs.first_obj(ss, "rowMenu")},
    J.KV{"deleting", Hubs.first_obj(ss, "deleting")},
    J.KV{"renaming", Hubs.first_obj(ss, "renaming")},
    J.KV{"removing", Hubs.first_obj(ss, "removing")},
    J.KV{"search", Hubs.first_obj(ss, "search")},
    J.KV{"folders", J.Json.get_obj(Hubs.of(ss, cur), "folders")},
    J.KV{"settings", J.Json.get_obj(Hubs.of(ss, cur), "settings")},
    J.KV{"tour", J.Json.get_obj(Hubs.of(ss, cur), "tour")},
    J.KV{"find", J.Json.get_obj(Hubs.of(ss, cur), "find")},
    J.KV{"island", Hubs.compact.go.loc(lc, ss, 0n, [], 0n)},
    J.KV{"islands", J.Json.arr(Hubs.islands(ss))},
    J.KV{"thread", Hubs.origin.focus(ss, cur, "thread")},
    J.KV{"uber", MPL.MUber.uber(pl)},
    J.KV{"bots", J.Json.arr(Hubs.bots(ss, list, Nat.is_gt(List.length(&2, Shown, ss), 1n)))},
    J.KV{"rooms", J.Json.arr(Hubs.rooms(ss, Nat.is_gt(List.length(&2, Shown, ss), 1n)))},
    J.KV{"newBot", J.Json.get_obj(Hubs.of(ss, cur), "newBot")},
    J.KV{"newRoom", J.Json.get_obj(Hubs.of(ss, cur), "newRoom")},
    J.KV{"bot", Hubs.origin.focus(ss, cur, "bot")},
    J.KV{"hub", J.Json.str(cur)},
    J.KV{"hubs", J.Json.arr(Hubs.rows(ss))},
    J.KV{"usageMachines", J.Json.arr(Hubs.usage.machines(ss))},
    J.KV{"found", J.Json.arr(Hubs.found(list))},
    J.KV{"tabs", Hubs.tabs(ss, list, Nat.is_gt(List.length(&2, Shown, ss), 1n))}]))

def Hubs.screen.p(+pl: MPL.MUber, +ss: List<&2, Shown>, +cur: String, +list: List<&2, Hub>) -> J.Json:
  Hubs.screen.p.loc(I.En{}, pl, ss, cur, list)

def Hubs.screen.k(+lc: I.Loc, +ss: List<&2, Shown>, +cur: String, +list: List<&2, Hub>, +want: Bool) -> J.Json:
  Hubs.screen.p.loc(lc, Hubs.pl.of(lc, ss, list, cur, Nat.is_gt(List.length(&2, Shown, ss), 1n), want), ss, cur, list)

# the whole screen: every hub's in one
def Hubs.screen.l(+lc: I.Loc, hs: Hubs) -> J.Json:
  match hs:
    case Hubs{_, _, +cur, +list}:
      Hubs.screen.k(lc, Hubs.relabel(Hubs.catalog.shown(Hubs.shown.w(lc, list, cur, Hubs.pl.want(list, cur)), list)), cur, list, Hubs.pl.want(list, cur))

def Hubs.screen(+hs: Hubs) -> J.Json:
  Hubs.screen.l(Hubs.loc(hs), hs)

# The screen in two halves, for the bridge (bridge.js keeps the first and
# builds it again only when what it is made of changed)
# -----------------------------------------------------------------------
# A phone with ten agents at work gets a frame every few milliseconds:
# streamed text, mostly for threads off the screen (no screen at all,
# Hubs.redraw), then the open thread's text and every change. The whole
# screen (every hub's list, the plane, the island, the tabs) took 80 ms
# with no JIT, so the bridge's one thread was always busy and taps waited.
#
# Hubs.lean is what the list half is made of: every client as it is but
# for its streamed text, which only the open thread shows (Mob.thread).
# Bend values never change in place, so when each part of Hubs.lean(a) is
# the very same value as in Hubs.lean(b) (bridge.js's same(), comparing
# the hubs, their list and clients field by field), Hubs.screen.fast of
# both is the same screen, and the bridge keeps the one it made.
# Hubs.opened is the thread field from the real clients, cheap (the open
# thread alone, its streamed reply rendered by the bridge a settled
# paragraph at a time: bridge.js's liveNodes); the screen is Hubs.screen.fast(Hubs.lean(hs)) with its
# "thread" made Hubs.opened(hs) (test/mhalves_test.bend; bench
# test/tools/phone_lean_bench.ts --verify compares them frame by frame).

def Hubs.lean.ui(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, _, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, Map.new(&2, String), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def Hubs.lean.go(hs: List<&2, Hub>) -> List<&2, Hub>:
  match hs:
    case Nil{}:
      Nil{}
    case Hub{k, u} <> t:
      Hub{k, Hubs.lean.ui(u)} <> Hubs.lean.go(t)

def Hubs.lean(hs: Hubs) -> Hubs:
  match hs:
    case Hubs{c, n, cur, list}:
      Hubs{c, n, cur, Hubs.lean.go(list)}

# (with one hub no row is another paired hub's mirror: Hubs.entity.dup
# asks for a hub other than the row's own, so the catalog drops nothing
# and its folds count nothing off; it walked every row and made the
# state's index twice more)
def Hubs.catalog.if(many: Bool, ss: List<&2, Shown>, +hs: List<&2, Hub>) -> List<&2, Shown>:
  match many:
    case True{}:
      Hubs.catalog.shown(ss, hs)
    case False{}:
      ss

def Hubs.screen.fast.l(+lc: I.Loc, hs: Hubs) -> J.Json:
  match hs:
    case Hubs{_, _, +cur, +list}:
      Hubs.screen.k(lc, Hubs.relabel(Hubs.catalog.if(Nat.is_gt(List.length(&2, Hub, list), 1n), Hubs.shown.w(lc, list, cur, Hubs.pl.want(list, cur)), list)), cur, list, Hubs.pl.want(list, cur))

def Hubs.screen.fast(+hs: Hubs) -> J.Json:
  Hubs.screen.fast.l(Hubs.loc(hs), hs)

def Hubs.opened.on(u: U.Ui) -> Bool:
  match u:
    case U.Ui{_, _, _, _, _, on, _, _, _, _, _, _, _, _, _, _, _}:
      on

# what Hubs.screen's "thread" reads of a hub's screen (Hubs.origin.focus):
# whether it is online, and for the hub in focus its open thread, its ids
# made the hub's (Q.all). The thread reads the frame's activity signals,
# as the whole screen's does (screen.bend's Mob.screen.ixr), or its phase
# says inactive while the whole screen says working or blocked.
def Hubs.opened.j(+lc: I.Loc, focus: Bool, +k: String, +u: U.Ui) -> J.Json:
  match focus:
    case True{}:
      J.Json.obj([J.KV{"online", J.Json.bool(Hubs.opened.on(u))}, J.KV{"thread", Q.all(S.Mob.focus.thread.r(lc, True{}, U.Side.with_signals(u), U.Ui.sel(u), False{}), k)}])
    case False{}:
      J.Json.obj([J.KV{"online", J.Json.bool(Hubs.opened.on(u))}])

def Hubs.opened.shown(+lc: I.Loc, hs: List<&2, Hub>, +cur: String) -> List<&2, Shown>:
  match hs:
    case Nil{}:
      Nil{}
    case Hub{+k, +u} <> t:
      Shown{k, Hubs.name(k, u), Hubs.opened.j(lc, String.eq(k, cur), k, u)} <> Hubs.opened.shown(lc, t, cur)

# the screen's "thread", but its streamed reply ("live": [], which
# bridge.js fills from Hubs.live.text with S.Mob.live.nodes)
def Hubs.opened.l(+lc: I.Loc, hs: Hubs) -> J.Json:
  match hs:
    case Hubs{_, _, +cur, +list}:
      Hubs.origin.focus(Hubs.relabel(Hubs.opened.shown(lc, list, cur)), cur, "thread")

def Hubs.opened(+hs: Hubs) -> J.Json:
  Hubs.opened.l(Hubs.loc(hs), hs)

# the streamed reply of the thread open on the hub in focus
def Hubs.live.text(+hs: Hubs) -> String:
  U.Ui.live.of(Hubs.ui(hs, Hubs.cur(hs)), U.Ui.sel(Hubs.ui(hs, Hubs.cur(hs))))

# a running thread t1 that ran a tool, then sent a message to t2 (filed
# under t1 too), and t2 that did something else since (for laws)
def Island.demo() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n},
    M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"t2", "p1", "y", M.Local{}, "", "", "claude", "", 1n},
    M.TurnChanged{"t1", M.Running{}, 2n}, M.ActivityLogged{"t1", "a1", "info", "tool", "ran ls", 3n},
    M.MessagePosted{"t2", M.Link.id("t1", "m1"), M.User{}, "hello", 4n}, M.ActivityLogged{"t2", "a2", "info", "edit", "wrote", 5n}], M.State.empty())

# A recent server presentation has no recipient-local Mailv cache. Advertise
# the supported native tab, visibly loading, until canonical replay finishes.
# Never synthesize mailbox data or enable writes from that presentation.
def Hubs.mail.loading.has(ts: List<&2, J.Json>) -> Bool:
  match ts:
    case Nil{}: False{}
    case +t <> r: Bool.or(String.eq(J.Json.get_str(t, "id", ""), "mail"), Hubs.mail.loading.has(r))

def Hubs.mail.loading.tabs.k(has: Bool, ts: List<&2, J.Json>, label: String) -> J.Json:
  match has:
    case True{}: J.Json.arr(ts)
    case False{}: J.Json.arr(List.append(&2, J.Json, ts, [J.Json.obj([J.KV{"id", J.Str{"mail"}}, J.KV{"label", J.Str{label}}])]))

def Hubs.mail.loading.tabs(+j: J.Json, label: String) -> J.Json:
  Hubs.mail.loading.tabs.k(Hubs.mail.loading.has(J.Json.elems(j)), J.Json.elems(j), label)

def Hubs.mail.loading.item(drop: Bool, tab: Bool, key: String, value: J.Json, tail: J.Json, label: String) -> J.Json:
  match drop tab:
    case True{} _: tail
    case False{} True{}: J.Field{key, Hubs.mail.loading.tabs(value, label), tail}
    case False{} False{}: J.Field{key, value, tail}

def Hubs.mail.loading.bot.fields(+label: String, fs: J.Json) -> J.Json:
  match fs:
    case J.Field{+key, value, tail}:
      Hubs.mail.loading.item(Bool.or(String.eq(key, "mailroom"), String.eq(key, "mailroomLoading")), String.eq(key, "tabs"), key, value, Hubs.mail.loading.bot.fields(label, tail), label)
    case x: x

def Hubs.mail.loading.bot.k(enabled: Bool, label: String, j: J.Json) -> J.Json:
  match enabled j:
    case True{} J.Obj{fields}:
      J.Obj{J.Field{"mailroomLoading", J.Flag{True{}}, J.Field{"mailroom", J.Null{}, Hubs.mail.loading.bot.fields(label, fields)}}}
    case _ x: x

def Hubs.mail.loading.bot(+u: U.Ui, +j: J.Json) -> J.Json:
  Hubs.mail.loading.bot.k(Bool.and(U.Mailv.enabled(u), Bool.and(String.eq(J.Json.get_str(j, "kind", ""), "bot"),
    Bool.not(FR.Fr.is(J.Json.get_str(j, "id", ""))))), MSG.T.native.mailroom.title(U.Lang.loc(u)), j)

def Hubs.mail.loading.child(bot: Bool, u: U.Ui, j: J.Json) -> J.Json:
  match bot:
    case True{}: Hubs.mail.loading.bot(u, j)
    case False{}: j

def Hubs.mail.loading.fields(+u: U.Ui, fs: J.Json) -> J.Json:
  match fs:
    case J.Field{+key, value, tail}:
      J.Field{key, Hubs.mail.loading.child(String.eq(key, "bot"), u, value), Hubs.mail.loading.fields(u, tail)}
    case x: x

def Hubs.mail.loading(+u: U.Ui, j: J.Json) -> J.Json:
  match j:
    case J.Obj{fields}: J.Obj{Hubs.mail.loading.fields(u, fields)}
    case x: x
