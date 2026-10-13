import Base
import ./live.bend as LV
import ./json.bend as J
import ./model.bend as M
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
import ./hist.bend as HI
import ./review.bend as RVW
import ./release.bend as REL
import ./links.bend as LK
import ./apikeys.bend as AK
import ./agy.bend as AGY
import ./diff.bend as DF
import ./fm.bend as FM
import ./onboard.bend as OB
import ./layers.bend as LY
import ./slash.bend as SL
import ./uber.bend as UB
import ./ubersync.bend as US
import ./ruledata.bend as RD
import ./tbdata.bend as TBD
import ./cron.bend as CR

# The client state, Elm-style, shared by the native window and the web
# page: a host feeds server messages to Ui.recv and user actions to Ui.act,
# then draws the result. The read model is folded by the same State.apply
# the server uses (law replay_snoc), so what a client shows is exactly what
# the server stored.

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
    terms: Map<&2, VT.Vt>
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
def Label.title(+st: M.State, +id: String, title: String) -> String:
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
    Map.set(&2, String, Map.new(&2, String), "clock", Nat.show(now)), Map.new(&2, String), Map.new(&2, VT.Vt)}

def Str.get.k(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

# Lookups
# -------

def Str.get(m: Map<&2, String>, +k: String) -> String:
  Str.get.k(Map.get(String, "", m, k))

def Flag.get.k(r: Map<&2, Bool> & Bool) -> Bool:
  (_, v) = r
  v

def Flag.get(m: Map<&2, Bool>, +k: String) -> Bool:
  Flag.get.k(Map.get(Bool, False{}, m, k))

def Ui.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

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

def Ui.fold(cs: List<&2, M.Change>, u: Ui) -> Ui:
  match cs u:
    case Nil{} _:
      u
    case +c <> t Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.fold(t, Ui.see(Ui{M.State.apply(st, c), sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, c))

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

def Ui.fold.d(d: Decoded, u: Ui) -> Ui:
  match d:
    case Decoded{cs, n}:
      Ui.skip(Ui.fold(List.reverse(&2, M.Change, cs), u), n)

# a message's items folded in, the unreadable ones counted
def Ui.fold.items(xs: List<&2, J.Json>, u: Ui) -> Ui:
  Ui.fold.d(Ui.decode.all(xs, Decoded{Nil{}, 0n}), u)

# a "changes" message (or "log") holding these items (for laws)
def Seq.demo(+t: String, xs: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{t}}, J.KV{"items", J.Json.arr(xs)}])

def Info.fields(fs: J.Json, m: Map<&2, String>) -> Map<&2, String>:
  match fs:
    case J.Field{k, J.Str{v}, t}:
      Info.fields(t, Map.set(&2, String, m, k, v))
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
  Local.keep.go(["cid", "local.machine", "local.machines", "local.drafts", "local.hub", "clock", "local.fixed", "local.generation", "local.state.hub", "local.state.gen", "local.remote.url", "local.remote.pre"], old, m)

# A reconnect to the same hub (the log's origin is the one this client
# had) keeps what the person chose in each thread's viewer: its kind, path
# and panel, whether they toggled it by hand, the diff, folds, the
# history step and comparison, the Files folder. Results asked again
# (refs:, fm.list:) and playback (hist.play:) are left out. Another
# machine's log keeps none of them (Local.keep runs on a switch too)
# (laws reset_keeps_viewer, reset_new_origin_drops_viewer)
def Reset.prefixes() -> List<&2, String>:
  ["view.", "panel:", "panel.hand:", "diff:", "fold:", "hist.at:", "hist.back:", "hist.diff:", "cmp.a:", "cmp.b:", "cmp.al", "cmp.bl", "fm.want:"]

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

def Machines.of(+j: J.Json, +cur: String) -> List<&2, MRow>:
  MRow{Bool.pick(String, String.is_empty(J.Json.get_str(J.Json.get_obj(j, "self"), "name", "")), "This machine", J.Json.get_str(J.Json.get_obj(j, "self"), "name", "")), "", String.is_empty(cur)}
    <> Machines.peers(J.Json.elems(J.Json.get_obj(j, "peers")), J.Json.get_str(J.Json.get_obj(j, "self"), "url", ""), cur)

# the name of the machine shown
def Machines.current(rs: List<&2, MRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case MRow{+n, _, +on} <> t:
      Bool.pick(String, on, n, Machines.current(t))

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
      Machine.drafts(t, dr, Machine.draft(String.starts_with(k, "@"), k, dr, out))

# The window assigns the actual connection tag when it opens the link.
# The transition itself also advances the generation, before any message
# can try to satisfy a selection against the old read model.
def Ui.machine(u: Ui, +url: String) -> Act:
  match u:
    case Ui{_, _, +dr, _, _, _, now, ad, _, _, rid, w, _, _, +nf, _, _}:
      Act{Ui{M.State.empty(), "", Machine.drafts(Map.keys(&2, String, dr), dr, Map.new(&2, String)), Map.new(&2, String), Map.new(&2, Bool), False{}, now, ad, False{}, "", rid, w, "", "connecting",
        Map.set(&2, String, Map.set(&2, String, Local.keep(nf, Map.new(&2, String)), "local.machine", url), "local.generation", "pending:" ++ Str.get(nf, "local.generation")), Map.new(&2, String), Map.new(&2, VT.Vt)}, [Connect{url}]}

def Ui.info_merge(u: Ui, o: J.Json) -> Ui:
  match u o:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} J.Obj{fs}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Machines.home(Info.fields(fs, nf)), ob, tr}
    case _ _:
      u

def Ui.base(resume: Bool, st: M.State) -> M.State:
  match resume:
    case True{}:
      st
    case False{}:
      M.State.empty()

# the server's log on (re)connect: from nothing, or, when it says `since`
# (the client already has that many changes), just what was missed; the
# client's id survives the machine info being replaced
def Ui.reset(u: Ui, +o: J.Json) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, _, now, ad, pa, _, rid, w, er, note, +nf, ob, tr}:
      Ui.fold.items(J.Json.elems(J.Json.get_obj(o, "items")),
        Ui{Ui.base(Nat.is_gt(J.Json.get_nat(o, "since", 0n), 0n), st), sel, Map.set(&2, String, dr, "@sync", "1"), live, op, True{}, now, ad, pa, J.Json.get_str(o, "version", ""), rid, w, er, note,
          Machines.home(Ui.reset.keep(String.eq(J.Json.get_str(o, "origin", ""), Str.get(nf, "origin")), nf,
            Local.keep(nf, Map.set(&2, String, Map.set(&2, String, Info.of(J.Json.get_obj(o, "info")), "cid", Str.get(nf, "cid")), "origin", J.Json.get_str(o, "origin", ""))))), ob, tr})

# how many changes this client holds (a reconnect asks for the rest)
# the log's read model handed over whole (the hub's own window: nothing
# to parse or replay)
def Ui.adopt.raw(u: Ui, st: M.State) -> Ui:
  match u:
    case Ui{_, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

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
def Ui.reply.err(+o: J.Json) -> String:
  Bool.pick(String, J.Json.get_bool(o, "ok", True{}), "", J.Json.get_str(o, "error", J.Json.get_str(J.Json.get_obj(o, "result"), "error", String.take(J.Json.get_str(J.Json.get_obj(o, "result"), "text", "failed"), 200n))))

# an answer: its request leaves the outbox
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

def Viewer.put(+nf: Map<&2, String>, +th: String, +kind: String, +path: String) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, nf, "view.kind:" ++ th, kind), "view.path:" ++ th, path), "view.n:" ++ th, Viewer.n(nf, th))

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

def Ui.reselect.go(u: Ui, +old: String) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, String.eq(Str.get(nf, "panel:" ++ sel), "1"), v, rid, w, er, note,
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
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Viewer.put(nf, sel, kind, ""), ob, tr}

# a diff opens its thread's viewer on it (on screen when that thread is)
def Ui.diff.k(u: Ui, +th: String, +r: J.Json) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, +nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, Bool.or(pa, String.eq(th, sel)), v, rid, w, er, note,
        Viewer.put(Map.set(&2, String, Map.set(&2, String, nf, "diff:" ++ th, J.Json.get_str(r, "diff", "")), "panel:" ++ th, "1"), th, "diff", ""), ob, tr}

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

def Tail.state.of(+t: String, +nf: Map<&2, String>) -> String:
  Bool.pick(String, String.eq(t, "reachable"), Str.get(nf, "tailnet"), Bool.pick(String, String.eq(t, "unavailable"), "not connected", t))

def Tail.state(+nf: Map<&2, String>) -> String:
  Tail.state.of(Tail.told(nf), nf)

def Tail.label(+nf: Map<&2, String>) -> String:
  "Tailscale: " ++ Tail.state(nf)

def Tail.note.of(+t: String, +nf: Map<&2, String>) -> String:
  Bool.pick(String, String.eq(t, "off"), "Listening on " ++ Str.get(nf, "listen") ++ " only: started with --no-tailscale.",
    Bool.pick(String, String.eq(t, "reachable"), "Also on your tailnet: your own devices get in, others need the pairing token.",
      Bool.pick(String, String.eq(t, "unavailable"), "Listening on " ++ Str.get(nf, "listen") ++ " only. Sign in to Tailscale, then restart Backplane.",
        "Looking for Tailscale.")))

def Tail.note(+nf: Map<&2, String>) -> String:
  Tail.note.of(Tail.told(nf), nf)

def Pair.note(+link: String) -> String:
  Bool.pick(String, String.is_empty(link), "Shown once Backplane listens on your tailnet.", link)

def Pair.rows(+s: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(s), Nil{}, String.split(s, '\n'))

# the pairing link as a QR code (info "pair.qr": its rows, '1' dark), for
# a phone's camera; the flag "pairqr" shows it in Settings
def Pair.qr(+nf: Map<&2, String>) -> List<&2, String>:
  Pair.rows(Str.get(nf, "pair.qr"))

def Pair.qr.ready(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "pair.qr")))

def Pair.qr.note() -> String:
  "Scan with the Backplane app, or the phone's camera."

# KiCad itself: the hub's kicad-cli (info "kicad.cli", "" when there is
# none on the machine), which ERC, DRC, exports and the 3D model need
def KiCad.found(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "kicad.cli")))

def KiCad.cli.label(+nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.found(nf), "KiCad CLI", "KiCad CLI: not found")

def KiCad.cli.note(+nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.found(nf), Str.get(nf, "kicad.cli"), "Install KiCad from kicad.org/download, or Backplane KiCad")

# FreeCAD, which agents model parts with (info "freecad": its command on
# the hub's machine, "" when there is none); Backplane never bundles it
def FreeCAD.found(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "freecad")))

def FreeCAD.label(+nf: Map<&2, String>) -> String:
  Bool.pick(String, FreeCAD.found(nf), "FreeCAD", "FreeCAD: not found")

def FreeCAD.note(+nf: Map<&2, String>) -> String:
  Bool.pick(String, FreeCAD.found(nf), Str.get(nf, "freecad"), "Agents model mechanical parts with it: get it from freecad.org/downloads")

# KiCad IPC is the Backplane fork's api-server: without the fork the
# setting has nothing to turn on, so Settings says where to get it instead
# of offering it (info "kicad.fork": the fork's version, "" for none)
def KiCad.ipc.ready(+nf: Map<&2, String>) -> Bool:
  Bool.not(String.is_empty(Str.get(nf, "kicad.fork")))

def KiCad.ipc.label(+nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.ipc.ready(nf), "KiCad IPC", "KiCad IPC: needs Backplane KiCad")

def KiCad.ipc.note(+nf: Map<&2, String>) -> String:
  Bool.pick(String, KiCad.ipc.ready(nf), "Agents may drive KiCad through the fork's api-server (NNG/protobuf).",
    "Install it with the button above (Linux x86-64) or from github.com/i2cjak/Backplane_KiCad")

def Update.line(+nf: Map<&2, String>, +note: String) -> String:
  Bool.pick(String, Update.packaged(nf), "Updated by its package.", Bool.pick(String, String.is_empty(note), "Up to date, checked hourly.", note))

# an update the hub holds back while threads work (hub.bend's Update.*):
# how many it waits for (0: none, or not waiting)
def Update.waits(+st: M.State) -> Nat:
  Bool.pick(Nat, Bool.and(String.is_empty(M.Setting.get(st, "update.wait", "")), String.is_empty(M.Setting.get(st, "update.restart", ""))), 0n, M.Work.threads(st))

def Update.waits.line(+n: Nat, +nf: Map<&2, String>, +note: String) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), Update.line(nf, note), "Update waits for " ++ Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " thread", " threads") ++ " to finish their work.")

# the version row's line, with the wait
def Update.line.w(+st: M.State, +nf: Map<&2, String>, +note: String) -> String:
  Update.waits.line(Update.waits(st), nf, note)

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
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, +rid, w, er, note, nf, +ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, pa, v, 1n+rid, w, er, note, nf, Map.set(&2, String, ob, Nat.show(rid), Rpc.text(rid, m, p)), tr}, [Send{Rpc.text(rid, m, p)}]}

def Ui.rpc(u: Ui, m: String, p: List<&2, J.KV>) -> Act:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, +rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, pa, v, 1n+rid, w, er, note, nf, ob, tr}, [Send{Rpc.text(rid, m, p)}]}

# a request about the selected thread, with more params
def Ui.on_thread.p(u: Ui, m: String, p: List<&2, J.KV>) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, m, J.KV{"thread", J.Str{sel}} <> p)

def Ui.on_thread(u: Ui, m: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, m, [J.KV{"thread", J.Str{sel}}])

def Ui.select.k(u: Ui, +id: String) -> Ui:
  match u:
    case Ui{st, _, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, id, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, dr, "@board", ""), "@del", ""), "@bot", ""), "@room", ""), "@remote", ""), "@bot.want", ""), "@thread.want", ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

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

# a kept draft, read back by the host when it starts
def Ui.restore(u: Ui, +id: String, saved: String) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, id, Draft.pick(Str.get(dr, id), saved)), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def Ui.also(a: Act, more: List<&2, Cmd>) -> Act:
  match a:
    case Act{u, cs}:
      Act{u, List.append(&2, Cmd, cs, more)}

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
# While a turn runs, a message either waits in the queue or steers the
# turn. The "followup" setting picks ("queue" or "steer"); unset, each
# provider keeps its way (Claude steers, Codex, Grok and Muse queue). Ctrl+Enter
# does the other one for that message (laws follow_*).

def Follow.flip(+m: String) -> String:
  Bool.pick(String, String.eq(m, "queue"), "steer", "queue")

def Follow.base(+setting: String, +provider: String) -> String:
  Bool.pick(String, Bool.or(String.eq(setting, "queue"), String.eq(setting, "steer")), setting,
    Bool.pick(String, String.eq(provider, "claude"), "steer", "queue"))

def Follow.mode.k(alt: Bool, +m: String) -> String:
  match alt:
    case True{}:
      Follow.flip(m)
    case False{}:
      m

# the mode a message is sent with
def Follow.mode(+setting: String, +provider: String, alt: Bool) -> String:
  Follow.mode.k(alt, Follow.base(setting, provider))

# what the send button says: Send, or while a turn runs what Enter does
def Follow.label.k(run: Bool, +mode: String) -> String:
  match run:
    case True{}:
      Bool.pick(String, String.eq(mode, "queue"), "Queue", "Steer")
    case False{}:
      "Send"

def Follow.label(+st: M.State, +sel: String, run: Bool) -> String:
  Follow.label.k(run, Follow.mode(M.Setting.get(st, "followup", ""), M.Thread.provider(Ui.thread(st, sel)), False{}))

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
def Follow.button.k(stop: Bool, +label: String) -> String:
  match stop:
    case True{}:
      "Stop"
    case False{}:
      label

def Follow.button(+st: M.State, +sel: String, run: Bool, stop: Bool, +draft: String) -> String:
  Follow.button.k(stop, Bool.pick(String, Btw.is(draft), "Ask", Bool.pick(String, Give.is(draft), "Give", Follow.label(st, sel, run))))

def Follow.act(stop: Bool) -> String:
  match stop:
    case True{}:
      "interrupt"
    case False{}:
      "send"

def Ui.send.t(u: Ui, text: String, +mode: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, +now, ad, pa, v, +rid, w, er, note, +nf, ob, tr}:
      Ui.also(Ui.rpc.keep(Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, sel, ""), "@att:" ++ sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr},
        "turn.start", [J.KV{"thread", J.Str{sel}}, J.KV{"text", J.Str{text}}, J.KV{"msg", J.Str{Ui.msg_id(Str.get(nf, "cid"), now, rid)}}, J.KV{"mode", J.Str{mode}}]), List.append(&2, Cmd, Draft.keep(sel, ""), [ScrollEnd{}]))

