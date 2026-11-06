exec 2>/dev/null
cd "$1" || exit 1
top=$(git rev-parse --show-toplevel) || exit 1
gd=$(cd "$(git rev-parse --git-common-dir)" && pwd) || exit 1
h=$(git rev-parse HEAD) || exit 1
for b in origin/HEAD origin/main origin/master main master; do
  m=$(git merge-base HEAD "$b") || continue
  [ "$m" != "$h" ] && { printf '%s\n%s\n%s\n%s\n' "$gd" "$top" "$m" "$h"; exit 0; }
done
exit 1