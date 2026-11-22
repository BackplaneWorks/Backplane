bp_safe() { n=${GIT_CONFIG_COUNT:-0}; ks=$(git config --show-scope --name-only --get-regexp '^filter[.].*[.](clean|smudge|process)$' 2>/dev/null | awk -F '	' '$1 == "local" || $1 == "worktree" { print $2 }'); o=$IFS; IFS='
'; for k in $ks; do export "GIT_CONFIG_KEY_$n=$k" "GIT_CONFIG_VALUE_$n="; n=$((n+1)); done; IFS=$o; export GIT_CONFIG_COUNT=$n; }; set -e
t=$(mktemp -d "${TMPDIR:-/tmp}/bp-review.XXXXXX")
trap 'rm -rf "$t"' 0
w() { if command -v cygpath >/dev/null 2>&1; then cygpath -m -- "$1"; else printf '%s\n' "$1"; fi; }
wt=$(w "$t")
if [ -n "$2" ]; then
  cd "$1"
  p=$(git rev-parse --show-prefix 2>/dev/null || true)
  mkdir "$t/src"
  bp_safe
  git archive --format=tar "$2" -o "$t/source.tar"
  tar -xf - -C "$t/src" < "$t/source.tar"
  links=$(find "$t/src" -type l -print)
  [ -z "$links" ] || { echo 'snapshot contains symlinks; release inputs must be committed regular files'; exit 1; }
  wd=$(w "$t/src/$p")
  wr=$(w "$t/src")
  [ -n "$wt" ] && [ -n "$wd" ] && [ -n "$wr" ] || exit 1
  printf '%s\n%s\n%s\n' "$wt" "$wd" "$wr"
else
  wd=$(w "$1")
  [ -n "$wt" ] && [ -n "$wd" ] || exit 1
  printf '%s\n%s\n' "$wt" "$wd"
fi
trap - 0