# the add form shut (the folder goes to Proj.adding)
def Ui.add_project(u: Ui) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, _, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, False{}, pa, v, rid, w, er, note, nf, ob, tr}

def Ui.toggle_add(u: Ui) -> Act:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, Bool.not(ad), pa, v, rid, w, er, note, nf, ob, tr}, [Focus{"project-path"}]}

# opened or shut by hand: the hub's own view leaves it alone from now on
def Ui.toggle_panel(u: Ui) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Act{Ui{st, sel, dr, live, op, on, now, ad, Bool.not(pa), v, rid, w, er, note, Map.set(&2, String, nf, "panel.hand:" ++ sel, "1"), ob, tr}, Nil{}}

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

# a setting applied here at once, before the hub answers with the same
# (the sequence is left alone, so nothing the hub sends is skipped)
def Ui.setting.here(u: Ui, ps: List<&2, String>) -> Ui:
  match u ps:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr} k <> t:
      Ui{M.State.setting(st, k, String.join(t, "=")), sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}
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
      Ui.on_thread.p(Ui.close(u, "models"), "thread.provider", [J.KV{"provider", J.Str{String.drop(value, 1n)}}])
    case False{}:
      Ui.on_thread.p(u, "thread.effort", [J.KV{"effort", J.Str{value}}])

# the selected thread's model (the picker closes)
def Ui.model(u: Ui, +model: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.rpc(Ui.close(Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}, "models"), "thread.modes", [J.KV{"thread", J.Str{sel}}, J.KV{"model", J.Str{model}}])

def Ui.info(u: Ui, +k: String) -> String:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, ob, tr}:
      Str.get(nf, k)

# the switcher: this machine first, then the others ("on" marks the one shown)
def Machines.rows(+u: Ui) -> List<&2, MRow>:
  Machines.of(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "local.machines")), J.Null{}), Ui.info(u, "local.machine"))

# only worth showing when there is somewhere else to go
def Machines.any(+u: Ui) -> Bool:
  Nat.is_gt(List.length(&2, MRow, Machines.rows(u)), 1n)

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
   "view-layer-pick", "view-layer-mode", "skill-close", "wake-cancel"]

# Reading the client state
# ------------------------

def Ui.sel(u: Ui) -> String:
  match u:
    case Ui{_, sel, _, _, _, _, _, _, _, _, _, _, _, _, nf, ob, tr}:
      sel

# the panel after the selection may have moved on from thread `old`
def Ui.reselect(+old: String, +u: Ui) -> Ui:
  Ui.reselect.k(String.eq(old, Ui.sel(u)), u, old)

# (Ui.act brings the thread's own panel); a thread chosen leaves Settings
def Ui.select(u: Ui, +id: String) -> Act:
  Act{Ui.flag_off(Ui.select.k(u, id), "settings"), [Focus{"composer"}, ScrollEnd{}]}

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
  Ui.diff.k(u, Viewer.of(J.Json.get_str(r, "thread", ""), Ui.sel(u)), r)

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

def Task.word(s: M.TaskState) -> String:
  match s:
    case M.TaskQueued{}:
      "queued"
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
  Bool.pick(String, String.is_empty(Tasks.parent(st, thread)), M.Launch.by(st, thread), Tasks.parent(st, thread))

# the row's words
def Tasks.up.label(+st: M.State, +thread: String, +p: String) -> String:
  Bool.pick(String, String.is_empty(Tasks.parent(st, thread)), "← launched by " ++ Launched.name(st, p), "← delegated by " ++ Tasks.title(st, p))

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
# first (by order key), the rest by latest activity, each with its threads
# on shelves (M.Sidebar.at: keys and snoozes), and the visible order
# ctrl+1..9 counts in.

type SRow is Data:
  SRow{rank: Nat, project: M.Project, shelves: M.Shelves}

def SRow.before(a: SRow, b: SRow) -> Bool:
  match a b:
    case SRow{ra, _, _} SRow{rb, _, _}:
      Nat.is_ge(ra, rb)

# a row with its project's order key (Nil: none)
type PSlot is Data:
  PSlot{key: List<&2, Bool>, row: SRow}

def PSlot.before.k(ka: List<&2, Bool>, kb: List<&2, Bool>, a: SRow, b: SRow) -> Bool:
  match ka kb:
    case Nil{} Nil{}:
      SRow.before(a, b)
    case Nil{} _ <> _:
      False{}
    case _ <> _ Nil{}:
      True{}
    case x <> xs y <> ys:
      Cmp.is_le(M.Key.cmp(x <> xs, y <> ys))

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
# frame (or phone screen) in grouped passes and never kept: each thread's
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

def Side.rows.k(removed: Bool, p: M.Project, +mine: List<&2, M.Thread>, +c: Nat, +navs: Map<&2, M.Nav>, +now: Nat, +key: String, rest: List<&2, PSlot>) -> List<&2, PSlot>:
  match removed:
    case True{}:
      rest
    case False{}:
      PSlot{M.Key.read(key), SRow{Side.latest(mine, c), p, M.Sidebar.at(mine, navs, now)}} <> rest

def Side.rows.go(ps: List<&2, M.Project>, +ts: List<&2, M.Thread>, +navs: Map<&2, M.Nav>, +now: Nat) -> List<&2, PSlot>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, +c, +rm} <> t:
      Side.rows.k(rm, M.Project{id, ti, r, c, rm}, M.Thread.filter_project(ts, id), c, navs, now, M.Nav.project_key(navs, id), Side.rows.go(t, ts, navs, now))

def Side.rows.go.ix(ps: List<&2, M.Project>, +ix: SIx, +navs: Map<&2, M.Nav>, +now: Nat) -> List<&2, PSlot>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, +c, +rm} <> t:
      Side.rows.k(rm, M.Project{id, ti, r, c, rm}, SIx.mine(ix, id), c, navs, now, M.Nav.project_key(navs, id), Side.rows.go.ix(t, ix, navs, now))

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

def Side.projs.go(ps: List<&2, M.Project>, +s: M.State) -> List<&2, M.Project>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, ti, r, c, rm} <> t:
      Side.projs.k(FR.Fr.is(id), s, id, M.Project{id, ti, r, c, rm}, Side.projs.go(t, s))

def Side.projs(+s: M.State) -> List<&2, M.Project>:
  Side.projs.go(Ui.projects(s), s)

# Side.sorted from the index
def Side.sorted.ix(+st: M.State, +ix: SIx, +now: Nat) -> List<&2, SRow>:
  PSlot.rows(List.sort(~PSlot, ~PSlot.before, Side.rows.go.ix(Side.projs(st), ix, M.State.navs(st), now)))

# the projects by order key, then latest activity, their threads on
# shelves (delegated child threads live under their parent instead)
def Side.sorted(+st: M.State, +now: Nat) -> List<&2, SRow>:
  PSlot.rows(List.sort(~PSlot, ~PSlot.before, Side.rows.go(Side.projs(st), Kids.keep(Ui.threads(st), Kids.set(st)), M.State.navs(st), now)))

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

# the sidebar's projects in order: live, quiet, older
def Side.rows(+st: M.State, +now: Nat) -> List<&2, SRow>:
  SGroups.all(Side.groups(Side.sorted(st, now), now))

# Side.rows from the index (law side_rows_ix_eq)
def Side.rows.ix(+st: M.State, +ix: SIx, +now: Nat) -> List<&2, SRow>:
  SGroups.all(Side.groups(Side.sorted.ix(st, ix, now), now))

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
      Find.keep(Nat.is_gt(FZ.Fuzzy.score(ti, q), 0n), SRow{r, M.Project{id, ti, ro, c, rm}, sh}, Find.rows(t, q))

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

# the threads the sidebar shows, top to bottom (collapsed shelves left out)
def Side.visible(u: Ui) -> List<&2, String>:
  match u:
    case Ui{+st, _, +dr, _, +op, _, +now, _, _, _, _, _, _, _, _, _, _}:
      Side.visible.go(Side.unfolded(Side.shown(st, now, Find.q(op, dr)), op, Find.q(op, dr), now), op)

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

# the sidebar's projects, top to bottom
def Side.projects(u: Ui) -> List<&2, String>:
  match u:
    case Ui{+st, _, +dr, _, +op, _, +now, _, _, _, _, _, _, _, _, _, _}:
      Side.pids(Side.unfolded(Side.shown(st, now, Find.q(op, dr)), op, Find.q(op, dr), now))

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

# the threads the sidebar shows in project pid, top to bottom
def Side.of(+u: Ui, +pid: String) -> List<&2, String>:
  Side.visible.go(Side.only(Side.rows(Side.st(u), Side.now(u)), pid), Side.open(u))

# the threads ctrl+1..9 count in
def Side.here(+u: Ui) -> List<&2, String>:
  Side.of(u, Side.cur(u))

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
  Usage.text(M.Usage.used(M.Setting.get(st, M.Usage.key(th), "")), M.Usage.window(M.Setting.get(st, M.Usage.key(th), "")))

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
      Links.recent.k(Bool.and(Nat.is_le(now, Nat.add(at, 900n)), Bool.not(String.is_empty(M.Link.from(id)))), M.Link.from(id), to, at, Links.recent.go(t, now, Nat.is_le(now, Nat.add(at, 4500n))))
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
      Term.get(tr, thread)

def Ui.term_set(u: Ui, +thread: String, v: VT.Vt) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, Map.set(&2, VT.Vt, tr, thread, v)}

def Ui.term_feed(u: Ui, +thread: String, bs: List<&2, U32>) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, +tr}:
      Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, Map.set(&2, VT.Vt, tr, thread, VT.Vt.feed(Term.get(tr, thread), bs))}

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
      Ui.rpc(Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, Map.set(&2, VT.Vt, tr, sel, VT.Vt.resize(Term.get(tr, sel), cols, rows))},
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

def Palette.threads(ts: List<&2, M.Thread>) -> List<&2, PItem>:
  match ts:
    case Nil{}:
      Nil{}
    case M.Thread{id, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> t:
      PItem{ti, "select", id} <> Palette.threads(t)

def Palette.actions(+pid: String) -> List<&2, PItem>:
  [PItem{"New thread", "new-thread", pid}, PItem{"Diff: what this thread changed", "diff", ""}, PItem{"Terminal", "key:term", ""},
   PItem{"Attach a file", "key:attach", ""}, PItem{"Commit & push", "git", "commit_push"}, PItem{"Open pull request", "git", "commit_push_pr"}, PItem{"Revert thread", "revert", "0"},
   PItem{"Settle thread", "settle", ""}, PItem{"Pin thread", "pin", ""}, PItem{"Archive thread", "archive", ""}, PItem{"Unarchive thread", "unarchive", ""}, PItem{"Delete thread", "delete", ""},
   PItem{"Snooze 1 hour", "snooze", "1h"}, PItem{"Snooze until tomorrow 9:00", "snooze", "tomorrow"}, PItem{"Snooze 1 week", "snooze", "1w"}, PItem{"Unsnooze", "snooze", "off"},
   PItem{"Search threads", "key:search", ""}, PItem{"Find file", "key:files", ""},
   PItem{"Viewer: board, schematic, 3D, Mech, files, diff, browser", "panel", ""}, PItem{"Open in KiCad", "kicad-open", ""}, PItem{"Settings", "flag", "settings"}, PItem{"Update Backplane", "update", ""}]

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

def Palette.filter(ps: List<&2, PItem>, +q: String) -> List<&2, PItem>:
  match ps:
    case Nil{}:
      Nil{}
    case PItem{+l, a, v} <> t:
      Palette.filter.k(Palette.hit(q, l), PItem{l, a, v}, Palette.filter(t, q))

# what the command palette offers for this query, actions first
def Palette.cmds(+u: Ui) -> List<&2, PItem>:
  Palette.filter(List.append(&2, PItem, Palette.actions(Ui.thread_project(Ui.thread(Ui.state(u), Ui.sel(u)))),
    Palette.threads(Kids.keep(Ui.threads(Ui.state(u)), Kids.set(Ui.state(u))))), Palette.query(u))

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

def Search.one(t: M.Thread, +q: String, +snips: Map<&2, String>, +rest: List<&2, PItem>) -> List<&2, PItem>:
  match t:
    case M.Thread{+id, _, +ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +de}:
      Bool.pick(List<&2, PItem>, de, rest,
        Bool.pick(List<&2, PItem>, Nat.is_gt(FZ.Fuzzy.score(ti, q), 0n), PItem{ti, "select", id} <> rest,
          Bool.pick(List<&2, PItem>, String.is_empty(Str.get(snips, id)), rest, PItem{ti ++ "  ·  " ++ Str.get(snips, id), "select", id} <> rest)))

def Search.all(ts: List<&2, M.Thread>, +q: String, +snips: Map<&2, String>) -> List<&2, PItem>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> rest:
      Search.one(t, q, snips, Search.all(rest, q, snips))

def Search.newer(a: M.Thread, b: M.Thread) -> Bool:
  match a b:
    case M.Thread{_, _, _, _, +x, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} M.Thread{_, _, _, _, +y, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Nat.is_ge(x, y)

def Search.items.k(empty: Bool, +q: String, +st: M.State) -> List<&2, PItem>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Search.all(List.sort(~M.Thread, ~Search.newer, Ui.threads(st)), q, Search.snips(Ui.entries(st), q, Map.new(&2, String)))

# threads whose title or messages hold every word of the query, newest first
def Search.items(+u: Ui) -> List<&2, PItem>:
  Search.items.k(String.is_empty(String.trim(Palette.query(u))), Palette.query(u), Ui.state(u))

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
def Palette.items.k(i: Nat, +u: Ui) -> List<&2, PItem>:
  match i:
    case 0n:
      Search.items(u)
    case 1n:
      Files.items(u)
    case _:
      Palette.cmds(u)

def Palette.items(+u: Ui) -> List<&2, PItem>:
  Palette.items.k(Keys.pos(["search", "files"], Palette.mode(u)), u)

# the kind of a row, shown at its right
def Palette.kind(+a: String) -> String:
  Bool.pick(String, String.eq(a, "select"), "thread", Bool.pick(String, String.eq(a, "mention"), "file", "action"))

# Row status
# ----------

def Asks.kind(as: List<&2, M.Ask>, +k: String) -> Bool:
  match as:
    case Nil{}:
      False{}
    case M.Ask{_, _, +x, _, _, _, _, _} <> t:
      Bool.or(String.eq(x, k), Asks.kind(t, k))

# what a thread's dot shows (M.Status.of has the order)
# (a thread's open asks are looked up once: a row asks three questions of
# them, and the sidebar has a row a thread)
def Side.status.k(+st: M.State, +id: String, turn: M.TurnState, +pending: Nat, +as: List<&2, M.Ask>) -> M.Status:
  M.Status.wait.f(Nat.is_gt(M.Work.rows(st, id), 0n), M.Status.of(Bool.or(Nat.is_gt(pending, 0n), Asks.kind(as, "approval")),
    Bool.or(Asks.kind(as, "input"), Asks.kind(as, "plan")), turn,
    M.Queue.waiting(st, id)))

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
    Bool.or(Asks.kind(as, "input"), Asks.kind(as, "plan")), Nat.add(Side.live(Tasks.of(st, id)), M.Work.rows(st, id)), turn,
    M.Queue.waiting(st, id))

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
    Bool.or(Asks.kind(as, "input"), Asks.kind(as, "plan")), Nat.add(SIx.kids(ix, id), SIx.work(ix, id)), turn,
    M.Queue.waiting(st, id))

# Side.phase from the index (law side_phase_ix_eq)
def Side.phase.ix(+ix: SIx, +st: M.State, t: M.Thread) -> M.Phase:
  match t:
    case M.Thread{+id, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Side.phase.ix.k(st, ix, id, turn, pending, SIx.open(ix, id))

# when a thread's snooze ends (0: not snoozed)
def Side.until(+st: M.State, +id: String) -> Nat:
  M.Nav.until(M.Nav.get(M.State.navs(st), id))

# Scratch state
# -------------
# Small client-only values (a drag in progress) kept in the drafts map
# under "@" keys, which no thread id takes.

def Ui.scratch(u: Ui, +k: String) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, k)

def Ui.scratch_set(u: Ui, +k: String, v: String) -> Ui:
  match u:
    case Ui{st, sel, dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, dr, k, v), live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

# Creating a thread
# -----------------
# New Thread, Fork and adding a project ask the hub for a thread; the hub
# answers with the thread it made ({"t":"reply", "thread"}: Reply.made).
# Only that answer selects it, and only on the hub and connection that
# asked ("@want.new" holds the request's id, "@want.new.at" where it went):
# another client's new thread, a replayed log or an older request's
# answer selects nothing (laws want_new_*, made_*, replay_cannot_select).

def Made.at(+u: Ui) -> String:
  Ui.info(u, "local.machine") ++ "|" ++ Ui.info(u, "local.generation")

