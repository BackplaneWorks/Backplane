import Base
import ./check.bend as C
import ../src/server/host.bend as H

# The real Bend/JS effect boundary, with a fresh private test directory.
# The test never opens a hub or reads the person's home or credentials.
def main() -> IO(Unit):
  do IO<Unit>:
    +home : String <- IO.try(String, H.FS.mkdtemp("/tmp/backplane-token-contract-"))
    +first : String <- IO.try(String, H.FS.pairing_token(home ++ "/token"))
    C.Check.str("pairing token effect creates 128-bit hex", Nat.show(String.length(first)), "32")
    again : String <- IO.try(String, H.FS.pairing_token(home ++ "/token"))
    C.Check.str("pairing token effect preserves", again, first)
    IO.try(Unit, H.FS.remove(home ++ "/token"))
    IO.try(Unit, H.FS.remove(home))
