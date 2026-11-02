import Base
import ./check.bend as C
import ../src/server/diagnostics.bend as D
import ../src/server/perf.bend as P
import ../src/server/host.bend as H

# The real Bend -> JS effect boundary. Always creates its own disposable
# home; does not start a hub or inspect an installed one.
def main() -> IO(Unit):
  do IO<Unit>:
    +home : String <- IO.try(String, H.FS.mkdtemp("/tmp/backplane-diagnostics-smoke-"))
    H.Env.put("BACKPLANE_HOME", home)
    +started : String <- D.Diagnostics.start("1.2.3")
    C.Check.str("diagnostics started", Bool.show(String.contains(started, "\"enabled\":true")), "True")
    t0 : U32 <- P.Perf.us()
    t1 : U32 <- P.Perf.span("hub.rpc", t0)
    P.Perf.add("hub.write_fail", 1)
    P.Log.error("test", "PRIVATE_TEST_TEXT /private/path")
    D.Diagnostics.event("connection", "reconnect", 1)
    D.Diagnostics.tick()
    +out : String <- D.Diagnostics.export()
    C.Check.str("real span hook", Bool.show(String.contains(out, "hub.rpc")), "True")
    C.Check.str("severity only", Bool.show(String.contains(out, "log_error")), "True")
    C.Check.str("no log text", Bool.show(String.contains(out, "PRIVATE_TEST_TEXT")), "False")
    +off : String <- D.Diagnostics.control("off")
    C.Check.str("disabled", Bool.show(String.contains(off, "\"enabled\":false")), "True")
    +clear : String <- D.Diagnostics.control("clear")
    +empty : String <- D.Diagnostics.export()
    C.Check.str("cleared", Bool.show(String.contains(empty, "\"events\":[]")), "True")
    IO.try(Unit, H.FS.remove(home ++ "/diagnostics/control"))
    IO.try(Unit, H.FS.remove(home ++ "/diagnostics"))
    +gone : Unit <- IO.try(Unit, H.FS.remove(home))
    return gone
