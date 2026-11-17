#!/bin/bash
# The linux-x64 release runner container's start (scripts/ci-linux/Dockerfile). /home/runner is the persistent volume: the runner, its _work (each
# crate's target/ stays warm), ~/.rustup, ~/.cargo, ~/.cache (sccache, OpenCascade). /opt/bp/runner-guard.sh is scripts/runner-guard.sh, mounted
# read-only from the host. First start: /home/runner/.reg-token (a registration token, removed once used) registers the runner.
set -euo pipefail
R=$HOME/actions-runner; V=2.338.2; SHA=baebf204307bac6193480852732f0aba2ae97b6c5597c244f4d5558390147c40
LABEL=${RUNNER_LABEL:?}; NAME=${RUNNER_NAME:?}
[ -x /opt/bp/runner-guard.sh ] || { echo "no guard at /opt/bp/runner-guard.sh: refusing to start" >&2; exit 1; }
if [ ! -x "$R/run.sh" ]; then
  mkdir -p "$R"; cd "$R"
  curl -sSfLo r.tgz "https://github.com/actions/runner/releases/download/v$V/actions-runner-linux-x64-$V.tar.gz"
  echo "$SHA  r.tgz" | sha256sum -c -; tar -xzf r.tgz; rm r.tgz
fi
[ -x "$HOME/.cargo/bin/rustup" ] || { curl -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain stable; }
mkdir -p "$HOME/.cache/sccache" "$HOME/_work"
cat > "$R/.env" <<ENV
PATH=$HOME/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
SCCACHE_DIR=$HOME/.cache/sccache
SCCACHE_CACHE_SIZE=${SCCACHE_CACHE_SIZE:-20G}
ACTIONS_RUNNER_HOOK_JOB_STARTED=/opt/bp/runner-guard.sh
LANG=C.UTF-8
ENV
cd "$R"
if [ -f "$HOME/.reg-token" ]; then
  t=$(cat "$HOME/.reg-token"); rm -f "$HOME/.reg-token"
  [ ! -f .runner ] || ./config.sh remove --token "$t" || true
  ./config.sh --unattended --url https://github.com/BackplaneWorks/Backplane --token "$t" --name "$NAME" --labels "$LABEL" --no-default-labels \
    --work "$HOME/_work" --replace
fi
[ -f .runner ] || { echo "runner not registered: put a registration token in /home/runner/.reg-token and restart" >&2; exit 1; }
exec ./run.sh
