import Base
import ./check.bend as C
import ../src/core/mech.bend as MC

# Render subprocess diagnostics: success paths can precede a failure,
# and a failed or interrupted child can have no output at all.
def main() -> IO(Unit):
  +why = {"backplane --render: could not write /p/renders/fixture.front.png" : String}
  do IO<Unit>:
    C.Check.str("successful paths are not an error", MC.Mech.error(0, "/p/renders/fixture.iso.png\n"), "")
    C.Check.str("successful exit controls the result", MC.Mech.error(0, why), "")
    C.Check.str("silent failure is visible", MC.Mech.error(1, ""), "backplane --render failed (exit 1)")
    C.Check.str("whitespace-only failure is visible", MC.Mech.error(2, " \n\t"), "backplane --render failed (exit 2)")
    C.Check.str("interrupted child is visible", MC.Mech.error(143, ""), "backplane --render failed (exit 143)")
    C.Check.str("diagnostic survives earlier successful view", MC.Mech.error(1, "/p/renders/fixture.iso.png\n" ++ why ++ "\n"), why)
    C.Check.str("diagnostic survives long successful paths", MC.Mech.error(1, String.repeat("x", 500n) ++ ".iso.png\n" ++ why ++ "\n"), why)
    C.Check.str("first diagnostic wins over final summary", MC.Mech.error(1, why ++ "\nbackplane --render: one or more models could not be rendered\n"), why)
    C.Check.str("diagnostic whitespace is trimmed", MC.Mech.error(1, "done.png\r\n  " ++ why ++ "\r\nsummary\n"), why)
    C.Check.str("unrecognized child error is preserved", MC.Mech.error(127, " executable not found\n"), "executable not found")
    C.Check.str("diagnostic length is bounded", Nat.show(String.length(MC.Mech.error(1, "backplane --render: " ++ String.repeat("x", 400n)))), "300")
    C.Check.str("fallback length is bounded", Nat.show(String.length(MC.Mech.error(1, String.repeat("x", 400n)))), "300")
