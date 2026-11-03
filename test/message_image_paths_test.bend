import Base
import ./check.bend as C
import ../src/core/images.bend as I
import ../src/server/message-image.bend as MI

def parity(+name: String, +text: String) -> IO(Unit):
  C.Check.str(name, String.join(MI.MessageImage.candidates(text), "\n"), String.join(I.Img.paths(text), "\n"))

def main() -> IO(Unit):
  do IO<Unit>:
    parity("ordinary streaming reply has no candidate images", "Done: inspected all twenty agents and 1000 settled threads; no changes needed.")
    parity("decimal and near-extension words remain negative", "3.1415 image.pn image.jp image.jpe image.web image.bm /tmp/nothing.pngz")
    parity("absolute mixed-case and file URLs keep paths", "`/tmp/a.PNG` file:///tmp/b.JpEg /tmp/c.weBp")
    parity("markdown images and punctuation remain exact", "![board](/tmp/board.png) /tmp/photo.jpg. /tmp/panel.gif! /tmp/mask.bmp?")
    parity("tilde Unicode paths and duplicates remain exact", "~/日本/基板.png /tmp/🧰.jpg ~/日本/基板.png")
    parity("extensions alone do not grant a path", "png .jpg image.jpeg http://example.invalid/a.png relative/a.bmp")
