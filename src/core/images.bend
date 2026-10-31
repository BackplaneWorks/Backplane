import Base
import ./fm.bend as FM
import ./utf8.bend as U

# Images in a thread. An agent shows the user an image by writing its path
# (absolute, or under ~/) in a reply, bare, in backticks or as a markdown
# image; both clients draw each such image under the message. What counts
# as an image is decided here, once, for both.

def Img.exts() -> List<&2, String>:
  [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"]

def Img.ends(xs: List<&2, String>, +p: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.ends_with(p, x), Img.ends(t, p))

# a path an image can be shown from: absolute or ~/, with an image extension
def Img.is_image(+p: String) -> Bool:
  Bool.and(Bool.or(String.starts_with(p, "/"), String.starts_with(p, "~/")), Img.ends(Img.exts(), String.to_lower(p)))

# a message's words, cut at spaces, brackets, quotes and punctuation, in
# one tail-recursive walk: String.split recurses once per character, and a
# long reply naming an image overflowed the phones' JS stack, which lost
# the hub's whole frame (a message sent after it never showed). A word
# longer than Img.long() is no path: it is dropped, so nothing per word
# walks more than that.
def Img.long() -> Nat:
  1024n

def Img.seps() -> List<&2, U32>:
  [32, 10, 9, 13, 40, 41, 91, 93, 60, 62, 96, 34, 39, 44, 59, 124]

def Img.sep.in(xs: List<&2, U32>, +c: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(U32.is_eq(x, c), Img.sep.in(t, c))

type IW is Data:
  IW{n: Nat, cur: String, acc: List<&2, String>}

def Img.push.k(keep: Bool, cur: String, acc: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      String.reverse(cur) <> acc
    case False{}:
      acc

def Img.push(+n: Nat, cur: String, acc: List<&2, String>) -> List<&2, String>:
  Img.push.k(Bool.and(Nat.is_gt(n, 0n), Nat.is_le(n, Img.long())), cur, acc)

def Img.add(long: Bool, +c: U32, cur: String) -> String:
  match long:
    case True{}:
      cur
    case False{}:
      SCon{Chr{c}, cur}

def Img.step(sep: Bool, +c: U32, st: IW) -> IW:
  match sep st:
    case True{} IW{+n, cur, acc}:
      IW{0n, SNil{}, Img.push(n, cur, acc)}
    case False{} IW{+n, cur, acc}:
      IW{Nat.add(n, 1n), Img.add(Nat.is_ge(n, Img.long()), c, cur), acc}

def Img.words.fin(st: IW) -> List<&2, String>:
  match st:
    case IW{+n, cur, acc}:
      List.reverse(&2, String, Img.push(n, cur, acc))

def Img.words.go(s: String, st: IW) -> List<&2, String>:
  match s:
    case SNil{}:
      Img.words.fin(st)
    case SCon{Chr{+c}, t}:
      Img.words.go(t, Img.step(Img.sep.in(Img.seps(), c), c, st))

def Img.words(s: String) -> List<&2, String>:
  Img.words.go(s, IW{0n, SNil{}, Nil{}})

# a path ends a sentence or a clause sometimes: drop what follows it
def Img.trim(+p: String) -> String:
  Bool.pick(String, Bool.or(Bool.or(String.ends_with(p, "."), String.ends_with(p, ":")), Bool.or(String.ends_with(p, "!"), String.ends_with(p, "?"))),
    String.take(p, Nat.sub(String.length(p), 1n)), p)

def Img.bare(+p: String) -> String:
  Img.trim(Bool.pick(String, String.starts_with(p, "file://"), String.drop(p, 7n), p))

def Img.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Img.has(t, x))

def Img.keep(ok: Bool, +p: String, acc: List<&2, String>) -> List<&2, String>:
  match ok:
    case True{}:
      p <> acc
    case False{}:
      acc

def Img.pick(ws: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +w <> t:
      Img.pick(t, Img.keep(Bool.and(Img.is_image(Img.bare(w)), Bool.not(Img.has(acc, Img.bare(w)))), Img.bare(w), acc))

def Img.starts(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.starts_with(s, x), Img.starts(t, s))

# after a dot: whether an image extension follows, any case (only its
# few characters are lowered)
def Img.dot(dot: Bool, +t: String) -> Bool:
  match dot:
    case True{}:
      Img.starts(Img.exts(), "." ++ String.to_lower(String.take(t, 4n)))
    case False{}:
      False{}

def Img.scan(s: String, found: Bool) -> Bool:
  match s found:
    case _ True{}:
      True{}
    case SNil{} False{}:
      False{}
    case SCon{Chr{+c}, +t} False{}:
      Img.scan(t, Img.dot(U32.is_eq(c, 46), t))

# whether a text may name an image: it holds an image extension, any
# case. Every path Img.paths finds ends in one, so a text without is
# skipped with a cheap search (a phone renders each reply on every screen).
# One tail-recursive walk: lowering the whole text (String.to_lower
# recurses once per character) overflowed the phones' V8 on a long message
def Img.may(+s: String) -> Bool:
  Img.scan(s, False{})

# the images a message shows, in order, each once
def Img.paths(+text: String) -> List<&2, String>:
  Img.pick(Img.words(text), Nil{})

def Img.hex(+n: U32) -> Char:
  Chr{U32.add(n, Bool.pick(U32, U32.is_lt(n, 10), 48, 55))}

def Img.safe(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(Bool.or(U32.is_eq(c, 47), U32.is_eq(c, 46)), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)))))

def Img.enc.k(ok: Bool, +c: U32, rest: String) -> String:
  match ok:
    case True{}:
      SCon{Chr{c}, rest}
    case False{}:
      SCon{'%', SCon{Img.hex(U32.shrn(c, 4n)), SCon{Img.hex(U32.and(c, 15)), rest}}}

def Img.enc.bytes(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      SNil{}
    case +b <> t:
      Img.enc.k(Img.safe(b), b, Img.enc.bytes(t))

# the URL a hub serves an image at (UTF-8, percent-encoded)
def Img.url(+p: String) -> String:
  "/img?path=" ++ Img.enc.bytes(U.Utf8.encode(p))

def Img.ctype.k(+p: String) -> String:
  Bool.pick(String, String.eq(FM.Fm.preview(p), "text"), "text/plain; charset=utf-8",
  Bool.pick(String, String.ends_with(p, ".pdf"), "application/pdf",
  Bool.pick(String, String.ends_with(p, ".png"), "image/png",
    Bool.pick(String, Bool.or(String.ends_with(p, ".jpg"), String.ends_with(p, ".jpeg")), "image/jpeg",
      Bool.pick(String, String.ends_with(p, ".gif"), "image/gif",
        Bool.pick(String, String.ends_with(p, ".webp"), "image/webp", "image/bmp"))))))

def Img.ctype(+p: String) -> String:
  Img.ctype.k(String.to_lower(p))

# what a hub serves from /img: an image, PDF or text note for Files
# (core/fm.bend), at a path it can be shown from
def Img.viewable(+p: String) -> Bool:
  Bool.or(Img.is_image(p), Bool.or(String.eq(FM.Fm.preview(p), "text"), Bool.and(Bool.or(String.starts_with(p, "/"), String.starts_with(p, "~/")), String.ends_with(String.to_lower(String.drop(p, Nat.sub(String.length(p), 4n))), ".pdf"))))

# the largest image a hub serves (bytes)
def Img.cap() -> U32:
  12582912
