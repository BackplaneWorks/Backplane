set -eu
cd "$1"
list=$(mktemp)
trap 'rm -f "$list" "$list.sorted"' 0
links=$(find . -type l -print)
[ -z "$links" ] || { echo 'linked release outputs are not complete regular files'; exit 1; }
find . -type f ! -name notes.md ! -name manifest.json > "$list"
LC_ALL=C sort "$list" > "$list.sorted"
sed 's|^\./||' "$list.sorted"