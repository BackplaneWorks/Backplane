import Base
import ./json.bend as J
import ./acp.bend as AC

# Google Antigravity (pure). Backplane runs Google's own agent,
# agy_acp_server, from the ACP registry (agentclientprotocol/registry,
# antigravity-acp), pinned by version, size and SHA-256 and kept under the
# hub's home (<home>/antigravity/<version>), and speaks ACP to it like Grok
# (acp.bend), one process per turn. The agent signs in by itself: its own
# methods (a Gemini API key, Vertex AI, Gemini Enterprise, a Google
# account) and its own tokens, kept in a profile folder the hub gives it
# (GEMINI_HOME = <home>/antigravity/profiles/<account>). Backplane never
# reads a Google token; an API key stays in the API keys file
# (apikeys.bend) and reaches only the agent's environment. Google's terms
# restrict third-party clients, so a Google account carries a warning and
# an API key is the default (Agy.warn, laws agy_*).

# The release
# -----------

def Agy.version() -> String:
  "1.3.0"

# one platform's archive: its uname (`uname -s`-`uname -m`), URL, size in
# bytes and SHA-256
type AgyRel is Data:
  AgyRel{uname: String, url: String, size: String, sha: String}

def Agy.rels() -> List<&2, AgyRel>:
  [AgyRel{"Linux-x86_64", "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.3.0-linux-x86_64.zip", "333727150", "9fb60956af0a9d76220a4db91ca9ac88e2a2372ad68f985ab5fceace6b825b96"},
   AgyRel{"Linux-aarch64", "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.3.0-linux-arm64.zip", "321690363", "500b0bc0fb858e88f4df404d4cedf80bf9298c178291e39e383d6c50b111cbdf"},
   AgyRel{"Linux-arm64", "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.3.0-linux-arm64.zip", "321690363", "500b0bc0fb858e88f4df404d4cedf80bf9298c178291e39e383d6c50b111cbdf"},
   AgyRel{"Darwin-arm64", "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.3.0-darwin-arm64.zip", "111456962", "7cd97045f7b4fe81175a107cdf16f9c51484e3c78a5162cae415338bb6aa5b88"},
   AgyRel{"Darwin-x86_64", "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.3.0-darwin-x86_64.zip", "117245544", "bb23956b89984bf5d354af2c3725e6c57f0cc1b7228e77a0e91c9c2bc1d47646"}]

# a sh `case` arm for one archive
def Agy.arm(r: AgyRel) -> String:
  match r:
    case AgyRel{n, u, s, h}:
      "  " ++ n ++ ") u='" ++ u ++ "' n=" ++ s ++ " s=" ++ h ++ ";;\n"

