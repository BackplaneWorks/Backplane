import Base
import ../../src/server/host.bend as H

# The Windows harness compares this with the actual Win32 process identity.
def main() -> IO(Unit):
  do IO<Unit>:
    pid : U32 <- H.Sys.native_pid()
    IO.print("BACKPLANE_NATIVE_PID=" ++ U32.show(pid))
