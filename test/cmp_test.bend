import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/board.bend as B
import ../src/core/hist.bend as H
import ../src/core/layers.bend as LY
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# Design review's comparisons (docs/design-review.md): layers, sheets, the
# Old/Diff/New flip, refs typed in, tags and the branch point.

def item(+k: String, +sig: U32, +layer: U32) -> B.Item:
  B.Item{k, sig, layer, B.PSeg{B.Pt{0.0, 0.0}, B.Pt{1.0, 1.0}, 0.2}, k}

def Show.items(xs: List<&2, B.Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Item{+k, _, +l, _, _} <> t:
      k ++ "@" ++ U32.show(l) ++ " " ++ Show.items(t)

def olds() -> List<&2, B.Item>:
  [item("a", 1, 12), item("b", 1, 5), item("c", 1, 12)]

def news() -> List<&2, B.Item>:
  [item("a", 2, 12), item("b", 2, 5), item("d", 1, 5)]

def labels(rs: List<&2, H.Ref>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      U.Hs.ref.label(r) <> labels(t)

def recv(+u: U.Ui, +text: String) -> U.Ui:
  U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def act(u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def sends(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      Nat.show(List.length(&2, U.Cmd, cs))

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

# thread t selected, its viewer on the board /r/b.kicad_pcb
def base() -> U.Ui:
  U.Ui.view_put(act(U.Ui.adopt(U.Ui.init(0n), state()), "select", "t"), "board", "/r/b.kicad_pcb")

def path(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.path")

def git(+c: String) -> String:
  H.Hist.git("/r/.git", c, "/r", "b.kicad_pcb")

def main() -> IO(Unit):
  +u = base()
  +cmp = act(u, "cmp-pick", "a\tc1\tv1\t/r/.git\t/r")
  +f = {LY.Focus{12, 1} : LY.Focus}
  +sch = act(U.Ui.view_put(u, "schematic", "/r/root.kicad_sch"), "cmp-pick", "a\tc1\tv1\t/r/.git\t/r")
  do IO<Unit>:
    # paths
    C.Check.str("diff on no layers is a plain diff", H.Hist.diff.on("x", "y", 0), "diff\nx\ny")
    C.Check.str("diff on layers", H.Hist.diff.on("x", "y", 4096), "diff\nx\ny\n4096")
    C.Check.str("bits read", U32.show(H.Hist.bits("diff\nx\ny\n4096")), "4096")
    C.Check.str("bits none", U32.show(H.Hist.bits("diff\nx\ny")), "0")
    C.Check.str("old and new ignore the bits", H.Hist.old("diff\nx\ny\n4096") ++ "|" ++ H.Hist.new("diff\nx\ny\n4096"), "x|y")
    C.Check.str("family ignores the bits", H.Hist.family(H.Hist.diff.on(git("c1"), "/r/b.kicad_pcb", 4096)), "/r/b.kicad_pcb")
    # marks on some layers
    C.Check.str("mark on every layer", Show.items(H.Hist.mark.on(0, olds(), news())), Show.items(H.Hist.mark(olds(), news())))
    C.Check.str("mark on layer 12 only", Show.items(H.Hist.mark.on(4096, olds(), news())), "a~was@26 a@27 c@26 b@5 d@5 ")
    C.Check.str("mark on layer 5 only", Show.items(H.Hist.mark.on(32, olds(), news())), "b~was@26 b@27 d@28 a@12 ")
    # sheets
    C.Check.str("sheet of a commit", H.Hist.sheet(H.Hist.git("/g", "c", "/t", "hw/root.kicad_sch"), "/t/hw/kid.kicad_sch"), H.Hist.git("/g", "c", "/t", "hw/kid.kicad_sch"))
    C.Check.str("sheet of a comparison", H.Hist.sheet(H.Hist.diff.on(H.Hist.git("/g", "c", "/t", "root.kicad_sch"), "/t/root.kicad_sch", 32), "/t/kid.kicad_sch"),
      H.Hist.diff.on(H.Hist.git("/g", "c", "/t", "kid.kicad_sch"), "/t/kid.kicad_sch", 32))
    C.Check.str("sheet of a comparison with a new file", H.Hist.sheet(H.Hist.diff("", "/t/root.kicad_sch"), "/t/kid.kicad_sch"), H.Hist.diff("", "/t/kid.kicad_sch"))
    C.Check.str("sheet picked from a commit's row", H.Hist.sheet(H.Hist.diff(H.Hist.git("/g", "c", "/t", "root.kicad_sch"), "/t/root.kicad_sch"), H.Hist.git("/g", "c", "/t", "kid.kicad_sch")),
      H.Hist.diff(H.Hist.git("/g", "c", "/t", "kid.kicad_sch"), "/t/kid.kicad_sch"))
    C.Check.str("Rv.sheet outside a comparison", U.Rv.sheet("/t/root.kicad_sch", "/t/kid.kicad_sch"), "/t/kid.kicad_sch")
    C.Check.str("Rv.sheet keeps the root as asked", U.Rv.sheet(H.Hist.diff("a", "b"), ""), "")
    # the flip
    C.Check.str("a comparison shows the diff", path(cmp), H.Hist.diff(git("c1"), "/r/b.kicad_pcb"))
    C.Check.str("showing diff", U.Rv.showing(cmp), "diff")
    C.Check.str("flip to old", path(act(cmp, "rv-as", "old")), git("c1"))
    C.Check.str("old is still a comparison", U.Rv.showing(act(cmp, "rv-as", "old")) ++ "|" ++ Bool.show(U.Hs.cmp(act(cmp, "rv-as", "old"))), "old|True")
    C.Check.str("flip to new", path(act(cmp, "rv-as", "new")), "/r/b.kicad_pcb")
    C.Check.str("flip back to the diff", path(act(act(cmp, "rv-as", "new"), "rv-as", "diff")), H.Hist.diff(git("c1"), "/r/b.kicad_pcb"))
    C.Check.str("no flip without a comparison", path(act(u, "rv-as", "old")), "/r/b.kicad_pcb")
    C.Check.str("closing forgets the flip", U.Rv.showing(act(act(cmp, "rv-as", "old"), "cmp-close", "")), "")
    C.Check.str("a new side shows the diff again", U.Rv.showing(act(act(cmp, "rv-as", "old"), "cmp-pick", "b\tc2\tv2\t/r/.git\t/r")), "diff")
    # layers
    C.Check.str("bits of a focus", U32.show(U.Rv.bits(0, f)), U32.show(LY.Lyr.kept(0, f)))
    C.Check.str("no bits without a focus", U32.show(U.Rv.bits(0, LY.Lyr.plain())), "0")
    C.Check.str("no bits on a schematic", U32.show(U.Rv.bits(1, LY.Focus{3, 1})), "0")
    C.Check.str("bits narrow a comparison", path(act(cmp, "rv-bits", "4096")), H.Hist.diff.on(git("c1"), "/r/b.kicad_pcb", 4096))
    C.Check.str("bits 0 widen it again", path(act(act(cmp, "rv-bits", "4096"), "rv-bits", "0")), H.Hist.diff(git("c1"), "/r/b.kicad_pcb"))
    C.Check.str("bits kept for the next comparison", path(act(act(u, "rv-bits", "32"), "cmp-pick", "a\tc1\tv1\t/r/.git\t/r")), H.Hist.diff.on(git("c1"), "/r/b.kicad_pcb", 32))
    # refs typed in
    C.Check.str("a ref typed in asks the hub", sends(U.Ui.act(cmp, "rv-ref", "a\tv1.2")), "1")
    C.Check.str("a blank ref asks nothing", sends(U.Ui.act(cmp, "rv-ref", "a\t  ")), "0")
    C.Check.str("the hub's answer is picked", path(recv(act(u, "rv-ref", "a\tv1.2"), "{\"t\":\"reply\",\"id\":1,\"ok\":true,\"result\":{\"resolved\":{\"thread\":\"t\",\"side\":\"a\",\"ref\":\"v1.2\",\"commit\":\"c9\",\"git\":\"/r/.git\",\"top\":\"/r\"}}}")),
      H.Hist.diff(git("c9"), "/r/b.kicad_pcb"))
    C.Check.str("another thread's answer is not", path(recv(u, "{\"t\":\"reply\",\"id\":1,\"ok\":true,\"result\":{\"resolved\":{\"thread\":\"x\",\"side\":\"a\",\"ref\":\"v1.2\",\"commit\":\"c9\",\"git\":\"/r/.git\",\"top\":\"/r\"}}}")),
      "/r/b.kicad_pcb")
    # tags and the branch point
    C.Check.str("tag and base refs", String.join(labels(H.Hist.refs("base\torigin/main\tm1\ntag\tv1.0\tt1\n")), ","), "Branch point · origin/main,v1.0")
    C.Check.str("tag and base kinds", U.Hs.ref.kind("tag") ++ "|" ++ U.Hs.ref.kind("base"), "tag|where this branch left it")
    # every board can be compared, steps or not
    C.Check.str("the bar shows with no steps", Bool.show(U.HBar.on(U.Hs.bar.for(u, True{}))), "True")
    C.Check.str("the live words with no steps", U.Hs.live.said(0n), "The live file. Compare it with any commit, branch or tag.")
    # a sheet picked while Old shows: Diff then compares that sheet
    C.Check.str("old, a sheet, then diff", path(act(U.Ui.view_put(act(sch, "rv-as", "old"), "schematic", H.Hist.sheet(path(act(sch, "rv-as", "old")), "/r/kid.kicad_sch")), "rv-as", "diff")),
      H.Hist.diff(H.Hist.git("/r/.git", "c1", "/r", "kid.kicad_sch"), "/r/kid.kicad_sch"))
