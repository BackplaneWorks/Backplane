import Base
import ../core/http.bend as W
import ./host.bend as H

# The existing static-route policy, kept apart from byte transport. The
# server calls these same defs before opening a file; nothing here chooses
# a view or alters canonical model state.
def Static.safe(+p: String) -> Bool:
  Bool.and(Bool.not(String.contains(p, "..")), String.starts_with(p, "/"))

def Static.file(+p: String) -> String:
  Bool.pick(String, Bool.or(String.eq(p, "/"), Bool.or(String.eq(p, "/pair"), String.eq(p, "/devices"))), "/index.html", p)

def Static.cache(+path: String) -> String:
  Bool.pick(String, String.starts_with(path, "/chunk-"), "public, max-age=31536000, immutable", "no-cache")

def Static.frames(+path: String) -> String:
  Bool.pick(String, String.starts_with(W.Http.ctype(path), "text/html"), "X-Frame-Options: DENY\r\nContent-Security-Policy: frame-ancestors 'none'\r\n", "")

def Static.sent(r: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Socket):
  (s, _) = r
  IO.pure(Socket, s)

# Path policy and the encoding preference (H.Static.coding) still come from
# the existing server route. The host opens one candidate (.br, .gz, plain),
# writes the original headers and streams bytes.
def Static.send(+coding: U32, s: Socket, +web: String, +path: String) -> IO(Unit):
  do IO<Unit>:
    sent : Socket & Result<&1, &1, U32 & String, Unit> <- H.Sock.send_static(s,
      H.StaticFile{web ++ path, W.Http.ctype(path), Static.cache(path), Static.frames(path), coding, H.Static.piece()})
    s2 : Socket <- Static.sent(sent)
    Socket.close(s2)
