import Base

# A terminal's viewport stays between the live screen and its oldest kept
# row. Moving back to the live screen never underflows.
def TermScroll.move(+at: Nat, +history: Nat, back: Bool, +rows: Nat) -> Nat:
  match back:
    case True{}: Nat.min(history, Nat.add(at, rows))
    case False{}: Nat.min(history, Nat.sub(at, rows))
