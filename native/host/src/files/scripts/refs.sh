exec 2>/dev/null
cd "$1" || exit 0
name=$2
if top=$(git rev-parse --show-toplevel 2>/dev/null) && [ -n "$top" ]; then
  # git names the top by its real path (macOS: /private/var/... for /var/...); the History compares it with the path the person
  # opened, so keep the folder's path as we were given it: the same folder, reached the way "$1" reached it (not on Windows' C:/...)
  case $top in /*) lt=$(cd "./$(git rev-parse --show-cdup)" && pwd) && [ "$lt" != "$top" ] && [ "$(cd "$lt" && pwd -P)" = "$(cd "$top" && pwd -P)" ] && top=$lt ;; esac
  gd=$(cd "$(git rev-parse --git-common-dir)" && pwd) || exit 0
  printf '%s\n%s\n' "$gd" "$top"
  h=$(git rev-parse -q --verify HEAD) && printf 'head\tHEAD\t%s\n' "$h"
  for b in origin/HEAD origin/main origin/master main master; do
    m=$(git merge-base HEAD "$b" 2>/dev/null) || continue
    [ "$m" != "$h" ] && printf 'base\t%s\t%s\n' "$(git rev-parse --abbrev-ref "$b" 2>/dev/null || echo "$b")" "$m"
    break
  done
  git for-each-ref --sort=-committerdate --count=40 --format='branch%09%(refname:short)%09%(objectname)' refs/heads refs/remotes
  git for-each-ref --sort=-creatordate --count=40 --format='tag%09%(refname:short)%09%(if)%(*objectname)%(then)%(*objectname)%(else)%(objectname)%(end)' refs/tags
  git log -n 40 --format='commit%x09%h %s%x09%H' -- '*.kicad_[ps]c[bh]'
  git for-each-ref --sort=-committerdate --format='turn%09%(refname)%09%(objectname)' "refs/backplane/checkpoints/$name/turn/"
  git for-each-ref --sort=-committerdate --count=40 --format='snap%09%(refname)%09%(objectname)' "refs/backplane/snapshots/$name/" | awk -F'\t' '$2 ~ /\/[0-9]+$/'
else
  top=$(pwd)
  printf '%s\n%s\n' "$top/.backplane/history.git" "$top"
fi