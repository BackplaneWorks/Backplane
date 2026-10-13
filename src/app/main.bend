import Base
import ../server/server.bend as S
import ../server/host.bend as H
import ../core/prompt.bend as Q
import ../core/json.bend as J
import ../core/client.bend as U
import ../gfx/raster.bend as R
import ./win.bend as W
import ./nui.bend as N
import ./layout.bend as L
import ./theme.bend as T
import ./viewer.bend as V
import ../server/plot.bend as PV
import ../core/board.bend as B
import ../gfx/draw.bend as D
import ../server/kicad.bend as K
import ../gfx/text.bend as X
import ../core/glb.bend as G
import ../gfx/mesh.bend as MS
import ../gfx/face.bend as FA
import ../core/pcb3d.bend as P3
import ./parts3.bend as PT
import ../core/schematic.bend as SC
import ../core/hier.bend as HI
import ../server/proc.bend as P
import ./link.bend as LK
import ./imgfx.bend as IF
import ../core/images.bend as IM
import ../gfx/bitmap.bend as BM
import ../gfx/tiles.bend as TI
import ../core/lightbox.bend as LBX
import ../core/model.bend as M
import ./bots.bend as BV
import ./cats.bend as CA
import ../server/perf.bend as PS
import ../core/perf.bend as PF
import ../gfx/fresh.bend as FR
import ./render.bend as RN
import ../core/mech.bend as MC
import ../core/hist.bend as HS
import ./rvw.bend as RW
import ../core/rview.bend as RV
import ../server/git.bend as GT
import ../server/review.bend as RVC

# `backplane`: the hub, the web server (for other devices), and the native
# window as client 0. Window events and hub messages meet in one queue; a
# frame is drawn at most every 12 ms and only when something changed.

type ShellMsg is Data:
  FromWin{words: List<&2, U32>}
  FromHub{tag: String, text: String}
  # this process's hub handing over its read model (S.WState)
  FromHubSt{tag: String, st: M.State, head: String}
  Frame{}
  Gone{tag: String}
  Retry{tag: String}
  Board{key: String, items: List<&2, B.Item>}
  Mesh3{key: String, scene: FA.Scene3}
  Parts3{key: String, t0: Nat, tris: List<&2, G.Tri3>}
  Status{key: String, text: String}
  Sheets{key: String, text: String}
  Shot{key: String, bmp: BM.Bitmap}
  # a round of a remote hub's files for a viewer's source came in (RW.Rvw.sync)
  RvGot{key: String, tag: String, ver: String, reread: Bool}
  PicOk{tag: String, path: String, w: U32, h: U32, rgb: List<&2, U32>}
  PicErr{tag: String, path: String, why: String}
  Blink{}
  # a desktop notification was clicked: its thread opens
  Opened{thread: String}
  # a client action come due (client.bend's Later: the history playing)
  Due{action: String, value: String}

# which hub the window talks to: its own (url "") or another machine's
# through a link; each connection gets its own tag so messages from one
# already left are dropped. Where this machine's hub is when it is not in
# this process (a `--headless` one already ran for the home, so the window
# joined it over a link: Start.run) is the client's "local.hub" ("" when it
# is).
type TieKeep is Data:
  TieKeep{pics: Map<&2, BM.Bitmap>, asked: Map<&2, Bool>, cache: List<&2, U.HubView>}

# Keep the growing connection data behind a list, as Kept does for parks:
# the window's IO continuations must stay under the native arity limit.
type Tie is Data:
  Tie{home: Chan(S.Msg), url: String, tag: String, n: Nat, kept: List<&2, TieKeep>}

def TieKeep.empty() -> List<&2, TieKeep>:
  [TieKeep{Map.new(&2, BM.Bitmap), Map.new(&2, Bool), Nil{}}]

def TieKeep.pics(ks: List<&2, TieKeep>) -> Map<&2, BM.Bitmap>:
  match ks:
    case TieKeep{ps, _, _} <> _: ps
    case Nil{}: Map.new(&2, BM.Bitmap)

def TieKeep.asked(ks: List<&2, TieKeep>) -> Map<&2, Bool>:
  match ks:
    case TieKeep{_, ak, _} <> _: ak
    case Nil{}: Map.new(&2, Bool)

def TieKeep.cache(ks: List<&2, TieKeep>) -> List<&2, U.HubView>:
  match ks:
    case TieKeep{_, _, cs} <> _: cs
    case Nil{}: Nil{}

def TieKeep.with_pics(ks: List<&2, TieKeep>, ps: Map<&2, BM.Bitmap>) -> List<&2, TieKeep>:
  match ks:
    case TieKeep{_, ak, cs} <> _: [TieKeep{ps, ak, cs}]
    case Nil{}: [TieKeep{ps, Map.new(&2, Bool), Nil{}}]

def TieKeep.with_asked(ks: List<&2, TieKeep>, ak: Map<&2, Bool>) -> List<&2, TieKeep>:
  match ks:
    case TieKeep{ps, _, cs} <> _: [TieKeep{ps, ak, cs}]
    case Nil{}: [TieKeep{Map.new(&2, BM.Bitmap), ak, Nil{}}]

def TieKeep.with_cache(ks: List<&2, TieKeep>, cs: List<&2, U.HubView>) -> List<&2, TieKeep>:
  match ks:
    case TieKeep{ps, ak, _} <> _: [TieKeep{ps, ak, cs}]
    case Nil{}: [TieKeep{Map.new(&2, BM.Bitmap), Map.new(&2, Bool), cs}]

# what one frame keeps for the next: its tiles, the timeline's memos
# (the open thread's first, then the threads shown before it), the
# bots' cats, flattened once per look and mood, the threads' parked
# viewers (one N.Park, behind a List: the window's state flattens to no
# more fields than an arity of 255, main.bend fails to build past it), and
# the posts on show wrapped (a room's, a bot's elsewhere: BV.Bv.wraps)
type Kept is Data:
  Kept{frames: TI.Frames, memos: List<&2, L.TLMemo>, cats: Map<&2, CA.CatF>, parks: List<&2, N.Park>, wraps: Map<&2, BV.PW>}

# the boards being watched and the fonts
type Aux is Data:
  Aux{watched: List<&2, String>, ty: List<&2, L.Ty>, tie: Tie, kept: List<&2, Kept>}

def Kept.none() -> Kept:
  Kept{TI.FNone{}, Nil{}, CA.Cats.none(), [N.Park.init()], BV.Bv.wraps.none()}

def Kept.of(ks: List<&2, Kept>) -> Kept:
  match ks:
    case Nil{}:
      Kept.none()
    case k <> _:
      k

type Shell is Type:
  Shell{nui: N.Nui, win: Window, dirty: Bool, queued: Bool, q: Chan(ShellMsg), inbox: Chan(S.Msg), aux: Aux}

def Pump.got(r: Window & List<&2, U32>, +q: Chan(ShellMsg), k: Window -> IO(Unit)) -> IO(Unit):
  (w, ws) = r
  do IO<Unit>:
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, FromWin{ws})
    k(w)

# Feeders
# -------

def Pump.loop(fuel: Nat, w: Window, +q: Chan(ShellMsg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Window & List<&2, U32>, Unit, W.Win.words(w), r => Pump.got(r, q, w2 => Pump.loop(f, w2, q)))

def Fwd.got(m: Maybe<&1, S.Wire>, +q: Chan(ShellMsg), +tag: String, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{S.WText{t}}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, FromHub{tag, t})
        k(Unit{})
    case Some{S.WState{st, h}}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, FromHubSt{tag, st, h})
        k(Unit{})
    case Some{S.WItems{hd, pl, _, xs, tl, _}}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, FromHub{tag, hd ++ pl ++ String.join(xs, ",") ++ tl})
        k(Unit{})
    case Some{_}:
      k(Unit{})
    case None{}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Gone{tag})
        return Unit{}

# every 32 frames the hub hears how many this client took (its credit,
# core/outbox.bend); a link to another hub ignores it (that hub's own
# writer reports)
def Fwd.told(due: Bool, +hub: Chan(S.Msg), +gen: U32, +n: Nat) -> IO(Unit):
  match due:
    case True{}:
      do IO<Unit>:
        ok : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, hub, S.Drained{0, gen, n, 0n})
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

def Fwd.counted(+hub: Chan(S.Msg), +gen: U32, +n: Nat, k: Unit -> IO(Unit)) -> IO(Unit):
  IO.bind(Unit, Unit, Fwd.told(Nat.is_eq(Nat.mod(n, 32n), 0n), hub, gen, n), k)

# a hub's messages for client 0, tagged with the connection
def Fwd.loop(fuel: Nat, +out: Chan(S.Wire), +q: Chan(ShellMsg), +tag: String, +hub: Chan(S.Msg), +gen: U32, +n: Nat) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, S.Wire>, Unit, Chan.recv(S.Wire, out), m => Fwd.got(m, q, tag, u => Fwd.counted(hub, gen, 1n+n, u2 => Fwd.loop(f, out, q, tag, hub, gen, 1n+n))))

# a connection's generation for client 0 (the window joins again under
# the same id; the hub tells the joins apart by this)
def Fwd.gen() -> IO(U32):
  do IO<U32>:
    t : U32 <- PS.Perf.us()
    return U32.or(t, 1)

# the next frame after wait ms (0: at once, behind what is queued)
def Timer.once(+q: Chan(ShellMsg), +wait: Nat) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(U32.from_nat(wait))
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Frame{})
    return Unit{}

def Frame.gap() -> Nat:
  12n

# frames keep Frame.gap() apart: a change after a quiet spell draws at once
# (a key shows in the next frame, not 12 ms later); changes that come
# faster are drawn together once the gap has passed
def Frame.wait(+last: Nat, +now: Nat) -> Nat:
  Nat.sub(Frame.gap(), Nat.sub(now, last))

def Frame.soon(+q: Chan(ShellMsg), +wait: Nat) -> IO(Unit):
  Bool.pick(IO(Unit), Nat.is_eq(wait, 0n), IO.bind(Result<&1, &1, ShellMsg, Unit>, Unit, Chan.send(ShellMsg, q, Frame{}), _ => IO.pure(Unit, Unit{})), IO.spawn(Unit, Timer.once(q, wait)))

# the caret's next toggle
def Timer.blink(+q: Chan(ShellMsg), wait: Nat) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(U32.from_nat(wait))
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Blink{})
    return Unit{}

# Drawing
# -------

def Shell.later(a: Shell) -> IO(Shell):
  match a:
    case Shell{n, w, _, True{}, +q, i, wa}:
      IO.pure(Shell, Shell{n, w, True{}, True{}, q, i, wa})
    case Shell{+n, w, _, False{}, +q, i, wa}:
      do IO<Shell>:
        now : Nat <- IO.now()
        Frame.soon(q, Frame.wait(N.Nui.ms_of(n), now))
        return Shell{n, w, True{}, True{}, q, i, wa}

# the frame's hits go in, and the hover is found again in them (dirty when
# it moved: one more frame draws it where the pointer is now)
def Shell.hits(n: N.Nui, hs: List<&2, N.Hit>, +now: Nat) -> N.Step:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, ms, vw}:
      N.Jump.settle(N.Hover.again(N.Nui{U.Ui.tick(u, now), w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}))

def Shell.hits.nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

# frames go on while something animates, or once more when the hover moved
def Shell.hits.anim(s: N.Step, anim: Bool) -> Bool:
  match s:
    case N.Step{_, _, d}:
      Bool.or(anim, d)

# a fade still running asks for the next frame
def Shell.again(anim: Bool, a: Shell) -> IO(Shell):
  match anim:
    case True{}:
      Shell.later(a)
    case False{}:
      IO.pure(Shell, a)

# a focused field asks for one frame at its caret's next toggle, unless
# one is already due
def Shell.blink.k(go: Bool, +n: N.Nui, +q: Chan(ShellMsg), +wait: Nat, +ms: Nat) -> IO(N.Nui):
  match go:
    case True{}:
      do IO<N.Nui>:
        IO.spawn(Unit, Timer.blink(q, wait))
        return N.Nui.with_ui(n, U.Ui.info_set(N.Nui.ui(n), "caret.due", Nat.show(Nat.add(ms, wait))))
    case False{}:
      IO.pure(N.Nui, n)

# Cats move at about 30 frames a second, and only while one on show moves
# and the window has the keys: each frame asks for the next, unless one
# is already due (a sleeping, failed or absent cat never asks)
def Cats.gap() -> Nat:
  33n

# while input holds the cats still (BV.Bv.calm), the next asks for the
# frame after it
def Shell.cats.k(go: Bool, +n: N.Nui, +q: Chan(ShellMsg), +wait: Nat, +ms: Nat) -> IO(N.Nui):
  match go:
    case True{}:
      do IO<N.Nui>:
        IO.spawn(Unit, Timer.blink(q, wait))
        return N.Nui.with_ui(n, U.Ui.info_set(N.Nui.ui(n), "cat.due", Nat.show(Nat.add(ms, wait))))
    case False{}:
      IO.pure(N.Nui, n)

def Shell.cats(+n: N.Nui, +q: Chan(ShellMsg), +ms: Nat) -> IO(N.Nui):
  Shell.cats.k(Bool.and(BV.Bv.awake(n, ms), Nat.is_le(Maybe.default(&2, Nat, Nat.read(U.Ui.info(N.Nui.ui(n), "cat.due")), 0n), ms)), n, q, Nat.add(BV.Bv.calm.left(n, ms), Cats.gap()), ms)

def Shell.blink(+n: N.Nui, +q: Chan(ShellMsg), +ms: Nat) -> IO(N.Nui):
  Shell.blink.k(Bool.and(Nat.is_gt(N.Blink.wait(n, ms), 0n), Nat.is_le(Maybe.default(&2, Nat, Nat.read(U.Ui.info(N.Nui.ui(n), "caret.due")), 0n), ms)),
    n, q, N.Blink.wait(n, ms), ms)

# Images in threads
# -----------------
# The layout marks each image a reply names with "img" hit regions (its
# rectangle once its size is known). New ones are decoded off the loop
# (fetched from the hub first when it is another machine's), kept per
# connection, noted in the client info ("img:<path>") so the layout gives
# them room, and drawn into the frame's base under the UI.

# the largest an image is drawn (the timeline's width, a screen's third)
def Pics.maxw() -> U32:
  760

def Pics.maxh() -> U32:
  360

def Pics.size(+w: U32, +h: U32) -> String:
  U32.show(w) ++ "x" ++ U32.show(h)

def Tie.pics(t: Tie) -> Map<&2, BM.Bitmap>:
  match t:
    case Tie{_, _, _, _, ks}:
      TieKeep.pics(ks)

def Pics.sent.ok(got: U32 & U32 & List<&2, U32>, +q: Chan(ShellMsg), +tag: String, +path: String) -> IO(Unit):
  (w, rest) = got
  (h, rgb) = rest
  do IO<Unit>:
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, PicOk{tag, path, w, h, rgb})
    return Unit{}

