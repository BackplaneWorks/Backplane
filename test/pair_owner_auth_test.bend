import Base
import ../src/core/auth.bend as A
import ../src/core/http.bend as H
import ./check.bend as C

def own() -> List<&2, String>:
  A.Auth.owner.hosts("100.64.0.8", "hub.fixture.ts.net", "3787")

def hs(+host: String) -> List<&2, H.Header>:
  [H.Header{"host", host}]

def allowed(+trusted: Bool, +headers: List<&2, H.Header>, +method: String, +path: String) -> String:
  Bool.show(A.Auth.owner.pair(trusted, own(), headers, method, path))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("owner IP and exact port", allowed(True{}, hs("100.64.0.8:3787"), "POST", "/bots/pair?name=peer"), "True")
    C.Check.str("owner IP without explicit port", allowed(True{}, hs("100.64.0.8"), "POST", "/bots/pair"), "True")
    C.Check.str("owner MagicDNS and port", allowed(True{}, hs("hub.fixture.ts.net:3787"), "POST", "/bots/pair"), "True")
    C.Check.str("owner MagicDNS without explicit port", allowed(True{}, hs("hub.fixture.ts.net"), "POST", "/bots/pair"), "True")
    C.Check.str("DNS case normalized", allowed(True{}, hs("HUB.FIXTURE.TS.NET:3787"), "POST", "/bots/pair"), "True")
    C.Check.str("unauthenticated remote spoofing own host denied", allowed(False{}, hs("100.64.0.8:3787"), "POST", "/bots/pair"), "False")
    C.Check.str("rebound attacker hostname denied", allowed(True{}, hs("attacker.invalid:3787"), "POST", "/bots/pair"), "False")
    C.Check.str("hostname suffix denied", allowed(True{}, hs("hub.fixture.ts.net.attacker.invalid:3787"), "POST", "/bots/pair"), "False")
    C.Check.str("hostname prefix denied", allowed(True{}, hs("attacker-hub.fixture.ts.net:3787"), "POST", "/bots/pair"), "False")
    C.Check.str("different port denied", allowed(True{}, hs("hub.fixture.ts.net:3788"), "POST", "/bots/pair"), "False")
    C.Check.str("malformed port denied", allowed(True{}, hs("hub.fixture.ts.net:3787x"), "POST", "/bots/pair"), "False")
    C.Check.str("missing host denied", allowed(True{}, Nil{}, "POST", "/bots/pair"), "False")
    C.Check.str("empty host denied", allowed(True{}, hs(""), "POST", "/bots/pair"), "False")
    C.Check.str("duplicate Host denied", allowed(True{}, [H.Header{"host", "100.64.0.8:3787"}, H.Header{"Host", "attacker.invalid"}], "POST", "/bots/pair"), "False")
    C.Check.str("empty alias list cannot grant", Bool.show(A.Auth.owner.pair(True{}, Nil{}, hs("100.64.0.8:3787"), "POST", "/bots/pair")), "False")
    C.Check.str("empty aliases cannot match empty Host", Bool.show(A.Auth.owner.pair(True{}, [""], hs(""), "POST", "/bots/pair")), "False")
    C.Check.str("loopback listener has no owner alias grant", Bool.show(A.Auth.owner.pair(True{}, Nil{}, hs("localhost:3787"), "POST", "/bots/pair")), "False")
    C.Check.str("foreign browser Origin denied", allowed(True{}, [H.Header{"host", "100.64.0.8:3787"}, H.Header{"origin", "https://attacker.invalid"}], "POST", "/bots/pair"), "False")
    C.Check.str("same-origin browser still needs existing credentials", allowed(True{}, [H.Header{"host", "100.64.0.8:3787"}, H.Header{"origin", "http://100.64.0.8:3787"}], "POST", "/bots/pair"), "False")
    C.Check.str("cross-site metadata denied", allowed(True{}, [H.Header{"host", "100.64.0.8:3787"}, H.Header{"sec-fetch-site", "cross-site"}], "POST", "/bots/pair"), "False")
    C.Check.str("proxy asserted identity cannot grant", allowed(True{}, [H.Header{"host", "100.64.0.8:3787"}, H.Header{"tailscale-user-login", "owner"}], "POST", "/bots/pair"), "False")
    C.Check.str("GET cannot pair", allowed(True{}, hs("100.64.0.8:3787"), "GET", "/bots/pair"), "False")
    C.Check.str("websocket unaffected", allowed(True{}, hs("100.64.0.8:3787"), "POST", "/ws"), "False")
    C.Check.str("MCP unaffected", allowed(True{}, hs("100.64.0.8:3787"), "POST", "/mcp/t1"), "False")
    C.Check.str("lock route unaffected", allowed(True{}, hs("100.64.0.8:3787"), "POST", "/lock/acquire"), "False")
    C.Check.str("OAuth unaffected", allowed(True{}, hs("100.64.0.8:3787"), "POST", "/oauth/google"), "False")
    C.Check.str("pair prefix cannot grant", allowed(True{}, hs("100.64.0.8:3787"), "POST", "/bots/pair-extra"), "False")
    C.Check.str("general auth still denies unpaired tailnet request", Bool.show(A.Auth.allowed(hs("100.64.0.8:3787"), "/debug/perf", "AUTH-FIXTURE")), "False")
