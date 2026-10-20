import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/app/nui.bend as N
import ../src/app/layout.bend as L
import ../src/mobile/screen.bend as MB
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

def state(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def sends(cs: List<&2, U.Cmd>) -> Bool:
  match cs:
    case Nil{}: False{}
    case U.Focus{_} <> t: sends(t)
    case _ <> _: True{}

def commands(a: U.Act) -> Bool:
  match a:
    case U.Act{_, cs}: sends(cs)

def present(hs: List<&2, N.Hit>, +a: String, +v: String) -> Bool:
  match hs:
    case Nil{}: False{}
    case N.Hit{_, _, _, _, +b, +w} <> t:
      Bool.or(Bool.and(String.eq(a, b), String.eq(v, w)), present(t, a, v))

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}: hs

def page(+u: U.Ui, +p: String, +width: U32, +height: U32, +off: U32) -> List<&2, N.Hit>:
  +v = state(U.Ui.act(u, "settings-page", p))
  hits(L.Lay.settings(L.Ty.none(), U.Ui.state.of(v), N.Nui.init(v, width, height), Map.new(&2, String), "test", "", 264, width, height, off))

def categories(ps: List<&2, String>, +u: U.Ui) -> Bool:
  match ps:
    case Nil{}: True{}
    case +p <> t:
      Bool.and(String.eq(U.Settings.page(state(U.Ui.act(u, "settings-page", p))), p), categories(t, u))

def native_back(ps: List<&2, String>, +u: U.Ui) -> Bool:
  match ps:
    case Nil{}: True{}
    case +p <> t: Bool.and(present(page(u, p, 800, 500, 0), "settings-page", ""), native_back(t, u))

def with_hits(n: N.Nui, hs: List<&2, N.Hit>) -> N.Nui:
  match n:
    case N.Nui{u,w,h,mx,my,sd,sc,ca,fo,pa,_,ms,vw}:
      N.Nui{u,w,h,mx,my,sd,sc,ca,fo,pa,hs,ms,vw}

def step(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n,_,_}: n

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = state(U.Ui.act(U.Ui.init(0n), "flag", "settings"))
    +saved : U.Ui = state(U.Ui.act(u, "setting", "agent.model=custom-model"))
    +dirty : U.Ui = U.Ui.scratch_set(U.Ui.scratch_set(saved, "@set.sc", "9000"), "@f.kname", "KEEP_ME")
    +go : U.Ui = state(U.Ui.act(dirty, "settings-page", "keys"))
    C.Check.str("every category opens", Bool.show(categories(U.Settings.pages(), u)), "True")
    C.Check.str("every native category has a back path at narrow width", Bool.show(native_back(U.Settings.pages(), u)), "True")
    C.Check.str("category changes reset scroll", U.Ui.scratch(go, "@set.sc"), "0")
    C.Check.str("navigation retains draft field", U.Ui.scratch(go, "@f.kname"), "KEEP_ME")
    C.Check.str("navigation retains saved setting", M.Setting.get(U.Ui.state.of(go), "agent.model", ""), "custom-model")
    C.Check.str("navigation never sends or persists", Bool.show(commands(U.Ui.act(dirty, "settings-page", "keys"))), "False")
    C.Check.str("invalid category returns home", U.Settings.page(state(U.Ui.act(go, "settings-page", "unknown"))), "")
    C.Check.str("back returns home", U.Settings.page(state(U.Ui.act(go, "settings-page", ""))), "")
    C.Check.str("usage shortcut opens usage", U.Settings.page(state(U.Ui.act(u, "usage-open", ""))), "usage")
    C.Check.str("KiCad shortcut opens CAD", U.Settings.page(state(U.Ui.act(u, "kicad-path", ""))), "cad")
    C.Check.str("appearance still has theme", Bool.show(present(page(u, "appearance", 800, 500, 0), "setting", "ui.theme=light")), "True")
    C.Check.str("CAD still has browser engine", Bool.show(present(page(u, "cad", 800, 900, 0), "setting", "browser.engine=cef")), "True")
    C.Check.str("CAD still has ordering", Bool.show(present(page(u, "cad", 800, 900, 0), "setting", "order.enabled=on")), "True")
    C.Check.str("locale category uses translated label", Bool.show(String.is_empty(U.Settings.title(I.Ja{}, "appearance"))), "False")
    C.Check.str("phones omit empty desktop voice page", Bool.show(U.Settings.known(U.Settings.phone.pages(), "voice")), "False")
    +home : N.Nui = with_hits(N.Nui.init(u, 800, 900), List.reverse(&2, N.Hit, page(u, "", 800, 900, 0)))
    +tabbed : N.Nui = step(N.Nui.key.m(home, N.Key.tab(), 0, 0))
    C.Check.str("Tab selects first category", U.Ui.scratch(N.Nui.ui(tabbed), "@set.focus"), "settings-page\tappearance")
    +entered : N.Nui = step(N.Nui.key.m(tabbed, N.Key.enter(), 0, 0))
    C.Check.str("Enter opens highlighted category", U.Settings.page(N.Nui.ui(entered)), "appearance")
    +back : N.Nui = step(N.Nui.key.m(entered, N.Key.escape(), 0, 0))
    C.Check.str("Escape returns to category menu", U.Settings.page(N.Nui.ui(back)), "")
    C.Check.str("Escape at menu closes Settings", Bool.show(U.Ui.flag(N.Nui.ui(step(N.Nui.key.m(back, N.Key.escape(), 0, 0))), "settings")), "False")
    C.Check.str("resize clamps before scrolling", N.Set.scroll.by("9000", "100", 0, 48), "52")
    C.Check.str("Appearance offers shared scale handler", Bool.show(present(page(u, "appearance", 800, 900, 0), "setting", "ui.text_scale=150")), "True")
