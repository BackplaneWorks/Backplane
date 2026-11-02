import Base
import ./hub.bend as H

# Only the final fallback passes may be bypassed. Run/Wake/Ride/Locks and
# watchdog after-processing must already have run before this classification.
# No state or capabilities are read, and no output is rebuilt or reordered.
def StreamStep.live_only(os: List<&2, H.Out>) -> Bool:
  match os:
    case Nil{}: True{}
    case H.Live{_, _} <> rest: StreamStep.live_only(rest)
    case _ <> rest: False{}

def StreamStep.eligible(step: H.Step) -> Bool:
  match step:
    case H.Step{Nil{}, os}: StreamStep.live_only(os)
    case H.Step{_ <> rest, _}: False{}
