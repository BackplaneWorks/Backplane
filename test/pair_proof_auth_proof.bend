import Base
import ../src/core/auth.bend as AUTH

# ---- tokenless pairing proofs (the same laws as LAWS.bend's, alone) ----
law auth_pair_proof_needs_token:
  for to: String
  for at: Nat
  for now: Nat
  for proof: String
  {AUTH.Auth.pair.checked("", to, at, now, proof) == False{} : Bool}

law auth_pair_proof_not_empty:
  for to: String
  for at: Nat
  for now: Nat
  {AUTH.Auth.pair.checked("secret", to, at, now, "") == False{} : Bool}

law auth_pair_proof_stale:
  for to: String
  {AUTH.Auth.pair.checked("secret", to, 1000n, 1061n, "ab") == False{} : Bool}
# ---- end tokenless pairing proofs ----

def auth_pair_proof_needs_token(to, at, now, proof): {==}
def auth_pair_proof_not_empty(to, at, now): {==}
def auth_pair_proof_stale(to): {==}