def Made.rid(u: Ui) -> Nat:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, rid, _, _, _, _, _, _}:
      rid

# the next request is one that makes a thread
def Made.want(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(u, "@want.new", Nat.show(Made.rid(u))), "@want.new.at", Made.at(u))

def Ui.new_thread(u: Ui, +pid: String) -> Act:
  Ui.rpc(Made.want(u), "thread.create", [J.KV{"project", J.Str{pid}}, J.KV{"title", J.Str{"New thread"}}])

# a fork of the selected thread; the new thread is selected when the
# hub's answer names it
def Ui.fork(+u: Ui) -> Act:
  Ui.rpc(Made.want(u), "thread.fork", [J.KV{"thread", J.Str{Ui.sel(u)}}])

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
    case M.PhQueued{}:
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
      Active.live(r, st, pid, pt, ARow{Urg.key(Side.phase(st, t), Active.pinned(t)), Active.activity(t), t, pid, pt, False{}} <> acc)

def Active.live.ix(ts: List<&2, M.Thread>, +st: M.State, +ix: SIx, +pid: String, +pt: String, acc: List<&2, ARow>) -> List<&2, ARow>:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Active.live.ix(r, st, ix, pid, pt, ARow{Urg.key(Side.phase.ix(ix, st, t), Active.pinned(t)), Active.activity(t), t, pid, pt, False{}} <> acc)

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

def Active.all(g: AParts) -> List<&2, ARow>:
  match g:
    case AParts{l, f}:
      List.append(&2, ARow, List.sort(~ARow, ~ARow.before, l), List.sort(~ARow, ~ARow.before, f))

# the active view's rows, top to bottom (laws active_*)
def Active.rows(+st: M.State, +now: Nat, +q: String) -> List<&2, ARow>:
  Active.find(Active.all(Active.gather(Side.rows(st, now), st, now, AParts{Nil{}, Nil{}})), q)

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
  Active.find(Active.all(Active.gather.ix(Side.rows.ix(st, ix, now), st, ix, now, AParts{Nil{}, Nil{}})), q)

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

def Menu.snoozes(+id: String) -> List<&2, MItem>:
  [MItem{"Snooze 1 hour", "row-snooze", id ++ "|1h", ""}, MItem{"Snooze until tomorrow", "row-snooze", id ++ "|tomorrow", ""},
   MItem{"Snooze 1 week", "row-snooze", id ++ "|1w", ""}]

def Menu.del(+id: String, armed: Bool) -> MItem:
  MItem{Bool.pick(String, armed, "Click again to delete", "Delete"), "row-delete", id, "danger"}

def Menu.pin(pinned: Bool, +id: String) -> MItem:
  match pinned:
    case True{}:
      MItem{"Unpin", "row-unpin", id, ""}
    case False{}:
      MItem{"Pin", "row-pin", id, ""}

# every thread can be renamed (Ren.*)
def Menu.ren(+id: String) -> MItem:
  MItem{"Rename", "row-rename", id, ""}

def Menu.thread.k(sh: M.Shelf, pinned: Bool, +id: String, armed: Bool) -> List<&2, MItem>:
  match sh:
    case M.Settled{}:
      [MItem{"Bring back", "row-unsettle", id, ""}, Menu.ren(id), MItem{"Archive", "row-archive", id, ""}, Menu.del(id, armed)]
    case M.Snoozed{}:
      [MItem{"Wake", "row-snooze", id ++ "|off", ""}, MItem{"Settle", "row-settle", id, ""}, Menu.ren(id), MItem{"Archive", "row-archive", id, ""}, Menu.del(id, armed)]
    case M.Hidden{}:
      [MItem{"Unarchive", "row-unarchive", id, ""}, Menu.ren(id), Menu.del(id, armed)]
    case _:
      List.concat(&2, MItem, [[Menu.pin(pinned, id), Menu.ren(id), MItem{"Settle", "row-settle", id, ""}], Menu.snoozes(id),
        [MItem{"Archive", "row-archive", id, ""}, Menu.del(id, armed)]])

# a thread's menu (armed: its Delete was picked once)
def Menu.thread(+st: M.State, +now: Nat, +id: String, armed: Bool) -> List<&2, MItem>:
  Menu.thread.k(M.Shelf.at(Ui.thread(st, id), Side.until(st, id), now), Active.pinned(Ui.thread(st, id)), id, armed)

def Menu.look(rich: Bool, +id: String) -> List<&2, MItem>:
  match rich:
    case True{}:
      [MItem{"Colour and icon", "look", id, ""}]
    case False{}:
      Nil{}

# a project's menu (rich: the window and the web, which have the badge
# editor; armed: Remove was picked once)
def Menu.project(+id: String, armed: Bool, rich: Bool) -> List<&2, MItem>:
  List.concat(&2, MItem, [[MItem{"New thread", "new-thread", id, ""}], Menu.look(rich, id),
    [MItem{Bool.pick(String, armed, "Click again to remove", "Remove project"), "proj-remove", id, "danger"}]])

# a file link's menu ("file-menu path", kept as "f:<path>"; a click on the
# link is its Open). The path goes as written; Flink.* resolves it
def Menu.file(+path: String) -> List<&2, MItem>:
  [MItem{"Open", "file-open", path, ""}, MItem{"Show in folder", "file-reveal", path, ""}, MItem{"Copy path", "file-copy", path, ""}]

# the menu open ("" for none), its kind ("t", "p" or "f") and its id
def Menu.open(+u: Ui) -> String:
  Ui.scratch(u, "@menu")

def Menu.kind(+m: String) -> String:
  String.take(m, 1n)

def Menu.id(+m: String) -> String:
  String.drop(m, 2n)

# a bot's menu ("bot-menu id", kept as "b:<id>"): the person makes a bot
# of this machine the uberbot, or not (core/uber.bend)
def Menu.bot(+st: M.State, +id: String) -> List<&2, MItem>:
  Bool.pick(List<&2, MItem>, UB.Uber.is(st, id), [MItem{"Not uberbot", "uber-demote", "", ""}],
    Bool.pick(List<&2, MItem>, UB.Uber.live(st, id), [MItem{"Make uberbot", "uber-promote", id, ""}], Nil{}))

def Menu.items.b(bot: Bool, +u: Ui, +id: String) -> List<&2, MItem>:
  match bot:
    case True{}:
      Menu.bot(Side.st(u), id)
    case False{}:
      Menu.thread(Side.st(u), Side.now(u), id, String.eq(Ui.scratch(u, "@del"), id))

def Menu.items.t(file: Bool, +u: Ui, +id: String) -> List<&2, MItem>:
  match file:
    case True{}:
      Menu.file(id)
    case False{}:
      Menu.items.b(String.eq(Menu.kind(Menu.open(u)), "b"), u, id)

def Menu.items.k(thread: Bool, +u: Ui, +id: String, rich: Bool) -> List<&2, MItem>:
  match thread:
    case True{}:
      Menu.items.t(String.eq(Menu.kind(Menu.open(u)), "f"), u, id)
    case False{}:
      Menu.project(id, String.eq(Ui.scratch(u, "@rm"), id), rich)

# the open menu's items (none while shut)
def Menu.items(+u: Ui, rich: Bool) -> List<&2, MItem>:
  Bool.pick(List<&2, MItem>, String.is_empty(Menu.open(u)), Nil{}, Menu.items.k(Bool.not(String.eq(Menu.kind(Menu.open(u)), "p")), u, Menu.id(Menu.open(u)), rich))

# its heading: the thread's title, the project's name or the file's path
def Menu.title(+u: Ui) -> String:
  Bool.pick(String, String.eq(Menu.kind(Menu.open(u)), "t"), Active.title(Ui.thread(Side.st(u), Menu.id(Menu.open(u)))),
    Bool.pick(String, String.eq(Menu.kind(Menu.open(u)), "f"), Menu.id(Menu.open(u)),
      Bool.pick(String, String.eq(Menu.kind(Menu.open(u)), "b"), BT.Bot.name(BT.World.bot(M.State.bots(Side.st(u)), Menu.id(Menu.open(u)))), Ui.project_title(Side.st(u), Menu.id(Menu.open(u))))))

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

# in the active view Enter opens the first thread the search kept
def Find.go.active(+u: Ui) -> Act:
  Bool.pick(Act, List.is_empty(&2, String, Active.ids(Active.rows(Side.st(u), Side.now(u), Ui.scratch(u, "@f.pq")))), Act{u, Nil{}},
    Ui.select(Ui.scratch_set(Ui.flag_off(u, "psearch"), "@f.pq", ""), Side.first(Active.ids(Active.rows(Side.st(u), Side.now(u), Ui.scratch(u, "@f.pq"))))))

def Find.go.k2(active: Bool, +u: Ui, +v: String) -> Act:
  match active:
    case True{}:
      Find.go.active(u)
    case False{}:
      Find.go.projects(u, v)

# (a phone names the project it means: v)
def Find.go(+u: Ui, +v: String) -> Act:
  Find.go.k2(Bool.and(String.is_empty(v), String.eq(Side.view(Side.st(u)), "active")), u, v)

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
def Ui.tool_fold(u: Ui, +v: String) -> Ui:
  Ui.info_set(u, FO.Fold.key(String.drop(v, 1n)), String.take(v, 1n))

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

def Btw.ask.k(u: Ui, +q: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.also(Ui.rpc(Ui{st, sel, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, dr, sel, ""), "@btw:" ++ sel, q), "@btw.a:" ++ sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr},
        "thread.btw", [J.KV{"thread", J.Str{sel}}, J.KV{"q", J.Str{q}}]), Draft.keep(sel, ""))

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

def Btw.reply(u: Ui, +o: J.Json) -> Ui:
  Btw.got(Btw.has(o), u, o)

def Btw.close(+u: Ui) -> Act:
  Act{Ui.scratch_set(Ui.scratch_set(u, "@btw:" ++ Ui.sel(u), ""), "@btw.a:" ++ Ui.sel(u), ""), Nil{}}

# the open thread's side question ("" for none) and its answer ("" while
# it is being answered)
def Btw.question(+u: Ui) -> String:
  Ui.scratch(u, "@btw:" ++ Ui.sel(u))

def Btw.reply.of(+u: Ui) -> String:
  Ui.scratch(u, "@btw.a:" ++ Ui.sel(u))

def Btw.text(+a: String) -> String:
  Bool.pick(String, String.is_empty(a), "Thinking…", a)

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

def Refs.want(a: Act) -> Act:
  match a:
    case Act{+u, cs}:
      Act{Refs.fill(RF.Refs.typing(Skills.token(Ui.draft_text(u))), u), cs}

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
  ["@claude", "@codex", "@grok", "@muse", "@antigravity"]

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
def Models.label_of(+info: String, +provider: String, +model: String) -> String:
  J.Json.get_str(J.Json.get_obj(Models.about(info, provider), "labels"), Bool.pick(String, String.is_empty(model), "default", model), "")

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
      Proj.ps.k(Proj.of(on, id), M.Project{id, ti, r, c, rm}, Proj.ps.go(t, on))

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
def Proj.machine.row(+u: Ui) -> List<&2, PItem>:
  Bool.pick(List<&2, PItem>, List.is_empty(&2, String, Proj.machines(u)), Nil{}, [PItem{"On " ++ Proj.on.name(u) ++ " (change)", "proj-on", Proj.next(u)}])

def Proj.query(u: Ui) -> String:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Str.get(dr, "@proj")

def Proj.index(u: Ui) -> Nat:
  match u:
    case Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Maybe.default(&2, Nat, Nat.read(Str.get(dr, "@proj.i")), 0n)

def Proj.set(u: Ui, +q: String, +i: Nat) -> Ui:
  match u:
    case Ui{st, sel, +dr, live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, "@proj", q), "@proj.i", Nat.show(i)), live, op, on, now, ad, pa, ve, rid, w, er, note, nf, ob, tr}

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
  Proj.open.k(String.is_empty(Proj.on(u)), u)

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
def Proj.add(+u: Ui, +dir: String) -> List<&2, PItem>:
  Bool.pick(List<&2, PItem>, Proj.is_project(Proj.ps(u), dir), [PItem{dir ++ " is already a project", "", ""}], [PItem{"Add " ++ dir ++ " as a project", "proj-add", dir}])

# (a match, not Bool.pick: only the list shown is made)
def Proj.items.q(empty: Bool, +u: Ui, +q: String, +dir: String, +ns: List<&2, String>) -> List<&2, PItem>:
  match empty:
    case True{}:
      List.concat(&2, PItem, [Proj.add(u, dir), Bool.pick(List<&2, PItem>, String.eq(dir, "/"), Nil{}, [PItem{"..", "proj-cd", Proj.parent(dir)}]),
        Proj.rows(Proj.scored(ns, q), dir)])
    case False{}:
      List.concat(&2, PItem, [Proj.rows(List.sort(~Scored, ~Scored.before, Proj.scored(ns, q)), dir),
        Bool.pick(List<&2, PItem>, Bool.and(Proj.name_ok(q), Bool.not(Proj.exact(ns, String.trim(q)))),
          [PItem{"Create project '" ++ String.trim(q) ++ "' here", "proj-new", Proj.join(dir, String.trim(q))}, PItem{"Create folder '" ++ String.trim(q) ++ "' here", "proj-mkdir", Proj.join(dir, String.trim(q))}], Nil{}),
        Proj.add(u, dir)])

def Proj.items.k(path: Bool, +u: Ui, +q: String, +dir: String, +ns: List<&2, String>) -> List<&2, PItem>:
  match path:
    case True{}:
      [PItem{"Go to " ++ q, "proj-cd", q}, PItem{"Add " ++ q ++ " as a project", "proj-add", q}, PItem{"Create project " ++ q, "proj-new", q}]
    case False{}:
      Proj.items.q(String.is_empty(q), u, q, dir, ns)

# the folder browser's rows. No filter: add the listed folder (once), the
# parent, the subfolders. A filter: matching subfolders best first, then
# creating the typed name (only when no folder has it), then adding. A
# typed path (/..., ~...) goes there instead
def Proj.folders(+u: Ui) -> List<&2, PItem>:
  Proj.items.k(Bool.or(String.starts_with(Proj.query(u), "/"), String.starts_with(Proj.query(u), "~")), u, Proj.query(u), Proj.dir(u), Proj.entries(u))

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

def Proj.cand.row(+a: String, +ti: String) -> String:
  Bool.pick(String, String.eq(a, "proj-pick"), ti, "Add " ++ ti)

def Proj.list.rows(cs: List<&2, PCand>) -> List<&2, PItem>:
  match cs:
    case Nil{}:
      Nil{}
    case PCand{_, ti, +a, ro} <> t:
      PItem{Proj.cand.row(a, ti), a, ro} <> Proj.list.rows(t)

# the projects matching the filter, best first (live first for a tie)
def Proj.list(+u: Ui, +q: String) -> List<&2, PItem>:
  Proj.list.rows(List.sort(~PCand, ~PCand.before, Proj.rank(Proj.cands(u), q)))

def Proj.browse.row() -> PItem:
  PItem{"Add a project from a folder", "proj-browse", ""}

# (a match, not Bool.pick: only the list shown is made)
def Proj.items.p(path: Bool, empty: Bool, +u: Ui, +q: String) -> List<&2, PItem>:
  match path empty:
    case True{} _:
      Proj.items.k(True{}, u, q, "", Nil{})
    case False{} True{}:
      Proj.browse.row() <> Proj.list(u, q)
    case False{} False{}:
      List.append(&2, PItem, Proj.list(u, q), [Proj.browse.row()])

def Proj.items.m(dir: Bool, +u: Ui) -> List<&2, PItem>:
  match dir:
    case True{}:
      Proj.folders(u)
    case False{}:
      Proj.items.p(Bool.or(String.starts_with(Proj.query(u), "/"), String.starts_with(Proj.query(u), "~")), String.is_empty(Proj.query(u)), u, Proj.query(u))

# what the picker offers. The projects: adding from a folder first, then
# every project (with a filter: those matching, best first, then adding
# from a folder). A typed path goes there, adds it, or creates it. The
# folders: Proj.folders
def Proj.items(+u: Ui) -> List<&2, PItem>:
  List.append(&2, PItem, Proj.items.m(Proj.browsing(u), u), Proj.machine.row(u))

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
def Proj.go(+u: Ui, +root: String) -> Act:
  Proj.go.p(String.is_empty(Proj.id_of(Proj.ps(u), root)), u, Proj.id_of(Proj.ps(u), root))

# the folder browser, where it was last (listed afresh)
def Proj.browse(+u: Ui) -> Act:
  Proj.rpc(Proj.mode(u, "dir"), "fs.list", [J.KV{"path", J.Str{Proj.dir(u)}}])

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

# the picker's actions, in Proj.run.k's order
def Proj.acts() -> List<&2, String>:
  ["proj-add", "proj-cd", "proj-mkdir", "proj-new", "proj-pick", "proj-found", "proj-browse", "proj-back", "proj-on"]

