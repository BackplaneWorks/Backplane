import Base

# The clock a hub-drawn view is drawn with (hub view protocol H): the hub's
# seconds rounded down to the step the page's own clock tick asks at
# (ViewClock.step, host.js's 30 s tick). Everything a view shows of time
# (ages "5m", "Older", snoozes waking, stalled marks, today's stamps) moves
# at most once a step, so a view drawn in a step is the view all through
# it: the sidebar's image and the render memo are kept for the step and
# not redrawn every second. Actions keep the exact clock (reminders, ids).
# Laws viewclock_*.

def ViewClock.step() -> Nat:
  30n

def ViewClock.of(+now: Nat) -> Nat:
  Nat.sub(now, Nat.mod(now, ViewClock.step()))

# two times in the same step
def ViewClock.same(+a: Nat, +b: Nat) -> Bool:
  Nat.is_eq(ViewClock.of(a), ViewClock.of(b))

# the same for milliseconds
def ViewClock.same.ms(+a: Nat, +b: Nat) -> Bool:
  ViewClock.same(Nat.div(a, 1000n), Nat.div(b, 1000n))
