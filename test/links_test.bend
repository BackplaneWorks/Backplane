import Base
import ./check.bend as C
import ../src/core/links.bend as L
import ../src/web/dom.bend as D

def Web.attrs(xs: List<&2, D.Attr>) -> String:
  match xs:
    case Nil{}:
      ""
    case D.At{n, v} <> t:
      " " ++ n ++ "=" ++ v ++ Web.attrs(t)
    case D.On{e, a} <> t:
      " on" ++ e ++ "=" ++ a ++ Web.attrs(t)

def Web.shows(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}:
      ""
    case D.Txt{s} <> t:
      s ++ Web.shows(t)
    case D.El{+tag, _, xs, ks} <> t:
      "<" ++ tag ++ Web.attrs(xs) ++ ">" ++ Web.shows(ks) ++ "</" ++ tag ++ ">" ++ Web.shows(t)

# pieces shown as text: links as <label|url>
def Show.go(ps: List<&2, L.Piece>) -> String:
  match ps:
    case Nil{}:
      ""
    case L.PText{s} <> t:
      s ++ Show.go(t)
    case L.PLink{l, u} <> t:
      "<" ++ l ++ "|" ++ u ++ ">" ++ Show.go(t)
    case L.PRef{l, u, b} <> t:
      "{" ++ l ++ "|" ++ u ++ "|" ++ Bool.pick(String, b, "1", "0") ++ "}" ++ Show.go(t)

