#!/bin/sh
# on h@backplane after bp-policy-build.sh: the files.dir read, against the policy that build left in ~/bp-gpui-build/slim-policy
export PATH="$HOME/.bun/bin:$PATH"
D=$HOME/bp-gpui-build/slim-policy/src-tree
cp test/gpui/slim/debug_fm.mjs $D/test/gpui/slim/
cd $D && exec bun test/gpui/slim/debug_fm.mjs $PWD/out/split/bp-policy.stripped $PWD/out/pl/log.txt 3881
