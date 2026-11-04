import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/cad-install.bend as CI
import ../src/server/cad-install.bend as FX

def snapshot(attempt: String, phase: String, done: Nat, total: Nat) -> J.Json:
  J.Json.obj([J.KV{"attempt", J.Str{attempt}}, J.KV{"phase", J.Str{phase}}, J.KV{"done", J.Json.nat(done)}, J.KV{"total", J.Json.nat(total)}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("in-phase bytes cannot decrease", Nat.show(J.Json.get_nat(CI.Cad.merge(snapshot("a", "download", 500n, 1000n), snapshot("a", "download", 400n, 1000n)), "done", 0n)), "500")
    C.Check.str("new attempt resets bytes", Nat.show(J.Json.get_nat(CI.Cad.merge(snapshot("a", "download", 500n, 1000n), snapshot("b", "download", 20n, 1000n)), "done", 0n)), "20")
    C.Check.str("new phase resets bytes", Nat.show(J.Json.get_nat(CI.Cad.merge(snapshot("a", "download", 500n, 1000n), snapshot("a", "extract", 0n, 0n)), "done", 1n)), "0")
    C.Check.str("download label uses real byte total", CI.Cad.transfer(snapshot("a", "download", 412000000n, 980000000n)), "Downloading 412 / 980 MB")
    C.Check.str("unknown length has no invented percentage", Bool.show(CI.Cad.known(snapshot("a", "download", 40n, 0n))), "False")
    C.Check.str("extraction is indeterminate", Bool.show(CI.Cad.known(snapshot("a", "extract", 100n, 100n))), "False")
    C.Check.str("unchanged percentage coalesces", Bool.show(CI.Cad.changed(snapshot("a", "download", 412000000n, 980000000n), snapshot("a", "download", 412000001n, 980000000n))), "False")
    C.Check.str("phase transition publishes", Bool.show(CI.Cad.changed(snapshot("a", "download", 500n, 1000n), snapshot("a", "extract", 0n, 0n))), "True")
    C.Check.str("cancel offers retry", CI.Cad.button(CI.CadCancelled{}), "Retry")
    C.Check.str("failed offers retry", CI.Cad.button(CI.CadFailed{}), "Retry")
    C.Check.str("running offers cancel", CI.Cad.action(CI.CadExtract{}), "cad-cancel")
    C.Check.str("unsupported offers guide", CI.Cad.action(CI.CadUnavailable{}), "cad-guide")
    C.Check.str("unknown phase fails visibly", CI.Cad.status(CI.Cad.phase("future")), "Installation failed")
    C.Check.str("Windows hub catalog", J.Json.get_str(FX.CadSpec.of("freecad", "/tmp/test", "CYGWIN_NT-10.0 x86_64"), "platform", ""), "windows-x86_64")
    C.Check.str("Mac hub catalog", J.Json.get_str(FX.CadSpec.of("freecad", "/tmp/test", "Darwin arm64"), "platform", ""), "darwin-arm64")
