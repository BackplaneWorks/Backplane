import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/server/orderio.bend as OD
import ../src/server/google.bend as GS

# An order server that does not answer at all is said in the person's
# words, not curl's.

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("no answer: what to do, not curl's words", J.Json.get_str(OD.Od.est.got(GS.GResp{"000", "", "curl: (7) Failed to connect to 127.0.0.1:9 after 0 ms: Could not connect to server"}), "error", ""), "the order server is not reachable: check this machine's internet connection, then Retry")
    C.Check.str("an answer with its own error keeps it", J.Json.get_str(OD.Od.est.got(GS.GResp{"400", "{\"error\":\"bad size\"}", ""}), "error", ""), "bad size")