def Agy.arms(rs: List<&2, AgyRel>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      Agy.arm(r) ++ Agy.arms(t)

# the folder the agent lives in, under the antigravity folder
def Agy.dir(+root: String) -> String:
  root ++ "/" ++ Agy.version()

# Installing: `sh -c SCRIPT sh ROOT [URL SIZE SHA]` downloads this
# platform's archive (or the one given, for tests), checks its size and
# hash before anything is unpacked, unpacks it beside the version's
# folder and moves it into place only whole, then removes older versions.
# It prints "ok" last, or a line saying why not.
def Agy.install.sh() -> String:
  "d=$1; v=" ++ Agy.version() ++ "\n"
    ++ "fail() { echo \"$*\"; rm -rf \"$d/dl\" \"$d/$v.new\"; exit 1; }\n"
    ++ "if [ -n \"$2\" ]; then u=$2 n=$3 s=$4; else\n"
    ++ "case \"$(uname -s)-$(uname -m)\" in\n" ++ Agy.arms(Agy.rels())
    ++ "  *) fail \"Antigravity has no build for $(uname -s) $(uname -m)\";;\n"
    ++ "esac; fi\n"
    ++ "mkdir -p \"$d/dl\" || fail \"cannot write $d\"\n"
    ++ "z=\"$d/dl/agy.zip\"\n"
    ++ "curl -fsSL --retry 3 --connect-timeout 20 -o \"$z\" \"$u\" || fail \"the download failed\"\n"
    ++ "[ \"$(wc -c < \"$z\" | tr -d ' ')\" = \"$n\" ] || fail \"the download is not the size Backplane pins\"\n"
    ++ "if command -v sha256sum >/dev/null 2>&1; then h=$(sha256sum \"$z\" | cut -d' ' -f1); else h=$(shasum -a 256 \"$z\" | cut -d' ' -f1); fi\n"
    ++ "[ \"$h\" = \"$s\" ] || fail \"the download's SHA-256 is not the one Backplane pins\"\n"
    ++ "rm -rf \"$d/$v.new\"; mkdir -p \"$d/$v.new\"\n"
    ++ "if command -v unzip >/dev/null 2>&1; then unzip -q -o \"$z\" -d \"$d/$v.new\"; elif command -v bsdtar >/dev/null 2>&1; then bsdtar -xf \"$z\" -C \"$d/$v.new\"; else python3 -m zipfile -e \"$z\" \"$d/$v.new\"; fi || fail \"the archive did not unpack\"\n"
    ++ "chmod 755 \"$d/$v.new/agy_acp_server.par\" \"$d/$v.new/localharness_external\" || fail \"the archive has no agy_acp_server.par\"\n"
    ++ "rm -rf \"$d/$v\" && mv \"$d/$v.new\" \"$d/$v\" && touch \"$d/$v/.ok\" || fail \"cannot move the agent into place\"\n"
    ++ "rm -rf \"$d/dl\"\n"
    ++ "for o in \"$d\"/*/.ok; do o=${o%/.ok}; [ -f \"$o/.ok\" ] && [ \"$o\" != \"$d/$v\" ] && rm -rf \"$o\"; done\n"
    ++ "echo ok\n"

def Agy.last(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      Agy.last(t, Bool.pick(String, String.is_empty(String.trim(l)), acc, l))

def Agy.or(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

# the install's answer: "" when done, else why not (its last line)
def Agy.install.err(+out: String) -> String:
  Bool.pick(String, String.eq(String.trim(Agy.last(String.lines(out), "")), "ok"), "",
    Agy.or(String.trim(Agy.last(String.lines(out), "")), "the install failed"))

# The runner
# ----------
# <root>/agy.sh, written by the hub before each start: `agy.sh DIR PROFILE
# METHOD KEYS SETTINGS` runs the agent with its profile as GEMINI_HOME, the
# method's settings.json, only the method's own key from the API keys file
# (none from the hub's environment), a temporary folder of its own (removed
# when it ends), and BROWSER pointing back at itself: `agy.sh --open URL`
# prints the sign-in address as one line, BACKPLANE-SIGNIN <url>, so a
# browser never opens on the hub's machine by itself. A missing agent says
# BACKPLANE-MISSING. Its stderr goes to <profile>/agy.log. (The agent runs
# in the background so a stop reaches it; its stdin is passed through fd 3:
# dash gives a background command /dev/null even with <&0.)
def Agy.run.sh() -> String:
  "#!/bin/sh\n"
    ++ "# Backplane's runner for Google's Antigravity agent (src/core/agy.bend)\n"
    ++ "if [ \"$1\" = --open ]; then printf 'BACKPLANE-SIGNIN %s\\n' \"$2\"; exit 0; fi\n"
    ++ "d=$1 p=$2 m=$3 k=$4\n"
    ++ "if [ ! -x \"$d/agy_acp_server.par\" ]; then echo BACKPLANE-MISSING; exit 1; fi\n"
    ++ "umask 077\n"
    ++ "mkdir -p \"$p/antigravity-acp\" && printf '%s\\n' \"$5\" > \"$p/antigravity-acp/settings.json\" || exit 1\n"
    ++ "t=\"$p/tmp/$$\"; mkdir -p \"$t\"\n"
    ++ "case \"$m\" in gemini-api-key) n=GEMINI_API_KEY;; agent-platform) n=GOOGLE_API_KEY;; *) n=;; esac\n"
    ++ "key=; if [ -n \"$n\" ] && [ -f \"$k\" ]; then key=$(unset \"$n\"; . \"$k\" >/dev/null 2>&1; eval \"printf %s \\\"\\${$n:-}\\\"\"); fi\n"
    ++ "unset GEMINI_API_KEY GOOGLE_API_KEY GOOGLE_CLOUD_PROJECT GOOGLE_CLOUD_LOCATION GOOGLE_GENAI_USE_VERTEXAI AGY_ACP_ENABLE_OAUTH AGY_ACP_CCPA_PROJECT ELECTRON_RUN_AS_NODE\n"
    ++ "[ -n \"$key\" ] && export \"$n=$key\"\n"
    ++ "export GEMINI_HOME=\"$p\" AGY_ACP_FORCE_FILE_STORAGE=1 PYTHONUNBUFFERED=1 TMPDIR=\"$t\" ANTIGRAVITY_HARNESS_PATH=\"$d/localharness_external\" BROWSER=\"'$0' --open %s\"\n"
    ++ "set -- \"$d/agy_acp_server.par\"; [ \"$(uname -s)\" = Linux ] && set -- \"$@\" --uid=\n"
    ++ "exec 3<&0\n"
    ++ "\"$@\" <&3 3<&- 2>\"$p/agy.log\" &\n"
    ++ "c=$!\n"
    ++ "trap 'kill $c 2>/dev/null' TERM INT HUP\n"
    ++ "wait $c; r=$?\n"
    ++ "while kill -0 $c 2>/dev/null; do wait $c; r=$?; done\n"
    ++ "rm -rf \"$t\"\n"
    ++ "exit $r\n"

# Methods and accounts
# --------------------
# The setting agy.auth names the method (the agent's own ids); unset is
# the API key. agy.account names the profile a Google sign-in keeps.

def Agy.methods() -> List<&2, String>:
  ["gemini-api-key", "agent-platform", "oauth-business", "oauth-personal"]

def Agy.method.k(i: Nat) -> String:
  match i:
    case 1n:
      "agent-platform"
    case 2n:
      "oauth-business"
    case 3n:
      "oauth-personal"
    case _:
      "gemini-api-key"

def Agy.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Agy.pos(t, x, 1n+i))

# a setting's method, the API key for anything not known
def Agy.method(+m: String) -> String:
  Agy.method.k(Agy.pos(Agy.methods(), m, 0n))

def Agy.label.k(i: Nat) -> String:
  match i:
    case 1n:
      "Vertex AI"
    case 2n:
      "Gemini Enterprise"
    case 3n:
      "Google account"
    case _:
      "API key"

def Agy.label(+m: String) -> String:
  Agy.label.k(Agy.pos(Agy.methods(), Agy.method(m), 0n))

# the methods that open Google's sign-in page
def Agy.browser(+m: String) -> Bool:
  String.starts_with(Agy.method(m), "oauth-")

# the API key a method reads (from the API keys file), "" for none
def Agy.key.k(i: Nat) -> String:
  match i:
    case 0n:
      "GEMINI_API_KEY"
    case 1n:
      "GOOGLE_API_KEY"
    case _:
      ""

def Agy.key(+m: String) -> String:
  Agy.key.k(Agy.pos(Agy.methods(), Agy.method(m), 0n))

# the methods that need a Google Cloud project and location
def Agy.gcp(+m: String) -> Bool:
  Bool.or(String.eq(Agy.method(m), "oauth-business"), String.eq(Agy.method(m), "agent-platform"))

def Agy.warn() -> String:
  "Google's Antigravity terms restrict third-party clients: signing in with your Google account here could get it suspended. An API key is safer."

def Agy.note.k(i: Nat) -> String:
  match i:
    case 0n:
      "Uses GEMINI_API_KEY from API keys. Google suggests a key for agents outside its own apps."
    case 1n:
      "Uses GOOGLE_API_KEY from API keys, or Application Default Credentials with a Google Cloud project and location."
    case 2n:
      "Signs in with Google for Gemini Enterprise; needs a Google Cloud project and location."
    case _:
      Agy.warn()

# what Settings says about a method
def Agy.note(+m: String) -> String:
  Agy.note.k(Agy.pos(Agy.methods(), Agy.method(m), 0n))

# an account's name as a folder: lower case letters, digits, - and _, at
# most 32 ("" is "default")
def Agy.acct.c(+c: U32, +rest: String) -> String:
  Bool.pick(String, Bool.or(Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95))),
    SCon{Chr{c}, rest}, Bool.pick(String, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), SCon{Chr{(c + 32 : U32)}, rest}, rest))

