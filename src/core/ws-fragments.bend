import Base
import ./http.bend as W

# A message may span frames. Control frames pass through immediately without
# consuming its buffered bytes; only a final data frame emits the message.
type Pending is Data:
  FragIdle{}
  FragOpen{op: U32, rev: List<&2, U32>}

type Taken is Data:
  FragReject{}
  FragHold{pending: Pending}
  FragEmit{pending: Pending, frame: W.Frame}

def Fragment.reverse(bytes: List<&2, U32>, acc: List<&2, U32>) -> List<&2, U32>:
  match bytes:
    case Nil{}: acc
    case b <> rest: Fragment.reverse(rest, b <> acc)

def Fragment.finish(fin: Bool, +op: U32, rev: List<&2, U32>) -> Taken:
  match fin:
    case False{}: FragHold{FragOpen{op, rev}}
    case True{}: FragEmit{FragIdle{}, W.Frame{True{}, op, Fragment.reverse(rev, Nil{})}}

def Fragment.continue(pending: Pending, +fin: Bool, bytes: List<&2, U32>) -> Taken:
  match pending:
    case FragIdle{}: FragReject{}
    case FragOpen{op, rev}: Fragment.finish(fin, op, Fragment.reverse(bytes, rev))

def Fragment.begin(fin: Bool, +op: U32, bytes: List<&2, U32>) -> Taken:
  match fin:
    case True{}: FragEmit{FragIdle{}, W.Frame{True{}, op, bytes}}
    case False{}: FragHold{FragOpen{op, Fragment.reverse(bytes, Nil{})}}

def Fragment.start(pending: Pending, +fin: Bool, +op: U32, bytes: List<&2, U32>) -> Taken:
  match pending:
    case FragIdle{}: Fragment.begin(fin, op, bytes)
    case FragOpen{_, _}: FragReject{}

def Fragment.data(+op: U32, pending: Pending, +fin: Bool, bytes: List<&2, U32>) -> Taken:
  match op:
    case 0: Fragment.continue(pending, fin, bytes)
    case 1: Fragment.start(pending, fin, op, bytes)
    case 2: Fragment.start(pending, fin, op, bytes)
    case _: FragReject{}

def Fragment.control(control: Bool, pending: Pending, +fin: Bool, +op: U32, bytes: List<&2, U32>) -> Taken:
  match control:
    case True{}: FragEmit{pending, W.Frame{fin, op, bytes}}
    case False{}: Fragment.data(op, pending, fin, bytes)

def Fragment.step(pending: Pending, frame: W.Frame) -> Taken:
  match frame:
    case W.Frame{fin, +op, bytes}:
      Fragment.control(Bool.or(U32.is_eq(op, 8), Bool.or(U32.is_eq(op, 9), U32.is_eq(op, 10))), pending, fin, op, bytes)
