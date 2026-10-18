import Base
import ../../src/server/proc.bend as P

# Real installed Windows CLIs, through the product's native spawn/stdio path.
# Version and discovery requests do not authenticate or call a model.
def Report(name: String, r: P.Ran) -> String:
  match r:
    case P.Ran{code, out}:
      name ++ " code=" ++ U32.show(code) ++ " out=" ++ out

def main() -> IO(Unit):
  do IO<Unit>:
    a : P.Ran <- P.Proc.run("sh", ["-c", "command -v claude"], "/tmp")
    IO.print(Report("CLAUDE_DISCOVERY", a))
    b : P.Ran <- P.Proc.run("claude", ["--version"], "/tmp")
    IO.print(Report("CLAUDE_VERSION", b))
    c : P.Ran <- P.Proc.run("sh", ["-c", "command -v codex"], "/tmp")
    IO.print(Report("CODEX_DISCOVERY", c))
    d : P.Ran <- P.Proc.run("codex", ["--version"], "/tmp")
    IO.print(Report("CODEX_VERSION", d))
