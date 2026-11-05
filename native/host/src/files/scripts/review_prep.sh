bp_safe() { n=${GIT_CONFIG_COUNT:-0}; ks=$(git config --show-scope --name-only --get-regexp '^filter[.].*[.](clean|smudge|process)$' 2>/dev/null | awk -F '	' '$1 == "local" || $1 == "worktree" { print $2 }'); o=$IFS; IFS='
'; for k in $ks; do export "GIT_CONFIG_KEY_$n=$k" "GIT_CONFIG_VALUE_$n="; n=$((n+1)); done; IFS=$o; export GIT_CONFIG_COUNT=$n; }; set -e
t=$(mktemp -d "${TMPDIR:-/tmp}/bp-review.XXXXXX")
trap 'rm -rf "$t"' 0
if [ -n "$2" ]; then
  cd "$1"
  p=$(git rev-parse --show-prefix 2>/dev/null || true)
  mkdir "$t/src"
  bp_safe
  git archive --format=tar "$2" -o "$t/source.tar"
  tar -xf "$t/source.tar" -C "$t/src"
  links=$(find "$t/src" -type l -print)
  [ -z "$links" ] || { echo 'snapshot contains symlinks; release inputs must be committed regular files'; exit 1; }
  printf '%s\n%s\n%s\n' "$t" "$t/src/$p" "$t/src"
else
  printf '%s\n%s\n' "$t" "$1"
fi
trap - 0