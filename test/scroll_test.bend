import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/app/nui.bend as N
import ../src/app/win.bend as W
import ../src/app/layout.bend as L
import ../src/app/theme.bend as T
import ../src/gfx/raster.bend as R
import ../src/gfx/tiles.bend as TI

# Scrolling the window: the sidebar moves a tile a notch and stops at the
# end of its list; the thread shows earlier entries as the wheel nears its
# top (no button) and drops them back at the bottom; the timeline's memo
# lines up with a page that slid or grew (TL.align.of). Layout runs with
# the bitmap face here.

def para(+i: Nat) -> String:
  "Message " ++ Nat.show(i) ++ ": short."

def msg(+k: Nat, +th: String, +at: Nat) -> M.Change:
  M.MessagePosted{th, th ++ "m" ++ Nat.show(k), Bool.pick(M.Role, Nat.is_eq(Nat.mod(k, 2n), 0n), M.User{}, M.Assistant{}), para(k), Nat.add(at, k)}

def msgs(+n: Nat, +th: String, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      msg(k, th, at) <> msgs(k, th, at)

def short(+k: Nat, +at: Nat) -> List<&2, M.Change>:
  M.ThreadCreated{"t" ++ Nat.show(1n+k), "p1", "Thread " ++ Nat.show(k), M.Local{}, "", "", "claude", "", at} <> msgs(20n, "t" ++ Nat.show(1n+k), at)

def threads(+n: Nat, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, M.Change, short(k, at), threads(k, at))

def log(+at: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "board", "/tmp/board", at}, M.ThreadCreated{"t0", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", at}],
    msgs(200n, "t0", at), threads(30n, at)])

