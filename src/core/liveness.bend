import Base
import ./json.bend as J

# A progress gap is an episode: one nudge, then one escalation, ending only
# on real progress. Idle work clears its visible stall but retains the
# episode markers, so a clock adjustment cannot cause duplicate nudges. Watchdog
# actions contain no process operation. IO delivers these notices separately.
type Live is Data:
  Live{last: Nat, nudged: Nat, stalled: Bool, reported: Bool, reason: String}

type Decision is Data:
  Decision{live: Live, nudge: Bool, notify: Bool}

def Liveness.default.gap() -> Nat:
  1200n

def Liveness.default.grace() -> Nat:
  60n

def Liveness.key(thread: String) -> String:
  "liveness." ++ thread

def Liveness.last(l: Live) -> Nat:
  match l:
    case Live{last, _, _, _, _}: last

def Liveness.nudged(l: Live) -> Nat:
  match l:
    case Live{_, nudged, _, _, _}: nudged

def Liveness.stalled(l: Live) -> Bool:
  match l:
    case Live{_, _, stalled, _, _}: stalled

def Liveness.reported(l: Live) -> Bool:
  match l:
    case Live{_, _, _, reported, _}: reported

def Liveness.reason(l: Live) -> String:
  match l:
    case Live{_, _, _, _, reason}: reason

def Decision.live(d: Decision) -> Live:
  match d:
    case Decision{live, _, _}: live

def Decision.nudge(d: Decision) -> Bool:
  match d:
    case Decision{_, nudge, _}: nudge

def Decision.notify(d: Decision) -> Bool:
  match d:
    case Decision{_, _, notify}: notify

# Explicitly total numeric parsing, also usable for settings. Avoid Nat.read
# in values laws normalize: it expands very slowly at epoch-second scale.
def Liveness.number.go(text: String, +fallback: Nat, acc: Nat) -> Nat:
  match text:
    case SNil{}: acc
    case SCon{Chr{+c}, rest}:
      match c:
        case 48: Liveness.number.go(rest, fallback, Nat.mul(acc, 10n))
        case 49: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 1n))
        case 50: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 2n))
        case 51: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 3n))
        case 52: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 4n))
        case 53: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 5n))
        case 54: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 6n))
        case 55: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 7n))
        case 56: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 8n))
        case 57: Liveness.number.go(rest, fallback, Nat.add(Nat.mul(acc, 10n), 9n))
        case _: fallback

def Liveness.number(text: String, +fallback: Nat) -> Nat:
  match text:
    case SNil{}: fallback
    case SCon{c, rest}: Liveness.number.go(SCon{c, rest}, fallback, 0n)

def Liveness.read.obj(+obj: J.Json) -> Live:
  Live{Liveness.number(J.Json.as_raw(J.Json.get(obj, "last"), "0"), 0n),
    Liveness.number(J.Json.as_raw(J.Json.get(obj, "nudged"), "0"), 0n),
    J.Json.get_bool(obj, "stalled", False{}), J.Json.get_bool(obj, "reported", False{}),
    J.Json.get_str(obj, "reason", "turn or owned work")}

def Liveness.read(line: String) -> Live:
  Liveness.read.obj(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))

def Liveness.write(l: Live) -> String:
  match l:
    case Live{last, nudged, stalled, reported, reason}:
      J.Json.show(J.Json.obj([J.KV{"last", J.Json.nat(last)}, J.KV{"nudged", J.Json.nat(nudged)},
        J.KV{"stalled", J.Flag{stalled}}, J.KV{"reported", J.Flag{reported}}, J.KV{"reason", J.Str{reason}}]))

def Liveness.progress(l: Live, +now: Nat, reason: String) -> Live:
  match l:
    case Live{+last, _, _, _, _}:
      Live{Bool.pick(Nat, Nat.is_ge(now, last), now, last), 0n, False{}, False{}, reason}

def Liveness.due(active: Bool, +now: Nat, +gap: Nat, +last: Nat) -> Bool:
  Bool.and(active, Bool.and(Nat.is_gt(gap, 0n), Bool.and(Nat.is_ge(now, last), Nat.is_ge(Nat.sub(now, last), gap))))

def Liveness.escalate(waited: Bool, +l: Live) -> Decision:
  match waited l:
    case False{} _: Decision{l, False{}, False{}}
    case True{} Live{last, nudged, _, +reported, reason}:
      Decision{Live{last, nudged, True{}, True{}, reason}, False{}, Bool.not(reported)}

def Liveness.gap(sent: Bool, +now: Nat, grace: Nat, +l: Live) -> Decision:
  match sent l:
    case False{} Live{last, _, _, _, reason}:
      Decision{Live{last, now, False{}, False{}, reason}, True{}, False{}}
    case True{} _:
      Liveness.escalate(Nat.is_ge(now, Nat.add(Liveness.nudged(l), grace)), l)

def Liveness.decide(due: Bool, +now: Nat, grace: Nat, +l: Live) -> Decision:
  match due l:
    case False{} Live{last, nudged, _, reported, reason}:
      Decision{Live{last, nudged, False{}, reported, reason}, False{}, False{}}
    case True{} _:
      Liveness.gap(Nat.is_gt(Liveness.nudged(l), 0n), now, grace, l)

def Liveness.tick(active: Bool, +now: Nat, +gap: Nat, grace: Nat, +l: Live) -> Decision:
  Liveness.decide(Liveness.due(active, now, gap, Liveness.last(l)), now, grace, l)

# There is deliberately no kill, interrupt, cancel or restart outcome.
def Decision.kills(d: Decision) -> Bool:
  match d:
    case Decision{_, _, _}: False{}

# Missing timeout takes the default. An explicitly present zero opts out.
def Liveness.timeout(present: Bool, seconds: Nat) -> Nat:
  match present:
    case True{}: seconds
    case False{}: Liveness.default.gap()

def Liveness.nudge.text(reason: String) -> String:
  "You appear stalled at " ++ reason ++ "; report status or continue. Busy work must keep running."
