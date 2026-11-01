import Base
import ./check.bend as C
import ./hist_test.bend as T
import ../src/core/hist.bend as H
import ../src/core/model.bend as M
import ../src/core/client.bend as U

def main() -> IO(Unit):
  +st = M.State.apply(T.state(), M.ActivityLogged{"other", "private-step", "step", "kicad", "{}", 11n})
  +empty = H.Hist.window(st, "t", M.State.empty())
  +page = H.Hist.window(st, "t", M.State.post(M.State.empty(), T.step("step:c1", H.Hist.record(T.out1(), "u"), 2n)))
  +shown = U.Ui.adopt.raw(T.act(T.base(), "hist-first", ""), empty)
  +long = M.State.apply(M.State.apply(st, M.ActivityLogged{"t", "long-tool", "tool", "Bash", String.repeat("x", 400n), 12n}),
    M.ActivityLogged{"t", "long-step", "step", "kicad", H.Hist.record("c5\nc4\n/r/.git\n/r\n" ++ T.z("M") ++ T.z("b.kicad_pcb"), "long-tool"), 13n})
  do IO<Unit>:
    C.Check.str("an empty chat page retains all selected snapshots", T.Show.ids(H.Hist.steps(empty, "t")), "step:c1 step:c2 step:c3 step:c4 ")
    C.Check.str("a visible step is indexed only once", T.Show.ids(H.Hist.steps(page, "t")), "step:c1 step:c2 step:c3 step:c4 ")
    C.Check.str("another thread's snapshots stay out", T.Show.ids(H.Hist.steps(empty, "other")), "")
    C.Check.str("the chat itself is still empty", Nat.show(List.length(&2, M.Entry, M.State.mine(empty, "t"))) ++ "," ++ Nat.show(List.length(&2, M.Entry, M.State.entries.all(empty))), "0,0")
    C.Check.str("off-page tool sources have short labels", H.Hist.label(M.State.mine(empty, H.Hist.sources.key("t")), "a1"), "Bash  python place.py")
    C.Check.str("the selected off-page snapshot is still selected", Nat.show(U.Hs.at(shown, U.Hs.track(shown))), "1")
    C.Check.str("the history track does not depend on the chat page", T.Show.ids(U.Hs.track(shown)), "step:c1 step:c2 step:c4 ")
    C.Check.str("the selected off-page source is available", H.Hist.label(U.Hs.entries(shown), "u"), "route it")
    C.Check.str("off-page sources never retain whole long tool bodies", Nat.show(String.length(H.Hist.label(M.State.mine(H.Hist.window(long, "t", M.State.empty()), H.Hist.sources.key("t")), "long-tool"))), "246")
