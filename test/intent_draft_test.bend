import Base
import ./check.bend as C
import ../src/core/client.bend as U

def base() -> U.Ui:
  U.Ui.scratch_set(U.Ui.select.k(U.Ui.init(0n), "t"), "t", "ordinary 日本語 draft")

def edit(u: U.Ui) -> U.Ui:
  U.Act.ui(U.Fab.act.k(1n, u, "t"))

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
