import Base
import ./check.bend as C
import ../src/core/apikeys.bend as K

# API keys' file and what agents are told (core/apikeys.bend). Prints the
# line kept for a hard value last, for a shell to load (see the check in
# the PR: `set -a; . FILE` gives the value back unchanged).

def main() -> IO(Unit):
  do IO<Unit>:
    +f : String = K.Keys.set(K.Keys.set("", "openai api key", "sk-1"), "Stripe-Secret", "rk_2")
    C.Check.str("names, as environment variables", String.join(K.Keys.names(f), ","), "OPENAI_API_KEY,STRIPE_SECRET")
    C.Check.str("set again replaces", String.join(K.Keys.names(K.Keys.set(f, "OPENAI_API_KEY", "sk-3")), ","), "STRIPE_SECRET,OPENAI_API_KEY")
    C.Check.str("the new value is kept", Bool.show(String.contains(K.Keys.set(f, "OPENAI_API_KEY", "sk-3"), "OPENAI_API_KEY='sk-3'")), "True")
    C.Check.str("delete", K.Keys.drop(f, "OPENAI_API_KEY"), "STRIPE_SECRET='rk_2'\n")
    C.Check.str("delete the last", K.Keys.drop("A='1'\n", "A"), "")
    C.Check.str("a value with = keeps its name", String.join(K.Keys.names("TOKEN='a=b=='\n"), ","), "TOKEN")
    C.Check.str("a bad key", K.Keys.bad("  ", "x"), "Give the key a name")
    C.Check.str("names are not silently normalized", K.Keys.bad(" TOKEN ", "x"), "Use the environment variable name: TOKEN")
    C.Check.str("no secret", K.Keys.bad("A", " "), "Paste the key itself")
    +p : String = K.Keys.prompt("/h/secrets/api.env", "OPENAI_API_KEY\nSTRIPE_SECRET")
    C.Check.str("agents get the names", Bool.show(Bool.and(String.contains(p, "`OPENAI_API_KEY`, `STRIPE_SECRET`"), String.contains(p, "/h/secrets/api.env"))), "True")
    C.Check.str("and how to use one unseen", Bool.show(Bool.and(String.contains(p, "set -a; . '/h/secrets/api.env'; set +a"), String.contains(p, "Never print"))), "True")
    C.Check.str("no keys, nothing", K.Keys.prompt("/h/secrets/api.env", ""), "")
    IO.print(K.Keys.line("hard", " it's $HOME \\ \"q\" `x` "))
