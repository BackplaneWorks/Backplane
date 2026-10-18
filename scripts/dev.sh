#!/bin/sh
# Try this checkout without CI or a release.
#
#   scripts/dev.sh                 build the app + web client into build/dev
#                                  and run it beside your installed Backplane
#   scripts/dev.sh run --no-build  run the last dev build again
#   scripts/dev.sh web             rebuild only the web client (reload the page;
#                                  the hub reads it from disk per request)
#   scripts/dev.sh install         app + server + web client + voice helper,
#                                  installed over ~/.local/share/backplane,
#                                  service restarted
#   scripts/dev.sh install h@host  the same onto another machine over ssh
#                                  (same os/arch; restarts backplane-bend there)
#                                  Neither restarts at once: the running hub is
#                                  asked (POST /mcp/deploy on loopback) to swap
#                                  the staged package in and restart at a safe
#                                  point, when no tool call is running and the
#                                  build locks are free (docs/deploy.md); the
#                                  turns it cuts resume. "deploy queued" is
#                                  printed and the script ends.
#   scripts/dev.sh install --force [h@host]
#                                  the old way: install and restart now (also
#                                  what happens when no hub answers)
#   scripts/dev.sh stage DIR       build the package (as install does) into DIR
#                                  and stop: no install, no hub asked. The
#                                  hub's deploy_main runs this (docs/deploy.md)
#
# The dev run keeps to itself: its own home (BACKPLANE_DEV_HOME, default
# ~/.backplane-dev, never ~/.backplane-bend), port 3788 (BACKPLANE_DEV_PORT),
# no tailnet, no self-update. Its window opens next to your normal one.
# The voice helper is built from this checkout (it takes a second); the
# browser helper too when tools/browser differs from the one installed (its
# source's hash, kept beside it as backplane-browser.stamp); the step2glb
# helper is borrowed from the installed copy.
#
# An installed build is stamped <next patch>-dev.<date>.<time>, e.g.
# 0.10.1-dev.20260925.2130: newer than the release it came after, older than
# the next one, so the updater replaces it with the next real release (or a
# later nightly) and never with an older one.
set -eu
cd "$(dirname "$0")/.."
. scripts/bendok.sh
export PATH="$HOME/.bend/bin:$HOME/.bun/bin:$PATH"
# 4 clang jobs at -O3 passed 28 GB here; 2 stay near 12 GB
export BACKPLANE_JOBS=${BACKPLANE_JOBS:-2}
prefix=${BACKPLANE_PREFIX:-$HOME/.local/share/backplane}
dev=build/dev
port=${BACKPLANE_DEV_PORT:-3788}
home=${BACKPLANE_DEV_HOME:-$HOME/.backplane-dev}

say() { printf '\033[1mdev:\033[0m %s\n' "$*"; }

check() {
  bend_check "$1" || exit 1
}

