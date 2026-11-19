#!/bin/sh
# scripts/release-dispatch.sh reads the RELEASE_* variables with `gh variable list` (gh 2.46 has no `variable get`) and refuses what it cannot start
here=$(cd "$(dirname "$0")/.." && pwd); d=$(mktemp -d "${TMPDIR:-/tmp}/rd.XXXXXX"); fail=0
cat > "$d/gh" <<'G'
#!/bin/sh
[ "$1 $2" = "variable list" ] || { echo "unknown command $2" >&2; exit 1; }
[ -z "$FAKE_GH_FAIL" ] || { echo "HTTP 401" >&2; exit 1; }
printf '%s' "$FAKE_VARS"
G
chmod +x "$d/gh"
run() { PATH="$d:$PATH" HOME="$d" sh -c "$1" 2>&1; }
vars='[{"name":"RELEASE_WIN_RUNNER","value":"bp-winbuild-release"},{"name":"RELEASE_MAC_RUNNER","value":"bp-mac"},{"name":"RELEASE_LINUX_RUNNER","value":"bp-linux-release"},{"name":"OTHER","value":"x"}]'
o=$(FAKE_VARS="$vars" run "bash $here/scripts/release-dispatch.sh _resolve"); [ "$o" = "win=bp-winbuild-release mac=bp-mac lin=bp-linux-release" ] || { echo "FAIL read: $o"; fail=1; }
o=$(FAKE_VARS='[]' run "bash $here/scripts/release-dispatch.sh _resolve"); [ "$o" = "win= mac= lin=" ] || { echo "FAIL unset: $o"; fail=1; }
o=$(FAKE_GH_FAIL=1 run "bash $here/scripts/release-dispatch.sh _resolve"); case "$o" in *"cannot read the repo variables"*) ;; *) echo "FAIL gh error: $o"; fail=1;; esac
o=$(FAKE_VARS='[{"name":"RELEASE_MAC_RUNNER","value":"macos-latest"}]' run "bash $here/scripts/release-dispatch.sh _resolve"); case "$o" in *"not a runner this script knows"*) ;; *) echo "FAIL unknown: $o"; fail=1;; esac
rm -f "$d/gh"; rmdir "$d"
[ "$fail" = 0 ] && echo "release-dispatch: ok"; exit $fail
