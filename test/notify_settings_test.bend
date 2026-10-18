import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N
import ../src/gfx/raster.bend as R
import ../src/gfx/font.bend as F
import ../src/gfx/draw.bend as D
import ../src/web/dom.bend as DOM
import ../src/web/view.bend as W
import ../src/mobile/screen.bend as P

# Check real Settings outputs, including the bitmap glyph runs the window
# draws (raster ops no longer hold text), and the existing setting RPC path.
def values(rs: List<&2, U.NotifyRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case U.NotifyRow{_, _, on, _} <> t:
      on <> values(t)

def texts(ns: List<&2, DOM.Node>) -> String:
  match ns:
    case Nil{}:
      ""
    case DOM.Txt{s} <> t:
      s ++ "\n" ++ texts(t)
    case DOM.El{_, _, _, kids} <> t:
      texts(kids) ++ texts(t)

def attrs(as: List<&2, DOM.Attr>, +v: String) -> Bool:
  match as:
    case Nil{}:
      False{}
    case DOM.At{+k, +s} <> t:
      Bool.or(Bool.and(String.eq(k, "data-value"), String.eq(s, v)), attrs(t, v))
    case DOM.On{_, _} <> t:
      attrs(t, v)

def button(ns: List<&2, DOM.Node>, +v: String) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case DOM.Txt{_} <> t:
      button(t, v)
    case DOM.El{_, _, as, kids} <> t:
      Bool.or(attrs(as, v), Bool.or(button(kids, v), button(t, v)))

def glyph(g: F.Glyph) -> String:
  match g:
    case F.Glyph{a, b, c, d}:
      U32.show(a) ++ ":" ++ U32.show(b) ++ ":" ++ U32.show(c) ++ ":" ++ U32.show(d) ++ ";"

def shape(s: R.Shape) -> String:
  match s:
    case R.SGlyph{_, _, g}:
      glyph(g)
    case _:
      ""

def glyphs(os: List<&2, R.Op>) -> String:
  match os:
    case Nil{}:
      ""
    case R.Op{_, _, _, _, _, _, s} <> t:
      shape(s) ++ glyphs(t)

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sends(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{s} <> t:
      s ++ sends(t)
    case _ <> t:
      sends(t)

def rpc(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sends(cs)

def changes(s: H.Step) -> List<&2, M.Change>:
  match s:
    case H.Step{cs, _}:
      cs

def names(+web: String, +phone: String, +window: String, ns: List<&2, String>) -> IO(Unit):
  match ns:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +nm <> t:
      do IO<Unit>:
        C.Check.str("web contains " ++ nm, Bool.show(String.contains(web, nm)), "True")
        C.Check.str("phones contain " ++ nm, Bool.show(String.contains(phone, nm)), "True")
        C.Check.str("window draws " ++ nm, Bool.show(String.contains(window, glyphs(L.Ty.text(L.Ty.none(), 0n, nm, D.Clip.all(), 0, 0, 28, 0)))), "True")
        names(web, phone, window, t)

def has_hit(hs: List<&2, N.Hit>, +v: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case N.Hit{_, _, _, _, +a, +s} <> t:
      Bool.or(Bool.and(String.eq(a, "setting"), String.eq(s, v)), has_hit(t, v))

def hits(o: L.Out, +v: String) -> Bool:
  match o:
    case L.Out{_, hs}:
      has_hit(hs, v)

def controls(ks: List<&2, String>, +out: L.Out, +web: DOM.Node, +phone: String) -> IO(Unit):
  match ks:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +k <> t:
      do IO<Unit>:
        C.Check.str("window on chip " ++ k, Bool.show(hits(out, k ++ "=on")), "True")
        C.Check.str("window off chip " ++ k, Bool.show(hits(out, k ++ "=off")), "True")
        C.Check.str("web on chip " ++ k, Bool.show(button([web], k ++ "=on")), "True")
        C.Check.str("web off chip " ++ k, Bool.show(button([web], k ++ "=off")), "True")
        C.Check.str("phones on chip " ++ k, Bool.show(String.contains(phone, k ++ "=on")), "True")
        C.Check.str("phones off chip " ++ k, Bool.show(String.contains(phone, k ++ "=off")), "True")
        +a : U.Act = U.Ui.act(U.Ui.init(0n), "setting", k ++ "=off")
        C.Check.str("setting applies locally " ++ k, M.Setting.get(U.Ui.state(ui(a)), k, ""), "off")
        C.Check.str("setting keeps sequence " ++ k, Nat.show(M.State.seq(U.Ui.state(ui(a)))), "0")
        C.Check.str("setting sends RPC " ++ k, Bool.show(String.contains(rpc(a), "setting.set")), "True")
        +s : M.State = M.State.replay(changes(H.Hub.request(M.State.empty(), 1n, 1, J.Json.obj([J.KV{"m", J.Str{"setting.set"}}, J.KV{"id", J.Str{"x"}}, J.KV{"p", J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"value", J.Str{"off"}}])}]))), M.State.empty())
        C.Check.str("hub accepts " ++ k, M.Setting.get(s, k, ""), "off")
        controls(t, out, web, phone)

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = U.Ui.init(0n)
    +s : M.State = M.State.empty()
    C.Check.str("defaults", String.join(values(U.Notify.rows(s)), ","), "on,on,on,off")
    C.Check.str("SettingSet flips Needs me", String.join(values(U.Notify.rows(M.State.apply(s, M.SettingSet{"notify.needs", "off", 1n}))), ","), "off,on,on,off")
    C.Check.str("SettingSet enables Everything", String.join(values(U.Notify.rows(M.State.apply(s, M.SettingSet{"notify.all", "on", 1n}))), ","), "on,on,on,on")
    +out : L.Out = L.Lay.settings(L.Ty.none(), s, N.Nui.init(u, 1400, 1000), Map.new(&2, String), "test", "", 240, 1400, 1000, 0)
    +web : DOM.Node = W.View.settings(u, s, Map.new(&2, String), "test", "")
    +phone : String = J.Json.show(P.Mob.settings.of(u, s, Map.new(&2, String), "test", "", Nil{}))
    names(texts([web]), phone, glyphs(ops(out)), ["Notify me for", "Needs me", "Settled", "Reminders", "Everything"])
    C.Check.str("web lead", Bool.show(String.contains(texts([web]), U.Notify.lead(I.En{}))), "True")
    C.Check.str("phone lead", Bool.show(String.contains(phone, U.Notify.lead(I.En{}))), "True")
    controls(["notify.needs", "notify.settled", "notify.reminders", "notify.all"], out, web, phone)
