import Base
import ./http.bend as H
import ./utf8.bend as U8
import ./sha256.bend as SH

# a token compared in constant time (sha256.bend's Ct.eq): the time a
# compare takes says nothing of how much of a guess was right
def Auth.same(+a: String, +b: String) -> Bool:
  Bool.and(Nat.is_eq(List.length(&2, U32, U8.Utf8.encode(a)), List.length(&2, U32, U8.Utf8.encode(b))), SH.Ct.eq(U8.Utf8.encode(a), U8.Utf8.encode(b)))

# Credentials are complete field values, never substrings of another field.
def Auth.cookie.go(xs: List<&2, String>, +token: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +x <> t:
      Bool.or(Auth.same(String.trim(x), "bp_token=" ++ token), Auth.cookie.go(t, token))

def Auth.credential(+hs: List<&2, H.Header>, +path: String, +token: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(token)), Bool.or(
    Auth.same(H.Http.header_of(hs, "authorization"), "Bearer " ++ token), Bool.or(
      Auth.same(H.Http.param(path, "token"), token),
      Auth.cookie.go(String.split(H.Http.header_of(hs, "cookie"), ';'), token))))

# Browsers serialize Origin as scheme://authority, with no path or userinfo.
# An absent Origin supports native clients; Fetch Metadata still rejects sites.
def Auth.origin(+origin: String, +host: String) -> Bool:
  Bool.or(String.is_empty(origin), Bool.and(Bool.not(String.is_empty(host)), Bool.or(
    String.eq(String.to_lower(origin), "http://" ++ String.to_lower(host)),
    String.eq(String.to_lower(origin), "https://" ++ String.to_lower(host)))))

def Auth.browser(+hs: List<&2, H.Header>) -> Bool:
  Bool.and(Auth.origin(H.Http.header_of(hs, "origin"), H.Http.header_of(hs, "host")),
    Bool.not(String.eq(String.to_lower(H.Http.header_of(hs, "sec-fetch-site")), "cross-site")))

# Native machine auto-pairing has a separate, route-only trust path.
# These authorities come from this listener's OWN tailnet address and
# Self.DNSName, never the request, a peer advertisement or proxy headers.
def Auth.owner.hosts.one(empty: Bool, +name: String, +port: String) -> List<&2, String>:
  match empty:
    case True{}: Nil{}
    case False{}: [String.to_lower(name), String.to_lower(name) ++ ":" ++ port]

def Auth.owner.hosts(+ip: String, +dns: String, +port: String) -> List<&2, String>:
  List.append(&2, String, Auth.owner.hosts.one(String.is_empty(ip), ip, port), Auth.owner.hosts.one(String.is_empty(dns), dns, port))