def Show(+s: String) -> String:
  Show.go(L.Link.pieces(s))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("bare", Show("see https://example.com/a?b=1 now"), "see <https://example.com/a?b=1|https://example.com/a?b=1> now")
    C.Check.str("sentence end", Show("Docs: http://x.io/docs."), "Docs: <http://x.io/docs|http://x.io/docs>.")
    C.Check.str("in parens", Show("(https://a.b/c), then"), "(<https://a.b/c|https://a.b/c>), then")
    C.Check.str("balanced parens", Show("https://en.wikipedia.org/wiki/Foo_(bar)"), "<https://en.wikipedia.org/wiki/Foo_(bar)|https://en.wikipedia.org/wiki/Foo_(bar)>")
    C.Check.str("markdown", Show("read [the guide](https://bend-lang.com/guide) first"), "read <the guide|https://bend-lang.com/guide> first")
    C.Check.str("markdown not web", Show("[x](javascript:alert(1)) and ![img](/tmp/a.png)"), "[x](javascript:alert(1)) and ![img](/tmp/a.png)")
    C.Check.str("brackets alone", Show("a [note] here [b"), "a [note] here [b")
    C.Check.str("scheme only", Show("http:// and https://"), "http:// and https://")
    C.Check.str("inside a word", Show("xhttp://a.b"), "xhttp://a.b")
    C.Check.str("angle", Show("<https://a.b/x>"), "<<https://a.b/x|https://a.b/x>>")
    C.Check.str("two", String.join(L.Link.urls(L.Link.pieces("https://a.b and [c](http://c.d)")), " "), "https://a.b http://c.d")
    C.Check.str("plain keeps text", L.Link.plain(L.Link.pieces("go to [site](https://s.t), ok")), "go to site, ok")
    C.Check.str("cut", Show.go(L.Link.cut("open site or https://a.b now", L.Link.dec(L.Link.enc(L.Link.pieces("[site](https://s.t) https://a.b"))))), "open <site|https://s.t> or <https://a.b|https://a.b> now")
    C.Check.str("cut none", Show.go(L.Link.cut("plain line", Nil{})), "plain line")
    C.Check.str("web only", String.join([Bool.show(L.Link.is_web("https://x")), Bool.show(L.Link.is_web("javascript:x")), Bool.show(L.Link.is_web("file:///etc"))], " "), "True False False")
    C.Check.str("bare path", Show("wrote /tmp/out/a.txt, then (src/core/links.bend:12)."), "wrote </tmp/out/a.txt|/tmp/out/a.txt>, then (<src/core/links.bend:12|src/core/links.bend:12>).")
    C.Check.str("home path", Show("see ~/notes.md and ./x"), "see <~/notes.md|~/notes.md> and <./x|./x>")
    C.Check.str("not paths", Show("and/or 1/2 /s origin/main 1.5/2.5 a / b //c/d.e"), "and/or 1/2 /s origin/main 1.5/2.5 a / b //c/d.e")
    C.Check.str("url not path", Show("https://a.b/c.png"), "<https://a.b/c.png|https://a.b/c.png>")
    C.Check.str("markdown file", Show("[links.bend](src/core/links.bend#L42) [n](notes.md) [f](file:///etc/hosts)"), "<links.bend|src/core/links.bend#L42> <n|notes.md> <f|/etc/hosts>")
    C.Check.str("markdown scheme", Show("[m](mailto:a@b.c) [w](word)"), "[m](mailto:a@b.c) [w](word)")
    C.Check.str("file abs", String.join([L.Link.file.abs("/w/t", "src/a.bend:12:3"), L.Link.file.abs("/w/t/", "./b#L2"), L.Link.file.abs("/w", "/etc/x:9"), L.Link.file.abs("/w", "~/y"), L.Link.file.abs("", "z/q.c")], " "), "/w/t/src/a.bend /w/t/b /etc/x ~/y z/q.c")
    C.Check.str("web code path", Web.shows(D.Md.render("Edit `src/app/main.bend:628` not `a.b`")), "<p>Edit <a class=link file href=src/app/main.bend:628 title=src/app/main.bend:628 onclick=file-open oncontextmenu=file-menu data-value=src/app/main.bend:628><code>src/app/main.bend:628</code></a> not <code>a.b</code></p>")
    C.Check.str("web bare path", Web.shows(D.Md.render("at /tmp/x/y.txt")), "<p>at <a class=link file href=/tmp/x/y.txt title=/tmp/x/y.txt onclick=file-open oncontextmenu=file-menu data-value=/tmp/x/y.txt>/tmp/x/y.txt</a></p>")
    C.Check.str("web reply", Web.shows(D.Md.render("See **this** at https://a.b/c. Or `https://no.link`")), "<p>See <strong>this</strong> at <a class=link href=https://a.b/c target=_blank rel=noopener noreferrer title=https://a.b/c>https://a.b/c</a>. Or <code>https://no.link</code></p>")
    C.Check.str("web markdown link", Web.shows(D.Md.render("- [docs](https://d.e)")), "<ul><li><a class=link href=https://d.e target=_blank rel=noopener noreferrer title=https://d.e>docs</a></li></ul>")
    C.Check.str("ref uri", Show("see backplane://thread/p1~t17-2#m1, now"), "see {backplane://thread/p1~t17-2#m1|backplane://thread/p1~t17-2#m1|0}, now")
    C.Check.str("ref web", Show("https://h:3787/#/thread/t1-2"), "{https://h:3787/#/thread/t1-2|backplane://thread/t1-2|0}")
    C.Check.str("ref markdown", Show("read [thread](backplane://thread/t1-2) now"), "read {thread|backplane://thread/t1-2|0} now")
    C.Check.str("ref markdown web", Show("[mail](http://h/#/mail/c/m1)"), "{mail|backplane://mail/c/m1|0}")
    C.Check.str("ref bare", Show("see t1-2 and p1~b3-4-5."), "see {t1-2|backplane://thread/t1-2|1} and {p1~b3-4-5|backplane://task/p1~b3-4-5|1}.")
    C.Check.str("ref boundaries", Show("t1-2,t3-4:xt1-2 _t1-2 t1-2_ t1-2-x"), "{t1-2|backplane://thread/t1-2|1},{t3-4|backplane://thread/t3-4|1}:xt1-2 _t1-2 t1-2_ t1-2-x")
    C.Check.str("ref in word", Show("xbackplane://thread/t1-2"), "xbackplane://thread/t1-2")
    C.Check.str("ref unknown", Show("backplane://nope/x"), "backplane://nope/x")
    C.Check.str("ref image", Show("![t1-2](backplane://thread/t3-4) ![x](https://h/#/thread/t1-2)"), "![t1-2](backplane://thread/t3-4) ![x](https://h/#/thread/t1-2)")
    C.Check.str("ref plain", Show("a line with no refs"), "a line with no refs")
    C.Check.str("ref cut", Show.go(L.Link.cut("see thread and t1-2", L.Link.dec(L.Link.enc(L.Link.pieces("[thread](backplane://thread/t3-4) t1-2"))))), "see {thread|backplane://thread/t3-4|0} and {t1-2|backplane://thread/t1-2|1}")
    C.Check.str("ref web node", Web.shows(D.Md.render("[thread](backplane://thread/t1-2) t3-4")), "<p><a class=ref href=backplane://thread/t1-2 data-ref=backplane://thread/t1-2 data-bare=0>thread</a> <a class=ref href=backplane://thread/t3-4 data-ref=backplane://thread/t3-4 data-bare=1>t3-4</a></p>")
    C.Check.str("ref brackets", Show("[t1-2] and (b3-4)."), "[{t1-2|backplane://thread/t1-2|1}] and ({b3-4|backplane://bot/b3-4|1}).")
    C.Check.str("ref spaced image", Show("![a t1-2 label](backplane://thread/t3-4) t5-6"), "![a t1-2 label](backplane://thread/t3-4) {t5-6|backplane://thread/t5-6|1}")
    C.Check.str("ref adjacent image", Show("![x](backplane://thread/t3-4)t5-6"), "![x](backplane://thread/t3-4){t5-6|backplane://thread/t5-6|1}")
    C.Check.str("ref file segments", Show("/tmp/t1-2 src/t1-2.txt"), "</tmp/t1-2|/tmp/t1-2> <src/t1-2.txt|src/t1-2.txt>")
    C.Check.str("ref canonical pct", Show("backplane://thread/t%3a1"), "{backplane://thread/t%3a1|backplane://thread/t%3A1|0}")
    C.Check.str("ref direct Dom link", Web.shows([D.Dom.link("thread", "http://h/#/thread/t1-2")]), "<a class=ref href=backplane://thread/t1-2 data-ref=backplane://thread/t1-2 data-bare=0>thread</a>")
    C.Check.str("ref dotted remote", Show("desk.local~t1-2,b3-4"), "{desk.local~t1-2|backplane://thread/desk.local~t1-2|1},{b3-4|backplane://bot/b3-4|1}")
    C.Check.str("ref dot boundary", Show("t1-2.t3-4"), "{t1-2|backplane://thread/t1-2|1}.{t3-4|backplane://thread/t3-4|1}")
    C.Check.str("ref dotted remote sentence", Show("desk.local~t1-2. Next"), "{desk.local~t1-2|backplane://thread/desk.local~t1-2|1}. Next")
