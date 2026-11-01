import Base
import ./json.bend as J
import ./utf8.bend as U
import ./diff.bend as DF
import ./range.bend as RG

# Attachments. A file a user attaches to a turn is stored by the server
# under <home>/attachments/<thread>/<key>-<name>; the message then carries
# each one as a trailing line of its own:
#
#   Look at this board
#
#   [attachment: /home/me/.backplane-bend/attachments/t1/c9a1-board.png]
#
# so MessagePosted keeps its shape, every provider sees the paths, and the
# clients draw those lines as chips. Claude also gets images as content
# blocks, Codex as `--image` flags; Grok reads the path (its ACP agent
# reports promptCapabilities.image false), and so does Muse.

# How large a file may be. Every way a file travels is a stream from disk
# to socket and back (POST /attach writes the body straight to the file,
# /img and a far view.get send it a piece at a time, host.js's Sock.save and
# Sock.send_path), and a message carries only the attachment's path, so
# the size costs disk, never the hub's memory or its event log. The setting
# "files.max.mb" (Att.key) picks the limit in MB: Att.sizes on every client,
# 512 by default (Att.mb.default), never under 1 nor over Att.mb.ceiling
# (4000: sizes travel as U32), and Sock.save also refuses a body that
# would leave the disk with under 2 GB free. Laws att_max_*.

def Att.key() -> String:
  "files.max.mb"

def Att.mb.default() -> Nat:
  512n

def Att.mb.ceiling() -> Nat:
  4000n

# the choices Settings offers
def Att.sizes() -> List<&2, String>:
  ["128", "512", "1024", "2048", "4000"]

def Att.mb.clamp(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(n, 0n), 1n, Bool.pick(Nat, Nat.is_gt(n, Att.mb.ceiling()), Att.mb.ceiling(), n))

def Att.mb.of(m: Maybe<&2, Nat>) -> Nat:
  match m:
    case Some{+n}:
      Att.mb.clamp(n)
    case None{}:
      Att.mb.default()

# the limit in MB for the setting's text ("" or anything not a count: the default)
def Att.mb(+v: String) -> Nat:
  Att.mb.of(RG.Rng.num(v))

# the limit in bytes for the setting's text
def Att.max(+v: String) -> Nat:
  Nat.mul(Att.mb(v), 1048576n)

# what a refused file is told
def Att.over(+v: String) -> String:
  "attachments are limited to " ++ Nat.show(Att.mb(v)) ++ " MB"

# the default limit, in bytes (where no setting is at hand)
def Att.cap() -> Nat:
  Att.max("")

# the most any file may be: what the hub serves or sends elsewhere
def Att.ceiling() -> Nat:
  Nat.mul(Att.mb.ceiling(), 1048576n)

# bytes per upload request (the web client sends a file in pieces)
def Att.chunk() -> Nat:
  196608n

# the largest image Claude gets inline (its base64 stays under 5 MB);
# bigger ones it reads by path
def Att.inline_max() -> U32:
  3932160

# Lines
# -----

def Att.line(+path: String) -> String:
  "[attachment: " ++ path ++ "]"

def Att.is(+l: String) -> Bool:
  Bool.and(String.starts_with(String.trim(l), "[attachment: "), String.ends_with(String.trim(l), "]"))

# the path an attachment line names
def Att.path(+l: String) -> String:
  String.trim(String.drop(String.take(String.trim(l), Nat.sub(String.length(String.trim(l)), 1n)), 13n))

type AK is Data:
  AkEnd{}
  AkAtt{}
  AkBlank{}
  AkText{}

def Att.kind.k(att: Bool, blank: Bool) -> AK:
  match att blank:
    case True{} _:
      AkAtt{}
    case False{} True{}:
      AkBlank{}
    case False{} False{}:
      AkText{}

# what the head of a (reversed) line list is
def Att.kind(ls: List<&2, String>) -> AK:
  match ls:
    case Nil{}:
      AkEnd{}
    case +l <> _:
      Att.kind.k(Att.is(l), String.is_empty(String.trim(l)))