def Proj.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Proj.pos(t, x, 1n+i))

def Proj.run(it: PItem, u: Ui) -> Act:
  match it:
    case PItem{_, +a, +v}:
      Proj.run.k(Proj.pos(Proj.acts(), a, 0n), u, v)

# Enter runs the selected row
def Proj.enter(+u: Ui) -> Act:
  Proj.run(Palette.at(Proj.items(u), Proj.index(u)), u)

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
      Ui.info_set(Ui.info_set(u, "fm.list:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "fmEntries", "")), "fm.dir:" ++ J.Json.get_str(r, "thread", ""), J.Json.get_str(r, "fmDir", ""))
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
      Ui{st, sel, dr, live, op, on, now, ad, True{}, v, rid, w, er, note, Map.set(&2, String, Viewer.put(nf, sel, kind, path), "panel:" ++ sel, "1"), ob, tr}

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

def Fm.ask(+u: Ui, +dir: String) -> Act:
  Ui.on_thread.p(Ui.info_set(Ui.info_set(Ui.view_put(u, "files", ""), "fm.want:" ++ Ui.sel(u), dir), "fm.top:" ++ Ui.sel(u), ""), "files.dir", [J.KV{"path", J.Str{dir}}])

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
      Fm.again.one(Bool.and(String.starts_with(k, "fm.want:"), Bool.not(String.is_empty(Ui.info(u, k)))), k, Act{u, cs})

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
  Fm.open.k(Bool.pick(Nat, Bool.or(String.is_empty(FM.Fm.kind(rel)), String.is_empty(Ui.root(u))), 2n, Bool.pick(Nat, String.eq(FM.Fm.kind(rel), "pdf"), 1n, 0n)), u, FM.Fm.kind(rel), FM.Fm.abs(Ui.root(u), rel))

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
    case 12n:
      Ui.rpc(u, "wake.cancel", [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"id", J.Str{value}}])
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
      FmRow{n, Bool.pick(String, String.is_empty(FM.Fm.kind(n)), "", "fm-open"), FM.Fm.join(dir, n), FM.Fm.size(size), FM.Fm.kind(n)}

def Fm.rows.go(es: List<&2, FM.FEnt>, +dir: String) -> List<&2, FmRow>:
  match es:
    case Nil{}:
      Nil{}
    case e <> t:
      Fm.row(e, dir) <> Fm.rows.go(t, dir)

# whether the listing on screen is the folder asked for
def Fm.ready(+u: Ui) -> Bool:
  Bool.and(String.eq(Ui.info(u, "fm.dir:" ++ Ui.sel(u)), Fm.dir(u)), Bool.not(String.is_empty(Ui.info(u, "fm.dir:" ++ Ui.sel(u)) ++ Ui.info(u, "fm.list:" ++ Ui.sel(u)))))

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
def Fm.trail(+u: Ui) -> List<&2, PItem>:
  PItem{"Project", "fm-cd", ""} <> Fm.trail.go(Bool.pick(List<&2, String>, String.is_empty(Fm.dir(u)), Nil{}, String.split(Fm.dir(u), '/')), "")

# a row's icon (core/icons.bend), the same on every client
def Fm.icon(+kind: String) -> String:
  Bool.pick(String, String.eq(kind, "folder"), "folder", Bool.pick(String, String.eq(kind, "board"), "circuit-board",
    Bool.pick(String, String.eq(kind, "schematic"), "cpu", Bool.pick(String, String.eq(kind, "3d"), "box",
      Bool.pick(String, String.eq(kind, "image"), "image", Bool.pick(String, String.eq(kind, "pdf"), "file-text", "file"))))))

# the tab a kind lights: an image or a PDF is the Files tab's
def Fm.tab(+kind: String) -> String:
  Bool.pick(String, Bool.or(String.eq(kind, "image"), String.eq(kind, "pdf")), "files", kind)

# the first row the window shows of a long folder
def Fm.top(+u: Ui) -> Nat:
  FM.Fm.num(Ui.info(u, "fm.top:" ++ Ui.sel(u)))

# an open file's name, for the image and PDF views' heads
def Fm.name(+u: Ui) -> String:
  FM.Fm.last(Ui.view_get(u, "view.path"))

def Mpage.first.k(m: Maybe<&2, MC.MPart>, +root: String) -> String:
  match m:
    case Some{MC.MPart{+p, _, _}}:
      MC.Mech.abs(root, p)
    case None{}:
      ""

# the part Mech opens on: the assembly, else the first (MC.Mech.default)
def Mpage.first(+ps: List<&2, MC.MPart>, +root: String) -> String:
  Mpage.first.k(MC.Mech.default(ps), root)

# Mech opened before the parts were known: once they are, the first shows
# in 3D ("@mech.to3d:<t>"); with none, the page says so
def Mpage.to3d.k(go: Bool, u: Ui, +th: String, +path: String) -> Ui:
  match go:
    case True{}:
      Ui.view_put.at(Ui.scratch_set(u, "@mech.to3d:" ++ th, ""), th, "3d", path)
    case False{}:
      u

def Mpage.to3d(+u: Ui, +th: String) -> Ui:
  Mpage.to3d.k(Bool.and(String.eq(Ui.scratch(u, "@mech.to3d:" ++ th), "1"), Bool.not(String.is_empty(Mpage.first(MC.Mech.parts(Ui.info(u, "mech:" ++ th)), Ui.info(u, "mech.root:" ++ th))))),
    u, th, Mpage.first(MC.Mech.parts(Ui.info(u, "mech:" ++ th)), Ui.info(u, "mech.root:" ++ th)))

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
def Mpage.render.label(+u: Ui) -> String:
  Bool.pick(String, Bool.not(String.is_empty(Mpage.busy(u))), "Rendering…", Bool.pick(String, Mpage.shots.k(Mpage.cur(u)), "Render again", "Create renders"))

def Mpage.open.k(known: Bool, +u: Ui, +path: String) -> Ui:
  match known:
    case True{}:
      Ui.view_put(u, "3d", path)
    case False{}:
      Ui.view_put(Ui.scratch_set(u, "@mech.to3d:" ++ Ui.sel(u), "1"), "mech", "")

# Mech: the part on show in 3D, once the parts are known
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
      Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, Viewer.put(nf, sel, kind, path), ob, tr}

def Ui.view_pick.back(+u: Ui, +kind: String) -> Ui:
  Ui.view_pick.p(u, kind, Rv.back(u, kind))

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
      Act{Ui.view_put(u, value, Rv.back(u, value)), Nil{}}
    case 7n:
      Act{Ui.view_put(u, "schematic", Rv.sheet(Ui.view_get(u, "view.path"), value)), Nil{}}
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

def Solid.num(+k: String) -> String:
  Bool.pick(String, String.eq(k, "board"), "0", Bool.pick(String, String.eq(k, "schematic"), "1", Bool.pick(String, String.eq(k, "3d"), "2", "")))

def Solid.ask(+k: String) -> String:
  k

def Solid.want(+u: Ui) -> String:
  Bool.pick(String, Bool.or(Bool.or(Bool.not(Ui.panel(u)), String.is_empty(Ui.root(u))), String.is_empty(Solid.num(Solid.kind(u)))), "",
    Solid.num(Solid.kind(u)) ++ "|" ++ Ui.root(u) ++ "|" ++ Ui.view_get(u, "view.path"))

def Solid.sync.k(same: Bool, u: Ui, +want: String, +kind: String, +path: String, +root: String, cmds: List<&2, Cmd>) -> Act:
  match same:
    case True{}:
      Act{u, cmds}
    case False{}:
      Ui.also(Ui.rpc(Ui.scratch_set(u, "@solid.at", want), "kicad.watch",
        [J.KV{"kind", J.Str{Bool.pick(String, String.is_empty(want), "", Solid.ask(kind))}}, J.KV{"root", J.Str{root}}, J.KV{"path", J.Str{path}}]), cmds)

def Solid.sync.m(a: Act) -> Act:
  match a:
    case Act{+u, cmds}:
      Solid.sync.k(String.eq(Solid.want(u), Ui.scratch(u, "@solid.at")), u, Solid.want(u), Solid.kind(u), Ui.view_get(u, "view.path"), Ui.root(u), cmds)

def Solid.sync(a: Act) -> Act:
  Fv.sync(Solid.sync.m(a))

# what the page says under its parts: where to get FreeCAD when the hub
# has none, else nothing
def Mpage.note(+u: Ui) -> String:
  Bool.pick(String, Bool.not(String.is_empty(Ui.info(u, "freecad"))), "", "FreeCAD is not installed on this hub's machine, and agents need it to model parts: get it from freecad.org/downloads.")

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
      Ui.on_thread.p(u, "git.diff", [J.KV{"from", J.Num{"0"}}, J.KV{"to", J.Num{"-1"}}])
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

def Qu.first(+first: Bool, +run: Bool) -> List<&2, QBtn>:
  Bool.pick(List<&2, QBtn>, Bool.and(first, Bool.not(run)), [QBtn{"Resume", "q-resume", ""}], Nil{})

# a queued row's buttons, left to right: while the turn runs, Steer sends
# it into the turn; while none runs (the queue is held, after a stop or a
# restart), Send starts it alone and the first row can resume them all
# (laws queue_row_*)
def Qu.buttons(+run: Bool, +first: Bool, +msg: String) -> List<&2, QBtn>:
  List.append(&2, QBtn, Qu.first(first, run),
    [QBtn{Bool.pick(String, run, "Steer", "Send"), "q-steer", msg}, QBtn{"Edit", "q-edit", msg}, QBtn{"Up", "q-up", msg}, QBtn{"Remove", "q-remove", msg}])

# a queued row as shown: handed-over context (a fork's start, a fork
# brought back, a provider switch) can only be removed; it goes with the
# next message
def Qu.row_buttons(+run: Bool, +first: Bool, +msg: String) -> List<&2, QBtn>:
  Bool.pick(List<&2, QBtn>, String.starts_with(msg, "handoff:"), [QBtn{"Remove", "q-remove", msg}], Qu.buttons(run, first, msg))

def Qu.tag(+run: Bool, +msg: String) -> String:
  Bool.pick(String, String.starts_with(msg, "handoff:"), "Context", Bool.pick(String, run, "Queued", "Held"))

# a queued row's text: context says what it is, a message itself
def Qu.label(+msg: String, +text: String) -> String:
  Bool.pick(String, String.starts_with(msg, "handoff:"), DF.Diff.first(String.drop(text, 12n)) ++ " (goes with the next message)", text)

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
  Todos.pick(TD.Todo.shown(Todos.of(st, th), run), Todos.of(st, th))

# The card's two controls, on all four clients: its head opens or shuts it
# (the "fold" action, key todos:<thread>; open until shut), and a list too
# long for the card ends in a line that shows every step, or the fitted
# few again (key todos.all:<thread>). A client that scrolls the card shows
# every step; the window, which cannot, as many as cap leaves room for.
def Todos.key(+th: String) -> String:
  "todos:" ++ th

def Todos.all.key(+th: String) -> String:
  "todos.all:" ++ th

def Todos.open(+info: Map<&2, String>, +th: String) -> Bool:
  FO.Fold.open(FO.Fold.user(info, Todos.key(th)), True{})

def Todos.all(+info: Map<&2, String>, +th: String) -> Bool:
  FO.Fold.open(FO.Fold.user(info, Todos.all.key(th)), False{})

# what a click on the head asks, and on the last line
def Todos.toggle(+open: Bool, +th: String) -> String:
  FO.Fold.ask(open, Todos.key(th))

def Todos.more.ask(+all: Bool, +th: String) -> String:
  FO.Fold.ask(all, Todos.all.key(th))

# the last line's words: "Show all 24" or "Show fewer", "" when the list fits
def Todos.more(+all: Bool, +ts: List<&2, TD.Todo>) -> String:
  Bool.pick(String, Nat.is_gt(List.length(&2, TD.Todo, ts), TD.Todo.fit()), Bool.pick(String, all, "Show fewer", "Show all " ++ Nat.show(List.length(&2, TD.Todo, ts))), "")

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
def Todos.rows(+info: Map<&2, String>, +th: String, +cap: Nat, +ts: List<&2, TD.Todo>) -> List<&2, TD.TRow>:
  Todos.rows.k(Todos.open(info, th), Todos.all(info, th), cap, ts)

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

def Bots.field(+u: Ui, +k: String) -> String:
  Ui.scratch(u, "@f." ++ k)

def Bots.fields.clear.go(ks: List<&2, String>, u: Ui) -> Ui:
  match ks:
    case Nil{}:
      u
    case +k <> t:
      Bots.fields.clear.go(t, Ui.scratch_set(u, "@f." ++ k, ""))

def Bots.fields.clear(u: Ui, ks: List<&2, String>) -> Ui:
  Bots.fields.clear.go(ks, u)

