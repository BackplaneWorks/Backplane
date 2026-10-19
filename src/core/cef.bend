import Base
import ./json.bend as J

# The embedded Chromium (tools/cef), the hub's side and the window's, pure.
# The native window tells its hub its X window (browser.embed), and the hub
# then moves the helper to the CEF engine (setting browser.engine "cef"):
# the pages are native child windows of the Backplane window. The window
# says where the page goes (browser.place) and the hub forwards it as show
# or hide. Who may click the page is the hub's drive state (core/teach.bend,
# a page's drive "" means the agent): a shield over the page swallows the
# person's pointer while the agent drives. Nothing here enters the log.

# The engine
# ----------

# Native embedding is the default request. An explicit Chrome choice remains
# authoritative; the live engine still depends on an embed and usable runtime.
def Cef.default() -> String:
  "cef"

def Cef.requested(+setting: String) -> String:
  Bool.pick(String, String.eq(setting, "chrome"), "chrome", Cef.default())

# cef when requested, a window's embed is known and the runtime has not failed
def Cef.engine(+setting: String, known: Bool, runtime_ok: Bool) -> Bool:
  Bool.and(String.eq(setting, "cef"), Bool.and(known, runtime_ok))

def Cef.name(on: Bool) -> String:
  match on:
    case True{}: "cef"
    case False{}: "chrome"

# a window's embed: a real X window on a display
def Cef.known(+xid: U32, +display: String) -> Bool:
  Bool.and(U32.is_gt(xid, 0), Bool.not(String.is_empty(display)))

# what the helper is to run, as a short key compared after each hub step
# that can change it ("chrome", or "cef <xid> <display>")
def Cef.want.key(on: Bool, +xid: U32, +display: String) -> String:
  match on:
    case True{}: "cef " ++ U32.show(xid) ++ " " ++ display
    case False{}: "chrome"

# the helper's engine request
def Cef.args(on: Bool, +xid: U32, +display: String) -> J.Json:
  match on:
    case True{}:
      J.Json.obj([J.KV{"engine", J.Str{"cef"}}, J.KV{"parent", J.Num{U32.show(xid)}}, J.KV{"display", J.Str{display}}])
    case False{}:
      J.Json.obj([J.KV{"engine", J.Str{"chrome"}}])

# the helper's start flags for the engine wanted (a restarted helper runs
# the engine the hub last asked for)
def Cef.argv(on: Bool, +xid: U32, +display: String) -> List<&2, String>:
  match on:
    case True{}:
      ["--engine", "cef", "--parent", U32.show(xid), "--display", display]
    case False{}:
      Nil{}

# Who may embed
# -------------

# a peer address on this machine
def Cef.loopback(+ip: String) -> Bool:
  Bool.or(String.eq(ip, "127.0.0.1"), Bool.or(String.eq(ip, "::1"), String.eq(ip, "::ffff:127.0.0.1")))

# A native link has no browser Origin or tailscale proxy identity.
# Check this before considering the socket's loopback address.
def Cef.link(+proxy: String, +origin: String) -> Bool:
  Bool.and(String.is_empty(proxy), String.is_empty(origin))

# browser.embed and browser.place come only from the window in this
# process (client 0) or a window linked to it over loopback; a web, tailnet
# or remote client is refused
def Cef.from(+client: U32, local: Bool) -> Bool:
  Bool.or(U32.is_eq(client, 0), local)

def Cef.ids.has(ids: List<&2, U32>, +id: U32) -> Bool:
  match ids:
    case Nil{}: False{}
    case +i <> t: Bool.or(U32.is_eq(i, id), Cef.ids.has(t, id))

def Cef.ids.keep(same: Bool, +i: U32, rest: List<&2, U32>) -> List<&2, U32>:
  match same:
    case True{}: rest
    case False{}: i <> rest

def Cef.ids.drop(ids: List<&2, U32>, +id: U32) -> List<&2, U32>:
  match ids:
    case Nil{}: Nil{}
    case +i <> t: Cef.ids.keep(U32.is_eq(i, id), i, Cef.ids.drop(t, id))

# Who may click
# -------------

