import Base
import ./refs.bend as RF

# Slash commands in the composer
# ------------------------------
# A draft that is one word starting with `/` is a command being typed; the
# composer offers the commands whose name starts with it. Backplane's own
# come first (any provider), then Claude's own (only in a Claude thread).

type Cmd is Data:
  Cmd{name: String, about: String}

# the commands every thread has
def Slash.own() -> List<&2, Cmd>:
  [Cmd{"/btw", "Ask a side question; the thread never sees it"}, Cmd{"/give", "Hand this conversation to a >thread, %project or @bot"},
   Cmd{"/secret", "Give an agent a key or token in a masked box, never in the chat"}]

# the commands only a Claude thread has
def Slash.claude() -> List<&2, Cmd>:
  [Cmd{"/compact", "Summarize the conversation to free context"}, Cmd{"/context", "Show what fills the context window"}]

def Slash.table.k(claude: Bool) -> List<&2, Cmd>:
  match claude:
    case True{}:
      List.append(&2, Cmd, Slash.own(), Slash.claude())
    case False{}:
      Slash.own()

# whether the draft is a slash command being typed: one word, no space yet
# (the "/" first: Bool.and is strict, and the token of a long draft is a
# walk of all of it)
def Slash.typing.k(slash: Bool, +draft: String) -> Bool:
  match slash:
    case True{}:
      String.eq(RF.Refs.token(draft), draft)
    case False{}:
      False{}

def Slash.typing(+draft: String) -> Bool:
  Slash.typing.k(String.starts_with(draft, "/"), draft)

def Slash.has(c: Cmd, +p: String) -> Bool:
  match c:
    case Cmd{n, _}:
      String.starts_with(n, p)

def Slash.keep(keep: Bool, +c: Cmd, rest: List<&2, Cmd>) -> List<&2, Cmd>:
  match keep:
    case True{}:
      c <> rest
    case False{}:
      rest

def Slash.pick(cs: List<&2, Cmd>, +p: String) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      Slash.keep(Slash.has(c, p), c, Slash.pick(t, p))

# the commands whose name starts with "/" and q, in table order
def Slash.cmds(claude: Bool, +q: String) -> List<&2, Cmd>:
  Slash.pick(Slash.table.k(claude), "/" ++ q)

def Slash.names(cs: List<&2, Cmd>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{n, _} <> t:
      n <> Slash.names(t)