def Auth.owner.host(xs: List<&2, String>, +host: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +x <> rest:
      Bool.or(Bool.and(Bool.not(String.is_empty(x)), String.eq(x, host)), Auth.owner.host(rest, host))

# Duplicate authority fields are ambiguous and cannot grant owner trust.
def Auth.owner.header_count(hs: List<&2, H.Header>, +name: String) -> Nat:
  match hs:
    case Nil{}: 0n
    case H.Header{k, _} <> rest:
      Nat.add(Bool.pick(Nat, String.eq(String.to_lower(k), name), 1n, 0n), Auth.owner.header_count(rest, name))

# trusted here is the socket peer's whois-verified owner identity. Only a
# native POST /bots/pair may use it, only to an exact own authority, with
# no browser Origin or proxy identity. Other routes keep their own
# token/signature gates, including locks, OAuth and MCP.
def Auth.owner.pair(+trusted: Bool, +own: List<&2, String>, +hs: List<&2, H.Header>, +method: String, +path: String) -> Bool:
  Bool.and(trusted, Bool.and(Nat.is_eq(Auth.owner.header_count(hs, "host"), 1n), Bool.and(String.eq(method, "POST"), Bool.and(String.eq(H.Http.route(path), "/bots/pair"),
    Bool.and(Auth.browser(hs), Bool.and(String.is_empty(H.Http.header_of(hs, "origin")),
      Bool.and(String.is_empty(H.Http.header_of(hs, "tailscale-user-login")),
        Auth.owner.host(own, String.to_lower(H.Http.header_of(hs, "host"))))))))))

# The same whois-verified owner identity on a linked machine's signed
# request (its signature is still checked): with no browser Origin or proxy
# identity, to an exact own authority, it says the link is one of the
# owner's own machines (server.bend's Hub.bot.vouched). It grants nothing
# else: the request still needs a live link's signature.
def Auth.owner.peer(+trusted: Bool, +own: List<&2, String>, +hs: List<&2, H.Header>) -> Bool:
  Bool.and(trusted, Bool.and(Nat.is_eq(Auth.owner.header_count(hs, "host"), 1n), Bool.and(String.is_empty(H.Http.header_of(hs, "origin")),
    Bool.and(String.is_empty(H.Http.header_of(hs, "tailscale-user-login")), Auth.owner.host(own, String.to_lower(H.Http.header_of(hs, "host")))))))

# Two hubs that name each other in BACKPLANE_PEERS (one machine, or a LAN
# without Tailscale) have no credential of each other's before they pair.
# The asking hub sends, with POST /bots/pair, a proof made from its own
# token: an HMAC of the address it asked and the time. The asked hub does
# not take the proof's word: it calls the asker back at the address its
# own machines list has for it (POST /bots/pair/check), and only the hub
# holding that token can say the proof is its own. The token never
# leaves the asker, and a proof is good for Auth.pair.window() seconds.
def Auth.pair.proof(+token: String, +to: String, +at: Nat) -> String:
  SH.Hex.encode(SH.Hmac.sha256(U8.Utf8.encode(token), U8.Utf8.encode("bots-pair\n" ++ to ++ "\n" ++ Nat.show(at))))

def Auth.pair.window() -> Nat:
  60n

def Auth.pair.fresh(+now: Nat, +at: Nat) -> Bool:
  Nat.is_le(Nat.add(Nat.sub(now, at), Nat.sub(at, now)), Auth.pair.window())

# whether this hub (token) made the proof, for an ask to `to` at `at`, and
# it is still fresh at now
def Auth.pair.checked(+token: String, +to: String, +at: Nat, +now: Nat, +proof: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(token)), Bool.and(Bool.not(String.is_empty(proof)),
    Bool.and(Auth.pair.fresh(now, at), Auth.same(proof, Auth.pair.proof(token, to, at)))))

# A Host naming this machine (DNS rebinding cannot fake one in a browser).
def Auth.local.host(+host: String) -> Bool:
  Bool.or(String.eq(host, "localhost"), Bool.or(String.starts_with(host, "localhost:"),
    Bool.or(String.eq(host, "127.0.0.1"), Bool.or(String.starts_with(host, "127.0.0.1:"),
      Bool.or(String.eq(host, "[::1]"), String.starts_with(host, "[::1]:"))))))

# a request from this machine (trusted: a loopback socket), not proxied by
# tailscale serve: where the OAuth callback and the locks' helper come from.
# Never a credential by itself.
def Auth.trusted(+trusted: Bool, +hs: List<&2, H.Header>) -> Bool:
  Bool.and(trusted, Bool.and(String.is_empty(H.Http.header_of(hs, "tailscale-user-login")),
    Bool.or(Auth.local.host(String.to_lower(H.Http.header_of(hs, "host"))),
      Bool.and(String.is_empty(H.Http.header_of(hs, "host")), String.is_empty(H.Http.header_of(hs, "origin"))))))

# Every client presents the pairing token or a paired device's credential,
# from this machine too: a loopback socket or a Host naming this machine is
# not a credential. Programs on this machine read the token from the home
# (<home>/token), as the desktop app does.
def Auth.allowed(+hs: List<&2, H.Header>, +path: String, +token: String) -> Bool:
  Bool.and(Auth.browser(hs), Auth.credential(hs, path, token))

# An agent's MCP endpoint (/mcp/<seg>, server.bend's Hub.mcp_url) takes a
# key of its own instead of the token: the token's HMAC of the segment
# (the thread, and the process's generation), so the token stays out of
# agents' arguments and a key opens that one thread's tools
def Auth.mcp.key(+token: String, +seg: String) -> String:
  SH.Hex.encode(SH.Hmac.sha256(U8.Utf8.encode(token), U8.Utf8.encode("mcp\n" ++ seg)))

def Auth.mcp.keyed(+path: String, +token: String, +seg: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(token)), Auth.same(H.Http.param(path, "k"), Auth.mcp.key(token, seg)))