# the draft (and attachments) of the selected thread cleared
def Bots.say.clear(u: Ui) -> Ui:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, sel, Map.set(&2, String, Map.set(&2, String, dr, sel, ""), "@att:" ++ sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def Bots.say(u: Ui, +bot: String, text: String) -> Act:
  Ui.also(Ui.rpc.keep.msg(Bots.say.clear(u), "bots.say", [J.KV{"bot", J.Str{bot}}, J.KV{"text", J.Str{text}}]), [ScrollEnd{}])

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

def Give.ask(u: Ui, +text: String) -> Act:
  match u:
    case Ui{st, +sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui.also(Ui.rpc(Ui{st, sel, Map.set(&2, String, dr, sel, ""), live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr},
        "bots.give", [J.KV{"thread", J.Str{sel}}, J.KV{"text", J.Str{text}}]), Draft.keep(sel, ""))

def Ui.send.give(give: Bool, +u: Ui, alt: Bool) -> Act:
  match give:
    case True{}:
      Give.ask(u, Ui.draft_text(u))
    case False{}:
      Ui.send.btw(Btw.is(Ui.draft_text(u)), u, alt)

def Ui.send(+u: Ui, alt: Bool) -> Act:
  Ui.send.give(Give.is(Ui.draft_text(u)), u, alt)

def Bots.sel.set(u: Ui, +th: String) -> Ui:
  match u:
    case Ui{st, _, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Ui{st, th, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

# (a bot still awaited, "@bot.want", gives way to what was chosen since)
def Bots.views(u: Ui, +bot: String, +room: String, +remote: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@board", ""), "@bot", bot), "@room", room), "@remote", remote), "@del", ""), "@bot.want", ""), "@thread.want", "")

# a bot shown: its thread is selected, its tab kept
# the thread a bot's Chat tab shows: its own, but the uberbot's is where the
# conversation runs now (ubersync.bend's Ub.chat: the holder's, as mirrored)
def Bots.chat.of(+st: M.State, +id: String, +th: String) -> String:
  Bool.pick(String, UB.Uber.is(st, id), US.Ub.chat(st), th)

def Bots.select.k(m: Maybe<&2, BT.Bot>, +u: Ui) -> Act:
  match m:
    case Some{BT.Bot{+i, _, +th, _, _, _, _}}:
      Act{Ui.flag_off(Bots.views(Bots.sel.set(u, Bots.chat.of(Ui.state.of(u), i, th)), i, "", ""), "settings"), [Focus{"composer"}, ScrollEnd{}]}
    case None{}:
      Act{u, Nil{}}

def Bots.select(+u: Ui, +id: String) -> Act:
  Bots.select.k(BT.World.find.id(BT.World.bot(M.State.bots(Ui.state.of(u)), id)), u)

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

def Notify.rows(+st: M.State) -> List<&2, NotifyRow>:
  [Notify.row(st, "Needs me", "Approvals, questions and anything an agent is blocked on until you act.", "needs"),
   Notify.row(st, "Settled", "Work you gave a thread is finished, failed, or its branch merged. Once per piece of work.", "settled"),
   Notify.row(st, "Reminders", "Reminders and digests you scheduled.", "reminders"),
   Notify.row(st, "Everything", "Also every turn end and room exchange, as before.", "all")]

def Notify.title() -> String:
  "Notify me for"

def Notify.lead() -> String:
  "Phone and desktop alerts. The rest stays in the app as unread."

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

def Mate.note.linked(+n: Nat, away: Bool) -> String:
  match away:
    case True{}:
      "Away: its projects stay listed and readable until it answers."
    case False{}:
      Bool.pick(String, Nat.is_eq(n, 0n), "In sync. Its projects and threads show here as they change.",
        "Catching up: " ++ Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " project", " projects") ++ " still loading. The rest are ready.")

def Mate.note.k(off: Bool, +link: String, +u: Ui) -> String:
  match off:
    case True{}:
      "Off. Nothing syncs with it."
    case False{}:
      Bool.pick(String, String.is_empty(link), "Linking. Its projects will appear here within a minute or two.",
        Mate.note.linked(Mate.behind(Ui.projects(Ui.state(u)), Ui.state(u), link, 0n), String.eq(Ui.info(u, "far.away." ++ link), "1")))

def Mate.row(+n: String, +url: String, +domain: String, +u: Ui) -> MateRow:
  MateRow{n, Mate.note.k(Mate.off(Ui.state(u), n), Mate.link(BT.World.peers(M.State.bots(Ui.state(u))), n, url, domain), u),
    Bool.pick(String, Mate.off(Ui.state(u), n), "off", "on"), Mate.key(n)}

def Mate.rows.k(skip: Bool, +m: J.Json, +domain: String, +u: Ui, rest: List<&2, MateRow>) -> List<&2, MateRow>:
  match skip:
    case True{}:
      rest
    case False{}:
      Mate.row(J.Json.get_str(m, "name", ""), J.Json.get_str(m, "url", ""), domain, u) <> rest

def Mate.rows.go(ms: List<&2, J.Json>, +me: String, +domain: String, +u: Ui) -> List<&2, MateRow>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Mate.rows.k(Bool.or(String.is_empty(J.Json.get_str(m, "url", "")), String.eq(J.Json.get_str(m, "url", ""), me)), m, domain, u, Mate.rows.go(t, me, domain, u))

def Mate.rows.of(+j: J.Json, +u: Ui) -> List<&2, MateRow>:
  Mate.rows.go(J.Json.elems(J.Json.get_obj(j, "peers")), J.Json.get_str(J.Json.get_obj(j, "self"), "url", ""), J.Json.get_str(J.Json.get_obj(j, "self"), "domain", ""), u)

# (the home hub's list: the machines it found)
def Mate.rows(+u: Ui) -> List<&2, MateRow>:
  Mate.rows.of(Maybe.default(&2, J.Json, J.Json.parse(Ui.info(u, "machines")), J.Null{}), u)

def Mate.title() -> String:
  "Your machines"

def Mate.lead(+u: Ui) -> String:
  Bool.pick(String, List.is_empty(&2, MateRow, Mate.rows(u)),
    "Other machines of yours on Tailscale that run Backplane show here. Their projects and threads sync into this sidebar by themselves.",
    "Projects and threads on these machines sync into this sidebar in the background, so you never have to switch. Turn one off to keep it apart.")

def Far.chat(u: Ui, +id: String) -> Act:
  Act{Bots.views(u, "", "", id), [Focus{"remote-send"}, ScrollEnd{}]}

# "name\u001ftext": a form field
def Bots.bfield.k(ps: List<&2, String>, u: Ui) -> Ui:
  match ps:
    case k <> t:
      Ui.scratch_set(u, "@f." ++ k, String.join(t, "\u{1f}"))
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

def Bots.create(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(Ui.scratch_set(u, "@bnew", ""), "@uber.new", ""), ["name", "persona"]), "bots.create",
    List.append(&2, J.KV, [J.KV{"name", J.Str{String.trim(Bots.field(u, "name"))}}, J.KV{"persona", J.Str{Bots.field(u, "persona")}},
     J.KV{"provider", J.Str{Bool.pick(String, String.is_empty(Bots.field(u, "provider")), M.Setting.get(Ui.state.of(u), "agent.provider", "claude"), Bots.field(u, "provider"))}},
     J.KV{"model", J.Str{Bots.field(u, "model")}}], Bots.uber.new(u)))

# the form's name field hint
def Bots.name.hint(+u: Ui) -> String:
  Bool.pick(String, String.eq(Ui.scratch(u, "@uber.new"), "1"), "Name your uberbot", "Name")

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
      Ui.also(Ui.rpc.keep.msg(Ui.scratch_set(u, "@f.rpost", ""), "bots.post", [J.KV{"room", J.Str{Ui.scratch(u, "@room")}}, J.KV{"text", J.Str{Bots.field(u, "rpost")}}]), [ScrollEnd{}])

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

def Bots.routine.save(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(u, "@onew", ""), ["oid", "oname", "ocron", "oprompt"]), "bots.routine",
    [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"routine", J.Str{Bots.field(u, "oid")}}, J.KV{"name", J.Str{Bool.pick(String, String.is_empty(Bots.field(u, "oname")), "routine", Bots.field(u, "oname"))}},
     J.KV{"cron", J.Str{Bots.field(u, "ocron")}}, J.KV{"prompt", J.Str{Bots.field(u, "oprompt")}}, J.KV{"on", J.Flag{True{}}}])

def Bots.routine.of(+u: Ui, +id: String) -> BT.Routine:
  BT.World.routine(M.State.bots(Ui.state.of(u)), id)

def Bots.routine.toggle(+u: Ui, r: BT.Routine) -> Act:
  match r:
    case BT.Routine{i, b, n, c, p, on, _, _, _}:
      Ui.rpc(u, "bots.routine", [J.KV{"bot", J.Str{b}}, J.KV{"routine", J.Str{i}}, J.KV{"name", J.Str{n}}, J.KV{"cron", J.Str{c}}, J.KV{"prompt", J.Str{p}}, J.KV{"on", J.Flag{Bool.not(on)}}])

# a routine's fields into the form, to change it
def Bots.routine.edit(u: Ui, r: BT.Routine) -> Ui:
  match r:
    case BT.Routine{i, _, n, c, p, _, _, _, _}:
      Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@onew", "1"), "@f.oid", i), "@f.oname", n), "@f.ocron", c), "@f.oprompt", p)

def Bots.hook.new(+u: Ui) -> Act:
  Ui.rpc(Bots.fields.clear(Ui.scratch_set(u, "@hook.secret", ""), ["hname"]), "bots.hook",
    [J.KV{"bot", J.Str{Bots.cur(u)}}, J.KV{"name", J.Str{Bool.pick(String, String.is_empty(Bots.field(u, "hname")), "webhook", Bots.field(u, "hname"))}}])

# a webhook's bearer token and the URL to post to, as the server answered
def Bots.token.clear(u: Ui) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@hook.token", ""), "@hook.tokfor", ""), "@hook.tokurl", "")

# show (made the first time), "new" or "off"
def Bots.token(u: Ui, +hook: String, +op: String) -> Act:
  Ui.rpc(Bots.token.clear(u), "bots.hook_token", [J.KV{"hook", J.Str{hook}}, J.KV{"op", J.Str{op}}])

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

def Bots.google.op(i: Nat, +u: Ui, +op: String) -> Act:
  match i:
    case 0n:
      Bots.google.rpc(u, "disconnect", String.drop(op, 8n))
    case 1n:
      Bots.google.rpc.x(u, "connect", "", "", Bots.google.loop(String.drop(op, 8n)))
    case 2n:
      Bots.google.rpc.x(u, "finish", "", String.drop(op, 5n), "")
    case _:
      Bots.google.rpc(u, op, "")

# "signout:<address>" signs one account out; "disconnect" signs out every one
def Bots.google.at(+op: String) -> Nat:
  Bool.pick(Nat, String.starts_with(op, "signout:"), 0n,
    Bool.pick(Nat, String.starts_with(op, "connect@"), 1n, Bool.pick(Nat, String.starts_with(op, "done:"), 2n, 3n)))

def Bots.google(+u: Ui, +op: String) -> Act:
  Bots.google.op(Bots.google.at(op), u, op)

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

def Hs.row.steps(ss: List<&2, HI.Step>, +side: String, +es: List<&2, M.Entry>, +i: Nat, acc: List<&2, HRow>) -> List<&2, HRow>:
  match ss:
    case Nil{}:
      acc
    case HI.Step{+id, _, +c, _, +g, +top, _, _} <> t:
      Hs.row.steps(t, side, es, 1n+i, HRow{"Step " ++ Nat.show(i), HI.Hist.label(es, HI.Hist.anchor(es, HI.Step{id, "", c, "", g, top, Nil{}, 0n})), Hs.row.value(side, c, "Step " ++ Nat.show(i), g, top)} <> acc)

def Hs.ref.label(+r: HI.Ref) -> String:
  match r:
    case HI.Ref{+k, +l, _}:
      Bool.pick(String, String.eq(k, "turn"), "Turn " ++ HI.Hist.at(List.reverse(&2, String, String.split(l, '/')), 0n),
        Bool.pick(String, String.eq(k, "base"), "Branch point · " ++ l, l))

# (a base is where the checked-out branch left the main one: compared with
# it, a pull request's whole change; a tag is a release)
def Hs.ref.kind(+k: String) -> String:
  Bool.pick(String, String.eq(k, "head"), "the commit checked out", Bool.pick(String, String.eq(k, "branch"), "branch", Bool.pick(String, String.eq(k, "turn"), "this thread, at the end of a turn",
    Bool.pick(String, String.eq(k, "tag"), "tag", Bool.pick(String, String.eq(k, "base"), "where this branch left it", "commit")))))

# (turn 0 is the thread before it began)
def Hs.ref.sub(+k: String, +label: String) -> String:
  Bool.pick(String, String.eq(label, "Turn 0"), "this thread, before it began", Hs.ref.kind(k))

def Hs.row.refs(rs: List<&2, HI.Ref>, +side: String, +git: String, +top: String) -> List<&2, HRow>:
  match rs:
    case Nil{}:
      Nil{}
    case HI.Ref{+k, +l, +c} <> t:
      HRow{Hs.ref.label(HI.Ref{k, l, c}), Hs.ref.sub(k, Hs.ref.label(HI.Ref{k, l, c})), Hs.row.value(side, c, Hs.ref.label(HI.Ref{k, l, c}), git, top)} <> Hs.row.refs(t, side, git, top)

# the newest steps the menu lists (older ones are on the scrubber), so
# the branches and commits after them stay in reach
def Hs.menu.steps() -> Nat:
  8n

# the menu's rows for a side ("a" the old version, "b" the new)
def Hs.menu(+u: Ui, +side: String) -> List<&2, HRow>:
  +r = Hs.refs(u)
  +es = M.State.mine(Ui.state(u), Ui.sel(u))
  List.append(&2, HRow, [HRow{"Working copy", "the files as they are now", Hs.row.value(side, "", "Working copy", "", "")}],
    List.append(&2, HRow, List.take(&2, HRow, Hs.row.steps(Hs.track(u), side, es, 1n, Nil{}), Hs.menu.steps()),
      Hs.row.refs(HI.Hist.refs(J.Json.get_str(r, "refs", "")), side, J.Json.get_str(r, "git", ""), J.Json.get_str(r, "top", ""))))

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
      Ui.error_set(u, "This file is not in the repository, so it cannot be compared.")
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

def Rp.label(+page: String) -> String:
  Bool.pick(String, String.eq(page, "bom"), "BOM", Bool.pick(String, String.eq(page, "checks"), "Checks", Bool.pick(String, String.eq(page, "comments"), "Comments", Bool.pick(String, String.eq(page, "release"), "Release", page))))

# the page on show ("" when the tab is not)
def Rp.page(+u: Ui) -> String:
  Bool.pick(String, String.eq(Ui.view_get(u, "view.kind"), "review"), Bool.pick(String, String.is_empty(Ui.view_get(u, "view.path")), "bom", Ui.view_get(u, "view.path")), "")

def Rp.key(+u: Ui, +k: String, +page: String) -> String:
  k ++ ":" ++ Ui.sel(u) ++ ":" ++ page

# a comparison picked (and not closed) is what the BOM page compares
def Rp.compared(+u: Ui) -> Bool:
  Bool.not(String.is_empty(Hs.get(u, "cmp.a")))

def Rp.bom.title(+u: Ui) -> String:
  Bool.pick(String, Rp.compared(u),
    "BOM change · " ++ Hs.get(u, "cmp.al") ++ " → " ++ Bool.pick(String, String.is_empty(Hs.get(u, "cmp.bl")), "Working copy", Hs.get(u, "cmp.bl")),
    "BOM · Working copy")

def Rp.ask.bom(+u: Ui) -> Act:
  Ui.rpc(Ui.info_set(Ui.scratch_set(u, "@rp.busy:" ++ Ui.sel(u), "bom"), Rp.key(u, "rp.title", "bom"), Rp.bom.title(u)), "review.bom",
    [J.KV{"thread", J.Str{Ui.sel(u)}}, J.KV{"a", J.Str{Bool.pick(String, Rp.compared(u), Hs.get(u, "cmp.a"), "")}}, J.KV{"b", J.Str{Bool.pick(String, Rp.compared(u), Hs.get(u, "cmp.b"), "")}}])

# the design checks run on the working copy, or on the new side of a
# comparison when that is a commit
def Rp.commit(+u: Ui) -> String:
  Bool.pick(String, Nat.is_eq(HI.Hist.kind(Hs.get(u, "cmp.b")), 1n), HI.Hist.git.commit(Hs.get(u, "cmp.b")), "")

def Rp.checks.title(+u: Ui) -> String:
  "Checks · " ++ Bool.pick(String, String.is_empty(Rp.commit(u)), "Working copy", Hs.get(u, "cmp.bl"))

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

def Rp.tabs(ps: List<&2, String>, +cur: String) -> List<&2, RpTab>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      RpTab{p, Rp.label(p), String.eq(p, cur)} <> Rp.tabs(t, cur)

# (the Comments page is the thread's own: no hub report)
def Rp.view.k(comments: Bool, +u: Ui, +page: String) -> RpView:
  match comments:
    case True{}:
      RpView{page, Rp.tabs(Rp.pages(), page), "Comments · " ++ Nat.show(Rc.count(u)) ++ " open", RVW.Review.md(Rc.all(u)), "", False{}}
    case False{}:
      RpView{page, Rp.tabs(Rp.pages(), page), Bool.pick(String, String.eq(page, "release"), "Tag " ++ Rl.tag(u), Ui.info(u, Rp.key(u, "rp.title", page))), Ui.info(u, Rp.key(u, "rp.md", page)), Ui.info(u, Rp.key(u, "rp.err", page)),
        String.eq(Ui.scratch(u, "@rp.busy:" ++ Ui.sel(u)), page)}

def Rp.view(+u: Ui) -> RpView:
  Rp.view.k(String.eq(Rp.page(u), "comments"), u, Rp.page(u))

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
def RpView.note(+v: RpView) -> String:
  Bool.pick(String, RpView.busy(v), "Reading the design…",
    Bool.pick(String, String.is_empty(RpView.err(v)), Bool.pick(String, String.is_empty(RpView.md(v)), Bool.pick(String, String.eq(RpView.page(v), "comments"), "No comments yet. Right-click a place on the board or schematic (long-press on a phone) to comment there.",
      Bool.pick(String, String.eq(RpView.page(v), "release"), "Tag this design and make its fabrication outputs: Gerbers, drill, CPL, BOM, PDFs and STEP, in .backplane/releases.", "Nothing yet.")), ""), RpView.err(v)))

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
  Rp.reply(Rp.reply.has(o), Rv.reply(Rv.reply.has(o), Fv.reply(Fm.has(J.Json.get_obj(o, "result"), "farView"), Skills.got(Skills.has(o), Mpage.reply(Mpage.has(o), Hs.refs.store(Hs.refs.has(o), Ui.reply.body(u, o), o), o), o), J.Json.get_obj(o, "result")), o), o)

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
def Hs.live.said(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "The live file. Compare it with any commit, branch or tag.", "The live file. Each tick is a change the agent made; play to watch them.")

def Hs.said.line(+s: String) -> String:
  HI.Hist.line(s)

def Hs.bar.k(m: Maybe<&2, HI.Step>, +u: Ui, +n: Nat, +at: Nat, +es: List<&2, M.Entry>) -> HBar:
  match m:
    case Some{+s}:
      HBar{True{}, n, at, "Step " ++ Nat.show(at) ++ " of " ++ Nat.show(n) ++ Bool.pick(String, HI.Step.first(s), " · before the agent's edits", ""),
        HI.Hist.anchor(es, s), HI.Hist.label(es, HI.Hist.anchor(es, s)), Hs.said.line(HI.Hist.said(es, HI.Hist.anchor(es, s))),
        Hs.playing(u), Hs.diff(u), False{}, "", "", Ui.scratch(u, "@cmp.menu")}
    case None{}:
      HBar{True{}, n, 0n, Bool.pick(String, Hs.cmp(u), "Comparing", Bool.pick(String, Nat.is_eq(n, 0n), "Live", "Live · " ++ Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " step", " steps"))),
        "", "", "", Hs.playing(u), Hs.diff(u), Hs.cmp(u), Hs.get(u, "cmp.al"), Bool.pick(String, String.is_empty(Hs.get(u, "cmp.bl")), "Working copy", Hs.get(u, "cmp.bl")), Ui.scratch(u, "@cmp.menu")}

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

