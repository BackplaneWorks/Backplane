import Base
import ../../src/core/port.bend as PT

# Emit the actual production wrapper for the shell integration regression.
def main() -> IO(Unit):
  IO.print(PT.Port.tmo())