# the agent drives (drive ""): the shield is on and swallows the person's
# pointer; the person holds the page ("takeover" or "teach"): it is off
def Cef.shield(+drive: String) -> Bool:
  String.is_empty(drive)

def Cef.shield.val(+drive: String) -> String:
  Bool.pick(String, Cef.shield(drive), "1", "0")

# the shield is to be sent: it is not what the helper was last told
def Cef.shield.due(+last: String, +drive: String) -> Bool:
  Bool.not(String.eq(last, Cef.shield.val(drive)))

# a click on the shield takes the page over only when the agent drives it
def Cef.shield.take(+drive: String) -> Bool:
  String.is_empty(drive)

# focus goes to the page once the person holds it
def Cef.shield.focus(+drive: String) -> Bool:
  Bool.not(String.is_empty(drive))

# a page id names a real thread or bot
def Cef.owner(+id: String, thread: Bool, bot: Bool) -> Bool:
  Bool.and(Bool.not(String.is_empty(id)), Bool.or(thread, bot))

# Placement
# ---------

# no hit while any overlay drawn over the main area is open (a toast does
# not hide the page)
def Cef.covered(palette: Bool, picker: Bool, menu: Bool, look: Bool, models: Bool, tour: Bool, perf: Bool, lightbox: Bool) -> Bool:
  Bool.or(Bool.or(Bool.or(palette, picker), Bool.or(menu, look)), Bool.or(Bool.or(models, tour), Bool.or(perf, lightbox)))

# the page shows natively: cef mode and nothing over it
def Cef.shows(live: Bool, covered: Bool) -> Bool:
  Bool.and(live, Bool.not(covered))

def Cef.at(+page: String, +x: U32, +y: U32, +w: U32, +h: U32) -> String:
  page ++ " " ++ U32.show(x) ++ " " ++ U32.show(y) ++ " " ++ U32.show(w) ++ " " ++ U32.show(h)

def Cef.extent(+lo: U32, +hi: U32) -> U32:
  Bool.pick(U32, U32.is_ge(hi, lo), (hi - lo : U32), 0)

# a hit's corners as the key the window compares ("" for an empty one)
def Cef.at.hit(+page: String, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(page)), Bool.and(U32.is_gt(x1, x0), U32.is_gt(y1, y0))),
    Cef.at(page, x0, y0, Cef.extent(x0, x1), Cef.extent(y0, y1)), "")

# what the window does after a frame: 0 nothing, 1 send place, 2 send hide.
# `last` is what it sent last ("" hidden), `now` what the frame leaves ("" none)
def Cef.place.step(+last: String, +now: String) -> Nat:
  Bool.pick(Nat, String.eq(last, now), 0n, Bool.pick(Nat, String.is_empty(now), 2n, 1n))

# after a frame: in cef mode the place step; otherwise 3 (forget what was
# last placed, sending nothing: the hub has gone back to chrome) when
# something was, else 0. A window not in cef mode never scans its hits.
def Cef.post.step(native: Bool, +last: String, +now: String) -> Nat:
  match native:
    case True{}: Cef.place.step(last, now)
    case False{}: Bool.pick(Nat, String.is_empty(last), 0n, 3n)

def Cef.place(page: String, x: U32, y: U32, w: U32, h: U32) -> J.Json:
  J.Json.obj([J.KV{"page", J.Str{page}}, J.KV{"x", J.Num{U32.show(x)}}, J.KV{"y", J.Num{U32.show(y)}}, J.KV{"w", J.Num{U32.show(w)}}, J.KV{"h", J.Num{U32.show(h)}}])

def Cef.bounds(page: String, +x0: U32, +y0: U32, x1: U32, y1: U32) -> J.Json:
  Cef.place(page, x0, y0, Cef.extent(x0, x1), Cef.extent(y0, y1))

def Cef.part(+k: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(k, ' '), i), "")

# a count written in decimal (a bad digit counts as zero)
def Cef.num(+s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Cef.num(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc))

def Cef.key.u32(+k: String, +i: Nat) -> U32:
  U32.from_nat(Cef.num(Cef.part(k, i), 0n))

