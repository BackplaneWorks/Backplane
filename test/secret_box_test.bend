import Base
import ../src/core/secret-box.bend as SB
import ./check.bend as C

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("secret Save has a secure-host slot", Nat.show(SB.Sbox.route("secret-save")), "9")
    C.Check.str("unknown secret actions cannot fall into List", Nat.show(SB.Sbox.route("secret-missing")), "10")
