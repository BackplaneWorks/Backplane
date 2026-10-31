import Base
import ./check.bend as C
import ../src/core/client.bend as U

def base() -> U.Ui:
  U.Ui.scratch_set(U.Ui.select.k(U.Ui.init(0n), "t"), "t", "ordinary 日本語 draft")

def edit(u: U.Ui) -> U.Ui:
  U.Act.ui(U.Fab.act.k(1n, u, "t"))

def keeps(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Keep{+t, +x} <> tail: "[" ++ t ++ "=" ++ x ++ "]" ++ keeps(tail)
    case _ <> tail: keeps(tail)

def edited() -> U.Act:
  U.Fab.act.k(1n, base(), "t")

# the page after a reload: what its Keeps left (the composer's /intent text,
# the borrowed draft and the mark), in a fresh client
def reloaded() -> U.Ui:
  U.Ui.restore(U.Ui.restore(U.Ui.restore(U.Ui.select.k(U.Ui.init(0n), "t"), "t", "/intent\n# Goal\nLamp"), "@intent.draft:t", "ordinary 日本語 draft"), "@intent.edit:t", "1")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Edit fills the composer", U.Ui.draft_text(edit(base())), "/intent\n# Goal\n\n# Parts\n\n# Notes\n")
    C.Check.str("Save restores the ordinary draft", U.Ui.draft_text(U.Act.ui(U.Intent.send(edit(base())))), "ordinary 日本語 draft")
    C.Check.str("Edit twice still restores the original", U.Ui.draft_text(U.Act.ui(U.Intent.send(edit(edit(base()))))), "ordinary 日本語 draft")
    C.Check.str("a later ordinary draft becomes the next backup", U.Ui.draft_text(U.Act.ui(U.Intent.send(edit(U.Ui.scratch_set(edit(base()), "t", "later draft"))))), "later draft")
    C.Check.str("sending ordinary text clears an abandoned intent backup", U.Ui.scratch(U.Act.ui(U.Draft.clear(edit(base()))), "@intent.draft:t"), "")
    C.Check.str("sending ordinary text clears the intent edit marker", U.Ui.scratch(U.Act.ui(U.Draft.clear(edit(base()))), "@intent.edit:t"), "")
    C.Check.str("Save clears its backup", U.Ui.scratch(U.Act.ui(U.Intent.send(edit(base()))), "@intent.draft:t"), "")
    C.Check.str("Save clears its edit marker", U.Ui.scratch(U.Act.ui(U.Intent.send(edit(base()))), "@intent.edit:t"), "")
    C.Check.str("typing intent directly returns an empty draft", U.Ui.draft_text(U.Act.ui(U.Intent.send(U.Ui.scratch_set(U.Ui.select.k(U.Ui.init(0n), "t"), "t", "/intent\n# Goal\nLamp")))), "")
    C.Check.str("Edit has the page keep the borrowed draft and the mark", keeps(U.Act.cmds(edited())), "[t=/intent\n# Goal\n\n# Parts\n\n# Notes\n][@intent.draft:t=ordinary 日本語 draft][@intent.edit:t=1]")
    C.Check.str("Save after a reload gives the conversation's draft back", U.Ui.draft_text(U.Act.ui(U.Intent.send(reloaded()))), "ordinary 日本語 draft")
    C.Check.str("Save has the page forget the borrowed draft", Bool.show(String.contains(keeps(U.Act.cmds(U.Intent.send(reloaded()))), "[t=ordinary 日本語 draft][@intent.draft:t=][@intent.edit:t=]")), "True")
    C.Check.str("an ordinary message over an abandoned edit forgets it on the page too", Bool.show(String.contains(keeps(U.Act.cmds(U.Ui.send.t(U.Ui.scratch_set(U.Act.ui(edited()), "t", "plain"), "plain", ""))), "[@intent.draft:t=][@intent.edit:t=]")), "True")
    C.Check.str("an ordinary message without an edit keeps only its own Keep", keeps(U.Act.cmds(U.Ui.send.t(base(), "x", ""))), "[t=]")
