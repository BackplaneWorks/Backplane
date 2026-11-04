import Base
import ../core/json.bend as J
import ../core/qr.bend as QR
import ../core/json-object.bend as JSONOBJECT

# Pure pairing info policy shared by production and its laws.

def Info.qr.k(none: Bool, +lk: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      QR.Qr.text(QR.Qr.link(lk))

def Info.qr(+lk: String) -> String:
  Info.qr.k(String.is_empty(lk), lk)

def Hub.pair_msg.of(+lk: String) -> String:
  "{\"t\":\"info\",\"info\":{\"pair\":\"\",\"pair.qr\":\"\"}}"

def Hub.pair_msg.k(hit: Bool, +kept: String, +lk: String) -> String:
  match hit:
    case True{}:
      String.drop(kept, 1n+String.length(lk))
    case False{}:
      Hub.pair_msg.of(lk)

def Hub.pair_msg.from(+kept: String, +lk: String) -> String:
  Hub.pair_msg.k(String.starts_with(kept, lk ++ "\n"), kept, lk)

def Hub.thin.pair.put(+kv: J.Json, +info: J.Json) -> J.Json:
  JSONOBJECT.Obj.put(JSONOBJECT.Obj.put(info, "pair", J.Str{J.Json.get_str(kv, "pair", "")}), "pair.qr", J.Str{J.Json.get_str(kv, "pair.qr", "")})

def Hub.thin.pair.k(+msg: String, +info: J.Json) -> J.Json:
  Hub.thin.pair.put(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(msg), J.Null{}), "info"), info)
