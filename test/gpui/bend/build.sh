#!/bin/sh
# Runs ON the Mac (hs-mac-mini) in ~/bp-gpui-bend/repo. Usage: build.sh NAME
# Emits C and JS with Bend 2.0.36, builds native -O2 and -O3, records time/RSS/size.
set -eu
N=$1
export BEND_NO_TELEMETRY=1 PATH=$HOME/bp-gpui-bend/tool/bendhome/bin:$PATH
cd "$HOME/bp-gpui-bend/repo/native/bend"
mkdir -p out
R=out/$N.build.txt; : > "$R"
t() { /usr/bin/time -l "$@" 2>&1 | grep -E "real|maximum resident" | tr -s ' ' | tr '\n' ' '; echo; }
echo "emit-c: $(t bend $N.bend -o out/$N.c)" >> "$R"
echo "emit-js: $(t bend $N.bend -o out/$N.js)" >> "$R"
for O in O2 O3; do
  echo "clang-$O: $(t clang -std=c11 -$O out/$N.c -lpthread -lm -o out/$N.$O)" >> "$R"
done
ls -l out/$N.c out/$N.js out/$N.O2 out/$N.O3 >> "$R"
cat "$R"
