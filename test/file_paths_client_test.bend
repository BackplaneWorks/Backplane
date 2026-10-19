import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def base() -> U.Ui:
  U.Ui.select.k(U.Ui.fold([M.ProjectCreated{"p", "Example", "/tmp/file-path-example", 1n}, M.ThreadCreated{"t", "p", "Files", M.Local{}, "", "", "claude", "", 2n}], U.Ui.init(10n)), "t")

def paths() -> List<&2, String>:
  ["café.step", "line\nbreak.step", "back\\slash.step", "quote\".step", " "]

def reply() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"1"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{"t"}}, J.KV{"files", J.Json.arr([J.Str{"café.step"}, J.Str{"line\nbreak.step"}, J.Str{"back\\slash.step"}, J.Str{"quote\".step"}, J.Str{" "}])}])}])

def loaded() -> U.Ui:
  U.Ui.recv(base(), reply())

def values(xs: List<&2, U.PItem>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case U.PItem{_, _, p} <> rest: J.Str{p} <> values(rest)

def picked(u: U.Ui, q: String) -> String:
  J.Json.show(J.Json.arr(values(U.Files.items(U.Palette.set(u, q, 0n)))))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("file reply marks the lossless record format", U.Ui.info(loaded(), "files.format"), "nul")
    C.Check.str("file reply preserves the owning thread", U.Ui.info(loaded(), "files.thread"), "t")
    C.Check.str("a newline filename is one palette payload", picked(loaded(), "break"), "[\"line\\nbreak.step\"]")
    C.Check.str("Unicode reaches the palette unchanged", picked(loaded(), "café"), "[\"café.step\"]")
    C.Check.str("backslashes reach the palette unchanged", picked(loaded(), "slash"), "[\"back\\\\slash.step\"]")
    C.Check.str("quotes reach the palette unchanged", picked(loaded(), "quote"), "[\"quote\\\".step\"]")
    C.Check.str("files belonging to another thread are hidden", picked(U.Ui.select.k(loaded(), "other"), ""), "[]")
    C.Check.str("legacy kept newline lists are readable", picked(U.Ui.info_set(U.Ui.info_set(base(), "files.thread", "t"), "files", "legacy.step\nother.step"), "legacy"), "[\"legacy.step\"]")
    C.Check.str("an empty listing replaces old records", picked(U.Ui.files(loaded(), J.Json.obj([J.KV{"thread", J.Str{"t"}}, J.KV{"files", J.Json.arr(Nil{})}])), ""), "[]")
    C.Check.str("mention payload retains embedded newline", U.Ui.draft_text(U.Act.ui(U.Ui.act(base(), "mention", "line\nbreak.step"))), "@line\nbreak.step ")
