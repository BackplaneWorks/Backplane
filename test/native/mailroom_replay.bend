import Base
import ./mailroom_snap.bend as SN
import ./memo_replay.bend as EQ
import ../mailroom_client_test.bend as MC
import ../mailroom_parity_test.bend as PT
import ../check.bend as C
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/mailview.bend as MV
import ../../src/app/mm.bend as MM
import ../../src/app/mailroom.bend as MR
import ../../src/app/bots.bend as B
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/app/cats.bend as CA

# The oracle always starts with empty wraps and explicitly redraws the
# body. It never reads BotM.key/same/due or the retained output to decide
# what should be drawn. Complete ops AND hits are compared.
type Case is Data:
  Case{name: String, step: Nat, w: U32, h: U32}

type Counts is Data:
  Counts{frames: Nat, hit: Nat, stale_body: Nat, stale_frame: Nat}

def fresh(+ty: L.Ty, +n: N.Nui, +ws: Map<&2, B.PW>) -> L.Out:
  B.Bv.mail.draw(ws, ty, L.Lay.shrink(n, L.Lay.term_h(n)))

# Keep reporting continuations small: the IO layer needs only verdicts
# and counters, while drawing and comparison stay pure.
type Checked is Data:
  Checked{label: String, body: Bool, frame: Bool, hit: Nat}

def checked(+label: String, +ty: L.Ty, +n: N.Nui, +ws: Map<&2, B.PW>, +cold: Map<&2, B.PW>) -> Checked:
  Checked{label,
    EQ.Eqh.out(B.Bv.mail.out(ws, ty, n), fresh(ty, n, cold)),
    EQ.Eqh.out(B.Bv.frame.w(ty, L.TL.memos.now(Nil{}), CA.Cats.none(), ws, n), B.Bv.frame.w(ty, L.TL.memos.now(Nil{}), CA.Cats.none(), cold, n)),
    B.Bv.mail.hit(ws, n)}

def one(check: Checked, count: Counts) -> IO(Counts):
  match check count:
    case Checked{+label, +body_ok, +frame_ok, hits} Counts{f, hit, sb, sf}:
      do IO<Counts>:
        C.Check.str(label ++ " body", Bool.show(body_ok), "True")
        C.Check.str(label ++ " full frame", Bool.show(frame_ok), "True")
        return Counts{1n+f, Nat.add(hit, hits), Nat.add(sb, Bool.pick(Nat, body_ok, 0n, 1n)), Nat.add(sf, Bool.pick(Nat, frame_ok, 0n, 1n))}

def base() -> U.Ui:
  SN.seed(U.Ui.scratch_set(MC.ui(), "@btab", "mail"))

