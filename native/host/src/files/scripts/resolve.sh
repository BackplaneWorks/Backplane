exec 2>/dev/null
cd "$1" || exit 1
top=$(git rev-parse --show-toplevel) || { echo 'not a git repository'; exit 1; }
gd=$(cd "$(git rev-parse --git-common-dir)" && pwd) || exit 1
c=$(git rev-parse --verify --quiet "$2^{commit}") || { echo "no commit, branch or tag named $2"; exit 1; }
printf '%s\n%s\n%s\n' "$c" "$gd" "$top"