def Agy.acct.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Agy.acct.c(c, Agy.acct.go(t))

def Agy.acct(+raw: String) -> String:
  Agy.or(String.take(Agy.acct.go(String.trim(raw)), 32n), "default")

def Agy.profile(+root: String, +acct: String) -> String:
  root ++ "/profiles/" ++ Agy.acct(acct)

# the agent's settings.json for a method: auth.type and, when given, the
# Google Cloud project and location (never a credential)
def Agy.gcp.kvs(+project: String, +location: String) -> List<&2, J.KV>:
  List.append(&2, J.KV, Bool.pick(List<&2, J.KV>, String.is_empty(String.trim(project)), Nil{}, [J.KV{"project", J.Str{String.trim(project)}}]),
    Bool.pick(List<&2, J.KV>, String.is_empty(String.trim(location)), Nil{}, [J.KV{"location", J.Str{String.trim(location)}}]))

def Agy.settings.k(+m: String, +kvs: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj(J.KV{"auth", J.Json.obj([J.KV{"type", J.Str{Agy.method(m)}}])}
    <> Bool.pick(List<&2, J.KV>, List.is_empty(&2, J.KV, kvs), Nil{}, [J.KV{"gcp", J.Json.obj(kvs)}])))

def Agy.settings(+m: String, +project: String, +location: String) -> String:
  Agy.settings.k(m, Agy.gcp.kvs(project, location))

# The runner's arguments: the script, the agent's folder, the profile, the
# method, the API keys file and the settings
def Agy.args(+root: String, +keys: String, +m: String, +acct: String, +project: String, +location: String) -> List<&2, String>:
  [root ++ "/agy.sh", Agy.dir(root), Agy.profile(root, acct), Agy.method(m), keys, Agy.settings(m, project, location)]

# A turn
# ------
# initialize (1), authenticate with the method (4), then session/new (2)
# or session/resume (2; no replay; a session the agent no longer has is
# opened afresh, 7), the permission mode (5 afresh, 15 resumed), the model
# when one is chosen (6/16), and the prompt (3; Backplane's instructions
# go first in a fresh session). Updates and permission requests are
# Grok's (AC.Acp.line).

type AgyTurn is Data:
  AgyTurn{acp: AC.AcpTurn, method: String, mode: String, model: String}

def Agy.acp(t: AgyTurn) -> AC.AcpTurn:
  match t:
    case AgyTurn{a, _, _, _}:
      a

def Agy.turn.method(t: AgyTurn) -> String:
  match t:
    case AgyTurn{_, m, _, _}:
      m

def Agy.turn.mode(t: AgyTurn) -> String:
  match t:
    case AgyTurn{_, _, m, _}:
      m

def Agy.turn.model(t: AgyTurn) -> String:
  match t:
    case AgyTurn{_, _, _, m}:
      m

# the agent's permission modes: default asks, auto_edit lets edits
# through, yolo allows everything
def Agy.mode.k(i: Nat) -> String:
  match i:
    case 1n:
      "auto_edit"
    case 3n:
      "yolo"
    case _:
      "default"

def Agy.init() -> String:
  AC.Acp.init()

def Agy.auth(+m: String) -> String:
  AC.Acp.rpc(4n, "authenticate", J.Json.obj([J.KV{"methodId", J.Str{Agy.method(m)}}]))

def Agy.new.rpc(+id: Nat, +cwd: String, mcp: J.Json) -> String:
  AC.Acp.rpc(id, "session/new", J.Json.obj([J.KV{"cwd", J.Str{cwd}}, J.KV{"mcpServers", mcp}]))

