set -eu
[ "$(git --git-dir="$3" rev-parse --verify --quiet "refs/tags/$4^{commit}")" = "$5" ] || { echo 'tag moved during export; no outputs published'; exit 1; }
[ ! -e "$2" ] || { echo 'outputs already exist; preserved unchanged'; exit 1; }
mv "$1" "$2"