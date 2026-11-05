#!/bin/bash
# Real temporary repositories (inside one mktemp -d, synthetic files only) for
# Git.ident.sh: which git directories are a folder's own repository.
#   ident_fs.sh SCRIPT_FILE   (the script test/ident_script.bend prints)
set -u
script=$(cat "$1")
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
fail=0
run() { sh -c "$script" sh "$1" "$2" 2>/dev/null; }
expect() { # name want(ok|no) cwd gd
  out=$(run "$3" "$4"); rc=$?
  if [ "$2" = ok ] && [ $rc -eq 0 ]; then echo "ok $1"
  elif [ "$2" = no ] && [ $rc -ne 0 ]; then echo "ok $1"
  else echo "FAIL $1: rc=$rc want=$2 out=$out"; fail=1; fi
}
commit() { (cd "$1" && git init -q . && echo x > a.kicad_sch && git add . && git commit -qm c); }
# canonical paths (the temp dir itself may sit behind a link)
t=$(cd "$t" && pwd -P)
mkdir "$t/proj" && commit "$t/proj"
expect "own repository" ok "$t/proj" "$t/proj/.git"
expect "own repository from a subfolder" ok "$t/proj" "$t/proj/.git"
mkdir "$t/proj/sub" && mkdir "$t/proj/sub/inner" && commit "$t/proj/sub/inner"
expect "a nested independent repository is its own" ok "$t/proj/sub/inner" "$t/proj/sub/inner/.git"
expect "the enclosing repository is not the nested one's" no "$t/proj/sub/inner" "$t/proj/.git"
expect "the nested repository is not the enclosing one's" no "$t/proj" "$t/proj/sub/inner/.git"
git -C "$t/proj" worktree add -q "$t/ext-wt" -b b2 2>/dev/null
expect "an external linked worktree: the common directory" ok "$t/ext-wt" "$t/proj/.git"
expect "an external linked worktree: its own directory" ok "$t/ext-wt" "$t/proj/.git/worktrees/ext-wt"
expect "the worktree is not another repository's" no "$t/ext-wt" "$t/proj/sub/inner/.git"
mkdir "$t/sep" && (cd "$t/sep" && git init -q --separate-git-dir "$t/sepgit" . && echo x > a.kicad_sch && git add . && git commit -qm c)
expect "a --separate-git-dir repository" ok "$t/sep" "$t/sepgit"
expect "and not by the folder's path" no "$t/sep" "$t/sep/.git"
ln -s "$t/proj" "$t/alias"
expect "a symlinked folder, the real git dir" ok "$t/alias" "$t/proj/.git"
expect "a symlinked folder, the link's git dir" ok "$t/alias" "$t/alias/.git"
ln -s "$t/proj/.git" "$t/gitlink"
expect "a link to the git dir" ok "$t/proj" "$t/gitlink"
mkdir "$t/plain"
expect "a folder with no repository: its shadow history" ok "$t/plain" "$t/plain/.backplane/history.git"
expect "a folder with no repository: another repository" no "$t/plain" "$t/proj/.git"
mkdir -p "$t/plain/.backplane" && git init -q --bare "$t/plain/.backplane/history.git"
expect "the shadow history once it exists" ok "$t/plain" "$t/plain/.backplane/history.git"
expect "a git folder's shadow history" ok "$t/proj" "$t/proj/.backplane/history.git"
expect "a relative git directory" no "$t/proj" ".git"
expect "a relative folder" no "proj" "$t/proj/.git"
expect "no such folder" no "$t/missing" "$t/proj/.git"
expect "an unrelated git directory" no "$t/proj" "$t/sepgit"
expect "an option-like directory" no "$t/proj" "--git-dir=$t/proj/.git"
out=$(run "$t/ext-wt" "$t/proj/.git"); top=$(printf '%s\n' "$out" | sed -n 1p)
[ "$top" = "$t/ext-wt" ] && echo "ok the top is the worktree's" || { echo "FAIL top: $top"; fail=1; }
out=$(run "$t/proj/sub/inner" "$t/proj/sub/inner/.git"); pre=$(printf '%s\n' "$out" | sed -n 2p)
[ -z "$pre" ] && echo "ok an inner repository's folder is its top" || { echo "FAIL prefix: $pre"; fail=1; }
mkdir "$t/proj/deep" && echo y > "$t/proj/deep/b.kicad_sch"
out=$(run "$t/proj/deep" "$t/proj/.git"); pre=$(printf '%s\n' "$out" | sed -n 2p)
[ "$pre" = "deep/" ] && echo "ok the folder's place under the top" || { echo "FAIL prefix: $pre"; fail=1; }
exit $fail
