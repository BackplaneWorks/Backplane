import Base

# Why an agent process ended (pure)
# =================================
# Each agent process's stderr goes to a file of its own (the server's
# Proc.spawn with err 3); when the process ends the hub reads its last
# Err.most() bytes (Proc.errtail), drops anything that looks like a secret
# (Err.scrub: a token=/key=/secret=/password= value, a Bearer token, a
# key-shaped word such as sk-..., ghp_..., AKIA...), and words the failure
# with the exit code or signal and that tail (Exit.say), so a turn that
# fails says why (laws errtail_*).

def Err.most() -> Nat:
  2048n

# a character a secret is made of
def Err.secret.c(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 45)), Bool.or(U32.is_eq(c, 46), Bool.or(U32.is_eq(c, 43), U32.is_eq(c, 47))))))

# a letter or digit (a word goes on)
def Err.word.c(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)))

# whether s starts with a secret's character
def Err.first(s: String) -> Bool:
  match s:
    case SCon{Chr{+c}, _}:
      Err.secret.c(c)
    case _:
      False{}

# s past the secret's characters (more: s starts with one)
def Err.skip.go(s: String, more: Bool) -> String:
  match s more:
    case SCon{_, +t} True{}:
      Err.skip.go(t, Err.first(t))
    case _ _:
      s

def Err.skip(+s: String) -> String:
  Err.skip.go(s, Err.first(s))

# the keys whose value goes wherever they stand, and the words that are a
# secret by their start (only at a word's start)
def Err.keys() -> List<&2, String>:
  ["token=", "key=", "secret=", "password=", "Bearer "]

def Err.starts() -> List<&2, String>:
  ["sk-", "ghp_", "gho_", "ghs_", "ghu_", "github_pat_", "xoxb-", "xoxp-", "AKIA", "AIza"]

def Err.any(ks: List<&2, String>, +s: String) -> String:
  match ks:
    case Nil{}:
      ""
    case +k <> t:
      Bool.pick(String, String.starts_with(s, k), k, Err.any(t, s))

def Err.hit.k(+k: String, +s: String, +edge: Bool) -> String:
  Bool.pick(String, Bool.not(String.is_empty(k)), k, Bool.pick(String, edge, Err.any(Err.starts(), s), ""))

# the key s starts with, here ("" when none): a start word counts only at
# a word's start
def Err.hit(+s: String, +edge: Bool) -> String:
  Err.hit.k(Err.any(Err.keys(), s), s, edge)

# the walk's state: what is left, whether a word may start here, and what
# is done so far (reversed)
type ES is Data:
  ES{rest: String, edge: Bool, acc: String}

def Err.step.h(none: Bool, +k: String, +c: U32, t: String, acc: String) -> ES:
  match none:
    case True{}:
      ES{t, Bool.not(Err.word.c(c)), SCon{Chr{c}, acc}}
    case False{}:
      ES{Err.skip(String.drop(SCon{Chr{c}, t}, String.length(k))), False{}, String.reverse(k ++ "…") ++ acc}

def Err.step.k(+k: String, +c: U32, t: String, acc: String) -> ES:
  Err.step.h(String.is_empty(k), k, c, t, acc)

def Err.step(st: ES) -> ES:
  match st:
    case ES{SCon{Chr{+c}, +t}, +edge, acc}:
      Err.step.k(Err.hit(SCon{Chr{c}, t}, edge), c, t, acc)
    case _:
      st

def Err.go(fuel: Nat, st: ES) -> String:
  match fuel st:
    case _ ES{SNil{}, _, acc}:
      String.reverse(acc)
    case 0n ES{_, _, acc}:
      String.reverse(acc)
    case 1n+f _:
      Err.go(f, Err.step(st))

# the text with its secrets dropped
def Err.scrub(+s: String) -> String:
  Err.go(1n+String.length(s), ES{s, True{}, ""})

# Wording an exit
# ---------------

def Exit.code.say(+code: U32) -> String:
  Bool.pick(String, U32.is_ge(code, 128), "signal " ++ U32.show((code - 128 : U32)), "exit " ++ U32.show(code))

# the words for an agent that ended before its turn did: what is known of
# the exit (Exit.why's words, else dflt), its exit code or signal, and the
# tail of what it wrote to stderr, secrets dropped
def Exit.say(+dflt: String, +why: String, +code: U32, +tail: String) -> String:
  Bool.pick(String, String.is_empty(why), dflt, why) ++ " (" ++ Exit.code.say(code) ++ ")" ++
    Bool.pick(String, String.is_empty(String.trim(tail)), "", ": " ++ String.trim(Err.scrub(tail)))
