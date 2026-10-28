import Base

# The panes of a wide page (the web and the desktop app, over 760 px): the
# sidebar, the thread (the rail of views beside it) and the viewer. The
# thread keeps Panes.thread() px. Opening the viewer where the window
# cannot hold the open sidebar, the thread at its minimum and a viewer of
# Panes.fair() px folds the sidebar by itself; it comes back when the
# viewer closes or the window grows. The person's own choice wins: a
# sidebar they shut stays shut, one they open while the viewer is open
# stays open; where it cannot stand beside the thread's minimum and a
# viewer of Panes.least() px, it opens over the thread as a drawer (the
# panes stay as beside its strip) until a click outside it, Esc or a
# thread picked puts it back. The viewer then takes its width (dragged, or
# its share: 11/21 of the room beside the sidebar, all but the thread's
# minimum for Split) and never more than leaves the thread its minimum,
# nor less than Panes.least(). Every width is in CSS px. Test:
# test/pane_room_test.bend.

def Panes.thread() -> Nat:
  360n

def Panes.fair() -> Nat:
  480n

def Panes.least() -> Nat:
  320n

def Panes.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

def Panes.max(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(a, b), a, b)

# what the viewer may take beside a sidebar this wide and the rail, the
# thread kept at its minimum
def Panes.room(+w: Nat, +side: Nat, +rail: Nat) -> Nat:
  Nat.sub(w, Nat.add(Nat.add(side, rail), Panes.thread()))

# the sidebar folds by itself: open (wider than its strip), not kept open
# by the person, and the room beside it short of a fair viewer
def Panes.fold.k(keep: Bool, +w: Nat, +side: Nat, +strip: Nat, +rail: Nat) -> Bool:
  match keep:
    case True{}:
      False{}
    case False{}:
      Bool.and(Nat.is_gt(side, strip), Nat.is_lt(Panes.room(w, side, rail), Panes.fair()))

# w the page's width, side the sidebar's as the person left it (open, or
# its strip when they shut it), strip its width folded, rail the views'
def Panes.fold(+w: Nat, +side: Nat, +strip: Nat, +rail: Nat, keep: Bool) -> Bool:
  Panes.fold.k(keep, w, side, strip, rail)

# the sidebar the person opened is a drawer over the thread: open, and the
# room beside it short of the viewer's least
def Panes.drawer.k(keep: Bool, +w: Nat, +side: Nat, +strip: Nat, +rail: Nat) -> Bool:
  match keep:
    case True{}:
      Bool.and(Nat.is_gt(side, strip), Nat.is_lt(Panes.room(w, side, rail), Panes.least()))
    case False{}:
      False{}

def Panes.drawer(+w: Nat, +side: Nat, +strip: Nat, +rail: Nat, keep: Bool) -> Bool:
  Panes.drawer.k(keep, w, side, strip, rail)

# the panes are laid out beside the sidebar's strip: folded by itself, or
# open over the thread as a drawer
def Panes.strip(+w: Nat, +side: Nat, +strip: Nat, +rail: Nat, +keep: Bool) -> Bool:
  Bool.or(Panes.fold(w, side, strip, rail, keep), Panes.drawer(w, side, strip, rail, keep))

# the viewer's wish: the width it was dragged to, else its share
def Panes.want(+w: Nat, +side: Nat, +rail: Nat, +drag: Nat, split: Bool) -> Nat:
  Bool.pick(Nat, Nat.is_gt(drag, 0n), drag,
    Bool.pick(Nat, split, Panes.room(w, side, rail), Nat.div(Nat.mul(Nat.sub(w, side), 11n), 21n)))

# its width beside a sidebar this wide (folded or not)
def Panes.at(+w: Nat, +side: Nat, +rail: Nat, +drag: Nat, split: Bool) -> Nat:
  Panes.max(Panes.least(), Panes.min(Panes.want(w, side, rail, drag, split), Panes.room(w, side, rail)))

# the viewer's width: beside the strip while the sidebar folds by itself
# or is a drawer
def Panes.viewer(+w: Nat, +side: Nat, +strip: Nat, +rail: Nat, +drag: Nat, split: Bool, +keep: Bool) -> Nat:
  Panes.at(w, Bool.pick(Nat, Panes.strip(w, side, strip, rail, keep), strip, side), rail, drag, split)
