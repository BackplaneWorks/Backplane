import Base
import ./check.bend as C
import ../src/web/workspace.bend as WS

# What Esc puts away next once no menu or dialog has it (workspace.bend's
# WS.esc.next): the floating terminal first, then the probe whose chip is on
# show, then the viewer; nothing open leaves "" (the keyboard goes back to
# the composer).

def next(term: Bool, probe: Bool, viewer: Bool) -> String:
  WS.WS.esc.next(term, probe, viewer)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("nothing open", next(False{}, False{}, False{}), "")
    C.Check.str("the terminal alone", next(True{}, False{}, False{}), "term")
    C.Check.str("the terminal over the viewer", next(True{}, False{}, True{}), "term")
    C.Check.str("the terminal over a probe", next(True{}, True{}, True{}), "term")
    C.Check.str("a probe before its viewer", next(False{}, True{}, True{}), "probe")
    C.Check.str("the viewer", next(False{}, False{}, True{}), "viewer")