def Agy.open.k(fresh: Bool, t: AC.AcpTurn) -> String:
  match fresh t:
    case True{} AC.AcpTurn{cwd, mcp, _, _, _, _}:
      Agy.new.rpc(2n, cwd, AC.Acp.mcp(mcp))
    case False{} AC.AcpTurn{cwd, mcp, _, _, sid, _}:
      AC.Acp.rpc(2n, "session/resume", J.Json.obj([J.KV{"sessionId", J.Str{sid}}, J.KV{"cwd", J.Str{cwd}}, J.KV{"mcpServers", AC.Acp.mcp(mcp)}]))

def Agy.open(+t: AgyTurn) -> String:
  Agy.open.k(String.is_empty(AC.Acp.session(Agy.acp(t))), Agy.acp(t))

def Agy.renew(t: AC.AcpTurn) -> String:
  match t:
    case AC.AcpTurn{cwd, mcp, _, _, _, _}:
      Agy.new.rpc(7n, cwd, AC.Acp.mcp(mcp))

def Agy.set_mode(+id: Nat, +sid: String, +mode: String) -> String:
  AC.Acp.rpc(id, "session/set_mode", J.Json.obj([J.KV{"sessionId", J.Str{sid}}, J.KV{"modeId", J.Str{mode}}]))

def Agy.set_model(+id: Nat, +sid: String, +model: String) -> String:
  AC.Acp.rpc(id, "session/set_config_option", J.Json.obj([J.KV{"sessionId", J.Str{sid}}, J.KV{"configId", J.Str{"model"}}, J.KV{"value", J.Str{model}}]))

def Agy.sid(a: AC.Acp) -> String:
  match a:
    case AC.Acp{s, _, _, _}:
      s

def Agy.with_sid(a: AC.Acp, +sid: String) -> AC.Acp:
  match a:
    case AC.Acp{_, b, d, k}:
      AC.Acp{sid, b, d, k}

def Agy.ended(a: AC.Acp) -> AC.Acp:
  match a:
    case AC.Acp{s, b, _, k}:
      AC.Acp{s, b, True{}, k}

# the turn fails: the reason, and how to fix a sign-in
def Agy.hint(+msg: String) -> String:
  Bool.pick(String, Bool.or(Bool.or(String.contains(msg, "auth"), String.contains(msg, "API key")), Bool.or(String.contains(msg, "credential"), String.contains(msg, "sign"))),
    msg ++ " (Settings › Agents › Antigravity: add a key or sign in)", msg)

def Agy.fail(a: AC.Acp, +msg: String) -> AC.AcpOut:
  AC.AcpOut{Agy.ended(a), Nil{}, [AC.Acp.failed(msg)], Nil{}}

def Agy.signin.msg() -> String:
  "Antigravity is not signed in: sign in under Settings › Agents › Antigravity first."

def Agy.missing.msg() -> String:
  "Antigravity is not installed: install it under Settings › Agents › Antigravity."

# the session is open (fresh or resumed): note it, then set the mode
def Agy.opened(fresh: Bool, a: AC.Acp, +t: AgyTurn, +sid: String) -> AC.AcpOut:
  AC.AcpOut{Agy.with_sid(a, sid), [Agy.set_mode(Bool.pick(Nat, fresh, 5n, 15n), sid, Agy.turn.mode(t))],
    [AC.Acp.event("thread.started", [J.KV{"thread_id", J.Str{sid}}])], Nil{}}

def Agy.say(fresh: Bool, +a: AC.Acp, +t: AgyTurn) -> AC.AcpOut:
  AC.AcpOut{a, [AC.Acp.say.k(fresh, Agy.acp(t), Agy.sid(a))], Nil{}, Nil{}}

# after the mode: the model, when one is chosen, else the prompt
def Agy.moded(+fresh: Bool, +a: AC.Acp, +t: AgyTurn) -> AC.AcpOut:
  Bool.pick(AC.AcpOut, String.is_empty(Agy.turn.model(t)), Agy.say(fresh, a, t),
    AC.AcpOut{a, [Agy.set_model(Bool.pick(Nat, fresh, 6n, 16n), Agy.sid(a), Agy.turn.model(t))], Nil{}, Nil{}})

def Agy.unmodel(+t: AgyTurn) -> String:
  "Antigravity has no model " ++ Agy.turn.model(t) ++ " for this sign-in: pick another in the composer."

def Agy.resumed(+a: AC.Acp, +t: AgyTurn, +o: J.Json) -> AC.AcpOut:
  Agy.opened(String.is_empty(AC.Acp.session(Agy.acp(t))), a, t, J.Json.get_str(J.Json.get_obj(o, "result"), "sessionId", AC.Acp.session(Agy.acp(t))))

# a resume that failed opens a fresh session; a fresh one that failed ends
# the turn
def Agy.reopen(fresh: Bool, a: AC.Acp, +t: AgyTurn, +o: J.Json) -> AC.AcpOut:
  match fresh:
    case True{}:
      Agy.fail(a, Agy.hint(AC.Acp.error(o)))
    case False{}:
      AC.AcpOut{a, [Agy.renew(Agy.acp(t))], Nil{}, Nil{}}

