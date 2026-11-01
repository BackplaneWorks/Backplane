import Base
import ./check.bend as C
import ./hist_test.bend as T
import ../src/core/model.bend as M
import ../src/core/hist.bend as H
import ../src/core/client.bend as U
import ../src/core/thin.bend as TH
import ../src/core/thin-live.bend as TL
import ../src/server/thin-view.bend as V

def main() -> IO(Unit):
  +whole = T.state()
  +loaded = H.Hist.window(whole, "t", M.State.empty())
  +target = V.Target.json("thread", "t")
  +index = TH.Index.of(whole)
  +empty = V.State.window(loaded, index, target, Nil{})
  +body = V.State.window.body(True{}, loaded, index, target, ["2"])
  +at = T.act(T.act(T.base(), "hist-first", ""), "hist-next", "")
  +rendered = V.Render.ui(at, loaded, index, target, Nil{}, TL.Stream.blank(), 20n)
  do IO<Unit>:
    C.Check.str("empty thin page retains every snapshot exactly once", T.Show.ids(H.Hist.steps(empty, "t")), "step:c1 step:c2 step:c3 step:c4 ")
    C.Check.str("bounded body page retains every snapshot exactly once", T.Show.ids(H.Hist.steps(body, "t")), "step:c1 step:c2 step:c3 step:c4 ")
    C.Check.str("thin renderer keeps the selected snapshot", Nat.show(U.Hs.at(rendered, U.Hs.track(rendered))), "2")
    C.Check.str("thin renderer never imports old chat bodies", Nat.show(List.length(&2, M.Entry, M.State.mine(U.Ui.state(rendered), "t"))), "0")
    C.Check.str("thin renderer retains the off-page tool label", H.Hist.label(U.Hs.entries(rendered), "a1"), "Bash  python place.py")
    C.Check.str("thin renderer does not duplicate the top-bar count", Nat.show(H.Hist.changes(M.State.steps(U.Ui.state(rendered), "t"))), "3")
