bp_safe() { n=${GIT_CONFIG_COUNT:-0}; ks=$(git config --show-scope --name-only --get-regexp '^filter[.].*[.](clean|smudge|process)$' 2>/dev/null | awk -F '	' '$1 == "local" || $1 == "worktree" { print $2 }'); o=$IFS; IFS='
'; for k in $ks; do export "GIT_CONFIG_KEY_$n=$k" "GIT_CONFIG_VALUE_$n="; n=$((n+1)); done; IFS=$o; export GIT_CONFIG_COUNT=$n; }; set -eu
cd "$1"
bp_safe
repo=$(pwd)
top=$(git rev-parse --show-toplevel)
gd=$(cd "$(git rev-parse --git-common-dir)" && pwd)
p=$(git rev-parse --show-prefix)
c=$(git rev-parse --verify --quiet "${3:-$2}^{commit}")
object=$(git rev-parse --verify "refs/tags/$2")
[ "$(git rev-parse --verify --quiet "$object^{commit}")" = "$c" ] || { echo 'release tag moved; retry validation'; exit 1; }
prev=$(git describe --tags --abbrev=0 "$c^" 2>/dev/null || true)
pc=$( [ -n "$prev" ] && git rev-parse --verify --quiet "$prev^{commit}" || true)
final="$repo/.backplane/releases/$2"
mkdir -p "$(dirname "$final")"
lock="$final.lock"
mkdir "$lock" || { echo "release is already running; retry after it completes (or inspect the stale lock: $lock)"; exit 1; }
t=""; stage=""
trap '[ -z "$t" ] || rm -rf "$t"; [ -z "$stage" ] || rm -rf "$stage"; rmdir "$lock"' 0
[ ! -e "$final" ] || { echo "release outputs already exist; preserved unchanged, choose a new tag"; exit 1; }
t=$(mktemp -d "${TMPDIR:-/tmp}/bp-release.XXXXXX")
stage=$(mktemp -d "$final.pending.XXXXXX")
git archive --format=tar "$c" -o "$t/source.tar"
mkdir "$t/src"
tar -xf "$t/source.tar" -C "$t/src"
links=$(find "$t/src" -type l -print)
[ -z "$links" ] || { echo 'snapshot contains symlinks; release inputs must be committed regular files'; exit 1; }
printf '%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n' "$t" "$t/src/$p" "$gd" "$top" "$c" "$prev" "$pc" "$final" "$stage" "$lock" "$object"
trap - 0