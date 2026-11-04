import Base

def main() -> IO(Unit):
  do IO<Unit>:
    t0 : Nat <- IO.now()
    IO.sleep(100)
    t1 : Nat <- IO.now()
    IO.print(Nat.show(Nat.sub(t1, t0)))
