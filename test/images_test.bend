import Base
import ./check.bend as C
import ../src/core/images.bend as I
import ../src/core/attach.bend as AT

# a long text: s doubled n times (tail-recursive)
def Long.go(n: Nat, +s: String) -> String:
  match n:
    case 0n:
      s
    case 1n+k:
      Long.go(k, s ++ s)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("bare path", String.join(I.Img.paths("Rendered to /tmp/board-top.png."), "|"), "/tmp/board-top.png")
    C.Check.str("backticks and markdown", String.join(I.Img.paths("See `/tmp/a.PNG` and ![top](~/renders/b.jpg), not /tmp/c.txt"), "|"), "/tmp/a.PNG|~/renders/b.jpg")
    C.Check.str("each once", String.join(I.Img.paths("/tmp/a.png then /tmp/a.png again"), "|"), "/tmp/a.png")
    C.Check.str("relative is not shown", String.join(I.Img.paths("out/a.png and a.png"), "|"), "")
    C.Check.str("file uri", String.join(I.Img.paths("file:///tmp/x.webp"), "|"), "/tmp/x.webp")
    C.Check.str("may: an extension in any case", Bool.show(Bool.and(I.Img.may("see /tmp/x.PNG"), I.Img.may("/tmp/y.Jpeg here"))), "True")
    C.Check.str("may: none", Bool.show(Bool.or(I.Img.may("a.txt and png, no dot before it"), I.Img.may("ends with a dot."))), "False")
    # a pasted log hundreds of thousands of characters long: one walk that
    # never nests (the phones' V8 died on a 14k-character message)
    C.Check.str("may: a long text", Bool.show(Bool.and(I.Img.may(Long.go(18n, "word. ") ++ "/tmp/z.png"), Bool.not(I.Img.may(Long.go(18n, "word. "))))), "True")
    # a long reply naming an image: the words found in one walk that never
    # nests (String.split per character overflowed the phones' stack and
    # lost the hub's frame, and with it the next message sent)
    C.Check.str("paths: a long text", String.join(I.Img.paths(Long.go(16n, "see this. ") ++ "/tmp/z.png and (~/q.gif)"), "|"), "/tmp/z.png|~/q.gif")
    C.Check.str("paths: a word too long to be a path", String.join(I.Img.paths("/" ++ Long.go(11n, "a") ++ ".png /tmp/ok.png"), "|"), "/tmp/ok.png")
    C.Check.str("paths: separators", String.join(I.Img.paths("a\r/tmp/r.png|/tmp/p.png;'/tmp/q.png'"), "|"), "/tmp/r.png|/tmp/p.png|/tmp/q.png")
    C.Check.str("url", I.Img.url("/tmp/my board (1).png"), "/img?path=/tmp/my%20board%20%281%29.png")
    C.Check.str("url round trip", AT.Att.from_uri("file://" ++ String.drop(I.Img.url("/tmp/résumé 2.png"), 10n)), "/tmp/résumé 2.png")
    C.Check.str("message url binds entry", I.Img.message_url("m:1", "/tmp/x.png"), "/message-image?entry=m%3A1&path=/tmp/x.png")
    C.Check.str("message exact reference", Bool.show(I.Img.message_allowed("See `/tmp/x.png`", "/tmp/x.png")), "True")
    C.Check.str("message unreferenced image", Bool.show(I.Img.message_allowed("/tmp/x.png", "/tmp/y.png")), "False")
    C.Check.str("message traversal", Bool.show(I.Img.message_allowed("/tmp/../x.png", "/tmp/../x.png")), "False")