def Pics.sent.err(e: U32 & String, +q: Chan(ShellMsg), +tag: String, +path: String) -> IO(Unit):
  (_, why) = e
  do IO<Unit>:
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, PicErr{tag, path, why})
    return Unit{}

def Pics.sent(r: Result<&1, &1, U32 & String, U32 & U32 & List<&2, U32>>, +q: Chan(ShellMsg), +tag: String, +path: String) -> IO(Unit):
  match r:
    case Done{got}:
      Pics.sent.ok(got, q, tag, path)
    case Fail{e}:
      Pics.sent.err(e, q, tag, path)

# "full:<path>": the image for the lightbox, decoded as large as a screen
def Pics.full(+key: String) -> Bool:
  String.starts_with(key, "full:")

# "shot:<bot>:<n>": frame n of a bot's browser page, a JPEG the hub keeps
# at <home>/shots/<bot>.jpg (GET /bots/<bot>/screen.jpg on another machine)
def Shot.is(+key: String) -> Bool:
  String.starts_with(key, "shot:")

def Shot.bot(+key: String) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(key, ':'), 1n), "")

# this hub's home (where the drafts live, one level up)
def Shot.home(+n: N.Nui) -> String:
  +d = U.Ui.info(N.Nui.ui(n), "local.drafts")
  Bool.pick(String, String.ends_with(d, "/drafts"), String.take(d, Nat.sub(String.length(d), 7n)), d)

def Shot.file(+home: String, +key: String) -> String:
  home ++ "/shots/" ++ Shot.bot(key) ++ ".jpg"

def Shot.url(+key: String) -> String:
  "/bots/" ++ Shot.bot(key) ++ "/screen.jpg"

# the file an image key names: "full:" and a render's "v:<sum>:" dropped
def Pics.src(+key: String) -> String:
  MC.Mech.key.path(Bool.pick(String, Pics.full(key), String.drop(key, 5n), key))

def Pics.maxw.of(+key: String) -> U32:
  Bool.pick(U32, Bool.or(Pics.full(key), Shot.is(key)), 1600, Pics.maxw())

def Pics.maxh.of(+key: String) -> U32:
  Bool.pick(U32, Bool.or(Pics.full(key), Shot.is(key)), 1000, Pics.maxh())

def Pics.decode(+file: String, +q: Chan(ShellMsg), +tag: String, +path: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, U32 & U32 & List<&2, U32>>, Unit, IF.Imgfx.decode(file, Pics.maxw.of(path), Pics.maxh.of(path)), r => Pics.sent(r, q, tag, path))

def Pics.fetched(r: P.Ran, +q: Chan(ShellMsg), +tag: String, +path: String) -> IO(Unit):
  match r:
    case P.Ran{0, +out}:
      do IO<Unit>:
        Pics.decode(String.trim(out), q, tag, path)
        r2 : P.Ran <- P.Proc.run("rm", ["-f", String.trim(out)], "/")
        return Unit{}
    case P.Ran{_, _}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, PicErr{tag, path, "that machine did not send it"})
        return Unit{}

# the file an image key names on this machine
def Pics.file(+src: String, +home: String, +bp: String) -> String:
  Bool.pick(String, Shot.is(src), Shot.file(bp, src), Bool.pick(String, String.starts_with(src, "~/"), home ++ String.drop(src, 1n), src))

# "tshot:<port>:<path>": a task run's or pause's screenshot, the path the hub
# serves it at (/bots/<id>/tasks/shot?...); <port> reaches the hub the window
# runs
def Tshot.is(+key: String) -> Bool:
  String.starts_with(key, "tshot:")

def Tshot.port(+key: String) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(key, ':'), 1n), "")

def Tshot.path(+key: String) -> String:
  String.drop(key, Nat.add(7n, String.length(Tshot.port(key))))

# where the hub is for an image key: the window's link, else this machine
def Pics.base(+p: String, +url: String) -> String:
  Bool.pick(String, Bool.and(Tshot.is(p), String.is_empty(url)), "http://127.0.0.1:" ++ Tshot.port(p), url)

# where another machine's hub serves an image key
def Pics.route(+src: String) -> String:
  Bool.pick(String, Tshot.is(src), Tshot.path(src), Bool.pick(String, Shot.is(src), Shot.url(src), IM.Img.url(src)))

# an image on this machine is read here; another machine's hub sends it
def Pics.load(local: Bool, +path: String, +url: String, +q: Chan(ShellMsg), +tag: String, +bp: String) -> IO(Unit):
  match local:
    case True{}:
      IO.bind(String, Unit, S.Env.get("HOME", ""), home => Pics.decode(Pics.file(Pics.src(path), home, bp), q, tag, path))
    case False{}:
      IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", "f=$(mktemp) && curl -sf -m 30 -o \"$f\" \"$1\" && echo \"$f\"", "sh", url ++ Pics.route(Pics.src(path))], "/"),
        r => Pics.fetched(r, q, tag, path))

def Pics.new.k(fresh: Bool, +v: String, rest: List<&2, String>) -> List<&2, String>:
  match fresh:
    case True{}:
      v <> rest
    case False{}:
      rest

def Pics.new(hs: List<&2, N.Hit>, +asked: Map<&2, Bool>) -> List<&2, String>:
  match hs:
    case Nil{}:
      Nil{}
    case N.Hit{_, _, _, _, +a, +v} <> t:
      Pics.new.k(Bool.and(String.eq(a, "img"), Bool.not(U.Flag.get(asked, v))), v, Pics.new(t, asked))

def Pics.mark(ps: List<&2, String>, asked: Map<&2, Bool>) -> Map<&2, Bool>:
  match ps:
    case Nil{}:
      asked
    case p <> t:
      Pics.mark(t, Map.set(&2, Bool, asked, p, True{}))

def Pics.spawn(ps: List<&2, String>, +url: String, +q: Chan(ShellMsg), +tag: String, +bp: String) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +p <> t:
      do IO<Unit>:
        IO.spawn(Unit, Pics.load(Bool.and(String.is_empty(url), Bool.not(Tshot.is(p))), p, Pics.base(p, url), q, tag, bp))
        Pics.spawn(t, url, q, tag, bp)

def Pics.keep(m: Maybe<&2, BM.Bitmap>, ps: Map<&2, BM.Bitmap>, +path: String) -> Map<&2, BM.Bitmap>:
  match m:
    case Some{b}:
      Map.set(&2, BM.Bitmap, ps, path, b)
    case None{}:
      ps

# a bot's older browser frames leave when a newer one arrives
def Shot.drop.k(hit: Bool, ps: Map<&2, BM.Bitmap>, +k: String) -> Map<&2, BM.Bitmap>:
  match hit:
    case True{}:
      Map.del(&2, BM.Bitmap, ps, k)
    case False{}:
      ps

def Shot.drop(ks: List<&2, String>, +pre: String, ps: Map<&2, BM.Bitmap>) -> Map<&2, BM.Bitmap>:
  match ks:
    case Nil{}:
      ps
    case +k <> t:
      Shot.drop(t, pre, Shot.drop.k(String.starts_with(k, pre), ps, k))

def Shot.keep(ok: Bool, m: Maybe<&2, BM.Bitmap>, +ps: Map<&2, BM.Bitmap>, +path: String) -> Map<&2, BM.Bitmap>:
  match ok:
    case True{}:
      Pics.keep(m, Shot.drop(Map.keys(&2, BM.Bitmap, ps), "shot:" ++ Shot.bot(path) ++ ":", ps), path)
    case False{}:
      Pics.keep(m, ps, path)

# the frame on show for a bot is the newest one loaded
def Shot.note(+u: U.Ui, +path: String, +note: String) -> U.Ui:
  Bool.pick(U.Ui, Bool.and(Shot.is(path), Bool.not(String.starts_with(note, "err:"))), U.Ui.info_set(u, "shot.ok." ++ Shot.bot(path), path), u)

# a loaded bot frame (not an error)
def Shot.ok(+path: String, +note: String) -> Bool:
  Bool.and(Shot.is(path), Bool.not(String.starts_with(note, "err:")))

# the client notes an image's size (or why it failed)
def Pics.noted(u: U.Ui, +path: String, +note: String) -> U.Ui:
  Shot.note(U.Ui.info_set(u, "img:" ++ path, note), path, note)

def Pics.store(a: Shell, +path: String, +note: String, m: Maybe<&2, BM.Bitmap>) -> IO(Shell):
  match a:
    case Shell{+n, w, _, qd, q, i, Aux{ws, ty, Tie{home, url, tag, k, +ks}, fr}}:
      Shell.later(Shell{N.Nui.with_ui(n, Pics.noted(N.Nui.ui(n), path, note)), w, True{}, qd, q, i,
        Aux{ws, ty, Tie{home, url, tag, k, TieKeep.with_pics(ks, Shot.keep(Shot.ok(path, note), m, TieKeep.pics(ks), path))}, fr}})

# an image loaded (or failed) for this connection: keep it and redraw
def Pics.got(current: Bool, a: Shell, +path: String, +note: String, m: Maybe<&2, BM.Bitmap>) -> IO(Shell):
  match current:
    case True{}:
      Pics.store(a, path, note, m)
    case False{}:
      IO.pure(Shell, a)

def Pics.had(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Pics.had(t, x))

def Pics.uniq(ps: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +p <> t:
      Pics.uniq(t, Bool.pick(List<&2, String>, Pics.had(acc, p), acc, p <> acc))

# images the frame shows that were never asked for start loading
def Pics.ask(+hs: List<&2, N.Hit>, wa: Aux, +q: Chan(ShellMsg), +bp: String) -> IO(Aux):
  match wa:
    case Aux{ws, ty, Tie{home, +url, +tag, n, +ks}, fr}:
      +ak = TieKeep.asked(ks)
      +fresh = Pics.uniq(Pics.new(hs, ak), Nil{})
      IO.bind(Unit, Aux, Pics.spawn(fresh, url, q, tag, bp), u => IO.pure(Aux, Aux{ws, ty, Tie{home, url, tag, n, TieKeep.with_asked(ks, Pics.mark(fresh, ak))}, fr}))

# no caret blinks under the lightbox (nothing else asks for frames then)
def Lbv.blink(on: Bool, +n: N.Nui, +q: Chan(ShellMsg), +ms: Nat) -> IO(N.Nui):
  match on:
    case True{}:
      IO.pure(N.Nui, n)
    case False{}:
      Shell.blink(n, q, ms)

# the lightbox's picture, asked for as large as a screen while it is up
def Lbv.want(on: Bool, +path: String) -> List<&2, N.Hit>:
  match on:
    case True{}:
      [N.Hit{0, 0, 0, 0, "img", "full:" ++ path}]
    case False{}:
      Nil{}

def Shell.shown(r: Window & Image, +n: N.Nui, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux, anim: Bool, +ms: Nat) -> IO(Shell):
  (w, img) = r
  do IO<Shell>:
    Unit <- IO.pure(Unit, Image.drop!(img))
    +n1 : N.Nui <- Lbv.blink(N.Lbx.on(n), n, q, ms)
    +n2 : N.Nui <- Shell.cats(n1, q, ms)
    wa2 : Aux <- Pics.ask(List.append(&2, N.Hit, Lbv.want(N.Lbx.on(n2), N.Lbx.path(n2)), N.Nui.hits(n2)), wa, q, Shot.home(n2))
    Shell.again(anim, Shell{n2, w, False{}, False{}, q, i, wa2})

# a frame shown with Win.show: the last frame's image comes back too
def Shell.shown.two(r: Window & Image & Image, +n: N.Nui, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux, anim: Bool, +ms: Nat) -> IO(Shell):
  (w, ab) = r
  (img, prev) = ab
  do IO<Shell>:
    Unit <- IO.pure(Unit, Image.drop!(prev))
    Shell.shown((w, img), n, q, i, wa, anim, ms)

def Pics.found(r: Map<&2, BM.Bitmap> & BM.Bitmap) -> BM.Bitmap:
  (_, b) = r
  b

# a loaded image (none yet: an empty one, width 0)
def Pics.get(pics: Map<&2, BM.Bitmap>, +k: String) -> BM.Bitmap:
  Pics.found(Map.get(BM.Bitmap, BM.Bitmap{0, 0, 0n, Pix{0}, 0, 0, 0, 0}, pics, k))

def Pics.place(b: BM.Bitmap, +x: U32, +y: U32, +w: U32, +h: U32, +ww: U32, +hh: U32, base: Image) -> Image:
  match b:
    case BM.Bitmap{0, _, _, _, _, _, _, _}:
      base
    case BM.Bitmap{bw, bh, d, t, _, _, _, _}:
      BM.Bitmap.over(ww, hh, BM.Bitmap.move(BM.Bitmap{bw, bh, d, t, 0, 0, 0, 0}, x, y, w, h), base)

def Pics.over.k(draw: Bool, +x: U32, +y: U32, +w: U32, +h: U32, +v: String, +pics: Map<&2, BM.Bitmap>, +ww: U32, +hh: U32, base: Image) -> Image:
  match draw:
    case True{}:
      Pics.place(Pics.get(pics, v), x, y, w, h, ww, hh, base)
    case False{}:
      base

# each image once, at its rectangle, under the UI
def Pics.over(hs: List<&2, N.Hit>, +pics: Map<&2, BM.Bitmap>, +done: List<&2, String>, +ww: U32, +hh: U32, base: Image) -> Image:
  match hs:
    case Nil{}:
      base
    case N.Hit{+x0, +y0, +x1, +y1, +a, +v} <> t:
      Pics.over(t, pics, v <> done, ww, hh,
        Pics.over.k(Bool.and(Bool.and(String.eq(a, "img"), U32.is_gt((x1 - x0 : U32), 1)), Bool.not(Pics.had(done, v))), x0, y0, (x1 - x0 : U32), (y1 - y0 : U32), v, pics, ww, hh, base))

# Machines
# --------

def Tie.tag(t: Tie) -> String:
  match t:
    case Tie{_, _, g, _, _}:
      g

def Tie.url(t: Tie) -> String:
  match t:
    case Tie{_, u, _, _, _}:
      u

# where this machine's hub is joined over a link ("" when in this process)
def Shell.local(+n: N.Nui) -> String:
  U.Ui.info(N.Nui.ui(n), "local.hub")

def Shell.hub.here(inside: Bool, +home: Chan(S.Msg), +lo: String) -> IO(Chan(S.Msg)):
  match inside:
    case True{}:
      IO.pure(Chan(S.Msg), home)
    case False{}:
      LK.Link.start(lo)

# the hub to join: this machine's own (in this process, or a new link to
# the one the window joined at start), or a link to another
def Shell.hub(local: Bool, +home: Chan(S.Msg), +url: String, +lo: String) -> IO(Chan(S.Msg)):
  match local:
    case True{}:
      Shell.hub.here(String.is_empty(lo), home, lo)
    case False{}:
      LK.Link.start(url)

def Shell.joined(+i2: Chan(S.Msg), +q: Chan(ShellMsg), +tag: String, since: Nat, origin: String) -> IO(Unit):
  do IO<Unit>:
    +out : Chan(S.Wire) <- Chan.new(S.Wire, 8192)
    +gen : U32 <- Fwd.gen()
    ok : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, i2, S.Join{0, out, since, origin, gen, 0})
    IO.spawn(Unit, Fwd.loop(S.Fuel.forever(), out, q, tag, i2, gen, 0n))

