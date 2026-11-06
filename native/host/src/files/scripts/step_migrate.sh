exec 2>/dev/null
reg=$1; shift
[ -e "$reg" ] && exit 0
ino() { stat -c '%d:%i' "$1" 2>/dev/null || stat -f '%d:%i' "$1"; }
: > "$reg.tmp" || exit 0
for top in "$@"; do
  gd=$top/.backplane/history.git
  [ -d "$gd" ] && [ ! -L "$gd" ] && [ ! -e "$gd/commondir" ] || continue
  l=$(git config --file "$gd/config" --name-only --list) || continue
  [ -z "$(printf '%s\n' "$l" | grep -v -E '^core[.](repositoryformatversion|filemode|bare|logallrefupdates|ignorecase|precomposeunicode|symlinks|worktree)$' | grep .)" ] || continue
  printf '%s %s\n' "$(ino "$gd")" "$gd" >> "$reg.tmp"
done
mv "$reg.tmp" "$reg"