# the place request for a key the window compares ("" hides)
def Cef.place.of(+key: String) -> J.Json:
  Bool.pick(J.Json, String.is_empty(key), J.Json.obj(Nil{}),
    Cef.place(Cef.part(key, 0n), Cef.key.u32(key, 1n), Cef.key.u32(key, 2n), Cef.key.u32(key, 3n), Cef.key.u32(key, 4n)))

def Cef.visible(+p: J.Json) -> Bool:
  Bool.and(Bool.not(String.is_empty(J.Json.get_str(p, "page", ""))), Bool.and(Nat.is_gt(J.Json.get_nat(p, "w", 0n), 0n), Nat.is_gt(J.Json.get_nat(p, "h", 0n), 0n)))

# the window's requests (a Rpc line, as main.bend sends them)
def Cef.rpc(method: String, p: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{method}}, J.KV{"p", p}]))

def Cef.embed(+xid: U32, display: String) -> J.Json:
  J.Json.obj([J.KV{"xid", J.Num{U32.show(xid)}}, J.KV{"display", J.Str{display}}])

# the window says its X window (decimal, "" without one): once per
# connection to this machine's hub, once that hub's log has come
def Cef.embed.due(+xid: String, +remote: String, +origin: String, +sent: String, +tag: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(xid)), String.is_empty(remote)), Bool.and(Bool.not(String.is_empty(origin)), Bool.not(String.eq(sent, tag))))

# the window's X id as kept in its scratch ("" for none)
def Cef.xid.text(+xid: U32) -> String:
  Bool.pick(String, U32.is_eq(xid, 0), "", U32.show(xid))

# Only the native local viewer consumes direct pixels. Other clients keep casts.
def Cef.native(+live: String, +remote: String) -> Bool:
  Bool.and(String.eq(live, "cef"), String.is_empty(remote))

def Cef.cast(window: Bool, live: String, remote: String) -> Bool:
  Bool.not(Bool.and(window, Cef.native(live, remote)))

# Far ids name a page on another hub; its pixels still arrive by JPEG.
def Cef.page_local(+page: String) -> Bool:
  Bool.not(String.contains(page, "~"))

# The strip over a thread's page
# ------------------------------

# A fallback must still let the person release a page they already hold.
def Cef.strip_on(+live: String, +drive: String) -> Bool:
  Bool.or(String.eq(live, "cef"), Bool.not(String.is_empty(drive)))

def Cef.drive_text(+drive: String, clickable: Bool) -> String:
  Bool.pick(String, String.is_empty(drive),
    Bool.pick(String, clickable, "Agent drives · click the page to take over", "Agent drives · view only"), "You drive")

# A requested native browser must never look like an unexplained Chrome
# default on headless/unsupported clients or while the native runtime waits.
def Cef.reason(requested: String, live: String, +err: String) -> String:
  Bool.pick(String, Bool.and(String.is_empty(err), Bool.and(String.eq(requested, "cef"), String.eq(live, "chrome"))),
    "native Linux X11 window/runtime is not active", err)

# the settings row's note: only why Embedded cannot run
def Cef.note(+err: String) -> String:
  Bool.pick(String, String.is_empty(err), "", "Chrome fallback (CEF unavailable: " ++ err ++ ")")

# Pages that exist
# ----------------
# The hub notes each page the helper has navigated (a frame may never come
# for a page that was never shown) and tells clients one key each (info
# browser.page.<id>) so the window can show it natively. The newest 64 are
# kept; one pushed out is unset for every client.

def Cef.page.key(+page: String) -> String:
  "browser.page." ++ page

def Cef.pages.has(ps: List<&2, String>, +page: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case +p <> t:
      Bool.or(String.eq(p, page), Cef.pages.has(t, page))

def Cef.pages.max() -> Nat:
  64n

def Cef.pages.add(+ps: List<&2, String>, +page: String) -> List<&2, String>:
  List.take(&2, String, page <> ps, Cef.pages.max())

# the page pushed out by adding one ("" none)
def Cef.pages.out(+ps: List<&2, String>) -> String:
  Bool.pick(String, Nat.is_lt(List.length(&2, String, ps), Cef.pages.max()), "", Maybe.default(&2, String, List.last(&2, String, ps), ""))

def Cef.pages.gone(+out: String) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, String.is_empty(out), Nil{}, [J.KV{Cef.page.key(out), J.Str{""}}])