def Agy.reply.k(i: Nat, ok: Bool, a: AC.Acp, +t: AgyTurn, +o: J.Json) -> AC.AcpOut:
  match i ok:
    case 1n True{}:
      AC.AcpOut{a, [Agy.auth(Agy.turn.method(t))], Nil{}, Nil{}}
    case 4n True{}:
      AC.AcpOut{a, [Agy.open(t)], Nil{}, Nil{}}
    case 2n True{}:
      Agy.resumed(a, t, o)
    case 2n False{}:
      Agy.reopen(String.is_empty(AC.Acp.session(Agy.acp(t))), a, t, o)
    case 7n True{}:
      Agy.opened(True{}, a, t, J.Json.get_str(J.Json.get_obj(o, "result"), "sessionId", ""))
    case 5n _:
      Agy.moded(True{}, a, t)
    case 15n _:
      Agy.moded(False{}, a, t)
    case 6n True{}:
      Agy.say(True{}, a, t)
    case 16n True{}:
      Agy.say(False{}, a, t)
    case 6n False{}:
      Agy.fail(a, Agy.unmodel(t))
    case 16n False{}:
      Agy.fail(a, Agy.unmodel(t))
    case 3n _:
      AC.Acp.line(a, Agy.acp(t), J.Json.show(o))
    case _ False{}:
      Agy.fail(a, Agy.hint(AC.Acp.error(o)))
    case _ True{}:
      AC.AcpOut{a, Nil{}, Nil{}, Nil{}}

# digits as a number (no Nat.read: laws normalize this)
def Agy.num.c(+c: U32, +acc: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc)

def Agy.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Agy.num(t, Agy.num.c(c, acc))

def Agy.id(+o: J.Json) -> Nat:
  Agy.num(J.Json.as_raw(J.Json.get(o, "id"), "0"), 0n)

def Agy.line.k(req: Bool, a: AC.Acp, +t: AgyTurn, +o: J.Json) -> AC.AcpOut:
  match req:
    case True{}:
      AC.Acp.line(a, Agy.acp(t), J.Json.show(o))
    case False{}:
      Agy.reply.k(Agy.id(o), String.eq(J.Json.show(J.Json.get_obj(o, "error")), "null"), a, t, o)

# a line that is not JSON: the runner's own words, or the agent's noise
def Agy.plain(+a: AC.Acp, +line: String) -> AC.AcpOut:
  Bool.pick(AC.AcpOut, String.starts_with(line, "BACKPLANE-SIGNIN "), Agy.fail(a, Agy.signin.msg()),
    Bool.pick(AC.AcpOut, String.starts_with(line, "BACKPLANE-MISSING"), Agy.fail(a, Agy.missing.msg()), AC.AcpOut{a, Nil{}, Nil{}, Nil{}}))

def Agy.line.parsed(m: Maybe<&2, J.Json>, a: AC.Acp, +t: AgyTurn, +line: String) -> AC.AcpOut:
  match m:
    case Some{+o}:
      Agy.line.k(Bool.not(String.is_empty(J.Json.get_str(o, "method", ""))), a, t, o)
    case None{}:
      Agy.plain(a, String.trim(line))

# a line as JSON when it is an object (the runner's own words are not)
def Agy.parse(+line: String) -> Maybe<&2, J.Json>:
  Bool.pick(Maybe<&2, J.Json>, String.starts_with(String.trim(line), "{"), J.Json.parse(line), None{})

# one line from the agent
def Agy.line(a: AC.Acp, +t: AgyTurn, +line: String) -> AC.AcpOut:
  Agy.line.parsed(Agy.parse(line), a, t, line)

# Models
# ------
# session/new answers the models this sign-in offers: configOptions'
# "model" select (its options, or groups of them), else models'
# availableModels. Kept as {"models":[{"id","name"}],"default":id}.

type AgyModel is Data:
  AgyModel{id: String, name: String}

# an option's entry (a group's options go flat, one level down)
def Agy.val(+o: J.Json) -> AgyModel:
  AgyModel{J.Json.get_str(o, "value", ""), Agy.or(J.Json.get_str(o, "name", ""), J.Json.get_str(o, "value", ""))}

def Agy.vals(xs: List<&2, J.Json>) -> List<&2, AgyModel>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> +t:
      Bool.pick(List<&2, AgyModel>, String.is_empty(J.Json.get_str(o, "value", "")), Agy.vals(t), Agy.val(o) <> Agy.vals(t))

def Agy.opt(+o: J.Json) -> List<&2, AgyModel>:
  Bool.pick(List<&2, AgyModel>, String.is_empty(J.Json.get_str(o, "value", "")), Agy.vals(J.Json.elems(J.Json.get_obj(o, "options"))), [Agy.val(o)])

def Agy.opts(xs: List<&2, J.Json>, rest: List<&2, AgyModel>) -> List<&2, AgyModel>:
  match xs:
    case Nil{}:
      rest
    case +o <> t:
      List.append(&2, AgyModel, Agy.opt(o), Agy.opts(t, rest))

def Agy.avail(xs: List<&2, J.Json>) -> List<&2, AgyModel>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      AgyModel{J.Json.get_str(o, "modelId", ""), Agy.or(J.Json.get_str(o, "name", ""), J.Json.get_str(o, "modelId", ""))} <> Agy.avail(t)

def Agy.config(xs: List<&2, J.Json>) -> Maybe<&2, J.Json>:
  match xs:
    case Nil{}:
      None{}
    case +o <> t:
      Bool.pick(Maybe<&2, J.Json>, String.eq(J.Json.get_str(o, "id", ""), "model"), Some{o}, Agy.config(t))

