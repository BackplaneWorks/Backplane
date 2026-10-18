import Base
import ../core/json.bend as J
import ../core/client.bend as U
import ../core/cbor.bend as CB
import ../core/sha1.bend as SH
import ./hubs.bend as H
import ./screen.bend as S
import ./bots.bend as MB
import ../core/msgs.bend as MSG

# What bridge.js calls. The phone apps see only strings: server messages
# in (binary CBOR frames, carried as base64, decoded by bridge.js) from
# each hub the phone is paired with, named by its key; the screen,
# commands (each for one hub) and alerts out, and what to send (CBOR, as
# base64). hubs.bend keeps a client per hub and makes one screen of them.

def init(now: Nat, +cid: String) -> H.Hubs:
  H.Hubs.init(now, cid)

def hubs.keys(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.as_str(Some{x}, "") <> hubs.keys(t)

# the hubs the phone is paired with, by key, as a JSON array of strings
def hubs.k(+hs: H.Hubs, j: J.Json) -> H.Hubs:
  H.Hubs.set(hs, hubs.keys(J.Json.elems(j)))

def hubs(+hs: H.Hubs, text: String) -> H.Hubs:
  hubs.k(hs, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

# Drafts the phone kept (keep commands, named "<hub>|<thread>"), read back
# into the client of the hub they belong to once the hubs are known

def restore.one(hit: Bool, u: U.Ui, +id: String, +text: String) -> U.Ui:
  match hit:
    case True{}:
      U.Ui.restore(u, id, text)
    case False{}:
      u

def restore.go(hs: List<&2, H.Hub>, +key: String, +id: String, +text: String) -> List<&2, H.Hub>:
  match hs:
    case Nil{}:
      Nil{}
    case H.Hub{+k, u} <> t:
      H.Hub{k, restore.one(String.eq(k, key), u, id, text)} <> restore.go(t, key, id, text)

def restore(hs: H.Hubs, +key: String, +id: String, +text: String) -> H.Hubs:
  match hs:
    case H.Hubs{c, n, cur, l}:
      H.Hubs{c, n, cur, restore.go(l, key, id, text)}

# the alerts a step raised, as JSON text
def alerts(a: H.HAct) -> String:
  match a:
    case H.HAct{_, _, als}:
      J.Json.show(J.Json.arr(als))


# a frame from hub `key` (a bot's browser frame, {"t":"shot"}, is kept as
# info "shot.<bot>", which its Browser tab shows as n, and said seen while
# on show: client.bend's Ui.recv_act.s), already decoded: bridge.js reads the base64 and
# CBOR natively (Cbor.decode here took 17x as long on a phone's
# JavaScriptCore, which has no JIT), with these dictionaries
def recv(+hs: H.Hubs, +key: String, +o: J.Json) -> H.HAct:
  H.Hubs.recv(hs, key, o)

# whether that frame can change the screen (Hubs.redraw): the bridge
# builds none for another thread's streamed text, and one for anything
# else from any paired hub
def shows(+hs: H.Hubs, +key: String, +o: J.Json) -> Bool:
  H.Hubs.redraw(hs, key, o)

# a cat's rig as JSON text, for the key the screen names it by
# ("look:mood"); the apps ask once per key and keep it
def cat(+look: Nat, +mood: String) -> String:
  J.Json.show(MB.MBots.rig(look, mood))

# a catalog text for a native screen part (Swift, Kotlin): the key itself
# when the catalog has none, so a missing key shows
def t.k(m: Maybe<&2, String>, key: String) -> String:
  match m:
    case Some{s}: s
    case None{}: key

def t(hs: H.Hubs, +key: String) -> String:
  t.k(MSG.T.plain(H.Hubs.loc(hs), key), key)

# A value of each Json constructor, for wire.js: bend 2.0.36 tags a module's
# constructors with their file ("../core/json.Str"), so the JavaScript that
# builds Json learns the tags from Bend instead of spelling them.
def json_tags() -> J.Json:
  J.Arr{J.Item{J.Null{}, J.Item{J.Flag{False{}}, J.Item{J.Num{"0"}, J.Item{J.Str{""}, J.Item{J.Obj{J.Field{"", J.Null{}, J.End{}}}, J.End{}}}}}}}

def cbor_keys() -> List<&2, String>:
  CB.Cbor.keys()

def cbor_words() -> List<&2, String>:
  CB.Cbor.words()

# a message for the hub (a Send command's JSON text) as the base64 of the
# CBOR frame the app sends
def wire(text: String) -> String:
  SH.B64.encode(CB.Cbor.encode(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))

# an action, routed to the hub it names (hubs.bend)
def act(+hs: H.Hubs, +action: String, +value: String) -> H.HAct:
  H.Hubs.act(hs, action, value)

# a choice of this device (ios-snappy tabs): the thread list's view
def local(hs: H.Hubs, key: String, value: String) -> H.Hubs:
  H.Hubs.local(hs, key, value)

def screen(+hs: H.Hubs) -> String:
  J.Json.show(H.Hubs.screen(hs))

# the screen as a JSON value: the bridge writes it with the JS engine's own
# JSON.stringify (writing it in Bend walks every string char by char, the
# most of a thread screen's cost on a phone's QuickJS)
def screenj(+hs: H.Hubs) -> J.Json:
  H.Hubs.screen(hs)

# The same screen in two halves (hubs.bend's Hubs.lean): the bridge keeps
# the list half while lean(hs) is made of the very same parts, and makes
# the thread (focus) for every screen
def lean(hs: H.Hubs) -> H.Hubs:
  H.Hubs.lean(hs)

def screen_list(+hs: H.Hubs) -> J.Json:
  H.Hubs.screen.fast(hs)

def focus(+hs: H.Hubs) -> J.Json:
  H.Hubs.opened(hs)

# the open thread's streamed reply, and markdown text as the nodes the
# thread's "live" shows
def live_text(+hs: H.Hubs) -> String:
  H.Hubs.live.text(hs)

def live_nodes(+text: String) -> J.Json:
  S.Mob.live.nodes(text)

def online.is(u: U.Ui, b: Bool) -> Bool:
  match u b:
    case U.Ui{_, _, _, _, _, True{}, _, _, _, _, _, _, _, _, _, _, _} True{}:
      True{}
    case U.Ui{_, _, _, _, _, False{}, _, _, _, _, _, _, _, _, _, _, _} False{}:
      True{}
    case _ _:
      False{}

# hub key already is online (or offline) as b says: a socket that keeps
# failing to open says so every few seconds, and nothing on the screen
# changes, so the bridge builds no screen for it
def online_same(+hs: H.Hubs, +key: String, b: Bool) -> Bool:
  online.is(H.Hubs.ui(hs, key), b)

def offer.go(rs: List<&2, U.SkillRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.SkillRow{n, d, on} <> t:
      n ++ "\t" ++ d ++ Bool.pick(String, on, "\t*", "") ++ "\n" ++ offer.go(t)

# what the composer of the hub in focus offers (skills after `$`; bots,
# threads, projects and rooms after `@`, `>`, `%` or `#`) as one string: a
# typed draft is quiet (no screen), unless it changes this
def offer(+hs: H.Hubs) -> String:
  offer.go(U.Ui.skills(H.Hubs.ui(hs, H.Hubs.cur(hs))))

def online(+hs: H.Hubs, +key: String, b: Bool) -> H.HAct:
  H.Hubs.online(hs, key, b)

def stale(hs: H.Hubs) -> H.Hubs:
  H.Hubs.stale(hs)

def tick(+hs: H.Hubs, now: Nat) -> H.HAct:
  H.Hubs.tick(hs, now)

# how many changes this app holds from hub `key` and which log they came
# from (a reconnect asks that hub for the rest)
def seq(+hs: H.Hubs, +key: String) -> String:
  Nat.show(U.Ui.seq(H.Hubs.ui(hs, key)))

def origin(+hs: H.Hubs, +key: String) -> String:
  U.Ui.info(H.Hubs.ui(hs, key), "origin")

# this phone, for pushes while the app sleeps: an APNs token for alerts,
# for one Live Activity ("activity", with its thread) or for starting one
# ("start"). Every paired hub gets it, kept in each outbox until answered.
def register(+hs: H.Hubs, platform: String, token: String, kind: String, thread: String, env: String, bundle: String) -> H.HAct:
  H.Hubs.register(hs, platform, token, kind, thread, env, bundle)

# the same for one hub alone (a Live Activity's token goes to its own hub)
def register_at(+hs: H.Hubs, +hub: String, platform: String, token: String, kind: String, thread: String, env: String, bundle: String) -> H.HAct:
  H.Hubs.register.at(hs, hub, platform, token, kind, thread, env, bundle)
