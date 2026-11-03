import Base
import ./check.bend as C
import ../src/core/diff.bend as D

# `git diff --cached --no-color -M` over: a new file, a binary change, a
# deleted file, two hunks (one deletes a line that starts "-- "), a rename
# with an edit, and a new file with no final newline
def fixture() -> String:
  "diff --git a/add.txt b/add.txt
new file mode 100644
index 0000000..92d5444
--- /dev/null
+++ b/add.txt
@@ -0,0 +1 @@
+fresh
diff --git a/b.bin b/b.bin
index 88768ef..3e3315e 100644
Binary files a/b.bin and b/b.bin differ
diff --git a/del.txt b/del.txt
deleted file mode 100644
index 286c5f5..0000000
--- a/del.txt
+++ /dev/null
@@ -1 +0,0 @@
-gone
diff --git a/m.txt b/m.txt
index 8afd661..d7ef15d 100644
--- a/m.txt
+++ b/m.txt
@@ -1,6 +1,5 @@
 l1
-l2
+L2
--- tricky
 l3
 l4
 l5
@@ -11,5 +10,6 @@ l10
 l11
 l12
 l13
-l14
+-- l14
 l15
+l16
diff --git a/old.txt b/new.txt
similarity index 85%
rename from old.txt
rename to new.txt
index b2f931a..b566061 100644
--- a/old.txt
+++ b/new.txt
@@ -3,3 +3,4 @@ two
 three
 four
 five
+six
diff --git a/nonl.txt b/nonl.txt
new file mode 100644
index 0000000..c1b0730
--- /dev/null
+++ b/nonl.txt
@@ -0,0 +1 @@
+x
\\ No newline at end of file
"

# a pure rename (no hunks) and a mode change, as git prints them
def fixture2() -> String:
  "diff --git a/a b/c.txt b/a b/d.txt
similarity index 100%
rename from a b/c.txt
rename to a b/d.txt
diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
"

def show.line(l: D.DLine) -> String:
  match l:
    case D.DLine{k, t, o, n}:
      U32.show(k) ++ ":" ++ Nat.show(o) ++ ":" ++ Nat.show(n) ++ ":" ++ t

def show.lines(ls: List<&2, D.DLine>, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      show.lines(t, acc ++ "|" ++ show.line(l))

def show.hunk(h: D.DHunk) -> String:
  match h:
    case D.DHunk{hd, ls}:
      "[" ++ hd ++ show.lines(ls, "") ++ "]"

def show.hunks(hs: List<&2, D.DHunk>, acc: String) -> String:
  match hs:
    case Nil{}:
      acc
    case h <> t:
      show.hunks(t, acc ++ show.hunk(h))

def show.file(f: D.DFile) -> String:
  match f:
    case D.DFile{o, n, s, hs}:
      s ++ " " ++ o ++ " > " ++ n ++ " " ++ show.hunks(hs, "")

def show.some(m: Maybe<&2, D.DFile>) -> String:
  match m:
    case Some{f}:
      show.file(f)
    case None{}:
      "(missing)"

def show.at(fs: List<&2, D.DFile>, n: Nat) -> String:
  show.some(List.get(&2, D.DFile, fs, n))

def show.isbin(m: Maybe<&2, D.DFile>) -> String:
  match m:
    case Some{f}:
      Bool.show(D.Diff.binary(f))
    case None{}:
      "(missing)"

def show.bin(fs: List<&2, D.DFile>, n: Nat) -> String:
  show.isbin(List.get(&2, D.DFile, fs, n))

def show.stats(s: D.DStats) -> String:
  match s:
    case D.DStats{f, a, d}:
      Nat.show(f) ++ " " ++ Nat.show(a) ++ " " ++ Nat.show(d)

# a one-file diff with n added lines, built back to front
def big.go(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      "diff --git a/big b/big\n--- a/big\n+++ b/big\n@@ -0,0 +1,20000 @@\n" ++ acc
    case 1n+p:
      big.go(p, "+line\n" ++ acc)

def main() -> IO(Unit):
  +fs = D.Diff.parse(fixture())
  +gs = D.Diff.parse(fixture2())
  do IO<Unit>:
    C.Check.str("diff files", Nat.show(List.length(&2, D.DFile, fs)), "6")
    C.Check.str("diff added", show.at(fs, 0n), "added add.txt > add.txt [@@ -0,0 +1 @@|1:0:1:fresh]")
    C.Check.str("diff binary", show.at(fs, 1n), "modified b.bin > b.bin [|3:0:0:Binary files a/b.bin and b/b.bin differ]")
    C.Check.str("diff binary flag", show.bin(fs, 1n), "True")
    C.Check.str("diff text flag", show.bin(fs, 0n), "False")
    C.Check.str("diff deleted", show.at(fs, 2n), "deleted del.txt > del.txt [@@ -1 +0,0 @@|2:1:0:gone]")
    C.Check.str("diff hunks", show.at(fs, 3n), "modified m.txt > m.txt [@@ -1,6 +1,5 @@|0:1:1:l1|2:2:0:l2|1:0:2:L2|2:3:0:-- tricky|0:4:3:l3|0:5:4:l4|0:6:5:l5][@@ -11,5 +10,6 @@ l10|0:11:10:l11|0:12:11:l12|0:13:12:l13|2:14:0:l14|1:0:13:-- l14|0:15:14:l15|1:0:15:l16]")
    C.Check.str("diff rename", show.at(fs, 4n), "renamed old.txt > new.txt [@@ -3,3 +3,4 @@ two|0:3:3:three|0:4:4:four|0:5:5:five|1:0:6:six]")
    C.Check.str("diff no newline", show.at(fs, 5n), "added nonl.txt > nonl.txt [@@ -0,0 +1 @@|1:0:1:x|3:0:0:\\ No newline at end of file]")
    C.Check.str("diff stats", show.stats(D.Diff.stats(fs)), "6 6 4")
    C.Check.str("diff summary", D.Diff.summary(fs), "6 files, +6 −4")
    C.Check.str("diff summary one", D.Diff.summary(D.Diff.parse(fixture())), "6 files, +6 −4")
    # a capped diff (git.bend's Git.diff.cap) counts what it left out
    C.Check.str("a capped diff's summary counts what it left out", D.Diff.summary(D.Diff.parse("diff --git a/big.step b/big.step\n--- a/big.step\n+++ b/big.step\n@@ -0,0 +0,0 @@\n\\ Too big to show here: +412827 -399001 lines\ndiff --git a/s.txt b/s.txt\n--- a/s.txt\n+++ b/s.txt\n@@ -1 +1 @@\n-a\n+b\n\\ 1638 more changed files not shown here\n")), "1640 files, +412828 −399002")
    C.Check.str("git's own meta line counts nothing", D.Diff.summary(D.Diff.parse("diff --git a/s.txt b/s.txt\n--- a/s.txt\n+++ b/s.txt\n@@ -1 +1 @@\n-a\n+b\n\\ No newline at end of file\n")), "1 file, +1 −1")
    C.Check.str("diff pure rename", show.at(gs, 0n), "renamed a b/c.txt > a b/d.txt ")
    C.Check.str("diff mode change", show.at(gs, 1n), "modified run.sh > run.sh ")
    C.Check.str("diff empty", D.Diff.summary(D.Diff.parse("")), "0 files, +0 −0")
    C.Check.str("diff one file", D.Diff.summary(D.Diff.parse("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n")), "1 file, +1 −1")
    C.Check.str("diff big", D.Diff.summary(D.Diff.parse(big.go(20000n, ""))), "1 file, +20000 −0")
