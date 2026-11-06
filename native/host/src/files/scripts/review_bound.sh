set -eu
root=$(cd "$1" && pwd -P)
shift
for file do
  [ -n "$file" ] || continue
  [ -f "$file" ] && [ ! -L "$file" ] || { echo "release input missing or linked: $file"; exit 1; }
  parent=$(cd "$(dirname "$file")" && pwd -P)
  case "$parent/$(basename "$file")" in "$root"/*) ;; *) echo "release input escapes committed snapshot: $file"; exit 1;; esac
done