# Save the departing installed state before an input step replaces it.
# The list behind Tie bounds both retained read models and record arity.
def Shell.remember(a: Shell) -> Shell:
  match a:
    case Shell{+nu, w, d, qd, q, i, Aux{ws, ty, Tie{home, url, tag, n, +ks}, fr}}:
      Shell{nu, w, d, qd, q, i, Aux{ws, ty, Tie{home, url, tag, n, TieKeep.with_cache(ks, U.HubCache.save(N.Nui.ui(nu), TieKeep.cache(ks)))}, fr}}

# the connection's hub is another machine's: where its files are copied to
# ("" both when it is this machine's own)
def Shell.remote.set(+n: N.Nui, +url: String, +pre: String) -> N.Nui:
  N.Nui.with_ui(n, U.Ui.info_set(U.Ui.info_set(N.Nui.ui(n), "local.remote.url", url), "local.remote.pre", pre))

def Shell.connect.k(t: Tie, a: Shell, +url: String) -> IO(Shell):
  match t a:
    case Tie{+home, _, _, +n, ks} Shell{+nu, w, d, qd, +q, +i, Aux{ws, ty, _, fr}}:
      do IO<Shell>:
        +cache : List<&2, U.HubView> = TieKeep.cache(ks)
        ok : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, i, S.Leave{0})
        +tag : String = url ++ "#" ++ Nat.show(n)
        +pre : String <- RW.Rvw.pre(url)
        +nu2 : N.Nui = Shell.remote.set(N.Nui.connection(nu, tag, U.HubCache.get(cache, url)), url, pre)
        +i2 : Chan(S.Msg) <- Shell.hub(String.is_empty(url), home, url, Shell.local(nu))
        Shell.joined(i2, q, tag, U.Ui.seq(N.Nui.ui(nu2)), U.Ui.info(N.Nui.ui(nu2), "origin"))
        return Shell{N.Nui.desk.reset(nu2), w, True{}, qd, q, i2, Aux{ws, ty, Tie{home, url, tag, 1n+n, [TieKeep{Map.new(&2, BM.Bitmap), Map.new(&2, Bool), cache}]}, [Kept.none()]}}

def Shell.retry.later(+q: Chan(ShellMsg), +tag: String) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(3000)
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Retry{tag})
    return Unit{}

# the caret toggled: the next frame redraws it
def Shell.blinked(a: Shell) -> Shell:
  match a:
    case Shell{n, w, _, qd, q, i, wa}:
      Shell{n, w, True{}, qd, q, i, wa}

def Shell.dirty(a: Shell) -> IO(Shell):
  match a:
    case Shell{n, w, True{}, qd, q, i, wa}:
      Shell.later(Shell{n, w, True{}, qd, q, i, wa})
    case _:
      IO.pure(Shell, a)

def Shell.queue(a: Shell, k: Chan(ShellMsg) -> Shell -> IO(Unit)) -> IO(Unit):
  match a:
    case Shell{n, w, d, qd, +q, i, wa}:
      k(q, Shell{n, w, d, qd, q, i, wa})

# The board watcher
# -----------------














def Shell.has(ws: List<&2, String>, +r: String) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case w <> t:
      Bool.or(String.eq(w, r), Shell.has(t, r))

def Aux.ty.of(t: List<&2, L.Ty>) -> L.Ty:
  match t:
    case Nil{}:
      L.Ty.none()
    case h <> _:
      h

def Aux.ty(a: Aux) -> L.Ty:
  match a:
    case Aux{_, t, _, _}:
      Aux.ty.of(t)

def Aux.watched(a: Aux) -> List<&2, String>:
  match a:
    case Aux{ws, _, _, _}:
      ws

def Aux.add(a: Aux, r: String) -> Aux:
  match a:
    case Aux{ws, t, ti, fr}:
      Aux{r <> ws, t, ti, fr}

def Aux.drop.k(same: Bool, +w: String, rest: List<&2, String>) -> List<&2, String>:
  match same:
    case True{}:
      rest
    case False{}:
      w <> rest

def Aux.drop.go(ws: List<&2, String>, +r: String) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      Aux.drop.k(String.eq(w, r), w, Aux.drop.go(t, r))

def Aux.drop(a: Aux, +r: String) -> Aux:
  match a:
    case Aux{ws, t, ti, fr}:
      Aux{Aux.drop.go(ws, r), t, ti, fr}

def Aux.tie(a: Aux) -> Tie:
  match a:
    case Aux{_, _, ti, _}:
      ti

# talk to the hub at url ("" is this machine's): leave the one now joined,
# join the other; its log replaces what the window shows
def Shell.connect.go(a: Shell, +url: String) -> IO(Shell):
  match a:
    case Shell{n, w, d, qd, q, i, Aux{ws, ty, +ti, fr}}:
      Shell.connect.k(ti, Shell{n, w, d, qd, q, i, Aux{ws, ty, ti, fr}}, url)

def Shell.connect(a: Shell, url: String) -> IO(Shell):
  Shell.connect.go(Shell.remember(a), url)

# Commands
# --------

# a link opens in the system's browser, detached (BACKPLANE_OPEN names
# another opener); the address goes as an argument, never as shell text
# a desktop notification through libnotify's notify-send (nothing when it
# is missing); one with an Open action waits for a click, which opens the
# thread here
def Shell.notify.sh() -> String:
  "command -v notify-send >/dev/null 2>&1 || exit 0; notify-send -a Backplane -i backplane -A open=Open -w -- \"$1\" \"$2\" 2>/dev/null || notify-send -a Backplane -i backplane -- \"$1\" \"$2\" 2>/dev/null"

def Shell.notified.k(open: Bool, +q: Chan(ShellMsg), thread: String) -> IO(Unit):
  match open:
    case True{}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Opened{thread})
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

def Shell.notified(r: P.Ran, +q: Chan(ShellMsg), thread: String) -> IO(Unit):
  match r:
    case P.Ran{_, +out}:
      Shell.notified.k(String.eq(String.trim(out), "open"), q, thread)

def Shell.notify(+q: Chan(ShellMsg), thread: String, +title: String, +body: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", Shell.notify.sh(), "sh", title, body], "/"), r => Shell.notified(r, q, thread))

def Shell.open(+url: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", "${BACKPLANE_OPEN:-xdg-open} \"$1\" >/dev/null 2>&1 &", "sh", url], "/"), r => IO.pure(Unit, Unit{}))

# Drafts: each thread's draft is a file in <home>/drafts, rewritten on
# every change (a temporary file renamed over it, so a crash mid-write
# leaves the last whole draft) and read back when the window opens

def Drafts.done(r: Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  IO.pure(Unit, Unit{})

def Drafts.wrote(w: File & Result<&1, &1, U32 & String, Unit>, +tmp: String, +path: String) -> IO(Unit):
  (f, _) = w
  do IO<Unit>:
    File.close(f)
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(tmp, path)
    return Unit{}

def Drafts.opened(r: Result<&1, &1, U32 & String, File>, +text: String, +tmp: String, +path: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, text), w => Drafts.wrote(w, tmp, path))
    case Fail{_}:
      IO.pure(Unit, Unit{})

def Drafts.put(empty: Bool, +text: String, +path: String) -> IO(Unit):
  match empty:
    case True{}:
      IO.bind(Result<&1, &1, U32 & String, Unit>, Unit, H.FS.remove(path), Drafts.done)
    case False{}:
      IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path ++ ".tmp", "w"), r => Drafts.opened(r, text, path ++ ".tmp", path))

def Drafts.write(+dir: String, +id: String, +text: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(dir), IO.pure(Unit, Unit{}), Drafts.put(String.is_empty(text), text, dir ++ "/" ++ id))

def Drafts.one(skip: Bool, u: U.Ui, +dir: String, +name: String) -> IO(U.Ui):
  match skip:
    case True{}:
      IO.pure(U.Ui, u)
    case False{}:
      IO.bind(String, U.Ui, S.Files.text(dir ++ "/" ++ name), t => IO.pure(U.Ui, U.Ui.restore(u, name, t)))

def Drafts.each(ns: List<String>, u: U.Ui, +dir: String) -> IO(U.Ui):
  match ns:
    case Nil{}:
      IO.pure(U.Ui, u)
    case +n <> t:
      IO.bind(U.Ui, U.Ui, Drafts.one(Bool.not(U.Draft.safe(n)), u, dir, n), u2 => Drafts.each(t, u2, dir))

def Drafts.listed(r: Result<&1, &1, U32 & String, List<String>>, u: U.Ui, +dir: String) -> IO(U.Ui):
  match r:
    case Done{ns}:
      Drafts.each(ns, u, dir)
    case Fail{_}:
      IO.pure(U.Ui, u)

# the kept drafts, into a fresh client (which also learns where they live)
def Drafts.load(u: U.Ui, +dir: String) -> IO(U.Ui):
  do IO<U.Ui>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    ns : Result<&1, &1, U32 & String, List<String>> <- H.FS.list(dir)
    Drafts.listed(ns, U.Ui.info_set(u, "local.drafts", dir), dir)

# a client action's timer (client.bend's Later)
def Timer.due(+q: Chan(ShellMsg), +ms: Nat, +action: String, +value: String) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(U32.from_nat(ms))
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Due{action, value})
    return Unit{}

def Shell.cmd(a: Shell, c: N.NCmd) -> IO(Shell):
  match a c:
    case Shell{n, w, d, qd, q, +i, wa} N.NSend{t}:
      do IO<Shell>:
        ok : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, i, S.Rpc{0, t})
        return Shell{n, w, d, qd, q, i, wa}
    case Shell{n, w, d, qd, q, i, wa} N.NCopy{t}:
      do IO<Shell>:
        w2 : Window <- W.Win.copy(w, t)
        return Shell{n, w2, d, qd, q, i, wa}
    case Shell{n, w, d, qd, q, i, wa} N.NPaste{}:
      do IO<Shell>:
        w2 : Window <- W.Win.paste(w)
        return Shell{n, w2, d, qd, q, i, wa}
    case _ N.NQuit{}:
      IO.die(Shell, 0, "")
    case _ N.NConnect{url}:
      Shell.connect(a, url)
    case Shell{n, w, d, qd, q, i, wa} N.NOpen{url}:
      do IO<Shell>:
        IO.spawn(Unit, Shell.open(url))
        return Shell{n, w, d, qd, q, i, wa}
    case Shell{+n, w, d, qd, q, i, wa} N.NKeep{id, t}:
      do IO<Shell>:
        Drafts.write(U.Ui.info(N.Nui.ui(n), "local.drafts"), id, t)
        return Shell{n, w, d, qd, q, i, wa}
    case Shell{n, w, d, qd, +q, i, wa} N.NNotify{th, ti, b}:
      do IO<Shell>:
        IO.spawn(Unit, Shell.notify(q, th, ti, b))
        return Shell{n, w, d, qd, q, i, wa}
    case Shell{n, w, d, qd, +q, i, wa} N.NLater{ms, action, value}:
      do IO<Shell>:
        IO.spawn(Unit, Timer.due(q, ms, action, value))
        return Shell{n, w, d, qd, q, i, wa}

# a connection closed: this machine's own hub (in this process) means the
# app is ending; another machine's, or this machine's joined over a link,
# is tried again in a moment; an old one is let go
def Shell.gone(current: Bool, local: Bool, a: Shell, +tag: String) -> IO(Shell):
  match current local a:
    case True{} True{} _:
      IO.die(Shell, 0, "")
    case True{} False{} Shell{+n, w, d, qd, +q, i, wa}:
      do IO<Shell>:
        IO.spawn(Unit, Shell.retry.later(q, tag))
        return Shell{N.Nui.with_ui(n, U.Ui.online(N.Nui.ui(n), False{})), w, True{}, qd, q, i, wa}
    case False{} _ _:
      IO.pure(Shell, a)

def Shell.retry(current: Bool, a: Shell, +url: String) -> IO(Shell):
  match current:
    case True{}:
      Shell.connect(a, url)
    case False{}:
      IO.pure(Shell, a)

def Shell.cmds(cs: List<&2, N.NCmd>, a: Shell) -> IO(Shell):
  match cs:
    case Nil{}:
      IO.pure(Shell, a)
    case c <> t:
      IO.bind(Shell, Shell, Shell.cmd(a, c), a2 => Shell.cmds(t, a2))

# fold a step into the app: its state, its commands, maybe a frame
def Shell.step.go(a: Shell, s: N.Step) -> IO(Shell):
  match a s:
    case Shell{_, w, d, qd, q, i, wa} N.Step{n, cs, dirty}:
      IO.bind(Shell, Shell, Shell.cmds(cs, Shell{n, w, Bool.or(d, dirty), qd, q, i, wa}), Shell.dirty)

def Shell.step(a: Shell, s: N.Step) -> IO(Shell):
  Shell.step.go(Shell.remember(a), s)

# the log's head read as usual, then the hub's read model in place of
# the (empty) replay, and the outbox sent again (N.Nui.adopt.step)
def Shell.adopt(current: Bool, a: Shell, st: M.State, +head: String) -> IO(Shell):
  match current a:
    case True{} Shell{+n, w, d, qd, q, i, wa}:
      Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.adopt.step(n, st, Maybe.default(&2, J.Json, J.Json.parse(head), J.Null{})))
    case False{} _:
      IO.pure(Shell, a)

# an action come due runs as a click would
def Shell.due(a: Shell, +action: String, +value: String) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, wa}:
      Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.act(n, action, value))

def Shell.recv(a: Shell, +text: String) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, wa}:
      Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.hear(n, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))

def Shell.from(current: Bool, a: Shell, +t: String) -> IO(Shell):
  match current:
    case True{}:
      Shell.recv(a, t)
    case False{}:
      IO.pure(Shell, a)

# messages that belong to one connection: only the current one's count
def Shell.tagged.k(m: ShellMsg, +ti: Tie, +lo: String, a: Shell) -> IO(Shell):
  match m:
    case FromHub{tag, t}:
      Shell.from(String.eq(tag, Tie.tag(ti)), a, t)
    case FromHubSt{tag, st, h}:
      Shell.adopt(String.eq(tag, Tie.tag(ti)), a, st, h)
    case Gone{tag}:
      Shell.gone(String.eq(tag, Tie.tag(ti)), Bool.and(String.is_empty(Tie.url(ti)), String.is_empty(lo)), a, Tie.tag(ti))
    case Retry{tag}:
      Shell.retry(String.eq(tag, Tie.tag(ti)), a, Tie.url(ti))
    case PicOk{tag, +path, +w, +h, rgb}:
      Pics.got(String.eq(tag, Tie.tag(ti)), a, path, Pics.size(w, h), Some{BM.Bitmap.image(w, h, rgb, 0, 0, w, h)})
    case PicErr{tag, +path, why}:
      Pics.got(String.eq(tag, Tie.tag(ti)), a, path, "err:" ++ why, None{})
    case _:
      IO.pure(Shell, a)

