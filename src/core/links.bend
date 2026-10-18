import Base
import ./ref.bend as R

# Links in a thread. A web address (http:// or https://) in a message, bare
# or as a markdown link `[label](url)`, is a link both clients draw and open
# (the web in a new tab, the window with the system's opener). A path to a
# file is a link too (Files below): it opens on the hub's machine. What
# counts as a link, and where it ends, is decided here, once, for all.
# Backplane refs use core/ref.bend; clients show their labels as plain text.

type Piece is Data:
  PText{text: String}
  PLink{label: String, url: String}
  PRef{label: String, uri: String, bare: Bool}

# only web addresses open (never javascript:, file:, ...)
def Link.is_web(+s: String) -> Bool:
  Bool.or(String.starts_with(s, "http://"), String.starts_with(s, "https://"))

# a web address with something after the scheme
def Link.valid(+s: String) -> Bool:
  Bool.and(Link.is_web(s), Bool.not(Bool.or(String.eq(s, "http://"), String.eq(s, "https://"))))

def Link.code(c: Char) -> U32:
  match c:
    case Chr{u}:
      u

# Files
# -----
# A path in a message is a link: bare, in backticks (Md.code in
# web/dom.bend) or as a markdown link's target. Its link keeps the path as
# written (a line may follow, :12 or #L12); a link that is not a web address
# is a file (Link.is_web decides). Clicking one asks the hub to open it
# (file.open); a relative path is in the thread's folder (Link.file.abs).

# what a path may hold besides letters and digits: . _ - / ~ + @ : #
def Link.pmark(+u: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_eq(u, 46), U32.is_eq(u, 95)), Bool.or(U32.is_eq(u, 45), U32.is_eq(u, 47))),
    Bool.or(Bool.or(U32.is_eq(u, 126), U32.is_eq(u, 43)), Bool.or(Bool.or(U32.is_eq(u, 64), U32.is_eq(u, 58)), U32.is_eq(u, 35))))

def Link.pchar(+c: Char) -> Bool:
  Bool.or(Bool.or(Char.is_alpha(c), Char.is_digit(c)), Link.pmark(Link.code(c)))

def Link.pchars(s: String, +ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{+h, t}:
      Link.pchars(t, Bool.and(ok, Link.pchar(h)))

# the path part: up to a line (:) or an anchor (#), walked to the end so
# a long word costs no stack
type Pt is Data:
  Pt{done: Bool, acc: String}

def Link.part.step(st: Pt, +h: Char) -> Pt:
  match st:
    case Pt{True{}, acc}:
      Pt{True{}, acc}
    case Pt{False{}, +acc}:
      Bool.pick(Pt, Bool.or(U32.is_eq(Link.code(h), 58), U32.is_eq(Link.code(h), 35)), Pt{True{}, acc}, Pt{False{}, SCon{h, acc}})

def Link.part.go(s: String, st: Pt) -> Pt:
  match s:
    case SNil{}:
      st
    case SCon{+h, t}:
      Link.part.go(t, Link.part.step(st, h))

def Link.part.of(st: Pt) -> String:
  match st:
    case Pt{_, acc}:
      String.reverse(acc)

def Link.part(+s: String) -> String:
  Link.part.of(Link.part.go(s, Pt{False{}, SNil{}}))

# a file's extension: a dot then a letter
def Link.ext(s: String, +dot: Bool, +ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{+h, t}:
      Link.ext(t, U32.is_eq(Link.code(h), 46), Bool.or(ok, Bool.and(dot, Char.is_alpha(h))))

def Link.digit(m: Maybe<&2, Char>) -> Bool:
  match m:
    case Some{c}:
      Char.is_digit(c)
    case None{}:
      False{}

# what may follow the path: nothing, a line (:12, :12:3) or #L12
def Link.sfx(+x: String) -> Bool:
  Bool.or(String.is_empty(x), Bool.or(String.starts_with(x, "#L"), Bool.and(String.starts_with(x, ":"), Link.digit(String.get(x, 1n)))))

def Link.last(+p: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(p, '/')), "")

# a path by its look: absolute with a folder or a file, under ~/, ./ or
# ../, or a folder then a file with an extension ("and/or", "1/2", "/s"
# and "origin/main" are not)
def Link.shape(+p: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(p, "~/"), String.starts_with(p, "./")), Bool.or(String.starts_with(p, "../"),
    Bool.or(Bool.and(String.starts_with(p, "/"), String.contains(String.drop(p, 1n), "/")), Link.ext(Link.last(p), False{}, False{}))))

# loose (a markdown link's target) also takes a lone file name (notes.md)
def Link.ref.ok(+p: String, +sfx: String, +loose: Bool) -> Bool:
  Bool.and(Bool.and(Link.sfx(sfx), Bool.not(String.ends_with(p, "."))), Bool.and(Bool.not(String.starts_with(p, "//")),
    Bool.or(Bool.and(String.contains(p, "/"), Link.shape(p)), Bool.and(loose, Link.ext(p, False{}, False{})))))

