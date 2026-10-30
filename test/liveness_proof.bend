import Base
import ../src/core/liveness.bend as LV

def threshold(active: Bool, +now: Nat, +gap: Nat, grace: Nat, +l: LV.Live,
  h: {LV.Liveness.due(active, now, gap, LV.Liveness.last(l)) == False{} : Bool}) ->
  {LV.Liveness.stalled(LV.Decision.live(LV.Liveness.tick(active, now, gap, grace, l))) == False{} : Bool}:
  match l:
    case LV.Live{+last, +nudged, +stalled, +reported, +reason}:
      %Equal.sym(Bool, LV.Liveness.due(active, now, gap, last), False{}, h) :
        {LV.Liveness.stalled(LV.Decision.live(LV.Liveness.decide(_, now, grace, LV.Live{last, nudged, stalled, reported, reason}))) == False{} : Bool}
      {==}

def escalate_nudge(waited: Bool, l: LV.Live) ->
  {LV.Decision.nudge(LV.Liveness.escalate(waited, l)) == False{} : Bool}:
  match waited l:
    case False{} _: {==}
    case True{} LV.Live{_, _, _, _, _}: {==}

def sent(+now: Nat, grace: Nat, +l: LV.Live) ->
  {LV.Decision.nudge(LV.Liveness.gap(True{}, now, grace, l)) == False{} : Bool}:
  match l:
    case LV.Live{+last, +nudged, +stalled, +reported, +reason}:
      escalate_nudge(Nat.is_ge(now, Nat.add(nudged, grace)), LV.Live{last, nudged, stalled, reported, reason})

def decide_sent(due: Bool, +now: Nat, +grace: Nat, +l: LV.Live,
  h: {Nat.is_gt(LV.Liveness.nudged(l), 0n) == True{} : Bool}) ->
  {LV.Decision.nudge(LV.Liveness.decide(due, now, grace, l)) == False{} : Bool}:
  match due l:
    case False{} LV.Live{_, _, _, _, _}: {==}
    case True{} LV.Live{+last, +nudged, +stalled, +reported, +reason}:
      %Equal.sym(Bool, Nat.is_gt(nudged, 0n), True{}, h) :
        {LV.Decision.nudge(LV.Liveness.gap(_, now, grace, LV.Live{last, nudged, stalled, reported, reason})) == False{} : Bool}
      sent(now, grace, LV.Live{last, nudged, stalled, reported, reason})

def tick_sent(active: Bool, +now: Nat, gap: Nat, grace: Nat, +l: LV.Live,
  h: {Nat.is_gt(LV.Liveness.nudged(l), 0n) == True{} : Bool}) ->
  {LV.Decision.nudge(LV.Liveness.tick(active, now, gap, grace, l)) == False{} : Bool}:
  decide_sent(LV.Liveness.due(active, now, gap, LV.Liveness.last(l)), now, grace, l, h)

def no_kill(d: LV.Decision) -> {LV.Decision.kills(d) == False{} : Bool}:
  match d:
    case LV.Decision{_, _, _}: {==}

def notify_once(waited: Bool, last: Nat, nudged: Nat, stalled: Bool, reason: String) ->
  {LV.Decision.notify(LV.Liveness.escalate(waited, LV.Live{last, nudged, stalled, True{}, reason})) == False{} : Bool}:
  match waited:
    case False{}: {==}
    case True{}: {==}

def progress(l: LV.Live, now: Nat, reason: String) ->
  {LV.Liveness.stalled(LV.Liveness.progress(l, now, reason)) == False{} : Bool}:
  match l:
    case LV.Live{_, _, _, _, _}: {==}

def nudge_first(due: Bool, now: Nat, grace: Nat, last: Nat, reason: String) ->
  {LV.Liveness.stalled(LV.Decision.live(LV.Liveness.decide(due, now, grace, LV.Live{last, 0n, False{}, False{}, reason}))) == False{} : Bool}:
  match due:
    case False{}: {==}
    case True{}: {==}
