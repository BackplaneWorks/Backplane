import Base
import ./json.bend as J

# First-run setup only observes provider metadata. Installed programs and model
# lists are not proof of authentication. No credential or session is held here.
def Ps.providers() -> List<&2, String>:
  ["codex", "claude", "antigravity", "grok", "muse", "opencode", "pi", "omp"]

def Ps.label(p: String) -> String:
  match p:
    case "codex": "Codex · ChatGPT"
    case "claude": "Claude Code · Claude.ai"
    case "antigravity": "Antigravity · Google"
    case "grok": "Grok Build"
    case "muse": "Muse Code"
    case "opencode": "OpenCode"
    case "pi": "Pi"
    case "omp": "Oh My Pi"
    case _: "Choose an agent"

def Ps.docs(p: String) -> String:
  match p:
    case "codex": "https://developers.openai.com/codex/cli"
    case "claude": "https://code.claude.com/docs/en/setup"
    case "grok": "https://x.ai/cli"
    case "muse": "https://dev.meta.ai/docs/muse-code"
    case "opencode": "https://opencode.ai/docs/"
    case "pi": "https://pi.dev"
    case "omp": "https://github.com/can1357/oh-my-pi"
    case _: ""

def Ps.login(p: String) -> String:
  match p:
    case "codex": "For an existing CLI account, run codex login on the hub machine. For a new subscription account, use the sign-in below."
    case "claude": "For an existing CLI account, run claude auth login on the hub machine. The sign-in below supports Claude.ai subscriptions; Console and API accounts use the CLI instead."
    case "grok": "On the hub machine, run grok login and follow its instructions."
    case "muse": "On the hub machine, run muse and follow its sign-in prompts. In an interactive session, /login reopens sign-in."
    case "opencode": "On the hub machine, run opencode auth login and choose your provider."
    case "pi": "On the hub machine, run pi, then enter /login and follow its instructions."
    case "omp": "On the hub machine, run omp, then enter /login and follow its instructions."
    case _: "Open Settings → Antigravity for its supported installation and sign-in options."

def Ps.connected(+p: J.Json) -> Bool:
  String.eq(J.Json.get_str(p, "state", ""), "connected")

# A browser ceremony may finish after its CLI was removed. Both are needed.
def Ps.ready(installed: Bool, profile: J.Json) -> Bool:
  Bool.and(installed, Ps.connected(profile))

def Ps.busy(+state: String) -> Bool:
  String.eq(state, "starting") || String.eq(state, "waiting") || String.eq(state, "verifying")

def Ps.progress(state: String) -> String:
  match state:
    case "starting": "Starting sign-in…"
    case "waiting": "Waiting for you to finish with the provider"
    case "verifying": "Checking your connection…"
    case "done": "Sign-in finished"
    case "failed": "Sign-in did not finish"
    case "expired": "Sign-in expired"
    case "cancelled": "Sign-in cancelled"
    case _: "Sign in to continue"
