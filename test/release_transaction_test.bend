import Base
import ./check.bend as C
import ../src/core/checks.bend as K
import ../src/core/json.bend as J
import ../src/server/review.bend as R
import ../src/server/proc.bend as P
import ../src/core/release.bend as L

def parsed(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("empty tag object cannot form deletion refspec", Bool.show(L.Release.object_valid("")), "False")
    C.Check.str("tag object requires full hex", Bool.show(L.Release.object_valid("fixture-tag")), "False")
    C.Check.str("full git object accepted", Bool.show(L.Release.object_valid("0123456789abcdef0123456789abcdef01234567")), "True")
    C.Check.str("ERC report requires sheets", Bool.show(K.Checks.report_valid("erc", "{}")), "False")
    C.Check.str("DRC report requires arrays", Bool.show(K.Checks.report_valid("drc", "{\"violations\":[]}")), "False")
    C.Check.str("malformed report refused", Bool.show(K.Checks.report_valid("erc", "{")), "False")
    C.Check.str("wrong report shape refused", Bool.show(K.Checks.report_valid("erc", "{\"sheets\":{}}")), "False")
    C.Check.str("malformed sheet refused", Bool.show(K.Checks.report_valid("erc", "{\"sheets\":[{}]}")), "False")
    C.Check.str("malformed violation refused", Bool.show(K.Checks.report_valid("drc", "{\"violations\":[{}],\"unconnected_items\":[],\"schematic_parity\":[]}")), "False")
    C.Check.str("empty valid ERC accepted", Bool.show(K.Checks.report_valid("erc", "{\"sheets\":[]}")), "True")
    C.Check.str("empty valid DRC accepted", Bool.show(K.Checks.report_valid("drc", "{\"violations\":[],\"unconnected_items\":[],\"schematic_parity\":[]}")), "True")
    C.Check.str("missing gate fails", R.Rvr.check.error(parsed("{}")), "checks returned no valid gate")
    C.Check.str("wrong typed gate fails", R.Rvr.check.error(parsed("{\"gate\":false}")), "checks returned no valid gate")
    C.Check.str("explicit gate passes", R.Rvr.check.error(parsed("{\"gate\":\"\"}")), "")
    C.Check.str("error cannot hide behind gate", R.Rvr.check.error(parsed("{\"gate\":\"\",\"err\":\"failed\"}")), "failed")
    C.Check.str("failed child cannot return success", Bool.show(String.contains(R.Rvr.check.error(R.Rvr.json(P.Ran{9, "{\"gate\":\"\"}"})), "exit 9")), "True")
    C.Check.str("command failure is infrastructure", K.Checks.execution(R.Rvc.chk.read(P.Ran{23, "synthetic"}, "erc", "")), "erc: check command failed (exit 23): synthetic\n")
    C.Check.str("invalid report is infrastructure", K.Checks.execution(R.Rvc.chk.read(P.Ran{0, ""}, "drc", "{}")), "drc: missing or malformed check report\n")
    C.Check.str("healthy control has no infrastructure errors", K.Checks.execution(R.Rvc.chk.read(P.Ran{0, ""}, "erc", "{\"sheets\":[]}")), "")