def Link.ref(+s: String, +loose: Bool) -> Bool:
  Bool.and(Bool.and(Link.pchars(s, True{}), Bool.not(String.contains(s, "://"))),
    Link.ref.ok(Link.part(s), String.drop(s, String.length(Link.part(s))), loose))

# a path as a reader writes one: bare in a sentence, or in backticks
def Link.is_path(+s: String) -> Bool:
  Link.ref(s, False{})

# a markdown target: file:// is dropped
def Link.target(+u: String) -> String:
  Bool.pick(String, String.starts_with(u, "file://"), String.drop(u, 7n), u)

# the path a file link names, without its line or anchor
def Link.file.path(+raw: String) -> String:
  Link.part(raw)

# absolute, or under ~/ (by its first char, so laws over "/" ++ s reduce)
def Link.rooted(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+h, t}:
      Bool.or(U32.is_eq(Link.code(h), 47), Bool.and(U32.is_eq(Link.code(h), 126), String.starts_with(t, "/")))

def Link.file.abs.k(keep: Bool, +cwd: String, +p: String) -> String:
  match keep:
    case True{}:
      p
    case False{}:
      Bool.pick(String, String.ends_with(cwd, "/"), cwd, cwd ++ "/") ++ Bool.pick(String, String.starts_with(p, "./"), String.drop(p, 2n), p)

# where a file link's file is: as written when absolute or under ~/ (or
# with no folder to be in), else in the thread's folder cwd
def Link.file.abs(+cwd: String, +raw: String) -> String:
  Link.file.abs.k(Bool.or(Link.rooted(raw), String.is_empty(cwd)), cwd, Link.file.path(raw))

# the hub's shell for file.open: the path is its $1, never shell text; ~/
# is the hub's home. "open" opens it with the system's opener
# (BACKPLANE_OPEN names another), "reveal" shows it in its folder (the
# desktop's file manager over D-Bus, else the folder opened; Finder on a Mac)
def Link.file.sh() -> String:
  "p=\"$1\"; case \"$p\" in \"~/\"*) p=\"$HOME/${p#??}\";; esac; case \"$p\" in /*) ;; *) p=\"$PWD/$p\";; esac; " ++
  "[ -e \"$p\" ] || { echo \"No such file: $p\"; exit 1; }; "

def Link.file.desk() -> String:
  "[ -n \"$DISPLAY$WAYLAND_DISPLAY\" ] || { echo \"No desktop on this machine to open it on\"; exit 1; }; "

def Link.file.open() -> String:
  Link.file.sh() ++ "if [ \"$(uname)\" = Darwin ]; then open \"$p\"; exit $?; fi; " ++ Link.file.desk() ++
  "(${BACKPLANE_OPEN:-xdg-open} \"$p\" >/dev/null 2>&1 &)"

def Link.file.reveal() -> String:
  Link.file.sh() ++ "if [ \"$(uname)\" = Darwin ]; then open -R \"$p\"; exit $?; fi; " ++ Link.file.desk() ++
  "u=$(printf %s \"$p\" | sed 's/%/%25/g; s/ /%20/g; s/#/%23/g; s/?/%3F/g'); " ++
  "dbus-send --session --print-reply --reply-timeout=3000 --dest=org.freedesktop.FileManager1 --type=method_call /org/freedesktop/FileManager1 " ++
  "org.freedesktop.FileManager1.ShowItems array:string:\"file://$u\" string:\"\" >/dev/null 2>&1 || " ++
  "(${BACKPLANE_OPEN:-xdg-open} \"$(dirname \"$p\")\" >/dev/null 2>&1 &)"

def Link.file.cmd(+how: String) -> String:
  Bool.pick(String, String.eq(how, "reveal"), Link.file.reveal(), Link.file.open())

# Scanning
# --------
# One pass over the text, char by char. Text so far is kept reversed (acc),
# as is the address or label being read.

type Scan is Data:
  SText{acc: String}
  SUrl{acc: String, u: String}
  SLab{acc: String, l: String}
  SMid{acc: String, l: String}
  SHref{acc: String, l: String, u: String}

type Sc is Data:
  Sc{scan: Scan, out: List<&2, Piece>}

# a finished run of text (acc reversed)
def Link.text(acc: String, out: List<&2, Piece>) -> List<&2, Piece>:
  match acc:
    case SNil{}:
      out
    case SCon{h, t}:
      PText{String.reverse(SCon{h, t})} <> out

# the char before continues a word (so "xhttp://" is not a link)
def Link.word(acc: String) -> Bool:
  match acc:
    case SNil{}:
      False{}
    case SCon{+h, _}:
      Bool.or(Char.is_alpha(h), Char.is_digit(h))

