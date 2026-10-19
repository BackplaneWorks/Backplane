import Base
import ./http.bend as H

# Credentials are complete field values, never substrings of another field.
def Auth.cookie.go(xs: List<&2, String>, +token: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +x <> t:
      Bool.or(String.eq(String.trim(x), "bp_token=" ++ token), Auth.cookie.go(t, token))

def Auth.credential(+hs: List<&2, H.Header>, +path: String, +token: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(token)), Bool.or(
    String.eq(H.Http.header_of(hs, "authorization"), "Bearer " ++ token), Bool.or(
      String.eq(H.Http.param(path, "token"), token),
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

# A browser cannot use DNS rebinding to inherit a localhost exemption.
def Auth.local.host(+host: String) -> Bool:
  Bool.or(String.eq(host, "localhost"), Bool.or(String.starts_with(host, "localhost:"),
    Bool.or(String.eq(host, "127.0.0.1"), Bool.or(String.starts_with(host, "127.0.0.1:"),
      Bool.or(String.eq(host, "[::1]"), String.starts_with(host, "[::1]:"))))))

def Auth.trusted(+trusted: Bool, +hs: List<&2, H.Header>) -> Bool:
  Bool.and(trusted, Bool.and(String.is_empty(H.Http.header_of(hs, "tailscale-user-login")),
    Bool.or(Auth.local.host(String.to_lower(H.Http.header_of(hs, "host"))),
      Bool.and(String.is_empty(H.Http.header_of(hs, "host")), String.is_empty(H.Http.header_of(hs, "origin"))))))

def Auth.allowed(+trusted: Bool, +hs: List<&2, H.Header>, +path: String, +token: String) -> Bool:
  Bool.and(Auth.browser(hs), Bool.or(Auth.trusted(trusted, hs), Auth.credential(hs, path, token)))

# Pairing never inherits the localhost exemption; it proves the shared secret.
def Auth.pair(+hs: List<&2, H.Header>, +token: String) -> Bool:
  Bool.and(Auth.browser(hs), Bool.and(Bool.not(String.is_empty(token)),
    String.eq(H.Http.header_of(hs, "authorization"), "Bearer " ++ token)))

def Auth.cookie(+token: String, https: Bool) -> String:
  "Set-Cookie: bp_token=" ++ token ++ "; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000" ++ Bool.pick(String, https, "; Secure", "") ++ "\r\n"
