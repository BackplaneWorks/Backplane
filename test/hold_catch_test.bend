import Base
import ./check.bend as C
import ../src/web/app.bend as A

# A slow scroll down a finished thread lands where it lands: only a reply
# streaming in lets a downward gesture near the end catch up with it
# (host.js's holdRead), and the end itself stays a few px.

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("streaming: a third of the view from the end catches up", Bool.show(A.hold_catch_up(240n, 744n, True{})), "True")
    C.Check.str("streaming: the end catches up", Bool.show(A.hold_catch_up(0n, 744n, True{})), "True")
    C.Check.str("streaming: past a third holds", Bool.show(A.hold_catch_up(249n, 744n, True{})), "False")
    C.Check.str("finished: a third of the view from the end holds", Bool.show(A.hold_catch_up(240n, 744n, False{})), "False")
    C.Check.str("finished: a notch short of the end holds", Bool.show(A.hold_catch_up(40n, 744n, False{})), "False")
    C.Check.str("at the end: within the slack", Bool.show(A.hold_at_end(4n)), "True")
    C.Check.str("at the end: not past it", Bool.show(A.hold_at_end(5n)), "False")
