import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./codec.bend as C

# A notice the window makes for itself (a viewer's state, why a remote
# file or link failed) travels as a stable id and its detail,
# "\u{1}<id>\u{1f}<detail>", and becomes words only where it is shown, in
# that frame's language. Anything else (a hub's or a tool's own words)
# shows as it came.
def Notice.make(+id: String, +detail: String) -> String:
  "\u{1}" ++ id ++ "\u{1f}" ++ detail

def Notice.is(+s: String) -> Bool:
  String.starts_with(s, "\u{1}")

def Notice.id(+s: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(String.drop(s, 1n), '\u{1f}')), "")

def Notice.detail(+s: String) -> String:
  String.drop(s, Nat.add(2n, String.length(Notice.id(s))))

def Notice.ids() -> List<&2, String>:
  ["loading", "exporting", "browser", "noboard", "noschematic", "nomesh", "export", "unread",
   "nosend", "silent", "denied", "nohandshake", "nohost", "keep", "notaken", "badreply", "away", "refused"]

def Notice.words(+l: I.Loc, i: Nat, +d: String) -> String:
  match i:
    case 0n: MSG.T.viewer.loading(l)
    case 1n: MSG.T.viewer.exporting(l)
    case 2n: MSG.T.viewer.browser.empty(l)
    case 3n: MSG.T.viewer.empty.board(l)
    case 4n: MSG.T.viewer.empty.schematic(l)
    case 5n: MSG.T.viewer.mesh.failed(l)
    case 6n: MSG.T.viewer.export.failed(l, d)
    case 7n: MSG.T.viewer.unread(l)
    case 8n: MSG.T.image.remote.failed(l)
    case 9n: MSG.T.remote.timeout(l)
    case 10n: MSG.T.remote.denied(l)
    case 11n: MSG.T.remote.handshake.failed(l)
    case 12n: MSG.T.remote.unreachable(l, d)
    case 13n: MSG.T.remote.cache.failed(l)
    case 14n: MSG.T.remote.request.failed(l)
    case 15n: MSG.T.remote.reply.failed(l)
    case 16n: MSG.T.remote.away(l)
    case 17n: MSG.T.remote.request.error(l, d)
    case _: d

def Notice.coded(code: Bool, +l: I.Loc, +s: String) -> String:
  match code:
    case True{}:
      Notice.words(l, C.Codec.pos(Notice.ids(), Notice.id(s)), Notice.detail(s))
    case False{}:
      s

# what the person reads
def Notice.shown(+l: I.Loc, +s: String) -> String:
  Notice.coded(Notice.is(s), l, s)
