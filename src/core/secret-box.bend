import Base

# Secret-box action ownership. The secure host owns Save; every other slot
# is a state action dispatched by client.bend's Sbox.act.k. A value never
# belongs to a state action, including an unknown one.
def Sbox.actions() -> List<&2, String>:
  ["secret-open", "secret-open-here", "secret-shut", "secret-sent", "secret-err", "secret-del", "secret-dismiss", "secret-list", "secret-open-set", "secret-save"]

def Sbox.route(action: String) -> Nat:
  match action:
    case "secret-open": 0n
    case "secret-open-here": 1n
    case "secret-shut": 2n
    case "secret-sent": 3n
    case "secret-err": 4n
    case "secret-del": 5n
    case "secret-dismiss": 6n
    case "secret-list": 7n
    case "secret-open-set": 8n
    case "secret-save": 9n
    case _: 10n

def Sbox.routes(xs: List<&2, String>) -> List<&2, Nat>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: Sbox.route(x) <> Sbox.routes(t)