build_web() {
  check src/web/app.bend
  rm -rf "$dev/web.new"
  bend src/web/index.html -o "$dev/web.new" >/dev/null
  python3 scripts/js-unmarshal.py ../core/client.Ui "$dev"/web.new/chunk-*.js
  python3 scripts/js-native.py "$dev"/web.new/chunk-*.js
  cp src/web/sw.js "$dev/web.new/sw.js"
  for f in "$dev"/web.new/*.js "$dev"/web.new/*.css "$dev"/web.new/*.html; do gzip -9 -k -n -f "$f"; done
  rm -rf "$dev/web" && mv "$dev/web.new" "$dev/web"
}

# the voice helper (Bun, one executable) into $1
build_voice() {
  out=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
  (cd tools/voice && bun build.ts "$out.new" >/dev/null)
  mv -f "$out.new" "$out"
}

# the KiCad IPC helper (tools/kipc): its dependencies installed, then built
build_kipc() {
  out=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
  (cd tools/kipc && { bun install --frozen-lockfile >/dev/null 2>&1 || bun install >/dev/null; } && bun build.ts "$out.new" >/dev/null)
  mv -f "$out.new" "$out"
}

# the browser helper's source, as one hash (what backplane-browser.stamp holds)
browser_stamp() {
  cat tools/browser/package.json tools/browser/bun.lock tools/browser/build.ts tools/browser/src/*.ts | sha256sum | cut -c1-16
}

# the browser helper into $1 when tools/browser differs from the installed
# one (or none is stamped); else nothing (the installed one stays). Answers
# whether it built one.
build_browser() {
  want=$(browser_stamp)
  have=$(cat "$prefix/backplane-browser.stamp" 2>/dev/null || true)
  [ "$want" = "$have" ] && [ -e "$prefix/backplane-browser" ] && return 1
  say "browser helper changed ($have -> $want): building it"
  scripts/build-browser.sh >/dev/null
  mv -f dist/backplane-browser "$1"
  echo "$want" > "$1.stamp"
  return 0
}

build_app() {
  check src/app/main.bend
  scripts/build-app.sh src/app/main.bend "$dev/backplane.new"
  mv -f "$dev/backplane.new" "$dev/backplane"
  rm -f "$dev/backplane-voice"
  build_voice "$dev/backplane-voice"
  rm -f "$dev/backplane-kipc"
  build_kipc "$dev/backplane-kipc" || { [ -e "$prefix/backplane-kipc" ] && ln -sf "$prefix/backplane-kipc" "$dev/backplane-kipc"; }
  rm -f "$dev/backplane-browser" "$dev/backplane-browser.stamp"
  build_browser "$dev/backplane-browser" || { [ -e "$prefix/backplane-browser" ] && ln -sf "$prefix/backplane-browser" "$dev/backplane-browser"; }
  [ -e "$prefix/backplane-step2glb" ] && ln -sf "$prefix/backplane-step2glb" "$dev/backplane-step2glb"
  return 0
}

run() {
  [ -x "$dev/backplane" ] || { say "no dev build yet: run scripts/dev.sh"; exit 1; }
  mkdir -p "$home"
  say "http://127.0.0.1:$port  home $home  (ctrl-c stops it)"
  BACKPLANE_NO_UPDATE=1 exec "$dev/backplane" --home "$home" --port "$port" --no-tailscale
}

stamp() {
  # the highest full release, whether or not this branch contains it
  git fetch -q --tags 2>/dev/null || true
  last=$(git tag --list 'v[0-9]*' --sort=-v:refname | grep -v -- - | head -n 1)
  last=${last:-v0.0.0}
  core=${last#v}; core=${core%%-*}
  echo "${core%.*}.$(( ${core##*.} + 1 ))-dev.$(date +%Y%m%d.%H%M)"
}

# the app, the server, the web client and the voice helper into dist/,
# stamped with the version (in the emitted C, BACKPLANE_VERSION_STAMP: the
# source is never rewritten). The browser helper is
# built only when tools/browser changed since the installed one
# (build_browser); the step2glb helper is not rebuilt (scripts/build.sh
# builds both for releases)
build_dist() {
  ver=$(stamp)
  # scripts/check.sh's full pass on exactly these inputs covers these three
  # (scripts/verified.sh); else they are checked here
  if scripts/verified.sh has check; then
    say "checked already: check.sh passed on these exact inputs"
  else
    for f in src/web/app.bend src/server/main.bend src/app/main.bend; do check "$f"; done
  fi
  rm -rf dist && mkdir -p dist
  bend src/web/index.html -o dist/web >/dev/null
  python3 scripts/js-unmarshal.py ../core/client.Ui dist/web/chunk-*.js
  python3 scripts/js-native.py dist/web/chunk-*.js
  cp src/web/sw.js dist/web/sw.js
  for f in dist/web/*.js dist/web/*.css dist/web/*.html; do gzip -9 -k -n -f "$f"; done
  BACKPLANE_VERSION_STAMP=$ver scripts/build-app.sh src/server/main.bend dist/backplane-serve
  BACKPLANE_VERSION_STAMP=$ver scripts/build-app.sh src/app/main.bend dist/backplane
  build_voice dist/backplane-voice
  build_kipc dist/backplane-kipc || true
  build_browser dist/backplane-browser.tmp && mv -f dist/backplane-browser.tmp dist/backplane-browser && mv -f dist/backplane-browser.tmp.stamp dist/backplane-browser.stamp || true
  say "built $ver"
}

# swap dist/ into a prefix the way deploy/install.sh.in does (renames, so a
# running binary is replaced safely); run here or over ssh
install_into() {
  cat <<'EOF'
set -eu
src=$1 prefix=$2
mkdir -p "$prefix" "$HOME/.local/bin"
rm -rf "$prefix/web.old"
[ -d "$prefix/web" ] && mv "$prefix/web" "$prefix/web.old"
mv "$src/web" "$prefix/web"
for f in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc LICENSE; do
  [ -e "$src/$f" ] && mv -f "$src/$f" "$prefix/$f"
done
ln -sf "$prefix/backplane" "$HOME/.local/bin/backplane"
ln -sf "$prefix/backplane-serve" "$HOME/.local/bin/backplane-serve"
EOF
}

stage() {
  rm -rf build/dev-pkg && mkdir -p build/dev-pkg
  cp -R dist/web build/dev-pkg/web
  for f in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc; do
    [ -e "dist/$f" ] && cp "dist/$f" build/dev-pkg/
  done
  [ -e LICENSE ] && cp LICENSE build/dev-pkg/
  return 0
}

# the request that queues a deploy of the package in $1 on a hub (the MCP
# route from loopback is trusted; "deploy" is no thread, so the person asked)
deploy_post() {
  body=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"deploy_restart","arguments":{"stage":"%s","note":"dev.sh install"}}}' "$1")
  printf "curl -s --max-time 10 -X POST -H 'content-type: application/json' -H 'accept: application/json' --data '%s' http://127.0.0.1:%s/mcp/deploy" "$body" "${BACKPLANE_INSTALL_PORT:-3787}"
}

hello_cmd() {
  printf 'curl -s --max-time 5 -o /dev/null -w %%{http_code} http://127.0.0.1:%s/hello' "${BACKPLANE_INSTALL_PORT:-3787}"
}

links() {
  mkdir -p "$HOME/.local/bin"
  ln -sf "$prefix/backplane" "$HOME/.local/bin/backplane"
  ln -sf "$prefix/backplane-serve" "$HOME/.local/bin/backplane-serve"
}

install_local() {
  build_dist
  stage
  if [ "$force" != 1 ] && [ "$(eval "$(hello_cmd)" 2>/dev/null)" = 200 ]; then
    # the staged package moves out of the checkout (a worktree may go
    # before the hub gets to it) into a folder of its own
    sd=$(mktemp -d "${TMPDIR:-/tmp}/bp-deploy.XXXXXX")
    cp -R build/dev-pkg/. "$sd"
    links
    if eval "$(deploy_post "$sd")" | grep -q scheduled; then
      say "deploy queued: restarts at a safe point (no tool call running, build locks free); staged in $sd"
      return 0
    fi
    say "the hub did not take the deploy: installing directly"
    rm -rf "$sd"
  fi
  install_into | sh -s "$PWD/build/dev-pkg" "$prefix"
  say "installed into $prefix"
  if systemctl --user is-active --quiet backplane.service 2>/dev/null; then
    # the restart ends every session inside the app (this one too, if it
    # runs there), so it waits a few seconds for this script to finish
    systemd-run --user --quiet --on-active=5 systemctl --user restart backplane.service
    say "backplane.service restarts in 5 s"
  else
    say "run: backplane"
  fi
}

install_remote() {
  host=$1
  build_dist
  stage
  rdir=$(ssh "$host" mktemp -d)
  tar -C build/dev-pkg -czf - . | ssh "$host" tar -C "$rdir" -xzf -
  if [ "$force" != 1 ] && [ "$(ssh "$host" "$(hello_cmd)" 2>/dev/null)" = 200 ]; then
    ssh "$host" 'mkdir -p "$HOME/.local/bin"; ln -sf "$HOME/.local/share/backplane/backplane" "$HOME/.local/bin/backplane"; ln -sf "$HOME/.local/share/backplane/backplane-serve" "$HOME/.local/bin/backplane-serve"'
    if ssh "$host" "$(deploy_post "$rdir")" | grep -q scheduled; then
      say "deploy queued on $host: restarts at a safe point; staged in $rdir"
      return 0
    fi
    say "the hub on $host did not take the deploy: installing directly"
  fi
  install_into | ssh "$host" sh -s "$rdir" '$HOME/.local/share/backplane'
  ssh "$host" "rm -rf $rdir; systemctl --user restart backplane-bend.service 2>/dev/null || systemctl --user restart backplane.service 2>/dev/null || true"
  say "installed on $host"
}

mkdir -p "$dev"
case ${1:-run} in
  run)
    [ "${2:-}" = --no-build ] || { build_web; build_app; }
    run ;;
  web)
    build_web
    say "web client rebuilt: reload the page" ;;
  install)
    force=0
    shift
    if [ "${1:-}" = --force ]; then force=1; shift; fi
    if [ -n "${1:-}" ]; then install_remote "$1"; else install_local; fi ;;
  stage)
    [ -n "${2:-}" ] || { say "usage: scripts/dev.sh stage DIR"; exit 1; }
    build_dist
    stage
    rm -rf "$2" && mkdir -p "$2"
    cp -R build/dev-pkg/. "$2"
    say "staged $ver in $2" ;;
  *)
    sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
    exit 1 ;;
esac