def Agy.models.k(m: Maybe<&2, J.Json>, +r: J.Json) -> List<&2, AgyModel>:
  match m:
    case Some{+c}:
      Agy.opts(J.Json.elems(J.Json.get_obj(c, "options")), Nil{})
    case None{}:
      Agy.avail(J.Json.elems(J.Json.get_obj(J.Json.get_obj(r, "models"), "availableModels")))

def Agy.models(+r: J.Json) -> List<&2, AgyModel>:
  Agy.models.k(Agy.config(J.Json.elems(J.Json.get_obj(r, "configOptions"))), r)

def Agy.current.k(m: Maybe<&2, J.Json>, +r: J.Json) -> String:
  match m:
    case Some{+c}:
      J.Json.get_str(c, "currentValue", "")
    case None{}:
      J.Json.get_str(J.Json.get_obj(r, "models"), "currentModelId", "")

def Agy.current(+r: J.Json) -> String:
  Agy.current.k(Agy.config(J.Json.elems(J.Json.get_obj(r, "configOptions"))), r)

def Agy.model.json(m: AgyModel) -> J.Json:
  match m:
    case AgyModel{i, n}:
      J.Json.obj([J.KV{"id", J.Str{i}}, J.KV{"name", J.Str{n}}])

def Agy.models.json(ms: List<&2, AgyModel>) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      Agy.model.json(m) <> Agy.models.json(t)

# the cache's text for a session/new result
def Agy.cache(+r: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"models", J.Json.arr(Agy.models.json(Agy.models(r)))}, J.KV{"default", J.Str{Agy.current(r)}}]))

# The sign-in
# -----------
# One process: initialize (1), authenticate (4), and once that answers, a
# session (2, in the hub's home, no MCP) whose answer lists the models.
# What it says: "url\t<address>" (a Google sign-in to open), "ok\t<cache>"
# or "fail\t<why>"; done once ok or fail.

type AgySign is Data:
  AgySign{url: String, done: Bool}

type AgySignOut is Data:
  AgySignOut{s: AgySign, send: List<&2, String>, say: List<&2, String>}

def AgySign.new() -> AgySign:
  AgySign{"", False{}}

def AgySign.done(s: AgySign) -> Bool:
  match s:
    case AgySign{_, d}:
      d

def AgySign.url(s: AgySign) -> String:
  match s:
    case AgySign{u, _}:
      u

def AgySign.fail(s: AgySign, +why: String) -> AgySignOut:
  match s:
    case AgySign{u, _}:
      AgySignOut{AgySign{u, True{}}, Nil{}, ["fail\t" ++ why]}

# Google's words made plain where they matter
def Agy.sign.why(+msg: String) -> String:
  Bool.pick(String, String.contains(msg, "SUBSCRIPTION_REQUIRED"), "Google needs an eligible Antigravity plan for this account.",
    Bool.pick(String, Bool.or(String.contains(msg, "access_denied"), String.contains(msg, "cancelled")), "Google's sign-in was not approved.", msg))

def AgySign.why(+o: J.Json) -> String:
  Agy.sign.why(J.Json.get_str(J.Json.get_obj(o, "error"), "message", "Antigravity could not sign in"))

def AgySign.reply.k(i: Nat, ok: Bool, s: AgySign, +m: String, +home: String, +o: J.Json) -> AgySignOut:
  match i ok:
    case 1n True{}:
      AgySignOut{s, [Agy.auth(m)], Nil{}}
    case 4n True{}:
      AgySignOut{s, [Agy.new.rpc(2n, home, J.Json.arr(Nil{}))], Nil{}}
    case 2n True{}:
      AgySignOut{AgySign{AgySign.url(s), True{}}, Nil{}, ["ok\t" ++ Agy.cache(J.Json.get_obj(o, "result"))]}
    case _ False{}:
      AgySign.fail(s, AgySign.why(o))
    case _ True{}:
      AgySignOut{s, Nil{}, Nil{}}

def AgySign.url.k(ok: Bool, s: AgySign, +u: String) -> AgySignOut:
  match ok:
    case True{}:
      AgySignOut{AgySign{u, AgySign.done(s)}, Nil{}, ["url\t" ++ u]}
    case False{}:
      AgySign.fail(s, "Antigravity gave a sign-in address that is not Google's")

# The way back from Google
# ------------------------
# Google sends the browser to the agent's loopback address. On the hub's
# own machine that just works; from another device the page fails to load,
# and the person pastes its address: it is taken only if it is the
# redirect this sign-in asked for (same port and path), carries this
# sign-in's state once and one code or error. The hub then fetches it on
# its own machine.

def Agy.hex(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), (c - 48 : U32),
    Bool.pick(U32, Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102)), (c - 87 : U32), Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 70)), (c - 55 : U32), 0)))

# percent-decoding (and + as a space), for a query's values: a walk
# whose state says whether a % (and its first digit) came before
type AgyPc is Data:
  AgyPc{st: Nat, hi: U32, out: U32, has: Bool}

def Agy.pc.k(st: Nat, +c: U32, +hi: U32) -> AgyPc:
  match st:
    case 0n:
      Bool.pick(AgyPc, U32.is_eq(c, 37), AgyPc{1n, 0, 0, False{}}, AgyPc{0n, 0, Bool.pick(U32, U32.is_eq(c, 43), 32, c), True{}})
    case 1n:
      AgyPc{2n, Agy.hex(c), 0, False{}}
    case _:
      AgyPc{0n, 0, ((hi * 16 : U32) + Agy.hex(c) : U32), True{}}