type Parts is Data:
  Parts{body: String, paths: List<&2, String>}

# lines kept last first, joined first to last with line breaks, in one
# tail-recursive walk (String.join recursed once per line: a pasted log of
# thousands of lines overflowed the web's stack)
def Att.unlines(rs: List<&2, String>, acc: String, first: Bool) -> String:
  match rs:
    case Nil{}:
      acc
    case +l <> t:
      Att.unlines(t, l ++ Bool.pick(String, first, "", "\n") ++ acc, False{})

# walk the lines from the end: attachment lines (and blank lines between
# them) are the attachments; the rest is the body
def Att.split.go(rs: List<&2, String>, k: AK, paths: List<&2, String>) -> Parts:
  match rs k:
    case Nil{} _:
      Parts{"", paths}
    case +l <> +t AkAtt{}:
      Att.split.go(t, Att.kind(t), Att.path(l) <> paths)
    case _ <> +t AkBlank{}:
      Att.split.go(t, Att.kind(t), paths)
    case +l <> +t _:
      Parts{Att.unlines(l <> t, "", True{}), paths}

# a message's text split into what was written and what was attached
def Att.split(+text: String) -> Parts:
  Att.split.go(List.reverse(&2, String, DF.Diff.lines(text)), Att.kind(List.reverse(&2, String, DF.Diff.lines(text))), Nil{})

def Att.paths.k(p: Parts) -> List<&2, String>:
  match p:
    case Parts{_, ps}:
      ps

def Att.paths(+text: String) -> List<&2, String>:
  Att.paths.k(Att.split(text))

def Att.body.k(p: Parts, +text: String) -> String:
  match p:
    case Parts{b, Nil{}}:
      text
    case Parts{b, _ <> _}:
      b

# the text without its attachment lines (all of it when it has none)
def Att.body(+text: String) -> String:
  Att.body.k(Att.split(text), text)

def Att.lines(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Att.line(p) <> Att.lines(t)

# a message: what was written, then one line per attachment
def Att.join(+text: String, +paths: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, paths), text,
    Bool.pick(String, String.is_empty(String.trim(text)), String.join(Att.lines(paths), "\n"), text ++ "\n\n" ++ String.join(Att.lines(paths), "\n")))

# Names
# -----

def Att.base(+path: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(path, '/')), path)

def Att.label.k(parts: List<&2, String>, +whole: String) -> String:
  match parts:
    case _ <> +a <> +t:
      String.join(a <> t, "-")
    case _:
      whole

# what a chip shows: the file name without the upload key
def Att.label(+path: String) -> String:
  Att.label.k(String.split(Att.base(path), '-'), Att.base(path))

def Att.ext(+path: String) -> String:
  String.to_lower(Maybe.default(&2, String, List.last(&2, String, String.split(Att.base(path), '.')), ""))

def Att.mime(+path: String) -> String:
  +e = Att.ext(path)
  Bool.pick(String, String.eq(e, "png"), "image/png",
    Bool.pick(String, Bool.or(String.eq(e, "jpg"), String.eq(e, "jpeg")), "image/jpeg",
      Bool.pick(String, String.eq(e, "gif"), "image/gif",
        Bool.pick(String, String.eq(e, "webp"), "image/webp", ""))))

def Att.image(+path: String) -> Bool:
  Bool.not(String.is_empty(Att.mime(path)))

def Att.images.k(img: Bool, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match img:
    case True{}:
      p <> rest
    case False{}:
      rest

def Att.images.go(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Att.images.k(Att.image(p), p, Att.images.go(t))

# the attached images of a message
def Att.images(+text: String) -> List<&2, String>:
  Att.images.go(Att.paths(text))

# a file name char that can never leave its directory: letters, digits,
# '.', '_' and '-' (law attach_clean_ok)
def Att.ok(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(U32.is_eq(c, 46), Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 45)))))

def Att.chr(ok: Bool, +c: U32) -> U32:
  match ok:
    case True{}:
      c
    case False{}:
      95

