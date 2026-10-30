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
#   BACKPLANE_DEV_SERVICE_ONLY=1 scripts/dev.sh stage DIR
#                                  stage only the hub, the launcher, the
#                                  matching web and LICENSE; no KiStack,
#                                  helpers or CEF
#   BACKPLANE_SKIP_CHECK=1           stage without type-checking (a follow
#                                  deploy of main: CI checked it)
#
# The hub is built for Bun's JS runtime (scripts/build-js.sh: about 45 s to
# emit, nothing when the server's sources are as last time).
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
# A nonempty caller BACKPLANE_VERSION_STAMP overrides that generated stamp.
set -eu
cd "$(dirname "$0")/.."
. scripts/bendok.sh
export PATH="$HOME/.bend/bin:$HOME/.bun/bin:$PATH"
prefix=${BACKPLANE_PREFIX:-$HOME/.local/share/backplane}
dev=build/dev
port=${BACKPLANE_DEV_PORT:-3788}
home=${BACKPLANE_DEV_HOME:-$HOME/.backplane-dev}
# Opt-in belongs only to stage; run/web/install retain their existing modes.
service_only=0

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
  sh scripts/build-viewer.sh "$dev/web.new/viewer/vendor"
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

# the hub and the launcher (no window binary: scripts/build.sh)
build_app() {
  check src/server/main.bend
  scripts/build-js.sh src/server/main.bend "$dev/backplane-serve.new"
  mv -f "$dev/backplane-serve.new" "$dev/backplane-serve"
  cp deploy/backplane-launch.sh "$dev/backplane.new" && chmod 755 "$dev/backplane.new"
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
  if [ -n "${BACKPLANE_VERSION_STAMP:-}" ]; then
    printf '%s\n' "$BACKPLANE_VERSION_STAMP"
    return 0
  fi
  # the highest full release, whether or not this branch contains it
  git fetch -q --tags 2>/dev/null || true
  last=$(git tag --list 'v[0-9]*' --sort=-v:refname | grep -v -- - | head -n 1)
  last=${last:-v0.0.0}
  core=${last#v}; core=${core%%-*}
  echo "${core%.*}.$(( ${core##*.} + 1 ))-dev.$(date +%Y%m%d.%H%M)"
}

# the web client into dist/web, kept as a tarball under scripts/web-key.sh
# (BACKPLANE_WEB_CACHE, default ~/.cache/backplane-bend/web; 0 = none;
# three kept): a deploy that leaves the web alone unpacks the last one
build_web_dist() {
  wcache=${BACKPLANE_WEB_CACHE-${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/web}
  [ "$wcache" = 0 ] && wcache=""
  wkey=""
  [ -n "$wcache" ] && wkey=$(scripts/web-key.sh 2>/dev/null || true)
  if [ -n "$wkey" ] && [ -f "$wcache/$wkey.tar" ] && mkdir -p dist/web && tar -xf "$wcache/$wkey.tar" -C dist/web; then
    touch "$wcache/$wkey.tar"
    say "web client reused ($wkey)"
    mkdir -p build
    printf '{"state":"reused","peak_rss_kib":null}\n' > build/web-metrics.json
    return 0
  fi
  rm -rf dist/web
  python3 scripts/local-phase.py --record build/web-metrics.json -- bend src/web/index.html -o dist/web >/dev/null
  python3 scripts/js-unmarshal.py ../core/client.Ui dist/web/chunk-*.js
  python3 scripts/js-native.py dist/web/chunk-*.js
  cp src/web/sw.js dist/web/sw.js
  sh scripts/build-viewer.sh dist/web/viewer/vendor
  for f in dist/web/*.js dist/web/*.css dist/web/*.html; do gzip -9 -k -n -f "$f"; done
  # kept only if nothing it read changed while it was built
  if [ -n "$wkey" ] && [ "$(scripts/web-key.sh 2>/dev/null || true)" = "$wkey" ]; then
    mkdir -p "$wcache"
    tar -cf "$wcache/$wkey.tar.$$" -C dist/web . && mv -f "$wcache/$wkey.tar.$$" "$wcache/$wkey.tar"
    ls -t "$wcache"/*.tar 2>/dev/null | tail -n +4 | while read -r old; do rm -f "$old"; done
  fi
}

# the memory this build may use in GB: BACKPLANE_MEM_GB, else the cap of
# the cgroup it runs in (bp-admit's MemoryMax), else 12
mem_budget() {
  cgmax=""
  cgp=$(cut -d: -f3 /proc/self/cgroup 2>/dev/null | head -n1)
  [ -n "$cgp" ] && [ -r "/sys/fs/cgroup$cgp/memory.max" ] && cgmax=$(cat "/sys/fs/cgroup$cgp/memory.max")
  case $cgmax in ""|max|*[!0-9]*) cgmax="" ;; *) cgmax=$(( cgmax / 1073741824 )) ;; esac
  echo "${BACKPLANE_MEM_GB:-${cgmax:-12}}"
}

# the app, the server, the web client and the voice helper into dist/,
# stamped with the version (in a copy of the emitted JS,
# BACKPLANE_VERSION_STAMP: the source is never rewritten). The browser helper is
# built only when tools/browser changed since the installed one
# (build_browser); the step2glb helper is not rebuilt (scripts/build.sh
# builds both for releases)
build_dist() {
  ver=$(stamp)
  # scripts/check.sh's full pass on exactly these inputs covers these three
  # (scripts/verified.sh); else they are checked here
  if [ "${BACKPLANE_SKIP_CHECK:-}" = 1 ]; then
    # a follow deploy of main: CI checked it before the merge
    say "not checked here (BACKPLANE_SKIP_CHECK=1)"
  elif scripts/verified.sh has check; then
    say "checked already: check.sh passed on these exact inputs"
  else
    for f in src/web/app.bend src/server/main.bend; do check "$f"; done
  fi
  rm -rf dist && mkdir -p dist
  # no window binary (scripts/build.sh): the hub on Bun's JS runtime
  # (scripts/build-js.sh: about 50 s and 8 GB to emit) and the launcher.
  # The web client (about 19 s, 7 GB) is built beside it when the budget
  # holds both (15 GB: measured 14.8 together), else after it
  if [ "$(mem_budget)" -lt 15 ]; then
    say "the hub, then the web (a $(mem_budget) GB budget)"
    BACKPLANE_VERSION_STAMP=$ver BACKPLANE_COMMIT_STAMP=${BACKPLANE_COMMIT_STAMP:-$(git rev-parse HEAD)} scripts/build-js.sh src/server/main.bend dist/backplane-serve
    build_web_dist
  else
    mkdir -p build
    build_web_dist > build/web-dist.log 2>&1 &
    webpid=$!
    hubrc=0
    BACKPLANE_VERSION_STAMP=$ver BACKPLANE_COMMIT_STAMP=${BACKPLANE_COMMIT_STAMP:-$(git rev-parse HEAD)} scripts/build-js.sh src/server/main.bend dist/backplane-serve || hubrc=$?
    webrc=0
    wait "$webpid" || webrc=$?
    [ "$webrc" = 0 ] || { cat build/web-dist.log >&2; say "the web client did not build"; exit 1; }
    [ "$hubrc" = 0 ] || { say "the hub did not build"; exit 1; }
  fi
  cp deploy/backplane-launch.sh dist/backplane && chmod 755 dist/backplane
  if [ "$service_only" = 1 ]; then
    say "built $ver"
    return 0
  fi
  scripts/build-kistack.sh dist/kistack
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
if [ -f "$src/cef-runtime.py" ]; then python3 "$src/cef-runtime.py" install "$src" "$prefix"; fi
rm -rf "$prefix/web.old"
[ -d "$prefix/web" ] && mv "$prefix/web" "$prefix/web.old"
mv "$src/web" "$prefix/web"
if [ -d "$src/kistack" ]; then rm -rf "$prefix/kistack.old"; [ -d "$prefix/kistack" ] && mv "$prefix/kistack" "$prefix/kistack.old"; mv "$src/kistack" "$prefix/kistack"; fi
for f in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc LICENSE; do
  [ -e "$src/$f" ] && mv -f "$src/$f" "$prefix/$f"
done
ln -sf "$prefix/backplane" "$HOME/.local/bin/backplane"
ln -sf "$prefix/backplane-serve" "$HOME/.local/bin/backplane-serve"
EOF
}

# Only machines with an optional runtime stage CEF. No runtime download or
# host build is triggered for the default Chrome-only installation.
stage_cef() {
  runtime=$(python3 scripts/cef-runtime.py discover "$prefix")
  [ -n "$runtime" ] || return 0
  say "staging matched optional CEF host and browser helper"
  scripts/build-cef.sh build/cef >/dev/null
  # Reuse the pinned libraries already installed; a different distribution
  # must never be paired with a host built against this one.
  [ "$(sha256sum "$runtime/libcef.so" | cut -d' ' -f1)" = "$(sha256sum build/cef/Release/libcef.so | cut -d' ' -f1)" ] || { say "installed CEF distribution differs from the pinned host"; return 1; }
  helper=build/dev-pkg/backplane-browser
  helper_stamp=build/dev-pkg/backplane-browser.stamp
  if [ ! -e "$helper" ]; then helper=$prefix/backplane-browser; helper_stamp=$prefix/backplane-browser.stamp; fi
  revision=$(cat tools/cef/host.cc tools/cef/CMakeLists.txt scripts/build-cef.sh | sha256sum | cut -d' ' -f1)
  python3 scripts/cef-runtime.py stage build/dev-pkg "$runtime" build/cef/Release/backplane-cef "$helper" "$helper_stamp" "$revision"
}

stage() {
  rm -rf build/dev-pkg && mkdir -p build/dev-pkg
  cp -R dist/web build/dev-pkg/web
  # the hub ships Bun's and Bend's notices (scripts/js-licenses.sh)
  scripts/js-licenses.sh build/dev-pkg/licenses
  if [ "$service_only" = 1 ]; then
    cp dist/backplane dist/backplane-serve build/dev-pkg/
    [ -e LICENSE ] && cp LICENSE build/dev-pkg/
    return 0
  fi
  [ -d dist/kistack ] && cp -R dist/kistack build/dev-pkg/kistack
  for f in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc; do
    [ -e "dist/$f" ] && cp "dist/$f" build/dev-pkg/
  done
  [ -e LICENSE ] && cp LICENSE build/dev-pkg/
  stage_cef
  return 0
}

# the request that queues a deploy of the package in $1 on a hub ("deploy"
# is no thread, so the person asked). It presents the hub's pairing token,
# read where it runs (its home: BACKPLANE_INSTALL_HOME, ~/.backplane-bend)
deploy_post() {
  body=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"deploy_restart","arguments":{"stage":"%s","note":"dev.sh install"}}}' "$1")
  printf "curl -s --max-time 10 -X POST -H 'content-type: application/json' -H 'accept: application/json' -H \"authorization: Bearer \$(cat \"%s/token\" 2>/dev/null)\" --data '%s' http://127.0.0.1:%s/mcp/deploy" "${BACKPLANE_INSTALL_HOME:-\$HOME/.backplane-bend}" "$body" "${BACKPLANE_INSTALL_PORT:-3787}"
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
    case ${BACKPLANE_DEV_SERVICE_ONLY:-0} in
      0|1) service_only=${BACKPLANE_DEV_SERVICE_ONLY:-0} ;;
      *) say "BACKPLANE_DEV_SERVICE_ONLY must be 0 or 1"; exit 1 ;;
    esac
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