def Shell.tagged(a: Shell, m: ShellMsg) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, Aux{ws, ty, +ti, fr}}:
      Shell.tagged.k(m, ti, Shell.local(n), Shell{n, w, d, qd, q, i, Aux{ws, ty, ti, fr}})

# Events
# ------

def Shell.pasted(r: Window & String, +n: N.Nui, d: Bool, qd: Bool, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux) -> IO(Shell):
  (w, text) = r
  Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.paste.m(U.Tour.on(N.Nui.ui(n)), n, text))

def Shell.dropped(r: Window & String, +n: N.Nui, d: Bool, qd: Bool, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux) -> IO(Shell):
  (w, text) = r
  Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.drop.m(U.Tour.on(N.Nui.ui(n)), n, text))

def Shell.event(a: Shell, e: W.WinEv) -> IO(Shell):
  match a e:
    case Shell{n, w, d, qd, q, i, wa} W.WPaste{_}:
      IO.bind(Window & String, Shell, W.Win.pasted(w), r => Shell.pasted(r, n, d, qd, q, i, wa))
    case Shell{n, w, d, qd, q, i, wa} W.WDrop{_}:
      IO.bind(Window & String, Shell, W.Win.dropped(w), r => Shell.dropped(r, n, d, qd, q, i, wa))
    case Shell{+n, w, d, qd, q, i, +wa} _:
      Shell.step(Shell{n, w, d, qd, q, i, wa}, L.Comp.pend(Aux.ty(wa), N.Nui.event.t(U.Tour.on(N.Nui.ui(n)), n, e)))

def Shell.events.go(es: List<&2, W.WinEv>, a: Shell) -> IO(Shell):
  match es:
    case Nil{}:
      IO.pure(Shell, a)
    case e <> t:
      IO.bind(Shell, Shell, Shell.event(a, e), a2 => Shell.events.go(t, a2))

# the person was at the window just now ("act.at", BV.Bv.present)
def Shell.acted(a: Shell, +ms: Nat) -> Shell:
  match a:
    case Shell{+n, w, d, qd, q, i, wa}:
      Shell{N.Nui.with_ui(n, U.Ui.info_set(N.Nui.ui(n), "act.at", Nat.show(ms))), w, d, qd, q, i, wa}

def Shell.desk.k(a: Shell, +now: Nat) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, wa}:
      Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.desk(n, now))

# this window is a desk (core/desk.bend): after its input, the hub may
# hear which thread it shows and whether it has the focus
def Shell.desk(a: Shell) -> IO(Shell):
  do IO<Shell>:
    t : U32 <- H.Clock.epoch()
    Shell.desk.k(a, U32.to_nat(t))

def Shell.events(es: List<&2, W.WinEv>, a: Shell) -> IO(Shell):
  do IO<Shell>:
    ms : Nat <- IO.now()
    a2 : Shell <- Shell.events.go(es, Shell.acted(a, ms))
    Shell.desk(a2)

# a desktop notification clicked: its thread opens
def Shell.clicked(a: Shell, +thread: String) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, wa}:
      Shell.step(Shell{n, w, d, qd, q, i, wa}, N.Nui.act(n, "select", thread))

def Aux.with_tie(a: Aux, ti: Tie) -> Aux:
  match a:
    case Aux{ws, t, _, fr}:
      Aux{ws, t, ti, fr}

# no display: keep serving the web UI
def Shell.nowin(e: U32 & String) -> IO(Unit):
  (_, msg) = e
  IO.print_err("backplane: no window (" ++ msg ++ "); serving the web UI only")

# Viewer sources
# --------------
# A source key is "kind|root|path" (V.Src.key): kind 0 board, 1 schematic,
# 2 3D; path, when set, is a file an agent asked to show, else the
# project's canonical file of that kind.

def Src.at(+key: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(key, '|'), i)), "")

def Src.kind(+key: String) -> U32:
  Maybe.default(&2, U32, U32.read(Src.at(key, 0n)), 0)

def Watch.file.k(+kind: U32, f: K.Files, +path: String) -> String:
  match f:
    case K.Files{_, pcb, sch, _}:
      Bool.pick(String, String.is_empty(path), Bool.pick(String, U32.is_eq(kind, 1), sch, pcb), path)

# the file behind a source ("" when the project has none)
def Watch.file(+key: String) -> IO(String):
  IO.bind(K.Files, String, K.Files.read(Src.at(key, 1n)), f => IO.pure(String, Watch.file.k(Src.kind(key), f, Src.at(key, 2n))))

def Watch.say(+key: String, +text: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Status{key, text})
    return Unit{}

def Watch.sorted(m: Maybe<&2, List<&2, B.Item>>) -> Maybe<&2, List<&2, B.Item>>:
  match m:
    case Some{+xs}:
      Some{V.Scene.sort(xs)}
    case None{}:
      None{}

def Watch.sent(m: Maybe<&2, List<&2, B.Item>>, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match m:
    case Some{xs}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Board{key, xs})
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

def Watch.parse.k(sch: Bool, bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  match sch:
    case True{}:
      SC.Sch.items(bs)
    case False{}:
      B.Board.items(bs)

def Watch.parse(+kind: U32, bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  Watch.parse.k(U32.is_eq(kind, 1), bs)

# a filename-safe form of a source key
def Watch.safe(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Bool.pick(U32, Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))), c, 95)}, Watch.safe(t)}

def Watch.cli.k(rt: K.Rt) -> String:
  match rt:
    case K.Rt{cli, _, _, _}:
      cli

def Watch.cli() -> IO(String):
  do IO<String>:
    +home : String <- S.Env.get("HOME", "/tmp")
    explicit : String <- S.Env.get("BACKPLANE_KICAD_CLI", "")
    exe : String <- S.Sys.exe_dir.io()
    rt : K.Rt <- K.Rt.resolve(explicit, exe, home ++ "/.backplane-bend")
    return Watch.cli.k(rt)

def Watch.step2glb() -> IO(String):
  do IO<String>:
    +d : String <- S.Sys.exe_dir.io()
    ok : Bool <- K.Io.is_file(d ++ "/backplane-step2glb")
    return Bool.pick(String, ok, d ++ "/backplane-step2glb", "backplane-step2glb")

# how a file becomes a GLB: as it is, from STEP, or exported from a board
# (a board through kicad-cli, which says so when KiCad is not installed)
def Watch.glb.cmd(+path: String, +out: String, +cli: String, +step: String) -> String & List<&2, String>:
  Bool.pick(String & List<&2, String>, Bool.or(String.ends_with(path, ".step"), Bool.or(String.ends_with(path, ".stp"), Bool.or(String.ends_with(path, ".STEP"), String.ends_with(path, ".STP")))),
    (step, [path, out]),
    (cli, ["pcb", "export", "glb", "--subst-models", "--include-tracks", "--include-pads", "--include-zones", "--include-soldermask", "--include-silkscreen", "-f", "-o", out, path]))

def Watch.glb.run(cmd: String & List<&2, String>) -> IO(P.Ran):
  (c, args) = cmd
  K.KiCad.run(c, args)

