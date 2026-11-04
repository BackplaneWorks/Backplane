import Base
import ../core/model.bend as M
import ../core/json.bend as J

# Private in-process writer values and their original offered-credit policy.
# Producers and native consumers import this same concrete data definition.

type Wire is Data:
  WText{text: String}
  # Socket-only text with its original offered credit. Native/link readers
  # keep WText; compression and batching never redefine this charge.
  WCharged{text: String, charge: Nat}
  WPong{payload: List<&2, U32>}
  # a message already encoded (a board's plot: never parsed again)
  WBin{cbor: List<&2, U32>}
  WClose{}
  # the hub's own window: the read model itself, with the log message's
  # head (no items), in place of the log as text
  WState{st: M.State, head: String}
  # a message made of a log's lines: head, the lines joined by ",", tail;
  # put together by the writer, off the hub (a join's chunk)
  WItems{head: String, plain: String, split: String, items: List<&2, String>, tail: String, ends: Bool}
  # a message as its JSON tree with its charge (a thin view's reply): the
  # writer encodes the tree, never printing and parsing it again
  WJson{json: J.Json, charge: Nat}
  # At most one floating pane in flight; its writer reports credit at once.
  WTerminal{json: J.Json, charge: Nat}

def Wire.charged(native: Bool, w: Wire, n: Nat) -> Wire:
  match native w:
    case False{} WText{t}: WCharged{t, n}
    case False{} WCharged{t, _}: WCharged{t, n}
    case False{} WJson{j, _}: WJson{j, n}
    case False{} WTerminal{j, _}: WTerminal{j, n}
    case _ _: w

def Wire.text.charge(+text: String) -> Nat:
  String.length(text)