def Hs.bar.on(on: Bool, +u: Ui) -> HBar:
  match on:
    case True{}:
      Hs.bar.k(HI.Hist.nth(Hs.track(u), Hs.at(u, Hs.track(u))), u, Hs.n(Hs.track(u)), Hs.at(u, Hs.track(u)), M.State.mine(Ui.state(u), Ui.sel(u)))
    case False{}:
      HBar{False{}, 0n, 0n, "", "", "", "", False{}, False{}, False{}, "", "", ""}

# the bar for the selected thread's viewer when it shows (on): off unless
# a board or a schematic shows
def Hs.bar.for(+u: Ui, on: Bool) -> HBar:
  Hs.bar.on(Bool.and(on, Bool.and(Nat.is_lt(Hs.kind(u), 2n), Bool.not(String.is_empty(Ui.sel(u))))), u)

# (the window's and the web's: while the panel is open)
def Hs.bar(+u: Ui) -> HBar:
  Hs.bar.for(u, Ui.panel(u))

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
# is sent), forgets one ("key-del" with its name), and lists the names the
# hub answers (@keys; opening Settings asks). Flags "keyform" and "keys"
# open the form and the list.
def ApiKeys.names(+u: Ui) -> List<&2, String>:
  AK.Keys.keep(String.lines(Ui.scratch(u, "@keys")))

def ApiKeys.add(+u: Ui) -> Act:
  Voice.rpc(Ui.flag_off(Ui.scratch_set(Ui.scratch_set(u, "@f.kval", ""), "@f.kname", ""), "keyform"), "api-add",
    [J.KV{"name", J.Str{Ui.scratch(u, "@f.kname")}}, J.KV{"value", J.Str{Ui.scratch(u, "@f.kval")}}])

def ApiKeys.del(+u: Ui, +name: String) -> Act:
  Voice.rpc(u, "api-del", [J.KV{"name", J.Str{name}}])

# what Settings says about them (every client)
def ApiKeys.note(+u: Ui) -> String:
  Bool.pick(String, List.is_empty(&2, String, ApiKeys.names(u)), "Keys for the services agents use, kept in this machine's secrets. Agents use them without ever reading them. None yet.",
    "Agents use these without ever reading them: " ++ Nat.show(List.length(&2, String, ApiKeys.names(u))) ++ " kept.")

def ApiKeys.form.note() -> String:
  "Agents find a key by its name, e.g. OPENAI_API_KEY. It is never shown again."

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
def Agv.install.note(+u: Ui) -> String:
  Bool.pick(String, Agv.busy(u, "install"), J.Json.get_str(Agv.job(u), "msg", ""),
    Bool.pick(String, String.eq(J.Json.get_str(Agv.job(u), "what", ""), "install"), J.Json.get_str(Agv.job(u), "msg", "") ++ " ", "")
      ++ Bool.pick(String, Bool.not(String.is_empty(Agv.have(u))), "Google's own agent, Antigravity " ++ Agv.have(u) ++ ", installed. Backplane runs it; it signs in by itself.",
        Bool.pick(String, Bool.not(String.is_empty(J.Json.get_str(Agv.info(u), "old", ""))), "Antigravity " ++ J.Json.get_str(Agv.info(u), "old", "") ++ " is installed; Backplane now pins " ++ AGY.Agy.version() ++ ".",
          Bool.pick(String, J.Json.get_bool(Agv.info(u), "plat", True{}), "Not installed. Backplane downloads Google's agent (agy_acp_server " ++ AGY.Agy.version() ++ ", from the ACP registry, a few hundred MB) and checks its pinned SHA-256.",
            "Google has no Antigravity build for this machine."))))

def Agv.install.btns(+u: Ui) -> List<&2, AgBtn>:
  Bool.pick(List<&2, AgBtn>, Bool.or(Agv.busy(u, "install"), Bool.not(J.Json.get_bool(Agv.info(u), "plat", True{}))), Nil{},
    [AgBtn{Bool.pick(String, String.is_empty(Agv.have(u)), Bool.pick(String, String.is_empty(J.Json.get_str(Agv.info(u), "old", "")), "Install", "Update to " ++ AGY.Agy.version()), "Reinstall"),
      "agy-install", "", String.is_empty(Agv.have(u))}])

def Agv.methods(ms: List<&2, String>, +cur: String) -> List<&2, AgBtn>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      AgBtn{AGY.Agy.label(m), "setting", "agy.auth=" ++ m, String.eq(m, cur)} <> Agv.methods(t, cur)

# the key the method reads, from the API keys (names only)
def Agv.key.row(+u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, String.is_empty(AGY.Agy.key(Agv.method(u))), Nil{},
    [AgRow{AGY.Agy.key(Agv.method(u)), Bool.pick(String, Agv.has(ApiKeys.names(u), AGY.Agy.key(Agv.method(u))), "Saved in API keys. Check it to list the models it offers.",
        "Not saved yet. It goes to this machine's API keys file; no client shows it again."),
      [AgField{"agykey", "Paste the key", True{}, Ui.scratch(u, "@f.agykey"), "agy-key"}],
      [AgBtn{"Save key", "agy-key", "", False{}}, AgBtn{"Check", "agy-signin", "", True{}}]}])

def Agv.gcp.note(+p: String, +l: String) -> String:
  Bool.pick(String, Bool.and(String.is_empty(p), String.is_empty(l)), "Not set.", "Project " ++ AGY.Agy.or(p, "not set") ++ ", location " ++ AGY.Agy.or(l, "not set") ++ ".")

def Agv.gcp.row(+u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, Bool.not(AGY.Agy.gcp(Agv.method(u))), Nil{},
    [AgRow{"Google Cloud project", Agv.gcp.note(M.Setting.get(Ui.state.of(u), "agy.gcp.project", ""), M.Setting.get(Ui.state.of(u), "agy.gcp.location", "")),
      [AgField{"agyproj", "Project ID", False{}, Ui.scratch(u, "@f.agyproj"), "agy-gcp"}, AgField{"agyloc", "Location, e.g. us-central1", False{}, Ui.scratch(u, "@f.agyloc"), "agy-gcp"}],
      [AgBtn{"Save", "agy-gcp", "", False{}}]}])

def Agv.acct.btns(xs: List<&2, String>, +cur: String) -> List<&2, AgBtn>:
  match xs:
    case Nil{}:
      Nil{}
    case +a <> t:
      AgBtn{a, "setting", "agy.account=" ++ a, String.eq(a, cur)} <> Agv.acct.btns(t, cur)

def Agv.accts.shown(+u: Ui) -> List<&2, String>:
  Bool.pick(List<&2, String>, Agv.has(Agv.accts(u), Agv.acct(u)), Agv.accts(u), List.append(&2, String, Agv.accts(u), [Agv.acct(u)]))

def Agv.google.row(+u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, Bool.not(AGY.Agy.browser(Agv.method(u))), Nil{},
    [AgRow{"Account", Agv.acct(u) ++ Bool.pick(String, Agv.signed(u), ": signed in. The agent keeps its own sign-in; Backplane never sees it.", ": not signed in."),
      [AgField{"agyacct", "Another account's name", False{}, Ui.scratch(u, "@f.agyacct"), "agy-acct"}],
      List.concat(&2, AgBtn, [Agv.acct.btns(Agv.accts.shown(u), Agv.acct(u)), [AgBtn{"Add account", "agy-acct", "", False{}}, AgBtn{"Sign in with Google", "agy-signin", "", True{}}],
        Bool.pick(List<&2, AgBtn>, Agv.signed(u), [AgBtn{"Sign out", "agy-signout", Agv.acct(u), False{}}], Nil{}),
        Bool.pick(List<&2, AgBtn>, Agv.has(Agv.accts(u), Agv.acct(u)), [AgBtn{"Remove", "agy-forget", Agv.acct(u), False{}}], Nil{})])}])

def Agv.job.k(i: Nat, +u: Ui, +j: J.Json) -> List<&2, AgRow>:
  match i:
    case 0n:
      [AgRow{"Sign-in", J.Json.get_str(j, "msg", ""), Nil{}, [AgBtn{"Cancel", "agy-cancel", "", False{}}]}]
    case 1n:
      [AgRow{"Sign-in", J.Json.get_str(j, "msg", ""), [AgField{"agyback", "Paste the address Google sent you to", False{}, Ui.scratch(u, "@f.agyback"), "agy-back"}],
        [AgBtn{"Open Google's sign-in", "open-url", J.Json.get_str(j, "url", ""), True{}}, AgBtn{"Continue", "agy-back", "", False{}}, AgBtn{"Cancel", "agy-cancel", "", False{}}]}]
    case 2n:
      [AgRow{"Sign-in", J.Json.get_str(j, "msg", ""), Nil{}, Nil{}}]
    case 3n:
      [AgRow{"Sign-in failed", J.Json.get_str(j, "msg", ""), Nil{}, Nil{}}]
    case _:
      Nil{}

# the sign-in in progress (or just finished): what it says, its link and
# the field for Google's way back
def Agv.job.row(+u: Ui) -> List<&2, AgRow>:
  Bool.pick(List<&2, AgRow>, Bool.or(String.eq(J.Json.get_str(Agv.job(u), "what", ""), "signin"), String.eq(J.Json.get_str(Agv.job(u), "what", ""), "signout")),
    Agv.job.k(C.Codec.pos(["running", "waiting", "done", "failed"], J.Json.get_str(Agv.job(u), "state", "")), u, Agv.job(u)), Nil{})

# every row, in order
def Agv.rows(+u: Ui) -> List<&2, AgRow>:
  List.concat(&2, AgRow, [
    [AgRow{"Antigravity", Agv.install.note(u), Nil{}, Agv.install.btns(u)},
     AgRow{"Sign in with", AGY.Agy.note(Agv.method(u)), Nil{}, Agv.methods(AGY.Agy.methods(), Agv.method(u))}],
    Agv.key.row(u), Agv.gcp.row(u), Agv.google.row(u), Agv.job.row(u)])

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

# an "agy-" action
def Agv.act(+u: Ui, +action: String, +value: String) -> Act:
  Agv.act.k(C.Codec.pos(["agy-install", "agy-signin", "agy-back", "agy-cancel", "agy-signout", "agy-forget", "agy-key", "agy-gcp", "agy-acct"], action), u, value)

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
      Voice.rpc(u, "unkey", Nil{})
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

# opening Settings asks for the voice settings; opening or closing it
# puts the pairing code away
def Voice.flag.k(ask: Bool, a: Act) -> Act:
  match ask a:
    case True{} Act{u, cs}:
      Ui.also(Voice.rpc(Ui.flag_off(u, "pairqr"), "status", Nil{}), cs)
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
  Voice.reply(o, Bots.reply.has(J.Json.get(o, "apiKeys"), "@keys", Bots.reply.has(J.Json.get(o, "googleAccounts"), "@google.accts", Bots.reply.put(o, "googleUrl", "@google.url", Bots.reply.put(o, "google", "@google", Bots.reply.put(o, "invite", "@peer.invite",
    Bots.reply.put(o, "secret", "@hook.secret", Bots.reply.put(o, "hook", "@hook.id",
      Bots.reply.put(o, "token", "@hook.token", Bots.reply.put(o, "tokenHook", "@hook.tokfor", Bots.reply.put(o, "tokenUrl", "@hook.tokurl", Bots.reply.bot(o, u))))))))))))

# A thread request uses the same binding as a bot request.
def Want.thread(+u: Ui, id: String) -> Ui:
  Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@bot.want", ""), "@thread.want", id), "@want.hub", Ui.info(u, "local.machine")), "@want.gen", Ui.info(u, "local.generation"))

def Made.got(go: Bool, u: Ui, +th: String) -> Ui:
  match go:
    case True{}:
      Ui.scratch_set(Want.thread(u, th), "@made.far", Bool.pick(String, FR.Fr.is(th), th, ""))
    case False{}:
      u

def Made.take(mine: Bool, +o: J.Json, +u: Ui) -> Ui:
  match mine:
    case True{}:
      Made.got(Bool.and(Bool.and(J.Json.get_bool(o, "ok", False{}), Bool.not(String.is_empty(J.Json.get_str(o, "thread", "")))), String.eq(Ui.scratch(u, "@want.new.at"), Made.at(u))),
        Ui.scratch_set(Ui.scratch_set(u, "@want.new", ""), "@want.new.at", ""), J.Json.get_str(o, "thread", ""))
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
def RulesUi.reply.k(mine: Bool, ok: Bool, +u: Ui, +o: J.Json) -> Ui:
  match mine ok:
    case True{} False{}:
      Ui.flag_on(Ui.scratch_set(Ui.scratch_set(u, "@rule.rid", ""), "@rule.err", Ui.reply.err(o)), "ruleform")
    case True{} True{}:
      Ui.scratch_set(u, "@rule.rid", "")
    case _ _:
      u

def RulesUi.reply(+u: Ui, +o: J.Json) -> Ui:
  RulesUi.reply.k(Bool.and(Bool.not(String.is_empty(Ui.scratch(u, "@rule.rid"))), String.eq(Ui.scratch(u, "@rule.rid"), J.Json.as_raw(J.Json.get(o, "id"), ""))), J.Json.get_bool(o, "ok", True{}), u, o)

def Ui.reply(u: Ui, +o: J.Json) -> Ui:
  RulesUi.reply(Made.reply(o, Bots.reply(o, Proj.reply(o, Btw.reply(Ui.reply.skills(u, o), o)))), o)

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

def Bots.wanted.m(m: Maybe<&2, BT.Bot>, u: Ui) -> Ui:
  match m:
    case Some{BT.Bot{+i, _, +th, _, _, _, _}}:
      Bots.views(Bots.sel.set(u, th), i, "", "")
    case None{}:
      u

def Want.thread.k(found: Bool, u: Ui, id: String) -> Ui:
  match found:
    case True{}:
      Ui.scratch_set(Bots.views(Bots.sel.set(u, id), "", "", ""), "@thread.want", "")
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

def Ui.adopt_log(u: Ui, st: M.State, o: J.Json) -> Ui:
  Ui.adopt(Ui.reset(u, o), st)

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

# a message folded in, and the join's progress noted
def Ui.recv.b(+i: Nat, u: Ui, +o: J.Json) -> Ui:
  Boot.mark(i, Ui.recv.k(i, u, o), o)

# A message can resolve a want only against the installed destination.
def Ui.recv(+u: Ui, +o: J.Json) -> Ui:
  Ui.reselect(Ui.sel(u), Bots.wanted(Ui.join(String.eq(J.Json.get_str(o, "t", ""), "log"), Ui.recv.b(C.Codec.pos(["log", "changes", "delta", "reply", "update", "view", "term", "info", "voice", "far", "live"], J.Json.get_str(o, "t", "")), u, o))))

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
      HubCache.put(HubView{Ui.info(u, "local.machine"), Ui.state.of(u), Ui.infos(u)}, vs)
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

# (the uberbot with no tab picked shows its Status, not its chat)
def Timeline.bot(m: Maybe<&2, BT.Bot>, +u: Ui) -> Bool:
  match m:
    case Some{BT.Bot{+bid, _, +th, _, _, _, _}}:
      Bool.or(Bool.not(Bots.chat.is(Ui.state.of(u), bid, th, Ui.sel(u))), Bool.or(Bool.and(String.is_empty(Ui.scratch(u, "@btab")), Bool.not(UB.Uber.is(Ui.state.of(u), bid))), String.eq(Ui.scratch(u, "@btab"), "chat")))
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

# (a heartbeat, {"t":"hb"}, draws nothing: law hb_shows_nothing)
def Ui.shows(+u: Ui, +o: J.Json) -> Bool:
  Ui.shows.t(String.eq(J.Json.get_str(o, "t", ""), "hb"), u, o)

# a message from the server, with what to send: a (re)connection (the
# server's log arrives first on each) sends the outbox again, so nothing
# the user sent is lost to a dropped link
def Ui.recv_act.go(u: Ui, +o: J.Json) -> Act:
  Ui.recv_act.t(Bool.and(String.eq(J.Json.get_str(o, "t", ""), "term"), Bool.not(EM.Emb.is_key(J.Json.get_str(o, "thread", "")))), J.Json.get_str(o, "thread", ""), Fm.again(String.eq(J.Json.get_str(o, "t", ""), "log"), Ui.recv_act.k(String.eq(J.Json.get_str(o, "t", ""), "log"), Ui.recv(u, o))))

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

# a desk's host calls this after its input, focus changes and messages
def Ui.desk(+u: Ui, +focused: Bool, +now: Nat) -> Act:
  Ui.desk.k(DK.Desk.due(Ui.scratch(u, "@desk"), Ui.scratch(u, "@desk.at"), Ui.sel(u), focused, now), u, Ui.sel(u), focused, now)

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

def Bots.act(i: Nat, +u: Ui, +value: String) -> Act:
  match i:
    case 0n:
      Bots.select(u, value)
    case 1n:
      Bots.tab.k(String.eq(value, "settings"), Ui.scratch_set(u, "@btab", value))
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
      Ui.rpc(u, "bots.routine_delete", [J.KV{"routine", J.Str{value}}])
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

