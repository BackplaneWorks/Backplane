import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as WP

# Diff shows at most V.View.diff.lines() lines in all (view.bend): a view
# page is at most 256 KB, and a diff of a few MB showed nothing at all.
# Every file keeps its head; one cut short says how many lines are left out.

def html(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

# n added lines "+line k"
def adds(+n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      adds(k, "+line " ++ Nat.show(n) ++ "\n" ++ acc)

def big() -> String:
  "diff --git a/big.txt b/big.txt\nnew file mode 100644\n--- /dev/null\n+++ b/big.txt\n@@ -0,0 +1,700 @@\n" ++ adds(700n, "")
    ++ "diff --git a/small.txt b/small.txt\n--- a/small.txt\n+++ b/small.txt\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n"

# n small files
def many(+n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      many(k, "diff --git a/f" ++ Nat.show(n) ++ ".txt b/f" ++ Nat.show(n) ++ ".txt\n--- a/f" ++ Nat.show(n) ++ ".txt\n+++ b/f" ++ Nat.show(n) ++ ".txt\n@@ -1 +1 @@\n-a\n+b\n" ++ acc)

def small() -> String:
  "diff --git a/small.txt b/small.txt\n--- a/small.txt\n+++ b/small.txt\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n"

def main() -> IO(Unit):
  do IO<Unit>:
    +h : String = html(V.View.diff.loc(I.En{}, big()))
    C.Check.str("the first lines are shown", Bool.show(String.contains(h, "line 120\"")), "True")
    C.Check.str("past the budget they are not", Bool.show(String.contains(h, "line 121\"")), "False")
    C.Check.str("the file says how many are left out", Bool.show(String.contains(h, "580 more lines not shown here")), "True")
    C.Check.str("a later file keeps its head", Bool.show(String.contains(h, "small.txt")), "True")
    C.Check.str("and says its lines are left out too", Bool.show(String.contains(h, "4 more lines not shown here")), "True")
    C.Check.str("in Japanese too", Bool.show(String.contains(html(V.View.diff.loc(I.Ja{}, big())), "あと580行")), "True")
    +m : String = html(V.View.diff.loc(I.En{}, many(200n, "")))
    C.Check.str("the first files are listed", Bool.show(String.contains(m, "f30.txt")), "True")
    C.Check.str("past the files' budget they are counted", Bool.show(Bool.and(Bool.not(String.contains(m, "f31.txt")), String.contains(m, "170 more changed files not shown here"))), "True")
    C.Check.str("a small diff is whole", Bool.show(Bool.and(String.contains(html(V.View.diff.loc(I.En{}, small())), "\"B\""), Bool.not(String.contains(html(V.View.diff.loc(I.En{}, small())), "not shown here")))), "True")