def Agy.pc.step(+c: U32, p: AgyPc) -> AgyPc:
  match p:
    case AgyPc{st, hi, _, _}:
      Agy.pc.k(st, c, hi)

def Agy.pc.emit(p: AgyPc, rest: String) -> String:
  match p:
    case AgyPc{_, _, o, True{}}:
      SCon{Chr{o}, rest}
    case AgyPc{_, _, _, False{}}:
      rest

def Agy.unpct.go(s: String, +p: AgyPc) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Agy.pc.emit(Agy.pc.step(c, p), Agy.unpct.go(t, Agy.pc.step(c, p)))

def Agy.unpct(s: String) -> String:
  Agy.unpct.go(s, AgyPc{0n, 0, 0, False{}})

def Agy.after(xs: List<&2, String>) -> String:
  match xs:
    case _ <> +t:
      String.join(t, "?")
    case Nil{}:
      ""

# the query after ?, its pairs
def Agy.query(+u: String) -> List<&2, String>:
  String.split(Agy.after(String.split(u, '?')), '&')

def Agy.base(xs: List<&2, String>) -> String:
  match xs:
    case h <> _:
      h
    case Nil{}:
      ""

# every value a query gives a name, decoded
def Agy.params(ps: List<&2, String>, +name: String) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> +t:
      Bool.pick(List<&2, String>, String.starts_with(p, name ++ "="), Agy.unpct(String.drop(p, 1n+String.length(name))) <> Agy.params(t, name), Agy.params(t, name))

def Agy.one(xs: List<&2, String>) -> String:
  match xs:
    case x <> Nil{}:
      x
    case _:
      ""

# a query's value when it gives the name exactly once, else ""
def Agy.param(+u: String, +name: String) -> String:
  Agy.one(Agy.params(Agy.query(u), name))

def Agy.redirect(+u: String) -> String:
  Agy.param(u, "redirect_uri")

# a sign-in address the agent prints, checked: Google's OAuth page, one
# state, a loopback redirect on a port of its own; anything else is not
# shown to anyone
def Agy.url.ok(+u: String) -> Bool:
  Bool.and(Bool.and(String.starts_with(u, "https://accounts.google.com/o/oauth2/v2/auth?"), Nat.is_lt(String.length(u), 16384n)),
    Bool.and(Bool.not(String.contains(u, " ")), Bool.and(String.starts_with(Agy.redirect(u), "http://127.0.0.1:"), Bool.not(String.is_empty(Agy.param(u, "state"))))))

def AgySign.plain(+s: AgySign, +line: String) -> AgySignOut:
  Bool.pick(AgySignOut, String.starts_with(line, "BACKPLANE-SIGNIN "), AgySign.url.k(Agy.url.ok(String.trim(String.drop(line, 17n))), s, String.trim(String.drop(line, 17n))),
    Bool.pick(AgySignOut, String.starts_with(line, "BACKPLANE-MISSING"), AgySign.fail(s, Agy.missing.msg()), AgySignOut{s, Nil{}, Nil{}}))

def AgySign.parsed(m: Maybe<&2, J.Json>, +s: AgySign, +meth: String, +home: String, +line: String) -> AgySignOut:
  match m:
    case Some{+o}:
      Bool.pick(AgySignOut, String.is_empty(J.Json.get_str(o, "method", "")), AgySign.reply.k(Agy.id(o), String.eq(J.Json.show(J.Json.get_obj(o, "error")), "null"), s, meth, home, o),
        AgySignOut{s, Nil{}, Nil{}})
    case None{}:
      AgySign.plain(s, String.trim(line))

# one line from the agent while signing in with method m (home: where its
# probe session opens)
def AgySign.line(s: AgySign, +m: String, +home: String, +line: String) -> AgySignOut:
  AgySign.parsed(Agy.parse(line), s, m, home, line)

def Agy.count(+u: String, +name: String) -> Nat:
  List.length(&2, String, Agy.params(Agy.query(u), name))

def Agy.trailing(+r: String) -> String:
  Bool.pick(String, String.ends_with(r, "/"), r, r ++ "/")

# whether a pasted address answers the sign-in that asked at `url`
def Agy.callback.ok(+url: String, +back: String) -> Bool:
  Bool.and(Bool.and(Agy.url.ok(url), String.eq(Agy.trailing(Agy.base(String.split(String.trim(back), '?'))), Agy.trailing(Agy.redirect(url)))),
    Bool.and(Bool.and(Nat.is_eq(Agy.count(back, "state"), 1n), String.eq(Agy.param(back, "state"), Agy.param(url, "state"))),
      Bool.and(Nat.is_eq(Nat.add(Agy.count(back, "code"), Agy.count(back, "error")), 1n), Bool.not(String.contains(back, "#")))))

# What the hub knows
# ------------------
# The probe (Agy.probe.sh) prints lines the hub reads into the models
# info: "have <version>" (installed), "old <version>" (an earlier one),
# "plat <uname>", "acct <0|1> <name>" (a profile, signed in or not) and
# "models <cache>".

def Agy.probe.sh(+root: String) -> String:
  "d=$1; [ -f \"$d/" ++ Agy.version() ++ "/.ok\" ] && echo have " ++ Agy.version() ++ "\n"
    ++ "for o in \"$d\"/*/.ok; do [ -f \"$o\" ] && echo old \"$(basename \"$(dirname \"$o\")\")\"; done\n"
    ++ "echo plat \"$(uname -s)-$(uname -m)\"\n"
    ++ "for p in \"$d\"/profiles/*/; do [ -d \"$p\" ] || continue; s=0; [ -f \"$p/antigravity-acp/acp_token.json\" ] && s=1; echo acct $s \"$(basename \"$p\")\"; done\n"
    ++ "[ -f \"$d/models.json\" ] && printf 'models %s\\n' \"$(tr -d '\\n' < \"$d/models.json\")\"\n"
    ++ "exit 0\n"