# what clients hear when a page first exists: its key, and the unset key of
# the one pushed out ("" none)
def Cef.pages.told(+page: String, +out: String) -> List<&2, J.KV>:
  J.KV{Cef.page.key(page), J.Str{"1"}} <> Cef.pages.gone(out)

def Cef.pages.info(ps: List<&2, String>) -> List<&2, J.KV>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      J.KV{Cef.page.key(p), J.Str{"1"}} <> Cef.pages.info(t)

# The hub's state
# ---------------

# the hub steps after which the engine may have to change: a request (a
# setting, an embed), a client leaving, an engine answer, a helper event;
# never after the rest (an agent's streamed line)
def Cef.after(+kind: String) -> Bool:
  Bool.or(Bool.or(String.eq(kind, "rpc"), String.eq(kind, "leave")), Bool.or(String.eq(kind, "cef_done"), String.eq(kind, "browser_ev")))

# a helper restart re-shows the placed page at most this many times in a row
def Cef.revive(live: Bool, placed: Bool, +tries: Nat) -> Bool:
  Bool.and(Bool.and(live, placed), Nat.is_lt(tries, 3n))

type CefWork is Data:
  CefWork{op: String, args: J.Json, drive: String, token: String}

# client: the window that embedded (0 also for none: the in-process window
# is 0); xid, display: its X window; request: the engine key last asked
# (Cef.want.key); live, err: the engine running and why not cef; place: the
# page shown; shield: the last shield sent ("1", "0", "" none); local: the
# loopback clients that may embed; pages: Cef.pages; tries: Cef.revive's
# count; queue: the serial worker; generation: the engine request's token
type CefSession is Data:
  CefSession{client: U32, xid: U32, display: String, request: String, live: String, err: String, place: J.Json, shield: String, local: List<&2, U32>, pages: List<&2, String>, tries: Nat, queue: Maybe<&2, Chan(CefWork)>, generation: Nat}

def Cef.empty() -> CefSession:
  CefSession{0, 0, "", "chrome", "chrome", "", J.Null{}, "", Nil{}, Nil{}, 0n, None{}, 0n}

def Cef.session.client(s: CefSession) -> U32:
  match s:
    case CefSession{c, _, _, _, _, _, _, _, _, _, _, _, _}: c

def Cef.session.xid(s: CefSession) -> U32:
  match s:
    case CefSession{_, x, _, _, _, _, _, _, _, _, _, _, _}: x

def Cef.session.display(s: CefSession) -> String:
  match s:
    case CefSession{_, _, d, _, _, _, _, _, _, _, _, _, _}: d

def Cef.session.known(+s: CefSession) -> Bool:
  Cef.known(Cef.session.xid(s), Cef.session.display(s))

def Cef.session.request(s: CefSession) -> String:
  match s:
    case CefSession{_, _, _, r, _, _, _, _, _, _, _, _, _}: r

def Cef.session.live(s: CefSession) -> String:
  match s:
    case CefSession{_, _, _, _, l, _, _, _, _, _, _, _, _}: l

def Cef.session.place(s: CefSession) -> J.Json:
  match s:
    case CefSession{_, _, _, _, _, _, p, _, _, _, _, _, _}: p

def Cef.session.page(+s: CefSession) -> String:
  J.Json.get_str(Cef.session.place(s), "page", "")

def Cef.session.shield(s: CefSession) -> String:
  match s:
    case CefSession{_, _, _, _, _, _, _, sh, _, _, _, _, _}: sh

def Cef.session.local(s: CefSession) -> List<&2, U32>:
  match s:
    case CefSession{_, _, _, _, _, _, _, _, lo, _, _, _, _}: lo

def Cef.session.pages(s: CefSession) -> List<&2, String>:
  match s:
    case CefSession{_, _, _, _, _, _, _, _, _, pg, _, _, _}: pg

