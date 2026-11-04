import Base
import ../core/json.bend as J

# Pure desktop info policy shared by production and its laws.

def Desktop.connection_info(kind: String, text: String, version: String, hub: String) -> String:
  match kind:
    case "connected": J.Json.show(J.Json.obj([J.KV{"version", J.Str{version}}, J.KV{"hub", J.Str{hub}}]))
    case _: text