# every other char becomes '_'
def Att.clean(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Att.chr(Att.ok(c), c)}, Att.clean(t)}

def Att.all_ok(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Att.ok(c), Att.all_ok(t))

def Att.safe.k(+s: String) -> String:
  Bool.pick(String, String.is_empty(s), "file", Bool.pick(String, String.starts_with(s, "."), "_" ++ String.drop(s, 1n), s))

# a name safe to store: cleaned, at most 100 chars, never empty, never
# hidden, never "." or ".."
def Att.safe(s: String) -> String:
  Att.safe.k(String.take(Att.clean(s), 100n))

# the stored file name of an upload
def Att.stored(key: String, name: String) -> String:
  Att.safe(key) ++ "-" ++ Att.safe(name)

# a file:// URI (what file managers put on the clipboard) as a path
def Att.hexv(+u: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)), (u - 48 : U32),
    Bool.pick(U32, Bool.and(U32.is_ge(u, 65), U32.is_le(u, 70)), (u - 55 : U32),
      Bool.pick(U32, Bool.and(U32.is_ge(u, 97), U32.is_le(u, 102)), (u - 87 : U32), 16)))

def Att.pct.k(ok: Bool, +hi: U32, +lo: U32, +part: String, rest: List<&2, U32>) -> List<&2, U32>:
  match ok:
    case True{}:
      (hi * 16 + lo : U32) <> U.Utf8.encode_onto(String.drop(part, 2n), rest)
    case False{}:
      37 <> U.Utf8.encode_onto(part, rest)

def Att.pct.one(+part: String, rest: List<&2, U32>) -> List<&2, U32>:
  +hi = Att.hexv(Char.to_u32(Maybe.default(&2, Char, List.head(&2, Char, String.to_list(part)), ' ')))
  +lo = Att.hexv(Char.to_u32(Maybe.default(&2, Char, List.head(&2, Char, String.to_list(String.drop(part, 1n))), ' ')))
  Att.pct.k(Bool.and(U32.is_lt(hi, 16), U32.is_lt(lo, 16)), hi, lo, part, rest)

def Att.pct.go(ps: List<&2, String>) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Att.pct.one(p, Att.pct.go(t))

def Att.pct(ps: List<&2, String>) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      U.Utf8.encode_onto(p, Att.pct.go(t))

def Att.from_uri(s: String) -> String:
  +t = String.trim(s)
  Bool.pick(String, String.starts_with(t, "file://"), U.Utf8.decode(Att.pct(String.split(String.drop(t, 7n), '%'))), t)

def Att.pasted.one(+l: String) -> Bool:
  Bool.or(String.is_empty(l), Bool.or(String.starts_with(l, "file://"), Bool.and(String.starts_with(l, "/"), Att.image(l))))

def Att.pasted.all(ls: List<&2, String>) -> Bool:
  match ls:
    case Nil{}:
      True{}
    case l <> t:
      Bool.and(Att.pasted.one(String.trim(l)), Att.pasted.all(t))

