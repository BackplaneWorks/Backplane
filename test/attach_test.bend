import Base
import ./check.bend as C
import ../src/core/attach.bend as A
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/sha1.bend as S

def show.list(xs: List<&2, String>) -> String:
  "[" ++ String.join(xs, "|") ++ "]"

def show.bytes(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      ""
    case b <> t:
      U32.show(b) ++ "," ++ show.bytes(t)

def show.cmds(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> rest:
      t ++ ";" ++ show.cmds(rest)
    case _ <> rest:
      show.cmds(rest)

def act.cmds(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      show.cmds(cs)

def act.ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def msg() -> String:
  "Look at this board\n\n[attachment: /h/.bp/attachments/t1/k1-board.png]\n[attachment: /h/.bp/attachments/t1/k2-notes-v2.pdf]\n"

# a client with thread t1 selected, one finished upload, and a draft
def ui.one() -> U.Ui:
  U.Ui.attached(act.ui(U.Ui.draft(act.ui(U.Ui.select(U.Ui.init(0n), "t1")), "hi")), "t1", "/h/a/t1/k-x.png")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("body", A.Att.body(msg()), "Look at this board")
    C.Check.str("paths", show.list(A.Att.paths(msg())), "[/h/.bp/attachments/t1/k1-board.png|/h/.bp/attachments/t1/k2-notes-v2.pdf]")
    C.Check.str("images", show.list(A.Att.images(msg())), "[/h/.bp/attachments/t1/k1-board.png]")
    C.Check.str("plain body", A.Att.body("no files here\n"), "no files here\n")
    C.Check.str("plain paths", show.list(A.Att.paths("no files here")), "[]")
    C.Check.str("inline line is text", show.list(A.Att.paths("[attachment: /a.png]\nthen text")), "[]")
    C.Check.str("only attachments", A.Att.body("[attachment: /a.png]"), "")
    C.Check.str("join", A.Att.join("hi", ["/a/b.png", "/c.txt"]), "hi\n\n[attachment: /a/b.png]\n[attachment: /c.txt]")
    C.Check.str("join empty text", A.Att.join("  ", ["/a/b.png"]), "[attachment: /a/b.png]")
    C.Check.str("join none", A.Att.join("hi", []), "hi")
    C.Check.str("roundtrip", show.list(A.Att.paths(A.Att.join("x\n[attachment: y]\nz", ["/p q/r.png"]))), "[/p q/r.png]")
    C.Check.str("roundtrip body", A.Att.body(A.Att.join("x\n[attachment: y]\nz", ["/p q/r.png"])), "x\n[attachment: y]\nz")
    C.Check.str("label", A.Att.label("/h/a/t1/c9a1-notes-v2.pdf"), "notes-v2.pdf")
    C.Check.str("label no key", A.Att.label("/tmp/plain.txt"), "plain.txt")
    C.Check.str("mime", A.Att.mime("/x/Shot.JPEG"), "image/jpeg")
    C.Check.str("not image", A.Att.mime("/x/a.pdf"), "")
    C.Check.str("safe", A.Att.safe("../../etc/passwd"), "_._.._etc_passwd")
    C.Check.str("safe dot", A.Att.safe(".."), "_.")
    C.Check.str("safe empty", A.Att.safe(""), "file")
    C.Check.str("safe unicode", A.Att.safe("é b.png"), "__b.png")
    C.Check.str("stored", A.Att.stored("c1-2", "My Shot.png"), "c1-2-My_Shot.png")
    C.Check.str("dir", A.Att.dir("/h", "t/../x"), "/h/attachments/t_.._x")
    C.Check.str("uri", A.Att.from_uri("file:///home/me/My%20Board%C3%A9.png\r"), "/home/me/My Boardé.png")
    C.Check.str("uri plain", A.Att.from_uri(" /tmp/a.png "), "/tmp/a.png")
    C.Check.str("pasted uris", show.list(A.Att.pasted("file:///a/b.txt\r\nfile:///c%20d.png\r\n")), "[/a/b.txt|/c d.png]")
    C.Check.str("pasted image path", show.list(A.Att.pasted("/tmp/shot.png")), "[/tmp/shot.png]")
    C.Check.str("pasted text", show.list(A.Att.pasted("/tmp/notes.txt")), "[]")
    C.Check.str("pasted words", show.list(A.Att.pasted("hello /tmp/a.png")), "[]")
    C.Check.str("b64", show.bytes(A.Att.b64("aGk/+w==")), "104,105,63,251,")
    C.Check.str("b64 url", show.bytes(A.Att.b64("aGk_-w")), "104,105,63,251,")
    C.Check.str("b64 roundtrip", show.bytes(A.Att.b64(S.B64.encode([0, 1, 2, 253, 254, 255, 7]))), "0,1,2,253,254,255,7,")
    C.Check.str("codex", show.list(A.Att.codex(msg())), "[--image|/h/.bp/attachments/t1/k1-board.png]")
    C.Check.str("claude", A.Att.claude("hi", [A.Img{"image/png", "QUJD"}]),
      "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"image\",\"source\":{\"type\":\"base64\",\"media_type\":\"image/png\",\"data\":\"QUJD\"}},{\"type\":\"text\",\"text\":\"hi\"}]}}")
    C.Check.str("client atts", show.list(U.Ui.atts(ui.one(), "t1")), "[/h/a/t1/k-x.png]")
    C.Check.str("client send", act.cmds(U.Ui.send(ui.one(), False{})),
      "{\"id\":0,\"m\":\"turn.start\",\"p\":{\"thread\":\"t1\",\"text\":\"hi\\n\\n[attachment: /h/a/t1/k-x.png]\",\"msg\":\"c0-0\",\"mode\":\"steer\"}};")
    C.Check.str("client sent clears", show.list(U.Ui.atts(act.ui(U.Ui.send(ui.one(), False{})), "t1")), "[]")
    C.Check.str("client detach", show.list(U.Ui.atts(act.ui(U.Ui.act(ui.one(), "detach", "/h/a/t1/k-x.png")), "t1")), "[]")
    C.Check.str("client rejects a byte over the 512 MiB default", act.cmds(U.Ui.act(ui.one(), "attach", "{\"key\":\"k\",\"name\":\"big.bin\",\"size\":536870913,\"i\":0,\"last\":true,\"data\":\"\"}")), "")
    C.Check.str("client piece", act.cmds(U.Ui.act(ui.one(), "attach", "{\"key\":\"k\",\"name\":\"a.png\",\"size\":3,\"i\":0,\"last\":true,\"data\":\"QUJD\"}")),
      "{\"id\":0,\"m\":\"attach.put\",\"p\":{\"thread\":\"t1\",\"key\":\"k\",\"name\":\"a.png\",\"i\":0,\"last\":true,\"data\":\"QUJD\"}};")
    C.Check.str("client reply", show.list(U.Ui.atts(U.Ui.recv(ui.one(), Maybe.default(&2, J.Json, J.Json.parse("{\"t\":\"reply\",\"id\":3,\"ok\":true,\"result\":{\"attached\":\"/h/a/t1/k-y.pdf\",\"thread\":\"t1\"}}"), J.Null{})), "t1")),
      "[/h/a/t1/k-x.png|/h/a/t1/k-y.pdf]")
    C.Check.str("client upload starts", U.Ui.uploading(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"big.mov\",\"size\":3}")), "t1"), "big.mov")
    C.Check.str("client upload ends", show.list(U.Ui.atts(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"attached\":\"/h/a/t1/k-y.pdf\",\"thread\":\"t1\"}")), "t1")),
      "[/h/a/t1/k-x.png|/h/a/t1/k-y.pdf]")
    C.Check.str("client upload ends shown", U.Ui.uploading(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"attached\":\"/h/a/t1/k-y.pdf\",\"thread\":\"t1\"}")), "t1"), "")
    C.Check.str("client upload accepts the 512 MiB default boundary", U.Ui.uploading(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"big.mov\",\"size\":536870912}")), "t1"), "big.mov")
    C.Check.str("client upload rejects a byte over the default", U.Ui.uploading(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"big.mov\",\"size\":536870913}")), "t1"), "")
    +small : U.Ui = act.ui(U.Ui.act(ui.one(), "setting", "files.max.mb=1"))
    C.Check.str("client upload accepts the configured boundary", U.Ui.uploading(act.ui(U.Ui.act(small, "attach-up", "{\"name\":\"small.mov\",\"size\":1048576}")), "t1"), "small.mov")
    C.Check.str("client upload rejects a byte over the configured limit", U.Ui.uploading(act.ui(U.Ui.act(small, "attach-up", "{\"name\":\"small.mov\",\"size\":1048577}")), "t1"), "")
    C.Check.str("client upload fails", U.Ui.uploading(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"text\":\"no\",\"thread\":\"t1\",\"upload\":true}")), "t1"), "")
    C.Check.str("client upload fails atts", show.list(U.Ui.atts(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"text\":\"no\",\"thread\":\"t1\",\"upload\":true}")), "t1")), "[/h/a/t1/k-x.png]")
    C.Check.str("phone upload answered under its hub ends", U.Ui.uploading(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"attached\":\"/h/a/t1/k-y.pdf\",\"thread\":\"h:1|t1\"}")), "t1"), "")
    C.Check.str("phone upload answered under its hub attaches", show.list(U.Ui.atts(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"attached\":\"/h/a/t1/k-y.pdf\",\"thread\":\"h:1|t1\"}")), "t1")), "[/h/a/t1/k-x.png|/h/a/t1/k-y.pdf]")
    C.Check.str("phone upload failed under its hub ends", U.Ui.uploading(act.ui(U.Ui.act(act.ui(U.Ui.act(ui.one(), "attach-up", "{\"name\":\"y.pdf\",\"size\":3}")), "attach-end", "{\"text\":\"no\",\"thread\":\"h:1|t1\",\"upload\":true}")), "t1"), "")
