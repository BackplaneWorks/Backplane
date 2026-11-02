import Base
import ./check.bend as C
import ../src/core/release.bend as R

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("successful export is quiet", R.Release.command_error("Gerbers", 0, "tool output"), "")
    C.Check.str("failed export names its status", R.Release.command_error("Gerbers", 2, ""), "Gerbers failed (exit 2)")
    C.Check.str("failed export keeps diagnostic", R.Release.command_error("Drill", 1, " missing source \n"), "Drill failed (exit 1): missing source")
