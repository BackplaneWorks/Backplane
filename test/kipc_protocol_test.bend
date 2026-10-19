import Base
import ./check.bend as C
import ../src/server/kserve.bend as KS
import ../src/server/proc.bend as P

def accepts(out: String) -> String:
  Bool.show(KS.KServe.protocol(P.Ran{0, out}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("matching helper protocol", accepts("{\"name\":\"backplane-kipc\",\"protocolVersion\":1,\"runtimeApi\":2}"), "True")
    C.Check.str("legacy helper rejected", accepts(""), "False")
    C.Check.str("malformed description rejected", accepts("{not JSON}"), "False")
    C.Check.str("unknown protocol rejected", accepts("{\"name\":\"backplane-kipc\",\"protocolVersion\":2,\"runtimeApi\":2}"), "False")
    C.Check.str("old runtime API rejected", accepts("{\"name\":\"backplane-kipc\",\"protocolVersion\":1,\"runtimeApi\":1}"), "False")
    C.Check.str("failed command rejected", Bool.show(KS.KServe.protocol(P.Ran{1, "{\"name\":\"backplane-kipc\",\"protocolVersion\":1,\"runtimeApi\":2}"})), "False")