# a part from STEP stands Z up (core/mech.bend's Mech.upright), as renders show it
def Watch.glb.up(step: Bool, ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match step:
    case True{}:
      MC.Mech.upright(ts)
    case False{}:
      ts

def Watch.glb.done(r: P.Ran, +glb: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match r:
    case P.Ran{0, _}:
      do IO<Unit>:
        bs : List<&2, U32> <- S.Files.read(glb)
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Mesh3{key, FA.Scene3.of_model(MS.Model.of(Watch.glb.up(MC.Mech.is_step(Src.at(key, 2n)), G.Mesh.tris(G.Glb.parse(bs)))))})
        return Unit{}
    case P.Ran{_, out}:
      Watch.say(key, "3D export failed: " ++ String.take(String.trim(out), 160n), q)

# a STEP's GLB from the models cache (converted once per version, shared
# with `backplane --render` and the phones' view)
def Watch.stepfile.got(+glb: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(glb), Watch.say(key, "Could not mesh this STEP file (installing FreeCAD lets Backplane mesh large ones)", q), Watch.glb.done(P.Ran{0, ""}, glb, key, q))

def Watch.stepfile(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    +step : String <- PT.Parts.step2glb()
    +cache : String <- PT.Parts.cache()
    mk : P.Ran <- P.Proc.run("mkdir", ["-p", cache], "/")
    +glb : String <- PT.Parts.glb(path, cache, step)
    Watch.stepfile.got(glb, key, q)

def Watch.mesh.k(glb: Bool, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match glb:
    case True{}:
      Watch.glb.done(P.Ran{0, ""}, path, key, q)
    case False{}:
      do IO<Unit>:
        +out : String = "/tmp/backplane-3d-" ++ String.drop(Watch.safe(key), Nat.sub(String.length(key), 120n)) ++ ".glb"
        cli : String <- Watch.cli()
        step : String <- Watch.step2glb()
        r : P.Ran <- Watch.glb.run(Watch.glb.cmd(path, out, cli, step))
        Watch.glb.done(r, out, key, q)

# Boards in 3D
# ------------
# A board is drawn from its own file at once (its face, edges and holes);
# its parts follow, each model file read once and placed at every
# footprint that uses it: those already converted all at once, the rest
# as four workers convert them (STEP to GLB by backplane-step2glb, into a
# cache keyed by the file's path, size and time). The viewer fades in what
# comes late.

# a model's triangles at every place it is used, onto acc
def Parts.placed(+glb: String, ms: List<&2, G.M4>, acc: List<&2, G.Tri3>) -> IO(List<&2, G.Tri3>):
  IO.bind(List<&2, U32>, List<&2, G.Tri3>, S.Files.read(glb), bs => IO.pure(List<&2, G.Tri3>, P3.Place.all(ms, G.Mesh.tris(G.Glb.parse(bs)), acc)))

def Parts.send(+ts: List<&2, G.Tri3>, +key: String, +t0: Nat, +q: Chan(ShellMsg)) -> IO(Unit):
  match ts:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case _:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Parts3{key, t0, ts})
        return Unit{}

type PCtx is Data:
  PCtx{board: String, dir: String, vars: List<&2, P3.KV>, cache: String, step: String, key: String, t0: Nat, q: Chan(ShellMsg)}

# a model still to convert: its file and where it goes
type Todo is Data:
  Todo{src: String, ms: List<&2, G.M4>}

# the placed triangles of the models ready so far, and the rest to convert
type Scan is Data:
  Scan{ready: List<&2, G.Tri3>, todo: List<&2, Todo>}

def Scan.k(ready: Bool, missing: Bool, +src: String, +glb: String, ms: List<&2, G.M4>, s: Scan) -> IO(Scan):
  match ready missing s:
    case True{} _ Scan{r, t}:
      IO.bind(List<&2, G.Tri3>, Scan, Parts.placed(glb, ms, r), r2 => IO.pure(Scan, Scan{r2, t}))
    case False{} False{} Scan{r, t}:
      IO.pure(Scan, Scan{r, Todo{src, ms} <> t})
    case _ _ _:
      IO.pure(Scan, s)

def Scan.g(+g: String, +src: String, ms: List<&2, G.M4>, s: Scan) -> IO(Scan):
  Scan.k(Bool.not(String.is_empty(g)), String.is_empty(src), src, g, ms, s)

def Scan.one.k(+src: String, ms: List<&2, G.M4>, +cache: String, s: Scan) -> IO(Scan):
  IO.bind(String, Scan, PT.Parts.cached(src, cache), g => Scan.g(g, src, ms, s))

def Scan.one(g: P3.Group3, +c: PCtx, s: Scan) -> IO(Scan):
  match g c:
    case P3.Group3{+raw, ms} PCtx{+board, +dir, +vars, +cache, _, _, _, _}:
      IO.bind(String, Scan, PT.Parts.file(raw, board, dir, vars, cache), src => Scan.one.k(src, ms, cache, s))

# every model found, and read at once when its GLB is ready
def Scan.go(gs: List<&2, P3.Group3>, +c: PCtx, s: Scan) -> IO(Scan):
  match gs:
    case Nil{}:
      IO.pure(Scan, s)
    case g <> t:
      IO.bind(Scan, Scan, Scan.one(g, c, s), s2 => Scan.go(t, c, s2))

def Parts.conv(t: Todo, +c: PCtx) -> IO(Unit):
  match t c:
    case Todo{+src, ms} PCtx{_, _, _, +cache, +step, +key, +t0, +q}:
      IO.bind(String, Unit, PT.Parts.glb(src, cache, step), g => IO.bind(List<&2, G.Tri3>, Unit, Parts.placed(g, ms, Nil{}), ts => Parts.send(ts, key, t0, q)))

def Parts.work(ts: List<&2, Todo>, +c: PCtx) -> IO(Unit):
  match ts:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case t <> r:
      IO.bind(Unit, Unit, Parts.conv(t, c), u => Parts.work(r, c))

type Deal4 is Data:
  Deal4{a: List<&2, Todo>, b: List<&2, Todo>, c: List<&2, Todo>, d: List<&2, Todo>}

def Deal4.go(ts: List<&2, Todo>, h: Deal4) -> Deal4:
  match ts h:
    case Nil{} _:
      h
    case t <> r Deal4{a, b, c, d}:
      Deal4.go(r, Deal4{b, c, d, t <> a})

def Parts.spawn(h: Deal4, +c: PCtx) -> IO(Unit):
  match h:
    case Deal4{a, b, cc, d}:
      do IO<Unit>:
        IO.spawn(Unit, Parts.work(a, c))
        IO.spawn(Unit, Parts.work(b, c))
        IO.spawn(Unit, Parts.work(cc, c))
        IO.spawn(Unit, Parts.work(d, c))
        return Unit{}

# the models ready in the cache go in one message; the rest are
# converted by four workers, each sent when done
def Parts.scanned(s: Scan, +c: PCtx) -> IO(Unit):
  match s c:
    case Scan{r, t} PCtx{_, _, _, _, _, +key, +t0, +q}:
      do IO<Unit>:
        Parts.send(r, key, t0, q)
        Parts.spawn(Deal4.go(t, Deal4{Nil{}, Nil{}, Nil{}, Nil{}}), c)

def Parts.start(+gs: List<&2, P3.Group3>, +board: String, +key: String, +t0: Nat, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    +cache : String <- PT.Parts.cache()
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(cache)
    +vars : List<&2, P3.KV> <- PT.Parts.vars(PT.Parts.dir(board))
    step : String <- Watch.step2glb()
    +c : PCtx = PCtx{board, PT.Parts.dir(board), vars, cache, step, key, t0, q}
    s : Scan <- Scan.go(gs, c, Scan{Nil{}, Nil{}})
    Parts.scanned(s, c)

def Watch.pcb3.k(items: Maybe<&2, List<&2, B.Item>>, p: Maybe<&2, P3.Pcb3>, +path: String, +key: String, +now: Nat, +q: Chan(ShellMsg)) -> IO(Unit):
  match items p:
    case Some{+its} Some{+pc}:
      do IO<Unit>:
        ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Mesh3{key, FA.Scene3.board(FA.Face3.of(its, pc), now)})
        Parts.start(P3.Pcb3.groups(pc), path, key, now, q)
    case _ _:
      IO.pure(Unit, Unit{})

type Read3 is Data:
  Read3{items: Maybe<&2, List<&2, B.Item>>, p: Maybe<&2, P3.Pcb3>}

# a board's items and its 3D facts, read at once
def Read3.of(+bs: List<&2, U32>) -> Read3:
  a b = B.Board.items(bs) P3.Pcb3.of(bs)
  Read3{a, b}

def Watch.pcb3.r(r: Read3, +path: String, +key: String, +now: Nat, +q: Chan(ShellMsg)) -> IO(Unit):
  match r:
    case Read3{items, p}:
      Watch.pcb3.k(items, p, path, key, now, q)

# a board: its face now, its parts as they load
def Watch.pcb3(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    +bs : List<&2, U32> <- S.Files.read(path)
    now : Nat <- IO.now()
    Watch.pcb3.r(Read3.of(bs), path, key, now, q)

def Watch.mesh.s(step: Bool, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match step:
    case True{}:
      Watch.stepfile(path, key, q)
    case False{}:
      Watch.mesh.k(String.ends_with(path, ".glb"), path, key, q)

def Watch.mesh.b(pcb: Bool, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match pcb:
    case True{}:
      Watch.pcb3(path, key, q)
    case False{}:
      Watch.mesh.s(MC.Mech.is_step(path), path, key, q)

# a 3D model: a board drawn from its file; a .glb as it is; a STEP
# converted to GLB first
def Watch.mesh(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  Watch.mesh.b(String.ends_with(path, ".kicad_pcb"), path, key, q)

# The sheet tree
# --------------
# Each schematic read also reads the tree it belongs to: the project's
# root schematic (or the file on show, when the project has none) and
# every sheet it reaches, each file once (HI.Hier.todo), at most 64.

def Tree.send(+root: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    tab : List<&2, HI.Kids> <- PV.Tree.load(64n, [root], Nil{})
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Sheets{key, HI.Hier.show(HI.Hier.rows(tab, root))})
    return Unit{}

def Watch.tree(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  IO.bind(K.Files, Unit, K.Files.read(Src.at(key, 1n)), f => Tree.send(PV.Tree.root(f, path), key, q))

def Watch.tree.if(sch: Bool, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match sch:
    case True{}:
      Watch.tree(path, key, q)
    case False{}:
      IO.pure(Unit, Unit{})

# a version that cannot be read (a file a commit lacks) shows nothing and
# says so (the scene of the version before must not stand in for it)
def Watch.sent.v(m: Maybe<&2, List<&2, B.Item>>, fixed: Bool, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match m fixed:
    case None{} True{}:
      do IO<Unit>:
        Watch.sent(Some{Nil{}}, key, q)
        Watch.say(key, PV.Pv.unread(), q)
    case _ _:
      Watch.sent(m, key, q)

def Watch.mark(+bits: U32, a: Maybe<&2, List<&2, B.Item>>, b: Maybe<&2, List<&2, B.Item>>) -> Maybe<&2, List<&2, B.Item>>:
  PV.Pv.mark(bits, a, b)

def Watch.version.of(bs: List<&2, U32>, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    Watch.sent.v(Watch.sorted(PV.Pv.version(Bool.not(Nat.is_eq(HS.Hist.kind(path), 0n)), Src.kind(key), bs)), Bool.not(Nat.is_eq(HS.Hist.kind(path), 0n)), key, q)
    Watch.tree.if(U32.is_eq(Src.kind(key), 1), path, key, q)

def Watch.version(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, GT.Git.version(path), bs => Watch.version.of(bs, path, key, q))

# a comparison (core/hist.bend): the new version's items marked against
# the old one's (none when it cannot be read: the file is new)
def Watch.compare.of(a: List<&2, U32>, b: List<&2, U32>, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  Watch.sent.v(Watch.sorted(Watch.mark(HS.Hist.bits(path), PV.Pv.side(Src.kind(key), a), PV.Pv.side(Src.kind(key), b))), PV.Pv.stamp.fixed(path), key, q)

def Watch.compare(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    a : List<&2, U32> <- GT.Git.version(HS.Hist.old(path))
    b : List<&2, U32> <- GT.Git.version(HS.Hist.new(path))
    Watch.compare.of(a, b, path, key, q)

def Watch.read.k(three: Bool, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match three:
    case True{}:
      Watch.mesh(path, key, q)
    case False{}:
      Bool.pick(IO(Unit), Nat.is_eq(HS.Hist.kind(path), 2n), Watch.compare(path, key, q), Watch.version(path, key, q))

# read, parse (in parallel) and send; a half-written file sends nothing
def Watch.read(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  Watch.read.k(U32.is_eq(Src.kind(key), 2), path, key, q)

def Watch.stat.k(v: U32 & U32 & U32) -> U32 & U32:
  (_, sm) = v
  sm

def Watch.stat(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> U32 & U32:
  match r:
    case Done{v}:
      Watch.stat.k(v)
    case Fail{_}:
      (0, 0)

def Watch.step.k(changed: Bool, re: Nat, +s2: U32, +m2: U32, +key: String, +path: String, +q: Chan(ShellMsg), k: U32 -> U32 -> Nat -> IO(Unit)) -> IO(Unit):
  match changed re:
    case True{} _:
      do IO<Unit>:
        Watch.read(path, key, q)
        k(s2, m2, 7n)
    case False{} 1n:
      do IO<Unit>:
        Watch.read(path, key, q)
        k(s2, m2, 0n)
    case False{} 1n+r:
      k(s2, m2, r)
    case False{} 0n:
      k(s2, m2, 0n)

def Watch.step(sm: U32 & U32, +sz: U32, +mt: U32, re: Nat, +key: String, +path: String, +q: Chan(ShellMsg), k: U32 -> U32 -> Nat -> IO(Unit)) -> IO(Unit):
  (+s2, +m2) = sm
  Watch.step.k(Bool.or(U32.is_ne(s2, sz), U32.is_ne(m2, mt)), re, s2, m2, key, path, q, k)

# every 150 ms: a changed size or mtime re-reads the file; one more read
# ~1 s after a change catches a second save within the same second (a 3D
# model re-exports only on a change: exports take seconds)
def Watch.loop(fuel: Nat, +key: String, +path: String, +sz: U32, +mt: U32, re: Nat, +q: Chan(ShellMsg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        IO.sleep(150)
        st : U32 & U32 <- PV.Pv.stamp.of(path)
        Watch.step(st, sz, mt, Bool.pick(Nat, U32.is_eq(Src.kind(key), 2), 0n, re), key, path, q, s2 => m2 => r2 => Watch.loop(f, key, path, s2, m2, r2, q))

def Watch.seq(h: BM.BmHead) -> U32:
  match h:
    case BM.BmHead{_, _, s}:
      s

# The agents' browser: its frame file starts with a 16-byte header whose
# seq grows with each frame; poll the header, load the pixels on a change

def Watch.head.k(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(U32):
  (f, res) = r
  do IO<U32>:
    File.close(f)
    return Watch.seq(BM.Bitmap.head(Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})))

def Watch.head.o(r: Result<&1, &1, U32 & String, File>) -> IO(U32):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, U32, File.read_bytes(f, 16), Watch.head.k)
    case Fail{_}:
      IO.pure(U32, 0)

def Watch.head(+path: String) -> IO(U32):
  IO.bind(Result<&1, &1, U32 & String, File>, U32, File.open(path, "r"), Watch.head.o)

def Watch.w(h: BM.BmHead) -> U32:
  match h:
    case BM.BmHead{w, _, _}:
      w

def Watch.h(h: BM.BmHead) -> U32:
  match h:
    case BM.BmHead{_, h2, _}:
      h2

def Watch.shot(+path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    +bs : List<&2, U32> <- S.Files.read(path)
    ok : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, Shot{key, BM.Bitmap.load(bs, 0, 0, Watch.w(BM.Bitmap.head(bs)), Watch.h(BM.Bitmap.head(bs)))})
    return Unit{}

def Watch.shots.k(changed: Bool, +seq: U32, +key: String, +path: String, +q: Chan(ShellMsg), k: U32 -> IO(Unit)) -> IO(Unit):
  match changed:
    case True{}:
      do IO<Unit>:
        Watch.shot(path, key, q)
        k(seq)
    case False{}:
      k(seq)

def Watch.shots(fuel: Nat, +key: String, +path: String, +last: U32, +q: Chan(ShellMsg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        IO.sleep(120)
        +seq : U32 <- Watch.head(path)
        Watch.shots.k(Bool.and(U32.is_ne(seq, last), U32.is_ne(seq, 0)), seq, key, path, q, s2 => Watch.shots(f, key, path, s2, q))

def Watch.none(+key: String) -> String:
  Bool.pick(String, U32.is_eq(Src.kind(key), 1), "No schematic in this project yet", "No board in this project yet")

def Watch.go(none: Bool, once: Bool, +key: String, +path: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match none once:
    case True{} _:
      Watch.say(key, Watch.none(key), q)
    case False{} True{}:
      Bool.pick(IO(Unit), U32.is_eq(Src.kind(key), 3), Watch.shot(path, key, q), Watch.read(path, key, q))
    case False{} False{}:
      Bool.pick(IO(Unit), U32.is_eq(Src.kind(key), 3), Watch.shots(S.Fuel.forever(), key, path, 0, q),
        Bool.pick(IO(Unit), PV.Pv.stamp.fixed(path), Watch.read(path, key, q), Watch.loop(S.Fuel.forever(), key, path, 0, 0, 0n, q)))

# A source on another machine's hub (core/rview.bend): the files are
# copies in the window's cache that RW.Rvw.sync keeps (spec: the hub, the
# folder of copies, the thread, the connection), so the viewers read them
# once after each round that changed something instead of watching them; a
# version in a commit is asked of the hub (its git directory is there).
# (a file of a commit through the hub; the working copy, a comparison's
# usual other side, from the copy here)
def Watch.hist.r.k(k: Nat, +spec: String, +key: String, +path: String) -> IO(List<&2, U32>):
  match k:
    case 1n:
      RW.Rvw.git(RV.Rview.part(spec, 0n), RV.Rview.part(spec, 1n), RV.Rview.part(spec, 2n), RV.Rview.back(RV.Rview.part(spec, 1n), Src.at(key, 1n)), path)
    case _:
      GT.Git.version(RV.Rview.at(RV.Rview.part(spec, 1n), path))

def Watch.hist.r(+spec: String, +key: String, +path: String) -> IO(List<&2, U32>):
  Watch.hist.r.k(HS.Hist.kind(path), spec, key, path)

def Watch.version.r(+spec: String, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, Watch.hist.r(spec, key, path), bs => Watch.version.of(bs, path, key, q))

def Watch.compare.r(+spec: String, +path: String, +key: String, +q: Chan(ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    a : List<&2, U32> <- Watch.hist.r(spec, key, HS.Hist.old(path))
    b : List<&2, U32> <- Watch.hist.r(spec, key, HS.Hist.new(path))
    Watch.compare.of(a, b, path, key, q)

def Watch.go.r(k: Nat, +spec: String, none: Bool, +key: String, +path: String, +q: Chan(ShellMsg)) -> IO(Unit):
  match k:
    case 1n:
      Watch.version.r(spec, path, key, q)
    case 2n:
      Watch.compare.r(spec, path, key, q)
    case _:
      Watch.go(none, True{}, key, path, q)

# watch a source (or, when it is watched already, read it once)
def Watch.start(once: Bool, +key: String, +q: Chan(ShellMsg), +spec: String) -> IO(Unit):
  do IO<Unit>:
    +path : String <- Watch.file(key)
    Bool.pick(IO(Unit), String.is_empty(spec), Watch.go(String.is_empty(path), once, key, path, q),
      Watch.go.r(HS.Hist.kind(path), spec, String.is_empty(path), key, path, q))

# one round for a source on another hub; the window decides whether
# another follows (RvGot, Shell.rvgot), so a shut viewer asks no more
def Watch.follow.done(g: RW.Got, +key: String, +q: Chan(ShellMsg), +tag: String) -> IO(Unit):
  match g:
    case RW.Got{+ok, +ver, reread, +why}:
      do IO<Unit>:
        Bool.pick(IO(Unit), ok, IO.pure(Unit, Unit{}), Watch.say(key, why, q))
        Bool.pick(IO(Unit), ok, IO.pure(Unit, Unit{}), IO.sleep(5000))
        sent : Result<&1, &1, ShellMsg, Unit> <- Chan.send(ShellMsg, q, RvGot{key, tag, ver, reread})
        return Unit{}

def Watch.follow(+key: String, +q: Chan(ShellMsg), +spec: String, +have: String) -> IO(Unit):
  IO.bind(RW.Got, Unit, RW.Rvw.sync(RV.Rview.part(spec, 0n), RV.Rview.part(spec, 1n), RV.Rview.part(spec, 2n), RV.Rview.back(RV.Rview.part(spec, 1n), Src.at(key, 1n)), RV.Rview.want(RV.Rview.part(spec, 1n), Src.at(key, 1n), Src.at(key, 2n)), have),
    g => Watch.follow.done(g, key, q, RV.Rview.part(spec, 3n)))

def Watch.begin.k(remote: Bool, once: Bool, +key: String, +q: Chan(ShellMsg), +spec: String) -> IO(Unit):
  match remote once:
    case True{} False{}:
      Watch.follow(key, q, spec, "")
    case _ _:
      Watch.start(once, key, q, spec)

# a source starts: a hub's own file is watched, another hub's followed
def Watch.begin(once: Bool, +key: String, +q: Chan(ShellMsg), +spec: String) -> IO(Unit):
  Watch.begin.k(Bool.not(String.is_empty(spec)), once, key, q, spec)

# what a fetching fiber is told when the window's hub is another machine's
# (core/rview.bend's Rview.spec; "" when it is this machine's)
def Shell.remote(+n: N.Nui, +tag: String) -> String:
  RV.Rview.spec(U.Ui.info(N.Nui.ui(n), "local.remote.url"), U.Ui.info(N.Nui.ui(n), "local.remote.pre"), U.Ui.sel(N.Nui.ui(n)), tag)

def Shell.watch.run(go: Bool, +key: String, +n: N.Nui, w: Window, d: Bool, qd: Bool, +q: Chan(ShellMsg), i: Chan(S.Msg), +wa: Aux) -> IO(Shell):
  match go:
    case True{}:
      do IO<Shell>:
        IO.spawn(Unit, Watch.begin(Shell.has(Aux.watched(wa), key), key, q, Shell.remote(n, Tie.tag(Aux.tie(wa)))))
        return Shell{N.Nui.asked(n, key), w, True{}, qd, q, i, Aux.add(wa, key)}
    case False{}:
      IO.pure(Shell, Shell{n, w, d, qd, q, i, wa})

# a source of another hub whose rounds stopped (its viewer was shut) is
# asked for again when the viewer is open on it
def Shell.stale(+n: N.Nui, +wa: Aux, +key: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Shell.remote(n, ""))), Bool.not(Shell.has(Aux.watched(wa), key)))

def Shell.watch.go(open: Bool, differs: Bool, +key: String, a: Shell) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, +wa}:
      Shell.watch.run(Bool.and(Bool.and(open, Bool.or(differs, Shell.stale(n, wa, key))), Bool.not(String.ends_with(key, "||"))), key, n, w, d, qd, q, i, wa)

# when the viewer is open on a source it has not asked for, ask
def Shell.watch(a: Shell) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, wa}:
      Shell.watch.go(N.Nui.panel(n), Bool.not(String.eq(N.Nui.source(n), N.Vx.shown(N.Vw.x(N.Nui.vw(n))))), N.Nui.source(n), Shell{n, w, d, qd, q, i, wa})

# a round of a remote hub's files came in: the viewer reads the copies
# again when they changed, and another round starts while the viewer is
# still open on that source of that connection; else the source is let go
# (Shell.stale asks for it again when the viewer returns)
def Watch.reread(reread: Bool, +key: String, +q: Chan(ShellMsg), +spec: String) -> IO(Unit):
  match reread:
    case True{}:
      IO.spawn(Unit, Watch.start(True{}, key, q, spec))
    case False{}:
      IO.pure(Unit, Unit{})

def Shell.rvgot.k(wanted: Bool, +key: String, +ver: String, reread: Bool, a: Shell) -> IO(Shell):
  match wanted a:
    case True{} Shell{+n, w, d, qd, +q, i, +wa}:
      do IO<Shell>:
        IO.spawn(Unit, Watch.follow(key, q, Shell.remote(n, Tie.tag(Aux.tie(wa))), ver))
        Watch.reread(reread, key, q, Shell.remote(n, Tie.tag(Aux.tie(wa))))
        return Shell{n, w, d, qd, q, i, wa}
    case False{} Shell{n, w, d, qd, q, i, wa}:
      IO.pure(Shell, Shell{n, w, d, qd, q, i, Aux.drop(wa, key)})

def Shell.rvgot(a: Shell, +key: String, +tag: String, +ver: String, reread: Bool) -> IO(Shell):
  match a:
    case Shell{+n, w, d, qd, q, i, +wa}:
      Shell.rvgot.k(Bool.and(String.eq(tag, Tie.tag(Aux.tie(wa))), Bool.and(N.Nui.panel(n), String.eq(N.Nui.source(n), key))), key, ver, reread, Shell{n, w, d, qd, q, i, wa})

def Shell.board(a: Shell, +key: String, +items: List<&2, B.Item>) -> IO(Shell):
  match a:
    case Shell{n, w, _, qd, q, i, wa}:
      do IO<Shell>:
        ms : Nat <- IO.now()
        Shell.later(Shell{N.Nui.board(n, key, items, ms), w, True{}, qd, q, i, wa})

def Shell.parts(a: Shell, +key: String, +t0: Nat, ts: List<&2, G.Tri3>) -> IO(Shell):
  match a:
    case Shell{n, w, _, qd, q, i, wa}:
      do IO<Shell>:
        ms : Nat <- IO.now()
        Shell.later(Shell{N.Nui.parts(n, key, t0, ts, ms), w, True{}, qd, q, i, wa})

def Shell.mesh(a: Shell, +key: String, ts: FA.Scene3) -> IO(Shell):
  match a:
    case Shell{n, w, _, qd, q, i, wa}:
      do IO<Shell>:
        ms : Nat <- IO.now()
        Shell.later(Shell{N.Nui.mesh(n, key, FA.Scene3.got(ts, ms)), w, True{}, qd, q, i, wa})

def Shell.shot(a: Shell, +key: String, b: BM.Bitmap) -> IO(Shell):
  match a:
    case Shell{n, w, _, qd, q, i, wa}:
      Shell.later(Shell{N.Nui.shot(n, key, b), w, True{}, qd, q, i, wa})

# the schematic's sheet tree, for the viewer's sheet list and trail
def Shell.sheets(a: Shell, +text: String) -> IO(Shell):
  match a:
    case Shell{+n, w, _, qd, q, i, wa}:
      Shell.later(Shell{N.Nui.with_ui(n, U.Ui.scratch_set(N.Nui.ui(n), "@vw.tree", text)), w, True{}, qd, q, i, wa})

def Shell.status(a: Shell, +key: String, +text: String) -> IO(Shell):
  match a:
    case Shell{n, w, _, qd, q, i, wa}:
      Shell.later(Shell{N.Nui.status(n, key, text), w, True{}, qd, q, i, wa})

# Viewer frames
# -------------

def Vx.scene(x: V.Vx) -> FA.Scene3:
  match x:
    case V.Vx{_, _, s, _, _, _, _, _, _, _, bm}:
      s

def Shell.busy(vw: V.Vw, +now: Nat) -> Bool:
  match vw:
    case V.Vw{ss, _, _, _, _, _, _, _, _, _, vx}:
      Bool.or(V.Fade.busy(ss, now), FA.Scene3.fading(Vx.scene(vx), now))

def Vx.kind(x: V.Vx) -> U32:
  match x:
    case V.Vx{k, _, _, _, _, _, _, _, _, _, bm}:
      k

# the browser's frame, scaled down to fit the panel and centred
def Shell.shot.base(+ww: U32, +hh: U32, +x0: U32, +flip: U32, b: BM.Bitmap) -> Image:
  match b:
    case BM.Bitmap{+w, +h, d, t, _, _, _, _}:
      +pw = {(ww - x0 : U32) : U32}
      +ph = {(hh - 52 : U32) : U32}
      +sc = {F32.min(1.0, F32.min(F32.div(U32.to_f32(pw), F32.max(U32.to_f32(w), 1.0)), F32.div(U32.to_f32(ph), F32.max(U32.to_f32(h), 1.0)))) : F32}
      +fw = {D.Draw.floor(F32.mul(U32.to_f32(w), sc)) : U32}
      +fh = {D.Draw.floor(F32.mul(U32.to_f32(h), sc)) : U32}
      BM.Bitmap.over(ww, hh, BM.Bitmap.move(BM.Bitmap{w, h, d, t, 0, 0, 0, 0}, (x0 + U32.shr((pw - fw : U32)) : U32), (52 + U32.shr((ph - fh : U32)) : U32), fw, fh),
        R.Frame.render(ww, hh, [D.Draw.rect(D.Clip.all(), x0, 52, pw, ph, V.Look.ground(flip), 255)], L.Theme.ground(flip)))

# the viewer's settled scene, drawn once per scene/view (the frame's base,
# plain background left of the panel, so frames compare it unchanged):
# a 3D model rendered into the panel, or the board/schematic's items drawn
# as tiles over every core
# half a frame's size, rounded up (a frame half as big spans one level
# less of the quadtree, so its image serves as the full frame's, each
# pixel four)
def Half.of(+n: U32) -> U32:
  U32.shr((n + 1 : U32))

# a 3D scene into the panel; while it is dragged, a draft at half size,
# at rest four samples a pixel
def Shell.base3(draft: Bool, +ww: U32, +hh: U32, +x0: U32, ts: FA.Scene3, +c: MS.Cam, +now: Nat, +bg: U32) -> Image:
  match draft:
    case True{}:
      FA.Scene3.render(Half.of(ww), Half.of(hh), Half.of(x0), 26, (Half.of(ww) - Half.of(x0) : U32), (Half.of(hh) - 26 : U32), ts, c, now, bg)
    case False{}:
      FA.Scene3.render.q(True{}, ww, hh, x0, 52, (ww - x0 : U32), (hh - 52 : U32), ts, c, now, bg)

def Shell.base(+ww: U32, +hh: U32, +x0: U32, +ss: List<&2, V.Seen>, +v: V.View, +now: Nat, +flip: U32, +fk: U32, +drag: Bool, x: V.Vx) -> Image:
  match x:
    case V.Vx{2, _, ts, c, _, _, _, _, _, _, bm}:
      R.Frame.over(ww, hh, [D.Draw.rect(D.Clip.all(), 264, 0, (x0 - 264 : U32), hh, L.Theme.ground(flip), 255)], Shell.base3(drag, ww, hh, x0, ts, V.Vx.cam(c), now, V.Look.ground(flip)))
    case V.Vx{3, _, _, _, _, _, _, _, _, _, bm}:
      Shell.shot.base(ww, hh, x0, flip, bm)
    case V.Vx{+k, _, _, _, _, _, _, _, _, _, bm}:
      FR.Fresh.frame(ww, hh, List.reverse(&2, R.Op, D.Draw.rect(D.Clip.all(), x0, 52, (ww - x0 : U32), hh, V.Look.ground(flip), 255)
        <> V.Ops.scene(ss, v, D.Clip{x0, 52, ww, hh}, now, now, True{}, U32.or(k, fk), flip)), L.Theme.ground(flip))

def Shell.vw.k(redraw: Bool, vw: V.Vw, +ww: U32, +hh: U32, +x0: U32, +now: Nat, +flip: U32, +fk: U32) -> V.Vw:
  match redraw vw:
    case True{} V.Vw{+ss, +v, f, r, +d, lx, ly, _, _, _, +vx}:
      V.Vw{ss, v, f, r, d, lx, ly, Shell.base(ww, hh, x0, ss, v, now, flip, fk, d, vx), True{}, now, vx}
    case False{} _:
      vw

# refresh the base if the scene or view changed, or fades ended since
def Shell.vw(+vw: V.Vw, +ww: U32, +hh: U32, +x0: U32, +now: Nat, +flip: U32, +fk: U32) -> V.Vw:
  match vw:
    case V.Vw{+ss, +v, f, r, d, lx, ly, b, +fr, +cut, vx}:
      Shell.vw.k(Bool.or(Bool.or(Bool.not(fr), V.Vw.late(ss, cut, now)), FA.Scene3.fading(Vx.scene(vx), cut)), V.Vw{ss, v, f, r, d, lx, ly, b, fr, cut, vx}, ww, hh, x0, now, flip, fk)

def Shell.vw.if(open: Bool, +vw: V.Vw, +ww: U32, +hh: U32, +x0: U32, +ms: Nat, +flip: U32, +fk: U32) -> V.Vw:
  match open:
    case True{}:
      Shell.vw(vw, ww, hh, x0, ms, flip, fk)
    case False{}:
      vw

def Vx.pick(x: V.Vx) -> String:
  match x:
    case V.Vx{_, p, _, _, _, _, _, _, _, _, bm}:
      p

# UI ops over the fading items and the picked item, over the base; tiles
# the last frame drew the same way are kept (the plan of TI.Frame.par)
def Shell.image.over(+ops: List<&2, R.Op>, vw: V.Vw, +ww: U32, +hh: U32, +x0: U32, +now: Nat, +flip: U32, +fk: U32, +hs: List<&2, N.Hit>, +pics: Map<&2, BM.Bitmap>, prev: TI.Frames, +dy: U32) -> TI.Plan:
  match vw:
    case V.Vw{+ss, +v, _, _, _, _, _, +b, _, +cut, +vx}:
      TI.Frame.plan(ww, hh, List.append(&2, R.Op, L.Theme.ops(V.Look.ui(flip), ops), List.reverse(&2, R.Op,
        List.append(&2, R.Op, Bool.pick(List<&2, R.Op>, U32.is_eq(Vx.kind(vx), 2), Nil{}, V.Ops.scene(ss, v, D.Clip{x0, 52, ww, hh}, now, cut, False{}, U32.or(Vx.kind(vx), fk), flip)),
          V.Ops.picked(ss, Vx.pick(vx), v, D.Clip{x0, 52, ww, hh}, flip)))),
        Pics.over(hs, pics, Nil{}, ww, hh, b), prev, dy)

# the frame: UI ops over the fading items over the base. dy is how far
# the thread scrolled since the last frame: tiles it moved are taken from
# there, not drawn again
def Shell.image(pa: Bool, +ops: List<&2, R.Op>, +vw: V.Vw, +ww: U32, +hh: U32, +x0: U32, +now: Nat, +flip: U32, +fk: U32, +hs: List<&2, N.Hit>, +pics: Map<&2, BM.Bitmap>, prev: TI.Frames, +dy: U32) -> TI.Plan:
  match pa:
    case True{}:
      Shell.image.over(ops, vw, ww, hh, x0, now, flip, fk, hs, pics, prev, dy)
    case False{}:
      TI.Frame.plan(ww, hh, L.Theme.ops(V.Look.ui(flip), ops), Pics.over(hs, pics, Nil{}, ww, hh, Pix{L.Theme.ground(flip)}), prev, dy)

def Aux.kept(a: Aux) -> Kept:
  match a:
    case Aux{_, _, _, k}:
      Kept.of(k)

def Aux.with_kept(a: Aux, k: Kept) -> Aux:
  match a:
    case Aux{ws, ty, ti, _}:
      Aux{ws, ty, ti, [k]}

def Kept.frames(k: Kept) -> TI.Frames:
  match k:
    case Kept{f, _, _, _, _}:
      f

def Kept.with_frames(k: Kept, f: TI.Frames) -> Kept:
  match k:
    case Kept{_, m, cs, pk, wr}:
      Kept{f, m, cs, pk, wr}

def Aux.frames(a: Aux) -> TI.Frames:
  Kept.frames(Aux.kept(a))

def Aux.with_frames(+a: Aux, f: TI.Frames) -> Aux:
  Aux.with_kept(a, Kept.with_frames(Aux.kept(a), f))

def Kept.memos(k: Kept) -> List<&2, L.TLMemo>:
  match k:
    case Kept{_, m, _, _, _}:
      m

def Kept.with_memos(k: Kept, m: List<&2, L.TLMemo>) -> Kept:
  match k:
    case Kept{f, _, cs, pk, wr}:
      Kept{f, m, cs, pk, wr}

def Aux.memos(a: Aux) -> List<&2, L.TLMemo>:
  Kept.memos(Aux.kept(a))

def Aux.with_memos(+a: Aux, m: List<&2, L.TLMemo>) -> Aux:
  Aux.with_kept(a, Kept.with_memos(Aux.kept(a), m))

def Kept.cats(k: Kept) -> Map<&2, CA.CatF>:
  match k:
    case Kept{_, _, cs, _, _}:
      cs

def Kept.with_cats(k: Kept, cs: Map<&2, CA.CatF>) -> Kept:
  match k:
    case Kept{f, m, _, pk, wr}:
      Kept{f, m, cs, pk, wr}

def Aux.cats(a: Aux) -> Map<&2, CA.CatF>:
  Kept.cats(Aux.kept(a))

def Aux.with_cats(+a: Aux, cs: Map<&2, CA.CatF>) -> Aux:
  Aux.with_kept(a, Kept.with_cats(Aux.kept(a), cs))

def Kept.wraps(k: Kept) -> Map<&2, BV.PW>:
  match k:
    case Kept{_, _, _, _, wr}:
      wr

def Kept.with_wraps(k: Kept, wr: Map<&2, BV.PW>) -> Kept:
  match k:
    case Kept{f, m, cs, pk, _}:
      Kept{f, m, cs, pk, wr}

def Aux.wraps(a: Aux) -> Map<&2, BV.PW>:
  Kept.wraps(Aux.kept(a))

def Aux.with_wraps(+a: Aux, wr: Map<&2, BV.PW>) -> Aux:
  Aux.with_kept(a, Kept.with_wraps(Aux.kept(a), wr))

# frames go on while a fade, a hover ease or a lightbox ease runs
def Shell.anim(+n: N.Nui, +ms: Nat) -> Bool:
  Bool.or(Bool.or(Bool.and(N.Nui.panel(n), Shell.busy(N.Nui.vw(n), ms)), N.Hover.busy(n, ms)), N.Lbx.busy(n, ms))

# a frame is on screen: it ends the frame's span (begun at t0) and the
# input and hub messages waiting for it (docs/perf.md)
def Shell.timed(+t0: U32) -> IO(Unit):
  do IO<Unit>:
    t : U32 <- PS.Perf.lap("ui.frame", t0)
    PS.Perf.done("ui.input")
    PS.Perf.done("ui.hub")

# the last frame shown from tiles (none yet: a solid one, never the same
# node as a frame's)
def Frames.image(f: TI.Frames) -> Image:
  match f:
    case TI.FNone{}:
      Pix{0}
    case TI.FSome{_, _, t}:
      TI.Tiles.image(t)

# carry out the frame's plan (TI.Frame.par one step at a time: the plan,
# the tiles to draw drawn in parallel, the tiles filled in), present them
# and keep them for the next frame; the window fills only what differs
# from the last one
def Shell.present(+pl: TI.Plan, o: L.Out, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t2: U32) -> IO(Shell):
  match o:
    case L.Out{_, +hs}:
      do IO<Shell>:
        +k : Nat = TI.Plan.count(pl)
        ta : U32 <- PS.Perf.lap("ui.plan", t2)
        PS.Perf.add("ui.drawn", U32.from_nat(k))
        +d : TI.Drawn = TI.Plan.drawn(pl, TI.Frame.edge(N.Nui.width(n), N.Nui.height(n)))
        tb : U32 <- PS.Perf.lap("ui.draw", ta)
        +tiles : TI.Tiles = TI.Plan.fill(pl, TI.Frame.edge(N.Nui.width(n), N.Nui.height(n)), 0n, d)
        tc : U32 <- PS.Perf.lap("ui.fill", tb)
        t3 : U32 <- PS.Perf.lap("ui.raster", t2)
        r : Window & Image & Image <- W.Win.show(w, TI.Tiles.image(tiles), Frames.image(Aux.frames(wa)))
        t4 : U32 <- PS.Perf.lap("ui.present", tc)
        Shell.timed(t0)
        +s : N.Step = Shell.hits(n, hs, epoch)
        Shell.shown.two(r, Shell.hits.nui(s), q, i, Aux.with_frames(wa, TI.FSome{N.Nui.width(n), N.Nui.height(n), tiles}), Shell.hits.anim(s, Shell.anim(n, ms)), ms)

# The lightbox (nui.bend's Lbx, core/lightbox.bend's Lb): the picture
# alone on a dark window, zoomed and panned, its full decode once there
# (else the thread's copy), and the close square on top
def Lbv.bg() -> U32:
  1052688

def Lbv.pick(full: BM.Bitmap, +thumb: BM.Bitmap) -> BM.Bitmap:
  match full:
    case BM.Bitmap{0, _, _, _, _, _, _, _}:
      thumb
    case BM.Bitmap{w, h, d, t, x0, y0, fw, fh}:
      BM.Bitmap{w, h, d, t, x0, y0, fw, fh}

def Lbv.pic(b: BM.Bitmap, +x: LBX.LbAx, +y: LBX.LbAx, +ww: U32, +hh: U32) -> Image:
  match b:
    case BM.Bitmap{0, _, _, _, _, _, _, _}:
      Pix{Lbv.bg()}
    case BM.Bitmap{bw, bh, d, t, x0, y0, fw, fh}:
      BM.Bitmap.view(ww, hh, BM.Bitmap{bw, bh, d, t, x0, y0, fw, fh},
        U32.from_nat(LBX.LbAx.at(x)), U32.from_nat(LBX.LbAx.at(y)), U32.from_nat(LBX.LbAx.d(x)), U32.from_nat(LBX.LbAx.d(y)),
        U32.from_nat(LBX.LbAx.s(x)), U32.from_nat(LBX.LbAx.s(y)), U32.from_nat(LBX.LbAx.len(x, U32.to_nat(ww))), U32.from_nat(LBX.LbAx.len(y, U32.to_nat(hh))), Pix{Lbv.bg()})

# the close square (LBX.Lb.close_hit's): a frame with an x in it (ops
# topmost first, as R.Frame.over takes them)
def Lbv.close(+ww: U32) -> List<&2, R.Op>:
  +x = U32.from_nat(LBX.Lb.close_x(U32.to_nat(ww)))
  List.append(&2, R.Op, [D.Draw.seg(D.Clip.all(), U32.to_f32((x + 12 : U32)), 24.0, U32.to_f32((x + 24 : U32)), 36.0, 1.5, T.Th.text(), 255),
      D.Draw.seg(D.Clip.all(), U32.to_f32((x + 24 : U32)), 24.0, U32.to_f32((x + 12 : U32)), 36.0, 1.5, T.Th.text(), 255)],
    List.append(&2, R.Op, D.Draw.frame(D.Clip.all(), x, 12, 36, 36, T.Th.line()), [D.Draw.rect(D.Clip.all(), x, 12, 36, 36, T.Th.panel(), 255)]))

def Lbv.draw(+n: N.Nui, +pics: Map<&2, BM.Bitmap>, +ms: Nat) -> Image:
  R.Frame.over(N.Nui.width(n), N.Nui.height(n), Lbv.close(N.Nui.width(n)),
    Lbv.pic(Lbv.pick(Pics.get(pics, "full:" ++ N.Lbx.path(n)), Pics.get(pics, N.Lbx.path(n))),
      LBX.Lb.x(N.Lbx.get(n), N.Lbx.box(n), ms), LBX.Lb.y(N.Lbx.get(n), N.Lbx.box(n), ms), N.Nui.width(n), N.Nui.height(n)))

# the frame: the lightbox while it is up, else the window's tiles. The
# lightbox is drawn whole and leaves the kept tiles alone: they still hold
# the window's last frame, which is what shows again when it closes
def Lbv.paint(on: Bool, +ops: List<&2, R.Op>, +hs: List<&2, N.Hit>, +oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t2: U32) -> IO(Shell):
  match on:
    case True{}:
      do IO<Shell>:
        r : Window & Image <- W.Win.present(w, Lbv.draw(n, Tie.pics(Aux.tie(wa)), ms))
        t3 : U32 <- PS.Perf.lap("ui.lightbox", t2)
        Shell.timed(t0)
        +s : N.Step = Shell.hits(n, hs, epoch)
        Shell.shown(r, Shell.hits.nui(s), q, i, wa, Shell.hits.anim(s, Shell.anim(n, ms)), ms)
    case False{}:
      Shell.present(Shell.image(N.Nui.panel(n), ops, N.Nui.vw(n), N.Nui.width(n), N.Nui.height(n), N.Nui.vx0(n), ms, N.Nui.look(n), N.Nui.lfocus.bits(n), hs, Tie.pics(Aux.tie(wa)), Aux.frames(wa), N.Hits.slide(hs, oh)),
        L.Out{ops, hs}, n, w, q, i, wa, epoch, ms, t0, t2)

def Shell.paint.out(o: L.Out, +oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t2: U32) -> IO(Shell):
  match o:
    case L.Out{+ops, +hs}:
      Lbv.paint(N.Lbx.on(n), ops, hs, oh, n, w, q, i, wa, epoch, ms, t0, t2)

# how many ops a layout made (counting them also finishes the layout)
def Lay.nops(o: L.Out) -> Nat:
  match o:
    case L.Out{os, _}:
      List.length(&2, R.Op, os)

def Lay.ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def Aux.with_ty(a: Aux, t: L.Ty) -> Aux:
  match a:
    case Aux{ws, _, ti, k}:
      Aux{ws, [t], ti, k}

# Match before calling Lay.memos: a hidden timeline does no wrapping.
def Shell.memos(visible: Bool, ty: L.Ty, n: N.Nui, ms: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  match visible:
    case True{}:
      L.Lay.memos(ty, n, ms)
    case False{}:
      ms

# Keep the repeated Nui use outside the frame's large IO continuation.
def Shell.memos.of(ty: L.Ty, +n: N.Nui, ms: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  Shell.memos(U.Ui.timeline(N.Nui.ui(n)), ty, n, ms)

# the frame again with the glyphs it wanted in the cache: the timeline's
# memos and the posts' lines are made again (they were measured without
# them)
def Shell.paint.again(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t2: U32) -> IO(Shell):
  do IO<Shell>:
    +memos : List<&2, L.TLMemo> = Shell.memos.of(ty, n, Nil{})
    +wraps : Map<&2, BV.PW> = BV.Bv.wraps(ty, n, BV.Bv.wraps.none())
    +o : L.Out = BV.Bv.frame.w(ty, L.TL.memos.now(memos), cats, wraps, n)
    +k : Nat = Lay.nops(o)
    +t3 : U32 <- PS.Perf.lap("ui.glyphs", t2)
    Shell.paint.out(o, oh, n, w, q, i, Aux.with_wraps(Aux.with_cats(Aux.with_memos(Aux.with_ty(wa, ty), memos), cats), wraps), epoch, ms, t0, t3)

# glyphs the layout found missing (text in a script the cache has not
# seen: Greek, CJK, symbols) are fetched, and the frame laid out again
def Shell.paint.want(ws: List<&2, X.Want>, o: L.Out, +cats: Map<&2, CA.CatF>, +memos: List<&2, L.TLMemo>, +oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t2: U32) -> IO(Shell):
  match ws:
    case Nil{}:
      Shell.paint.out(o, oh, n, w, q, i, Aux.with_cats(Aux.with_memos(wa, memos), cats), epoch, ms, t0, t2)
    case +h <> +t:
      IO.bind(L.Ty, Shell, L.Ty.fill(Aux.ty(wa), h <> t), ty => Shell.paint.again(ty, cats, oh, n, w, q, i, wa, epoch, ms, t0, t2))

# the cats the frame shows are posed into the cache first, and the posts
# it shows wrapped (kept in wa); the layout is timed from t1 (ui.layout,
# with the ops it made as ui.ops)
def Shell.paint.c(+cats: Map<&2, CA.CatF>, +memos: List<&2, L.TLMemo>, +oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t1: U32) -> IO(Shell):
  do IO<Shell>:
    +o : L.Out = BV.Bv.frame.w(Aux.ty(wa), L.TL.memos.now(memos), cats, Aux.wraps(wa), n)
    +k : Nat = Lay.nops(o)
    +t2 : U32 <- PS.Perf.lap("ui.layout", t1)
    PS.Perf.add("ui.ops", U32.from_nat(k))
    Shell.paint.want(X.Text.wants(Lay.ops(o), Nil{}), o, cats, memos, oh, n, w, q, i, wa, epoch, ms, t0, t2)

def Shell.paint.m(+memos: List<&2, L.TLMemo>, +oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat, +t0: U32, +t1: U32) -> IO(Shell):
  Shell.paint.c(BV.Bv.warm(N.Nui.ui(n), Aux.cats(wa), BV.Bv.clock.of(n)), memos, oh, n, w, q, i, Aux.with_wraps(wa, BV.Bv.wraps(Aux.ty(wa), n, Aux.wraps(wa))), epoch, ms, t0, t1)

# the timeline's memo is brought up to this frame first; layout draws from
# it. Each step is timed: ui.memo, ui.layout, then ui.raster and
# ui.present as the frame goes out
def Shell.paint.go(+oh: List<&2, N.Hit>, +n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), +wa: Aux, +epoch: Nat, +ms: Nat) -> IO(Shell):
  do IO<Shell>:
    +t0 : U32 <- PS.Perf.us()
    +memos : List<&2, L.TLMemo> = Shell.memos.of(Aux.ty(wa), n, Aux.memos(wa))
    +t1 : U32 <- PS.Perf.lap("ui.memo", t0)
    Shell.paint.m(memos, oh, n, w, q, i, wa, epoch, ms, t0, t1)

def Shell.paint.k(+n0: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux, +epoch: Nat, +ms: Nat) -> IO(Shell):
  match n0:
    case N.Nui{u, +ww, +hh, mx, my, sd, sc, ca, fo, pa, oh, _, vw0}:
      Shell.paint.go(oh, N.Nui{u, ww, hh, mx, my, sd, sc, ca, fo, pa, Nil{}, ms,
        Shell.vw.if(N.Nui.panel(n0), vw0, ww, hh, N.Nui.vx0(n0), ms, N.Nui.look(n0), N.Nui.lfocus.bits(n0))}, w, q, i, wa, epoch, ms)

# what a frame notes before it is laid out: the hover's and the caret's
# starts, the lightbox's ease, the cats' clock
def Shell.stamp.k(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms)

def Shell.stamp(+n: N.Nui, +ms: Nat) -> N.Nui:
  BV.Bv.stamp(Shell.stamp.k(n, ms), ms)

def Shell.paint(+n: N.Nui, w: Window, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux, +epoch: Nat, +ms: Nat) -> IO(Shell):
  Shell.paint.k(Shell.stamp(n, ms), w, q, i, wa, epoch, ms)

# The performance overlay (ctrl+shift+h): each frame drawn while it is up
# reads the numbers first (timed as ui.hud) and gives N.Hud.feed the text
def Hud.feed(on: Bool, +n: N.Nui, +ms: Nat) -> IO(N.Nui):
  match on:
    case True{}:
      do IO<N.Nui>:
        t0 : U32 <- PS.Perf.us()
        cur : String <- PS.Perf.take()
        +n2 : N.Nui = N.Hud.feed(n, cur, ms)
        t1 : U32 <- PS.Perf.span("ui.hud", t0)
        return n2
    case False{}:
      IO.pure(N.Nui, n)

# draw now if anything changed since the last frame
def Shell.frame(a: Shell) -> IO(Shell):
  match a:
    case Shell{+n, w, True{}, _, q, i, wa}:
      do IO<Shell>:
        t : U32 <- H.Clock.epoch()
        +ms : Nat <- IO.now()
        n2 : N.Nui <- Hud.feed(U.Ui.flag(N.Nui.ui(n), "perf"), n, ms)
        Shell.paint(n2, w, q, i, wa, U32.to_nat(t), ms)
    case Shell{n, w, False{}, _, q, i, wa}:
      IO.pure(Shell, Shell{n, w, False{}, False{}, q, i, wa})

def Shell.handle(a: Shell, m: ShellMsg) -> IO(Shell):
  match m:
    case FromWin{ws}:
      Shell.events(W.Win.decode(ws), a)
    case FromHub{tag, t}:
      Shell.tagged(a, FromHub{tag, t})
    case FromHubSt{tag, st, h}:
      Shell.tagged(a, FromHubSt{tag, st, h})
    case Frame{}:
      Shell.frame(a)
    case Blink{}:
      Shell.frame(Shell.blinked(a))
    case Due{+action, +value}:
      Shell.due(a, action, value)
    case Gone{tag}:
      Shell.tagged(a, Gone{tag})
    case Retry{tag}:
      Shell.tagged(a, Retry{tag})
    case Board{key, items}:
      Shell.board(a, key, items)
    case Mesh3{key, ts}:
      Shell.mesh(a, key, ts)
    case Parts3{key, t0, ts}:
      Shell.parts(a, key, t0, ts)
    case Status{key, text}:
      Shell.status(a, key, text)
    case Sheets{key, text}:
      Shell.sheets(a, text)
    case Shot{key, b}:
      Shell.shot(a, key, b)
    case RvGot{key, tag, ver, reread}:
      Shell.rvgot(a, key, tag, ver, reread)
    case PicOk{tag, path, w, h, rgb}:
      Shell.tagged(a, PicOk{tag, path, w, h, rgb})
    case PicErr{tag, path, why}:
      Shell.tagged(a, PicErr{tag, path, why})
    case Opened{th}:
      Shell.clicked(a, th)

def Kept.park(k: Kept) -> N.Park:
  match k:
    case Kept{_, _, _, pk, _}:
      Maybe.default(&2, N.Park, List.head(&2, N.Park, pk), N.Park.init())

def Kept.with_park(k: Kept, pk: N.Park) -> Kept:
  match k:
    case Kept{f, m, cs, _, wr}:
      Kept{f, m, cs, [pk], wr}

def Aux.park(a: Aux) -> N.Park:
  Kept.park(Aux.kept(a))

def Aux.with_park(+a: Aux, pk: N.Park) -> Aux:
  Aux.with_kept(a, Kept.with_park(Aux.kept(a), pk))

def Shell.parked(r: N.Nui & N.Park, w: Window, qd: Bool, +q: Chan(ShellMsg), +i: Chan(S.Msg), wa: Aux) -> Shell:
  (n, pk) = r
  Shell{n, w, True{}, qd, q, i, Aux.with_park(wa, pk)}

def Shell.park.k(same: Bool, a: Shell) -> Shell:
  match same a:
    case True{} _:
      a
    case False{} Shell{+n, w, _, qd, q, i, +wa}:
      Shell.parked(N.Park.swap.k(Aux.park(wa), n, U.Ui.sel(N.Nui.ui(n))), w, qd, q, i, wa)

# another thread selected: its own viewer comes back, the last one waits
def Shell.park(a: Shell) -> Shell:
  match a:
    case Shell{+n, w, d, qd, q, i, +wa}:
      Shell.park.k(String.eq(N.Park.thread(Aux.park(wa)), U.Ui.sel(N.Nui.ui(n))), Shell{n, w, d, qd, q, i, wa})

def ShellMsg.kind(m: ShellMsg) -> String:
  match m:
    case FromWin{_}:
      "win"
    case FromHub{_, _}:
      "hub"
    case FromHubSt{_, _, _}:
      "hub_state"
    case Frame{}:
      "frame"
    case Gone{_}:
      "gone"
    case Retry{_}:
      "retry"
    case Board{_, _}:
      "board"
    case Mesh3{_, _}:
      "mesh"
    case Status{_, _}:
      "status"
    case Parts3{_, _, _}:
      "parts"
    case Sheets{_, _}:
      "sheets"
    case Shot{_, _}:
      "shot"
    case RvGot{_, _, _, _}:
      "rvgot"
    case PicOk{_, _, _, _, _}:
      "pic"
    case PicErr{_, _, _}:
      "pic_err"
    case Blink{}:
      "blink"
    case Opened{_}:
      "opened"
    case Due{_, _}:
      "due"

# the span a message that leaves a frame to draw starts: input to pixels,
# hub message to pixels
def ShellMsg.waits(m: ShellMsg) -> String:
  match m:
    case FromWin{_}:
      "ui.input"
    case FromHub{_, _}:
      "ui.hub"
    case FromHubSt{_, _, _}:
      "ui.hub"
    case _:
      ""

def Shell.mark(a: Shell, +name: String) -> IO(Shell):
  match a:
    case Shell{n, w, True{}, qd, q, i, wa}:
      IO.bind(Unit, Shell, Bool.pick(IO(Unit), String.is_empty(name), IO.pure(Unit, Unit{}), PS.Perf.mark(name)), u => IO.pure(Shell, Shell{n, w, True{}, qd, q, i, wa}))
    case Shell{n, w, False{}, qd, q, i, wa}:
      IO.pure(Shell, Shell{n, w, False{}, qd, q, i, wa})

# each message's handling is timed as ui.msg.<kind> (docs/perf.md)
def Shell.got(m: Maybe<&1, ShellMsg>, a: Shell, k: Shell -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+msg}:
      do IO<Unit>:
        t0 : U32 <- PS.Perf.us()
        a2 : Shell <- IO.bind(Shell, Shell, Shell.handle(a, msg), h => Shell.watch(Shell.park(h)))
        t1 : U32 <- PS.Perf.lap("ui.msg." ++ ShellMsg.kind(msg), t0)
        a3 : Shell <- Shell.mark(a2, ShellMsg.waits(msg))
        k(a3)
    case None{}:
      IO.pure(Unit, Unit{})

def Shell.loop(fuel: Nat, a: Shell) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      Shell.queue(a, q => a2 =>
        IO.bind(Maybe<&1, ShellMsg>, Unit, Chan.recv(ShellMsg, q), m => Shell.got(m, a2, a3 => Shell.loop(f, a3))))

# Start
# -----


# the fonts, loaded while the hub starts (Start.fonts)
def Shell.fonts(m: Maybe<&1, L.Ty>) -> IO(L.Ty):
  match m:
    case Some{ty}:
      IO.pure(L.Ty, ty)
    case None{}:
      L.Ty.load()

# the first join: the in-process hub hands its state over (JoinHere); a
# hub joined over a link sends its log like any other machine's
def Shell.join.first(+inbox: Chan(S.Msg), +out: Chan(S.Wire), +lo: String, +gen: U32) -> IO(Result<&1, &1, S.Msg, Unit>):
  Bool.pick(IO(Result<&1, &1, S.Msg, Unit>), String.is_empty(lo), Chan.send(S.Msg, inbox, S.JoinHere{0, out, gen}), Chan.send(S.Msg, inbox, S.Join{0, out, 0n, "", gen, 0}))

def Shell.twin(r: Window & Window, +inbox: Chan(S.Msg), +drafts: String, +fq: Chan(L.Ty), +lo: String) -> IO(Unit):
  (ev, pw) = r
  do IO<Unit>:
    +q : Chan(ShellMsg) <- Chan.new(ShellMsg, 8192)
    +out : Chan(S.Wire) <- Chan.new(S.Wire, 8192)
    t : U32 <- H.Clock.epoch()
    +gen : U32 <- Fwd.gen()
    ok : Result<&1, &1, S.Msg, Unit> <- Shell.join.first(inbox, out, lo, gen)
    IO.spawn(Unit, Pump.loop(S.Fuel.forever(), ev, q))
    IO.spawn(Unit, Fwd.loop(S.Fuel.forever(), out, q, "#0", inbox, gen, 0n))
    got : Maybe<&1, L.Ty> <- Chan.recv(L.Ty, fq)
    ty : L.Ty <- Shell.fonts(got)
    ui : U.Ui <- Drafts.load(U.Ui.info_set(U.Ui.init(U32.to_nat(t)), "local.hub", lo), drafts)
    a : Shell <- Shell.later(Shell{N.Nui.init(ui, 1400, 900), pw, True{}, False{}, q, inbox, Aux{Nil{}, [ty], Tie{inbox, "", "#0", 1n, TieKeep.empty()}, [Kept.none()]}})
    Shell.loop(S.Fuel.forever(), a)

def Shell.opened(r: Result<&1, &1, U32 & String, Window>, +inbox: Chan(S.Msg), +drafts: String, +fq: Chan(L.Ty), +lo: String) -> IO(Unit):
  match r:
    case Done{w}:
      IO.bind(Window & Window, Unit, W.Win.twin(w), p => Shell.twin(p, inbox, drafts, fq, lo))
    case Fail{e}:
      Shell.nowin(e)

def Start.fonts(+fq: Chan(L.Ty)) -> IO(Unit):
  do IO<Unit>:
    ty : L.Ty <- L.Ty.load()
    ok : Result<&1, &1, L.Ty, Unit> <- Chan.send(L.Ty, fq, ty)
    return Unit{}

# `--headless`: no window even with a display; the hub, the web client and
# the tailnet listener run as they would without one, for the phones and
# browsers that connect. It goes to the background: the same program starts
# again in a session of its own with `--foreground` (its output appended to
# <home>/headless.log), this one says where and ends, and closing the
# terminal leaves it running. `--headless --foreground` stays (a service's)
def Start.has(xs: List<&2, String>, +flag: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, flag), Start.has(t, flag))

def Start.headless(+xs: List<&2, String>) -> Bool:
  Start.has(xs, "--headless")

# the value after `--home` (d without one)
def Start.home(xs: List<&2, String>, +d: String) -> String:
  match xs:
    case +k <> +v <> +t:
      Bool.pick(String, String.eq(k, "--home"), v, Start.home(v <> t, d))
    case _:
      d

def Start.detach.sh() -> String:
  "log=$1; shift; mkdir -p \"$(dirname \"$log\")\"; setsid \"$@\" --foreground >>\"$log\" 2>&1 </dev/null & echo $!"

def Start.detached(r: P.Ran, +log: String) -> IO(Unit):
  match r:
    case P.Ran{0, +out}:
      IO.print("Backplane runs in the background (pid " ++ String.trim(out) ++ "), its log in " ++ log ++ ". Stop it: kill " ++ String.trim(out))
    case P.Ran{_, +out}:
      IO.print_err("backplane: could not start in the background: " ++ out)

def Start.detach.k(free: Bool, +held: String, +dir: String, +xs: List<&2, String>) -> IO(Unit):
  match free:
    case True{}:
      do IO<Unit>:
        +exe : String <- H.Sys.exe_path()
        +log : String = dir ++ "/headless.log"
        r : P.Ran <- P.Proc.run("sh", List.append(&2, String, ["-c", Start.detach.sh(), "sh", log, exe], xs), "/")
        Start.detached(r, log)
    case False{}:
      IO.print("Backplane already runs for " ++ dir ++ " (pid " ++ S.Lock.word(held, 0n) ++ ", port " ++ S.Lock.word(held, 1n) ++ "); nothing was started")

# (a hub already running for the home is said before anything detaches)
def Start.detach(+xs: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    +home : String <- S.Env.get("HOME", "/tmp")
    +dir : String = Start.home(xs, home ++ "/.backplane-bend")
    +held : String <- S.Lock.find(dir)
    Start.detach.k(String.is_empty(held), held, dir, xs)

def Start.args(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Start.args(t)

def Start.window(headless: Bool) -> IO(Result<&1, &1, U32 & String, Window>):
  match headless:
    case True{}:
      IO.pure(Result<&1, &1, U32 & String, Window>, Fail{(0, "--headless")})
    case False{}:
      W.Win.open("Backplane", 1400, 900)

# the home's hub already runs (S.Lock): the window joins it on its port,
# `--headless` says where it is and ends; else this process runs the hub
def Start.hub(free: Bool, headless: Bool, +held: String, +cfg: S.Cfg, r: Result<&1, &1, U32 & String, Window>, +drafts: String, +fq: Chan(L.Ty)) -> IO(Unit):
  match free headless:
    case True{} _:
      S.Main.boot(cfg, False{}, i => Shell.opened(r, i, drafts, fq, ""))
    case False{} True{}:
      IO.print("Backplane already runs for " ++ S.Cfg.home(cfg) ++ " (pid " ++ S.Lock.word(held, 0n) ++ "), serving http://" ++ S.Cfg.host(cfg) ++ ":" ++ S.Lock.word(held, 1n) ++ "/")
    case False{} False{}:
      do IO<Unit>:
        +lo : String = "http://" ++ S.Cfg.host(cfg) ++ ":" ++ S.Lock.word(held, 1n)
        IO.print("Backplane already runs for " ++ S.Cfg.home(cfg) ++ " (pid " ++ S.Lock.word(held, 0n) ++ "): the window joins it at " ++ lo)
        i : Chan(S.Msg) <- LK.Link.start(lo)
        Shell.opened(r, i, drafts, fq, lo)

# the window opens first and the fonts load beside the hub's start, so
# the first frame waits only for the log
def Start.run(+xs: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    +fq : Chan(L.Ty) <- Chan.new(L.Ty, 1)
    IO.spawn(Unit, Start.fonts(fq))
    r : Result<&1, &1, U32 & String, Window> <- Start.window(Start.headless(xs))
    +user : String <- S.Env.get("HOME", "/tmp")
    +exe : String <- S.Sys.exe_dir.io()
    b : S.Boot <- S.Main.prepare(user, exe)
    +cfg : S.Cfg = S.Main.config(user, exe, Start.args(xs), b)
    +drafts : String = S.Cfg.home(cfg) ++ "/drafts"
    +held : String <- S.Lock.find(S.Cfg.home(cfg))
    Start.hub(String.is_empty(held), Start.headless(xs), held, cfg, r, drafts, fq)

def Start.pick(detach: Bool, +xs: List<&2, String>) -> IO(Unit):
  match detach:
    case True{}:
      Start.detach(xs)
    case False{}:
      Start.run(xs)

def Start.go(+xs: List<&2, String>) -> IO(Unit):
  Start.pick(Bool.and(Start.headless(xs), Bool.not(Start.has(xs, "--foreground"))), xs)

def start.k(render: Bool, +xs: List<&2, String>) -> IO(Unit):
  match render:
    case True{}:
      RN.Render.main(xs)
    case False{}:
      Start.go(xs)

# (`--render MODEL` draws a part's views to PNG files, render.bend)
def start.go(+xs: List<&2, String>) -> IO(Unit):
  start.k(RN.Render.wants(xs), xs)

def start(args: List<String>) -> IO(Unit):
  start.go(S.Names.copy(args))

# (`--plot KIND FILE` is the hub's own board reader, src/server/plot.bend;
# `--review bom OLD NEW` design review's, src/server/review.bend)
def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- IO.args()
    RVC.Rvc.main(args, a2 => PV.Pv.main(a2, a => start(a)))
