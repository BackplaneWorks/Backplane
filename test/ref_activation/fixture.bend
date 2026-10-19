import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/far.bend as F
import ../../src/core/links.bend as LK
import ../../src/web/dom.bend as D
import ../../src/web/app.bend as A
import ../../src/app/layout.bend as L
import ../../src/app/nui.bend as N
import ../../src/gfx/draw.bend as G

def loaded(st: M.State, u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{_, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def fresh() -> U.Ui:
  +st = M.State.replay([
    M.ProjectCreated{"p1-1", "Navigation fixture", "/tmp/navigation", 1n},
    M.ThreadCreated{"t1-1", "p1-1", "Source", M.Local{}, "", "", "codex", "", 2n},
    M.ThreadCreated{"t1792085069-132986", "p1-1", "Raul / SeaGull fixture", M.Local{}, "", "", "codex", "", 3n},
    M.ProjectCreated{"peer~p1-1", "Remote fixture", "/tmp/remote", 4n},
    M.ThreadCreated{"peer~t1792085069-132986", "peer~p1-1", "Remote SeaGull fixture", M.Local{}, "", "", "codex", "", 5n}
  ], M.State.empty())
  U.Act.ui(U.Ui.act(loaded(F.Fr.mark(st, "peer", "Other machine", False{}), U.Ui.init(10n)), "select", "t1-1"))

def render(s: String) -> List<&2, D.Node>:
  D.Md.render(s)

def activate(u: U.Ui, action: String, value: String) -> U.Ui:
  U.Act.ui(A.act(u, action, value))

def selected(u: U.Ui) -> String:
  U.Ui.sel(u)

def hit(h: Maybe<&2, N.Hit>) -> String:
  match h:
    case None{}: ""
    case Some{N.Hit{_, _, _, _, a, v}}: a ++ " " ++ v

def native.hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}: hs

def native.parts(s: String) -> List<&2, N.Hit>:
  native.hits(L.TL.link.segs(LK.Link.pieces(s), L.Ty.none(), G.Clip{0, 0, 1200, 900}, 300, 100, 0))

def native_hit(s: String) -> String:
  hit(N.Hit.find(native.parts(s), 301, 101))

def native_tap(+s: String) -> String:
  +n = N.Nui.init(fresh(), 1400, 900)
  U.Ui.sel(N.Nui.ui(N.Step.nui(N.Row.press(N.Hit.find(native.parts(s), 301, 101), n, 301, 101))))
