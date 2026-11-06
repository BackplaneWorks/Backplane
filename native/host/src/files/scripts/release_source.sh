set -eu
cd "$1"
c=$(git rev-parse --verify HEAD^{commit})
top=$(git rev-parse --show-toplevel)
cd "$top"
git diff --quiet && git diff --cached --quiet || { echo 'commit or stash working changes before releasing; checks and exports use a single committed snapshot'; exit 1; }
dirty=$(git status --porcelain --untracked-files=all -- . ":(exclude).backplane/releases" ":(glob,exclude)**/.backplane/releases/**" ":(exclude).backplane/worktrees" ":(glob,exclude)**/.backplane/worktrees/**" ":(exclude).backplane/history.git" ":(glob,exclude)**/.backplane/history.git/**")
[ -z "$dirty" ] || { echo "commit or stash working changes before releasing; checks and exports use a single committed snapshot"; exit 1; }
printf '%s\n' "$c"