import Base
import ../src/core/json.bend as J
import ../src/core/secret.bend as S
import ../src/core/client.bend as U

# The secret box's pure defs in the emitted JavaScript runtime (driver:
# test/tools/secret.mjs, which also gates where SecVal and Vault may be
# built and runs the vault effect against a temporary home).

def vault(vals: String) -> S.Vault:
  S.Sec.vault(String.split(vals, '\n'))

def mask(vals: String, s: String) -> String:
  S.Sec.mask(vault(vals), s)

def clean(vals: String, s: String) -> Bool:
  S.Sec.clean(vault(vals), s)

def fine(vals: String, s: String) -> String:
  S.Sec.mask(vault(vals), s)

def json(vals: String, text: String) -> String:
  J.Json.show(S.Sec.json(vault(vals), Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))

def json_clean(+vals: String, text: String) -> Bool:
  S.Sec.json.clean(vault(vals), S.Sec.json(vault(vals), Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))

def name(raw: String) -> String:
  S.Sec.name(raw)

def bad(n: String, v: String) -> String:
  S.Sec.bad(n, v)

def req(packed: String) -> String:
  U.Sbox.req(packed)

# Settings' API key Save on the web, built the same way (client.bend's
# ApiKeys.req): the value only in the request, never in a view action
def key_req(packed: String) -> String:
  U.ApiKeys.req(packed)

# every sealed form's Save on the web (client.bend's Sealed.req): the
# value only in the request, never in a view action
def sealed_req(packed: String) -> String:
  U.Sealed.req(packed)

def show.cmds(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> rest:
      t ++ ";" ++ show.cmds(rest)
    case _ <> rest:
      show.cmds(rest)

# what the hub's answer to a sealed Save leaves (Sealed.after, a phone's
# "key-sent"/"key-err" too): the form's note ("" for none), then the
# requests it sends ("<why>|<requests>")
def sealed_after.k(a: U.Act, form: String) -> String:
  match a:
    case U.Act{+u, cs}:
      U.Sealed.err(u, form) ++ "|" ++ show.cmds(cs)

def sealed_after(action: String, value: String, form: String) -> String:
  sealed_after.k(U.Ui.act(U.Ui.init(0n), action, value), form)

def handle(n: String) -> String:
  J.Json.show(S.Sec.handle(n, True{}))

def prompt(metas: String) -> String:
  S.Sec.prompt(S.Sec.metas(metas))

def main() -> IO(Unit):
  IO.print(mask("SECRET-FIXTURE-1", "the value SECRET-FIXTURE-1 here"))