def Att.pasted.keep.k(empty: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      Att.from_uri(l) <> rest

def Att.pasted.keep(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Att.pasted.keep.k(String.is_empty(String.trim(l)), String.trim(l), Att.pasted.keep(t))

# the files a paste names, when every line of it is a file:// URI (a file
# manager's copy) or the absolute path of an image; otherwise none, and the
# paste is text
def Att.pasted(+text: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Att.pasted.all(DF.Diff.lines(text)), Att.pasted.keep(DF.Diff.lines(text)), Nil{})

# Drops
# -----
# A drag from a file manager carries text/uri-list (RFC 2483): a URI per
# line (CRLF), '#' lines are comments. Its file: URIs are the files dropped
# (file:///p, file://host/p, file:/p, percent-encoded); everything else is
# ignored (laws drop_non_file_ignored, drop_uri_list_example).

# a percent-encoded text, decoded (bytes as UTF-8)
def Att.unpct(s: String) -> String:
  U.Utf8.decode(Att.pct(String.split(s, '%')))

# after "file://": the host (empty, localhost, a machine name) goes
def Att.uri.after_host(ps: List<&2, String>) -> String:
  match ps:
    case _ <> +a <> t:
      "/" ++ String.join(a <> t, "/")
    case _:
      ""

# after "file:": the path, still encoded
def Att.uri.local(+r: String) -> String:
  Bool.pick(String, String.starts_with(r, "//"), Att.uri.after_host(String.split(String.drop(r, 2n), '/')), r)

# the path a file: URI names
def Att.uri.path(+l: String) -> String:
  Att.unpct(Att.uri.local(String.drop(l, 5n)))

def Att.uri.put(abs: Bool, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match abs:
    case True{}:
      p <> rest
    case False{}:
      rest

def Att.uri.keep(file: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match file:
    case True{}:
      Att.uri.put(String.starts_with(Att.uri.path(l), "/"), Att.uri.path(l), rest)
    case False{}:
      rest

# one (trimmed) line of a uri-list: a file: URI with an absolute path
# joins the dropped files; any other line is ignored
def Att.uri.one(+l: String, rest: List<&2, String>) -> List<&2, String>:
  Att.uri.keep(String.starts_with(l, "file:"), l, rest)

def Att.uri.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Att.uri.one(String.trim(l), Att.uri.go(t))

# the files a text/uri-list names, in order
def Att.uri_list(s: String) -> List<&2, String>:
  Att.uri.go(String.lines(s))

# where an attachment lives
def Att.dir(+home: String, thread: String) -> String:
  home ++ "/attachments/" ++ Att.safe(thread)

# Base64 (uploads arrive base64; the standard and URL alphabets both read)
# ------

def Att.b64v(+u: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(u, 65), U32.is_le(u, 90)), (u - 65 : U32),
    Bool.pick(U32, Bool.and(U32.is_ge(u, 97), U32.is_le(u, 122)), (u - 71 : U32),
      Bool.pick(U32, Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)), (u + 4 : U32),
        Bool.pick(U32, Bool.or(U32.is_eq(u, 43), U32.is_eq(u, 45)), 62,
          Bool.pick(U32, Bool.or(U32.is_eq(u, 47), U32.is_eq(u, 95)), 63, 64)))))

type B64S is Data:
  B64S{buf: U32, n: U32, out: List<&2, U32>}

def Att.b64.full(full: Bool, +b: U32, +n: U32, out: List<&2, U32>) -> B64S:
  match full:
    case True{}:
      B64S{0, 0, U32.and(b, 255) <> U32.and(U32.shrn(b, 8n), 255) <> U32.and(U32.shrn(b, 16n), 255) <> out}
    case False{}:
      B64S{b, n, out}

def Att.b64.put(skip: Bool, st: B64S, +v: U32) -> B64S:
  match skip st:
    case True{} _:
      st
    case False{} B64S{+b, +n, out}:
      Att.b64.full(U32.is_eq(n, 3), U32.or(U32.shln(b, 6n), v), (n + 1 : U32), out)

def Att.b64.end(st: B64S) -> List<&2, U32>:
  match st:
    case B64S{+b, +n, +out}:
      List.reverse(&2, U32, Bool.pick(List<&2, U32>, U32.is_eq(n, 2), U32.and(U32.shrn(b, 4n), 255) <> out,
        Bool.pick(List<&2, U32>, U32.is_eq(n, 3), U32.and(U32.shrn(b, 2n), 255) <> U32.and(U32.shrn(b, 10n), 255) <> out, out)))

def Att.b64.go(s: String, st: B64S) -> List<&2, U32>:
  match s:
    case SNil{}:
      Att.b64.end(st)
    case SCon{Chr{+c}, t}:
      Att.b64.go(t, Att.b64.put(U32.is_eq(Att.b64v(c), 64), st, Att.b64v(c)))

# the bytes of a base64 text (padding and stray chars are skipped)
def Att.b64(s: String) -> List<&2, U32>:
  Att.b64.go(s, B64S{0, 0, Nil{}})

# Providers
# ---------

type Img is Data:
  Img{mime: String, data: String}

def Att.block(i: Img) -> J.Json:
  match i:
    case Img{m, d}:
      J.Json.obj([J.KV{"type", J.Str{"image"}}, J.KV{"source", J.Json.obj([J.KV{"type", J.Str{"base64"}}, J.KV{"media_type", J.Str{m}}, J.KV{"data", J.Str{d}}])}])

def Att.blocks(xs: List<&2, Img>, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      rest
    case i <> t:
      Att.block(i) <> Att.blocks(t, rest)

# Claude's stream-json user message with its images as content blocks
# (images first, then the text with its attachment lines)
def Att.claude(text: String, imgs: List<&2, Img>) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"type", J.Str{"user"}},
    J.KV{"message", J.Json.obj([
      J.KV{"role", J.Str{"user"}},
      J.KV{"content", J.Json.arr(Att.blocks(imgs, [J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{text}}])]))}])}]))

