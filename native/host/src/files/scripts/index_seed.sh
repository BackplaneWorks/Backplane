set -eu
src=$(git rev-parse --path-format=absolute --git-path index)
if [ ! -f "$src" ]; then rm -f "$1"; exit 0; fi
paths=$(mktemp "$1.paths.XXXXXX")
trap 'rm -f "$paths"' 0
git ls-files --full-name --stage -z -- :/ > "$paths"
export GIT_INDEX_FILE="$1"
rm -f "$1"
git read-tree --empty
git -c core.ignorestat=false update-index -z --index-info < "$paths"