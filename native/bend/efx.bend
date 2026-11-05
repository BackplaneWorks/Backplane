import Base
import ../../src/server/db.bend as D

# What does a C emit do with an effect that has only a .js twin (Db.open)?
def main() -> IO(Unit):
  do IO<Unit>:
    r : String <- D.Db.open("/tmp/efx.db")
    IO.print(r)
