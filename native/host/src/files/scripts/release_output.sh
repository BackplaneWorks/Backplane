set -eu
list=$(mktemp)
trap 'rm -f "$list"' 0
if command -v sha256sum >/dev/null 2>&1; then sum=sha256sum; else sum="shasum -a 256"; fi
if [ -d "$1" ]; then
  find "$1" -type f -size +0c -exec $sum {} + > "$list"
elif [ -s "$1" ]; then
  $sum "$1" > "$list"
fi
LC_ALL=C sort "$list"