def Ui.act.k(i: Nat, u: Ui, +value: String) -> Act:
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
      Voice.flag(value, Ui.toggle_settled(u, value))
    case 19n:
      Ui.setting(u, value)
    case 20n:
      Ui.model(u, value)
    case 21n:
      Ui.rpc(u, "kicad.install", Nil{})
    case 22n:
      Ui.answer(u, value)
    case 23n:
      Ui.on_thread.p(u, "git.act", [J.KV{"action", J.Str{value}}])
    case 24n:
      Ui.on_thread.p(u, "git.diff", [J.KV{"from", J.Num{"0"}}, J.KV{"to", J.Num{"-1"}}])
    case 25n:
      Ui.on_thread.p(u, "git.revert", [J.KV{"turn", J.Num{Bool.pick(String, String.is_empty(value), "0", value)}}])
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
      Proj.enter(u)
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
      Ui.on_thread(u, "kicad.open")
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
      Ui.fork(u)
    case 76n:
      Ui.on_thread(u, "thread.back")
    case _:
      Bots.act(Nat.sub(i, 77n), u, value)

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
      Ec.make(tr, st, th, Side.now(u))

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
      EM.Emb.pins.go(M.State.mine(st, th), st, last, now)

# Terminals in the chat (core/emb.bend)
# -------------------------------------
# A live one the selected thread shows and this client holds no screen
# for is asked for once (term.snap, scratch "@emb.asked:<key>"); keys and
# pastes go to it by key; a click on an inactive one starts it again; the
# pin is the setting "emb.pin:<key>".

def Emb.has(+u: Ui, +key: String) -> Bool:
  match u:
    case Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, tr}:
      Emb.has.k(Map.has(&2, VT.Vt, tr, key))

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

# a desktop notification ({"t":"notify"}, sent only to a desk)
def Ui.recv_act.n(notify: Bool, u: Ui, +o: J.Json) -> Act:
  match notify:
    case True{}:
      Act{u, [Notify{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "title", ""), J.Json.get_str(o, "body", ""), J.Json.get_str(o, "key", "")}]}
    case False{}:
      Emb.sync.if(C.Codec.pos(["log", "changes", "view"], J.Json.get_str(o, "t", "")), Mpage.sync(Ui.recv_act.go(u, o)))

def Ui.recv_act(u: Ui, +o: J.Json) -> Act:
  Ui.recv_act.n(String.eq(J.Json.get_str(o, "t", ""), "notify"), u, o)

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

# The uberbot step alone: an install that finished the tour before it had
# that step (or demoted its uberbot) is asked once more, until the step has
# been answered (the hub setting "onboard.uber": Done, Skip or a choice, in
# the full tour too). Only while no uberbot is set, only for a tour that came
# up by itself, and (like it) only once this connection's state has arrived.
def Tour.answered(+st: M.State) -> Bool:
  String.eq(M.Setting.get(st, "onboard.uber", ""), "1")

def Tour.single.k(+st: M.State, +raw: String) -> Bool:
  Bool.and(Bool.and(Tour.done(st), Bool.not(Tour.answered(st))), Bool.and(String.is_empty(UB.Uber.bot(st)), Bool.not(OB.Ob.manual(raw))))

def Tour.single(+u: Ui) -> Bool:
  Bool.and(Tour.ready(u), Tour.single.k(Ui.state.of(u), Ui.scratch(u, "@tour")))

def Tour.at.k(+i: Nat, up: Bool) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_eq(i, OB.Ob.count()), up), OB.Ob.uber(), i)

def Tour.at(+u: Ui) -> Nat:
  Tour.at.k(OB.Ob.at(Tour.ready(u), Tour.done(Ui.state(u)), Ui.scratch(u, "@tour")), Tour.single(u))

# what the card says: no "10 of 10" for a step asked alone
def Tour.counter(+u: Ui) -> String:
  Bool.pick(String, Tour.single(u), "", OB.Ob.counter(Tour.at(u)))

# whether the card has a Back (not the first step, not a step alone)
def Tour.back(+u: Ui) -> Bool:
  Bool.and(Nat.is_gt(Tour.at(u), 0n), Bool.not(Tour.single(u)))

def Tour.on(+u: Ui) -> Bool:
  OB.Ob.live(Tour.at(u))

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

def Tour.pick.one(first: Bool, +b: BT.Bot) -> TChoice:
  TChoice{Bool.pick(String, first, "Make " ++ BT.Bot.name(b) ++ " my uberbot", "Make " ++ BT.Bot.name(b)), "uber-promote", BT.Bot.id(b)}

def Tour.picks(us: List<&2, Used>, first: Bool) -> List<&2, TChoice>:
  match us:
    case Nil{}:
      Nil{}
    case Used{_, b} <> t:
      Tour.pick.one(first, b) <> Tour.picks(t, False{})

def Tour.choices.k(+uc: String, +st: M.State) -> List<&2, TChoice>:
  Bool.pick(List<&2, TChoice>, String.eq(uc, "none"), [TChoice{"Create my uberbot", "uber-new", ""}],
    Bool.pick(List<&2, TChoice>, String.eq(uc, "pick"), List.append(&2, TChoice, Tour.picks(List.take(&2, Used, Tour.ranked(st), 3n), True{}), [TChoice{"Create a new one", "uber-new", ""}]), Nil{}))

# the buttons under the uberbot step's words (none on other steps)
def Tour.choices.on(on: Bool, +u: Ui) -> List<&2, TChoice>:
  match on:
    case True{}:
      Tour.choices.k(Tour.uber.case(u), Ui.state.of(u))
    case False{}:
      Nil{}

def Tour.choices(+u: Ui) -> List<&2, TChoice>:
  Tour.choices.on(Nat.is_eq(Tour.at(u), OB.Ob.uber()), u)

# the step's words for a kind of client (0 window, 1 web, 2 phone)
def Tour.body(+k: Nat, +u: Ui) -> String:
  OB.Ob.body.uber(k, Tour.days(u), Tour.uber.case(u), Tour.uber.name(u), Tour.at(u))

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
def Tour.go.k(live: Bool, +i: Nat, +u: Ui) -> Act:
  match live:
    case True{}:
      Act{Ui.scratch_set(Ui.scratch_set(u, "@tour", OB.Ob.raw(OB.Ob.manual(Ui.scratch(u, "@tour")), i)), "@tour.sc", ""), Nil{}}
    case False{}:
      Tour.finish(u)

def Tour.go(+i: Nat, +u: Ui) -> Act:
  Tour.go.k(OB.Ob.live(i), i, u)

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

def Tour.act(+u: Ui, +action: String) -> Act:
  Tour.act.on(Tour.on(u), u, action)

# The uberbot (core/uber.bend, docs/uber.md): "uber" shows its Status tab,
# "uber-promote <bot>" and "uber-demote" ask the hub (bots.uber), which backs
# the bot up before it logs the promotion. Routed ahead of the action table.
def Uber.show.k(none: Bool, +u: Ui, +id: String) -> Act:
  match none:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      Bots.select(Ui.scratch_set(u, "@btab", "status"), id)

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
      Emb.sync.sel(Ui.sel(u), Mpage.sync(Ui.act.m(Nat.is_lt(C.Codec.pos(Menu.actions(), action), 17n), C.Codec.pos(Menu.actions(), action), u, action, value)))

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

# (design review's "rv-" actions route before the rest: Rv.act)
def Ui.act.r(rv: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match rv:
    case True{}:
      Rv.act(u, action, value)
    case False{}:
      Ui.act.u(Nat.is_lt(C.Codec.pos(["uber-promote", "uber-new"], action), 2n), u, action, value)

# (Antigravity's "agy-" actions route before the rest too: Agv.act)
def Ui.act.g(agy: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match agy:
    case True{}:
      Agv.act(u, action, value)
    case False{}:
      Ui.act.r(String.starts_with(action, "rv-"), u, action, value)

def Ui.act.s(close: Bool, +u: Ui, +action: String, +value: String) -> Act:
  match close:
    case True{}:
      Skills.close(u)
    case False{}:
      Ui.act.g(String.starts_with(action, "agy-"), u, action, value)

def Ui.now(u: Ui) -> Nat:
  match u:
    case Ui{_, _, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}:
      now

# Standing rules and task board: shared rows, forms, actions for all clients.
type RRow is Data:
  RRow{id: String, title: String, note: String, buttons: List<&2, AgBtn>}

def RulesUi.lead() -> String:
  "Standing rules every agent is given. Enforced rules refuse the named tools unless your latest message has the word."

def RulesUi.note(+r: RD.Rule, +st: M.State) -> String:
  RD.Rules.scope(r, st) ++ Bool.pick(String, String.is_empty(RD.Rule.tool(r)), "", " · tool " ++ RD.Rule.tool(r)) ++
    Bool.pick(String, RD.Rule.enforce(r), " · enforced" ++ Bool.pick(String, String.is_empty(RD.Rule.word(r)), "", " unless \"" ++ RD.Rule.word(r) ++ "\""), "") ++
    Bool.pick(String, RD.Rule.on(r), "", " · off")

def RulesUi.del.btns(+u: Ui, +id: String) -> List<&2, AgBtn>:
  Bool.pick(List<&2, AgBtn>, String.eq(Ui.scratch(u, "@rule.del"), id),
    [AgBtn{"Delete?", "rule-del", id, True{}}, AgBtn{"Keep", "rule-keep", "", False{}}], [AgBtn{"Delete", "rule-del", id, False{}}])

def RulesUi.rows.go(rs: List<&2, RD.Rule>, +u: Ui) -> List<&2, RRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      RRow{RD.Rule.id(r), RD.Rule.text(r), RulesUi.note(r, Ui.state(u)), List.append(&2, AgBtn,
        [AgBtn{Bool.pick(String, RD.Rule.on(r), "On", "Off"), "rule-on", RD.Rule.id(r), RD.Rule.on(r)}, AgBtn{"Edit", "rule-edit", RD.Rule.id(r), False{}}],
        RulesUi.del.btns(u, RD.Rule.id(r)))} <> RulesUi.rows.go(t, u)

def RulesUi.rows(+u: Ui) -> List<&2, RRow>:
  RulesUi.rows.go(RD.Rules.all(Ui.state(u)), u)

def RulesUi.bot.chip(here: Bool, +b: BT.Bot, +cur: String, rest: List<&2, AgBtn>) -> List<&2, AgBtn>:
  match here:
    case True{}:
      AgBtn{BT.Bot.name(b), "rule-bot", BT.Bot.id(b), String.eq(cur, BT.Bot.id(b))} <> rest
    case False{}:
      rest

def RulesUi.bots(bs: List<&2, BT.Bot>, +cur: String) -> List<&2, AgBtn>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      RulesUi.bot.chip(Bool.not(FR.Fr.is(BT.Bot.id(b))), b, cur, RulesUi.bots(t, cur))

def RulesUi.projects(ps: List<&2, M.Project>, +cur: String) -> List<&2, AgBtn>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, title, _, _, False{}} <> t:
      AgBtn{title, "rule-proj", id, String.eq(cur, id)} <> RulesUi.projects(t, cur)
    case _ <> t:
      RulesUi.projects(t, cur)

def RulesUi.form.k(open: Bool, +u: Ui) -> List<&2, AgRow>:
  match open:
    case False{}:
      Nil{}
    case True{}:
      [AgRow{Bool.pick(String, String.is_empty(Ui.scratch(u, "@rule.edit")), "New rule", "Edit rule"), Ui.scratch(u, "@rule.err"),
        [AgField{"rtext", "Rule", False{}, Bots.field(u, "rtext"), "rule-save"}, AgField{"rtool", "Tools", False{}, Bots.field(u, "rtool"), "rule-save"},
         AgField{"rword", "Unlock word", False{}, Bots.field(u, "rword"), "rule-save"}],
        [AgBtn{"Enforce", "rule-enf", "", String.eq(Ui.scratch(u, "@rule.enf"), "1")}, AgBtn{"Save", "rule-save", "", True{}}, AgBtn{"Cancel", "rule-cancel", "", False{}}]},
       AgRow{"Bot", Bool.pick(String, String.is_empty(Bots.field(u, "rbot")), "Any", RD.Rules.bot.name(Ui.state(u), Bots.field(u, "rbot"))), Nil{},
         AgBtn{"Any", "rule-bot", "", String.is_empty(Bots.field(u, "rbot"))} <> RulesUi.bots(BT.World.bots(M.State.bots(Ui.state(u))), Bots.field(u, "rbot"))},
       AgRow{"Project", Bool.pick(String, String.is_empty(Bots.field(u, "rproj")), "Any", RD.Rules.project.name(Ui.state(u), Bots.field(u, "rproj"))), Nil{},
         AgBtn{"Any", "rule-proj", "", String.is_empty(Bots.field(u, "rproj"))} <> RulesUi.projects(Ui.projects(Ui.state(u)), Bots.field(u, "rproj"))}]

def RulesUi.form(+u: Ui) -> List<&2, AgRow>:
  RulesUi.form.k(Ui.flag(u, "ruleform"), u)

def RulesUi.row.forms(rs: List<&2, RRow>) -> List<&2, AgRow>:
  match rs:
    case Nil{}:
      [AgRow{"No rules yet.", "", Nil{}, Nil{}}]
    case RRow{_, t, n, bs} <> Nil{}:
      [AgRow{t, n, Nil{}, bs}]
    case RRow{_, t, n, bs} <> tail:
      AgRow{t, n, Nil{}, bs} <> RulesUi.row.forms(tail)

def RulesUi.cards(+u: Ui) -> List<&2, AgRow>:
  List.concat(&2, AgRow, [[AgRow{"Rules", RulesUi.lead(), Nil{}, [AgBtn{"Add rule", "rule-new", "", False{}}]}], RulesUi.form(u), RulesUi.row.forms(RulesUi.rows(u))])

def RulesUi.fill(+u: Ui, +r: RD.Rule) -> Ui:
  Ui.flag_on(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u,
    "@f.rtext", RD.Rule.text(r)), "@f.rbot", RD.Rule.bot(r)), "@f.rproj", RD.Rule.project(r)), "@f.rtool", RD.Rule.tool(r)), "@f.rword", RD.Rule.word(r)),
    "@rule.enf", Bool.pick(String, RD.Rule.enforce(r), "1", "")), "@rule.edit", RD.Rule.id(r)), "@rule.err", ""), "ruleform")

def RulesUi.params(+u: Ui) -> J.Json:
  J.Json.obj(List.append(&2, J.KV, Bool.pick(List<&2, J.KV>, String.is_empty(Ui.scratch(u, "@rule.edit")), Nil{}, [J.KV{"id", J.Str{Ui.scratch(u, "@rule.edit")}}]),
    [J.KV{"text", J.Str{Bots.field(u, "rtext")}}, J.KV{"bot", J.Str{Bots.field(u, "rbot")}}, J.KV{"project", J.Str{Bots.field(u, "rproj")}},
     J.KV{"tool", J.Str{Bots.field(u, "rtool")}}, J.KV{"word", J.Str{Bots.field(u, "rword")}}, J.KV{"enforce", J.Flag{String.eq(Ui.scratch(u, "@rule.enf"), "1")}},
     J.KV{"on", J.Flag{RD.Rule.on(Maybe.default(&2, RD.Rule, RD.Rules.get(Ui.state(u), Ui.scratch(u, "@rule.edit")), RD.Rules.blank()))}}]))

def RulesUi.save(+u: Ui) -> Act:
  Ui.rpc.keep(Ui.flag_off(Ui.scratch_set(Ui.scratch_set(u, "@rule.rid", Nat.show(Made.rid(u))), "@rule.err", ""), "ruleform"), "rules.set", [J.KV{"rule", RulesUi.params(u)}])

def RulesUi.delete(armed: Bool, +u: Ui, +id: String) -> Act:
  match armed:
    case True{}:
      Ui.rpc.keep(Ui.scratch_set(u, "@rule.del", ""), "rules.drop", [J.KV{"id", J.Str{id}}])
    case False{}:
      Act{Ui.scratch_set(u, "@rule.del", id), Nil{}}

