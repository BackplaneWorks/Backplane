#!/bin/bash
# Build the linux-x64 release runner's image and (re)start its container on this machine (a Boat box, or the desktop's rootless Docker:
# DOCKER_HOST set). State under BP_CI (default ~/bp-ci): runner-home/ is the container's /home/runner volume, runner-guard.sh the guard it mounts
# read-only. The registration token comes on stdin when the runner is not registered yet:
#   gh api -X POST repos/BackplaneWorks/Backplane/actions/runners/registration-token -q .token | ssh box 'bash ~/ci-linux/up.sh NAME'
set -euo pipefail
name=${1:?up.sh RUNNER_NAME}; label=${RUNNER_LABEL:-bp-linux-release}
here=$(cd "$(dirname "$0")" && pwd); ci=${BP_CI:-$HOME/bp-ci}
mkdir -p "$ci/runner-home"
install -m 755 "$here/runner-guard.sh" "$ci/runner-guard.sh"
docker build -q -t bp-linux-release-runner --build-arg UID="$(id -u)" "$here"
if [ ! -f "$ci/runner-home/actions-runner/.runner" ]; then
  [ ! -t 0 ] || { echo "not registered: pipe a registration token in" >&2; exit 1; }
  umask 077; cat > "$ci/runner-home/.reg-token"; [ -s "$ci/runner-home/.reg-token" ] || { echo "empty token" >&2; exit 1; }
fi
docker rm -f bp-linux-release-runner >/dev/null 2>&1 || true
docker run -d --name bp-linux-release-runner --restart unless-stopped --cpus "$(nproc)" \
  -e RUNNER_NAME="$name" -e RUNNER_LABEL="$label" -e SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-20G}" \
  -v "$ci/runner-home:/home/runner" -v "$ci/runner-guard.sh:/opt/bp/runner-guard.sh:ro" \
  bp-linux-release-runner >/dev/null
echo "started bp-linux-release-runner ($name, label $label)"
