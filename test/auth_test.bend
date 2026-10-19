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

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("native loopback", yes(A.Auth.allowed(True{}, Nil{}, "/ws", "secret")), "yes")
    C.Check.str("unpaired remote", yes(A.Auth.allowed(False{}, Nil{}, "/ws", "secret")), "no")
    C.Check.str("exact query", yes(A.Auth.allowed(False{}, Nil{}, "/ws?token=secret", "secret")), "yes")
    C.Check.str("query prefix denied", yes(A.Auth.allowed(False{}, Nil{}, "/ws?token=secret-tail", "secret")), "no")
    C.Check.str("wrong parameter denied", yes(A.Auth.allowed(False{}, Nil{}, "/ws?not_token=secret", "secret")), "no")
    C.Check.str("path substring denied", yes(A.Auth.allowed(False{}, Nil{}, "/token=secret/ws", "secret")), "no")
    C.Check.str("cookie exact", yes(A.Auth.allowed(False{}, hs("", "", "a=b; bp_token=secret; c=d"), "/ws", "secret")), "yes")
    C.Check.str("cookie suffix denied", yes(A.Auth.allowed(False{}, hs("", "", "bp_token=secret-tail"), "/ws", "secret")), "no")
    C.Check.str("cookie name denied", yes(A.Auth.allowed(False{}, hs("", "", "not_bp_token=secret"), "/ws", "secret")), "no")
    C.Check.str("empty secret denied", yes(A.Auth.allowed(False{}, hs("", "", "bp_token="), "/ws?token=", "")), "no")
    C.Check.str("same origin local browser", yes(A.Auth.allowed(True{}, hs("http://127.0.0.1:3787", "127.0.0.1:3787", ""), "/ws", "secret")), "yes")
    C.Check.str("foreign browser denied", yes(A.Auth.allowed(True{}, hs("https://evil.example", "127.0.0.1:3787", ""), "/ws", "secret")), "no")
    C.Check.str("foreign browser with token denied", yes(A.Auth.allowed(True{}, hs("https://evil.example", "127.0.0.1:3787", "bp_token=secret"), "/ws", "secret")), "no")
    C.Check.str("null origin denied", yes(A.Auth.allowed(True{}, hs("null", "127.0.0.1:3787", ""), "/ws", "secret")), "no")
    C.Check.str("DNS rebound hostname denied", yes(A.Auth.allowed(True{}, hs("http://evil.example:3787", "evil.example:3787", ""), "/ws", "secret")), "no")
    C.Check.str("DNS rebound GET without origin denied", yes(A.Auth.allowed(True{}, hs("", "evil.example:3787", ""), "/debug/perf", "secret")), "no")
    C.Check.str("paired tailnet browser", yes(A.Auth.allowed(True{}, hs("https://mine.ts.net", "mine.ts.net", "bp_token=secret"), "/ws", "secret")), "yes")
    C.Check.str("wrong port denied", yes(A.Auth.allowed(True{}, hs("http://localhost:4000", "localhost:3787", ""), "/ws", "secret")), "no")
    C.Check.str("origin case normalization", yes(A.Auth.allowed(True{}, hs("http://LOCALHOST:3787", "localhost:3787", ""), "/ws", "secret")), "yes")
    C.Check.str("cross site metadata denied", yes(A.Auth.allowed(True{}, [H.Header{"sec-fetch-site", "cross-site"}], "/ws", "secret")), "no")
    C.Check.str("proxy identity needs credential", yes(A.Auth.allowed(True{}, [H.Header{"tailscale-user-login", "me"}], "/ws", "secret")), "no")
    C.Check.str("bearer pairing", yes(A.Auth.pair([H.Header{"authorization", "Bearer secret"}], "secret")), "yes")
    C.Check.str("pair cannot inherit trust", yes(A.Auth.pair(Nil{}, "secret")), "no")
    C.Check.str("pair query insufficient", yes(A.Auth.pair(hs("", "", "bp_token=secret"), "secret")), "no")
    C.Check.str("cookie flags", A.Auth.cookie("secret", True{}), "Set-Cookie: bp_token=secret; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000; Secure\r\n")
