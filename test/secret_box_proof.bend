import Base
import ../src/core/secret-box.bend as SB

law secp_box_handlers:
  {SB.Sbox.routes(SB.Sbox.actions()) == [0n, 1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n] : List<&2, Nat>}

def secp_box_handlers(): {==}