type AgyInfo is Data:
  AgyInfo{have: String, old: String, plat: String, accts: List<&2, String>, signed: List<&2, String>, cache: String}

def Agy.info.new() -> AgyInfo:
  AgyInfo{"", "", "", Nil{}, Nil{}, ""}

def Agy.info.k(i: Nat, a: AgyInfo, +l: String) -> AgyInfo:
  match i a:
    case 0n AgyInfo{_, o, p, as, ss, c}:
      AgyInfo{String.drop(l, 5n), o, p, as, ss, c}
    case 1n AgyInfo{h, o, p, as, ss, c}:
      AgyInfo{h, Bool.pick(String, String.eq(String.drop(l, 4n), Agy.version()), o, String.drop(l, 4n)), p, as, ss, c}
    case 2n AgyInfo{h, o, _, as, ss, c}:
      AgyInfo{h, o, String.drop(l, 5n), as, ss, c}
    case 3n AgyInfo{h, o, p, as, +ss, c}:
      AgyInfo{h, o, p, List.append(&2, String, as, [String.drop(l, 7n)]), Bool.pick(List<&2, String>, String.starts_with(l, "acct 1 "), List.append(&2, String, ss, [String.drop(l, 7n)]), ss), c}
    case 4n AgyInfo{h, o, p, as, ss, _}:
      AgyInfo{h, o, p, as, ss, String.drop(l, 7n)}
    case _ _:
      a

def Agy.word(+l: String) -> String:
  Agy.base(String.split(l, ' '))

def Agy.info.line(a: AgyInfo, +l: String) -> AgyInfo:
  Agy.info.k(Agy.pos(["have", "old", "plat", "acct", "models"], Agy.word(l), 0n), a, l)

def Agy.info.go(ls: List<&2, String>, a: AgyInfo) -> AgyInfo:
  match ls:
    case Nil{}:
      a
    case +l <> t:
      Agy.info.go(t, Agy.info.line(a, String.trim(l)))

def Agy.info(+out: String) -> AgyInfo:
  Agy.info.go(String.lines(out), Agy.info.new())

def Agy.info.cache(a: AgyInfo) -> J.Json:
  match a:
    case AgyInfo{_, _, _, _, _, c}:
      Maybe.default(&2, J.Json, J.Json.parse(c), J.Null{})

def Agy.info.have(a: AgyInfo) -> String:
  match a:
    case AgyInfo{h, _, _, _, _, _}:
      h

# whether this machine has a build (its uname is in the pins)
def Agy.plat.ok(rs: List<&2, AgyRel>, +p: String) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case AgyRel{n, _, _, _} <> t:
      Bool.or(String.eq(n, p), Agy.plat.ok(t, p))

def Agy.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Agy.strs(t)

# the "agy" object in the models info: the pinned version, the one
# installed ("" none), an earlier one still there, whether this machine
# has a build, the accounts and those signed in with Google
def Agy.info.json(a: AgyInfo) -> J.Json:
  match a:
    case AgyInfo{h, o, p, as, ss, _}:
      J.Json.obj([J.KV{"pin", J.Str{Agy.version()}}, J.KV{"have", J.Str{h}}, J.KV{"old", J.Str{o}}, J.KV{"plat", J.Json.bool(Agy.plat.ok(Agy.rels(), p))},
        J.KV{"accts", J.Json.arr(Agy.strs(as))}, J.KV{"signed", J.Json.arr(Agy.strs(ss))}])

def Agy.ids(ms: List<&2, J.Json>) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      J.Json.get_str(m, "id", "") <> Agy.ids(t)

def Agy.labels(ms: List<&2, J.Json>, +dm: String) -> List<&2, J.KV>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      J.KV{J.Json.get_str(m, "id", ""), J.Str{J.Json.get_str(m, "name", "") ++ Bool.pick(String, String.eq(J.Json.get_str(m, "id", ""), dm), " · Antigravity's default", "")}} <> Agy.labels(t, dm)

# the models a client lists (none until the agent is installed)
def Agy.info.models.k(on: Bool, +c: J.Json) -> List<&2, String>:
  match on:
    case True{}:
      Agy.ids(J.Json.elems(J.Json.get_obj(c, "models")))
    case False{}:
      Nil{}

def Agy.info.models(+a: AgyInfo) -> List<&2, String>:
  Agy.info.models.k(Bool.not(String.is_empty(Agy.info.have(a))), Agy.info.cache(a))

# "about" for the picker: what is installed and each model's name
def Agy.info.about(+a: AgyInfo) -> J.Json:
  J.Json.obj([J.KV{"cli", J.Str{Bool.pick(String, String.is_empty(Agy.info.have(a)), "", "Antigravity " ++ Agy.info.have(a))}},
    J.KV{"labels", J.Json.obj(J.KV{"default", J.Str{"Antigravity's own default"}} <> Agy.labels(J.Json.elems(J.Json.get_obj(Agy.info.cache(a), "models")), J.Json.get_str(Agy.info.cache(a), "default", "")))}])
