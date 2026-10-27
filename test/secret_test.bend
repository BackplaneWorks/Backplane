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

def handle(n: String) -> String:
  J.Json.show(S.Sec.handle(n, True{}))

def prompt(metas: String) -> String:
  S.Sec.prompt(S.Sec.metas(metas))

def main() -> IO(Unit):
  IO.print(mask("SECRET-FIXTURE-1", "the value SECRET-FIXTURE-1 here"))
