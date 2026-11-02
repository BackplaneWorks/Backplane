import Base
import ../src/core/allowance.bend as A

# Independent proofs of the same new laws appended to LAWS/PROOF. This
# permits qualification without hiding unrelated composed-baseline failures.
law allowance_remaining_reported:
  {A.Allowance.remaining(280n) == 72n : Nat}
def allowance_remaining_reported(): {==}
law allowance_refresh_held:
  {A.Allowance.due.at(1000n, 1299n) == False{} : Bool}
def allowance_refresh_held(): {==}
law allowance_refresh_due:
  {A.Allowance.due.at(1000n, 1300n) == True{} : Bool}
def allowance_refresh_due(): {==}
law allowance_future_unknown:
  {A.Allowance.time.known(1001n, 1000n) == False{} : Bool}
def allowance_future_unknown(): {==}

def main() -> {A.Allowance.remaining(280n) == 72n : Nat}:
  allowance_remaining_reported()
