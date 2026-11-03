import Base
import ./check.bend as C
import ../src/core/pending-stream.bend as P
import ../src/core/thin-live.bend as TL
import ../src/core/thin.bend as T
import ../src/core/chat-pending.bend as CP
import ../src/core/i18n.bend as I
import ../src/core/model.bend as M
import ../src/core/thin-pace.bend as TP

def stream(gen: Nat, size: Nat, text: String, cut: Bool, version: Nat) -> TL.Stream:
  TL.Stream{gen, size, T.Blocks.of(text), cut, version}

def changed(name: String, old: TL.Stream, new: TL.Stream, expected: String) -> IO(Unit):
  C.Check.str(name, Bool.show(P.PendingStream.changed(old, new)), expected)

def row.text(entry: M.Entry, lc: I.Loc) -> String:
  match entry:
    case M.Msg{id, _, _, text, _}: CP.Pending.text(lc, id, text)
    case _: "unexpected non-message"

# Actual synthetic row identity and Pending.text determine the visible label;
# its underlying preview may change freely while the static row stays present.
def row(s: TL.Stream, lc: I.Loc) -> String:
  row.text(TL.Stream.entry(s, "thread", 100n), lc)

# Arbitrary partial contents are erased by the shared pending presentation.
def pending_label(+lc: I.Loc, text: String) ->
    {CP.Pending.text(lc, "@live:thread", text) == CP.Pending.label(lc) : String}:
  {==}

def labels(+name: String, +old: TL.Stream, +new: TL.Stream, +lc: I.Loc) -> IO(Unit):
  do IO<Unit>:
    C.Check.str(name ++ " old label", row(old, lc), CP.Pending.label(lc))
    C.Check.str(name ++ " new label", row(new, lc), CP.Pending.label(lc))
    C.Check.str(name ++ " same visible row", row(old, lc), row(new, lc))
    C.Check.str(name ++ " no delta notice", Bool.show(P.PendingStream.changed(old, new)), "False")

def gate(notice: Bool, base: TP.Touch) -> TP.Touch:
  match notice:
    case False{}: base
    case True{}: TP.Touch.add(base, TP.Touch.live("thread"))

def main() -> IO(Unit):
  do IO<Unit>:
    changed("first delta changes presence", stream(7n, 0n, "", False{}, 1n), stream(7n, 1n, "x", False{}, 2n), "True")
    changed("initial generation and first delta", TL.Stream.blank(), stream(7n, 1n, "x", False{}, 2n), "True")
    changed("clear changes presence", stream(7n, 1n, "x", False{}, 2n), stream(7n, 0n, "", False{}, 3n), "True")
    changed("generation change stays conservative", stream(7n, 1n, "x", False{}, 1n), stream(8n, 1n, "x", False{}, 2n), "True")
    changed("generation change while both empty", stream(7n, 0n, "", False{}, 1n), stream(8n, 0n, "", False{}, 2n), "True")
    changed("generation rollback stays conservative", stream(8n, 1n, "x", False{}, 1n), stream(7n, 1n, "x", False{}, 2n), "True")
    changed("same empty stream remains quiet", TL.Stream.blank(), TL.Stream.blank(), "False")
    changed("empty stream ignores version and cut", stream(7n, 0n, "", False{}, 1n), stream(7n, 0n, "", True{}, 500n), "False")
    changed("same size and generation ignore changed blocks", stream(7n, 1n, "x", False{}, 1n), stream(7n, 1n, "y", False{}, 2n), "False")
    changed("size growth stays quiet", stream(7n, 1n, "x", False{}, 1n), stream(7n, 1000n, "larger", False{}, 2n), "False")
    changed("size shrink stays quiet", stream(7n, 1000n, "larger", False{}, 1n), stream(7n, 1n, "x", False{}, 2n), "False")
    changed("cut transition stays quiet", stream(7n, 1n, "x", False{}, 1n), stream(7n, 1n, "y", True{}, 2n), "False")
    changed("cut cleared stays quiet", stream(7n, 1n, "x", True{}, 1n), stream(7n, 1n, "y", False{}, 2n), "False")
    changed("empty blocks do not erase present scalar size", stream(7n, 1n, "x", False{}, 1n), stream(7n, 1n, "", False{}, 2n), "False")
    changed("nonempty blocks cannot create zero-size presence", stream(7n, 0n, "", False{}, 1n), stream(7n, 0n, "ignored block", False{}, 2n), "False")
    labels("English static pending", stream(7n, 1n, "SECRET PARTIAL A", False{}, 1n), stream(7n, 1000n, "different λ🙂 content", True{}, 2n), I.En{})
    labels("Japanese static pending", stream(7n, 1000n, "different λ🙂 content", True{}, 3n), stream(7n, 1n, "SECRET PARTIAL B", False{}, 4n), I.Ja{})
    C.Check.str("canonical content still shows its complete reply", CP.Pending.text(I.En{}, "posted-message", "Full canonical answer"), "Full canonical answer")
    +quiet : Bool = P.PendingStream.changed(stream(7n, 1n, "x", False{}, 1n), stream(7n, 2n, "xy", False{}, 2n))
    C.Check.str("quiet delta does not invent a global touch", Nat.show(TP.Touch.level(gate(quiet, TP.Touch.none()), "other")), "0")
    C.Check.str("quiet delta retains an existing global touch", Nat.show(TP.Touch.level(gate(quiet, TP.Touch.add(TP.Touch.none(), "*")), "other")), "2")
    C.Check.str("quiet delta retains an existing canonical thread touch", Nat.show(TP.Touch.level(gate(quiet, TP.Touch.add(TP.Touch.none(), "other")), "other")), "2")
    C.Check.str("quiet delta retains a private client touch", Nat.show(TP.Touch.level.for(gate(quiet, TP.Touch.add(TP.Touch.none(), TP.Touch.client("7"))), "other", "7")), "2")