def RulesUi.act.k(i: Nat, +u: Ui, +v: String) -> Act:
  match i:
    case 0n:
      Act{RulesUi.fill(u, RD.Rules.blank()), [Focus{"rtext"}]}
    case 1n:
      Act{RulesUi.fill(u, Maybe.default(&2, RD.Rule, RD.Rules.get(Ui.state(u), v), RD.Rules.blank())), [Focus{"rtext"}]}
    case 2n:
      Act{Ui.scratch_set(u, "@rule.enf", Bool.pick(String, String.eq(Ui.scratch(u, "@rule.enf"), "1"), "", "1")), Nil{}}
    case 3n:
      Act{Ui.scratch_set(u, "@f.rbot", v), Nil{}}
    case 4n:
      Act{Ui.scratch_set(u, "@f.rproj", v), Nil{}}
    case 5n:
      RulesUi.save(u)
    case 6n:
      Act{Ui.flag_off(u, "ruleform"), Nil{}}
    case 7n:
      Ui.rpc.keep(u, "rules.set", [J.KV{"rule", J.Json.obj([J.KV{"id", J.Str{v}}, J.KV{"on", J.Flag{Bool.not(RD.Rule.on(Maybe.default(&2, RD.Rule, RD.Rules.get(Ui.state(u), v), RD.Rules.blank())))}}])}])
    case 8n:
      RulesUi.delete(String.eq(Ui.scratch(u, "@rule.del"), v), u, v)
    case 9n:
      Act{Ui.scratch_set(u, "@rule.del", ""), Nil{}}
    case _:
      Act{u, Nil{}}

def RulesUi.act(+u: Ui, +a: String, +v: String) -> Act:
  RulesUi.act.k(C.Codec.pos(["rule-new", "rule-edit", "rule-enf", "rule-bot", "rule-proj", "rule-save", "rule-cancel", "rule-on", "rule-del", "rule-keep"], a), u, v)

type BRow is Data:
  BRow{id: String, title: String, sub: String, status: String, due_soon: Bool, overdue: Bool, open: Bool}

type BSec is Data:
  BSec{title: String, count: Nat, rows: List<&2, BRow>}

def BoardUi.is(+u: Ui) -> Bool:
  String.eq(Ui.scratch(u, "@board"), "1")

def BoardUi.filter(+u: Ui) -> String:
  Bool.pick(String, String.eq(Ui.scratch(u, "@tb.f"), "all"), "all", "open")

def BoardUi.join(xs: List<&2, String>) -> String:
  String.join(TBD.Tb.text.nonempty(xs), " · ")

def BoardUi.due.day(+d: String) -> Nat:
  CR.Cron.days(TBD.Tb.num(String.take(d, 4n), 0n), TBD.Tb.num(String.take(String.drop(d, 5n), 2n), 0n), TBD.Tb.num(String.drop(d, 8n), 0n))

def BoardUi.overdue(+d: String, +now: Nat) -> Bool:
  Bool.and(TBD.Tb.date(d), Nat.is_lt(BoardUi.due.day(d), Nat.div(now, 86400n)))

def BoardUi.soon(+d: String, +now: Nat) -> Bool:
  Bool.and(TBD.Tb.date(d), Bool.and(Nat.is_ge(BoardUi.due.day(d), Nat.div(now, 86400n)), Nat.is_le(BoardUi.due.day(d), Nat.add(Nat.div(now, 86400n), 2n))))

def BoardUi.due(+d: String) -> String:
  Bool.pick(String, TBD.Tb.date(d), "due " ++ Maybe.default(&2, String, List.get(&2, String, ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"], Nat.sub(TBD.Tb.num(String.take(String.drop(d, 5n), 2n), 0n), 1n)), "") ++ " " ++ Nat.show(TBD.Tb.num(String.drop(d, 8n), 0n)), "")

def BoardUi.item(+u: Ui, +id: String) -> TBD.TbItem:
  Maybe.default(&2, TBD.TbItem, TBD.Tb.get(Ui.state(u), id), TBD.Tb.blank(id, 0n))

def BoardUi.row(+t: TBD.TbItem, +u: Ui) -> BRow:
  BRow{TBD.Tb.id(t), TBD.Tb.title(t), BoardUi.join([TBD.Tb.owner.name(Ui.state(u), TBD.Tb.owner(t)), TBD.Tb.proj.name(Ui.state(u), TBD.Tb.project(t)), BoardUi.due(TBD.Tb.due(t)), TBD.Tb.group(t)]),
    TBD.Tb.status(t), BoardUi.soon(TBD.Tb.due(t), Ui.now(u)), BoardUi.overdue(TBD.Tb.due(t), Ui.now(u)), String.eq(Ui.scratch(u, "@tb.row"), TBD.Tb.id(t))}

def BoardUi.rows(xs: List<&2, TBD.TbItem>, +u: Ui) -> List<&2, BRow>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      BoardUi.row(x, u) <> BoardUi.rows(t, u)

def BoardUi.items(+u: Ui) -> List<&2, TBD.TbItem>:
  TBD.Tb.order(TBD.Tb.filter(TBD.Tb.all(Ui.state(u)), Bool.pick(String, String.eq(BoardUi.filter(u), "all"), "", "doing,blocked,todo"), Ui.scratch(u, "@tb.who"), Ui.scratch(u, "@tb.proj"), Ui.scratch(u, "@tb.group")))

def BoardUi.sections.go(ss: List<&2, String>, +xs: List<&2, TBD.TbItem>, +u: Ui) -> List<&2, BSec>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      +rows = BoardUi.rows(TBD.Tb.filter(xs, s, "", "", ""), u)
      BSec{TBD.Tb.words(s), List.length(&2, BRow, rows), rows} <> BoardUi.sections.go(t, xs, u)

def BoardUi.sections(+u: Ui) -> List<&2, BSec>:
  BoardUi.sections.go(Bool.pick(List<&2, String>, String.eq(BoardUi.filter(u), "all"), TBD.Tb.statuses(), ["doing", "blocked", "todo"]), BoardUi.items(u), u)

def BoardUi.count(+u: Ui) -> Nat:
  List.length(&2, TBD.TbItem, TBD.Tb.open(TBD.Tb.all(Ui.state(u))))

def BoardUi.label(+u: Ui) -> String:
  "Board · " ++ Nat.show(BoardUi.count(u)) ++ " open"

def BoardUi.cycle(+s: String) -> String:
  Bool.pick(String, String.eq(s, "todo"), "doing", Bool.pick(String, String.eq(s, "doing"), "done", "todo"))

def BoardUi.mark(+s: String) -> String:
  Bool.pick(String, String.eq(s, "todo"), "□", Bool.pick(String, String.eq(s, "doing"), "◧", Bool.pick(String, String.eq(s, "done"), "✓", Bool.pick(String, String.eq(s, "blocked"), "!", "–"))))

def BoardUi.btns(+u: Ui, +id: String) -> List<&2, AgBtn>:
  List.append(&2, AgBtn, [AgBtn{"Blocked", "tb-status", id ++ "\tblocked", False{}}, AgBtn{"Drop", "tb-status", id ++ "\tdropped", False{}},
    AgBtn{"To do", "tb-status", id ++ "\ttodo", False{}}, AgBtn{"Doing", "tb-status", id ++ "\tdoing", False{}}, AgBtn{"Done", "tb-status", id ++ "\tdone", False{}}],
    Bool.pick(List<&2, AgBtn>, String.eq(Ui.scratch(u, "@tb.del"), id), [AgBtn{"Delete?", "tb-del", id, True{}}, AgBtn{"Keep", "tb-keep", "", False{}}], [AgBtn{"Delete", "tb-del", id, False{}}]))

def BoardUi.chip.val(+t: TBD.TbItem, k: Nat) -> String:
  match k:
    case 0n:
      Bool.pick(String, String.is_empty(TBD.Tb.owner(t)), "person", TBD.Tb.owner(t))
    case 1n:
      TBD.Tb.project(t)
    case _:
      TBD.Tb.group(t)

def BoardUi.chip.name(+v: String, k: Nat, +u: Ui) -> String:
  match k:
    case 0n:
      Bool.pick(String, String.eq(v, "person"), "you", TBD.Tb.owner.name(Ui.state(u), v))
    case 1n:
      TBD.Tb.proj.name(Ui.state(u), v)
    case _:
      v

def BoardUi.chip.push(skip: Bool, +v: String, +k: Nat, +a: String, +cur: String, +u: Ui, rest: List<&2, AgBtn>) -> List<&2, AgBtn>:
  match skip:
    case True{}:
      rest
    case False{}:
      AgBtn{BoardUi.chip.name(v, k, u), a, v, String.eq(cur, v)} <> rest

def BoardUi.chips.go(xs: List<&2, TBD.TbItem>, +k: Nat, +a: String, +cur: String, +u: Ui, +seen: List<&2, String>) -> List<&2, AgBtn>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      +v = BoardUi.chip.val(x, k)
      BoardUi.chip.push(Bool.or(String.is_empty(v), TBD.Tb.has(seen, v)), v, k, a, cur, u, BoardUi.chips.go(t, k, a, cur, u, v <> seen))

def BoardUi.filters(+u: Ui) -> List<&2, AgBtn>:
  List.concat(&2, AgBtn, [[AgBtn{"Open", "tb-f", "open", String.eq(BoardUi.filter(u), "open")}, AgBtn{"All", "tb-f", "all", String.eq(BoardUi.filter(u), "all")},
    AgBtn{"Any owner", "tb-who", "", String.is_empty(Ui.scratch(u, "@tb.who"))}], BoardUi.chips.go(TBD.Tb.all(Ui.state(u)), 0n, "tb-who", Ui.scratch(u, "@tb.who"), u, Nil{}),
    [AgBtn{"Any project", "tb-proj", "", String.is_empty(Ui.scratch(u, "@tb.proj"))}], BoardUi.chips.go(TBD.Tb.all(Ui.state(u)), 1n, "tb-proj", Ui.scratch(u, "@tb.proj"), u, Nil{}),
    [AgBtn{"Any group", "tb-group", "", String.is_empty(Ui.scratch(u, "@tb.group"))}], BoardUi.chips.go(TBD.Tb.all(Ui.state(u)), 2n, "tb-group", Ui.scratch(u, "@tb.group"), u, Nil{})])

def BoardUi.links(xs: List<&2, String>) -> List<&2, AgBtn>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      AgBtn{x, Bool.pick(String, LK.Link.is_web(x), "open-url", "file-open"), x, False{}} <> BoardUi.links(t)

def BoardUi.detail(+u: Ui, +id: String) -> List<&2, AgRow>:
  [AgRow{"", TBD.Tb.notes(BoardUi.item(u, id)), Nil{}, BoardUi.links(TBD.Tb.links(BoardUi.item(u, id)))},
   AgRow{"Title", "", [AgField{"tbedit", "Title", False{}, Bots.field(u, "tbedit"), "tb-title"}], [AgBtn{"Rename", "tb-title", id, False{}}]},
   AgRow{"Notes", "", [AgField{"tbnotes", "Notes", False{}, Bots.field(u, "tbnotes"), "tb-notes"}], [AgBtn{"Save notes", "tb-notes", id, False{}}]},
   AgRow{"", "", Nil{}, BoardUi.btns(u, id)}]

def BoardUi.set(+u: Ui, fs: List<&2, J.KV>) -> Act:
  Ui.rpc.keep(u, "board.set", [J.KV{"item", J.Json.obj(fs)}])

def BoardUi.status(+u: Ui, +id: String, +s: String) -> Act:
  BoardUi.set(u, [J.KV{"id", J.Str{id}}, J.KV{"status", J.Str{s}}])

def BoardUi.delete(armed: Bool, +u: Ui, +id: String) -> Act:
  match armed:
    case True{}:
      Ui.rpc.keep(Ui.scratch_set(u, "@tb.del", ""), "board.drop", [J.KV{"id", J.Str{id}}])
    case False{}:
      Act{Ui.scratch_set(u, "@tb.del", id), Nil{}}

def BoardUi.add(empty: Bool, +u: Ui) -> Act:
  match empty:
    case True{}:
      Act{u, Nil{}}
    case False{}:
      BoardUi.set(Ui.scratch_set(u, "@f.tbtitle", ""), [J.KV{"title", J.Str{Bots.field(u, "tbtitle")}}, J.KV{"owner", J.Str{Ui.scratch(u, "@tb.who")}},
        J.KV{"project", J.Str{Ui.scratch(u, "@tb.proj")}}, J.KV{"group", J.Str{Ui.scratch(u, "@tb.group")}}])

def BoardUi.open(+u: Ui) -> Ui:
  Ui.scratch_set(Ui.flag_off(Ui.select.k(u, ""), "settings"), "@board", "1")

def BoardUi.act.k(i: Nat, +u: Ui, +v: String) -> Act:
  match i:
    case 0n:
      Act{BoardUi.open(u), Nil{}}
    case 1n:
      Act{Ui.scratch_set(u, "@board", ""), Nil{}}
    case 2n:
      Act{Ui.scratch_set(u, "@tb.f", v), Nil{}}
    case 3n:
      Act{Ui.scratch_set(u, "@tb.who", v), Nil{}}
    case 4n:
      Act{Ui.scratch_set(u, "@tb.proj", v), Nil{}}
    case 5n:
      Act{Ui.scratch_set(u, "@tb.group", v), Nil{}}
    case 6n:
      BoardUi.status(u, v, BoardUi.cycle(TBD.Tb.status(BoardUi.item(u, v))))
    case 7n:
      BoardUi.status(u, Maybe.default(&2, String, List.head(&2, String, String.split(v, '\t')), ""), Maybe.default(&2, String, List.get(&2, String, String.split(v, '\t'), 1n), "todo"))
    case 8n:
      Act{Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(Ui.scratch_set(u, "@tb.del", ""), "@f.tbnotes", TBD.Tb.notes(BoardUi.item(u, v))), "@f.tbedit", TBD.Tb.title(BoardUi.item(u, v))), "@tb.row", Bool.pick(String, String.eq(Ui.scratch(u, "@tb.row"), v), "", v)), Nil{}}
    case 9n:
      BoardUi.add(String.is_empty(String.trim(Bots.field(u, "tbtitle"))), u)
    case 10n:
      BoardUi.set(u, [J.KV{"id", J.Str{Bool.pick(String, String.is_empty(v), Ui.scratch(u, "@tb.row"), v)}}, J.KV{"notes", J.Str{Bots.field(u, "tbnotes")}}])
    case 11n:
      BoardUi.set(u, [J.KV{"id", J.Str{Bool.pick(String, String.is_empty(v), Ui.scratch(u, "@tb.row"), v)}}, J.KV{"title", J.Str{Bots.field(u, "tbedit")}}])
    case 12n:
      BoardUi.delete(String.eq(Ui.scratch(u, "@tb.del"), v), u, v)
    case 13n:
      Act{Ui.scratch_set(u, "@tb.del", ""), Nil{}}
    case _:
      Act{u, Nil{}}

def BoardUi.act(+u: Ui, +a: String, +v: String) -> Act:
  BoardUi.act.k(C.Codec.pos(["tb-open", "tb-close", "tb-f", "tb-who", "tb-proj", "tb-group", "tb-tick", "tb-status", "tb-row", "tb-add", "tb-notes", "tb-title", "tb-del", "tb-keep"], a), u, v)

# Small adversarial examples read by the UI laws: a todo older than doing,
# and a closed item which the default filter must hide.
def BoardUi.demo() -> Ui:
  Ui.far.st(Ui.init(200000n), M.State.replay([
    M.SettingSet{"board.todo", "{\"title\":\"Plan\",\"status\":\"todo\",\"at\":1}", 1n},
    M.SettingSet{"board.doing", "{\"title\":\"Build\",\"status\":\"doing\",\"at\":2}", 2n},
    M.SettingSet{"board.done", "{\"title\":\"Finished\",\"status\":\"done\",\"at\":3}", 3n}], M.State.empty()))

def BoardUi.rows.ids(rs: List<&2, BRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case BRow{id, _, _, _, _, _, _} <> t:
      id <> BoardUi.rows.ids(t)

def BoardUi.sections.ids(ss: List<&2, BSec>) -> List<&2, String>:
  match ss:
    case Nil{}:
      Nil{}
    case BSec{_, _, rs} <> t:
      List.append(&2, String, BoardUi.rows.ids(rs), BoardUi.sections.ids(t))

def Ui.act.tb(board: Bool, +u: Ui, +a: String, +v: String) -> Act:
  match board:
    case True{}:
      BoardUi.act(u, a, v)
    case False{}:
      Ui.act.s(String.eq(a, "skill-close"), u, a, v)

def Ui.act.rule(rule: Bool, +u: Ui, +a: String, +v: String) -> Act:
  match rule:
    case True{}:
      RulesUi.act(u, a, v)
    case False{}:
      Ui.act.tb(String.starts_with(a, "tb-"), u, a, v)

def Ui.act(+u: Ui, +action: String, +value: String) -> Act:
  Rc.sync.of(Ui.act.rule(String.starts_with(action, "rule-"), u, action, value))

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

def Page.newest(+es: List<&2, M.Entry>, +th: String, +n: Nat) -> Win:
  Page.newest.from(es, th, 0n, n)

# when a thread began (its entries are no older)
def Page.since.k(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, c, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      c

def Page.since(s: M.State, +th: String) -> Nat:
  Page.since.k(Ui.thread(s, th))
