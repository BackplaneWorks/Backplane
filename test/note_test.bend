import Base
import ./check.bend as C
import ../src/core/i18n.bend as I
import ../src/core/images.bend as IM
import ../src/web/dom.bend as D
import ../src/web/note.bend as NT

# A Markdown note in Files (web/note.bend): its paths from its own folder,
# its images shown, files Files previews opened there.

def attrs(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}:
      ""
    case D.At{n, v} <> t:
      " " ++ n ++ "=" ++ v ++ attrs(t)
    case D.On{e, a} <> t:
      " on" ++ e ++ "=" ++ a ++ attrs(t)

def shows(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}:
      ""
    case D.Txt{s} <> t:
      s ++ shows(t)
    case D.El{+tag, _, as, ks} <> t:
      "<" ++ tag ++ attrs(as) ++ ">" ++ shows(ks) ++ "</" ++ tag ++ ">" ++ shows(t)

def note(+path: String, +text: String) -> String:
  shows(NT.Note.render(I.En{}, "/w/band", path, text))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a path from the note's folder", NT.Note.path("docs", "media/a.png"), "docs/media/a.png")
    C.Check.str("up from the note's folder", NT.Note.path("docs/rev", "../../README.md"), "README.md")
    C.Check.str("a path leaving the project", NT.Note.path("docs", "../../x.md"), "")
    C.Check.str("an anchor is dropped", NT.Note.path("", "./docs/x.md#part"), "docs/x.md")
    C.Check.str("a place in the page names no file", NT.Note.path("docs", "#part"), "")
    C.Check.str("a note's folder", String.join([NT.Note.dir("/w/band", "/w/band/docs/a.md"), NT.Note.dir("/w/band", "/w/band/README.md")], "|"), "docs|")
    C.Check.str("an image is shown", note("/w/band/README.md", "![on the wrist](docs/hero.jpg)"),
      "<p><a class=fm-md-img href=" ++ IM.Img.url("/w/band/docs/hero.jpg") ++ " data-lightbox=" ++ IM.Img.url("/w/band/docs/hero.jpg") ++ " title=on the wrist><img src=" ++ IM.Img.url("/w/band/docs/hero.jpg") ++ " alt=on the wrist loading=lazy></img></a></p>")
    C.Check.str("an image in a table, from a note's folder", note("/w/band/docs/a.md", "| x |\n|---|\n| ![](media/b.png) |"),
      "<table><tr><th>x</th></tr><tr><td><a class=fm-md-img href=" ++ IM.Img.url("/w/band/docs/media/b.png") ++ " data-lightbox=" ++ IM.Img.url("/w/band/docs/media/b.png") ++ " title=><img src=" ++ IM.Img.url("/w/band/docs/media/b.png") ++ " alt= loading=lazy></img></a></td></tr></table>")
    C.Check.str("text around an image stays", note("/w/band/README.md", "see ![a](b.png) here!"),
      "<p>see <a class=fm-md-img href=" ++ IM.Img.url("/w/band/b.png") ++ " data-lightbox=" ++ IM.Img.url("/w/band/b.png") ++ " title=a><img src=" ++ IM.Img.url("/w/band/b.png") ++ " alt=a loading=lazy></img></a> here!</p>")
    C.Check.str("a web image is a link, never loaded", note("/w/band/README.md", "![x](https://example.com/x.png)"),
      "<p>!<a class=link href=https://example.com/x.png target=_blank rel=noopener noreferrer title=https://example.com/x.png>x</a></p>")
    C.Check.str("an image outside the project stays as written", note("/w/band/README.md", "![x](../other/x.png)"), "<p>![x](../other/x.png)</p>")
    C.Check.str("another note opens in Files", note("/w/band/docs/a.md", "[cost](rev_c/cost.md)"),
      "<p><a class=link file href=docs/rev_c/cost.md title=docs/rev_c/cost.md onclick=fm-open data-value=docs/rev_c/cost.md>cost</a></p>")
    C.Check.str("a script opens on the machine", note("/w/band/README.md", "run `tools/copper.py`"),
      "<p>run <a class=link file href=/w/band/tools/copper.py title=/w/band/tools/copper.py onclick=file-open oncontextmenu=file-menu data-value=/w/band/tools/copper.py><code>tools/copper.py</code></a></p>")
    C.Check.str("a place in the page is its label", note("/w/band/README.md", "See [what's next](#status-and-next-steps)."), "<p>See what's next.</p>")
    C.Check.str("a web link stays", note("/w/band/README.md", "[site](https://example.com)"),
      "<p><a class=link href=https://example.com target=_blank rel=noopener noreferrer title=https://example.com>site</a></p>")
    C.Check.str("code keeps its text", note("/w/band/README.md", "```\n![x](a.png)\n```"),
      "<div class=codeblock><button class=copy title=Copy data-value=![x](a.png) onclick=copy>Copy</button><pre><code>![x](a.png)</code></pre></div>")
    C.Check.str("brackets that are no link stay", note("/w/band/README.md", "a [b] c ! d ![e"), "<p>a [b] c ! d ![e</p>")
