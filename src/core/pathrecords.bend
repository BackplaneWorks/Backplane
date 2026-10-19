import Base

# POSIX paths cannot contain NUL. All other characters, including newlines
# and whitespace-only names, are part of a path rather than record framing.
def Paths.keep(cur: String, acc: List<&2, String>) -> List<&2, String>:
  match cur:
    case SNil{}: acc
    case SCon{c, t}: String.reverse(SCon{c, t}) <> acc

def Paths.read.go(text: String, cur: String, acc: List<&2, String>) -> List<&2, String>:
  match text:
    case SNil{}:
      List.reverse(&2, String, Paths.keep(cur, acc))
    case SCon{Chr{0}, rest}:
      Paths.read.go(rest, SNil{}, Paths.keep(cur, acc))
    case SCon{c, rest}:
      Paths.read.go(rest, SCon{c, cur}, acc)

def Paths.read(text: String) -> List<&2, String>:
  Paths.read.go(text, SNil{}, Nil{})

def Paths.pack(paths: List<&2, String>) -> String:
  String.join(paths, "\0")

# Existing kept client states contain newline-delimited file lists.
def Paths.kept(nul: Bool, text: String) -> List<&2, String>:
  match nul:
    case True{}: Paths.read(text)
    case False{}: String.lines(text)
