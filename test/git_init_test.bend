import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/server/git.bend as G
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as WP

# A project folder that is no git repository (Diff's "Not a git
# repository"): besides the commands, a button runs them (git.act "init":
# git init, Backplane's folder excluded, the folder committed as it is),
# so new threads get worktrees of their own.

def names(xs: List<&2, G.AStep>) -> String:
  match xs:
    case Nil{}: ""
    case G.AInit{} <> t: "init " ++ names(t)
    case G.ACommit{} <> t: "commit " ++ names(t)
    case G.APush{} <> t: "push " ++ names(t)
    case G.APr{} <> t: "pr " ++ names(t)

def steps.k(m: Maybe<&2, List<&2, G.AStep>>) -> String:
  match m:
    case Some{xs}: names(xs)
    case None{}: "none"

def steps(+a: String) -> String:
  steps.k(G.Git.act.steps(a))

def html(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("init: one step (git init, exclude, commit)", steps("init"), "init ")
    C.Check.str("the others stay", steps("commit_push") ++ "|" ++ steps("nonsense"), "commit push |none")
    C.Check.str("the repository leaves .backplane out, and a repository is left alone", Bool.show(String.contains(G.Git.init.sh(), "/.backplane/")) ++ Bool.show(String.starts_with(G.Git.init.sh(), "if git rev-parse --git-dir")), "TrueTrue")
    C.Check.str("no identity: nothing changes (checked before git init)", Bool.show(String.contains(G.Git.init.sh(), "git var GIT_COMMITTER_IDENT >/dev/null 2>&1; then echo 'git does not know your name")), "True")
    C.Check.str("Diff offers the button", Bool.show(String.contains(html(V.View.diff.nogit(I.En{}, "/work/x")), "Make it a git repository")), "True")
    C.Check.str("it runs git.act init", Bool.show(String.contains(html(V.View.diff.nogit(I.En{}, "/work/x")), "\"init\"")), "True")