def ui_sel(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      hs

def with_hits(n: N.Nui, hs: List<&2, N.Hit>, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}

def prepared(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms)

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def again(+ty: L.Ty, +n0: N.Nui) -> N.Nui:
  +n = prepared(n0, 0n)
  with_hits(n, hits(L.Lay.frame(ty, L.Lay.memo(ty, n, L.TL.memo.none()), n)), 0n)

def ev(+n: N.Nui, e: W.WinEv) -> N.Nui:
  step_nui(N.Nui.event(n, e))

def entry(+k: Nat) -> M.Entry:
  M.Msg{"m" ++ Nat.show(k), "t0", M.User{}, "text " ++ Nat.show(k), k}

# entries a (included) to b (excluded)
def entries(n: Nat, +a: Nat) -> List<&2, M.Entry>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      entry(a) <> entries(k, 1n+a)

def align.show(c: L.TLAlign) -> String:
  match c:
    case L.TLSame{}:
      "same"
    case L.TLDrop{d}:
      "drop " ++ Nat.show(d)
    case L.TLPad{k}:
      "pad " ++ Nat.show(k)

def align(+ty: L.Ty, es: List<&2, M.Entry>, old: List<&2, M.Entry>) -> String:
  align.show(L.TL.align.of(es, L.TL.memo.make(ty, old, 600, "t0")))

# n notches of the wheel at x, laid out after each
def wheel(k: Nat, +ty: L.Ty, +x: U32, +dir: U32, +n: N.Nui) -> N.Nui:
  match k:
    case 0n:
      n
    case 1n+j:
      wheel(j, ty, x, dir, again(ty, ev(n, W.WWheel{x, 400, dir})))

def sd(n: N.Nui) -> U32:
  match n:
    case N.Nui{_, _, _, _, _, s, _, _, _, _, _, _, _}:
      s

def sc(n: N.Nui) -> U32:
  match n:
    case N.Nui{_, _, _, _, _, _, s, _, _, _, _, _, _}:
      s

# Reading while the thread grows (client.bend's Hold): a frame as main.bend
# draws it, its place taken after (nui.bend's Hold.take)
def held(+ty: L.Ty, +n: N.Nui) -> N.Nui:
  N.Hold.take(again(ty, n))

def hwheel(k: Nat, +ty: L.Ty, +dir: U32, +n: N.Nui) -> N.Nui:
  match k:
    case 0n:
      n
    case 1n+j:
      hwheel(j, ty, dir, held(ty, ev(n, W.WWheel{800, 400, dir})))

# new messages k..k+n, oldest first
def news(n: Nat, +k: Nat, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+j:
      msg(k, "t0", at) <> news(j, 1n+k, at)

def grow(+ty: L.Ty, +n: N.Nui, cs: List<&2, M.Change>) -> N.Nui:
  held(ty, N.Nui.with_ui(n, U.Ui.fold(cs, N.Nui.ui(n))))

def stream(+ty: L.Ty, +n: N.Nui, +text: String) -> N.Nui:
  held(ty, N.Nui.with_ui(n, U.Ui.delta(N.Nui.ui(n), "t0", text)))

# 200 lines streamed in, a frame after every 20
def lines(n: Nat, +i: Nat) -> String:
  match n:
    case 0n:
      ""
    case 1n+k:
      "line " ++ Nat.show(i) ++ " of the stream\n\n" ++ lines(k, 1n+i)

def streams(k: Nat, +ty: L.Ty, +i: Nat, +n: N.Nui) -> N.Nui:
  match k:
    case 0n:
      n
    case 1n+j:
      streams(j, ty, Nat.add(i, 20n), stream(ty, n, lines(20n, i)))

def first.go(hs: List<&2, N.Hit>) -> String:
  match hs:
    case Nil{}:
      "none"
    case N.Hit{_, +y0, _, _, +a, +v} <> t:
      Bool.pick(String, String.eq(a, "tl-line"), v ++ "@" ++ U32.show(y0), first.go(t))

# the first thread line on screen: its number and where it is
def first(+n: N.Nui) -> String:
  first.go(N.Nui.hits(n))

# where line v is now ("gone" when off screen)
def line.at(hs: List<&2, N.Hit>, +v: String) -> String:
  match hs:
    case Nil{}:
      "gone"
    case N.Hit{_, +y0, _, _, +a, +hv} <> t:
      Bool.pick(String, Bool.and(String.eq(a, "tl-line"), String.eq(hv, v)), v ++ "@" ++ U32.show(y0), line.at(t, v))

def first.v(+s: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(s, '@')), "")

# the last thread line's bottom (the end follows: it sits at the foot)
def last.go(hs: List<&2, N.Hit>, +acc: U32) -> U32:
  match hs:
    case Nil{}:
      acc
    case N.Hit{_, _, _, +y1, +a, _} <> t:
      last.go(t, Bool.pick(U32, Bool.and(String.eq(a, "tl-line"), U32.is_gt(y1, acc)), y1, acc))

# Real entry rows with an image before the view. Only its decoded size
# changes between frames; downstream entry ids and their text stay put.
def lines.list(k: Nat) -> List<&2, String>:
  match k:
    case 0n:
      Nil{}
    case 1n+j:
      "below" <> lines.list(j)

def image.rows(+ty: L.Ty, +size: String) -> List<&2, L.TLine>:
  List.concat(&2, L.TLine, [[L.TLine{"", 0, 29, "image", 0}], L.TL.img(ty, size, "/tmp/hold.png", 780),
    [L.TLine{"", 0, 29, "reading", 0}, L.TLine{"still reading", 0, 0, "", 0}], L.TL.texts(lines.list(80n), 0, 0)])

def image.frame(+ty: L.Ty, +size: String, +hold: String, +sc: U32, +top: U32) -> List<&2, N.Hit>:
  hits(L.Lay.timeline(ty, image.rows(ty, size), 300, 1080, top, 900, sc, L.TSel.none(), hold, "image", L.Out.none()))

def main() -> IO(Unit):
  do IO<Unit>:
    +ty : L.Ty <- L.Ty.load()
    C.Check.str("a page that slid drops its oldest item", align(ty, entries(60n, 1n), entries(60n, 0n)), "drop 1")
    C.Check.str("a page added in front makes only it", align(ty, entries(120n, 0n), entries(60n, 60n)), "pad 60")
    C.Check.str("the same page stays", align(ty, entries(60n, 0n), entries(60n, 0n)), "drop 0")
    C.Check.str("a page dropped in front", align(ty, entries(60n, 60n), entries(120n, 0n)), "drop 60")
    C.Check.str("nothing in common", align(ty, entries(5n, 0n), entries(5n, 100n)), "same")
    +u : U.Ui = ui_sel(U.Ui.act(U.Ui.fold(log(1790000000n), U.Ui.init(1790000100n)), "select", "t0"))
    +n : N.Nui = again(ty, N.Nui.init(u, 1400, 900))
    +s1 : N.Nui = wheel(1n, ty, 100, 1, n)
    +s9 : N.Nui = wheel(40n, ty, 100, 1, n)
    +s10 : N.Nui = wheel(1n, ty, 100, 1, s9)
    C.Check.str("a sidebar notch is a tile", U32.show(sd(s1)), U32.show(N.Nui.notch()))
    C.Check.str("the sidebar stops at its end", Bool.show(U32.is_lt(sd(s9), (40 * N.Nui.notch() : U32))), "True")
    C.Check.str("and stays there", U32.show(sd(s10)), U32.show(sd(s9)))
    C.Check.str("a page at first", Nat.show(U.Page.more(N.Nui.ui(n), "t0")), "0")
    +t1 : N.Nui = wheel(40n, ty, 800, 0, n)
    C.Check.str("the wheel near the top shows earlier entries", Bool.show(Nat.is_gt(U.Page.more(N.Nui.ui(t1), "t0"), 0n)), "True")
    +t2 : N.Nui = wheel(200n, ty, 800, 1, t1)
    C.Check.str("back at the bottom", U32.show(sc(t2)), "0")
    C.Check.str("they go again", Nat.show(U.Page.more(N.Nui.ui(t2), "t0")), "0")
    # An image above the view changes from its loading row to 400px.
    # The initial offset 30 sits ten pixels inside the reading entry.
    +ih0 : List<&2, N.Hit> = image.frame(ty, "", "", 1153, 52)
    +ia0 : String = N.Hit.value(ih0, "tl-at")
    +ih1 : List<&2, N.Hit> = image.frame(ty, "200x400", ia0, Maybe.default(&2, U32, U32.read(N.Hold.part(ia0, 0n)), 0), 52)
    C.Check.str("image growth above the view keeps its entry", N.Hold.part(N.Hit.value(ih1, "tl-at"), 4n), "reading")
    C.Check.str("image growth keeps the intra-entry offset", N.Hold.part(N.Hit.value(ih1, "tl-at"), 1n), "430")
    +ih2 : List<&2, N.Hit> = image.frame(ty, "200x400", N.Hit.value(ih1, "tl-at"), Maybe.default(&2, U32, U32.read(N.Hold.part(N.Hit.value(ih1, "tl-at"), 0n)), 0), 100)
    C.Check.str("panel expansion compensates in the same frame", N.Hold.part(N.Hit.value(ih2, "tl-at"), 1n), "478")
    +ih3 : List<&2, N.Hit> = image.frame(ty, "", N.Hit.value(ih1, "tl-at"), Maybe.default(&2, U32, U32.read(N.Hold.part(N.Hit.value(ih1, "tl-at"), 0n)), 0), 52)
    C.Check.str("image shrink keeps the reading offset", N.Hold.part(N.Hit.value(ih3, "tl-at"), 1n), "30")
    C.Check.str("entry boundary takes no height", U32.show(L.TL.h(29)), "0")
    C.Check.str("entry boundary has no selectable text", Bool.show(L.TL.textual(29n)), "False")
    C.Check.str("held page keeps room for every new entry", Nat.show(U.Hold.page(entries(140n, 0n), "m120", 60n)), "180")
    C.Check.str("unheld page keeps its limit", Nat.show(U.Hold.page(entries(140n, 0n), "", 60n)), "60")
    # reading mid-thread: new messages and a streamed reply move nothing
    +r0 : N.Nui = hwheel(6n, ty, 0, held(ty, n))
    +f0 : String = first(r0)
    C.Check.str("scrolled up", Bool.show(U32.is_gt(sc(r0), 0)), "True")
    +r1 : N.Nui = grow(ty, r0, news(12n, 300n, 1790000000n))
    C.Check.str("new messages below move nothing read", line.at(N.Nui.hits(r1), first.v(f0)), f0)
    C.Check.str("the pill counts them", U.Hold.pill(N.Nui.ui(r1), "t0"), "12 new ↓")
    +r2 : N.Nui = streams(10n, ty, 0n, r1)
    C.Check.str("200 streamed lines move nothing read", line.at(N.Nui.hits(r2), first.v(f0)), f0)
    +r3 : N.Nui = grow(ty, r2, news(70n, 400n, 1790000000n))
    C.Check.str("nor does a page's worth more (the page holds)", line.at(N.Nui.hits(r3), first.v(f0)), f0)
    C.Check.str("the pill is a click back down", Bool.show(N.Hit.has(N.Nui.hits(r3), "vw-bottom")), "True")
    C.Check.str("the click's own frame has no pill", U.Hold.pill(N.Nui.ui(step_nui(N.Hold.bottom(r3))), "t0"), "")
    +b0 : N.Nui = held(ty, step_nui(N.Hold.bottom(r3)))
    C.Check.str("back at the end", U32.show(sc(b0)), "0")
    C.Check.str("no pill there", U.Hold.pill(N.Nui.ui(b0), "t0"), "")
    C.Check.str("the page is the newest again", U.Ui.scratch(N.Nui.ui(b0), "@tl.pin.t0"), "")
    # at the end the thread follows
    +e0 : U32 = last.go(N.Nui.hits(b0), 0)
    +e1 : N.Nui = streams(5n, ty, 500n, grow(ty, b0, news(5n, 600n, 1790000000n)))
    C.Check.str("at the end it follows", U32.show(sc(e1)), "0")
    C.Check.str("its last line at the foot", U32.show(last.go(N.Nui.hits(e1), 0)), U32.show(e0))