def read(+u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(U.Mailv.put(U.Ui.scratch_set(u, "@mr.conv", "c"), U.Mailv.read.put(U.Mailv.data(u), MV.Mv.put_read(U.Mailv.cache(u), MC.read_result(), False{}), "c", Map.new(&2, String), Nil{})), "@mr.to", "bot:b1")

def origin_state() -> U.Ui:
  U.Ui.fold([M.MessageFrom{"t1", "m1", "thread:x", "thread", "", 200001n}], read(base()))

def incoming_state() -> U.Ui:
  U.Ui.fold([M.MailPosted{"m2", "c", "bot:b1", "person", "thread", "", "act", "new", 3n, 200001n}], origin_state())

# Keep only small scenario descriptors in the case list. Construct one Ui
# per frame so native C emission never flattens 28 deep states at once.
def state_at(step: Nat) -> U.Ui:
  match step:
    case 0n: base()
    case 1n: base()
    case 2n: U.Ui.tick(base(), 200001n)
    case 3n: U.Ui.tick(base(), 200039n)
    case 4n: U.Ui.tick(base(), 200040n)
    case 5n: U.Ui.tick(base(), 200040n)
    case 6n: U.Ui.tick(base(), 199999n)
    case 7n: U.Ui.info_set(base(), "origin", "second")
    case 8n: U.Ui.info_set(base(), "local.generation", "next")
    case 9n: U.Mailv.chip(base(), "b2")
    case 10n: U.Ui.scratch_set(base(), "@mr.project", "Sensor")
    case 11n: U.Ui.scratch_set(base(), "@mr.run", "1")
    case 12n: U.Ui.scratch_set(base(), "@mr.status", "today")
    case 13n: U.Ui.info_set(base(), "pane:mr", "2")
    case 14n: U.Ui.scratch_set(base(), "@f.mrsearch", "Deploy")
    case 15n: read(base())
    case 16n: read(base())
    case 17n: U.Ui.scratch_set(read(base()), "@f.mrreply", "hello")
    case 18n: U.Ui.info_set(read(base()), "pane:mrbody", "1")
    case 19n: origin_state()
    case 20n: incoming_state()
    case 21n: U.Ui.flag_on(read(base()), "term")
    case 22n: read(base())
    case 23n: read(base())
    case 24n: U.Ui.scratch_set(read(base()), "@lang.tag", "ja")
    case 25n: U.Act.ui(U.Ui.act(read(base()), "setting", "ui.theme=light"))
    case 26n: U.Ui.scratch_set(base(), "other-thread", "background draft")
    case _: base()

def replay(cs: List<&2, Case>, +ty0: L.Ty, ws: Map<&2, B.PW>, count: Counts) -> IO(Counts):
  match cs:
    case Nil{}: IO.pure(Counts, count)
    case Case{name, step, w, h} <> t:
      do IO<Counts>:
        +u : U.Ui = state_at(step)
        +ty : L.Ty = L.Ty.with_loc(ty0, U.Lang.loc(u))
        +n : N.Nui = N.Nui.init(u, w, h)
        +kept : Map<&2, B.PW> = B.Bv.wraps.ix(ty, n, ws, U.Side.ix(U.Ui.state(u)))
        +cold : Map<&2, B.PW> = B.Bv.wraps.ix(ty, n, B.Bv.wraps.none(), U.Side.ix(U.Ui.state(u)))
        next : Counts <- one(checked(name, ty, n, kept, cold), count)
        replay(t, ty0, kept, next)

def c(name: String, step: Nat) -> Case:
  Case{name, step, 1400, 900}

def cases() -> List<&2, Case>:
  [
    c("initial", 0n),
    c("unchanged", 1n),
    c("one second", 2n),
    c("before relative age boundary", 3n),
    c("relative age boundary", 4n),
    c("wall minute", 5n),
    c("clock backwards", 6n),
    c("origin", 7n),
    c("same revisions next generation", 8n),
    c("filter bot", 9n),
    c("filter project", 10n),
    c("run expanded", 11n),
    c("today", 12n),
    c("scroll list", 13n),
    c("search typed", 14n),
    c("open", 15n),
    c("open unchanged", 16n),
    c("reply typed", 17n),
    c("body scroll", 18n),
    c("origin metadata", 19n),
    c("incoming header", 20n),
    c("terminal height", 21n),
    Case{"narrow", 22n, 760, 900},
    Case{"resized", 23n, 1200, 720},
    c("Japanese", 24n),
    c("light", 25n),
    c("unrelated draft", 26n),
    c("final unchanged", 27n)
  ]

def report(c: Counts) -> IO(Unit):
  match c:
    case Counts{f, +hit, body, frame}:
      do IO<Unit>:
        C.Check.str("body memo is exercised", Bool.show(Nat.is_gt(hit, 0n)), "True")
        C.Check.str("zero stale bodies", Nat.show(body), "0")
        C.Check.str("zero stale full frames", Nat.show(frame), "0")
        IO.print("frames=" ++ Nat.show(f) ++ " body_hits=" ++ Nat.show(hit))

def main() -> IO(Unit):
  do IO<Unit>:
    ty : L.Ty <- L.Ty.load()
    c : Counts <- replay(cases(), ty, B.Bv.wraps.none(), Counts{0n, 0n, 0n, 0n})
    report(c)
