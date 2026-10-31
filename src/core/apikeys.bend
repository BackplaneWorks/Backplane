import Base

# API keys the person keeps for agents (pure). Settings adds one (a name
# and its secret), lists the names and deletes one; no client ever gets a
# secret back. They live in one .env file in the home's secrets
# (server/apikeys.bend: <home>/secrets/api.env, 0600), a `NAME='value'` line
# each, so a shell can load it (`set -a; . FILE; set +a`). Agents are told
# the names and how to use a key without reading it (laws keys_*).

# a name as an environment variable: upper case, anything but a letter,
# digit or _ made _, never starting with a digit ("openai key" is
# OPENAI_KEY)
def Keys.up.c(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), (c - 32 : U32), c)

def Keys.word.c(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))), U32.is_eq(c, 95))

def Keys.name.c(+c: U32) -> Char:
  Chr{Bool.pick(U32, Keys.word.c(Keys.up.c(c)), Keys.up.c(c), 95)}

def Keys.name.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Keys.name.c(c), Keys.name.go(t)}

def Keys.digit(+s: String) -> Bool:
  Bool.or(Bool.or(Bool.or(String.starts_with(s, "0"), String.starts_with(s, "1")), Bool.or(String.starts_with(s, "2"), String.starts_with(s, "3"))),
    Bool.or(Bool.or(Bool.or(String.starts_with(s, "4"), String.starts_with(s, "5")), Bool.or(String.starts_with(s, "6"), String.starts_with(s, "7"))), Bool.or(String.starts_with(s, "8"), String.starts_with(s, "9"))))

def Keys.name.k(+n: String) -> String:
  Bool.pick(String, Keys.digit(n), "_" ++ n, n)

# (at most 64 characters; a name is a label, not a text)
def Keys.name(+raw: String) -> String:
  Keys.name.k(Keys.name.go(String.take(String.trim(raw), 64n)))

def Keys.quote.k(kind: Nat, +c: U32, rest: String) -> String:
  match kind:
    case 0n:
      "'\\''" ++ rest
    case 1n:
      rest
    case _:
      SCon{Chr{c}, rest}

def Keys.quote.c(+c: U32, rest: String) -> String:
  Keys.quote.k(Bool.pick(Nat, U32.is_eq(c, 39), 0n, Bool.pick(Nat, Bool.or(U32.is_eq(c, 10), U32.is_eq(c, 13)), 1n, 2n)), c, rest)

# a value in single quotes, each ' written '\'' (line breaks dropped: a key
# is one line)
def Keys.quote.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Keys.quote.c(c, Keys.quote.go(t))

def Keys.quote(+v: String) -> String:
  "'" ++ Keys.quote.go(String.trim(v)) ++ "'"

# a key's line in the file
def Keys.line(+name: String, +value: String) -> String:
  Keys.name(name) ++ "=" ++ Keys.quote(value)

# the name a line sets ("" for a blank line, a comment, or anything else)
def Keys.of.k(ps: List<&2, String>) -> String:
  match ps:
    case +n <> _ <> _:
      Bool.pick(String, Bool.and(Bool.not(String.is_empty(n)), String.eq(Keys.name(n), n)), n, "")
    case _:
      ""

def Keys.of(+l: String) -> String:
  Keys.of.k(String.split(String.trim(l), '='))

def Keys.drop.k(gone: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match gone:
    case True{}:
      rest
    case False{}:
      l <> rest

def Keys.names.k(+n: String, rest: List<&2, String>) -> List<&2, String>:
  Keys.drop.k(String.is_empty(n), n, rest)

def Keys.names.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Keys.names.k(Keys.of(l), Keys.names.go(t))

# the names the file sets, in its order
def Keys.names(+text: String) -> List<&2, String>:
  Keys.names.go(String.lines(text))

def Keys.drop.go(ls: List<&2, String>, +name: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Keys.drop.k(String.eq(Keys.of(l), name), l, Keys.drop.go(t, name))

def Keys.text(+ls: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, ls), "", String.join(ls, "\n") ++ "\n")

def Keys.keep(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Keys.drop.k(String.is_empty(String.trim(l)), l, Keys.keep(t))

# the file without the name's line (every other line kept as it was)
def Keys.drop(+text: String, +name: String) -> String:
  Keys.text(Keys.keep(Keys.drop.go(String.lines(text), name)))

# the file with the name set to value (an old line for it replaced)
def Keys.set(+text: String, +name: String, +value: String) -> String:
  Keys.text(List.append(&2, String, Keys.keep(Keys.drop.go(String.lines(text), Keys.name(name))), [Keys.line(name, value)]))

# why a new key cannot be kept ("" when it can)
def Keys.bad(+name: String, +value: String) -> String:
  Bool.pick(String, String.is_empty(Keys.name(name)), "Give the key a name",
    Bool.pick(String, Nat.is_gt(String.length(String.trim(name)), 64n), "Use at most 64 characters for the key name",
      Bool.pick(String, Bool.not(String.eq(name, Keys.name(name))), "Use the environment variable name: " ++ Keys.name(name),
        Bool.pick(String, String.is_empty(String.trim(value)), "Paste the key itself", ""))))

# What agents are told
# --------------------

def Keys.list(ns: List<&2, String>) -> String:
  match ns:
    case Nil{}:
      ""
    case +n <> +t:
      Bool.pick(String, List.is_empty(&2, String, t), "`" ++ n ++ "`", "`" ++ n ++ "`, " ++ Keys.list(t))

def Keys.prompt.k(none: Bool, +path: String, +ns: List<&2, String>) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      "<api_keys>The user keeps API keys for you in `" ++ path ++ "` (`NAME='value'` lines): " ++ Keys.list(ns) ++ ". "
      ++ "Use a key without ever seeing it: load the file in the same shell command that needs it, e.g. `set -a; . '" ++ path ++ "'; set +a; curl -H \"Authorization: Bearer $" ++ Maybe.default(&2, String, List.head(&2, String, ns), "NAME") ++ "\" ...`, or in a script read it from the environment that command gives it (`os.environ[\"NAME\"]`). "
      ++ "Never print, echo, cat, grep or log the file or a key's value, never write a value into a file, a commit, a message or a tool's arguments, and never pass it on the command line where it would show. If a command's output could show a key, cut it off or redact it before it reaches you. "
      ++ "To see which names exist now (the user may add some while you work): `sed -n 's/=.*//p' '" ++ path ++ "'`. When a task needs a key that is not there, ask the user to add it in Backplane's Settings, API keys, and say which name to give it.</api_keys>"

# the block agents get from the names kept (a name a line; nothing when
# there are none): the names only, never the file
def Keys.prompt(+path: String, +names: String) -> String:
  Keys.prompt.k(List.is_empty(&2, String, Keys.keep(String.lines(names))), path, Keys.keep(String.lines(names)))

# Only names cross into the browser; a replacement asks twice before a save.
def Keys.has(ns: List<&2, String>, +name: String) -> Bool:
  match ns:
    case Nil{}: False{}
    case +n <> t: Bool.or(String.eq(n, name), Keys.has(t, name))
