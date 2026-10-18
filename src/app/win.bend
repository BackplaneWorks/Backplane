import Base

# The app's window (effects/win.c): resizable, with text, wheel, resize
# and clipboard events. Frames are Base's Image quadtree.

# mods: 1 shift, 2 ctrl, 4 alt, 8 super. code is the X keysym; text is the
# code point the key typed (0 for none). Wheel dir: 0 up, 1 down, 2 left,
# 3 right. A key press or release also says its keycode (the physical key).
type WinEv is Data:
  WKey{code: U32, text: U32, mods: U32, down: Bool, kc: U32}
  WButton{x: U32, y: U32, button: U32, down: Bool}
  WMove{x: U32, y: U32, mods: U32}
  WWheel{x: U32, y: U32, dir: U32}
  WSize{w: U32, h: U32}
  WExpose{}
  WQuit{}
  WPaste{text: String}
  WImage{}
  WDragOver{x: U32, y: U32}
  WDragLeave{}
  WDrop{text: String}
  # the window lost the keyboard (keys held then are let go unseen)
  WBlur{}
  # the window has the keyboard again
  WFocus{}
  # text the input method committed (one event owns its own text)
  WCommit{id: U32, text: String}
  # the preedit: characters [first, first + len) replaced by text (feedback
  # one char per char, '0' + bits); only: just the feedback changed; caret
  # in the preedit after the update
  WPreedit{first: U32, len: U32, id: U32, text: String, fb: String, caret: U32, only: Bool}
  WPreCaret{pos: U32}
  WPreStart{}
  WPreEnd{}
  # an event kind this build does not know (never a quit)
  WJunk{}

def Win.open(title: String, w: U32, h: U32) -> IO(Result<&1, &1, U32 & String, Window>):
  import "./effects/win.c"
  import "./effects/win.js"

# the events since the last call, five words each; parks until there is
# at least one (Win.decode reads them)
def Win.words(win: Window) -> IO(Window & List<&2, U32>):
  import "./effects/win.c"
  import "./effects/win.js"

# show a frame; the image comes back so the caller can free it
def Win.present(win: Window, image: Image) -> IO(Window & Image):
  import "./effects/win.c"
  import "./effects/win.js"

# show the window's own frame; prev is the last frame it showed this way
# (regions that are the same in both are not filled again). Both images
# come back so the caller can free them
def Win.show(win: Window, image: Image, prev: Image) -> IO(Window & Image & Image):
  import "./effects/win.c"
  import "./effects/win.js"

def Win.copy(win: Window, text: String) -> IO(Window):
  import "./effects/win.c"
  import "./effects/win.js"

# request the clipboard; it arrives as a WPaste event
def Win.paste(win: Window) -> IO(Window):
  import "./effects/win.c"
  import "./effects/win.js"

def Win.pasted(win: Window) -> IO(Window & String):
  import "./effects/win.c"
  import "./effects/win.js"

# MIME, byte count, and ASCII base64 of the last WImage
def Win.pasted_image(win: Window) -> IO(Window & String & U32 & String):
  import "./effects/win.c"
  import "./effects/win.js"

# the text/uri-list of the last WDrop (files dragged in from outside)
def Win.dropped(win: Window) -> IO(Window & String):
  import "./effects/win.c"
  import "./effects/win.js"

def Win.size(win: Window) -> IO(Window & U32 & U32):
  import "./effects/win.c"
  import "./effects/win.js"

# a second handle: one computation waits on events, another presents
def Win.twin(win: Window) -> IO(Window & Window):
  import "./effects/win.c"
  import "./effects/win.js"

def Win.title(win: Window, title: String) -> IO(Window):
  import "./effects/win.c"
  import "./effects/win.js"

# how the input method is used: 0 plain keys, 1 preedit drawn by the app,
# 2 drawn by the method
def Win.ime(win: Window) -> IO(Window & U32):
  import "./effects/win.c"
  import "./effects/win.js"

# drop a composition in the method (the field with the keys changed)
def Win.ime_reset(win: Window) -> IO(Window):
  import "./effects/win.c"
  import "./effects/win.js"

# the caret's place in the window, for the method's candidate window
def Win.ime_spot(win: Window, x: U32, y: U32) -> IO(Window):
  import "./effects/win.c"
  import "./effects/win.js"

# the text of a WCommit or WPreedit (by its id, once) and its feedback
def Win.payload(win: Window, id: U32) -> IO(Window & String & String):
  import "./effects/win.c"
  import "./effects/win.js"

# queue what the method would, for tests: kind 0 a commit, 1 a preedit,
# 2 the preedit done
def Win.ime_inject(win: Window, kind: U32, text: String) -> IO(Window):
  import "./effects/win.c"
  import "./effects/win.js"

def Win.ev.k(k: Nat, +a: U32, +b: U32, +c: U32, +d: U32) -> WinEv:
  match k:
    case 0n:
      WKey{a, b, c, U32.is_ne(U32.and(d, 1), 0), U32.shrn(d, 1n)}
    case 1n:
      WButton{a, b, c, U32.is_ne(d, 0)}
    case 2n:
      WMove{a, b, c}
    case 3n:
      WWheel{a, b, c}
    case 4n:
      WSize{a, b}
    case 5n:
      WExpose{}
    case 7n:
      WPaste{""}
    case 8n:
      WDragOver{a, b}
    case 9n:
      WDragLeave{}
    case 10n:
      WDrop{""}
    case 11n:
      WBlur{}
    case 12n:
      WFocus{}
    case 13n:
      WImage{}
    case 14n:
      WCommit{a, ""}
    case 15n:
      WPreedit{a, b, c, "", "", U32.and(d, 2147483647), U32.is_ne(U32.shrn(d, 31n), 0)}
    case 6n:
      WQuit{}
    case 16n:
      WPreCaret{a}
    case 17n:
      WPreStart{}
    case 18n:
      WPreEnd{}
    case _:
      WJunk{}

def Win.ev(+k: U32, +a: U32, +b: U32, +c: U32, +d: U32) -> WinEv:
  Win.ev.k(U32.to_nat(k), a, b, c, d)

# five words per event
def Win.decode(ws: List<&2, U32>) -> List<&2, WinEv>:
  match ws:
    case +k <> +a <> +b <> +c <> +d <> t:
      Win.ev(k, a, b, c, d) <> Win.decode(t)
    case _:
      Nil{}

# an event kind this build does not know
def Win.junk(e: WinEv) -> Bool:
  match e:
    case WJunk{}:
      True{}
    case _:
      False{}

def Win.quit(e: WinEv) -> Bool:
  match e:
    case WQuit{}:
      True{}
    case _:
      False{}

# X11 parent id for the helper's native content children.
def Win.xid(win: Window) -> IO(Window & U32):
  import "./effects/win.c"
  import "./effects/win.js"