# An address starts at h + ttp(s):// or b + ackplane://. Only inspect
# the reversed text boundary once a scheme prefix actually matches.
def Link.starts.k(prefix: Bool, acc: String) -> Bool:
  match prefix:
    case True{}: Bool.not(Link.word(acc))
    case False{}: False{}

def Link.starts(+acc: String, +t: String) -> Bool:
  Link.starts.k(Bool.or(Bool.or(String.starts_with(t, "ttp://"), String.starts_with(t, "ttps://")), String.starts_with(t, "ackplane://")), acc)

def Link.text.k(k: Nat, acc: String, out: List<&2, Piece>, +c: Char) -> Sc:
  match k:
    case 1n:
      Sc{SUrl{acc, SCon{c, SNil{}}}, out}
    case 2n:
      Sc{SLab{acc, SNil{}}, out}
    case _:
      Sc{SText{SCon{c, acc}}, out}

# Asked only at h or b; Link.starts checks the cheap prefix first.
def Link.text.h(h: Bool, +acc: String, +t: String) -> Bool:
  match h:
    case True{}:
      Link.starts(acc, t)
    case False{}:
      False{}

def Link.text.step(+acc: String, out: List<&2, Piece>, +c: Char, +t: String) -> Sc:
  Link.text.k(Bool.pick(Nat, Link.text.h(Bool.or(U32.is_eq(Link.code(c), 104), U32.is_eq(Link.code(c), 98)), acc, t), 1n, Bool.pick(Nat, U32.is_eq(Link.code(c), 91), 2n, 0n)), acc, out, c)

# what ends a bare address: space, < > " `
def Link.stops(+c: Char) -> Bool:
  Bool.or(Char.is_space(c), Bool.or(Bool.or(U32.is_eq(Link.code(c), 60), U32.is_eq(Link.code(c), 62)), Bool.or(U32.is_eq(Link.code(c), 34), U32.is_eq(Link.code(c), 96))))

def Link.count(s: String, +x: U32) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{+h, t}:
      Nat.add(Bool.pick(Nat, U32.is_eq(Link.code(h), x), 1n, 0n), Link.count(t, x))

# what a sentence puts after an address: . , ; : ! ? ' ] and a ) it did not open
def Link.punct(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_eq(c, 46), U32.is_eq(c, 44)), Bool.or(U32.is_eq(c, 59), U32.is_eq(c, 58))),
    Bool.or(Bool.or(U32.is_eq(c, 33), U32.is_eq(c, 63)), Bool.or(U32.is_eq(c, 39), U32.is_eq(c, 93))))

# u: the address up to and including its last char c, reversed
def Link.trims(+c: U32, +u: String) -> Bool:
  Bool.or(Link.punct(c), Bool.and(U32.is_eq(c, 41), Nat.is_gt(Link.count(u, 41), Link.count(u, 40))))

# trimming walks the reversed address from its end: trailing punctuation
# moves to the text after it (kept reversed), the rest is the address
type Tr is Data:
  Tr{done: Bool, keep: String, moved: String}

def Link.tr.done(st: Tr) -> Bool:
  match st:
    case Tr{d, _, _}:
      d

def Link.tr.cut(done: Bool, +h: Char, +u: String) -> Bool:
  match done:
    case True{}:
      False{}
    case False{}:
      Link.trims(Link.code(h), u)

def Link.tr.k(cut: Bool, +h: Char, st: Tr) -> Tr:
  match cut st:
    case True{} Tr{d, k, m}:
      Tr{d, k, String.append(m, SCon{h, SNil{}})}
    case False{} Tr{_, k, m}:
      Tr{True{}, SCon{h, k}, m}

def Link.tr(u: String, +st: Tr) -> Tr:
  match u:
    case SNil{}:
      st
    case SCon{+h, +t}:
      Link.tr(t, Link.tr.k(Link.tr.cut(Link.tr.done(st), h, SCon{h, t}), h, st))

def Link.piece(m: Maybe<&2, R.Ref>, label: String, url: String) -> Piece:
  match m:
    case Some{r}: PRef{label, R.Ref.uri(r), False{}}
    case None{}: PLink{label, url}

def Link.url_end.v(ok: Bool, +url: String, acc: String, moved: String, out: List<&2, Piece>) -> Sc:
  match ok:
    case True{}:
      Sc{SText{moved}, Link.piece(R.Ref.parse(url), url, url) <> Link.text(acc, out)}
    case False{}:
      Sc{SText{String.append(moved, String.append(String.reverse(url), acc))}, out}

def Link.url_end.k(tr: Tr, acc: String, out: List<&2, Piece>) -> Sc:
  match tr:
    case Tr{_, +url, moved}:
      Link.url_end.v(Bool.or(Link.valid(url), Maybe.is_some(&2, R.Ref, R.Ref.parse(url))), url, acc, moved, out)

