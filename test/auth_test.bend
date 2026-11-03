import Base
import ../src/core/auth.bend as A
import ../src/core/http.bend as H
import ./check.bend as C

def yes(b: Bool) -> String:
  match b:
    case True{}: "yes"
    case False{}: "no"

def hs(+origin: String, host: String, cookie: String) -> List<&2, H.Header>:
  [H.Header{"origin", origin}, H.Header{"host", host}, H.Header{"cookie", cookie}]

def oauth(+host: String, +origin: String, +mode: String, +dest: String, proxy: String) -> List<&2, H.Header>:
  [H.Header{"host", host}, H.Header{"origin", origin}, H.Header{"sec-fetch-site", "cross-site"},
    H.Header{"sec-fetch-mode", mode}, H.Header{"sec-fetch-dest", dest}, H.Header{"tailscale-user-login", proxy}]

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("loopback without the token denied", yes(A.Auth.allowed([H.Header{"host", "127.0.0.1:3787"}], "/ws", "secret")), "no")
    C.Check.str("localhost without the token denied", yes(A.Auth.allowed([H.Header{"host", "localhost:3787"}], "/ws", "secret")), "no")
    C.Check.str("native without Host or token denied", yes(A.Auth.allowed(Nil{}, "/ws", "secret")), "no")
    C.Check.str("loopback with the token", yes(A.Auth.allowed([H.Header{"host", "127.0.0.1:3787"}], "/ws?token=secret", "secret")), "yes")
    C.Check.str("loopback with a bearer token", yes(A.Auth.allowed([H.Header{"host", "127.0.0.1:3787"}, H.Header{"authorization", "Bearer secret"}], "/ws", "secret")), "yes")
    C.Check.str("unpaired remote", yes(A.Auth.allowed(Nil{}, "/ws", "secret")), "no")
    C.Check.str("exact query", yes(A.Auth.allowed(Nil{}, "/ws?token=secret", "secret")), "yes")
    C.Check.str("query prefix denied", yes(A.Auth.allowed(Nil{}, "/ws?token=secret-tail", "secret")), "no")
    C.Check.str("wrong parameter denied", yes(A.Auth.allowed(Nil{}, "/ws?not_token=secret", "secret")), "no")
    C.Check.str("path substring denied", yes(A.Auth.allowed(Nil{}, "/token=secret/ws", "secret")), "no")
    C.Check.str("cookie exact", yes(A.Auth.allowed(hs("", "", "a=b; bp_token=secret; c=d"), "/ws", "secret")), "yes")
    C.Check.str("cookie suffix denied", yes(A.Auth.allowed(hs("", "", "bp_token=secret-tail"), "/ws", "secret")), "no")
    C.Check.str("cookie name denied", yes(A.Auth.allowed(hs("", "", "not_bp_token=secret"), "/ws", "secret")), "no")
    C.Check.str("empty secret denied", yes(A.Auth.allowed(hs("", "", "bp_token="), "/ws?token=", "")), "no")
    C.Check.str("same origin local browser without its cookie denied", yes(A.Auth.allowed(hs("http://127.0.0.1:3787", "127.0.0.1:3787", ""), "/ws", "secret")), "no")
    C.Check.str("same origin local browser with its cookie", yes(A.Auth.allowed(hs("http://127.0.0.1:3787", "127.0.0.1:3787", "bp_token=secret"), "/ws", "secret")), "yes")
    C.Check.str("foreign browser denied", yes(A.Auth.allowed(hs("https://evil.example", "127.0.0.1:3787", ""), "/ws", "secret")), "no")
    C.Check.str("foreign browser with token denied", yes(A.Auth.allowed(hs("https://evil.example", "127.0.0.1:3787", "bp_token=secret"), "/ws", "secret")), "no")
    C.Check.str("null origin denied", yes(A.Auth.allowed(hs("null", "127.0.0.1:3787", ""), "/ws", "secret")), "no")
    C.Check.str("DNS rebound hostname denied", yes(A.Auth.allowed(hs("http://evil.example:3787", "evil.example:3787", ""), "/ws", "secret")), "no")
    C.Check.str("DNS rebound GET without origin denied", yes(A.Auth.allowed(hs("", "evil.example:3787", ""), "/debug/perf", "secret")), "no")
    C.Check.str("paired tailnet browser", yes(A.Auth.allowed(hs("https://mine.ts.net", "mine.ts.net", "bp_token=secret"), "/ws", "secret")), "yes")
    C.Check.str("wrong port denied", yes(A.Auth.allowed(hs("http://localhost:4000", "localhost:3787", ""), "/ws", "secret")), "no")
    C.Check.str("origin case normalization", yes(A.Auth.allowed(hs("http://LOCALHOST:3787", "localhost:3787", "bp_token=secret"), "/ws", "secret")), "yes")
    C.Check.str("cross site metadata denied", yes(A.Auth.allowed([H.Header{"sec-fetch-site", "cross-site"}], "/ws", "secret")), "no")
    C.Check.str("proxy identity needs credential", yes(A.Auth.allowed([H.Header{"tailscale-user-login", "me"}], "/ws", "secret")), "no")
    C.Check.str("bearer pairing", yes(A.Auth.pair([H.Header{"authorization", "Bearer secret"}], "secret")), "yes")
    C.Check.str("pair cannot inherit trust", yes(A.Auth.pair(Nil{}, "secret")), "no")
    C.Check.str("pair query insufficient", yes(A.Auth.pair(hs("", "", "bp_token=secret"), "secret")), "no")
    C.Check.str("cookie flags", A.Auth.cookie("secret", True{}), "Set-Cookie: bp_token=secret; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000; Secure\r\n")
    C.Check.str("token compared whole", yes(A.Auth.same("secret", "secret")), "yes")
    C.Check.str("a prefix of the token is not it", yes(A.Auth.same("secre", "secret")), "no")
    C.Check.str("a longer guess is not it", yes(A.Auth.same("secretx", "secret")), "no")
    C.Check.str("a wrong token is refused", yes(A.Auth.allowed(Nil{}, "/ws?token=secreT", "secret")), "no")
    C.Check.str("an MCP key is a sha256 HMAC, hex", Nat.show(String.length(A.Auth.mcp.key("secret", "t1.3"))), "64")
    C.Check.str("an MCP key is not the token", yes(String.contains(A.Auth.mcp.key("secret", "t1.3"), "secret")), "no")
    C.Check.str("an MCP key differs per thread", yes(String.eq(A.Auth.mcp.key("secret", "t1.3"), A.Auth.mcp.key("secret", "t2.3"))), "no")
    C.Check.str("an MCP call with its key", yes(A.Auth.mcp(Nil{}, "/mcp/t1.3?k=" ++ A.Auth.mcp.key("secret", "t1.3"), "secret", "t1.3")), "yes")
    C.Check.str("another thread's key is refused", yes(A.Auth.mcp(Nil{}, "/mcp/t2.3?k=" ++ A.Auth.mcp.key("secret", "t1.3"), "secret", "t2.3")), "no")
    C.Check.str("an MCP call with the token", yes(A.Auth.mcp(Nil{}, "/mcp/t1?token=secret", "secret", "t1")), "yes")
    C.Check.str("an MCP call with neither, from loopback, is refused", yes(A.Auth.mcp([H.Header{"host", "127.0.0.1:3787"}], "/mcp/t1", "secret", "t1")), "no")
    C.Check.str("a keyed MCP call from a foreign site is refused", yes(A.Auth.mcp(hs("https://evil.example", "127.0.0.1:3787", ""), "/mcp/t1?k=" ++ A.Auth.mcp.key("secret", "t1"), "secret", "t1")), "no")
    C.Check.str("no key without a token", yes(A.Auth.mcp(Nil{}, "/mcp/t1?k=" ++ A.Auth.mcp.key("", "t1"), "", "t1")), "no")
    C.Check.str("Google cross-site document callback reaches state check", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "", "navigate", "document", ""), "GET", "/oauth/google?state=pending&code=code", "secret")), "yes")
    C.Check.str("Google callback keeps native compatibility", yes(A.Auth.oauth(True{}, hs("", "localhost:3787", ""), "GET", "/oauth/google?state=pending&code=code", "secret")), "yes")
    C.Check.str("Google navigation does not authorize ordinary routes", yes(A.Auth.allowed(oauth("127.0.0.1:3787", "", "navigate", "document", ""), "/ws", "secret")), "no")
    C.Check.str("Google gate is callback-only", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "", "navigate", "document", ""), "GET", "/ws?token=secret", "secret")), "no")
    C.Check.str("Google gate refuses POST", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "", "navigate", "document", ""), "POST", "/oauth/google", "secret")), "no")
    C.Check.str("Google gate refuses foreign Origin", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "https://foreign.invalid", "navigate", "document", ""), "GET", "/oauth/google", "secret")), "no")
    C.Check.str("Google gate refuses script fetch", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "", "cors", "empty", ""), "GET", "/oauth/google", "secret")), "no")
    C.Check.str("Google gate refuses iframe navigation", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "", "navigate", "iframe", ""), "GET", "/oauth/google", "secret")), "no")
    C.Check.str("Google gate refuses rebound Host", yes(A.Auth.oauth(True{}, oauth("foreign.invalid", "", "navigate", "document", ""), "GET", "/oauth/google", "secret")), "no")
    C.Check.str("Google gate refuses proxy identity", yes(A.Auth.oauth(True{}, oauth("127.0.0.1:3787", "", "navigate", "document", "person"), "GET", "/oauth/google", "secret")), "no")
    C.Check.str("Google callback without Fetch Metadata reaches state check", yes(A.Auth.oauth(True{}, [H.Header{"host", "127.0.0.1:3787"}], "GET", "/oauth/google?state=pending&code=code", "secret")), "yes")
    C.Check.str("Google callback without Fetch Metadata still needs loopback", yes(A.Auth.oauth(False{}, [H.Header{"host", "127.0.0.1:3787"}], "GET", "/oauth/google?state=pending&code=code", "secret")), "no")
    C.Check.str("Google gate requires loopback connection", yes(A.Auth.oauth(False{}, oauth("127.0.0.1:3787", "", "navigate", "document", ""), "GET", "/oauth/google", "secret")), "no")
    C.Check.str("pairing proof checks for the hub that made it", yes(A.Auth.pair.checked("secret", "http://127.0.0.1:4002", 1000n, 1030n, A.Auth.pair.proof("secret", "http://127.0.0.1:4002", 1000n))), "yes")
    C.Check.str("pairing proof of another token refused", yes(A.Auth.pair.checked("secret", "http://127.0.0.1:4002", 1000n, 1030n, A.Auth.pair.proof("other", "http://127.0.0.1:4002", 1000n))), "no")
    C.Check.str("pairing proof for another address refused", yes(A.Auth.pair.checked("secret", "http://127.0.0.1:4003", 1000n, 1030n, A.Auth.pair.proof("secret", "http://127.0.0.1:4002", 1000n))), "no")
    C.Check.str("pairing proof for another time refused", yes(A.Auth.pair.checked("secret", "http://127.0.0.1:4002", 1001n, 1030n, A.Auth.pair.proof("secret", "http://127.0.0.1:4002", 1000n))), "no")
    C.Check.str("stale pairing proof refused", yes(A.Auth.pair.checked("secret", "http://127.0.0.1:4002", 1000n, 1061n, A.Auth.pair.proof("secret", "http://127.0.0.1:4002", 1000n))), "no")
    C.Check.str("empty pairing proof refused", yes(A.Auth.pair.checked("secret", "http://127.0.0.1:4002", 1000n, 1000n, "")), "no")
    C.Check.str("no token, no pairing proof", yes(A.Auth.pair.checked("", "http://127.0.0.1:4002", 1000n, 1000n, A.Auth.pair.proof("", "http://127.0.0.1:4002", 1000n))), "no")
