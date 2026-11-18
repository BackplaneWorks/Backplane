exec 2>/dev/null
bp_safe() { n=${GIT_CONFIG_COUNT:-0}; ks=$(git config --show-scope --name-only --get-regexp '^filter[.].*[.](clean|smudge|process)$' 2>/dev/null | awk -F '	' '$1 == "local" || $1 == "worktree" { print $2 }'); o=$IFS; IFS='
'; for k in $ks; do export "GIT_CONFIG_KEY_$n=$k" "GIT_CONFIG_VALUE_$n="; n=$((n+1)); done; IFS=$o; export GIT_CONFIG_COUNT=$n; }; 
cd "$1" || exit 0
name=$2; reg=$3
ino() { stat -c '%d:%i' "$1" 2>/dev/null || stat -f '%d:%i' "$1"; }
if top=$(git rev-parse --show-toplevel) && [ -n "$top" ]; then
  gd=$(cd "$(git rev-parse --git-common-dir)" && pwd) || exit 0
  pd=$(git rev-parse --absolute-git-dir) || exit 0
  shadow=
else
  top=$(pwd); gd=$top/.backplane/history.git; pd=$gd; shadow=1
  [ "$top" = / ] || [ "$top" = "$HOME" ] && exit 0
  [ -d "$gd" ] || [ -L "$gd" ] || [ -n "$(find . -maxdepth 4 \( -name '*.kicad_pcb' -o -name '*.kicad_sch' -o -name '*.kicad_pro' -o -iname '*.pcbdoc' -o -iname '*.schdoc' -o -iname '*.prjpcb' \) -print | head -n 1)" ] || exit 0
  if [ -e "$gd" ] || [ -L "$gd" ]; then
    # trusted: the registry names this folder with its inode (the device number before the colon is not compared: it changes when the disk is mounted again)
    if ! { [ -n "$reg" ] && [ ! -L "$gd" ] && [ -d "$gd" ] && i=$(ino "$gd") && awk -v i="${i#*:}" -v g="$gd" '{ k = $1; sub(/^[^:]*:/, "", k) } k == i && substr($0, length($1) + 2) == g { f = 1 } END { exit !f }' "$reg"; }; then
      mv "$gd" "$gd.untrusted-$(date +%s)" || exit 0
    fi
  fi
  export GIT_DIR="$gd" GIT_WORK_TREE="$top"
  if [ ! -d "$gd" ]; then
    mkdir -p "$top/.backplane" && git init -q >/dev/null || exit 0
    printf '/.backplane/\n' >> "$gd/info/exclude"
    [ -n "$reg" ] && printf '%s %s\n' "$(ino "$gd")" "$gd" >> "$reg"
  fi
fi
bp_safe
ref=refs/backplane/checkpoints/$name/steps
spec='*.kicad_[ps][cr][boh]'
# Altium's boards, sheets and projects (any case; History/ is Altium's own backups)
aspec1=':(icase)*.pcbdoc' aspec2=':(icase)*.schdoc' aspec3=':(icase)*.prjpcb'
[ -n "$shadow" ] || git rev-parse -q --verify "$ref" >/dev/null || [ -n "$(git ls-files -- "$spec" "$aspec1" "$aspec2" "$aspec3" | head -n 1)" ] || [ -n "$(find . -maxdepth 4 \( -name '*.kicad_pcb' -o -name '*.kicad_sch' -o -name '*.kicad_pro' -o -iname '*.pcbdoc' -o -iname '*.schdoc' -o -iname '*.prjpcb' \) -not -path './.backplane/*' -print | head -n 1)" ] || exit 0
idx=$pd/backplane-$name.steps.index
lock=$pd/backplane-$name.steps.lock
n=0
until mkdir "$lock"; do
  o=$(cat "$lock/pid")
  if [ -n "$o" ] && ! kill -0 "$o"; then rm -rf "$lock"; continue; fi
  n=$((n+1)); [ $n -gt 600 ] && exit 0; sleep 0.1
done
echo $$ > "$lock/pid"
trap '[ "$(cat "$lock/pid")" = $$ ] && rm -rf "$lock"' EXIT
rm -f "$idx.lock"
# only the patterns that match a file here or in the index: git add refuses a pathspec that matches nothing (an Altium-only
# folder has no KiCad file, a KiCad one no Altium file); globbing off so the shell leaves the patterns to git
set -f; specs=
for x in "$spec" "$aspec1" "$aspec2" "$aspec3"; do
  [ -n "$(git ls-files -- "$x" | head -n 1)" ] || [ -n "$(git ls-files -o --exclude-standard -- "$x" | head -n 1)" ] && specs="$specs $x"
done
[ -n "$specs" ] || exit 0
[ -n "$shadow" ] || git ls-files -z -- $specs | GIT_INDEX_FILE=$idx xargs -0 git add -f -- >/dev/null
# Altium's own backups (History/) are left out; a KiCad file in a folder of that name is still the project's
hx=":(exclude,icase)History/*.schdoc :(exclude,icase)*/History/*.schdoc :(exclude,icase)History/*.pcbdoc :(exclude,icase)*/History/*.pcbdoc :(exclude,icase)History/*.prjpcb :(exclude,icase)*/History/*.prjpcb"
GIT_INDEX_FILE=$idx git add -A -- $specs ':(exclude)*_autosave-*' ':(exclude)*-backups/*' $hx >/dev/null
tree=$(GIT_INDEX_FILE=$idx git write-tree) || exit 0
prev=$(git rev-parse -q --verify "$ref")
if [ -n "$prev" ]; then
  [ "$(git rev-parse "$prev^{tree}")" = "$tree" ] && exit 0
else
  [ "$tree" = "$(git hash-object -t tree /dev/null)" ] && exit 0
fi
c=$(GIT_AUTHOR_NAME=Backplane GIT_AUTHOR_EMAIL=backplane@localhost GIT_COMMITTER_NAME=Backplane GIT_COMMITTER_EMAIL=backplane@localhost git commit-tree --no-gpg-sign -m "backplane step" ${prev:+-p "$prev"} "$tree") || exit 0
if [ -n "$prev" ]; then git update-ref "$ref" "$c" "$prev" || exit 0
else printf 'create %s %s\n' "$ref" "$c" | git update-ref --stdin || exit 0; fi
printf '%s\n%s\n%s\n%s\n' "$c" "$prev" "$gd" "$top"
git diff-tree -r -z --no-commit-id --name-status --root ${prev:+"$prev"} "$c"