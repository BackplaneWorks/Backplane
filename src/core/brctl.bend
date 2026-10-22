import Base
import ./json.bend as J

# The agent's browser (tools/browser, Chrome through playwright-core), pure:
# which page ids are real, the strip over a page the person holds, and the
# toolbar's addresses and commands. Nothing here enters the log.

# a page id names a real thread or bot
def Brc.owner(+id: String, thread: Bool, bot: Bool) -> Bool:
  Bool.and(Bool.not(String.is_empty(id)), Bool.or(thread, bot))

# the strip over a page shows while the person holds it (takeover or teach),
# so they can hand it back
def Brc.strip_on(+drive: String) -> Bool:
  Bool.not(String.is_empty(drive))

def Brc.drive_text(+drive: String) -> String:
  Bool.pick(String, String.is_empty(drive), "Agent drives · view only", "You drive")

# Browser controls: URL intent is shared by the window and web clients.
def Brc.url.local(+s: String) -> Bool:
  +host = String.to_lower(Maybe.default(&2, String, List.head(&2, String, String.split(Maybe.default(&2, String, List.head(&2, String, String.split(s, '/')), ""), ':')), ""))
  Bool.or(Bool.or(String.eq(host, "localhost"), String.ends_with(host, ".localhost")),
    Bool.or(String.starts_with(s, "127."), Bool.or(String.starts_with(s, "[::1]"), String.eq(host, "0.0.0.0"))))

def Brc.url.port(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Brc.url.port(t))

def Brc.url.hostport(+s: String) -> Bool:
  +authority = Maybe.default(&2, String, List.head(&2, String, String.split(s, '/')), "")
  +port = Maybe.default(&2, String, List.get(&2, String, String.split(authority, ':'), 1n), "")
  Bool.and(Bool.not(String.is_empty(port)), Brc.url.port(port))

def Brc.url.bare(ok: Bool, +s: String) -> String:
  match ok:
    case True{}: Bool.pick(String, Bool.or(Brc.url.local(s), Brc.url.hostport(s)), "http://", "https://") ++ s
    case False{}: ""

def Brc.url.explicit(ok: Bool, +s: String) -> String:
  match ok:
    case True{}: s
    case False{}: Brc.url.bare(Bool.and(Bool.not(String.is_empty(s)), Bool.and(Bool.not(String.contains(s, " ")),
      Bool.or(Bool.not(String.contains(s, ":")), Bool.or(Brc.url.local(s), Brc.url.hostport(s))))), s)

def Brc.url(+s: String) -> String:
  +t = String.trim(s)
  +lower = String.to_lower(t)
  Brc.url.explicit(Bool.or(Bool.or(String.starts_with(lower, "http://"), String.starts_with(lower, "https://")),
    Bool.or(String.eq(t, "about:blank"), String.eq(t, "about:credits"))), t)

def Brc.toolbar.allowed(+op: String) -> Bool:
  Bool.or(Bool.or(String.eq(op, "navigate"), String.eq(op, "back")), Bool.or(Bool.or(String.eq(op, "forward"), String.eq(op, "reload")), String.eq(op, "tabs")))

def Brc.toolbar.tab.close(+o: J.Json) -> J.Json:
  J.Json.obj([J.KV{"closeId", J.Str{J.Json.get_str(o, "closeId", "")}}])

def Brc.toolbar.tab.select(has: Bool, +o: J.Json) -> J.Json:
  match has:
    case True{}: J.Json.obj([J.KV{"selectId", J.Str{J.Json.get_str(o, "selectId", "")}}])
    case False{}: Brc.toolbar.tab.close(o)

def Brc.toolbar.tab(new: Bool, +o: J.Json) -> J.Json:
  match new:
    case True{}: J.Json.obj([J.KV{"new", J.Flag{True{}}}])
    case False{}: Brc.toolbar.tab.select(Bool.not(String.is_empty(J.Json.get_str(o, "selectId", ""))), o)

def Brc.toolbar.args.k(nav: Bool, tabs: Bool, +o: J.Json) -> J.Json:
  match nav tabs:
    case True{} _: J.Json.obj([J.KV{"url", J.Str{Brc.url(J.Json.get_str(o, "url", ""))}}])
    case False{} True{}: Brc.toolbar.tab(J.Json.get_bool(o, "new", False{}), o)
    case _ _: J.Json.obj(Nil{})

def Brc.toolbar.args(+o: J.Json) -> J.Json:
  Brc.toolbar.args.k(String.eq(J.Json.get_str(o, "command", ""), "navigate"), String.eq(J.Json.get_str(o, "command", ""), "tabs"), o)