# a bare address ended (u reversed)
def Link.url_end(acc: String, u: String, out: List<&2, Piece>) -> Sc:
  Link.url_end.k(Link.tr(u, Tr{False{}, SNil{}, SNil{}}), acc, out)

# the text step after an address ended at c
def Link.again(sc: Sc, +c: Char, +t: String) -> Sc:
  match sc:
    case Sc{SText{+acc}, out}:
      Link.text.step(acc, out, c, t)
    case Sc{s, out}:
      Sc{s, out}

def Link.url.k(stop: Bool, acc: String, u: String, out: List<&2, Piece>, +c: Char, +t: String) -> Sc:
  match stop:
    case True{}:
      Link.again(Link.url_end(acc, u, out), c, t)
    case False{}:
      Sc{SUrl{acc, SCon{c, u}}, out}

# a [label that went nowhere, back to text
def Link.lab.raw(l: String, acc: String) -> String:
  String.append(l, SCon{'[', acc})

def Link.lab.k(k: Nat, +acc: String, +l: String, out: List<&2, Piece>, +c: Char, +t: String) -> Sc:
  match k:
    case 1n:
      Sc{SMid{acc, l}, out}
    case 2n:
      Link.text.step(Link.lab.raw(l, acc), out, c, t)
    case _:
      Sc{SLab{acc, SCon{c, l}}, out}

def Link.mid.k(open: Bool, +acc: String, +l: String, out: List<&2, Piece>, +c: Char, +t: String) -> Sc:
  match open:
    case True{}:
      Sc{SHref{acc, l, SNil{}}, out}
    case False{}:
      Link.text.step(SCon{']', Link.lab.raw(l, acc)}, out, c, t)

# [label](u that went nowhere, back to text
def Link.href.raw(acc: String, l: String, u: String) -> String:
  String.append(u, SCon{'(', SCon{']', Link.lab.raw(l, acc)}})

def Link.label(+l: String, +u: String) -> String:
  Bool.pick(String, String.is_empty(l), String.reverse(u), String.reverse(l))

# a markdown image (![alt](path)) stays text: the image shows under it
def Link.bang(acc: String) -> Bool:
  match acc:
    case SNil{}:
      False{}
    case SCon{+h, _}:
      U32.is_eq(Link.code(h), 33)

# a markdown link's target: a web address, or a file (not an image's)
def Link.href.ref(ref: Bool, +u: String, +acc: String) -> Bool:
  match ref:
    case True{}: Bool.not(Link.bang(acc))
    case False{}: Bool.or(Link.valid(u), Bool.and(Bool.not(Link.bang(acc)), Link.ref(Link.target(u), True{})))

def Link.href.ok(+u: String, +acc: String) -> Bool:
  Link.href.ref(Maybe.is_some(&2, R.Ref, R.Ref.parse(u)), u, acc)

def Link.close(ok: Bool, +acc: String, +l: String, +u: String, out: List<&2, Piece>) -> Sc:
  match ok:
    case True{}:
      Sc{SText{SNil{}}, Link.piece(R.Ref.parse(String.reverse(u)), Link.label(l, u), Link.target(String.reverse(u))) <> Link.text(acc, out)}
    case False{}:
      Sc{SText{SCon{')', Link.href.raw(acc, l, u)}}, out}

def Link.href.k(k: Nat, +acc: String, +l: String, +u: String, out: List<&2, Piece>, +c: Char, +t: String) -> Sc:
  match k:
    case 1n:
      Link.close(Link.href.ok(String.reverse(u), acc), acc, l, u, out)
    case 2n:
      Link.text.step(Link.href.raw(acc, l, u), out, c, t)
    case _:
      Sc{SHref{acc, l, SCon{c, u}}, out}

def Link.step(sc: Sc, +c: Char, +t: String) -> Sc:
  match sc:
    case Sc{SText{+acc}, out}:
      Link.text.step(acc, out, c, t)
    case Sc{SUrl{acc, u}, out}:
      Link.url.k(Link.stops(c), acc, u, out, c, t)
    case Sc{SLab{acc, l}, out}:
      Link.lab.k(Bool.pick(Nat, U32.is_eq(Link.code(c), 93), 1n, Bool.pick(Nat, Bool.or(U32.is_eq(Link.code(c), 10), U32.is_eq(Link.code(c), 91)), 2n, 0n)), acc, l, out, c, t)
    case Sc{SMid{acc, l}, out}:
      Link.mid.k(U32.is_eq(Link.code(c), 40), acc, l, out, c, t)
    case Sc{SHref{acc, l, u}, out}:
      Link.href.k(Bool.pick(Nat, U32.is_eq(Link.code(c), 41), 1n, Bool.pick(Nat, Link.stops(c), 2n, 0n)), acc, l, u, out, c, t)

