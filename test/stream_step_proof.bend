import Base
import ../src/core/stream-step.bend as S
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# A witness for any finite sequence accepted by live_only, including empty.
type LiveRow is Data:
  LiveRow{thread: String, text: String}

def outputs(xs: List<&2, LiveRow>) -> List<&2, H.Out>:
  match xs:
    case Nil{}: Nil{}
    case LiveRow{thread, text} <> rest: H.Live{thread, text} <> outputs(rest)

def stream(xs: List<&2, LiveRow>) -> H.Step:
  H.Step{Nil{}, outputs(xs)}

def eligible(+xs: List<&2, LiveRow>) -> {S.StreamStep.eligible(stream(xs)) == True{} : Bool}:
  match xs:
    case Nil{}: {==}
    case LiveRow{thread, text} <> +rest:
      eligible(rest)

def fallback_outs(+xs: List<&2, LiveRow>, +s: M.State, +caps: J.Json, +now: Nat) ->
    {H.Fallback.outs(outputs(xs), s, caps, now) == stream(xs) : H.Step}:
  match xs:
    case Nil{}: {==}
    case LiveRow{+thread, +text} <> +rest:
      %fallback_outs(rest, s, caps, now) :
        {H.Step.cat(H.Step{Nil{}, [H.Live{thread, text}]}, H.Fallback.outs(outputs(rest), s, caps, now)) == H.Step.cat(H.Step{Nil{}, [H.Live{thread, text}]}, _) : H.Step}
      {==}

# Exact identity for arbitrary state, capabilities, time, thread IDs, text,
# and stream length. No assumption about installed providers or open asks.
def fallback_validate(+xs: List<&2, LiveRow>, +s: M.State, +caps: J.Json, +now: Nat) ->
    {H.Fallback.validate(s, caps, now, stream(xs)) == stream(xs) : H.Step}:
  %fallback_outs(xs, s, caps, now) :
    {H.Step.before(Nil{}, Nil{}, H.Fallback.outs(outputs(xs), s, caps, now)) == H.Step.before(Nil{}, Nil{}, _) : H.Step}
  {==}

def run_fallback(+xs: List<&2, LiveRow>, +s: M.State, +now: Nat) ->
    {H.Run.fallback(s, now, stream(xs)) == stream(xs) : H.Step}:
  {==}

def safe_bypass(+xs: List<&2, LiveRow>, +s: M.State, +caps: J.Json, +now: Nat) ->
    {H.Run.fallback(s, now, H.Fallback.validate(s, caps, now, stream(xs))) == stream(xs) : H.Step}:
  Equal.trans(H.Step, H.Run.fallback(s, now, H.Fallback.validate(s, caps, now, stream(xs))), H.Run.fallback(s, now, stream(xs)), stream(xs),
    Equal.cong(H.Step, H.Step, x => H.Run.fallback(s, now, x), H.Fallback.validate(s, caps, now, stream(xs)), stream(xs), fallback_validate(xs, s, caps, now)),
    run_fallback(xs, s, now))