def Auth.mcp(+hs: List<&2, H.Header>, +path: String, +token: String, +seg: String) -> Bool:
  Bool.or(Auth.allowed(hs, path, token), Bool.and(Auth.browser(hs), Auth.mcp.keyed(path, token, seg)))

# Google's browser redirect is a cross-site document navigation. It has
# neither the native pairing token nor a SameSite=Strict pairing cookie.
# Only an unproxied loopback callback may reach the pending PKCE/state
# check this way; ordinary HTTP, websocket, upload and MCP gates stay as-is.
# A browser that sends no Fetch Metadata at all (an older one) is a plain
# document request too.
def Auth.oauth.document(+hs: List<&2, H.Header>) -> Bool:
  Bool.or(Bool.and(String.eq(String.to_lower(H.Http.header_of(hs, "sec-fetch-mode")), "navigate"),
      String.eq(String.to_lower(H.Http.header_of(hs, "sec-fetch-dest")), "document")),
    Bool.and(String.is_empty(H.Http.header_of(hs, "sec-fetch-mode")), Bool.and(String.is_empty(H.Http.header_of(hs, "sec-fetch-dest")),
      String.is_empty(H.Http.header_of(hs, "sec-fetch-site")))))

def Auth.oauth.navigation(+trusted: Bool, +hs: List<&2, H.Header>) -> Bool:
  Bool.and(String.is_empty(H.Http.header_of(hs, "origin")), Bool.and(
    Auth.oauth.document(hs), Bool.and(
    Auth.local.host(String.to_lower(H.Http.header_of(hs, "host"))),
    Auth.trusted(trusted, hs))))

def Auth.oauth(+trusted: Bool, +hs: List<&2, H.Header>, +method: String, +path: String, +token: String) -> Bool:
  Bool.and(String.eq(method, "GET"), Bool.and(String.eq(H.Http.route(path), "/oauth/google"),
    Bool.or(Auth.allowed(hs, path, token), Auth.oauth.navigation(trusted, hs))))

# Pairing never inherits the localhost exemption; it proves the shared secret.
def Auth.pair(+hs: List<&2, H.Header>, +token: String) -> Bool:
  Bool.and(Auth.browser(hs), Bool.and(Bool.not(String.is_empty(token)),
    Auth.same(H.Http.header_of(hs, "authorization"), "Bearer " ++ token)))

def Auth.cookie(+token: String, https: Bool) -> String:
  "Set-Cookie: bp_token=" ++ token ++ "; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000" ++ Bool.pick(String, https, "; Secure", "") ++ "\r\n"

# Readable cache identity is never a bearer credential or an MCP key. It is
# scoped to this pairing secret and a fixed, separate HMAC domain.
def Auth.cache.key.nonempty(empty: Bool, token: String) -> String:
  match empty:
    case True{}: ""
    case False{}: SH.Hex.encode(SH.Hmac.sha256(U8.Utf8.encode(token), U8.Utf8.encode("ui-cache\nv1")))

def Auth.cache_key(+token: String) -> String:
  Auth.cache.key.nonempty(String.is_empty(token), token)

def Auth.cache.cookie.nonempty(empty: Bool, token: String, https: Bool) -> String:
  match empty:
    case True{}: ""
    case False{}:
      "Set-Cookie: bp_cache_v1=" ++ Auth.cache_key(token) ++ "; Path=/; SameSite=Strict; Max-Age=31536000" ++ Bool.pick(String, https, "; Secure", "") ++ "\r\n"

def Auth.cache_cookie(+token: String, https: Bool) -> String:
  Auth.cache.cookie.nonempty(String.is_empty(token), token, https)

def Auth.pair.cookies(+token: String, +https: Bool) -> String:
  Auth.cookie(token, https) ++ Auth.cache_cookie(token, https)

# Existing paired browsers can renew cache identity without reading their
# HttpOnly credential. No legacy trust or query-token authentication here.
def Auth.cache_request(+method: String, +hs: List<&2, H.Header>, +token: String) -> Bool:
  Bool.and(String.eq(method, "GET"), Bool.and(Auth.browser(hs), Bool.and(Bool.not(String.is_empty(token)),
    Bool.or(Auth.same(H.Http.header_of(hs, "authorization"), "Bearer " ++ token),
      Auth.cookie.go(String.split(H.Http.header_of(hs, "cookie"), ';'), token)))))