def Link.fin.text(sc: Sc) -> List<&2, Piece>:
  match sc:
    case Sc{SText{acc}, out}:
      List.reverse(&2, Piece, Link.text(acc, out))
    case Sc{_, out}:
      List.reverse(&2, Piece, out)

def Link.fin(sc: Sc) -> List<&2, Piece>:
  match sc:
    case Sc{SText{acc}, out}:
      List.reverse(&2, Piece, Link.text(acc, out))
    case Sc{SUrl{acc, u}, out}:
      Link.fin.text(Link.url_end(acc, u, out))
    case Sc{SLab{acc, l}, out}:
      List.reverse(&2, Piece, Link.text(Link.lab.raw(l, acc), out))
    case Sc{SMid{acc, l}, out}:
      List.reverse(&2, Piece, Link.text(SCon{']', Link.lab.raw(l, acc)}, out))
    case Sc{SHref{acc, l, u}, out}:
      List.reverse(&2, Piece, Link.text(Link.href.raw(acc, l, u), out))

def Link.go(s: String, sc: Sc) -> List<&2, Piece>:
  match s:
    case SNil{}:
      Link.fin(sc)
    case SCon{+c, +t}:
      Link.go(t, Link.step(sc, c, t))

# Bare paths and ids
# ------------------
# After the scan, each run of text is cut into words at spaces; a word
# that is a path (Link.is_path) once what wraps it is dropped (a leading
# ( [ " ' * < ` and a trailing . , ; : ! ? ' " ) ] * >) becomes a link.
# Text so far (txt), the word being read and the pieces are kept reversed.

type Bw is Data:
  Bw{txt: String, word: String, out: List<&2, Piece>, image: Nat, bang: Bool}

def Link.wrap(+u: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_eq(u, 40), U32.is_eq(u, 91)), Bool.or(U32.is_eq(u, 34), U32.is_eq(u, 39))),
    Bool.or(Bool.or(U32.is_eq(u, 42), U32.is_eq(u, 60)), U32.is_eq(u, 96)))

def Link.after(+u: U32) -> Bool:
  Bool.or(Bool.or(Link.punct(u), U32.is_eq(u, 34)), Bool.or(Bool.or(U32.is_eq(u, 41), U32.is_eq(u, 42)), Bool.or(U32.is_eq(u, 62), U32.is_eq(u, 96))))

# how many of s's first chars wrap it
def Link.lead(s: String, +n: Nat, +on: Bool) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{+h, t}:
      Link.lead(t, Bool.pick(Nat, Bool.and(on, Link.wrap(Link.code(h))), 1n+n, n), Bool.and(on, Link.wrap(Link.code(h))))

# how many of a reversed word's first chars follow it
def Link.tail(s: String, +n: Nat, +on: Bool) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{+h, t}:
      Link.tail(t, Bool.pick(Nat, Bool.and(on, Link.after(Link.code(h))), 1n+n, n), Bool.and(on, Link.after(Link.code(h))))

def Link.mid(+w: String, +lead: Nat, +trail: Nat) -> String:
  String.take(String.drop(w, lead), Nat.sub(Nat.sub(String.length(w), lead), trail))

# The existing bare-word pass also recognises ids. Each whitespace word
# is checked as a file first; its remaining id words use exact boundaries.
# Image state crosses whitespace words, so alt text and targets stay text.
def Link.idchar(+c: Char) -> Bool:
  Bool.or(Bool.or(Char.is_alpha(c), Char.is_digit(c)), Bool.or(U32.is_eq(Link.code(c), 46), Bool.or(U32.is_eq(Link.code(c), 95), Bool.or(U32.is_eq(Link.code(c), 45), U32.is_eq(Link.code(c), 126)))))

