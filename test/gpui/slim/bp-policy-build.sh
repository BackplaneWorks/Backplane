#!/bin/sh
# PROVISIONAL policy build + the policy's checks on h@backplane (never a gate: the gating run is on Boat). Own folder ~/bp-gpui-build/slim-policy,
# pinned Bend 2.0.36 (the box's ~/.bend is 2.0.27 and stays), gcc for the C units (no clang there). Take lock bp-build via work_run.
# Stops at the first crash. Result: build/gpui/slim/bp-policy-bp (stripped) and build/gpui/results/slim/bp-policy-out/.
set -eu
REPO=$(git rev-parse --show-toplevel); D=bp-gpui-build/slim-policy; SSH="ssh -o BatchMode=yes h@backplane"
COMMIT=$(git -C "$REPO" rev-parse --short HEAD)
SUM=02089dc0eed0fd5fd6d73c74cc9cffcb2c6638dd3b02ae83f7b8cc57fbe381ba
$SSH "mkdir -p ~/$D/bend-dl && cd ~/$D && { [ -x bend-home/.bend/bin/bend ] || { curl --proto '=https' --tlsv1.2 -fsSL -o bend-dl/b.tgz https://github.com/bendlang/bend/releases/download/v2.0.36/bend-2.0.36-linux-x64.tar.gz && echo '$SUM  bend-dl/b.tgz' | sha256sum -c - && tar -xzf bend-dl/b.tgz -C bend-dl && mkdir -p bend-home/.bend/bin && mv bend-dl/bend/bend2 bend-dl/bend/guide bend-home/.bend/ && mv bend-dl/bend/bin/bend bend-home/.bend/bin/bend; }; } && ~/$D/bend-home/.bend/bin/bend version"
rsync -a --delete --exclude .git --exclude /build --exclude target --exclude 'native/*/target' --exclude /mobile --exclude /desktop --exclude /docs --exclude /deploy --exclude /order \
  --exclude /stage --exclude 'stage-*' --exclude staged-bins --exclude .lowmem --exclude bend-home --exclude bend-dl --exclude /out "$REPO/" "h@backplane:$D/src-tree/"
echo "provisional: commit $COMMIT (tree may be dirty), host backplane, bend 2.0.36 pinned, gcc"
$SSH "cd ~/$D/src-tree && export PATH=\$HOME/$D/bend-home/.bend/bin:\$HOME/.bun/bin:\$PATH BEND_NO_TELEMETRY=1 CC=gcc CSTD=gnu11 && rm -rf out && systemd-run --user --scope -q -p MemoryMax=14G -p MemorySwapMax=0 -- sh -c 'sh test/gpui/slim/build-split.sh && sh test/gpui/slim/policy-checks.sh'" 2>&1 | tee /tmp/slim-policy-bp.log | grep -vE "^cc out|^$" | tail -40
mkdir -p build/gpui/slim build/gpui/results/slim/bp-policy-out
rsync -a "h@backplane:$D/src-tree/out/split/bp-policy.stripped" build/gpui/slim/bp-policy-bp
rsync -a "h@backplane:$D/src-tree/out/pl/" build/gpui/results/slim/bp-policy-out/
echo "POLICY build/gpui/slim/bp-policy-bp $(sha256sum build/gpui/slim/bp-policy-bp | cut -c1-16) commit $COMMIT PROVISIONAL"
