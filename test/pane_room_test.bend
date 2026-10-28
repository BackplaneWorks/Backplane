import Base
import ./check.bend as C
import ../src/core/panes.bend as P

# The panes of a wide page (core/panes.bend): when the sidebar folds by
# itself beside the viewer, when the person's open sidebar is a drawer over
# the thread, and how wide the viewer is. Folded, the sidebar is a 44 px
# strip; the rail is 76 px (44 compact). The thread keeps 360 px wherever
# the strip, the rail, 360 px and the viewer's least fit (800 px).

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def fold(+w: Nat, +side: Nat, +rail: Nat, keep: Bool) -> String:
  yes(P.Panes.fold(w, side, 44n, rail, keep))

def view(+w: Nat, +side: Nat, +rail: Nat, +drag: Nat, split: Bool, keep: Bool) -> String:
  Nat.show(P.Panes.viewer(w, side, 44n, rail, drag, split, keep))

def drawer(+w: Nat, +side: Nat, +rail: Nat, keep: Bool) -> String:
  yes(P.Panes.drawer(w, side, 44n, rail, keep))

# what the thread gets beside them (the rail apart; a drawer lies over it)
def thread(+w: Nat, +side: Nat, +rail: Nat, +drag: Nat, split: Bool, +keep: Bool) -> String:
  +eff = Bool.pick(Nat, P.Panes.strip(w, side, 44n, rail, keep), 44n, side)
  Nat.show(Nat.sub(w, Nat.add(Nat.add(eff, rail), P.Panes.viewer(w, side, 44n, rail, drag, split, keep))))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("1400: the sidebar stays", fold(1400n, 264n, 76n, False{}), "no")
    C.Check.str("1400: the viewer takes its share", view(1400n, 264n, 76n, 0n, False{}, False{}), "595")
    C.Check.str("1400: the thread keeps more than its minimum", thread(1400n, 264n, 76n, 0n, False{}, False{}), "465")
    C.Check.str("1180: the sidebar still stays", fold(1180n, 264n, 76n, False{}), "no")
    C.Check.str("1179: it folds", fold(1179n, 264n, 76n, False{}), "yes")
    C.Check.str("1100: the sidebar folds", fold(1100n, 264n, 76n, False{}), "yes")
    C.Check.str("1100: the viewer takes its share of the whole page", view(1100n, 264n, 76n, 0n, False{}, False{}), "553")
    C.Check.str("1100: the thread", thread(1100n, 264n, 76n, 0n, False{}, False{}), "427")
    C.Check.str("800: folded, the viewer gives way to the thread", view(800n, 264n, 76n, 0n, False{}, False{}), "320")
    C.Check.str("800: the thread keeps its minimum", thread(800n, 264n, 76n, 0n, False{}, False{}), "360")
    C.Check.str("761: the viewer at its least", view(761n, 264n, 76n, 0n, False{}, False{}), "320")
    C.Check.str("a compact rail: 1150 holds all three", fold(1150n, 264n, 44n, False{}), "no")
    C.Check.str("shut by the person: no fold", fold(1100n, 44n, 76n, False{}), "no")
    C.Check.str("shut by the person: the share of the page", view(1100n, 44n, 76n, 0n, False{}, False{}), "553")
    C.Check.str("opened by the person: it stays", fold(1100n, 264n, 76n, True{}), "no")
    C.Check.str("opened by the person: the viewer gives way", view(1100n, 264n, 76n, 0n, False{}, True{}), "400")
    C.Check.str("opened by the person at 800: the viewer at its least", view(800n, 264n, 76n, 0n, False{}, True{}), "320")
    C.Check.str("opened by the person at 800: a drawer over the thread", drawer(800n, 264n, 76n, True{}), "yes")
    C.Check.str("opened by the person at 800: the thread keeps its minimum", thread(800n, 264n, 76n, 0n, False{}, True{}), "360")
    C.Check.str("opened by the person at 900: a drawer", drawer(900n, 264n, 76n, True{}), "yes")
    C.Check.str("opened by the person at 900: the thread keeps its minimum", thread(900n, 264n, 76n, 0n, False{}, True{}), "360")
    C.Check.str("opened by the person at 1019: still a drawer", drawer(1019n, 264n, 76n, True{}), "yes")
    C.Check.str("opened by the person at 1020: beside them", drawer(1020n, 264n, 76n, True{}), "no")
    C.Check.str("opened by the person at 1020: the thread keeps its minimum", thread(1020n, 264n, 76n, 0n, False{}, True{}), "360")
    C.Check.str("opened by the person at 1100: beside them, no drawer", drawer(1100n, 264n, 76n, True{}), "no")
    C.Check.str("opened by the person at 1100: the thread keeps its minimum", thread(1100n, 264n, 76n, 0n, False{}, True{}), "360")
    C.Check.str("dragged wide, opened at 800: the thread keeps its minimum", thread(800n, 264n, 76n, 900n, False{}, True{}), "360")
    C.Check.str("Split, opened at 900: the thread keeps its minimum", thread(900n, 264n, 76n, 0n, True{}, True{}), "360")
    C.Check.str("folded by itself: no drawer", drawer(800n, 264n, 76n, False{}), "no")
    C.Check.str("shut by the person: no drawer", drawer(800n, 44n, 76n, True{}), "no")
    C.Check.str("dragged wide at 1400: the thread keeps its minimum", view(1400n, 264n, 76n, 900n, False{}, False{}), "700")
    C.Check.str("dragged narrow: as dragged", view(1400n, 264n, 76n, 400n, False{}, False{}), "400")
    C.Check.str("dragged under its least", view(1400n, 264n, 76n, 200n, False{}, False{}), "320")
    C.Check.str("a drag never folds the sidebar", fold(1400n, 264n, 76n, False{}), "no")
    C.Check.str("Split at 1400: all but the thread's minimum", view(1400n, 264n, 76n, 0n, True{}, False{}), "700")
    C.Check.str("Split at 1100: folded, all but the thread's minimum", view(1100n, 264n, 76n, 0n, True{}, False{}), "620")
