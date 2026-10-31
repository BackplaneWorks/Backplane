import Base
import ./browser-frame.bend as BFRAME
import ./view-page.bend as VP
import ./accounts.bend as ACCT
import ./subthreads.bend as ST
import ./fallback.bend as FB
import ./title.bend as TTL
import ./mailrun.bend as RUN
import ./cef.bend as CF
import ./ref.bend as R
import ./refview.bend as RE
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./kihealth.bend as KH
import ./kserve.bend as KSV
import ./kexport.bend as KEX
import ./deploy.bend as DP
import ./live.bend as LV
import ./json.bend as J
import ./model.bend as M
import ./signal.bend as SG
import ./usage.bend as UZ
import ./usage_page.bend as UP
import ./usage_graph.bend as UG
import ./codec.bend as C
import ./vt.bend as VT
import ./emb.bend as EM
import ./fuzzy.bend as FZ
import ./agent.bend as AG
import ./attach.bend as AT
import ./fold.bend as FO
import ./todo.bend as TD
import ./bot.bend as BT
import ./voice.bend as VC
import ./space.bend as SPC
import ./refs.bend as RF
import ./mech.bend as MC
import ./far.bend as FR
import ./fview.bend as FV
import ./farlive.bend as FLV
import ./desk.bend as DK
import ./notice.bend as NO
import ./wnote.bend as WN
import ./hist.bend as HI
import ./hier.bend as HR
import ./review.bend as RVW
import ./release.bend as REL
import ./links.bend as LK
import ./apikeys.bend as AK
import ./agy.bend as AGY
import ./diff.bend as DF
import ./fm.bend as FM
import ./onboard.bend as OB
import ./telemetry.bend as TLM
import ./layers.bend as LY
import ./slash.bend as SL
import ./uber.bend as UB
import ./ubersync.bend as US
import ./cron.bend as CR
import ./tz.bend as TZ
import ./when.bend as WH
import ./sched.bend as SC
import ./task.bend as TK
import ./order.bend as OR
import ./tools.bend as TO
import ./mailview.bend as MV
import ./mail.bend as ML
import ./thin-chat.bend as TC
import ./secret.bend as SEC
import ./modelpick.bend as MPK
import ./reply.bend as RTO

# The client state, Elm-style, shared by the native window and the web
# page: a host feeds server messages to Ui.recv and user actions to Ui.act,
# then draws the result. The read model is folded by the same State.apply
# the server uses (law replay_snoc), so what a client shows is exactly what
# the server stored.

type MailvIndex is Data:
  MailvIndex{wanted: Map<&2, ML.Order>, old: Map<&2, ML.Conv>, act: Map<&2, Nat>}

type MailvData is Data:
  MailvData{cache: MV.Cache, find: MV.Find, bubbles: List<&2, MV.Bub>, targets: List<&2, MV.Opt>, rev: Nat, touch: Nat, index: MailvIndex}

# Typed local stores share the old final Ui slot: ordinary Ui copies stay cheap.
# The mail record is behind a List to bound native record flattening.
type MailvLocal is Data:
  MailvLocal{terms: Map<&2, VT.Vt>, mail: List<&2, MailvData>}

def MailvLocal.empty() -> MailvLocal:
  MailvLocal{Map.new(&2, VT.Vt), Nil{}}

def MailvLocal.terms(s: MailvLocal) -> Map<&2, VT.Vt>:
  match s:
    case MailvLocal{t, _}: t

def MailvLocal.with_terms(s: MailvLocal, t: Map<&2, VT.Vt>) -> MailvLocal:
  match s:
    case MailvLocal{_, m}: MailvLocal{t, m}

type Ui is Data:
  Ui{
    st: M.State,
    sel: String,
    drafts: Map<&2, String>,
    live: Map<&2, String>,
    open: Map<&2, Bool>,
    online: Bool,
    now: Nat,
    adding: Bool,
    panel: Bool,
    version: String,
    rid: Nat,
    want_new: Bool,
    error: String,
    note: String,
    info: Map<&2, String>,
    outbox: Map<&2, String>,
    stores: MailvLocal
  }

type Cmd is Data:
  Send{text: String}
  Copy{text: String}
  Focus{id: String}
  ScrollEnd{}
  # talk to another machine's hub ("" is this machine's own)
  Connect{url: String}
  # a thread's draft as it now is: the host writes it down at once (a file,
  # local storage) so a crash loses nothing typed; "" forgets it
  Keep{thread: String, text: String}
  # a desktop notification the hub sent this desk (core/desk.bend): the
  # host shows it (a click opens the thread); `key` replaces an earlier one
  Notify{thread: String, title: String, body: String, key: String}
  # run an action after ms (the viewer's history playing, Hs.tick)
  Later{ms: Nat, action: String, value: String}
  # show an entry of the selected thread (the history's way back to it)
  Jump{id: String}

type Act is Data:
  Act{ui: Ui, cmds: List<&2, Cmd>}

# The machine marker for projects, sidebar rows and timeline headers.
# a thread's title as shown (title.bend; the laws read it here)
def Title.shown(l: I.Loc, +title: String) -> String:
  TTL.Title.shown(l, title)

def Label.title(+st: M.State, +id: String, +title: String) -> String:
  FR.Fr.title(st, id, title)

# a project elsewhere still catching up: listed, faded, "(syncing)"
def Label.syncing(+st: M.State, +id: String) -> Bool:
  FR.Fr.syncing(st, id)

# whether sync with a machine is off here ("sync.<name>" = "off", Mate.*
# below): nothing goes to it or comes from it, and what it shared before
# is not listed
def Mate.key(+name: String) -> String:
  "sync." ++ String.to_lower(name)

def Mate.off(+s: M.State, +name: String) -> Bool:
  String.eq(M.Setting.get(s, Mate.key(name), ""), "off")

def Mate.hidden(+s: M.State, +id: String) -> Bool:
  Bool.and(FR.Fr.is(id), Mate.off(s, FR.Fr.name(s, id)))

def Act.cmds(a: Act) -> List<&2, Cmd>:
  match a:
    case Act{_, cs}:
      cs

def Act.ui(a: Act) -> Ui:
  match a:
    case Act{u, _}:
      u

# (the clock goes in the info map from the start, as Ui.tick puts it)
def Ui.init(+now: Nat) -> Ui:
  Ui{M.State.empty(), "", Map.new(&2, String), Map.new(&2, String), Map.new(&2, Bool), False{}, now, False{}, False{}, "", 0n, False{}, "", "",
    Map.set(&2, String, Map.new(&2, String), "clock", Nat.show(now)), Map.new(&2, String), MailvLocal.empty()}

def Str.get.k(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

# Lookups
# -------

def Str.get(m: Map<&2, String>, +k: String) -> String:
  Str.get.k(Map.get(String, "", m, k))

def Ui.scratch(u: Ui, +k: String) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, k)

# the device language, read once per screen and passed to what builds text
def Lang.loc(u: Ui) -> I.Loc:
  I.I18n.of_tag(Ui.scratch(u, "@lang.tag"))

def Flag.get.k(r: Map<&2, Bool> & Bool) -> Bool:
  (_, v) = r
  v

def Flag.get(m: Map<&2, Bool>, +k: String) -> Bool:
  Flag.get.k(Map.get(Bool, False{}, m, k))

# ---- mail 2c: list filtering (Mailv belongs to spec 5w) ----
def Runs.add(hidden: Bool, +t: M.Thread, +rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match hidden:
    case True{}: rest
    case False{}: t <> rest

def Runs.keep(+ts: List<&2, M.Thread>, +s: M.State) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> +rest: Runs.add(RUN.Run.is(s, M.Thread.id(t)), t, Runs.keep(rest, s))

def Ui.threads(+s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Runs.keep(Map.values(&2, M.Thread, ts), s)

def Ui.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

def Ui.thread.k(r: Map<&2, M.Thread> & M.Thread) -> M.Thread:
  (_, t) = r
  t

def Ui.thread(s: M.State, +id: String) -> M.Thread:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Ui.thread.k(Map.get(M.Thread, M.Thread.blank(id), ts, id))

def Ui.entries(s: M.State) -> List<&2, M.Entry>:
  match s:
    case M.State{_, _, es, _, _, _, _, _, _, _, _}:
      es

# Time
# ----

# "now", "5m", "3h", "2d" (model.bend's, which folded turns use too)
def Time.ago(+now: Nat, +at: Nat) -> String:
  M.Time.ago(now, at)

def Ui.decode.k(m: Maybe<&2, M.Change>, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match m:
    case Some{c}:
      c <> rest
    case None{}:
      rest

# Receiving
# ---------

# tail-recursive, reversed once: a log holds thousands of changes, and a
# frame per change overflowed the phone's JavaScriptCore
def Ui.decode.go(xs: List<&2, J.Json>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Ui.decode.go(t, Ui.decode.k(C.Change.decode(x), acc))

def Ui.decode(xs: List<&2, J.Json>) -> List<&2, M.Change>:
  List.reverse(&2, M.Change, Ui.decode.go(xs, Nil{}))

# what a change means for the page beyond the read model: a finished
# reply ends its live text (a new thread is selected by the hub's answer
# to the request that made it, Made.reply, never by its arrival)
def Ui.see(u: Ui, c: M.Change) -> Ui:
  match u c:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} M.MessagePosted{+thread, _, M.Assistant{}, _, _}:
      Ui{st, sel, dr, Map.set(&2, String, live, thread, ""), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} M.TurnChanged{+thread, M.Completed{}, _}:
      Ui{st, sel, dr, Map.set(&2, String, live, thread, ""), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} M.TurnChanged{+thread, M.Failed{}, _}:
      Ui{st, sel, dr, Map.set(&2, String, live, thread, ""), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} M.TurnChanged{+thread, M.Interrupted{}, _}:
      Ui{st, sel, dr, Map.set(&2, String, live, thread, ""), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}
    case _ _:
      u

# Changes this client cannot read (a hub newer than it: a TestFlight build
# behind, a cached web page) still count: the hub resumes a reconnect by
# how many it sent, so each is a tick of the read model's sequence (and
# nothing else), or every reconnect would ask for the tail again and post
# its messages twice (laws ui_seq_counts_unknown, ui_seq_known_unchanged)
type Decoded is Data:
  Decoded{cs: List<&2, M.Change>, missed: Nat}

def Ui.decode.d(m: Maybe<&2, M.Change>, d: Decoded) -> Decoded:
  match m d:
    case Some{c} Decoded{cs, n}:
      Decoded{c <> cs, n}
    case None{} Decoded{cs, n}:
      Decoded{cs, 1n+n}

# (tail-recursive, reversed once, as Ui.decode)
def Ui.decode.all(xs: List<&2, J.Json>, d: Decoded) -> Decoded:
  match xs:
    case Nil{}:
      d
    case +x <> t:
      Ui.decode.all(t, Ui.decode.d(C.Change.decode(x), d))

def Ui.skip.st(n: Nat, st: M.State) -> M.State:
  match n:
    case 0n:
      st
    case 1n+k:
      Ui.skip.st(k, M.State.tick(st))

def Ui.skip(u: Ui, n: Nat) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{Ui.skip.st(n, st), sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# a "changes" message (or "log") holding these items (for laws)
def Seq.demo(+t: String, xs: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{t}}, J.KV{"items", J.Json.arr(xs)}])

# A provider with hundreds of models (opencode signed in to a few) would
# make the models info every client parses, often per frame, many times
# larger: its list and labels are kept apart, under "models.oc", and only
# its picker and Settings read them (Models.of). "models" keeps its first
# few, so it still counts as installed.
def Info.drop.k(skip: Bool, +k: String, v: J.Json, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match skip:
    case True{}:
      rest
    case False{}:
      J.KV{k, v} <> rest

def Info.drop.go(fs: J.Json, +key: String) -> List<&2, J.KV>:
  match fs:
    case J.Field{+k, v, t}:
      Info.drop.k(String.eq(k, key), k, v, Info.drop.go(t, key))
    case _:
      Nil{}

# an object's fields but `key`'s
def Info.drop(o: J.Json, +key: String) -> List<&2, J.KV>:
  match o:
    case J.Obj{fs}:
      Info.drop.go(fs, key)
    case _:
      Nil{}

def Info.drop.obj(+j: J.Json, +key: String) -> J.Json:
  J.Json.obj(Info.drop(j, key))

def Info.models.small.j(+j: J.Json) -> String:
  J.Json.show(J.Json.obj(List.append(&2, J.KV, Info.drop(Info.drop.obj(j, "about"), "opencode"),
    [J.KV{"opencode", J.Json.arr(List.take(&2, J.Json, J.Json.elems(J.Json.get_obj(j, "opencode")), 12n))},
     J.KV{"about", J.Json.obj(List.append(&2, J.KV, Info.drop(J.Json.get_obj(j, "about"), "opencode"),
       [J.KV{"opencode", J.Json.obj([J.KV{"cli", J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(j, "about"), "opencode"), "cli")}])}]))}])))

def Info.models.oc.j(+j: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"opencode", J.Json.get_obj(j, "opencode")},
    J.KV{"about", J.Json.obj([J.KV{"opencode", J.Json.get_obj(J.Json.get_obj(j, "about"), "opencode")}])}]))

# (and "models.v" counts the times: the window's memos key on it, not on
# the lists, tens of KB to join every frame; mm.bend's Mm.volatile)
def Info.put.models(+m: Map<&2, String>, +j: J.Json) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, m, "models", Info.models.small.j(j)), "models.oc", Info.models.oc.j(j)),
    "models.v", Nat.show(1n+Maybe.default(&2, Nat, Nat.read(Str.get(m, "models.v")), 0n)))

# (a models info that is not JSON is kept as it came)
def Info.put.m(j: Maybe<&2, J.Json>, +m: Map<&2, String>, +v: String) -> Map<&2, String>:
  match j:
    case Some{J.Obj{+fs}}:
      Info.put.models(m, J.Obj{fs})
    case _:
      Map.set(&2, String, m, "models", v)

# The models info (every provider's models, opencode's hundreds) is parsed
# and cut down only when it changed: the hub's views merge the info on
# every render, and parsing it again each time was most of a render's time
# (measured 2026-10-24: ~60% of the hub's CPU while a long thread scrolled).
# "models.raw" keeps the text the cut-down parts came from.
def Info.put.same(same: Bool, +m: Map<&2, String>, +v: String) -> Map<&2, String>:
  match same:
    case True{}:
      m
    case False{}:
      Map.set(&2, String, Info.put.m(J.Json.parse(v), m, v), "models.raw", v)

def Info.put.k(models: Bool, +m: Map<&2, String>, +k: String, +v: String) -> Map<&2, String>:
  match models:
    case True{}:
      Info.put.same(String.eq(Str.get(m, "models.raw"), v), m, v)
    case False{}:
      Map.set(&2, String, m, k, v)

def Info.put(m: Map<&2, String>, +k: String, +v: String) -> Map<&2, String>:
  Info.put.k(String.eq(k, "models"), m, k, v)

def Info.fields(fs: J.Json, m: Map<&2, String>) -> Map<&2, String>:
  match fs:
    case J.Field{+k, J.Str{+v}, t}:
      Info.fields(t, Info.put(m, k, v))
    case J.Field{_, _, t}:
      Info.fields(t, m)
    case _:
      m

# the machine's info (strings only) from the log message
def Info.of(o: J.Json) -> Map<&2, String>:
  match o:
    case J.Obj{fs}:
      Info.fields(fs, Map.new(&2, String))
    case _:
      Map.new(&2, String)

# the machine info changed while connected (new models): its fields replace
# the old ones, the rest stays
# Machines
# --------
# The owner's machines that run Backplane (the hub finds them on the
# tailnet). This client's own keys start with "local." and survive a new
# log: which machine it talks to ("" is its home hub) and the home hub's
# list, so the switcher stays the same while another machine is shown.

def Local.keep.go(ks: List<&2, String>, +old: Map<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Local.keep.go(t, old, Map.set(&2, String, m, k, Str.get(old, k)))

# (and the clock this client last ticked, which a folded turn's "5m ago"
# reads)
def Local.keep(+old: Map<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  Local.keep.go(["cid", "local.machine", "local.machines", "local.drafts", "local.hub", "local.token", "clock", "local.fixed", "local.generation", "local.state.hub", "local.state.gen", "local.remote.url", "local.remote.pre"], old, m)

# A reconnect to the same hub (the log's origin is the one this client
# had) keeps what the person chose in each thread's viewer: its kind, path
# and panel, whether they toggled it by hand, the diff, folds, the
# history step and comparison, the Files folder. Results asked again
# (refs:, fm.list:) and playback (hist.play:) are left out. Another
# machine's log keeps none of them (Local.keep runs on a switch too)
# (laws reset_keeps_viewer, reset_new_origin_drops_viewer)
def Reset.prefixes() -> List<&2, String>:
  ["subs:", "view.", "panel:", "panel.hand:", "diff:", "fold:", "hist.at:", "hist.back:", "hist.diff:", "cmp.a:", "cmp.b:", "cmp.al", "cmp.bl", "fm.want:", "fab."]

def Reset.hit(ps: List<&2, String>, +k: String, found: Bool) -> Bool:
  match ps found:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case p <> t False{}:
      Reset.hit(t, k, String.starts_with(k, p))

def Reset.one(hit: Bool, +k: String, +old: Map<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  match hit:
    case True{}:
      Map.set(&2, String, m, k, Str.get(old, k))
    case False{}:
      m

def Reset.copy(ks: List<&2, String>, +old: Map<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Reset.copy(t, old, Reset.one(Reset.hit(Reset.prefixes(), k, False{}), k, old, m))

def Ui.reset.keep(same: Bool, +old: Map<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  match same:
    case True{}:
      Reset.copy(Map.keys(&2, String, old), old, m)
    case False{}:
      m

# a log from this origin (for laws)
def Reset.demo(+origin: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"origin", J.Str{origin}}, J.KV{"items", J.Json.arr(Nil{})}])

def Machines.home.k(home: Bool, +nf: Map<&2, String>) -> Map<&2, String>:
  Bool.pick(Map<&2, String>, Bool.and(home, Bool.not(String.is_empty(Str.get(nf, "machines")))), Map.set(&2, String, nf, "local.machines", Str.get(nf, "machines")), nf)

# what the home hub says about machines becomes the switcher's list
def Machines.home(+nf: Map<&2, String>) -> Map<&2, String>:
  Machines.home.k(String.is_empty(Str.get(nf, "local.machine")), nf)

type MRow is Data:
  MRow{name: String, url: String, on: Bool}

def Machines.peer(skip: Bool, +n: String, +url: String, +cur: String, rest: List<&2, MRow>) -> List<&2, MRow>:
  match skip:
    case True{}:
      rest
    case False{}:
      MRow{n, url, String.eq(url, cur)} <> rest

def Machines.peers(ps: List<&2, J.Json>, +me: String, +cur: String) -> List<&2, MRow>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Machines.peer(String.eq(J.Json.get_str(p, "url", ""), me), J.Json.get_str(p, "name", ""), J.Json.get_str(p, "url", ""), cur, Machines.peers(t, me, cur))

def Machines.of(+l: I.Loc, +j: J.Json, +cur: String) -> List<&2, MRow>:
  MRow{Bool.pick(String, String.is_empty(J.Json.get_str(J.Json.get_obj(j, "self"), "name", "")), MSG.T.machines.self(l), J.Json.get_str(J.Json.get_obj(j, "self"), "name", "")), "", String.is_empty(cur)}
    <> Machines.peers(J.Json.elems(J.Json.get_obj(j, "peers")), J.Json.get_str(J.Json.get_obj(j, "self"), "url", ""), cur)

# the name of the machine shown
def Machines.current(rs: List<&2, MRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case MRow{+n, _, +on} <> t:
      Bool.pick(String, on, n, Machines.current(t))

# Exactly these device inputs and the resolved tag survive a hub switch.
def Lang.key(+k: String) -> Bool:
  Bool.or(String.eq(k, "@lang"), Bool.or(String.eq(k, "@lang.os"), Bool.or(String.eq(k, "@lang.tag"), String.eq(k, "@lang.preview"))))

def Machine.draft(scratch: Bool, +k: String, dr: Map<&2, String>, out: Map<&2, String>) -> Map<&2, String>:
  match scratch:
    case True{}:
      out
    case False{}:
      Map.set(&2, String, out, k, Str.get(dr, k))

# A transition discards every active scratch selector, live delta, viewer,
# terminal and outbox from the departing hub. Unsent thread drafts stay.
def Machine.drafts(ks: List<&2, String>, +dr: Map<&2, String>, out: Map<&2, String>) -> Map<&2, String>:
  match ks:
    case Nil{}:
      out
    case +k <> t:
      Machine.drafts(t, dr, Machine.draft(Bool.and(String.starts_with(k, "@"), Bool.not(Lang.key(k))), k, dr, out))

# The window assigns the actual connection tag when it opens the link.
# The transition itself also advances the generation, before any message
# can try to satisfy a selection against the old read model.
def Ui.machine(u: Ui, +url: String) -> Act:
  match u:
    case Ui{_, _, +dr, _, _, _, now, ad, _, _, rid, w, _, _, +nf, _, _}:
      Act{Ui{M.State.empty(), "", Machine.drafts(Map.keys(&2, String, dr), dr, Map.new(&2, String)), Map.new(&2, String), Map.new(&2, Bool), False{}, now, ad, False{}, "", rid, w, "", "connecting",
        Map.set(&2, String, Map.set(&2, String, Local.keep(nf, Map.new(&2, String)), "local.machine", url), "local.generation", "pending:" ++ Str.get(nf, "local.generation")), Map.new(&2, String), MailvLocal.empty()}, [Connect{url}]}

def Ui.info_merge.raw(u: Ui, o: J.Json) -> Ui:
  match u o:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} J.Obj{fs}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Machines.home(Info.fields(fs, nf)), ob, tr}
    case _ _:
      u

# A hub that let this client in without its pairing token (the legacy way,
# core/auth.bend) says so (info "auth.legacy"): the client asks the person,
# once a connection, to pair again (the error line every client shows and
# the person dismisses). Goes with the legacy way.
def Legacy.warn.k(warn: Bool, u: Ui, +text: String) -> Ui:
  match warn u:
    case True{} Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, _, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@auth.legacy", "1"), live, op, on, now, ad, pa, v, rid, w, text, note, nf, ob, tr}
    case False{} _:
      u

def Legacy.warn.at(+nf: Map<&2, String>, +dr: Map<&2, String>) -> Bool:
  Bool.and(String.eq(Str.get(nf, "auth.legacy"), "1"), Bool.not(String.eq(Str.get(dr, "@auth.legacy"), "1")))

def Legacy.warn(u: Ui) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Legacy.warn.k(Legacy.warn.at(nf, dr), Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, MSG.T.auth.legacy(I.I18n.of_tag(Str.get(dr, "@lang.tag"))))

def Ui.info_merge(u: Ui, o: J.Json) -> Ui:
  Legacy.warn(Ui.info_merge.raw(u, o))

def Ui.base(resume: Bool, st: M.State) -> M.State:
  match resume:
    case True{}:
      st
    case False{}:
      M.State.empty()

# how many changes this client holds (a reconnect asks for the rest)
# the log's read model handed over whole (the hub's own window: nothing
# to parse or replay)
def Todos.closed(done: Bool, info: Map<&2, String>, th: String) -> Map<&2, String>:
  match done:
    case False{}: info
    case True{}: Map.set(&2, String, info, "fold:todos:" ++ th, "0")

# The list shuts when it completes, and when it is retired (a list that
# was there is gone), so the agent's next list starts shut. Todos.see runs
# this per change for every thread; a whole state adopted at once
# (Ui.adopt.raw) checks only the selected thread.
def Todos.retired(+old: M.State, +fresh: M.State, +th: String) -> Bool:
  Bool.and(Bool.not(List.is_empty(&2, TD.Todo, M.Todos.of(old, th))), List.is_empty(&2, TD.Todo, M.Todos.of(fresh, th)))

def Todos.autoclose(+old: M.State, +fresh: M.State, info: Map<&2, String>, +th: String) -> Map<&2, String>:
  Todos.closed(Bool.or(Bool.and(Bool.not(TD.Todo.complete(M.Todos.of(old, th))), TD.Todo.complete(M.Todos.of(fresh, th))), Todos.retired(old, fresh, th)), info, th)

def Ui.adopt.raw(u: Ui, +st: M.State) -> Ui:
  match u:
    case Ui{+old, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Todos.autoclose(old, st, nf, sel), ob, tr}

def Ui.seq(u: Ui) -> Nat:
  match u:
    case Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, tr}:
      M.State.seq(st)

def Ui.live.put(r: Map<&2, String> & Map<&2, String>, u: Ui) -> Ui:
  (lv, nf) = r
  match u:
    case Ui{st, sel, dr, _, op, on, now, ad, pa, v, rid, w, er, note, _, ob, tr}:
      Ui{st, sel, dr, lv, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# a hub's snapshot of every thread's streamed text ({"t":"live"}, when this
# client is live again after a join): it replaces the live text, so a
# reconnect shows a reply once and whole (core/live.bend; laws live_*)
def Ui.live.set(u: Ui, +o: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, +live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui.live.put(LV.Live.set(o, live, nf), Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr})

# Splitting a streamed reply (laws live_prefix_exact, live_split_fence_safe)
# -------------------------------------------------------------------------
# The markdown parser ends every block at a blank line (Md.bstep) except
# inside a fenced code block, so the text up to the last blank line outside
# a fence renders to exactly the nodes it renders to alone: it is settled,
# and a client lays it out once and keeps it, laying out only what follows.

def Live.fenced(+l: String) -> Bool:
  String.starts_with(String.trim_start(l), "```")

def Live.blank(+l: String) -> Bool:
  String.is_empty(String.trim(l))

# a fence line flips whether the next lines are inside code
def Live.flip(fl: Bool, fence: Bool) -> Bool:
  match fl fence:
    case True{} True{}:
      False{}
    case True{} False{}:
      True{}
    case False{} _:
      fence

# a blank line outside code ends the settled text at `end`
def Live.split.best(bl: Bool, fence: Bool, +end: Nat, +best: Nat) -> Nat:
  match bl fence:
    case True{} False{}:
      end
    case _ _:
      best

def Live.split.go(ls: List<&2, String>, +fence: Bool, +off: Nat, +best: Nat) -> Nat:
  match ls:
    case Nil{}:
      best
    case +l <> t:
      Live.split.go(t, Live.flip(Live.fenced(l), fence), Nat.add(off, Nat.add(String.length(l), 1n)),
        Live.split.best(Live.blank(l), fence, Nat.add(off, Nat.add(String.length(l), 1n)), best))

# the characters of the settled prefix (0: none yet)
def Live.at(+text: String) -> Nat:
  Live.split.go(DF.Diff.lines(text), False{}, 0n, 0n)

# the first n characters, tail-recursive (String.take is not, and a reply
# runs to hundreds of kilobytes)
def Live.take(+n: Nat, s: String, acc: String) -> String:
  match n s:
    case 0n _:
      String.reverse(acc)
    case 1n+k SNil{}:
      String.reverse(acc)
    case 1n+k SCon{c, t}:
      Live.take(k, t, SCon{c, acc})

def Live.head.k(+text: String, +n: Nat) -> String:
  Live.take(n, text, SNil{})

def Live.rest.k(+text: String, +n: Nat) -> String:
  String.drop(text, n)

# the settled prefix and the text after it: head ++ rest is the text
def Live.head(+text: String) -> String:
  Live.head.k(text, Live.at(text))

def Live.rest(+text: String) -> String:
  Live.rest.k(text, Live.at(text))

# the live text a thread shows
def Ui.live.of(u: Ui, +thread: String) -> String:
  match u:
    case Ui{_, _, _, live, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(live, thread)

def Ui.delta(u: Ui, +thread: String, text: String) -> Ui:
  match u:
    case Ui{st, sel, dr, +live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, Map.set(&2, String, live, thread, Str.get(live, thread) ++ text), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# a failed request shows its error, or the text of a failed git step
def Ui.reply.err.std(+o: J.Json) -> String:
  Bool.pick(String, J.Json.get_bool(o, "ok", True{}), "", J.Json.get_str(o, "error", J.Json.get_str(J.Json.get_obj(o, "result"), "error", String.take(J.Json.get_str(J.Json.get_obj(o, "result"), "text", "failed"), 200n))))

# an answer: its request leaves the outbox
def Ui.reply.err(+o: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(J.Json.get_obj(o, "result"), "kicad", "")), Ui.reply.err.std(o), "")

def Ui.reply.k(u: Ui, +o: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, _, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, Ui.reply.err(o), note, nf, Map.set(&2, String, ob, J.Json.as_raw(J.Json.get(o, "id"), ""), ""), tr}

# The viewer belongs to a thread: what it shows for thread t is kept as
# "view.kind:t" and "view.path:t" (a file an agent asked for, else the
# thread's own board), its diff as "diff:t", and "view.n:t" counts the
# changes, so the same file asked for twice still comes to the front.

def Viewer.n(+nf: Map<&2, String>, +th: String) -> String:
  Nat.show(1n+Maybe.default(&2, Nat, Nat.read(Str.get(nf, "view.n:" ++ th)), 0n))

# Drawing choices outlive tabs whose path names a report or a folder.
# A mechanical part is not a board choice.
def Viewer.design(+kind: String, +path: String) -> Bool:
  Bool.or(Bool.or(String.eq(kind, "board"), String.eq(kind, "schematic")),
    Bool.or(String.eq(kind, "split"), Bool.and(String.eq(kind, "3d"), Bool.or(String.is_empty(path), String.ends_with(path, ".kicad_pcb")))))

def Viewer.board_changed(+nf: Map<&2, String>, +th: String, +path: String) -> Bool:
  +old = HI.Hist.family(Str.get(nf, "view.design:" ++ th))
  +file = HI.Hist.family(path)
  Bool.and(String.ends_with(file, ".kicad_pcb"), Bool.and(String.ends_with(old, ".kicad_pcb"), Bool.not(String.eq(old, file))))

def Viewer.remember(drawing: Bool, schematic: Bool, +nf: Map<&2, String>, +th: String, +path: String) -> Map<&2, String>:
  match drawing schematic:
    case True{} True{}:
      +board = Str.get(nf, "view.design:" ++ th)
      Map.set(&2, String, Map.set(&2, String, nf, "view.design:" ++ th,
        Bool.pick(String, String.ends_with(board, ".kicad_pcb"), board, HI.Hist.family(path))), "view.sheet:" ++ th, HI.Hist.family(path))
    case True{} False{}:
      +kept = Bool.pick(Map<&2, String>, Viewer.board_changed(nf, th, path), Map.set(&2, String, Map.set(&2, String, nf, "view.tree:" ++ th, ""), "view.sheet:" ++ th, ""), nf)
      Map.set(&2, String, kept, "view.design:" ++ th, HI.Hist.family(path))
    case False{} _: nf

def Viewer.put(+nf: Map<&2, String>, +th: String, +kind: String, +path: String) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Viewer.remember(Viewer.design(kind, path), String.eq(kind, "schematic"), nf, th, path), "view.kind:" ++ th, kind), "view.path:" ++ th, path), "view.n:" ++ th, Viewer.n(nf, th))

# the thread a view message is for: named, else the selected one (a hub
# from before views were per thread names none)
def Viewer.of(+th: String, +sel: String) -> String:
  Bool.pick(String, String.is_empty(th), sel, th)

# Each thread's panel is open or shut on its own: "panel:t" is "1" while
# thread t's is open. The panel field is the selected thread's; it is put
# away and the next thread's brought back whenever the selection changes.

def Viewer.flag(b: Bool) -> String:
  match b:
    case True{}:
      "1"
    case False{}:
      ""

# (and a thread whose viewer is on Mech has its parts listed again when it
# is chosen, as when Mech is opened: "@mech.at" forgets the last asked,
# Mpage.sync; going back to a thread used to show the parts it had)
def Ui.reselect.go(u: Ui, +old: String) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@mech.at", ""), live, op, on, now, ad, String.eq(Str.get(nf, "panel:" ++ sel), "1"), v, rid, w, er, note,
        Map.set(&2, String, nf, "panel:" ++ old, Viewer.flag(pa)), ob, tr}

def Ui.reselect.k(same: Bool, u: Ui, +old: String) -> Ui:
  match same:
    case True{}:
      u
    case False{}:
      Ui.reselect.go(u, old)

# whether the viewer panel is open
def Ui.panel(u: Ui) -> Bool:
  match u:
    case Ui{_, _, _, _, _, _, _, _, pa, _, _, _, _, _, nf, ob, tr}:
      pa

# the viewer switched to another kind by hand, for the selected thread
def Ui.view_pick(u: Ui, +kind: String) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@revert", ""), live, op, on, now, ad, pa, v, rid, w, er, note, Viewer.put(nf, sel, kind, ""), ob, tr}

# A diff is kept as "diff:t", and "diff.none:t" names the folder when
# thread t's is no git repository (the hub's "nogit": nothing to compare).
# An open Diff follows its thread (Dv.sync): "diff.at:t" is the thread's
# turn and step count the diff on show was asked at (Dv.at), and an answer
# to Diff's own ask ("diff.auto:t" "1") only puts the text in place,
# leaving the panel as the person has it by then.
def Dv.at(+st: M.State, +th: String) -> String:
  C.Turn.name(M.Thread.turn(M.State.thread.get(st, th))) ++ "|" ++ Nat.show(List.length(&2, M.Entry, M.State.steps(st, th)))

def Dv.text(+nf: Map<&2, String>, +th: String, +r: J.Json) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, nf, "diff:" ++ th, J.Json.get_str(r, "diff", "")), "diff.none:" ++ th, J.Json.get_str(r, "nogit", ""))

# a diff asked for by hand opens its thread's viewer on it (on screen when
# that thread is)
def Ui.diff.k(u: Ui, +th: String, +r: J.Json) -> Ui:
  match u:
    case Ui{+st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, Bool.or(pa, String.eq(th, sel)), v, rid, w, er, note,
        Viewer.put(Map.set(&2, String, Map.set(&2, String, Dv.text(nf, th, r), "diff.at:" ++ th, Dv.at(st, th)), "panel:" ++ th, "1"), th, "diff", ""), ob, tr}

def Ui.diff.quiet(u: Ui, +th: String, +r: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Map.set(&2, String, Dv.text(nf, th, r), "diff.auto:" ++ th, ""), ob, tr}

def Ui.diff.by(auto: Bool, u: Ui, +th: String, +r: J.Json) -> Ui:
  match auto:
    case True{}:
      Ui.diff.quiet(u, th, r)
    case False{}:
      Ui.diff.k(u, th, r)

def Files.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{x} <> t:
      x <> Files.strs(t)
    case _ <> t:
      Files.strs(t)

def Ui.files(u: Ui, +r: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note,
        Map.set(&2, String, Map.set(&2, String, nf, "files", String.join(Files.strs(J.Json.elems(J.Json.get_obj(r, "files"))), "\n")), "files.thread", J.Json.get_str(r, "thread", "")), ob, tr}

def Ui.reply.files(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.files(u, J.Json.get_obj(o, "result"))
    case False{}:
      u

def Skills.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "skills"), J.Null{})), "null"))

def Ui.update.note(+ver: String, +state: String) -> String:
  Bool.pick(String, String.is_empty(state), Bool.pick(String, String.is_empty(ver), "", "update " ++ ver), state)

# the settings' version row: a packaged install (info "update" is
# "package") updates through its package manager and offers no buttons
def Update.packaged(+nf: Map<&2, String>) -> Bool:
  String.eq(Str.get(nf, "update"), "package")

# the hub's pairing link (the tailnet address and the token), which a
# phone or a browser pastes; Settings shows it with Copy
def Pair.link(+nf: Map<&2, String>) -> String:
  Str.get(nf, "pair")

# Tailscale in Settings, for every client. The hub says what it found (info
# "tailscale": off for --no-tailscale, checking, unavailable when Tailscale
# gave no address or the bind failed, reachable when the hub answers on its
# tailnet address, even without a pairing link of its own, as with
# --host 0.0.0.0). A hub too old to say ("tailscale" missing) is read the old
# way: up only once it has a pairing link ("pair").
def Tail.told(+nf: Map<&2, String>) -> String:
  Bool.pick(String, String.is_empty(Str.get(nf, "tailscale")),
    Bool.pick(String, String.eq(Str.get(nf, "tailnet"), "off"), "off", Bool.pick(String, String.is_empty(Pair.link(nf)), "unavailable", "reachable")),
    Str.get(nf, "tailscale"))

def Tail.state.of(+l: I.Loc, +t: String, +nf: Map<&2, String>) -> String:
  Bool.pick(String, String.eq(t, "reachable"), Str.get(nf, "tailnet"), Bool.pick(String, String.eq(t, "unavailable"), MSG.T.settings.tailscale.notconnected(l), t))

def Tail.state(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Tail.state.of(l, Tail.told(nf), nf)

def Tail.label(+l: I.Loc, +nf: Map<&2, String>) -> String:
  MSG.T.settings.tailscale.label(l, Tail.state(l, nf))

def Tail.note.of(+l: I.Loc, +t: String, +nf: Map<&2, String>) -> String:
  Bool.pick(String, String.eq(t, "off"), MSG.T.settings.tailscale.off.note(l, Str.get(nf, "listen")),
    Bool.pick(String, String.eq(t, "reachable"), MSG.T.settings.tailscale.reachable.note(l),
      Bool.pick(String, String.eq(t, "unavailable"), MSG.T.settings.tailscale.unavailable.note(l, Str.get(nf, "listen")),
        MSG.T.settings.tailscale.looking(l))))

def Tail.note(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Tail.note.of(l, Tail.told(nf), nf)

def Pair.note(+l: I.Loc, +link: String) -> String:
  Bool.pick(String, String.is_empty(link), MSG.T.settings.pair.none(l), link)

def Pair.rows(+s: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(s), Nil{}, String.split(s, '\n'))

# the pairing link as a QR code (info "pair.qr": its rows, '1' dark), for
# a phone's camera; the flag "pairqr" shows it in Settings
def Pair.qr(+nf: Map<&2, String>) -> List<&2, String>:
  Pair.rows(Str.get(nf, "pair.qr"))

def Pair.qr.ready(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "pair.qr")))

def Pair.qr.note(+l: I.Loc) -> String:
  MSG.T.settings.pair.qr(l)

# KiCad itself: the hub's kicad-cli (info "kicad.cli", "" when there is
# none on the machine), which ERC, DRC, exports and the 3D model need
def KiCad.found(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "kicad.cli")))

def KiCad.cli.label.loc(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.found(nf), "KiCad CLI", MSG.T.settings.kicad.cli.missing(l))

def KiCad.cli.label(nf: Map<&2, String>) -> String:
  KiCad.cli.label.loc(I.En{}, nf)

def KiCad.cli.note(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.found(nf), Str.get(nf, "kicad.cli"), MSG.T.settings.kicad.cli.install(l))

# One health notice model shared by every surface and client.
# a headless server that stopped for good is a notice too, once the runtime
# itself has none
def KN.ipc(n: Maybe<&2, KH.KNote>, +servers: String) -> Maybe<&2, KH.KNote>:
  match n:
    case Some{x}: Some{x}
    case None{}: KH.KH.ipc.notice(KSV.KS.failed.project(servers), KSV.KS.name(KSV.KS.failed.project(servers)), KSV.KS.failed.why(servers))

def KN.of(+info: Map<&2, String>, +st: M.State) -> Maybe<&2, KH.KNote>:
  KN.ipc(KH.KH.notice(Str.get(info, "kicad.status"), Str.get(info, "kicad.cli"), Str.get(info, "kicad.version"), Str.get(info, "kicad.why"), String.eq(Str.get(info, "kicad.can_install"), "1")), Str.get(info, "kicad.ipc.servers"))

def KN.visible.k(n: Maybe<&2, KH.KNote>, +st: M.State) -> Maybe<&2, KH.KNote>:
  match n:
    case Some{KH.KNote{+key, title, body, acts}}:
      Bool.pick(Maybe<&2, KH.KNote>, KH.KH.dismissed(st, key), None{}, Some{KH.KNote{key, title, body, acts}})
    case None{}: None{}

def KN.visible(+info: Map<&2, String>, +st: M.State) -> Maybe<&2, KH.KNote>:
  KN.visible.k(KN.of(info, st), st)

def KN.thread.k(shown: Bool, +info: Map<&2, String>, +st: M.State) -> Maybe<&2, KH.KNote>:
  match shown:
    case True{}: KN.visible(info, st)
    case False{}: None{}

def KN.thread(+info: Map<&2, String>, +st: M.State, +sel: String) -> Maybe<&2, KH.KNote>:
  KN.thread.k(String.eq(Str.get(info, "kn:" ++ sel), "1"), info, st)

def KN.action(+a: String) -> String:
  Bool.pick(String, String.eq(a, "link"), "open-url", a)

def KN.summary.k(+status: String, +v: String, +api: String, +source: String) -> String:
  match status:
    case "ok": "Backplane KiCad " ++ v ++ ", API " ++ api ++ ", from " ++ source
    case "stock": "KiCad " ++ v ++ ", from " ++ source ++ " · stock KiCad"
    case "old": "Backplane KiCad " ++ v ++ ", API " ++ api ++ ", from " ++ source ++ " · update needed"
    case "broken": "KiCad from " ++ source ++ " · doesn't run"
    case "missing": "KiCad isn't installed · from " ++ source
    case _: "Checking KiCad"

def KN.summary(+nf: Map<&2, String>) -> String:
  KN.summary.k(Str.get(nf, "kicad.status"), Str.get(nf, "kicad.version"), Str.get(nf, "kicad.api"), Str.get(nf, "kicad.source"))

# FreeCAD, which agents model parts with (info "freecad": its command on
# the hub's machine, "" when there is none); Backplane never bundles it
def FreeCAD.found(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "freecad")))

def FreeCAD.label.loc(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Bool.pick(String, FreeCAD.found(nf), "FreeCAD", MSG.T.settings.freecad.missing(l))

def FreeCAD.label(nf: Map<&2, String>) -> String:
  FreeCAD.label.loc(I.En{}, nf)

def FreeCAD.note(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Bool.pick(String, FreeCAD.found(nf), Str.get(nf, "freecad"), MSG.T.settings.freecad.install(l))

# KiCad IPC is the Backplane fork's api-server: without the fork the
# setting has nothing to turn on, so Settings says where to get it instead
# of offering it (info "kicad.fork": the fork's version, "" for none)
def KiCad.ipc.ready(+nf: Map<&2, String>) -> Bool:
  String.eq(Str.get(nf, "kicad.status"), "ok")

def KiCad.ipc.label(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.ipc.ready(nf), "KiCad IPC", MSG.T.settings.kicad.ipc.missing(l))

# with Backplane KiCad: what the hub's headless server for the project is
# doing ("KiCad server: ready for <name>", or why it is not), else what
# the setting does
def KiCad.ipc.on(+l: I.Loc, +servers: String) -> String:
  Bool.pick(String, String.is_empty(servers), MSG.T.settings.kicad.ipc.on(l), KSV.KS.note(servers))

def KiCad.ipc.note(+l: I.Loc, +nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.ipc.ready(nf), KiCad.ipc.on(l, Str.get(nf, "kicad.ipc.servers")),
    MSG.T.settings.kicad.ipc.install(l))

# the stored note ("update <version>" offers one; the hub's own progress
# words stay as it sent them) as shown
def Update.note.shown(+l: I.Loc, +note: String) -> String:
  Bool.pick(String, String.starts_with(note, "update "), MSG.T.settings.update.available(l, String.drop(note, 7n)), note)

def Update.line(+l: I.Loc, +nf: Map<&2, String>, +note: String) -> String:
  Bool.pick(String, Update.packaged(nf), MSG.T.settings.update.packaged(l), Bool.pick(String, String.is_empty(note), MSG.T.settings.update.current(l), Update.note.shown(l, note)))

# an update the hub holds back while threads work (hub.bend's Update.*):
# how many it waits for (0: none, or not waiting)
def Update.waits(+st: M.State) -> Nat:
  Bool.pick(Nat, Bool.and(String.is_empty(M.Setting.get(st, "update.wait", "")), String.is_empty(M.Setting.get(st, "update.restart", ""))), 0n, M.Work.threads(st))

def Update.waits.line(+l: I.Loc, +n: Nat, +nf: Map<&2, String>, +note: String) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), Update.line(l, nf, note), MSG.T.settings.update.waits(l, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other")))

# a deploy waiting for a safe point (core/deploy.bend) shows in its place
def Update.line.d(+l: I.Loc, +row: String, +n: Nat, +nf: Map<&2, String>, +note: String) -> String:
  Bool.pick(String, String.is_empty(row), Update.waits.line(l, n, nf, note), row)

# the version row's line, with the wait
def Update.line.w(+l: I.Loc, +st: M.State, +nf: Map<&2, String>, +note: String) -> String:
  Update.line.d(l, DP.Deploy.row(st), Update.waits(st), nf, note)

# which button the version row shows: 0 none (a packaged install),
# 1 install an update offered, 2 check now, 3 update now (one waiting)
def Update.btn(+st: M.State, +nf: Map<&2, String>, +note: String) -> Nat:
  Bool.pick(Nat, Update.packaged(nf), 0n, Bool.pick(Nat, Nat.is_gt(Update.waits(st), 0n), 3n, Bool.pick(Nat, String.starts_with(note, "update "), 1n, 2n)))

# an update notice: a version to offer, or the progress of one being applied
def Ui.update(u: Ui, +o: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, _, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w,
        J.Json.get_str(o, "error", ""), Ui.update.note(J.Json.get_str(o, "version", ""), J.Json.get_str(o, "state", "")), nf, ob, tr}

# the kind an agent's viewer_show takes: a part shown on Mech ("mech" and
# a model) opens in 3D, where Mech shows its parts
def Viewer.kind.of(+kind: String, +path: String) -> String:
  Bool.pick(String, Bool.and(String.eq(kind, "mech"), MC.Mech.is_model(path)), "3d", kind)

# an agent asked to show a file: its thread's viewer shows it, and opens
# when that thread is the one on screen (another thread's request waits,
# open, for the user to go there)
def Ui.view.k(u: Ui, +th: String, +o: J.Json) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, +pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, Bool.or(pa, String.eq(th, sel)), v, rid, w, er, note,
        Viewer.put(Map.set(&2, String, nf, "panel:" ++ th, "1"), th, Viewer.kind.of(J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "path", "")), J.Json.get_str(o, "path", "")), ob, tr}

def Out.resend.k(skip: Bool, x: String, rest: List<&2, Cmd>) -> List<&2, Cmd>:
  match skip:
    case True{}:
      rest
    case False{}:
      Send{x} <> rest

def Out.resend(xs: List<&2, String>) -> List<&2, Cmd>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Out.resend.k(String.is_empty(x), x, Out.resend(t))

# what the outbox still holds, to send again
def Ui.outbox(u: Ui) -> List<&2, Cmd>:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, ob, tr}:
      Out.resend(Map.values(&2, String, ob))

def Ui.recv_act.k(connected: Bool, +u: Ui) -> Act:
  Act{u, Bool.pick(List<&2, Cmd>, connected, Ui.outbox(u), Nil{})}

# the client's own id (a host sets it once at start)
def Ui.set_cid(u: Ui, +cid: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Map.set(&2, String, nf, "cid", cid), ob, tr}

def Pending.stored(es: List<&2, M.Entry>, +id: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case M.Msg{i, _, _, _, _} <> t:
      Bool.or(String.eq(i, id), Pending.stored(t, id))
    case M.Act{_, _, _, _, _, _} <> t:
      Pending.stored(t, id)

def Pending.one.k(+p: J.Json, +thread: String, +es: List<&2, M.Entry>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.and(String.eq(J.Json.get_str(p, "thread", ""), thread), Bool.not(Pending.stored(es, J.Json.get_str(p, "msg", "")))),
    [J.Json.get_str(p, "text", "")], Nil{})

# texts sent to a thread and not yet stored by the server, oldest first
def Pending.one(+x: String, +thread: String, +es: List<&2, M.Entry>) -> List<&2, String>:
  Pending.one.k(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(x), J.Null{}), "p"), thread, es)

def Pending.go(xs: List<&2, String>, +thread: String, +es: List<&2, M.Entry>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      List.append(&2, String, Bool.pick(List<&2, String>, String.is_empty(x), Nil{}, Pending.one(x, thread, es)), Pending.go(t, thread, es))

def Ui.pending(u: Ui, +thread: String) -> List<&2, String>:
  match u:
    case Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, ob, tr}:
      Pending.go(Map.values(&2, String, ob), thread, M.State.mine(st, thread))

# (a new connection has answered nothing: what the Mechanical page and a
# part in 3D asked for is asked again)
def Ui.online(u: Ui, b: Bool) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, _, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, dr, "@mech.at", ""), "@solid.at", ""), "@desk", ""), "@sync", ""), live, op, b, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# the clock goes in the info map too, where a thread's folded turns read
# it ("5m ago", FO.Fold.roles)
def Ui.tick(u: Ui, +now: Nat) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, _, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Map.set(&2, String, nf, "clock", Nat.show(now)), ob, tr}

# Acting
# ------

def Rpc.text(+rid: Nat, m: String, p: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Num{Nat.show(rid)}}, J.KV{"m", J.Str{m}}, J.KV{"p", J.Json.obj(p)}]))

# send method m with params p, counting the request
# an rpc that must reach the server: kept in the outbox, sent again on
# every reconnect, until the server answers it
def Ui.rpc.keep(u: Ui, +m: String, +p: List<&2, J.KV>) -> Act:
  match u:
    case Ui{+st, sel, dr, live, op, on, now, ad, pa, v, +rid, w, er, note, nf, +ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, pa, v, 1n+rid, w, er, note, nf, Map.set(&2, String, ob, Nat.show(rid), Rpc.text(rid, m, ACCT.Accounts.rpc.params(st, p))), tr}, [Send{Rpc.text(rid, m, ACCT.Accounts.rpc.params(st, p))}]}

def Ui.rpc(u: Ui, m: String, p: List<&2, J.KV>) -> Act:
  match u:
    case Ui{+st, sel, dr, live, op, on, now, ad, pa, v, +rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, pa, v, 1n+rid, w, er, note, nf, ob, tr}, [Send{Rpc.text(rid, m, ACCT.Accounts.rpc.params(st, p))}]}

# a request about the selected thread, with more params
def Ui.on_thread.p(u: Ui, m: String, p: List<&2, J.KV>) -> Act:
  match u:
    case Ui{+st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, m, J.KV{"thread", J.Str{sel}} <> p)

def Ui.on_thread(u: Ui, m: String) -> Act:
  match u:
    case Ui{+st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, m, [J.KV{"thread", J.Str{sel}}])

def Ui.select.k(u: Ui, +id: String) -> Ui:
  match u:
    case Ui{st, _, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, id, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, dr, "@revert", ""), "@del", ""), "@bot", ""), "@room", ""), "@remote", ""), "@bot.want", ""), "@thread.want", ""), "@intent", ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# Drafts kept
# -----------
# Every change to a thread's draft goes out as a Keep at once, so the host
# can write it down before anything else can go wrong. Only names a file
# can take safely are kept: thread ids, never scratch ("@") keys.

def Draft.safe.c(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))),
    Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.or(U32.is_eq(c, 126), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)))))

def Draft.safe.go(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{c}, t}:
      Bool.and(Draft.safe.c(c), Draft.safe.go(t))

# a name made only of letters, digits, '-', '_' and far's '~' separator
def Draft.safe(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{c, t}:
      Draft.safe.go(SCon{c, t})

def Draft.keep.k(ok: Bool, id: String, text: String) -> List<&2, Cmd>:
  match ok:
    case True{}:
      [Keep{id, text}]
    case False{}:
      Nil{}

# what a draft change asks the host to write
def Draft.keep(+id: String, text: String) -> List<&2, Cmd>:
  Draft.keep.k(Draft.safe(id), id, text)

# a draft read back after a restart: what is typed now wins
def Draft.pick(now: String, saved: String) -> String:
  match now:
    case SNil{}:
      saved
    case SCon{c, t}:
      SCon{c, t}

def Ui.draft.set(u: Ui, +text: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, Map.set(&2, String, dr, sel, text), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, Draft.keep(sel, text)}

# a kept draft, read back by the host when it starts; what was read is
# marked ("@kept:<thread>") so Draft.stale can tell it was never touched
def Ui.restore(u: Ui, +id: String, +saved: String) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, id, Draft.pick(Str.get(dr, id), saved)), "@kept:" ++ id, saved), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# Sending empties the draft in the same step that sends it, and that step
# also tells the host to forget its kept copy (a Keep of ""), so text that
# went out is never read back (laws draft_clear_*). Every way a draft is
# sent goes through Draft.clear: a message, a bot's (bots.say), /btw, /give.
# Sending ordinary text ends an abandoned intent edit too. Threads that
# never borrowed the composer need no intent scratch entries.
def Draft.intent.clear(editing: Bool, +sel: String, dr: Map<&2, String>) -> Map<&2, String>:
  match editing:
    case False{}: dr
    case True{}:
      Map.set(&2, String, Map.set(&2, String, dr, "@intent.draft:" ++ sel, ""), "@intent.edit:" ++ sel, "")

def Draft.clear(u: Ui) -> Act:
  match u:
    case Ui{st, +sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, Map.set(&2, String, Draft.intent.clear(String.eq(Str.get(dr, "@intent.edit:" ++ sel), "1"), sel, dr), sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, Draft.keep(sel, "")}

# An intent edit's borrowed draft and its mark are kept by the page as
# drafts of their own (Intent.kept); Save, or an ordinary message sent over
# an abandoned edit, has the page forget them too
def Draft.intent.forget(editing: Bool, +sel: String) -> List<&2, Cmd>:
  match editing:
    case False{}: Nil{}
    case True{}: [Keep{"@intent.draft:" ++ sel, ""}, Keep{"@intent.edit:" ++ sel, ""}]

def Draft.sent.f(u: Ui) -> List<&2, Cmd>:
  match u:
    case Ui{_, +sel, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Draft.intent.forget(String.eq(Str.get(dr, "@intent.edit:" ++ sel), "1"), sel)

# the same with the thread's attachments, which went out with it
def Draft.sent.k(a: Act, more: List<&2, Cmd>) -> Act:
  match a:
    case Act{Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, cs}:
      Act{Ui{st, sel, Map.set(&2, String, dr, "@att:" ++ sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, List.append(&2, Cmd, cs, more)}

def Draft.sent(+u: Ui) -> Act:
  Draft.sent.k(Draft.clear(u), Draft.sent.f(u))

# the commands of a step that came first, before those of the next
def Ui.first(cs: List<&2, Cmd>, a: Act) -> Act:
  match a:
    case Act{u, more}:
      Act{u, List.append(&2, Cmd, cs, more)}

# A draft read back from before this fix may be text that was sent (a
# bot's thread once kept it after sending). When its thread is opened and
# the draft is still exactly what was read back, and the thread's newest
# message from the person is that text, it is forgotten (laws draft_stale_*).

def Draft.user(r: M.Role) -> Bool:
  match r:
    case M.User{}:
      True{}
    case _:
      False{}

def Draft.said.k(mine: Bool, text: String) -> Maybe<&2, String>:
  match mine:
    case True{}:
      Some{text}
    case False{}:
      None{}

# a person's message in thread th, else None
def Draft.said(e: M.Entry, +th: String) -> Maybe<&2, String>:
  match e:
    case M.Msg{_, t, r, text, _}:
      Draft.said.k(Bool.and(String.eq(t, th), Draft.user(r)), text)
    case M.Act{_, _, _, _, _, _}:
      None{}

# the newest such message (entries come newest first; the walk stops there)
def Draft.last.go(es: List<&2, M.Entry>, +th: String, got: Maybe<&2, String>) -> Maybe<&2, String>:
  match es got:
    case _ Some{x}:
      Some{x}
    case Nil{} None{}:
      None{}
    case e <> t None{}:
      Draft.last.go(t, th, Draft.said(e, th))

def Draft.last(es: List<&2, M.Entry>, +th: String) -> Maybe<&2, String>:
  Draft.last.go(es, th, None{})

# whether the message said is the draft d (alone, or with attachments after it)
def Draft.same(+said: String, +d: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(String.trim(d))),
    Bool.or(String.eq(String.trim(said), String.trim(d)), String.starts_with(String.trim(said), String.trim(d) ++ "\n\n")))

def Ui.state.at(u: Ui) -> M.State:
  match u:
    case Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      st

def Ui.sel.at(u: Ui) -> String:
  match u:
    case Ui{_, sel, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      sel

# the selected thread's draft, and what was read back for it
def Draft.of(u: Ui) -> String:
  match u:
    case Ui{_, +sel, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, sel)

def Draft.mark(u: Ui) -> String:
  match u:
    case Ui{_, +sel, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, "@kept:" ++ sel)

def Ui.also(a: Act, more: List<&2, Cmd>) -> Act:
  match a:
    case Act{u, cs}:
      Act{u, List.append(&2, Cmd, cs, more)}

# the mark goes once the draft has been checked against the thread
def Draft.unmark(a: Act) -> Act:
  match a:
    case Act{Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, cs}:
      Act{Ui{st, sel, Map.set(&2, String, dr, "@kept:" ++ sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, cs}

def Draft.stale.k(sent: Bool, u: Ui) -> Act:
  match sent:
    case True{}:
      Draft.unmark(Draft.clear(u))
    case False{}:
      Draft.unmark(Act{u, Nil{}})

def Draft.stale.m(m: Maybe<&2, String>, +u: Ui, +d: String) -> Act:
  match m:
    case Some{said}:
      Draft.stale.k(Draft.same(said, d), u)
    case None{}:
      Act{u, Nil{}}

# (only a draft still as it was read back is looked at)
def Draft.stale.u(fresh: Bool, +u: Ui) -> Act:
  match fresh:
    case True{}:
      Draft.stale.m(Draft.last(M.State.mine(Ui.state.at(u), Ui.sel.at(u)), Ui.sel.at(u)), u, Draft.of(u))
    case False{}:
      Act{u, Nil{}}

# the thread now selected, checked
def Draft.stale(+u: Ui) -> Act:
  Draft.stale.u(Bool.and(Bool.not(String.is_empty(Draft.mark(u))), String.eq(Draft.mark(u), Draft.of(u))), u)

# a step that selected a thread, its draft checked after it
def Draft.stale.after(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Ui.also(Draft.stale(u), cs)

# a message's id, made here so a resend is recognised: this client, the
# second and the request number
def Ui.msg_id(+cid: String, +now: Nat, +rid: Nat) -> String:
  "c" ++ cid ++ Nat.show(now) ++ "-" ++ Nat.show(rid)

# a kept request that names itself (`msg`, as a sent message does): the
# outbox sends it again after a dropped link, and the hub, seeing the
# same id, stores and does it once (bots.say, bots.post, bots.tell)
def Ui.rpc.keep.msg(u: Ui, +m: String, +p: List<&2, J.KV>) -> Act:
  match u:
    case Ui{st, sel, dr, live, op, on, +now, ad, pa, v, +rid, w, er, note, +nf, ob, tr}:
      Ui.rpc.keep(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, m,
        List.append(&2, J.KV, p, [J.KV{"msg", J.Str{Ui.msg_id(Str.get(nf, "cid"), now, rid)}}]))

# Follow-ups
# ----------
# A message is delivered at once (hub.bend's Hub.deliver): while a turn
# runs it steers that turn, so the send button says Steer; with none
# running it says Send. There is no queue and no mode to pick; the hub
# ignores the mode older clients still send (laws follow_*).

def Follow.mode(+setting: String, +provider: String, alt: Bool) -> String:
  "steer"

# what the send button says: Send, or while a turn runs Steer
def Follow.label.k(+l: I.Loc, run: Bool) -> String:
  match run:
    case True{}:
      MSG.T.composer.steer(l)
    case False{}:
      MSG.T.composer.send(l)

def Follow.label(+l: I.Loc, +st: M.State, +sel: String, run: Bool) -> String:
  Follow.label.k(l, run)

# While a turn runs and nothing is typed or attached, the send button is
# Stop: sending would do nothing there (laws follow_stop_*)
def Follow.stop.k(run: Bool, blank: Bool) -> Bool:
  match run:
    case True{}:
      blank
    case False{}:
      False{}

def Follow.stop(run: Bool, +draft: String, +atts: List<&2, String>) -> Bool:
  Follow.stop.k(run, Bool.and(String.is_empty(String.trim(draft)), List.is_empty(&2, String, atts)))

# "/btw <question>": a side question about the thread, answered off to
# the side by the writing model; the turn and the thread never see it
# (Btw.*, laws btw_*)
def Btw.is(+draft: String) -> Bool:
  Bool.or(String.eq(String.trim(draft), "/btw"), Bool.or(String.starts_with(draft, "/btw "), String.starts_with(draft, "/btw\n")))

def Btw.q(+draft: String) -> String:
  String.trim(String.drop(String.trim(draft), 4n))

# "/give <refs> [note]": the thread's conversation handed to each thread
# (>), project (%) and bot (@) the text names (bothub.bend's Give); the way
# a scratch thread (M.Scratch) goes somewhere (laws give_*)
def Give.is(+draft: String) -> Bool:
  Bool.or(String.eq(String.trim(draft), "/give"), Bool.or(String.starts_with(draft, "/give "), String.starts_with(draft, "/give\n")))

# what the send button says, and what it does
def Follow.button.k(+l: I.Loc, stop: Bool, +label: String) -> String:
  match stop:
    case True{}:
      MSG.T.composer.stop(l)
    case False{}:
      label

def Follow.button(+l: I.Loc, +st: M.State, +sel: String, run: Bool, stop: Bool, +draft: String) -> String:
  Follow.button.k(l, stop, Bool.pick(String, Btw.is(draft), MSG.T.composer.ask(l), Bool.pick(String, Give.is(draft), MSG.T.composer.give(l), Bool.pick(String, OR.Intent.is(draft), MSG.T.common.save(l), Follow.label(l, st, sel, run)))))

# what Enter does in the composer (its data-enter): takes a popup's row,
# else sends; while an intent is edited ("/intent ...", Edit's) it is the
# text's own, a new line, and only Save (or Ctrl+Enter, send-alt) saves
def Draft.enter(+rows: Bool, +draft: String) -> String:
  Bool.pick(String, rows, "skill", Bool.pick(String, OR.Intent.is(draft), "", "send"))

def Follow.act(stop: Bool) -> String:
  match stop:
    case True{}:
      "interrupt"
    case False{}:
      "send"

# Replies in bot chats (core/reply.bend): Reply on a message keeps its id in
# the scratch "@reply:<thread>" and its first line in "@reply.q:<thread>"
# (Rpl.set); the composer shows it until "reply-drop" or the next send,
# which carries it as `reply` and forgets it (the hub quotes the message,
# RTO.Rto.resolve).
def Rpl.key(+th: String) -> String:
  "@reply:" ++ th

def Rpl.q.key(+th: String) -> String:
  "@reply.q:" ++ th

def Rpl.kv(+id: String) -> List<&2, J.KV>:
  match id:
    case SNil{}: Nil{}
    case SCon{+h, +t}: [J.KV{"reply", J.Str{SCon{h, t}}}]

# the drafts with thread th's reply forgotten
def Rpl.forget(dr: Map<&2, String>, +th: String) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, dr, Rpl.key(th), ""), Rpl.q.key(th), "")

def Ui.send.rpc(u: Ui, text: String, +mode: String) -> Act:
  match u:
    case Ui{st, +sel, +dr, live, op, on, +now, ad, pa, v, +rid, w, er, note, +nf, ob, tr}:
      Ui.also(Ui.rpc.keep(Ui{st, sel, Rpl.forget(dr, sel), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr},
        "turn.start", List.append(&2, J.KV, [J.KV{"thread", J.Str{sel}}, J.KV{"text", J.Str{text}}, J.KV{"msg", J.Str{Ui.msg_id(Str.get(nf, "cid"), now, rid)}}, J.KV{"mode", J.Str{mode}}], Rpl.kv(Str.get(dr, Rpl.key(sel))))), [ScrollEnd{}])

def Ui.send.t.k(a: Act, text: String, +mode: String) -> Act:
  match a:
    case Act{u, cs}:
      Ui.first(cs, Ui.send.rpc(u, text, mode))

# the draft and attachments emptied, the host told, the message sent: one step
def Ui.send.t(u: Ui, text: String, +mode: String) -> Act:
  Ui.send.t.k(Draft.sent(u), text, mode)

# the add form shut (the folder goes to Proj.adding)
def Ui.add_project(u: Ui) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, _, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, False{}, pa, v, rid, w, er, note, nf, ob, tr}

def Ui.toggle_add(u: Ui) -> Act:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, Bool.not(ad), pa, v, rid, w, er, note, nf, ob, tr}, [Focus{"project-path"}]}

# (the viewer opened by hand comes back on what it showed: on Mech, its
# parts are listed again, as when Mech is opened, Mpage.sync)
def Ui.panel.opened(shut: Bool, dr: Map<&2, String>) -> Map<&2, String>:
  match shut:
    case True{}:
      Map.set(&2, String, dr, "@mech.at", "")
    case False{}:
      dr

# opened or shut by hand: the hub's own view leaves it alone from now on
def Ui.toggle_panel(u: Ui) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, +pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, Ui.panel.opened(Bool.not(pa), Map.set(&2, String, dr, "@revert", "")), live, op, on, now, ad, Bool.not(pa), v, rid, w, er, note, Map.set(&2, String, nf, "panel.hand:" ++ sel, "1"), ob, tr}, Nil{}}

def Ui.toggle_settled(u: Ui, +pid: String) -> Act:
  match u:
    case Ui{st, sel, dr, live, +op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, Map.set(&2, Bool, op, pid, Bool.not(Flag.get(op, pid))), on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, Nil{}}

def Ui.setting.k(ps: List<&2, String>) -> List<&2, J.KV>:
  match ps:
    case k <> t:
      [J.KV{"key", J.Str{k}}, J.KV{"value", J.Str{String.join(t, "=")}}]
    case Nil{}:
      Nil{}

# the read model changed here, not by the hub ("local.touch" counts it:
# the window's memos, app/mm.bend, key on the client's info)
def Local.touch.k(r: Map<&2, String> & String) -> Map<&2, String>:
  (m, v) = r
  Map.set(&2, String, m, "local.touch", Nat.show(1n+Maybe.default(&2, Nat, Nat.read(v), 0n)))

def Local.touch(nf: Map<&2, String>) -> Map<&2, String>:
  Local.touch.k(Map.get(String, "0", nf, "local.touch"))

# a setting applied here at once, before the hub answers with the same
# (the sequence is left alone, so nothing the hub sends is skipped)
def Ui.setting.here(u: Ui, ps: List<&2, String>) -> Ui:
  match u ps:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} k <> t:
      Ui{M.State.setting(st, k, String.join(t, "=")), sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Local.touch(nf), ob, tr}
    case x Nil{}:
      x

# "key=value" -> setting.set, shown at once (the sidebar's view swaps
# without waiting for the hub)
def Ui.setting(u: Ui, +kv: String) -> Act:
  Ui.rpc(Ui.setting.here(u, String.split(kv, '=')), "setting.set", Ui.setting.k(String.split(kv, '=')))

def Ui.close(u: Ui, +key: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, +op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, Map.set(&2, Bool, op, key, False{}), on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# a chip in the picker: a provider ("@codex") or an effort
def Ui.effort.set(prov: Bool, u: Ui, +value: String) -> Act:
  match prov:
    case True{}:
      Ui.on_thread.p(u, "thread.provider", [J.KV{"provider", J.Str{String.drop(value, 1n)}}])
    case False{}:
      Ui.on_thread.p(u, "thread.effort", [J.KV{"effort", J.Str{value}}])

# the selected thread's model (the picker stays open for another choice; a
# click outside it or on the chip closes it)
def Ui.model(u: Ui, +model: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, "thread.modes", [J.KV{"thread", J.Str{sel}}, J.KV{"model", J.Str{model}}])

def Ui.info(u: Ui, +k: String) -> String:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, ob, tr}:
      Str.get(nf, k)

# the switcher: this machine first, then the others ("on" marks the one shown)
def Machines.rows.loc(+l: I.Loc, +u: Ui) -> List<&2, MRow>:
  Machines.of(l, Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "local.machines")), J.Null{}), Ui.info(u, "local.machine"))

def Machines.rows(+u: Ui) -> List<&2, MRow>:
  Machines.rows.loc(Lang.loc(u), u)

# only worth showing when there is somewhere else to go
def Machines.any(+u: Ui) -> Bool:
  Nat.is_gt(List.length(&2, MRow, Machines.rows.loc(Lang.loc(u), u)), 1n)

def Ui.flag(u: Ui, +k: String) -> Bool:
  match u:
    case Ui{_, _, _, _, op, _, _, _, _, _, _, _, _, _, _, ob, tr}:
      Flag.get(op, k)

# the ui with flag k on
def Ui.flag_on(u: Ui, +k: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, +op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, Map.set(&2, Bool, op, k, True{}), on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

def Ui.flag_off(u: Ui, +k: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, +op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, Map.set(&2, Bool, op, k, False{}), on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

# Settings opened, whichever way (the top bar's gear, the palette, a
# secret's Set, KiCad's path, Usage), shuts the selected thread's viewer as
# its rail button would (the "panel" action: shut by hand), so Settings
# fills the main area and the thread comes back without the viewer
# (test/click_away_test.bend)
def Settings.viewer.shut(open: Bool, u: Ui) -> Ui:
  match open:
    case True{}:
      Act.ui(Ui.toggle_panel(u))
    case False{}:
      u

def Settings.settle.note.raw(+lc: I.Loc, +raw: String) -> String:
  Bool.pick(String, Nat.is_eq(M.Switch.num(raw, 0n), 0n), MSG.T.settings.settle.idle.off(lc), MSG.T.settings.settle.note(lc, Nat.show(M.Switch.num(raw, 0n)), Bool.pick(String, Nat.is_eq(M.Switch.num(raw, 0n), 1n), "one", "other")))

def Settings.settle.note(+lc: I.Loc, +st: M.State) -> String:
  Settings.settle.note.raw(lc, M.Setting.get(st, "settle.idle.minutes", "30"))

def Settings.on(+u: Ui) -> Ui:
  Ui.flag_on(Settings.viewer.shut(Ui.panel(u), u), "settings")

# the "flag" action: a flag turned on or off; Settings turned on goes
# through Settings.on
def Ui.flag.k(opening: Bool, +u: Ui, +value: String) -> Act:
  match opening:
    case True{}:
      Act{Settings.on(u), Nil{}}
    case False{}:
      Ui.toggle_settled(u, value)

def Ui.flag.act(+u: Ui, +value: String) -> Act:
  Ui.flag.k(Bool.and(String.eq(value, "settings"), Bool.not(Ui.flag(u, "settings"))), u, value)

def Ui.dismiss(u: Ui) -> Act:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, _, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, "", note, nf, ob, tr}, Nil{}}

def Ui.actions() -> List<&2, String>:
  ["select", "new-thread", "draft", "send", "add-project", "toggle-add", "settle", "unsettle",
   "pin", "unpin", "archive", "interrupt", "toggle-settled", "copy", "panel", "dismiss", "delete", "update",
   "flag", "setting", "model", "kicad-install", "answer", "git", "diff", "revert", "proj-open", "proj-cd", "proj-add", "proj-mkdir", "proj-q", "proj-close", "proj-enter",
   "effort", "skill", "snooze", "unarchive", "reorder", "move", "mention", "files",
   "delete-no", "proj-new", "proj-find", "proj-remove", "proj-go", "proj-find-q", "proj-keep", "-48", "drop-hover",
   "attach", "attach-file", "detach", "term-toggle", "term-key", "term-paste", "key", "update-check", "machine",
   "row-settle", "row-unsettle", "row-snooze", "row-delete", "fold", "row-toggle", "kicad-open", "reorder-project", "look", "look-set",
   "q-steer", "q-remove", "q-up", "q-edit", "q-resume", "send-alt", "fork", "back",
   "bot", "bot-tab", "bot-new", "bot-create", "bot-delete", "bot-edit", "bfield", "room", "room-new", "room-save", "room-pick", "room-post",
   "space", "space-input", "mem-forget", "routine-save", "routine-toggle", "routine-delete", "routine-run", "hook-new", "hook-revoke",
   "peer-invite", "peer-join", "peer-revoke", "google", "remote", "remote-send", "bot-look", "room-delete", "routine-edit", "hook-done", "form-close", "earlier",
   "hook-token", "hook-retoken", "hook-untoken",
   "voice", "voice-key", "voice-unkey", "voice-dict", "voice-use", "voice-add", "voice-fix", "voice-fixed", "voice-skip", "voice-err", "voice-term", "voice-unterm",
   "btw-close", "vw-light", "proj-pick", "proj-found", "proj-browse", "proj-back", "space-link", "skill-move",
   "mech", "mech-part", "mech-3d", "mech-refresh", "mech-renders", "mech-render", "view-kind", "view-sheet", "view-layer",
   "attach-up", "attach-end",
   "hist-go", "hist-prev", "hist-next", "hist-first", "hist-last", "hist-play", "hist-tick", "hist-diff", "hist-live", "hist-jump",
   "hist-show", "cmp", "cmp-pick", "cmp-close", "cmp-side", "cmp-shut", "key-add", "key-del", "fm", "fm-cd", "fm-up", "fm-open", "pdf-page", "fm-page", "update-now", "work-cancel", "task-cancel",
   "view-layer-pick", "view-layer-mode", "skill-close", "wake-cancel", "deploy-cancel", "lock-release", "lock-cancel", "ref-go", "ref-back", "ref-fwd", "ref-copy", "ref-copy-web", "kicad-retry", "kicad-path", "kicad-path-save"]

# Reading the client state
# ------------------------

def Ui.sel(u: Ui) -> String:
  match u:
    case Ui{_, sel, _, _, _, _, _, _, _, _, _, _, _, _, nf, ob, tr}:
      sel

# the panel after the selection may have moved on from thread `old`
def Ui.reselect(+old: String, +u: Ui) -> Ui:
  Ui.reselect.k(String.eq(old, Ui.sel(u)), u, old)

# a thread chosen (the one already selected too) leaves Settings; the web's
# hub-drawn view picks threads with it too (presentation.bend's Session.select)
def Ui.select.nav(u: Ui, +id: String) -> Ui:
  Ui.flag_off(Ui.select.k(u, id), "settings")

# (Ui.act brings the thread's own panel)
def Ui.select(u: Ui, +id: String) -> Act:
  Draft.stale.after(Act{Ui.select.nav(u, id), [Focus{"composer"}, ScrollEnd{}]})

def Ui.reselect.act(+old: String, a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Act{Ui.reselect(old, u), cs}

# the selected thread's view: its kind, path, change count and diff
# What a thread elsewhere's viewer shows waits for the file: its kind and
# path read as the Files tab and nothing until the hub that keeps the cache
# says the file is in it ("fv.ready:<t>" names the path it had then).
def Fv.kinds() -> List<&2, String>:
  ["board", "schematic", "3d", "image", "pdf"]

def Fv.kind(+k: String) -> Bool:
  FR.Fr.may.go(Fv.kinds(), k)

# a path as the owner's folder names it ("" for one outside, or no folder yet)
def Fv.rel.of(+root: String, +path: String) -> String:
  Bool.pick(String, String.is_empty(root), "", Bool.pick(String, String.starts_with(path, "/"), FV.Fv.rel(root, path), path))

def Fv.t(+u: Ui, +k: String) -> String:
  Ui.info(u, k ++ ":" ++ Ui.sel(u))

def Fv.pending(+u: Ui) -> Bool:
  Bool.and(Bool.and(FR.Fr.is(Ui.sel(u)), Fv.kind(Fv.t(u, "view.kind"))),
    Bool.and(Bool.not(String.is_empty(Fv.rel.of(Fv.t(u, "fv.root"), Fv.t(u, "view.path")))), Bool.not(String.eq(Fv.t(u, "view.path"), Fv.t(u, "fv.ready")))))

def Ui.view_get.k(held: Bool, +u: Ui, +k: String) -> String:
  match held:
    case True{}:
      Bool.pick(String, String.eq(k, "view.kind"), "files", "")
    case False{}:
      Ui.info(u, k ++ ":" ++ Ui.sel(u))

def Ui.view_get.far(far: Bool, +u: Ui, +k: String) -> String:
  match far:
    case True{}:
      Ui.view_get.k(Bool.and(Bool.or(String.eq(k, "view.kind"), String.eq(k, "view.path")), Fv.pending(u)), u, k)
    case False{}:
      Ui.info(u, k ++ ":" ++ Ui.sel(u))

def Ui.view_get(+u: Ui, +k: String) -> String:
  Ui.view_get.far(FR.Fr.is(Ui.sel(u)), u, k)

def Ui.diff(+u: Ui, +r: J.Json) -> Ui:
  +th = Viewer.of(J.Json.get_str(r, "thread", ""), Ui.sel(u))
  Ui.diff.by(String.eq(Ui.info(u, "diff.auto:" ++ th), "1"), u, th, r)

def Ui.reply.diff(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.diff(Ui.reply.k(u, o), J.Json.get_obj(o, "result"))
    case False{}:
      Ui.reply.k(u, o)

# The hub's own view ("auto"), sent when an agent starts in a KiCad
# project: it opens the thread's panel on its board (or schematic) only
# while that thread's viewer has shown nothing and its panel was never
# opened or shut by hand ("panel.hand:t"); an agent's viewer_show always
# opens it (laws viewer_auto_*)
def Viewer.auto(+u: Ui, +th: String) -> Bool:
  Bool.and(String.is_empty(Ui.info(u, "view.kind:" ++ th)), Bool.not(String.eq(Ui.info(u, "panel.hand:" ++ th), "1")))

def Ui.view.auto(go: Bool, +u: Ui, +th: String, +o: J.Json) -> Ui:
  match go:
    case True{}:
      Ui.view.k(u, th, o)
    case False{}:
      u

def Ui.view.at(auto: Bool, +u: Ui, +th: String, +o: J.Json) -> Ui:
  match auto:
    case True{}:
      Ui.view.auto(Viewer.auto(u, th), u, th, o)
    case False{}:
      Ui.view.k(u, th, o)

def Ui.view(+u: Ui, +o: J.Json) -> Ui:
  Ui.view.at(J.Json.get_bool(o, "auto", False{}), u, Viewer.of(J.Json.get_str(o, "thread", ""), Ui.sel(u)), o)

# the selected thread's draft
def Ui.draft_text(u: Ui) -> String:
  match u:
    case Ui{_, +sel, drafts, _, _, _, _, _, _, _, _, _, _, _, nf, ob, tr}:
      Str.get(drafts, sel)

def Ui.project_root.k(r: Map<&2, M.Project> & M.Project) -> String:
  (_, p) = r
  match p:
    case M.Project{_, _, root, _, _}:
      root

def Ui.project_root(s: M.State, +id: String) -> String:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Ui.project_root.k(Map.get(M.Project, M.Project.blank(id), ps, id))

def Ui.project_title.k(r: Map<&2, M.Project> & M.Project) -> String:
  (_, p) = r
  match p:
    case M.Project{_, title, _, _, _}:
      title

def Ui.project_title(s: M.State, +id: String) -> String:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Ui.project_title.k(Map.get(M.Project, M.Project.blank(id), ps, id))

def Ui.thread_project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def Ui.thread_worktree(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, wt, _, _, _, _, _, _}:
      wt

def Ui.dir(+st: M.State, +wt: String, +project: String) -> String:
  Bool.pick(String, String.is_empty(wt), Ui.project_root(st, project), wt)

# the selected thread's directory: its worktree, else its project's folder
# ("" with nothing selected). The viewer shows the board the thread edits.
def Ui.root.here(+st: M.State, +sel: String) -> String:
  Bool.pick(String, String.is_empty(sel), "", Ui.dir(st, Ui.thread_worktree(Ui.thread(st, sel)), Ui.thread_project(Ui.thread(st, sel))))

# A thread elsewhere shows the folder its hub keeps its files in (the
# cache core/fview.bend names; "" until the first answer of far.view says
# which): the viewers read it as they read a folder here.
def Ui.root(u: Ui) -> String:
  match u:
    case Ui{+st, +sel, _, _, _, _, _, _, _, _, _, _, _, _, +nf, ob, tr}:
      Bool.pick(String, FR.Fr.is(sel), Str.get(nf, "fv.root:" ++ sel), Ui.root.here(st, sel))

# Views both clients share
# ------------------------
# Delegated child threads live under their parent, not in the sidebar,
# except a bot's: a bot's delegated work is a thread of its own in the
# project it named, where the person can follow it (law
# kids_bot_child_shown); a thread lists the tasks it delegated; the model
# list comes from the machine's info ({"provider": [model, ...]}).

def Kids.add(bot: Bool, m: Map<&2, Bool>, +child: String) -> Map<&2, Bool>:
  match bot:
    case True{}:
      m
    case False{}:
      Map.set(&2, Bool, m, child, True{})

def Kids.of(ts: List<&2, M.Task>, +w: BT.World, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match ts:
    case Nil{}:
      m
    case M.Task{_, +parent, +child, _, _, _, _, _, _} <> t:
      Kids.of(t, w, Kids.add(BT.World.is_bot_thread(w, parent), m, child))

def Kids.set.k(s: M.State, +w: BT.World) -> Map<&2, Bool>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Kids.of(Map.values(&2, M.Task, tk), w, Map.new(&2, Bool))

def Kids.set(+s: M.State) -> Map<&2, Bool>:
  Kids.set.k(s, M.State.bots(s))

def Kids.keep.k(kid: Bool, t: M.Thread, rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match kid:
    case True{}:
      rest
    case False{}:
      t <> rest

def Kids.keep(ts: List<&2, M.Thread>, +kids: Map<&2, Bool>) -> List<&2, M.Thread>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{+id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Kids.keep.k(Flag.get(kids, id), M.Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, Kids.keep(t, kids))

def Models.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{s} <> t:
      s <> Models.strs(t)
    case _ <> t:
      Models.strs(t)

# the models info a provider's picker reads (opencode's is apart: Info.put)
def Models.of(+nf: Map<&2, String>, +prov: String) -> String:
  Str.get(nf, Bool.pick(String, String.eq(prov, "opencode"), "models.oc", "models"))

def Models.list(+info: String, +provider: String) -> List<&2, String>:
  Models.strs(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), provider)))

def Models.menu.k(skip: Bool, +m: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      rest
    case False{}:
      m <> rest

def Models.menu.go(ms: List<&2, String>) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Models.menu.k(Bool.or(String.is_empty(m), String.eq(m, "default")), m, Models.menu.go(t))

# the model menu's entries: the provider's default ("") first, then each
# model the CLI reports once (a CLI that lists "default" itself is the
# same entry)
def Models.menu(+info: String, +provider: String) -> List<&2, String>:
  "" <> Models.menu.go(Models.list(info, provider))

# The writing model: the small model that titles threads, names branches
# and writes commit messages (settings text.provider, text.model; each
# provider's light model unless set, like T3 Code's text generation model)
def TextGen.provider(+st: M.State) -> String:
  AG.Text.provider(st)

def TextGen.model(+st: M.State) -> String:
  AG.Text.model(st)

def Models.label(+provider: String, +model: String) -> String:
  provider ++ " · " ++ Bool.pick(String, String.is_empty(model), "default", model)

def Tasks.mine.k(keep: Bool, t: M.Task, rest: List<&2, M.Task>) -> List<&2, M.Task>:
  match keep:
    case True{}:
      t <> rest
    case False{}:
      rest

def Tasks.mine(ts: List<&2, M.Task>, +parent: String) -> List<&2, M.Task>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Task{i, +p, c, d, st, su, dl, pr, mo} <> t:
      Tasks.mine.k(String.eq(p, parent), M.Task{i, p, c, d, st, su, dl, pr, mo}, Tasks.mine(t, parent))

def Tasks.of(s: M.State, +parent: String) -> List<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Tasks.mine(Map.values(&2, M.Task, tk), parent)

def Tasks.title.k(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

def Tasks.title(s: M.State, +id: String) -> String:
  Tasks.title.k(Ui.thread(s, id))

# A sidebar drag holds a thread id, or "project:<id>" for a project
def Drag.project(+v: String) -> Bool:
  String.starts_with(v, "project:")

def Drag.title.k(project: Bool, +s: M.State, +v: String) -> String:
  match project:
    case True{}:
      Ui.project_title(s, String.drop(v, 8n))
    case False{}:
      Tasks.title(s, v)

# what the row being dragged is called
def Drag.title(+s: M.State, +v: String) -> String:
  Drag.title.k(Drag.project(v), s, v)

def Tasks.parent.find(ts: List<&2, M.Task>, +thread: String) -> String:
  match ts:
    case Nil{}:
      ""
    case M.Task{_, p, +c, _, _, _, _, _, _} <> t:
      Bool.pick(String, String.eq(c, thread), p, Tasks.parent.find(t, thread))

def Tasks.parent(+st: M.State, +thread: String) -> String:
  match st:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Tasks.parent.find(Map.values(&2, M.Task, tk), thread)

# A subagent row's state word as shown: the word stays the row's token
# (colours, order, cancel), and Japanese shows its own word for it
def StateWord.ja(+w: String) -> String:
  Bool.pick(String, String.eq(w, "running"), MSG.T.state.running(I.Ja{}), Bool.pick(String, String.eq(w, "starting"), MSG.T.state.starting(I.Ja{}),
    Bool.pick(String, String.eq(w, "done"), MSG.T.state.done(I.Ja{}), Bool.pick(String, String.eq(w, "failed"), MSG.T.state.failed(I.Ja{}),
      Bool.pick(String, String.eq(w, "cancelled"), MSG.T.state.cancelled(I.Ja{}), Bool.pick(String, String.eq(w, "stopped"), MSG.T.state.stopped(I.Ja{}),
        Bool.pick(String, String.eq(w, "idle"), MSG.T.state.idle(I.Ja{}), Bool.pick(String, String.eq(w, "needs you"), MSG.T.state.needsyou(I.Ja{}),
          Bool.pick(String, String.eq(w, "reporting"), MSG.T.state.reporting(I.Ja{}), Bool.pick(String, String.eq(w, "waiting"), MSG.T.state.waiting(I.Ja{}),
            Bool.pick(String, String.eq(w, "waiting on"), MSG.T.state.waitingon(I.Ja{}), Bool.pick(String, String.eq(w, "held"), MSG.T.state.held(I.Ja{}), w))))))))))))

def StateWord.label(l: I.Loc, +w: String) -> String:
  match l:
    case I.En{}: w
    case I.Ja{}: StateWord.ja(w)

def Task.word(s: M.TaskState) -> String:
  match s:
    case M.TaskQueued{}:
      "starting"
    case M.TaskRunning{}:
      "running"
    case M.TaskDone{}:
      "done"
    case M.TaskFailed{}:
      "failed"
    case M.TaskCancelled{}:
      "cancelled"

# Launched threads
# ----------------
# A thread started with thread_launch links back to its launcher the way a
# delegated child links to its parent (every client's "parent" row), and
# its launcher lists it with its state (subs.bend's Subs.launched). A
# launcher is named as the person knows it: a bot by its name, a thread by
# its title (law launched_by_bot_named).

def Launched.title(+t: String) -> String:
  Bool.pick(String, String.is_empty(t), "a thread on another machine", t)

def Launched.bot(m: Maybe<&2, BT.Bot>, +st: M.State, +id: String) -> String:
  match m:
    case Some{+b}:
      BT.Bot.name(b)
    case None{}:
      Launched.title(Tasks.title(st, id))

# the launcher's name ("" for a thread nobody launched)
def Launched.name(+st: M.State, +by: String) -> String:
  Bool.pick(String, String.is_empty(by), "", Launched.bot(BT.World.of_thread(M.State.bots(st), by), st, by))

# where a thread's link row goes: the parent that delegated it, else who
# launched it ("" for neither)
def Tasks.up(+st: M.State, +thread: String) -> String:
  Bool.pick(String, String.is_empty(ST.Tree.parent(st, thread)), Bool.pick(String, String.is_empty(Tasks.parent(st, thread)), Bool.pick(String, String.is_empty(M.Launch.by(st, thread)), M.Fork.source(st, thread), M.Launch.by(st, thread)), Tasks.parent(st, thread)), ST.Tree.parent(st, thread))

# the row's words
def Tasks.up.label.loc(+lc: I.Loc, +st: M.State, +thread: String, +p: String) -> String:
  Bool.pick(String, String.is_empty(ST.Tree.parent(st, thread)), Bool.pick(String, String.is_empty(Tasks.parent(st, thread)), Bool.pick(String, String.is_empty(M.Launch.by(st, thread)), MSG.T.tasks.origin.forked(lc, Tasks.title(st, p)), MSG.T.tasks.origin.launch(lc, Launched.name(st, p))), MSG.T.tasks.origin.delegate(lc, Tasks.title(st, p))), MSG.T.tasks.origin.parent(lc, Tasks.title(st, p)))

def Tasks.up.label(st: M.State, thread: String, p: String) -> String:
  Tasks.up.label.loc(I.En{}, st, thread, p)

# a launched thread's state as its launcher's list says it
def Launched.word.k(t: M.TurnState, +pending: Nat) -> String:
  match t:
    case M.Running{}:
      "running"
    case M.Failed{}:
      "failed"
    case M.Interrupted{}:
      "stopped"
    case M.Completed{}:
      Bool.pick(String, Nat.is_gt(pending, 0n), "needs you", "done")
    case _:
      Bool.pick(String, Nat.is_gt(pending, 0n), "needs you", "idle")

def Launched.word(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Launched.word.k(turn, pending)

# its row's second line: "launched · <model>"
def Launched.who(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      "launched · " ++ Models.label(pr, mo)

# at work or waiting on the person
def Launched.live.k(t: M.TurnState, +pending: Nat) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      Nat.is_gt(pending, 0n)

def Launched.live(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Launched.live.k(turn, pending)

# still there to show: made, not deleted or archived
def Launched.shown(t: M.Thread, +id: String) -> Bool:
  match t:
    case M.Thread{+i, _, _, _, _, _, _, archived, _, _, _, _, _, _, _, _, _, _, _, _, deleted}:
      Bool.and(String.eq(i, id), Bool.not(Bool.or(archived, deleted)))

# Asks
# ----

# "askid|answer" -> ask.answer
def Ui.answer.k(ps: List<&2, String>) -> List<&2, J.KV>:
  match ps:
    case id <> t:
      [J.KV{"id", J.Str{id}}, J.KV{"answer", J.Str{String.join(t, "|")}}]
    case Nil{}:
      Nil{}

def Ui.answer(u: Ui, v: String) -> Act:
  Ui.rpc(u, "ask.answer", Ui.answer.k(String.split(v, '|')))

# Sidebar model
# -------------
# Both clients draw the sidebar from these: projects dragged into place
# first (by order key), the rest by creation time and ID, each with its threads
# on shelves (M.Sidebar.at: keys and snoozes), and the visible order
# ctrl+1..9 counts in.

type SRow is Data:
  SRow{rank: Nat, project: M.Project, shelves: M.Shelves}

def SRow.before(a: SRow, b: SRow) -> Bool:
  match a b:
    case SRow{_, M.Project{ia, _, _, +ca, _}, _} SRow{_, M.Project{ib, _, _, +cb, _}, _}:
      Bool.or(Nat.is_gt(ca, cb), Bool.and(Nat.is_eq(ca, cb), ST.Tree.cmp.le(String.cmp(ia, ib))))

# a row with its project's order key (Nil: none)
type PSlot is Data:
  PSlot{key: List<&2, Bool>, row: SRow}

def PSlot.before.c(c: Cmp, a: SRow, b: SRow) -> Bool:
  match c:
    case LT{}: True{}
    case GT{}: False{}
    case EQ{}: SRow.before(a, b)

def PSlot.before.k(ka: List<&2, Bool>, kb: List<&2, Bool>, a: SRow, b: SRow) -> Bool:
  match ka kb:
    case Nil{} Nil{}:
      SRow.before(a, b)
    case Nil{} _ <> _:
      False{}
    case _ <> _ Nil{}:
      True{}
    case x <> xs y <> ys:
      PSlot.before.c(M.Key.cmp(x <> xs, y <> ys), a, b)

def PSlot.before(a: PSlot, b: PSlot) -> Bool:
  match a b:
    case PSlot{ka, ra} PSlot{kb, rb}:
      PSlot.before.k(ka, kb, ra, rb)

def PSlot.rows(xs: List<&2, PSlot>) -> List<&2, SRow>:
  match xs:
    case Nil{}:
      Nil{}
    case PSlot{_, r} <> t:
      r <> PSlot.rows(t)

# The sidebar's index
# -------------------
# What every row of a frame asks of the whole state, gathered once per
# frame (or phone screen) in grouped passes: each thread's
# open asks, each parent's unfinished child tasks, and the threads of each
# project in the order the state lists them. Side.*.ix answer from it what
# Side.* answer by walking the state (laws side_*_ix_eq, subs_count_ix_eq).

type SIx is Data:
  SIx{by: Map<&2, List<&2, M.Thread>>, asks: Map<&2, List<&2, M.Ask>>, kids: Map<&2, Nat>, work: Map<&2, Nat>}

def SIx.push.k(r: Map<&2, List<&2, M.Thread>> & List<&2, M.Thread>, +k: String, t: M.Thread) -> Map<&2, List<&2, M.Thread>>:
  (m, xs) = r
  Map.set(&2, List<&2, M.Thread>, m, k, t <> xs)

def SIx.pid(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def SIx.push(m: Map<&2, List<&2, M.Thread>>, +t: M.Thread) -> Map<&2, List<&2, M.Thread>>:
  SIx.push.k(Map.get(List<&2, M.Thread>, Nil{}, m, SIx.pid(t)), SIx.pid(t), t)

# threads by project, each list in the order given (walked from the end)
def SIx.by.go(ts: List<&2, M.Thread>, m: Map<&2, List<&2, M.Thread>>) -> Map<&2, List<&2, M.Thread>>:
  match ts:
    case Nil{}:
      m
    case +t <> rest:
      SIx.by.go(rest, SIx.push(m, t))

def SIx.ask.k(r: Map<&2, List<&2, M.Ask>> & List<&2, M.Ask>, +k: String, a: M.Ask) -> Map<&2, List<&2, M.Ask>>:
  (m, xs) = r
  Map.set(&2, List<&2, M.Ask>, m, k, a <> xs)

def SIx.ask(open: Bool, m: Map<&2, List<&2, M.Ask>>, +th: String, a: M.Ask) -> Map<&2, List<&2, M.Ask>>:
  match open:
    case True{}:
      SIx.ask.k(Map.get(List<&2, M.Ask>, Nil{}, m, th), th, a)
    case False{}:
      m

def SIx.asks.go(as: List<&2, M.Ask>, m: Map<&2, List<&2, M.Ask>>) -> Map<&2, List<&2, M.Ask>>:
  match as:
    case Nil{}:
      m
    case M.Ask{i, +th, k, t, d, o, +op, an} <> rest:
      SIx.asks.go(rest, SIx.ask(op, m, th, M.Ask{i, th, k, t, d, o, op, an}))

# the index of a state (Kids.keep: delegated children live under their
# parent, not in their project)
def Side.ix(+st: M.State) -> SIx:
  SIx{SIx.by.go(List.reverse(&2, M.Thread, Kids.keep(Ui.threads(st), Kids.set(st))), Map.new(&2, List<&2, M.Thread>)),
    SIx.asks.go(List.reverse(&2, M.Ask, M.Asks.all(st)), Map.new(&2, List<&2, M.Ask>)), M.Work.kids.map(st), M.Work.rows.map(st)}

# Typed canonical input for the plain metadata index. World/mail are needed
# for bot-child and mail-run visibility; transcript/clock/sequence/UI are not.
# This input is transient and is never retained as a memo or sent to a client.
type SidebarInput is Data:
  SidebarInput{threads: Map<&2, M.Thread>, tasks: Map<&2, M.Task>, settings: Map<&2, String>, asks: Map<&2, M.Ask>, world: BT.World, mail: ML.Box}

def SidebarInput.of(+st: M.State) -> SidebarInput:
  match st:
    case M.State{_, ts, _, _, tk, sg, _, ak, _, _, _}:
      SidebarInput{ts, tk, sg, ak, M.State.bots(st), M.State.mail(st)}

def SidebarInput.state(input: SidebarInput) -> M.State:
  match input:
    case SidebarInput{ts, tk, sg, ak, world, mail}:
      M.State{Map.new(&2, M.Project), ts, Nil{}, 0n, tk, sg, Map.new(&2, List<&2, M.QItem>), ak, Map.new(&2, M.Device), Map.new(&2, M.Nav), [M.Ext{world, Map.new(&2, List<&2, M.Entry>), [mail]}]}

def Side.ix.inputs(input: SidebarInput) -> SIx:
  Side.ix(SidebarInput.state(input))

def SIx.list.k(r: Map<&2, List<&2, M.Thread>> & List<&2, M.Thread>) -> List<&2, M.Thread>:
  (_, xs) = r
  xs

def SIx.mine(+ix: SIx, +pid: String) -> List<&2, M.Thread>:
  match ix:
    case SIx{by, _, _, _}:
      SIx.list.k(Map.get(List<&2, M.Thread>, Nil{}, by, pid))

def SIx.asks.k(r: Map<&2, List<&2, M.Ask>> & List<&2, M.Ask>) -> List<&2, M.Ask>:
  (_, xs) = r
  xs

# a thread's open asks
def SIx.open(+ix: SIx, +th: String) -> List<&2, M.Ask>:
  match ix:
    case SIx{_, as, _, _}:
      SIx.asks.k(Map.get(List<&2, M.Ask>, Nil{}, as, th))

# a parent's unfinished child tasks
def SIx.kids(+ix: SIx, +th: String) -> Nat:
  match ix:
    case SIx{_, _, ks, _}:
      M.Work.kids.at.k(Map.get(Nat, 0n, ks, th))

# a thread's own rows, background and watched (Work.rows, from the index)
def SIx.work(+ix: SIx, +th: String) -> Nat:
  match ix:
    case SIx{_, _, _, wk}:
      M.Work.kids.at.k(Map.get(Nat, 0n, wk, th))

def Side.latest(ts: List<&2, M.Thread>, +acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case M.Thread{_, _, _, _, +a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      Side.latest(t, M.Time.max(acc, a))

def Side.rows.k(+st: M.State, removed: Bool, p: M.Project, +mine: List<&2, M.Thread>, +c: Nat, +navs: Map<&2, M.Nav>, +now: Nat, +key: String, rest: List<&2, PSlot>) -> List<&2, PSlot>:
  match removed:
    case True{}:
      rest
    case False{}:
      PSlot{M.Key.read(key), SRow{Side.latest(mine, c), p, ST.Tree.shelves(st, M.Sidebar.at(mine, navs, now))}} <> rest

def Side.rows.go(ps: List<&2, M.Project>, +st: M.State, +ts: List<&2, M.Thread>, +navs: Map<&2, M.Nav>, +now: Nat) -> List<&2, PSlot>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, +c, +rm} <> t:
      Side.rows.k(st, rm, M.Project{id, ti, r, c, rm}, M.Thread.filter_project(ts, id), c, navs, now, M.Nav.project_key(navs, id), Side.rows.go(t, st, ts, navs, now))

def Side.rows.go.ix(ps: List<&2, M.Project>, +st: M.State, +ix: SIx, +navs: Map<&2, M.Nav>, +now: Nat) -> List<&2, PSlot>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, +c, +rm} <> t:
      Side.rows.k(st, rm, M.Project{id, ti, r, c, rm}, SIx.mine(ix, id), c, navs, now, M.Nav.project_key(navs, id), Side.rows.go.ix(t, st, ix, navs, now))

# the projects the sidebar lists: all but those of a machine sync is off
# with (only a project elsewhere is asked)
def Side.keep(hide: Bool, p: M.Project, rest: List<&2, M.Project>) -> List<&2, M.Project>:
  match hide:
    case True{}:
      rest
    case False{}:
      p <> rest

def Side.projs.k(far: Bool, +s: M.State, +id: String, p: M.Project, rest: List<&2, M.Project>) -> List<&2, M.Project>:
  match far:
    case True{}:
      Side.keep(Mate.hidden(s, id), p, rest)
    case False{}:
      p <> rest

def Side.projs.keep(legacy: Bool, far: Bool, s: M.State, id: String, p: M.Project, rest: List<&2, M.Project>) -> List<&2, M.Project>:
  match legacy:
    case True{}: rest
    case False{}: Side.projs.k(far, s, id, p, rest)

def Side.projs.go(ps: List<&2, M.Project>, +s: M.State) -> List<&2, M.Project>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, c, rm} <> t:
      Side.projs.keep(M.Scratch.is(FR.Fr.local(id)), FR.Fr.is(id), s, id, M.Project{id, ti, r, c, rm}, Side.projs.go(t, s))

def Side.projs(+s: M.State) -> List<&2, M.Project>:
  Side.projs.go(Ui.projects(s), s)

# Side.sorted from the index
def Side.sorted.ix(+st: M.State, +ix: SIx, +now: Nat) -> List<&2, SRow>:
  PSlot.rows(List.sort(~PSlot, ~PSlot.before, Side.rows.go.ix(Side.projs(st), st, ix, M.State.navs(st), now)))

# the projects by order key, then creation time and ID, their threads on
# shelves (delegated child threads live under their parent instead)
def Side.sorted(+st: M.State, +now: Nat) -> List<&2, SRow>:
  PSlot.rows(List.sort(~PSlot, ~PSlot.before, Side.rows.go(Side.projs(st), st, Kids.keep(Ui.threads(st), Kids.set(st)), M.State.navs(st), now)))

# Quiet projects
# --------------
# A project is live while it has a thread on a shelf of its own (pinned,
# active, snoozed). The others are quiet: they sink under the live ones,
# faded, and once they have gone Quiet.days without activity they fall
# into the folded "Older" shelf at the end (flag "old-projects"; a search
# shows them too). Each group keeps the order it had (laws side_groups_*).

def Quiet.days() -> Nat:
  259200n

# something at `at` is still recent at now
def Quiet.recent(+now: Nat, +at: Nat) -> Bool:
  Nat.is_gt(Nat.add(at, Quiet.days()), now)

def SRow.live(r: SRow) -> Bool:
  match r:
    case SRow{_, _, M.Shelves{p, a, _, _, sn}}:
      Bool.not(Bool.and(List.is_empty(&2, M.Thread, p), Bool.and(List.is_empty(&2, M.Thread, a), List.is_empty(&2, M.Thread, sn))))

def SRow.rank(r: SRow) -> Nat:
  match r:
    case SRow{k, _, _}:
      k

def SRow.scratch(r: SRow) -> Bool:
  match r:
    case SRow{_, M.Project{+id, _, _, _, _}, _}:
      M.Scratch.is(id)

# 0 live, 1 quiet, 2 older; Scratch (M.Scratch) never folds away, so a new
# scratch thread is always a click off
def SRow.group(+now: Nat, +r: SRow) -> Nat:
  Bool.pick(Nat, Bool.or(SRow.live(r), SRow.scratch(r)), 0n, Bool.pick(Nat, Quiet.recent(now, SRow.rank(r)), 1n, 2n))

type SGroups is Data:
  SGroups{live: List<&2, SRow>, quiet: List<&2, SRow>, old: List<&2, SRow>}

def SGroups.put(i: Nat, r: SRow, g: SGroups) -> SGroups:
  match i g:
    case 0n SGroups{l, q, o}:
      SGroups{r <> l, q, o}
    case 1n SGroups{l, q, o}:
      SGroups{l, r <> q, o}
    case _ SGroups{l, q, o}:
      SGroups{l, q, r <> o}

def Side.groups(rs: List<&2, SRow>, +now: Nat) -> SGroups:
  match rs:
    case Nil{}:
      SGroups{Nil{}, Nil{}, Nil{}}
    case +r <> t:
      SGroups.put(SRow.group(now, r), r, Side.groups(t, now))

def SGroups.all(g: SGroups) -> List<&2, SRow>:
  match g:
    case SGroups{l, q, o}:
      List.append(&2, SRow, l, List.append(&2, SRow, q, o))

# The open thread's project keeps its place: for an hour after the open
# thread settled (by itself, Settings' "Settle when done", or by the
# person) the project counts as live, so it neither sinks nor fades under
# the reader (cur: that project, "" for none: Side.groups; Side.cur.held)
def SRow.pid(r: SRow) -> String:
  match r:
    case SRow{_, M.Project{id, _, _, _, _}, _}:
      id

def SRow.group.at(+now: Nat, +cur: String, +r: SRow) -> Nat:
  Bool.pick(Nat, Bool.and(Bool.not(String.is_empty(cur)), String.eq(SRow.pid(r), cur)), 0n, SRow.group(now, r))

def Side.groups.at(rs: List<&2, SRow>, +now: Nat, +cur: String) -> SGroups:
  match rs:
    case Nil{}:
      SGroups{Nil{}, Nil{}, Nil{}}
    case +r <> t:
      SGroups.put(SRow.group.at(now, cur, r), r, Side.groups.at(t, now, cur))

# the open thread's project ("" with none open)
def Side.cur.of(+st: M.State, +sel: String) -> String:
  Bool.pick(String, String.is_empty(sel), "", Ui.thread_project(Ui.thread(st, sel)))

def Side.hold() -> Nat:
  3600n

def Side.cur.held.k(fresh: Bool, +st: M.State, +sel: String) -> String:
  match fresh:
    case True{}:
      Side.cur.of(st, sel)
    case False{}:
      ""

# the project the open thread holds in place: its own, while that thread
# settled within Side.hold() ("" else: a thread settled long ago, opened
# from a quiet project, moves nothing either)
def Side.cur.held.at(+sa: Nat, +st: M.State, +sel: String, +now: Nat) -> String:
  Side.cur.held.k(Bool.and(Bool.and(Bool.not(String.is_empty(sel)), Nat.is_gt(sa, 0n)), Nat.is_gt(Nat.add(sa, Side.hold()), now)), st, sel)

# (the active view lists no projects: nothing to hold there)
def Side.cur.held(+st: M.State, +sel: String, +now: Nat) -> String:
  Bool.pick(String, String.eq(M.Setting.get(st, "sidebar.view", ""), "active"), "", Side.cur.held.at(M.Thread.settled_rank(Ui.thread(st, sel)), st, sel, now))

# the sidebar's projects in order: live, quiet, older
def Side.rows(+st: M.State, +now: Nat) -> List<&2, SRow>:
  SGroups.all(Side.groups(Side.sorted(st, now), now))

# Side.rows from the index (law side_rows_ix_eq)
def Side.rows.ix(+st: M.State, +ix: SIx, +now: Nat) -> List<&2, SRow>:
  SGroups.all(Side.groups(Side.sorted.ix(st, ix, now), now))

# Side.rows.ix with the open thread's project held live
def Side.rows.ix.at(+st: M.State, +ix: SIx, +now: Nat, +cur: String) -> List<&2, SRow>:
  SGroups.all(Side.groups.at(Side.sorted.ix(st, ix, now), now, cur))

def Side.rows.at(+st: M.State, +now: Nat, +cur: String) -> List<&2, SRow>:
  SGroups.all(Side.groups.at(Side.sorted(st, now), now, cur))

# Finding a project: while the sidebar's search is open (the "psearch"
# flag) it shows only the projects whose name fuzzy-matches the query, form
# field "pq" ("@f.pq"); an empty query keeps them all (law find_empty_keeps)
def Find.keep(keep: Bool, r: SRow, rest: List<&2, SRow>) -> List<&2, SRow>:
  match keep:
    case True{}:
      r <> rest
    case False{}:
      rest

def Find.rows(rs: List<&2, SRow>, +q: String) -> List<&2, SRow>:
  match rs:
    case Nil{}:
      Nil{}
    case SRow{r, M.Project{id, +ti, ro, c, rm}, sh} <> t:
      Find.keep(Nat.is_gt(FZ.Fuzzy.score(ti, String.trim(q)), 0n), SRow{r, M.Project{id, ti, ro, c, rm}, sh}, Find.rows(t, q))

# the query in force: "" while the search is shut
def Find.q.k(on: Bool, +dr: Map<&2, String>) -> String:
  match on:
    case True{}:
      Str.get(dr, "@f.pq")
    case False{}:
      ""

# the field sits in the sidebar all the time now, so its query holds
# whether or not "psearch" (the phones' old toggle) is on
def Find.q(+op: Map<&2, Bool>, +dr: Map<&2, String>) -> String:
  Find.q.k(True{}, dr)

# the projects the sidebar shows, in order
def Side.shown(+st: M.State, +now: Nat, +q: String) -> List<&2, SRow>:
  Find.rows(Side.rows(st, now), q)

# Side.shown from the index
def Side.shown.ix(+st: M.State, +ix: SIx, +now: Nat, +q: String) -> List<&2, SRow>:
  Find.rows(Side.rows.ix(st, ix, now), q)

def Side.ids(ts: List<&2, M.Thread>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> rest:
      M.Thread.id(t) <> Side.ids(rest)

def Side.when(on: Bool, xs: List<&2, String>) -> List<&2, String>:
  match on:
    case True{}:
      xs
    case False{}:
      Nil{}

def Side.visible.go(rs: List<&2, SRow>, +open: Map<&2, Bool>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case SRow{_, M.Project{+id, _, _, _, _}, M.Shelves{p, a, st, h, sn}} <> t:
      List.concat(&2, String, [Side.ids(p), Side.ids(a), Side.ids(sn), Side.when(Flag.get(open, id), Side.ids(st)),
        Side.when(Flag.get(open, "arch:" ++ id), Side.ids(M.Archive.of(h))), Side.visible.go(t, open)])

# the projects shown with the Older shelf folded unless it is open or a
# search is on
def Side.fold.k(show: Bool, g: SGroups) -> List<&2, SRow>:
  match show g:
    case True{} x:
      SGroups.all(x)
    case False{} SGroups{l, q, _}:
      List.append(&2, SRow, l, q)

def Side.unfolded(rs: List<&2, SRow>, +op: Map<&2, Bool>, +q: String, +now: Nat) -> List<&2, SRow>:
  Side.fold.k(Bool.or(Flag.get(op, "old-projects"), Bool.not(String.is_empty(q))), Side.groups(rs, now))

def Side.unfolded.at(rs: List<&2, SRow>, +op: Map<&2, Bool>, +q: String, +now: Nat, +cur: String) -> List<&2, SRow>:
  Side.fold.k(Bool.or(Flag.get(op, "old-projects"), Bool.not(String.is_empty(q))), Side.groups.at(rs, now, cur))

def Side.tree_ids(+st: M.State, ts: List<&2, M.Thread>, +sel: String) -> List<&2, String>:
  ST.Tree.row_ids(ST.Tree.rows(st, ts, sel))

def Side.visible.tree(rs: List<&2, SRow>, +open: Map<&2, Bool>, +st: M.State, +sel: String) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case SRow{_, M.Project{+id, _, _, _, _}, M.Shelves{p, a, settled, h, sn}} <> t:
      List.concat(&2, String, [Side.tree_ids(st, p, sel), Side.tree_ids(st, a, sel), Side.tree_ids(st, sn, sel),
        Side.when(Flag.get(open, id), Side.tree_ids(st, settled, sel)),
        Side.when(Flag.get(open, "arch:" ++ id), Side.tree_ids(st, M.Archive.of(h), sel)), Side.visible.tree(t, open, st, sel)])

# the threads the sidebar shows, top to bottom (collapsed shelves left out)
def Side.visible.projects(u: Ui) -> List<&2, String>:
  match u:
    case Ui{+st, +sel, +dr, _, +op, _, +now, _, _, _, _, _, _, _, _, _, _}:
      Side.visible.tree(Side.unfolded(Side.shown(st, now, Find.q(op, dr)), op, Find.q(op, dr), now), op, st, sel)

def Side.nth(xs: List<&2, String>, i: Nat) -> String:
  match xs i:
    case x <> _ 0n:
      x
    case _ <> t 1n+j:
      Side.nth(t, j)
    case Nil{} _:
      ""

type Sect is Data:
  Sect{name: String, ids: List<&2, String>}

def Side.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case h <> t:
      Bool.or(String.eq(h, x), Side.has(t, x))

def Side.section.k(s: M.Shelves, +id: String) -> Sect:
  match s:
    case M.Shelves{+p, +a, _, _, _}:
      Bool.pick(Sect, Side.has(Side.ids(p), id), Sect{"pinned", Side.ids(p)},
        Bool.pick(Sect, Side.has(Side.ids(a), id), Sect{"active", Side.ids(a)}, Sect{"", Nil{}}))

# the shelf a thread can be dragged within, with its threads in order
# ("" for a shelf without manual order)
def Side.section(+st: M.State, +now: Nat, +id: String) -> Sect:
  Side.section.k(M.Sidebar.at(M.Thread.filter_project(Kids.keep(Ui.threads(st), Kids.set(st)), Ui.thread_project(Ui.thread(st, id))), M.State.navs(st), now), id)

# Reordering, snoozing, mentions
# ------------------------------

def Side.st(u: Ui) -> M.State:
  match u:
    case Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      st

def Side.now(u: Ui) -> Nat:
  match u:
    case Ui{_, _, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}:
      now

def Side.pair(ps: List<&2, String>) -> Sect:
  match ps:
    case a <> t:
      Sect{a, t}
    case Nil{}:
      Sect{"", Nil{}}

def Side.first(xs: List<&2, String>) -> String:
  Maybe.default(&2, String, List.head(&2, String, xs), "")

def Side.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Side.strs(t)

def Side.open(u: Ui) -> Map<&2, Bool>:
  match u:
    case Ui{_, _, _, _, op, _, _, _, _, _, _, _, _, _, _, _, _}:
      op

# Hotkeys: alt+1..9 goes to the Nth project the sidebar shows, ctrl+1..9 to
# the Nth thread the sidebar shows in the selected thread's project.

def Side.pids(rs: List<&2, SRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case SRow{_, M.Project{id, _, _, _, _}, _} <> t:
      id <> Side.pids(t)

# the sidebar's projects, top to bottom (the open thread's kept in place)
def Side.projects(u: Ui) -> List<&2, String>:
  match u:
    case Ui{+st, +sel, +dr, _, +op, _, +now, _, _, _, _, _, _, _, _, _, _}:
      Side.pids(Side.unfolded.at(Find.rows(Side.rows.at(st, now, Side.cur.held(st, sel, now)), Find.q(op, dr)), op, Find.q(op, dr), now, Side.cur.held(st, sel, now)))

def Side.only.k(keep: Bool, r: SRow, rest: List<&2, SRow>) -> List<&2, SRow>:
  match keep:
    case True{}:
      r <> rest
    case False{}:
      rest

def Side.only(rs: List<&2, SRow>, +pid: String) -> List<&2, SRow>:
  match rs:
    case Nil{}:
      Nil{}
    case SRow{r, M.Project{+id, ti, ro, c, rm}, sh} <> t:
      Side.only.k(String.eq(id, pid), SRow{r, M.Project{id, ti, ro, c, rm}, sh}, Side.only(t, pid))

# the selected thread's project ("" with nothing selected)
def Side.cur(u: Ui) -> String:
  match u:
    case Ui{+st, +sel, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(sel), "", Ui.thread_project(Ui.thread(st, sel)))

def Side.live.go(rs: List<&2, SRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case SRow{_, _, M.Shelves{p, a, st, _, sn}} <> t:
      List.concat(&2, String, [Side.ids(p), Side.ids(a), Side.ids(sn), Side.ids(st), Side.live.go(t)])

# where alt+N lands in project pid: the selected thread when it is already
# there, else the project's top thread, collapsed shelves included (""
# when it has none, so the caller starts one)
def Side.home.k(here: Bool, +u: Ui, +pid: String) -> String:
  match here:
    case True{}:
      Ui.sel(u)
    case False{}:
      Side.first(Side.live.go(Side.only(Side.rows(Side.st(u), Side.now(u)), pid)))

def Side.home(+u: Ui, +pid: String) -> String:
  Side.home.k(Bool.and(Bool.not(String.is_empty(pid)), String.eq(Side.cur(u), pid)), u, pid)

# x's place in xs counting from 1, 0 when absent (the number a hint shows)
def Side.num(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      0n
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), 1n+i, Side.num(t, x, 1n+i))

def Ui.reorder.k(ok: Bool, +u: Ui, s: Sect, +id: String, +to: Nat) -> Act:
  match ok s:
    case True{} Sect{+name, +ids}:
      Ui.rpc(u, "thread.reorder", [J.KV{"section", J.Str{name}}, J.KV{"thread", J.Str{id}}, J.KV{"ids", J.Json.arr(Side.strs(M.Order.move(ids, id, to)))}])
    case False{} _:
      Act{u, Nil{}}

def Ui.reorder.s(+u: Ui, +s: Sect, +id: String, +target: String) -> Act:
  match s:
    case Sect{+name, +ids}:
      Ui.reorder.k(Bool.and(Bool.and(Bool.not(String.is_empty(name)), Bool.not(String.eq(id, target))), Side.has(ids, target)), u, Sect{name, ids}, id, M.Order.index(ids, target, 0n))

# drop thread `id` where `target` sits on the same shelf
def Ui.reorder.to(+u: Ui, +id: String, +target: String) -> Act:
  Ui.reorder.s(u, Side.section(Side.st(u), Side.now(u), id), id, target)

def Ui.reorder.pair(+u: Ui, s: Sect) -> Act:
  match s:
    case Sect{+id, t}:
      Ui.reorder.to(u, id, Side.first(t))

# "id|target"
def Ui.reorder(+u: Ui, v: String) -> Act:
  Ui.reorder.pair(u, Side.pair(String.split(v, '|')))

def Ui.reorder_project.k(ok: Bool, +u: Ui, +ids: List<&2, String>, +id: String, +to: Nat) -> Act:
  match ok:
    case True{}:
      Ui.rpc(u, "project.reorder", [J.KV{"project", J.Str{id}}, J.KV{"ids", J.Json.arr(Side.strs(M.Order.move(ids, id, to)))}])
    case False{}:
      Act{u, Nil{}}

def Ui.reorder_project.to(+u: Ui, +ids: List<&2, String>, +id: String, +target: String) -> Act:
  Ui.reorder_project.k(Bool.and(Bool.and(Side.has(ids, id), Bool.not(String.eq(id, target))), Side.has(ids, target)), u, ids, id, M.Order.index(ids, target, 0n))

# a project dropped on a row: the target is a project, or a thread standing
# for its project
def Ui.reorder_project.pair(+u: Ui, s: Sect) -> Act:
  match s:
    case Sect{+id, t}:
      +ids = Side.projects(u)
      +target = Side.first(t)
      Ui.reorder_project.to(u, ids, id, Bool.pick(String, Side.has(ids, target), target, Ui.thread_project(Ui.thread(Side.st(u), target))))

# "project|target": move project `id` to where `target` (a project or one
# of its threads) sits in the sidebar
def Ui.reorder_project(+u: Ui, v: String) -> Act:
  Ui.reorder_project.pair(u, Side.pair(String.split(v, '|')))

def Ui.move.go(+u: Ui, +id: String, +up: Bool, +ids: List<&2, String>) -> Act:
  +i = M.Order.index(ids, id, 0n)
  Ui.reorder.to(u, id, Bool.pick(String, up, Bool.pick(String, Nat.is_eq(i, 0n), "", Side.nth(ids, Nat.sub(i, 1n))), Side.nth(ids, 1n+i)))

def Ui.move.s(+u: Ui, s: Sect, +id: String, +up: Bool) -> Act:
  match s:
    case Sect{_, +ids}:
      Ui.move.go(u, id, up, ids)

def Ui.move.k(+u: Ui, s: Sect) -> Act:
  match s:
    case Sect{+id, t}:
      Ui.move.s(u, Side.section(Side.st(u), Side.now(u), id), id, String.eq(Side.first(t), "up"))

# "id|up" or "id|down": one place up or down its shelf
def Ui.move(+u: Ui, v: String) -> Act:
  Ui.move.k(u, Side.pair(String.split(v, '|')))

# snooze the selected thread ("1h", "tomorrow", "1w"; "off" wakes it)
def Ui.snooze(u: Ui, v: String) -> Act:
  Ui.on_thread.p(Ui.close(u, "snooze"), "thread.snooze", [J.KV{"for", J.Str{v}}])

def Ui.mention.sep(+d: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(d), Bool.or(String.ends_with(d, " "), String.ends_with(d, "\n"))), "", " ")

# "@path " added to the selected thread's draft
def Ui.mention(u: Ui, +path: String) -> Act:
  match u:
    case Ui{st, +sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.also(Ui.draft.set(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, Str.get(dr, sel) ++ Ui.mention.sep(Str.get(dr, sel)) ++ "@" ++ path ++ " "), [Focus{"composer"}])

# Skills and references in the composer
# -------------------------------------
# Typing `$` asks the server for the thread's skills (user and project,
# once per thread); the composer then offers those whose name starts with
# what follows the `$`. Typing `@`, `>`, `%` or `#` offers bots, threads,
# projects or group chats (refs.bend's Refs.offer, ranked), worked out
# once per edit of the draft and kept in the info map ("refs.for", "refs"),
# so a frame or a phone's screen only reads them. Tab or Enter completes
# the first (or a clicked one). A row's name is the whole word it puts in,
# sigil and all. The arrow keys move the highlight (Skills.move).

type SkillRow is Data:
  SkillRow{name: String, description: String, on: Bool}

# the draft's last word
def Skills.token(+draft: String) -> String:
  RF.Refs.token(draft)

# whether the draft ends in a `$name` being typed
def Skills.typing(+draft: String) -> Bool:
  Bool.and(String.starts_with(Skills.token(draft), "$"), String.eq(AG.Mention.name(Skills.token(draft)), String.drop(Skills.token(draft), 1n)))

def Skills.rows.k(keep: Bool, +r: SkillRow, rest: List<&2, SkillRow>) -> List<&2, SkillRow>:
  match keep:
    case True{}:
      r <> rest
    case False{}:
      rest

def Skills.rows(xs: List<&2, J.Json>, +q: String) -> List<&2, SkillRow>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Skills.rows.k(String.starts_with(J.Json.get_str(x, "name", ""), q), SkillRow{"$" ++ J.Json.get_str(x, "name", ""), J.Json.get_str(x, "description", ""), False{}}, Skills.rows(t, q))

def Skills.offer(on: Bool, +list: String, +q: String) -> List<&2, SkillRow>:
  match on:
    case True{}:
      List.take(&2, SkillRow, Skills.rows(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(list), J.Null{})), q), 6n)
    case False{}:
      Nil{}

# the skills the composer offers now (none unless a `$name` is being typed)
def Skills.only(+sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  Skills.offer(Bool.and(Skills.typing(Str.get(dr, sel)), String.eq(Str.get(nf, "skills.for"), sel)), Str.get(nf, "skills"), String.drop(Skills.token(Str.get(dr, sel)), 1n))

# References: the cache's key, and its rows as lines of "word\tabout"
def Refs.for(+sel: String, +draft: String) -> String:
  sel ++ "\n" ++ Skills.token(draft)

def Refs.flat(+s: String) -> String:
  String.join(String.split(String.join(String.split(s, '\n'), " "), '\t'), " ")

def Refs.show(cs: List<&2, RF.Cand>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case RF.Cand{w, a, _, _} <> t:
      (w ++ "\t" ++ Refs.flat(a)) <> Refs.show(t)

def Refs.row(+l: String) -> SkillRow:
  SkillRow{Maybe.default(&2, String, List.head(&2, String, String.split(l, '\t')), ""), String.join(List.drop(&2, String, String.split(l, '\t'), 1n), " "), False{}}

def Refs.rows(ls: List<&2, String>) -> List<&2, SkillRow>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Skills.rows.k(Bool.not(String.is_empty(l)), Refs.row(l), Refs.rows(t))

def Refs.kept(same: Bool, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  match same:
    case True{}:
      Refs.rows(String.split(Str.get(nf, "refs"), '\n'))
    case False{}:
      Nil{}

# the references offered now: the kept rows, while they are for this
# thread's draft as it is
def Refs.of(+sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  Refs.kept(Bool.and(RF.Refs.typing(Skills.token(Str.get(dr, sel))), String.eq(Str.get(nf, "refs.for"), Refs.for(sel, Str.get(dr, sel)))), nf)

def Skills.of.k(dollar: Bool, +sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  match dollar:
    case True{}:
      Skills.only(sel, dr, nf)
    case False{}:
      Refs.of(sel, dr, nf)

# what the composer offers now: skills after `$`, references after a sigil
def Skills.of(+sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  Skills.of.k(String.starts_with(Skills.token(Str.get(dr, sel)), "$"), sel, dr, nf)

# Slash commands: a draft that is one word starting with `/` offers the
# commands (slash.bend; Claude's own only in a Claude thread) and then the
# thread's skills, as `$name` rows, whose name starts with what follows
def Slash.rows(cs: List<&2, SL.Cmd>) -> List<&2, SkillRow>:
  match cs:
    case Nil{}:
      Nil{}
    case SL.Cmd{n, a} <> t:
      SkillRow{n, a, False{}} <> Slash.rows(t)

def Slash.of(claude: Bool, +sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  List.take(&2, SkillRow, List.append(&2, SkillRow, Slash.rows(SL.Slash.cmds(claude, String.drop(Skills.token(Str.get(dr, sel)), 1n))),
    Skills.offer(String.eq(Str.get(nf, "skills.for"), sel), Str.get(nf, "skills"), String.drop(Skills.token(Str.get(dr, sel)), 1n))), 8n)

def Skills.all.k(slash: Bool, +st: M.State, +sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  match slash:
    case True{}:
      Slash.of(String.eq(M.Thread.provider(Ui.thread(st, sel)), "claude"), sel, dr, nf)
    case False{}:
      Skills.of(sel, dr, nf)

# what the composer offers now: slash commands, skills or references
def Skills.all(+st: M.State, +sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  Skills.all.k(SL.Slash.typing(Str.get(dr, sel)), st, sel, dr, nf)

# The row the arrow keys have moved to: info "sk.at" (its index), kept
# for the draft it was moved on ("sk.on", thread and text); any other
# draft starts at the first row. Enter or Tab takes it.
def Skills.on(+sel: String, +draft: String) -> String:
  sel ++ "\n" ++ draft

def Skills.at.of(same: Bool, +at: String) -> Nat:
  match same:
    case True{}:
      Maybe.default(&2, Nat, Nat.read(at), 0n)
    case False{}:
      0n

def Skills.at.k(+sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> Nat:
  Skills.at.of(String.eq(Str.get(nf, "sk.on"), Skills.on(sel, Str.get(dr, sel))), Str.get(nf, "sk.at"))

# the rows with the one at k (counted from 1) marked
def Skills.mark(rs: List<&2, SkillRow>, +k: Nat) -> List<&2, SkillRow>:
  match rs:
    case Nil{}:
      Nil{}
    case SkillRow{n, d, _} <> t:
      SkillRow{n, d, Nat.is_eq(k, 1n)} <> Skills.mark(t, Nat.sub(k, 1n))

# what the composer offers now, the highlighted row marked (the first
# when the arrows moved past the end of a shorter list)
def Skills.marked(+rs: List<&2, SkillRow>, +at: Nat) -> List<&2, SkillRow>:
  Skills.mark(rs, Nat.add(Bool.pick(Nat, Nat.is_lt(at, List.length(&2, SkillRow, rs)), at, 0n), 1n))

# the slash commands a draft being typed offers, the first marked, from
# the draft alone (the browser's composer between the hub's renders,
# view.bend's View.local; the hub adds the thread's own skills)
def Slash.local(claude: Bool, +draft: String) -> List<&2, SkillRow>:
  Skills.marked(Slash.rows(SL.Slash.cmds(claude, String.drop(Skills.token(draft), 1n))), 0n)

def Slash.typing.d(+draft: String) -> Bool:
  SL.Slash.typing(draft)

def Ui.skills.k(shut: Bool, +st: M.State, +sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  match shut:
    case True{}:
      Nil{}
    case False{}:
      Skills.marked(Skills.all(st, sel, dr, nf), Skills.at.k(sel, dr, nf))

# what the composer shows: the rows, the highlighted one marked; Esc shuts
# the menu ("skill-close", info "sk.off" is the draft it was shut on) and
# it stays shut until the draft changes (the web reads this too)
def Skills.shown(+st: M.State, +sel: String, +dr: Map<&2, String>, +nf: Map<&2, String>) -> List<&2, SkillRow>:
  Ui.skills.k(String.eq(Str.get(nf, "sk.off"), Skills.on(sel, Str.get(dr, sel))), st, sel, dr, nf)

def Ui.skills(u: Ui) -> List<&2, SkillRow>:
  match u:
    case Ui{+st, +sel, +dr, _, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      Skills.shown(st, sel, dr, nf)

def Skills.lit(rs: List<&2, SkillRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case SkillRow{n, _, True{}} <> _:
      n
    case SkillRow{_, _, False{}} <> t:
      Skills.lit(t)

def Skills.idx(rs: List<&2, SkillRow>, +i: Nat) -> Nat:
  match rs:
    case Nil{}:
      0n
    case SkillRow{_, _, True{}} <> _:
      i
    case SkillRow{_, _, False{}} <> t:
      Skills.idx(t, 1n+i)

# the arrow keys move the highlight ("down", else up), wrapping round
def Skills.step(+down: Bool, +i: Nat, +n: Nat) -> Nat:
  match n:
    case 0n:
      0n
    case _:
      Nat.mod(Bool.pick(Nat, down, Nat.add(i, 1n), Nat.add(i, Nat.sub(n, 1n))), n)

def Skills.moved(u: Ui, +i: Nat) -> Ui:
  match u:
    case Ui{st, +sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note,
        Map.set(&2, String, Map.set(&2, String, nf, "sk.at", Nat.show(i)), "sk.on", Skills.on(sel, Str.get(dr, sel))), ob, tr}

def Skills.move(+u: Ui, +v: String) -> Act:
  Act{Skills.moved(u, Skills.step(String.eq(v, "down"), Skills.idx(Ui.skills(u), 0n), List.length(&2, SkillRow, Ui.skills(u)))), Nil{}}

def Skills.named(rs: List<&2, SkillRow>, +v: String) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case SkillRow{n, _, _} <> t:
      Bool.or(String.eq(n, v), Skills.named(t, v))

# the word a completion puts in: the one clicked, else the highlighted
def Skills.pick(+rs: List<&2, SkillRow>, +v: String) -> String:
  Bool.pick(String, Skills.named(rs, v), v, Skills.lit(rs))

# the draft with its last word replaced by a completion
def Skills.complete(+draft: String, +word: String) -> String:
  String.take(draft, Nat.sub(String.length(draft), String.length(Skills.token(draft)))) ++ word ++ " "

# the composer keeps the keyboard (the web draws the page again)
def Ui.skill.k(+word: String, +u: Ui) -> Act:
  Bool.pick(Act, String.is_empty(word), Act{u, Nil{}}, Ui.also(Ui.draft.set(u, Skills.complete(Ui.draft_text(u), word)), [Focus{"composer"}]))

# complete the word being typed
def Ui.skill(+u: Ui, +v: String) -> Act:
  Ui.skill.k(Skills.pick(Ui.skills(u), v), u)

def Skills.need(+u: Ui) -> Bool:
  Bool.and(Bool.or(Skills.typing(Ui.draft_text(u)), SL.Slash.typing(Ui.draft_text(u))), Bool.not(String.eq(Ui.info(u, "skills.for"), Ui.sel(u))))

# Reasoning effort in the composer
# --------------------------------

# a thread's effort ("" for its provider's default)
def Effort.of(+st: M.State, +thread: String) -> String:
  M.Setting.get(st, AG.Effort.key(thread), "")

def Ui.effort(u: Ui) -> String:
  match u:
    case Ui{+st, +sel, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Effort.of(st, sel)

# the levels a provider offers, its default ("") first
def Effort.chips.of(+levels: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, List.is_empty(&2, String, levels), Nil{}, "" <> levels)

def Effort.known(m: Maybe<&2, J.Json>, +provider: String) -> List<&2, String>:
  match m:
    case Some{+j}:
      AG.Effort.keep(AG.Effort.levels(provider), Models.strs(J.Json.elems(j)))
    case None{}:
      AG.Effort.levels(provider)

# Thinking's opt-in (model.bend's Think), on Settings > Agents
def Think.label() -> String:
  "Show thinking"

def Think.note() -> String:
  "Ask agents for their thinking and show it in the thread, folded (Claude, Codex and Pi; a Claude session restarts to take it)"

def Effort.name(+e: String) -> String:
  Bool.pick(String, String.is_empty(e), "auto", e)

# Context usage: how much of its model's window the thread's session
# holds, after the model chip's label ("ctx 12% of 200k"; tokens alone
# while the window is unknown; nothing before the first reading)

def Usage.tokens(+n: Nat) -> String:
  Bool.pick(String, Nat.is_ge(n, 1000000n), Nat.show(Nat.div(n, 1000000n)) ++ "M",
    Bool.pick(String, Nat.is_ge(n, 1000n), Nat.show(Nat.div(n, 1000n)) ++ "k", Nat.show(n)))

def Usage.text(+used: Nat, +window: Nat) -> String:
  Bool.pick(String, Nat.is_eq(used, 0n), "",
    Bool.pick(String, Nat.is_eq(window, 0n), " · ctx " ++ Usage.tokens(used),
      " · ctx " ++ Nat.show(Nat.div(Nat.mul(used, 100n), window)) ++ "% of " ++ Usage.tokens(window)))

def Usage.tail(+st: M.State, +th: String) -> String:
  UZ.Use.tail(M.Thread.provider(Ui.thread(st, th)), M.Usage.used(M.Setting.get(st, M.Usage.key(th), "")),
    Usage.text(M.Usage.used(M.Setting.get(st, M.Usage.key(th), "")), M.Usage.window(M.Setting.get(st, M.Usage.key(th), ""))))

# the model chip's label with the thread's effort
def Effort.label(+label: String, +e: String) -> String:
  Bool.pick(String, String.is_empty(e), label, label ++ " · " ++ e)

def Asks.open.k(keep: Bool, a: M.Ask, rest: List<&2, M.Ask>) -> List<&2, M.Ask>:
  match keep:
    case True{}:
      a <> rest
    case False{}:
      rest

def Asks.open.go(as: List<&2, M.Ask>, +thread: String) -> List<&2, M.Ask>:
  match as:
    case Nil{}:
      Nil{}
    case M.Ask{i, +th, k, t, d, o, +op, an} <> rest:
      Asks.open.k(Bool.and(op, String.eq(th, thread)), M.Ask{i, th, k, t, d, o, op, an}, Asks.open.go(rest, thread))

# what a thread's agent waits on the user for
def Asks.open(s: M.State, +thread: String) -> List<&2, M.Ask>:
  Asks.open.go(M.Asks.all(s), thread)

# the sidebar marks a thread that waits on the user
def Asks.waiting(s: M.State, +thread: String) -> Bool:
  Bool.not(List.is_empty(&2, M.Ask, Asks.open(s, thread)))

# Links between threads
# ---------------------

type Link2 is Data:
  Link2{from: String, to: String, at: Nat}

def Links.recent.k(keep: Bool, +from: String, +to: String, +at: Nat, rest: List<&2, Link2>) -> List<&2, Link2>:
  match keep:
    case True{}:
      Link2{from, to, at} <> rest
    case False{}:
      rest

def Links.recent.f(+from: String, +to: String, +at: Nat, rest: List<&2, Link2>) -> List<&2, Link2>:
  Links.recent.k(Bool.not(String.is_empty(from)), from, to, at, rest)

# a message young enough to draw: its sender read from its id (only then:
# reading every message's id each frame was most of the sidebar's links)
def Links.recent.y(young: Bool, +id: String, +to: String, +at: Nat, rest: List<&2, Link2>) -> List<&2, Link2>:
  match young:
    case True{}:
      Links.recent.f(M.Link.from(id), to, at, rest)
    case False{}:
      rest

# (live: the entry before was under 75 minutes old; the log is in time
# order, so the walk stops at the first older one instead of going
# through the whole log each frame)
def Links.recent.go(es: List<&2, M.Entry>, +now: Nat, live: Bool) -> List<&2, Link2>:
  match es live:
    case _ False{}:
      Nil{}
    case Nil{} _:
      Nil{}
    case M.Msg{+id, +to, _, _, +at} <> t True{}:
      Links.recent.y(Nat.is_le(now, Nat.add(at, 900n)), id, to, at, Links.recent.go(t, now, Nat.is_le(now, Nat.add(at, 4500n))))
    case M.Act{_, _, _, _, _, +at} <> t True{}:
      Links.recent.go(t, now, Nat.is_le(now, Nat.add(at, 4500n)))

# messages between threads in the last 15 minutes (the sidebar draws them)
def Links.recent(s: M.State, +now: Nat) -> List<&2, Link2>:
  Links.recent.go(M.State.links(s), now, True{})

# Terminals
# ---------
# Each thread's terminal screen is folded here from the shell's output (the
# same bytes reach every client, so every client's screen agrees).

def Term.bytes(xs: List<&2, J.Json>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      U32.from_nat(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(Some{x}, "0")), 0n)) <> Term.bytes(t)

def Term.nums(bs: List<&2, U32>) -> List<&2, J.Json>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      J.Num{U32.show(b)} <> Term.nums(t)

def Term.get.k(r: Map<&2, VT.Vt> & VT.Vt) -> VT.Vt:
  (_, v) = r
  v

# a thread's screen (a blank 100x24 one before its shell spoke)
def Term.get(+m: Map<&2, VT.Vt>, +thread: String) -> VT.Vt:
  Term.get.k(Map.get(VT.Vt, VT.Vt.new(100, 24), m, thread))

def Ui.term(u: Ui, +thread: String) -> VT.Vt:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, tr}:
      Term.get(MailvLocal.terms(tr), thread)

def Ui.term_set(u: Ui, +thread: String, v: VT.Vt) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, +tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, MailvLocal.with_terms(tr, Map.set(&2, VT.Vt, MailvLocal.terms(tr), thread, v))}

def Ui.term_feed(u: Ui, +thread: String, bs: List<&2, U32>) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, +tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, MailvLocal.with_terms(tr, Map.set(&2, VT.Vt, MailvLocal.terms(tr), thread, VT.Vt.feed(Term.get(MailvLocal.terms(tr), thread), bs)))}

# a chat terminal's snapshot ("reset": [cols, rows]) starts its screen
# over; other bytes go onto it
def Emb.recv(r: List<&2, J.Json>, +u: Ui, +key: String, bs: List<&2, U32>) -> Ui:
  match r:
    case +c <> +w <> _:
      Ui.term_set(u, key, VT.Vt.feed(VT.Vt.new(U32.from_nat(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(Some{c}, "80")), 80n)),
        U32.from_nat(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(Some{w}, "24")), 24n))), bs))
    case _:
      Ui.term_feed(u, key, bs)

def Ui.term_replies.k(none: Bool, +u: Ui, +thread: String, +bs: List<&2, U32>, cs: List<&2, Cmd>) -> Act:
  match none:
    case True{}:
      Act{u, cs}
    case False{}:
      Ui.also(Ui.rpc(Ui.term_set(u, thread, VT.Vt.drain(Ui.term(u, thread))), "term.input", [J.KV{"thread", J.Str{thread}}, J.KV{"b", J.Json.arr(Term.nums(bs))}]), cs)

def Ui.term_replies(+u: Ui, +thread: String, cs: List<&2, Cmd>) -> Act:
  Ui.term_replies.k(List.is_empty(&2, U32, VT.Vt.replies(Ui.term(u, thread))), u, thread, VT.Vt.replies(Ui.term(u, thread)), cs)

# a terminal that asked something (cursor position, device attributes)
# gets its answer sent back to its shell
def Ui.recv_act.t(term: Bool, +thread: String, a: Act) -> Act:
  match term a:
    case True{} Act{u, cs}:
      Ui.term_replies(u, thread, cs)
    case False{} _:
      a

# open the selected thread's terminal at this size (or resize it)
def Ui.term_open(u: Ui, +cols: U32, +rows: U32) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, +tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, MailvLocal.with_terms(tr, Map.set(&2, VT.Vt, MailvLocal.terms(tr), sel, VT.Vt.resize(Term.get(MailvLocal.terms(tr), sel), cols, rows)))},
        "term.open", [J.KV{"thread", J.Str{sel}}, J.KV{"cols", J.Num{U32.show(cols)}}, J.KV{"rows", J.Num{U32.show(rows)}}])

# keys typed into the selected thread's terminal
def Ui.term_keys(u: Ui, bs: List<&2, U32>) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}, "term.input", [J.KV{"thread", J.Str{sel}}, J.KV{"b", J.Json.arr(Term.nums(bs))}])

def Keys.pos(xs: List<&2, String>, +x: String) -> Nat:
  C.Codec.pos(xs, x)

def Ui.state(u: Ui) -> M.State:
  match u:
    case Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      st

# the browser page the selected thread's agent drives (BT.World.page): a
# bot's own, else the thread's; its frames arrive as "shot.<page>"
def Ui.page(+u: Ui) -> String:
  BT.World.page(M.State.bots(Ui.state(u)), Ui.sel(u))

# Command palette
# ---------------
# ctrl+k: threads and actions, filtered by what was typed. Its query and
# selection live in the drafts map under keys no thread id can take.

type PItem is Data:
  PItem{label: String, action: String, value: String}

def Palette.query(u: Ui) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, "@palette")

def Palette.index(u: Ui) -> Nat:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Maybe.default(&2, Nat, Nat.read(Str.get(dr, "@palette.i")), 0n)

# "" (commands), "search" (threads and messages) or "files"
def Palette.mode(u: Ui) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, "@palette.mode")

def Palette.open(u: Ui, +mode: String) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui.flag_on(Ui{st, sel, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, dr, "@palette", ""), "@palette.i", "0"), "@palette.mode", mode), live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}, "palette")

def Palette.set(u: Ui, +q: String, +i: Nat) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, "@palette", q), "@palette.i", Nat.show(i)), live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

def Palette.threads(+l: I.Loc, ts: List<&2, M.Thread>) -> List<&2, PItem>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{id, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      PItem{TTL.Title.shown(l, ti), "select", id} <> Palette.threads(l, t)

def Palette.actions(+l: I.Loc, +pid: String) -> List<&2, PItem>:
  [PItem{MSG.T.palette.newthread(l), "new-thread", pid}, PItem{MSG.T.palette.diff(l), "diff", ""}, PItem{MSG.T.palette.terminal(l), "key:term", ""},
   PItem{MSG.T.palette.attach(l), "key:attach", ""}, PItem{MSG.T.palette.commit(l), "git", "commit_push"}, PItem{MSG.T.palette.pr(l), "git", "commit_push_pr"}, PItem{MSG.T.palette.revert(l), "revert", "ask"},
   PItem{MSG.T.palette.settle(l), "settle", ""}, PItem{MSG.T.palette.pin(l), "pin", ""}, PItem{MSG.T.palette.archive(l), "archive", ""}, PItem{MSG.T.palette.unarchive(l), "unarchive", ""}, PItem{MSG.T.palette.delete(l), "delete", ""},
   PItem{MSG.T.palette.snooze.hour(l), "snooze", "1h"}, PItem{MSG.T.palette.snooze.tomorrow(l), "snooze", "tomorrow"}, PItem{MSG.T.palette.snooze.week(l), "snooze", "1w"}, PItem{MSG.T.palette.unsnooze(l), "snooze", "off"},
   PItem{MSG.T.palette.search(l), "key:search", ""}, PItem{MSG.T.palette.findfile(l), "key:files", ""},
   PItem{MSG.T.palette.viewer(l), "panel", ""}, PItem{MSG.T.palette.kicad(l), "kicad-open", ""}, PItem{MSG.T.palette.kicad10(l), "kicad-export10", ""}, PItem{MSG.T.palette.settings(l), "flag", "settings"}, PItem{MSG.T.palette.update(l), "update", ""}]

def Palette.words(ws: List<&2, String>, +l: String) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case +w <> t:
      Bool.and(Bool.or(String.is_empty(w), String.contains(l, w)), Palette.words(t, l))

# does every word of the query appear in the label (any case)?
def Palette.hit(+q: String, +label: String) -> Bool:
  Palette.words(String.split(String.to_lower(q), ' '), String.to_lower(label))

def Palette.filter.k(keep: Bool, p: PItem, rest: List<&2, PItem>) -> List<&2, PItem>:
  match keep:
    case True{}:
      p <> rest
    case False{}:
      rest

def Palette.keep(ps: List<&2, PItem>, +q: String) -> List<&2, PItem>:
  match ps:
    case Nil{}:
      Nil{}
    case PItem{+l, a, v} <> t:
      Palette.filter.k(Palette.hit(q, l), PItem{l, a, v}, Palette.keep(t, q))

# a row's name: its label before its keys ("New thread · Ctrl+N")
def Palette.name(+label: String) -> String:
  String.trim(Maybe.default(&2, String, List.head(&2, String, String.split(label, '·')), label))

# does every word of the (lower-case) query start a word of the label?
def Palette.starts(ws: List<&2, String>, +l: String) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case +w <> t:
      Bool.and(Bool.or(String.is_empty(w), Bool.or(String.starts_with(l, w), Bool.or(String.contains(l, " " ++ w),
        Bool.or(String.contains(l, "/" ++ w), Bool.or(String.contains(l, "-" ++ w), String.contains(l, "(" ++ w)))))), Palette.starts(t, l))

def Palette.rank.k(+q: String, +name: String, +l: String) -> Nat:
  Bool.pick(Nat, String.eq(name, q), 0n, Bool.pick(Nat, String.starts_with(name, q), 1n,
    Bool.pick(Nat, Palette.starts(String.split(q, ' '), l), 2n, 3n)))

# how well a label the query hits answers it, best first: 0 its name is the
# query, 1 its name starts with it, 2 every word of the query starts a word
# of it, 3 the words are only inside it (any case)
def Palette.rank(+q: String, +label: String) -> Nat:
  Palette.rank.k(String.trim(String.to_lower(q)), String.to_lower(Palette.name(label)), String.to_lower(label))

type PRanked is Data:
  PRanked{rank: Nat, item: PItem}

def Palette.scored(ps: List<&2, PItem>, +q: String) -> List<&2, PRanked>:
  match ps:
    case Nil{}:
      Nil{}
    case PItem{+l, a, v} <> t:
      PRanked{Palette.rank(q, l), PItem{l, a, v}} <> Palette.scored(t, q)

def Palette.tier.k(keep: Bool, p: PItem, rest: List<&2, PItem>) -> List<&2, PItem>:
  match keep:
    case True{}:
      p <> rest
    case False{}:
      rest

# the rows of one rank, in the list's own order
def Palette.tier(rs: List<&2, PRanked>, +n: Nat) -> List<&2, PItem>:
  match rs:
    case Nil{}:
      Nil{}
    case PRanked{r, p} <> t:
      Palette.tier.k(Nat.is_eq(r, n), p, Palette.tier(t, n))

def Palette.tiers(+rs: List<&2, PRanked>) -> List<&2, PItem>:
  List.concat(&2, PItem, [Palette.tier(rs, 0n), Palette.tier(rs, 1n), Palette.tier(rs, 2n), Palette.tier(rs, 3n)])

def Palette.ranked(empty: Bool, ps: List<&2, PItem>, +q: String) -> List<&2, PItem>:
  match empty:
    case True{}:
      ps
    case False{}:
      Palette.tiers(Palette.scored(Palette.keep(ps, q), q))

# the rows whose label holds every word of the query, the best answers
# first (Palette.rank: the exact name, then names that start with it, then
# word starts, then the rest), each rank in the list's own order
def Palette.filter(ps: List<&2, PItem>, +q: String) -> List<&2, PItem>:
  Palette.ranked(String.is_empty(String.trim(q)), ps, q)

# what the command palette offers for this query, actions first
def Palette.cmds.loc(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  Palette.filter(List.append(&2, PItem, Palette.actions(l, Ui.thread_project(Ui.thread(Ui.state(u), Ui.sel(u)))),
    Palette.threads(l, Kids.keep(Ui.threads(Ui.state(u)), Kids.set(Ui.state(u))))), Palette.query(u))

def Palette.cmds(+u: Ui) -> List<&2, PItem>:
  Palette.cmds.loc(Lang.loc(u), u)

def Palette.at(ps: List<&2, PItem>, i: Nat) -> PItem:
  match ps i:
    case p <> _ 0n:
      p
    case _ <> t 1n+j:
      Palette.at(t, j)
    case Nil{} _:
      PItem{"", "", ""}

# Thread search (ctrl+shift+f)
# ----------------------------
# Titles (fuzzy, FZ.Fuzzy) and message text (every word of the query) of
# every thread; a thread shows once, with the newest message line that
# matches when its title does not.

def Search.line(ls: List<&2, String>, +q: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, Palette.hit(q, l), String.trim(l), Search.line(t, q))

def Search.put.k(r: Map<&2, String> & String, +th: String, +text: String, +q: String) -> Map<&2, String>:
  (+m, +cur) = r
  Bool.pick(Map<&2, String>, String.is_empty(cur), Map.set(&2, String, m, th, Search.line(DF.Diff.lines(text), q)), m)

def Search.put(hit: Bool, m: Map<&2, String>, +th: String, +text: String, +q: String) -> Map<&2, String>:
  match hit:
    case True{}:
      Search.put.k(Map.get(String, "", m, th), th, text, q)
    case False{}:
      m

# thread -> the newest matching line of its messages
def Search.snips(es: List<&2, M.Entry>, +q: String, m: Map<&2, String>) -> Map<&2, String>:
  match es:
    case Nil{}:
      m
    case M.Msg{_, +th, _, +text, _} <> t:
      Search.snips(t, q, Search.put(Palette.hit(q, text), m, th, text, q))
    case M.Act{_, _, _, _, _, _} <> t:
      Search.snips(t, q, m)

def Search.one(+l: I.Loc, t: M.Thread, +q: String, +snips: Map<&2, String>, +rest: List<&2, PItem>) -> List<&2, PItem>:
  match t:
    case M.Thread{+id, _, +ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +de}:
      Bool.pick(List<&2, PItem>, de, rest,
        Bool.pick(List<&2, PItem>, Nat.is_gt(FZ.Fuzzy.score(ti, q), 0n), PItem{TTL.Title.shown(l, ti), "select", id} <> rest,
          Bool.pick(List<&2, PItem>, String.is_empty(Str.get(snips, id)), rest, PItem{TTL.Title.shown(l, ti) ++ "  ·  " ++ Str.get(snips, id), "select", id} <> rest)))

def Search.all(+l: I.Loc, ts: List<&2, M.Thread>, +q: String, +snips: Map<&2, String>) -> List<&2, PItem>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> rest:
      Search.one(l, t, q, snips, Search.all(l, rest, q, snips))

def Search.newer(a: M.Thread, b: M.Thread) -> Bool:
  match a b:
    case M.Thread{_, _, _, _, +x, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} M.Thread{_, _, _, _, +y, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Nat.is_ge(x, y)

def Search.items.k(+l: I.Loc, empty: Bool, +q: String, +st: M.State) -> List<&2, PItem>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Search.all(l, List.sort(~M.Thread, ~Search.newer, Ui.threads(st)), q, Search.snips(Ui.entries(st), q, Map.new(&2, String)))

# threads whose title or messages hold every word of the query, newest first
def Search.items(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  Search.items.k(l, String.is_empty(String.trim(Palette.query(u))), Palette.query(u), Ui.state(u))

# File picker (ctrl+p)
# --------------------
# The selected thread's files (files.list: git ls-files, else a bounded
# walk); picking one mentions it in the draft as @path.

type FRow is Data:
  FRow{score: Nat, path: String}

def FRow.before(a: FRow, b: FRow) -> Bool:
  match a b:
    case FRow{x, _} FRow{y, _}:
      Nat.is_ge(x, y)

def Files.keep(+sc: Nat, +p: String, +rest: List<&2, FRow>) -> List<&2, FRow>:
  Bool.pick(List<&2, FRow>, Bool.and(Nat.is_gt(sc, 0n), Bool.not(String.is_empty(p))), FRow{sc, p} <> rest, rest)

def Files.scored(ps: List<&2, String>, +q: String) -> List<&2, FRow>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Files.keep(FZ.Fuzzy.score(p, q), p, Files.scored(t, q))

def Files.rows(rs: List<&2, FRow>) -> List<&2, PItem>:
  match rs:
    case Nil{}:
      Nil{}
    case FRow{_, +p} <> t:
      PItem{p, "mention", p} <> Files.rows(t)

# the files that fuzzily match the query, best first
def Files.pick(+ps: List<&2, String>, +q: String) -> List<&2, PItem>:
  Files.rows(List.take(&2, FRow, List.sort(~FRow, ~FRow.before, Files.scored(ps, q)), 200n))

def Files.items.k(same: Bool, +files: String, +q: String) -> List<&2, PItem>:
  match same:
    case True{}:
      Files.pick(String.lines(files), q)
    case False{}:
      Nil{}

def Files.items(+u: Ui) -> List<&2, PItem>:
  Files.items.k(String.eq(Ui.info(u, "files.thread"), Ui.sel(u)), Ui.info(u, "files"), Palette.query(u))

# what the palette offers in its mode
def Palette.items.k(+l: I.Loc, i: Nat, +u: Ui) -> List<&2, PItem>:
  match i:
    case 0n:
      Search.items(l, u)
    case 1n:
      Files.items(u)
    case _:
      Palette.cmds.loc(l, u)

def Palette.items.loc(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  Palette.items.k(l, Keys.pos(["search", "files"], Palette.mode(u)), u)

def Palette.items(+u: Ui) -> List<&2, PItem>:
  Palette.items.loc(Lang.loc(u), u)

# the kind of a row, shown at its right
def Palette.kind(+a: String) -> String:
  Bool.pick(String, String.eq(a, "select"), "thread", Bool.pick(String, String.eq(a, "mention"), "file", "action"))

def Palette.place.k(thread: Bool, +st: M.State, +v: String) -> String:
  match thread:
    case True{}: Ui.project_title(st, Ui.thread_project(Ui.thread(st, v)))
    case False{}: ""

# the project a thread's row names beside its title ("" for any other row),
# so a thread called "New thread" never reads as the New thread command
def Palette.place(+st: M.State, +a: String, +v: String) -> String:
  Palette.place.k(String.eq(a, "select"), st, v)

# Row status
# ----------

# the head of an approval card, one text for every client: "Send this email?"
# for gmail_send ("Send this draft?" when its detail starts with "Draft:"),
# "Create/Change/Delete this event?" for calendar_create/update/delete,
# "Allow <tool>?" for any other tool
def Ask.head.tool(+l: I.Loc, +tool: String, +d: String) -> String:
  Bool.pick(String, String.eq(tool, "gmail_send"), Bool.pick(String, String.starts_with(d, "Draft:"), MSG.T.ask.send.draft(l), MSG.T.ask.send.email(l)),
    Bool.pick(String, String.eq(tool, "calendar_create"), MSG.T.ask.event.create(l),
      Bool.pick(String, String.eq(tool, "calendar_update"), MSG.T.ask.event.change(l),
        Bool.pick(String, String.eq(tool, "calendar_delete"), MSG.T.ask.event.delete(l),
          Bool.pick(String, String.eq(tool, "order_quote"), "Send these files for a quote?", MSG.T.ask.allow(l, tool))))))

# the kind an ask's buttons follow: an approval to send an order's files
# reads Send / Not now (accept / decline) instead of Allow / Deny
def Ask.bkind(+k: String, +tool: String) -> String:
  Bool.pick(String, Bool.and(String.eq(k, "approval"), String.eq(tool, "order_quote")), "send", k)

def Ask.head.loc(+l: I.Loc, +k: String, +tool: String, +d: String) -> String:
  Bool.pick(String, String.eq(k, "plan"), MSG.T.ask.plan(l), Bool.pick(String, String.eq(k, "input"), MSG.T.ask.input(l), Ask.head.tool(l, tool, d)))

def Ask.head(k: String, tool: String, d: String) -> String:
  Ask.head.loc(I.En{}, k, tool, d)

def Asks.kind(as: List<&2, M.Ask>, +k: String) -> Bool:
  match as:
    case Nil{}:
      False{}
    case M.Ask{_, _, +x, _, _, _, _, _} <> t:
      Bool.or(String.eq(x, k), Asks.kind(t, k))

# a question for the person: an agent's, a proposed plan, or a browser task
# paused for help (docs/teach.md)
def Asks.question(+as: List<&2, M.Ask>) -> Bool:
  Bool.or(Bool.or(Asks.kind(as, "input"), Asks.kind(as, "plan")), Asks.kind(as, "browser"))

# what a thread's dot shows (M.Status.of has the order)
# (a thread's open asks are looked up once: a row asks three questions of
# them, and the sidebar has a row a thread)
def Side.status.k(+st: M.State, +id: String, turn: M.TurnState, +pending: Nat, +as: List<&2, M.Ask>) -> M.Status:
  M.Status.wait.f(Nat.is_gt(M.Work.rows(st, id), 0n), M.Status.of(Bool.or(Nat.is_gt(pending, 0n), Asks.kind(as, "approval")),
    Asks.question(as), turn,
    M.Queue.starting(st, id)))

def Side.status(+st: M.State, t: M.Thread) -> M.Status:
  match t:
    case M.Thread{+id, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Side.status.k(st, id, turn, pending, Asks.open(st, id))

# how many of a thread's child tasks are not finished
def Side.live(ts: List<&2, M.Task>) -> Nat:
  match ts:
    case Nil{}:
      0n
    case M.Task{_, _, _, _, st, _, _, _, _} <> t:
      Nat.add(Bool.pick(Nat, M.TaskState.terminal(st), 0n, 1n), Side.live(t))

# what the line at the end of a thread's timeline says (M.Phase.of has the order)
def Side.phase.k(+st: M.State, +id: String, turn: M.TurnState, +pending: Nat, +as: List<&2, M.Ask>) -> M.Phase:
  M.Phase.of(Bool.or(Nat.is_gt(pending, 0n), Asks.kind(as, "approval")),
    Asks.question(as), Nat.add(Side.live(Tasks.of(st, id)), M.Work.rows(st, id)), turn,
    M.Queue.starting(st, id))

def Side.phase(+st: M.State, t: M.Thread) -> M.Phase:
  match t:
    case M.Thread{+id, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Side.phase.k(st, id, turn, pending, Asks.open(st, id))

# Side.status from the index
def Side.status.ix(+ix: SIx, +st: M.State, t: M.Thread) -> M.Status:
  match t:
    case M.Thread{+id, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Side.status.k(st, id, turn, pending, SIx.open(ix, id))

def Side.phase.ix.k(+st: M.State, +ix: SIx, +id: String, turn: M.TurnState, +pending: Nat, +as: List<&2, M.Ask>) -> M.Phase:
  M.Phase.of(Bool.or(Nat.is_gt(pending, 0n), Asks.kind(as, "approval")),
    Asks.question(as), Nat.add(SIx.kids(ix, id), SIx.work(ix, id)), turn,
    M.Queue.starting(st, id))

# Side.phase from the index (law side_phase_ix_eq)
def Side.phase.ix(+ix: SIx, +st: M.State, t: M.Thread) -> M.Phase:
  match t:
    case M.Thread{+id, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Side.phase.ix.k(st, ix, id, turn, pending, SIx.open(ix, id))

# when a thread's snooze ends (0: not snoozed)
def Side.wakes.span(l: I.Loc, +d: Nat) -> String:
  match l:
    case I.Ja{}: I.I18n.dur(I.Ja{}, d)
    case _: I.I18n.ago(I.En{}, d, 0n)

def Side.wakes.k(+l: I.Loc, soon: Bool, +d: Nat) -> String:
  match soon:
    case True{}: MSG.T.sidebar.wakes.soon(l)
    case False{}: MSG.T.sidebar.wakes(l, Side.wakes.span(l, d))

# a snoozed row's time says when it wakes ("wakes in 5m"), never an age
def Side.wakes(+l: I.Loc, +until: Nat, +now: Nat) -> String:
  Side.wakes.k(l, Nat.is_lt(Nat.sub(until, now), 60n), Nat.sub(until, now))

def Side.settled.k(sh: M.Shelf) -> Bool:
  match sh:
    case M.Settled{}: True{}
    case _: False{}

# a thread that is settled though it shows among the live rows (a parent a
# live subthread keeps there): its row says "settled" where its age goes
def Side.settled(+t: M.Thread) -> Bool:
  Side.settled.k(M.Shelf.of(t))

# a row's note where its age goes: when a snoozed row wakes, "settled" on a
# live shelf's settled row, else its age
def Side.note(+l: I.Loc, snoozed: Bool, settled: Bool, +until: Nat, +now: Nat, +at: Nat) -> String:
  match snoozed settled:
    case True{} _: Side.wakes(l, until, now)
    case False{} True{}: MSG.T.sidebar.row.settled(l)
    case False{} False{}: I.I18n.ago(l, now, at)

def Side.until(+st: M.State, +id: String) -> Nat:
  M.Nav.until(M.Nav.get(M.State.navs(st), id))

# Scratch state
# -------------
# Small client-only values (a drag in progress) kept in the drafts map
# under "@" keys, which no thread id takes.

def Ui.scratch_set(u: Ui, +k: String, v: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, k, v), live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

# Device language. Only apply writes the resolved tag; hosts call on input changes.
def Lang.ready() -> Bool:
  False{}

def Lang.normalize.k(en: Bool, ja: Bool) -> String:
  match en ja:
    case True{} _: "en"
    case False{} True{}: "ja"
    case False{} False{}: "auto"

def Lang.normalize(+s: String) -> String:
  Lang.normalize.k(String.eq(s, "en"), String.eq(s, "ja"))

def Lang.resolve.k(+l: I.Loc, allow: Bool) -> String:
  match l allow:
    case I.Ja{} True{}: "ja"
    case _ _: "en"

def Lang.resolve(setting: String, os: String, preview: Bool) -> String:
  Lang.resolve.k(I.I18n.resolve(setting, os), Bool.or(Lang.ready(), preview))

def Lang.choice(+u: Ui) -> String:
  Lang.normalize(Ui.scratch(u, "@lang"))

def Lang.preview(+u: Ui) -> Bool:
  String.eq(Ui.scratch(u, "@lang.preview"), "1")

def Lang.apply(+u: Ui, +setting: String, +os: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@lang", Lang.normalize(setting)), "@lang.os", os),
    "@lang.tag", Lang.resolve(Lang.normalize(setting), os, Lang.preview(u)))

def Lang.offered.k(allow: Bool) -> List<&2, String>:
  match allow:
    case True{}: ["auto", "en", "ja"]
    case False{}: ["auto", "en"]

def Lang.offered(+u: Ui) -> List<&2, String>:
  Lang.offered.k(Bool.or(Lang.ready(), Lang.preview(u)))

def Lang.label.k(k: Nat, +l: I.Loc) -> String:
  match k:
    case 0n: MSG.T.settings.lang.en(l)
    case 1n: MSG.T.settings.lang.ja(l)
    case _: MSG.T.settings.lang.auto(l)

def Lang.label(+l: I.Loc, v: String) -> String:
  Lang.label.k(C.Codec.pos(["en", "ja"], v), l)

def Lang.agent.on(u: Ui) -> Bool:
  +s = M.Setting.get(Ui.state(u), "agent.lang", "off")
  Bool.or(String.eq(s, "en"), String.eq(s, "ja"))

def Lang.agent.value(on: Bool, u: Ui) -> String:
  match on:
    case True{}: I.I18n.tag(Lang.loc(u))
    case False{}: "off"

def Lang.follow.add(a: Act, cs: List<&2, Cmd>) -> Act:
  match a:
    case Act{u, cs2}: Act{u, List.append(&2, Cmd, cs, cs2)}

# "Agents reply in my language" follows this device's language: when the
# person changes it while the setting is on, agent.lang moves to the new tag
def Lang.follow(on: Bool, a: Act) -> Act:
  match on a:
    case True{} Act{+u, cs}: Lang.follow.add(Ui.setting(u, "agent.lang=" ++ Ui.scratch(u, "@lang.tag")), cs)
    case False{} a: a

def Lang.act(k: Nat, +u: Ui, +value: String) -> Act:
  match k:
    case 0n:
      Lang.follow(Lang.agent.on(u), Act{Lang.apply(u, value, Ui.scratch(u, "@lang.os")), [Keep{"@lang", Lang.normalize(value)}]})
    case _:
      Act{Lang.apply(u, Lang.choice(u), value), Nil{}}

# Creating a thread
# -----------------
# New Thread, Fork and adding a project ask the hub for a thread; the hub
# answers with the thread it made ({"t":"reply", "thread"}: Reply.made).
# Only that answer selects it, and only on the hub and connection that
# asked ("@want.new" holds the request's id, "@want.new.at" where it went):
# another client's new thread, a replayed log or an older request's
# answer selects nothing (laws want_new_*, made_*, replay_cannot_select).

# Browser pages on show (server's castio.bend)
# ---------------------------------------------
# A client showing the selected thread's Browser tab tells the hub so with
# every fourth frame of that page it hears (browser.seen): the hub then
# sends it every frame of the page and the page is cast faster. (The
# window says it as each frame lands instead: main.bend's Shot.seen.)

# the page the Browser tab shows ("" when the tab is not on show)
def Ui.cast.page(+u: Ui) -> String:
  Bool.pick(String, Bool.and(Ui.panel(u), String.eq(Ui.info(u, "view.kind:" ++ Ui.sel(u)), "browser")), Ui.page(u), "")

def Ui.cast.n(+n: Nat, +u: Ui, +page: String) -> Act:
  Bool.pick(Act, Nat.is_eq(n, 1n), Ui.rpc(u, "browser.seen", [J.KV{"page", J.Str{page}}]), Act{u, Nil{}})

def Ui.cast.k(on: Bool, +u: Ui, +page: String) -> Act:
  match on:
    case True{}:
      Ui.cast.n(Nat.mod(Nat.add(M.Switch.num(Ui.scratch(u, "@cast.n"), 0n), 1n), 4n), Ui.scratch_set(u, "@cast.n", Nat.show(Nat.mod(Nat.add(M.Switch.num(Ui.scratch(u, "@cast.n"), 0n), 1n), 4n))), page)
    case False{}:
      Act{u, Nil{}}

# the page on show: the selected thread's Browser tab's, else a bot's
# Browser tab's (the bot's own page)
def Ui.shown.page.k(bot: Bool, +u: Ui) -> String:
  match bot:
    case True{}:
      Ui.scratch(u, "@bot")
    case False{}:
      Ui.cast.page(u)

def Ui.shown.page(+u: Ui) -> String:
  Ui.shown.page.k(Bool.and(String.eq(Ui.scratch(u, "@btab"), "browser"), Bool.not(String.is_empty(Ui.scratch(u, "@bot")))), u)

# the native window shows its own hub's pages itself (core/cef.bend), so
# it takes no frames of them
def Ui.cast.allowed(+u: Ui) -> Bool:
  CF.Cef.cast(Bool.and(String.eq(Ui.scratch(u, "@cef.window"), "1"), CF.Cef.page_local(Ui.shown.page(u))), Ui.info(u, "browser.engine.live"), Ui.info(u, "local.remote.url"))

# a frame of page heard ({"t":"shot"}): said seen when the tab shows it
def Ui.cast.seen(+u: Ui, +page: String) -> Act:
  Ui.cast.k(Bool.and(Ui.cast.allowed(u), Bool.and(Bool.not(String.is_empty(page)), String.eq(page, Ui.shown.page(u)))), u, page)

# An unwatched page is not cast at all (the helper stops its screencast and
# keeps a still every few seconds), so a client cannot wait for a frame to
# say it shows one: an action that brings a page on show says so at once
def Ui.cast.add(a: Act, cs: List<&2, Cmd>) -> Act:
  match a:
    case Act{u, c2}:
      Act{u, List.append(&2, Cmd, cs, c2)}

def Ui.cast.now.k(go: Bool, a: Act, +page: String) -> Act:
  match go a:
    case True{} Act{+u, cs}:
      Ui.cast.add(Ui.rpc(Ui.scratch_set(u, "@cast.n", "1"), "browser.seen", [J.KV{"page", J.Str{page}}]), cs)
    case False{} _:
      a

def Ui.cast.now.p(+before: String, a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Ui.cast.now.k(Bool.and(Ui.cast.allowed(u), Bool.and(Bool.not(String.is_empty(Ui.shown.page(u))), Bool.not(String.eq(Ui.shown.page(u), before)))), Act{u, cs}, Ui.shown.page(u))

def Ui.cast.now(+u: Ui, a: Act) -> Act:
  Ui.cast.now.p(Ui.shown.page(u), a)

def Made.at(+u: Ui) -> String:
  Ui.info(u, "local.machine") ++ "|" ++ Ui.info(u, "local.generation")

def Made.rid(u: Ui) -> Nat:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, rid, _, _, _, _, _, _}:
      rid

# the next request is one that makes a thread
def Made.want(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@want.viewer", ""), "@want.new", Nat.show(Made.rid(u))), "@want.new.at", Made.at(u))

def Ui.new_thread.project(u: Ui, +pid: String) -> Act:
  Ui.rpc(Made.want(u), "thread.create", [J.KV{"project", J.Str{pid}}, J.KV{"title", J.Str{"New thread"}}])

# a fork of the selected thread; the new thread is selected when the
# hub's answer names it
# (a row's menu names its thread; the header's ▾ and the palette name none:
# the selected one)
def Ui.fork.of(+u: Ui, +value: String) -> String:
  Bool.pick(String, String.is_empty(value), Ui.sel(u), value)

# State put back from a cache (a page's saved log, a phone's saved state)
# is not this connection's: only a live log sets "@sync" again (Ui.reset),
# so the first-run tour waits for the hub (law onboard_cache_unsynced)
def Ui.unsync(u: Ui) -> Ui:
  Ui.scratch_set(u, "@sync", "")

# The active view
# ---------------
# The "sidebar.view" setting: "projects" (the default) or "active". The
# active view lists every thread on a live shelf of any project, those
# that need the person first (Urg.rank: an approval or a question, then a
# failure, then a finished or stopped turn, then work in progress, then
# idle; pinned ahead within each), newest first within each; then the
# threads settled in the last Quiet.days, faded, newest settled first,
# for bringing back. Snoozed threads return when they wake. A search
# keeps the threads whose title or project's name matches.

def Side.view(+st: M.State) -> String:
  Bool.pick(String, String.eq(M.Setting.get(st, "sidebar.view", ""), "active"), "active", "projects")

# how much a thread waits on the person (higher first)
def Urg.rank(p: M.Phase) -> Nat:
  match p:
    case M.PhApproval{}:
      8n
    case M.PhInput{}:
      7n
    case M.PhFailed{}:
      6n
    case M.PhComplete{}:
      5n
    case M.PhStopped{}:
      4n
    case M.PhWorking{}:
      2n
    case M.PhMonitoring{_}:
      2n
    case M.PhIdle{}:
      0n

# a row's key: its phase's rank, pinned ahead of unpinned
def Urg.key(p: M.Phase, pinned: Bool) -> Nat:
  Nat.add(Nat.mul(Urg.rank(p), 2n), Bool.pick(Nat, pinned, 1n, 0n))

# urg: Urg.key (0 for a faded row); at: its last activity, or when it
# settled; faded: settled, shown to bring back
def Urg.display(+st: M.State, +t: M.Thread, p: M.Phase, +pinned: Bool) -> Nat:
  Bool.pick(Nat, SG.Signal.display.stalled(st, M.Thread.id(t)), Nat.add(12n, Bool.pick(Nat, pinned, 1n, 0n)), Urg.key(p, pinned))

type ARow is Data:
  ARow{urg: Nat, at: Nat, thread: M.Thread, project: String, title: String, faded: Bool}

def ARow.before(a: ARow, b: ARow) -> Bool:
  match a b:
    case ARow{+ua, +aa, _, _, _, _} ARow{+ub, +ab, _, _, _, _}:
      Bool.pick(Bool, Nat.is_eq(ua, ub), Nat.is_ge(aa, ab), Nat.is_gt(ua, ub))

def ARow.faded(r: ARow) -> Bool:
  match r:
    case ARow{_, _, _, _, _, f}:
      f

def Active.pinned(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, p, _, _, _, _, _, _, _, _, _}:
      p

def Active.activity(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, a, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      a

def Active.settled_at(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, sa, _, _, _, _, _, _, _, _, _, _, _}:
      sa

def Active.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

def Active.live(ts: List<&2, M.Thread>, +st: M.State, +pid: String, +pt: String, acc: List<&2, ARow>) -> List<&2, ARow>:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Active.live(r, st, pid, pt, ARow{Urg.display(st, t, Side.phase(st, t), Active.pinned(t)), Active.activity(t), t, pid, pt, False{}} <> acc)

def Active.live.ix(ts: List<&2, M.Thread>, +st: M.State, +ix: SIx, +pid: String, +pt: String, acc: List<&2, ARow>) -> List<&2, ARow>:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Active.live.ix(r, st, ix, pid, pt, ARow{Urg.display(st, t, Side.phase.ix(ix, st, t), Active.pinned(t)), Active.activity(t), t, pid, pt, False{}} <> acc)

def Active.keep(keep: Bool, r: ARow, acc: List<&2, ARow>) -> List<&2, ARow>:
  match keep:
    case True{}:
      r <> acc
    case False{}:
      acc

def Active.settled(ts: List<&2, M.Thread>, +now: Nat, +pid: String, +pt: String, acc: List<&2, ARow>) -> List<&2, ARow>:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Active.settled(r, now, pid, pt, Active.keep(Quiet.recent(now, Active.settled_at(t)), ARow{0n, Active.settled_at(t), t, pid, pt, True{}}, acc))

type AParts is Data:
  AParts{live: List<&2, ARow>, faded: List<&2, ARow>}

def Active.gather(rs: List<&2, SRow>, +st: M.State, +now: Nat, acc: AParts) -> AParts:
  match rs:
    case Nil{}:
      acc
    case SRow{_, M.Project{+id, +ti, _, _, _}, M.Shelves{p, a, se, _, _}} <> t:
      match acc:
        case AParts{l, f}:
          Active.gather(t, st, now, AParts{Active.live(p, st, id, ti, Active.live(a, st, id, ti, l)), Active.settled(se, now, id, ti, f)})

def Active.gather.ix(rs: List<&2, SRow>, +st: M.State, +ix: SIx, +now: Nat, acc: AParts) -> AParts:
  match rs:
    case Nil{}:
      acc
    case SRow{_, M.Project{+id, +ti, _, _, _}, M.Shelves{p, a, se, _, _}} <> t:
      match acc:
        case AParts{l, f}:
          Active.gather.ix(t, st, ix, now, AParts{Active.live.ix(p, st, ix, id, ti, Active.live.ix(a, st, ix, id, ti, l)), Active.settled(se, now, id, ti, f)})

def Active.match(+q: String, r: ARow) -> Bool:
  match r:
    case ARow{_, _, +t, _, +pt, _}:
      Bool.or(Nat.is_gt(FZ.Fuzzy.score(Active.title(t), q), 0n), Nat.is_gt(FZ.Fuzzy.score(pt, q), 0n))

def Active.find(rs: List<&2, ARow>, +q: String) -> List<&2, ARow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Active.keep(Active.match(q, r), r, Active.find(t, q))

def Active.find.desc(ts: List<&2, M.Thread>, +st: M.State, +root: String, +q: String) -> Bool:
  match ts:
    case Nil{}: False{}
    case +t <> rest:
      Bool.or(Bool.and(String.eq(ST.Tree.root(st, M.Thread.id(t)), root), Nat.is_gt(FZ.Fuzzy.score(Active.title(t), q), 0n)), Active.find.desc(rest, st, root, q))

def Active.row_id(r: ARow) -> String:
  match r:
    case ARow{_, _, t, _, _, _}: M.Thread.id(t)

def Active.match.tree(empty: Bool, +st: M.State, +q: String, +r: ARow) -> Bool:
  match empty:
    case True{}: True{}
    case False{}: Bool.or(Active.match(q, r), Active.find.desc(ST.Tree.all(st), st, Active.row_id(r), q))

def Active.find.tree(rs: List<&2, ARow>, +st: M.State, +q: String) -> List<&2, ARow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest:
      Active.keep(Active.match.tree(String.is_empty(q), st, q, r), r, Active.find.tree(rest, st, q))

def Active.all(g: AParts) -> List<&2, ARow>:
  match g:
    case AParts{l, f}:
      List.append(&2, ARow, List.sort(~ARow, ~ARow.before, l), List.sort(~ARow, ~ARow.before, f))

# the active view's rows, top to bottom (laws active_*)
def Active.rows(+st: M.State, +now: Nat, +q: String) -> List<&2, ARow>:
  Active.find.tree(Active.all(Active.gather(Side.rows(st, now), st, now, AParts{Nil{}, Nil{}})), st, q)

# A state for the index's laws: two projects, a running thread with an
# approval open, one waiting on input with a pending tool, a delegated
# child at work under a third, a finished child under the first, a
# closed plan
def Side.demo() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "One", "/a", 1n}, M.ProjectCreated{"p2", "Two", "/b", 2n},
    M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 3n}, M.ThreadCreated{"t2", "p1", "B", M.Local{}, "", "", "claude", "", 4n},
    M.ThreadCreated{"t3", "p2", "C", M.Local{}, "", "", "claude", "", 5n}, M.ThreadCreated{"t4", "p2", "D", M.Local{}, "", "", "claude", "", 6n},
    M.ThreadCreated{"t5", "p1", "E", M.Local{}, "", "", "claude", "", 7n},
    M.TurnChanged{"t1", M.Running{}, 8n}, M.TurnChanged{"t3", M.Completed{}, 9n}, M.PendingChanged{"t2", 1n, 10n},
    M.TaskDelegated{"k1", "t3", "t4", 1n, "claude", "", 11n}, M.TaskDelegated{"k2", "t1", "t5", 1n, "claude", "", 12n}, M.TaskStateSet{"k2", M.TaskDone{}, "ok", 13n},
    M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 14n}, M.AskOpened{"a2", "t2", "input", "", "?", "", 15n}, M.AskOpened{"a3", "t3", "plan", "", "p", "", 16n},
    M.AskClosed{"a3", "yes", 17n}], M.State.empty())

# the same with background rows on t1 (two) and watched work on t4
def Side.demo.work() -> M.State:
  M.State.replay([M.SettingSet{"bg.t1", "k1\tsleeping", 20n}, M.SettingSet{"watch.t1", "w1\twatching", 21n}, M.SettingSet{"watch.t4", "w2\tx", 22n}], Side.demo())

def Side.phases(ts: List<&2, M.Thread>, +st: M.State) -> List<&2, M.Phase>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Side.phase(st, t) <> Side.phases(rest, st)

def Side.phases.ix(ts: List<&2, M.Thread>, +ix: SIx, +st: M.State) -> List<&2, M.Phase>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Side.phase.ix(ix, st, t) <> Side.phases.ix(rest, ix, st)

def Side.busies(ts: List<&2, M.Thread>, +st: M.State) -> List<&2, Nat>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      M.Work.busy(st, M.Thread.id(t)) <> Side.busies(rest, st)

def Side.busies.ix(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +st: M.State) -> List<&2, Nat>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      M.Work.busy.at(kids, st, M.Thread.id(t)) <> Side.busies.ix(rest, kids, st)

# Active.rows from the index (law active_rows_ix_eq)
def Active.rows.ix(+st: M.State, +ix: SIx, +now: Nat, +q: String) -> List<&2, ARow>:
  Active.find.tree(Active.all(Active.gather.ix(Side.rows.ix(st, ix, now), st, ix, now, AParts{Nil{}, Nil{}})), st, q)

# Reuse this frame's fresh project/shelf rows. The caller still supplies
# the phase index independently: display signals never choose key targets.
def Active.rows.given(+st: M.State, +ix: SIx, +now: Nat, +q: String, rows: List<&2, SRow>) -> List<&2, ARow>:
  Active.find.tree(Active.all(Active.gather.ix(rows, st, ix, now, AParts{Nil{}, Nil{}})), st, q)

def Active.ids(rs: List<&2, ARow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case ARow{_, _, t, _, _, _} <> r:
      M.Thread.id(t) <> Active.ids(r)

# the rows split: live ones, then the settled ones, each in order
def Active.parts.put(faded: Bool, r: ARow, p: AParts) -> AParts:
  match faded p:
    case True{} AParts{l, f}:
      AParts{l, r <> f}
    case False{} AParts{l, f}:
      AParts{r <> l, f}

def Active.parts(rs: List<&2, ARow>) -> AParts:
  match rs:
    case Nil{}:
      AParts{Nil{}, Nil{}}
    case +r <> t:
      Active.parts.put(ARow.faded(r), r, Active.parts(t))

# The settled rows fold under one "Settled N" row, shut until opened (flag
# "active-settled", action "side-settled"); a search shows them in place
def Active.open(+op: Map<&2, Bool>, +q: String) -> Bool:
  Bool.or(Flag.get(op, "active-settled"), Bool.not(String.is_empty(q)))

# the rows shown: the settled ones only while their fold is open (law
# active_fold_shut)
def Active.folded(show: Bool, p: AParts) -> List<&2, ARow>:
  match show p:
    case True{} AParts{l, f}:
      List.append(&2, ARow, l, f)
    case False{} AParts{l, _}:
      l

# A shut fold still shows the open thread's row: a thread that settled (or
# was archived) while open stays in sight until the person moves on
# (Settings' "Settle when done")
def Side.sel.k(hit: Bool, t: M.Thread, rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match hit:
    case True{}:
      t <> rest
    case False{}:
      rest

# the rows a shut shelf still shows: the open thread's, and a pinned one
# (so a pin is never folded out of sight)
def Side.sel(ts: List<&2, M.Thread>, +sel: String) -> List<&2, M.Thread>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      Side.sel.k(Bool.or(Active.pinned(t), Bool.and(Bool.not(String.is_empty(sel)), String.eq(M.Thread.id(t), sel))), t, Side.sel(r, sel))

# a shut fold's words: its name and count, and how many of its rows show
# under it anyway (the open thread stays on screen: Side.sel, Active.sel),
# so a shut chevron over a row never reads as a contradiction
def Shelf.words(+lc: I.Loc, +label: String, +shown: Nat) -> String:
  Bool.pick(String, Nat.is_eq(shown, 0n), label, MSG.T.sidebar.shelf.shown(lc, label, Nat.show(shown)))

def Active.sel.k(hit: Bool, a: ARow, rest: List<&2, ARow>) -> List<&2, ARow>:
  match hit:
    case True{}:
      a <> rest
    case False{}:
      rest

def Active.sel(rs: List<&2, ARow>, +sel: String) -> List<&2, ARow>:
  match rs:
    case Nil{}:
      Nil{}
    case ARow{u, a, +t, p, ti, f} <> r:
      Active.sel.k(Bool.and(Bool.not(String.is_empty(sel)), String.eq(M.Thread.id(t), sel)), ARow{u, a, t, p, ti, f}, Active.sel(r, sel))

def Active.faded_n(p: AParts) -> Nat:
  match p:
    case AParts{_, f}:
      List.length(&2, ARow, f)

# Row menus
# ---------
# A right-click on a thread's row or a project's name (a long press on a
# phone) opens its menu: "row-menu id" or "proj-menu id" (the same again
# shuts it), kept as "@menu" ("t:<id>" or "p:<id>") in the scratch.
# Picking an item takes its action and shuts the menu, except Delete and
# Remove, which ask first: their first pick arms them and the menu stays
# open saying so (law menu_delete_asks). Anything else shuts it.

# what an item says, what it sends and its tone ("danger" or "")
type MItem is Data:
  MItem{label: String, action: String, value: String, tone: String}

def Menu.snoozes(+l: I.Loc, +id: String) -> List<&2, MItem>:
  [MItem{MSG.T.menu.snooze.hour(l), "row-snooze", id ++ "|1h", ""}, MItem{MSG.T.menu.snooze.tomorrow(l), "row-snooze", id ++ "|tomorrow", ""},
   MItem{MSG.T.menu.snooze.week(l), "row-snooze", id ++ "|1w", ""}]

def Menu.del(+l: I.Loc, +id: String, armed: Bool) -> MItem:
  MItem{Bool.pick(String, armed, MSG.T.menu.delete.confirm(l), MSG.T.common.delete(l)), "row-delete", id, "danger"}

def Menu.pin(+l: I.Loc, pinned: Bool, +id: String) -> MItem:
  match pinned:
    case True{}:
      MItem{MSG.T.menu.unpin(l), "row-unpin", id, ""}
    case False{}:
      MItem{MSG.T.menu.pin(l), "row-pin", id, ""}

# every thread can be renamed (Ren.*)
def Menu.ren(+l: I.Loc, +id: String) -> MItem:
  MItem{MSG.T.menu.rename(l), "row-rename", id, ""}

def Menu.ref(+l: I.Loc, r: R.Ref) -> MItem:
  MItem{MSG.T.menu.copylink(l), "ref-copy", R.Ref.uri(r), ""}

def Menu.room(+l: I.Loc, id: String) -> List<&2, MItem>:
  [Menu.ref(l, R.Ref.room(id))]

def Menu.msg(+l: I.Loc, thread: String, msg: String) -> List<&2, MItem>:
  [Menu.ref(l, R.Ref.msg(thread, msg))]

def Menu.thread.k.loc(+l: I.Loc, sh: M.Shelf, pinned: Bool, +id: String, armed: Bool) -> List<&2, MItem>:
  match sh:
    case M.Settled{}:
      [MItem{MSG.T.menu.bringback(l), "row-unsettle", id, ""}, Menu.ren(l, id), MItem{MSG.T.menu.archive(l), "row-archive", id, ""}, Menu.del(l, id, armed)]
    case M.Snoozed{}:
      [MItem{MSG.T.menu.wake(l), "row-snooze", id ++ "|off", ""}, MItem{MSG.T.menu.settle(l), "row-settle", id, ""}, Menu.ren(l, id), MItem{MSG.T.menu.archive(l), "row-archive", id, ""}, Menu.del(l, id, armed)]
    case M.Hidden{}:
      [MItem{MSG.T.menu.unarchive(l), "row-unarchive", id, ""}, Menu.ren(l, id), Menu.del(l, id, armed)]
    case _:
      List.concat(&2, MItem, [[Menu.pin(l, pinned, id), Menu.ren(l, id), MItem{MSG.T.menu.settle(l), "row-settle", id, ""}], Menu.snoozes(l, id),
        [MItem{MSG.T.menu.archive(l), "row-archive", id, ""}, Menu.del(l, id, armed)]])

def Menu.thread.k(sh: M.Shelf, pinned: Bool, id: String, armed: Bool) -> List<&2, MItem>:
  Menu.thread.k.loc(I.En{}, sh, pinned, id, armed)

def Menu.look(+l: I.Loc, rich: Bool, +id: String) -> List<&2, MItem>:
  match rich:
    case True{}:
      [MItem{MSG.T.menu.look(l), "look", id, ""}]
    case False{}:
      Nil{}

# a project's menu (rich: the window and the web, which have the badge
# editor; armed: Remove was picked once). Intent… shows what the project is
# for (Intent.open)
def Menu.project(+l: I.Loc, +id: String, armed: Bool, rich: Bool) -> List<&2, MItem>:
  List.concat(&2, MItem, [[MItem{MSG.T.thread.new(l), "new-thread", id, ""}, MItem{MSG.T.menu.intent(l), "proj-intent", id, ""}], Menu.look(l, rich, id),
    [MItem{Bool.pick(String, armed, MSG.T.menu.remove.confirm(l), MSG.T.menu.project.remove(l)), "proj-remove", id, "danger"}, Menu.ref(l, R.Ref.project(id))]])

# a file link's menu ("file-menu path", kept as "f:<path>"; a click on the
# link is its Open). The path goes as written; Flink.* resolves it
def Menu.file.loc(+l: I.Loc, +path: String) -> List<&2, MItem>:
  [MItem{MSG.T.menu.file.open(l), "file-open", path, ""}, MItem{MSG.T.menu.file.reveal(l), "file-reveal", path, ""}, MItem{MSG.T.menu.file.copy(l), "file-copy", path, ""}]

def Menu.file(path: String) -> List<&2, MItem>:
  Menu.file.loc(I.En{}, path)

# the menu open ("" for none), its kind ("t", "p" or "f") and its id
def Menu.open(+u: Ui) -> String:
  Ui.scratch(u, "@menu")

def Menu.kind(+m: String) -> String:
  String.take(m, 1n)

def Menu.id(+m: String) -> String:
  String.drop(m, 2n)

# a bot's menu ("bot-menu id", kept as "b:<id>"): the person makes a bot
# of this machine the uberbot, or not (core/uber.bend)
def Menu.bot(+l: I.Loc, +st: M.State, +id: String) -> List<&2, MItem>:
  List.append(&2, MItem, Bool.pick(List<&2, MItem>, UB.Uber.is(st, id), [MItem{MSG.T.menu.uber.demote(l), "uber-demote", "", ""}],
    Bool.pick(List<&2, MItem>, UB.Uber.live(st, id), [MItem{MSG.T.menu.uber.promote(l), "uber-promote", id, ""}], Nil{})), [Menu.ref(l, R.Ref.bot(id))])

# A file URI carries a project-relative path. Files outside the selected
# project's root keep the ordinary file menu.
def Menu.file.here.k(+l: I.Loc, +pid: String, +path: String, rel: String) -> List<&2, MItem>:
  match rel:
    case SNil{}: Menu.file.loc(l, path)
    case SCon{h, t}: List.append(&2, MItem, Menu.file.loc(l, path), [Menu.ref(l, R.Ref.file(pid, R.Ref.split(LK.Link.file.path(SCon{h, t}), '/')))])

def Menu.file.here.pid(+l: I.Loc, +pid: String, +u: Ui, +path: String) -> List<&2, MItem>:
  match pid:
    case SNil{}: Menu.file.loc(l, path)
    case SCon{+h, +t}:
      Menu.file.here.k(l, SCon{h, t}, path, Bool.pick(String, String.starts_with(path, "/"), FM.Fm.rel(Ui.project_root(Side.st(u), SCon{h, t}), path), path))

def Menu.file.here.loc(+l: I.Loc, +u: Ui, +path: String) -> List<&2, MItem>:
  Menu.file.here.pid(l, Ui.thread_project(Ui.thread(Side.st(u), Ui.sel(u))), u, path)

def Menu.file.here(+u: Ui, path: String) -> List<&2, MItem>:
  Menu.file.here.loc(Lang.loc(u), u, path)

# its heading: the thread's title, the project's name or the file's path
def Menu.title.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, String.eq(Menu.kind(Menu.open(u)), "t"), TTL.Title.shown(l, Active.title(Ui.thread(Side.st(u), Menu.id(Menu.open(u))))),
    Bool.pick(String, String.eq(Menu.kind(Menu.open(u)), "f"), Menu.id(Menu.open(u)),
      Bool.pick(String, String.eq(Menu.kind(Menu.open(u)), "b"), BT.Bot.name(BT.World.bot(M.State.bots(Side.st(u)), Menu.id(Menu.open(u)))), Ui.project_title(Side.st(u), Menu.id(Menu.open(u))))))

def Menu.title(+u: Ui) -> String:
  Menu.title.loc(Lang.loc(u), u)

# opening one disarms any delete or remove; the same again shuts it
def Menu.set(+u: Ui, +m: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@del", ""), "@rm", ""), "@menu", Bool.pick(String, String.eq(Menu.open(u), m), "", m))

def Menu.actions() -> List<&2, String>:
  ["row-menu", "proj-menu", "menu-close", "row-pin", "row-unpin", "row-archive", "row-unarchive", "side-older", "side-settled",
   "file-menu", "file-open", "file-reveal", "file-copy", "row-rename", "rename-save", "rename-no", "bot-menu"]

# a file link's path where the hub finds it: in the selected thread's folder
# when relative (core/links.bend's Link.file.abs)
def Flink.abs(+u: Ui, +raw: String) -> String:
  LK.Link.file.abs(Ui.root(u), raw)

# the hub opens it, or shows it in its folder (how: "open" or "reveal")
def Flink.open(+u: Ui, +raw: String, +how: String) -> Act:
  Ui.rpc(u, "file.open", [J.KV{"path", J.Str{Flink.abs(u, raw)}}, J.KV{"how", J.Str{how}}])

def Menu.shut(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Act{Ui.scratch_set(u, "@menu", ""), cs}

# Renaming a thread, any thread (a fork, a scratch one, one far away):
# "row-rename id" opens its title for editing (@ren names the thread, the
# field @f.title starts from its title; the window and the web edit it in
# the thread's header, the phones in a dialog), "rename-save" keeps the
# field (the phones send "id\u001ftitle" as its value), "rename-no" leaves
# it as it was. A blank title is never sent (laws ren_*).
def Ren.clear(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(u, "@ren", ""), "@f.title", "")

def Ren.open(+u: Ui, +id: String) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@menu", ""), "@ren", id), "@f.title", Active.title(Ui.thread(Side.st(u), id))), [Focus{"thread-title"}]}

# the thread and the title a save names ("" for none)
def Ren.id(+u: Ui, +v: String) -> String:
  Bool.pick(String, String.contains(v, "\u{1f}"), Maybe.default(&2, String, List.head(&2, String, String.split(v, '\u{1f}')), ""), Ui.scratch(u, "@ren"))

def Ren.title(+u: Ui, +v: String) -> String:
  String.trim(Bool.pick(String, String.contains(v, "\u{1f}"), String.join(List.drop(&2, String, String.split(v, '\u{1f}'), 1n), " "), Ui.scratch(u, "@f.title")))

def Ren.save.k(blank: Bool, +u: Ui, +id: String, +title: String) -> Act:
  match blank:
    case True{}:
      Act{Ren.clear(u), Nil{}}
    case False{}:
      Ui.rpc(Ren.clear(u), "thread.rename", [J.KV{"thread", J.Str{id}}, J.KV{"title", J.Str{title}}])

def Ren.save(+u: Ui, +v: String) -> Act:
  Ren.save.k(Bool.or(String.is_empty(Ren.id(u, v)), String.is_empty(Ren.title(u, v))), u, Ren.id(u, v), Ren.title(u, v))

# the thread whose title is being edited ("" for none)
def Ren.on(+u: Ui) -> String:
  Ui.scratch(u, "@ren")

def Menu.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Act{Menu.set(u, "t:" ++ value), Nil{}}
    case 1n:
      Act{Menu.set(u, "p:" ++ value), Nil{}}
    case 2n:
      Act{Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@menu", ""), "@del", ""), "@rm", ""), Nil{}}
    case 3n:
      Menu.shut(Ui.rpc(u, "thread.pin", [J.KV{"thread", J.Str{value}}]))
    case 4n:
      Menu.shut(Ui.rpc(u, "thread.unpin", [J.KV{"thread", J.Str{value}}]))
    case 5n:
      Menu.shut(Ui.rpc(u, "thread.archive", [J.KV{"thread", J.Str{value}}]))
    # the folded shelf of older projects opened or shut
    case 7n:
      Ui.toggle_settled(u, "old-projects")
    # the active view's settled rows folded or shown
    case 8n:
      Ui.toggle_settled(u, "active-settled")
    case 9n:
      Act{Menu.set(u, "f:" ++ value), Nil{}}
    case 10n:
      Menu.shut(Flink.open(u, value, "open"))
    case 11n:
      Menu.shut(Flink.open(u, value, "reveal"))
    case 12n:
      Menu.shut(Act{u, [Copy{Flink.abs(u, value)}]})
    case 13n:
      Ren.open(u, value)
    case 14n:
      Ren.save(u, value)
    case 15n:
      Act{Ren.clear(u), Nil{}}
    case 16n:
      Act{Menu.set(u, "b:" ++ value), Nil{}}
    case _:
      Menu.shut(Ui.rpc(u, "thread.unarchive", [J.KV{"thread", J.Str{value}}]))

# after any other action: the menu stays open only while shut already, or
# while a Delete or Remove it armed waits for its second pick
def Menu.keep(+action: String, +u: Ui) -> Bool:
  Bool.or(String.is_empty(Menu.open(u)),
    Bool.or(Bool.and(String.eq(action, "row-delete"), Bool.not(String.is_empty(Ui.scratch(u, "@del")))),
      Bool.and(String.eq(action, "proj-remove"), Bool.not(String.is_empty(Ui.scratch(u, "@rm"))))))

def Menu.after.k(keep: Bool, a: Act) -> Act:
  match keep:
    case True{}:
      a
    case False{}:
      Menu.shut(a)

def Menu.after(+action: String, a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Menu.after.k(Menu.keep(action, u), Act{u, cs})

# A project's badge editor ("@look" is the project whose editor is open):
# "look" opens it for a project, or closes it when it is open for that
# one (or for ""); "look-set" sets project.color.<id> or project.icon.<id>
# ("key=value") and leaves it open.
def Ui.look(+u: Ui, +id: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(u, "@rm", ""), "@look", Bool.pick(String, String.eq(Ui.scratch(u, "@look"), id), "", id))

# "proj-remove" from the badge editor takes two clicks: the first arms it
# ("@rm" is the project), the second removes the project. A removed
# project leaves the sidebar with its threads kept; adding its folder
# again brings it back as it was (Hub.project_add)
def Ui.remove_project.k(armed: Bool, +u: Ui, +id: String) -> Act:
  match armed:
    case True{}:
      Ui.rpc(Ui.scratch_set(Ui.scratch_set(u, "@rm", ""), "@look", ""), "project.remove", [J.KV{"project", J.Str{id}}])
    case False{}:
      Act{Ui.scratch_set(u, "@rm", id), Nil{}}

def Ui.remove_project(+u: Ui, +id: String) -> Act:
  Ui.remove_project.k(Bool.and(Bool.not(String.is_empty(id)), String.eq(Ui.scratch(u, "@rm"), id)), u, id)

# "proj-find" opens the sidebar's search, focused and empty, or shuts it
# ("on" and "off" say which; "" flips it). "proj-find-q" sets its query (the
# phones' field), "proj-keep" disarms a remove
def Find.toggle.k(open: Bool, +u: Ui) -> Act:
  match open:
    case True{}:
      Act{Ui.scratch_set(Ui.flag_off(u, "psearch"), "@f.pq", ""), Nil{}}
    case False{}:
      Act{Ui.scratch_set(Ui.flag_on(u, "psearch"), "@f.pq", ""), [Focus{"proj-find"}]}

def Find.toggle(+u: Ui, +v: String) -> Act:
  Find.toggle.k(Bool.pick(Bool, String.is_empty(v), Ui.flag(u, "psearch"), String.eq(v, "off")), u)

# in the active view Enter opens the first thread the search kept
def Find.go.active(+u: Ui) -> Act:
  Bool.pick(Act, List.is_empty(&2, String, Active.ids(Active.rows(Side.st(u), Side.now(u), Ui.scratch(u, "@f.pq")))), Act{u, Nil{}},
    Ui.select(Ui.scratch_set(Ui.flag_off(u, "psearch"), "@f.pq", ""), Side.first(Active.ids(Active.rows(Side.st(u), Side.now(u), Ui.scratch(u, "@f.pq"))))))

def Ui.info_drop(u: Ui, +k: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, Map.del(&2, String, nf, k), ob, tr}

def Ui.info_set(u: Ui, +k: String, +v: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, Map.set(&2, String, nf, k, v), ob, tr}

# a view key of the selected thread set (the phones' viewer lends the
# client its kind)
def Ui.view_get.set(+u: Ui, +k: String, +v: String) -> Ui:
  Ui.info_set(u, k ++ ":" ++ Ui.sel(u), v)

# a run of tool calls opened or shut: the value is "1" or "0", then the
# run's id (FO.Fold.ask); the choice is kept in the info map (FO.Fold.user)
# (a to-do list opened stores "1:<the hub's clear token>", Todos.open)
def Ui.tool_fold.v(+u: Ui, +id: String, +c: String) -> String:
  Bool.pick(String, Bool.and(String.eq(c, "1"), String.starts_with(id, "todos:")), "1:" ++ M.Todos.gen(Ui.state(u), String.drop(id, 6n)), c)

def Ui.tool_fold(+u: Ui, +v: String) -> Ui:
  Ui.info_set(u, FO.Fold.key(String.drop(v, 1n)), Ui.tool_fold.v(u, String.drop(v, 1n), String.take(v, 1n)))

def Skills.got(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.info_set(Ui.info_set(u, "skills", J.Json.show(J.Json.get_obj(J.Json.get_obj(o, "result"), "skills"))), "skills.for", J.Json.get_str(J.Json.get_obj(o, "result"), "thread", ""))
    case False{}:
      u

def Skills.fetch(need: Bool, a: Act) -> Act:
  match need a:
    case True{} Act{+u, cs}:
      Ui.also(Ui.on_thread(Ui.info_set(Ui.info_set(u, "skills.for", Ui.sel(u)), "skills", "[]"), "skills.list"), cs)
    case False{} _:
      a

# Side questions
# --------------
# The question and its answer live in the scratch, per thread ("@btw:t",
# "@btw.a:t", "" while it is being answered); the card above the composer
# shows them until closed ("btw-close")

def Btw.ask.go(u: Ui, +q: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, "@btw:" ++ sel, q), "@btw.a:" ++ sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr},
        "thread.btw", [J.KV{"thread", J.Str{sel}}, J.KV{"q", J.Str{q}}])

def Btw.ask.d(a: Act, +q: String) -> Act:
  match a:
    case Act{u, cs}:
      Ui.first(cs, Btw.ask.go(u, q))

def Btw.ask.k(u: Ui, +q: String) -> Act:
  Btw.ask.d(Draft.clear(u), q)

# "/btw" with no question asks nothing
def Btw.ask(+u: Ui, +q: String) -> Act:
  Bool.pick(Act, String.is_empty(q), Act{u, Nil{}}, Btw.ask.k(u, q))

def Btw.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "btw"), J.Null{})), "null"))

def Btw.answer(+a: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(a)), "No answer came back (Settings, Writing: the model it asks).", String.trim(a))

def Btw.got(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.scratch_set(u, "@btw.a:" ++ J.Json.get_str(J.Json.get_obj(o, "result"), "thread", ""), Btw.answer(J.Json.get_str(J.Json.get_obj(o, "result"), "btw", "")))
    case False{}:
      u

def Btw.reply(+u: Ui, +o: J.Json) -> Ui:
  +result = J.Json.get_obj(o, "result")
  +thread = J.Json.get_str(result, "thread", "")
  Btw.got(Bool.and(Btw.has(o), Bool.or(Bool.not(ACCT.Accounts.named(Ui.state(u), thread)), String.eq(ACCT.Accounts.owner(Ui.state(u), thread), J.Json.get_str(result, "accountBinding", "")))), u, o)

def Btw.close(+u: Ui) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(u, "@btw:" ++ Ui.sel(u), ""), "@btw.a:" ++ Ui.sel(u), ""), Nil{}}

# the open thread's side question ("" for none) and its answer ("" while
# it is being answered)
def Btw.question(+u: Ui) -> String:
  Ui.scratch(u, "@btw:" ++ Ui.sel(u))

def Btw.reply.of(+u: Ui) -> String:
  Ui.scratch(u, "@btw.a:" ++ Ui.sel(u))

# (the answer is kept as it came; no answer is kept as Btw.answer's English
# words, said in the language where it shows)
def Btw.text(+l: I.Loc, +a: String) -> String:
  Bool.pick(String, String.is_empty(a), MSG.T.btw.thinking(l), Bool.pick(String, String.eq(a, Btw.answer("")), MSG.T.btw.none(l), a))

# the composer keeps the keyboard (the web draws the page again)
def Skills.close(+u: Ui) -> Act:
  Act{Ui.info_set(u, "sk.off", Skills.on(Ui.sel(u), Ui.draft_text(u))), [Focus{"composer"}]}

def Skills.want(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Skills.fetch(Skills.need(u), Act{u, cs})

def Refs.fill(on: Bool, u: Ui) -> Ui:
  match on u:
    case True{} Ui{+st, +sel, +dr, live, op, on2, +now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on2, now, ad, pa, v, rid, w, er, note,
        Map.set(&2, String, Map.set(&2, String, nf, "refs", String.join(Refs.show(RF.Refs.offer.with(st, sel, now, Skills.token(Str.get(dr, sel)),
          RF.Refs.fars.if(String.starts_with(Skills.token(Str.get(dr, sel)), "@"), Str.get(nf, "bots.remote"), BT.World.peers(M.State.bots(st))))), "\n")), "refs.for", Refs.for(sel, Str.get(dr, sel))), ob, tr}
    case False{} _:
      u

# Derive reference rows after a thin view adopts its current local draft.
# This only reads the authoritative catalog; it sends no command or Keep.
def Refs.refresh(+u: Ui) -> Ui:
  Refs.fill(RF.Refs.typing(Skills.token(Ui.draft_text(u))), u)

def Refs.want(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Act{Refs.refresh(u), cs}

# an edit of the draft opens a menu Esc shut again (the same text sent
# again leaves it shut)
def Skills.reopen.k(same: Bool, +u: Ui) -> Ui:
  match same:
    case True{}:
      u
    case False{}:
      Ui.info_set(u, "sk.off", "")

def Skills.reopen(+u: Ui, +text: String) -> Ui:
  Skills.reopen.k(String.eq(Ui.draft_text(u), text), u)

# the selected thread's draft; typing a `$` fetches its skills once, a
# reference being typed is looked up
def Ui.draft(u: Ui, +text: String) -> Act:
  Refs.want(Skills.want(Ui.draft.set(Skills.reopen(u, text), text)))

# What the machine says about a provider: its CLI version ("" when it is
# not installed) and a label per model
def Models.about(+info: String, +provider: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), "about"), provider)

# the effort chips for a thread's model: the levels its CLI says that model
# takes (Claude reports them per model), else the provider's
def Effort.chips(+info: String, +provider: String, +model: String) -> List<&2, String>:
  Effort.chips.of(Effort.known(J.Json.get(J.Json.get_obj(Models.about(info, provider), "efforts"), Bool.pick(String, String.is_empty(model), "default", model)), provider))

# The picker's chips: the providers first ("@claude", "@codex", "@grok", "@muse";
# a thread goes on with another between turns, V2's provider switch),
# then the efforts its model takes. One chip row and one action for all
# clients: a chip's value starting with "@" is a provider.

def Chip.providers() -> List<&2, String>:
  ["@claude", "@codex", "@grok", "@muse", "@antigravity", "@opencode", "@pi", "@omp"]

def Chip.all(+info: String, +provider: String, +model: String) -> List<&2, String>:
  List.append(&2, String, Chip.providers(), Effort.chips(info, provider, model))

def Chip.name(+c: String) -> String:
  Bool.pick(String, String.starts_with(c, "@"), String.drop(c, 1n), Effort.name(c))

# whether a chip is the thread's provider or effort now
def Chip.on(+c: String, +effort: String, +provider: String) -> Bool:
  Bool.or(String.eq(c, effort), String.eq(c, "@" ++ provider))

def Models.cli(+info: String, +provider: String) -> String:
  J.Json.get_str(Models.about(info, provider), "cli", "")

# "opus" -> "Opus 5.5 · claude-opus-5-5" ("" when unknown)
def Models.labels(+info: String, +provider: String) -> J.Json:
  J.Json.get_obj(Models.about(info, provider), "labels")

def Models.label.given(+labels: J.Json, +model: String) -> String:
  J.Json.get_str(labels, Bool.pick(String, String.is_empty(model), "default", model), "")

def Models.label_of(+info: String, +provider: String, +model: String) -> String:
  Models.label.given(Models.labels(info, provider), model)

# The model picker's search (field "mq", scratch "@f.mq"): a model shows
# when every word of the query is in its id or its label, case aside. The
# field shows once a provider offers more than Models.searchable() models
# (opencode, signed in to a few providers, lists hundreds).
def Models.searchable() -> Nat:
  12n

def Models.search(+ms: List<&2, String>) -> Bool:
  Nat.is_gt(List.length(&2, String, ms), Models.searchable())

def Models.words.all(ws: List<&2, String>, +hay: String) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case +w <> t:
      Bool.and(Bool.or(String.is_empty(w), String.contains(hay, w)), Models.words.all(t, hay))

def Models.find.go(ms: List<&2, String>, +ws: List<&2, String>, +labels: J.Json) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Models.menu.k(Bool.not(Models.words.all(ws, String.to_lower(m ++ " " ++ J.Json.get_str(labels, Bool.pick(String, String.is_empty(m), "default", m), "")))), m, Models.find.go(t, ws, labels))

def Models.find.k(all: Bool, +ms: List<&2, String>, +q: String, +info: String, +prov: String) -> List<&2, String>:
  match all:
    case True{}:
      ms
    case False{}:
      Models.find.go(ms, String.split(String.to_lower(String.trim(q)), ' '), J.Json.get_obj(Models.about(info, prov), "labels"))

def Models.find(+ms: List<&2, String>, +q: String, +info: String, +prov: String) -> List<&2, String>:
  Models.find.k(String.is_empty(String.trim(q)), ms, q, info, prov)

# what the picker lists: the provider's models, those the search finds,
# at most Models.cap() (the search narrows the rest), so a frame draws and
# labels no more than that
def Models.cap() -> Nat:
  50n

def Models.shown(+info: String, +prov: String, +q: String) -> List<&2, String>:
  List.take(&2, String, Models.find(Models.menu(info, prov), q, info, prov), Models.cap())

# Settings' model chips: the default, the chosen model and what the search
# finds, a dozen at most
def Models.with(+cur: String, +ms: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.or(String.is_empty(cur), List.contains(~String, ~String.eq, ms, cur)), ms, List.append(&2, String, ms, [cur]))

def Models.settings.k(many: Bool, +info: String, +prov: String, +q: String, +cur: String) -> List<&2, String>:
  match many:
    case True{}:
      Models.with(cur, List.take(&2, String, Models.find(Models.menu(info, prov), q, info, prov), 12n))
    case False{}:
      Models.menu(info, prov)

def Models.settings(+info: String, +prov: String, +q: String, +cur: String) -> List<&2, String>:
  Models.settings.k(Models.search(Models.menu(info, prov)), info, prov, q, cur)

def Models.view.strs(ms: List<&2, String>) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      J.Str{m} <> Models.view.strs(t)

def Models.view.go(ms: List<&2, String>, +labels: J.Json) -> List<&2, J.KV>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      J.KV{Bool.pick(String, String.is_empty(m), "default", m), J.Str{J.Json.get_str(labels, Bool.pick(String, String.is_empty(m), "default", m), "")}} <> Models.view.go(t, labels)

# the models info cut to the models shown, for the picker's rows, which
# look up each one's label in it: a few KB to read, not all of them
def Models.view(+info: String, +prov: String, +ms: List<&2, String>) -> String:
  J.Json.show(J.Json.obj([J.KV{prov, J.Json.arr(Models.view.strs(ms))},
    J.KV{"about", J.Json.obj([J.KV{prov, J.Json.obj([J.KV{"labels", J.Json.obj(Models.view.go(ms, J.Json.get_obj(Models.about(info, prov), "labels")))}])}])}]))

# what a phone's picker gets: opencode's models cut to those its search
# finds (the phones draw every row they are sent); others' info as is
def Models.phone.k(oc: Bool, +nf: Map<&2, String>, +prov: String, +q: String) -> String:
  match oc:
    case True{}:
      Models.view(Models.of(nf, prov), prov, Models.shown(Models.of(nf, prov), prov, q))
    case False{}:
      Str.get(nf, "models")

def Models.phone(+nf: Map<&2, String>, +prov: String, +q: String) -> String:
  Models.phone.k(String.eq(prov, "opencode"), nf, prov, q)

# The picker opening empties its search; on opencode it also has the hub
# read opencode's models again (models.refresh: about a second, so a
# provider signed in to since shows its models at once)
def Models.opened.k(open: Bool, oc: Bool, +u: Ui, cs: List<&2, Cmd>) -> Act:
  match open oc:
    case True{} True{}:
      Ui.also(Ui.rpc(Ui.scratch_set(Ui.scratch_set(Ui.flag_off(u, "accounts"), "@f.mq", ""), "@mp", ""), "models.refresh", Nil{}), List.append(&2, Cmd, cs, [Focus{"model-q"}]))
    case True{} False{}:
      Act{Ui.scratch_set(Ui.scratch_set(Ui.flag_off(u, "accounts"), "@f.mq", ""), "@mp", ""), cs}
    case False{} _:
      Act{u, cs}

def Models.shut.drop(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Act{Ui.scratch_set(u, "@mp", ""), cs}

# (shut, the keys go back to the composer, not a search no longer shown,
# and the picker's cursor goes: Mpk.*)
def Models.shut.k(shut: Bool, a: Act) -> Act:
  match shut:
    case True{}:
      Ui.also(Models.shut.drop(a), [Focus{"composer"}])
    case False{}:
      a

def Models.opened(+value: String, a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Models.shut.k(Bool.and(String.eq(value, "models"), Bool.not(Ui.flag(u, "models"))),
        Models.opened.k(Bool.and(String.eq(value, "models"), Ui.flag(u, "models")), String.eq(M.Thread.provider(Ui.thread(Ui.state(u), Ui.sel(u))), "opencode"), u, cs))

# The model picker's cursor (web and desktop; core/modelpick.bend)
# ----------------------------------------------------------------
# Kept in the scratch "@mp" for the thread it was moved on, once a key or
# a click moved it while the picker is open; none is the thread's own
# agent, model and effort. Shutting the picker drops it (Models.shut.k,
# Mpk.shut). Actions: mp-key (tab, stab, up, down, left, right, enter,
# esc), and the clicks mp-agent, mp-model, mp-effort, which apply at once
# and leave the picker open. The phones and the old model/effort actions
# never see it.

def Mpk.now.t(t: M.Thread, +e: String) -> MPK.MpCur:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, pr, mo, _, _, _, _}:
      MPK.MpCur{1n, pr, mo, e}

# the selected thread's own agent, model and effort
def Mpk.now(+st: M.State, +sel: String) -> MPK.MpCur:
  Mpk.now.t(Ui.thread(st, sel), Effort.of(st, sel))

# the cursor, from the scratch the picker keeps it in (drafts, "@mp")
def Mpk.cur.of(+st: M.State, +sel: String, +dr: Map<&2, String>) -> MPK.MpCur:
  MPK.ModelPick.read(Str.get(dr, "@mp"), sel, Mpk.now(st, sel))

def Mpk.cur(+u: Ui) -> MPK.MpCur:
  MPK.ModelPick.read(Ui.scratch(u, "@mp"), Ui.sel(u), Mpk.now(Ui.state(u), Ui.sel(u)))

def Mpk.agents.go(cs: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      String.drop(c, 1n) <> Mpk.agents.go(t)

# the agent column's rows
def Mpk.agents() -> List<&2, String>:
  Mpk.agents.go(Chip.providers())

def Mpk.models.k(own: Bool, ms: List<&2, String>, +m: String) -> List<&2, String>:
  match own:
    case True{}:
      Models.with(m, ms)
    case False{}:
      ms

# the model column's rows for cursor c: its agent's models (what the
# search finds, Models.shown), with the thread's own model when c is on
# the thread's agent and its CLI does not list that model
def Mpk.models(+nf: Map<&2, String>, +c: MPK.MpCur, +now: MPK.MpCur, +q: String) -> List<&2, String>:
  Mpk.models.k(Bool.and(String.eq(MPK.MpCur.agent(c), MPK.MpCur.agent(now)), String.is_empty(String.trim(q))),
    Models.shown(Models.of(nf, MPK.MpCur.agent(c)), MPK.MpCur.agent(c), q), MPK.MpCur.model(now))

# the effort meter for cursor c's agent and model
def Mpk.steps(+nf: Map<&2, String>, +c: MPK.MpCur) -> List<&2, MPK.MpStep>:
  MPK.ModelPick.steps(Effort.chips(Str.get(nf, "models"), MPK.MpCur.agent(c), MPK.MpCur.model(c)))

def Mpk.nf(u: Ui) -> Map<&2, String>:
  match u:
    case Ui{+st, +sel, _, _, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      ACCT.Accounts.nf(nf, st, sel)

def Mpk.keep(+u: Ui, +c: MPK.MpCur) -> Ui:
  Ui.scratch_set(u, "@mp", MPK.ModelPick.show(Ui.sel(u), c))

def Mpk.send.cat(cs: List<&2, Cmd>, a: Act) -> Act:
  match a:
    case Act{u, ds}:
      Act{u, List.append(&2, Cmd, cs, ds)}

def Mpk.send.one(a: Act, m: String, args: List<&2, J.KV>) -> Act:
  match a:
    case Act{u, cs}:
      Mpk.send.cat(cs, Ui.on_thread.p(u, m, args))

def Mpk.send(rs: List<&2, MPK.MpReq>, a: Act) -> Act:
  match rs:
    case Nil{}:
      a
    case MPK.MpReq{m, args} <> t:
      Mpk.send(t, Mpk.send.one(a, m, args))

# apply cursor c (what changed against the thread's own: ModelPick.plan)
# and keep it as the picker's
def Mpk.apply(+u: Ui, +c: MPK.MpCur) -> Act:
  Mpk.send(MPK.ModelPick.plan(c, Mpk.now(Ui.state(u), Ui.sel(u))), Act{Mpk.keep(u, c), Nil{}})

# the picker shut, its cursor and search dropped, the keys back to the composer
def Mpk.shut(u: Ui) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(Ui.flag_off(u, "models"), "@mp", ""), "@f.mq", ""), [Focus{"composer"}]}

def Mpk.enter(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Mpk.send.cat(cs, Mpk.shut(u))

def Mpk.move(+u: Ui, +k: String, +c: MPK.MpCur, +now: MPK.MpCur) -> Act:
  Act{Mpk.keep(u, MPK.ModelPick.key(k, c, Mpk.agents(), Mpk.models(Mpk.nf(u), c, now, Ui.scratch(u, "@f.mq")), Mpk.steps(Mpk.nf(u), c), now)), Nil{}}

def Mpk.key.k(+u: Ui, +k: String) -> Act:
  match k:
    case "enter":
      Mpk.enter(Mpk.apply(u, Mpk.cur(u)))
    case "esc":
      Mpk.shut(u)
    case _:
      Mpk.move(u, k, Mpk.cur(u), Mpk.now(Ui.state(u), Ui.sel(u)))

# a key in the picker (only while it is open)
def Mpk.key.on(open: Bool, +u: Ui, +k: String) -> Act:
  match open:
    case True{}:
      Mpk.key.k(u, k)
    case False{}:
      Act{u, Nil{}}

def Mpk.key(+u: Ui, +k: String) -> Act:
  Mpk.key.on(Ui.flag(u, "models"), u, k)

# a click on a row of column col (0 agent, 1 model, 2 effort): the cursor
# moves there and is applied; a model's click hands the keys back to the
# composer, as the old picker did
def Mpk.click.k(col: Nat, +u: Ui, +v: String, c: MPK.MpCur, +now: MPK.MpCur) -> Act:
  match col:
    case 0n:
      Mpk.apply(u, MPK.ModelPick.col(MPK.ModelPick.agent(c, v, now), 0n))
    case 1n:
      Ui.also(Mpk.apply(u, MPK.ModelPick.col(MPK.ModelPick.model(c, v), 1n)), [Focus{"composer"}])
    case _:
      Mpk.apply(u, MPK.ModelPick.col(MPK.ModelPick.effort(c, v), 2n))

def Mpk.click(+u: Ui, +col: Nat, +v: String) -> Act:
  Mpk.click.k(col, u, v, Mpk.cur(u), Mpk.now(Ui.state(u), Ui.sel(u)))

# Project picker
# --------------
# It opens on the projects: this hub's own (Enter goes to one), and those
# it once had or found on disk (fs.find: a folder holding .backplane or
# .backplane.json), each added back in one step. "Add a folder" (or a
# typed path) turns it into a folder browser with fuzzy filtering
# ("@proj.mode" is "dir"): the listed folder and its subfolders come from
# the server (fs.list). The query and selection live in the drafts map
# under keys no thread id can take.

# The machine the picker works on: "" the hub this client is connected to,
# else the link of another of the owner's machines (info "proj.on"). Its
# requests carry it as `on`, and the hub sends them there (Fr.params.link).
def Proj.on(u: Ui) -> String:
  Ui.info(u, "proj.on")

def Proj.on.kv(+on: String, +p: List<&2, J.KV>) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, String.is_empty(on), p, J.KV{"on", J.Str{on}} <> p)

# whether a project (its id here) is on the machine `on` ("" this hub's own)
def Proj.of(+on: String, +id: String) -> Bool:
  Bool.pick(Bool, String.is_empty(on), Bool.not(FR.Fr.is(id)), String.eq(FR.Fr.link(id), on))

def Proj.ps.k(keep: Bool, p: M.Project, rest: List<&2, M.Project>) -> List<&2, M.Project>:
  match keep:
    case True{}:
      p <> rest
    case False{}:
      rest

def Proj.ps.go(ps: List<&2, M.Project>, +on: String) -> List<&2, M.Project>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, c, rm} <> t:
      Proj.ps.k(Bool.and(Proj.of(on, id), Bool.not(M.Scratch.is(FR.Fr.local(id)))), M.Project{id, ti, r, c, rm}, Proj.ps.go(t, on))

# the projects of the machine the picker works on
def Proj.ps(+u: Ui) -> List<&2, M.Project>:
  Proj.ps.go(Ui.projects(Ui.state(u)), Proj.on(u))

# The owner's other machines the picker can work on: the links whose
# mirror has answered ("far.name.<link>", Ui.far.k) and are not away
def Proj.machines.k(keep: Bool, +id: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      id <> rest
    case False{}:
      rest

def Proj.machines.go(ps: List<&2, BT.Peer>, +u: Ui) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case BT.Peer{+id, _, _, _, +rv} <> t:
      Proj.machines.k(Bool.and(Bool.and(Bool.not(rv), Bool.not(Mate.off(Ui.state(u), Ui.info(u, "far.name." ++ id)))), Bool.and(Bool.not(String.is_empty(Ui.info(u, "far.name." ++ id))), Bool.not(String.eq(Ui.info(u, "far.away." ++ id), "1")))), id, Proj.machines.go(t, u))

def Proj.machines(+u: Ui) -> List<&2, String>:
  Proj.machines.go(BT.World.peers(M.State.bots(Ui.state(u))), u)

def Proj.cycle.next(ls: List<&2, String>, +first: String) -> String:
  match ls:
    case Nil{}:
      first
    case x <> _:
      x

# the machine after `on` in ["", links...], and "" after the last
def Proj.cycle(ls: List<&2, String>, +on: String, +first: String) -> String:
  match ls:
    case Nil{}:
      first
    case +x <> +t:
      Bool.pick(String, String.eq(x, on), Proj.cycle.next(t, first), Proj.cycle(t, on, first))

# the machine the next tap of the picker's machine row picks
def Proj.next(+u: Ui) -> String:
  Proj.cycle("" <> Proj.machines(u), Proj.on(u), "")

def Proj.on.name(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Proj.on(u)), "this machine", Ui.info(u, "far.name." ++ Proj.on(u)))

# the picker's row naming the machine it works on: shown only when the
# owner has other machines up; a tap goes to the next
def Proj.machine.row.loc(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  Bool.pick(List<&2, PItem>, List.is_empty(&2, String, Proj.machines(u)), Nil{}, [PItem{MSG.T.proj.on(l, Proj.on.name(u)), "proj-on", Proj.next(u)}])

def Proj.machine.row(+u: Ui) -> List<&2, PItem>:
  Proj.machine.row.loc(Lang.loc(u), u)

# the picker's field: a form field ("bfield" "proj", @f.proj), so the
# browser keeps what is typed and merges keystrokes instead of waiting on
# the hub for each one (law-free: typing sets it, Bots.bfield)
def Proj.query(u: Ui) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      String.trim(Str.get(dr, "@f.proj"))

def Proj.index(u: Ui) -> Nat:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Maybe.default(&2, Nat, Nat.read(Str.get(dr, "@proj.i")), 0n)

def Proj.set(u: Ui, +q: String, +i: Nat) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, "@f.proj", q), "@proj.i", Nat.show(i)), live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

def Proj.reset(u: Ui, +q: String) -> Ui:
  Ui.info_set(Proj.set(u, q, 0n), "proj.err", "")

def Proj.dir(u: Ui) -> String:
  Ui.info(u, "proj.dir")

def Ui.rid(u: Ui) -> Nat:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, rid, _, _, _, _, _, _}:
      rid

def Ui.error_put(u: Ui, e: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, _, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, e, note, nf, ob, tr}

# a request of the picker's: only the answer to the newest one counts (a
# listing that arrives after the user has moved on is dropped)
def Proj.rpc(+u: Ui, m: String, p: List<&2, J.KV>) -> Act:
  Ui.rpc(Ui.scratch_set(u, "@proj.rid", Nat.show(Ui.rid(u))), m, Proj.on.kv(Proj.on(u), p))

# whether a new thread is waited for (Ui.see selects it when it comes)
def Ui.want_new(u: Ui, b: Bool) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, _, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, b, er, note, nf, ob, tr}

# a folder added or made as a project: the hub starts a thread there with
# it (Hub.project_add), and this client goes to it
def Proj.adding(+u: Ui, m: String, +path: String) -> Act:
  Proj.rpc(Made.want(Ui.info_set(u, "proj.err", "")), m, [J.KV{"path", J.Str{path}}])

def Proj.mine(+u: Ui, +o: J.Json) -> Bool:
  Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@proj.rid"))), String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), Ui.scratch(u, "@proj.rid")))

def Proj.focus(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Act{u, List.append(&2, Cmd, cs, [Focus{"proj"}])}

# browsing folders rather than choosing among projects
def Proj.browsing(+u: Ui) -> Bool:
  String.eq(Ui.scratch(u, "@proj.mode"), "dir")

# the list shown ("" projects, "dir" folders), with an empty filter
def Proj.mode(+u: Ui, +m: String) -> Ui:
  Ui.scratch_set(Proj.reset(u, ""), "@proj.mode", m)

def Proj.cmds.front(cs: List<&2, Cmd>, a: Act) -> Act:
  match a:
    case Act{u, ds}:
      Act{u, List.append(&2, Cmd, cs, ds)}

# the projects on disk are asked for as the picker opens; the answer is
# kept apart from the browsing requests (@proj.frid)
def Proj.find(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Proj.cmds.front(cs, Ui.rpc(Ui.scratch_set(u, "@proj.frid", Nat.show(Ui.rid(u))), "fs.find", Proj.on.kv(Proj.on(u), Nil{})))

# opens on the projects; the folder list is fetched where it was last
# (home the first time)
def Proj.open(+u: Ui) -> Act:
  Proj.focus(Proj.find(Proj.rpc(Proj.mode(Ui.flag_on(u, "proj"), ""), "fs.list", [J.KV{"path", J.Str{Proj.dir(u)}}])))

# a machine chosen: its folders and projects are asked for afresh
def Proj.machine(+u: Ui, +on: String) -> Act:
  Proj.open(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "proj.on", on), "proj.dir", ""), "proj.found", ""), "proj.home", ""), "@mpick", ""), "@mpick.asked", ""), "@mpick.follow", ""))

def Proj.open.k(here: Bool, +u: Ui) -> Act:
  match here:
    case True{}:
      Proj.open(u)
    case False{}:
      Proj.machine(u, "")

# the picker as it opens: on the hub this client is connected to
def Proj.open.fresh(+u: Ui) -> Act:
  Proj.open.k(String.is_empty(Proj.on(u)), Ui.scratch_set(u, "@thread.route", ""))

def Proj.names(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{s} <> t:
      s <> Proj.names(t)
    case _ <> t:
      Proj.names(t)

# names are kept joined by NUL, which no file name holds: split once per
# use rather than parsing JSON every frame
def Proj.sep() -> Char:
  Chr{0}

def Proj.nonempty.k(skip: Bool, x: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      rest
    case False{}:
      x <> rest

def Proj.nonempty(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Proj.nonempty.k(String.is_empty(x), x, Proj.nonempty(t))

# the subfolders of the listed folder
def Proj.entries(u: Ui) -> List<&2, String>:
  Proj.nonempty(String.split(Ui.info(u, "proj.entries"), Proj.sep()))

# what an answer is to the picker: 0 not its newest request's, 1 a
# listing, 2 a project added, 3 an error
def Proj.which(mine: Bool, ok: Bool, dir: Bool) -> Nat:
  match mine ok dir:
    case False{} _ _:
      0n
    case True{} True{} True{}:
      1n
    case True{} True{} False{}:
      2n
    case True{} False{} _:
      3n

# an error shows in the picker while it is open (not also as a toast)
def Proj.failed(open: Bool, u: Ui, +e: String) -> Ui:
  match open:
    case True{}:
      Ui.info_set(Ui.error_put(u, ""), "proj.err", e)
    case False{}:
      u

def Proj.reply.k(i: Nat, +u: Ui, +o: J.Json) -> Ui:
  match i:
    case 1n:
      Ui.info_set(Ui.info_set(Ui.info_set(Ui.scratch_set(u, "@proj.rid", ""), "proj.dir", J.Json.get_str(J.Json.get_obj(o, "result"), "dir", "")), "proj.entries",
        String.join(Proj.names(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "result"), "entries"))), SCon{Proj.sep(), SNil{}})), "proj.err", "")
    case 2n:
      Ui.want_new(Ui.info_set(Ui.flag_off(Ui.scratch_set(u, "@proj.rid", ""), "proj"), "proj.err", ""), False{})
    case 3n:
      Proj.failed(Ui.flag(u, "proj"), Ui.want_new(Ui.scratch_set(u, "@proj.rid", ""), False{}), Ui.reply.err(o))
    case _:
      u

def Proj.found.mine(+u: Ui, +o: J.Json) -> Bool:
  Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@proj.frid"))), String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), Ui.scratch(u, "@proj.frid")))

# the projects found on disk, and home to shorten paths with. A hub that
# cannot look finds none, and says nothing of it
def Proj.found.put(ok: Bool, +u: Ui, +o: J.Json) -> Ui:
  match ok:
    case True{}:
      Ui.info_set(Ui.info_set(u, "proj.home", J.Json.get_str(J.Json.get_obj(o, "result"), "home", Ui.info(u, "proj.home"))), "proj.found",
        String.join(Proj.names(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "result"), "found"))), SCon{Proj.sep(), SNil{}}))
    case False{}:
      Ui.error_put(u, "")

def Proj.reply.f(found: Bool, +o: J.Json, +u: Ui) -> Ui:
  match found:
    case True{}:
      Proj.found.put(J.Json.get_bool(o, "ok", True{}), Ui.scratch_set(u, "@proj.frid", ""), o)
    case False{}:
      Proj.reply.k(Proj.which(Proj.mine(u, o), J.Json.get_bool(o, "ok", True{}), Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(o, "result"), "dir", "")))), u, o)

# an answer to the picker: the projects found fill its list, a listing
# replaces the one shown, an added project closes it, an error shows in it
def Proj.reply(+o: J.Json, +u: Ui) -> Ui:
  Proj.reply.f(Proj.found.mine(u, o), o, u)

def Proj.join(+dir: String, +name: String) -> String:
  Bool.pick(String, String.ends_with(dir, "/"), dir ++ name, dir ++ "/" ++ name)

def Proj.parent.k(+ps: List<&2, String>) -> String:
  Bool.pick(String, Nat.is_le(List.length(&2, String, ps), 2n), "/", String.join(List.take(&2, String, ps, Nat.sub(List.length(&2, String, ps), 1n)), "/"))

def Proj.parent(+dir: String) -> String:
  Proj.parent.k(String.split(dir, '/'))

def Proj.plain(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.and(U32.is_ge(c, 32), U32.is_ne(c, 127)), Proj.plain(t))

def Proj.name_ok.t(+q: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(q)), Bool.and(Bool.not(String.contains(q, "/")), Proj.plain(q))), Bool.and(Bool.not(String.eq(q, ".")), Bool.not(String.eq(q, ".."))))

# a name a new folder may take (trimmed): not blank, no slash or control
# characters, not "." or ".."
def Proj.name_ok(+q: String) -> Bool:
  Proj.name_ok.t(String.trim(q))

def Proj.exact(ns: List<&2, String>, +q: String) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case n <> t:
      Bool.or(String.eq(n, q), Proj.exact(t, q))

type Scored is Data:
  Scored{score: Nat, name: String}

def Scored.before(a: Scored, b: Scored) -> Bool:
  match a b:
    case Scored{+x, n} Scored{+y, m}:
      Bool.or(Nat.is_gt(x, y), Bool.and(Nat.is_eq(x, y), String.is_le(n, m)))

def Proj.scored.k(+sc: Nat, +n: String, +rest: List<&2, Scored>) -> List<&2, Scored>:
  Bool.pick(List<&2, Scored>, Nat.is_eq(sc, 0n), rest, Scored{sc, n} <> rest)

# matching folders; hidden ones (.git) only for a filter starting with "."
def Proj.scored(ns: List<&2, String>, +q: String) -> List<&2, Scored>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      Proj.scored.k(Bool.pick(Nat, Bool.and(String.starts_with(n, "."), Bool.not(String.starts_with(q, "."))), 0n, FZ.Fuzzy.score(n, q)), n, Proj.scored(t, q))

def Proj.rows(xs: List<&2, Scored>, +dir: String) -> List<&2, PItem>:
  match xs:
    case Nil{}:
      Nil{}
    case Scored{_, +n} <> t:
      PItem{n ++ "/", "proj-cd", Proj.join(dir, n)} <> Proj.rows(t, dir)

def Proj.is_project(ps: List<&2, M.Project>, +root: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case M.Project{_, _, r, _, rm} <> t:
      Bool.or(Bool.and(String.eq(r, root), Bool.not(rm)), Proj.is_project(t, root))

# the listed folder as a project (once)
def Proj.add(+l: I.Loc, +u: Ui, +dir: String) -> List<&2, PItem>:
  Bool.pick(List<&2, PItem>, Proj.is_project(Proj.ps(u), dir), [PItem{MSG.T.proj.exists(l, dir), "", ""}], [PItem{MSG.T.proj.add(l, dir), "proj-add", dir}])

# (a match, not Bool.pick: only the list shown is made)
def Proj.items.q(+l: I.Loc, empty: Bool, +u: Ui, +q: String, +dir: String, +ns: List<&2, String>) -> List<&2, PItem>:
  match empty:
    case True{}:
      List.concat(&2, PItem, [Proj.add(l, u, dir), Bool.pick(List<&2, PItem>, String.eq(dir, "/"), Nil{}, [PItem{"..", "proj-cd", Proj.parent(dir)}]),
        Proj.rows(Proj.scored(ns, q), dir)])
    case False{}:
      List.concat(&2, PItem, [Proj.rows(List.sort(~Scored, ~Scored.before, Proj.scored(ns, q)), dir),
        Bool.pick(List<&2, PItem>, Bool.and(Proj.name_ok(q), Bool.not(Proj.exact(ns, String.trim(q)))),
          [PItem{MSG.T.proj.create(l, String.trim(q)), "proj-new", Proj.join(dir, String.trim(q))}, PItem{MSG.T.proj.mkdir(l, String.trim(q)), "proj-mkdir", Proj.join(dir, String.trim(q))}], Nil{}),
        Proj.add(l, u, dir)])

# a typed path: a folder typed in full is added by Enter (its first row);
# one ending in a slash is gone into first, to pick inside it
def Proj.path.rows(into: Bool, +l: I.Loc, +q: String) -> List<&2, PItem>:
  match into:
    case True{}:
      [PItem{MSG.T.proj.goto(l, q), "proj-cd", q}, PItem{MSG.T.proj.add(l, q), "proj-add", q}, PItem{MSG.T.proj.createat(l, q), "proj-new", q}]
    case False{}:
      [PItem{MSG.T.proj.add(l, q), "proj-add", q}, PItem{MSG.T.proj.goto(l, q), "proj-cd", q}, PItem{MSG.T.proj.createat(l, q), "proj-new", q}]

def Proj.items.k(+l: I.Loc, path: Bool, +u: Ui, +q: String, +dir: String, +ns: List<&2, String>) -> List<&2, PItem>:
  match path:
    case True{}:
      Proj.path.rows(String.ends_with(q, "/"), l, q)
    case False{}:
      Proj.items.q(l, String.is_empty(q), u, q, dir, ns)

# the folder browser's rows. No filter: add the listed folder (once), the
# parent, the subfolders. A filter: matching subfolders best first, then
# creating the typed name (only when no folder has it), then adding. A
# typed path (/..., ~...) goes there instead
def Proj.folders(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  Proj.items.k(l, Bool.or(String.starts_with(Proj.query(u), "/"), String.starts_with(Proj.query(u), "~")), u, Proj.query(u), Proj.dir(u), Proj.entries(u))

# a path under home written with ~ (as is when home is not known)
def Proj.tilde(+home: String, +p: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(home)), Bool.or(String.eq(p, home), String.starts_with(p, home ++ "/"))),
    "~" ++ String.drop(p, String.length(home)), p)

# a folder's own name
def Proj.base(+p: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, Proj.nonempty(String.split(p, '/'))), p)

# a project the list may offer: live ones rank above the rest
type PCand is Data:
  PCand{rank: Nat, title: String, action: String, root: String}

def PCand.before(a: PCand, b: PCand) -> Bool:
  match a b:
    case PCand{+x, n, _, _} PCand{+y, m, _, _}:
      Bool.or(Nat.is_gt(x, y), Bool.and(Nat.is_eq(x, y), String.is_le(String.to_lower(n), String.to_lower(m))))

def Proj.cands.k(keep: Bool, c: PCand, rest: List<&2, PCand>) -> List<&2, PCand>:
  match keep:
    case True{}:
      c <> rest
    case False{}:
      rest

def Proj.cand(rm: Bool, ti: String, r: String) -> PCand:
  match rm:
    case True{}:
      PCand{1n, ti, "proj-found", r}
    case False{}:
      PCand{2n, ti, "proj-pick", r}

# the hub's projects: the live ones, or those removed
def Proj.cands.ps(ps: List<&2, M.Project>, +live: Bool) -> List<&2, PCand>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{_, ti, r, _, +rm} <> t:
      Proj.cands.k(Bool.pick(Bool, live, Bool.not(rm), rm), Proj.cand(rm, ti, r), Proj.cands.ps(t, live))

def Proj.cands.found(rs: List<&2, String>) -> List<&2, PCand>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      PCand{1n, Proj.base(r), "proj-found", r} <> Proj.cands.found(t)

def Proj.seen(xs: List<&2, PCand>, +root: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case PCand{_, _, _, r} <> t:
      Bool.or(String.eq(r, root), Proj.seen(t, root))

# each folder once, the first offer of it kept
def Proj.dedup(xs: List<&2, PCand>, +acc: List<&2, PCand>) -> List<&2, PCand>:
  match xs:
    case Nil{}:
      acc
    case PCand{r, ti, a, +ro} <> t:
      Proj.dedup(t, Proj.cands.k(Bool.not(Proj.seen(acc, ro)), PCand{r, ti, a, ro}, acc))

# live projects, then removed ones, then those found on disk
def Proj.cands(+u: Ui) -> List<&2, PCand>:
  Proj.dedup(List.concat(&2, PCand, [Proj.cands.ps(Proj.ps(u), True{}), Proj.cands.ps(Proj.ps(u), False{}),
    Proj.cands.found(Proj.nonempty(String.split(Ui.info(u, "proj.found"), Proj.sep())))]), Nil{})

def Proj.rank.k(+sc: Nat, +r: Nat, +ti: String, +a: String, +ro: String, +rest: List<&2, PCand>) -> List<&2, PCand>:
  Bool.pick(List<&2, PCand>, Nat.is_eq(sc, 0n), rest, PCand{Nat.add(Nat.mul(sc, 4n), r), ti, a, ro} <> rest)

# those whose name matches the filter
def Proj.rank(cs: List<&2, PCand>, +q: String) -> List<&2, PCand>:
  match cs:
    case Nil{}:
      Nil{}
    case PCand{r, +ti, a, ro} <> t:
      Proj.rank.k(FZ.Fuzzy.score(ti, q), r, ti, a, ro, Proj.rank(t, q))

def Proj.cand.row(+l: I.Loc, +a: String, +ti: String) -> String:
  Bool.pick(String, String.eq(a, "proj-pick"), ti, MSG.T.proj.addcand(l, ti))

def Proj.list.rows(+l: I.Loc, cs: List<&2, PCand>) -> List<&2, PItem>:
  match cs:
    case Nil{}:
      Nil{}
    case PCand{_, ti, +a, ro} <> t:
      PItem{Proj.cand.row(l, a, ti), a, ro} <> Proj.list.rows(l, t)

# the projects matching the filter, best first (live first for a tie)
def Proj.list.loc(+l: I.Loc, +u: Ui, +q: String) -> List<&2, PItem>:
  Proj.list.rows(l, List.sort(~PCand, ~PCand.before, Proj.rank(Proj.cands(u), q)))

def Proj.list(+u: Ui, q: String) -> List<&2, PItem>:
  Proj.list.loc(Lang.loc(u), u, q)

def Proj.browse.row(+l: I.Loc) -> PItem:
  PItem{MSG.T.proj.browse(l), "proj-browse", ""}

# (a match, not Bool.pick: only the list shown is made)
def Proj.items.p(+l: I.Loc, path: Bool, empty: Bool, +u: Ui, +q: String) -> List<&2, PItem>:
  match path empty:
    case True{} _:
      Proj.items.k(l, True{}, u, q, "", Nil{})
    case False{} True{}:
      Proj.browse.row(l) <> Proj.list.loc(l, u, q)
    case False{} False{}:
      List.append(&2, PItem, Proj.list.loc(l, u, q), [Proj.browse.row(l)])

def Proj.items.m(+l: I.Loc, dir: Bool, +u: Ui) -> List<&2, PItem>:
  match dir:
    case True{}:
      Proj.folders(l, u)
    case False{}:
      Proj.items.p(l, Bool.or(String.starts_with(Proj.query(u), "/"), String.starts_with(Proj.query(u), "~")), String.is_empty(Proj.query(u)), u, Proj.query(u))

# what the picker offers. The projects: adding from a folder first, then
# every project (with a filter: those matching, best first, then adding
# from a folder). A typed path goes there, adds it, or creates it. The
# folders: Proj.folders
def ThreadTree.routing(+u: Ui) -> Bool:
  Bool.and(Ui.flag(u, "proj"), Bool.not(String.is_empty(Ui.scratch(u, "@thread.route"))))

def ThreadTree.route.note() -> String:
  "Choose an existing project. Open the conversation that should manage the new subthread, then choose New subthread there. This conversation and its history stay here."

def ThreadTree.route.items(+l: I.Loc, +u: Ui, q: String) -> List<&2, PItem>:
  Proj.list.rows(l, List.sort(~PCand, ~PCand.before, Proj.rank(Proj.cands.ps(Proj.ps(u), True{}), q)))

def Proj.items.route(route: Bool, +l: I.Loc, +u: Ui) -> List<&2, PItem>:
  match route:
    case True{}: ThreadTree.route.items(l, u, Proj.query(u))
    case False{}: List.append(&2, PItem, Proj.items.m(l, Proj.browsing(u), u), Proj.machine.row.loc(l, u))

def Proj.items.loc(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  Proj.items.route(ThreadTree.routing(u), l, u)

def Proj.items(+u: Ui) -> List<&2, PItem>:
  Proj.items.loc(Lang.loc(u), u)

# the path a row of the projects list stands for, as it is shown beside it
def Proj.detail(+u: Ui, it: PItem) -> String:
  match it:
    case PItem{_, +a, +v}:
      Bool.pick(String, Bool.or(String.eq(a, "proj-pick"), String.eq(a, "proj-found")), Proj.tilde(Ui.info(u, "proj.home"), v), "")

type PBest is Data:
  PBest{id: String, at: Nat}

def PBest.at(b: PBest) -> Nat:
  match b:
    case PBest{_, +a}:
      a

def PBest.step(keep: Bool, id: String, a: Nat, b: PBest) -> PBest:
  match keep:
    case True{}:
      PBest{id, a}
    case False{}:
      b

# the project's thread most recently active (not archived or deleted)
def Proj.latest(ts: List<&2, M.Thread>, +pid: String, +b: PBest) -> PBest:
  match ts:
    case Nil{}:
      b
    case M.Thread{id, p, _, _, +a, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de} <> t:
      Proj.latest(t, pid, PBest.step(Bool.and(Bool.and(String.eq(p, pid), Bool.not(Bool.or(ar, de))), Nat.is_ge(a, PBest.at(b))), id, a, b))

def Proj.id_of.k(hit: Bool, id: String, rest: String) -> String:
  match hit:
    case True{}:
      id
    case False{}:
      rest

# the live project with this root ("" when none)
def Proj.id_of(ps: List<&2, M.Project>, +root: String) -> String:
  match ps:
    case Nil{}:
      ""
    case M.Project{id, _, r, _, rm} <> t:
      Proj.id_of.k(Bool.and(String.eq(r, root), Bool.not(rm)), id, Proj.id_of(t, root))

# the folder browser, where it was last (listed afresh)
def Proj.browse(+u: Ui) -> Act:
  Proj.rpc(Proj.mode(u, "dir"), "fs.list", [J.KV{"path", J.Str{Proj.dir(u)}}])

# the picker's actions, in Proj.run.k's order
def Proj.acts() -> List<&2, String>:
  ["proj-add", "proj-cd", "proj-mkdir", "proj-new", "proj-pick", "proj-found", "proj-browse", "proj-back", "proj-on"]

def Proj.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Proj.pos(t, x, 1n+i))

# Attachments
# -----------
# A thread's attachments wait in the drafts map under "@att:<thread>" (a
# path per line) until its message is sent; "@att.up:<thread>" names a file
# still uploading. No thread id starts with '@'.

def Att.keep(skip: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      rest
    case False{}:
      x <> rest

def Att.nonblank(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Att.keep(String.is_empty(String.trim(l)), String.trim(l), Att.nonblank(t))

def Att.drop(ps: List<&2, String>, +p: String) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +x <> t:
      Att.keep(String.eq(x, p), x, Att.drop(t, p))

# what a thread's next message attaches
def Ui.atts(u: Ui, +thread: String) -> List<&2, String>:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Att.nonblank(String.lines(Str.get(dr, "@att:" ++ thread)))

# the file a thread is still uploading ("" for none)
def Ui.uploading(u: Ui, +thread: String) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, "@att.up:" ++ thread)

def Ui.atts_set(u: Ui, +thread: String, ps: List<&2, String>) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@att:" ++ thread, String.join(ps, "\n")), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def Ui.up_set(u: Ui, +thread: String, name: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@att.up:" ++ thread, name), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# an upload finished: its path joins the thread's attachments
def Ui.attached(+u: Ui, +thread: String, +path: String) -> Ui:
  Ui.up_set(Ui.atts_set(u, thread, List.append(&2, String, Att.drop(Ui.atts(u, thread), path), [path])), thread, "")

def Ui.reply.att.k(done: Bool, failed: Bool, +thread: String, +path: String, u: Ui) -> Ui:
  match done failed:
    case True{} _:
      Ui.attached(u, thread, path)
    case False{} True{}:
      Ui.up_set(u, thread, "")
    case False{} False{}:
      u

def Att.last(xs: List<&2, String>, +acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Att.last(t, x)

# a thread's own id: a phone over several hubs names it "<hub>|<id>" (no
# thread id has '|'), and the hub answers with the name it was sent; the
# upload was marked under the hub's own id (law upload_thread_local)
def Att.local(+t: String) -> String:
  Att.last(String.split(t, '|'), t)

def Ui.reply.att(+r: J.Json, +ok: Bool, u: Ui) -> Ui:
  Ui.reply.att.k(Bool.not(String.is_empty(J.Json.get_str(r, "attached", ""))), Bool.and(Bool.not(ok), J.Json.get_bool(r, "upload", False{})),
    Att.local(J.Json.get_str(r, "thread", "")), J.Json.get_str(r, "attached", ""), u)

# The viewer's Files tab (core/fm.bend): a folder the hub listed is kept
# per thread (fm.list:<t>, and fm.dir:<t> the folder it is), and a PDF's
# page drawn for the window (pdf.png:<path>#<page>, pdf.n:<path> its pages,
# pdf.err:<path> why not)
def Fm.got.k(has: Bool, u: Ui, +r: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.info_set(Ui.info_set(Ui.info_set(u, "fm.list:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "fmEntries", "")), "fm.dir:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "fmDir", "")), "fm.err:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "fmError", ""))
    case False{}:
      u

def Pdf.got.k(has: Bool, u: Ui, +r: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.info_set(Ui.info_set(Ui.info_set(u, "pdf.err:" ++ J.Json.get_str(r, "pdfPath", ""), J.Json.get_str(r, "pdfErr", "")),
        "pdf.n:" ++ J.Json.get_str(r, "pdfPath", ""), J.Json.get_str(r, "pdfPages", "")),
        "pdf.png:" ++ J.Json.get_str(r, "pdfPath", "") ++ "#" ++ J.Json.get_str(r, "pdfPage", ""), J.Json.get_str(r, "pdfPng", ""))
    case False{}:
      u

def Fm.has(+r: J.Json, +k: String) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(r, k), J.Null{})), "null"))

def Fm.reply(u: Ui, +o: J.Json) -> Ui:
  Pdf.got.k(Fm.has(J.Json.get_obj(o, "result"), "pdfPath"), Fm.got.k(Fm.has(J.Json.get_obj(o, "result"), "fmEntries"), u, J.Json.get_obj(o, "result")), J.Json.get_obj(o, "result"))

# a diff opens the diff view; a file list (files.list) is kept for the
# file picker; a finished upload becomes an attachment of its thread; a
# folder or a PDF page for the viewer's Files tab is kept (Fm.reply)
def Ui.reply.body.k(u: Ui, +o: J.Json) -> Ui:
  Ui.reply.att(J.Json.get_obj(o, "result"), J.Json.get_bool(o, "ok", True{}),
    Ui.reply.files(Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "files"), J.Null{})), "null")),
      Ui.reply.diff(Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "diff"), J.Null{})), "null")), u, o), o))

def Ui.reply.body(u: Ui, +o: J.Json) -> Ui:
  Fm.reply(Ui.reply.body.k(u, o), o)

# an answer: its request leaves the outbox; a diff opens the diff view; a
# skills list is kept for the composer
# The Mechanical page
# -------------------
# A thread's parts (core/mech.bend). Mech opens the part on show (the
# first when none) in the 3D viewer, kind "3d" on its path, with a bar of
# the parts and a Renders button over it; Renders is kind "mech", the
# part's pictures, drawn on request (Create renders: mech.render, a job on
# the hub; "@mech.busy:<t>" names the part while it runs, "@mech.err:<t>"
# why it failed). Each thread's listing is kept as "mech:<t>" (the hub's
# cksum lines) and "mech.root:<t>" (its project root), asked for (mech.list)
# whenever either view comes up, and on Refresh.

# the selected thread's viewer set to a kind and file, its panel open
def Ui.view_put(u: Ui, +kind: String, +path: String) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, _, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@revert", ""), live, op, on, now, ad, True{}, v, rid, w, er, note, Map.set(&2, String, Viewer.put(nf, sel, kind, path), "panel:" ++ sel, "1"), ob, tr}

def Mpage.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "mech"), J.Null{})), "null"))

def Mpage.store(u: Ui, +r: J.Json) -> Ui:
  Ui.info_set(Ui.info_set(u, "mech:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "mech", "")), "mech.root:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "root", ""))

# a render job's answer: the part is no longer busy; why it failed, if it did
def Mpage.done.k(job: Bool, u: Ui, +th: String, +err: String) -> Ui:
  match job:
    case True{}:
      Ui.scratch_set(Ui.scratch_set(u, "@mech.busy:" ++ th, ""), "@mech.err:" ++ th, err)
    case False{}:
      u

# thread th's viewer on a file, its panel open (on screen when th is)
def Ui.view_put.at(u: Ui, +th: String, +kind: String, +path: String) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, +pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, Bool.or(pa, String.eq(th, sel)), v, rid, w, er, note, Map.set(&2, String, Viewer.put(nf, th, kind, path), "panel:" ++ th, "1"), ob, tr}

# The Files tab's actions: "fm" opens it on the folder last shown, "fm-cd"
# goes to a folder (from the project's root), "fm-up" to the one above,
# "fm-open" opens a file in the viewer (a board or schematic in the board
# viewer, a STEP or GLB in 3D, an image or a PDF in a view of its own; any
# other file is only listed), "pdf-page" turns a PDF's page in the window
def Fm.dir(+u: Ui) -> String:
  Ui.info(u, "fm.want:" ++ Ui.sel(u))

def Fm.at(+u: Ui, dir: String) -> String:
  Ui.sel(u) ++ "\n" ++ dir

def Fm.ask(+u: Ui, +dir: String) -> Act:
  Ui.on_thread.p(Ui.info_set(Ui.info_set(Ui.scratch_set(Ui.view_put(u, "files", ""), "@fm.at", Fm.at(u, dir)), "fm.want:" ++ Ui.sel(u), dir), "fm.top:" ++ Ui.sel(u), ""), "files.dir", [J.KV{"path", J.Str{dir}}])

# A reconnect's log drops the listings (they may be stale) but keeps each
# thread's folder (fm.want:<t>), so every kept folder is listed again;
# else a Files tab left open waits on "Listing the folder" for good
def Fm.again.join(cs: List<&2, Cmd>, a: Act) -> Act:
  match a:
    case Act{u, ds}:
      Act{u, List.append(&2, Cmd, cs, ds)}

def Fm.again.one(want: Bool, +k: String, a: Act) -> Act:
  match want a:
    case True{} Act{+u, cs}:
      Fm.again.join(cs, Ui.rpc(u, "files.dir", [J.KV{"thread", J.Str{String.drop(k, 8n)}}, J.KV{"path", J.Str{Ui.info(u, k)}}]))
    case False{} _:
      a

def Fm.again.k(+k: String, a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      # This key came from Map.keys: an empty value is the project root,
      # not an absent request. Reconnect must refresh that folder too.
      Fm.again.one(String.starts_with(k, "fm.want:"), k, Act{u, cs})

def Fm.again.go(ks: List<&2, String>, a: Act) -> Act:
  match ks:
    case Nil{}:
      a
    case +k <> t:
      Fm.again.go(t, Fm.again.k(k, a))

def Fm.again.keys(u: Ui) -> List<&2, String>:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, ob, tr}:
      Map.keys(&2, String, nf)

def Fm.again(log: Bool, a: Act) -> Act:
  match log a:
    case True{} Act{+u, cs}:
      Fm.again.go(Fm.again.keys(u), Act{u, cs})
    case False{} _:
      a

# A restored Files panel has no listing in its fresh view actor. Ask once
# for this thread and folder; ordinary refreshes do not repeat the read.
def Fm.need(panel: Bool, +kind: String, +at: String, +asked: String) -> Bool:
  Bool.and(panel, Bool.and(String.eq(kind, "files"), Bool.not(String.eq(at, asked))))

def Fm.sync.k(needed: Bool, a: Act) -> Act:
  match needed a:
    case True{} Act{+u, cs}: Ui.also(Fm.ask(u, Fm.dir(u)), cs)
    case False{} _: a

def Fm.sync(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Fm.sync.k(Fm.need(Ui.panel(u), Ui.view_get(u, "view.kind"), Fm.at(u, Fm.dir(u)), Ui.scratch(u, "@fm.at")), Act{u, cs})

def Pdf.ask(+u: Ui, +path: String, +page: String) -> Act:
  Ui.rpc(Ui.info_set(u, "pdf.page:" ++ Ui.sel(u), page), "pdf.page", [J.KV{"path", J.Str{path}}, J.KV{"page", J.Str{page}}])

# a PDF opened: its first page is asked for; on a thread elsewhere once the
# file is in the cache (Fv.sync)
def Fm.pdf(far: Bool, +u: Ui, +path: String) -> Act:
  match far:
    case True{}:
      Act{Ui.info_set(Ui.view_put(u, "pdf", path), "pdf.page:" ++ Ui.sel(u), ""), Nil{}}
    case False{}:
      Pdf.ask(Ui.view_put(u, "pdf", path), path, "1")

def Fm.open.k(i: Nat, +u: Ui, +kind: String, +path: String) -> Act:
  match i:
    case 0n:
      Act{Ui.view_put(u, kind, path), Nil{}}
    case 1n:
      Fm.pdf(FR.Fr.is(Ui.sel(u)), u, path)
    case _:
      Act{u, Nil{}}

def Fm.open(+u: Ui, +rel: String) -> Act:
  Fm.open.k(Bool.pick(Nat, Bool.or(String.is_empty(FM.Fm.preview(rel)), String.is_empty(Ui.root(u))), 2n, Bool.pick(Nat, String.eq(FM.Fm.preview(rel), "pdf"), 1n, 0n)), u, FM.Fm.preview(rel), FM.Fm.abs(Ui.root(u), rel))

# the page a PDF shows in the window, its picture, how many pages, why not
def Pdf.page(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Ui.info(u, "pdf.page:" ++ Ui.sel(u))), "1", Ui.info(u, "pdf.page:" ++ Ui.sel(u)))

def Pdf.png(+u: Ui) -> String:
  Ui.info(u, "pdf.png:" ++ Ui.view_get(u, "view.path") ++ "#" ++ Pdf.page(u))

def Pdf.pages(+u: Ui) -> String:
  Ui.info(u, "pdf.n:" ++ Ui.view_get(u, "view.path"))

def Pdf.err(+u: Ui) -> String:
  Ui.info(u, "pdf.err:" ++ Ui.view_get(u, "view.path"))

# The web's layer focus (core/layers.bend): the active layer and how the
# others show, for the board and for the schematic apart ("@focus.<kind>",
# beside Solid.off's "@off.<kind>"); "view-layer-pick" makes a layer the
# active one (LY.Lyr.pick), "view-layer-mode" sets how the others show
# (0 as they are, 1 dimmed, 2 hidden)
def Lfocus.kind.of(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Ui.view_get(u, "view.kind")), "board", Ui.view_get(u, "view.kind"))

def Lfocus.key(+u: Ui) -> String:
  "@focus." ++ Lfocus.kind.of(u)

def Lfocus.of(+u: Ui) -> LY.Focus:
  LY.Lyr.read(Ui.scratch(u, Lfocus.key(u)))

# the look kind of what the web shows (0 a board, 1 a schematic)
def Lfocus.kind(+u: Ui) -> U32:
  Bool.pick(U32, String.eq(Lfocus.kind.of(u), "schematic"), 1, 0)

def Lfocus.put(+u: Ui, +f: LY.Focus) -> Act:
  Act{Ui.scratch_set(u, Lfocus.key(u), LY.Lyr.show(f)), Nil{}}

def Lfocus.pick(+u: Ui, +value: String) -> Act:
  Lfocus.put(u, LY.Lyr.pick(Lfocus.of(u), LY.Lyr.num(value, LY.Lyr.none())))

def Lfocus.mode(+u: Ui, +value: String) -> Act:
  Lfocus.put(u, LY.Lyr.set(Lfocus.kind(u), Lfocus.of(u), LY.Lyr.num(value, 0)))

def Fm.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Fm.ask(u, Fm.dir(u))
    case 1n:
      Fm.ask(u, value)
    case 2n:
      Fm.ask(u, FM.Fm.parent(Fm.dir(u)))
    case 3n:
      Fm.open(u, value)
    case 4n:
      Pdf.ask(u, Ui.view_get(u, "view.path"), value)
    # (the window shows a long folder a page at a time: the first row shown)
    case 5n:
      Act{Ui.info_set(u, "fm.top:" ++ Ui.sel(u), value), Nil{}}
    # (an update held back while threads work goes now)
    case 6n:
      Ui.rpc(u, "update.now", Nil{})
    # (a Subagents row's Stop: SB.Subs.cancel)
    case 7n:
      Ui.rpc(u, "work.cancel", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"id", J.Str{value}}])
    case 8n:
      Ui.rpc(u, "task.cancel", [J.KV{"child", J.Str{value}}])
    case 9n:
      Lfocus.pick(u, value)
    case 10n:
      Lfocus.mode(u, value)
    case 14n:
      Ui.rpc(u, "lock.release", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"name", J.Str{value}}])
    case 15n:
      Ui.rpc(u, "lock.cancel", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"name", J.Str{value}}])
    case 12n:
      Ui.rpc(u, "wake.cancel", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"id", J.Str{value}}])
    # (a restart waiting for a safe point is cancelled: the plane's row)
    case 13n:
      Ui.rpc(u, "deploy.cancel", Nil{})
    case _:
      Act{u, Nil{}}

# A listed entry as every client shows it: its name, what a tap sends, its
# size (files), and its kind ("folder", "board", "schematic", "3d",
# "image", "pdf", or "" for a file only listed)
type FmRow is Data:
  FmRow{label: String, action: String, value: String, detail: String, kind: String}

def Fm.row(e: FM.FEnt, +dir: String) -> FmRow:
  match e:
    case FM.FEnt{True{}, _, +n}:
      FmRow{n, "fm-cd", FM.Fm.join(dir, n), "", "folder"}
    case FM.FEnt{False{}, +size, +n}:
      FmRow{n, Bool.pick(String, String.is_empty(FM.Fm.preview(n)), "", "fm-open"), FM.Fm.join(dir, n), FM.Fm.size(size), FM.Fm.preview(n)}

def Fm.rows.go(es: List<&2, FM.FEnt>, +dir: String) -> List<&2, FmRow>:
  match es:
    case Nil{}:
      Nil{}
    case e <> t:
      Fm.row(e, dir) <> Fm.rows.go(t, dir)

# whether the listing on screen is the folder asked for
def Fm.received.k(r: Map<&2, String> & Bool) -> Bool:
  (_, found) = r
  found

def Fm.received(u: Ui) -> Bool:
  match u:
    case Ui{_, sel, _, _, _, _, _, _, _, _, _, _, _, _, nf, _, _}:
      Fm.received.k(Map.has(&2, String, nf, "fm.dir:" ++ sel))

def Fm.ready(+u: Ui) -> Bool:
  # An answered empty root has both strings empty. Presence distinguishes
  # that successful reply from a missing or invalidated listing.
  Bool.and(Fm.received(u), String.eq(Ui.info(u, "fm.dir:" ++ Ui.sel(u)), Fm.dir(u)))

# the folder's rows, once the hub has listed it
def Fm.rows(+u: Ui) -> List<&2, FmRow>:
  Bool.pick(List<&2, FmRow>, Fm.ready(u), Fm.rows.go(FM.Fm.parse(Ui.info(u, "fm.list:" ++ Ui.sel(u))), Fm.dir(u)), Nil{})

def Fm.trail.go(ps: List<&2, String>, +at: String) -> List<&2, PItem>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      PItem{p, "fm-cd", FM.Fm.join(at, p)} <> Fm.trail.go(t, FM.Fm.join(at, p))

# where the folder is: the project, then each folder down to it
def Fm.trail.loc(+l: I.Loc, +u: Ui) -> List<&2, PItem>:
  PItem{MSG.T.fm.root(l), "fm-cd", ""} <> Fm.trail.go(Bool.pick(List<&2, String>, String.is_empty(Fm.dir(u)), Nil{}, String.split(Fm.dir(u), '/')), "")

def Fm.trail(+u: Ui) -> List<&2, PItem>:
  Fm.trail.loc(Lang.loc(u), u)

# a row's icon (core/icons.bend), the same on every client
def Fm.icon(+kind: String) -> String:
  Bool.pick(String, String.eq(kind, "folder"), "folder", Bool.pick(String, String.eq(kind, "board"), "circuit-board",
    Bool.pick(String, String.eq(kind, "schematic"), "cpu", Bool.pick(String, String.eq(kind, "3d"), "box",
      Bool.pick(String, String.eq(kind, "image"), "image", Bool.pick(String, String.eq(kind, "pdf"), "file-text", "file"))))))

# the tab a kind lights: an image, PDF or text note belongs to Files
def Fm.tab(+kind: String) -> String:
  Bool.pick(String, Bool.or(String.eq(kind, "text"), Bool.or(String.eq(kind, "image"), String.eq(kind, "pdf"))), "files", kind)

# the first row the window shows of a long folder
def Fm.top(+u: Ui) -> Nat:
  FM.Fm.num(Ui.info(u, "fm.top:" ++ Ui.sel(u)))

# an open file's name, for the image and PDF views' heads
def Fm.name(+u: Ui) -> String:
  FM.Fm.last(Ui.view_get(u, "view.path"))

# Mech opened (Mpage.open marks "@mech.to3d:<t>"; it shows at once what
# the parts last listed give, and asks for them again, Mpage.sync): once
# the fresh listing is in, the thread's viewer shows what it gives
# (MC.Mech.after: the part on show while it is still there, else the
# assembly or the first part), or the page that says there are none, and
# the next listing with parts opens one. Before, a part that had gone
# stayed on show. The viewer only moves when that differs from what it
# shows (a part still there stays as it is: no refit, no flicker), and only
# while it is still on Mech, a part in 3D or the parts page (Renders or a
# view picked meanwhile stay). A move leaves "@mech.at" at the view it
# made, so the parts are not listed again for it.
def Mpage.on.at(+u: Ui, +th: String) -> Bool:
  Bool.or(Bool.and(String.eq(Ui.info(u, "view.kind:" ++ th), "3d"), MC.Mech.is_model(Ui.info(u, "view.path:" ++ th))),
    Bool.and(String.eq(Ui.info(u, "view.kind:" ++ th), "mech"), String.is_empty(Ui.info(u, "view.path:" ++ th))))

# what the thread's fresh listing gives (a path from its project root)
def Mpage.after(+u: Ui, +th: String) -> String:
  MC.Mech.after(MC.Mech.parts(Ui.info(u, "mech:" ++ th)), Ui.info(u, "view.path:" ++ th), Ui.info(u, "mech.root:" ++ th))

def Mpage.seen.k(sel: Bool, +u: Ui) -> Ui:
  match sel:
    case True{}:
      Ui.scratch_set(u, "@mech.at", Ui.sel(u) ++ "#" ++ Ui.view_get(u, "view.n"))
    case False{}:
      u

def Mpage.moved(+u: Ui, +th: String, +kind: String, +path: String) -> Ui:
  Mpage.seen.k(String.eq(th, Ui.sel(u)), Ui.view_put.at(u, th, kind, path))

# (none left: a part still in 3D gives way to the parts page, which stays
# marked; else the part, unless it is the one on show)
def Mpage.to3d.go(none: Bool, +u: Ui, +th: String, +path: String) -> Ui:
  match none:
    case True{}:
      Bool.pick(Ui, String.eq(Ui.info(u, "view.kind:" ++ th), "3d"), Mpage.moved(u, th, "mech", ""), u)
    case False{}:
      Bool.pick(Ui, Bool.and(String.eq(Ui.info(u, "view.kind:" ++ th), "3d"), String.eq(Ui.info(u, "view.path:" ++ th), path)),
        Ui.scratch_set(u, "@mech.to3d:" ++ th, ""), Mpage.moved(Ui.scratch_set(u, "@mech.to3d:" ++ th, ""), th, "3d", path))

def Mpage.to3d.k(on: Bool, +u: Ui, +th: String) -> Ui:
  match on:
    case True{}:
      Mpage.to3d.go(String.is_empty(Mpage.after(u, th)), u, th, MC.Mech.abs(Ui.info(u, "mech.root:" ++ th), Mpage.after(u, th)))
    case False{}:
      Ui.scratch_set(u, "@mech.to3d:" ++ th, "")

def Mpage.to3d.m(marked: Bool, +u: Ui, +th: String) -> Ui:
  match marked:
    case True{}:
      Mpage.to3d.k(Mpage.on.at(u, th), u, th)
    case False{}:
      u

def Mpage.to3d(+u: Ui, +th: String) -> Ui:
  Mpage.to3d.m(String.eq(Ui.scratch(u, "@mech.to3d:" ++ th), "1"), u, th)

def Mpage.got(u: Ui, +r: J.Json) -> Ui:
  Mpage.to3d(Mpage.done.k(Bool.not(String.is_empty(J.Json.get_str(r, "rendered", ""))), Mpage.store(u, r), J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "error", "")), J.Json.get_str(r, "thread", ""))

def Mpage.reply(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Mpage.got(u, J.Json.get_obj(o, "result"))
    case False{}:
      u

# ask for a thread's parts
def Mpage.ask(u: Ui, +th: String) -> Act:
  Ui.rpc(u, "mech.list", [J.KV{"thread", J.Str{th}}])

# Reading the page: the selected thread's listing and root
def Mpage.text(+u: Ui) -> String:
  Ui.info(u, "mech:" ++ Ui.sel(u))

def Mpage.root(+u: Ui) -> String:
  Ui.info(u, "mech.root:" ++ Ui.sel(u))

def Mpage.parts(+u: Ui) -> List<&2, MC.MPart>:
  MC.Mech.parts(Mpage.text(u))

# the part on show
def Mpage.cur(+u: Ui) -> Maybe<&2, MC.MPart>:
  MC.Mech.pick(Mpage.parts(u), Ui.view_get(u, "view.path"), Mpage.root(u))

# 0 while the parts are asked for, 1 when there are none, 2 with parts
def Mpage.state(+u: Ui) -> Nat:
  Bool.pick(Nat, String.is_empty(Mpage.root(u)), 0n, Bool.pick(Nat, List.is_empty(&2, MC.MPart, Mpage.parts(u)), 1n, 2n))

def Mpage.path.k(m: Maybe<&2, MC.MPart>) -> String:
  match m:
    case Some{p}:
      MC.Mech.part.path(p)
    case None{}:
      ""

def Mpage.cur.path(+u: Ui) -> String:
  Mpage.path.k(Mpage.cur(u))

# the project a thread's parts are under: the listing's, else the thread's
def Mpage.base(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Mpage.root(u)), Ui.root(u), Mpage.root(u))

# the viewer shows a model in 3D: a STEP or GLB, one of the thread's parts
# or any other an agent showed. That is Mech; the 3D tab is the board's own
def Mpage.onpart(+u: Ui) -> Bool:
  Bool.and(String.eq(Ui.view_get(u, "view.kind"), "3d"), MC.Mech.is_model(Ui.view_get(u, "view.path")))

# The rail
# --------
# The web's tool rail (view.bend's View.workspace): a button for each view
# (Board, Schematic, 3D, Mech, Files, Diff, Review, Browser) and the tools
# (Terminal, Commands). A button toggles what it opens: a view's button
# pressed while its view is on show shuts the viewer (the "panel" action),
# pressed otherwise it shows its view; Terminal and Commands open and shut
# theirs. The keys do the same (ctrl+D Diff, ctrl+P Files; workspace.bend's
# WS.key). Laws rail_*.

# the view the rail lights: the one on show, "" while the viewer is shut (a
# part shown in 3D is Mech's, an image or a PDF Files', as the viewer's own
# tabs say)
def Rail.tab.of(open: Bool, +kind: String, +path: String) -> String:
  match open:
    case True{}:
      Bool.pick(String, Bool.and(String.eq(kind, "3d"), MC.Mech.is_model(path)), "mech", Fm.tab(kind))
    case False{}:
      ""

def Rail.tab(+u: Ui) -> String:
  Rail.tab.of(Ui.panel(u), Ui.view_get(u, "view.kind"), Ui.view_get(u, "view.path"))

# a view's button shuts the viewer when its view is the one on show
def Rail.shuts(+cur: String, +kind: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(cur)), String.eq(cur, kind))

# what the button of view `kind` sends: its own action and value, or
# "panel" (the viewer shut) while its view is on show
def Rail.act(+cur: String, +kind: String, +act: String) -> String:
  Bool.pick(String, Rail.shuts(cur, kind), "panel", act)

def Rail.value(+cur: String, +kind: String, +value: String) -> String:
  Bool.pick(String, Rail.shuts(cur, kind), "", value)

# Commands: pressed (or its keys) while the palette is open, it shuts it
def Rail.palette(open: Bool) -> String:
  Bool.pick(String, open, "ws-palette-close", "ws-palette-open")

# the model on show, from the project root when it is under it
def Mpage.shown(+u: Ui) -> String:
  Bool.pick(String, String.starts_with(Ui.view_get(u, "view.path"), Mpage.base(u) ++ "/"),
    String.drop(Ui.view_get(u, "view.path"), 1n+String.length(Mpage.base(u))), Ui.view_get(u, "view.path"))

# the model on show is one of the thread's parts (its chip is lit, and it
# has Renders); another model is named on its own
def Mpage.listed(+u: Ui) -> Bool:
  Bool.not(Maybe.is_none(&2, MC.MPart, MC.Mech.pick.k(Mpage.parts(u), Mpage.shown(u))))

# a render job running for the part shown (its path), and why the last failed
def Mpage.busy(+u: Ui) -> String:
  Ui.scratch(u, "@mech.busy:" ++ Ui.sel(u))

def Mpage.err(+u: Ui) -> String:
  Ui.scratch(u, "@mech.err:" ++ Ui.sel(u))

def Mpage.shots.k(m: Maybe<&2, MC.MPart>) -> Bool:
  match m:
    case Some{MC.MPart{_, _, fs}}:
      Bool.not(List.is_empty(&2, MC.MFile, fs))
    case None{}:
      False{}

# what the render button says: its pictures drawn, again, or at work
def Mpage.render.label.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, Bool.not(String.is_empty(Mpage.busy(u))), MSG.T.mech.rendering(l), Bool.pick(String, Mpage.shots.k(Mpage.cur(u)), MSG.T.mech.again(l), MSG.T.mech.create(l)))

def Mpage.render.label(+u: Ui) -> String:
  Mpage.render.label.loc(Lang.loc(u), u)

def Mpage.open.k(known: Bool, +u: Ui, +path: String) -> Ui:
  match known:
    case True{}:
      Ui.view_put(Ui.scratch_set(u, "@mech.to3d:" ++ Ui.sel(u), "1"), "3d", path)
    case False{}:
      Ui.view_put(Ui.scratch_set(u, "@mech.to3d:" ++ Ui.sel(u), "1"), "mech", "")

# Mech: the part on show in 3D, once the parts are known; either way they
# are listed again (Mpage.sync: a new view) and what that listing gives is
# shown when it comes (Mpage.to3d)
def Mpage.open(+u: Ui) -> Ui:
  Mpage.open.k(Nat.is_eq(Mpage.state(u), 2n), u, MC.Mech.abs(Mpage.root(u), Mpage.cur.path(u)))

# another part: in 3D when a part is on show there, else its pictures
def Mpage.part.k(three: Bool, +u: Ui, +value: String) -> Ui:
  match three:
    case True{}:
      Ui.view_put(u, "3d", MC.Mech.abs(Mpage.root(u), value))
    case False{}:
      Ui.view_put(u, "mech", value)

# the web viewer's layers turned off, a bit per layer (as core/look.bend
# numbers them) for the board and for the schematic apart
def Solid.off.key(+u: Ui) -> String:
  "@off." ++ Bool.pick(String, String.is_empty(Ui.view_get(u, "view.kind")), "board", Ui.view_get(u, "view.kind"))

def Solid.off(+u: Ui) -> U32:
  Maybe.default(&2, U32, U32.read(Ui.scratch(u, Solid.off.key(u))), 0)

# the layers turned off and the focus a kind keeps (the combined view's
# panes show each as its own tab does)
def Solid.off.of(+u: Ui, +kind: String) -> U32:
  Maybe.default(&2, U32, U32.read(Ui.scratch(u, "@off." ++ kind)), 0)

def Lfocus.of.at(+u: Ui, +kind: String) -> LY.Focus:
  LY.Lyr.read(Ui.scratch(u, "@focus." ++ kind))

# a sheet picked while a comparison shows: that sheet compared (the same
# versions, the same layers); anything else as picked
def Rv.sheet.k(cmp: Bool, +cur: String, +file: String) -> String:
  match cmp:
    case True{}:
      HI.Hist.sheet(cur, file)
    case False{}:
      file

def Rv.sheet(+cur: String, +file: String) -> String:
  Rv.sheet.k(Bool.and(Nat.is_eq(HI.Hist.kind(cur), 2n), Bool.not(String.is_empty(file))), cur, file)

# The project's boards (the card's board files, from its folder, kept as
# "fab.boards:<t>"): with more than one, the viewer's Board, Schematic and
# 3D tabs offer each by its folder's name (chips on the web and in the
# window, the Sheet menu on phones); "board-pick" shows one (laws bd_*)
def Bd.files(+u: Ui) -> List<&2, String>:
  Att.nonblank(String.lines(Ui.info(u, "fab.boards:" ++ Ui.sel(u))))

def Bd.many(+u: Ui) -> Bool:
  Nat.is_gt(List.length(&2, String, Bd.files(u)), 1n)

def Bd.name(+rel: String) -> String:
  Bool.pick(String, String.is_empty(MC.Mech.parent(rel)), MC.Mech.stem(rel), MC.Mech.last(MC.Mech.parent(rel)))

# what a kind shows of a board: its schematic beside it, else the board
def Bd.path(+kind: String, +rel: String) -> String:
  Bool.pick(String, String.eq(kind, "schematic"), MC.Mech.in(MC.Mech.parent(rel), MC.Mech.stem(rel) ++ ".kicad_sch"), rel)

# the board on show: the viewer's file is in its folder (or is it)
def Bd.on(+path: String, +rel: String) -> Bool:
  Bool.pick(Bool, String.is_empty(MC.Mech.parent(rel)), String.ends_with(path, "/" ++ rel),
    String.contains(path, "/" ++ MC.Mech.parent(rel) ++ "/"))

# the board on show: the one whose folder holds the viewer's file ("" for
# none), and what another tab shows of it (Bd.keep: switching between
# Board, Schematic and 3D stays on the board picked; "" when the project
# has one board or the viewer is on none of them, for the project's own)
def Bd.cur(fs: List<&2, String>, +path: String) -> String:
  match fs:
    case Nil{}:
      ""
    case +f <> t:
      Bool.pick(String, Bd.on(path, f), f, Bd.cur(t, path))

def Bd.keep.k(+cur: String, +kind: String, +root: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(cur)), Bool.or(Bool.or(String.eq(kind, "board"), String.eq(kind, "split")), Bool.or(String.eq(kind, "schematic"), String.eq(kind, "3d")))),
    FM.Fm.abs(root, Bd.path(kind, cur)), "")

# Non-drawing tabs keep their own path; the last drawing is separate.
def Bd.context(+u: Ui) -> String:
  Bool.pick(String, Viewer.design(Ui.view_get(u, "view.kind"), Ui.view_get(u, "view.path")), Ui.view_get(u, "view.path"), Ui.view_get(u, "view.design"))

# A picked board is useful before its asynchronously read card arrives.
# Its sheets keep this board anchor while remembering their own file.
def Bd.anchor(+kind: String, +board: String) -> String:
  Bool.pick(String, Bool.and(String.ends_with(board, ".kicad_pcb"), Bool.or(Bool.or(String.eq(kind, "board"), String.eq(kind, "split")), String.eq(kind, "3d"))), board, "")

def Bd.tree.saved(+root: String, +tree: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(root)), Bool.and(String.starts_with(tree, root ++ "/"),
    Bool.and(String.ends_with(tree, ".kicad_sch"), FM.Fm.safe(String.drop(tree, 1n+String.length(root)))))), tree, "")

def Bd.keep(+u: Ui, +kind: String, +path: String) -> String:
  +kept = Bd.keep.k(Bool.pick(String, Bd.many(u), Bd.cur(Bd.files(u), path), ""), kind, Ui.root(u))
  Bool.pick(String, String.is_empty(kept), Bool.pick(String, String.eq(kind, "schematic"), Bd.tree.saved(Ui.root(u), Ui.view_get(u, "view.tree")), Bd.anchor(kind, Ui.view_get(u, "view.design"))), kept)

def Bd.or(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

# the file the viewer shows: the one picked, else the project's own board
# (the card's board_default, "fab.board:<t>"), so its chip is lit too
def Bd.shown.k(+path: String, +root: String, +own: String) -> String:
  Bd.or(path, Bool.pick(String, Bool.or(String.is_empty(own), String.is_empty(root)), "", FM.Fm.abs(root, own)))

def Bd.shown(+u: Ui, +path: String) -> String:
  Bd.shown.k(path, Ui.root(u), Ui.info(u, "fab.board:" ++ Ui.sel(u)))

# the root sheet of the board on show where a project has several: its
# own hierarchy (the hub reads the tree from there); "" for the project's
# own (path: the file on show)
def Bd.tree.k(+cur: String, +root: String) -> String:
  Bool.pick(String, String.is_empty(cur), "", FM.Fm.abs(root, Bd.path("schematic", cur)))

def Bd.tree(+u: Ui, +path: String) -> String:
  Bd.or(Bd.tree.saved(Ui.root(u), Ui.view_get(u, "view.tree")),
    Bool.pick(String, Bd.many(u), Bd.tree.k(Bd.cur(Bd.files(u), Bd.shown(u, path)), Ui.root(u)), ""))

def Bd.sheet.allowed(+u: Ui, +path: String) -> Bool:
  +sheet = Ui.view_get(u, "view.sheet")
  +tree = Ui.view_get(u, "view.tree")
  Bool.and(Bool.not(String.is_empty(sheet)), Bool.or(Bool.and(Bool.not(String.is_empty(tree)), String.eq(path, tree)),
    Bool.or(Bool.and(String.is_empty(path), Bool.not(Bd.many(u))), String.eq(MC.Mech.parent(sheet), MC.Mech.parent(path)))))

def Bd.id.k(+root: String, +board: String, +tree: String, +known: String) -> String:
  Bool.pick(String, Bool.and(String.is_empty(known), Bool.and(Bool.not(String.is_empty(root)),
    Bool.and(String.ends_with(board, ".kicad_pcb"), Bool.and(String.starts_with(board, root ++ "/"), String.starts_with(tree, root ++ "/"))))),
    String.drop(board, 1n+String.length(root)), known)

def Bd.id(+u: Ui) -> String:
  Bd.id.k(Ui.root(u), Ui.view_get(u, "view.design"), Bd.tree.saved(Ui.root(u), Ui.view_get(u, "view.tree")), Bd.cur(Bd.files(u), Bd.shown(u, Bd.context(u))))

# The card is asked again only when the thread or its turn moves on (Fab.sync),
# so a board added or removed meanwhile never reached the picker. Each time
# the picker opens ("board-list": the web's board selector as it opens,
# data-on-open; a phone's viewer opening on a board, mobile/view.bend's
# View.open.k) the boards are read again (boards.list, one rescan of the
# thread's folder: core/order.bend's Boards.list) and kept where the card
# keeps them, so the picker shows what is there; an answer that found the
# same boards changes nothing on screen. Never for a thread elsewhere (its
# hub has the files) or with none selected (laws bd_look_*).
def Bd.can(+th: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(th)), Bool.not(FR.Fr.is(th)))

def Bd.look.k(ask: Bool, +u: Ui) -> Act:
  match ask:
    case True{}:
      Ui.rpc(u, "boards.list", [J.KV{"thread", J.Str{Ui.sel(u)}}])
    case False{}:
      Act{u, Nil{}}

def Bd.look(+u: Ui) -> Act:
  Bd.look.k(Bd.can(Ui.sel(u)), u)

# (an answer to boards.list: OR.Boards.listed)
def Bd.has(+o: J.Json) -> Bool:
  OR.Boards.listed(J.Json.get_obj(o, "result"))

def Bd.got(u: Ui, +r: J.Json) -> Ui:
  Ui.info_set(Ui.info_set(u, "fab.boards:" ++ J.Json.get_str(r, "thread", ""), OR.Boards.text(r)), "fab.board:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "board_default", ""))

def Bd.reply(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Bd.got(u, J.Json.get_obj(o, "result"))
    case False{}:
      u

# The combined view
# -----------------
# Kind "split": the schematic beside the board and its 3D model at once,
# each pane named, put away and brought back on its own (the setting
# "split.hide": the panes put away, words of "sch pcb 3d"; one always
# stays, test/split_test.bend). Its board is the viewer's file, as the Board tab's
# (the board picked, else the project's own); its sheet is kept per thread
# ("@split.sheet:<t>"): the board's own schematic until another is opened
# there.

def Split.panes() -> List<&2, String>:
  ["sch", "pcb", "3d"]

def Split.hide(+u: Ui) -> String:
  M.Setting.get(Ui.state(u), "split.hide", "")

def Split.has(+hide: String, +p: String) -> Bool:
  String.contains(" " ++ hide ++ " ", " " ++ p ++ " ")

def Split.flip.keep(on: Bool, +q: String, rest: List<&2, String>) -> List<&2, String>:
  match on:
    case True{}:
      q <> rest
    case False{}:
      rest

def Split.flip.go(ps: List<&2, String>, +hide: String, +p: String) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +q <> t:
      Split.flip.keep(Bool.xor(Split.has(hide, q), String.eq(q, p)), q, Split.flip.go(t, hide, p))

def Split.flip.k(+hs: List<&2, String>, +hide: String) -> String:
  Bool.pick(String, Nat.is_ge(List.length(&2, String, hs), 3n), hide, String.join(hs, " "))

# the panes put away once pane p is flipped, in order; the last one shown
# stays
def Split.flip(+hide: String, +p: String) -> String:
  Split.flip.k(Split.flip.go(Split.panes(), hide, p), hide)

def Split.on(+u: Ui, +p: String) -> Bool:
  Bool.not(Split.has(Split.hide(u), p))

def Split.is(+u: Ui) -> Bool:
  String.eq(Ui.view_get(u, "view.kind"), "split")

def Split.key(+u: Ui) -> String:
  "@split.sheet:" ++ Ui.sel(u)

# (many: the project has several boards, root: the board's own schematic;
# a sheet of another board's folder is left for the board's own)
def Split.sheet.k(+kept: String, +root: String, many: Bool) -> String:
  Bool.pick(String, Bool.or(String.is_empty(kept), Bool.and(many, Bool.not(String.eq(MC.Mech.parent(kept), MC.Mech.parent(root))))), root, kept)

def Split.sheet(+u: Ui) -> String:
  +root = Bd.tree(u, Ui.view_get(u, "view.path"))
  Bool.pick(String, Bd.sheet.allowed(u, root), Ui.view_get(u, "view.sheet"), Split.sheet.k(Ui.scratch(u, Split.key(u)), root, Bd.many(u)))

# the panes the hub watches for it: "s" the schematic, "b" the board, "3"
# its 3D model
def Split.letters(+u: Ui) -> String:
  Bool.pick(String, Split.on(u, "sch"), "s", "") ++ Bool.pick(String, Split.on(u, "pcb"), "b", "") ++ Bool.pick(String, Split.on(u, "3d"), "3", "")

# a pane's source: "<kind>|<root>|<path>", as the hub keys it
def Split.src(+u: Ui, +n: String, +path: String) -> String:
  n ++ "|" ++ Ui.root(u) ++ "|" ++ path

def Split.src.sch(+u: Ui) -> String:
  Split.src(u, "1", Split.sheet(u))

def Split.src.pcb(+u: Ui) -> String:
  Split.src(u, "0", Ui.view_get(u, "view.path"))

def Split.src.solid(+u: Ui) -> String:
  Split.src(u, "2", Ui.view_get(u, "view.path"))

# the sheet the schematic's navigation moves: the combined view's, else the
# viewer's file
def Sch.path(+u: Ui) -> String:
  Bool.pick(String, Split.is(u), Split.sheet(u), Ui.view_get(u, "view.path"))

# a sheet put on show: in the combined view its pane's, else the
# Schematic tab's
def Sch.put(split: Bool, +u: Ui, +file: String) -> Ui:
  match split:
    case True{}:
      Ui.scratch_set(Ui.info_set(u, "view.sheet:" ++ Ui.sel(u), HI.Hist.family(file)), Split.key(u), file)
    case False{}:
      Ui.view_put(u, "schematic", file)

# Sheet navigation
# ----------------
# A hierarchical schematic's sheets (hier.bend's rows, info
# "sheets:<root>"; each file's child sheet boxes, "sheetbox:<root>"),
# navigated by instance on every client. Per thread: the row on show as
# "@sch:<t>" ("<row>\t<place>": the place wins when the rows change, the
# row tells apart two siblings of one name), the rows shown before and
# after as "@sch.back:<t>"/"@sch.fwd:<t>" (newest first, a line each, at
# most 32), a branch folded as "@sch.fold:<t>:<place>", and "@sch.note:<t>"
# "gone" when a sheet asked for is no longer there (nothing else changes).
# "@sch.tree" "0" puts the tree away. Actions: "sch-go" ("<row>\t<place>
# \t<uuid path>", or less: a tree row, a crumb, a sheet here, a sheet
# symbol), "sch-up", "sch-root", "sch-back", "sch-fwd", "sch-fold" (a row's
# key: its uuid path, else its place), "sch-tree".
# An instance is its KiCad uuid path when the files give one (hier.bend's
# Hier.uids: stable across renames and moves), else its place by names.
# Each tree the hub sends names its slot (the live design, or a version's
# commit) and when it was asked for: a client keeps each root and slot
# apart and takes a slot's reply only when it is no older than the one it
# holds, so a late reply never replaces the hierarchy or the sheet on show
# (Sch.got). A version with no tree yet navigates the live one's, mapped
# into the version (Sch.value).

# a row as a client lists it: state 0 plain, 1 above the one on show (an
# ancestor), 2 on show; kids: rows under it; open: not folded
type SchRow is Data:
  SchRow{i: Nat, depth: Nat, name: String, value: String, state: Nat, kids: Bool, open: Bool, loop: Bool}

# what a client draws: the tree's visible rows, the trail from the root to
# the row on show, the sheets under it, its row (count when the file on
# show is not in the hierarchy: out), its place, whether Back and Forward
# have somewhere to go, whether the tree shows, the sheet symbols' boxes
# for the canvas ("<row>\t<place>\t<x0 y0 x1 y1>" lines) and the note
type SchView is Data:
  SchView{rows: List<&2, SchRow>, trail: List<&2, SchRow>, here: List<&2, SchRow>, at: Nat, count: Nat, place: String, back: Bool, fwd: Bool, out: Bool, tree: Bool, boxes: String, note: String, page: String}

def Sch.sfx(+slot: String) -> String:
  Bool.pick(String, String.is_empty(slot), "", "\t" ++ slot)

# the tree the view on show wants: the live design's, or a version's
def Sch.slot.of(+p: String) -> String:
  Bool.pick(String, Nat.is_eq(HI.Hist.kind(p), 1n), HI.Hist.git.dir(p) ++ "\t" ++ HI.Hist.git.commit(p), "")

def Sch.want(+u: Ui) -> String:
  Ui.root(u) ++ Sch.sfx(Sch.slot.of(Sch.path(u)))

def Sch.tk.k(none: Bool, +u: Ui) -> String:
  match none:
    case True{}:
      Ui.root(u)
    case False{}:
      Sch.want(u)

# the tree read: the one wanted once it came, else the live design's
def Sch.tk(+u: Ui) -> String:
  Sch.tk.k(String.is_empty(Ui.info(u, "sheets:" ++ Sch.want(u))), u)

def Sch.rows(+u: Ui) -> List<&2, HR.Row>:
  HR.Hier.read(Ui.info(u, "sheets:" ++ Sch.tk(u)))

def Sch.fam(+p: String) -> String:
  HI.Hist.family(p)

def Sch.ids.go(rs: List<&2, HR.Row>, +text: String) -> List<&2, List<&2, String>>:
  match rs:
    case Nil{}:
      Nil{}
    case HR.Row{_, _, g, _} <> t:
      HR.Hier.boxes.of(text, Sch.fam(g)) <> Sch.ids.go(t, text)

# each row's "<uuid path>\t<page>" (hier.bend's Hier.uids)
def Sch.us(+u: Ui, +rs: List<&2, HR.Row>) -> List<&2, String>:
  HR.Hier.uids(rs, Sch.ids.go(rs, Ui.info(u, "sheetid:" ++ Sch.tk(u))), Ui.info(u, "sheettop:" ++ Sch.tk(u)))

def Sch.uid(+us: List<&2, String>, +j: Nat) -> String:
  HI.Hist.field(HI.Hist.at(us, j), 0n)

def Sch.page(+us: List<&2, String>, +j: Nat) -> String:
  HI.Hist.field(HI.Hist.at(us, j), 1n)

def Sch.useek.go(us: List<&2, String>, +j: Nat, +id: String) -> Maybe<&2, Nat>:
  match us:
    case Nil{}:
      None{}
    case x <> t:
      HR.Hier.seek.k(String.eq(HI.Hist.field(x, 0n), id), j, Sch.useek.go(t, 1n+j, id))

# a row's key for its folds: its uuid path, else its place
def Sch.vkey(+v: String) -> String:
  Bool.pick(String, String.is_empty(HI.Hist.field(v, 2n)), HI.Hist.field(v, 1n), HI.Hist.field(v, 2n))

# the live file on show ("" is the project's own: the root's)
def Sch.file(+u: Ui, +rs: List<&2, HR.Row>) -> String:
  Sch.fam(Bool.pick(String, String.is_empty(Sch.path(u)), HR.Hier.file(rs, 0n), Sch.path(u)))

def Sch.num(+s: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(s), 0n)

def Sch.key(+u: Ui) -> String:
  "@sch:" ++ Ui.sel(u)

def Sch.back.key(+u: Ui) -> String:
  "@sch.back:" ++ Ui.sel(u)

def Sch.fwd.key(+u: Ui) -> String:
  "@sch.fwd:" ++ Ui.sel(u)

def Sch.note.key(+u: Ui) -> String:
  "@sch.note:" ++ Ui.sel(u)

def Sch.fold.key(+u: Ui, +p: String) -> String:
  "@sch.fold:" ++ Ui.sel(u) ++ ":" ++ p

def Sch.with.go(rs: List<&2, HR.Row>, +j: Nat, +f: String) -> Maybe<&2, Nat>:
  match rs:
    case Nil{}:
      None{}
    case HR.Row{_, _, g, _} <> t:
      HR.Hier.seek.k(String.eq(Sch.fam(g), f), j, Sch.with.go(t, 1n+j, f))

def Sch.find.k(keep: Bool, +rs: List<&2, HR.Row>, +i: Nat, +p: String) -> Maybe<&2, Nat>:
  match keep:
    case True{}:
      Some{i}
    case False{}:
      HR.Hier.seek(rs, p)

# by names: its row while that is still its place (or it names no place),
# else the first row at its place, else none
def Sch.find.p(+rs: List<&2, HR.Row>, +e: String) -> Maybe<&2, Nat>:
  Sch.find.k(Bool.or(String.is_empty(HI.Hist.field(e, 1n)), String.eq(HR.Hier.path(rs, Sch.num(HI.Hist.field(e, 0n))), HI.Hist.field(e, 1n))),
    rs, Sch.num(HI.Hist.field(e, 0n)), HI.Hist.field(e, 1n))

def Sch.find.v(m: Maybe<&2, Nat>, +rs: List<&2, HR.Row>, +e: String) -> Maybe<&2, Nat>:
  match m:
    case Some{+j}:
      Some{j}
    case None{}:
      Sch.find.p(rs, e)

def Sch.find.i(+us: List<&2, String>, +i: Nat, +id: String) -> Maybe<&2, Nat>:
  Bool.pick(Maybe<&2, Nat>, String.eq(Sch.uid(us, i), id), Some{i}, Sch.useek.go(us, 0n, id))

def Sch.find.u(none: Bool, +u: Ui, +rs: List<&2, HR.Row>, +e: String) -> Maybe<&2, Nat>:
  match none:
    case True{}:
      Sch.find.p(rs, e)
    case False{}:
      Sch.find.v(Sch.find.i(Sch.us(u, rs), Sch.num(HI.Hist.field(e, 0n)), HI.Hist.field(e, 2n)), rs, e)

# an entry's row ("<row>\t<place>\t<uuid path>"): by its uuid path when it
# has one and a row still has it, else by its place, else none
def Sch.find(+u: Ui, +rs: List<&2, HR.Row>, +e: String) -> Maybe<&2, Nat>:
  Sch.find.u(String.is_empty(HI.Hist.field(e, 2n)), u, rs, e)

def Sch.entry(+u: Ui, +rs: List<&2, HR.Row>, +j: Nat) -> String:
  Nat.show(j) ++ "\t" ++ HR.Hier.path(rs, j) ++ "\t" ++ Sch.uid(Sch.us(u, rs), j)

def Sch.first(+rs: List<&2, HR.Row>, +file: String) -> Nat:
  Maybe.default(&2, Nat, Sch.with.go(rs, 0n, file), List.length(&2, HR.Row, rs))

def Sch.at.j(same: Bool, +j: Nat, +rs: List<&2, HR.Row>, +file: String) -> Nat:
  match same:
    case True{}:
      j
    case False{}:
      Sch.first(rs, file)

def Sch.at.k(m: Maybe<&2, Nat>, +rs: List<&2, HR.Row>, +file: String) -> Nat:
  match m:
    case Some{+j}:
      Sch.at.j(String.eq(Sch.fam(HR.Hier.file(rs, j)), file), j, rs, file)
    case None{}:
      Sch.first(rs, file)

# the row on show: the instance kept while it shows the file on show, else
# the first row of that file; the rows' count when the file is not in the
# hierarchy (an agent showed another schematic)
def Sch.at(+u: Ui, +rs: List<&2, HR.Row>) -> Nat:
  Sch.at.k(Sch.find(u, rs, Ui.scratch(u, Sch.key(u))), rs, Sch.file(u, rs))

def Sch.inside(+rs: List<&2, HR.Row>, +j: Nat) -> Bool:
  Nat.is_lt(j, List.length(&2, HR.Row, rs))

# History: a line per entry, newest first
def Sch.cap(+s: String) -> String:
  String.join(List.take(&2, String, String.split(s, '\n'), 31n), "\n")

def Sch.cons(+e: String, +s: String) -> String:
  Bool.pick(String, String.is_empty(s), e, e ++ "\n" ++ Sch.cap(s))

def Sch.head(+s: String) -> String:
  HI.Hist.at(String.split(s, '\n'), 0n)

def Sch.tail(+s: String) -> String:
  String.join(List.drop(&2, String, String.split(s, '\n'), 1n), "\n")

# the entry of the row on show put in front of a history (nothing when the
# file on show is not in the hierarchy)
def Sch.mark(+u: Ui, +rs: List<&2, HR.Row>, +at: Nat, +s: String) -> String:
  Bool.pick(String, Sch.inside(rs, at), Sch.cons(Sch.entry(u, rs, at), s), s)

# the file row j shows: the project's own ("") for the root of the live
# design (own: the tree is the project's, not a board's of several);
# while a version shows, that file in the same version (its own tree's
# rows, or the live tree's mapped in); else its file
def Sch.value(own: Bool, +cur: String, +rs: List<&2, HR.Row>, +j: Nat) -> String:
  Bool.pick(String, Bool.and(own, Bool.and(Nat.is_eq(j, 0n), Nat.is_eq(HI.Hist.kind(cur), 0n))), "",
    Bool.pick(String, Nat.is_eq(HI.Hist.kind(cur), 1n), HI.Hist.rebase(cur, Sch.fam(HR.Hier.file(rs, j))), HR.Hier.file(rs, j)))

# row j on show (a comparison keeps comparing that sheet: Rv.sheet), kept
# as the instance, the note cleared
def Sch.show(+u: Ui, +rs: List<&2, HR.Row>, +j: Nat) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Sch.put(Split.is(u), u, Rv.sheet(Sch.path(u),
    Sch.value(String.is_empty(Bd.tree(u, Ui.view_get(u, "view.path"))), Sch.path(u), rs, j))),
    Sch.key(u), Sch.entry(u, rs, j)), Sch.note.key(u), "")

def Sch.gone(+u: Ui) -> Ui:
  Ui.scratch_set(u, Sch.note.key(u), "gone")

def Sch.visit.k(moved: Bool, +u: Ui, +rs: List<&2, HR.Row>, +j: Nat) -> Ui:
  match moved:
    case True{}:
      Sch.show(Ui.scratch_set(Ui.scratch_set(u, Sch.back.key(u), Sch.mark(u, rs, Sch.at(u, rs), Ui.scratch(u, Sch.back.key(u)))), Sch.fwd.key(u), ""), rs, j)
    case False{}:
      Sch.show(u, rs, j)

# a new place: the one on show goes on Back, Forward is cleared (going to
# the sheet already on show changes no history)
def Sch.visit(+u: Ui, +rs: List<&2, HR.Row>, +j: Nat) -> Ui:
  Sch.visit.k(Bool.not(Nat.is_eq(j, Sch.at(u, rs))), u, rs, j)

def Sch.go.in(ok: Bool, +u: Ui, +rs: List<&2, HR.Row>, +j: Nat) -> Ui:
  match ok:
    case True{}:
      Sch.visit(u, rs, j)
    case False{}:
      Sch.gone(u)

def Sch.go.k(m: Maybe<&2, Nat>, +u: Ui, +rs: List<&2, HR.Row>) -> Ui:
  match m:
    case Some{+j}:
      Sch.go.in(Sch.inside(rs, j), u, rs, j)
    case None{}:
      Sch.gone(u)

# Back or Forward: the newest entry of one history comes on show and the
# one on show goes on the other; an entry no longer there is dropped with
# the note, and the sheet on show stays
def Sch.step.in(ok: Bool, +u: Ui, +rs: List<&2, HR.Row>, +j: Nat, +from: String, +to: String) -> Ui:
  match ok:
    case True{}:
      Sch.show(Ui.scratch_set(Ui.scratch_set(u, to, Sch.mark(u, rs, Sch.at(u, rs), Ui.scratch(u, to))), from, Sch.tail(Ui.scratch(u, from))), rs, j)
    case False{}:
      Sch.gone(Ui.scratch_set(u, from, Sch.tail(Ui.scratch(u, from))))

def Sch.step.k(m: Maybe<&2, Nat>, +u: Ui, +rs: List<&2, HR.Row>, +from: String, +to: String) -> Ui:
  match m:
    case Some{+j}:
      Sch.step.in(Sch.inside(rs, j), u, rs, j, from, to)
    case None{}:
      Sch.step.in(False{}, u, rs, 0n, from, to)

def Sch.step.e(none: Bool, +u: Ui, +rs: List<&2, HR.Row>, +from: String, +to: String) -> Ui:
  match none:
    case True{}:
      u
    case False{}:
      Sch.step.k(Sch.find(u, rs, Sch.head(Ui.scratch(u, from))), u, rs, from, to)

def Sch.step(+u: Ui, +rs: List<&2, HR.Row>, +from: String, +to: String) -> Ui:
  Sch.step.e(String.is_empty(Ui.scratch(u, from)), u, rs, from, to)

def Sch.root(+u: Ui, +rs: List<&2, HR.Row>) -> Ui:
  Sch.go.in(Sch.inside(rs, 0n), u, rs, 0n)

def Sch.up.in(top: Bool, +u: Ui, +rs: List<&2, HR.Row>, +at: Nat) -> Ui:
  match top:
    case True{}:
      u
    case False{}:
      Sch.visit(u, rs, HR.Hier.parent(rs, at))

# Up: the parent of the row on show; at the root nothing; from a file
# outside the hierarchy, the root
def Sch.up.k(inside: Bool, +u: Ui, +rs: List<&2, HR.Row>, +at: Nat) -> Ui:
  match inside:
    case True{}:
      Sch.up.in(Nat.is_eq(at, 0n), u, rs, at)
    case False{}:
      Sch.root(u, rs)

def Sch.up(+u: Ui, +rs: List<&2, HR.Row>) -> Ui:
  Sch.up.k(Sch.inside(rs, Sch.at(u, rs)), u, rs, Sch.at(u, rs))

def Sch.shut(+u: Ui, +p: String) -> Bool:
  String.eq(Ui.scratch(u, Sch.fold.key(u, p)), "1")

def Sch.fold(+u: Ui, +p: String) -> Ui:
  Ui.scratch_set(u, Sch.fold.key(u, p), Bool.pick(String, Sch.shut(u, p), "", "1"))

def Sch.tree.on(+u: Ui) -> Bool:
  Bool.not(String.eq(Ui.scratch(u, "@sch.tree"), "0"))

def Sch.tree.flip(+u: Ui) -> Ui:
  Ui.scratch_set(u, "@sch.tree", Bool.pick(String, Sch.tree.on(u), "0", ""))

def Sch.act.k(i: Nat, +u: Ui, +rs: List<&2, HR.Row>, +value: String) -> Ui:
  match i:
    case 0n:
      Sch.go.k(Sch.find(u, rs, value), u, rs)
    case 1n:
      Sch.up(u, rs)
    case 2n:
      Sch.root(u, rs)
    case 3n:
      Sch.step(u, rs, Sch.back.key(u), Sch.fwd.key(u))
    case 4n:
      Sch.step(u, rs, Sch.fwd.key(u), Sch.back.key(u))
    case 5n:
      Sch.fold(u, value)
    case 6n:
      Sch.tree.flip(u)
    case _:
      u

def Sch.actions() -> List<&2, String>:
  ["sch-go", "sch-up", "sch-root", "sch-back", "sch-fwd", "sch-fold", "sch-tree"]

def Sch.act(+u: Ui, +action: String, +value: String) -> Act:
  Act{Sch.act.k(C.Codec.pos(Sch.actions(), action), u, Sch.rows(u), value), Nil{}}

# "view-sheet": "#<row>" a row (the phones' menu), else a file (the
# instance kept is then the first row of that file)
def Sch.sheet(row: Bool, +u: Ui, +value: String) -> Act:
  match row:
    case True{}:
      Act{Sch.act.k(0n, u, Sch.rows(u), String.drop(value, 1n)), Nil{}}
    case False{}:
      Act{Ui.scratch_set(Sch.put(Split.is(u), u, Rv.sheet(Sch.path(u), value)), Sch.key(u), ""), Nil{}}

# The view
def Sch.has(xs: List<&2, Nat>, +n: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(Nat.is_eq(x, n), Sch.has(t, n))

def Sch.state(+chain: List<&2, Nat>, +cur: Nat, +j: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(j, cur), 2n, Bool.pick(Nat, Sch.has(chain, j), 1n, 0n))

def Sch.kids(+rs: List<&2, HR.Row>, +j: Nat, +e: Nat) -> Bool:
  Nat.is_eq(HR.Hier.depth(rs, 1n+j), 1n+e)

# folded, unless it is above the row on show (that one always shows)
def Sch.closed(+u: Ui, +p: String, +st: Nat) -> Bool:
  Bool.and(Sch.shut(u, p), Bool.not(Nat.is_eq(st, 1n)))

# h: 0, or one past the depth of the folded row whose subtree is hidden
def Sch.vis(+h: Nat, +e: Nat) -> Bool:
  Bool.or(Nat.is_eq(h, 0n), Nat.is_lt(e, h))

def Sch.hide(vis: Bool, +h: Nat, +e: Nat, +kids: Bool, +closed: Bool) -> Nat:
  match vis:
    case True{}:
      Bool.pick(Nat, Bool.and(kids, closed), 1n+e, 0n)
    case False{}:
      h

def Sch.keep(vis: Bool, r: SchRow, rest: List<&2, SchRow>) -> List<&2, SchRow>:
  match vis:
    case True{}:
      r <> rest
    case False{}:
      rest

def Sch.row(+j: Nat, +e: Nat, n: String, l: Bool, +v: String, +st: Nat, +kids: Bool, +closed: Bool) -> SchRow:
  SchRow{j, e, n, v, st, kids, Bool.not(closed), l}

# a row's value: "<row>\t<place>\t<uuid path>"
def Sch.val(+ps: List<&2, String>, +us: List<&2, String>, +j: Nat) -> String:
  Nat.show(j) ++ "\t" ++ HI.Hist.at(ps, j) ++ "\t" ++ Sch.uid(us, j)

def Sch.rows.go(rs: List<&2, HR.Row>, +all: List<&2, HR.Row>, +ps: List<&2, String>, +us: List<&2, String>, +j: Nat, +h: Nat, +u: Ui, +chain: List<&2, Nat>, +cur: Nat) -> List<&2, SchRow>:
  match rs:
    case Nil{}:
      Nil{}
    case HR.Row{+e, n, _, l} <> t:
      Sch.keep(Sch.vis(h, e), Sch.row(j, e, n, l, Sch.val(ps, us, j), Sch.state(chain, cur, j), Sch.kids(all, j, e), Sch.closed(u, Sch.vkey(Sch.val(ps, us, j)), Sch.state(chain, cur, j))),
        Sch.rows.go(t, all, ps, us, 1n+j, Sch.hide(Sch.vis(h, e), h, e, Sch.kids(all, j, e), Sch.closed(u, Sch.vkey(Sch.val(ps, us, j)), Sch.state(chain, cur, j))), u, chain, cur))

def Sch.loop.k(m: Maybe<&2, HR.Row>) -> Bool:
  match m:
    case Some{HR.Row{_, _, _, l}}:
      l
    case None{}:
      False{}

def Sch.trail.go(cs: List<&2, Nat>, +rs: List<&2, HR.Row>, +ps: List<&2, String>, +us: List<&2, String>, +cur: Nat) -> List<&2, SchRow>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      SchRow{c, HR.Hier.depth(rs, c), HR.Hier.name(rs, c), Sch.val(ps, us, c), Bool.pick(Nat, Nat.is_eq(c, cur), 2n, 1n), False{}, True{}, False{}}
        <> Sch.trail.go(t, rs, ps, us, cur)

def Sch.here.go(js: List<&2, Nat>, +rs: List<&2, HR.Row>, +ps: List<&2, String>, +us: List<&2, String>) -> List<&2, SchRow>:
  match js:
    case Nil{}:
      Nil{}
    case +j <> t:
      SchRow{j, HR.Hier.depth(rs, j), HR.Hier.name(rs, j), Sch.val(ps, us, j), 0n, Sch.kids(rs, j, HR.Hier.depth(rs, j)), True{}, Sch.loop.k(HR.Hier.row(rs, j))}
        <> Sch.here.go(t, rs, ps, us)

# "<row>\t<place>\t<uuid path>\t<x0 y0 x1 y1>" per sheet symbol
def Sch.box.go(js: List<&2, Nat>, +k: Nat, +bs: List<&2, String>, +ps: List<&2, String>, +us: List<&2, String>) -> List<&2, String>:
  match js:
    case Nil{}:
      Nil{}
    case +j <> t:
      (Sch.val(ps, us, j) ++ "\t" ++ HI.Hist.at(bs, k)) <> Sch.box.go(t, 1n+k, bs, ps, us)

def Sch.view.at(+cur: Nat, +ps: List<&2, String>, +us: List<&2, String>, +rs: List<&2, HR.Row>, +u: Ui) -> SchView:
  SchView{Sch.rows.go(rs, rs, ps, us, 0n, 0n, u, HR.Hier.chain(rs, cur), cur),
    Sch.trail.go(HR.Hier.chain(rs, cur), rs, ps, us, cur),
    Sch.here.go(HR.Hier.under(rs, cur), rs, ps, us),
    cur, List.length(&2, HR.Row, rs), HI.Hist.at(ps, cur),
    Bool.not(String.is_empty(Ui.scratch(u, Sch.back.key(u)))), Bool.not(String.is_empty(Ui.scratch(u, Sch.fwd.key(u)))),
    Bool.not(Sch.inside(rs, cur)), Sch.tree.on(u),
    String.join(Sch.box.go(HR.Hier.under(rs, cur), 0n, HR.Hier.boxes.of(Ui.info(u, "sheetbox:" ++ Sch.tk(u)), Sch.fam(HR.Hier.file(rs, cur))), ps, us), "\n"),
    Ui.scratch(u, Sch.note.key(u)), Sch.page(us, cur)}

def Sch.view.k(+rs: List<&2, HR.Row>, +u: Ui) -> SchView:
  Sch.view.at(Sch.at(u, rs), HR.Hier.paths(rs), Sch.us(u, rs), rs, u)

# the schematic's navigation as the client shows it
def Sch.view(+u: Ui) -> SchView:
  Sch.view.k(Sch.rows(u), u)

# whether there is a hierarchy to navigate (more than one sheet)
def Sch.many(+u: Ui) -> Bool:
  Nat.is_gt(List.length(&2, HR.Row, Sch.rows(u)), 1n)

def Sch.boxes.v(v: SchView) -> String:
  match v:
    case SchView{_, _, _, _, _, _, _, _, _, _, b, _, _}:
      b

# back on the Board or Schematic tab while a comparison of that kind is
# open (the Review tab between): it shows again as it was (Old, Diff or
# New); else the project's own file ("")
def Rv.back.k(same: Bool, +u: Ui) -> String:
  match same:
    case True{}:
      +bits = Maybe.default(&2, U32, U32.read(Ui.view_get(u, "cmp.bits")), 0)
      Bool.pick(String, String.eq(Ui.view_get(u, "cmp.as"), "old"), Ui.view_get(u, "cmp.a"),
        Bool.pick(String, String.eq(Ui.view_get(u, "cmp.as"), "new"), Ui.view_get(u, "cmp.b"), HI.Hist.diff.on(Ui.view_get(u, "cmp.a"), Ui.view_get(u, "cmp.b"), bits)))
    case False{}:
      ""

def Rv.back(+u: Ui, +kind: String) -> String:
  Rv.back.k(Bool.and(Bool.not(String.is_empty(Ui.view_get(u, "cmp.a"))),
    Bool.and(Bool.or(String.eq(kind, "board"), String.eq(kind, "schematic")),
      Nat.is_eq(HI.Hist.ext(HI.Hist.family(Ui.view_get(u, "cmp.f"))), Bool.pick(Nat, String.eq(kind, "schematic"), 1n, 0n)))), u)

# the window's tab picked: the same, keeping a comparison (Rv.back)
def Ui.view_pick.p(u: Ui, +kind: String, +path: String) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, "@revert", ""), live, op, on, now, ad, pa, v, rid, w, er, note, Viewer.put(nf, sel, kind, path), ob, tr}

def Ui.view_pick.back(+u: Ui, +kind: String) -> Ui:
  Ui.view_pick.p(u, kind, Rv.back(u, kind))

# the window's tab picked: a comparison kept (Rv.back), else the board picked
def Ui.view_pick.bd(+u: Ui, +kind: String) -> Ui:
  Ui.view_pick.p(u, kind, Bd.or(Rv.back(u, kind), Bd.keep(u, kind, Bd.shown(u, Bd.context(u)))))

def Bd.kind(+k: String) -> String:
  Bool.pick(String, Bool.or(Bool.or(String.eq(k, "schematic"), String.eq(k, "split")), String.eq(k, "3d")), k, "board")

def Bd.picked(+u: Ui, +rel: String) -> Ui:
  +board = FM.Fm.abs(Ui.root(u), rel)
  +tree = FM.Fm.abs(Ui.root(u), Bd.path("schematic", rel))
  +changed = Bool.not(String.eq(Ui.view_get(u, "view.design"), board))
  +picked = Ui.info_set(Ui.info_set(u, "view.design:" ++ Ui.sel(u), board), "view.tree:" ++ Ui.sel(u), tree)
  Bool.pick(Ui, changed, Ui.scratch_set(Ui.info_set(picked, "view.sheet:" ++ Ui.sel(u), tree), Split.key(u), tree), picked)

def Bd.pick(+u: Ui, +rel: String) -> Act:
  Act{Ui.view_put(Bd.picked(u, rel), Bd.kind(Ui.view_get(u, "view.kind")), FM.Fm.abs(Ui.root(u), Bd.path(Bd.kind(Ui.view_get(u, "view.kind")), rel))), Nil{}}

# a viewer tab's file: a comparison kept (Rv.back), else the board picked
# (Bd.keep); the Schematic tab from the combined view shows its sheet
def Bd.sheet.keep(+u: Ui, +kind: String, +path: String) -> String:
  Bool.pick(String, Bool.and(String.eq(kind, "schematic"), Bd.sheet.allowed(u, path)), Ui.view_get(u, "view.sheet"), path)

def View.kind.path(+u: Ui, +value: String) -> String:
  Bd.or(Rv.back(u, value), Bool.pick(String, Bool.and(Split.is(u), String.eq(value, "schematic")), Split.sheet(u),
    Bd.sheet.keep(u, value, Bd.keep(u, value, Bd.shown(u, Bd.context(u))))))

# into the combined view from the Schematic tab: its sheet comes along
def Split.enter(sch: Bool, +u: Ui) -> Ui:
  match sch:
    case True{}:
      Ui.scratch_set(u, Split.key(u), Ui.view_get(u, "view.path"))
    case False{}:
      u

# "mech" opens Mech (the part in 3D), "mech-part" shows another part,
# "mech-3d" a part in 3D, "mech-refresh" asks for the parts again,
# "mech-renders" shows a part's pictures, "mech-render" asks the hub to
# draw them, "view-kind" shows the project's own board, schematic or 3D
# model (a web tab), "view-sheet" a sheet of its schematic (its file; ""
# the root), and "view-layer" turns a layer of the board or schematic on
# or off (its index)
def Mpage.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Act{Mpage.open(u), Nil{}}
    case 1n:
      Act{Mpage.part.k(Mpage.onpart(u), u, value), Nil{}}
    case 2n:
      Act{Ui.view_put(u, "3d", value), Nil{}}
    case 3n:
      Mpage.ask(u, Ui.sel(u))
    case 4n:
      Act{Ui.view_put(u, "mech", value), Nil{}}
    case 5n:
      Ui.rpc(Ui.scratch_set(Ui.scratch_set(u, "@mech.busy:" ++ Ui.sel(u), value), "@mech.err:" ++ Ui.sel(u), ""), "mech.render",
        [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"path", J.Str{value}}])
    case 6n:
      Act{Ui.view_put(Split.enter(Bool.and(String.eq(value, "split"), String.eq(Ui.view_get(u, "view.kind"), "schematic")), u), value, View.kind.path(u, value)), Nil{}}
    case 7n:
      Sch.sheet(String.starts_with(value, "#"), u, value)
    case 8n:
      Act{Ui.scratch_set(u, Solid.off.key(u), U32.show(U32.xor(Solid.off(u), U32.shln(1, U32.to_nat(Maybe.default(&2, U32, U32.read(value), 31)))))), Nil{}}
    # (past the table: an action no one takes)
    case _:
      Act{u, Nil{}}

# The page asks for its parts once for each thread and change of its view
# ("@mech.at": the thread and view.n last asked for): opened by hand or
# by an agent (viewer_show, kind mech), or a thread switched to that is on
# it. Every client runs this after each action and message.
def Mpage.at(+u: Ui) -> String:
  Ui.sel(u) ++ "#" ++ Ui.view_get(u, "view.n")

def Mpage.sync.k(want: Bool, +u: Ui, +at: String, cmds: List<&2, Cmd>) -> Act:
  match want:
    case True{}:
      Ui.also(Mpage.ask(Ui.scratch_set(u, "@mech.at", at), Ui.sel(u)), cmds)
    case False{}:
      Act{u, cmds}

# (on: the page is on screen; the phones keep their viewer apart)
def Mpage.sync.if(on: Bool, a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Mpage.sync.k(Bool.and(on, Bool.not(String.eq(Mpage.at(u), Ui.scratch(u, "@mech.at")))), u, Mpage.at(u), cmds)

# Viewers for a thread elsewhere (core/fview.bend)
# ------------------------------------------------
# The hub this client is on keeps the thread's files in a cache the viewers
# read, brought from the owning hub by far.view. The client asks, naming
# the file on screen and the digest of the list it last got; the answer
# comes when the owner's files change (or after about half a minute, "same"),
# and the next ask follows it, so the cache follows the owner with no timer
# here. Kept per thread in the info map: fv.root (the cache folder),
# fv.have (the digest), fv.sent ("w:" and the file asked for), fv.fly ("1"
# while an ask is out) and fv.at (when), fv.ready (the path that was in the
# cache when the last answer came), fv.err (what could not be reached).
def Fv.now(u: Ui) -> Nat:
  match u:
    case Ui{_, _, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}:
      now

# the viewer needs the cache: a thread elsewhere with its panel open on the
# board, the schematic, 3D, an image, a PDF, or the Files tab
def Fv.on(+u: Ui) -> Bool:
  Bool.and(Bool.and(FR.Fr.is(Ui.sel(u)), Ui.panel(u)),
    Bool.or(Bool.or(String.is_empty(Fv.t(u, "view.kind")), Fv.kind(Fv.t(u, "view.kind"))), String.eq(Fv.t(u, "view.kind"), "files")))

# the file on screen, from the owner's folder
def Fv.want(+u: Ui) -> String:
  Bool.pick(String, Fv.kind(Fv.t(u, "view.kind")), Fv.rel.of(Fv.t(u, "fv.root"), Fv.t(u, "view.path")), "")

def Fv.stale(+u: Ui) -> Bool:
  Nat.is_gt(Fv.now(u), Nat.add(Maybe.default(&2, Nat, Nat.read(Fv.t(u, "fv.at")), 0n), 45n))

# an ask is due: none out, or one out for too long, or for another file; and
# not one that just failed for this file
def Fv.due(+u: Ui, +w: String) -> Bool:
  Bool.and(Bool.or(Bool.not(String.eq(Fv.t(u, "fv.fly"), "1")), Bool.or(Fv.stale(u), Bool.not(String.eq("w:" ++ w, Fv.t(u, "fv.sent"))))),
    Bool.or(Fv.stale(u), Bool.not(Bool.and(String.eq("w:" ++ w, Fv.t(u, "fv.err")), Bool.not(String.eq(Fv.t(u, "fv.fly"), "1"))))))

def Fv.mark(+u: Ui, +w: String) -> Ui:
  Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "fv.fly:" ++ Ui.sel(u), "1"), "fv.sent:" ++ Ui.sel(u), "w:" ++ w), "fv.at:" ++ Ui.sel(u), Nat.show(Fv.now(u))), "fv.err:" ++ Ui.sel(u), "")

# the digest to hold the answer on: the last one, when it was for this file
def Fv.have(+u: Ui, +w: String) -> String:
  Bool.pick(String, String.eq("w:" ++ w, Fv.t(u, "fv.sent")), Fv.t(u, "fv.have"), "")

def Fv.send(+u: Ui, +w: String, cmds: List<&2, Cmd>) -> Act:
  Ui.also(Ui.rpc(Fv.mark(u, w), "far.view", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"want", J.Str{w}}, J.KV{"have", J.Str{Fv.have(u, w)}}]), cmds)

def Fv.sync.k(go: Bool, +u: Ui, +w: String, cmds: List<&2, Cmd>) -> Act:
  match go:
    case True{}:
      Fv.send(u, w, cmds)
    case False{}:
      Act{u, cmds}

# a PDF that came: its first page is asked for
def Fv.pdf.k(go: Bool, +u: Ui, cmds: List<&2, Cmd>) -> Act:
  match go:
    case True{}:
      Ui.also(Pdf.ask(u, Fv.t(u, "view.path"), "1"), cmds)
    case False{}:
      Act{u, cmds}

# An answer of far.view: the folder, the digest, and the file on screen
# either in the cache or not on the owner (it shows then, as a file that is
# not there); one for a file no longer asked for is dropped.
def Fv.at(+u: Ui, +th: String, +k: String) -> String:
  Ui.info(u, k ++ ":" ++ th)

def Fv.put(+u: Ui, +th: String, +k: String, +v: String) -> Ui:
  Ui.info_set(u, k ++ ":" ++ th, v)

def Fv.got.fresh(+u: Ui, +th: String, +r: J.Json) -> Ui:
  Fv.put(Fv.put(Fv.put(Fv.put(Fv.put(u, th, "fv.fly", ""), th, "fv.root", J.Json.get_str(r, "root", "")), th, "fv.have", J.Json.get_str(r, "ver", "")),
    th, "fv.err", ""), th, "fv.ready", Bool.pick(String, J.Json.get_bool(r, "same", False{}), Fv.at(u, th, "fv.ready"), Fv.at(u, th, "view.path")))

def Fv.got.down(+u: Ui, +th: String, +r: J.Json) -> Ui:
  Fv.put(Fv.put(Fv.put(u, th, "fv.fly", ""), th, "fv.err", "w:" ++ J.Json.get_str(r, "want", "")), th, "fv.ready", Fv.at(u, th, "view.path"))

def Fv.got.k(down: Bool, +u: Ui, +th: String, +r: J.Json) -> Ui:
  match down:
    case True{}:
      Fv.got.down(u, th, r)
    case False{}:
      Fv.got.fresh(u, th, r)

def Fv.got(+u: Ui, +r: J.Json) -> Ui:
  Bool.pick(Ui, String.eq("w:" ++ J.Json.get_str(r, "want", ""), Fv.at(u, J.Json.get_str(r, "thread", ""), "fv.sent")),
    Fv.got.k(Bool.not(String.is_empty(J.Json.get_str(r, "down", ""))), u, J.Json.get_str(r, "thread", ""), r), u)

def Fv.reply(has: Bool, +u: Ui, +r: J.Json) -> Ui:
  match has:
    case True{}:
      Fv.got(u, r)
    case False{}:
      u

def Fv.pdf.due(+u: Ui) -> Bool:
  Bool.and(Bool.and(String.eq(Fv.t(u, "view.kind"), "pdf"), Bool.not(String.is_empty(Fv.t(u, "view.path")))),
    Bool.and(String.eq(Fv.t(u, "fv.ready"), Fv.t(u, "view.path")), String.is_empty(Fv.t(u, "pdf.page"))))

def Fv.pdf(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Fv.pdf.k(Fv.pdf.due(u), u, cmds)

# (w: the file the viewer shows, from the owner's folder; the phones pass
# their own viewer's, mobile/view.bend)
def Fv.sync.on(on: Bool, +w: String, a: Act) -> Act:
  match on a:
    case True{} Act{+u, cmds}:
      Fv.pdf(Fv.sync.k(Fv.due(u, w), u, w, cmds))
    case False{} _:
      a

# (only a thread elsewhere does anything here)
def Fv.sync.far(far: Bool, a: Act) -> Act:
  match far a:
    case True{} Act{+u, cmds}:
      Fv.sync.on(Fv.on(u), Fv.want(u), Act{u, cmds})
    case False{} _:
      a

# (a phone asks for its own viewer's file, mobile/view.bend's View.fv: on
# a phone ("local.fixed") this one stays out, so one file is asked for,
# not two in turn)
def Fv.sync(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Fv.sync.far(Bool.and(FR.Fr.is(Ui.sel(u)), Bool.not(String.eq(Ui.info(u, "local.fixed"), "1"))), Act{u, cmds})

def Mpage.sync.m(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Mpage.sync.if(Bool.or(String.eq(Ui.view_get(u, "view.kind"), "mech"), Mpage.onpart(u)), Act{u, cmds})

def Mpage.sync(a: Act) -> Act:
  Fv.sync(Mpage.sync.m(a))

# Diff asked for by hand (the rail, ctrl+d, the palette): its answer opens
# the viewer on it, even with one of Diff's own asks still out
def Dv.ask(+u: Ui) -> Act:
  Ui.on_thread.p(Ui.info_set(u, "diff.auto:" ++ Ui.sel(u), ""), "git.diff", [J.KV{"from", J.Num{"0"}}, J.KV{"to", J.Num{"-1"}}])

# An open Diff follows the thread's changes: when the selected thread's
# panel shows its Diff and the thread's turn or steps moved on from what
# the diff on show was asked at ("diff.at:t", Dv.at: a turn started or
# ended, a step changed a design file), it is asked again, quietly. Every
# client runs this after each action and message.
def Dv.sync.k(go: Bool, +u: Ui, +at: String, cmds: List<&2, Cmd>) -> Act:
  match go:
    case True{}:
      Ui.also(Ui.on_thread.p(Ui.info_set(Ui.info_set(u, "diff.at:" ++ Ui.sel(u), at), "diff.auto:" ++ Ui.sel(u), "1"), "git.diff",
        [J.KV{"from", J.Num{"0"}}, J.KV{"to", J.Num{"-1"}}]), cmds)
    case False{}:
      Act{u, cmds}

def Dv.sync.on(on: Bool, a: Act) -> Act:
  match on a:
    case True{} Act{+u, cmds}:
      Dv.sync.k(Bool.not(String.eq(Dv.at(Ui.state(u), Ui.sel(u)), Ui.info(u, "diff.at:" ++ Ui.sel(u)))), u, Dv.at(Ui.state(u), Ui.sel(u)), cmds)
    case False{} _:
      a

def Dv.sync(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Dv.sync.on(Bool.and(Ui.panel(u), String.eq(Ui.view_get(u, "view.kind"), "diff")), Act{u, cmds})

# Cross-probing
# -------------
# The part or net probed (the web's probe.js picks it from a click and
# lights it in every view) is kept per project and board as the setting
# "probe:<root>|<board>" (probe.js's text: "c\t<ref>" a part, "n\t<net>
# \t<pins>" a net), so it holds across the views, the viewer shut and a
# reload, and on every client of the hub. A part on another sheet than the
# one on show is named with its sheet and opens it with a click
# (Probe.away, the tree's parts per sheet, hier.bend's Hier.refs); in the
# combined view a part probed on the board or in 3D opens its sheet at
# once (laws probe_*).

# the board the probe is kept for: the one picked where a project has
# several, else the project's own ("")
def Probe.board(+u: Ui) -> String:
  Bool.pick(String, Bd.many(u), Bd.cur(Bd.files(u), Bd.shown(u, Ui.view_get(u, "view.path"))), "")

def Probe.key(+u: Ui) -> String:
  "probe:" ++ Ui.root(u) ++ "|" ++ Probe.board(u)

def Probe.get(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Ui.root(u)), "", M.Setting.get(Ui.state(u), Probe.key(u), ""))

# a part's ref ("" for a net, or none)
def Probe.ref(+v: String) -> String:
  Bool.pick(String, String.starts_with(v, "c\t"), HI.Hist.field(v, 1n), "")

def Probe.first(+s: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(s, ' ')), "")

# a net's name as the chip says it (KiCad's own for an unlabelled net,
# "Net-(R1-Pad2)", already says what it is)
def Probe.net(+n: String) -> String:
  Bool.pick(String, String.starts_with(n, "Net-"), n, "Net " ++ n)

# the chip's words: the part's ref, or "Net" and its name (one of its pins
# while the board names none)
def Probe.label(+v: String) -> String:
  Bool.pick(String, String.starts_with(v, "c\t"), HI.Hist.field(v, 1n),
    Bool.pick(String, String.starts_with(v, "n\t"), Probe.net(Bd.or(HI.Hist.field(v, 1n), Probe.first(HI.Hist.field(v, 2n)))), ""))

# the board's source a schematic's net is found on ("0|<root>|<board>",
# as the Board tab's)
def Probe.board.src(+u: Ui) -> String:
  "0|" ++ Ui.root(u) ++ "|" ++ Bd.keep(u, "board", Bd.shown(u, Ui.view_get(u, "view.path")))

def Probe.has(rs: List<&2, String>, +ref: String) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case r <> t:
      Bool.or(String.eq(r, ref), Probe.has(t, ref))

def Probe.row.k(hit: Bool, +j: Nat, rest: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match hit:
    case True{}:
      Some{j}
    case False{}:
      rest

# the first row whose file places the part (text: the tree's refs lines)
def Probe.row(rs: List<&2, HR.Row>, +j: Nat, +text: String, +ref: String) -> Maybe<&2, Nat>:
  match rs:
    case Nil{}:
      None{}
    case HR.Row{_, _, f, _} <> t:
      Probe.row.k(Probe.has(HR.Hier.boxes.of(text, Sch.fam(f)), ref), j, Probe.row(t, 1n+j, text, ref))

def Probe.where.k(none: Bool, +u: Ui, +ref: String) -> Maybe<&2, Nat>:
  match none:
    case True{}:
      None{}
    case False{}:
      Probe.row(Sch.rows(u), 0n, Ui.info(u, "sheetrefs:" ++ Sch.tk(u)), ref)

# the row of the sheet a part is on
def Probe.where(+u: Ui, +ref: String) -> Maybe<&2, Nat>:
  Probe.where.k(String.is_empty(ref), u, ref)

def Probe.away.k(m: Maybe<&2, Nat>, +at: Nat) -> Maybe<&2, Nat>:
  match m:
    case Some{+j}:
      Bool.pick(Maybe<&2, Nat>, Nat.is_eq(j, at), None{}, Some{j})
    case None{}:
      None{}

# the probed part's sheet when it is not the one on show
def Probe.away(+u: Ui) -> Maybe<&2, Nat>:
  Probe.away.k(Probe.where(u, Probe.ref(Probe.get(u))), Sch.at(u, Sch.rows(u)))

# what the schematic says of it: the part, its sheet's name, and the value
# "sch-go" opens it with ("" when it is on show, or on no sheet known)
type ProbeAway is Data:
  ProbeAway{ref: String, sheet: String, value: String}

def Probe.note.k(m: Maybe<&2, Nat>, +u: Ui) -> ProbeAway:
  match m:
    case Some{+j}:
      ProbeAway{Probe.ref(Probe.get(u)), HR.Hier.name(Sch.rows(u), j), Sch.entry(u, Sch.rows(u), j)}
    case None{}:
      ProbeAway{"", "", ""}

def Probe.note(+u: Ui) -> ProbeAway:
  Probe.note.k(Probe.away(u), u)

def Probe.go(m: Maybe<&2, Nat>, +u: Ui) -> Ui:
  match m:
    case Some{+j}:
      Sch.visit(u, Sch.rows(u), j)
    case None{}:
      u

# (in the combined view a part probed elsewhere opens its sheet)
def Probe.open(on: Bool, a: Act) -> Act:
  match on a:
    case True{} Act{+u, cmds}:
      Act{Probe.go(Probe.away(u), u), cmds}
    case False{} _:
      a

def Probe.set.k(same: Bool, +u: Ui, +v: String) -> Act:
  match same:
    case True{}:
      Probe.open(Split.is(u), Act{u, Nil{}})
    case False{}:
      Probe.open(Split.is(u), Ui.rpc(Ui.setting.here(u, [Probe.key(u), v]), "setting.set", [J.KV{"key", J.Str{Probe.key(u)}}, J.KV{"value", J.Str{v}}]))

# "probe": the probe a click made (probe.js's text; "" clears it), kept for
# the project and board, applied here at once (the same again is kept as
# it is, and in the combined view still opens its part's sheet)
def Probe.set(+u: Ui, +v: String) -> Act:
  Probe.set.k(String.eq(Probe.get(u), v), u, v)

# The web's viewer: the board, the schematic and a 3D model (the board's,
# or a part's) come as they come to the phones. The page asks the hub for
# the source on screen (kicad.watch) whenever it changes, and host.js
# draws the plots that come back under the source's key
# "<kind>|<root>|<path>" (0 board, 1 schematic, 2 3D; path "" for the
# project's own file). "" when the pane shows something else.

# the viewer's kind on the web: the thread's, the board before any
# (a file an agent showed with no kind is what its name says)
def Solid.kind(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Ui.view_get(u, "view.kind")),
    Bool.pick(String, Nat.is_eq(HI.Hist.ext(HI.Hist.family(Ui.view_get(u, "view.path"))), 1n), "schematic", "board"), Ui.view_get(u, "view.kind"))

def Sch.boxes.k(on: Bool, +u: Ui) -> String:
  match on:
    case True{}:
      Sch.boxes.v(Sch.view(u))
    case False{}:
      ""

# the sheet symbols a click on the schematic on show opens (none on a board)
def Sch.boxes(+u: Ui) -> String:
  Sch.boxes.k(Bool.and(String.eq(Solid.kind(u), "schematic"), Sch.many(u)), u)

def Sch.got.k(take: Bool, u: Ui, +k: String, +o: J.Json) -> Ui:
  match take:
    case True{}:
      Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "sheets:" ++ k, J.Json.get_str(o, "rows", "")), "sheetbox:" ++ k, J.Json.get_str(o, "boxes", "")),
        "sheetid:" ++ k, J.Json.get_str(o, "ids", "")), "sheettop:" ++ k, J.Json.get_str(o, "top", "")), "sheetgen:" ++ k, J.Json.get_str(o, "gen", "")),
        "sheetrefs:" ++ k, J.Json.get_str(o, "refs", ""))
    case False{}:
      u

# a reply is taken when its slot holds none yet, or its generation (the
# plot actor's request order, a decimal string) is greater than the one held
def Sch.newer(+gen: String, +held: String) -> Bool:
  Bool.or(String.is_empty(held), Nat.is_gt(Sch.num(gen), Sch.num(held)))

def Sch.got.at(+u: Ui, +k: String, +o: J.Json) -> Ui:
  Sch.got.k(Sch.newer(J.Json.get_str(o, "gen", ""), Ui.info(u, "sheetgen:" ++ k)), u, k, o)

# a schematic's sheets as the hub sends them ({"t":"sheets", root, slot,
# gen, rows, boxes, ids, top}), kept under its project root and slot, all
# together, and only when newer than the slot's last: a reply to an earlier
# request (even one asked in the same second) leaves the hierarchy and the
# sheet on show as they are, and a reply for another root or version never
# touches this one's
def Sch.got(+u: Ui, +o: J.Json) -> Ui:
  Sch.got.at(u, J.Json.get_str(o, "root", "") ++ Sch.sfx(J.Json.get_str(o, "slot", "")), o)

def Solid.num(+k: String) -> String:
  Bool.pick(String, String.eq(k, "board"), "0", Bool.pick(String, String.eq(k, "schematic"), "1", Bool.pick(String, String.eq(k, "3d"), "2",
    Bool.pick(String, String.eq(k, "split"), "s", ""))))

def Solid.ask(+k: String) -> String:
  k

# (the combined view's names its sheet and the panes shown too)
def Solid.want(+u: Ui) -> String:
  Bool.pick(String, Bool.or(Bool.or(Bool.not(Ui.panel(u)), String.is_empty(Ui.root(u))), String.is_empty(Solid.num(Solid.kind(u)))), "",
    Solid.num(Solid.kind(u)) ++ "|" ++ Ui.root(u) ++ "|" ++ Ui.view_get(u, "view.path")
      ++ Bool.pick(String, Split.is(u), "|" ++ Split.sheet(u) ++ "|" ++ Split.letters(u), ""))

# a source key's family (plot2d.js's data-fam: a key of the same family
# keeps the view and the last plot until the next comes): the live file
# its version or comparison is of, and for the project's own file (no
# path in the key) that file itself (its board, Bd.shown; the root sheet
# beside it), as a step's version names it, so stepping from the live
# board to a step keeps the board and fades the change in
# (test/hist_test.bend)
def Solid.fam.k(+fam: String, +kind: String, +root: String, +own: String) -> String:
  Bool.pick(String, Bool.or(Bool.not(String.eq(fam, kind ++ "|" ++ root ++ "|")), Bool.or(String.is_empty(own), String.is_empty(root))), fam,
    fam ++ Bool.pick(String, String.eq(kind, "1"), Bd.path("schematic", own), own))

def Solid.fam(+u: Ui, +key: String) -> String:
  Solid.fam.k(HI.Hist.key.family(key), HI.Hist.at(String.split(key, '|'), 0n), Ui.root(u), Bd.shown(u, ""))

# (tree: where the schematic's hierarchy starts, Bd.tree; sheet: the
# combined view's, whose kind names the panes it shows, "split:sb3")
def Solid.sync.k(same: Bool, u: Ui, +want: String, +kind: String, +path: String, +root: String, +tree: String, +sheet: String, cmds: List<&2, Cmd>) -> Act:
  match same:
    case True{}:
      Act{u, cmds}
    case False{}:
      Ui.also(Ui.rpc(Ui.scratch_set(u, "@solid.at", want), "kicad.watch",
        [J.KV{"kind", J.Str{Bool.pick(String, String.is_empty(want), "", Solid.ask(kind))}}, J.KV{"root", J.Str{root}}, J.KV{"path", J.Str{path}}, J.KV{"tree", J.Str{tree}},
         J.KV{"sheet", J.Str{sheet}}]), cmds)

def Solid.sync.m(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Solid.sync.k(String.eq(Solid.want(u), Ui.scratch(u, "@solid.at")), u, Solid.want(u), Bool.pick(String, Split.is(u), "split:" ++ Split.letters(u), Solid.kind(u)),
        Ui.view_get(u, "view.path"), Ui.root(u), Bd.tree(u, Ui.view_get(u, "view.path")), Split.sheet(u), cmds)

def Solid.sync(a: Act) -> Act:
  Fv.sync(Solid.sync.m(a))

# A reload keeps each thread's viewer: whether it is open, what it shows
# (its kind and file; a design-history step or a comparison comes back as
# the live file it is of) and the schematic's sheet (the Schematic tab's
# and the combined view's). The page keeps the selected thread's as
# Viewer.keep packs it (host.js's "backplane-viewer:<t>"), and puts each
# back before its first view (Viewer.restore); the hub's view of a fresh
# connection starts from them (thin-actor.bend's Actor.safe_input)
# (test/hist_test.bend)
def Viewer.live(+path: String) -> String:
  Bool.pick(String, Nat.is_eq(HI.Hist.kind(path), 0n), path, "")

def Viewer.record(+u: Ui, +th: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"open", J.Str{Bool.pick(String, String.eq(Ui.sel(u), th), Viewer.flag(Ui.panel(u)), Ui.info(u, "panel:" ++ th))}},
    J.KV{"thread", J.Str{th}}, J.KV{"root", J.Str{Ui.root.here(Ui.state(u), th)}}, J.KV{"kind", J.Str{Ui.info(u, "view.kind:" ++ th)}},
    J.KV{"path", J.Str{Viewer.live(Ui.info(u, "view.path:" ++ th))}},
    J.KV{"design", J.Str{Viewer.live(Ui.info(u, "view.design:" ++ th))}}, J.KV{"tree", J.Str{Viewer.live(Ui.info(u, "view.tree:" ++ th))}}, J.KV{"drawingSheet", J.Str{Viewer.live(Ui.info(u, "view.sheet:" ++ th))}},
    J.KV{"sheet", J.Str{Ui.scratch(u, "@sch:" ++ th)}}, J.KV{"split", J.Str{Ui.scratch(u, "@split.sheet:" ++ th)}}]))

def Viewer.keep(+u: Ui) -> String:
  Viewer.record(u, Ui.sel(u))

def Viewer.restore.k(some: Bool, +u: Ui, +th: String, +o: J.Json) -> Ui:
  match some:
    case True{}:
      +kept = Ui.info_set(Ui.info_set(Ui.scratch_set(Ui.scratch_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "panel:" ++ th, Viewer.flag(String.eq(J.Json.get_str(o, "open", ""), "1"))),
        "view.kind:" ++ th, J.Json.get_str(o, "kind", "")), "view.path:" ++ th, Viewer.live(J.Json.get_str(o, "path", ""))),
        "@sch:" ++ th, J.Json.get_str(o, "sheet", "")), "@split.sheet:" ++ th, J.Json.get_str(o, "split", "")),
        "view.design:" ++ th, Viewer.live(J.Json.get_str(o, "design", Bool.pick(String, Viewer.design(J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "path", "")), J.Json.get_str(o, "path", ""), "")))),
        "view.sheet:" ++ th, Viewer.live(J.Json.get_str(o, "drawingSheet", Bool.pick(String, String.eq(J.Json.get_str(o, "kind", ""), "schematic"), J.Json.get_str(o, "path", ""), J.Json.get_str(o, "split", "")))))
      Ui.info_set(kept, "view.tree:" ++ th, Viewer.live(J.Json.get_str(o, "tree", "")))
    case False{}:
      u

# (a thread's record put back; one that does not parse changes nothing)
def Viewer.restore(+u: Ui, +th: String, +text: String) -> Ui:
  +o = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
  Viewer.restore.k(Bool.and(Bool.not(String.is_empty(th)), Maybe.is_some(&2, J.Json, J.Json.get(o, "open"))), u, th, o)

# A fork takes a snapshot at the press, including a row menu's source.
# A failed or superseded creation never applies that snapshot elsewhere.
def Ui.fork.with(+u: Ui, +value: String) -> Act:
  Ui.rpc(Ui.scratch_set(Made.want(u), "@want.viewer", Viewer.record(u, Ui.fork.of(u, value))), "thread.fork", [J.KV{"thread", J.Str{Ui.fork.of(u, value)}}])

def Ui.fork(u: Ui) -> Act:
  Ui.fork.with(u, "")

def Viewer.fork.path(+path: String, +old: String, +root: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(root)), Bool.and(Bool.not(String.is_empty(old)), String.starts_with(path, old ++ "/"))), root ++ String.drop(path, String.length(old)), path)

def Viewer.fork(+u: Ui, +th: String, +o: J.Json) -> Ui:
  +old = J.Json.get_str(o, "root", "")
  +root = Ui.root.here(Ui.state(u), th)
  Ui.scratch_set(Viewer.restore.k(Maybe.is_some(&2, J.Json, J.Json.get(o, "open")), u, th,
    J.Json.obj([J.KV{"open", J.Str{J.Json.get_str(o, "open", "")}}, J.KV{"kind", J.Str{J.Json.get_str(o, "kind", "")}},
      J.KV{"path", J.Str{Viewer.fork.path(J.Json.get_str(o, "path", ""), old, root)}},
      J.KV{"design", J.Str{Viewer.fork.path(J.Json.get_str(o, "design", ""), old, root)}},
      J.KV{"tree", J.Str{Viewer.fork.path(J.Json.get_str(o, "tree", ""), old, root)}},
      J.KV{"drawingSheet", J.Str{Viewer.fork.path(J.Json.get_str(o, "drawingSheet", ""), old, root)}},
      J.KV{"sheet", J.Str{J.Json.get_str(o, "sheet", "")}},
      J.KV{"split", J.Str{Viewer.fork.path(J.Json.get_str(o, "split", ""), old, root)}}])), "@viewer.fork:" ++ th, J.Json.get_str(o, "thread", ""))

# what the page says under its parts: where to get FreeCAD when the hub
# has none, else nothing
def Mpage.note.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, Bool.not(String.is_empty(Ui.info(u, "freecad"))), "", MSG.T.mech.nofreecad(l))

def Mpage.note(+u: Ui) -> String:
  Mpage.note.loc(Lang.loc(u), u)

def Ui.error_set(u: Ui, e: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, _, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, e, note, nf, ob, tr}

def Ui.attach.k(big: Bool, first: Bool, +u: Ui, +o: J.Json) -> Act:
  match big first:
    case True{} True{}:
      Act{Ui.error_set(u, "attachments are limited to " ++ Nat.show(Nat.div(AT.Att.cap(), 1048576n)) ++ " MB"), Nil{}}
    case True{} False{}:
      Act{u, Nil{}}
    case False{} _:
      Ui.on_thread.p(Bool.pick(Ui, first, Ui.up_set(u, Ui.sel(u), J.Json.get_str(o, "name", "file")), u), "attach.put",
        [J.KV{"key", J.Str{J.Json.get_str(o, "key", "u")}}, J.KV{"name", J.Str{J.Json.get_str(o, "name", "file")}},
         J.KV{"i", J.Num{Nat.show(J.Json.get_nat(o, "i", 0n))}}, J.KV{"last", J.Flag{J.Json.get_bool(o, "last", True{})}},
         J.KV{"data", J.Str{J.Json.get_str(o, "data", "")}}])

# a piece of a file the page read ({key, name, size, i, last, data} with
# data base64): sent to the selected thread's attachments; a file over the
# cap is refused here, before any of it is sent
def Ui.attach.o(+u: Ui, +o: J.Json) -> Act:
  Ui.attach.k(Nat.is_gt(J.Json.get_nat(o, "size", 0n), AT.Att.cap()), Nat.is_eq(J.Json.get_nat(o, "i", 0n), 0n), u, o)

def Ui.attach(+u: Ui, v: String) -> Act:
  Ui.attach.o(u, Maybe.default(&2, J.Json, J.Json.parse(v), J.Null{}))

# files are dragged over the window ("1") or have left it (""); while
# they hover, the composer shows where they drop
def Ui.drop_hover.k(on: Bool, u: Ui) -> Ui:
  match on:
    case True{}:
      Ui.flag_on(u, "drop")
    case False{}:
      Ui.flag_off(u, "drop")

def Ui.drop_hover(u: Ui, +v: String) -> Ui:
  Ui.drop_hover.k(Bool.not(String.is_empty(v)), u)

# a file on the machine the hub runs on (the native window), copied in
def Ui.attach_file(+u: Ui, +path: String) -> Act:
  Ui.on_thread.p(Ui.up_set(u, Ui.sel(u), AT.Att.base(AT.Att.from_uri(path))), "attach.file", [J.KV{"path", J.Str{path}}])

def Ui.detach(+u: Ui, +path: String) -> Act:
  Act{Ui.atts_set(u, Ui.sel(u), Att.drop(Ui.atts(u, Ui.sel(u)), path)), Nil{}}

# "120x30" -> (120, 30), else the default screen
def Term.size.k(ps: List<&2, String>) -> U32 & U32:
  match ps:
    case +c <> +r <> _:
      (U32.from_nat(Maybe.default(&2, Nat, Nat.read(c), 100n)), U32.from_nat(Maybe.default(&2, Nat, Nat.read(r), 24n)))
    case _:
      (100, 24)

# DOM key names and their X keysyms (what VT.Vt.key reads)
def Keys.dom.names() -> List<&2, String>:
  ["Enter", "Backspace", "Tab", "Escape", "ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown", "Home", "End", "PageUp", "PageDown", "Delete", "Insert",
   "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"]

def Keys.dom.syms() -> List<&2, U32>:
  [65293, 65288, 65289, 65307, 65361, 65362, 65363, 65364, 65360, 65367, 65365, 65366, 65535, 65379,
   65470, 65471, 65472, 65473, 65474, 65475, 65476, 65477, 65478, 65479, 65480, 65481]

def Keys.at(xs: List<&2, U32>, i: Nat) -> U32:
  match xs i:
    case x <> _ 0n:
      x
    case _ <> t 1n+j:
      Keys.at(t, j)
    case Nil{} _:
      0

def Keys.char.k(one: Bool, +cp: U32) -> U32 & U32:
  match one:
    case True{}:
      (Bool.pick(U32, U32.is_lt(cp, 256), cp, (cp + 16777216 : U32)), cp)
    case False{}:
      (0, 0)

def Keys.dom.k(named: Bool, +i: Nat, +k: String) -> U32 & U32:
  match named:
    case True{}:
      (Keys.at(Keys.dom.syms(), i), 0)
    case False{}:
      Keys.char.k(Nat.is_eq(String.length(k), 1n), Char.to_u32(Maybe.default(&2, Char, List.head(&2, Char, String.to_list(k)), ' ')))

# a DOM key name as (keysym, the code point it types); (0, 0) for keys a
# terminal ignores (Shift alone, Meta, ...)
def Keys.dom(+k: String) -> U32 & U32:
  Keys.dom.k(Nat.is_lt(C.Codec.pos(Keys.dom.names(), k), List.length(&2, String, Keys.dom.names())), C.Codec.pos(Keys.dom.names(), k), k)

# the shortcuts a page takes for itself (the rest reach the focused field;
# in the terminal only ctrl+j, so ctrl+d still ends a shell's input)
def Ui.keys(term: Bool) -> List<&2, String>:
  match term:
    case True{}:
      ["ctrl+j"]
    case False{}:
      ["ctrl+j", "ctrl+d"]

def Term.key.k(ks: U32 & U32, +u: Ui, +mods: U32) -> Act:
  (+sym, +cp) = ks
  Bool.pick(Act, U32.is_eq(sym, 0), Act{u, Nil{}}, Ui.term_keys(u, VT.Vt.key(Ui.term(u, Ui.sel(u)), sym, cp, mods)))

def Term.key.p(ps: List<&2, String>, +u: Ui) -> Act:
  match ps:
    case +k <> +m <> _:
      Term.key.k(Keys.dom(k), u, U32.from_nat(Maybe.default(&2, Nat, Nat.read(m), 0n)))
    case _:
      Act{u, Nil{}}

# a key typed into the web page's terminal: "<DOM key>\t<mods>" (mods: 1
# shift, 4 ctrl, 8 alt, as xterm counts them)
def Ui.term_key(+u: Ui, v: String) -> Act:
  Term.key.p(String.split(v, '\t'), u)

def Ui.term_paste(+u: Ui, text: String) -> Act:
  Ui.term_keys(u, VT.Vt.paste(Ui.term(u, Ui.sel(u)), text))

def Term.toggle.k(open: Bool, +u: Ui, +c: U32, +r: U32) -> Act:
  match open:
    case True{}:
      Act{Ui.flag_off(u, "term"), [Focus{"composer"}]}
    case False{}:
      Ui.also(Ui.term_open(Ui.flag_on(u, "term"), U32.max(c, 20), U32.max(r, 5)), [Focus{"term"}])

def Term.toggle.s(sz: U32 & U32, open: Bool, +u: Ui) -> Act:
  (+c, +r) = sz
  Term.toggle.k(open, u, c, r)

# open or close the selected thread's terminal (the web page's ctrl+j and
# button; value "<cols>x<rows>" is the size it has room for)
def Ui.term_toggle(+u: Ui, +size: String) -> Act:
  Term.toggle.s(Term.size.k(String.split(size, 'x')), Ui.flag(u, "term"), u)

def Ui.key.k(i: Nat, +u: Ui, +size: String) -> Act:
  match i:
    case 0n:
      Ui.term_toggle(u, size)
    case 1n:
      Dv.ask(u)
    case _:
      Act{u, Nil{}}

def Ui.key.p(ps: List<&2, String>, +u: Ui) -> Act:
  match ps:
    case +k <> +sz <> _:
      Ui.key.k(C.Codec.pos(["ctrl+j", "ctrl+d"], k), u, sz)
    case +k <> Nil{}:
      Ui.key.k(C.Codec.pos(["ctrl+j", "ctrl+d"], k), u, "")
    case Nil{}:
      Act{u, Nil{}}

# a page shortcut: "<combo>\t<terminal size>"
def Ui.key(+u: Ui, v: String) -> Act:
  Ui.key.p(String.split(v, '\t'), u)

# Deleting a thread
# -----------------
# Deleting asks first: "delete" arms it (the header asks to confirm),
# "delete" again deletes the thread and goes back to the start, and
# "delete-no" disarms it. Only the selected thread can be armed, so
# choosing another one disarms it too.

def Ui.deleting(+u: Ui) -> Bool:
  Bool.and(Bool.not(String.is_empty(Ui.sel(u))), String.eq(Ui.scratch(u, "@del"), Ui.sel(u)))

def Ui.unselect(a: Act) -> Act:
  match a:
    case Act{Ui{st, _, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, cs}:
      Act{Ui{st, "", dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, cs}

def Ui.delete.k(armed: Bool, +u: Ui) -> Act:
  match armed:
    case True{}:
      Ui.unselect(Ui.on_thread(Ui.scratch_set(u, "@del", ""), "thread.delete"))
    case False{}:
      Act{Ui.scratch_set(u, "@del", Ui.sel(u)), Nil{}}

def Ui.delete(+u: Ui) -> Act:
  Ui.delete.k(Ui.deleting(u), u)

# Revert asks before replacing the working copy, just as Delete asks
# before removing a thread. The web's Revert ("ask": the diff's button, the
# palette) arms it for the thread on show; its own Revert ("confirm") sends
# it. A turn number is a revert already confirmed: the phones' native
# dialogs ask first and send "0", so it goes at once, as it always did.

def Ui.revert.entry(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case e <> _: M.Idx.id(e)

def Ui.revert.at(+u: Ui) -> String:
  C.Turn.name(M.Thread.turn(Ui.thread(Ui.state.at(u), Ui.sel(u)))) ++ "|" ++ Ui.revert.entry(M.State.mine(Ui.state.at(u), Ui.sel(u)))

# A finished turn can return to the same state; it must never rearm an old ask.
def Ui.revert.see.k(changed: Bool, u: Ui) -> Ui:
  match changed:
    case False{}: u
    case True{}: Ui.scratch_set(u, "@revert", "")

def Ui.revert.see(c: M.Change, +u: Ui) -> Ui:
  match c:
    case M.TurnChanged{thread, _, _}: Ui.revert.see.k(String.eq(thread, Ui.scratch(u, "@revert")), u)
    case _: u

def Ui.reverting.k(armed: Bool, +u: Ui) -> Bool:
  match armed:
    case False{}: False{}
    case True{}:
      Bool.and(Ui.panel(u), Bool.and(String.eq(Ui.view_get(u, "view.kind"), "diff"),
        String.eq(Ui.scratch(u, "@revert.at"), Ui.revert.at(u))))

def Ui.reverting(+u: Ui) -> Bool:
  Ui.reverting.k(Bool.and(Bool.not(String.is_empty(Ui.sel(u))), String.eq(Ui.scratch(u, "@revert"), Ui.sel(u))), u)

def Ui.revert.confirm(armed: Bool, +u: Ui) -> Act:
  match armed:
    case True{}:
      Ui.on_thread.p(Ui.scratch_set(u, "@revert", ""), "git.revert", [J.KV{"turn", J.Num{Ui.scratch(u, "@revert.turn")}}])
    case False{}:
      Act{u, Nil{}}

def Ui.revert.k(i: Nat, +u: Ui, +turn: String) -> Act:
  match i:
    case 0n:
      Ui.revert.confirm(Ui.reverting(u), u)
    case 1n:
      Act{Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.view_put(u, "diff", ""), "@revert", Ui.sel(u)), "@revert.turn", "0"), "@revert.at", Ui.revert.at(u)), Nil{}}
    case _:
      Ui.on_thread.p(Ui.scratch_set(u, "@revert", ""), "git.revert", [J.KV{"turn", J.Num{Bool.pick(String, String.is_empty(turn), "0", turn)}}])

def Ui.revert(+u: Ui, +turn: String) -> Act:
  Ui.revert.k(C.Codec.pos(["confirm", "ask"], turn), u, turn)

# Acting on a thread's row
# ------------------------
# The phone's swipes act on a thread without selecting it: the value is
# its id ("id|1h" for a snooze). Deleting from a row asks first the same
# way: "row-delete id" arms "@del" with that id, the same again deletes
# it, and "delete-no" disarms.

def Ui.on_row(u: Ui, +id: String, m: String) -> Act:
  Ui.rpc(u, m, [J.KV{"thread", J.Str{id}}])

# a right-click on a sidebar row settles it, or brings a settled one back
def Ui.row_toggle.m(sh: M.Shelf) -> String:
  match sh:
    case M.Settled{}:
      "thread.unsettle"
    case _:
      "thread.settle"

def Ui.row_toggle(+u: Ui, +id: String) -> Act:
  Ui.on_row(u, id, Ui.row_toggle.m(M.Shelf.of(Ui.thread(Ui.state(u), id))))

def Ui.row_snooze.k(u: Ui, s: Sect) -> Act:
  match s:
    case Sect{id, t}:
      Ui.rpc(u, "thread.snooze", [J.KV{"thread", J.Str{id}}, J.KV{"for", J.Str{Side.first(t)}}])

def Ui.row_snooze(u: Ui, v: String) -> Act:
  Ui.row_snooze.k(u, Side.pair(String.split(v, '|')))

def Ui.unselect.if(on: Bool, a: Act) -> Act:
  match on:
    case True{}:
      Ui.unselect(a)
    case False{}:
      a

def Ui.row_delete.k(armed: Bool, +u: Ui, +id: String) -> Act:
  match armed:
    case True{}:
      Ui.unselect.if(String.eq(id, Ui.sel(u)), Ui.on_row(Ui.scratch_set(u, "@del", ""), id, "thread.delete"))
    case False{}:
      Act{Ui.scratch_set(u, "@del", id), Nil{}}

# the thread a row asks to delete ("" when none)
def Ui.row_deleting(+u: Ui) -> String:
  Ui.scratch(u, "@del")

def Ui.row_delete(+u: Ui, +id: String) -> Act:
  Ui.row_delete.k(Bool.and(Bool.not(String.is_empty(id)), String.eq(Ui.row_deleting(u), id)), u, id)

# The queue above the composer
# ----------------------------

# where a queued message sits (0: next)
def Qu.index(qs: List<&2, M.QItem>, +msg: String) -> Nat:
  match qs:
    case Nil{}:
      0n
    case M.QItem{+m, _} <> t:
      Bool.pick(Nat, String.eq(m, msg), 0n, 1n+Qu.index(t, msg))

type QBtn is Data:
  QBtn{label: String, action: String, value: String}

def Qu.first(+l: I.Loc, +first: Bool, +run: Bool) -> List<&2, QBtn>:
  Bool.pick(List<&2, QBtn>, Bool.and(first, Bool.not(run)), [QBtn{MSG.T.queue.resume(l), "q-resume", ""}], Nil{})

# a queued row's buttons, left to right: while the turn runs, Steer sends
# it into the turn; while none runs (the queue is held, after a stop or a
# restart), Send starts it alone and the first row can resume them all
# (laws queue_row_*)
def Qu.buttons.loc(+l: I.Loc, +run: Bool, +first: Bool, +msg: String) -> List<&2, QBtn>:
  List.append(&2, QBtn, Qu.first(l, first, run),
    [QBtn{Bool.pick(String, run, MSG.T.composer.steer(l), MSG.T.composer.send(l)), "q-steer", msg}, QBtn{MSG.T.queue.edit(l), "q-edit", msg}, QBtn{MSG.T.queue.up(l), "q-up", msg}, QBtn{MSG.T.queue.remove(l), "q-remove", msg}])

def Qu.buttons(run: Bool, first: Bool, msg: String) -> List<&2, QBtn>:
  Qu.buttons.loc(I.En{}, run, first, msg)

# a queued row as shown: handed-over context (a fork's start, a fork
# brought back, a provider switch) can only be removed; it goes with the
# next message
def Qu.row_buttons(+l: I.Loc, +run: Bool, +first: Bool, +msg: String) -> List<&2, QBtn>:
  Bool.pick(List<&2, QBtn>, String.starts_with(msg, "handoff:"), [QBtn{MSG.T.queue.remove(l), "q-remove", msg}], Qu.buttons.loc(l, run, first, msg))

def Qu.tag(+l: I.Loc, +run: Bool, +msg: String) -> String:
  Bool.pick(String, String.starts_with(msg, "handoff:"), MSG.T.queue.context(l), Bool.pick(String, run, MSG.T.queue.queued(l), MSG.T.queue.held(l)))

# a queued row's text: context says what it is, a message itself
def Qu.label(+l: I.Loc, +msg: String, +text: String) -> String:
  Bool.pick(String, String.starts_with(msg, "handoff:"), MSG.T.queue.handoff(l, DF.Diff.first(String.drop(text, 12n))), RTO.Rto.text(text))

# a thread's todo list as it shows (TD.Todo.shown): empty when hidden
def Todos.of(+st: M.State, +th: String) -> List<&2, TD.Todo>:
  TD.Todo.read(M.Setting.get(st, "todos." ++ th, ""))

def Todos.pick(show: Bool, ts: List<&2, TD.Todo>) -> List<&2, TD.Todo>:
  match show:
    case True{}:
      ts
    case False{}:
      Nil{}

def Todos.shown(+st: M.State, +th: String, +run: Bool) -> List<&2, TD.Todo>:
  Todos.pick(Bool.and(M.Todos.visible(st, th), TD.Todo.shown(Todos.of(st, th), run)), Todos.of(st, th))

# The list sits at the top of the thread beside its subagents, on all four
# clients, as one line ("To-do 3/7"). Its two controls: that line opens or
# shuts it (the "fold" action, key todos:<thread>), and a list too long for
# it ends in a line that shows every step, or the fitted few again (key
# todos.all:<thread>). A client that scrolls the list shows every step; the
# window, which cannot, as many as cap leaves room for.
def Todos.key(+th: String) -> String:
  "todos:" ++ th

def Todos.all.key(+th: String) -> String:
  "todos.all:" ++ th

# Shut until the person opens it: only their click (a stored "1") opens
# it, never a new list, a turn starting or ending, or a thread switch (laws
# todos_*). It was open until shut before; that default was never stored,
# so a stored "1" is always the person's own click (shut, then opened).
#
# The open choice carries the hub's clear token (M.Todos.gen): the head's
# click stores "1:<token>" (Ui.tool_fold), and the list is open only while
# that token is still the thread's. A clear (completed or retired) changes
# the token, so the list shuts with no write: adopting a whole state, the
# thin lane's renders and the client's fold all agree. A bare "1" (stored
# before tokens) counts only while the token is "".
def Todos.open.v(+user: String, +gen: String) -> Bool:
  Bool.or(String.eq(user, "1:" ++ gen), Bool.and(String.eq(user, "1"), String.is_empty(gen)))

def Todos.open(+info: Map<&2, String>, +st: M.State, +th: String) -> Bool:
  Todos.open.v(FO.Fold.user(info, Todos.key(th)), M.Todos.gen(st, th))

def Todos.all(+info: Map<&2, String>, +th: String) -> Bool:
  FO.Fold.open(FO.Fold.user(info, Todos.all.key(th)), False{})

# what a click on the head asks, and on the last line
def Todos.toggle(+open: Bool, +th: String) -> String:
  FO.Fold.ask(open, Todos.key(th))

def Todos.more.ask(+all: Bool, +th: String) -> String:
  FO.Fold.ask(all, Todos.all.key(th))

# the last line's words: "Show all 24" or "Show fewer", "" when the list fits
def Todos.more(+l: I.Loc, +all: Bool, +ts: List<&2, TD.Todo>) -> String:
  Bool.pick(String, Nat.is_gt(List.length(&2, TD.Todo, ts), TD.Todo.fit()), Bool.pick(String, all, MSG.T.todos.fewer(l), MSG.T.todos.all(l, Nat.show(List.length(&2, TD.Todo, ts)))), "")

def Todos.rows.all(+cap: Nat, +ts: List<&2, TD.Todo>) -> List<&2, TD.TRow>:
  TD.Todo.rows.k(Nat.is_gt(List.length(&2, TD.Todo, ts), cap), ts, List.length(&2, TD.Todo, ts), cap, TD.Todo.open(ts))

def Todos.rows.k(open: Bool, all: Bool, +cap: Nat, +ts: List<&2, TD.Todo>) -> List<&2, TD.TRow>:
  match open all:
    case False{} _:
      Nil{}
    case True{} True{}:
      Todos.rows.all(cap, ts)
    case True{} False{}:
      TD.Todo.rows(ts)

# the card's rows: none shut, every step (up to cap) shown all, else the fitted few
def Todos.rows(+info: Map<&2, String>, +st: M.State, +th: String, +cap: Nat, +ts: List<&2, TD.Todo>) -> List<&2, TD.TRow>:
  Todos.rows.k(Todos.open(info, st, th), Todos.all(info, th), cap, ts)

# a client that scrolls the card shows every step
def Todos.cap.all() -> Nat:
  100000n

# how many queued rows show (the rest are counted on the last)
def Qu.shown() -> Nat:
  4n

# a queued message one place earlier
def Ui.q_up(+u: Ui, +msg: String) -> Act:
  Ui.on_thread.p(u, "queue.move", [J.KV{"msg", J.Str{msg}}, J.KV{"to", J.Num{Nat.show(Nat.sub(Qu.index(M.Queue.items(Ui.state(u), Ui.sel(u)), msg), 1n))}}])

def Qu.text(m: Maybe<&2, M.QItem>) -> String:
  match m:
    case Some{M.QItem{_, t}}:
      t
    case None{}:
      ""

# the draft with a queued message's text after it (a blank line between)
def Qu.merge(+draft: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(draft)), text, draft ++ "\n\n" ++ text)

def Ui.q_edit.k(a: Act, +msg: String) -> Act:
  match a:
    case Act{u2, cs}:
      Ui.also(Ui.on_thread.p(u2, "queue.remove", [J.KV{"msg", J.Str{msg}}]), cs)

# a queued message back in the composer to change (it leaves the queue;
# sending it again queues or steers it as any message)
def Ui.q_edit(+u: Ui, +msg: String) -> Act:
  Ui.q_edit.k(Ui.draft.set(u, Qu.merge(Ui.draft_text(u), Qu.text(M.Queue.find(M.Queue.items(Ui.state(u), Ui.sel(u)), msg)))), msg)

# Bots
# ----
# (docs/bots.md) The client keeps which bot, room or remote bot is shown in
# scratch keys: "@bot" (its thread is `sel`), "@room", "@remote", and the
# bot's tab "@btab". Form fields are scratch keys "@f.<name>" set by the
# "bfield" action ("<name>\u001f<text>"). A bot's thread gets the usual
# timeline, composer and model picker; what is sent there goes to the bot
# as "bots.say", which starts its conversation over (hop 0).

def Ui.state.of(u: Ui) -> M.State:
  match u:
    case Ui{st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      st

def Ui.sel.of(u: Ui) -> String:
  match u:
    case Ui{_, sel, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      sel

def Bots.of_thread(+st: M.State, +thread: String) -> Maybe<&2, BT.Bot>:
  BT.World.of_thread(M.State.bots(st), thread)

# A room's unsent post belongs to that room, rather than to a form that
# goes away when the person opens another view.
def Bots.field.key(+u: Ui, +k: String) -> String:
  Bool.pick(String, String.eq(k, "rpost"), "@room.draft:" ++ Ui.scratch(u, "@room"), "@f." ++ k)

def Bots.field(+u: Ui, +k: String) -> String:
  Ui.scratch(u, Bots.field.key(u, k))

def Bots.fields.clear.go(ks: List<&2, String>, u: Ui) -> Ui:
  match ks:
    case Nil{}:
      u
    case +k <> t:
      Bots.fields.clear.go(t, Ui.scratch_set(u, "@f." ++ k, ""))

def Bots.fields.clear(u: Ui, ks: List<&2, String>) -> Ui:
  Bots.fields.clear.go(ks, u)

def Bots.say.k(a: Act, +bot: String, text: String) -> Act:
  match a:
    case Act{+u, cs}:
      Ui.first(cs, Ui.also(Ui.rpc.keep.msg(Ui.scratch_set(Ui.scratch_set(u, Rpl.key(Ui.sel(u)), ""), Rpl.q.key(Ui.sel(u)), ""), "bots.say",
        List.append(&2, J.KV, [J.KV{"bot", J.Str{bot}}, J.KV{"text", J.Str{text}}], Rpl.kv(Ui.scratch(u, Rpl.key(Ui.sel(u)))))), [ScrollEnd{}]))

# (the clear goes to the host too: a bot's thread once kept its sent text)
def Bots.say(u: Ui, +bot: String, text: String) -> Act:
  Bots.say.k(Draft.sent(u), bot, text)

# The project card (core/order.bend)
# ----------------------------------
# Above a project thread every client shows the intent's goal (opened: the
# whole intent), Edit (the composer gets "/intent" and the text; sending
# it writes .backplane/intent.md) and "Buy it · from $X" (order.buy: a buy
# thread, gone to at once). The hub answers fab.look with the card, kept
# as info fab:<thread>; it is asked when a thread is chosen and whenever
# its turn ends (Fab.sync after every action and message; "@fab.at" is
# what was asked last).

# Settings' rows on every client: Ordering (order.enabled, on unless the
# person turns it off: off, no Buy it, nothing sent to the order server;
# order.bend's Order.enabled.of) and, with it on, the estimates sent or not
# (order.estimates)
def Fab.enabled(+st: M.State) -> Bool:
  OR.Order.enabled.of(st)

def Fab.set.enabled(+st: M.State) -> String:
  Bool.pick(String, Fab.enabled(st), "on", "off")

def Fab.set.order.label() -> String:
  "Ordering"

def Fab.set.order.note() -> String:
  "Order boards and parts from Backplane. Off: no prices or uploads."

# Settings' Ordering page opens with how ordering works
def Fab.set.about() -> String:
  "With Ordering on, a project whose design is ready shows Buy it: in the Ready to order bar above the composer, and with the thread's tools (on a computer, at the foot of the tool rail beside the conversation). Buy it opens a buy thread. Its agent checks that every board and part is ready, asks you anything missing (quantities, finishes, where to ship), exports the manufacturing files into the project's mfg/ folder and sends them to Backplane's order server, which measures them and prices the exact order: manufacturing (boards, assembly, components and parts, with Backplane's fee included), shipping, and duties and taxes. The quote shows in the thread, and nothing is charged until you press Buy now and pay on Stripe's checkout page. A quote holds for 3 days. Prices are estimates: if the real cost turns out higher, you are asked before anything more is charged and can have a full refund instead; if it is lower, the difference is refunded. Once paid, the thread shows the order's progress (placed, shipped, tracking). With Ordering off, nothing about your projects leaves this machine for ordering. Price estimates add a rough \"from $X\" to Buy it, worked out from each part's size and options without sending any files."

def Fab.set.label() -> String:
  "Price estimates"

def Fab.set.note.k(on: Bool) -> String:
  match on:
    case True{}:
      "Buy it shows a price from each part's size and options. No files are sent."
    case False{}:
      "Comes with Ordering."

def Fab.set.on(+st: M.State) -> String:
  Bool.pick(String, String.eq(M.Setting.get(st, "order.estimates", "on"), "off"), "off", "on")

def Fab.th(+u: Ui, +value: String) -> String:
  Bool.pick(String, String.is_empty(value), Ui.sel(u), value)

# the card's fields, kept apart when it arrives (Fab.reply) so no client
# parses it per frame: "fab.line:<t>" the goal (or a nudge), "fab.intent:<t>",
# "fab.buy:<t>" the button's words, "fab.shown:<t>" "1" when there is a card,
# and "fab.open:<t>" "1" while the person has it open
def Fab.line(+u: Ui) -> String:
  Ui.info(u, "fab.line:" ++ Ui.sel(u))

def Fab.intent(+u: Ui, +th: String) -> String:
  Ui.info(u, "fab.intent:" ++ th)

def Fab.open(+u: Ui) -> Bool:
  String.eq(Ui.info(u, "fab.open:" ++ Ui.sel(u)), "1")

def Fab.buying(+st: M.State, +th: String) -> Bool:
  Bool.not(String.is_empty(M.Setting.get(st, OR.Order.buy.key(th), "")))

# the Buy button's words ("" for none: ordering off, or a buy thread,
# which has none of its own; laws fab_buy_*)
def Fab.buy.k(off: Bool, +label: String) -> String:
  Bool.pick(String, off, "", label)

def Fab.buy.of(+st: M.State, +nf: Map<&2, String>, +th: String) -> String:
  Fab.buy.k(Bool.or(Fab.buying(st, th), Bool.not(Fab.enabled(st))), Str.get(nf, "fab.buy:" ++ th))

def Fab.buy(+u: Ui) -> String:
  Fab.buy.k(Bool.or(Fab.buying(Ui.state.of(u), Ui.sel(u)), Bool.not(Fab.enabled(Ui.state.of(u)))), Ui.info(u, "fab.buy:" ++ Ui.sel(u)))

def Fab.shown(+u: Ui) -> Bool:
  String.eq(Ui.info(u, "fab.shown:" ++ Ui.sel(u)), "1")

# the line above the message box once there is something to buy (with
# Buy it beside it): "Ready to order · LED board · Lamp shade", "" for none
# (no Buy it, or a buy thread); "fab.ready:<t>" is kept by Fab.reply
def Fab.turn(t: M.Thread) -> String:
  match t:
    case M.Thread{_, +project, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(project), "", C.Turn.name(turn))

# (and none while the agent works: it is ready once the turn is over)
def Fab.ready.k(+buy: String, +line: String, run: Bool) -> String:
  Bool.pick(String, Bool.or(run, String.is_empty(buy)), "", Bool.pick(String, String.is_empty(line), "Ready to order", line))

# the bar's button: Buy it with its price, or Buy again once ordered
def Fab.ready.btn.k(again: Bool, +buy: String) -> String:
  Bool.pick(String, Bool.or(Bool.not(again), String.is_empty(buy)), buy, "Buy again")

def Fab.ready.btn.of(+st: M.State, +nf: Map<&2, String>, +th: String) -> String:
  Fab.ready.btn.k(String.eq(Str.get(nf, "fab.again:" ++ th), "1"), Fab.buy.of(st, nf, th))

def Fab.ready.btn(+u: Ui) -> String:
  Fab.ready.btn.k(String.eq(Ui.info(u, "fab.again:" ++ Ui.sel(u)), "1"), Fab.buy(u))

# The bar can be put away (its ×) and comes back from a small Buy it in the
# thread's header: the setting fab.min.<project> "1", one per project and
# kept like any setting, so every device follows (laws fab_min_*)
def Fab.min.key(+project: String) -> String:
  "fab.min." ++ project

def Fab.project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, +project, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      project

def Fab.min.of(+st: M.State, +th: String) -> Bool:
  String.eq(M.Setting.get(st, Fab.min.key(Fab.project(Ui.thread(st, th))), ""), "1")

# the bar's line once put away: none
def Fab.ready.shown.k(min: Bool, +line: String) -> String:
  Bool.pick(String, min, "", line)

# the header's Buy it: while the bar is put away and has something to show
def Fab.mini.k(min: Bool, +line: String) -> Bool:
  Bool.and(min, Bool.not(String.is_empty(line)))

def Fab.mini.label(again: Bool) -> String:
  Bool.pick(String, again, "Ordered", "Buy it")

def Fab.mini.tools.k(show: Bool, again: Bool, +th: String) -> List<&2, TO.Tool>:
  match show:
    case True{}:
      [TO.Tool{"box", Fab.mini.label(again), "Show the Ready to order bar again", "fab-show", th, False{}, False{}, False{}}]
    case False{}:
      Nil{}

def Fab.ready.all(+u: Ui) -> String:
  Fab.ready.k(Fab.buy(u), Ui.info(u, "fab.ready:" ++ Ui.sel(u)), String.eq(Fab.turn(Ui.thread(Ui.state.of(u), Ui.sel(u))), "running"))

def Fab.ready(+u: Ui) -> String:
  Fab.ready.shown.k(Fab.min.of(Ui.state.of(u), Ui.sel(u)), Fab.ready.all(u))

# Buy it in the web's tool rail: the Ready to order bar's button, there
# whenever it has one (ordering on, not a buy thread), the bar shown, put
# away or waiting for the turn; its words and its price apart, so the rail
# can stack them ("Buy it" over "from $5.00"; "Buy again" alone)
def Fab.rail(+u: Ui) -> String:
  Fab.ready.btn(u)

def Fab.rail.word(+label: String) -> String:
  Bool.pick(String, String.starts_with(label, "Buy it · "), "Buy it", label)

def Fab.rail.price(+label: String) -> String:
  Bool.pick(String, String.starts_with(label, "Buy it · "), String.drop(label, 9n), "")

# the header's tools for the open thread ([] or its Buy it)
def Fab.mini.tools(+u: Ui) -> List<&2, TO.Tool>:
  Fab.mini.tools.k(Fab.mini.k(Fab.min.of(Ui.state.of(u), Ui.sel(u)), Fab.ready.all(u)), String.eq(Ui.info(u, "fab.again:" ++ Ui.sel(u)), "1"), Ui.sel(u))

# the same from the state and the info a window or phone frame has
def Fab.mini.tools.of(+st: M.State, +nf: Map<&2, String>, +th: String, run: Bool) -> List<&2, TO.Tool>:
  Fab.mini.tools.k(Fab.mini.k(Fab.min.of(st, th), Fab.ready.k(Fab.buy.of(st, nf, th), Str.get(nf, "fab.ready:" ++ th), run)), String.eq(Str.get(nf, "fab.again:" ++ th), "1"), th)

# (nothing for a thread with no project: no fab.min. without one)
def Fab.min.set.k(none: Bool, +u: Ui, +project: String, +v: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Ui.setting(u, Fab.min.key(project) ++ "=" ++ v)

def Fab.min.set(+u: Ui, +th: String, +v: String) -> Act:
  Fab.min.set.k(String.is_empty(Fab.project(Ui.thread(Ui.state.of(u), th))), u, Fab.project(Ui.thread(Ui.state.of(u), th)), v)

# what was last asked for: the thread and where its turn stands
def Fab.at(+u: Ui) -> String:
  Fab.set.enabled(Ui.state.of(u)) ++ Fab.set.on(Ui.state.of(u)) ++ "#" ++ Ui.sel(u) ++ "#" ++ Fab.turn(Ui.thread(Ui.state.of(u), Ui.sel(u)))

# The intent (core/order.bend's .backplane/intent.md) says what the project
# is for. The agent keeps it up to date, so it is not on show all the time:
# a project's menu has "Intent…" ("proj-intent <project>"), which opens the
# project's thread (Side.home: the selected one when it is in that project,
# else its top one) with the intent over it, looked up afresh ("@intent"
# names the thread; "intent-close" or picking another thread puts it away).
# Its Edit is the card's: the intent in the composer as "/intent ...",
# which Send writes (laws intent_*).
def Intent.look(+u: Ui) -> Act:
  Ui.also(Ui.rpc(Ui.scratch_set(u, "@fab.at", Fab.at(u)), "fab.look", [J.KV{"thread", J.Str{Ui.sel(u)}}]), [Focus{"intent"}])

# the intent is on show over the selected thread
def Intent.on(+u: Ui) -> Bool:
  Bool.and(Bool.not(String.is_empty(Ui.sel(u))), String.eq(Ui.scratch(u, "@intent"), Ui.sel(u)))

# what it shows: the intent (Markdown, less its empty headings,
# OR.Intent.shown); "" while it is looked up or when there is none
# (Intent.got tells which)
def Intent.text(+u: Ui) -> String:
  OR.Intent.shown(Fab.intent(u, Ui.sel(u)))

# the hub has answered for this thread (its card's line is never blank)
def Intent.got(+u: Ui) -> Bool:
  Bool.not(String.is_empty(Ui.info(u, "fab.line:" ++ Ui.sel(u))))

# (nothing for a project with no thread, or one on another machine, whose
# files are there; the Fab actions route before the menus' own, so the menu
# is shut here, and the thread's viewer comes with it as Ui.act's select's).
# The look goes with the move: the hub-drawn page has the answer applied in
# the thread it opens (server/thin-actor.bend's Actor.accept)
def Intent.open.k(none: Bool, +u: Ui, +th: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Intent.look(Ui.scratch_set(Ui.reselect(Ui.sel(u), Ui.select.nav(u, th)), "@intent", th))

def Intent.open(+u: Ui, +pid: String) -> Act:
  Intent.open.k(Bool.or(String.is_empty(Side.home(u, pid)), FR.Fr.is(pid)), Ui.scratch_set(u, "@menu", ""), Side.home(u, pid))

def Intent.close(+u: Ui) -> Act:
  Act{Ui.scratch_set(u, "@intent", ""), [Focus{"composer"}]}

# "Intent saved", once the hub has written it (Intent.send.go)
def Intent.saved(+u: Ui) -> Bool:
  String.eq(Ui.scratch(u, "@intent.saved"), "1")

# the note goes ("intent-saved-off": its time is up, or a click), unless a
# later Save waits for its own
def Intent.saved.off(+u: Ui, +id: String) -> Act:
  Act{Bool.pick(Ui, Bool.or(String.is_empty(id), String.eq(id, Ui.scratch(u, "@intent.saving"))), Ui.scratch_set(Ui.scratch_set(u, "@intent.saving", ""), "@intent.saved", ""), u), Nil{}}

# Edit borrows the composer. Keep the conversation draft once, including
# when Edit is pressed again before Save.
def Intent.borrow.k(editing: Bool, +u: Ui, +th: String) -> Ui:
  match editing:
    case True{}: u
    case False{}:
      Ui.scratch_set(Ui.scratch_set(u, "@intent.draft:" ++ th, Ui.draft_text(u)), "@intent.edit:" ++ th, "1")

def Intent.borrow(+u: Ui, +th: String) -> Ui:
  Intent.borrow.k(Bool.and(String.eq(Ui.scratch(u, "@intent.edit:" ++ th), "1"), OR.Intent.is(Ui.draft_text(u))), u, th)

# The page keeps the borrowed draft and the mark as drafts of their own
# (Keeps of "@intent.draft:<t>" and "@intent.edit:<t>"; thin-host.js's
# requestSession and thin-actor.bend's Actor.intent.input bring them to a
# new connection's actor), so Save after a reload or a reconnect still
# gives the conversation's draft back.
def Intent.kept(+u: Ui, +th: String) -> List<&2, Cmd>:
  [Keep{"@intent.draft:" ++ th, Ui.scratch(u, "@intent.draft:" ++ th)}, Keep{"@intent.edit:" ++ th, Ui.scratch(u, "@intent.edit:" ++ th)}, Focus{"composer"}]

def Intent.borrowed(+b: Ui, +text: String, +th: String) -> Act:
  Ui.also(Ui.draft(Ui.scratch_set(b, "@intent", ""), text), Intent.kept(b, th))

def Fab.buy.start(waiting: Bool, +u: Ui, +th: String) -> Act:
  match waiting:
    case True{}: Act{u, Nil{}}
    case False{}:
      Ui.rpc(Made.want(u), "order.buy", [J.KV{"thread", J.Str{th}}])

def Fab.act.k(i: Nat, +u: Ui, +th: String) -> Act:
  match i:
    # the card opened or shut
    case 0n:
      Act{Ui.info_set(u, "fab.open:" ++ th, Bool.pick(String, String.eq(Ui.info(u, "fab.open:" ++ th), "1"), "", "1")), Nil{}}
    # the intent into the composer, to change and send (the intent on show
    # goes away)
    case 1n:
      Intent.borrowed(Intent.borrow(u, th), OR.Intent.edit(Fab.intent(u, th)), th)
    # Buy it: the hub makes the buy thread and this client goes to it
    case 2n:
      Fab.buy.start(Bool.or(Bool.not(String.is_empty(Ui.scratch(u, "@want.new"))), Fab.buying(Ui.state.of(u), th)), u, th)
    # the bar's ×: put away for the project; the header's Buy it: back
    case 4n:
      Fab.min.set(u, th, "1")
    case 5n:
      Fab.min.set(u, th, "0")
    case 9n:
      Ui.rpc(u, "fab.look", [J.KV{"thread", J.Str{th}}])
    case _:
      Act{u, Nil{}}

def Fab.actions() -> List<&2, String>:
  ["fab-open", "fab-edit", "buy", "board-pick", "fab-hide", "fab-show", "proj-intent", "intent-close", "intent-saved-off", "fab-refresh"]

def Fab.is(+action: String) -> Bool:
  Nat.is_lt(C.Codec.pos(Fab.actions(), action), 10n)

def Fab.act.v(+i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 3n:
      Bd.pick(u, value)
    case 6n:
      Intent.open(u, value)
    case 7n:
      Intent.close(u)
    case 8n:
      Intent.saved.off(u, value)
    case _:
      Fab.act.k(i, u, Fab.th(u, value))

def Fab.act(+u: Ui, +action: String, +value: String) -> Act:
  Fab.act.v(C.Codec.pos(Fab.actions(), action), u, value)

# asked again when the turn moves on; while it runs only if this thread has
# no card yet (a thread opened mid-turn), since its files are changing;
# never for a thread elsewhere (its hub has the files)
def Fab.want(+u: Ui) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.not(String.is_empty(Ui.sel(u))), Bool.not(FR.Fr.is(Ui.sel(u)))), Bool.not(String.eq(Fab.at(u), Ui.scratch(u, "@fab.at")))),
    Bool.and(Bool.not(String.ends_with(Fab.at(u), "#")),
      Bool.or(Bool.not(String.ends_with(Fab.at(u), "#running")), String.is_empty(Ui.info(u, "fab.line:" ++ Ui.sel(u))))))

# (after the action's own commands: a setting it changed, Ordering's say,
# must reach the hub before the look it asks for)
def Fab.sync.k(want: Bool, +u: Ui, cmds: List<&2, Cmd>) -> Act:
  match want:
    case True{}:
      Ui.first(cmds, Ui.rpc(Ui.scratch_set(u, "@fab.at", Fab.at(u)), "fab.look", [J.KV{"thread", J.Str{Ui.sel(u)}}]))
    case False{}:
      Act{u, cmds}

def Fab.sync(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Fab.sync.k(Fab.want(u), u, cmds)

# the hub's card for a thread
def Fab.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "goal"), J.Null{})), "null"))

def Fab.keep(u: Ui, +th: String, +c: OR.Card) -> Ui:
  Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "fab.line:" ++ th, OR.Card.line(c)), "fab.intent:" ++ th, OR.Card.intent(c)),
    "fab.buy:" ++ th, OR.Card.action(c)), "fab.shown:" ++ th, Bool.pick(String, OR.Card.shown(c), "1", ""))

def Fab.reply.r(u: Ui, +th: String, +r: J.Json) -> Ui:
  Fab.keep(Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "fab.ready:" ++ th, OR.Card.ready.or(OR.Card.ready.j(r), J.Json.get_str(r, "order", ""))), "fab.boards:" ++ th, OR.Card.boards.j(r)),
    "fab.board:" ++ th, J.Json.get_str(r, "board_default", "")), "fab.again:" ++ th, Bool.pick(String, OR.Order.ordered(J.Json.get_str(r, "order", "")), "1", "")), th, OR.Card.of.j(r))

# the answer to the Save waited for: the intent is saved
def Intent.saved.got(+u: Ui, +o: J.Json) -> Ui:
  Bool.pick(Ui, Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@intent.saving"))), String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), Ui.scratch(u, "@intent.saving"))),
    Ui.scratch_set(u, "@intent.saved", "1"), u)

def Fab.reply(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Intent.saved.got(Fab.reply.r(u, J.Json.get_str(J.Json.get_obj(o, "result"), "thread", ""), J.Json.get_obj(o, "result")), o)
    case False{}:
      u

# "/intent <text>" sent: the project's intent is that text (the hub writes
# it and answers with the card). The request's id waits in "@intent.saving"
# for that answer, which says "Intent saved" (Intent.saved) until
# "intent-saved-off" with the same id comes, a few seconds after Save (a
# hub's answer brings no commands, so the action asks for it at once)
def Intent.saved.ms() -> Nat:
  4000n

def Intent.send.go(u: Ui, +text: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, +rid, w, er, note, nf, ob, tr}:
      Ui.also(Ui.rpc(Ui.scratch_set(Ui.scratch_set(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, "@intent.saving", Nat.show(rid)), "@intent.saved", ""),
        "fab.intent", [J.KV{"thread", J.Str{sel}}, J.KV{"text", J.Str{text}}]), [Later{Intent.saved.ms(), "intent-saved-off", Nat.show(rid)}])

def Intent.send.d(a: Act, +text: String) -> Act:
  match a:
    case Act{u, cs}:
      Ui.first(cs, Intent.send.go(u, text))

# Save returns the borrowed composer to its conversation, through the
# usual draft command so the page keeps the restored text too.
def Intent.return(+u: Ui) -> Act:
  Ui.also(Ui.draft(Ui.scratch_set(Ui.scratch_set(u, "@intent.draft:" ++ Ui.sel(u), ""), "@intent.edit:" ++ Ui.sel(u), ""), Ui.scratch(u, "@intent.draft:" ++ Ui.sel(u))),
    Draft.intent.forget(True{}, Ui.sel(u)))

def Intent.send(+u: Ui) -> Act:
  Intent.send.d(Intent.return(u), OR.Intent.body(Ui.draft_text(u)))

def Ui.send.b(empty: Bool, b: Maybe<&2, BT.Bot>, u: Ui, text: String, +mode: String) -> Act:
  match empty b:
    case True{} _:
      Act{u, Nil{}}
    case False{} Some{bot}:
      Bots.say(u, BT.Bot.id(bot), text)
    case False{} None{}:
      Ui.send.t(u, text, mode)

def Ui.send.k(empty: Bool, +u: Ui, text: String, +mode: String) -> Act:
  Ui.send.b(empty, Bots.of_thread(Ui.state.of(u), Ui.sel.of(u)), u, text, mode)

# the draft, then a line per attachment (attach.bend's format)
# the draft sent; alt: the other follow-up mode (ctrl+Enter)
def Ui.send.go(u: Ui, alt: Bool) -> Act:
  match u:
    case Ui{+st, +sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      +text = AT.Att.join(String.trim(Str.get(dr, sel)), Att.nonblank(String.lines(Str.get(dr, "@att:" ++ sel))))
      Ui.send.k(String.is_empty(String.trim(text)), Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, text,
        Follow.mode(M.Setting.get(st, "followup", ""), M.Thread.provider(Ui.thread(st, sel)), alt))

def Ui.send.btw(btw: Bool, +u: Ui, alt: Bool) -> Act:
  match btw:
    case True{}:
      Btw.ask(u, Btw.q(Ui.draft_text(u)))
    case False{}:
      Ui.send.go(u, alt)

def Give.ask.go(u: Ui, +text: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr},
        "bots.give", [J.KV{"thread", J.Str{sel}}, J.KV{"text", J.Str{text}}])

def Give.ask.d(a: Act, +text: String) -> Act:
  match a:
    case Act{u, cs}:
      Ui.first(cs, Give.ask.go(u, text))

def Give.ask(u: Ui, +text: String) -> Act:
  Give.ask.d(Draft.clear(u), text)

def Ui.send.give(give: Bool, +u: Ui, alt: Bool) -> Act:
  match give:
    case True{}:
      Give.ask(u, Ui.draft_text(u))
    case False{}:
      Ui.send.btw(Btw.is(Ui.draft_text(u)), u, alt)

def Ui.send.intent(intent: Bool, +u: Ui, alt: Bool) -> Act:
  match intent:
    case True{}:
      Intent.send(u)
    case False{}:
      Ui.send.give(Give.is(Ui.draft_text(u)), u, alt)

# "/secret [name]": the secret box opens in this chat (Sbox.*); the draft
# is cleared, nothing is sent
def Sbox.is(+draft: String) -> Bool:
  Bool.or(String.eq(String.trim(draft), "/secret"), Bool.or(String.starts_with(draft, "/secret "), String.starts_with(draft, "/secret\n")))

def Bots.sel.set(u: Ui, +th: String) -> Ui:
  match u:
    case Ui{st, _, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, th, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# (a bot still awaited, "@bot.want", gives way to what was chosen since)
def Bots.views(u: Ui, +bot: String, +room: String, +remote: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@bot", bot), "@room", room), "@remote", remote), "@del", ""), "@bot.want", ""), "@thread.want", "")

# a bot shown: its thread is selected, its tab kept
# the thread a bot's Chat tab shows: its own, but the uberbot's is where the
# conversation runs now (ubersync.bend's Ub.chat: the holder's, as mirrored)
def Bots.chat.of(+st: M.State, +id: String, +th: String) -> String:
  Bool.pick(String, UB.Uber.is(st, id), US.Ub.chat(st), th)

def Bots.select.k(m: Maybe<&2, BT.Bot>, +u: Ui) -> Act:
  match m:
    case Some{BT.Bot{+i, _, +th, _, _, _, _}}:
      Draft.stale.after(Act{Ui.flag_off(Bots.views(Bots.sel.set(u, Bots.chat.of(Ui.state.of(u), i, th)), i, "", ""), "settings"), [Focus{"composer"}, ScrollEnd{}]})
    case None{}:
      Act{u, Nil{}}

def Bots.select(+u: Ui, +id: String) -> Act:
  Bots.select.k(BT.World.find.id(BT.World.bot(M.State.bots(Ui.state.of(u)), id)), u)

# The host keeps this small navigation record across a reload.
def Bots.navigation(+o: J.Json) -> String:
  +dr = J.Json.get_obj(o, "drafts")
  J.Json.show(J.Json.obj([J.KV{"sel", J.Str{J.Json.get_str(o, "sel", "")}}, J.KV{"bot", J.Str{J.Json.get_str(dr, "@bot", "")}},
    J.KV{"room", J.Str{J.Json.get_str(dr, "@room", "")}}, J.KV{"remote", J.Str{J.Json.get_str(dr, "@remote", "")}}, J.KV{"tab", J.Str{J.Json.get_str(dr, "@btab", "")}}]))

def Bots.navigation.restore(+u: Ui, +o: J.Json) -> Ui:
  Ui.scratch_set(Bots.views(Ui.reselect(Ui.sel(u), Ui.select.nav(u, J.Json.get_str(o, "sel", ""))), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "room", ""), J.Json.get_str(o, "remote", "")), "@btab", J.Json.get_str(o, "tab", ""))

# Rooms seen: the setting room.seen.<room> is the time (seconds) the person
# last opened it; its unread posts are those after it that they did not
# write (a room never opened: those of the last day, not its whole past).
# Opening a room with some unread says so; one with none sends nothing.
def Rooms.seen(+u: Ui, +room: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(M.Setting.get(Side.st(u), "room.seen." ++ room, "")), Nat.sub(Side.now(u), 86400n))

def Rooms.unread(+u: Ui, +room: String) -> Nat:
  BT.World.unread(M.State.bots(Side.st(u)), room, Rooms.seen(u, room))

def Rooms.mark.go(a: Act, +id: String) -> Act:
  match a:
    case Act{+u, cs}:
      Ui.also(Ui.setting(u, "room.seen." ++ id ++ "=" ++ Nat.show(Side.now(u))), cs)

def Rooms.mark(+n: Nat, a: Act, +id: String) -> Act:
  match n:
    case 0n:
      a
    case 1n+_:
      Rooms.mark.go(a, id)

def Bots.room.select(+u: Ui, +id: String) -> Act:
  Rooms.mark(Rooms.unread(u, id), Act{Ui.flag_off(Bots.views(u, "", id, ""), "settings"), [Focus{"room-post"}, ScrollEnd{}]}, id)

# A bot elsewhere on one of the owner's machines opens there: the window
# switches to that machine's hub and shows the bot with all its tabs (its
# name waits in "@bot.want" until that hub's log comes). A bot on anyone
# else's machine is only a conversation, as is every bot elsewhere on a
# client that cannot switch hubs ("local.fixed": the phones).

# a url's host:port ("https://box.ts.net:3787/x" → "box.ts.net:3787")
def Far.hostport(+url: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(Bool.pick(String, String.starts_with(url, "https://"), String.drop(url, 8n),
    Bool.pick(String, String.starts_with(url, "http://"), String.drop(url, 7n), url)), '/')), "")

def Far.host(+url: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(Far.hostport(url), ':')), "")

def Far.port(+url: String) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(Far.hostport(url), ':'), 1n), "")

# a host's first label, lowercased (a MagicDNS name's machine)
def Far.label(+url: String) -> String:
  String.to_lower(Maybe.default(&2, String, List.head(&2, String, String.split(Far.host(url), '.')), ""))

# whether a url's host is a machine on the owner's tailnet (domain: its
# MagicDNS domain, "" when unknown, then none is)
def Far.ours(+purl: String, +domain: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(domain)), String.ends_with(String.to_lower(Far.host(purl)), "." ++ String.to_lower(domain)))

# whether the machine named n at url is the peer called pname at purl: the
# same address, or, when purl is on the owner's tailnet, the same name (its
# MagicDNS label, or the peer's own) on the same port. A name alone is not
# enough: someone else's "laptop" is not the owner's.
def Far.hit(+n: String, +url: String, +pname: String, +purl: String, +domain: String) -> Bool:
  Bool.or(Bool.and(Bool.not(String.is_empty(url)), String.eq(Far.hostport(url), Far.hostport(purl))),
    Bool.and(Bool.and(Bool.and(Bool.not(String.is_empty(n)), Far.ours(purl, domain)), Bool.or(String.is_empty(Far.port(purl)), String.eq(Far.port(url), Far.port(purl)))),
      Bool.or(String.eq(String.to_lower(n), Far.label(purl)), String.eq(String.to_lower(n), String.to_lower(pname)))))

def Far.peers(ps: List<&2, J.Json>, +pname: String, +purl: String, +domain: String) -> Maybe<&2, String>:
  match ps:
    case Nil{}:
      None{}
    case +p <> t:
      Bool.pick(Maybe<&2, String>, Far.hit(J.Json.get_str(p, "name", ""), J.Json.get_str(p, "url", ""), pname, purl, domain), Some{J.Json.get_str(p, "url", "")}, Far.peers(t, pname, purl, domain))

# which of the owner's machines (the switcher's JSON) is the peer: "" for
# this one, a url for another, None when it is none of them
def Far.machine.k(+j: J.Json, +pname: String, +purl: String, +domain: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, Far.hit(J.Json.get_str(J.Json.get_obj(j, "self"), "name", ""), J.Json.get_str(J.Json.get_obj(j, "self"), "url", ""), pname, purl, domain), Some{""},
    Far.peers(J.Json.elems(J.Json.get_obj(j, "peers")), pname, purl, domain))

def Far.machine(+j: J.Json, +pname: String, +purl: String) -> Maybe<&2, String>:
  Far.machine.k(j, pname, purl, J.Json.get_str(J.Json.get_obj(j, "self"), "domain", ""))

# Notification Settings: the policy supplies values and defaults for every client.
type NotifyRow is Data:
  NotifyRow{name: String, note: String, on: String, key: String}

def Notify.row(+st: M.State, name: String, note: String, +kind: String) -> NotifyRow:
  NotifyRow{name, note, Bool.pick(String, NO.Policy.on(st, kind), "on", "off"), "notify." ++ kind}

def Notify.rows.loc(+l: I.Loc, +st: M.State) -> List<&2, NotifyRow>:
  [Notify.row(st, MSG.T.notify.needs(l), MSG.T.notify.needs.note(l), "needs"),
   Notify.row(st, MSG.T.notify.settled(l), MSG.T.notify.settled.note(l), "settled"),
   Notify.row(st, MSG.T.notify.reminders(l), MSG.T.notify.reminders.note(l), "reminders"),
   Notify.row(st, MSG.T.notify.all(l), MSG.T.notify.all.note(l), "all")]

def Notify.rows(st: M.State) -> List<&2, NotifyRow>:
  Notify.rows.loc(I.En{}, st)

def Notify.title(+l: I.Loc) -> String:
  MSG.T.notify.title(l)

def Notify.lead(+l: I.Loc) -> String:
  MSG.T.notify.lead(l)

# Pairing the owner's machines (Mate.*)
# ----------------------------
# Each hub finds the owner's other hubs (Machines.* in server.bend: the
# tailnet's online machines that answer GET /hello). One it is not linked
# to yet is paired by the hubs themselves, with no invite to paste: the
# hub with the lower address asks the other (POST /bots/pair, which only
# the owner's own devices may call: Tailscale has said whose they are)
# and links with the invite it answers (the usual signed POST /bots/link).
# From then on the two mirror each other's projects and threads in the
# background (far.bend). The setting "sync.<name>" = "off" keeps a
# machine out, on either side.

# whether a live link already reaches the machine called n at url
def Mate.linked(ps: List<&2, BT.Peer>, +n: String, +url: String, +domain: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case BT.Peer{_, +pn, +pu, _, _} <> t:
      Bool.or(Far.hit(n, url, pn, pu, domain), Mate.linked(t, n, url, domain))

# whether this hub (at `me`) asks the machine called n at url: it is not
# this one, this one's address is the lower (so of two hubs one asks),
# neither link nor setting says otherwise
# this hub's address as the other side of the comparison sees it: a hub
# reached on this machine's loopback (BACKPLANE_PEERS) is compared with
# this one's loopback address on its port, never its tailnet one, or both
# would find themselves the lower and both would ask
def Mate.local(+url: String) -> Bool:
  Bool.or(String.eq(Far.host(url), "127.0.0.1"), String.eq(Far.host(url), "localhost"))

def Mate.me(+me: String, +url: String) -> String:
  Bool.pick(String, Mate.local(url), "http://127.0.0.1:" ++ Far.port(me), me)

def Mate.wants(+n: String, +url: String, +ps: List<&2, BT.Peer>, +s: M.State, +me: String, +domain: String) -> Bool:
  Bool.and(Bool.and(Bool.and(Bool.not(String.is_empty(url)), Bool.not(String.is_empty(me))), LV.Live.cmp.lt(String.cmp(Mate.me(me, url), url))),
    Bool.and(Bool.not(Mate.off(s, n)), Bool.not(Mate.linked(ps, n, url, domain))))

def Mate.due.k(go: Bool, +n: String, +url: String, rest: List<&2, MRow>) -> List<&2, MRow>:
  match go:
    case True{}:
      MRow{n, url, True{}} <> rest
    case False{}:
      rest

def Mate.due.go(ms: List<&2, J.Json>, +tailnet: Bool, +ps: List<&2, BT.Peer>, +s: M.State, +me: String, +domain: String) -> List<&2, MRow>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Mate.due.k(Bool.and(Bool.or(tailnet, J.Json.get_bool(m, "named", False{})), Mate.wants(J.Json.get_str(m, "name", ""), J.Json.get_str(m, "url", ""), ps, s, me, domain)),
        J.Json.get_str(m, "name", ""), J.Json.get_str(m, "url", ""), Mate.due.go(t, tailnet, ps, s, me, domain))

# the machines (the switcher's JSON) this hub should ask to pair now. A
# hub started with --no-tailscale (tailnet False) still lists the owner's
# tailnet machines to switch to, but pairs only with those BACKPLANE_PEERS
# names ("named"): a throwaway hub never links itself to the real ones.
def Mate.due(+tailnet: Bool, +j: J.Json, +ps: List<&2, BT.Peer>, +s: M.State) -> List<&2, MRow>:
  Mate.due.go(J.Json.elems(J.Json.get_obj(j, "peers")), tailnet, ps, s, J.Json.get_str(J.Json.get_obj(j, "self"), "url", ""), J.Json.get_str(J.Json.get_obj(j, "self"), "domain", ""))

def Mate.known(m: Maybe<&2, String>) -> Bool:
  match m:
    case Some{+u}:
      Bool.not(String.is_empty(u))
    case None{}:
      False{}

# what this hub answers a machine that asks to pair: 0 an invite, 1 no
# (sync with it is off here), 2 no (a live link already reaches it), 3 no
# (it is none of the machines this hub found: a link must reach back)
def Mate.answer(+j: J.Json, +ps: List<&2, BT.Peer>, +s: M.State, +n: String, +url: String) -> Nat:
  Bool.pick(Nat, Bool.not(Mate.known(Far.machine(j, n, url))), 3n,
    Bool.pick(Nat, Mate.off(s, n), 1n,
      Bool.pick(Nat, Mate.linked(ps, n, url, J.Json.get_str(J.Json.get_obj(j, "self"), "domain", "")), 2n, 0n)))

# the owner's machines as a hub at `me` found them: one other, "desk" (for laws)
def Mate.demo(+me: String) -> J.Json:
  J.Json.obj([J.KV{"self", J.Json.obj([J.KV{"name", J.Str{"laptop"}}, J.KV{"url", J.Str{me}}, J.KV{"domain", J.Str{"t.ts.net"}}])},
    J.KV{"peers", J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"desk"}}, J.KV{"url", J.Str{"http://100.2.0.1:3787"}}, J.KV{"version", J.Str{"1"}}])])}])

# two hubs on one machine, each named in the other's BACKPLANE_PEERS: this
# one on `port` (its tailnet address in "self"), the other on 4002 (for laws)
def Mate.demo.lo(+port: String) -> J.Json:
  J.Json.obj([J.KV{"self", J.Json.obj([J.KV{"name", J.Str{"box"}}, J.KV{"url", J.Str{"http://100.9.0.1:" ++ port}}, J.KV{"domain", J.Str{"t.ts.net"}}])},
    J.KV{"peers", J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"127.0.0.1"}}, J.KV{"url", J.Str{"http://127.0.0.1:4002"}}, J.KV{"version", J.Str{"1"}}, J.KV{"named", J.Flag{True{}}}])])}])

# Settings' machines: each of the owner's other machines this hub found,
# whether it syncs with it ("sync.<name>": on unless "off"), and how that
# goes: linking (within the minute), synced, catching up (how many of its
# projects), away; nothing syncs with one turned off
type MateRow is Data:
  MateRow{name: String, note: String, on: String, key: String}

def Mate.link(ps: List<&2, BT.Peer>, +n: String, +url: String, +domain: String) -> String:
  match ps:
    case Nil{}:
      ""
    case BT.Peer{+i, +pn, +pu, _, _} <> t:
      Bool.pick(String, Far.hit(n, url, pn, pu, domain), i, Mate.link(t, n, url, domain))

def Mate.behind(ps: List<&2, M.Project>, +s: M.State, +link: String, +acc: Nat) -> Nat:
  match ps:
    case Nil{}:
      acc
    case M.Project{+id, _, _, _, _} <> t:
      Mate.behind(t, s, link, Bool.pick(Nat, Bool.and(String.eq(FR.Fr.link(id), link), FR.Fr.syncing(s, id)), 1n+acc, acc))

def Mate.note.linked(+l: I.Loc, +n: Nat, away: Bool) -> String:
  match away:
    case True{}:
      MSG.T.machines.away(l)
    case False{}:
      Bool.pick(String, Nat.is_eq(n, 0n), MSG.T.machines.sync(l), MSG.T.machines.behind(l, Nat.show(n), Bool.pick(String, Nat.is_eq(n, 1n), "one", "other")))

def Mate.note.k(+l: I.Loc, off: Bool, +link: String, +u: Ui) -> String:
  match off:
    case True{}:
      MSG.T.machines.off(l)
    case False{}:
      Bool.pick(String, String.is_empty(link), MSG.T.machines.linking(l),
        Mate.note.linked(l, Mate.behind(Ui.projects(Ui.state(u)), Ui.state(u), link, 0n), String.eq(Ui.info(u, "far.away." ++ link), "1")))

def Mate.row(+l: I.Loc, +n: String, +url: String, +domain: String, +u: Ui) -> MateRow:
  MateRow{n, Mate.note.k(l, Mate.off(Ui.state(u), n), Mate.link(BT.World.peers(M.State.bots(Ui.state(u))), n, url, domain), u),
    Bool.pick(String, Mate.off(Ui.state(u), n), "off", "on"), Mate.key(n)}

def Mate.rows.k(+l: I.Loc, skip: Bool, +m: J.Json, +domain: String, +u: Ui, rest: List<&2, MateRow>) -> List<&2, MateRow>:
  match skip:
    case True{}:
      rest
    case False{}:
      Mate.row(l, J.Json.get_str(m, "name", ""), J.Json.get_str(m, "url", ""), domain, u) <> rest

def Mate.rows.go(+l: I.Loc, ms: List<&2, J.Json>, +me: String, +domain: String, +u: Ui) -> List<&2, MateRow>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Mate.rows.k(l, Bool.or(String.is_empty(J.Json.get_str(m, "url", "")), String.eq(J.Json.get_str(m, "url", ""), me)), m, domain, u, Mate.rows.go(l, t, me, domain, u))

def Mate.rows.of(+l: I.Loc, +j: J.Json, +u: Ui) -> List<&2, MateRow>:
  Mate.rows.go(l, J.Json.elems(J.Json.get_obj(j, "peers")), J.Json.get_str(J.Json.get_obj(j, "self"), "url", ""), J.Json.get_str(J.Json.get_obj(j, "self"), "domain", ""), u)

# (the home hub's list: the machines it found)
def Mate.rows.loc(+l: I.Loc, +u: Ui) -> List<&2, MateRow>:
  Mate.rows.of(l, Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "machines")), J.Null{}), u)

def Mate.rows(+u: Ui) -> List<&2, MateRow>:
  Mate.rows.loc(Lang.loc(u), u)

def Mate.title(+l: I.Loc) -> String:
  MSG.T.machines.title(l)

def Mate.lead.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, List.is_empty(&2, MateRow, Mate.rows.loc(l, u)),
    MSG.T.machines.lead.none(l),
    MSG.T.machines.lead(l))

def Mate.lead(+u: Ui) -> String:
  Mate.lead.loc(Lang.loc(u), u)

def Far.chat(u: Ui, +id: String) -> Act:
  Act{Bots.views(u, "", "", id), [Focus{"remote-send"}, ScrollEnd{}]}

# the project picker's query changed: its highlight goes back to the top
def Bots.bfield.proj(proj: Bool, u: Ui) -> Ui:
  match proj:
    case True{}:
      Ui.scratch_set(u, "@proj.i", "0")
    case False{}:
      u

# (the secret box's value field, "sval", is never kept: an older phone app
# that sends its keystrokes as a form field leaves nothing in this state)
def Bots.bfield.keep(vault: Bool, +k: String, t: List<&2, String>, +u: Ui) -> Ui:
  match vault:
    case True{}:
      u
    case False{}:
      Bots.bfield.proj(String.eq(k, "proj"), Ui.scratch_set(u, Bots.field.key(u, k), String.join(t, "\u{1f}")))

# "name\u001ftext": a form field
def Bots.bfield.k(ps: List<&2, String>, u: Ui) -> Ui:
  match ps:
    case +k <> t:
      Bots.bfield.keep(String.eq(k, "sval"), k, t, u)
    case Nil{}:
      u

def Bots.bfield(u: Ui, +v: String) -> Ui:
  Bots.bfield.k(String.split(v, '\u{1f}'), u)

def Bots.flag(+u: Ui, +k: String) -> Ui:
  Ui.scratch_set(u, k, Bool.pick(String, String.eq(Ui.scratch(u, k), "1"), "", "1"))

# the new bot form was opened by the tour's "Create my uberbot" (scratch
# "@uber.new"): the request asks the hub to make the bot the uberbot too
def Bots.uber.new(+u: Ui) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, String.eq(Ui.scratch(u, "@uber.new"), "1"), [J.KV{"uber", J.Flag{True{}}}], Nil{})

def Bots.create.valid(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(Ui.scratch_set(u, "@bnew", ""), "@uber.new", ""), ["name", "persona"]), "bots.create",
    List.append(&2, J.KV, [J.KV{"name", J.Str{String.trim(Bots.field(u, "name"))}}, J.KV{"persona", J.Str{Bots.field(u, "persona")}},
     J.KV{"provider", J.Str{Bool.pick(String, String.is_empty(Bots.field(u, "provider")), M.Setting.get(Ui.state.of(u), "agent.provider", "claude"), Bots.field(u, "provider"))}},
     J.KV{"model", J.Str{Bots.field(u, "model")}}], Bots.uber.new(u)))

# Refusing a name never clears the persona or closes its form.
def Bots.create.k(valid: Bool, +u: Ui) -> Act:
  match valid:
    case True{}: Bots.create.valid(u)
    case False{}: Act{u, [Focus{"bot-name"}]}

def Bots.create(+u: Ui) -> Act:
  Bots.create.k(Bool.and(BT.Bot.name.valid(String.trim(Bots.field(u, "name"))), Bool.not(BT.Maybe.is(BT.Bot, BT.World.find(M.State.bots(Ui.state(u)), String.trim(Bots.field(u, "name")))))), u)

# the form's name field hint
def Bots.name.hint.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, String.eq(Ui.scratch(u, "@uber.new"), "1"), MSG.T.bots.name.uber(l), MSG.T.bots.name(l))

def Bots.name.hint(+u: Ui) -> String:
  Bots.name.hint.loc(Lang.loc(u), u)

def Bots.cur(+u: Ui) -> String:
  Ui.scratch(u, "@bot")

def Bots.edit(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(u, ["persona"]), "bots.edit", [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"persona", J.Str{Bots.field(u, "persona")}}])

def Bots.look(+u: Ui, +v: String) -> Act:
  Ui.rpc(u, "bots.edit", [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"look", J.Num{v}}])

def Bots.dir.peer(+o: J.Json) -> BT.Peer:
  BT.Peer{J.Json.get_str(o, "peer", ""), J.Json.get_str(o, "peerName", J.Json.get_str(o, "peer", "")), "", 0n, False{}}

def Bots.dir.peers(os: List<&2, J.Json>, +known: List<&2, BT.Peer>) -> List<&2, BT.Peer>:
  match os:
    case Nil{}:
      Nil{}
    case +o <> t:
      BT.Post.keep(BT.Peer, Bool.not(BT.Maybe.is(BT.Peer, BT.Peer.by.id(known, J.Json.get_str(o, "peer", "")))), Bots.dir.peer(o),
        Bots.dir.peers(t, Bots.dir.peer(o) <> known))

def Bots.peers.k(+ps: List<&2, BT.Peer>, +u: Ui) -> List<&2, BT.Peer>:
  List.append(&2, BT.Peer, ps, Bots.dir.peers(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "bots.remote")), J.Json.arr(Nil{}))), ps))

# the linked machines, then any the linked machines' bots list names that
# the log holds no record of (so their bots are still told apart by id)
def Bots.peers(+u: Ui) -> List<&2, BT.Peer>:
  Bots.peers.k(BT.World.peers(M.State.bots(Ui.state.of(u))), u)

def Far.mirrored(bs: List<&2, BT.Bot>, +key: String) -> Maybe<&2, BT.Bot>:
  match bs:
    case Nil{}:
      None{}
    case +b <> t:
      Bool.pick(Maybe<&2, BT.Bot>, Bool.and(FR.Fr.is(BT.Bot.id(b)), String.eq(BT.Member.far(BT.Bot.name(b), FR.Fr.link(BT.Bot.id(b))), key)), Some{b}, Far.mirrored(t, key))

def Far.pick(m: Maybe<&2, BT.Bot>, +u: Ui, +id: String) -> Act:
  match m:
    case Some{b}:
      Bots.select(u, BT.Bot.id(b))
    case None{}:
      Far.chat(u, id)

# a bot elsewhere, chosen: the bot itself when its machine shares its
# threads with this one (it is mirrored here: the same view as a bot
# here, at once, law far_pick_mirrored), else the conversation with it.
# The window never switches hubs to show a bot: that fetched the other
# hub's whole log (11-20 s on a 13 MB log).
def Bots.remote.select(+u: Ui, +id: String) -> Act:
  Far.pick(Far.mirrored(BT.World.bots(M.State.bots(Ui.state.of(u))), BT.Member.key(Bots.peers(u), id)), u, id)

# the members that are not the bot k stands for (BT.Member.key: "Kit@box"
# and "Kit@<box's id>" are one bot)
def Bots.drop(ms: List<&2, String>, +ps: List<&2, BT.Peer>, +k: String) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> +t:
      Bool.pick(List<&2, String>, String.eq(BT.Member.key(ps, m), k), Bots.drop(t, ps, k), m <> Bots.drop(t, ps, k))

# the room form's members: bot ids, one per line; picking one again drops
# it, however the member was spelled
def Bots.pick.k(has: Bool, +ms: List<&2, String>, +ps: List<&2, BT.Peer>, +id: String) -> List<&2, String>:
  match has:
    case True{}:
      List.filter(~String, ~(x => Bool.not(String.is_empty(x))), Bots.drop(ms, ps, BT.Member.key(ps, id)))
    case False{}:
      List.append(&2, String, ms, [id])

def Bots.pick.of(+u: Ui, +ms: List<&2, String>, +ps: List<&2, BT.Peer>, +id: String) -> Ui:
  Ui.scratch_set(u, "@f.rmembers", String.join(Bots.pick.k(BT.Member.has(ms, ps, BT.Member.key(ps, id)), ms, ps, id), "\n"))

def Bots.pick(+u: Ui, +id: String) -> Ui:
  Bots.pick.of(u, BT.Room.members.of(Bots.field(u, "rmembers")), Bots.peers(u), id)

def Bots.room.save(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(u, "@rnew", ""), ["rname", "rmembers", "rid"]), "bots.room",
    [J.KV{"room", J.Str{Bots.field(u, "rid")}}, J.KV{"name", J.Str{Bool.pick(String, String.is_empty(String.trim(Bots.field(u, "rname"))), "room", String.trim(Bots.field(u, "rname")))}},
     J.KV{"members", J.Str{Bots.field(u, "rmembers")}}])

def Bots.room.post.k(empty: Bool, +u: Ui) -> Act:
  match empty:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Ui.also(Ui.rpc.keep.msg(Ui.scratch_set(u, Bots.field.key(u, "rpost"), ""), "bots.post", [J.KV{"room", J.Str{Ui.scratch(u, "@room")}}, J.KV{"text", J.Str{Bots.field(u, "rpost")}}]), [ScrollEnd{}])

def Bots.room.post(+u: Ui) -> Act:
  Bots.room.post.k(String.is_empty(String.trim(Bots.field(u, "rpost"))), u)

def Bots.remote.send.k(empty: Bool, +u: Ui) -> Act:
  match empty:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Ui.also(Ui.rpc.keep.msg(Ui.scratch_set(u, "@f.rtext", ""), "bots.tell", [J.KV{"to", J.Str{Ui.scratch(u, "@remote")}}, J.KV{"text", J.Str{Bots.field(u, "rtext")}}]), [ScrollEnd{}])

def Bots.remote.send(+u: Ui) -> Act:
  Bots.remote.send.k(String.is_empty(String.trim(Bots.field(u, "rtext"))), u)

# a space's button ("bot\u001faction") or field ("bot\u001faction\u001ftext")
def Bots.space.k(ps: List<&2, String>, u: Ui) -> Act:
  match ps:
    case b <> a <> t:
      Ui.rpc(u, "bots.space", [J.KV{"bot", J.Str{b}}, J.KV{"action", J.Str{a}}, J.KV{"value", J.Str{String.join(t, "\u{1f}")}}])
    case _:
      Act{u, Nil{}}

def Bots.space(u: Ui, +v: String) -> Act:
  Bots.space.k(String.split(v, '\u{1f}'), u)

def Bots.space.link.k(ps: List<&2, String>, u: Ui) -> Act:
  match ps:
    case b <> +url <> _:
      Bots.space.k(b <> SPC.Space.link(url), u)
    case _:
      Act{u, Nil{}}

# a link or form in a bot's page ("bot\u001furl"): a "space:" one is a
# button or field; any other URL does nothing here (the app opens it)
def Bots.space.link(u: Ui, +v: String) -> Act:
  Bots.space.link.k(String.split(v, '\u{1f}'), u)

# What the form's When field says, in the person's zone: a schedule, a
# one-time phrase, or what is wrong with it (core/sched.bend's Sched.hint)
def Routine.hint.loc(+l: I.Loc, +u: Ui) -> String:
  SC.Sched.hint(l, Ui.state.of(u), Bots.field(u, "ocron"), Side.now(u), 0n, True{})

def Routine.hint(+u: Ui) -> String:
  Routine.hint.loc(Lang.loc(u), u)

# whether the routine form's When field reads as nothing (Sched.hint.bad)
def Routine.hint.bad(+u: Ui) -> Bool:
  SC.Sched.hint.bad(Ui.state.of(u), Bots.field(u, "ocron"), Side.now(u), 0n, True{})

# the key the text goes under: a schedule is `cron`, a time `at`
def Routine.key.k(cron: Bool, at: Bool) -> String:
  match cron at:
    case True{} _:
      "cron"
    case False{} True{}:
      "at"
    case False{} False{}:
      "cron"

def Routine.key(+u: Ui, +text: String) -> String:
  Routine.key.k(BT.Maybe.is(CR.Cron, CR.Cron.parse(text)), BT.Maybe.is(Nat, WH.When.parse(text, Side.now(u), SC.Sched.person(Ui.state.of(u), 0n, True{}))))

# the routine form's What field: a one-time routine runs once, so it is not
# asked what to do "each time"
def Routine.prompt(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, String.eq(Routine.key(u, Bots.field(u, "ocron")), "at"), MSG.T.web.routine.prompt.once(l), MSG.T.web.routine.prompt(l))

def Bots.routine.save.valid(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(u, "@onew", ""), ["oid", "oname", "ocron", "oprompt"]), "bots.routine",
    [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"routine", J.Str{Bots.field(u, "oid")}}, J.KV{"name", J.Str{Bool.pick(String, String.is_empty(Bots.field(u, "oname")), "routine", Bots.field(u, "oname"))}},
     J.KV{Routine.key(u, String.trim(Bots.field(u, "ocron"))), J.Str{String.trim(Bots.field(u, "ocron"))}}, J.KV{"prompt", J.Str{Bots.field(u, "oprompt")}}, J.KV{"on", J.Flag{True{}}}])

# The preview and Save use the same parser. A bad schedule keeps the
# person's name and prompt, with the keyboard where it can be corrected.
def Bots.routine.save.k(bad: Bool, u: Ui) -> Act:
  match bad:
    case True{}: Act{u, [Focus{"routine-cron"}]}
    case False{}: Bots.routine.save.valid(u)

def Bots.routine.save(+u: Ui) -> Act:
  Bots.routine.save.k(Bool.or(String.is_empty(String.trim(Bots.field(u, "ocron"))), Routine.hint.bad(u)), u)

def Bots.routine.of(+u: Ui, +id: String) -> BT.Routine:
  BT.World.routine(M.State.bots(Ui.state.of(u)), id)

def Bots.routine.toggle(+u: Ui, r: BT.Routine) -> Act:
  match r:
    case BT.Routine{i, b, n, c, p, on, _, _, _, _, _, _, _, _}:
      Ui.rpc(u, "bots.routine", [J.KV{"bot", J.Str{b}}, J.KV{"routine", J.Str{i}}, J.KV{"name", J.Str{n}}, J.KV{"cron", J.Str{c}}, J.KV{"prompt", J.Str{p}}, J.KV{"on", J.Flag{Bool.not(on)}}, J.KV{"keep", J.Flag{True{}}}])

def Routine.edit.when.k(once: Bool, rule: TZ.TzRule, +t: Nat, +c: String) -> String:
  Bool.pick(String, once, String.take(TZ.Tz.iso(rule, t), 16n), c)

# the When field of a routine being changed: its schedule, or a one-time
# routine's moment as a local time ("2026-10-14T09:00")
def Routine.edit.when(+s: M.State, +r: BT.Routine, +c: String) -> String:
  Routine.edit.when.k(Nat.is_gt(BT.Routine.once(r), 0n), SC.Sched.person(s, 0n, True{}), BT.Routine.once(r), c)

# a routine's fields into the form, to change it
def Bots.routine.edit(+u: Ui, r: BT.Routine) -> Ui:
  match r:
    case BT.Routine{+i, _, +n, +c, +p, _, _, _, _, _, _, _, _, _}:
      Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@onew", "1"), "@f.oid", i), "@f.oname", n), "@f.ocron", Routine.edit.when(Ui.state.of(u), r, c)), "@f.oprompt", p)

def Bots.hook.new(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(u, "@hook.secret", ""), ["hname"]), "bots.hook",
    [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"name", J.Str{Bool.pick(String, String.is_empty(Bots.field(u, "hname")), "webhook", Bots.field(u, "hname"))}}])

# a webhook's bearer token and the URL to post to, as the server answered
def Bots.token.clear(u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@hook.token", ""), "@hook.tokfor", ""), "@hook.tokurl", "")

# show (made the first time), "new" or "off"
def Bots.token(u: Ui, +hook: String, +op: String) -> Act:
  Ui.rpc(Bots.token.clear(u), "bots.hook_token", [J.KV{"hook", J.Str{hook}}, J.KV{"op", J.Str{op}}])

# Sealed fields
# -------------
# On the web (and the desktop app, the same page) a field that takes a
# secret has no binding (bots.bend's WB.sealed), as the secret box's: what
# is typed stays in the page, never in this state, a view action, a draft
# or a frame from the hub. Save, or Enter in one of its fields, is the
# action "sealed" with the form's name, which its fields carry too; host.js
# (sealedSave) reads them once, Sealed.req builds and checks the form's
# request ({"rpc"}, or {"error": why} and nothing is sent), and it goes
# straight to the hub. The hub's answer to it comes back as "sealed-sent"
# (value: the form) or "sealed-err" ("<form>\u001f<why>", said in the
# form: Sealed.say). The forms are Sealed.forms (a form may carry an
# argument, "<form>:<arg>"); phones and the window keep their own.

def Sealed.first(ps: List<&2, String>) -> String:
  match ps:
    case +h <> _:
      h
    case Nil{}:
      ""

# a form without its argument ("agykey:GEMINI_API_KEY" is "agykey")
def Sealed.base(+form: String) -> String:
  Sealed.first(String.split(form, ':'))

# why the form's last Save went nowhere ("" for none)
def Sealed.err(+u: Ui, +form: String) -> String:
  Ui.scratch(u, "@sealed:" ++ Sealed.base(form))

# a form's note, or why its last Save went nowhere
def Sealed.say(+u: Ui, +form: String, +note: String) -> String:
  Bool.pick(String, String.is_empty(Sealed.err(u, form)), note, Sealed.err(u, form))

def Sealed.forget(+u: Ui, +form: String) -> Ui:
  Ui.scratch_set(u, "@sealed:" ++ Sealed.base(form), "")

# a request for the host to send, or why nothing goes
def Sealed.out(+bad: String, +rpc: String) -> String:
  Bool.pick(String, String.is_empty(bad), J.Json.show(J.Json.obj([J.KV{"rpc", J.Str{rpc}}])), J.Json.show(J.Json.obj([J.KV{"error", J.Str{bad}}])))

def Bots.google.rpc.x(+u: Ui, +op: String, +account: String, +pasted: String, +redirect: String) -> Act:
  Ui.rpc(u, "bots.google", [J.KV{"op", J.Str{op}}, J.KV{"clientId", J.Str{Bots.field(u, "gid")}}, J.KV{"clientSecret", J.Str{Bots.field(u, "gsecret")}},
    J.KV{"pasted", J.Str{pasted}}, J.KV{"account", J.Str{account}}, J.KV{"redirect", J.Str{redirect}}])

def Bots.google.rpc(+u: Ui, +op: String, +account: String) -> Act:
  Bots.google.rpc.x(u, op, account, Bots.field(u, "gpaste"), "")

# A phone signs in through a loopback listener of its own (docs/bots.md
# "Google"): "connect@<port>" begins with Google sending its browser back
# to http://127.0.0.1:<port>/oauth/google, and "done:<address>" finishes
# with the address the listener caught.
def Bots.google.loop(+port: String) -> String:
  "http://127.0.0.1:" ++ port ++ "/oauth/google"

# An account's signature (gmail_send and gmail_draft add it after the body):
# the hub setting "gmail.sig.<address>". "sig-edit:<address>" opens it ("@gsig"
# names the account, the field "gsig" holds its text), "sig-save" writes it
# (an empty text removes it), "sig-cancel" shuts the editor.
def Bots.sig.key(+a: String) -> String:
  "gmail.sig." ++ a

def Bots.sig.of(+u: Ui, +a: String) -> String:
  M.Setting.get(Ui.state(u), Bots.sig.key(a), "")

# the address whose signature is being edited ("" for none)
def Bots.sig.editing(+u: Ui) -> String:
  Ui.scratch(u, "@gsig")

def Bots.sig.edit(+u: Ui, +a: String) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(u, "@gsig", a), "@f.gsig", Bots.sig.of(u, a)), Nil{}}

def Bots.sig.save.k(none: Bool, +u: Ui, +a: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Ui.setting(Ui.scratch_set(Ui.scratch_set(u, "@gsig", ""), "@f.gsig", ""), Bots.sig.key(a) ++ "=" ++ Bots.field(u, "gsig"))

def Bots.sig.save(+u: Ui) -> Act:
  Bots.sig.save.k(String.is_empty(Bots.sig.editing(u)), u, Bots.sig.editing(u))

def Bots.sig.cancel(+u: Ui) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(u, "@gsig", ""), "@f.gsig", ""), Nil{}}

def Bots.google.op(i: Nat, +u: Ui, +op: String) -> Act:
  match i:
    case 0n:
      Bots.google.rpc(u, "disconnect", String.drop(op, 8n))
    case 1n:
      Bots.google.rpc.x(u, "connect", "", "", Bots.google.loop(String.drop(op, 8n)))
    case 2n:
      Bots.google.rpc.x(u, "finish", "", String.drop(op, 5n), "")
    case 3n:
      Bots.sig.edit(u, String.drop(op, 9n))
    case 4n:
      Bots.sig.save(u)
    case 5n:
      Bots.sig.cancel(u)
    case _:
      Bots.google.rpc(u, op, "")

# "signout:<address>" signs one account out; "disconnect" signs out every one
def Bots.google.at(+op: String) -> Nat:
  Bool.pick(Nat, String.starts_with(op, "signout:"), 0n,
    Bool.pick(Nat, String.starts_with(op, "connect@"), 1n, Bool.pick(Nat, String.starts_with(op, "done:"), 2n,
      Bool.pick(Nat, String.starts_with(op, "sig-edit:"), 3n, Bool.pick(Nat, String.eq(op, "sig-save"), 4n, Bool.pick(Nat, String.eq(op, "sig-cancel"), 5n, 6n))))))

# (what the web's last sealed Save of Google's own client or its way back
# said goes)
def Bots.google(+u: Ui, +op: String) -> Act:
  Bots.google.op(Bots.google.at(op), Sealed.forget(Sealed.forget(u, "google"), "gpaste"), op)

def Bots.google.keep(empty: Bool, x: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      x <> rest

def Bots.google.full(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Bots.google.keep(String.is_empty(x), x, Bots.google.full(t))

# the addresses signed in, the default first (the hub's "googleAccounts",
# one a line)
def Bots.google.accts(+u: Ui) -> List<&2, String>:
  Bots.google.full(String.split(Ui.scratch(u, "@google.accts"), '\n'))

# Voice
# -----
# Dictation into the composer (docs/voice.md). The hub records its own
# machine's microphone with GPT-Live-Transcribe and sends {"t":"voice"}
# messages to the client that started it. "@voice" is the session
# (starting, live, stopping, "" when off), "@voice.th" its thread,
# "@voice.dev" the microphone, "@voice.lv" its level (0..100), "@voice.hint"
# why nothing is coming (no-audio, silent), "@voice.part" the words of the
# segment being heard, "@voice.err" the last error. A finished segment goes
# into its thread's draft where the caret is (VC.Voice.put); the words it may have misheard wait in "@voice.unsure"
# (VC.Voice.unsure.json, for thread "@voice.uth") to be clarified: take the
# guess, type the right spelling (it joins the dictionary), add the word as
# heard to the dictionary, or skip it (skipped words, "@voice.ign", are not
# asked about again).

def Voice.on(+u: Ui) -> Bool:
  Bool.not(String.is_empty(Ui.scratch(u, "@voice")))

def Voice.unsure(+u: Ui) -> List<&2, VC.Unsure>:
  VC.Voice.unsure.parse(Maybe.default(&2, J.Json, J.Json.parse(Ui.scratch(u, "@voice.unsure")), J.Null{}))

def Voice.unsure.set(u: Ui, us: List<&2, VC.Unsure>) -> Ui:
  Ui.scratch_set(u, "@voice.unsure", J.Json.show(VC.Voice.unsure.json(us)))

def Voice.ignored(+u: Ui) -> List<&2, String>:
  String.lines(Ui.scratch(u, "@voice.ign"))

def Voice.word(x: VC.Unsure) -> String:
  match x:
    case VC.Unsure{w, _}:
      w

def Voice.guess(x: VC.Unsure) -> String:
  match x:
    case VC.Unsure{_, g}:
      g

def Voice.has(us: List<&2, VC.Unsure>, +w: String) -> Bool:
  match us:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(Voice.word(x), w), Voice.has(t, w))

def Voice.merge.k(new: Bool, x: VC.Unsure, acc: List<&2, VC.Unsure>) -> List<&2, VC.Unsure>:
  match new:
    case True{}:
      List.append(&2, VC.Unsure, acc, [x])
    case False{}:
      acc

# the words to clarify: the old ones, then new ones not already asked
# about or skipped, at most VC.Voice.max_unsure()
def Voice.merge.go(us: List<&2, VC.Unsure>, +ig: List<&2, String>, +acc: List<&2, VC.Unsure>) -> List<&2, VC.Unsure>:
  match us:
    case Nil{}:
      acc
    case +x <> t:
      Voice.merge.go(t, ig, Voice.merge.k(Bool.and(Bool.not(Voice.has(acc, Voice.word(x))), Bool.not(VC.Voice.dict.has(ig, Voice.word(x)))), x, acc))

def Voice.merge(old: List<&2, VC.Unsure>, new: List<&2, VC.Unsure>, +ig: List<&2, String>) -> List<&2, VC.Unsure>:
  VC.Voice.cap(VC.Unsure, VC.Voice.max_unsure(), Voice.merge.go(new, ig, old))

def Voice.at(us: List<&2, VC.Unsure>, +n: String) -> VC.Unsure:
  Maybe.default(&2, VC.Unsure, List.get(&2, VC.Unsure, us, Maybe.default(&2, Nat, Nat.read(n), 0n)), VC.Unsure{"", ""})

def Voice.without.go(us: List<&2, VC.Unsure>, n: Nat) -> List<&2, VC.Unsure>:
  match us n:
    case Nil{} _:
      Nil{}
    case +x <> t 0n:
      t
    case +x <> t 1n+k:
      x <> Voice.without.go(t, k)

def Voice.drop(+u: Ui, +n: String) -> Ui:
  Voice.unsure.set(u, Voice.without.go(Voice.unsure(u), Maybe.default(&2, Nat, Nat.read(n), 0n)))

# a finished segment: into its thread's draft, and its words to clarify
def Voice.heard.ins(+u: Ui, +thread: String, +ins: VC.Ins, us: List<&2, VC.Unsure>) -> Ui:
  Voice.unsure.set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, thread, VC.Ins.text(ins)), "@voice.part", ""), "@voice.uth", thread),
    "@voice.at", ""), "@voice.to", Nat.show(VC.Ins.caret(ins))),
    Voice.merge(Bool.pick(List<&2, VC.Unsure>, String.eq(Ui.scratch(u, "@voice.uth"), thread), Voice.unsure(u), Nil{}), us, Voice.ignored(u)))

# where a segment goes: "@voice.at" when the window set it (its caret),
# else the draft's end
def Voice.at.of(+u: Ui, +thread: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Ui.scratch(u, "@voice.at")), String.length(Ui.scratch(u, thread)))

# a finished segment into its thread's draft (VC.Voice.put); "@voice.to"
# is where the caret goes after it
def Voice.heard(+u: Ui, +thread: String, +text: String, firm: Bool, us: List<&2, VC.Unsure>) -> Ui:
  Voice.heard.ins(u, thread, VC.Voice.put(firm, Ui.scratch(u, thread), Voice.at.of(u, thread), text), us)

def Voice.dev(+o: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(o, "label", "")), J.Json.get_str(o, "device", ""), J.Json.get_str(o, "label", ""))

def Voice.live(+u: Ui, +o: J.Json) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@voice", "live"), "@voice.dev", Voice.dev(o)), "@voice.lv", "0"),
    "@voice.hint", ""), "@voice.rec", J.Json.get_str(o, "recorder", ""))

def Voice.idle(u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@voice", ""), "@voice.part", ""), "@voice.lv", ""), "@voice.hint", "")

def Voice.recv.k(i: Nat, +u: Ui, +o: J.Json) -> Ui:
  match i:
    case 0n:
      Voice.live(u, o)
    case 1n:
      Ui.scratch_set(u, "@voice.part", Ui.scratch(u, "@voice.part") ++ J.Json.get_str(o, "text", ""))
    case 2n:
      Voice.heard(u, J.Json.get_str(o, "thread", ""), String.trim(J.Json.get_str(o, "text", "")), J.Json.get_bool(o, "firm", False{}), VC.Voice.unsure.parse(J.Json.get_obj(o, "unsure")))
    case 3n:
      Ui.scratch_set(u, "@voice.lv", Nat.show(J.Json.get_nat(o, "v", 0n)))
    case 4n:
      Ui.scratch_set(u, "@voice.hint", Bool.pick(String, String.eq(J.Json.get_str(o, "hint", ""), "ok"), "", J.Json.get_str(o, "hint", "")))
    case 5n:
      Ui.scratch_set(u, "@voice", "stopping")
    case 6n:
      Ui.scratch_set(u, "@voice.err", J.Json.get_str(o, "error", ""))
    case 7n:
      Voice.idle(u)
    case _:
      u

# a {"t":"voice"} message from the hub
def Voice.recv(+u: Ui, +o: J.Json) -> Ui:
  Voice.recv.k(C.Codec.pos(["live", "part", "text", "level", "hint", "stopping", "error", "idle"], J.Json.get_str(o, "ev", "")), u, o)

# what the hub answers about voice: the key (masked), the dictionary, the
# microphones ("@voice.set" once known)
def Voice.reply.k(has: Bool, +o: J.Json, +u: Ui) -> Ui:
  match has:
    case True{}:
      Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@voice.key", J.Json.get_str(o, "voiceKey", "")), "@voice.dict", J.Json.get_str(o, "voiceDict", "")),
        "@f.vdict", J.Json.get_str(o, "voiceDict", "")), "@voice.set", "1")
    case False{}:
      u

def Voice.reply.devs(+o: J.Json, +u: Ui) -> Ui:
  Bool.pick(Ui, String.is_empty(J.Json.get_str(o, "voiceDevices", "")), u, Ui.scratch_set(u, "@voice.devs", J.Json.get_str(o, "voiceDevices", "")))

def Voice.reply(+o: J.Json, +u: Ui) -> Ui:
  Voice.reply.devs(o, Voice.reply.k(Maybe.is_some(&2, J.Json, J.Json.get(o, "voiceDict")), o, u))

# the dictionary as the hub last answered it
def Voice.dict(+u: Ui) -> List<&2, String>:
  VC.Voice.dict.parse(Ui.scratch(u, "@voice.dict"))

# a microphone the hub found: its name (what the recorder takes) and label
type VDev is Data:
  VDev{name: String, label: String}

def Voice.devs.go(js: List<&2, J.Json>) -> List<&2, VDev>:
  match js:
    case Nil{}:
      Nil{}
    case +j <> t:
      VDev{J.Json.get_str(j, "name", ""), Bool.pick(String, String.is_empty(J.Json.get_str(j, "label", "")), J.Json.get_str(j, "name", ""), J.Json.get_str(j, "label", ""))} <> Voice.devs.go(t)

def Voice.devices(+u: Ui) -> List<&2, VDev>:
  Voice.devs.go(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(Ui.scratch(u, "@voice.devs")), J.Null{}), "devices")))

def Voice.label.go(ds: List<&2, VDev>, +name: String) -> String:
  match ds:
    case Nil{}:
      name
    case VDev{+n, +l} <> t:
      Bool.pick(String, String.eq(n, name), l, Voice.label.go(t, name))

# a microphone's label, from its name
def Voice.label(+u: Ui, +name: String) -> String:
  Voice.label.go(Voice.devices(u), name)

# the microphone the hub records by default
def Voice.default(+u: Ui) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(Ui.scratch(u, "@voice.devs")), J.Null{}), "default", "")

def Voice.rpc(u: Ui, +op: String, kvs: List<&2, J.KV>) -> Act:
  Ui.rpc(u, "bots.voice", J.KV{"op", J.Str{op}} <> kvs)

def Voice.start(+u: Ui) -> Act:
  Voice.rpc(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@voice", "starting"), "@voice.th", Ui.sel(u)), "@voice.err", ""), "@voice.hint", ""),
    "@voice.dev", ""), "start", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"device", J.Str{M.Setting.get(Ui.state.of(u), "voice.device", "")}}])

def Voice.stop(+u: Ui) -> Act:
  Voice.rpc(Ui.scratch_set(u, "@voice", "stopping"), "stop", [J.KV{"thread", J.Str{Ui.scratch(u, "@voice.th")}}])

# the mic button: start listening into the selected thread, or stop
def Voice.toggle(on: Bool, +u: Ui) -> Act:
  match on:
    case True{}:
      Voice.stop(u)
    case False{}:
      Voice.start(u)

# a word replaced in the draft it was heard into
def Voice.fix(+u: Ui, +w: String, +r: String) -> Ui:
  Ui.scratch_set(u, Ui.scratch(u, "@voice.uth"), VC.Voice.fix(Ui.scratch(u, Ui.scratch(u, "@voice.uth")), w, r))

def Voice.use(+u: Ui, x: VC.Unsure, +n: String) -> Act:
  match x:
    case VC.Unsure{+w, +g}:
      Act{Voice.drop(Bool.pick(Ui, String.is_empty(g), u, Voice.fix(u, w, g)), n), Nil{}}

def Voice.add(+u: Ui, x: VC.Unsure, +n: String) -> Act:
  Voice.rpc(Voice.drop(u, n), "add", [J.KV{"term", J.Str{Voice.word(x)}}])

def Voice.skip(+u: Ui, x: VC.Unsure, +n: String) -> Act:
  Act{Voice.drop(Ui.scratch_set(u, "@voice.ign", Ui.scratch(u, "@voice.ign") ++ "\n" ++ Voice.word(x)), n), Nil{}}

# "clarify": the word into a field to type its right spelling
def Voice.clarify(+u: Ui, +x: VC.Unsure, +n: String) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(u, "@voice.fix", n), "@f.vfix", Bool.pick(String, String.is_empty(Voice.guess(x)), Voice.word(x), Voice.guess(x))), [Focus{"voice-fix"}]}

def Voice.fixed.k(none: Bool, +u: Ui, +x: VC.Unsure, +r: String) -> Act:
  match none:
    case True{}:
      Act{Ui.scratch_set(u, "@voice.fix", ""), Nil{}}
    case False{}:
      Voice.rpc(Ui.scratch_set(Ui.scratch_set(Voice.drop(Voice.fix(u, Voice.word(x), r), Ui.scratch(u, "@voice.fix")), "@voice.fix", ""), "@f.vfix", ""),
        "add", [J.KV{"term", J.Str{r}}])

# the right spelling typed: it replaces the word, and joins the dictionary
def Voice.fixed(+u: Ui) -> Act:
  Voice.fixed.k(String.is_empty(String.trim(Ui.scratch(u, "@f.vfix"))), u, Voice.at(Voice.unsure(u), Ui.scratch(u, "@voice.fix")), String.trim(Ui.scratch(u, "@f.vfix")))

# Uploads over HTTP
# -----------------
# The phones send a file as the body of one POST /attach to the thread's
# hub (server.bend's Upload); the answer comes back here as "attach-end":
# the path joins the thread's attachments, or the failure shows

def Upload.fail(failed: Bool, +o: J.Json, u: Ui) -> Ui:
  match failed:
    case True{}:
      Ui.error_set(u, J.Json.get_str(o, "text", "the upload failed"))
    case False{}:
      u

def Upload.end.k(+o: J.Json, +u: Ui) -> Act:
  Act{Upload.fail(J.Json.get_bool(o, "upload", False{}), o, Ui.reply.att(o, Bool.not(J.Json.get_bool(o, "upload", False{})), u)), Nil{}}

def Upload.start.k(big: Bool, +u: Ui, +name: String) -> Act:
  match big:
    case True{}:
      Act{Ui.error_set(u, "attachments are limited to " ++ Nat.show(Nat.div(AT.Att.cap(), 1048576n)) ++ " MB"), Nil{}}
    case False{}:
      Act{Ui.up_set(u, Ui.sel(u), name), Nil{}}

def Upload.start(+u: Ui, +o: J.Json) -> Act:
  Upload.start.k(Nat.is_gt(J.Json.get_nat(o, "size", 0n), AT.Att.cap()), u, J.Json.get_str(o, "name", "file"))

def Upload.end(+u: Ui, +v: String) -> Act:
  Upload.end.k(Maybe.default(&2, J.Json, J.Json.parse(v), J.Json.obj([J.KV{"text", J.Str{"the upload failed"}}, J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"upload", J.Flag{True{}}}])), u)

# Design history
# --------------
# The viewer's history of the thread's board or schematic (core/hist.bend):
# the steps the agent's tool calls made, walked one by one or played (each
# version fades in over the last, only what changed), each shown as it was
# or compared with the step before, with what the agent said before it and
# a way back to that point of the thread; and any two versions compared
# (a branch, a commit, a turn, a step, the working copy). Kept per thread
# in the info map: "hist.at:<t>" the step on show ("" live), "hist.back:<t>"
# the path to go back to, "hist.diff:<t>" "1" to compare each step with the
# one before, "hist.play:<t>" the playback's token while it plays,
# "cmp.a:<t>"/"cmp.b:<t>" (and labels "cmp.al"/"cmp.bl") a comparison's
# versions, "refs:<t>" the hub's history.refs answer, "hist.jump" the
# entry last gone to (marked in the thread); the scratch's "@cmp.menu" the
# side a menu picks for.

def Hs.ext(+path: String) -> Nat:
  Bool.pick(Nat, Nat.is_eq(HI.Hist.ext(HI.Hist.family(path)), 1n), 1n, 0n)

def Hs.kind.of(+k: String, +path: String) -> Nat:
  Bool.pick(Nat, String.eq(k, "schematic"), 1n, Bool.pick(Nat, String.eq(k, "board"), 0n, Bool.pick(Nat, String.is_empty(k), Hs.ext(path), 2n)))

# the board (0), schematic (1) or neither (2) the thread's viewer shows
def Hs.kind(+u: Ui) -> Nat:
  Hs.kind.of(Ui.view_get(u, "view.kind"), Ui.view_get(u, "view.path"))

def Hs.kname(+k: Nat) -> String:
  Bool.pick(String, Nat.is_eq(k, 1n), "schematic", "board")

def Hs.get(+u: Ui, +k: String) -> String:
  Ui.view_get(u, k)

def Hs.set(+u: Ui, +k: String, +v: String) -> Ui:
  Ui.info_set(u, k ++ ":" ++ Ui.sel(u), v)

# the steps that changed the kind of file on show, oldest first
def Hs.track(+u: Ui) -> List<&2, HI.Step>:
  HI.Hist.track(HI.Hist.steps(Ui.state(u), Ui.sel(u)), Hs.kind(u))

def Hs.path(+u: Ui) -> String:
  Ui.view_get(u, "view.path")

# the position on show (1 the first step), 0 while the live file shows
def Hs.at(+u: Ui, +ss: List<&2, HI.Step>) -> Nat:
  Bool.pick(Nat, Nat.is_eq(HI.Hist.kind(Hs.path(u)), 0n), 0n, HI.Hist.index(ss, Hs.get(u, "hist.at")))

def Hs.diff(+u: Ui) -> Bool:
  String.eq(Hs.get(u, "hist.diff"), "1")

# (a comparison on show, or one of its sides: "cmp.as:<t>" old or new)
def Hs.cmp(+u: Ui) -> Bool:
  Bool.and(Bool.not(String.is_empty(Hs.get(u, "cmp.a"))), Bool.or(Nat.is_eq(HI.Hist.kind(Hs.path(u)), 2n), Bool.not(String.is_empty(Hs.get(u, "cmp.as")))))

def Hs.playing(+u: Ui) -> Bool:
  Bool.not(String.is_empty(Hs.get(u, "hist.play")))

# the live file is kept to come back to, the first time a version shows
def Hs.keep(live: Bool, +u: Ui, +path: String) -> Ui:
  match live:
    case True{}:
      Hs.set(u, "hist.back", path)
    case False{}:
      u

def Hs.keep.live(+u: Ui) -> Ui:
  Hs.keep(Nat.is_eq(HI.Hist.kind(Hs.path(u)), 0n), u, Hs.path(u))

def Hs.clear.cmp(u: Ui) -> Ui:
  Hs.set(Hs.set(Hs.set(Hs.set(Hs.set(u, "cmp.a", ""), "cmp.b", ""), "cmp.al", ""), "cmp.bl", ""), "cmp.as", "")

# show a step (as it was, or compared with the one before)
def Hs.show(+u: Ui, +s: HI.Step, +k: Nat) -> Ui:
  Ui.view_put(Hs.set(Hs.clear.cmp(Hs.keep.live(u)), "hist.at", HI.Step.id(s)), Hs.kname(k),
    HI.Step.spec(s, k, HI.Hist.family(Hs.path(u)), Hs.diff(u)))

# the path to go back to: the one kept, when it is a file of the kind on
# show (a step of the other kind moved the viewer: its project's own file)
def Hs.back(+u: Ui) -> String:
  Bool.pick(String, Nat.is_eq(Hs.ext(Hs.get(u, "hist.back")), Hs.kind(u)), Hs.get(u, "hist.back"), "")

# back to the live file
def Hs.live(+u: Ui) -> Ui:
  Ui.view_put(Hs.clear.cmp(Hs.set(Hs.set(Hs.set(u, "hist.at", ""), "hist.play", ""), "hist.back", "")), Hs.kname(Hs.kind(u)),
    Bool.pick(String, Nat.is_eq(HI.Hist.kind(Hs.path(u)), 0n), Hs.path(u), Hs.back(u)))

def Hs.go.k(m: Maybe<&2, HI.Step>, +u: Ui) -> Ui:
  match m:
    case Some{+s}:
      Hs.show(u, s, Hs.kind(u))
    case None{}:
      Hs.live(u)

# go to position i of the track (0, or past its end: the live file)
def Hs.go(+u: Ui, +ss: List<&2, HI.Step>, +i: Nat) -> Ui:
  Hs.go.k(HI.Hist.nth(ss, i), u)

def Hs.n(+ss: List<&2, HI.Step>) -> Nat:
  List.length(&2, HI.Step, ss)

def Hs.step(fwd: Bool, +u: Ui) -> Ui:
  match fwd:
    case True{}:
      Hs.go(u, Hs.track(u), HI.Hist.fwd(Hs.n(Hs.track(u)), Hs.at(u, Hs.track(u))))
    case False{}:
      Hs.go(u, Hs.track(u), HI.Hist.back(Hs.n(Hs.track(u)), Hs.at(u, Hs.track(u))))

# Playback: a step every Hs.beat() ms. Each start takes a new token, and a
# tick only counts with the token still playing, so pausing and playing
# again never leaves two clocks running.

def Hs.beat() -> Nat:
  1400n

# a step is shown its beat once the viewer has drawn it: the tick waits
# for the page's plots before its beat starts (presentation.bend's Later
# "after" "drawn"; plot2d.js's whenDrawn), so a step that takes the hub
# a while to read is still seen (test/hist_test.bend)
def Hs.drawn(+action: String) -> Bool:
  String.eq(action, "hist-tick")

def Hs.tick(+u: Ui, +token: String) -> Cmd:
  Later{Hs.beat(), "hist-tick", Ui.sel(u) ++ "\t" ++ token}

def Hs.num(s: String) -> Nat:
  M.Switch.num(s, 0n)

def Hs.play.start(+u: Ui, +ss: List<&2, HI.Step>) -> Act:
  +token = Nat.show(1n+Hs.num(Ui.info(u, "hist.gen")))
  +at = Hs.at(u, ss)
  +u2 = Hs.set(Ui.info_set(u, "hist.gen", token), "hist.play", token)
  Act{Bool.pick(Ui, Bool.or(Nat.is_eq(at, 0n), Nat.is_ge(at, Hs.n(ss))), Hs.go(u2, ss, 1n), u2), [Hs.tick(u, token)]}

def Hs.play.k(on: Bool, +u: Ui) -> Act:
  match on:
    case True{}:
      Act{Hs.set(u, "hist.play", ""), Nil{}}
    case False{}:
      Hs.play.start(u, Hs.track(u))

def Hs.play(+u: Ui) -> Act:
  Hs.play.k(Hs.playing(u), u)

def Hs.tick.go(last: Bool, +u: Ui, +ss: List<&2, HI.Step>, +at: Nat, +token: String) -> Act:
  match last:
    case True{}:
      Act{Hs.set(u, "hist.play", ""), Nil{}}
    case False{}:
      Act{Hs.go(u, ss, 1n+at), [Hs.tick(u, token)]}

def Hs.tick.k(mine: Bool, +u: Ui, +token: String) -> Act:
  match mine:
    case True{}:
      Hs.tick.go(Bool.or(Nat.is_eq(Hs.at(u, Hs.track(u)), 0n), Nat.is_ge(Hs.at(u, Hs.track(u)), Hs.n(Hs.track(u)))), u, Hs.track(u), Hs.at(u, Hs.track(u)), token)
    case False{}:
      Act{u, Nil{}}

# a tick: value "<thread>\t<token>"; on another thread, or a stale
# token, it does nothing
def Hs.tick.away(here: Bool, +u: Ui, +th: String, +token: String) -> Act:
  match here:
    case True{}:
      Hs.tick.k(Bool.and(Hs.playing(u), String.eq(token, Hs.get(u, "hist.play"))), u, token)
    case False{}:
      Act{Ui.info_set(u, "hist.play:" ++ th, ""), Nil{}}

# (a tick for a thread no longer on show stops its playback: it plays
# only while seen)
def Hs.tick.on(+u: Ui, +value: String) -> Act:
  Hs.tick.away(String.eq(HI.Hist.at(String.split(value, '\t'), 0n), Ui.sel(u)), u, HI.Hist.at(String.split(value, '\t'), 0n), HI.Hist.at(String.split(value, '\t'), 1n))

def Hs.diff.flip(+u: Ui) -> Ui:
  Hs.go(Hs.set(u, "hist.diff", Bool.pick(String, Hs.diff(u), "", "1")), Hs.track(u), Hs.at(u, Hs.track(u)))

# Going back to the thread
# ------------------------
# The entry a step followed is revealed (its folds open, enough pages
# shown) and marked; the host scrolls to it (Jump).

def Hs.pos(es: List<&2, M.Entry>, +id: String, +i: Nat) -> Nat:
  match es:
    case Nil{}:
      i
    case +e <> t:
      Bool.pick(Nat, String.eq(M.Idx.id(e), id), i, Hs.pos(t, id, 1n+i))

def Hs.opens(ks: List<&2, String>, u: Ui) -> Ui:
  match ks:
    case Nil{}:
      u
    case +k <> t:
      Hs.opens(t, Ui.info_set(u, k, "1"))

# enough pages that the entry p back shows, on any client (a phone's
# first page is shorter than the window's)
def Hs.pages(+u: Ui, +th: String, +p: Nat) -> Ui:
  Ui.scratch_set(u, "@more." ++ th, Nat.show(Bool.pick(Nat, Nat.is_ge(Hs.num(Ui.scratch(u, "@more." ++ th)), Nat.div(Nat.add(p, 59n), 60n)), Hs.num(Ui.scratch(u, "@more." ++ th)), Nat.div(Nat.add(p, 59n), 60n))))

# (playing stops: the thread is on show now)
def Hs.jump.to(+u: Ui, +es: List<&2, M.Entry>, +id: String) -> Act:
  Act{Ui.info_set(Hs.pages(Hs.opens(FO.Fold.reveal(List.reverse(&2, M.Entry, es), id), Hs.set(u, "hist.play", "")), Ui.sel(u), Hs.pos(es, id, 0n)), "hist.jump", id), [Jump{id}]}

def Hs.anchor.k(m: Maybe<&2, HI.Step>, +es: List<&2, M.Entry>) -> String:
  match m:
    case Some{+s}:
      HI.Hist.anchor(es, s)
    case None{}:
      ""

# the entry the step on show followed
def Hs.anchor(+u: Ui, +es: List<&2, M.Entry>) -> String:
  Hs.anchor.k(HI.Hist.nth(Hs.track(u), Hs.at(u, Hs.track(u))), es)

def Hs.jump.k(none: Bool, +u: Ui, +es: List<&2, M.Entry>, +id: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Hs.jump.to(u, es, id)

# go to an entry of the selected thread ("" the one the step on show followed)
def Hs.jump(+u: Ui, +value: String) -> Act:
  +es = M.State.mine(Ui.state(u), Ui.sel(u))
  +id = Bool.pick(String, String.is_empty(value), Hs.anchor(u, es), value)
  Hs.jump.k(String.is_empty(id), u, es, id)

# A step's row in the thread shows it: its board, else its schematic (the
# kind on show when the step changed one)
def Hs.show.kind(+s: HI.Step, +k: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_lt(k, 2n), HI.Step.has(s, k)), k, Bool.pick(Nat, HI.Step.has(s, 0n), 0n, 1n))

def Hs.show.at(m: Maybe<&2, HI.Step>, +u: Ui) -> Ui:
  match m:
    case Some{+s}:
      Hs.show(Ui.view_put(u, Hs.kname(Hs.show.kind(s, Hs.kind(u))), Bool.pick(String, Nat.is_eq(HI.Hist.kind(Hs.path(u)), 0n), Hs.path(u), Hs.get(u, "hist.back"))), s, Hs.show.kind(s, Hs.kind(u)))
    case None{}:
      u

def Hs.find(ss: List<&2, HI.Step>, +id: String) -> Maybe<&2, HI.Step>:
  match ss:
    case Nil{}:
      None{}
    case +s <> t:
      Bool.pick(Maybe<&2, HI.Step>, String.eq(HI.Step.id(s), id), Some{s}, Hs.find(t, id))

def Hs.show.id(+u: Ui, +id: String) -> Ui:
  Hs.show.at(Hs.find(HI.Hist.steps(Ui.state(u), Ui.sel(u)), id), Hs.keep.live(u))

# Comparing
# ---------
# The menu lists what a side may be: the working copy, the thread's steps
# (newest first), then what the hub found (history.refs: HEAD, branches,
# commits that touched KiCad files, the thread's turns). A row's value is
# "<side>\t<commit>\t<label>\t<git dir>\t<top>" (commit "" the working copy).

type HRow is Data:
  HRow{label: String, sub: String, value: String}

def Hs.refs(+u: Ui) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Hs.get(u, "refs")), J.Null{})

def Hs.row.value(+side: String, +commit: String, +label: String, +git: String, +top: String) -> String:
  side ++ "\t" ++ commit ++ "\t" ++ label ++ "\t" ++ git ++ "\t" ++ top

# A side's label is kept as the menu's value says it, in English ("Step
# 3", "Turn 2", "Branch point · main", "Working copy": the scratch holds
# it); Hs.label.shown says it in a language where it shows.
def Hs.label.k(+l: I.Loc, +s: String, step: Bool, turn: Bool, base: Bool) -> String:
  match step turn base:
    case True{} _ _: MSG.T.history.step(l, String.drop(s, 5n))
    case False{} True{} _: MSG.T.history.turn(l, String.drop(s, 5n))
    case False{} False{} True{}: MSG.T.history.branchpoint(l, String.drop(s, 15n))
    case False{} False{} False{}: Bool.pick(String, String.eq(s, "Working copy"), MSG.T.history.workingcopy(l), s)

def Hs.label.shown(+l: I.Loc, +s: String) -> String:
  Hs.label.k(l, s, String.starts_with(s, "Step "), String.starts_with(s, "Turn "), String.starts_with(s, "Branch point · "))

def Hs.row.steps(ss: List<&2, HI.Step>, +l: I.Loc, +side: String, +es: List<&2, M.Entry>, +i: Nat, acc: List<&2, HRow>) -> List<&2, HRow>:
  match ss:
    case Nil{}:
      acc
    case HI.Step{+id, _, +c, _, +g, +top, _, _} <> t:
      Hs.row.steps(t, l, side, es, 1n+i, HRow{MSG.T.history.step(l, Nat.show(i)), HI.Hist.label(es, HI.Hist.anchor(es, HI.Step{id, "", c, "", g, top, Nil{}, 0n})), Hs.row.value(side, c, "Step " ++ Nat.show(i), g, top)} <> acc)

def Hs.ref.label(+r: HI.Ref) -> String:
  match r:
    case HI.Ref{+k, +l, _}:
      Bool.pick(String, String.eq(k, "turn"), "Turn " ++ HI.Hist.at(List.reverse(&2, String, String.split(l, '/')), 0n),
        Bool.pick(String, String.eq(k, "base"), "Branch point · " ++ l, l))

# (a base is where the checked-out branch left the main one: compared with
# it, a pull request's whole change; a tag is a release)
def Hs.ref.kind.loc(+l: I.Loc, +k: String) -> String:
  Bool.pick(String, String.eq(k, "head"), MSG.T.history.ref.head(l), Bool.pick(String, String.eq(k, "branch"), MSG.T.history.ref.branch(l), Bool.pick(String, String.eq(k, "turn"), MSG.T.history.ref.turn(l),
    Bool.pick(String, String.eq(k, "tag"), MSG.T.history.ref.tag(l), Bool.pick(String, String.eq(k, "base"), MSG.T.history.ref.base(l), MSG.T.history.ref.commit(l))))))

def Hs.ref.kind(k: String) -> String:
  Hs.ref.kind.loc(I.En{}, k)

# (turn 0 is the thread before it began)
def Hs.ref.sub(+l: I.Loc, +k: String, +label: String) -> String:
  Bool.pick(String, String.eq(label, "Turn 0"), MSG.T.history.ref.start(l), Hs.ref.kind.loc(l, k))

def Hs.row.refs(rs: List<&2, HI.Ref>, +l: I.Loc, +side: String, +git: String, +top: String) -> List<&2, HRow>:
  match rs:
    case Nil{}:
      Nil{}
    case HI.Ref{+k, +n, +c} <> t:
      HRow{Hs.label.shown(l, Hs.ref.label(HI.Ref{k, n, c})), Hs.ref.sub(l, k, Hs.ref.label(HI.Ref{k, n, c})), Hs.row.value(side, c, Hs.ref.label(HI.Ref{k, n, c}), git, top)} <> Hs.row.refs(t, l, side, git, top)

# the newest steps the menu lists (older ones are on the scrubber), so
# the branches and commits after them stay in reach
def Hs.menu.steps() -> Nat:
  8n

# the menu's rows for a side ("a" the old version, "b" the new)
# the menu's step rows: the newest Hs.menu.steps(), newest first, as
# taking them from every step's row gives (each row's label walks the
# thread's entries: made for every step, a thread with a long history
# took seconds a screen on the phones)
def Hs.row.last(+ss: List<&2, HI.Step>, +l: I.Loc, +side: String, +es: List<&2, M.Entry>) -> List<&2, HRow>:
  Hs.row.steps(List.drop(&2, HI.Step, ss, Nat.sub(List.length(&2, HI.Step, ss), Hs.menu.steps())), l, side, es, 1n+Nat.sub(List.length(&2, HI.Step, ss), Hs.menu.steps()), Nil{})

def Hs.menu.loc(+l: I.Loc, +u: Ui, +side: String) -> List<&2, HRow>:
  +r = Hs.refs(u)
  +es = M.State.mine(Ui.state(u), Ui.sel(u))
  List.append(&2, HRow, [HRow{MSG.T.history.workingcopy(l), MSG.T.history.workingcopy.note(l), Hs.row.value(side, "", "Working copy", "", "")}],
    List.append(&2, HRow, Hs.row.last(Hs.track(u), l, side, es),
      Hs.row.refs(HI.Hist.refs(J.Json.get_str(r, "refs", "")), l, side, J.Json.get_str(r, "git", ""), J.Json.get_str(r, "top", ""))))

def Hs.menu(+u: Ui, side: String) -> List<&2, HRow>:
  Hs.menu.loc(Lang.loc(u), u, side)

# the live file compared: the one on show, else the project's (the hub's
# answer names its board and schematic)
def Hs.live.abs(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(HI.Hist.family(Hs.path(u))),
    J.Json.get_str(Hs.refs(u), Bool.pick(String, Nat.is_eq(Hs.kind(u), 1n), "sch", "pcb"), ""), HI.Hist.family(Hs.path(u)))

# the sides picked belong to one file ("cmp.f:<t>"): another file on show
# starts a comparison over
def Hs.cmp.fresh(+u: Ui) -> Ui:
  Bool.pick(Ui, String.eq(Hs.get(u, "cmp.f"), Hs.live.abs(u)), u, Hs.set(Hs.clear.cmp(u), "cmp.f", Hs.live.abs(u)))

# one side's version: the working copy, or the file in a commit
def Hs.side.spec(+commit: String, +git: String, +top: String, +abs: String) -> String:
  Bool.pick(String, String.is_empty(commit), abs,
    Bool.pick(String, String.starts_with(abs, top ++ "/"), HI.Hist.git(git, commit, top, String.drop(abs, 1n+String.length(top))), ""))

# a side not picked is the working copy
def Hs.cmp.side(+u: Ui, +side: String) -> String:
  Bool.pick(String, String.is_empty(Hs.get(u, "cmp." ++ side)), Hs.live.abs(u), Hs.get(u, "cmp." ++ side))

# (on the layers "cmp.bits:<t>" names: the active layer's, 0 every one)
def Hs.cmp.path(+u: Ui) -> String:
  HI.Hist.diff.on(Hs.cmp.side(u, "a"), Hs.cmp.side(u, "b"), Maybe.default(&2, U32, U32.read(Hs.get(u, "cmp.bits")), 0))

def Hs.cmp.show(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.view_put(u, Hs.kname(Hs.kind(u)), Hs.cmp.path(u)), "@cmp.menu", "")

def Hs.cmp.fill.a(+u: Ui) -> Ui:
  Bool.pick(Ui, String.is_empty(Hs.get(u, "cmp.a")), Hs.set(Hs.set(u, "cmp.a", Hs.live.abs(u)), "cmp.al", "Working copy"), u)

def Hs.cmp.fill.b(+u: Ui) -> Ui:
  Bool.pick(Ui, String.is_empty(Hs.get(u, "cmp.b")), Hs.set(Hs.set(u, "cmp.b", Hs.live.abs(u)), "cmp.bl", "Working copy"), u)

# the side not picked yet is the working copy (named, so the bar and
# Hs.cmp see a comparison whichever side came first)
def Hs.cmp.fill(+u: Ui) -> Ui:
  Hs.cmp.fill.b(Hs.cmp.fill.a(u))

# a version picked for a side
def Hs.pick.k(bad: Bool, +u: Ui, +side: String, +spec: String, +label: String) -> Ui:
  match bad:
    case True{}:
      Ui.error_set(u, MSG.T.history.notinrepo(Lang.loc(u)))
    case False{}:
      Hs.cmp.show(Hs.cmp.fill(Hs.set(Hs.set(Hs.set(Hs.set(Hs.set(Hs.cmp.fresh(Hs.keep.live(u)), "hist.at", ""), "hist.play", ""), "cmp." ++ side, spec), "cmp." ++ side ++ "l", label), "cmp.as", "")))

def Hs.pick.v(+u: Ui, +xs: List<&2, String>) -> Ui:
  +side = Bool.pick(String, String.eq(HI.Hist.at(xs, 0n), "b"), "b", "a")
  +spec = Hs.side.spec(HI.Hist.at(xs, 1n), HI.Hist.at(xs, 3n), HI.Hist.at(xs, 4n), Hs.live.abs(u))
  Hs.pick.k(Bool.or(String.is_empty(Hs.live.abs(u)), String.is_empty(spec)), u, side, spec, HI.Hist.at(xs, 2n))

def Hs.pick(+u: Ui, +value: String) -> Ui:
  Hs.pick.v(u, String.split(value, '\t'))

# opening the menu asks the hub what there is to compare with
def Hs.menu.open(+u: Ui, +side: String) -> Act:
  Ui.rpc(Ui.scratch_set(u, "@cmp.menu", side), "history.refs", [J.KV{"thread", J.Str{Ui.sel(u)}}])

def Hs.menu.k(open: Bool, +u: Ui, +side: String) -> Act:
  match open:
    case True{}:
      Act{Ui.scratch_set(u, "@cmp.menu", ""), Nil{}}
    case False{}:
      Hs.menu.open(u, side)

def Hs.refs.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "refs"), J.Null{})), "null"))

# the project's own board and schematic are kept in the scratch too, which
# a phone keeps across a relaunch: comments pin on them before the next
# answer (Rc.file); empty answers leave what was kept
def Hs.refs.keep.k(+k: String, +v: String, +u: Ui) -> Ui:
  Bool.pick(Ui, String.is_empty(v), u, Ui.scratch_set(u, k, v))

def Hs.refs.keep(u: Ui, +r: J.Json) -> Ui:
  Hs.refs.keep.k("@rc.sch:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "sch", ""),
    Hs.refs.keep.k("@rc.pcb:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "pcb", ""), u))

def Hs.refs.store(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Hs.refs.keep(Ui.info_set(u, "refs:" ++ J.Json.get_str(J.Json.get_obj(o, "result"), "thread", ""), J.Json.show(J.Json.get_obj(o, "result"))), J.Json.get_obj(o, "result"))
    case False{}:
      u

# Design review: comparing (docs/design-review.md)
# -------------------------------------------------
# Actions "rv-…" route here before the action table (Rv.act). A
# comparison shows as Old, Diff or New ("rv-as": old, diff, new; kept as
# "cmp.as:<t>", "" the diff): one canvas, the same family, so a flip only
# changes what is drawn. "rv-bits" restricts its marks to some layers (the
# clients send the active layer's, Rv.bits; 0 every layer). "rv-ref" asks
# the hub for any commit, branch or tag typed in ("<side>\t<text>",
# history.resolve), and its answer is picked like a menu row.

# a side at the sheet on show (a sheet picked while Old or New shows keeps
# its place when the flip moves on)
def Rv.side.at(+u: Ui, +side: String) -> String:
  HI.Hist.sheet(Hs.cmp.side(u, side), HI.Hist.family(Hs.path(u)))

def Rv.diff.at(+u: Ui) -> String:
  HI.Hist.diff.on(Rv.side.at(u, "a"), Rv.side.at(u, "b"), Maybe.default(&2, U32, U32.read(Hs.get(u, "cmp.bits")), 0))

def Rv.as.path(+u: Ui, +as: String) -> String:
  Bool.pick(String, String.eq(as, "old"), Rv.side.at(u, "a"), Bool.pick(String, String.eq(as, "new"), Rv.side.at(u, "b"), Rv.diff.at(u)))

def Rv.as.k(on: Bool, +u: Ui, +as: String) -> Ui:
  match on:
    case True{}:
      +v = Bool.pick(String, Bool.or(String.eq(as, "old"), String.eq(as, "new")), as, "")
      Ui.view_put(Hs.set(u, "cmp.as", v), Hs.kname(Hs.kind(u)), Rv.as.path(u, v))
    case False{}:
      u

def Rv.as(+u: Ui, +as: String) -> Ui:
  Rv.as.k(Hs.cmp(u), u, as)

# which of Old, Diff and New shows ("" none: no comparison)
def Rv.showing(+u: Ui) -> String:
  Bool.pick(String, Hs.cmp(u), Bool.pick(String, String.is_empty(Hs.get(u, "cmp.as")), "diff", Hs.get(u, "cmp.as")), "")

# the layers a focus is about, as a comparison's bits (0 when no layer
# stands out): the active one and those kept with it
def Rv.bits(+kind: U32, +f: LY.Focus) -> U32:
  Bool.pick(U32, Bool.and(LY.Lyr.on(f), Bool.not(U32.is_eq(kind, 1))), LY.Lyr.kept(kind, f), 0)

def Rv.bits.k(diff: Bool, +u: Ui, +bits: String) -> Ui:
  match diff:
    case True{}:
      Ui.view_put(Hs.set(u, "cmp.bits", bits), Hs.kname(Hs.kind(u)), Rv.diff.at(Hs.set(u, "cmp.bits", bits)))
    case False{}:
      Hs.set(u, "cmp.bits", bits)

def Rv.bits.set(+u: Ui, +bits: String) -> Ui:
  Rv.bits.k(Bool.and(Hs.cmp(u), String.is_empty(Hs.get(u, "cmp.as"))), u, Bool.pick(String, String.eq(bits, "0"), "", bits))

def Rv.ref.ask(+u: Ui, +side: String, +text: String) -> Act:
  Ui.rpc(Ui.scratch_set(u, "@cmp.menu", ""), "history.resolve", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"side", J.Str{side}}, J.KV{"ref", J.Str{text}}])

def Rv.ref.k(blank: Bool, +u: Ui, +side: String, +text: String) -> Act:
  match blank:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Rv.ref.ask(u, side, text)

def Rv.ref(+u: Ui, +xs: List<&2, String>) -> Act:
  Rv.ref.k(String.is_empty(String.trim(HI.Hist.at(xs, 1n))), u, Bool.pick(String, String.eq(HI.Hist.at(xs, 0n), "b"), "b", "a"), String.trim(HI.Hist.at(xs, 1n)))

# the hub's answer: {resolved: {thread, side, ref, commit, git, top}}, a
# version picked for its side (on its own thread only)
def Rv.reply.k(+o: J.Json, +u: Ui) -> Ui:
  Bool.pick(Ui, String.eq(J.Json.get_str(o, "thread", ""), Ui.sel(u)),
    Hs.pick(u, Hs.row.value(J.Json.get_str(o, "side", "a"), J.Json.get_str(o, "commit", ""), J.Json.get_str(o, "ref", ""), J.Json.get_str(o, "git", ""), J.Json.get_str(o, "top", ""))), u)

def Rv.reply.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "resolved"), J.Null{})), "null"))

def Rv.reply(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Rv.reply.k(J.Json.get_obj(J.Json.get_obj(o, "result"), "resolved"), u)
    case False{}:
      u

# Design review: comments
# -----------------------
# Comments pinned to a place in a design (core/review.bend), the thread's
# own: the person's and its agent's. Every client draws the pins of the
# file on show (Rc.pins, where it draws the board or the schematic) and
# opens one with "rv-pin" (its id; "" shuts it: "@rc.sel:<t>"). A right
# click or a long press on a place asks to comment there: "rv-note" with
# "x\ty\tlayer\titem" (mm in the file's coordinates, the layer's name, the
# key of the item under it), kept as "@rc.at:<t>" while the text is
# written (field "@f.rcnote"); "rv-note-send" sends it (review.comment, op
# add), "rv-note-shut" forgets it. "rv-reply-send" answers the open one
# (field "@f.rcreply"), "rv-resolve" and "rv-reopen" take its id. The
# Review tab's Comments page lists them all.

def Rc.all(+u: Ui) -> List<&2, RVW.Comment>:
  RVW.Review.of_state(Ui.state(u), Ui.sel(u))

# the file on show, from the thread's folder (as the agent names it)
def Rc.rel.k(under: Bool, +abs: String, +root: String) -> String:
  match under:
    case True{}:
      String.drop(abs, 1n+String.length(root))
    case False{}:
      abs

def Rc.at(+u: Ui) -> String:
  Ui.scratch(u, "@rc.at:" ++ Ui.sel(u))

# (a viewer on the project's own board or schematic names no file: the
# hub's history.refs answer does, Rc.sync asks for it once)
def Rc.own(+u: Ui) -> String:
  Ui.scratch(u, Bool.pick(String, Nat.is_eq(Hs.kind(u), 1n), "@rc.sch:", "@rc.pcb:") ++ Ui.sel(u))

def Rc.file(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(HI.Hist.family(Ui.view_get(u, "view.path"))), Rc.own(u), HI.Hist.family(Ui.view_get(u, "view.path")))

def Rc.rel(+u: Ui) -> String:
  +abs = Rc.file(u)
  Rc.rel.k(Bool.and(Bool.not(String.is_empty(Ui.root(u))), String.starts_with(abs, Ui.root(u) ++ "/")), abs, Ui.root(u))

# whether the thread's board or schematic shows with no file named and the
# hub has not been asked which it is (the "asked" mark lives in the info map
# beside the answer, so a phone's kept state never keeps one without the
# other)
def Rc.need(+u: Ui) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(Ui.sel(u))), Bool.and(Nat.is_lt(Hs.kind(u), 2n), Bool.not(String.eq(Ui.view_get(u, "view.kind"), "review")))),
    Bool.and(Bool.and(String.is_empty(HI.Hist.family(Ui.view_get(u, "view.path"))), String.is_empty(Rc.own(u))), String.is_empty(Ui.info(u, "rc.refs:" ++ Ui.sel(u)))))

def Rc.sync.join(a: Act, cs: List<&2, Cmd>) -> Act:
  match a:
    case Act{u, ds}:
      Act{u, List.append(&2, Cmd, cs, ds)}

def Rc.sync.k(need: Bool, a: Act) -> Act:
  match need a:
    case True{} Act{+u, cs}:
      Rc.sync.join(Ui.rpc(Ui.info_set(u, "rc.refs:" ++ Ui.sel(u), "1"), "history.refs", [J.KV{"thread", J.Str{Ui.sel(u)}}]), cs)
    case False{} +x:
      x

# (only for a thread with comments to pin, or a note being started: other
# threads ask nothing more)
# (a phone back online asks at once: its kept state reopens the board with
# no select, and the comments to pin may arrive only with the log)
def Rc.sync.any(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Rc.sync.k(Rc.need(u), Act{u, cs})

def Rc.sync.of(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Rc.sync.k(Bool.and(Rc.need(u), Bool.or(Bool.not(String.is_empty(Rc.at(u))), Bool.not(List.is_empty(&2, RVW.Comment, Rc.all(u))))), Act{u, cs})

# (a note sent before the hub named the project's own file was kept with
# no file: the viewer on that file shows it too)
def Rc.pins.k(own: Bool, +u: Ui) -> List<&2, RVW.Pin>:
  match own:
    case True{}:
      List.append(&2, RVW.Pin, RVW.Review.pins(Rc.all(u), Rc.rel(u), ""), RVW.Review.pins(Rc.all(u), "", ""))
    case False{}:
      RVW.Review.pins(Rc.all(u), Rc.rel(u), "")

def Rc.pins(+u: Ui) -> List<&2, RVW.Pin>:
  Rc.pins.k(Bool.and(String.is_empty(HI.Hist.family(Ui.view_get(u, "view.path"))), Bool.not(String.is_empty(Rc.rel(u)))), u)

def Rc.sel(+u: Ui) -> String:
  Ui.scratch(u, "@rc.sel:" ++ Ui.sel(u))

# the one open: its words, who and where, its replies (each "text\u001fby"),
# whether it is resolved
type RcOpen is Data:
  RcOpen{id: String, text: String, by: String, place: String, resolved: Bool, replies: List<&2, String>}

def Rc.reply.lines(rs: List<&2, RVW.Comment>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      (RVW.Rv.text(r) ++ SCon{Chr{31}, SNil{}} ++ RVW.Rv.by(r)) <> Rc.reply.lines(t)

def Rc.find(cs: List<&2, RVW.Comment>, +id: String) -> Maybe<&2, RVW.Comment>:
  match cs:
    case Nil{}:
      None{}
    case +c <> t:
      Bool.pick(Maybe<&2, RVW.Comment>, String.eq(RVW.Rv.id(c), id), Some{c}, Rc.find(t, id))

def Rc.open.k(m: Maybe<&2, RVW.Comment>, +all: List<&2, RVW.Comment>) -> Maybe<&2, RcOpen>:
  match m:
    case Some{+c}:
      Some{RcOpen{RVW.Rv.id(c), RVW.Rv.text(c), RVW.Rv.by(c), RVW.Rv.where(c), RVW.Rv.resolved(c), Rc.reply.lines(RVW.Review.replies(all, RVW.Rv.id(c)))}}
    case None{}:
      None{}

def Rc.open(+u: Ui) -> Maybe<&2, RcOpen>:
  Rc.open.k(Bool.pick(Maybe<&2, RVW.Comment>, String.is_empty(Rc.sel(u)), None{}, Rc.find(Rc.all(u), Rc.sel(u))), Rc.all(u))

def Rc.count(+u: Ui) -> Nat:
  RVW.Review.count.open(Rc.all(u))

# the version a new comment is made on: a commit's, else the working copy
def Rc.commit(+u: Ui) -> String:
  Bool.pick(String, Nat.is_eq(HI.Hist.kind(Ui.view_get(u, "view.path")), 1n), HI.Hist.git.commit(Ui.view_get(u, "view.path")), "")

# a number as JSON wants it (0 when it is not one)
def Rc.num(+s: String) -> String:
  Bool.pick(String, Maybe.is_some(&2, F32, F32.read(s)), s, "0")

def Rc.send.k(+xs: List<&2, String>, +u: Ui, +text: String) -> Act:
  Ui.rpc(Ui.scratch_set(Ui.scratch_set(u, "@rc.at:" ++ Ui.sel(u), ""), "@f.rcnote", ""), "review.comment",
    [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"op", J.Str{"add"}}, J.KV{"file", J.Str{Rc.rel(u)}}, J.KV{"commit", J.Str{Rc.commit(u)}},
     J.KV{"kind", J.Str{Hs.kname(Hs.kind(u))}}, J.KV{"layer", J.Str{HI.Hist.at(xs, 2n)}}, J.KV{"x", J.Num{Rc.num(HI.Hist.at(xs, 0n))}}, J.KV{"y", J.Num{Rc.num(HI.Hist.at(xs, 1n))}},
     J.KV{"item", J.Str{HI.Hist.at(xs, 3n)}}, J.KV{"text", J.Str{text}}])

def Rc.send(+u: Ui) -> Act:
  Bool.pick(Act, Bool.or(String.is_empty(Rc.at(u)), String.is_empty(String.trim(Ui.scratch(u, "@f.rcnote")))), Act{u, Nil{}},
    Rc.send.k(String.split(Rc.at(u), '\t'), u, String.trim(Ui.scratch(u, "@f.rcnote"))))

def Rc.op(+u: Ui, +op: String, +id: String) -> Act:
  Ui.rpc(u, "review.comment", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"op", J.Str{op}}, J.KV{"id", J.Str{id}}])

def Rc.reply(+u: Ui) -> Act:
  Bool.pick(Act, Bool.or(String.is_empty(Rc.sel(u)), String.is_empty(String.trim(Ui.scratch(u, "@f.rcreply")))), Act{u, Nil{}},
    Ui.rpc(Ui.scratch_set(u, "@f.rcreply", ""), "review.comment", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"op", J.Str{"add"}}, J.KV{"parent", J.Str{Rc.sel(u)}},
      J.KV{"file", J.Str{Rc.rel(u)}}, J.KV{"text", J.Str{String.trim(Ui.scratch(u, "@f.rcreply"))}}]))

def Rc.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Act{Ui.scratch_set(Ui.scratch_set(u, "@rc.at:" ++ Ui.sel(u), value), "@rc.sel:" ++ Ui.sel(u), ""), Nil{}}
    case 1n:
      Rc.send(u)
    case 2n:
      Act{Ui.scratch_set(u, "@rc.at:" ++ Ui.sel(u), ""), Nil{}}
    case 3n:
      Act{Ui.scratch_set(Ui.scratch_set(u, "@rc.sel:" ++ Ui.sel(u), value), "@rc.at:" ++ Ui.sel(u), ""), Nil{}}
    case 4n:
      Rc.reply(u)
    case 5n:
      Rc.op(u, "resolve", value)
    case 6n:
      Rc.op(u, "reopen", value)
    case _:
      Act{u, Nil{}}

def Rc.actions() -> List<&2, String>:
  ["rv-note", "rv-note-send", "rv-note-shut", "rv-pin", "rv-reply-send", "rv-resolve", "rv-reopen"]

# Design review: the Review tab
# ------------------------------
# A viewer kind of its own ("review"), its path the page on show (Rp.pages:
# "bom"). A page is a report the hub writes as Markdown (review.bom: the
# BOM, or what changed between the two versions a comparison picked, kept
# until it is closed), which every client draws with its own Markdown.
# Kept per thread and page in the info map: "rp.md:<t>:<page>",
# "rp.err:<t>:<page>", "rp.title:<t>:<page>"; the page being asked for is
# the scratch's "@rp.busy:<t>". "rv-page" shows a page and asks for it,
# "rv-run" asks again, "rv-bom" (the comparison bar's BOM) opens the BOM.

def Rp.pages() -> List<&2, String>:
  ["bom", "checks", "comments", "release"]

def Rp.label(+l: I.Loc, +page: String) -> String:
  Bool.pick(String, String.eq(page, "bom"), MSG.T.review.bom(l), Bool.pick(String, String.eq(page, "checks"), MSG.T.review.checks(l), Bool.pick(String, String.eq(page, "comments"), MSG.T.review.comments(l), Bool.pick(String, String.eq(page, "release"), MSG.T.review.release(l), page))))

# the page on show ("" when the tab is not)
def Rp.page(+u: Ui) -> String:
  Bool.pick(String, String.eq(Ui.view_get(u, "view.kind"), "review"), Bool.pick(String, String.is_empty(Ui.view_get(u, "view.path")), "bom", Ui.view_get(u, "view.path")), "")

def Rp.key(+u: Ui, +k: String, +page: String) -> String:
  k ++ ":" ++ Ui.sel(u) ++ ":" ++ page

# a comparison picked (and not closed) is what the BOM page compares
def Rp.compared(+u: Ui) -> Bool:
  Bool.not(String.is_empty(Hs.get(u, "cmp.a")))

# Reading positions are separate for each report and board/comparison.
# The host supplies only pixel coordinates under this identity.
def Rp.place(+u: Ui) -> String:
  J.Json.show(J.Json.arr([J.Str{Rp.page(u)}, J.Str{Bd.id(u)}, J.Str{Hs.get(u, "cmp.a")}, J.Str{Hs.get(u, "cmp.b")}]))

def Rp.bom.title.l(+l: I.Loc, +u: Ui) -> String:
  +board = Bd.id(u)
  Bool.pick(String, Rp.compared(u),
    MSG.T.review.bom.change(l, Hs.label.shown(l, Hs.get(u, "cmp.al")), Hs.label.shown(l, Bool.pick(String, String.is_empty(Hs.get(u, "cmp.bl")), "Working copy", Hs.get(u, "cmp.bl")))),
    MSG.T.review.bom.working(l) ++ Bool.pick(String, Bool.and(Bool.not(String.is_empty(board)), Bool.or(Bd.many(u), Bool.not(String.is_empty(Ui.view_get(u, "view.tree"))))), " · " ++ Bd.name(board), ""))

def Rp.bom.title(+u: Ui) -> String:
  Rp.bom.title.l(Lang.loc(u), u)

def Rp.bom.root(+u: Ui) -> String:
  Bd.tree(u, Bd.context(u))

def Rp.ask.bom(+u: Ui) -> Act:
  Ui.rpc(Ui.info_set(Ui.scratch_set(u, "@rp.busy:" ++ Ui.sel(u), "bom"), Rp.key(u, "rp.title", "bom"), Rp.bom.title(u)), "review.bom",
    [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"a", J.Str{Bool.pick(String, Rp.compared(u), Hs.get(u, "cmp.a"), "")}}, J.KV{"b", J.Str{Bool.pick(String, Rp.compared(u), Hs.get(u, "cmp.b"), Rp.bom.root(u))}}])

# the design checks run on the working copy, or on the new side of a
# comparison when that is a commit
def Rp.commit(+u: Ui) -> String:
  Bool.pick(String, Nat.is_eq(HI.Hist.kind(Hs.get(u, "cmp.b")), 1n), HI.Hist.git.commit(Hs.get(u, "cmp.b")), "")

def Rp.checks.title.l(+l: I.Loc, +u: Ui) -> String:
  MSG.T.review.checks.of(l, Hs.label.shown(l, Bool.pick(String, String.is_empty(Rp.commit(u)), "Working copy", Hs.get(u, "cmp.bl"))))

def Rp.checks.title(+u: Ui) -> String:
  Rp.checks.title.l(Lang.loc(u), u)

def Rp.ask.checks(+u: Ui) -> Act:
  Ui.rpc(Ui.info_set(Ui.scratch_set(u, "@rp.busy:" ++ Ui.sel(u), "checks"), Rp.key(u, "rp.title", "checks"), Rp.checks.title(u)), "review.checks",
    [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"commit", J.Str{Rp.commit(u)}}, J.KV{"trigger", J.Str{"manual"}}])

# The Release page: a tag (the field "@f.rltag", else the next one after
# the repository's tags, REL.Release.next over history.refs' tags), whether
# to publish it on GitHub ("@rl.pub:<t>") and to release even when the
# checks fail ("@rl.force:<t>"); "rv-release" asks the hub (review.release),
# "rv-release-pub" and "rv-release-force" turn those on and off
type RlForm is Data:
  RlForm{tag: String, next: String, publish: Bool, force: Bool}

def Rl.tags.k(tag: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match tag:
    case True{}:
      l <> rest
    case False{}:
      rest

def Rl.tags(rs: List<&2, HI.Ref>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case HI.Ref{+k, +l, _} <> t:
      Rl.tags.k(String.eq(k, "tag"), l, Rl.tags(t))

def Rl.next(+u: Ui) -> String:
  REL.Release.next(Rl.tags(HI.Hist.refs(J.Json.get_str(Hs.refs(u), "refs", ""))))

def Rl.tag(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(String.trim(Ui.scratch(u, "@f.rltag"))), Rl.next(u), String.trim(Ui.scratch(u, "@f.rltag")))

def Rl.on(+u: Ui, +k: String) -> Bool:
  String.eq(Ui.scratch(u, k ++ ":" ++ Ui.sel(u)), "1")

def Rl.form(+u: Ui) -> RlForm:
  RlForm{Rl.tag(u), Rl.next(u), Rl.on(u, "@rl.pub"), Rl.on(u, "@rl.force")}

def Rl.flip(+u: Ui, +k: String) -> Ui:
  Ui.scratch_set(u, k ++ ":" ++ Ui.sel(u), Bool.pick(String, Rl.on(u, k), "", "1"))

def Rl.ask(+u: Ui) -> Act:
  Ui.rpc(Ui.info_set(Ui.scratch_set(u, "@rp.busy:" ++ Ui.sel(u), "release"), Rp.key(u, "rp.err", "release"), ""), "review.release",
    [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"tag", J.Str{Rl.tag(u)}}, J.KV{"publish", J.Flag{Rl.on(u, "@rl.pub")}}, J.KV{"force", J.Flag{Rl.on(u, "@rl.force")}}])

# opening the page asks for the tags once (the next one is proposed)
def Rl.open(+u: Ui) -> Act:
  Bool.pick(Act, String.is_empty(Hs.get(u, "refs")), Ui.rpc(u, "history.refs", [J.KV{"thread", J.Str{Ui.sel(u)}}]), Act{u, Nil{}})

def Rp.ask(+u: Ui, +page: String) -> Act:
  Bool.pick(Act, String.eq(page, "bom"), Rp.ask.bom(u), Bool.pick(Act, String.eq(page, "checks"), Rp.ask.checks(u), Bool.pick(Act, String.eq(page, "release"), Rl.open(u), Act{u, Nil{}})))

def Rp.open(+u: Ui, +page: String) -> Act:
  Rp.ask(Ui.view_put(u, "review", page), page)

# A restored Review page owes its first read once in this view. Mark the
# attempt before sending so an empty report or an error never loops.
def Rp.sync.k(go: Bool, +u: Ui, +page: String, cs: List<&2, Cmd>) -> Act:
  match go:
    case True{}:
      Ui.also(Rp.ask(Ui.scratch_set(u, "@rp.at:" ++ Ui.sel(u), page), page), cs)
    case False{}: Act{u, cs}

def Rp.sync(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      +page = Rp.page(u)
      # A reload can read the report before fab.look supplies its board list.
      # Once that list resolves the saved drawing, read the chosen board.
      +changed = Bool.and(String.eq(page, "bom"), Bool.not(String.eq(Ui.info(u, Rp.key(u, "rp.title", page)), Rp.bom.title(u))))
      Rp.sync.k(Bool.and(Ui.panel(u), Bool.and(Bool.not(String.is_empty(page)), Bool.and(String.is_empty(Ui.scratch(u, "@rp.busy:" ++ Ui.sel(u))),
        Bool.or(changed, Bool.and(String.is_empty(Ui.info(u, Rp.key(u, "rp.title", page))), Bool.not(String.eq(Ui.scratch(u, "@rp.at:" ++ Ui.sel(u)), page))))))), u, page, cs)

# the hub's answer: {report: {thread, page, md, err}}, a check's with
# {gate, line, tasks} too
def Rp.store(+u: Ui, +r: J.Json) -> Ui:
  +k = J.Json.get_str(r, "thread", "") ++ ":" ++ J.Json.get_str(r, "page", "")
  Ui.scratch_set(Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(Ui.info_set(u, "rp.md:" ++ k, J.Json.get_str(r, "md", "")), "rp.err:" ++ k, J.Json.get_str(r, "err", "")),
    "rp.gate:" ++ k, J.Json.get_str(r, "gate", "")), "rp.line:" ++ k, J.Json.get_str(r, "line", "")), "rp.tasks:" ++ k, J.Json.show(Maybe.default(&2, J.Json, J.Json.get(r, "tasks"), J.Json.arr(Nil{})))),
    "@rp.busy:" ++ J.Json.get_str(r, "thread", ""), "")

# a box ticked: {ticked: {thread, tasks}}
def Rp.ticked(+u: Ui, +r: J.Json) -> Ui:
  Ui.info_set(u, "rp.tasks:" ++ J.Json.get_str(r, "thread", "") ++ ":checks", J.Json.show(Maybe.default(&2, J.Json, J.Json.get(r, "tasks"), J.Json.arr(Nil{}))))

def Rp.ticked.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "ticked"), J.Null{})), "null"))

def Rp.ticked.k(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Rp.ticked(u, J.Json.get_obj(J.Json.get_obj(o, "result"), "ticked"))
    case False{}:
      u

def Rp.reply.has(+o: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(o, "result"), "report"), J.Null{})), "null"))

def Rp.reply.k(has: Bool, u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Rp.store(u, J.Json.get_obj(J.Json.get_obj(o, "result"), "report"))
    case False{}:
      u

def Rp.reply(has: Bool, u: Ui, +o: J.Json) -> Ui:
  Rp.ticked.k(Rp.ticked.has(o), Rp.reply.k(has, u, o), o)

# The checks page: why the gate fails ("" it passes), the one-line summary,
# and the checklist's boxes (each ticked with "rv-tick" and its index)
type RpTask is Data:
  RpTask{index: Nat, text: String, done: Bool, group: String}

def Rp.gate(+u: Ui) -> String:
  Ui.info(u, Rp.key(u, "rp.gate", "checks"))

def Rp.line(+u: Ui) -> String:
  Ui.info(u, Rp.key(u, "rp.line", "checks"))

def Rp.task.list(xs: List<&2, J.Json>, +i: Nat) -> List<&2, RpTask>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      RpTask{i, J.Json.get_str(x, "text", ""), J.Json.get_bool(x, "done", False{}), J.Json.get_str(x, "group", "")} <> Rp.task.list(t, 1n+i)

def Rp.tasks(+u: Ui) -> List<&2, RpTask>:
  Rp.task.list(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, Rp.key(u, "rp.tasks", "checks"))), J.Json.arr(Nil{}))), 0n)

def Rp.tick(+u: Ui, +i: String) -> Act:
  Ui.rpc(u, "review.tick", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"index", J.Num{Nat.show(Maybe.default(&2, Nat, Nat.read(i), 0n))}}])

# what every client draws: the pages (their labels and which is on), the
# title, the report's Markdown, why it failed, whether it is being made
type RpTab is Data:
  RpTab{page: String, label: String, on: Bool}

type RpView is Data:
  RpView{page: String, tabs: List<&2, RpTab>, title: String, md: String, err: String, busy: Bool}

def Rp.tabs(ps: List<&2, String>, +l: I.Loc, +cur: String) -> List<&2, RpTab>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      RpTab{p, Rp.label(l, p), String.eq(p, cur)} <> Rp.tabs(t, l, cur)

# (the Comments page is the thread's own: no hub report)
def Rp.view.k(+l: I.Loc, comments: Bool, +u: Ui, +page: String) -> RpView:
  match comments:
    case True{}:
      RpView{page, Rp.tabs(Rp.pages(), l, page), MSG.T.review.comments.open(l, Nat.show(Rc.count(u))), RVW.Review.md(Rc.all(u)), "", False{}}
    case False{}:
      RpView{page, Rp.tabs(Rp.pages(), l, page), Bool.pick(String, String.eq(page, "release"), MSG.T.review.tag(l, Rl.tag(u)), Ui.info(u, Rp.key(u, "rp.title", page))), Ui.info(u, Rp.key(u, "rp.md", page)), Ui.info(u, Rp.key(u, "rp.err", page)),
        String.eq(Ui.scratch(u, "@rp.busy:" ++ Ui.sel(u)), page)}

def Rp.view.loc(+l: I.Loc, +u: Ui) -> RpView:
  Rp.view.k(l, String.eq(Rp.page(u), "comments"), u, Rp.page(u))

def Rp.view(+u: Ui) -> RpView:
  Rp.view.loc(I.En{}, u)

def RpView.page(v: RpView) -> String:
  match v:
    case RpView{p, _, _, _, _, _}:
      p

def RpView.md(v: RpView) -> String:
  match v:
    case RpView{_, _, _, md, _, _}:
      md

def RpView.title(v: RpView) -> String:
  match v:
    case RpView{_, _, t, _, _, _}:
      t

def RpView.err(v: RpView) -> String:
  match v:
    case RpView{_, _, _, _, e, _}:
      e

def RpView.busy(v: RpView) -> Bool:
  match v:
    case RpView{_, _, _, _, _, b}:
      b

def RpView.tabs(v: RpView) -> List<&2, RpTab>:
  match v:
    case RpView{_, ts, _, _, _, _}:
      ts

# what the page says while it has nothing to show
def RpView.note.loc(+l: I.Loc, +v: RpView) -> String:
  Bool.pick(String, RpView.busy(v), MSG.T.review.reading(l),
    Bool.pick(String, String.is_empty(RpView.err(v)), Bool.pick(String, String.is_empty(RpView.md(v)), Bool.pick(String, String.eq(RpView.page(v), "comments"), MSG.T.review.comments.none(l),
      Bool.pick(String, String.eq(RpView.page(v), "release"), MSG.T.review.release.note(l), MSG.T.review.empty(l))), ""), RpView.err(v)))

def RpView.note(+v: RpView) -> String:
  RpView.note.loc(I.En{}, v)

def Rv.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Act{Rv.as(u, value), Nil{}}
    case 1n:
      Act{Rv.bits.set(u, value), Nil{}}
    case 2n:
      Rv.ref(u, String.split(value, '\t'))
    case 3n:
      Rp.open(u, Bool.pick(String, String.is_empty(value), "bom", value))
    case 4n:
      Rp.ask(u, Rp.page(u))
    case 5n:
      Rp.open(u, "bom")
    case 6n:
      Rp.tick(u, value)
    case 7n:
      Rl.ask(u)
    case 8n:
      Act{Rl.flip(u, "@rl.pub"), Nil{}}
    case 9n:
      Act{Rl.flip(u, "@rl.force"), Nil{}}
    case _:
      Act{u, Nil{}}

def Rv.act.c(comment: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match comment:
    case True{}:
      Rc.act(C.Codec.pos(Rc.actions(), action), u, value)
    case False{}:
      Rv.act.k(C.Codec.pos(["rv-as", "rv-bits", "rv-ref", "rv-page", "rv-run", "rv-bom", "rv-tick", "rv-release", "rv-release-pub", "rv-release-force"], action), u, value)

def Rv.act(+u: Ui, +action: String, +value: String) -> Act:
  Rv.act.c(Nat.is_lt(C.Codec.pos(Rc.actions(), action), 7n), u, action, value)

def Ui.reply.skills(u: Ui, +o: J.Json) -> Ui:
  Bd.reply(Bd.has(o), Rp.reply(Rp.reply.has(o), Rv.reply(Rv.reply.has(o), Fv.reply(Fm.has(J.Json.get_obj(o, "result"), "farView"), Skills.got(Skills.has(o), Mpage.reply(Mpage.has(o), Hs.refs.store(Hs.refs.has(o), Ui.reply.body(u, o), o), o), o), J.Json.get_obj(o, "result")), o), o), o)

# The bar
# -------
# (shown under every board and schematic, so any of them can be compared
# with another version: docs/design-review.md)
# What every client's viewer shows under a board or schematic with a
# history (or a comparison on show): on, the track's length and the
# position (0 live), what the step followed and what was said before it.

type HBar is Data:
  HBar{on: Bool, n: Nat, at: Nat, title: String, entry: String, label: String, said: String, playing: Bool, diff: Bool, cmp: Bool, a: String, b: String, menu: String}

# what the bar says while the live file shows, every client the same
def Hs.live.said.loc(+l: I.Loc, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), MSG.T.history.live.note(l), MSG.T.history.live.ticks(l))

def Hs.live.said(+n: Nat) -> String:
  Hs.live.said.loc(I.En{}, n)

def Hs.said.line(+s: String) -> String:
  HI.Hist.line(s)

def Hs.bar.pos(+l: I.Loc, first: Bool, +at: Nat, +n: Nat) -> String:
  match first:
    case True{}: MSG.T.history.position.first(l, Nat.show(at), Nat.show(n))
    case False{}: MSG.T.history.position(l, Nat.show(at), Nat.show(n))

def Hs.bar.k(+l: I.Loc, m: Maybe<&2, HI.Step>, +u: Ui, +n: Nat, +at: Nat, +es: List<&2, M.Entry>, +c: Nat) -> HBar:
  match m:
    case Some{+s}:
      HBar{True{}, n, at, Hs.bar.pos(l, HI.Step.first(s), at, n),
        HI.Hist.anchor(es, s), HI.Hist.label(es, HI.Hist.anchor(es, s)), Hs.said.line(HI.Hist.said(es, HI.Hist.anchor(es, s))),
        Hs.playing(u), Hs.diff(u), False{}, "", "", Ui.scratch(u, "@cmp.menu")}
    case None{}:
      HBar{True{}, n, 0n, Bool.pick(String, Hs.cmp(u), MSG.T.history.comparing(l), Bool.pick(String, Nat.is_eq(c, 0n), MSG.T.history.live(l), MSG.T.history.live.steps(l, Nat.show(c), Bool.pick(String, Nat.is_eq(c, 1n), "one", "other")))),
        "", "", "", Hs.playing(u), Hs.diff(u), Hs.cmp(u), Hs.label.shown(l, Hs.get(u, "cmp.al")), Hs.label.shown(l, Bool.pick(String, String.is_empty(Hs.get(u, "cmp.bl")), "Working copy", Hs.get(u, "cmp.bl"))), Ui.scratch(u, "@cmp.menu")}

def HBar.on(b: HBar) -> Bool:
  match b:
    case HBar{on, _, _, _, _, _, _, _, _, _, _, _, _}:
      on

def HBar.cmp(b: HBar) -> Bool:
  match b:
    case HBar{_, _, _, _, _, _, _, _, _, c, _, _, _}:
      c

# how much of the window's viewer the bar takes (layout.bend's Hb.draw;
# a board is fitted above it)
def HBar.h(+b: HBar) -> U32:
  Bool.pick(U32, HBar.on(b), Bool.pick(U32, HBar.cmp(b), 72, 96), 0)

# how long a change takes to fade in, every viewer the same (the window's
# V.Fade.ms)
def Hs.fade() -> Nat:
  220n

# (the track made once: each Hs.track parses every step's record)
def Hs.bar.t(+l: I.Loc, +tr: List<&2, HI.Step>, +u: Ui) -> HBar:
  Hs.bar.k(l, HI.Hist.nth(tr, Hs.at(u, tr)), u, Hs.n(tr), Hs.at(u, tr), M.State.mine(Ui.state(u), Ui.sel(u)), HI.Hist.track.changes(tr, M.State.steps(Ui.state(u), Ui.sel(u))))

def Hs.bar.on(+l: I.Loc, on: Bool, +u: Ui) -> HBar:
  match on:
    case True{}:
      Hs.bar.t(l, Hs.track(u), u)
    case False{}:
      HBar{False{}, 0n, 0n, "", "", "", "", False{}, False{}, False{}, "", "", ""}

# the bar for the selected thread's viewer when it shows (on): off unless
# a board or a schematic shows
def Hs.bar.for.loc(+l: I.Loc, +u: Ui, on: Bool) -> HBar:
  Hs.bar.on(l, Bool.and(on, Bool.and(Nat.is_lt(Hs.kind(u), 2n), Bool.not(String.is_empty(Ui.sel(u))))), u)

# (the window's and the web's: while the panel is open)
def Hs.bar.for(+u: Ui, on: Bool) -> HBar:
  Hs.bar.for.loc(Lang.loc(u), u, on)

def Hs.bar.loc(+l: I.Loc, +u: Ui) -> HBar:
  Hs.bar.for.loc(l, u, Ui.panel(u))

def Hs.bar(+u: Ui) -> HBar:
  Hs.bar.loc(Lang.loc(u), u)

# Actions (Ui.actions from "hist-go")
def Hs.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Act{Hs.go(u, Hs.track(u), HI.Hist.index(Hs.track(u), value)), Nil{}}
    case 1n:
      Act{Hs.step(False{}, u), Nil{}}
    case 2n:
      Act{Hs.step(True{}, u), Nil{}}
    case 3n:
      Act{Hs.go(u, Hs.track(u), 1n), Nil{}}
    case 4n:
      Act{Hs.go(u, Hs.track(u), Hs.n(Hs.track(u))), Nil{}}
    case 5n:
      Hs.play(u)
    case 6n:
      Hs.tick.on(u, value)
    case 7n:
      Act{Hs.diff.flip(u), Nil{}}
    case 8n:
      Act{Hs.live(u), Nil{}}
    case 9n:
      Hs.jump(u, value)
    case 10n:
      Act{Hs.show.id(u, value), Nil{}}
    case 11n:
      Hs.menu.k(Bool.not(String.is_empty(Ui.scratch(u, "@cmp.menu"))), u, Bool.pick(String, String.is_empty(value), "a", value))
    case 12n:
      Act{Hs.pick(u, value), Nil{}}
    case 13n:
      Act{Hs.live(u), Nil{}}
    case 14n:
      Hs.menu.open(u, Bool.pick(String, String.eq(value, "b"), "b", "a"))
    case _:
      Act{Ui.scratch_set(u, "@cmp.menu", ""), Nil{}}

# a phone's upload over HTTP: it starts (value: {"name", "size"}; one over
# the cap is refused here, and the phone sends nothing, as the screen's
# "cap" says), and the hub answered (value: its JSON, as an attach.put's)
# API keys (core/apikeys.bend): Settings keeps one ("key-add", from the
# fields @f.kname and @f.kval; the secret leaves the client's state as it
# is sent), forgets one ("key-del" with its name: the first asks, "@keys.rm"
# holding the name, the same again forgets it), and lists the names the
# hub answers (@keys; opening Settings asks). Flags "keyform" and "keys"
# open the form and the list; a key kept shuts the form and opens the list,
# so its name shows.
def ApiKeys.names(+u: Ui) -> List<&2, String>:
  AK.Keys.keep(String.lines(Ui.scratch(u, "@keys")))

def ApiKeys.add(+u: Ui) -> Act:
  Voice.rpc(Ui.flag_on(Ui.flag_off(Ui.scratch_set(Ui.scratch_set(u, "@f.kval", ""), "@f.kname", ""), "keyform"), "keys"), "api-add",
    [J.KV{"name", J.Str{Ui.scratch(u, "@f.kname")}}, J.KV{"value", J.Str{Ui.scratch(u, "@f.kval")}}])

# whether name's Delete was pressed once (its button asks to confirm)
def ApiKeys.arming(+u: Ui, +name: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(name)), String.eq(Ui.scratch(u, "@keys.rm"), name))

def ApiKeys.del.k(again: Bool, +u: Ui, +name: String) -> Act:
  match again:
    case True{}:
      Voice.rpc(Ui.scratch_set(u, "@keys.rm", ""), "api-del", [J.KV{"name", J.Str{name}}])
    case False{}:
      Act{Ui.scratch_set(u, "@keys.rm", name), Nil{}}

def ApiKeys.del(+u: Ui, +name: String) -> Act:
  ApiKeys.del.k(ApiKeys.arming(u, name), u, name)

# The API key's sealed form ("key"): the name and the value ("<name>
# \u001f<value>") into the api-add request ({"rpc", "name"}, or {"error"}:
# Keys.bad). Once the hub has it, the form shuts and the names are asked
# for again (ApiKeys.sent).
def ApiKeys.req.k(+bad: String, +name: String, +value: String) -> String:
  Bool.pick(String, String.is_empty(bad),
    J.Json.show(J.Json.obj([J.KV{"rpc", J.Str{Rpc.text(0n, "bots.voice", [J.KV{"op", J.Str{"api-add"}}, J.KV{"name", J.Str{name}}, J.KV{"value", J.Str{value}}])}}, J.KV{"name", J.Str{AK.Keys.name(name)}}])),
    J.Json.show(J.Json.obj([J.KV{"error", J.Str{bad}}])))

def ApiKeys.req.ps(ps: List<&2, String>) -> String:
  match ps:
    case +n <> +v <> _:
      ApiKeys.req.k(AK.Keys.bad(n, v), n, v)
    case _:
      J.Json.show(J.Json.obj([J.KV{"error", J.Str{AK.Keys.bad("", "")}}]))

def ApiKeys.req(+packed: String) -> String:
  ApiKeys.req.ps(String.split(packed, Chr{31}))

def ApiKeys.sent(+u: Ui) -> Act:
  Voice.rpc(Ui.flag_on(Ui.flag_off(Ui.scratch_set(u, "@f.kname", ""), "keyform"), "keys"), "status", Nil{})

# what Settings says about them (every client)
def ApiKeys.note.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, List.is_empty(&2, String, ApiKeys.names(u)), MSG.T.apikeys.none(l),
    MSG.T.apikeys.kept(l, Nat.show(List.length(&2, String, ApiKeys.names(u)))))

def ApiKeys.note(+u: Ui) -> String:
  ApiKeys.note.loc(Lang.loc(u), u)

# The secret box (core/secret.bend, docs/secrets.md), every client
# ------------------------------------------------------------------
# A masked box where the person pastes a key for an agent. An agent's
# secret_request is the setting secret.ask.<name> (a card in its thread),
# "/secret [name]" or the header's key opens one in any chat, and Settings
# lists the vault (@secrets, @vault: the hub's secret.list answer, names
# and metadata only). The value never enters this state: the box's field
# has no binding, and at Save the host reads it once and sends the request
# Sbox.req builds straight to the hub (never a view action, a draft or the
# outbox). Scratch: @sbox the name being set ("+" a new one, "" shut),
# @sbox.th the thread it was opened in, @sbox.err, @sbox.rm (a delete
# asked once), @f.sname / @f.spurpose the name and purpose fields.

type SAsk is Data:
  SAsk{name: String, purpose: String, thread: String}

def Sbox.ask.k(+name: String, +o: J.Json) -> SAsk:
  SAsk{name, J.Json.get_str(o, "purpose", ""), J.Json.get_str(o, "thread", "")}

def Sbox.ask.of(+st: M.State, +key: String) -> SAsk:
  Sbox.ask.k(String.drop(key, 11n), Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(st, key, "")), J.Null{}))

def Sbox.asks.add(none: Bool, +st: M.State, +k: String, acc: List<&2, SAsk>) -> List<&2, SAsk>:
  match none:
    case True{}:
      acc
    case False{}:
      Sbox.ask.of(st, k) <> acc

def Sbox.asks.go(ks: List<&2, String>, +st: M.State, acc: List<&2, SAsk>) -> List<&2, SAsk>:
  match ks:
    case Nil{}:
      List.reverse(&2, SAsk, acc)
    case +k <> t:
      Sbox.asks.go(t, st, Sbox.asks.add(String.is_empty(M.Setting.get(st, k, "")), st, k, acc))

# the open requests, oldest key first
def Sbox.asks(+st: M.State) -> List<&2, SAsk>:
  Sbox.asks.go(M.Setting.keys.pre(st, "secret.ask."), st, Nil{})

def SAsk.name(a: SAsk) -> String:
  match a:
    case SAsk{n, _, _}:
      n

def SAsk.thread(a: SAsk) -> String:
  match a:
    case SAsk{_, _, t}:
      t

def SAsk.purpose(a: SAsk) -> String:
  match a:
    case SAsk{_, p, _}:
      p

def Sbox.ask.in(xs: List<&2, SAsk>, +thread: String) -> Maybe<&2, SAsk>:
  match xs:
    case Nil{}:
      None{}
    case +a <> t:
      Bool.pick(Maybe<&2, SAsk>, String.eq(SAsk.thread(a), thread), Some{a}, Sbox.ask.in(t, thread))

def Sbox.ask.named(xs: List<&2, SAsk>, +name: String) -> Maybe<&2, SAsk>:
  match xs:
    case Nil{}:
      None{}
    case +a <> t:
      Bool.pick(Maybe<&2, SAsk>, String.eq(SAsk.name(a), name), Some{a}, Sbox.ask.named(t, name))

def Sbox.title.k(t: M.Thread) -> String:
  match t:
    case M.Thread{id, _, +title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(title), id, title)

# who asked: the asking thread's title
def Sbox.who(+st: M.State, +thread: String) -> String:
  Sbox.title.k(M.State.thread.get(st, thread))

# The box, as every client draws it: the name (editable when new), the
# purpose, who asks ("" when the person opened it), the thread it belongs
# to, any error
type SBox is Data:
  SBox{name: String, purpose: String, who: String, thread: String, fresh: Bool, err: String}

def Sbox.box.ask(m: Maybe<&2, SAsk>, +st: M.State, +err: String) -> Maybe<&2, SBox>:
  match m:
    case Some{SAsk{+n, p, +t}}:
      Some{SBox{n, p, Sbox.who(st, t), t, False{}, err}}
    case None{}:
      None{}

def Sbox.box.open(+st: M.State, +dr: Map<&2, String>, +name: String) -> SBox:
  SBox{Bool.pick(String, String.eq(name, "+"), Str.get(dr, "@f.sname"), name),
    Bool.pick(String, String.eq(name, "+"), Str.get(dr, "@f.spurpose"), Maybe.default(&2, String, Maybe.map(&2, SAsk, String, SAsk.purpose, Sbox.ask.named(Sbox.asks(st), name)), Str.get(dr, "@f.spurpose"))),
    "", Str.get(dr, "@sbox.th"), String.eq(name, "+"), Str.get(dr, "@sbox.err")}

def Sbox.box.k(shut: Bool, +st: M.State, +dr: Map<&2, String>, +thread: String) -> Maybe<&2, SBox>:
  match shut:
    case True{}:
      Sbox.box.ask(Sbox.ask.in(Sbox.asks(st), thread), st, Str.get(dr, "@sbox.err"))
    case False{}:
      Bool.pick(Maybe<&2, SBox>, String.eq(Str.get(dr, "@sbox.th"), thread), Some{Sbox.box.open(st, dr, Str.get(dr, "@sbox"))}, None{})

# the box for a thread's chat, or for Settings (thread ""): the one the
# person opened there, else the thread's oldest request (st: the state,
# dr: the client's scratch, where the box's own fields live)
def Sbox.box.d(+st: M.State, +dr: Map<&2, String>, +thread: String) -> Maybe<&2, SBox>:
  Sbox.box.k(String.is_empty(Str.get(dr, "@sbox")), st, dr, thread)

def Sbox.dr(u: Ui) -> Map<&2, String>:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      dr

def Sbox.box(+u: Ui, +thread: String) -> Maybe<&2, SBox>:
  Sbox.box.d(Ui.state.of(u), Sbox.dr(u), thread)

# a request's row in Settings: its name, who asked and why
def Sbox.ask.note(+st: M.State, a: SAsk) -> String:
  match a:
    case SAsk{_, +p, +t}:
      Sbox.who(st, t) ++ Bool.pick(String, String.is_empty(p), "", " · " ++ p)

# the request the host sends at Save, built from "<name>\u001f<purpose>\u001f
# <thread>\u001f<value>": {"rpc": text} or {"error": why}. The value is
# checked here (Sec.bad) and goes nowhere else.
def Sbox.req.k(+bad: String, +name: String, +purpose: String, +thread: String, +value: String) -> String:
  Bool.pick(String, String.is_empty(bad),
    J.Json.show(J.Json.obj([J.KV{"rpc", J.Str{Rpc.text(0n, "secret.set", [J.KV{"name", J.Str{SEC.Sec.name(name)}}, J.KV{"purpose", J.Str{purpose}}, J.KV{"thread", J.Str{thread}}, J.KV{"value", J.Str{String.trim(value)}}])}}, J.KV{"name", J.Str{SEC.Sec.name(name)}}])),
    J.Json.show(J.Json.obj([J.KV{"error", J.Str{bad}}])))

def Sbox.req.ps(ps: List<&2, String>) -> String:
  match ps:
    case +n <> +p <> +t <> +v <> _:
      Sbox.req.k(SEC.Sec.bad(n, v), n, p, t, v)
    case _:
      J.Json.show(J.Json.obj([J.KV{"error", J.Str{"Paste the secret"}}]))

def Sbox.req(+packed: String) -> String:
  Sbox.req.ps(String.split(packed, Chr{31}))

def Sbox.list(+u: Ui) -> Act:
  Ui.rpc(u, "secret.list", Nil{})

# open the box: value a name (Replace, a request's Set) or "" (a new one)
def Sbox.open(+u: Ui, +name: String, +thread: String) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@sbox", Bool.pick(String, String.is_empty(name), "+", name)), "@sbox.th", thread), "@sbox.err", ""),
    "@f.sname", name), Nil{}}

def Sbox.open.a(a: Act, +name: String, +thread: String) -> Act:
  match a:
    case Act{u, cs}:
      Ui.also(Sbox.open(u, name, thread), cs)

def Ui.send.sec(sec: Bool, +u: Ui, alt: Bool) -> Act:
  match sec:
    case True{}:
      Sbox.open.a(Draft.clear(u), RF.Refs.token(String.trim(String.drop(String.trim(Ui.draft_text(u)), 7n))), Ui.sel(u))
    case False{}:
      Ui.send.intent(OR.Intent.is(Ui.draft_text(u)), u, alt)

def Ui.send(+u: Ui, alt: Bool) -> Act:
  Ui.send.sec(Sbox.is(Ui.draft_text(u)), u, alt)

def Sbox.shut.u(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@sbox", ""), "@sbox.err", ""), "@f.sname", ""), "@f.spurpose", "")

# after the host sent a value: the box shuts and the list is asked again
def Sbox.sent(+u: Ui) -> Act:
  Sbox.list(Sbox.shut.u(u))

def Sbox.del.k(again: Bool, +u: Ui, +name: String) -> Act:
  match again:
    case True{}:
      Ui.rpc(Ui.scratch_set(u, "@sbox.rm", ""), "secret.del", [J.KV{"name", J.Str{name}}])
    case False{}:
      Act{Ui.scratch_set(u, "@sbox.rm", name), Nil{}}

def Sbox.dismiss(+u: Ui, +name: String) -> Act:
  Ui.rpc(Sbox.shut.u(u), "secret.dismiss", [J.KV{"name", J.Str{name}}])

# (phones: a request's Set opens the box in Settings, where the masked
# field is)
def Sbox.open.set(+u: Ui, +name: String) -> Act:
  Sbox.list(Settings.on(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@sbox", Bool.pick(String, String.is_empty(name), "+", name)), "@sbox.th", ""), "@sbox.err", ""), "@f.sname", name)))

# "secret-" actions: open (value: a name or ""), open-here (in the
# selected thread), shut, sent, err (value: why), del (asks once), dismiss,
# list
def Sbox.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Sbox.open(u, value, "")
    case 1n:
      Sbox.open(u, value, Ui.sel(u))
    case 2n:
      Act{Sbox.shut.u(u), Nil{}}
    case 3n:
      Sbox.sent(u)
    case 4n:
      Act{Ui.scratch_set(u, "@sbox.err", value), Nil{}}
    case 5n:
      Sbox.del.k(String.eq(Ui.scratch(u, "@sbox.rm"), value), u, value)
    case 6n:
      Sbox.dismiss(u, value)
    case 8n:
      Sbox.open.set(u, value)
    case _:
      Sbox.list(u)

def Sbox.act(+u: Ui, +action: String, +value: String) -> Act:
  Sbox.act.k(C.Codec.pos(["secret-open", "secret-open-here", "secret-shut", "secret-sent", "secret-err", "secret-del", "secret-dismiss", "secret-list", "secret-open-set"], action), u, value)

# Settings' rows: the vault's metadata as the hub answered it
def Sbox.metas(+u: Ui) -> List<&2, SEC.SecMeta>:
  SEC.Sec.metas(Ui.scratch(u, "@secrets"))

def Sbox.vault(+u: Ui) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Ui.scratch(u, "@vault")), J.Null{})

# what Settings says about the vault
def Sbox.note(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, J.Json.get_bool(Sbox.vault(u), "locked", False{}), MSG.T.secret.vault.locked(l, J.Json.get_str(Sbox.vault(u), "why", "")),
    Bool.pick(String, List.is_empty(&2, SEC.SecMeta, Sbox.metas(u)), MSG.T.secret.none(l), MSG.T.secret.kept(l, Nat.show(List.length(&2, SEC.SecMeta, Sbox.metas(u))))
      ++ " " ++ Bool.pick(String, String.eq(J.Json.get_str(Sbox.vault(u), "key", "file"), "file"), MSG.T.secret.vault.file(l), MSG.T.secret.vault.keyring(l))))

# a stored secret's row: its name, its purpose and last use, and whether
# Delete was asked once
type SRowV is Data:
  SRowV{name: String, note: String, rm: Bool}

def Sbox.row(+l: I.Loc, +u: Ui, m: SEC.SecMeta) -> SRowV:
  match m:
    case SEC.SecMeta{+n, +p, _, _, +ua, _}:
      SRowV{n, Bool.pick(String, String.is_empty(p), "", p ++ " · ") ++ Bool.pick(String, String.is_empty(ua), MSG.T.secret.unused(l), MSG.T.secret.used(l, String.take(ua, 16n))), String.eq(Ui.scratch(u, "@sbox.rm"), n)}

def Sbox.rows.go(ms: List<&2, SEC.SecMeta>, +l: I.Loc, +u: Ui, acc: List<&2, SRowV>) -> List<&2, SRowV>:
  match ms:
    case Nil{}:
      List.reverse(&2, SRowV, acc)
    case m <> t:
      Sbox.rows.go(t, l, u, Sbox.row(l, u, m) <> acc)

def Sbox.rows(+l: I.Loc, +u: Ui) -> List<&2, SRowV>:
  Sbox.rows.go(Sbox.metas(u), l, u, Nil{})

def Sbox.reply.s(m: Maybe<&2, J.Json>, u: Ui) -> Ui:
  match m:
    case Some{J.Str{s}}:
      Ui.scratch_set(u, "@secrets", s)
    case _:
      u

def Sbox.reply.v(m: Maybe<&2, J.Json>, u: Ui) -> Ui:
  match m:
    case Some{J.Str{s}}:
      Ui.scratch_set(u, "@vault", s)
    case _:
      u

# the answer to secret.list (and to set and del): names and metadata only
def Sbox.reply(+o: J.Json, u: Ui) -> Ui:
  Sbox.reply.v(J.Json.get(o, "vault"), Sbox.reply.s(J.Json.get(o, "secrets"), u))

def ApiKeys.form.note(+l: I.Loc) -> String:
  MSG.T.apikeys.form(l)

# the form's note, or why its Save went nowhere (the web's sealed form)
def ApiKeys.form.say(+l: I.Loc, +u: Ui) -> String:
  Sealed.say(u, "key", ApiKeys.form.note(l))

# a secret as it shows while typed: a dot a character
def ApiKeys.dots(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{_, t}:
      SCon{'•', ApiKeys.dots(t)}

# Antigravity (core/agy.bend) in Settings: one list of rows all four
# clients draw (each a label, a note, fields and buttons). The agent and
# its accounts come from the models info ("agy"), the job in progress from
# the hub's answer to this client ("agy.job"), the method and account from
# the settings agy.auth and agy.account (an API key unless set; a Google
# account carries Google's warning). Actions "agy-*" go to the hub as
# bots.voice op "agy" (server.bend's AgyJob); a key goes to the API keys
# file under the method's name (GEMINI_API_KEY or GOOGLE_API_KEY) and is
# never kept in the client's state.

type AgBtn is Data:
  AgBtn{label: String, action: String, value: String, on: Bool}

type AgField is Data:
  AgField{name: String, hint: String, secret: Bool, value: String, enter: String}

type AgRow is Data:
  AgRow{label: String, note: String, fields: List<&2, AgField>, btns: List<&2, AgBtn>}

# a form's argument ("agykey:GEMINI_API_KEY" has "GEMINI_API_KEY")
def Sealed.arg(+form: String) -> String:
  String.drop(form, Nat.add(String.length(Sealed.base(form)), 1n))

# Account labels are metadata. Authorization responses use sealed forms only.
def Acv.live(+u: Ui) -> J.Json:
  ACCT.Accounts.parse(Ui.info(u, "accounts.live"))

def Acv.provider(+u: Ui) -> String:
  Ui.scratch(u, "@acct.provider")

def Acv.selected.provider(+u: Ui) -> String:
  Bool.pick(String, String.eq(Acv.provider(u), "claude"), "claude", "codex")

def Acv.rpc(+u: Ui, +op: String, +id: String) -> Act:
  Ui.rpc(u, "accounts", [J.KV{"op", J.Str{op}}, J.KV{"profile", J.Str{id}}])

def Acv.sealed(+arg: String, +code: String) -> String:
  Sealed.out(Bool.pick(String, String.is_empty(String.trim(code)), "Enter the provider's sign-in response", ""),
    Rpc.text(0n, "accounts", [J.KV{"op", J.Str{"respond"}}, J.KV{"profile", J.Str{Sealed.base(arg)}}, J.KV{"attempt", J.Str{Sealed.arg(arg)}}, J.KV{"code", J.Str{String.trim(code)}}]))

def Acv.select(+u: Ui, +id: String) -> Act:
  +st = Ui.state(u)
  +th = Ui.sel(u)
  +pr = M.Thread.provider(Ui.thread(st, th))
  Ui.on_thread.p(Ui.flag_off(u, "accounts"), "accounts.select", [J.KV{"profile", J.Str{id}},
    J.KV{"epoch", J.Json.nat(J.Json.get_nat(ACCT.Accounts.profile(st, id), "epoch", 0n))},
    J.KV{"expected", J.Str{M.Setting.get(st, ACCT.Accounts.binding.key(th, pr), "")}}])

def Acv.add.k(blank: Bool, +u: Ui) -> Act:
  match blank:
    case True{}:
      Act{Ui.scratch_set(u, "@acct.label.err", MSG.T.account.label.required(Lang.loc(u))), [Focus{"agy-accountlabel"}]}
    case False{}:
      Ui.rpc(Ui.scratch_set(Ui.scratch_set(u, "@f.accountlabel", ""), "@acct.label.err", ""), "accounts", [J.KV{"op", J.Str{"add"}}, J.KV{"provider", J.Str{Acv.selected.provider(u)}}, J.KV{"label", J.Str{String.trim(Ui.scratch(u, "@f.accountlabel"))}}])

def Acv.act(+u: Ui, +action: String, +value: String) -> Act:
  match action:
    case "acct-provider": Act{Ui.scratch_set(u, "@acct.provider", value), Nil{}}
    case "acct-open": Acv.rpc(Bool.pick(Ui, Ui.flag(u, "accounts"), Ui.flag_off(u, "accounts"), Ui.flag_on(u, "accounts")), "list", "")
    case "acct-add": Acv.add.k(String.is_empty(String.trim(Ui.scratch(u, "@f.accountlabel"))), u)
    case "acct-login": Acv.rpc(u, "login", value)
    case "acct-status": Acv.rpc(u, "status", value)
    case "acct-disconnect": Acv.rpc(u, "disconnect", value)
    case "acct-cancel": Ui.rpc(u, "accounts", [J.KV{"op", J.Str{"cancel"}}, J.KV{"profile", J.Str{Sealed.base(value)}}, J.KV{"attempt", J.Str{Sealed.arg(value)}}])
    case "acct-select": Acv.select(u, value)
    case "acct-default":
      +p = ACCT.Accounts.profile(Ui.state(u), value)
      Ui.rpc(u, "accounts.default", [J.KV{"provider", J.Str{J.Json.get_str(p, "provider", "")}}, J.KV{"profile", J.Str{value}}, J.KV{"epoch", J.Json.nat(J.Json.get_nat(p, "epoch", 0n))}])
    case "acct-legacy-default": Ui.rpc(u, "accounts.default", [J.KV{"provider", J.Str{value}}, J.KV{"profile", J.Str{""}}, J.KV{"epoch", J.Json.nat(0n)}])
    case _: Act{u, Nil{}}

def Acv.reset(t: Nat) -> String:
  match t:
    case 0n: "unknown"
    case 1n+k: TZ.Tz.iso(TZ.Tz.utc0(), 1n+k)

def Acv.windows(xs: List<&2, J.Json>) -> String:
  match xs:
    case Nil{}: ""
    case +w <> t:
      " · " ++ J.Json.get_str(w, "name", "shared pool") ++ ": " ++ J.Json.as_raw(J.Json.get(w, "used_percent"), "?") ++ "% used; reset " ++ Acv.reset(J.Json.get_nat(w, "resets_at", 0n)) ++ " UTC" ++ Acv.windows(t)

def Acv.note(+p: J.Json) -> String:
  J.Json.get_str(p, "provider", "") ++ " · " ++ J.Json.get_str(p, "state", "unchecked") ++ " · " ++ J.Json.get_str(J.Json.get_obj(p, "identity"), "email", "Identity unverified") ++ " · " ++ J.Json.get_str(J.Json.get_obj(p, "identity"), "plan", "") ++
    " · allowance: " ++ J.Json.get_str(J.Json.get_obj(p, "allowance"), "state", "unknown") ++ Acv.windows(J.Json.elems(J.Json.get_obj(J.Json.get_obj(p, "allowance"), "windows"))) ++
    " · checked " ++ Nat.show(J.Json.get_nat(p, "checkedAt", 0n)) ++ "; provider pools may be shared across profiles. " ++ J.Json.get_str(p, "message", "")

def Acv.profile.rows(xs: List<&2, J.Json>) -> List<&2, AgRow>:
  match xs:
    case Nil{}: Nil{}
    case +p <> t:
      +id = J.Json.get_str(p, "id", "")
      AgRow{J.Json.get_str(p, "label", "Account"), Acv.note(p), Nil{},
        [AgBtn{"Connect / Login", "acct-login", id, False{}}, AgBtn{"Status / Refresh", "acct-status", id, False{}},
         AgBtn{"Disconnect", "acct-disconnect", id, False{}}, AgBtn{"Default for new threads", "acct-default", id, False{}}]} <> Acv.profile.rows(t)

def Acv.attempt.rows(xs: List<&2, J.Json>) -> List<&2, AgRow>:
  match xs:
    case Nil{}: Nil{}
    case +a <> t:
      +arg = J.Json.get_str(a, "profile", "") ++ ":" ++ J.Json.get_str(a, "id", "")
      AgRow{"Sign-in · " ++ J.Json.get_str(a, "state", ""), J.Json.get_str(a, "message", "") ++ " " ++ J.Json.get_str(a, "userCode", ""),
        Bool.pick(List<&2, AgField>, Bool.and(String.eq(J.Json.get_str(a, "state", ""), "waiting"), String.is_empty(J.Json.get_str(a, "userCode", ""))),
          [AgField{"account-" ++ J.Json.get_str(a, "id", ""), "If Claude asks: provider sign-in response", True{}, "", "acct-response:" ++ arg}], Nil{}),
        List.append(&2, AgBtn, Bool.pick(List<&2, AgBtn>, String.is_empty(J.Json.get_str(a, "url", "")), Nil{}, [AgBtn{"Open provider sign-in", "open-url", J.Json.get_str(a, "url", ""), True{}}]),
          [AgBtn{"Complete sign-in", "acct-response:" ++ arg, "", False{}}, AgBtn{"Cancel", "acct-cancel", arg, False{}}])} <> Acv.attempt.rows(t)

def Acv.rows(+u: Ui) -> List<&2, AgRow>:
  List.concat(&2, AgRow, [[AgRow{"Accounts on this machine", "Subscription accounts use private provider-owned stores. Existing Default accounts remain available. Claude Console accounts use a separate login store and are not supported here. " ++ J.Json.get_str(Acv.live(u), "error", "") ++ " " ++ Ui.scratch(u, "@acct.label.err"),
    [AgField{"accountlabel", "Account label (e.g. Personal)", False{}, Ui.scratch(u, "@f.accountlabel"), "acct-add"}],
    [AgBtn{"Codex", "acct-provider", "codex", String.eq(Acv.selected.provider(u), "codex")}, AgBtn{"Claude", "acct-provider", "claude", String.eq(Acv.selected.provider(u), "claude")}, AgBtn{"Add account", "acct-add", "", True{}}]},
    AgRow{"Existing Default accounts", "Preserve current CLI authentication and sessions; select explicitly beside the model.", Nil{}, [AgBtn{"Default Claude for new threads", "acct-legacy-default", "claude", False{}}, AgBtn{"Default Codex for new threads", "acct-legacy-default", "codex", False{}}]}],
    Acv.profile.rows(ACCT.Accounts.profiles(Ui.state(u))), Acv.attempt.rows(J.Json.elems(J.Json.get_obj(Acv.live(u), "attempts")))])

def Agv.info(+u: Ui) -> J.Json:
  J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "models")), J.Null{}), "agy")

def Agv.job(+u: Ui) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "agy.job")), J.Null{})

def Agv.method(+u: Ui) -> String:
  AGY.Agy.method(M.Setting.get(Ui.state.of(u), "agy.auth", ""))

def Agv.acct(+u: Ui) -> String:
  AGY.Agy.acct(M.Setting.get(Ui.state.of(u), "agy.account", ""))

def Agv.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.as_str(Some{x}, "") <> Agv.strs(t)

def Agv.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Agv.has(t, x))

def Agv.accts(+u: Ui) -> List<&2, String>:
  Agv.strs(J.Json.elems(J.Json.get_obj(Agv.info(u), "accts")))

def Agv.signed(+u: Ui) -> Bool:
  Agv.has(Agv.strs(J.Json.elems(J.Json.get_obj(Agv.info(u), "signed"))), Agv.acct(u))

def Agv.busy(+u: Ui, +what: String) -> Bool:
  Bool.and(String.eq(J.Json.get_str(Agv.job(u), "what", ""), what), String.eq(J.Json.get_str(Agv.job(u), "state", ""), "running"))

def Agv.have(+u: Ui) -> String:
  J.Json.get_str(Agv.info(u), "have", "")

# what Settings says about the agent itself
def Agv.install.note(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, Agv.busy(u, "install"), J.Json.get_str(Agv.job(u), "msg", ""),
    Bool.pick(String, String.eq(J.Json.get_str(Agv.job(u), "what", ""), "install"), J.Json.get_str(Agv.job(u), "msg", "") ++ " ", "")
      ++ Bool.pick(String, Bool.not(String.is_empty(Agv.have(u))), MSG.T.agy.installed(l, Agv.have(u)),
        Bool.pick(String, Bool.not(String.is_empty(J.Json.get_str(Agv.info(u), "old", ""))), MSG.T.agy.old(l, J.Json.get_str(Agv.info(u), "old", ""), AGY.Agy.version()),
          Bool.pick(String, J.Json.get_bool(Agv.info(u), "plat", True{}), MSG.T.agy.download(l, AGY.Agy.version()),
            MSG.T.agy.unsupported(l)))))

def Agv.install.btns(+l: I.Loc, +u: Ui) -> List<&2, AgBtn>:
  Bool.pick(List<&2, AgBtn>, Bool.or(Agv.busy(u, "install"), Bool.not(J.Json.get_bool(Agv.info(u), "plat", True{}))), Nil{},
    [AgBtn{Bool.pick(String, String.is_empty(Agv.have(u)), Bool.pick(String, String.is_empty(J.Json.get_str(Agv.info(u), "old", "")), MSG.T.agy.install(l), MSG.T.agy.update(l, AGY.Agy.version())), MSG.T.agy.reinstall(l)),
      "agy-install", "", String.is_empty(Agv.have(u))}])

def Agv.methods(ms: List<&2, String>, +cur: String) -> List<&2, AgBtn>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      AgBtn{AGY.Agy.label(m), "setting", "agy.auth=" ++ m, String.eq(m, cur)} <> Agv.methods(t, cur)

# the key the method reads, from the API keys (names only)
def Agv.key.row(+l: I.Loc, +u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, String.is_empty(AGY.Agy.key(Agv.method(u))), Nil{},
    [AgRow{AGY.Agy.key(Agv.method(u)), Sealed.say(u, "agykey", Bool.pick(String, Agv.has(ApiKeys.names(u), AGY.Agy.key(Agv.method(u))), MSG.T.agy.key.saved(l),
        MSG.T.agy.key.unsaved(l))),
      [AgField{"agykey", MSG.T.agy.key.paste(l), True{}, Ui.scratch(u, "@f.agykey"), "agy-key"}],
      [AgBtn{MSG.T.agy.key.save(l), "agy-key", "", False{}}, AgBtn{MSG.T.agy.key.check(l), "agy-signin", "", True{}}]}])

def Agv.gcp.note(+lc: I.Loc, +p: String, +l: String) -> String:
  Bool.pick(String, Bool.and(String.is_empty(p), String.is_empty(l)), MSG.T.agy.gcp.unset(lc), MSG.T.agy.gcp.set(lc, AGY.Agy.or(p, MSG.T.agy.gcp.none(lc)), AGY.Agy.or(l, MSG.T.agy.gcp.none(lc))))

def Agv.gcp.row(+l: I.Loc, +u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, Bool.not(AGY.Agy.gcp(Agv.method(u))), Nil{},
    [AgRow{MSG.T.agy.gcp(l), Agv.gcp.note(l, M.Setting.get(Ui.state.of(u), "agy.gcp.project", ""), M.Setting.get(Ui.state.of(u), "agy.gcp.location", "")),
      [AgField{"agyproj", MSG.T.agy.gcp.project(l), False{}, Ui.scratch(u, "@f.agyproj"), "agy-gcp"}, AgField{"agyloc", MSG.T.agy.gcp.location(l), False{}, Ui.scratch(u, "@f.agyloc"), "agy-gcp"}],
      [AgBtn{MSG.T.common.save(l), "agy-gcp", "", False{}}]}])

def Agv.acct.btns(xs: List<&2, String>, +cur: String) -> List<&2, AgBtn>:
  match xs:
    case Nil{}:
      Nil{}
    case +a <> t:
      AgBtn{a, "setting", "agy.account=" ++ a, String.eq(a, cur)} <> Agv.acct.btns(t, cur)

def Agv.accts.shown(+u: Ui) -> List<&2, String>:
  Bool.pick(List<&2, String>, Agv.has(Agv.accts(u), Agv.acct(u)), Agv.accts(u), List.append(&2, String, Agv.accts(u), [Agv.acct(u)]))

def Agv.google.note(+l: I.Loc, signed: Bool, +acct: String) -> String:
  match signed:
    case True{}: MSG.T.agy.account.signedin(l, acct)
    case False{}: MSG.T.agy.account.signedout(l, acct)

def Agv.google.row(+l: I.Loc, +u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, Bool.not(AGY.Agy.browser(Agv.method(u))), Nil{},
    [AgRow{MSG.T.agy.account(l), Agv.google.note(l, Agv.signed(u), Agv.acct(u)),
      [AgField{"agyacct", MSG.T.agy.account.other(l), False{}, Ui.scratch(u, "@f.agyacct"), "agy-acct"}],
      List.concat(&2, AgBtn, [Agv.acct.btns(Agv.accts.shown(u), Agv.acct(u)), [AgBtn{MSG.T.agy.account.add(l), "agy-acct", "", False{}}, AgBtn{MSG.T.agy.google.signin(l), "agy-signin", "", True{}}],
        Bool.pick(List<&2, AgBtn>, Agv.signed(u), [AgBtn{MSG.T.agy.signout(l), "agy-signout", Agv.acct(u), False{}}], Nil{}),
        Bool.pick(List<&2, AgBtn>, Agv.has(Agv.accts(u), Agv.acct(u)), [AgBtn{MSG.T.common.remove(l), "agy-forget", Agv.acct(u), False{}}], Nil{})])}])

def Agv.job.k(i: Nat, +l: I.Loc, +u: Ui, +j: J.Json) -> List<&2, AgRow>:
  match i:
    case 0n:
      [AgRow{MSG.T.agy.signin(l), J.Json.get_str(j, "msg", ""), Nil{}, [AgBtn{MSG.T.common.cancel(l), "agy-cancel", "", False{}}]}]
    case 1n:
      [AgRow{MSG.T.agy.signin(l), Sealed.say(u, "agyback", J.Json.get_str(j, "msg", "")), [AgField{"agyback", MSG.T.agy.signin.back(l), False{}, Ui.scratch(u, "@f.agyback"), "agy-back"}],
        [AgBtn{MSG.T.agy.signin.open(l), "open-url", J.Json.get_str(j, "url", ""), True{}}, AgBtn{MSG.T.agy.signin.next(l), "agy-back", "", False{}}, AgBtn{MSG.T.common.cancel(l), "agy-cancel", "", False{}}]}]
    case 2n:
      [AgRow{MSG.T.agy.signin(l), J.Json.get_str(j, "msg", ""), Nil{}, Nil{}}]
    case 3n:
      [AgRow{MSG.T.agy.signin.failed(l), J.Json.get_str(j, "msg", ""), Nil{}, Nil{}}]
    case _:
      Nil{}

# the sign-in in progress (or just finished): what it says, its link and
# the field for Google's way back
def Agv.job.row(+l: I.Loc, +u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, Bool.or(String.eq(J.Json.get_str(Agv.job(u), "what", ""), "signin"), String.eq(J.Json.get_str(Agv.job(u), "what", ""), "signout")),
    Agv.job.k(C.Codec.pos(["running", "waiting", "done", "failed"], J.Json.get_str(Agv.job(u), "state", "")), l, u, Agv.job(u)), Nil{})

# every row, in order
def Agv.rows.loc(+l: I.Loc, +u: Ui) -> List<&2, AgRow>:
  List.concat(&2, AgRow, [
    [AgRow{"Antigravity", Agv.install.note(l, u), Nil{}, Agv.install.btns(l, u)},
     AgRow{MSG.T.agy.method(l), AGY.Agy.note(Agv.method(u)), Nil{}, Agv.methods(AGY.Agy.methods(), Agv.method(u))}],
    Agv.key.row(l, u), Agv.gcp.row(l, u), Agv.google.row(l, u), Agv.job.row(l, u)])

def Agv.rows(+u: Ui) -> List<&2, AgRow>:
  Agv.rows.loc(Lang.loc(u), u)

def Agv.rpc(u: Ui, +what: String, kvs: List<&2, J.KV>) -> Act:
  Voice.rpc(u, "agy", J.KV{"do", J.Str{what}} <> kvs)

def Agv.signin(+u: Ui) -> Act:
  Agv.rpc(u, "signin", [J.KV{"method", J.Str{Agv.method(u)}}, J.KV{"account", J.Str{Agv.acct(u)}},
    J.KV{"project", J.Str{M.Setting.get(Ui.state.of(u), "agy.gcp.project", "")}}, J.KV{"location", J.Str{M.Setting.get(Ui.state.of(u), "agy.gcp.location", "")}}])

def Agv.then.k(b: Act, cs: List<&2, Cmd>) -> Act:
  match b:
    case Act{u2, cs2}:
      Act{u2, List.append(&2, Cmd, cs, cs2)}

# a second setting after an action
def Agv.then(a: Act, +kv: String) -> Act:
  match a:
    case Act{u, cs}:
      Agv.then.k(Ui.setting(u, kv), cs)

def Agv.gcp(+u: Ui) -> Act:
  Agv.then(Ui.setting(Ui.scratch_set(Ui.scratch_set(u, "@f.agyproj", ""), "@f.agyloc", ""), "agy.gcp.project=" ++ AGY.Agy.or(String.trim(Ui.scratch(u, "@f.agyproj")), M.Setting.get(Ui.state.of(u), "agy.gcp.project", ""))),
    "agy.gcp.location=" ++ AGY.Agy.or(String.trim(Ui.scratch(u, "@f.agyloc")), M.Setting.get(Ui.state.of(u), "agy.gcp.location", "")))

def Agv.key(+u: Ui) -> Act:
  Voice.rpc(Ui.scratch_set(u, "@f.agykey", ""), "api-add", [J.KV{"name", J.Str{AGY.Agy.key(Agv.method(u))}}, J.KV{"value", J.Str{String.trim(Ui.scratch(u, "@f.agykey"))}}])

def Agv.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Agv.rpc(u, "install", Nil{})
    case 1n:
      Agv.signin(u)
    case 2n:
      Agv.rpc(Ui.scratch_set(u, "@f.agyback", ""), "back", [J.KV{"url", J.Str{String.trim(Ui.scratch(u, "@f.agyback"))}}])
    case 3n:
      Agv.rpc(u, "cancel", Nil{})
    case 4n:
      Agv.rpc(u, "signout", [J.KV{"account", J.Str{value}}])
    case 5n:
      Agv.rpc(u, "forget", [J.KV{"account", J.Str{value}}])
    case 6n:
      Agv.key(u)
    case 7n:
      Agv.gcp(u)
    case 8n:
      Ui.setting(Ui.scratch_set(u, "@f.agyacct", ""), "agy.account=" ++ AGY.Agy.acct(Ui.scratch(u, "@f.agyacct")))
    case _:
      Agv.rpc(u, "status", Nil{})

# an "agy-" action (what the web's last sealed Save of the key or the way
# back said goes)
def Agv.act(+u: Ui, +action: String, +value: String) -> Act:
  Agv.act.k(C.Codec.pos(["agy-install", "agy-signin", "agy-back", "agy-cancel", "agy-signout", "agy-forget", "agy-key", "agy-gcp", "agy-acct"], action),
    Sealed.forget(Sealed.forget(u, "agykey"), "agyback"), value)

# The web's sealed forms (Sealed.*, above): each form's request, built
# from its fields as host.js read them ("<form>\u001f<field>..."), and
# what its Save leads to once the hub has answered.

# Dictation's key ("vkey"): the hub keeps only an OpenAI key (Vo.set_key)
def Voice.key.bad(+k: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(k)), "Paste the key itself",
    Bool.pick(String, VC.Voice.openai.ok(String.trim(k)), "", "That is not an OpenAI API key (they start with sk-)"))

def Voice.key.req(+k: String) -> String:
  Sealed.out(Voice.key.bad(k), Rpc.text(0n, "bots.voice", [J.KV{"op", J.Str{"key"}}, J.KV{"key", J.Str{String.trim(k)}}]))

# Antigravity's key ("agykey:<NAME>"): kept in the API keys under the
# method's name, as Agv.key does
def Agv.key.req(+name: String, +k: String) -> String:
  Sealed.out(AK.Keys.bad(name, k), Rpc.text(0n, "bots.voice", [J.KV{"op", J.Str{"api-add"}}, J.KV{"name", J.Str{name}}, J.KV{"value", J.Str{String.trim(k)}}]))

# the address Antigravity's sign-in landed on ("agyback"): the hub takes
# it only for the sign-in that asked (AgyJob.back)
def Agv.back.req(+url: String) -> String:
  Sealed.out(Bool.pick(String, String.is_empty(String.trim(url)), "Paste the address Google sent you to", ""),
    Rpc.text(0n, "bots.voice", [J.KV{"op", J.Str{"agy"}}, J.KV{"do", J.Str{"back"}}, J.KV{"url", J.Str{String.trim(url)}}]))

# the web's form for an Antigravity row's field or button: its key and its
# way back are sealed ("" for the rest, which stay view actions)
def Sealed.agy(+u: Ui, +action: String) -> String:
  Bool.pick(String, String.starts_with(action, "acct-response:"), "account:" ++ String.drop(action, 14n),
    Bool.pick(String, String.eq(action, "agy-key"), "agykey:" ++ AGY.Agy.key(Agv.method(u)),
    Bool.pick(String, String.eq(action, "agy-back"), "agyback", "")))

def Bots.google.req(+op: String, +id: String, +secret: String, +pasted: String) -> String:
  Rpc.text(0n, "bots.google", [J.KV{"op", J.Str{op}}, J.KV{"clientId", J.Str{String.trim(id)}}, J.KV{"clientSecret", J.Str{String.trim(secret)}},
    J.KV{"pasted", J.Str{String.trim(pasted)}}, J.KV{"account", J.Str{""}}, J.KV{"redirect", J.Str{""}}])

# Google's own OAuth client ("google": its id and secret; the hub keeps
# them only together, Google.configure)
def Bots.google.own.req(+id: String, +secret: String) -> String:
  Sealed.out(Bool.pick(String, Bool.or(String.is_empty(String.trim(id)), String.is_empty(String.trim(secret))), "Give both the client id and its secret", ""),
    Bots.google.req("config", id, secret, ""))

def Bots.google.own.ps(ps: List<&2, String>) -> String:
  match ps:
    case +id <> +secret <> _:
      Bots.google.own.req(id, secret)
    case _:
      Bots.google.own.req("", "")

# the address Google's sign-in landed on ("gpaste")
def Bots.google.paste.req(+pasted: String) -> String:
  Sealed.out(Bool.pick(String, String.is_empty(String.trim(pasted)), "Paste the address the sign-in ended on", ""),
    Bots.google.req("finish", "", "", pasted))

# another machine's invite ("invite")
def Bots.join.req(+invite: String) -> String:
  Sealed.out(Bool.pick(String, String.is_empty(String.trim(invite)), "Paste the invite the other machine made", ""),
    Rpc.text(0n, "bots.join", [J.KV{"invite", J.Str{String.trim(invite)}}]))

def Sealed.none() -> String:
  J.Json.show(J.Json.obj([J.KV{"error", J.Str{"Nothing to save"}}]))

def Sealed.req.k(i: Nat, +arg: String, +vs: List<&2, String>) -> String:
  match i:
    case 0n:
      ApiKeys.req.ps(vs)
    case 1n:
      Voice.key.req(Sealed.first(vs))
    case 2n:
      Agv.key.req(arg, Sealed.first(vs))
    case 3n:
      Agv.back.req(Sealed.first(vs))
    case 4n:
      Bots.google.own.ps(vs)
    case 5n:
      Bots.google.paste.req(Sealed.first(vs))
    case 6n:
      Bots.join.req(Sealed.first(vs))
    case 7n:
      Acv.sealed(arg, Sealed.first(vs))
    case _:
      Sealed.none()

def Sealed.forms() -> List<&2, String>:
  ["key", "vkey", "agykey", "agyback", "google", "gpaste", "invite", "account"]

def Sealed.req.ps(ps: List<&2, String>) -> String:
  match ps:
    case +form <> +vs:
      Sealed.req.k(C.Codec.pos(Sealed.forms(), Sealed.base(form)), Sealed.arg(form), vs)
    case Nil{}:
      Sealed.none()

# a sealed form's request (host.js's sealedSave): "<form>\u001f<field>..."
# to {"rpc"} or {"error": why}
def Sealed.req(+packed: String) -> String:
  Sealed.req.ps(String.split(packed, Chr{31}))

# once the hub has a form's value: the API key's form shuts; what the
# voice key and Antigravity's key show is asked again (Antigravity's way
# back reports through the sign-in's own news); Google's own client form
# shuts and Google's status is asked
def Sealed.sent.k(i: Nat, +u: Ui) -> Act:
  match i:
    case 0n:
      ApiKeys.sent(u)
    case 1n:
      Voice.rpc(u, "status", Nil{})
    case 2n:
      Voice.rpc(u, "status", Nil{})
    case 4n:
      Bots.google.rpc(Ui.scratch_set(u, "@f.gown", ""), "status", "")
    case 5n:
      Bots.google.rpc(u, "status", "")
    case _:
      Act{u, Nil{}}

def Sealed.err.put(ps: List<&2, String>, u: Ui) -> Ui:
  match ps:
    case +form <> why:
      Ui.scratch_set(u, "@sealed:" ++ Sealed.base(form), String.join(why, "\u{1f}"))
    case Nil{}:
      u

# "sealed-sent" (value: the form) or "sealed-err" ("<form>\u001f<why>")
def Sealed.after(sent: Bool, +u: Ui, +value: String) -> Act:
  match sent:
    case True{}:
      Sealed.sent.k(C.Codec.pos(Sealed.forms(), Sealed.base(value)), Sealed.forget(u, value))
    case False{}:
      Act{Sealed.err.put(String.split(value, Chr{31}), u), Nil{}}

# past the phone's upload: the viewer's history (Hs.act, from 31)
def Upload.rest(hist: Bool, +i: Nat, +u: Ui, +value: String) -> Act:
  match hist:
    case True{}:
      Hs.act(Nat.sub(i, 31n), u, value)
    case False{}:
      Mpage.act(Nat.sub(i, 20n), u, value)

# past the API keys': the Files tab's (49-53, Fm.act)
def Upload.more(files: Bool, +i: Nat, +u: Ui, +value: String) -> Act:
  match files:
    case True{}:
      Fm.act(Nat.sub(i, 49n), u, value)
    case False{}:
      Upload.rest(Nat.is_ge(i, 31n), i, u, value)

def Upload.act(+i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 29n:
      Upload.start(u, Maybe.default(&2, J.Json, J.Json.parse(value), J.Null{}))
    case 30n:
      Upload.end(u, value)
    case 47n:
      ApiKeys.add(u)
    case 48n:
      ApiKeys.del(u, value)
    case _:
      Upload.more(Nat.is_ge(i, 49n), i, u, value)

def KN.prefill(+u: Ui) -> Ui:
  Ui.scratch_set(u, "@f.kpath", M.Setting.get(Ui.state.of(u), "kicad.path", ""))

# Settings > Usage's Boat side: the hub reads Boat's CLI (src/core/usage.bend's
# Bj: "read" when it is due, "refresh" from the Refresh button) and answers
# with info "boat.usage"
def Usage.ask.k(cs: List<&2, Cmd>, b: Act) -> Act:
  match b:
    case Act{u, more}:
      Act{u, List.append(&2, Cmd, cs, more)}

def Usage.ask(a: Act, +what: String) -> Act:
  match a:
    case Act{u, cs}:
      Usage.ask.k(cs, Voice.rpc(u, "usage", [J.KV{"do", J.Str{what}}]))

# opening Settings asks for the voice settings and Boat's usage; opening or
# closing it puts the pairing code away
# (and the vault's list for Settings > Secrets)
def Sbox.also(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Ui.also(Sbox.list(u), cs)

def Voice.flag.k(ask: Bool, a: Act) -> Act:
  match ask a:
    case True{} Act{u, cs}:
      Usage.ask(Sbox.also(Ui.also(Voice.rpc(KN.prefill(Ui.flag_off(u, "pairqr")), "status", Nil{}), cs)), "read")
    case False{} _:
      a

def Voice.flag(+value: String, a: Act) -> Act:
  Voice.flag.k(String.eq(value, "settings"), a)

def Bots.reply.put(+o: J.Json, +key: String, +into: String, +u: Ui) -> Ui:
  Bool.pick(Ui, String.is_empty(J.Json.get_str(o, key, "")), u, Ui.scratch_set(u, into, J.Json.get_str(o, key, "")))

# a key the answer has, even when empty (no account left is news)
def Bots.reply.has(m: Maybe<&2, J.Json>, +into: String, +u: Ui) -> Ui:
  match m:
    case Some{J.Str{s}}:
      Ui.scratch_set(u, into, s)
    case _:
      u

# A pending selection names both its hub and its connection generation.
# Only a read model installed for that exact pair can resolve it.
def Want.accept(hub: Bool, generation: Bool, installed: Bool) -> Bool:
  match hub generation installed:
    case True{} True{} True{}:
      True{}
    case _ _ _:
      False{}

def Want.bound(+u: Ui) -> Bool:
  Want.accept(String.eq(Ui.scratch(u, "@want.hub"), Ui.info(u, "local.machine")),
    String.eq(Ui.scratch(u, "@want.gen"), Ui.info(u, "local.generation")),
    Bool.and(String.eq(Ui.info(u, "local.state.hub"), Ui.info(u, "local.machine")), String.eq(Ui.info(u, "local.state.gen"), Ui.info(u, "local.generation"))))

def Want.bot(+u: Ui, name: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@thread.want", ""), "@bot.want", name), "@want.hub", Ui.info(u, "local.machine")), "@want.gen", Ui.info(u, "local.generation"))

def Far.opened(a: Act, +name: String) -> Act:
  match a:
    case Act{u, cs}:
      Act{Want.bot(u, name), cs}

def Far.open(same: Bool, +u: Ui, +url: String, +id: String, +name: String) -> Act:
  match same:
    case True{}:
      Far.chat(u, id)
    case False{}:
      Far.opened(Ui.machine(u, url), name)

def Far.go(m: Maybe<&2, String>, +u: Ui, +id: String, +name: String) -> Act:
  match m:
    case Some{+url}:
      Far.open(Bool.or(String.eq(url, Ui.info(u, "local.machine")), String.eq(Ui.info(u, "local.fixed"), "1")), u, url, id, name)
    case None{}:
      Far.chat(u, id)

# a bot elsewhere ("name@machine", BU.far's id): the machine it names, as
# BT.Peer.of finds it (an id first, then a name)
def Far.peer(m: Maybe<&2, BT.Peer>, +u: Ui, +id: String) -> Act:
  match m:
    case Some{BT.Peer{_, +n, +url, _, _}}:
      Far.go(Far.machine(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "local.machines")), J.Null{}), n, url), u, id, BT.Member.name(id))
    case None{}:
      Far.chat(u, id)

def Bots.reply.show(m: Maybe<&2, BT.Bot>, +id: String, u: Ui) -> Ui:
  match m:
    case Some{BT.Bot{+i, _, +th, _, _, _, _}}:
      Bots.views(Bots.sel.set(u, th), i, "", "")
    case None{}:
      Want.bot(u, id)

def Bots.reply.bot.k(none: Bool, +o: J.Json, +u: Ui) -> Ui:
  match none:
    case True{}:
      u
    case False{}:
      Bots.reply.show(BT.World.find.id(BT.World.bot(M.State.bots(Ui.state.of(u)), J.Json.get_str(o, "bot", ""))), J.Json.get_str(o, "bot", ""), u)

# answers that carry something for the bots' screens: a new bot is shown;
# a webhook's secret and an invite are shown once; Google's sign-in link
def Bots.reply.bot(+o: J.Json, u: Ui) -> Ui:
  Bots.reply.bot.k(String.is_empty(J.Json.get_str(o, "bot", "")), o, u)

def Bots.reply(+o: J.Json, +u: Ui) -> Ui:
  Voice.reply(o, Sbox.reply(o, Bots.reply.has(J.Json.get(o, "apiKeys"), "@keys", Bots.reply.has(J.Json.get(o, "googleAccounts"), "@google.accts", Bots.reply.put(o, "googleUrl", "@google.url", Bots.reply.put(o, "google", "@google", Bots.reply.put(o, "invite", "@peer.invite",
    Bots.reply.put(o, "secret", "@hook.secret", Bots.reply.put(o, "hook", "@hook.id",
      Bots.reply.put(o, "token", "@hook.token", Bots.reply.put(o, "tokenHook", "@hook.tokfor", Bots.reply.put(o, "tokenUrl", "@hook.tokurl", Bots.reply.bot(o, u)))))))))))))

# A thread request uses the same binding as a bot request.
def Want.thread(+u: Ui, id: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@bot.want", ""), "@thread.want", id), "@want.hub", Ui.info(u, "local.machine")), "@want.gen", Ui.info(u, "local.generation"))

def Made.got(go: Bool, u: Ui, +th: String) -> Ui:
  match go:
    case True{}:
      Ui.scratch_set(Want.thread(u, th), "@made.far", Bool.pick(String, FR.Fr.is(th), th, ""))
    case False{}:
      u

def Made.viewer(copy: Bool, +u: Ui, +th: String) -> Ui:
  match copy:
    case True{}: Viewer.fork(u, th, Maybe.default(&2, J.Json, J.Json.parse(Ui.scratch(u, "@want.viewer")), J.Null{}))
    case False{}: u

def Made.take(mine: Bool, +o: J.Json, +u: Ui) -> Ui:
  match mine:
    case True{}:
      +go = Bool.and(Bool.and(J.Json.get_bool(o, "ok", False{}), Bool.not(String.is_empty(J.Json.get_str(o, "thread", "")))), String.eq(Ui.scratch(u, "@want.new.at"), Made.at(u)))
      Made.got(go,
        Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Made.viewer(go, u, J.Json.get_str(o, "thread", "")), "@want.viewer", ""), "@want.new", ""), "@want.new.at", ""), J.Json.get_str(o, "thread", ""))
    case False{}:
      u

# the answer to this client's newest request for a thread: the thread it
# names is wanted (selected now if here, else when it arrives); a failure
# or a hub too old to name it selects nothing; any other answer leaves
# the want alone
def Made.reply(+o: J.Json, +u: Ui) -> Ui:
  Made.take(Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@want.new"))), String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), Ui.scratch(u, "@want.new"))), o, u)

# a hub's answer naming the thread a request made (for laws)
def Made.demo(+id: String, +thread: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"thread", J.Str{thread}}])

# a thread made by someone, as a frame of log changes (for laws)
def Made.made(+t: String, +th: String) -> J.Json:
  Seq.demo(t, [C.Change.encode(M.ThreadCreated{th, "p1", "New thread", M.Local{}, "", "", "claude", "", 1n})])

# an answer: its request leaves the outbox; a diff opens the diff view
def KN.reply.k(has: Bool, +u: Ui, +o: J.Json) -> Ui:
  match has:
    case True{}:
      Ui.info_set(u, "kn:" ++ J.Json.get_str(J.Json.get_obj(o, "result"), "thread", Ui.sel(u)), "1")
    case False{}: u

def KN.reply(+u: Ui, +o: J.Json) -> Ui:
  KN.reply.k(Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(o, "result"), "kicad", ""))), u, o)

def KX.result(+nf: Map<&2, String>, +th: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Str.get(nf, "kx:" ++ th)), J.Null{})

def KX.pending(+nf: Map<&2, String>, +th: String) -> Bool:
  Bool.not(String.is_empty(J.Json.get_str(KX.result(nf, th), "export10", "")))

def KX.offset(+nf: Map<&2, String>, +th: String) -> Nat:
  KH.KH.num(Str.get(nf, "kx.page:" ++ th), 0n)

def KX.next(next: Bool, +th: String) -> List<&2, KH.KAct>:
  match next:
    case True{}: [KH.KAct{"Next page", "kicad-export10-next", th}]
    case False{}: Nil{}

def KX.buttons(confirm: Bool, +nf: Map<&2, String>, +th: String, +n: Nat) -> List<&2, KH.KAct>:
  match confirm:
    case True{}: List.append(&2, KH.KAct, [KH.KAct{"Export anyway", "kicad-export10-anyway", th}, KH.KAct{"Cancel", "kicad-export10-cancel", th}], KX.next(Nat.is_gt(n, 8n), th))
    case False{}: List.append(&2, KH.KAct, [KH.KAct{"Close", "kicad-export10-cancel", th}], KX.next(Nat.is_gt(n, 8n), th))

def KX.note.k(present: Bool, +nf: Map<&2, String>, +th: String, +r: J.Json) -> Maybe<&2, KH.KNote>:
  match present:
    case False{}: None{}
    case True{}:
      +ls = KEX.KX.lines(KEX.KX.report(J.Json.get_obj(r, "report")))
      +page = List.take(&2, String, List.drop(&2, String, ls, KX.offset(nf, th)), 8n)
      Some{KH.KNote{"export10", "KiCad 10 exported copy", J.Json.get_str(r, "text", "") ++ "\n" ++ String.join(page, "\n"), KX.buttons(String.eq(J.Json.get_str(r, "export10", ""), "confirm"), nf, th, List.length(&2, String, ls))}}

def KX.note(+nf: Map<&2, String>, +th: String) -> Maybe<&2, KH.KNote>:
  KX.note.k(KX.pending(nf, th), nf, th, KX.result(nf, th))

def KX.reply.k(has: Bool, +u: Ui, +r: J.Json) -> Ui:
  match has:
    case False{}: u
    case True{}: Ui.info_set(Ui.info_set(u, "kx:" ++ J.Json.get_str(r, "thread", Ui.sel(u)), J.Json.show(r)), "kx.page:" ++ J.Json.get_str(r, "thread", Ui.sel(u)), "0")

def KX.reply(+u: Ui, +o: J.Json) -> Ui:
  KX.reply.k(Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(o, "result"), "export10", ""))), u, J.Json.get_obj(o, "result"))

# A link's threads and bots (far.bend): its batch folded into the read
# model under this hub's ids, and whether it answered ("far.away.<link>")
# and what it is called ("far.name.<link>") kept for the rows that show it
def Far.arrived.m(m: Maybe<&2, BT.Bot>, u: Ui) -> Ui:
  match m:
    case Some{BT.Bot{+id, _, +th, _, _, _, _}}:
      Ui.flag_off(Bots.views(Bots.sel.set(u, th), id, "", ""), "settings")
    case None{}:
      u

def Far.arrived.k(waiting: Bool, +u: Ui) -> Ui:
  match waiting:
    case True{}:
      Far.arrived.m(Far.mirrored(BT.World.bots(M.State.bots(Ui.state(u))), BT.Member.key(Bots.peers(u), Ui.scratch(u, "@remote"))), u)
    case False{}:
      u

# A reported-only bot opens immediately. If its owner subsequently
# shares its thread, the same choice gains the normal timeline and tabs.
# Keep an unfinished remote composer in place until it has been sent.
def Far.arrived(+u: Ui) -> Ui:
  Far.arrived.k(Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@remote"))), String.is_empty(Bots.field(u, "rtext"))), u)

def Ui.far.st(u: Ui, st: M.State) -> Ui:
  match u:
    case Ui{_, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def Ui.far.k(+u: Ui, +link: String, +o: J.Json) -> Ui:
  Ui.info_set(Ui.info_set(Ui.far.st(u, FR.Fr.mark(FR.Fr.cat.apply(J.Json.elems(J.Json.get_obj(o, "catalog")), link, FR.Fr.recv(Ui.state(u), link, J.Json.get_str(o, "epoch", ""), J.Json.get_nat(o, "since", 0n), J.Json.get_nat(o, "head", 0n),
      FR.Fr.items.of(J.Json.elems(J.Json.get_obj(o, "items"))))), link, J.Json.get_str(o, "name", link), J.Json.get_bool(o, "away", False{}))),
    "far.away." ++ link, Bool.pick(String, J.Json.get_bool(o, "away", False{}), "1", "")), "far.name." ++ link, J.Json.get_str(o, "name", link))

# a thread elsewhere whose message was posted or turn ended shows no live
# text any more (the same rule as a local one, Ui.see; core/farlive.bend)
def Ui.live.gone(u: Ui, +thread: String) -> Ui:
  match u:
    case Ui{st, sel, dr, +live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, Map.set(&2, String, live, thread, ""), op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def Ui.far.live(ids: List<&2, String>, u: Ui) -> Ui:
  match ids:
    case Nil{}:
      u
    case +id <> t:
      Ui.far.live(t, Ui.live.gone(u, id))

def Ui.far(+u: Ui, +o: J.Json) -> Ui:
  Far.arrived(Ui.far.live(FLV.FL.ended.ids(FLV.FL.ended(FR.Fr.items.of(J.Json.elems(J.Json.get_obj(o, "items"))), J.Json.get_str(o, "link", ""))), Ui.far.k(u, J.Json.get_str(o, "link", ""), o)))

# whether the machine a thing elsewhere lives on did not answer last time
def Ui.far.away(+u: Ui, +id: String) -> Bool:
  Bool.and(FR.Fr.is(id), String.eq(Ui.info(u, "far.away." ++ FR.Fr.link(id)), "1"))

# the name of the machine a thing elsewhere lives on ("" for one here)
def Ui.far.name(+u: Ui, +id: String) -> String:
  Bool.pick(String, FR.Fr.is(id), Bool.pick(String, String.is_empty(Ui.info(u, "far.name." ++ FR.Fr.link(id))), FR.Fr.link(id), Ui.info(u, "far.name." ++ FR.Fr.link(id))), "")

# a thread's or bot's name as shown: "Kit · desk" for one elsewhere, with
# " (away)" while its machine does not answer
def Ui.far.title(+u: Ui, +id: String, +title: String) -> String:
  Label.title(Ui.state(u), id, title)

def Want.rebind(bound: Bool, u: Ui, +generation: String) -> Ui:
  match bound:
    case True{}:
      Ui.scratch_set(u, "@want.gen", generation)
    case False{}:
      u

# Called only for the destination chosen by Ui.machine. Old socket
# messages are rejected by the host's connection tag before Ui.recv.
def Ui.connection(+u: Ui, +generation: String) -> Ui:
  Ui.online(Ui.info_set(Want.rebind(Bool.and(String.eq(Ui.scratch(u, "@want.hub"), Ui.info(u, "local.machine")), String.eq(Ui.scratch(u, "@want.gen"), Ui.info(u, "local.generation"))), u, generation), "local.generation", generation), False{})

def Ui.installed(+u: Ui) -> Ui:
  Ui.info_set(Ui.info_set(u, "local.state.hub", Ui.info(u, "local.machine")), "local.state.gen", Ui.info(u, "local.generation"))

# A thread or bot the person asked for (New thread, Fork, a machine's
# thread or bot) opens as it arrives and leaves Settings, as a click on it
# in the sidebar does (law made_leaves_settings)
def Bots.wanted.m(m: Maybe<&2, BT.Bot>, u: Ui) -> Ui:
  match m:
    case Some{BT.Bot{+i, _, +th, _, _, _, _}}:
      Ui.flag_off(Bots.views(Bots.sel.set(u, th), i, "", ""), "settings")
    case None{}:
      u

def Want.thread.k(found: Bool, u: Ui, id: String) -> Ui:
  match found:
    case True{}:
      Ui.flag_off(Ui.scratch_set(Bots.views(Bots.sel.set(u, id), "", "", ""), "@thread.want", ""), "settings")
    case False{}:
      u

def Want.thread.has(r: Map<&2, M.Thread> & Bool) -> Bool:
  (_, found) = r
  found

def Want.thread.exists(st: M.State, +id: String) -> Bool:
  match st:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Want.thread.has(Map.has(&2, M.Thread, ts, id))

# a thread this client asked for is still to come (the phones show its
# page at once, waiting, rather than leave the button to be tapped again;
# the answer, made or failed, ends it: laws made_waiting_*)
# the hub that owns the project made the thread and told us its id here,
# but its mirror has not landed yet (the push may follow the answer): still
# making, until the thread is here or the person goes elsewhere (which
# clears "@thread.want", Bots.sel.set)
def Made.coming(+u: Ui) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@made.far"))), String.eq(Ui.scratch(u, "@made.far"), Ui.scratch(u, "@thread.want"))),
    Bool.not(Want.thread.exists(Ui.state.of(u), Ui.scratch(u, "@made.far"))))

def Made.waiting(+u: Ui) -> Bool:
  Bool.or(Bool.not(String.is_empty(Ui.scratch(u, "@want.new"))), Made.coming(u))

def Bots.wanted.k(bound: Bool, +u: Ui) -> Ui:
  match bound:
    case True{}:
      Want.thread.k(Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@thread.want"))), Want.thread.exists(Ui.state.of(u), Ui.scratch(u, "@thread.want"))),
        Bots.wanted.m(BT.World.find(M.State.bots(Ui.state.of(u)), Ui.scratch(u, "@bot.want")), u), Ui.scratch(u, "@thread.want"))
    case False{}:
      u

def Bots.wanted.any(pending: Bool, +u: Ui) -> Ui:
  match pending:
    case True{}:
      Bots.wanted.k(Want.bound(u), u)
    case False{}:
      u

def Bots.wanted(+u: Ui) -> Ui:
  Bots.wanted.any(Bool.or(Bool.not(String.is_empty(Ui.scratch(u, "@bot.want"))), Bool.not(String.is_empty(Ui.scratch(u, "@thread.want")))), u)

# Adoption installs the complete state before resolving a pending want
# and restoring that thread's viewer. No empty-log intermediate selection.
def Ui.adopt(+u: Ui, st: M.State) -> Ui:
  Ui.reselect(Ui.sel(u), Bots.wanted(Ui.installed(Ui.adopt.raw(u, st))))

def Ui.join(log: Bool, u: Ui) -> Ui:
  match log:
    case True{}:
      Ui.installed(u)
    case False{}:
      u

# Joining in chunks: a hub sends a long log a chunk at a time, the first
# ("log") saying "more", the last ("changes") "boot": false. Until then
# the client is still catching up ("@boot" in the scratch): what it folds
# is history, so nothing compares it for alerts (mobile/hubs.bend's
# One.got) (laws boot_no_alerts, ui_chunked_join_equals_whole)
def Boot.ends(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.Flag{False{}}}:
      True{}
    case _:
      False{}

def Boot.end(ends: Bool, u: Ui) -> Ui:
  match ends:
    case True{}:
      Ui.scratch_set(u, "@boot", "")
    case False{}:
      u

# a changes frame with "boot": true starts (or goes on with) a catch-up:
# a live client that fell behind takes its log's changes this way, with
# no "log" frame saying "more" before them
def Boot.begins(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.Flag{True{}}}:
      True{}
    case _:
      False{}

def Boot.begin(begins: Bool, u: Ui) -> Ui:
  match begins:
    case True{}:
      Ui.scratch_set(u, "@boot", "1")
    case False{}:
      u

def Boot.mark(i: Nat, u: Ui, +o: J.Json) -> Ui:
  match i:
    case 0n:
      Ui.scratch_set(u, "@boot", Bool.pick(String, J.Json.get_bool(o, "more", False{}), "1", ""))
    case 1n:
      Boot.begin(Boot.begins(J.Json.get(o, "boot")), Boot.end(Boot.ends(J.Json.get(o, "boot")), u))
    case _:
      u

# a chunk of a join (for laws): a log saying more, a changes with boot
def Boot.demo(+t: String, +more: Bool, +boot: Bool, xs: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{t}}, J.KV{"more", J.Json.bool(more)}, J.KV{"boot", J.Json.bool(boot)}, J.KV{"items", J.Json.arr(xs)}])

# whether the client is still taking a log in chunks
def Boot.on(+u: Ui) -> Bool:
  String.eq(Ui.scratch(u, "@boot"), "1")

# The cache contains home and at most one linked hub; each slot keeps its
# read model and machine info (including the log origin) together.
type HubView is Data:
  HubView{hub: String, state: M.State, info: Map<&2, String>}

def HubView.hub(v: HubView) -> String:
  match v:
    case HubView{hub, _, _}:
      hub

def HubCache.home.k(home: Bool, v: HubView, rest: List<&2, HubView>) -> List<&2, HubView>:
  match home:
    case True{}:
      [v]
    case False{}:
      rest

def HubCache.home(vs: List<&2, HubView>) -> List<&2, HubView>:
  match vs:
    case Nil{}:
      Nil{}
    case +v <> t:
      HubCache.home.k(String.is_empty(HubView.hub(v)), v, HubCache.home(t))

def HubCache.remote.k(home: Bool, v: HubView, rest: List<&2, HubView>) -> List<&2, HubView>:
  match home:
    case True{}:
      rest
    case False{}:
      [v]

def HubCache.remote(vs: List<&2, HubView>) -> List<&2, HubView>:
  match vs:
    case Nil{}:
      Nil{}
    case +v <> t:
      HubCache.remote.k(String.is_empty(HubView.hub(v)), v, HubCache.remote(t))

def HubCache.put.k(home: Bool, v: HubView, vs: List<&2, HubView>) -> List<&2, HubView>:
  match home:
    case True{}:
      v <> HubCache.remote(vs)
    case False{}:
      v <> HubCache.home(vs)

def HubCache.put(+v: HubView, vs: List<&2, HubView>) -> List<&2, HubView>:
  HubCache.put.k(String.is_empty(HubView.hub(v)), v, vs)

def HubCache.get.k(same: Bool, v: HubView, rest: Maybe<&2, HubView>) -> Maybe<&2, HubView>:
  match same:
    case True{}:
      Some{v}
    case False{}:
      rest

def HubCache.get(vs: List<&2, HubView>, +hub: String) -> Maybe<&2, HubView>:
  match vs:
    case Nil{}:
      None{}
    case +v <> t:
      HubCache.get.k(String.eq(HubView.hub(v), hub), v, HubCache.get(t, hub))

def Ui.infos(u: Ui) -> Map<&2, String>:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, _, _}:
      nf

def HubCache.save.k(installed: Bool, +u: Ui, vs: List<&2, HubView>) -> List<&2, HubView>:
  match installed:
    case True{}:
      HubCache.put(HubView{Ui.info(u, "local.machine"), Ui.state.of(u), Map.del(&2, String, Ui.infos(u), "accounts.live")}, vs)
    case False{}:
      vs

def HubCache.save(+u: Ui, vs: List<&2, HubView>) -> List<&2, HubView>:
  HubCache.save.k(Bool.and(String.eq(Ui.info(u, "local.machine"), Ui.info(u, "local.state.hub")), String.eq(Ui.info(u, "local.generation"), Ui.info(u, "local.state.gen"))), u, vs)

def Ui.cached.raw(u: Ui, v: HubView) -> Ui:
  match u v:
    case Ui{_, sel, dr, live, op, on, now, ad, pa, ver, rid, w, er, note, +nf, ob, tr} HubView{_, st, info}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ver, rid, w, er, note, Local.keep(nf, info), ob, tr}

def Ui.cached.k(same: Bool, +u: Ui, v: HubView) -> Ui:
  match same:
    case True{}:
      Ui.reselect(Ui.sel(u), Bots.wanted(Ui.installed(Ui.cached.raw(u, v))))
    case False{}:
      u

def Ui.cached(m: Maybe<&2, HubView>, +u: Ui) -> Ui:
  match m:
    case Some{+v}:
      Ui.cached.k(String.eq(HubView.hub(v), Ui.info(u, "local.machine")), u, v)
    case None{}:
      u

# Whether the actual main area uses the thread timeline. Rooms and remote
# conversations have their own posts, and every bot tab except Chat has
# its own pane. Keep this decision before any wrapping work.
def Timeline.visible(settings: Bool, selected: Bool, thread: Bool) -> Bool:
  match settings selected thread:
    case False{} True{} True{}:
      True{}
    case _ _ _:
      False{}

# whether `sel` is the thread a bot's view is on: its own, or (the
# uberbot, whose conversation runs on one machine at a time) any of its
# replicas' (ubersync.bend's Ub.chat.is)
def Bots.chat.is(+st: M.State, +id: String, +th: String, +sel: String) -> Bool:
  Bool.or(String.eq(th, sel), Bool.and(UB.Uber.is(st, id), US.Ub.chat.is(st, sel)))

# No explicit bot tab is Chat, including the main bot. An explicitly picked
# operational tab still leaves its conversation timeline inactive.
def Timeline.bot(m: Maybe<&2, BT.Bot>, +u: Ui) -> Bool:
  match m:
    case Some{BT.Bot{+bid, _, +th, _, _, _, _}}:
      Bool.or(Bool.not(Bots.chat.is(Ui.state.of(u), bid, th, Ui.sel(u))), Bool.or(String.is_empty(Ui.scratch(u, "@btab")), String.eq(Ui.scratch(u, "@btab"), "chat")))
    case None{}:
      True{}

def Timeline.room(r: BT.Room, +u: Ui) -> Bool:
  match r:
    case BT.Room{_, _, _, _, deleted}:
      Bool.and(deleted, Bool.and(String.is_empty(Ui.scratch(u, "@remote")), Timeline.bot(BT.World.find.id(BT.World.bot(M.State.bots(Ui.state.of(u)), Ui.scratch(u, "@bot"))), u)))

def Ui.timeline(+u: Ui) -> Bool:
  Timeline.visible(Ui.flag(u, "settings"), Bool.not(String.is_empty(Ui.sel(u))), Timeline.room(BT.World.room(M.State.bots(Ui.state.of(u)), Ui.scratch(u, "@room")), u))

def Ui.shows.k(delta: Bool, +sel: String, +o: J.Json) -> Bool:
  match delta:
    case True{}:
      String.eq(J.Json.get_str(o, "thread", ""), sel)
    case False{}:
      True{}

# whether a message from the server can change what the window shows: a
# thread's streamed text is drawn only while that thread is open, so
# another thread's delta asks for no frame (with fifty agents streaming,
# a frame per delta kept the window drawing all the time; laws
# shows_other_delta, shows_own_delta, shows_changes)
def Ui.shows.t(beat: Bool, +u: Ui, +o: J.Json) -> Bool:
  match beat:
    case True{}:
      False{}
    case False{}:
      Ui.shows.k(String.eq(J.Json.get_str(o, "t", ""), "delta"), Ui.sel(u), o)

# (a heartbeat, {"t":"hb"}, draws nothing: law hb_shows_nothing; nor does
# the end of a thread's streamed text, {"t":"delta_end"}, which this client
# does not read: Ui.recv leaves it alone)
def Ui.shows(+u: Ui, +o: J.Json) -> Bool:
  Ui.shows.t(Bool.or(String.eq(J.Json.get_str(o, "t", ""), "hb"), String.eq(J.Json.get_str(o, "t", ""), "delta_end")), u, o)

# Desks
# -----
# The window and the web page (never a phone) tell the hub which thread
# they show and whether they have the focus, when either changes and at
# most every beat while in use (core/desk.bend's Desk.due). The hub then
# holds phone alerts for a thread a desk watches, and sends the desk used
# last a desktop notification for the rest. A new connection resets
# "@desk" (Ui.online), so it hears the look again.

def Ui.desk.k(due: Bool, +u: Ui, +thread: String, +focused: Bool, +now: Nat) -> Act:
  match due:
    case True{}:
      Ui.rpc(Ui.scratch_set(Ui.scratch_set(u, "@desk", DK.Desk.said(thread, focused)), "@desk.at", Nat.show(now)), "desk.look", DK.Desk.params(thread, focused))
    case False{}:
      Act{u, Nil{}}

# a new connection to a hub (the window's link): the desk says its look again
def Ui.desk.reset(u: Ui) -> Ui:
  Ui.scratch_set(u, "@desk", "")

# Pages of a timeline
# -------------------
# A thread can hold thousands of entries; a client shows its newest
# Page.size(), and "earlier" (value: the thread, "" for the selected one)
# shows Page.size() more each time ("@more.<thread>" counts the presses).

def Page.size() -> Nat:
  60n

def Page.more(+u: Ui, +thread: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Ui.scratch(u, "@more." ++ thread)), 0n)

# how many of a thread's newest entries are shown
def Page.count(+more: Nat) -> Nat:
  Nat.mul(Page.size(), 1n+more)

def Page.shown(+u: Ui, +thread: String) -> Nat:
  Page.count(Page.more(u, thread))

def Page.earlier.k(+u: Ui, +thread: String) -> Ui:
  Ui.scratch_set(u, "@more." ++ thread, Nat.show(1n+Page.more(u, thread)))

def Page.earlier(+u: Ui, +value: String) -> Ui:
  Page.earlier.k(u, Bool.pick(String, String.is_empty(value), Ui.sel.of(u), value))

# Reading while a thread grows (all four clients)
# ------------------------------------------------
# A reader scrolled up the timeline keeps what they read where it is: new
# entries, streamed text and panels changing move nothing on screen; only
# a view at its end (within Hold.slack() px) follows. Each client keeps
# its own place (the window: nui.bend's Hold, layout.bend's TL.hold; the
# web: host.js's held; the phones: their lists) and says here when its
# view leaves the end ("hold-away") and comes back ("hold-end"), the value
# the thread ("" the selected one). Away, the thread's newest entry then
# is kept as "@away.<thread>", and what came after it is the "N new"
# pill that jumps back down (Hold.label).

def Hold.slack() -> Nat:
  4n

# a view this far from its end is at the end, and follows
def Hold.at_end(+gap: Nat) -> Bool:
  Nat.is_le(gap, Hold.slack())

def Hold.key(+u: Ui, +value: String) -> String:
  "@away." ++ Bool.pick(String, String.is_empty(value), Ui.sel.of(u), value)

# the newest entry's id ("-" for none: anything later is new)
def Hold.newest(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      "-"
    case e <> _:
      M.Idx.id(e)

def Hold.mark.k(has: Bool, +u: Ui, +k: String, +th: String) -> Ui:
  match has:
    case True{}:
      u
    case False{}:
      Ui.scratch_set(u, k, Hold.newest(M.State.mine(Ui.state.of(u), th)))

def Hold.thread(+u: Ui, +value: String) -> String:
  Bool.pick(String, String.is_empty(value), Ui.sel.of(u), value)

# away: the newest entry is kept (being away again keeps the first mark)
def Hold.away(+u: Ui, +value: String) -> Ui:
  Hold.mark.k(Bool.not(String.is_empty(Ui.scratch(u, Hold.key(u, value)))), u, Hold.key(u, value), Hold.thread(u, value))

def Hold.end(+u: Ui, +value: String) -> Ui:
  Ui.scratch_set(u, Hold.key(u, value), "")

def Hold.is_away(+u: Ui, +th: String) -> Bool:
  Bool.not(String.is_empty(Ui.scratch(u, "@away." ++ th)))

# where the window's timeline starts reading once its top moved from t0
# to t1 (a panel above it grew or shrank), from off pixels hidden above:
# what was read stays where it was on screen (law hold_off_still), at most
# over (the timeline's end)
def Hold.off(+off: Nat, +t0: Nat, +t1: Nat, +over: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(Nat.sub(Nat.add(off, t1), t0), over), Nat.sub(Nat.add(off, t1), t0), over)

def Hold.most() -> Nat:
  99n

def Hold.msg(e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, _, _, _}:
      1n
    case M.Act{_, _, _, _, _, _}:
      0n

def Hold.stop(es: List<&2, M.Entry>, +seen: String, +n: Nat) -> Bool:
  match es:
    case Nil{}:
      True{}
    case e <> _:
      Bool.or(String.eq(M.Idx.id(e), seen), Nat.is_ge(n, Hold.most()))

# the messages after the entry seen (newest first: the walk stops at it,
# and at Hold.most())
def Hold.fresh(es: List<&2, M.Entry>, stop: Bool, +seen: String, +n: Nat) -> Nat:
  match es stop:
    case +_ True{}:
      n
    case Nil{} False{}:
      n
    case +e <> t False{}:
      Hold.fresh(t, Hold.stop(t, seen, Nat.add(n, Hold.msg(e))), seen, Nat.add(n, Hold.msg(e)))

def Hold.count.k(+es: List<&2, M.Entry>, +seen: String) -> Nat:
  Hold.fresh(es, Hold.stop(es, seen, 0n), seen, 0n)

# something is new: an entry after the one seen, or a reply streaming in
def Hold.any.k(+es: List<&2, M.Entry>, +seen: String, +live: String) -> Bool:
  Bool.or(Bool.not(String.eq(Hold.newest(es), seen)), Bool.not(String.is_empty(live)))

# "3 new ↓" (messages), "New ↓" (other entries, or a reply streaming in),
# "" for nothing new
def Hold.label(+n: Nat, any: Bool) -> String:
  Bool.pick(String, Nat.is_gt(n, 0n), Bool.pick(String, Nat.is_ge(n, Hold.most()), "99+ new ↓", Nat.show(n) ++ " new ↓"), Bool.pick(String, any, "New ↓", ""))

def Hold.pill.k(away: Bool, +u: Ui, +th: String) -> String:
  match away:
    case True{}:
      Hold.label(Hold.count.k(M.State.mine(Ui.state.of(u), th), Ui.scratch(u, "@away." ++ th)),
        Hold.any.k(M.State.mine(Ui.state.of(u), th), Ui.scratch(u, "@away." ++ th), Ui.live.of(u, th)))
    case False{}:
      ""

# the pill a thread's view shows while away ("" at the end)
def Hold.pill(+u: Ui, +th: String) -> String:
  Hold.pill.k(Hold.is_away(u, th), u, th)

# Extra entries received since the reader left the end. The page grows
# by these entries so its old first entry cannot be evicted underneath it.
def Hold.arrived(es: List<&2, M.Entry>, stop: Bool, +seen: String, +n: Nat) -> Nat:
  match es stop:
    case _ True{}:
      n
    case Nil{} False{}:
      n
    case e <> +t False{}:
      Hold.arrived(t, String.eq(Hold.newest(t), seen), seen, 1n+n)

def Hold.page(+es: List<&2, M.Entry>, +seen: String, +base: Nat) -> Nat:
  match seen:
    case SNil{}:
      base
    case SCon{_, _}:
      Nat.add(base, Hold.arrived(es, String.eq(Hold.newest(es), seen), seen, 0n))

def Hold.act(+u: Ui, +action: String, +value: String) -> Act:
  Bool.pick(Act, String.eq(action, "hold-away"), Act{Hold.away(u, value), Nil{}},
    Bool.pick(Act, String.eq(action, "hold-end"), Act{Hold.end(u, value), Nil{}},
      Bool.pick(Act, String.eq(action, "hold-bottom"), Act{Hold.end(u, value), [ScrollEnd{}]}, Act{u, Nil{}})))

def Bots.tab.with(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Act{u, ScrollEnd{} <> cs}

# a bot's tab; Settings asks Google's status too, so the accounts signed
# in (perhaps in a browser since) are the hub's
def Bots.tab.k(settings: Bool, u: Ui) -> Act:
  match settings:
    case True{}:
      Bots.tab.with(Bots.google.rpc(u, "status", ""))
    case False{}:
      Act{u, [ScrollEnd{}]}

# ---- links (cards, trail) ----
# Scratch storage avoids changing Ui's native representation.
def Trail.read.list(+s: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(s), Nil{}, R.Ref.split(s, '\n'))

def Trail.read(u: Ui) -> RE.Trail:
  match u:
    case Ui{_, _, +dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      RE.Trail{Trail.read.list(Str.get(dr, "@trail.b")), Str.get(dr, "@trail.c"), Trail.read.list(Str.get(dr, "@trail.f"))}

def Trail.store(+u: Ui, +t: RE.Trail) -> Ui:
  match t:
    case RE.Trail{+bs, +c, +fs}: Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@trail.b", String.join(bs, "\n")), "@trail.c", c), "@trail.f", String.join(fs, "\n"))

def Here.choose(+bot: String, +room: String, +remote: String, +th: String, +project: String) -> String:
  match bot room remote th project:
    case SCon{+h, +t} _ _ _ _: R.Ref.uri(R.Ref.bot(SCon{h, t}))
    case SNil{} SCon{+h, +t} _ _ _: R.Ref.uri(R.Ref.room(SCon{h, t}))
    case SNil{} SNil{} SCon{+h, +t} _ _: R.Ref.uri(R.Ref.bot(SCon{h, t}))
    case SNil{} SNil{} SNil{} SCon{+h, +t} _: R.Ref.uri(R.Ref.thread(SCon{h, t}))
    case SNil{} SNil{} SNil{} SNil{} SCon{+h, +t}: R.Ref.uri(R.Ref.project(SCon{h, t}))
    case _ _ _ _ _: ""

def Here.ref(u: Ui) -> String:
  match u:
    case Ui{+st, +sel, +dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Here.choose(Str.get(dr, "@bot"), Str.get(dr, "@room"), Str.get(dr, "@remote"), sel,
        Bool.pick(String, String.is_empty(sel), "", Ui.thread_project(Ui.thread(st, sel))))

def RefNav.note(+u: Ui, +text: String) -> Act:
  match u:
    case Ui{+st, +sel, +dr, +live, +op, +on, +now, +ad, +pa, +v, +rid, +w, +er, _, +nf, +ob, +tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, text, nf, ob, tr}, Nil{}}

def RefNav.jump(+a: Act, +frag: String) -> Act:
  match a:
    case Act{+u, +cs}: Act{u, List.append(&2, Cmd, cs, Bool.pick(List<&2, Cmd>, String.is_empty(frag), Nil{}, [Jump{frag}]))}

def RefNav.routine(+a: Act) -> Act:
  match a:
    case Act{+u, +cs}: Act{Ui.scratch_set(u, "@btab", "routines"), cs}

def RefNav.activity(+t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, +at, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: at

def RefNav.newer.k(+yes: Bool, +t: M.Thread, +best: M.Thread) -> M.Thread:
  match yes:
    case True{}: t
    case False{}: best

# One walk on click; retain the current project thread, otherwise choose
# the most recently active live thread. No timeline or sidebar scan.
def RefNav.newer(+t: M.Thread, +best: M.Thread, +pid: String) -> M.Thread:
  match t:
    case M.Thread{_, +p, _, _, +at, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +gone}:
      RefNav.newer.k(Bool.and(Bool.and(String.eq(p, pid), Bool.not(gone)), Nat.is_ge(at, RefNav.activity(best))), t, best)

def RefNav.latest(+ts: List<&2, M.Thread>, +pid: String, +best: M.Thread) -> String:
  match ts:
    case Nil{}: Bool.pick(String, RE.Card.thread.live(best), M.Thread.id(best), "")
    case +t <> +rest: RefNav.latest(rest, pid, RefNav.newer(t, best, pid))

def RefNav.home.choose(here: Bool, st: M.State, sel: String, pid: String) -> String:
  match here:
    case True{}: sel
    case False{}: RefNav.latest(Ui.threads(st), pid, M.Thread.blank(""))

def RefNav.home(u: Ui, +pid: String) -> String:
  match u:
    case Ui{+st, +sel, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      RefNav.home.choose(String.eq(Ui.thread_project(Ui.thread(st, sel)), pid), st, sel, pid)

def RefNav.file.open(+a: Act, +path: String) -> Act:
  match a:
    case Act{+u, +cs}: Ui.also(Fm.open.k(Bool.pick(Nat, String.eq(FM.Fm.kind(path), "pdf"), 1n, 0n), u, FM.Fm.kind(path), path), cs)

def RefNav.file.k(+view: Bool, +u: Ui, +pid: String, +path: String) -> Act:
  match view:
    case True{}: RefNav.file.open(Ui.select(u, RefNav.home(u, pid)), path)
    case False{}: Flink.open(u, path, "open")

def RefNav.file(+u: Ui, +pid: String, +xs: List<&2, String>) -> Act:
  +path = FM.Fm.abs(RE.Card.project.root(Side.st(u), pid), String.join(xs, "/"))
  RefNav.file.k(Bool.not(String.is_empty(FM.Fm.kind(path))), u, pid, path)

def RefNav.task.thread(+u: Ui, +th: String) -> Act:
  match th:
    case SNil{}: RefNav.note(u, MSG.T.nav.task.none(Lang.loc(u)))
    case SCon{+h, +t}: Ui.select(u, SCon{h, t})

def RefNav.task.json(+u: Ui, +j: J.Json) -> Act:
  RefNav.task.thread(u, J.Json.get_str(j, "thread", ""))

def RefNav.task(+u: Ui, +id: String) -> Act:
  RefNav.task.json(u, Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(Side.st(u), "board." ++ id, "")), J.Null{}))

def RefNav.pr(+ok: Bool, +u: Ui, +repo: String, +n: String) -> Act:
  match ok:
    case True{}:
      # The existing file.open system opener accepts URLs too. No new
      # Cmd variant (which would break every client's exhaustive match).
      Ui.rpc(u, "file.open", [J.KV{"path", J.Str{"https://github.com/" ++ repo ++ "/pull/" ++ n}}, J.KV{"how", J.Str{"open"}}])
    case False{}: RefNav.note(u, MSG.T.nav.pr.norepo(Lang.loc(u)))

def RefNav.machine.go(+rs: List<&2, MRow>, +id: String, +found: String) -> String:
  match rs:
    case Nil{}: found
    case MRow{+name, +url, _} <> +t:
      RefNav.machine.go(t, id, Bool.pick(String, Bool.or(String.eq(name, id), String.eq(url, id)), url, found))

def RefNav.machine(+rs: List<&2, MRow>, +id: String) -> String:
  RefNav.machine.go(rs, id, id)

def RefNav.machine.named(+u: Ui, name: String) -> Act:
  Ui.machine(u, RefNav.machine(Machines.rows.loc(Lang.loc(u), u), name))

def RefNav.machine.visit(+u: Ui, id: String) -> Act:
  RefNav.machine.named(u, FR.Fr.name(Side.st(u), id ++ "~x"))

def RefNav.keep(+a: Act, +trail: RE.Trail) -> Act:
  match a:
    case Act{+u, +cs}: Act{Trail.store(u, trail), cs}

def RefNav.seed.k(empty: Bool, +u: Ui, +t: RE.Trail) -> RE.Trail:
  match empty t:
    case True{} RE.Trail{bs, _, fs}: RE.Trail{bs, Here.ref(u), fs}
    case False{} _: RE.Trail.go(t, Here.ref(u))

def RefNav.seed.from(u: Ui, +t: RE.Trail) -> RE.Trail:
  RefNav.seed.k(String.is_empty(RE.Trail.cur(t)), u, t)

def RefNav.seed(+u: Ui) -> RE.Trail:
  RefNav.seed.from(u, Trail.read(u))

def RefNav.copy.base(u: Ui, r: R.Ref, +base: String) -> Act:
  Act{u, [Copy{R.Ref.web(Bool.pick(String, String.is_empty(base), "http://127.0.0.1:3787", base), r)}]}

def RefNav.copy.web(m: Maybe<&2, R.Ref>, +u: Ui) -> Act:
  match m:
    case None{}: Act{u, Nil{}}
    case Some{r}: RefNav.copy.base(u, r, Ui.info(u, "hub.base"))

# Settings navigation is client-local; choosing a page never persists a setting.
# (a page's place here is its number in every client's rows: a new page
# goes at the end; the menus list them in their own order below)
def Settings.pages() -> List<&2, String>:
  ["appearance", "threads", "agents", "writing", "cad", "network", "voice", "keys", "antigravity", "timezone", "usage", "about", "privacy", "ordering"]

# the order the desktop and web menus list the pages in
def Settings.menu.pages() -> List<&2, String>:
  ["appearance", "threads", "agents", "writing", "cad", "ordering", "network", "voice", "keys", "antigravity", "timezone", "usage", "about", "privacy"]

def Settings.phone.pages() -> List<&2, String>:
  ["appearance", "threads", "agents", "cad", "ordering", "network", "keys", "antigravity", "timezone", "usage", "about", "privacy"]

def Settings.known(ps: List<&2, String>, +page: String) -> Bool:
  match ps:
    case Nil{}: False{}
    case +p <> t: Bool.or(String.eq(p, page), Settings.known(t, page))

def Settings.clean(+page: String) -> String:
  Bool.pick(String, Settings.known(Settings.pages(), page), page, "")

# Webhook (core/wnote.bend) in Settings: the address and the secret
# file's path, one list of rows all four clients draw. The secret's
# contents are never read and never shown; only the path is.
def Hookv.bad(+flag: String) -> Bool:
  String.eq(flag, "bad")

def Hookv.url.note.k(bad: Bool, +l: I.Loc) -> String:
  match bad:
    case True{}:
      MSG.T.hook.url.bad(l)
    case False{}:
      MSG.T.hook.url.note(l)

def Hookv.url.note(+l: I.Loc, +flag: String) -> String:
  Hookv.url.note.k(Hookv.bad(flag), l)

def Hookv.keep(+raw: String) -> Bool:
  Bool.or(String.is_empty(raw), WN.Wn.url.ok(raw))

def Hookv.rows.loc(+l: I.Loc, +u: Ui) -> List<&2, AgRow>:
  [AgRow{MSG.T.hook.title(l), MSG.T.hook.lead(l), Nil{}, Nil{}},
   AgRow{MSG.T.hook.url(l), Hookv.url.note(l, Ui.scratch(u, "@hook.err")),
     [AgField{"whurl", MSG.T.hook.url.hint(l), False{}, Ui.scratch(u, "@f.whurl"), "hook-url"}],
     [AgBtn{MSG.T.common.save(l), "hook-url", "", False{}}]},
   AgRow{MSG.T.hook.secret(l), MSG.T.hook.secret.note(l),
     [AgField{"whsec", MSG.T.hook.secret.hint(l), False{}, Ui.scratch(u, "@f.whsec"), "hook-secret"}],
     [AgBtn{MSG.T.common.save(l), "hook-secret", "", False{}}]}]

def Hookv.rows(+u: Ui) -> List<&2, AgRow>:
  Hookv.rows.loc(Lang.loc(u), u)

# a field's Enter sends its text; a Save button sends nothing and the scratch holds it
def Hookv.take(empty: Bool, +u: Ui, +field: String, +value: String) -> Ui:
  match empty:
    case True{}:
      u
    case False{}:
      Ui.scratch_set(u, field, value)

def Hookv.url.k(ok: Bool, +u: Ui, +raw: String) -> Act:
  match ok:
    case True{}:
      Ui.setting(Ui.scratch_set(Ui.scratch_set(u, "@hook.err", ""), "@f.whurl", raw), WN.Wn.url.key() ++ "=" ++ raw)
    case False{}:
      Act{Ui.scratch_set(u, "@hook.err", "bad"), Nil{}}

def Hookv.url(+u: Ui, +value: String) -> Act:
  +u2 = Hookv.take(String.is_empty(value), u, "@f.whurl", value)
  +raw = String.trim(Ui.scratch(u2, "@f.whurl"))
  Hookv.url.k(Hookv.keep(raw), u2, raw)

def Hookv.secret(+u: Ui, +value: String) -> Act:
  +u2 = Hookv.take(String.is_empty(value), u, "@f.whsec", value)
  +raw = String.trim(Ui.scratch(u2, "@f.whsec"))
  Ui.setting(Ui.scratch_set(u2, "@f.whsec", raw), WN.Wn.secret.key() ++ "=" ++ raw)

def Hookv.act.k(url: Bool, +u: Ui, +value: String) -> Act:
  match url:
    case True{}:
      Hookv.url(u, value)
    case False{}:
      Hookv.secret(u, value)

def Hookv.act(+u: Ui, +action: String, +value: String) -> Act:
  Hookv.act.k(String.eq(action, "hook-url"), u, value)

# the outgoing webhook's two actions; every other hook-* action (Create,
# Revoke, the token's show, rotate and off) is an incoming hook's and
# goes on to the bot-action table
def Hookv.ours(+action: String) -> Bool:
  Bool.or(String.eq(action, "hook-url"), String.eq(action, "hook-secret"))

def Settings.page(+u: Ui) -> String:
  Settings.clean(Ui.scratch(u, "@settings.page"))

def Settings.label(i: Nat, +l: I.Loc) -> String:
  match i:
    case 0n: MSG.T.settings.appearance(l)
    case 1n: MSG.T.tour.title.threads(l)
    case 2n: MSG.T.settings.agents(l)
    case 3n: MSG.T.settings.writing(l)
    case 4n: MSG.T.settings.cad(l)
    case 5n: MSG.T.settings.network(l)
    case 6n: MSG.T.settings.voice(l)
    case 7n: MSG.T.settings.keys(l)
    case 8n: "Antigravity"
    case 9n: MSG.T.settings.zone(l)
    case 10n: MSG.T.settings.usage(l)
    case 12n: MSG.T.settings.privacy(l)
    case 13n: Fab.set.order.label()
    case _: MSG.T.settings.tour(l)

def Settings.title(+l: I.Loc, +page: String) -> String:
  Bool.pick(String, String.is_empty(page), MSG.T.palette.settings(l), Settings.label(C.Codec.pos(Settings.pages(), page), l))

# Settings' search (the field "setq", "@f.setq"): a page is found when its
# title, a row on it or a word about it holds the query, whatever the case;
# an empty query finds every page. Its hint names the rows that matched.
def Settings.q(+u: Ui) -> String:
  String.to_lower(String.trim(Ui.scratch(u, "@f.setq")))

# the rows a page shows, as named on it
def Settings.rows.of(i: Nat, +l: I.Loc) -> List<&2, String>:
  match i:
    case 0n: [MSG.T.settings.theme(l), MSG.T.settings.lang(l), MSG.T.settings.sidebar(l), MSG.T.settings.textsize(l)]
    case 1n: [MSG.T.settings.settle(l), MSG.T.settings.settle.done(l), MSG.T.settings.restart(l), MSG.T.hook.title(l), MSG.T.hook.url(l), MSG.T.hook.secret(l)]
    case 2n: [MSG.T.settings.provider(l), MSG.T.settings.model(l), Think.label()]
    case 3n: [MSG.T.settings.writing.provider(l)]
    case 4n: ["KiCad", "FreeCAD", "KiCad IPC"]
    case 5n: ["Tailscale", MSG.T.settings.pair(l), MSG.T.settings.channel(l)]
    case 11n: [MSG.T.settings.tour(l)]
    case 10n: [UZ.Ts.label()]
    case 13n: [Fab.set.order.label(), Fab.set.label()]
    case _: Nil{}

# words about a page that its rows do not say
def Settings.words(i: Nat) -> String:
  match i:
    case 0n: "appearance dark light theme font text size zoom scale language sidebar"
    case 1n: "threads settle done finished restart continue notifications alerts notify webhook hook url address secret post"
    case 2n: "agents provider model claude codex grok opencode muse pi omp effort thinking reasoning chain of thought"
    case 3n: "writing titles summaries text model"
    case 4n: "cad kicad freecad ipc path schematic board"
    case 5n: "network tailscale tailnet pairing pair phone qr machines sync update channel nightly stable version"
    case 6n: "voice microphone dictation transcribe speech openai"
    case 7n: "keys api secrets tokens environment"
    case 8n: "antigravity google gemini sign in"
    case 9n: "timezone time zone clock"
    case 10n: "usage limits tokens cost token stats speed tok/s context cache"
    case 11n: "about tour help version"
    case 12n: "privacy share data"
    case 13n: "ordering order buy fab manufacturing quote price estimates stripe payment checkout fee"
    case _: ""

def Settings.has(+text: String, +q: String) -> Bool:
  String.contains(String.to_lower(text), q)

def Settings.rows.hit.k(hit: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}: x <> rest
    case False{}: rest

def Settings.rows.hit(xs: List<&2, String>, +q: String) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> t: Settings.rows.hit.k(Settings.has(x, q), x, Settings.rows.hit(t, q))

# the rows of a page the query names, for its hint ("" when none)
def Settings.hint(+l: I.Loc, +page: String, +q: String) -> String:
  Bool.pick(String, String.is_empty(q), "", String.join(Settings.rows.hit(Settings.rows.of(C.Codec.pos(Settings.pages(), page), l), q), " · "))

def Settings.hit(+l: I.Loc, +page: String, +q: String) -> Bool:
  Bool.or(String.is_empty(q), Bool.or(Settings.has(Settings.title(l, page), q),
    Bool.or(Settings.has(Settings.words(C.Codec.pos(Settings.pages(), page)), q), Bool.not(String.is_empty(Settings.hint(l, page, q))))))

def Settings.found.k(hit: Bool, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}: p <> rest
    case False{}: rest

# the pages of a menu the query finds, in the menu's order
def Settings.found(ps: List<&2, String>, +l: I.Loc, +q: String) -> List<&2, String>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t: Settings.found.k(Settings.hit(l, p, q), p, Settings.found(t, l, q))

def Settings.loc(+u: Ui) -> I.Loc:
  I.I18n.of_tag(Ui.scratch(u, "@lang.tag"))

# opening Threads fills the webhook fields from the saved settings, so a
# Save does not wipe an address the person has not retyped
def Settings.seed(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@f.whurl", M.Setting.get(Ui.state.of(u), WN.Wn.url.key(), "")), "@f.whsec", M.Setting.get(Ui.state.of(u), WN.Wn.secret.key(), "")), "@hook.err", "")

def Settings.seed.k(threads: Bool, +u: Ui) -> Ui:
  match threads:
    case True{}:
      Settings.seed(u)
    case False{}:
      u

# Reentering Voice reloads its private dictionary and the hub's devices.
# A reconnect can replace the presentation without those transient replies.
def Settings.refresh(voice: Bool, a: Act) -> Act:
  match voice a:
    case True{} Act{u, cs}: Ui.also(Voice.rpc(u, "status", Nil{}), cs)
    case False{} _: a

def Settings.go.k(+page: String, +before: Ui) -> Act:
  +u = Ui.scratch_set(before, "@keys.rm", "")
  Settings.refresh(String.eq(page, "voice"), Act{Settings.seed.k(String.eq(page, "threads"), Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.flag_off(u, "pairqr"), "@settings.page", page), "@set.sc", "0"), "@set.focus", "")), [Focus{""}]})

def Settings.go(+u: Ui, +page: String) -> Act:
  Settings.go.k(Settings.clean(page), u)

def Settings.focus(a: Act, id: String) -> Act:
  match a:
    case Act{u, _}: Act{u, [Focus{id}]}

def KN.path(+u: Ui) -> Act:
  Settings.focus(Settings.go(Ui.scratch_set(Settings.on(u), "@f.kpath", M.Setting.get(Ui.state.of(u), "kicad.path", "")), "cad"), "kicad-path")

def KN.save(+u: Ui) -> Act:
  Ui.setting(u, "kicad.path=" ++ Ui.scratch(u, "@f.kpath"))

# the KiCad notice's actions (183 on: kicad-retry, kicad-path, kicad-path-save)
def KN.act(i: Nat, +u: Ui) -> Act:
  match i:
    case 0n: Ui.rpc(u, "kicad.retry", Nil{})
    case 1n: KN.path(u)
    case 2n: KN.save(u)
    case _: Act{u, Nil{}}

def Emb.has.k(r: Map<&2, VT.Vt> & Bool) -> Bool:
  (_, b) = r
  b

# what drawing a thread's terminals needs (every client): the screens,
# when the person last wrote in the thread, the clock
type Ec is Data:
  Ec{tr: Map<&2, VT.Vt>, last: Nat, now: Nat}

def Ec.none() -> Ec:
  Ec{Map.new(&2, VT.Vt), 0n, 0n}

def Ec.make(+tr: Map<&2, VT.Vt>, +st: M.State, +th: String, +now: Nat) -> Ec:
  Ec{tr, EM.Emb.last_user(M.State.mine(st, th), 0n), now}

def Ec.of(+u: Ui, +th: String) -> Ec:
  match u:
    case Ui{+st, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +tr}:
      Ec.make(MailvLocal.terms(tr), st, th, Side.now(u))

def Ec.vt(+ec: Ec, +key: String) -> VT.Vt:
  match ec:
    case Ec{+tr, _, _}:
      Term.get(tr, key)

def Ec.view(+ec: Ec, +st: M.State, +e: M.Entry) -> EM.EV:
  match ec:
    case Ec{_, +last, +now}:
      EM.Emb.view(st, e, last, now)

def Ec.pins(+ec: Ec, +st: M.State, +th: String) -> List<&2, EM.EV>:
  match ec:
    case Ec{_, +last, +now}:
      EM.Emb.pins.go(M.State.terms(st, th), st, last, now)

# Terminals in the chat (core/emb.bend)
# -------------------------------------
# A live one the selected thread shows and this client holds no screen
# for is asked for once (term.snap, scratch "@emb.asked:<key>"); keys and
# pastes go to it by key; a click on an inactive one starts it again; the
# pin is the setting "emb.pin:<key>".

def Emb.has(+u: Ui, +key: String) -> Bool:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, tr}:
      Emb.has.k(Map.has(&2, VT.Vt, MailvLocal.terms(tr), key))

def Emb.views.k(is_t: Bool, +s: M.State, +e: M.Entry, +last: Nat, +now: Nat, rest: List<&2, EM.EV>) -> List<&2, EM.EV>:
  match is_t:
    case True{}:
      EM.Emb.view(s, e, last, now) <> rest
    case False{}:
      rest

# what the thread's terminals look like now (key, live, pinned, size): a
# tail call onto a reversed accumulator, as core/emb.bend's walks (built
# around the recursive call, a thread of thousands of entries overflowed
# the web's and phones' stack on every action, and it could not be opened)
def Emb.views.acc(es: List<&2, M.Entry>, +s: M.State, +last: Nat, +now: Nat, acc: List<&2, EM.EV>) -> List<&2, EM.EV>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Emb.views.acc(t, s, last, now, Emb.views.k(EM.Emb.is(e), s, e, last, now, acc))

def Emb.views.go(es: List<&2, M.Entry>, +s: M.State, +last: Nat, +now: Nat) -> List<&2, EM.EV>:
  List.reverse(&2, EM.EV, Emb.views.acc(es, s, last, now, Nil{}))

def Emb.views(+u: Ui, +th: String) -> List<&2, EM.EV>:
  Emb.views.go(M.State.mine(Ui.state(u), th), Ui.state(u), EM.Emb.last_user(M.State.mine(Ui.state(u), th), 0n), Side.now(u))

def Emb.want.k(need: Bool, +key: String, a: Act) -> Act:
  match need a:
    case True{} Act{u, cs}:
      Ui.also(Ui.rpc(Ui.scratch_set(u, "@emb.asked:" ++ key, "1"), "term.snap", [J.KV{"key", J.Str{key}}]), cs)
    case False{} _:
      a

def Emb.want.go(live: Bool, +key: String, a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Emb.want.k(Bool.and(live, Bool.and(Bool.not(Emb.has(u, key)), String.is_empty(Ui.scratch(u, "@emb.asked:" ++ key)))), key, Act{u, cs})

def Emb.want(vs: List<&2, EM.EV>, a: Act) -> Act:
  match vs:
    case Nil{}:
      a
    case EM.EV{+key, _, _, _, _, live, _} <> t:
      Emb.want(t, Emb.want.go(live, key, a))

def Emb.sync(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Emb.want(Emb.views(u, Ui.sel(u)), Act{u, cs})

# after a log, its changes or a view (not every delta)
def Emb.sync.if(i: Nat, a: Act) -> Act:
  match i:
    case 0n:
      Emb.sync(a)
    case 1n:
      Emb.sync(a)
    case 2n:
      Emb.sync(a)
    case _:
      a

# after an action that chose another thread
def Emb.sync.sel.k(moved: Bool, a: Act) -> Act:
  match moved:
    case True{}:
      Emb.sync(a)
    case False{}:
      a

def Emb.sync.sel(+old: String, a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Emb.sync.sel.k(Bool.not(String.eq(old, Ui.sel(u))), Act{u, cs})

def Ui.emb_keys(u: Ui, +key: String, bs: List<&2, U32>) -> Act:
  Ui.rpc(u, "term.input", [J.KV{"thread", J.Str{key}}, J.KV{"b", J.Json.arr(Term.nums(bs))}])

def Emb.key.k(ks: U32 & U32, +u: Ui, +key: String, +mods: U32) -> Act:
  (+sym, +cp) = ks
  Bool.pick(Act, U32.is_eq(sym, 0), Act{u, Nil{}}, Ui.emb_keys(u, key, VT.Vt.key(Ui.term(u, key), sym, cp, mods)))

def Emb.part(+ps: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, ps, i), "")

# "key<tab>DOM key<tab>mods" (the web and phones)
def Emb.key(+u: Ui, +ps: List<&2, String>) -> Act:
  Emb.key.k(Keys.dom(Emb.part(ps, 1n)), u, Emb.part(ps, 0n), U32.from_nat(Maybe.default(&2, Nat, Nat.read(Emb.part(ps, 2n)), 0n)))

# "key<tab>text"
def Emb.paste(+u: Ui, +key: String, +text: String) -> Act:
  Ui.emb_keys(u, key, VT.Vt.paste(Ui.term(u, key), text))

# "thread<tab>id"
def Emb.restart(+u: Ui, +ps: List<&2, String>) -> Act:
  Ui.rpc(u, "term.restart", [J.KV{"thread", J.Str{Emb.part(ps, 0n)}}, J.KV{"id", J.Str{Emb.part(ps, 1n)}}])

def Emb.pin(+u: Ui, +key: String) -> Act:
  Ui.setting(u, EM.Emb.pin.key(key) ++ "=" ++ Bool.pick(String, EM.Emb.pinned(Ui.state(u), key), "", "1"))

def Emb.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Emb.key(u, String.split(value, '\t'))
    case 1n:
      Emb.paste(u, Emb.part(String.split(value, '\t'), 0n), String.join(List.drop(&2, String, String.split(value, '\t'), 1n), "\t"))
    case 2n:
      Emb.restart(u, String.split(value, '\t'))
    case 3n:
      Emb.pin(u, value)
    case _:
      Act{u, Nil{}}

def Emb.act(+u: Ui, +action: String, +value: String) -> Act:
  Emb.act.k(C.Codec.pos(["emb-key", "emb-paste", "emb-restart", "emb-pin"], action), u, value)

# The first-run tour (core/onboard.bend): the step on screen is the
# scratch "@tour" ("" and digits when it came up by itself, "m" and digits
# when Settings' "Show the tour" asked for it); the hub keeps only that it
# was finished or skipped, the setting "onboard.done", so every client of
# a hub agrees. A tour that came up by itself waits until this connection's
# log has arrived ("@sync", set by Ui.reset, cleared when the connection
# changes: a cached state is not current) and ends when the hub says it is
# done, even from another client. Actions "ob-next", "ob-back", "ob-skip"
# and "ob-open" (which shuts Settings: the tour shows over the usual
# screen). Leaving puts the keyboard in a field of the view on screen (a
# room's, a remote chat's, an open thread's composer), else nowhere, so no
# hidden field keeps it; the thread's buttons are only pointed at while its
# header shows.
def Tour.done(+st: M.State) -> Bool:
  String.eq(M.Setting.get(st, "onboard.done", ""), "1")

# ... and, on a long log sent in chunks, until its last chunk has come
# ("@boot" clear): the first said nothing of the "onboard.done" at its end
def Tour.ready(+u: Ui) -> Bool:
  Bool.and(Bool.and(Nat.is_gt(Ui.seq(u), 0n), String.eq(Ui.scratch(u, "@sync"), "1")), String.is_empty(Ui.scratch(u, "@boot")))

# Anonymous usage counts (Settings, the tour's "Anonymous usage" step;
# core/telemetry.bend, docs/telemetry.md). The hub-held settings: telemetry.share
# ("on", "off"; unset is on), telemetry.asked ("1" once the person answered
# or touched the row), telemetry.reset (a counter: a new id), and, written by
# the hub, telemetry.live ("1" when this build can send at all) and
# hub.platform (e.g. "linux-x64", for the bug report link). The install id
# is never shown or sent to a client.
def Share.on(+st: M.State) -> Bool:
  TLM.Tm.share(st)

def Share.asked(+st: M.State) -> Bool:
  TLM.Tm.asked(st)

def Share.live(+u: Ui) -> Bool:
  String.eq(Ui.info(u, "telemetry.live"), "1")

# the question is worth asking (a build that can send) and not yet answered
def Share.pending(+u: Ui) -> Bool:
  Bool.and(Share.live(u), Bool.not(Share.asked(Ui.state.of(u))))

def Share.privacy() -> String:
  "https://backplane.works/privacy"

def Share.note() -> String:
  "A random id, the version and OS, once a day. Nothing else."

# the note ends with the install id the hub reported (info telemetry.id), which
# the privacy page asks people to quote when they ask for their data to go
def Share.note.id(+id: String) -> String:
  Bool.pick(String, String.is_empty(id), Share.note(), Share.note() ++ " Install id: " ++ id)

def Share.title() -> String:
  "Usage"

def Share.ver(u: Ui) -> String:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, v, _, _, _, _, _, _, _}:
      v

# the issue form with the version, OS and architecture the hub reported
def Share.bug(+u: Ui) -> String:
  TLM.Bug.url(Share.ver(u), Ui.info(u, "os"), Ui.info(u, "arch"))

def Share.rows.k(+on: Bool, +bug: String, +note: String) -> List<&2, AgRow>:
  [AgRow{"Share anonymous usage", note, Nil{}, [AgBtn{"On", "share", "on", on}, AgBtn{"Off", "share", "off", Bool.not(on)}, AgBtn{"Reset id", "share-reset", "", False{}},
    AgBtn{"Privacy", "open-url", Share.privacy(), False{}}, AgBtn{"Report an issue", "open-url", bug, False{}}]}]

# the Settings section, drawn by every client as it draws Antigravity's rows
def Share.id(+u: Ui) -> String:
  Bool.pick(String, Share.on(Ui.state.of(u)), Ui.info(u, "telemetry.id"), "")

def Share.rows(+u: Ui) -> List<&2, AgRow>:
  Share.rows.k(Share.on(Ui.state.of(u)), Share.bug(u), Share.note.id(Share.id(u)))

# The uberbot step alone: an install that finished the tour before it had
# that step (or demoted its uberbot) is asked once more, until the step has
# been answered (the hub setting "onboard.uber": Done, Skip or a choice, in
# the full tour too). Only while no uberbot is set, only for a tour that came
# up by itself, and (like it) only once this connection's state has arrived.
def Tour.answered(+st: M.State) -> Bool:
  String.eq(M.Setting.get(st, "onboard.uber", ""), "1")

def Tour.uber.due(+st: M.State) -> Bool:
  Bool.and(Bool.not(Tour.answered(st)), String.is_empty(UB.Uber.bot(st)))

# Steps asked alone, once each, for an install that finished the tour before
# they existed: the usage question first (while this build can send), then
# the uberbot
def Tour.single.k(+st: M.State, +raw: String, pending: Bool) -> Bool:
  Bool.and(Bool.and(Tour.done(st), Bool.not(OB.Ob.manual(raw))), Bool.or(Tour.uber.due(st), pending))

def Tour.single(+u: Ui) -> Bool:
  Bool.and(Tour.ready(u), Tour.single.k(Ui.state.of(u), Ui.scratch(u, "@tour"), Share.pending(u)))

def Tour.at.k(+i: Nat, up: Bool, share: Bool) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_eq(i, OB.Ob.count()), up), Bool.pick(Nat, share, OB.Ob.share(), OB.Ob.uber()), i)

# Step 9 used to be the uberbot. A cached old tour keeps that meaning;
# newly reached usage steps carry an explicit s marker (m still means manual).
def Tour.legacy(+raw: String, +i: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_eq(i, OB.Ob.share()), Bool.not(String.contains(raw, "s"))), OB.Ob.uber(), i)

def Tour.at(+u: Ui) -> Nat:
  Tour.at.k(Tour.legacy(Ui.scratch(u, "@tour"), OB.Ob.at(Tour.ready(u), Tour.done(Ui.state.of(u)), Ui.scratch(u, "@tour"))), Tour.single(u), Share.pending(u))

# the forward button: a step asked alone is answered with Done
def Tour.forward(+u: Ui) -> String:
  Bool.pick(String, Tour.single(u), "Done", OB.Ob.forward(Lang.loc(u), Tour.at(u)))

# what the card says: no "10 of 10" for a step asked alone
def Tour.counter.loc(+l: I.Loc, +u: Ui) -> String:
  Bool.pick(String, Tour.single(u), "", OB.Ob.counter(l, Tour.at(u)))

def Tour.counter(+u: Ui) -> String:
  Tour.counter.loc(Lang.loc(u), u)

# whether the card has a Back (not the first step, not a step alone)
def Tour.back(+u: Ui) -> Bool:
  Bool.and(Nat.is_gt(Tour.at(u), 0n), Bool.not(Tour.single(u)))

def Tour.on(+u: Ui) -> Bool:
  OB.Ob.live(Tour.at(u))

# The KiCad notice waits while the tour is up (the card would sit over it,
# and it is one thing more at once): it shows once the tour ends
def Tour.kn.k(on: Bool, n: Maybe<&2, KH.KNote>) -> Maybe<&2, KH.KNote>:
  match on:
    case True{}:
      None{}
    case False{}:
      n

def Tour.kn(+u: Ui, n: Maybe<&2, KH.KNote>) -> Maybe<&2, KH.KNote>:
  Tour.kn.k(Tour.on(u), n)

# Scratch's lifetime in days (the "scratch.days" setting)
def Tour.days(+u: Ui) -> String:
  Nat.show(Nat.div(M.Scratch.window(Ui.state(u)), 86400n))

# The uberbot step (core/onboard.bend's Ob.uber): what it offers depends on
# the bots here. "have": an uberbot is set; "pick": bots, none set; "none":
# no bots. A bot is counted by the entries of its thread (only while the
# tour is up; M.State.mine reads the thread's own index).
type Used is Data:
  Used{n: Nat, bot: BT.Bot}

# most used first, ties the oldest first
def Used.before(a: Used, b: Used) -> Bool:
  match a b:
    case Used{+na, ba} Used{+nb, bb}:
      Bool.pick(Bool, Nat.is_gt(na, nb), True{}, Bool.pick(Bool, Nat.is_eq(na, nb), BT.Bot.before(ba, bb), False{}))

def Tour.used(bs: List<&2, BT.Bot>, +st: M.State) -> List<&2, Used>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      Used{List.length(&2, M.Entry, M.State.mine(st, BT.Bot.thread(b))), b} <> Tour.used(t, st)

# this machine's bots, the most used first
def Tour.ranked(+st: M.State) -> List<&2, Used>:
  List.sort(~Used, ~Used.before, Tour.used(BT.World.bots.here(M.State.bots(st)), st))

def Tour.uber.case.k(+have: String, any: Bool) -> String:
  Bool.pick(String, Bool.not(String.is_empty(have)), "have", Bool.pick(String, any, "pick", "none"))

def Tour.uber.case(+u: Ui) -> String:
  Tour.uber.case.k(UB.Uber.bot(Ui.state.of(u)), Bool.not(List.is_empty(&2, BT.Bot, BT.World.bots.here(M.State.bots(Ui.state.of(u))))))

def Tour.uber.name.k(uc: String, +first: String, +have: String) -> String:
  Bool.pick(String, String.eq(uc, "have"), have, first)

def Tour.uber.first(us: List<&2, Used>) -> String:
  match us:
    case Used{_, b} <> _:
      BT.Bot.name(b)
    case Nil{}:
      ""

# the name the words use: the uberbot's, or the bot used most
def Tour.uber.name(+u: Ui) -> String:
  Tour.uber.name.k(Tour.uber.case(u), Tour.uber.first(Tour.ranked(Ui.state.of(u))), BT.Bot.name(BT.World.bot(M.State.bots(Ui.state.of(u)), UB.Uber.bot(Ui.state.of(u)))))

# a choice on the tour's card: a button that sends an action
type TChoice is Data:
  TChoice{label: String, action: String, value: String}

def Tour.pick.one(+l: I.Loc, first: Bool, +b: BT.Bot) -> TChoice:
  TChoice{Bool.pick(String, first, MSG.T.tour.uber.first(l, BT.Bot.name(b)), MSG.T.tour.uber.make(l, BT.Bot.name(b))), "uber-promote", BT.Bot.id(b)}

def Tour.picks(+l: I.Loc, us: List<&2, Used>, first: Bool) -> List<&2, TChoice>:
  match us:
    case Nil{}:
      Nil{}
    case Used{_, b} <> t:
      Tour.pick.one(l, first, b) <> Tour.picks(l, t, False{})

def Tour.choices.k(+l: I.Loc, +uc: String, +st: M.State) -> List<&2, TChoice>:
  Bool.pick(List<&2, TChoice>, String.eq(uc, "none"), [TChoice{MSG.T.tour.uber.create(l), "uber-new", ""}],
    Bool.pick(List<&2, TChoice>, String.eq(uc, "pick"), List.append(&2, TChoice, Tour.picks(l, List.take(&2, Used, Tour.ranked(st), 3n), True{}), [TChoice{MSG.T.tour.uber.new(l), "uber-new", ""}]), Nil{}))

# the buttons under the uberbot step's words (none on other steps)
def Tour.choices.on(+l: I.Loc, on: Bool, +u: Ui) -> List<&2, TChoice>:
  match on:
    case True{}:
      Tour.choices.k(l, Tour.uber.case(u), Ui.state.of(u))
    case False{}:
      Nil{}

# The first step and the usage step asked alone offer the bug report link;
# the usage step answers or reads the policy ("open-url" never moves the tour)
def Tour.report(+u: Ui) -> TChoice:
  TChoice{"Report an issue", "open-url", Share.bug(u)}

def Tour.share.choices(single: Bool, +u: Ui) -> List<&2, TChoice>:
  List.append(&2, TChoice, [TChoice{"Keep sharing", "share", "on"}, TChoice{"Opt out", "share", "off"}, TChoice{"Privacy policy", "open-url", Share.privacy()}],
    Bool.pick(List<&2, TChoice>, single, [Tour.report(u)], Nil{}))

def Tour.choices.at(i: Nat, single: Bool, +u: Ui) -> List<&2, TChoice>:
  match i:
    case 0n:
      [Tour.report(u)]
    case 9n:
      Tour.share.choices(single, u)
    case 10n:
      Tour.choices.k(Lang.loc(u), Tour.uber.case(u), Ui.state.of(u))
    case _:
      Nil{}

def Tour.choices(+u: Ui) -> List<&2, TChoice>:
  Tour.choices.at(Tour.at(u), Tour.single(u), u)

def Tour.body.k(alone: Bool, +words: String) -> String:
  Bool.pick(String, alone, words ++ " " ++ OB.Ob.beta(), words)

# the step's words for a kind of client (0 window, 1 web, 2 phone)
def Tour.body.loc(+l: I.Loc, +k: Nat, +u: Ui) -> String:
  Tour.body.k(Bool.and(Tour.single(u), Nat.is_eq(Tour.at(u), OB.Ob.share())), OB.Ob.body.uber(l, k, Tour.days(u), Tour.uber.case(u), Tour.uber.name(u), Tour.at(u)))

def Tour.body(k: Nat, +u: Ui) -> String:
  Tour.body.loc(Lang.loc(u), k, u)

def Tour.target.k(+g: String, open: Bool) -> String:
  Bool.pick(String, Bool.and(String.eq(g, "head"), Bool.not(open)), "", g)

# a bot of this thread is what the main area shows
def Tour.bot(m: Maybe<&2, BT.Bot>, +u: Ui) -> Bool:
  match m:
    case Some{BT.Bot{_, _, +th, _, _, _, _}}:
      String.eq(th, Ui.sel(u))
    case None{}:
      False{}

# an open thread's own header is on screen: its timeline is what shows
# (not Settings, a room, a remote chat or a bot's view, whatever thread
# is still selected under them)
def Tour.head(+u: Ui) -> Bool:
  Bool.and(Ui.timeline(u), Bool.not(Tour.bot(BT.World.find.id(BT.World.bot(M.State.bots(Ui.state.of(u)), Ui.scratch(u, "@bot"))), u)))

# what the step points at: the thread's buttons only when its header shows
def Tour.target(+k: Nat, +u: Ui) -> String:
  Tour.target.k(OB.Ob.target(k, Tour.at(u)), Tour.head(u))

def Tour.room.on(r: BT.Room) -> Bool:
  match r:
    case BT.Room{_, _, _, _, deleted}:
      Bool.not(deleted)

# the field the keyboard goes back to when the tour is left: the one of the
# view that is on screen (a room's, a remote chat's, an open thread's
# composer), else nowhere
def Tour.focus.k(settings: Bool, room: Bool, +remote: String, timeline: Bool) -> String:
  Bool.pick(String, settings, "", Bool.pick(String, room, "room-post", Bool.pick(String, Bool.not(String.is_empty(remote)), "remote-send", Bool.pick(String, timeline, "composer", ""))))

def Tour.focus(+u: Ui) -> String:
  Tour.focus.k(Ui.flag(u, "settings"), Tour.room.on(BT.World.room(M.State.bots(Ui.state.of(u)), Ui.scratch(u, "@room"))), Ui.scratch(u, "@remote"), Ui.timeline(u))

def Tour.leave(a: Act, +field: String) -> Act:
  match a:
    case Act{u, cs}:
      Act{u, Focus{field} <> cs}

# the tour is over, and its uberbot step answered ("onboard.uber")
def Tour.finish.cat(a: Act, cs: List<&2, Cmd>) -> Act:
  match a:
    case Act{u, fs}:
      Act{u, List.append(&2, Cmd, cs, fs)}

def Tour.finish.both(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Tour.finish.cat(Ui.setting(u, "onboard.uber=1"), cs)

def Tour.finish(+u: Ui) -> Act:
  Tour.leave(Tour.finish.both(Ui.setting(Ui.scratch_set(u, "@tour", ""), "onboard.done=1")), Tour.focus(u))

# to step i (by hand or not as the tour was), or (past the last) the end
def Tour.raw(manual: Bool, +i: Nat) -> String:
  Bool.pick(String, manual, "m", "") ++ Bool.pick(String, Nat.is_eq(i, OB.Ob.share()), "s", "") ++ Nat.show(i)

# The thread-tools step needs a real header. On a fresh welcome screen,
# select an existing thread; the tour never creates or starts one.
def Tour.preview.k(pick: Bool, +u: Ui) -> Ui:
  match pick:
    case False{}: u
    case True{}: Ui.reselect(Ui.sel(u), Ui.select.nav(u, Side.first(Side.visible.projects(u))))

def Tour.example(i: Nat) -> Bool:
  Nat.is_eq(i, 3n)

def Tour.preview(+i: Nat, +u: Ui) -> Ui:
  Tour.preview.k(Bool.and(Nat.is_eq(i, 3n), String.is_empty(Ui.sel(u))), u)

def Tour.go.k(live: Bool, +i: Nat, +u: Ui) -> Act:
  match live:
    case True{}:
      Act{Ui.scratch_set(Ui.scratch_set(u, "@tour", Tour.raw(OB.Ob.manual(Ui.scratch(u, "@tour")), i)), "@tour.sc", ""), Nil{}}
    case False{}:
      Tour.finish(u)

def Tour.go(+i: Nat, +u: Ui) -> Act:
  Tour.go.k(OB.Ob.live(i), i, Tour.preview(i, u))

def Tour.open.k(+u: Ui) -> Act:
  Tour.leave(Act{Ui.scratch_set(Ui.scratch_set(u, "@tour", OB.Ob.raw(True{}, 0n)), "@tour.sc", ""), Nil{}}, Tour.focus(u))

def Tour.open(+u: Ui) -> Act:
  Tour.open.k(Ui.flag_off(u, "settings"))

# Back goes nowhere on the first step or a step asked alone
def Tour.back.k(has: Bool, +u: Ui) -> Act:
  match has:
    case True{}:
      Tour.go(OB.Ob.back(Tour.at(u)), u)
    case False{}:
      Act{u, Nil{}}

def Tour.act.k(i: Nat, +u: Ui) -> Act:
  match i:
    case 0n:
      Tour.go(OB.Ob.next(Tour.at(u)), u)
    case 1n:
      Tour.back.k(Tour.back(u), u)
    case 2n:
      Tour.finish(u)
    case 3n:
      Tour.open(u)
    case _:
      Act{u, Nil{}}

# a move needs a step on screen (a stray one never ends the tour)
def Tour.act.on(on: Bool, +u: Ui, +action: String) -> Act:
  match on:
    case True{}:
      Tour.act.k(C.Codec.pos(["ob-next", "ob-back", "ob-skip", "ob-open"], action), u)
    case False{}:
      Tour.act.k(Bool.pick(Nat, String.eq(action, "ob-open"), 3n, 9n), u)

def Tour.act.base(+u: Ui, +action: String) -> Act:
  Tour.act.on(Tour.on(u), u, action)

# Anonymous usage: the choices, Settings' row and the tour's usage step
# set telemetry.share and telemetry.asked together ("share on|off"); Next
# or Skip on the step set telemetry.asked too (keeping the default, sharing).
# On the step, a choice moves on (asked alone: the card closes); "share-reset"
# makes a new id and "open-url" is the client's own and never gets here.
def Share.then(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Tour.finish.cat(Ui.setting(u, "telemetry.asked=1"), cs)

def Share.val(+v: String) -> String:
  Bool.pick(String, String.eq(v, "off"), "off", "on")

def Share.set(+u: Ui, +value: String) -> Act:
  Share.then(Ui.setting(Ui.info_set(u, "telemetry.id", ""), "telemetry.share=" ++ Share.val(value)))

def Share.close(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Tour.leave(Act{Ui.scratch_set(u, "@tour", ""), cs}, Tour.focus(u))

def Share.step(single: Bool, a: Act) -> Act:
  match single:
    case True{}:
      Share.close(a)
    case False{}:
      match a:
        case Act{u, cs}:
          Tour.finish.cat(Tour.go(OB.Ob.next(OB.Ob.share()), u), cs)

def Share.act.k(onstep: Bool, single: Bool, +u: Ui, +value: String) -> Act:
  match onstep:
    case True{}:
      Share.step(single, Share.set(u, value))
    case False{}:
      Share.set(u, value)

def Share.act(+u: Ui, +value: String) -> Act:
  Share.act.k(Nat.is_eq(Tour.at(u), OB.Ob.share()), Tour.single(u), u, value)

def Share.reset(+u: Ui) -> Act:
  Share.then(Ui.setting(Ui.info_set(u, "telemetry.id", ""), "telemetry.reset=" ++ Nat.show(Nat.add(OB.Ob.num(M.Setting.get(Ui.state.of(u), "telemetry.reset", ""), 0n), 1n))))

def Share.route(isshare: Bool, +u: Ui, +value: String) -> Act:
  match isshare:
    case True{}:
      Share.act(u, value)
    case False{}:
      Share.reset(u)

def Share.is(+action: String) -> Bool:
  Bool.or(String.eq(action, "share"), String.eq(action, "share-reset"))

def Tour.act.q(single: Bool, +u: Ui, +action: String) -> Act:
  match single:
    case True{}:
      Share.close(Share.then(Act{u, Nil{}}))
    case False{}:
      Share.then(Tour.act.base(u, action))

def Tour.act.m(moves: Bool, single: Bool, +u: Ui, +action: String) -> Act:
  match moves:
    case True{}:
      Tour.act.q(single, u, action)
    case False{}:
      Tour.act.base(u, action)

def Tour.act(+u: Ui, +action: String) -> Act:
  Tour.act.m(Bool.and(Nat.is_eq(Tour.at(u), OB.Ob.share()), Bool.or(String.eq(action, "ob-next"), String.eq(action, "ob-skip"))), Tour.single(u), u, action)

# The uberbot (core/uber.bend, docs/uber.md): "uber" opens its Chat tab on a
# client with the Mailroom (Mailv.enabled, "local.mailroom"), else its
# Status (the phones' plane); "uber-promote <bot>" and "uber-demote" ask the
# hub (bots.uber), which backs the bot up before it logs the promotion.
# Routed ahead of the action table.
def Uber.show.k(none: Bool, +u: Ui, +id: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Bots.select(Ui.scratch_set(u, "@btab", Bool.pick(String, String.eq(Ui.info(u, "local.mailroom"), "on"), "chat", "status")), id)

def Uber.show(+u: Ui) -> Act:
  Uber.show.k(String.is_empty(UB.Uber.bot(Ui.state.of(u))), u, UB.Uber.bot(Ui.state.of(u)))

# "uber-bot <bot>|<tab>": a bot with a tab shown (a routine's row)
def Uber.bot.at(+u: Ui, +value: String) -> Act:
  Bots.select(Ui.scratch_set(u, "@btab", Maybe.default(&2, String, List.get(&2, String, String.split(value, '|'), 1n), "")), Maybe.default(&2, String, List.head(&2, String, String.split(value, '|')), ""))

# "uber-new": the new bot form opens for the uberbot (the tour's "Create my
# uberbot"): Settings shut, "@uber.new" set so the request carries uber
def Uber.new(+u: Ui) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(Ui.flag_off(u, "settings"), "@bnew", "1"), "@uber.new", "1"), [Focus{"bot-name"}]}

# "uber-home" asks the hub (bots.uber_home) to make this machine hold the
# uberbot's conversation (the Status strip's "Make this the home").
def Uber.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Uber.show(u)
    case 1n:
      Ui.rpc(u, "bots.uber", [J.KV{"bot", J.Str{value}}])
    case 2n:
      Ui.rpc(u, "bots.uber", [J.KV{"bot", J.Str{""}}])
    case 3n:
      Uber.bot.at(u, value)
    case 4n:
      Uber.new(u)
    case 5n:
      Ui.rpc(u, "bots.uber_home", Nil{})
    case _:
      Act{u, Nil{}}

# Teaching a bot browser tasks (docs/teach.md)
# --------------------------------------------
# The Browser tab, the Tasks tab and a paused run's card, for all four
# clients. The hub says who drives a bot's page (info "drive.<bot>": ""
# the bot, "teach" or "takeover" the person), whether a recording runs
# ("teach.<bot>": "rec <start secs> <steps>", or "review" once stopped)
# and the draft steps ("teach.draft.<bot>", a JSON array of task.bend's
# steps). Everything a client draws is decided here: the strip's state and
# buttons, whether the frame takes the person's input, the save sheet's rows
# (a typed value that is no secret can be marked as a parameter), the
# tasks, their runs and the stuck card. Secrets reach no client: a
# recorded secret is only its label.

type TBtn is Data:
  TBtn{label: String, action: String, value: String, primary: Bool}

# mode: 0 idle, 1 recording, 2 review (the save sheet), 3 the person has the
# browser
type TStrip is Data:
  TStrip{mode: Nat, title: String, clock: String, steps: Nat, btns: List<&2, TBtn>}

# one draft step in the save sheet: what it did; its typed value as a chip
# the person can make a parameter ("" for a secret or a step with none)
type TStepRow is Data:
  TStepRow{n: Nat, text: String, label: String, chip: String, secret: Bool, marked: Bool, pname: String}

type TRun is Data:
  TRun{run: String, slug: String, status: String, n: Nat, summary: String, shots: List<&2, String>}

type TTask is Data:
  TTask{slug: String, name: String, description: String, params: List<&2, TK.TkParam>, open: Bool, asking: Bool, last: String}

type TAsk is Data:
  TAsk{id: String, run: String, slug: String, n: Nat, kind: String, question: String, risky: Bool, shot: String, btns: List<&2, TBtn>}

def Teach.drive(+u: Ui, +bot: String) -> String:
  Ui.info(u, "drive." ++ bot)

def Teach.info(+u: Ui, +bot: String) -> String:
  Ui.info(u, "teach." ++ bot)

# the strip's state from the teach info and the drive info
def Teach.mode.of(+t: String, +d: String) -> Nat:
  Bool.pick(Nat, String.starts_with(t, "rec"), 1n,
    Bool.pick(Nat, String.eq(t, "review"), 2n,
      Bool.pick(Nat, String.eq(d, "takeover"), 3n,
        Bool.pick(Nat, String.eq(d, "teach"), 1n, 0n))))

def Teach.mode(+u: Ui, +bot: String) -> Nat:
  Teach.mode.of(Teach.info(u, bot), Teach.drive(u, bot))

# the frame takes the person's clicks, keys and wheel only while they drive
# (law teach_input_only_when_driving)
def Teach.driving(+d: String) -> Bool:
  Bool.not(String.is_empty(d))

def Teach.input(+u: Ui, +bot: String) -> Bool:
  Teach.driving(Teach.drive(u, bot))

def Teach.part(+t: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(t, ' '), i), "")

def Teach.two(+n: Nat) -> String:
  Bool.pick(String, Nat.is_lt(n, 10n), "0" ++ Nat.show(n), Nat.show(n))

# m:ss
def Teach.clock(+secs: Nat) -> String:
  Nat.show(Nat.div(secs, 60n)) ++ ":" ++ Teach.two(Nat.mod(secs, 60n))

def Teach.draft(+u: Ui, +bot: String) -> List<&2, TK.TkStep>:
  TK.Tk.steps.of(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "teach.draft." ++ bot)), J.Null{})))

def Teach.label(t: TK.TkTarget) -> String:
  match t:
    case TK.TkTarget{_, +name, +label, +placeholder, +text, +testid, +css, _, _, _}:
      TK.Tk.first(name, TK.Tk.first(label, TK.Tk.first(placeholder, TK.Tk.first(text, TK.Tk.first(testid, css)))))

# the steps recorded so far (the draft's length, else the info's count)
def Teach.count(+u: Ui, +bot: String) -> Nat:
  +n = {List.length(&2, TK.TkStep, Teach.draft(u, bot)) : Nat}
  Bool.pick(Nat, Nat.is_eq(n, 0n), M.Switch.num(Teach.part(Teach.info(u, bot), 2n), 0n), n)

def Teach.elapsed(+u: Ui, +bot: String) -> Nat:
  +start = {M.Switch.num(Teach.part(Teach.info(u, bot), 1n), 0n) : Nat}
  Bool.pick(Nat, Nat.is_eq(start, 0n), 0n, Nat.sub(Side.now(u), start))

def Teach.title(+l: I.Loc, +mode: Nat, +name: String) -> String:
  match mode:
    case 0n:
      ""
    case 1n:
      MSG.T.teach.watching(l, name)
    case 2n:
      MSG.T.teach.save(l)
    case _:
      MSG.T.teach.youhave(l)

def Teach.btns(+l: I.Loc, mode: Nat) -> List<&2, TBtn>:
  match mode:
    case 0n:
      [TBtn{MSG.T.teach.teach(l), "tk-teach", "", True{}}, TBtn{MSG.T.teach.takeover(l), "tk-takeover", "", False{}}]
    case 1n:
      [TBtn{MSG.T.teach.stopsave(l), "tk-stop", "", True{}}, TBtn{MSG.T.teach.discard(l), "tk-discard", "", False{}}]
    case 2n:
      [TBtn{MSG.T.common.save(l), "tk-save", "", True{}}, TBtn{MSG.T.teach.discard(l), "tk-discard", "", False{}}]
    case _:
      [TBtn{MSG.T.teach.handback(l), "tk-handback", "", True{}}]

def Teach.strip.k(+l: I.Loc, +mode: Nat, +name: String, +clock: String, steps: Nat) -> TStrip:
  TStrip{mode, Teach.title(l, mode, name), Bool.pick(String, Nat.is_eq(mode, 1n), clock, ""), steps, Teach.btns(l, mode)}

def Teach.bot.name(+u: Ui, +bot: String) -> String:
  BT.Bot.name(BT.World.bot(M.State.bots(Ui.state(u)), bot))

# the strip above the bot's page
def Teach.strip.loc(+l: I.Loc, +u: Ui, +bot: String) -> TStrip:
  Teach.strip.k(l, Teach.mode(u, bot), Teach.bot.name(u, bot), Teach.clock(Teach.elapsed(u, bot)), Teach.count(u, bot))

def Teach.strip(+u: Ui, bot: String) -> TStrip:
  Teach.strip.loc(Lang.loc(u), u, bot)

# The save sheet
# --------------

# a step in words, a whole sentence round its target
def Teach.step.k(+l: I.Loc, +act: String, +t: String) -> String:
  Bool.pick(String, String.eq(act, "navigate"), MSG.T.teach.step.navigate(l, t),
    Bool.pick(String, String.eq(act, "click"), MSG.T.teach.step.click(l, t),
      Bool.pick(String, String.eq(act, "type"), MSG.T.teach.step.typein(l, t),
        Bool.pick(String, String.eq(act, "select"), MSG.T.teach.step.select(l, t),
          Bool.pick(String, String.eq(act, "press"), MSG.T.teach.step.press(l, t),
            Bool.pick(String, String.eq(act, "scroll"), MSG.T.teach.step.scroll(l, t),
              Bool.pick(String, String.eq(act, "wait"), MSG.T.teach.step.wait(l, t),
                Bool.pick(String, String.eq(act, "check"), MSG.T.teach.step.check(l, t), t))))))))

def Teach.chip(+act: String, v: TK.TkValue) -> String:
  match v:
    case TK.VLit{t}:
      Bool.pick(String, Bool.or(String.eq(act, "type"), String.eq(act, "select")), t, "")
    case _:
      ""

def Teach.secret(v: TK.TkValue) -> Bool:
  match v:
    case TK.VSecret{_}:
      True{}
    case _:
      False{}

def Teach.what(+l: I.Loc, +act: String, +target: TK.TkTarget, +url: String, +note: String) -> String:
  Bool.pick(String, String.eq(act, "note"), note,
    Teach.step.k(l, act, Bool.pick(String, String.eq(act, "navigate"), url, Teach.label(target))))

# a parameter's default name: the field's label, else value<n>
def Teach.pdflt(+label: String, +n: Nat) -> String:
  Bool.pick(String, String.is_empty(label), "value" ++ Nat.show(n), TK.Task.slug(label))

def Teach.pname(+u: Ui, +label: String, +n: Nat) -> String:
  +f = {String.trim(Ui.scratch(u, "@f.tkp." ++ Nat.show(n))) : String}
  Bool.pick(String, String.is_empty(f), Teach.pdflt(label, n), f)

def Teach.row(+l: I.Loc, +u: Ui, s: TK.TkStep) -> TStepRow:
  match s:
    case TK.TkStep{+n, +act, +target, +value, _, _, +url, _, +note}:
      +label = {Teach.label(target) : String}
      TStepRow{n, Teach.what(l, act, target, url, note), label, Teach.chip(act, value), Teach.secret(value), String.eq(Ui.scratch(u, "@tkm." ++ Nat.show(n)), "1"), Teach.pname(u, label, n)}

def Teach.rows.go(+l: I.Loc, ss: List<&2, TK.TkStep>, +u: Ui) -> List<&2, TStepRow>:
  match ss:
    case Nil{}:
      Nil{}
    case s <> t:
      Teach.row(l, u, s) <> Teach.rows.go(l, t, u)

# every draft step as a row
def Teach.rows.loc(+l: I.Loc, +u: Ui, +bot: String) -> List<&2, TStepRow>:
  Teach.rows.go(l, Teach.draft(u, bot), u)

def Teach.rows(+u: Ui, bot: String) -> List<&2, TStepRow>:
  Teach.rows.loc(Lang.loc(u), u, bot)

# the chips: the typed values that are no secret
def Teach.chip.ok(r: TStepRow) -> Bool:
  match r:
    case TStepRow{_, _, _, +c, +s, _, _}:
      Bool.and(Bool.not(s), Bool.not(String.is_empty(c)))

def Teach.chips.k(keep: Bool, r: TStepRow, rest: List<&2, TStepRow>) -> List<&2, TStepRow>:
  match keep:
    case True{}:
      r <> rest
    case False{}:
      rest

def Teach.chips.go(rs: List<&2, TStepRow>) -> List<&2, TStepRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +h <> r:
      Teach.chips.k(Teach.chip.ok(h), h, Teach.chips.go(r))

# The parameters the person marked, as the save request wants them
def Teach.params.k(keep: Bool, +n: Nat, +t: String, +p: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      J.Json.obj([J.KV{"step", J.Json.nat(n)}, J.KV{"name", J.Str{p}}, J.KV{"label", J.Str{t}}]) <> rest
    case False{}:
      rest

# Runs
# ----

def Teach.params.go(rs: List<&2, TStepRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case TStepRow{+n, _, +t, +c, +s, +m, +p} <> r:
      Teach.params.k(Bool.and(m, Bool.and(Bool.not(s), Bool.not(String.is_empty(c)))), n, t, p, Teach.params.go(r))

def Teach.shots(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{s} <> t:
      s <> Teach.shots(t)
    case _ <> t:
      Teach.shots(t)

def Teach.run.of(+o: J.Json) -> TRun:
  TRun{J.Json.get_str(o, "run", ""), J.Json.get_str(o, "slug", ""), J.Json.get_str(o, "status", ""), J.Json.get_nat(o, "n", 0n), J.Json.get_str(o, "summary", ""), Teach.shots(J.Json.elems(J.Json.get_obj(o, "shots")))}

def Teach.run.k(ok: Bool, +sum: String) -> Maybe<&2, TRun>:
  match ok:
    case True{}:
      Some{Teach.run.of(Maybe.default(&2, J.Json, J.Json.parse(sum), J.Null{}))}
    case False{}:
      None{}

# a run from the thread's entry (an Act of tone "task", its text the JSON)
def Teach.run.entry(e: M.Entry) -> Maybe<&2, TRun>:
  match e:
    case M.Act{_, _, +tone, _, +sum, _}:
      Teach.run.k(String.eq(tone, "task"), sum)
    case _:
      None{}

def Teach.seen(rs: List<&2, TRun>, +run: String) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case TRun{+r, _, _, _, _, _} <> t:
      Bool.or(String.eq(r, run), Teach.seen(t, run))

def Teach.runs.put(seen: Bool, r: TRun, acc: List<&2, TRun>) -> List<&2, TRun>:
  match seen:
    case True{}:
      acc
    case False{}:
      r <> acc

def Teach.runs.add(m: Maybe<&2, TRun>, +acc: List<&2, TRun>) -> List<&2, TRun>:
  match m:
    case Some{TRun{+r, sl, st, n, sm, sh}}:
      Teach.runs.put(Teach.seen(acc, r), TRun{r, sl, st, n, sm, sh}, acc)
    case None{}:
      acc

# a bot's runs, newest first, each once (its newest log): the thread's newest
# entries, within a fixed reach
def Teach.runs.go(fuel: Nat, es: List<&2, M.Entry>, acc: List<&2, TRun>) -> List<&2, TRun>:
  match fuel es:
    case 0n _:
      List.reverse(&2, TRun, acc)
    case 1n+_ Nil{}:
      List.reverse(&2, TRun, acc)
    case 1n+k e <> t:
      Teach.runs.go(k, t, Teach.runs.add(Teach.run.entry(e), acc))

def Teach.runs.of(st: M.State, +thread: String) -> List<&2, TRun>:
  Teach.runs.go(400n, M.State.mine(st, thread), Nil{})

def Teach.runs(+u: Ui, +bot: String) -> List<&2, TRun>:
  Teach.runs.of(Ui.state(u), BT.Bot.thread(BT.World.bot(M.State.bots(Ui.state(u)), bot)))

# a run row titled by its task's name (the slug when the task is gone)
def Teach.run.name.k(m: Maybe<&2, TK.TkTask>, +slug: String) -> String:
  match m:
    case Some{TK.TkTask{_, +name, _, _, _, _, _, _, _, _}}:
      Bool.pick(String, String.is_empty(name), slug, name)
    case None{}:
      slug

def Teach.run.name(m: Maybe<&2, String>, +slug: String) -> String:
  match m:
    case Some{+j}:
      Teach.run.name.k(TK.Task.read(j), slug)
    case None{}:
      slug

def Teach.runs.named(rs: List<&2, TRun>, +w: BT.World, +bot: String) -> List<&2, TRun>:
  match rs:
    case Nil{}:
      Nil{}
    case TRun{+r, +sl, +st, +n, +sm, +sh} <> t:
      TRun{r, Teach.run.name(BT.World.task(w, bot, sl), sl), st, n, sm, sh} <> Teach.runs.named(t, w, bot)

# the runs as the clients list them: each titled by its task's name
def Teach.runs.shown(+u: Ui, +bot: String) -> List<&2, TRun>:
  Teach.runs.named(Teach.runs(u, bot), M.State.bots(Ui.state(u)), bot)

def Teach.last(rs: List<&2, TRun>, +slug: String) -> String:
  match rs:
    case Nil{}:
      ""
    case TRun{_, +s, +st, _, _, _} <> t:
      Bool.pick(String, String.eq(s, slug), st, Teach.last(t, slug))

# The tasks
# ---------

def Teach.task.row(t: TK.TkTask, runs: List<&2, TRun>, +open: String, +asking: String) -> TTask:
  match t:
    case TK.TkTask{+slug, +name, +description, _, _, params, _, _, _, _}:
      TTask{slug, TK.Tk.first(name, slug), description, params, String.eq(open, slug), String.eq(asking, slug), Teach.last(runs, slug)}

def Teach.tasks.k(m: Maybe<&2, TK.TkTask>, rest: List<&2, TTask>, runs: List<&2, TRun>, +open: String, +asking: String) -> List<&2, TTask>:
  match m:
    case Some{t}:
      Teach.task.row(t, runs, open, asking) <> rest
    case None{}:
      rest

def Teach.tasks.go(rs: List<&2, BT.TRow>, +runs: List<&2, TRun>, +open: String, +asking: String) -> List<&2, TTask>:
  match rs:
    case Nil{}:
      Nil{}
    case BT.TRow{_, +json} <> t:
      Teach.tasks.k(TK.Task.read(json), Teach.tasks.go(t, runs, open, asking), runs, open, asking)

# the bot's tasks by name, each with how its last run went
def Teach.tasks(+u: Ui, +bot: String) -> List<&2, TTask>:
  Teach.tasks.go(BT.World.tasks_of(M.State.bots(Ui.state(u)), bot), Teach.runs(u, bot), Ui.scratch(u, "@tkopen"), Ui.scratch(u, "@tkdel"))

# a run form's field for parameter `name`
def Teach.pfield(+name: String) -> String:
  "tkrun." ++ name

# The stuck card
# --------------

def Teach.choice.label(+l: I.Loc, +c: String) -> String:
  Bool.pick(String, String.eq(c, "confirm"), MSG.T.teach.choice.confirm(l),
    Bool.pick(String, String.eq(c, "continue"), MSG.T.teach.choice.continue(l),
      Bool.pick(String, String.eq(c, "skip"), MSG.T.teach.choice.skip(l),
        Bool.pick(String, String.eq(c, "stop"), MSG.T.teach.choice.stop(l), c))))

# a risky step goes on only by Confirm, never Continue
def Teach.choice.safe(+risky: Bool, +c: String) -> String:
  Bool.pick(String, Bool.and(risky, String.eq(c, "continue")), "confirm", c)

def Teach.choices(+o: String, +risky: Bool) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(String.trim(o)),
    Bool.pick(List<&2, String>, risky, ["confirm", "skip", "stop"], ["continue", "skip", "stop"]),
    String.split(o, '\n'))

def Teach.ask.btns.loc(+l: I.Loc, cs: List<&2, String>, +id: String, +risky: Bool) -> List<&2, TBtn>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      TBtn{Teach.choice.label(l, Teach.choice.safe(risky, c)), "answer", id ++ "|" ++ Teach.choice.safe(risky, c), Bool.not(String.eq(c, "stop"))} <> Teach.ask.btns.loc(l, t, id, risky)

def Teach.ask.btns(cs: List<&2, String>, id: String, risky: Bool) -> List<&2, TBtn>:
  Teach.ask.btns.loc(I.En{}, cs, id, risky)

# Take over is offered while the bot has the browser
def Teach.ask.take(+l: I.Loc, drive: Bool) -> List<&2, TBtn>:
  Bool.pick(List<&2, TBtn>, drive, Nil{}, [TBtn{MSG.T.teach.takeover(l), "tk-takeover", "", False{}}])

# the card of an open ask of kind "browser" (its detail is a JSON object)
def Teach.ask(+l: I.Loc, a: M.Ask, +drive: String) -> TAsk:
  match a:
    case M.Ask{+id, _, _, _, +d, +o, _, _}:
      +j = {Maybe.default(&2, J.Json, J.Json.parse(d), J.Null{}) : J.Json}
      +risky = {J.Json.get_bool(j, "risky", False{}) : Bool}
      TAsk{id, J.Json.get_str(j, "run", ""), J.Json.get_str(j, "slug", ""), J.Json.get_nat(j, "n", 0n), J.Json.get_str(j, "kind", ""), J.Json.get_str(j, "question", ""), risky, J.Json.get_str(j, "shot", ""),
        List.append(&2, TBtn, Teach.ask.btns.loc(l, Teach.choices(o, risky), id, risky), Teach.ask.take(l, Bool.not(String.is_empty(drive))))}

# the bot an ask is about (its detail's "bot")
def Teach.ask.bot(a: M.Ask) -> String:
  match a:
    case M.Ask{_, _, _, _, +d, _, _, _}:
      J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(d), J.Null{}), "bot", "")

# the card of a browser ask, with the drive info from the info map
def Teach.ask.from.loc(+l: I.Loc, +a: M.Ask, nf: Map<&2, String>) -> TAsk:
  Teach.ask(l, a, Str.get(nf, "drive." ++ Teach.ask.bot(a)))

def Teach.ask.from(a: M.Ask, nf: Map<&2, String>) -> TAsk:
  Teach.ask.from.loc(I.En{}, a, nf)

# the person's choices for a browser ask whose detail is `d`
def Teach.ask.of.loc(+l: I.Loc, a: M.Ask, +u: Ui, +bot: String) -> TAsk:
  Teach.ask(l, a, Teach.drive(u, bot))

def Teach.ask.of(a: M.Ask, +u: Ui, bot: String) -> TAsk:
  Teach.ask.of.loc(Lang.loc(u), a, u, bot)

# The Browser tab's actions
# -------------------------

def Teach.is(+action: String) -> Bool:
  Bool.or(String.starts_with(action, "tk-"), String.starts_with(action, "bb-"))

def Teach.marks.go(rs: List<&2, TStepRow>, u: Ui) -> Ui:
  match rs:
    case Nil{}:
      u
    case TStepRow{+n, _, _, _, _, _, _} <> t:
      Teach.marks.go(t, Ui.scratch_set(Ui.scratch_set(u, "@tkm." ++ Nat.show(n), ""), "@f.tkp." ++ Nat.show(n), ""))

# the save sheet's marks and fields forgotten
def Teach.clear(+u: Ui, +bot: String) -> Ui:
  Bots.fields.clear(Teach.marks.go(Teach.rows.loc(Lang.loc(u), u, bot), u), ["tkname", "tkdesc", "tknote"])

def Teach.teach(+u: Ui, +bot: String, +op: String) -> Act:
  Ui.rpc(u, "bots.teach", [J.KV{"bot", J.Str{bot}}, J.KV{"op", J.Str{op}}])

def Teach.browser(u: Ui, +bot: String, +op: String, ps: List<&2, J.KV>) -> Act:
  Ui.rpc(u, "bots.browser", J.KV{"bot", J.Str{bot}} <> J.KV{"op", J.Str{op}} <> ps)

def Teach.note.k(none: Bool, +u: Ui, +bot: String, +t: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Ui.rpc(Bots.fields.clear(u, ["tknote"]), "bots.teach", [J.KV{"bot", J.Str{bot}}, J.KV{"op", J.Str{"note"}}, J.KV{"text", J.Str{t}}])

def Teach.note(+u: Ui, +bot: String) -> Act:
  +t = {String.trim(Bots.field(u, "tknote")) : String}
  Teach.note.k(String.is_empty(t), u, bot, t)

def Teach.save.k(none: Bool, +u: Ui, +bot: String, +name: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Ui.rpc(Teach.clear(u, bot), "bots.teach", [J.KV{"bot", J.Str{bot}}, J.KV{"op", J.Str{"save"}}, J.KV{"name", J.Str{name}},
        J.KV{"description", J.Str{String.trim(Bots.field(u, "tkdesc"))}}, J.KV{"params", J.Json.arr(Teach.params.go(Teach.rows.loc(Lang.loc(u), u, bot)))}])

def Teach.save(+u: Ui, +bot: String) -> Act:
  +name = {String.trim(Bots.field(u, "tkname")) : String}
  Teach.save.k(String.is_empty(name), u, bot, name)

# a chip marked as a parameter, or unmarked
def Teach.mark(+u: Ui, +n: String) -> Act:
  Act{Ui.scratch_set(u, "@tkm." ++ n, Bool.pick(String, String.eq(Ui.scratch(u, "@tkm." ++ n), "1"), "", "1")), Nil{}}

# a task's run form: one value for each parameter, by name
def Teach.args(ps: List<&2, TK.TkParam>, +u: Ui) -> List<&2, J.KV>:
  match ps:
    case Nil{}:
      Nil{}
    case TK.TkParam{+name, _, _, _} <> t:
      J.KV{name, J.Str{Bots.field(u, Teach.pfield(name))}} <> Teach.args(t, u)

def Teach.fields.go(ps: List<&2, TK.TkParam>, u: Ui) -> Ui:
  match ps:
    case Nil{}:
      u
    case TK.TkParam{+name, _, _, _} <> t:
      Teach.fields.go(t, Ui.scratch_set(u, "@f." ++ Teach.pfield(name), ""))

def Teach.task.k(m: Maybe<&2, String>) -> Maybe<&2, TK.TkTask>:
  match m:
    case Some{j}:
      TK.Task.read(j)
    case None{}:
      None{}

def Teach.task.get(st: M.State, +bot: String, +slug: String) -> Maybe<&2, TK.TkTask>:
  Teach.task.k(BT.World.task(M.State.bots(st), bot, slug))

def Teach.params.of(m: Maybe<&2, TK.TkTask>) -> List<&2, TK.TkParam>:
  match m:
    case Some{TK.TkTask{_, _, _, _, _, ps, _, _, _, _}}:
      ps
    case None{}:
      Nil{}

def Teach.run(+u: Ui, +bot: String, +slug: String) -> Act:
  +ps = {Teach.params.of(Teach.task.get(Ui.state(u), bot, slug)) : List<&2, TK.TkParam>}
  Ui.rpc(Teach.fields.go(ps, u), "bots.task", [J.KV{"bot", J.Str{bot}}, J.KV{"op", J.Str{"run"}}, J.KV{"slug", J.Str{slug}}, J.KV{"params", J.Json.obj(Teach.args(ps, u))}])

# deleting asks first (the same click again deletes)
def Teach.del.k(sure: Bool, +u: Ui, +bot: String, +slug: String) -> Act:
  match sure:
    case True{}:
      Ui.rpc(Ui.scratch_set(u, "@tkdel", ""), "bots.task", [J.KV{"bot", J.Str{bot}}, J.KV{"op", J.Str{"delete"}}, J.KV{"slug", J.Str{slug}}])
    case False{}:
      Act{Ui.scratch_set(u, "@tkdel", slug), Nil{}}

def Teach.del(+u: Ui, +bot: String, +slug: String) -> Act:
  Teach.del.k(String.eq(Ui.scratch(u, "@tkdel"), slug), u, bot, slug)

def Teach.open(+u: Ui, +slug: String) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(u, "@tkdel", ""), "@tkopen", Bool.pick(String, String.eq(Ui.scratch(u, "@tkopen"), slug), "", slug)), Nil{}}

# thousandths of the frame: "x y"
def Teach.thou(+s: String, +i: Nat) -> J.Json:
  J.Num{Nat.show(Nat.min(M.Switch.num(Teach.part(s, i), 0n), 1000n))}

def Teach.click(u: Ui, +bot: String, +v: String) -> Act:
  Teach.browser(u, bot, "click", [J.KV{"x", Teach.thou(v, 0n)}, J.KV{"y", Teach.thou(v, 1n)}])

# a wheel notch's pixels, negative upwards
def Teach.dy(+v: String) -> J.Json:
  J.Num{Bool.pick(String, String.starts_with(v, "-"), "-" ++ Nat.show(M.Switch.num(String.drop(v, 1n), 0n)), Nat.show(M.Switch.num(v, 0n)))}

def Teach.url(+s: String) -> String:
  Bool.pick(String, String.is_empty(s), "", Bool.pick(String, String.contains(s, "://"), s, "https://" ++ s))

def Teach.nav.k(none: Bool, +u: Ui, +bot: String, +url: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Teach.browser(u, bot, "navigate", [J.KV{"url", J.Str{url}}])

def Teach.nav(+u: Ui, +bot: String) -> Act:
  +url = {Teach.url(String.trim(Bots.field(u, "bburl"))) : String}
  Teach.nav.k(String.is_empty(url), u, bot, url)

def Teach.act.k(i: Nat, +u: Ui, +bot: String, +value: String) -> Act:
  match i:
    case 0n:
      Teach.teach(Teach.clear(u, bot), bot, "start")
    case 1n:
      Teach.note(u, bot)
    case 2n:
      Teach.teach(u, bot, "stop")
    case 3n:
      Teach.teach(Teach.clear(u, bot), bot, "discard")
    case 4n:
      Teach.save(u, bot)
    case 5n:
      Teach.mark(u, value)
    case 6n:
      Teach.browser(u, bot, "takeover", Nil{})
    case 7n:
      Teach.browser(u, bot, "handback", Nil{})
    case 8n:
      Teach.run(u, bot, value)
    case 9n:
      Teach.del(u, bot, value)
    case 10n:
      Teach.open(u, value)
    case 11n:
      Teach.click(u, bot, value)
    case 12n:
      Teach.browser(u, bot, "type", [J.KV{"text", J.Str{value}}])
    case 13n:
      Teach.browser(u, bot, "press", [J.KV{"key", J.Str{value}}])
    case 14n:
      Teach.browser(u, bot, "scroll", [J.KV{"dy", Teach.dy(value)}])
    case 15n:
      Teach.nav(u, bot)
    case 16n:
      Teach.browser(u, bot, "back", Nil{})
    case 17n:
      Teach.browser(u, bot, "reload", Nil{})
    case _:
      Act{u, Nil{}}

def Teach.actions() -> List<&2, String>:
  ["tk-teach", "tk-note", "tk-stop", "tk-discard", "tk-save", "tk-mark", "tk-takeover", "tk-handback", "tk-run", "tk-del", "tk-open",
   "bb-click", "bb-type", "bb-key", "bb-scroll", "bb-nav", "bb-back", "bb-reload"]

# the page a tk- or bb- action acts on: the bot whose tab is open, but take
# over and hand back (6, 7) carrying a page id in their value act on that
# page, a thread's own (its Browser pane's strip)
def Teach.target(+i: Nat, +cur: String, +value: String) -> String:
  Bool.pick(String, Bool.and(Bool.or(Nat.is_eq(i, 6n), Nat.is_eq(i, 7n)), Bool.not(String.is_empty(value))), value, cur)

# The frame under the keyboard may be an ordinary thread's page; task
# editing still belongs to the bot whose tab is open.
def Teach.act(+u: Ui, +action: String, +value: String) -> Act:
  Teach.act.k(C.Codec.pos(Teach.actions(), action), u, Teach.target(C.Codec.pos(Teach.actions(), action), Bool.pick(String, String.starts_with(action, "bb-"), BFRAME.BrowserFrame.owner(Bots.cur(u), Ui.page(u)), Bots.cur(u)), value), value)

# A choice on the tour's uberbot step does its action, then ends the tour
# like Done (the action's own commands come after the tour's focus).
def Tour.choose.fin(f: Act, cs: List<&2, Cmd>) -> Act:
  match f:
    case Act{u, fs}:
      Act{u, List.append(&2, Cmd, fs, cs)}

def Tour.choose.k(a: Act) -> Act:
  match a:
    case Act{u, cs}:
      Tour.choose.fin(Tour.finish(u), cs)

# Export actions bypass the positional table, preserving its stable indices.
def KX.act.k(action: String, +u: Ui, +th: String) -> Act:
  match action:
    case "menu-close": Act{Ui.info_set(u, "kx:" ++ th, ""), Nil{}}
    case "kicad-export10-read": Act{u, Nil{}}
    case "kicad-export10-cancel": Act{Ui.info_set(u, "kx:" ++ th, ""), Nil{}}
    case "kicad-export10-next":
      +ls = KEX.KX.lines(KEX.KX.report(J.Json.get_obj(KX.result(Ui.infos(u), th), "report")))
      +off = Nat.add(KX.offset(Ui.infos(u), th), 8n)
      Act{Ui.info_set(u, "kx.page:" ++ th, Nat.show(Bool.pick(Nat, Nat.is_ge(off, List.length(&2, String, ls)), 0n, off))), Nil{}}
    case "kicad-export10-anyway":
      Ui.rpc(Ui.info_set(u, "kx:" ++ th, ""), "kicad.export10", [J.KV{"thread", J.Str{th}}, J.KV{"anyway", J.Flag{True{}}}, J.KV{"report", J.Str{J.Json.show(J.Json.get_obj(KX.result(Ui.infos(u), th), "report"))}}])
    case _: Ui.rpc(Ui.info_set(u, "kx:" ++ th, ""), "kicad.export10", [J.KV{"thread", J.Str{th}}])

def KX.act(+u: Ui, action: String, +value: String) -> Act:
  KX.act.k(action, Ui.scratch_set(u, "@menu", ""), Bool.pick(String, String.is_empty(value), Ui.sel(u), value))

# Browser chrome belongs to the same owner as its native page/remote frame.
def Browse.owner(+u: Ui) -> String:
  Ui.shown.page(u)

def Browse.data(+u: Ui, +page: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "browser.tabs." ++ page)), J.Null{})

def Browse.tabs(+u: Ui, +page: String) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(Browse.data(u, page), "tabs"))

def Browse.url(+u: Ui, +page: String) -> String:
  J.Json.get_str(Browse.data(u, page), "url", "")

def Browse.field(+page: String) -> String:
  "brurl:" ++ page

def Browse.address(+u: Ui, +page: String) -> String:
  Bool.pick(String, String.eq(Ui.scratch(u, "@browse.edit"), Browse.field(page)), Bots.field(u, Browse.field(page)), Browse.url(u, page))

def Browse.edit(+u: Ui, +page: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(u, "@f." ++ Browse.field(page), Browse.address(u, page)), "@browse.edit", Browse.field(page))

def Browse.send(+u: Ui, +op: String, ps: List<&2, J.KV>) -> Act:
  Teach.browser(Ui.scratch_set(u, "@browse.edit", ""), Browse.owner(u), "toolbar", J.KV{"command", J.Str{op}} <> ps)

def Browse.nav.k(empty: Bool, +u: Ui, +url: String) -> Act:
  match empty:
    case True{}: Act{Ui.update(u, J.Json.obj([J.KV{"error", J.Str{"Enter an http(s) address, hostname, or local host:port."}}])), Nil{}}
    case False{}: Browse.send(u, "navigate", [J.KV{"url", J.Str{url}}])

def Browse.nav(+u: Ui) -> Act:
  +url = CF.Cef.url(Browse.address(u, Browse.owner(u)))
  Browse.nav.k(String.is_empty(url), u, url)

def Browse.offset.k(+at: Nat, +count: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(at, count), at, Nat.sub(count, 1n))

def Browse.offset(+u: Ui, +page: String) -> Nat:
  Browse.offset.k(M.Switch.num(Ui.scratch(u, "@browse.tabs." ++ page), 0n), List.length(&2, J.Json, Browse.tabs(u, page)))

def Browse.move(+u: Ui, +next: Bool) -> Act:
  +page = Browse.owner(u)
  +at = Browse.offset(u, page)
  Act{Ui.scratch_set(u, "@browse.tabs." ++ page, Nat.show(Bool.pick(Nat, next,
    Bool.pick(Nat, Nat.is_lt(1n+at, List.length(&2, J.Json, Browse.tabs(u, page))), 1n+at, at), Nat.sub(at, 1n)))), Nil{}}

def Browse.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n: Browse.nav(u)
    case 1n: Browse.send(u, "back", Nil{})
    case 2n: Browse.send(u, "forward", Nil{})
    case 3n: Browse.send(u, "reload", Nil{})
    case 4n: Browse.send(Ui.scratch_set(u, "@browse.tabs." ++ Browse.owner(u), Nat.show(List.length(&2, J.Json, Browse.tabs(u, Browse.owner(u))))), "tabs", [J.KV{"new", J.Flag{True{}}}])
    case 5n: Browse.send(u, "tabs", [J.KV{"selectId", J.Str{value}}])
    case 6n: Browse.send(u, "tabs", [J.KV{"closeId", J.Str{value}}])
    case 7n: Browse.move(u, False{})
    case 8n: Browse.move(u, True{})
    case _: Act{u, Nil{}}

# The time zone (Settings, all clients)
# ---------------------------------------
# The person's zone: the one they or a bot set (person.tz), else the one a
# phone reported (person.tz.device), else the hub machine's. The row shows
# "Europe/Helsinki · EEST +03:00 · from your phone", a field for a zone name
# with Set ("tz-set", field @f.tzname) and Automatic ("tz-auto", on while a
# zone is set). A phone reports its zone ("tz-report" with the zone, sent to
# every paired hub). An error from the hub shows under the row (@tz.try).

def Tzs.er(u: Ui) -> String:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, er, _, _, _, _}:
      er

def Tzs.source(+l: I.Loc, +s: String) -> String:
  Bool.pick(String, String.eq(s, "set"), MSG.T.tz.source.set(l), Bool.pick(String, String.eq(s, "phone"), MSG.T.tz.source.phone(l), Bool.pick(String, String.eq(s, "machine"), MSG.T.tz.source.machine(l), MSG.T.tz.source.none(l))))

def Tzs.off(+u: Ui) -> TZ.TzOff:
  TZ.Tz.off(SC.Sched.person(Ui.state.of(u), 0n, True{}), Side.now(u))

# "Europe/Helsinki · EEST +03:00 · from your phone"
def Tzs.line.loc(+l: I.Loc, +u: Ui) -> String:
  SC.Zone.person(Ui.state.of(u)) ++ " · " ++ TZ.Tz.abbr(Tzs.off(u)) ++ " " ++ TZ.Tz.offset.text(Tzs.off(u)) ++ " · " ++ Tzs.source(l, SC.Zone.source(Ui.state.of(u)))

def Tzs.line(+u: Ui) -> String:
  Tzs.line.loc(Lang.loc(u), u)

def Tzs.auto(+u: Ui) -> Bool:
  Bool.not(String.is_empty(M.Setting.get(Ui.state.of(u), "person.tz", "")))

# what Automatic follows: the phone's zone, else this machine's, else UTC
# (sched.bend's Zone.person with no zone set)
def Tzs.auto.from(+s: M.State) -> String:
  Bool.pick(String, String.is_empty(M.Setting.get(s, "person.tz.device", "")),
    Bool.pick(String, String.is_empty(M.Setting.get(s, "tz.machine", "")), "utc", "machine"), "phone")

def Tzs.auto.tip.k(+l: I.Loc, set: Bool, +from: String) -> String:
  match set:
    case True{}:
      Bool.pick(String, String.eq(from, "phone"), MSG.T.settings.tz.follow.phone(l),
        Bool.pick(String, String.eq(from, "machine"), MSG.T.settings.tz.follow.machine(l), MSG.T.settings.tz.follow.utc(l)))
    case False{}:
      Bool.pick(String, String.eq(from, "phone"), MSG.T.settings.tz.auto.phone(l),
        Bool.pick(String, String.eq(from, "machine"), MSG.T.settings.tz.auto.machine(l), MSG.T.settings.tz.auto.utc(l)))

# Automatic's words: what it follows while no zone is set (it is on), or
# what it goes back to once one is
def Tzs.auto.tip(+l: I.Loc, +u: Ui) -> String:
  Tzs.auto.tip.k(l, Tzs.auto(u), Tzs.auto.from(Ui.state.of(u)))

# the error shows only while the zone tried (@tz.try) is not the one set:
# once the hub takes it, a later unrelated error stays off this row
def Tzs.err(+u: Ui) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@tz.try"))),
    Bool.not(String.eq(Ui.scratch(u, "@tz.try"), M.Setting.get(Ui.state.of(u), "person.tz", "")))), Tzs.er(u), "")

# Automatic: back to it while a zone is set; while none is, it already is
def Tzs.automatic(set: Bool, +u: Ui) -> Act:
  match set:
    case True{}:
      Ui.rpc(u, "zone.set", [J.KV{"zone", J.Str{"auto"}}])
    case False{}:
      Act{u, Nil{}}

def Tzs.set.k(ok: Bool, +u: Ui, +zone: String) -> Act:
  match ok:
    case True{}:
      Ui.rpc(Ui.scratch_set(u, "@tz.try", zone), "zone.set", [J.KV{"zone", J.Str{zone}}])
    case False{}:
      Act{Ui.error_set(Ui.scratch_set(u, "@tz.try", Bool.pick(String, String.is_empty(zone), "?", zone)), MSG.T.tz.invalid(Lang.loc(u))), Nil{}}

def Tzs.set(+u: Ui) -> Act:
  Tzs.set.k(SC.Zone.valid(String.trim(Bots.field(u, "tzname"))), u, String.trim(Bots.field(u, "tzname")))

def Tzs.act.k(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Tzs.set(u)
    case 1n:
      Tzs.automatic(Tzs.auto(u), Ui.scratch_set(Ui.scratch_set(u, "@f.tzname", ""), "@tz.try", ""))
    case 2n:
      Ui.rpc(u, "zone.report", [J.KV{"zone", J.Str{value}}])
    case _:
      Act{u, Nil{}}

def Tzs.act(+u: Ui, +action: String, +value: String) -> Act:
  Tzs.act.k(C.Codec.pos(["tz-set", "tz-auto", "tz-report"], action), u, value)

# Settings > Usage (src/core/usage.bend): the model side from the log, Boat's
# from the hub's info "boat.usage"; every client draws these rows
def Usage.rows(+u: Ui) -> List<&2, UZ.URow>:
  List.concat(&2, UZ.URow, [UZ.Use.rows(Ui.state.of(u), Ui.threads(Ui.state.of(u))),
    UZ.Tk.rows(Ui.state.of(u), Ui.threads(Ui.state.of(u)), Ui.projects(Ui.state.of(u)), Side.now(u)), UZ.Bj.rows(Ui.state.of(u), Ui.info(u, "boat.usage"), Side.now(u))])

# a thread's Boat chip opens Settings (Usage is in it), as its button does
def Usage.page.period(+u: Ui) -> String:
  UP.Up.period(Ui.scratch(u, "@usage.period"))

def Usage.page.metric(+u: Ui) -> String:
  UP.Up.metric(Ui.scratch(u, "@usage.metric"))

def Usage.page.providers(+u: Ui) -> List<&2, String>:
  UG.Ug.enabled(Ui.info(u, "models"))

def Usage.page.provider(+u: Ui) -> String:
  UG.Ug.selected(Usage.page.providers(u), Ui.scratch(u, "@usage.provider"))

def Usage.page.index(+u: Ui, +name: String) -> Nat:
  M.Switch.num(Ui.scratch(u, "@usage." ++ name ++ "Page"), 0n)

def Usage.page.controls.more(month: Bool, +u: Ui) -> List<&2, UP.UpControl>:
  match month:
    case True{}: UP.Up.pages(Ui.state.of(u), Ui.projects(Ui.state.of(u)), Side.now(u), Usage.page.index(u, "model"), Usage.page.index(u, "project"))
    case False{}: Nil{}

def Usage.page.controls(+u: Ui) -> List<&2, UP.UpControl>:
  List.append(&2, UP.UpControl, UP.Up.controls(Usage.page.period(u), Usage.page.metric(u)),
    Usage.page.controls.more(Bool.and(String.eq(Usage.page.period(u), "month"), Bool.not(String.eq(Usage.page.metric(u), "limits"))), u))

def Usage.page.extra(limits: Bool, +u: Ui) -> List<&2, UZ.URow>:
  match limits:
    case True{}: List.append(&2, UZ.URow, UZ.Use.rows(Ui.state.of(u), Ui.threads(Ui.state.of(u))), UZ.Bj.rows(Ui.state.of(u), Ui.info(u, "boat.usage"), Side.now(u)))
    case False{}: Nil{}

def Usage.page.connection(u: Ui) -> String:
  match u:
    case Ui{_, _, _, _, _, on, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.pick(String, on, "Connected", "Disconnected / cached; readings may be stale")

def Usage.page.owner(+u: Ui) -> UZ.URow:
  +self = J.Json.get_obj(UZ.Bj.parse(Ui.info(u, "machines")), "self")
  UZ.URow{"Machine", J.Json.get_str(self, "name", "Identity unavailable") ++ " · " ++ Usage.page.connection(u), ""}

# Dedicated Usage page. The legacy Usage.rows remains available unchanged.
def Usage.page.rows(+u: Ui) -> List<&2, UZ.URow>:
  Usage.page.owner(u) <> List.append(&2, UZ.URow, UP.Up.rows(Ui.state.of(u), Ui.projects(Ui.state.of(u)), Side.now(u), Usage.page.period(u), Usage.page.metric(u), Usage.page.index(u, "model"), Usage.page.index(u, "project")),
    Usage.page.extra(String.eq(Usage.page.metric(u), "limits"), u))

def Usage.page.choose(+u: Ui, +key: String, value: String) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, key, value), "@usage.modelPage", "0"), "@usage.projectPage", "0"), Nil{}}

def Usage.page.turn(+u: Ui, +name: String, +value: String) -> Act:
  Act{Ui.scratch_set(u, "@usage." ++ name ++ "Page", Nat.show(Bool.pick(Nat, String.eq(value, "next"), Nat.add(Usage.page.index(u, name), 1n), Nat.sub(Usage.page.index(u, name), 1n)))), Nil{}}

def Usage.open(open: Bool, +u: Ui) -> Act:
  match open:
    case True{}:
      Settings.go(u, "usage")
    case False{}:
      Voice.flag("settings", Settings.go(Settings.on(u), "usage"))

# Only inspect the thread prefix, leaving the potentially long key snapshot alone.
def Ui.subs.thread(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{'\t', _}:
      String.reverse(acc)
    case SCon{c, t}:
      Ui.subs.thread(t, SCon{c, acc})

# Device-local panel choice, including the live/attention signatures at shut.
def Ui.subs.shut(u: Ui, +value: String) -> Ui:
  +th = Ui.subs.thread(value, "")
  Ui.info_set(u, "subs:" ++ th, "0" ++ String.drop(value, Nat.add(String.length(th), 1n)))

# the newest n of a list (oldest first), and how many are left out
type Win is Data:
  Win{hidden: Nat, shown: List<&2, M.Entry>}

def Page.drop(n: Nat, es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match n es:
    case 0n _:
      es
    case 1n+k _ <> t:
      Page.drop(k, t)
    case 1n+_ Nil{}:
      Nil{}

def Page.window.k(+hidden: Nat, es: List<&2, M.Entry>) -> Win:
  Win{hidden, Page.drop(hidden, es)}

def Page.window(+es: List<&2, M.Entry>, +n: Nat) -> Win:
  Page.window.k(Nat.sub(List.length(&2, M.Entry, es), n), es)

def Win.hidden(w: Win) -> Nat:
  match w:
    case Win{h, _}:
      h

def Win.shown(w: Win) -> List<&2, M.Entry>:
  match w:
    case Win{_, es}:
      es

# A thread's newest page, found from the log's newest end: the log is
# newest first, so the walk stops once it has one entry more than the page
# (which says there are earlier ones) instead of going through every
# thread's entries. An entry is the thread's, or a message it sent to
# another (its id names the sender; read only when the entry is not the
# thread's own). `hidden` is 1 when earlier entries exist, else 0.
type PW is Data:
  PW{left: Nat, acc: List<&2, M.Entry>}
  PDone{acc: List<&2, M.Entry>}

def Page.entry.thread(e: M.Entry) -> String:
  match e:
    case M.Msg{_, t, _, _, _}:
      t
    case M.Act{_, t, _, _, _, _}:
      t

def Page.entry.id(e: M.Entry) -> String:
  match e:
    case M.Msg{i, _, _, _, _}:
      i
    case M.Act{i, _, _, _, _, _}:
      i

def Page.mine(own: Bool, +e: M.Entry, +th: String) -> Bool:
  match own:
    case True{}:
      True{}
    case False{}:
      String.eq(M.Link.from(Page.entry.id(e)), th)

def Page.newest.step(own: Bool, e: M.Entry, st: PW) -> PW:
  match own st:
    case _ PDone{acc}:
      PDone{acc}
    case True{} PW{0n, acc}:
      PW{0n, acc}
    case True{} PW{1n+k, acc}:
      PW{k, e <> acc}
    case False{} PW{l, acc}:
      PW{l, acc}

def Page.entry.at(e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, _, _, at}:
      at
    case M.Act{_, _, _, _, _, at}:
      at

# an entry from before the thread began (an hour to spare for the clock)
# ends the walk: the log is in time order and nothing older is the
# thread's, so a thread with fewer entries than a page no longer walks
# the whole log (since 0: walk it all)
def Page.newest.cut(old: Bool, st: PW) -> PW:
  match old st:
    case True{} PW{_, acc}:
      PDone{acc}
    case _ _:
      st

def Page.newest.go(es: List<&2, M.Entry>, +th: String, +since: Nat, st: PW) -> PW:
  match es st:
    case +_ PW{0n, acc}:
      PW{0n, acc}
    case +_ PDone{acc}:
      PDone{acc}
    case Nil{} PW{l, acc}:
      PW{l, acc}
    case +e <> t PW{1n+k, acc}:
      Page.newest.go(t, th, since, Page.newest.cut(Nat.is_lt(Nat.add(Page.entry.at(e), 3600n), since), Page.newest.step(Page.mine(String.eq(Page.entry.thread(e), th), e, th), e, PW{1n+k, acc})))

# one more than the page was found: drop the oldest, say there are more
def Page.newest.k(st: PW) -> Win:
  match st:
    case PW{0n, _ <> rest}:
      Win{1n, rest}
    case PW{_, acc}:
      Win{0n, acc}
    case PDone{acc}:
      Win{0n, acc}

def Page.newest.from(+es: List<&2, M.Entry>, +th: String, +since: Nat, +n: Nat) -> Win:
  Page.newest.k(Page.newest.go(es, th, since, PW{1n+n, Nil{}}))

# Internal delivery acknowledgement, identified by its event fields and
# producer id. Message text and all other activity kinds remain untouched.
def Page.delivery.receipt(e: M.Entry) -> Bool:
  match e:
    case M.Act{+id, _, +tone, +kind, _, _}:
      Bool.and(String.starts_with(id, "ride:"), Bool.and(String.eq(tone, "info"), String.eq(kind, "ride")))
    case M.Msg{_, _, _, _, _}:
      False{}

# Shared with bounded memory and stored thin materialization. The caller
# enables this projection only for a bot/uberbot conversation timeline.
def Page.chat.hidden(e: M.Entry) -> Bool:
  TC.Chat.hidden(e)

# Chat-only projection during the bounded newest scan. Hidden receipts do
# not consume a page slot or create a false earlier sentinel. Source entries
# stay in the model; stop after n+1 visible entries, as the ordinary pager.
def Page.newest.visible.go(es: List<&2, M.Entry>, +th: String, +since: Nat, st: PW) -> PW:
  match es st:
    case +_ PW{0n, acc}:
      PW{0n, acc}
    case +_ PDone{acc}:
      PDone{acc}
    case Nil{} PW{l, acc}:
      PW{l, acc}
    case +e <> t PW{1n+k, acc}:
      Page.newest.visible.go(t, th, since, Page.newest.cut(Nat.is_lt(Nat.add(Page.entry.at(e), 3600n), since), Page.newest.step(Bool.and(Bool.not(Page.chat.hidden(e)), Page.mine(String.eq(Page.entry.thread(e), th), e, th)), e, PW{1n+k, acc})))

def Page.newest.visible.from(hide: Bool, +es: List<&2, M.Entry>, +th: String, +since: Nat, +n: Nat) -> Win:
  match hide:
    case True{}:
      Page.newest.k(Page.newest.visible.go(es, th, since, PW{1n+n, Nil{}}))
    case False{}:
      Page.newest.from(es, th, since, n)

def Page.newest(+es: List<&2, M.Entry>, +th: String, +n: Nat) -> Win:
  Page.newest.from(es, th, 0n, n)

# when a thread began (its entries are no older)
def Page.since.k(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, c, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      c

def Page.since(s: M.State, +th: String) -> Nat:
  Page.since.k(Ui.thread(s, th))

# The window's memos (main.bend's Mm): which threads a change touches,
# for keys that say whether a thread's view can have changed. Every change
# is named here (no catch-all: a new kind of change does not build until it
# is placed); anything not clearly one thread's own touches everything
# ("*"), so a doubtful one can only cost a memo, never show a stale view.
# A message from another thread (a link) touches its sender too. A setting
# may also change whose parent or child a thread is (launches are kept as
# settings): "!".
def Link.from.or(+msg: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(M.Link.from(msg)), Nil{}, [M.Link.from(msg)])

# A setting touches all; it may change whose parent or child a thread is
# ("!", the window's MM.Mm.rels) only when it is a launch's: Mm.rels.of
# reads the tasks (U.Tasks.up, U.Tasks.of: TaskDelegated, which touches
# both ends), the launcher of a thread ("launch.<id>", M.Launch.by) and a
# launcher's list ("launched.<by>", M.Launch.of), the settings
# ThreadLaunched writes (it touches both ends too) and a SettingSet may
def Touch.setting(+key: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.or(String.starts_with(key, "launch."), String.starts_with(key, "launched.")), ["*", "!"], ["*"])

def Touch.raw(c: M.Change) -> List<&2, String>:
  match c:
    case M.ProjectCreated{_, _, _, _}:
      ["*"]
    case M.ProjectRemoved{_, _}:
      ["*"]
    case M.ThreadCreated{+id, _, _, _, _, _, _, _, _}:
      [id]
    case M.ThreadRenamed{_, _, _}:
      ["*"]
    case M.MessagePosted{+thread, +msg, _, _, _}:
      List.concat(&2, String, [[thread], Link.from.or(msg)])
    case M.TurnChanged{+thread, _, _}:
      [thread]
    case M.PendingChanged{+thread, _, _}:
      [thread]
    case M.ActivityLogged{+thread, _, _, _, _, _}:
      [thread]
    case M.ThreadSettled{+id, _, _}:
      [id]
    case M.ThreadUnsettled{+id, _}:
      [id]
    case M.ThreadWoke{+id, _}:
      [id]
    case M.ThreadArchived{+id, _, _}:
      [id]
    case M.ThreadPinned{+id, _, _}:
      [id]
    case M.ThreadDeleted{+id, _}:
      [id]
    case M.WorktreeSet{+id, _, _, _}:
      [id]
    case M.SessionSet{+id, _, _}:
      [id]
    case M.ModesSet{+id, _, _, _, _}:
      [id]
    case M.TaskDelegated{_, +parent, +child, _, _, _, _}:
      List.concat(&2, String, [[parent], [child]])
    case M.TaskStateSet{_, _, _, _}:
      ["*"]
    case M.TaskDelivered{_, _}:
      ["*"]
    case M.SettingSet{+key, _, _}:
      Touch.setting(key)
    case M.TurnQueued{+thread, _, _, _}:
      [thread]
    case M.QueueTaken{+thread, _}:
      [thread]
    case M.AskOpened{_, +thread, _, _, _, _, _}:
      [thread]
    case M.AskClosed{_, _, _}:
      ["*"]
    case M.DeviceRegistered{_, _, _, _, _, _, _}:
      ["*"]
    case M.DeviceDropped{_, _}:
      ["*"]
    case M.ThreadSnoozed{+id, _, _}:
      [id]
    case M.ThreadOrdered{+id, _, _, _}:
      [id]
    case M.WorktreeSkipped{+id, _}:
      [id]
    case M.ThreadMerged{+id, _, _}:
      [id]
    case M.WorktreeRemoved{+id, _}:
      [id]
    case M.ProjectOrdered{_, _, _}:
      ["*"]
    case M.QueueEdited{+thread, _, _, _}:
      [thread]
    case M.QueueRemoved{+thread, _, _}:
      [thread]
    case M.QueueMoved{+thread, _, _, _}:
      [thread]
    case M.QueueTakenOne{+thread, _, _}:
      [thread]
    case M.ThreadForked{+id, +source, _}:
      List.concat(&2, String, [[id], [source]])
    case M.ProviderSet{+id, _, _, _}:
      [id]
    case M.TodosChanged{+thread, _, _, _}:
      [thread]
    case M.TodosSet{+thread, _, _}:
      [thread]
    case M.UsageSet{+thread, _, _, _}:
      [thread]
    case M.BotSet{_, _, _, _, _, _}:
      ["*"]
    case M.BotDeleted{_, _}:
      ["*"]
    case M.RoomSet{_, _, _, _}:
      ["*"]
    case M.RoomDeleted{_, _}:
      ["*"]
    case M.RoomPosted{_, _, _, _, _, _}:
      ["*"]
    case M.HookSet{_, _, _, _, _}:
      ["*"]
    case M.HookHit{_, _}:
      ["*"]
    case M.RoutineSet{_, _, _, _, _, _, _}:
      ["*"]
    case M.RoutineDeleted{_, _}:
      ["*"]
    case M.RoutineFired{_, _}:
      ["*"]
    case M.MemorySaved{_, _, _, _, _, _}:
      ["*"]
    case M.MemoryForgot{_, _, _}:
      ["*"]
    case M.SpaceSet{_, _, _}:
      ["*"]
    case M.PeerSet{_, _, _, _, _}:
      ["*"]
    case M.BotWoke{_, _, _}:
      ["*"]
    case M.SpacePageSet{_, _, _}:
      ["*"]
    case M.WorkSet{+thread, _, _, _}:
      [thread]
    case M.UberSet{_, _, _}:
      ["*"]
    case M.UberOp{_, _, _, _, _, _}:
      ["*"]
    case M.ThreadParentSet{+id, +parent, _}:
      ["*", "!", id, parent]
    case M.ThreadLaunched{+id, +by, _, _}:
      List.concat(&2, String, [[id], [by]])
    case M.WakeSet{_, _, _, _, _, _, _, _}:
      ["*"]
    case M.WakeDropped{_, _}:
      ["*"]
    case M.LockChanged{_, _, _}:
      ["*"]
    case M.RoutinePlan{_, _, _, _, _}:
      ["*"]
    case M.RoutineSlot{_, _, _}:
      ["*"]
    case M.RoutineDone{_, _}:
      ["*"]
    case M.Reminded{_, _, _, _, _}:
      ["*"]
    case M.TaskSet{_, _, _, _}:
      ["*"]
    case M.TaskDeleted{_, _, _}:
      ["*"]
    case M.MessageFrom{thread, _, _, _, _, _}:
      [thread]
    case M.ConvSet{_, _, _, _, _, _, _}:
      ["@mail"]
    case M.MailPosted{_, _, _, _, _, _, _, _, _, _}:
      ["@mail"]
    case M.MailMarked{_, _, _, _, _, _}:
      ["@mail"]
    case M.MailTagged{_, _, _, _}:
      ["@mail"]
    case M.RunSet{_, _, _, _, _}:
      ["@mail"]
    case M.CharterSet{_, _, _, _}:
      ["@mail"]
    case M.DecisionLogged{_, _, _, _, _, _, _}:
      ["@mail"]
    case M.TurnUsage{+thread, _, _, _, _, _, _, _, _, _, _, _, _}:
      [thread]
    case M.DeployAsked{_, _, _, _, _}:
      ["*", "!"]
    case M.DeployBuilt{_, _, _, _, _}:
      ["*", "!"]

# a thread id as touched: a subagent's log ("@sub:<thread>", model.bend's
# Bg.log) is its thread's; any other name with "@" is no plain thread, so
# it touches everything
def Touch.id(+id: String) -> String:
  Bool.pick(String, String.starts_with(id, "@sub:"), String.drop(id, 5n), Bool.pick(String, Bool.and(String.starts_with(id, "@"), Bool.not(String.eq(id, "@mail"))), "*", id))

def Touch.ids(ids: List<&2, String>) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case +id <> t:
      Touch.id(id) <> Touch.ids(t)

def Touch.of(c: M.Change) -> List<&2, String>:
  Touch.ids(Touch.raw(c))

# Native content can only be embedded in this machine's own window
# (core/cef.bend): the page given (a thread's, or a bot's in its tab) shows
# natively when this window's hub is its own and runs cef.
def CefUI.native(+u: Ui, +page: String) -> Bool:
  Bool.and(CF.Cef.native(Ui.info(u, "browser.engine.live"), Ui.info(u, "local.remote.url")), CF.Cef.page_local(page))

# an overlay drawn over the main area is open (a toast is not one)
def CefUI.covered(+u: Ui) -> Bool:
  CF.Cef.covered(Ui.flag(u, "palette"), Ui.flag(u, "proj"), Bool.not(String.is_empty(Menu.open(u))), Bool.not(String.is_empty(Ui.scratch(u, "@look"))),
    Ui.flag(u, "models"), Tour.on(u), Ui.flag(u, "perf"), Bool.not(String.is_empty(Ui.scratch(u, "@lb"))))

def CefUI.strip(+u: Ui, +id: String) -> String:
  CF.Cef.drive_text(Teach.drive(u, id), Bool.and(String.eq(Ui.scratch(u, "@cef.window"), "1"), CefUI.native(u, id)))

def CefUI.strip_on(+u: Ui, +id: String) -> Bool:
  CF.Cef.strip_on(Ui.info(u, "browser.engine.live"), Teach.drive(u, id))

# ---- global fallback Settings: shared row model for every client ----
def Fallbackv.state(u: Ui) -> M.State:
  Ui.state.of(u)

def Fallbackv.chain(u: Ui) -> List<&2, FB.Target>:
  FB.Chain.of(Ui.state.of(u))

def Fallbackv.btn(label: String, value: String) -> AgBtn:
  AgBtn{label, "setting", value, False{}}

def Fallbackv.order(xs: List<&2, FB.Target>, +all: List<&2, FB.Target>, +caps: J.Json, +s: M.State, +n: Nat) -> List<&2, AgRow>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t:
      +key = FB.Target.key(h)
      AgRow{Nat.show(n) ++ ". " ++ key, Bool.pick(String, FB.Caps.valid(caps, h), "", "Unavailable on this machine. Skipped during fallback."), Nil{},
        [Fallbackv.btn("Move up", "fallback.chain=" ++ FB.Chain.show(FB.Chain.up(all, key))),
         Fallbackv.btn("Move down", "fallback.chain=" ++ FB.Chain.show(List.reverse(&2, FB.Target, FB.Chain.up(List.reverse(&2, FB.Target, all), key)))),
         Fallbackv.btn("Remove", "fallback.chain=" ++ FB.Chain.show(FB.Chain.remove(all, key)))]} <> Fallbackv.order(t, all, caps, s, 1n+n)

def Fallbackv.provider.k(missing: Bool, +p: String, +cur: String, rest: List<&2, AgBtn>) -> List<&2, AgBtn>:
  match missing:
    case True{}: rest
    case False{}: AgBtn{p, "setting", "fallback.add.provider=" ++ p, String.eq(p, cur)} <> rest

def Fallbackv.providers(ps: List<&2, String>, +caps: J.Json, +cur: String) -> List<&2, AgBtn>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t:
      Fallbackv.provider.k(List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(caps, p))), p, cur, Fallbackv.providers(t, caps, cur))

def Fallbackv.models(xs: List<&2, String>, +cur: String) -> List<&2, AgBtn>:
  match xs:
    case Nil{}: Nil{}
    case +m <> t: AgBtn{m, "setting", "fallback.add.model=" ++ m, String.eq(m, cur)} <> Fallbackv.models(t, cur)

def Fallbackv.add.k(valid: Bool, +all: List<&2, FB.Target>, target: FB.Target) -> List<&2, AgBtn>:
  match valid:
    case False{}: Nil{}
    case True{}: [Fallbackv.btn("Add to hierarchy", "fallback.chain=" ++ FB.Chain.show(List.append(&2, FB.Target, all, [target])))]

def Fallbackv.add(+u: Ui, +caps: J.Json) -> List<&2, AgRow>:
  +s = Ui.state.of(u)
  +p = M.Setting.get(s, "fallback.add.provider", "codex")
  +m = M.Setting.get(s, "fallback.add.model", "gpt-6.1-sol")
  +target = {FB.Target{p, m} : FB.Target}
  [AgRow{"Add a provider", "Choose an installed provider and model, then add it to the end.", Nil{}, Fallbackv.providers(["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], caps, p)},
   AgRow{"Add a model", "Only installed models can be saved. Duplicate entries are not allowed.", Nil{}, Fallbackv.models(Models.list(Ui.info(u, "models"), p), m)},
   AgRow{FB.Target.key(target), "", Nil{}, Fallbackv.add.k(Bool.and(FB.Caps.valid(caps, target), Bool.not(FB.Chain.has(Fallbackv.chain(u), FB.Target.key(target)))), Fallbackv.chain(u), target)}]

def Fallbackv.outage.one(held: Bool, +scope: String, +o: J.Json) -> List<&2, AgRow>:
  match held:
    case False{}: Nil{}
    case True{}: [AgRow{"Unavailable: " ++ scope, J.Json.get_str(o, "reason", "Usage limit") ++ Bool.pick(String, Nat.is_eq(J.Json.get_nat(o, "reset", 0n), 0n), ". No reset supplied; held until you clear it.", ". Eligible again after the provider's reported reset."), Nil{}, [Fallbackv.btn("Clear availability hold", "fallback.outage." ++ scope ++ "=")]}]

def Fallbackv.outages(xs: List<&2, String>, +s: M.State, +now: Nat) -> List<&2, AgRow>:
  match xs:
    case Nil{}: Nil{}
    case +sc <> rest: List.append(&2, AgRow, Fallbackv.outage.one(FB.Limit.active(s, sc, now), sc, FB.Limit.record(s, sc)), Fallbackv.outages(rest, s, now))

def Fallbackv.scopes(xs: List<&2, FB.Target>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: FB.Target.key(h) <> Fallbackv.scopes(t)

def Fallbackv.rows(+u: Ui) -> List<&2, AgRow>:
  +s = Ui.state.of(u)
  +caps = Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "models")), J.Null{})
  +all = Fallbackv.chain(u)
  List.concat(&2, AgRow, [[AgRow{"Fallback hierarchy", "Applies to all threads and bots, including delegates, routines and mail runs. Your chosen model runs first. At a confirmed usage limit, try entries after it; if it is absent, start at the top. Never wrap or switch working threads back. Login and network failures do not change provider.", Nil{},
    [AgBtn{"Enabled", "setting", "fallback.on=on", FB.Chain.on(s)}, AgBtn{"Disabled", "setting", "fallback.on=off", Bool.not(FB.Chain.on(s))}]}],
    Fallbackv.order(all, all, caps, s, 1n), Fallbackv.add(u, caps),
    [AgRow{"Fallback notifications", "One alert per outage, with an audit row in each affected thread. If the whole chain is unavailable, work pauses and asks for attention. A limit without a reset stays held until cleared here.", Nil{}, Nil{}}],
    Fallbackv.outages(List.append(&2, String, ["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], Fallbackv.scopes(all)), s, Side.now(u))])

def Mailv.put(u: Ui, m: MailvData) -> Ui:
  match u:
    case Ui{st, sel, dr, lv, op, on, now, ad, pa, v, rid, w, er, no, nf, ob, MailvLocal{ts, _}}:
      Ui{st, sel, dr, lv, op, on, now, ad, pa, v, rid, w, er, no, nf, ob, MailvLocal{ts, [m]}}

def Mailv.cache.of(m: MailvData) -> MV.Cache:
  match m:
    case MailvData{c, _, _, _, _, _, _}: c

def Mailv.find.of(m: MailvData) -> MV.Find:
  match m:
    case MailvData{_, f, _, _, _, _, _}: f

def Mailv.bubbles.of(m: MailvData) -> List<&2, MV.Bub>:
  match m:
    case MailvData{_, _, b, _, _, _, _}: b

def Mailv.targets.of(m: MailvData) -> List<&2, MV.Opt>:
  match m:
    case MailvData{_, _, _, t, _, _, _}: t

def Mailv.rev.of(m: MailvData) -> Nat:
  match m:
    case MailvData{_, _, _, _, r, _, _}: r

def Mailv.touch.of(m: MailvData) -> Nat:
  match m:
    case MailvData{_, _, _, _, _, t, _}: t

def Mailv.owner.thread(b: Maybe<&2, BT.Bot>) -> String:
  match b:
    case Some{x}: BT.Bot.id(x)
    case None{}: ""

def Mailv.owner.explicit(empty: Bool, +u: Ui) -> String:
  match empty:
    case False{}: Ui.scratch(u, "@bot")
    case True{}: Mailv.owner.thread(BT.World.of_thread(M.State.bots(Ui.state(u)), Ui.sel(u)))

def Mailv.owner(+u: Ui) -> String:
  Mailv.owner.explicit(String.is_empty(Ui.scratch(u, "@bot")), u)

def Mailv.base(+u: Ui) -> String:
  Bool.pick(String, UB.Uber.is(Ui.state(u), Mailv.owner(u)), "*", Mailv.owner(u))

def Mailv.bot(+u: Ui) -> String:
  Bool.pick(String, Bool.and(UB.Uber.is(Ui.state(u), Mailv.owner(u)), Bool.not(String.is_empty(Ui.scratch(u, "@mr.bot")))), Ui.scratch(u, "@mr.bot"), Mailv.base(u))

def Mailv.tab(+u: Ui) -> String:
  Bool.pick(String, String.is_empty(Ui.scratch(u, "@btab")), Bool.pick(String, UB.Uber.is(Ui.state(u), Mailv.owner(u)), "mail", "chat"), Ui.scratch(u, "@btab"))

# Capability supplied only by a client with a Mailroom renderer.
def Mailv.enabled(+u: Ui) -> Bool:
  String.eq(Ui.info(u, "local.mailroom"), "on")

def Mailv.shown(+u: Ui) -> Bool:
  Bool.and(Mailv.enabled(u), Bool.and(Bool.not(Ui.flag(u, "settings")), Bool.and(Bool.not(String.is_empty(Mailv.owner(u))), Bool.and(Bool.not(FR.Fr.is(Mailv.owner(u))), String.eq(Mailv.tab(u), "mail")))))

def Mailv.filter(+u: Ui) -> MV.Filter:
  MV.Filter{Mailv.bot(u), Ui.scratch(u, "@mr.status"), Ui.scratch(u, "@mr.project"), Ui.scratch(u, "@f.mrsearch")}

def Mailv.key(+u: Ui) -> String:
  MV.Mv.key(Mailv.filter(u))

def Mailv.scroll(+u: Ui) -> Nat:
  M.Switch.num(Ui.info(u, "pane:mr"), 0n)

def Mailv.skip(+u: Ui) -> Nat:
  Nat.mul(Nat.div(Nat.sub(Mailv.scroll(u), 2n), 50n), 50n)

def Mailv.token(+u: Ui) -> String:
  Mailv.key(u) ++ "/" ++ Nat.show(Mailv.skip(u)) ++ "/" ++ Ui.scratch(u, "@mr.dirty")

def Mailv.badge(+u: Ui, +id: String) -> MV.Badge:
  MV.Mv.badge.n(id, Bool.pick(Nat, UB.Uber.is(Ui.state(u), id), ML.Box.unread_all(M.State.mail(Ui.state(u))), ML.Box.unread(M.State.mail(Ui.state(u)), id)))

def Mailv.run_open(u: Ui) -> Bool:
  String.eq(Ui.scratch(u, "@mr.run"), "1")

# A chip is a view edit, never a bot navigation action.
def Mailv.chip(+u: Ui, +v: String) -> Ui:
  Ui.info_set(Ui.scratch_set(u, "@mr.bot", v), "pane:mr", "0")

def Mailv.change_conv(c: M.Change) -> String:
  match c:
    case M.RoomSet{id, _, _, _}: "room:" ++ id
    case M.RoomPosted{_, room, _, _, _, _}: "room:" ++ room
    case M.ConvSet{i, _, _, _, _, _, _}: i
    case M.MailPosted{_, i, _, _, _, _, _, _, _, _}: i
    case M.MailMarked{_, i, _, _, _, _}: i
    case M.MailTagged{_, i, _, _}: i
    case M.RunSet{_, _, i, _, _}: i
    case M.DecisionLogged{_, _, i, _, _, _, _}: i
    case _: ""

def Mailv.bump(m: MailvData) -> MailvData:
  match m:
    case MailvData{c, f, b, t, r, n, ix}: MailvData{c, f, b, t, r, 1n+n, ix}

# Non-durable reads are rate limited separately from durable sends/marks.
# A reply retains the exact filter, page and touch it was requested for.
def Mailv.request(+u: Ui, +method: String, +params: List<&2, J.KV>, +slot: String, +tag: String) -> Act:
  Ui.rpc(Ui.scratch_set(Ui.scratch_set(u, "@mr.req:" ++ Nat.show(Ui.rid(u)), J.Json.show(J.Json.obj([J.KV{"m", J.Str{method}}, J.KV{"p", J.Json.obj(params)}, J.KV{"slot", J.Str{slot}}, J.KV{"tag", J.Str{tag}}]))), "@mr." ++ slot, Nat.show(Ui.rid(u))), method, params)

def Mailv.before.msg(m: MV.Msg) -> String:
  match m:
    case MV.Msg{i, _, _, _, _, _, _}: i

def Mailv.before.opt(m: Maybe<&2, MV.Msg>) -> String:
  match m:
    case Some{x}: Mailv.before.msg(x)
    case None{}: ""

def Mailv.read(+u: Ui, +before: String) -> Act:
  Mailv.request(Ui.scratch_set(u, "@mr.read.dirty", ""), "mail.read", [J.KV{"bot", J.Str{Mailv.bot(u)}}, J.KV{"conv", J.Str{Ui.scratch(u, "@mr.conv")}}, J.KV{"before", J.Str{before}}, J.KV{"n", J.Json.nat(50n)}], "read", Ui.scratch(u, "@mr.conv"))

def Mailv.search.set(m: MailvData, f: MV.Find) -> MailvData:
  match m:
    case MailvData{c, _, b, t, r, n, ix}: MailvData{c, f, b, t, 1n+r, n, ix}

def Mailv.live_targets(rs: List<&2, ML.Run>, +conv: String) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t:
      MV.Mv.cons_if(Bool.and(ML.Run.live(r), String.eq(ML.Run.conv(r), conv)), "thread:" ++ ML.Run.thread(r), Mailv.live_targets(t, conv))

# A room mailbox has an explicit file-only Room recipient in addition to
# its existing members/live runs. Keep existing option order and selection.
def Mailv.room.target.end(found: Bool, room: String) -> List<&2, MV.Opt>:
  match found:
    case True{}: Nil{}
    case False{}: [MV.Opt{room, "Room"}]

def Mailv.room.target.row(same: Bool, found: Bool, +addr: String, label: String, rest: List<&2, MV.Opt>) -> List<&2, MV.Opt>:
  match same found:
    case False{} _: MV.Opt{addr, label} <> rest
    case True{} False{}: MV.Opt{addr, "Room"} <> rest
    case True{} True{}: rest

def Mailv.room.target.rows(os: List<&2, MV.Opt>, +room: String, +found: Bool) -> List<&2, MV.Opt>:
  match os:
    case Nil{}: Mailv.room.target.end(found, room)
    case MV.Opt{+addr, label} <> t:
      Mailv.room.target.row(String.eq(addr, room), found, addr, label, Mailv.room.target.rows(t, room, Bool.or(found, String.eq(addr, room))))

def Mailv.room.target.ref(ref: Maybe<&2, R.Ref>, os: List<&2, MV.Opt>) -> List<&2, MV.Opt>:
  match ref:
    case Some{R.Ref{"room", id <> Nil{}, _}}: Mailv.room.target.rows(os, R.Ref.addr(R.Ref.room(id)), False{})
    case _: os

def Mailv.room.targets(conv: String, os: List<&2, MV.Opt>) -> List<&2, MV.Opt>:
  Mailv.room.target.ref(R.Ref.of_addr(conv), os)

def Mailv.read.put(+m: MailvData, +c: MV.Cache, +conv: String, +names: Map<&2, String>, +live: List<&2, String>) -> MailvData:
  match m:
    case MailvData{_, f, _, _, r, n, ix}:
      MailvData{c, f, MV.Mv.bubbles(MV.Mv.read_of(c, conv), names), Mailv.room.targets(MV.Read.id(MV.Mv.read_of(c, conv)), MV.Mv.targets(MV.Read.parts(MV.Mv.read_of(c, conv)), live, names)), 1n+r, n, ix}

def Mailv.list.put(m: MailvData, c: MV.Cache) -> MailvData:
  match m:
    case MailvData{_, f, b, t, r, n, ix}: MailvData{c, f, b, t, 1n+r, n, ix}

def Mailv.filter.kind(bot: Bool, project: Bool, machine: Bool, +u: Ui, +v: String) -> Ui:
  match bot project machine:
    case True{} _ _: Mailv.chip(u, String.drop(v, 4n))
    case False{} True{} _: Ui.scratch_set(u, "@mr.project", String.drop(v, 8n))
    case False{} False{} True{}: Ui.scratch_set(u, "@mr.machine", String.drop(v, 8n))
    case False{} False{} False{}:
      Ui.scratch_set(u, "@mr.status", Bool.pick(String, String.eq(Ui.scratch(u, "@mr.status"), v), "", v))

def Mailv.filter.at(i: Nat, +u: Ui, +v: String) -> Ui:
  match i:
    case 0n: Mailv.chip(u, String.drop(v, 4n))
    case _:
      Mailv.filter.kind(String.starts_with(v, "bot="), String.starts_with(v, "project="), String.starts_with(v, "machine="), u, v)

def Mailv.filter.set(+u: Ui, +v: String) -> Ui:
  Ui.info_set(Mailv.filter.at(C.Codec.pos(["bot", "project", "machine"], String.take(v, 3n)), u, v), "pane:mr", "0")

def Mailv.send.ok(ok: Bool, +u: Ui, +to: String, +text: String) -> Act:
  match ok:
    case False{}: Act{u, Nil{}}
    case True{}:
      Ui.rpc.keep(Ui.scratch_set(u, "@f.mrreply", ""), "mail.send", [J.KV{"conv", J.Str{Ui.scratch(u, "@mr.conv")}}, J.KV{"to", J.Str{to}}, J.KV{"text", J.Str{text}}, J.KV{"want", J.Str{"act"}}])

# Reading marks a conversation, but a mark must never trigger another
# read. Only a newly filed body refreshes the open conversation.
def Mailv.see.room.body(off: Bool, +u: Ui, +room: String) -> Ui:
  match off:
    case False{}: u
    case True{}: Ui.scratch_set(u, "@mr.read.dirty", Bool.pick(String, String.eq(Ui.scratch(u, "@mr.conv"), "room:" ++ room), "1", Ui.scratch(u, "@mr.read.dirty")))

def Mailv.see.body(c: M.Change, +u: Ui) -> Ui:
  match c:
    case M.RoomPosted{_, room, _, _, _, _}: Mailv.see.room.body(Bool.not(String.eq(M.Setting.get(Ui.state(u), "mail.on", "off"), "on")), u, room)
    case M.MailPosted{_, +conv, _, _, _, _, _, _, _, _}:
      Ui.scratch_set(u, "@mr.read.dirty", Bool.pick(String, String.eq(Ui.scratch(u, "@mr.conv"), conv), "1", Ui.scratch(u, "@mr.read.dirty")))
    case _: u

def Mailv.status_rpc(+status: String) -> String:
  Bool.pick(String, Nat.is_lt(C.Codec.pos(["unread", "open", "snoozed", "done"], status), 4n), status, "")

def Mailv.list(+u: Ui) -> Act:
  Mailv.request(u, "mail.list", [J.KV{"bot", J.Str{Mailv.bot(u)}}, J.KV{"status", J.Str{Mailv.status_rpc(Ui.scratch(u, "@mr.status"))}}, J.KV{"project", J.Str{Ui.scratch(u, "@mr.project")}}, J.KV{"view_status", J.Str{Ui.scratch(u, "@mr.status")}}, J.KV{"skip", J.Json.nat(Mailv.skip(u))}, J.KV{"n", J.Json.nat(100n)}], "list", Mailv.token(u))

def Mailv.join(on: Bool, u: Ui) -> Ui:
  match on:
    case False{}: u
    case True{}:
      Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@mr.list", ""), "@mr.read", ""), "@mr.find", ""), "@mr.loaded", ""), "@mr.qkey", "")

# Act/FYI are absent from the 2b list RPC's status enum. A small derived
# order per bot/project is maintained while folding headers, never while
# laying out. The ordinary MV page cache still owns every displayed row.
def Mailv.index.empty() -> MailvIndex:
  MailvIndex{Map.new(&2, ML.Order), Map.new(&2, ML.Conv), Map.new(&2, Nat)}

# ---- mail ui ----
# Parsed pages, search hits and grouped bubbles live in the typed Ui stores.
# @mr.* are small view/request identities only. No JSON is read by layout.
def Mailv.empty() -> MailvData:
  MailvData{MV.Mv.cache(), MV.Mv.found(), Nil{}, Nil{}, 0n, 0n, Mailv.index.empty()}

def Mailv.first(xs: List<&2, MailvData>) -> MailvData:
  match xs:
    case Nil{}: Mailv.empty()
    case x <> _: x

def Mailv.data(u: Ui) -> MailvData:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, MailvLocal{_, m}}: Mailv.first(m)

def Mailv.cache(u: Ui) -> MV.Cache:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, MailvLocal{_, m}}: Mailv.cache.of(Mailv.first(m))

def Mailv.find(u: Ui) -> MV.Find:
  Mailv.find.of(Mailv.data(u))

def Mailv.bubbles(u: Ui) -> List<&2, MV.Bub>:
  Mailv.bubbles.of(Mailv.data(u))

def Mailv.targets(u: Ui) -> List<&2, MV.Opt>:
  Mailv.targets.of(Mailv.data(u))

def Mailv.rev(u: Ui) -> Nat:
  Mailv.rev.of(Mailv.data(u))

def Mailv.touch(u: Ui) -> Nat:
  Mailv.touch.of(Mailv.data(u))

def Mailv.see.k(affected: Bool, +u: Ui, +conv: String) -> Ui:
  match affected:
    case False{}: u
    case True{}:
      Ui.scratch_set(u, "@mr.dirty", Nat.show(Mailv.touch(u)))

def Mailv.see.mail(yes: Bool, +u: Ui, +conv: String) -> Ui:
  match yes:
    case False{}: u
    case True{}:
      Mailv.see.k(Bool.or(String.eq(Mailv.bot(u), "*"), ML.Parts.has(ML.Conv.parts(ML.Box.conv(M.State.mail(Ui.state(u)), conv)), "bot:" ++ Mailv.bot(u), False{})), Mailv.put(u, Mailv.bump(Mailv.data(u))), conv)

def Mailv.before(+u: Ui) -> String:
  Mailv.before.opt(List.head(&2, MV.Msg, Map.values(&2, MV.Msg, MV.Read.msgs(MV.Mv.read_of(Mailv.cache(u), Ui.scratch(u, "@mr.conv"))))))

def Mailv.found(+u: Ui, +q: String, +result: J.Json) -> Ui:
  Mailv.put(u, Mailv.search.set(Mailv.data(u), MV.Find{q, MV.Mv.merge_hits(MV.Find.hits(Mailv.find(u)), MV.Mv.parse(result, "person", ML.Box.names(M.State.mail(Ui.state(u))))), False{}}))

def Mailv.reply.find(same: Bool, +u: Ui, +q: String, +res: J.Json) -> Ui:
  match same:
    case True{}: Mailv.found(u, q, res)
    case False{}: u

def Mailv.index.of(m: MailvData) -> MailvIndex:
  match m:
    case MailvData{_, _, _, _, _, _, ix}: ix

def Mailv.index(u: Ui) -> MailvIndex:
  Mailv.index.of(Mailv.data(u))

def Mailv.index.set(m: MailvData, ix: MailvIndex) -> MailvData:
  match m:
    case MailvData{c, f, b, t, r, n, _}: MailvData{c, f, b, t, r, n, ix}

def Mailv.index.key(+bot: String, +want: String, +project: String) -> String:
  Nat.show(String.length(bot)) ++ ":" ++ bot ++ want ++ ":" ++ project

def Mailv.index.orders(ix: MailvIndex) -> Map<&2, ML.Order>:
  match ix:
    case MailvIndex{os, _, _}: os

def Mailv.index.order(+ix: MailvIndex, +key: String) -> ML.Order:
  ML.Box.order_got(Map.get(ML.Order, ML.OEmpty{}, Mailv.index.orders(ix), key))

def Mailv.index.old(ix: MailvIndex, +id: String) -> ML.Conv:
  match ix:
    case MailvIndex{_, old, _}: ML.Get.conv.got(Map.get(ML.Conv, ML.Conv.empty(), old, id))

def Mailv.index.act(ix: MailvIndex) -> Map<&2, Nat>:
  match ix:
    case MailvIndex{_, _, act}: act

def Mailv.act_unread(+u: Ui) -> Nat:
  ML.Get.nat(Mailv.index.act(Mailv.index(u)), "total")

def Mailv.index.tree(yes: Bool, +ix: MailvIndex, +key: String, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  match yes ix:
    case False{} MailvIndex{+os, cs, ns}:
      MailvIndex{Map.set(&2, ML.Order, os, key, ML.Order.drop(ML.Box.order_got(Map.get(ML.Order, ML.OEmpty{}, os, key)), ML.Conv.last(old), ML.Conv.id(cv))), cs, ns}
    case True{} MailvIndex{+os, cs, ns}:
      MailvIndex{Map.set(&2, ML.Order, os, key, ML.Order.put(ML.Order.drop(ML.Box.order_got(Map.get(ML.Order, ML.OEmpty{}, os, key)), ML.Conv.last(old), ML.Conv.id(cv)), ML.Conv.last(cv), ML.Conv.id(cv))), cs, ns}

def Mailv.index.pair(+ix: MailvIndex, +bot: String, +project: String, +want: String, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  Mailv.index.tree(String.eq(want, "fyi"), Mailv.index.tree(String.eq(want, "act"), ix, Mailv.index.key(bot, "act", project), old, cv), Mailv.index.key(bot, "fyi", project), old, cv)

def Mailv.index.count.put(ix: MailvIndex, ns: Map<&2, Nat>) -> MailvIndex:
  match ix:
    case MailvIndex{os, cs, _}: MailvIndex{os, cs, ns}

def Mailv.index.count(+ix: MailvIndex, +bot: String, +box: ML.Box, +cv: ML.Conv) -> MailvIndex:
  +ns = Mailv.index.act(ix)
  +key = "conv:" ++ Nat.show(String.length(bot)) ++ ":" ++ bot ++ ML.Conv.id(cv)
  +was = ML.Get.nat(ns, key)
  +now = Bool.pick(Nat, Bool.and(String.eq(ML.Mark.status(ML.Box.mark_of(box, bot, ML.Conv.id(cv))), "unread"), Bool.or(Nat.is_gt(was, 0n), String.eq(ML.Mail.wanted(ML.Box.newest(box, ML.Conv.id(cv))), "act"))), 1n, 0n)
  Mailv.index.count.put(ix, Map.set(&2, Nat, Map.set(&2, Nat, Map.set(&2, Nat, ns, key, now), "bot:" ++ bot, Nat.add(Nat.sub(ML.Get.nat(ns, "bot:" ++ bot), was), now)), "total", Nat.add(Nat.sub(ML.Get.nat(ns, "total"), was), now)))

def Mailv.index.finish(ix: MailvIndex, +cv: ML.Conv) -> MailvIndex:
  match ix:
    case MailvIndex{os, cs, ns}: MailvIndex{os, Map.set(&2, ML.Conv, cs, ML.Conv.id(cv), cv), ns}

def Mailv.index.merge_status(+a: String, +b: String) -> String:
  Bool.pick(String, Bool.or(String.eq(a, "unread"), String.eq(b, "unread")), "unread", Bool.pick(String, Bool.or(String.eq(a, "snoozed"), String.eq(b, "snoozed")), "snoozed", Bool.pick(String, Bool.or(String.eq(a, "open"), String.eq(b, "open")), "open", "done")))

# Aggregate marks for All mail: one unread participant makes it unread.
def Mailv.index.aggregate(ps: List<&2, String>, +box: ML.Box, +id: String, +status: String) -> String:
  match ps:
    case Nil{}: status
    case +p <> t:
      Mailv.index.aggregate(t, box, id, Mailv.index.merge_status(status, ML.Mark.status(ML.Box.mark_of(box, String.drop(p, 4n), id))))

def Mailv.index.status(+ix: MailvIndex, +bot: String, +project: String, +status: String, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  Mailv.index.tree(String.eq(status, "done"), Mailv.index.tree(String.eq(status, "snoozed"), Mailv.index.tree(String.eq(status, "unread"), ix, Mailv.index.key(bot, "unread", project), old, cv), Mailv.index.key(bot, "snoozed", project), old, cv), Mailv.index.key(bot, "done", project), old, cv)

def Mailv.index.markorders(+ix: MailvIndex, +bot: String, +box: ML.Box, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  +status = Bool.pick(String, String.eq(bot, "*"), Mailv.index.aggregate(ML.Conv.parts(cv), box, ML.Conv.id(cv), "done"), ML.Mark.status(ML.Box.mark_of(box, bot, ML.Conv.id(cv))))
  Mailv.index.tree(True{}, Mailv.index.tree(True{}, Mailv.index.status(Mailv.index.status(ix, bot, "", status, old, cv), bot, ML.Conv.project(cv), status, old, cv), Mailv.index.key(bot, "", ""), old, cv), Mailv.index.key(bot, "", ML.Conv.project(cv)), old, cv)

def Mailv.index.bot(+ix: MailvIndex, +bot: String, +box: ML.Box, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  +want = ML.Mail.wanted(ML.Box.newest(box, ML.Conv.id(cv)))
  Mailv.index.count(Mailv.index.markorders(Mailv.index.pair(Mailv.index.pair(ix, bot, "", want, old, cv), bot, ML.Conv.project(cv), want, old, cv), bot, box, old, cv), bot, box, cv)

def Mailv.index.part(bot: Bool, ix: MailvIndex, +addr: String, +box: ML.Box, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  match bot:
    case False{}: ix
    case True{}: Mailv.index.bot(ix, String.drop(addr, 4n), box, old, cv)

def Mailv.index.parts(ps: List<&2, String>, ix: MailvIndex, +box: ML.Box, +old: ML.Conv, +cv: ML.Conv) -> MailvIndex:
  match ps:
    case Nil{}: ix
    case +p <> t:
      Mailv.index.parts(t, Mailv.index.part(String.starts_with(p, "bot:"), ix, p, box, old, cv), box, old, cv)

def Mailv.index.conv(+u: Ui, +id: String) -> Ui:
  +ix = Mailv.index(u)
  +box = M.State.mail(Ui.state(u))
  +cv = ML.Box.conv(box, id)
  +old = Mailv.index.old(ix, id)
  +want = ML.Mail.wanted(ML.Box.newest(box, id))
  Mailv.put(u, Mailv.index.set(Mailv.data(u), Mailv.index.finish(Mailv.index.parts(ML.Conv.parts(cv), Mailv.index.markorders(Mailv.index.pair(Mailv.index.pair(ix, "*", "", want, old, cv), "*", ML.Conv.project(cv), want, old, cv), "*", box, old, cv), box, old, cv), cv)))

# RoomSet may replace membership. Clear only this room's old participant
# entries before rebuilding its indexed projection from the canonical box.
def Mailv.index.room.keys(ks: List<&2, String>, +ix: MailvIndex, +bot: String, +old: ML.Conv) -> MailvIndex:
  match ks:
    case Nil{}: ix
    case +k <> t: Mailv.index.room.keys(t, Mailv.index.tree(False{}, Mailv.index.tree(False{}, ix, Mailv.index.key(bot, k, ""), old, old), Mailv.index.key(bot, k, ML.Conv.project(old)), old, old), bot, old)

def Mailv.index.room.count(+ix: MailvIndex, +bot: String, +old: ML.Conv) -> MailvIndex:
  +ns = Mailv.index.act(ix)
  +key = "conv:" ++ Nat.show(String.length(bot)) ++ ":" ++ bot ++ ML.Conv.id(old)
  +was = ML.Get.nat(ns, key)
  Mailv.index.count.put(ix, Map.set(&2, Nat, Map.set(&2, Nat, Map.set(&2, Nat, ns, key, 0n), "bot:" ++ bot, Nat.sub(ML.Get.nat(ns, "bot:" ++ bot), was)), "total", Nat.sub(ML.Get.nat(ns, "total"), was)))

def Mailv.index.room.part(bot: Bool, +ix: MailvIndex, +addr: String, +old: ML.Conv) -> MailvIndex:
  match bot:
    case False{}: ix
    case True{}: Mailv.index.room.count(Mailv.index.room.keys(["", "act", "fyi", "unread", "snoozed", "done"], ix, String.drop(addr, 4n), old), String.drop(addr, 4n), old)

def Mailv.index.room.parts(ps: List<&2, String>, ix: MailvIndex, +old: ML.Conv) -> MailvIndex:
  match ps:
    case Nil{}: ix
    case +p <> t: Mailv.index.room.parts(t, Mailv.index.room.part(String.starts_with(p, "bot:"), ix, p, old), old)

def Mailv.index.room(+u: Ui, +id: String) -> Ui:
  +ix = Mailv.index(u)
  +old = Mailv.index.old(ix, id)
  Mailv.index.conv(Mailv.put(u, Mailv.index.set(Mailv.data(u), Mailv.index.room.parts(ML.Conv.parts(old), ix, old))), id)

def Mailv.index.update(c: M.Change, u: Ui) -> Ui:
  match c:
    case M.RoomSet{id, _, _, _}: Mailv.index.room(u, "room:" ++ id)
    case M.RoomPosted{_, room, _, _, _, _}: Mailv.index.conv(u, "room:" ++ room)
    case M.ConvSet{+id, _, _, _, _, _, _}: Mailv.index.conv(u, id)
    case M.MailPosted{_, +id, _, _, _, _, _, _, _, _}: Mailv.index.conv(u, id)
    case M.MailMarked{_, +id, _, _, _, _}: Mailv.index.conv(u, id)
    case M.MailTagged{_, +id, _, _}: Mailv.index.conv(u, id)
    case _: u

# Only state that Plane draws changes this stamp. Mail-only arrivals
# cannot cause another walk of every thread to rebuild PW/@plane.
def Mailv.plane_event(c: M.Change) -> Bool:
  match c:
    case M.ConvSet{_, _, _, _, _, _, _}: False{}
    case M.MailPosted{_, _, _, _, _, _, _, _, _, _}: False{}
    case M.MailMarked{_, _, _, _, _, _}: False{}
    case M.MailTagged{_, _, _, _}: False{}
    case M.MessageFrom{_, _, _, _, _, _}: False{}
    case M.CharterSet{_, _, _, _}: False{}
    case M.DecisionLogged{_, _, _, _, _, _, _}: False{}
    case _: True{}

def Mailv.plane_seen(on: Bool, +u: Ui) -> Ui:
  match on:
    case False{}: u
    case True{}: Ui.scratch_set(u, "@mr.plane", Nat.show(Ui.seq(u)))

# Durable origins have their own event after MessagePosted. They must
# revise mail memos too, without invalidating the thread's timeline key.
def Mailv.origin(c: M.Change, +u: Ui) -> Ui:
  match c:
    case M.MessageFrom{_, _, _, _, _, _}: Mailv.put(u, Mailv.bump(Mailv.data(u)))
    case _: u

def Mailv.see.room.members(c: M.Change, +u: Ui) -> Ui:
  match c:
    case M.RoomSet{id, _, _, _}:
      +conv = "room:" ++ id
      +old = Mailv.index.old(Mailv.index(u), conv)
      Mailv.see.k(ML.Parts.has(ML.Conv.parts(old), "bot:" ++ Mailv.bot(u), False{}), u, conv)
    case _: u

def Mailv.see(+u: Ui, +c: M.Change) -> Ui:
  Mailv.plane_seen(Mailv.plane_event(c), Mailv.origin(c, Mailv.index.update(c, Mailv.see.body(c, Mailv.see.room.members(c, Mailv.see.mail(Bool.not(String.is_empty(Mailv.change_conv(c))), u, Mailv.change_conv(c)))))))

def Todos.see(+pre: M.State, +c: M.Change, u: Ui) -> Ui:
  match c u:
    case M.TodosSet{+th, _, _} Ui{+st, se, dr, li, op, on, no, ad, pa, v, ri, w, er, note, nf, ob, tr}:
      Ui{st, se, dr, li, op, on, no, ad, pa, v, ri, w, er, note, Todos.autoclose(pre, st, nf, th), ob, tr}
    case M.TodosChanged{+th, _, _, _} Ui{+st, se, dr, li, op, on, no, ad, pa, v, ri, w, er, note, nf, ob, tr}:
      Ui{st, se, dr, li, op, on, no, ad, pa, v, ri, w, er, note, Todos.autoclose(pre, st, nf, th), ob, tr}
    case M.TurnChanged{+th, _, _} Ui{+st, se, dr, li, op, on, no, ad, pa, v, ri, w, er, note, nf, ob, tr}:
      Ui{st, se, dr, li, op, on, no, ad, pa, v, ri, w, er, note, Todos.autoclose(pre, st, nf, th), ob, tr}
    case _ _: u

def Ui.fold(cs: List<&2, M.Change>, u: Ui) -> Ui:
  match cs u:
    case Nil{} _:
      u
    case +c <> t Ui{+st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.fold(t, Ui.revert.see(c, Todos.see(st, c, Mailv.see(Ui.see(Ui{M.State.apply(st, c), sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, c), c))))

def Ui.fold.d(d: Decoded, u: Ui) -> Ui:
  match d:
    case Decoded{cs, n}:
      Ui.skip(Ui.fold(List.reverse(&2, M.Change, cs), u), n)

# a message's items folded in, the unreadable ones counted
def Ui.fold.items(xs: List<&2, J.Json>, u: Ui) -> Ui:
  Ui.fold.d(Ui.decode.all(xs, Decoded{Nil{}, 0n}), u)

# the server's log on (re)connect: from nothing, or, when it says `since`
# (the client already has that many changes), just what was missed; the
# client's id survives the machine info being replaced
# Full replay replaces the mail read model; a true same-origin resume
# retains it. Terminal buffers follow their existing independent policy.
def MailvLocal.reset(keep: Bool, m: MailvLocal) -> MailvLocal:
  match keep m:
    case True{} _: m
    case False{} MailvLocal{terms, _}: MailvLocal{terms, Nil{}}

def Ui.reset(u: Ui, +o: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, _, now, ad, pa, _, rid, w, er, note, +nf, ob, tr}:
      Ui.fold.items(J.Json.elems(J.Json.get_obj(o, "items")),
        Ui{Ui.base(Nat.is_gt(J.Json.get_nat(o, "since", 0n), 0n), st), sel, Map.set(&2, String, Map.set(&2, String, dr, "@sync", "1"), "@fab.at", ""), live, op, True{}, now, ad, pa, J.Json.get_str(o, "version", ""), rid, w, er, note,
          Machines.home(Ui.reset.keep(String.eq(J.Json.get_str(o, "origin", ""), Str.get(nf, "origin")), nf,
            Local.keep(nf, Map.set(&2, String, Map.set(&2, String, Info.of(J.Json.get_obj(o, "info")), "cid", Str.get(nf, "cid")), "origin", J.Json.get_str(o, "origin", ""))))), ob, MailvLocal.reset(Bool.and(Nat.is_gt(J.Json.get_nat(o, "since", 0n), 0n), String.eq(J.Json.get_str(o, "origin", ""), Str.get(nf, "origin"))), tr)})

def Ui.adopt_log(u: Ui, st: M.State, o: J.Json) -> Ui:
  Ui.adopt(Ui.reset(u, o), st)

def Mailv.header.status(+box: ML.Box, +bot: String, +cv: ML.Conv) -> String:
  Bool.pick(String, String.eq(bot, "*"), Mailv.index.aggregate(ML.Conv.parts(cv), box, ML.Conv.id(cv), "done"), ML.Mark.status(ML.Box.mark_of(box, bot, ML.Conv.id(cv))))

def Mailv.header(+id: String, +box: ML.Box, +bot: String) -> MV.Row:
  +cv = ML.Box.conv(box, id)
  +mail = ML.Box.newest(box, id)
  MV.Mv.make(id, ML.Conv.subject(cv), MV.Mv.who(MV.Mv.labels(ML.Conv.parts(cv), "person", ML.Box.names(box))), ML.Mail.header(mail), ML.Conv.last(cv), String.eq(Mailv.header.status(box, bot, cv), "unread"), Mailv.header.status(box, bot, cv), ML.Conv.project(cv))

def Mailv.headers(ids: List<&2, String>, +box: ML.Box, +bot: String) -> List<&2, MV.Row>:
  match ids:
    case Nil{}: Nil{}
    case id <> t: Mailv.header(id, box, bot) <> Mailv.headers(t, box, bot)

def Mailv.list.ordinary.row(missing: Bool, +r: MV.Row, +box: ML.Box, +bot: String) -> MV.Row:
  match missing:
    case True{}: r
    case False{}: Mailv.header(MV.Row.id(r), box, bot)

def Mailv.list.ordinary.rows(rs: List<&2, MV.Row>, +box: ML.Box, +bot: String) -> List<&2, MV.Row>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t:
      Mailv.list.ordinary.row(String.is_empty(ML.Conv.id(ML.Box.conv(box, MV.Row.id(r)))), r, box, bot) <> Mailv.list.ordinary.rows(t, box, bot)

def Mailv.list.ordinary(+u: Ui, +p: J.Json, +res: J.Json, +f: MV.Filter) -> MV.Cache:
  MV.Mv.put_page(Mailv.cache(u), f, J.Json.get_nat(p, "skip", 0n), J.Json.get_nat(res, "total", 0n), Mailv.list.ordinary.rows(MV.Mv.parse(res, "person", ML.Box.names(M.State.mail(Ui.state(u)))), M.State.mail(Ui.state(u)), MV.Filter.bot(f)))

def Mailv.index.kind(+status: String) -> String:
  Bool.pick(String, Nat.is_lt(C.Codec.pos(["act", "fyi", "unread", "done", "snoozed"], status), 5n), status, "")

def Mailv.list.result.if(local: Bool, +u: Ui, +p: J.Json, +res: J.Json, +f: MV.Filter) -> MV.Cache:
  match local:
    case False{}: Mailv.list.ordinary(u, p, res, f)
    case True{}:
      +order = Mailv.index.order(Mailv.index(u), Mailv.index.key(MV.Filter.bot(f), Mailv.index.kind(MV.Filter.status(f)), MV.Filter.project(f)))
      MV.Mv.put_page(Mailv.cache(u), f, J.Json.get_nat(p, "skip", 0n), ML.Order.count(order), Mailv.headers(ML.Order.slice(order, J.Json.get_nat(p, "skip", 0n), 100n), M.State.mail(Ui.state(u)), MV.Filter.bot(f)))

def Mailv.list.result(+u: Ui, +p: J.Json, +res: J.Json) -> MV.Cache:
  +f = {MV.Filter{J.Json.get_str(p, "bot", "*"), J.Json.get_str(p, "view_status", J.Json.get_str(p, "status", "")), J.Json.get_str(p, "project", ""), ""} : MV.Filter}
  Mailv.list.result.if(Bool.or(Bool.or(String.eq(MV.Filter.status(f), "act"), String.eq(MV.Filter.status(f), "fyi")), String.eq(MV.Filter.bot(f), "*")), u, p, res, f)

def Mailv.local.previous(same: Bool, +u: Ui) -> MV.Find:
  match same:
    case True{}: Mailv.find(u)
    case False{}: MV.Mv.found()

def Mailv.search.local(+u: Ui) -> Ui:
  Mailv.put(Ui.scratch_set(u, "@mr.local.key", Mailv.key(u)), Mailv.search.set(Mailv.data(u), MV.Mv.find_next(Mailv.local.previous(String.eq(Ui.scratch(u, "@mr.local.key"), Mailv.key(u)), u), Mailv.cache(u), Mailv.key(u), Ui.scratch(u, "@f.mrsearch"), 100n)))

def Mailv.local.if(need: Bool, +u: Ui) -> Ui:
  match need:
    case True{}: Mailv.search.local(Ui.info_set(u, "pane:mr", "0"))
    case False{}: u

def Mailv.reply.apply(i: Nat, +u: Ui, +p: J.Json, +tag: String, +res: J.Json) -> Ui:
  match i:
    case 0n:
      Ui.scratch_set(Mailv.search.local(Mailv.put(u, Mailv.list.put(Mailv.data(u), Mailv.list.result(u, p, res)))), "@mr.loaded", tag)
    case 1n:
      Mailv.put(u, Mailv.read.put(Mailv.data(u), MV.Mv.put_read(Mailv.cache(u), res, Bool.not(String.is_empty(J.Json.get_str(p, "before", "")))), Ui.scratch(u, "@mr.conv"), ML.Box.names(M.State.mail(Ui.state(u))), Mailv.live_targets(Map.values(&2, ML.Run, ML.Box.runs(M.State.mail(Ui.state(u)))), Ui.scratch(u, "@mr.conv"))))
    case 2n:
      Mailv.reply.find(Bool.and(String.eq(J.Json.get_str(p, "q", ""), Ui.scratch(u, "@f.mrsearch")), String.eq(tag, Mailv.key(u))), u, J.Json.get_str(p, "q", ""), res)
    case _: u

def Mailv.reply.got(ok: Bool, +u: Ui, +req: J.Json, +o: J.Json) -> Ui:
  match ok:
    case True{}:
      Mailv.reply.apply(C.Codec.pos(["mail.list", "mail.read", "mail.find"], J.Json.get_str(req, "m", "")), u, J.Json.get_obj(req, "p"), J.Json.get_str(req, "tag", ""), J.Json.get_obj(o, "result"))
    case False{}:
      Ui.scratch_set(u, "@mr.loaded", Mailv.token(u))

def Mailv.reply.clear(same: Bool, u: Ui, +slot: String) -> Ui:
  match same:
    case True{}: Ui.scratch_set(u, "@mr." ++ slot, "")
    case False{}: u

def Mailv.reply.on(has: Bool, +u: Ui, +o: J.Json, +id: String) -> Ui:
  match has:
    case False{}: u
    case True{}:
      +req = Maybe.default(&2, J.Json, J.Json.parse(Ui.scratch(u, "@mr.req:" ++ id)), J.Null{})
      Mailv.reply.got(J.Json.get_bool(o, "ok", True{}), Ui.scratch_set(Mailv.reply.clear(String.eq(Ui.scratch(u, "@mr." ++ J.Json.get_str(req, "slot", "")), id), u, J.Json.get_str(req, "slot", "")), "@mr.req:" ++ id, ""), req, o)

def Mailv.reply(+u: Ui, +o: J.Json) -> Ui:
  Mailv.reply.on(Bool.not(String.is_empty(Ui.scratch(u, "@mr.req:" ++ J.Json.as_raw(J.Json.get(o, "id"), "")))), u, o, J.Json.as_raw(J.Json.get(o, "id"), ""))

def Ui.reply(u: Ui, +o: J.Json) -> Ui:
  KX.reply(KN.reply(Made.reply(o, Bots.reply(o, Proj.reply(o, Btw.reply(Fab.reply(Fab.has(o), Ui.reply.skills(Mailv.reply(u, o), o), o), o)))), o), o)

def Ui.recv.k(i: Nat, u: Ui, +o: J.Json) -> Ui:
  match i:
    case 0n:
      Ui.reset(u, o)
    case 1n:
      Ui.fold.items(J.Json.elems(J.Json.get_obj(o, "items")), u)
    case 2n:
      Ui.delta(u, J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "text", ""))
    case 3n:
      Ui.reply(u, o)
    case 4n:
      Ui.update(u, o)
    case 5n:
      Ui.view(u, o)
    case 6n:
      Emb.recv(J.Json.elems(J.Json.get_obj(o, "reset")), u, J.Json.get_str(o, "thread", ""), Term.bytes(J.Json.elems(J.Json.get_obj(o, "b"))))
    case 7n:
      Ui.info_merge(u, J.Json.get_obj(o, "info"))
    case 8n:
      Voice.recv(u, o)
    case 9n:
      Ui.far(u, o)
    case 10n:
      Ui.live.set(u, o)
    case _:
      u

# a message folded in, and the join's progress noted
def Ui.recv.b(+i: Nat, u: Ui, +o: J.Json) -> Ui:
  Boot.mark(i, Ui.recv.k(i, u, o), o)

# A message can resolve a want only against the installed destination.
def Ui.recv(+u: Ui, +o: J.Json) -> Ui:
  Ui.reselect(Ui.sel(u), Bots.wanted(Ui.join(String.eq(J.Json.get_str(o, "t", ""), "log"), Ui.recv.b(C.Codec.pos(["log", "changes", "delta", "reply", "update", "view", "term", "info", "voice", "far", "live"], J.Json.get_str(o, "t", "")), Mailv.join(String.eq(J.Json.get_str(o, "t", ""), "log"), u), o))))

# a message from the server, with what to send: a (re)connection (the
# server's log arrives first on each) sends the outbox again, so nothing
# the user sent is lost to a dropped link
def Ui.recv_act.go(u: Ui, +o: J.Json) -> Act:
  Ui.recv_act.t(Bool.and(String.eq(J.Json.get_str(o, "t", ""), "term"), Bool.not(EM.Emb.is_key(J.Json.get_str(o, "thread", "")))), J.Json.get_str(o, "thread", ""), Fm.again(String.eq(J.Json.get_str(o, "t", ""), "log"), Ui.recv_act.k(String.eq(J.Json.get_str(o, "t", ""), "log"), Ui.recv(u, o))))

# a desktop notification ({"t":"notify"}, sent only to a desk)
def Ui.recv_act.n(notify: Bool, u: Ui, +o: J.Json) -> Act:
  match notify:
    case True{}:
      Act{u, [Notify{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "title", ""), J.Json.get_str(o, "body", ""), J.Json.get_str(o, "key", "")}]}
    case False{}:
      Emb.sync.if(C.Codec.pos(["log", "changes", "view"], J.Json.get_str(o, "t", "")), Fab.sync(Dv.sync(Mpage.sync(Ui.recv_act.go(u, o)))))

# a frame of a page ({"t":"shot","bot":page,"n":n}) where the client keeps
# it (the phones): its number as info "shot.<page>", seen when on show
def Ui.recv_act.s(shot: Bool, u: Ui, +o: J.Json) -> Act:
  match shot:
    case True{}:
      Ui.cast.seen(Ui.info_set(u, "shot." ++ J.Json.get_str(o, "bot", ""), J.Json.as_raw(J.Json.get(o, "n"), "0")), J.Json.get_str(o, "bot", ""))
    case False{}:
      Ui.recv_act.n(String.eq(J.Json.get_str(o, "t", ""), "notify"), u, o)

def Mailv.search.one(+u: Ui, +bot: String) -> Act:
  Mailv.request(u, "mail.find", [J.KV{"bot", J.Str{bot}}, J.KV{"q", J.Str{Ui.scratch(u, "@f.mrsearch")}}, J.KV{"n", J.Json.nat(100n)}], "find", Mailv.key(u))

def Mailv.search.bots(bs: List<&2, BT.Bot>, a: Act) -> Act:
  match bs a:
    case Nil{} _: a
    case +b <> t Act{u, cs}:
      Mailv.search.bots(t, Ui.first(cs, Mailv.search.one(u, BT.Bot.id(b))))

def Mailv.search.route(all: Bool, +u: Ui) -> Act:
  match all:
    case False{}: Mailv.search.one(u, Mailv.bot(u))
    case True{}: Mailv.search.bots(BT.World.bots.here(M.State.bots(Ui.state(u))), Act{u, Nil{}})

def Mailv.search(+u: Ui) -> Act:
  Mailv.search.route(String.eq(Mailv.bot(u), "*"), Ui.scratch_set(Ui.scratch_set(Mailv.search.local(u), "@mr.qkey", Mailv.key(u)), "@mr.q", Ui.scratch(u, "@f.mrsearch")))

def Mailv.sync.search(need: Bool, +u: Ui, cs: List<&2, Cmd>) -> Act:
  match need:
    case True{}: Ui.first(cs, Mailv.search(u))
    case False{}: Act{u, cs}

def Mailv.sync.read(need: Bool, +u: Ui, cs: List<&2, Cmd>) -> Act:
  match need:
    case True{}: Ui.first(cs, Mailv.read(u, ""))
    case False{}:
      Mailv.sync.search(Bool.and(String.is_empty(Ui.scratch(u, "@mr.find")), Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@f.mrsearch"))), Bool.or(Bool.not(String.eq(Ui.scratch(u, "@mr.q"), Ui.scratch(u, "@f.mrsearch"))), Bool.not(String.eq(Ui.scratch(u, "@mr.qkey"), Mailv.key(u)))))), u, cs)

# Bind presentation immediately to the selected conversation's cache.
# A failed/absent read must not leave another room's body or recipients up.
def Mailv.open.bind(+u: Ui, +conv: String) -> Ui:
  +selected = Ui.scratch_set(Ui.scratch_set(u, "@mr.conv", conv), "@mr.to", "")
  Mailv.put(selected, Mailv.read.put(Mailv.data(selected), Mailv.cache(selected), conv, ML.Box.names(M.State.mail(Ui.state(selected))), Mailv.live_targets(Map.values(&2, ML.Run, ML.Box.runs(M.State.mail(Ui.state(selected)))), conv)))

def Mailv.action(i: Nat, +u: Ui, +v: String) -> Act:
  match i:
    case 0n: Act{Mailv.filter.set(u, v), Nil{}}
    case 1n: Mailv.read(Mailv.open.bind(u, v), "")
    case 2n: Mailv.read(u, Mailv.before(u))
    case 3n:
      Mailv.send.ok(Bool.and(Bool.not(String.is_empty(String.trim(Ui.scratch(u, "@f.mrreply")))), Bool.not(String.is_empty(Ui.scratch(u, "@mr.to")))), u, Ui.scratch(u, "@mr.to"), Ui.scratch(u, "@f.mrreply"))
    case 4n:
      Ui.rpc.keep(u, "mail.mark", [J.KV{"bot", J.Str{Mailv.bot(u)}}, J.KV{"conv", J.Str{Ui.scratch(u, "@mr.conv")}}, J.KV{"status", J.Str{v}}])
    case 5n: Mailv.search(Ui.info_set(Ui.scratch_set(u, "@f.mrsearch", v), "pane:mr", "0"))
    case 6n: Act{Ui.scratch_set(u, "@mr.run", Bool.pick(String, Mailv.run_open(u), "", "1")), Nil{}}
    case 7n: Act{Ui.scratch_set(u, "@mr.to", v), Nil{}}
    case 8n: Act{Ui.scratch_set(u, "@mr.conv", ""), Nil{}}
    case _: Act{u, Nil{}}

def Mailv.sync.page(need: Bool, +u: Ui, cs: List<&2, Cmd>) -> Act:
  match need:
    case True{}: Ui.first(cs, Mailv.list(u))
    case False{}:
      Mailv.sync.read(Bool.and(String.is_empty(Ui.scratch(u, "@mr.read")), Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@mr.conv"))), String.eq(Ui.scratch(u, "@mr.read.dirty"), "1"))), u, cs)

def Mailv.sync.on(on: Bool, +u: Ui, cs: List<&2, Cmd>) -> Act:
  match on:
    case False{}: Act{u, cs}
    case True{}:
      Mailv.sync.page(Bool.and(String.is_empty(Ui.scratch(u, "@mr.list")), Bool.not(String.eq(Ui.scratch(u, "@mr.loaded"), Mailv.token(u)))), u, cs)

def Mailv.sync(a: Act) -> Act:
  match a:
    case Act{+u, cs}: Mailv.sync.on(Mailv.shown(u), Mailv.local.if(Bool.and(Mailv.shown(u), Bool.or(Bool.not(String.eq(MV.Find.q(Mailv.find(u)), Ui.scratch(u, "@f.mrsearch"))), Bool.not(String.eq(Ui.scratch(u, "@mr.local.key"), Mailv.key(u))))), u), cs)

# a desk's host calls this after its input, focus changes and messages
def Ui.desk(+u: Ui, +focused: Bool, +now: Nat) -> Act:
  Mailv.sync(Ui.desk.k(DK.Desk.due(Ui.scratch(u, "@desk"), Ui.scratch(u, "@desk.at"), Ui.sel(u), focused, now), u, Ui.sel(u), focused, now))

def Browse.field.change(edit: Bool, +u: Ui, +value: String) -> Ui:
  match edit:
    case True{}: Ui.scratch_set(u, "@browse.edit", Maybe.default(&2, String, List.head(&2, String, String.split(value, '\u{1f}')), ""))
    case False{}: u

# "settings-page" opens a page; its value "?" (Enter in the search) opens
# the first page the search finds, or stays put when it finds none
def Settings.nav.first(ps: List<&2, String>, +u: Ui) -> Act:
  match ps:
    case Nil{}: Act{u, Nil{}}
    case +p <> _: Settings.go(u, p)

def Settings.nav(+u: Ui, +value: String) -> Act:
  Bool.pick(Act, String.eq(value, "?"), Settings.nav.first(Settings.found(Settings.menu.pages(), Settings.loc(u), Settings.q(u)), u), Settings.go(u, value))

def Active.root_threads(rs: List<&2, ARow>) -> List<&2, M.Thread>:
  match rs:
    case Nil{}: Nil{}
    case ARow{_, _, t, _, _, _} <> rest: t <> Active.root_threads(rest)

def Side.legacy.ids(ts: List<&2, M.Thread>, +q: String) -> List<&2, String>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      List.append(&2, String, Side.when(Palette.hit(q, Active.title(t)), [M.Thread.id(t)]), Side.legacy.ids(rest, q))

# One list for rendered rows and keyboard destinations, with no duplicate
# legacy row when the remaining history is expanded or searched.
def Side.legacy.filtered(ts: List<&2, M.Thread>, +q: String) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest:
      ST.Tree.roots.add(Bool.not(Palette.hit(q, Active.title(t))), t, Side.legacy.filtered(rest, q))

def Side.legacy.shown(+u: Ui) -> List<&2, String>:
  List.append(&2, String, Side.legacy.ids(ST.Legacy.live(Ui.state(u), Ui.sel(u)), Ui.scratch(u, "@f.pq")),
    Side.when(Bool.or(Ui.flag(u, "conversation-history"), Bool.not(String.is_empty(Ui.scratch(u, "@f.pq")))),
      Side.legacy.ids(ST.Legacy.history(Ui.state(u), Ui.sel(u)), Ui.scratch(u, "@f.pq"))))

def Side.visible.kind(active: Bool, +u: Ui) -> List<&2, String>:
  match active:
    case True{}:
      +q = Ui.scratch(u, "@f.pq")
      ST.Tree.row_ids(ST.Tree.rows_search(Ui.state(u), Active.root_threads(Active.folded(Active.open(Side.open(u), q), Active.parts(Active.rows(Ui.state(u), Side.now(u), q)))), Ui.sel(u), q))
    case False{}: Side.visible.projects(u)

def Side.visible(+u: Ui) -> List<&2, String>:
  List.append(&2, String, Side.visible.kind(String.eq(Side.view(Ui.state(u)), "active"), u),
    Side.legacy.shown(u))

# Navigation counts exactly the current project's visible rows, in the
# same order as the selected sidebar mode and its expanded branches.
def Side.in_project(xs: List<&2, String>, +st: M.State, +pid: String) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +id <> rest:
      List.append(&2, String, Side.when(String.eq(ST.Tree.project(Ui.thread(st, id)), pid), [id]), Side.in_project(rest, st, pid))

# the threads the sidebar shows in project pid, top to bottom
def Side.of(+u: Ui, +pid: String) -> List<&2, String>:
  Side.in_project(Side.visible(u), Ui.state(u), pid)

# the threads ctrl+1..9 count in
def Side.here(+u: Ui) -> List<&2, String>:
  Side.of(u, Side.cur(u))

# Modifier hints reuse the frame's existing index. Only the selected
# project's forest can contribute Ctrl digits: parent links cannot cross
# projects, so unrelated forests need not be expanded and discarded.
def Side.projects.ix(u: Ui, +ix: SIx) -> List<&2, String>:
  match u:
    case Ui{+st, +sel, +dr, _, +op, _, +now, _, _, _, _, _, _, _, _, _, _}:
      Side.pids(Side.unfolded.at(Find.rows(Side.rows.ix.at(st, ix, now, Side.cur.held(st, sel, now)), Find.q(op, dr)), op, Find.q(op, dr), now, Side.cur.held(st, sel, now)))

def Side.active.project(rs: List<&2, ARow>, +pid: String) -> List<&2, ARow>:
  match rs:
    case Nil{}: Nil{}
    case ARow{rank, at, t, +project, title, faded} <> rest:
      Active.keep(String.eq(project, pid), ARow{rank, at, t, project, title, faded}, Side.active.project(rest, pid))

def Side.here.kind(active: Bool, +u: Ui, +ix: SIx, +pid: String) -> List<&2, String>:
  match active:
    case True{}:
      +q = Ui.scratch(u, "@f.pq")
      ST.Tree.row_ids(ST.Tree.rows_search(Ui.state(u), Active.root_threads(Side.active.project(
        Active.folded(Active.open(Side.open(u), q), Active.parts(Active.rows.ix(Ui.state(u), ix, Side.now(u), q))), pid)), Ui.sel(u), q))
    case False{}:
      match u:
        case Ui{+st, +sel, +dr, _, +op, _, +now, _, _, _, _, _, _, _, _, _, _}:
          Side.visible.tree(Side.only(Side.unfolded(Side.shown.ix(st, ix, now, Find.q(op, dr)), op, Find.q(op, dr), now), pid), op, st, sel)

def Side.here.ix(+u: Ui, +ix: SIx) -> List<&2, String>:
  List.append(&2, String, Side.here.kind(String.eq(Side.view(Ui.state(u)), "active"), u, ix, Side.cur(u)),
    Side.in_project(Side.legacy.shown(u), Ui.state(u), Side.cur(u)))

def Uber.chat.k(none: Bool, +u: Ui, id: String) -> Act:
  match none:
    case True{}: Uber.new(u)
    case False{}: Bots.select(Ui.scratch_set(u, "@btab", "chat"), id)

# A blank/default casual conversation goes to the existing uberbot Chat.
# No project, thread, promotion or data migration is invented here.
def Uber.chat(+u: Ui) -> Act:
  Uber.chat.k(String.is_empty(UB.Uber.bot(Ui.state.of(u))), u, UB.Uber.bot(Ui.state.of(u)))

def Ui.new_thread.kind(casual: Bool, u: Ui, pid: String) -> Act:
  match casual:
    case True{}: Uber.chat(u)
    case False{}: Ui.new_thread.project(u, pid)

def Ui.new_thread(+u: Ui, +pid: String) -> Act:
  Ui.new_thread.kind(Bool.or(String.is_empty(pid), M.Scratch.is(FR.Fr.local(pid))), u, pid)

# Enter in the search goes to the first project shown, or to project v
# when it names one (a phone names it: its list spans hubs), to its top
# thread (or a new thread when it has none), and shuts the search
def Find.go.k(none: Bool, +u: Ui, +pid: String, +home: String) -> Act:
  match none:
    case True{}:
      Ui.new_thread(u, pid)
    case False{}:
      Ui.select(u, home)

def Find.go.p(+u: Ui, +pid: String) -> Act:
  Bool.pick(Act, String.is_empty(pid), Act{u, Nil{}}, Find.go.k(String.is_empty(Side.home(u, pid)), u, pid, Side.home(u, pid)))

def Find.go.projects(+u: Ui, +v: String) -> Act:
  Find.go.p(Ui.scratch_set(Ui.flag_off(u, "psearch"), "@f.pq", ""), Bool.pick(String, String.is_empty(v), Side.first(Side.projects(u)), v))

def Find.go.k2(active: Bool, +u: Ui, +v: String) -> Act:
  match active:
    case True{}:
      Find.go.active(u)
    case False{}:
      Find.go.projects(u, v)

def Find.go.named(+u: Ui, +v: String) -> Act:
  Find.go.k2(Bool.and(String.is_empty(v), String.eq(Side.view(Side.st(u)), "active")), u, v)

# (a phone names the project it means: v; the web's field sends what was
# typed, which names none: Enter goes to the first shown, never to a
# project of that name, which made a thread in no project)
def Find.go(+u: Ui, +v: String) -> Act:
  Find.go.named(u, Bool.pick(String, FR.Fr.st.has.project(Side.st(u), v), v, ""))

def Proj.go.k(none: Bool, u: Ui, +pid: String, +tid: String) -> Act:
  match none:
    case True{}:
      Ui.new_thread(u, pid)
    case False{}:
      Ui.select(u, tid)

def Proj.go.t(u: Ui, +pid: String, b: PBest) -> Act:
  match b:
    case PBest{+tid, _}:
      Proj.go.k(String.is_empty(tid), u, pid, tid)

def Proj.go.p(none: Bool, +u: Ui, +pid: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Proj.go.t(Ui.flag_off(u, "proj"), pid, Proj.latest(Ui.threads(Ui.state(u)), pid, PBest{"", 0n}))

# going to a project opens its latest thread, or starts one
def ThreadTree.route.pick(none: Bool, +u: Ui, thread: String) -> Act:
  match none:
    case True{}: Act{Ui.info_set(u, "proj.err", "This project has no conversation to manage a subthread. Choose another project, or cancel and create a project conversation first."), Nil{}}
    case False{}: Ui.select(Ui.scratch_set(Ui.flag_off(u, "proj"), "@thread.route", ""), thread)

def Proj.go.route(route: Bool, +u: Ui, +root: String) -> Act:
  match route:
    case True{}:
      +thread = RefNav.latest(Ui.threads(Ui.state(u)), Proj.id_of(Proj.ps(u), root), M.Thread.blank(""))
      ThreadTree.route.pick(String.is_empty(thread), u, thread)
    case False{}: Proj.go.p(String.is_empty(Proj.id_of(Proj.ps(u), root)), u, Proj.id_of(Proj.ps(u), root))

def Proj.go(+u: Ui, +root: String) -> Act:
  Proj.go.route(ThreadTree.routing(u), u, root)

# add (closes the picker), browse, create, go to a project, add one found,
# browse folders, back to the projects
def Proj.run.k(i: Nat, +u: Ui, +v: String) -> Act:
  match i:
    case 0n:
      Proj.adding(u, "project.add", v)
    case 1n:
      Proj.rpc(Proj.mode(u, "dir"), "fs.list", [J.KV{"path", J.Str{v}}])
    case 2n:
      Proj.rpc(Proj.reset(u, ""), "fs.mkdir", [J.KV{"path", J.Str{v}}])
    case 3n:
      Proj.adding(u, "project.new", v)
    case 4n:
      Proj.go(u, v)
    case 5n:
      Proj.adding(u, "project.add", v)
    case 6n:
      Proj.browse(u)
    case 7n:
      Act{Proj.mode(u, ""), Nil{}}
    case 8n:
      Proj.machine(u, v)
    case _:
      Act{u, Nil{}}

def Proj.run(it: PItem, u: Ui) -> Act:
  match it:
    case PItem{_, +a, +v}:
      Proj.run.k(Proj.pos(Proj.acts(), a, 0n), u, v)

# Enter runs the selected row
def Proj.enter(+l: I.Loc, +u: Ui) -> Act:
  Proj.run(Palette.at(Proj.items.loc(l, u), Proj.index(u)), u)

def Proj.enter.u(+u: Ui) -> Act:
  Proj.enter(Lang.loc(u), u)

# past the table's own: the project picker's (14-17), then the Mechanical
# page's (20-28; client.bend's Mpage.act), then a phone's upload (29, 30),
# then the viewer's history (31-46; Hs.act), then the API keys' (47, 48)
def Voice.act.tail.k(picker: Bool, +i: Nat, +u: Ui, +value: String) -> Act:
  match picker:
    case True{}:
      Proj.run.k(Nat.add(Nat.sub(i, 14n), 4n), u, value)
    case False{}:
      Upload.act(i, u, value)

def Voice.act.tail(+i: Nat, +u: Ui, +value: String) -> Act:
  Voice.act.tail.k(Nat.is_lt(i, 19n), i, u, value)

def Voice.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Voice.toggle(Voice.on(u), u)
    case 1n:
      Voice.rpc(Ui.scratch_set(u, "@f.vkey", ""), "key", [J.KV{"key", J.Str{Ui.scratch(u, "@f.vkey")}}])
    case 2n:
      Voice.rpc(Sealed.forget(u, "vkey"), "unkey", Nil{})
    case 3n:
      Voice.rpc(u, "dict", [J.KV{"dict", J.Str{Ui.scratch(u, "@f.vdict")}}])
    case 4n:
      Voice.use(u, Voice.at(Voice.unsure(u), value), value)
    case 5n:
      Voice.add(u, Voice.at(Voice.unsure(u), value), value)
    case 6n:
      Voice.clarify(u, Voice.at(Voice.unsure(u), value), value)
    case 7n:
      Voice.fixed(u)
    case 8n:
      Voice.skip(u, Voice.at(Voice.unsure(u), value), value)
    case 9n:
      Act{Ui.scratch_set(Ui.scratch_set(u, "@voice.err", ""), "@voice.hint", ""), Nil{}}
    case 10n:
      Voice.rpc(Ui.scratch_set(u, "@f.vterm", ""), "add", [J.KV{"term", J.Str{String.trim(Ui.scratch(u, "@f.vterm"))}}])
    case 11n:
      Voice.rpc(u, "dict", [J.KV{"dict", J.Str{VC.Voice.dict.show(VC.Voice.dict.remove(Voice.dict(u), value))}}])
    # the table's last action is not voice's: a side question closed
    case 12n:
      Btw.close(u)
    # the phones' viewer ground (the window takes its own "vw-light")
    case 13n:
      Act{Ui.scratch_set(u, "@vw.light", Bool.pick(String, String.is_empty(Ui.scratch(u, "@vw.light")), "1", "")), Nil{}}
    # a link or form in a bot's page
    case 18n:
      Bots.space.link(u, value)
    # the arrow keys in the composer's menu
    case 19n:
      Skills.move(u, value)
    # and the project picker's own: go to a project, add one found, browse
    # folders, back to the projects
    case _:
      Voice.act.tail(i, u, value)

# The web's inline routine confirmation, as for a room's Delete.
def Bots.routine.armed(+u: Ui, +id: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(id)), String.eq(Bots.field(u, "delroutine"), id))

def Bots.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Bots.select(u, value)
    case 1n:
      Bots.tab.k(String.eq(value, "settings"), Ui.scratch_set(Ui.scratch_set(u, "@f.delroutine", ""), "@btab", value))
    case 2n:
      Act{Bots.flag(Ui.scratch_set(u, "@uber.new", ""), "@bnew"), [Focus{"bot-name"}]}
    case 3n:
      Bots.create(u)
    case 4n:
      Ui.rpc(Bots.views(u, "", "", ""), "bots.delete", [J.KV{"bot", J.Str{value}}])
    case 5n:
      Bots.edit(u)
    case 6n:
      Act{Bots.bfield(u, value), Nil{}}
    case 7n:
      Bots.room.select(u, value)
    case 8n:
      Act{Bots.flag(Bots.fields.clear(u, ["rname", "rmembers", "rid"]), "@rnew"), [Focus{"room-name"}]}
    case 9n:
      Bots.room.save(u)
    case 10n:
      Act{Bots.pick(u, value), Nil{}}
    case 11n:
      Bots.room.post(u)
    case 12n:
      Bots.space(u, value)
    case 13n:
      Bots.space(u, value)
    case 14n:
      Ui.rpc(u, "bots.forget", [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"key", J.Str{value}}])
    case 15n:
      Bots.routine.save(u)
    case 16n:
      Bots.routine.toggle(u, Bots.routine.of(u, value))
    case 17n:
      Ui.rpc(Ui.scratch_set(u, "@f.delroutine", ""), "bots.routine_delete", [J.KV{"routine", J.Str{value}}])
    case 18n:
      Ui.rpc(u, "bots.routine_run", [J.KV{"routine", J.Str{value}}])
    case 19n:
      Bots.hook.new(u)
    case 20n:
      Ui.rpc(u, "bots.hook_revoke", [J.KV{"hook", J.Str{value}}])
    case 21n:
      Ui.rpc(Ui.scratch_set(u, "@peer.invite", ""), "bots.invite", [J.KV{"url", J.Str{Bots.field(u, "purl")}}])
    case 22n:
      Ui.rpc(Bots.fields.clear(u, ["invite"]), "bots.join", [J.KV{"invite", J.Str{String.trim(Bots.field(u, "invite"))}}])
    case 23n:
      Ui.rpc(u, "bots.unpeer", [J.KV{"peer", J.Str{value}}])
    case 24n:
      Bots.google(u, value)
    case 25n:
      Bots.remote.select(u, value)
    case 26n:
      Bots.remote.send(u)
    case 27n:
      Bots.look(u, value)
    case 28n:
      Ui.rpc(Bots.views(u, "", "", ""), "bots.room_delete", [J.KV{"room", J.Str{value}}])
    case 29n:
      Act{Bots.routine.edit(u, Bots.routine.of(u, value)), [Focus{"routine-name"}]}
    case 30n:
      Act{Bots.token.clear(Ui.scratch_set(Ui.scratch_set(u, "@hook.secret", ""), "@hook.id", "")), Nil{}}
    case 31n:
      Act{Ui.scratch_set(u, value, ""), Nil{}}
    case 32n:
      Act{Page.earlier(u, value), Nil{}}
    case 33n:
      Bots.token(u, value, "show")
    case 34n:
      Bots.token(u, value, "new")
    case 35n:
      Bots.token(u, value, "off")
    case _:
      Voice.act(Nat.sub(i, 36n), u, value)

# Shared destination helper: history traversal calls this directly,
# never recursively dispatching Ui.act.
def RefNav.visit(+u: Ui, +r: R.Ref) -> Act:
  match r:
    case R.Ref{"thread", +id <> +tail, +frag}: RefNav.jump(Ui.select(u, id), frag)
    case R.Ref{"bot", +id <> +tail, _}: Bots.select(u, id)
    case R.Ref{"room", +id <> +tail, _}: Bots.room.select(u, id)
    case R.Ref{"project", +id <> +tail, _}: Find.go(u, id)
    case R.Ref{"machine", +id <> +tail, _}: RefNav.machine.visit(u, id)
    case R.Ref{"routine", +id <> +tail, _}: RefNav.routine(Bots.select(u, BT.Routine.bot(BT.World.routine(M.State.bots(Side.st(u)), id))))
    case R.Ref{"file", +id <> +path, _}: RefNav.file(u, id, path)
    case R.Ref{"work", +id <> +tail, _}: Ui.select(u, RE.Card.work.thread(id))
    case R.Ref{"task", +id <> +tail, _}: RefNav.task(u, id)
    case R.Ref{"pr", +repo <> +n <> +tail, _}: RefNav.pr(String.contains(repo, "/"), u, repo, n)
    case R.Ref{"mailroom", +id <> +tail, _}: Bots.select(u, id)
    case _: Act{u, Nil{}}

def RefNav.go.resolved(+m: Maybe<&2, RE.Card>, +u: Ui, +r: R.Ref) -> Act:
  match m:
    case None{}: Act{u, Nil{}}
    case Some{_}: RefNav.keep(RefNav.visit(u, r), RE.Trail.go(RefNav.seed(u), R.Ref.uri(r)))

def RefNav.go.parsed(+m: Maybe<&2, R.Ref>, +u: Ui) -> Act:
  match m:
    case None{}: Act{u, Nil{}}
    case Some{+r}: RefNav.go.resolved(RE.Card.of(Side.st(u), r), u, r)

def RefNav.go(+u: Ui, +uri: String) -> Act:
  RefNav.go.parsed(R.Ref.parse(uri), u)

def RefNav.move.resolved(m: Maybe<&2, RE.Card>, u: Ui, r: R.Ref, t: RE.Trail) -> Act:
  match m:
    case None{}: Act{u, Nil{}}
    case Some{_}: RefNav.keep(RefNav.visit(u, r), t)

def RefNav.move.parsed(+m: Maybe<&2, R.Ref>, +u: Ui, +t: RE.Trail) -> Act:
  match m:
    case None{}: Act{u, Nil{}}
    case Some{+r}: RefNav.move.resolved(RE.Card.of(Side.st(u), r), u, r, t)

def RefNav.move(+u: Ui, +t: RE.Trail) -> Act:
  RefNav.move.parsed(R.Ref.parse(RE.Trail.cur(t)), u, t)

def RefNav.step(ok: Bool, u: Ui, t: RE.Trail) -> Act:
  match ok:
    case True{}: RefNav.move(u, t)
    case False{}: Act{u, Nil{}}

def RefNav.back.from(u: Ui, +t: RE.Trail) -> Act:
  RefNav.step(RE.Trail.can_back(t), u, RE.Trail.back(t))

def RefNav.back(+u: Ui) -> Act:
  RefNav.back.from(u, Trail.read(u))

def RefNav.fwd.from(u: Ui, +t: RE.Trail) -> Act:
  RefNav.step(RE.Trail.can_fwd(t), u, RE.Trail.fwd(t))

def RefNav.fwd(+u: Ui) -> Act:
  RefNav.fwd.from(u, Trail.read(u))

# the link actions (178 on: ref-go, ref-back, ref-fwd, ref-copy,
# ref-copy-web), reached by a native compare, never by peeling 178 cases
def RefNav.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n: RefNav.go(u, value)
    case 1n: RefNav.back(u)
    case 2n: RefNav.fwd(u)
    case 3n: Act{u, [Copy{value}]}
    case _: RefNav.copy.web(R.Ref.parse(value), u)

def Ui.act.ref(kn: Bool, +i: Nat, u: Ui, +value: String) -> Act:
  match kn:
    case True{}: KN.act(Nat.sub(i, 183n), u)
    case False{}: RefNav.act(Nat.sub(i, 178n), u, value)

def Ui.act.far(far: Bool, +i: Nat, u: Ui, +value: String) -> Act:
  match far:
    case True{}: Ui.act.ref(Nat.is_ge(i, 183n), i, u, value)
    case False{}: Bots.act(Nat.sub(i, 77n), u, value)

def Ui.act.k(+i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Ui.select(u, value)
    case 1n:
      Ui.new_thread(u, value)
    case 2n:
      Ui.draft(u, value)
    case 3n:
      Ui.send(u, False{})
    case 4n:
      Proj.adding(Ui.add_project(u), "project.add", value)
    case 5n:
      Ui.toggle_add(u)
    case 6n:
      Ui.on_thread(u, "thread.settle")
    case 7n:
      Ui.on_thread(u, "thread.unsettle")
    case 8n:
      Ui.on_thread(u, "thread.pin")
    case 9n:
      Ui.on_thread(u, "thread.unpin")
    case 10n:
      Ui.on_thread(u, "thread.archive")
    case 11n:
      Ui.on_thread(u, "turn.interrupt")
    case 12n:
      Ui.toggle_settled(u, value)
    case 13n:
      Act{u, [Copy{value}]}
    case 14n:
      Ui.toggle_panel(u)
    case 15n:
      Ui.dismiss(u)
    case 16n:
      Ui.delete(u)
    case 17n:
      Ui.rpc(u, "app.update", Nil{})
    case 18n:
      Models.opened(value, Voice.flag(value, Ui.flag.act(u, value)))
    case 19n:
      Ui.setting(u, value)
    case 20n:
      Ui.also(Ui.model(u, value), [Focus{"composer"}])
    case 21n:
      Ui.rpc(u, "kicad.install", Nil{})
    case 22n:
      Ui.answer(u, value)
    case 23n:
      Ui.on_thread.p(u, "git.act", [J.KV{"action", J.Str{value}}])
    case 24n:
      Dv.ask(u)
    case 25n:
      Ui.revert(u, value)
    case 26n:
      Proj.open.fresh(u)
    case 27n:
      Proj.run.k(1n, u, value)
    case 28n:
      Proj.run.k(0n, u, value)
    case 29n:
      Proj.run.k(2n, u, value)
    case 30n:
      Act{Proj.set(u, value, 0n), Nil{}}
    case 31n:
      Act{Ui.flag_off(u, "proj"), Nil{}}
    case 32n:
      Proj.enter.u(u)
    case 33n:
      Ui.effort.set(String.starts_with(value, "@"), u, value)
    case 34n:
      Ui.skill(u, value)
    case 35n:
      Ui.snooze(u, value)
    case 36n:
      Ui.on_thread(u, "thread.unarchive")
    case 37n:
      Ui.reorder(u, value)
    case 38n:
      Ui.move(u, value)
    case 39n:
      Ui.mention(u, value)
    case 40n:
      Ui.on_thread(u, "files.list")
    case 41n:
      Act{Ui.scratch_set(u, "@del", ""), Nil{}}
    case 42n:
      Proj.run.k(3n, u, value)
    case 43n:
      Find.toggle(u, value)
    case 44n:
      Ui.remove_project(u, value)
    case 45n:
      Find.go(u, value)
    case 46n:
      Act{Ui.scratch_set(u, "@f.pq", value), Nil{}}
    case 47n:
      Act{Ui.scratch_set(u, "@rm", ""), Nil{}}
    case 49n:
      Act{Ui.drop_hover(u, value), Nil{}}
    case 50n:
      Ui.attach(u, value)
    case 51n:
      Ui.attach_file(u, value)
    case 52n:
      Ui.detach(u, value)
    case 53n:
      Ui.term_toggle(u, value)
    case 54n:
      Ui.term_key(u, value)
    case 55n:
      Ui.term_paste(u, value)
    case 56n:
      Ui.key(u, value)
    case 57n:
      Ui.rpc(u, "update.check", Nil{})
    case 58n:
      Ui.machine(u, value)
    case 59n:
      Ui.on_row(u, value, "thread.settle")
    case 60n:
      Ui.on_row(u, value, "thread.unsettle")
    case 61n:
      Ui.row_snooze(u, value)
    case 62n:
      Ui.row_delete(u, value)
    case 63n:
      Act{Ui.tool_fold(u, value), Nil{}}
    case 64n:
      Ui.row_toggle(u, value)
    case 65n:
      Ui.on_row(u, Bool.pick(String, String.is_empty(value), Ui.sel(u), value), "kicad.open")
    case 66n:
      Ui.reorder_project(u, value)
    case 67n:
      Act{Ui.look(u, value), Nil{}}
    case 68n:
      Ui.setting(u, value)
    case 69n:
      Ui.on_thread.p(u, "queue.steer", [J.KV{"msg", J.Str{value}}])
    case 70n:
      Ui.on_thread.p(u, "queue.remove", [J.KV{"msg", J.Str{value}}])
    case 71n:
      Ui.q_up(u, value)
    case 72n:
      Ui.q_edit(u, value)
    case 73n:
      Ui.on_thread(u, "queue.resume")
    case 74n:
      Ui.send(u, True{})
    case 75n:
      Ui.fork.with(u, value)
    case 76n:
      Ui.on_row(u, Ui.fork.of(u, value), "thread.back")
    case _:
      Ui.act.far(Nat.is_ge(i, 178n), i, u, value)

def Uber.act.go(route: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match route:
    case True{}:
      Uber.act.k(C.Codec.pos(["uber", "uber-promote", "uber-demote", "uber-bot", "uber-new", "uber-home"], action), u, value)
    case False{}:
      Ui.act.k(C.Codec.pos(Ui.actions(), action), u, value)

# a user action with its value
def Ui.act.m(menu: Bool, +i: Nat, +u: Ui, +action: String, +value: String) -> Act:
  match menu:
    case True{}:
      Menu.act.k(i, u, value)
    case False{}:
      Menu.after(action, Ui.reselect.act(Ui.sel(u), Uber.act.go(String.starts_with(action, "uber"), u, action, value)))

def Ui.act.e(emb: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match emb:
    case True{}:
      Emb.act(u, action, value)
    case False{}:
      Emb.sync.sel(Ui.sel(u), Fab.sync(Dv.sync(Mpage.sync(Ui.act.m(Nat.is_lt(C.Codec.pos(Menu.actions(), action), 17n), C.Codec.pos(Menu.actions(), action), u, action, value)))))

# the picker's machine row (routed before the action table)
def Ui.act.p(pick: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match pick:
    case True{}:
      Proj.machine(u, value)
    case False{}:
      Ui.act.e(String.starts_with(action, "emb-"), u, action, value)

def Ui.act.t(tour: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match tour:
    case True{}:
      Tour.act(u, action)
    case False{}:
      Ui.act.p(String.eq(action, "proj-on"), u, action, value)

def Ui.act.c(choice: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match choice:
    case True{}:
      Tour.choose.k(Ui.act.t(False{}, u, action, value))
    case False{}:
      Ui.act.t(String.starts_with(action, "ob-"), u, action, value)

def Ui.act.u(pick: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match pick:
    case True{}:
      Ui.act.c(Nat.is_eq(Tour.at(u), OB.Ob.uber()), u, action, value)
    case False{}:
      Ui.act.c(False{}, u, action, value)

def Ui.act.share(share: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match share:
    case True{}: Share.route(String.eq(action, "share"), u, value)
    case False{}: Ui.act.u(Nat.is_lt(C.Codec.pos(["uber-promote", "uber-new"], action), 2n), u, action, value)

def Ui.act.kx(kx: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match kx:
    case True{}: KX.act(u, action, value)
    case False{}: Ui.act.share(Share.is(action), u, action, value)

# (design review's "rv-" actions route before the rest: Rv.act)
def Ui.act.r(rv: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match rv:
    case True{}:
      Rv.act(u, action, value)
    case False{}:
      Ui.act.kx(Bool.or(String.starts_with(action, "kicad-export10"), Bool.and(String.eq(action, "menu-close"), KX.pending(Ui.infos(u), Ui.sel(u)))), u, action, value)

# (Antigravity's "agy-" actions route before the rest too: Agv.act)
# (the secret box's "secret-" actions: Sbox.act)
def Ui.act.sec(sec: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match sec:
    case True{}:
      Sbox.act(u, action, value)
    case False{}:
      Ui.act.r(String.starts_with(action, "rv-"), u, action, value)

# a phone's API key Save (bridge.js's apiKey) says "key-sent" or
# "key-err" (value: why): the "key" form's
def Sealed.phone(+action: String, +value: String) -> String:
  Bool.pick(String, String.eq(action, "key-sent"), "key", Bool.pick(String, String.eq(action, "key-err"), "key\u{1f}" ++ value, value))

# (the web's sealed Save, after host.js's sealedSave: "sealed-sent",
# "sealed-err"; a phone's API key Save: "key-sent", "key-err")
def Ui.act.key(key: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match key:
    case True{}:
      Sealed.after(Bool.or(String.eq(action, "sealed-sent"), String.eq(action, "key-sent")), u, Sealed.phone(action, value))
    case False{}:
      Ui.act.sec(String.starts_with(action, "secret-"), u, action, value)

def Ui.act.g.legacy(agy: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match agy:
    case True{}:
      Agv.act(u, action, value)
    case False{}:
      Ui.act.key(Nat.is_lt(C.Codec.pos(["sealed-sent", "sealed-err", "key-sent", "key-err"], action), 4n), u, action, value)

def Acv.dispatch(account: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match account:
    case True{}: Acv.act(u, action, value)
    case False{}: Ui.act.g.legacy(String.starts_with(action, "agy-"), u, action, value)

# (browser tasks' "tk-" and "bb-" actions: Teach.act)
# (the schematic's sheet navigation, "sch-": Sch.act)
def Ui.act.sch(sch: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match sch:
    case True{}:
      Sch.act(u, action, value)
    case False{}:
      Acv.dispatch(String.starts_with(action, "acct-"), u, action, value)

# (the combined view's "split-pane": a pane put away or back, kept as the
# setting split.hide for every client of the hub)
# (and the probe's "probe": Probe.set)
def Ui.act.split(i: Nat, +u: Ui, +action: String, +value: String) -> Act:
  match i:
    case 0n:
      Ui.setting(u, "split.hide=" ++ Split.flip(Split.hide(u), value))
    case 1n:
      Probe.set(u, value)
    case _:
      Ui.act.sch(String.starts_with(action, "sch-"), u, action, value)

def Ui.act.tk(tk: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match tk:
    case True{}:
      Teach.act(u, action, value)
    case False{}:
      Ui.act.split(C.Codec.pos(["split-pane", "probe"], action), u, action, value)

def Ui.act.s(close: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match close:
    case True{}:
      Skills.close(u)
    case False{}:
      Ui.act.tk(Teach.is(action), u, action, value)

# (Settings > Usage's "usage-refresh": Boat's CLI read again now;
# "usage-open": a thread's Boat chip)
def Ui.act.us(i: Nat, +u: Ui, +action: String, +value: String) -> Act:
  match i:
    case 0n:
      Usage.ask(Act{u, Nil{}}, "refresh")
    case 1n:
      Usage.open(Ui.flag(u, "settings"), u)
    case 2n:
      Usage.page.choose(u, "@usage.period", UP.Up.period(value))
    case 3n:
      Usage.page.choose(u, "@usage.metric", UP.Up.metric(value))
    case 4n:
      Usage.page.turn(u, "model", value)
    case 5n:
      Usage.page.turn(u, "project", value)
    case 6n:
      Usage.page.choose(u, "@usage.provider", UG.Ug.selected(Usage.page.providers(u), value))
    case _:
      Ui.act.s(String.eq(action, "skill-close"), u, action, value)

# (the project card's fab-open, fab-edit and buy: Fab.act)
def Ui.act.fab(fab: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match fab:
    case True{}:
      Fab.act(u, action, value)
    case False{}:
      Ui.act.us(C.Codec.pos(["usage-refresh", "usage-open", "usage-period", "usage-metric", "usage-model-page", "usage-project-page", "usage-provider"], action), u, action, value)

# (the board picker opened: Bd.look)
def Ui.act.bd(look: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match look:
    case True{}:
      Bd.look(u)
    case False{}:
      Ui.act.fab(Fab.is(action), u, action, value)

def Ui.act.z(tz: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match tz:
    case True{}:
      Tzs.act(u, action, value)
    case False{}:
      Ui.act.bd(String.eq(action, "board-list"), u, action, value)

def Ui.act.subs(shut: Bool, open: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match shut open:
    case True{} _:
      Act{Ui.subs.shut(u, value), Nil{}}
    case False{} True{}:
      Act{Ui.info_set(u, "subs:" ++ value, "1"), Nil{}}
    case False{} False{}:
      Ui.act.z(String.starts_with(action, "tz-"), u, action, value)

# Hold actions share the ordinary route; language actions keep their own
# dispatch and the review sync still wraps the ordinary route.
def Ui.act.hold(hold: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match hold:
    case True{}:
      Hold.act(u, action, value)
    case False{}:
      Ui.act.subs(String.eq(action, "subs-shut"), String.eq(action, "subs-open"), u, action, value)

def Lang.dispatch(k: Nat, +u: Ui, +action: String, +value: String) -> Act:
  match k:
    case 0n: Lang.act(0n, u, value)
    case 1n: Lang.act(1n, u, value)
    case _: Rc.sync.of(Ui.act.hold(String.starts_with(action, "hold-"), u, action, value))

def Mailv.dispatch(mail: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match mail:
    case True{}: Mailv.action(C.Codec.pos(["mail-filter", "mail-open", "mail-more", "mail-send", "mail-mark", "mail-search", "mail-run", "mail-to", "mail-close"], action), u, value)
    case False{}: Lang.dispatch(C.Codec.pos(["lang", "lang-os"], action), u, action, value)

def Browse.dispatch(browser: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match browser:
    case True{}: Browse.act.k(C.Codec.pos(["br-go", "br-back", "br-forward", "br-reload", "br-new", "br-select", "br-close", "br-prev", "br-next"], action), u, value)
    case False{}: Mailv.dispatch(String.starts_with(action, "mail-"), u, action, value)

def Hookv.dispatch(hit: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match hit:
    case True{}:
      Hookv.act(u, action, value)
    case False{}:
      Mailv.sync(Ui.cast.now(u, Browse.dispatch(String.starts_with(action, "br-"),
        Browse.field.change(Bool.and(String.eq(action, "bfield"), String.starts_with(value, "brurl:")), u, value), action, value)))

def Settings.dispatch(nav: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match nav:
    case True{}: Settings.nav(u, value)
    case False{}: Hookv.dispatch(Hookv.ours(action), u, action, value)

def ThreadTree.can_create(+st: M.State, +id: String) -> Bool:
  Bool.and(ST.Tree.alive(Ui.thread(st, id)), Bool.and(Bool.not(String.is_empty(ST.Tree.project(Ui.thread(st, id)))), Bool.not(M.Scratch.is(FR.Fr.local(ST.Tree.project(Ui.thread(st, id)))))))

def ThreadTree.can_route(+st: M.State, +id: String) -> Bool:
  ST.Tree.alive(Ui.thread(st, id))

def ThreadTree.route.mark(a: Act, id: String) -> Act:
  match a:
    case Act{u, cs}: Act{Ui.scratch_set(u, "@thread.route", id), cs}

def ThreadTree.route.open(alive: Bool, +u: Ui, id: String) -> Act:
  match alive:
    case True{}: ThreadTree.route.mark(Proj.open.fresh(Ui.scratch_set(u, "@menu", "")), id)
    case False{}: Act{Ui.error_set(u, "This conversation is no longer available."), Nil{}}

def ThreadTree.create.k(allowed: Bool, +u: Ui, +id: String) -> Act:
  match allowed:
    case True{}:
      Ui.rpc(Made.want(Ui.scratch_set(u, "@menu", "")), "thread.create", [J.KV{"project", J.Str{ST.Tree.project(Ui.thread(Ui.state(u), id))}}, J.KV{"parent", J.Str{id}}, J.KV{"title", J.Str{"New subthread"}}])
    case False{}: ThreadTree.route.open(ThreadTree.can_route(Ui.state(u), id), u, id)

def ThreadTree.create(+u: Ui, +id: String) -> Act:
  ThreadTree.create.k(ThreadTree.can_create(Ui.state(u), id), u, id)

def ThreadTree.menu.child(allowed: Bool, +id: String) -> List<&2, MItem>:
  match allowed:
    case True{}: [MItem{"New subthread", "thread-child", id, ""}]
    case False{}: Nil{}

def ThreadTree.menu(+st: M.State, +id: String) -> List<&2, MItem>:
  List.append(&2, MItem, ThreadTree.menu.child(ThreadTree.can_route(st, id), id),
    Bool.pick(List<&2, MItem>, String.is_empty(ST.Tree.parent(st, id)), Nil{}, [MItem{"Move to top level", "thread-detach", id, ""}]))

# Fork thread, and a fork's way back to its source: what the header's ▾
# offers, for the row's own thread (its value names it)
def Menu.fork(+l: I.Loc, forked: Bool, +id: String) -> List<&2, MItem>:
  match forked:
    case True{}:
      [MItem{MSG.T.tools.forkthread(l), "fork", id, ""}, MItem{MSG.T.menu.forkback(l), "back", id, ""}]
    case False{}:
      [MItem{MSG.T.tools.forkthread(l), "fork", id, ""}]

# a thread's menu (armed: its Delete was picked once)
def Menu.thread(+l: I.Loc, +st: M.State, +now: Nat, +id: String, armed: Bool) -> List<&2, MItem>:
  List.concat(&2, MItem, [ThreadTree.menu(st, id), Menu.fork(l, Bool.not(String.is_empty(M.Fork.source(st, id))), id),
    Menu.thread.k.loc(l, M.Shelf.at(Ui.thread(st, id), Side.until(st, id), now), Active.pinned(Ui.thread(st, id)), id, armed), [Menu.ref(l, R.Ref.thread(id))]])

def Menu.items.b(+l: I.Loc, bot: Bool, +u: Ui, +id: String) -> List<&2, MItem>:
  match bot:
    case True{}:
      Menu.bot(l, Side.st(u), id)
    case False{}:
      Menu.thread(l, Side.st(u), Side.now(u), id, String.eq(Ui.scratch(u, "@del"), id))

def Menu.items.t(+l: I.Loc, file: Bool, +u: Ui, +id: String) -> List<&2, MItem>:
  match file:
    case True{}:
      Menu.file.here.loc(l, u, id)
    case False{}:
      Menu.items.b(l, String.eq(Menu.kind(Menu.open(u)), "b"), u, id)

def Menu.items.k(+l: I.Loc, thread: Bool, +u: Ui, +id: String, rich: Bool) -> List<&2, MItem>:
  match thread:
    case True{}:
      Menu.items.t(l, String.eq(Menu.kind(Menu.open(u)), "f"), u, id)
    case False{}:
      Menu.project(l, id, String.eq(Ui.scratch(u, "@rm"), id), rich)

# the open menu's items (none while shut)
def Menu.items.loc(+l: I.Loc, +u: Ui, rich: Bool) -> List<&2, MItem>:
  Bool.pick(List<&2, MItem>, String.is_empty(Menu.open(u)), Nil{}, Menu.items.k(l, Bool.not(String.eq(Menu.kind(Menu.open(u)), "p")), u, Menu.id(Menu.open(u)), rich))

def Menu.items(+u: Ui, rich: Bool) -> List<&2, MItem>:
  Menu.items.loc(Lang.loc(u), u, rich)

def ThreadTree.target(+u: Ui, +id: String) -> String:
  Bool.pick(String, String.is_empty(id), Ui.sel(u), id)

# the model picker's actions (Mpk.*)
def Mpk.dispatch(i: Nat, +u: Ui, +value: String, +action: String) -> Act:
  match i:
    case 0n: Mpk.key(u, value)
    case 1n: Mpk.click(u, 0n, value)
    case 2n: Mpk.click(u, 1n, value)
    case 3n: Mpk.click(u, 2n, value)
    case _: Settings.dispatch(String.eq(action, "settings-page"), u, action, value)

def ThreadTree.dispatch(i: Nat, +u: Ui, +value: String, +action: String) -> Act:
  match i:
    case 0n: ThreadTree.create(u, ThreadTree.target(u, value))
    case 1n: Ui.setting(u, ST.Tree.open_key(value) ++ "=" ++ Bool.pick(String, ST.Tree.open(Ui.state(u), value, ""), "0", "1"))
    case 2n: Ui.rpc(Ui.scratch_set(u, "@menu", ""), "thread.detach", [J.KV{"thread", J.Str{value}}])
    case 3n: Ui.toggle_settled(u, "conversation-history")
    case _: Mpk.dispatch(C.Codec.pos(["mp-key", "mp-agent", "mp-model", "mp-effort"], action), u, value, action)

def Ui.act.list_page(on: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match on:
    case True{}: Act{Ui.info_set(u, "local.view.pages", VP.Page.positions(Ui.info(u, "local.view.pages"), value)), Nil{}}
    case False{}: ThreadTree.dispatch(C.Codec.pos(["thread-child", "thread-tree", "thread-detach", "thread-history"], action), u, value, action)

def Todos.edit.rpc(+u: Ui, +p: J.Json) -> Act:
  Ui.rpc(u, "todos.edit", [J.KV{"thread", J.Str{J.Json.get_str(p, "thread", "")}}, J.KV{"op", J.Str{J.Json.get_str(p, "op", "")}},
    J.KV{"id", J.Str{J.Json.get_str(p, "id", "")}}, J.KV{"text", J.Str{J.Json.get_str(p, "text", "")}},
    J.KV{"status", J.Str{J.Json.get_str(p, "status", "")}}, J.KV{"to", J.Num{Nat.show(J.Json.get_nat(p, "to", 0n))}}])

def Todos.text.parts(xs: List<&2, String>, +u: Ui, +add: Bool) -> Act:
  match xs:
    case th <> id <> rest:
      Todos.edit.rpc(u, Maybe.default(&2, J.Json, J.Json.parse(TD.Todo.ask(th, Bool.pick(String, add, "add", "set"), id, String.join(rest, "\u{1f}"), "", 0n)), J.Null{}))
    case _: Act{u, Nil{}}

def Todos.dispatch(edit: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match edit:
    case True{}: Todos.edit.rpc(u, Maybe.default(&2, J.Json, J.Json.parse(value), J.Null{}))
    case False{}: Todos.text.parts(String.split(value, '\u{1f}'), u, String.eq(action, "todo-add"))

def Ui.act.todo(on: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match on:
    case False{}: Ui.act.list_page(Bool.and(String.eq(action, "list-page"), String.eq(Ui.info(u, "local.view.lazy"), "1")), u, action, value)
    case True{}:
      Todos.dispatch(String.eq(action, "todo-edit"), u, action, value)

def Ui.act.revert(cancel: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match cancel:
    case True{}: Act{Ui.scratch_set(u, "@revert", ""), Nil{}}
    case False{}: Ui.act.todo(Nat.is_lt(C.Codec.pos(["todo-edit", "todo-text", "todo-add"], action), 3n), u, action, value)

# Reply on message `id` of the thread on show (none: nothing)
def Rpl.set.k(+found: Maybe<&2, String>, +u: Ui, +id: String) -> Ui:
  match found:
    case Some{+x}: Ui.scratch_set(Ui.scratch_set(u, Rpl.key(Ui.sel(u)), id), Rpl.q.key(Ui.sel(u)), RTO.Rto.snip(x))
    case None{}: u

def Rpl.set(+u: Ui, +id: String) -> Ui:
  Rpl.set.k(RTO.Rto.find(M.State.mine(Ui.state(u), Ui.sel(u)), id), u, id)

def Rpl.drop(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(u, Rpl.key(Ui.sel(u)), ""), Rpl.q.key(Ui.sel(u)), "")

# the message the composer replies to ("" none) and its line
def Rpl.of(+u: Ui) -> String:
  Ui.scratch(u, Rpl.key(Ui.sel(u)))

def Rpl.label(+u: Ui) -> String:
  Ui.scratch(u, Rpl.q.key(Ui.sel(u)))

# "reply-to" (value: the message's id) and "reply-drop"
def Rpl.pick(to: Bool, +u: Ui, +value: String) -> Ui:
  match to:
    case True{}: Rpl.set(u, value)
    case False{}: Rpl.drop(u)

def Ui.act.rpl(rpl: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match rpl:
    case True{}: Act{Rpl.pick(String.eq(action, "reply-to"), u, value), [Focus{"composer"}]}
    case False{}: Ui.act.revert(String.eq(action, "revert-no"), u, action, value)

def Ui.act(+u: Ui, +action: String, +value: String) -> Act:
  Ui.act.rpl(Bool.or(String.eq(action, "reply-to"), String.eq(action, "reply-drop")), u, action, value)

def Ui.recv_act(u: Ui, +o: J.Json) -> Act:
  Mailv.sync(Ui.recv_act.s(String.eq(J.Json.get_str(o, "t", ""), "shot"), u, o))

# The index may be supplied by a server-owned canonical-revision memo.
# Online visibility and every unrelated info value are applied anew.
def Side.with_signals.given(u: Ui, signals: Map<&2, Nat>) -> Ui:
  match u:
    case Ui{st, se, dr, li, op, +on, now, ad, pa, ve, ri, wa, er, no, nf, ob, tr}:
      Ui{st, se, dr, li, op, on, now, ad, pa, ve, ri, wa, er, no, SG.Signal.cache(on, signals, nf), ob, tr}

# The server can retain canonical strings while applying recipient info
# freshly on every view. This Ui is display-only and is never adopted.
def Side.signals.online(u: Ui) -> Bool:
  match u:
    case Ui{_, _, _, _, _, online, _, _, _, _, _, _, _, _, _, _, _}: online

def Side.with_signal_text(u: Ui, canonical: Map<&2, String>) -> Ui:
  match u:
    case Ui{st, se, dr, li, op, on, now, ad, pa, ve, ri, wa, er, no, nf, ob, tr}:
      Ui{st, se, dr, li, op, on, now, ad, pa, ve, ri, wa, er, no, SG.Signal.cache.overlay(canonical, nf), ob, tr}

# Display-only activity for this frame. Never merge this decorated Ui back
# into the client: the next frame derives fresh signals from current state.
def Side.with_signals(u: Ui) -> Ui:
  match u:
    case Ui{+st, se, dr, li, op, +on, now, ad, pa, ve, ri, wa, er, no, nf, ob, tr}:
      Ui{st, se, dr, li, op, on, now, ad, pa, ve, ri, wa, er, no, SG.Signal.cache(on, SG.Signal.index(st), nf), ob, tr}

def Side.signal(+u: Ui, t: M.Thread) -> Nat:
  SG.Signal.cached(Ui.infos(u), M.Thread.id(t))

# Only the presentation's local copy of SIx is decorated. Side.ix and its
# native incremental cache retain their original counts and shape.
def Side.signals.ix(ix: SIx, +u: Ui) -> SIx:
  match ix:
    case SIx{by, asks, kids, work}:
      SIx{by, asks, kids, SG.Signal.cache.work(Ui.infos(u), work)}

# The same fresh visibility transform, with no Nat -> String -> Nat trip.
# The caller checks that the undecorated info has no reserved signal keys.
def Side.signals.ix.given(ix: SIx, u: Ui, signals: Map<&2, Nat>) -> SIx:
  match ix u:
    case SIx{by, asks, kids, work} Ui{_, _, _, _, _, on, _, _, _, _, _, _, _, _, _, _, _}:
      SIx{by, asks, kids, SG.Signal.work.given(Map.to_list(&2, Nat, signals), on, work)}

def Side.signal.ix(+ix: SIx, t: M.Thread) -> Nat:
  SIx.work(ix, SG.Signal.key(M.Thread.id(t)))

def Side.signal.has(+ix: SIx, +id: String) -> Bool:
  match ix:
    case SIx{_, _, _, work}:
      SG.Signal.has(work, SG.Signal.key(id))
