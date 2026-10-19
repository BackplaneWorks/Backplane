import Base
import ./ui_snap.bend as SN
import ../mailroom_parity_test.bend as PT
import ../mailroom_client_test.bend as CL
import ../../src/core/client.bend as U
import ../../src/core/mailview.bend as MV
import ../../src/app/bots.bend as BV
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/app/cats.bend as CA
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

# A deterministic synthetic Mailroom, no hub, provider or private data.
def page() -> List<&2, MV.Row>:
  [MV.Mv.make("c", "Deploy review", "Kit, thread t1", "Build verified; waiting for your review.", 199980n, True{}, "unread", "Backplane"),
   MV.Mv.make("c2", "Routing the sensor board", "Pip, hardware", "The differential pair is ready to inspect.", 199850n, True{}, "unread", "Sensor"),
   MV.Mv.make("c3", "Window benchmark", "Kit, perf", "The latest measurements are filed here.", 199600n, False{}, "open", "Backplane"),
   MV.Mv.make("c4", "Morning digest", "Pip", "Three threads finished overnight.", 199200n, False{}, "open", "Ops")]

def seed(+u: U.Ui) -> U.Ui:
  U.Mailv.put(u, U.Mailv.list.put(U.Mailv.data(u), MV.Mv.put_page(U.Mailv.cache(u), U.Mailv.filter(u), 0n, 4n, page())))

def bot(on: Bool, u: U.Ui) -> U.Ui:
  match on:
    case False{}: U.Act.ui(U.Ui.act(u, "bot", "b1"))
    case True{}: U.Ui.scratch_set(U.Act.ui(U.Ui.act(u, "bot", "b2")), "@btab", "mail")

def opened(on: Bool, +u: U.Ui) -> U.Ui:
  match on:
    case False{}: u
    case True{}:
      U.Ui.scratch_set(U.Mailv.put(U.Ui.scratch_set(u, "@mr.conv", "c"), U.Mailv.read.put(U.Mailv.data(u), MV.Mv.put_read(U.Mailv.cache(u), CL.read_result(), False{}), "c", Map.new(&2, String), Nil{})), "@mr.to", "bot:b1")

def go(+args: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    +kind : String = SN.arg(args, 0n)
    +light : Bool = String.eq(SN.arg(args, 1n), "light")
    +u : U.Ui = opened(Bool.or(String.eq(kind, "open"), String.eq(kind, "narrow")), seed(bot(String.eq(kind, "bot"), PT.ui())))
    +look : U.Ui = U.Act.ui(U.Ui.act(u, "setting", "ui.theme=" ++ Bool.pick(String, light, "light", "dark")))
    +w : U32 = Bool.pick(U32, String.eq(kind, "narrow"), 760, 1400)
    +n : N.Nui = N.Nui.init(look, w, 900)
    +ty : L.Ty <- L.Ty.load()
    +ws : Map<&2, BV.PW> = BV.Bv.wraps(ty, n, BV.Bv.wraps.none())
    +out : L.Out = BV.Bv.with_cats(BV.Bv.frame.w(ty, L.TL.memos.now(L.Lay.memos(ty, n, Nil{})), CA.Cats.none(), ws, n), CA.Cats.none(), 0n)
    +img : Image = R.Frame.render(w, 900, L.Theme.ops(N.Nui.light(n), SN.ops(out)), L.Theme.ground(N.Nui.look(n)))
    f : File <- IO.try(File, File.open(SN.arg(args, 2n), "w"))
    result : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(f, D.Snap.ppm(img, w, 900))
    IO.print("wrote " ++ SN.arg(args, 2n))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    go(SN.copy(args))
