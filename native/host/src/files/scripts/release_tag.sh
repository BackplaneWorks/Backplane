set -eu
cd "$1"
[ "$(git rev-parse HEAD)" = "$3" ] || { echo "HEAD moved during validation; no tag made, retry to validate the new commit"; exit 1; }
if git show-ref --verify --quiet "refs/tags/$2"; then
  [ "$(git rev-parse "$2^{commit}")" = "$3" ] || { echo "tag already names a different commit"; exit 1; }
else
  git tag -a "$2" "$3" -m "Release $2"
fi