def Link.ids.part(m: Maybe<&2, R.Ref>, txt: String, +w: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match m:
    case Some{r}: Bw{"", "", PRef{String.reverse(w), R.Ref.uri(r), True{}} <> Link.text(txt, out), image, bang}
    case None{}: Bw{String.append(w, txt), "", out, image, bang}

def Link.ids.part.flush(st: Bw) -> Bw:
  match st:
    case Bw{txt, SNil{}, out, image, bang}: Bw{txt, "", out, image, bang}
    case Bw{txt, +w, out, image, bang}: Link.ids.part(R.Ref.bare(String.reverse(w)), txt, w, out, image, bang)

def Link.ids.dot.push(st: Bw) -> Bw:
  match st:
    case Bw{txt, w, out, image, bang}: Bw{SCon{'.', txt}, w, out, image, bang}

def Link.ids.dot.step(dot: Bool, st: Bw, c: Char) -> Bw:
  match dot st:
    case True{} _: Link.ids.dot.push(Link.ids.part.flush(st))
    case False{} Bw{txt, w, out, image, bang}: Bw{txt, SCon{c, w}, out, image, bang}

def Link.ids.dot.go(s: String, st: Bw) -> Bw:
  match s:
    case SNil{}: Link.ids.part.flush(st)
    case SCon{+c, t}: Link.ids.dot.go(t, Link.ids.dot.step(Char.is_eq(c, '.'), st, c))

def Link.ids.retry(dot: Bool, txt: String, w: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match dot:
    case True{}: Link.ids.dot.go(String.reverse(w), Bw{txt, "", out, image, bang})
    case False{}: Bw{String.append(w, txt), "", out, image, bang}

def Link.ids.edge(m: Maybe<&2, R.Ref>, txt: String, +w: String, out: List<&2, Piece>, image: Nat, bang: Bool, +trail: Nat) -> Bw:
  match m:
    case Some{r}: Bw{String.take(w, trail), "", PRef{String.reverse(String.drop(w, trail)), R.Ref.uri(r), True{}} <> Link.text(txt, out), image, bang}
    case None{}: Link.ids.retry(True{}, txt, w, out, image, bang)

def Link.ids.miss(dot: Bool, txt: String, +w: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match dot:
    case True{}: Link.ids.edge(R.Ref.bare(String.reverse(String.drop(w, Link.tail(w, 0n, True{})))), txt, w, out, image, bang, Link.tail(w, 0n, True{}))
    case False{}: Bw{String.append(w, txt), "", out, image, bang}

# A dot can belong to a machine prefix. Try the complete candidate first,
# then without sentence punctuation, before splitting internal dots.
def Link.ids.end(m: Maybe<&2, R.Ref>, txt: String, +w: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match m:
    case Some{r}: Bw{"", "", PRef{String.reverse(w), R.Ref.uri(r), True{}} <> Link.text(txt, out), image, bang}
    case None{}: Link.ids.miss(String.contains(w, "."), txt, w, out, image, bang)

def Link.ids.test(muted: Bool, txt: String, +w: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match muted:
    case True{}: Bw{String.append(w, txt), "", out, image, bang}
    case False{}: Link.ids.end(R.Ref.bare(String.reverse(w)), txt, w, out, image, bang)

def Link.ids.flush(st: Bw, muted: Bool) -> Bw:
  match st:
    case Bw{txt, SNil{}, out, image, bang}: Bw{txt, "", out, image, bang}
    case Bw{txt, SCon{h, t}, out, image, bang}: Link.ids.test(muted, txt, SCon{h, t}, out, image, bang)

def Link.ids.delim(st: Bw, +c: Char) -> Bw:
  match st:
    case Bw{txt, w, out, image, bang}:
      Bw{SCon{c, txt}, w, out, Bool.pick(Nat, Bool.and(bang, U32.is_eq(Link.code(c), 91)), 1n, image), U32.is_eq(Link.code(c), 33)}

def Link.ids.normal(word: Bool, st: Bw, +c: Char, muted: Bool) -> Bw:
  match word st:
    case True{} Bw{txt, w, out, image, _}: Bw{txt, SCon{c, w}, out, image, False{}}
    case False{} _: Link.ids.delim(Link.ids.flush(st, muted), c)

def Link.ids.image(st: Bw, c: Char, image: Nat) -> Bw:
  match st:
    case Bw{txt, w, out, _, _}: Bw{SCon{c, txt}, w, out, image, False{}}

def Link.ids.after(open: Bool, st: Bw, +c: Char, muted: Bool) -> Bw:
  match open st:
    case True{} _: Link.ids.image(st, c, 3n)
    case False{} Bw{txt, w, out, _, b}: Link.ids.normal(Link.idchar(c), Bw{txt, w, out, 0n, b}, c, muted)

def Link.ids.step(st: Bw, +c: Char, muted: Bool) -> Bw:
  match st:
    case Bw{txt, w, out, 0n, b}: Link.ids.normal(Link.idchar(c), Bw{txt, w, out, 0n, b}, c, muted)
    case Bw{txt, w, out, 1n, b}: Link.ids.image(Bw{txt, w, out, 1n, b}, c, Bool.pick(Nat, U32.is_eq(Link.code(c), 93), 2n, 1n))
    case Bw{txt, w, out, 2n, b}: Link.ids.after(U32.is_eq(Link.code(c), 40), Bw{txt, w, out, 2n, b}, c, muted)
    case Bw{txt, w, out, +n, b}: Link.ids.image(Bw{txt, w, out, n, b}, c, Bool.pick(Nat, U32.is_eq(Link.code(c), 41), 0n, n))

def Link.ids.go(s: String, st: Bw, +muted: Bool) -> Bw:
  match s:
    case SNil{}: Link.ids.flush(st, muted)
    case SCon{+c, t}: Link.ids.go(t, Link.ids.step(st, c, muted), muted)

def Link.bare.hit(ok: Bool, +w: String, +lead: Nat, +core: String, txt: String, out: List<&2, Piece>, +image: Nat, bang: Bool) -> Bw:
  match ok:
    case True{}:
      Bw{String.reverse(String.drop(w, Nat.add(lead, String.length(core)))), SNil{}, PLink{core, core} <> Link.text(String.append(String.reverse(String.take(w, lead)), txt), out), image, False{}}
    case False{}:
      Link.ids.go(w, Bw{txt, "", out, image, bang}, Bool.and(String.contains(w, "://"), Bool.and(Nat.is_eq(image, 0n), Bool.not(String.contains(w, "![")))))

def Link.bare.core(+w: String, +lead: Nat, +trail: Nat, txt: String, out: List<&2, Piece>, +image: Nat, bang: Bool) -> Bw:
  Link.bare.hit(Link.is_path(Link.mid(w, lead, trail)), w, lead, Link.mid(w, lead, trail), txt, out, image, bang)

def Link.bare.word.k(slash: Bool, +rev: String, txt: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match slash:
    case True{}: Link.bare.core(String.reverse(rev), Link.lead(String.reverse(rev), 0n, True{}), Link.tail(rev, 0n, True{}), txt, out, image, bang)
    case False{}: Link.bare.hit(False{}, String.reverse(rev), 0n, "", txt, out, image, bang)

def Link.bare.word(+rev: String, txt: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  Link.bare.word.k(String.contains(rev, "/"), rev, txt, out, image, bang)

def Link.bare.flush(txt: String, word: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match word:
    case SNil{}: Bw{txt, "", out, image, bang}
    case SCon{h, t}: Link.bare.word(SCon{h, t}, txt, out, image, bang)

def Link.bare.push(b: Bw, +c: Char) -> Bw:
  match b:
    case Bw{txt, w, out, +image, _}: Bw{SCon{c, txt}, w, out, Bool.pick(Nat, Nat.is_eq(image, 2n), 0n, image), False{}}

def Link.bare.k(sp: Bool, +c: Char, txt: String, word: String, out: List<&2, Piece>, image: Nat, bang: Bool) -> Bw:
  match sp:
    case True{}: Link.bare.push(Link.bare.flush(txt, word, out, image, bang), c)
    case False{}: Bw{txt, SCon{c, word}, out, image, bang}

def Link.bare.step(st: Bw, +c: Char) -> Bw:
  match st:
    case Bw{txt, word, out, image, bang}: Link.bare.k(Char.is_space(c), c, txt, word, out, image, bang)

def Link.bare.go(s: String, st: Bw) -> Bw:
  match s:
    case SNil{}: st
    case SCon{+c, t}: Link.bare.go(t, Link.bare.step(st, c))

def Link.bare.end(b: Bw) -> List<&2, Piece>:
  match b:
    case Bw{txt, _, out, _, _}: List.reverse(&2, Piece, Link.text(txt, out))

def Link.bare.fin(st: Bw) -> List<&2, Piece>:
  match st:
    case Bw{txt, word, out, image, bang}: Link.bare.end(Link.bare.flush(txt, word, out, image, bang))

def Link.bare.text.k(scan: Bool, +s: String) -> List<&2, Piece>:
  match scan:
    case True{}: Link.bare.fin(Link.bare.go(s, Bw{"", "", Nil{}, 0n, False{}}))
    case False{}: [PText{s}]

def Link.files.go(ps: List<&2, Piece>, acc: List<&2, Piece>) -> List<&2, Piece>:
  match ps:
    case Nil{}: List.reverse(&2, Piece, acc)
    case PText{+s} <> t:
      Link.files.go(t, List.reverse.go(&2, Piece, Link.bare.text.k(Bool.or(String.contains(s, "/"), String.contains(s, "-")), s), acc))
    case PLink{l, u} <> t: Link.files.go(t, PLink{l, u} <> acc)
    case PRef{l, u, b} <> t: Link.files.go(t, PRef{l, u, b} <> acc)

def Link.files(ps: List<&2, Piece>) -> List<&2, Piece>:
  Link.files.go(ps, Nil{})

# text as runs of plain text and links, in order
def Link.pieces(+s: String) -> List<&2, Piece>:
  Link.files(Link.go(s, Sc{SText{SNil{}}, Nil{}}))

# the text a reader sees (a link shows its label)
def Link.plain(ps: List<&2, Piece>) -> String:
  match ps:
    case Nil{}:
      ""
    case PText{s} <> t:
      s ++ Link.plain(t)
    case PLink{l, _} <> t:
      l ++ Link.plain(t)
    case PRef{l, _, _} <> t:
      l ++ Link.plain(t)

# the addresses, in order
def Link.urls(ps: List<&2, Piece>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case PText{_} <> t:
      Link.urls(t)
    case PLink{_, u} <> t:
      u <> Link.urls(t)
    case PRef{_, u, _} <> t:
      u <> Link.urls(t)

# Lines
# -----
# The window wraps a paragraph's text into lines first, then finds in each
# line the labels of the paragraph's links. A label cut by a wrap is not a
# link on either line.

# the first link whose label starts s
def Link.at(ls: List<&2, Piece>, +s: String) -> Maybe<&2, Piece>:
  match ls:
    case Nil{}:
      None{}
    case PText{_} <> t:
      Link.at(t, s)
    case PRef{+l, +u, +b} <> t:
      Bool.pick(Maybe<&2, Piece>, Bool.and(Bool.not(String.is_empty(l)), String.starts_with(s, l)), Some{PRef{l, u, b}}, Link.at(t, s))
    case PLink{+l, +u} <> t:
      Bool.pick(Maybe<&2, Piece>, Bool.and(Bool.not(String.is_empty(l)), String.starts_with(s, l)), Some{PLink{l, u}}, Link.at(t, s))

type Ct is Data:
  Ct{acc: String, skip: Nat, out: List<&2, Piece>}

def Link.cut.hit(m: Maybe<&2, Piece>, +c: Char, st: Ct) -> Ct:
  match m st:
    case Some{PRef{+l, u, b}} Ct{acc, _, out}:
      Ct{SNil{}, Nat.sub(String.length(l), 1n), PRef{l, u, b} <> Link.text(acc, out)}
    case Some{PLink{+l, u}} Ct{acc, _, out}:
      Ct{SNil{}, Nat.sub(String.length(l), 1n), PLink{l, u} <> Link.text(acc, out)}
    case _ Ct{acc, k, out}:
      Ct{SCon{c, acc}, k, out}

def Link.cut.step(skip: Nat, +ls: List<&2, Piece>, +c: Char, +s: String, st: Ct) -> Ct:
  match skip st:
    case 1n+k Ct{acc, _, out}:
      Ct{acc, k, out}
    case 0n _:
      Link.cut.hit(Link.at(ls, s), c, st)

def Link.cut.skip(st: Ct) -> Nat:
  match st:
    case Ct{_, k, _}:
      k

def Link.cut.fin(st: Ct) -> List<&2, Piece>:
  match st:
    case Ct{acc, _, out}:
      List.reverse(&2, Piece, Link.text(acc, out))

def Link.cut.go(s: String, +ls: List<&2, Piece>, +st: Ct) -> List<&2, Piece>:
  match s:
    case SNil{}:
      Link.cut.fin(st)
    case SCon{+c, +t}:
      Link.cut.go(t, ls, Link.cut.step(Link.cut.skip(st), ls, c, SCon{c, t}, st))

# a drawn line as runs of text and the links whose labels it shows
def Link.cut(+line: String, ls: List<&2, Piece>) -> List<&2, Piece>:
  match ls:
    case Nil{}:
      [PText{line}]
    case +h <> t:
      Link.cut.go(line, h <> t, Ct{SNil{}, 0n, Nil{}})

# Links carried in a line's spare field: label US url, joined by RS.
# A ref adds a third US field (1 bare, 0 explicit); old links keep two.
def Link.enc(ls: List<&2, Piece>) -> String:
  match ls:
    case Nil{}:
      ""
    case PText{_} <> t:
      Link.enc(t)
    case PLink{l, u} <> t:
      l ++ SCon{Chr{31}, u} ++ SCon{Chr{30}, Link.enc(t)}
    case PRef{l, u, b} <> t:
      l ++ SCon{Chr{31}, u} ++ SCon{Chr{31}, Bool.pick(String, b, "1", "0")} ++ SCon{Chr{30}, Link.enc(t)}

def Link.dec.parts(xs: List<&2, String>) -> List<&2, Piece>:
  match xs:
    case l <> u <> b <> Nil{}: [PRef{l, u, String.eq(b, "1")}]
    case l <> u <> Nil{}: [PLink{l, u}]
    case l <> Nil{}: [PLink{l, ""}]
    case Nil{}: Nil{}
    case _: Nil{}

def Link.dec.one.k(empty: Bool, s: String) -> List<&2, Piece>:
  match empty:
    case True{}: Nil{}
    case False{}: Link.dec.parts(String.split(s, Chr{31}))

def Link.dec.one(+s: String) -> List<&2, Piece>:
  Link.dec.one.k(String.is_empty(s), s)

def Link.dec.all(xs: List<&2, String>) -> List<&2, Piece>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      List.append(&2, Piece, Link.dec.one(x), Link.dec.all(t))

def Link.dec(+s: String) -> List<&2, Piece>:
  Link.dec.all(String.split(s, Chr{30}))
