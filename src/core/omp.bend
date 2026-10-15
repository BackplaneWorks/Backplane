import Base
import ./model.bend as M
import ./json.bend as J

# oh-my-pi (omp, core/omp.bend): the approval modes it takes on its ACP
# command line, the thinking levels it offers, the chat models it lists
# from `omp models --json`, its install line and its auth hint. One process
# per turn: `omp acp --approval-mode <mode>`. Pure: no IO here.

# the --approval-mode argument for a runtime mode
def Omp.approval(r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}:
      "always-ask"
    case M.AcceptEdits{}:
      "write"
    case M.AutoMode{}:
      "write"
    case M.FullAccess{}:
      "yolo"

# the argument vector after the `omp` command
def Omp.args(r: M.RuntimeMode) -> List<&2, String>:
  ["acp", "--approval-mode", Omp.approval(r)]

# the thinking levels omp accepts
def Omp.levels() -> List<&2, String>:
  ["off", "auto", "low", "medium", "high", "xhigh", "max"]

# one entry's selector ahead of the rest, when it is a chat model
def Omp.chat.one(on: Bool, +m: J.Json, +rest: List<&2, String>) -> List<&2, String>:
  match on:
    case True{}:
      J.Json.get_str(m, "selector", "") <> rest
    case False{}:
      rest

def Omp.chat.rows(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +m <> t:
      Omp.chat.one(String.eq(J.Json.get_str(m, "kind", ""), "chat"), m, Omp.chat.rows(t))

# the selectors of the chat models in `omp models --json` ("" when the text
# is not JSON or has no models)
def Omp.models(+text: String) -> List<&2, String>:
  Omp.chat.rows(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "models")))

def Omp.install() -> String:
  "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`."

# an auth-looking failure gets the sign-in hint, as Acp.hint does for Grok
def Omp.hint(+msg: String) -> String:
  Bool.pick(String, Bool.or(String.contains(String.to_lower(msg), "auth"), String.contains(String.to_lower(msg), "login")),
    msg ++ " (sign in once with `omp login`)", msg)
