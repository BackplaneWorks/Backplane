import Base
import ../src/server/git.bend as GT

# Prints the script a version's repository is proved with (git.bend's
# Git.ident.sh), so test/tools/ident_fs.sh can run the very text the hub
# runs against real temporary repositories.

def main() -> IO(Unit):
  IO.print(GT.Git.ident.sh())