def Att.codex.go(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      "--image" <> p <> Att.codex.go(t)

# `codex exec` flags for a message's images
def Att.codex(+text: String) -> List<&2, String>:
  Att.codex.go(Att.images(text))

# ui-panel-paste-lag: paste
# ------------------------

def Att.paste.ext(+mime: String) -> String:
  Bool.pick(String, String.eq(mime, "image/png"), "png",
    Bool.pick(String, String.eq(mime, "image/jpeg"), "jpg",
      Bool.pick(String, String.eq(mime, "image/webp"), "webp",
        Bool.pick(String, String.eq(mime, "image/gif"), "gif",
          Bool.pick(String, String.eq(mime, "image/bmp"), "bmp", "bin")))))

def Att.paste.name(mime: String) -> String:
  "pasted." ++ Att.paste.ext(mime)

# A raw chunk is divisible by three, so each base64 piece ends on a
# quartet boundary and the hub can decode it independently.
def Att.paste.width() -> Nat:
  Nat.mul(Nat.div(Att.chunk(), 3n), 4n)

# Fuel is the number of full pieces. The final remainder is always kept,
# including an empty upload. Do not walk the remaining text per piece.
def Att.paste.parts.go(fuel: Nat, +width: Nat, +s: String) -> List<&2, String>:
  match fuel:
    case 0n:
      [s]
    case 1n+k:
      String.take(s, width) <> Att.paste.parts.go(k, width, String.drop(s, width))

def Att.paste.parts.k(zero: Bool, +s: String, +width: Nat) -> List<&2, String>:
  match zero:
    case True{}:
      [s]
    case False{}:
      Att.paste.parts.go(Nat.div(Nat.sub(String.length(s), 1n), width), width, s)

def Att.paste.parts(+s: String, +width: Nat) -> List<&2, String>:
  Att.paste.parts.k(Nat.is_eq(width, 0n), s, width)

# A concatenation with no separators (also the reconstruction law).
def Att.paste.join(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}:
      ""
    case p <> t:
      p ++ Att.paste.join(t)

def Att.paste.piece(+key: String, +name: String, +size: Nat, +i: Nat, last: Bool, data: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"key", J.Str{key}}, J.KV{"name", J.Str{name}},
    J.KV{"size", J.Num{Nat.show(size)}}, J.KV{"i", J.Num{Nat.show(i)}},
    J.KV{"last", J.Flag{last}}, J.KV{"data", J.Str{data}}]))

# Four random bytes as eight hex digits, like the web upload key.
def Att.paste.hex(n: Nat, +word: U32, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      +digit = U32.and(word, 15)
      Att.paste.hex(k, U32.shrn(word, 4n), SCon{Chr{Bool.pick(U32, U32.is_lt(digit, 10), (digit + 48 : U32), (digit + 87 : U32))}, acc})

def Att.paste.key(word: U32) -> String:
  Att.paste.hex(8n, word, "")
