import Base
import ./check.bend as C
import ../src/core/auth.bend as A
import ../src/core/http.bend as H

def yes(x: Bool) -> String:
  Bool.pick(String, x, "yes", "no")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("cache tag exact independent HMAC fixture", A.Auth.cache_key("secret"), "13c9ba2208a9d5e8e75883c72331ec75e0e6f3d8f5dce5449b305ad9e46fc720")
    C.Check.str("cache tag restart stable", A.Auth.cache_key("secret"), A.Auth.cache_key("secret"))
    C.Check.str("rotated credential changes cache identity", yes(String.eq(A.Auth.cache_key("secret"), A.Auth.cache_key("rotated"))), "no")
    C.Check.str("empty secret has no cache identity", A.Auth.cache_key(""), "")
    C.Check.str("empty secret emits no cache cookie", A.Auth.cache_cookie("", True{}), "")
    C.Check.str("cache tag is full SHA256 hex", Nat.show(String.length(A.Auth.cache_key("secret"))), "64")
    C.Check.str("cache cookie strict secure host scope", A.Auth.cache_cookie("secret", True{}), "Set-Cookie: bp_cache_v1=13c9ba2208a9d5e8e75883c72331ec75e0e6f3d8f5dce5449b305ad9e46fc720; Path=/; SameSite=Strict; Max-Age=31536000; Secure\r\n")
    C.Check.str("local HTTP cache cookie has no Secure flag", A.Auth.cache_cookie("secret", False{}), "Set-Cookie: bp_cache_v1=13c9ba2208a9d5e8e75883c72331ec75e0e6f3d8f5dce5449b305ad9e46fc720; Path=/; SameSite=Strict; Max-Age=31536000\r\n")
    C.Check.str("authentication cookie remains byte exact HttpOnly", A.Auth.cookie("secret", True{}), "Set-Cookie: bp_token=secret; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000; Secure\r\n")
    C.Check.str("pair headers retain original auth cookie first", A.Auth.pair.cookies("secret", True{}), A.Auth.cookie("secret", True{}) ++ A.Auth.cache_cookie("secret", True{}))
    C.Check.str("readable identity is not auth token", yes(A.Auth.allowed(Nil{}, "/ws?token=" ++ A.Auth.cache_key("secret"), "secret")), "no")
    C.Check.str("readable identity is not bearer credential", yes(A.Auth.allowed([H.Header{"authorization", "Bearer " ++ A.Auth.cache_key("secret")}], "/ws", "secret")), "no")
    C.Check.str("readable identity cookie confers no authentication", yes(A.Auth.allowed([H.Header{"cookie", "bp_cache_v1=" ++ A.Auth.cache_key("secret")}], "/ws", "secret")), "no")
    C.Check.str("readable identity cannot pair", yes(A.Auth.pair([H.Header{"authorization", "Bearer " ++ A.Auth.cache_key("secret")}], "secret")), "no")
    C.Check.str("readable identity is not any selected MCP key", yes(A.Auth.mcp(Nil{}, "/mcp/t1.3?k=" ++ A.Auth.cache_key("secret"), "secret", "t1.3")), "no")
    C.Check.str("cache and MCP HMAC domains differ", yes(String.eq(A.Auth.cache_key("secret"), A.Auth.mcp.key("secret", "ui-cache\nv1"))), "no")
    C.Check.str("real bearer pairing still accepted", yes(A.Auth.pair([H.Header{"authorization", "Bearer secret"}], "secret")), "yes")
    C.Check.str("real MCP key still accepted", yes(A.Auth.mcp(Nil{}, "/mcp/t1.3?k=" ++ A.Auth.mcp.key("secret", "t1.3"), "secret", "t1.3")), "yes")
    C.Check.str("cache cookie does not bypass browser origin", yes(A.Auth.allowed([H.Header{"host", "localhost"}, H.Header{"origin", "https://foreign.invalid"}, H.Header{"cookie", "bp_cache_v1=" ++ A.Auth.cache_key("secret")}], "/ws", "secret")), "no")
    C.Check.str("read-only identity accepts existing paired cookie", yes(A.Auth.cache_request("GET", [H.Header{"cookie", "bp_token=secret"}], "secret")), "yes")
    C.Check.str("read-only identity accepts exact bearer", yes(A.Auth.cache_request("GET", [H.Header{"authorization", "Bearer secret"}], "secret")), "yes")
    C.Check.str("read-only identity rejects missing cookie", yes(A.Auth.cache_request("GET", Nil{}, "secret")), "no")
    C.Check.str("read-only identity never inherits legacy loopback", yes(A.Auth.cache_request("GET", [H.Header{"host", "localhost"}], "secret")), "no")
    C.Check.str("read-only identity rejects cache-tag-only cookie", yes(A.Auth.cache_request("GET", [H.Header{"cookie", "bp_cache_v1=" ++ A.Auth.cache_key("secret")}], "secret")), "no")
    C.Check.str("read-only identity rejects expired credential", yes(A.Auth.cache_request("GET", [H.Header{"cookie", "bp_token=old"}], "secret")), "no")
    C.Check.str("read-only identity rejects empty credential", yes(A.Auth.cache_request("GET", [H.Header{"authorization", "Bearer "}], "")), "no")
    C.Check.str("read-only identity is GET-only", yes(A.Auth.cache_request("POST", [H.Header{"cookie", "bp_token=secret"}], "secret")), "no")
    C.Check.str("read-only identity refuses foreign Origin even with cookie", yes(A.Auth.cache_request("GET", [H.Header{"host", "localhost"}, H.Header{"origin", "https://foreign.invalid"}, H.Header{"cookie", "bp_token=secret"}], "secret")), "no")
    C.Check.str("read-only identity refuses cross-site metadata even with cookie", yes(A.Auth.cache_request("GET", [H.Header{"sec-fetch-site", "cross-site"}, H.Header{"cookie", "bp_token=secret"}], "secret")), "no")
    C.Check.str("query authentication cannot reach the header-only identity guard", yes(Bool.and(A.Auth.allowed(Nil{}, "/auth/cache?token=secret", "secret"), Bool.not(A.Auth.cache_request("GET", Nil{}, "secret")))), "yes")