def Cef.session.tries(s: CefSession) -> Nat:
  match s:
    case CefSession{_, _, _, _, _, _, _, _, _, _, t, _, _}: t

def Cef.session.queue(s: CefSession) -> Maybe<&2, Chan(CefWork)>:
  match s:
    case CefSession{_, _, _, _, _, _, _, _, _, _, _, q, _}: q

def Cef.session.token(s: CefSession) -> String:
  match s:
    case CefSession{_, _, _, _, _, _, _, _, _, _, _, _, n}: Nat.show(n)

def Cef.current(s: CefSession, +token: String) -> Bool:
  String.eq(Cef.session.token(s), token)

def Cef.session.info(s: CefSession) -> List<&2, J.KV>:
  match s:
    case CefSession{_, _, _, _, l, e, _, _, _, _, _, _, _}:
      [J.KV{"browser.engine.live", J.Str{l}}, J.KV{"browser.engine.err", J.Str{e}}]

# a window embedded: its X window is known, the shield is to be sent again
def Cef.session.embed_set(s: CefSession, client: U32, xid: U32, display: String) -> CefSession:
  match s:
    case CefSession{_, _, _, r, l, err, _, _, lo, pg, _, q, n}: CefSession{client, xid, display, r, l, err, J.Null{}, "", lo, pg, 0n, q, n}

# the engine was asked for: the next generation
def Cef.session.request_set(s: CefSession, r: String) -> CefSession:
  match s:
    case CefSession{c, x, d, _, l, err, p, sh, lo, pg, t, q, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, q, 1n+n}

# the helper answered: the engine running, and why not cef
def Cef.session.done(s: CefSession, l: String, err: String) -> CefSession:
  match s:
    case CefSession{c, x, d, r, _, _, p, _, lo, pg, _, q, n}: CefSession{c, x, d, r, l, err, p, "", lo, pg, 0n, q, n}

# the window placed a page (or hid it): the shield sent with it
def Cef.session.place_set(s: CefSession, p: J.Json, sh: String) -> CefSession:
  match s:
    case CefSession{c, x, d, r, l, err, _, _, lo, pg, t, q, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, q, n}

def Cef.session.shield_set(s: CefSession, sh: String) -> CefSession:
  match s:
    case CefSession{c, x, d, r, l, err, p, _, lo, pg, t, q, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, q, n}

def Cef.session.local_set(s: CefSession, lo: List<&2, U32>) -> CefSession:
  match s:
    case CefSession{c, x, d, r, l, err, p, sh, _, pg, t, q, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, q, n}

def Cef.session.pages_set(s: CefSession, pg: List<&2, String>) -> CefSession:
  match s:
    case CefSession{c, x, d, r, l, err, p, sh, lo, _, t, q, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, q, n}

# a helper restart re-showed the page once more
def Cef.session.tried(s: CefSession) -> CefSession:
  match s:
    case CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, q, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, 1n+t, q, n}

def Cef.session.queued(s: CefSession, q: Chan(CefWork)) -> CefSession:
  match s:
    case CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, _, n}: CefSession{c, x, d, r, l, err, p, sh, lo, pg, t, Some{q}, n}

# a loopback client may embed (the newest 32 kept: ids are never reused)
def Cef.session.near(+s: CefSession, +client: U32) -> CefSession:
  Cef.session.local_set(s, List.take(&2, U32, client <> Cef.session.local(s), 32n))

def Cef.session.is_local(+s: CefSession, +client: U32) -> Bool:
  Cef.ids.has(Cef.session.local(s), client)

def Cef.session.leave.k(owns: Bool, s: CefSession) -> CefSession:
  match owns:
    case True{}: Cef.session.embed_set(s, 0, 0, "")
    case False{}: s

# a client left: its embed (if it was the window's) and its loopback mark go
def Cef.session.leave(+s: CefSession, +client: U32) -> CefSession:
  Cef.session.local_set(Cef.session.leave.k(Bool.and(Cef.session.known(s), U32.is_eq(client, Cef.session.client(s))), s),
    Cef.ids.drop(Cef.session.local(s), client))
