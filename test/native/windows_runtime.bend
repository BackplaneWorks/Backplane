import Base

# Runs before the platform-window test: exercises the emitted C runtime and
# IO on Windows without any Backplane window effects.
def main() -> IO(Unit):
  do IO<Unit>:
    IO.print("WINDOWS_RUNTIME_OK 日本😀")
    a : U32 <- IO.try(U32, IO.random_u32())
    b : U32 <- IO.try(U32, IO.random_u32())
    IO.print("WINDOWS_RANDOM_OK " ++ U32.show(a) ++ " " ++ U32.show(b))
