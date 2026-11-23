#!/usr/bin/env bash
# Tests iOS MARKETING_VERSION synchronization with the hub version in scripts/ios-archive.sh and mobile/ios/build.sh.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"

unset MARKETING_VERSION BACKPLANE_VERSION

. "$ROOT/scripts/lib-bptmp.sh"

TMP=$(mktemp -d "$BP_TMP/ios-version-test.XXXXXX")
trap 'rm -rf "$TMP"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_eq() {
  local got=$1 want=$2 msg=$3
  [ "$got" = "$want" ] || fail "$msg (got '$got', want '$want')"
}

# 1. Shell syntax checks (both POSIX sh and bash)
sh -n "$ROOT/scripts/ios-archive.sh" "$ROOT/mobile/ios/build.sh"

# 2. Live repo checks (independent of caller env and future version bumps)
expected_live=$(python3 - "$ROOT" <<'PY'
import re, subprocess, sys
root = sys.argv[1]
def ver_tuple(s):
    return tuple(int(x) for x in s.split("."))

cands = []
head_tags = subprocess.check_output(["git", "-C", root, "tag", "--points-at", "HEAD", "v[0-9]*"], text=True).splitlines()
stable_head = [t[1:] for t in head_tags if re.fullmatch(r"v\d+\.\d+\.\d+", t)]
pre_head = [t[1:].split("-", 1)[0] for t in head_tags if re.fullmatch(r"v\d+\.\d+\.\d+-.*", t)]
if stable_head:
    print(max(stable_head, key=ver_tuple))
    sys.exit(0)
if pre_head:
    print(max(pre_head, key=ver_tuple))
    sys.exit(0)

cargo = open(f"{root}/native/host/Cargo.toml").read()
m = re.search(r'(?ms)^\[package\][^\[]*?^\s*version\s*=\s*"([^"]+)"', cargo)
if m:
    cands.append(m.group(1).split("-", 1)[0])
for t in subprocess.check_output(["git", "-C", root, "tag", "-l", "v[0-9]*.[0-9]*.[0-9]*"], text=True).splitlines():
    if re.fullmatch(r"v\d+\.\d+\.\d+", t):
        cands.append(t[1:])
print(max(cands, key=ver_tuple))
PY
)
got=$(sh "$ROOT/scripts/ios-archive.sh" version)
[[ "$got" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "live version '$got' is not X.Y.Z"
assert_eq "$got" "$expected_live" "repo scripts/ios-archive.sh version"

got=$(MARKETING_VERSION=0.17.1 sh "$ROOT/scripts/ios-archive.sh" version)
assert_eq "$got" "0.17.1" "MARKETING_VERSION override"

got=$(BACKPLANE_VERSION=v0.16.5-nightly.20261122.3 sh "$ROOT/scripts/ios-archive.sh" version)
assert_eq "$got" "0.16.5" "BACKPLANE_VERSION normalization"

if BACKPLANE_VERSION=not-a-version sh "$ROOT/scripts/ios-archive.sh" version >/dev/null 2>&1; then
  fail "expected invalid BACKPLANE_VERSION to fail"
fi

if MARKETING_VERSION="0.16.4; rm -rf /" sh "$ROOT/scripts/ios-archive.sh" version >/dev/null 2>&1; then
  fail "expected unsafe MARKETING_VERSION to fail"
fi

# 3. Synthetic isolated git repo tests
REPO="$TMP/repo"
mkdir -p "$REPO/scripts" "$REPO/native/host"
cp "$ROOT/scripts/ios-archive.sh" "$REPO/scripts/ios-archive.sh"
chmod +x "$REPO/scripts/ios-archive.sh"

git -C "$REPO" init -q
git -C "$REPO" config user.name "Test User"
git -C "$REPO" config user.email "test@example.com"

write_cargo() {
  cat > "$REPO/native/host/Cargo.toml" <<EOF
[package]
name = "backplane-hub"
version = "$1"
edition = "2021"
EOF
}

write_cargo "0.16.4"
git -C "$REPO" add native/host/Cargo.toml scripts/ios-archive.sh
git -C "$REPO" commit -q -m "initial 0.16.4"
git -C "$REPO" tag v0.16.3

# Second commit tagged v0.16.4
echo "line2" >> "$REPO/native/host/Cargo.toml"
git -C "$REPO" add native/host/Cargo.toml
git -C "$REPO" commit -q -m "second"
git -C "$REPO" tag v0.16.4

# Third commit tagged v0.16.5-nightly.20261122.3 only
echo "line2b" >> "$REPO/native/host/Cargo.toml"
git -C "$REPO" add native/host/Cargo.toml
git -C "$REPO" commit -q -m "nightly"
git -C "$REPO" tag v0.16.5-nightly.20261122.3

# Untagged commit on top
echo "line3" >> "$REPO/native/host/Cargo.toml"
git -C "$REPO" add native/host/Cargo.toml
git -C "$REPO" commit -q -m "untagged head"

got=$(sh "$REPO/scripts/ios-archive.sh" version)
assert_eq "$got" "0.16.4" "untagged HEAD ignores nightly tag and picks 0.16.4"

# Newer stable tag v0.16.10 wins numerically over stale Cargo.toml 0.16.4
git -C "$REPO" tag v0.16.10 v0.16.4
got=$(sh "$REPO/scripts/ios-archive.sh" version)
assert_eq "$got" "0.16.10" "numeric semver comparison picks 0.16.10 over 0.16.4"

# Newer Cargo.toml 0.17.0 wins over older tag v0.16.10
write_cargo "0.17.0"
got=$(sh "$REPO/scripts/ios-archive.sh" version)
assert_eq "$got" "0.17.0" "newer Cargo.toml 0.17.0 wins over tag v0.16.10"
git -C "$REPO" commit -q -am "bump to 0.17.0"

# Exact tag checkout v0.16.3 wins even when v0.16.10 exists in repo
git -C "$REPO" checkout -q v0.16.3
got=$(sh "$REPO/scripts/ios-archive.sh" version)
assert_eq "$got" "0.16.3" "exact stable tag v0.16.3 checkout"

# Exact prerelease tag checkout v0.16.5-nightly.20261122.3
git -C "$REPO" checkout -q v0.16.5-nightly.20261122.3
got=$(sh "$REPO/scripts/ios-archive.sh" version)
assert_eq "$got" "0.16.5" "exact prerelease tag checkout strips -nightly suffix"

echo "PASS: ios_version_test.sh"
