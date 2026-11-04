#!/bin/sh
# inside Boat: the build coordinator's low-memory composite compiler (compiler 41262875,
# recipe from bp-coord/pr944-capped-recipe) asked for C instead of JS.
set -u
L=test/gpui/bend/lowmem; C=build/lc; O=out/hub2; mkdir -p $C $O build/spill
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, swap $(free -m | awk '/Swap/{print $2}') MiB" | tee $O/machine.txt
[ "$(sha256sum $L/compiler-41262875.tar.gz | cut -c1-64)" = be39bfb8c44da887702c51ef80dea07a6b0366c754f2eb15615e73a1f0d62581 ] || { echo archive; exit 90; }
tar -xzf $L/compiler-41262875.tar.gz -C $C
(cd $C && sha256sum -c --quiet ../../$L/compiler-files.sha256) || { echo manifest; exit 90; }
BUN=$PWD/$C/bun
curl -fsSL --max-time 60 -o $C/bun.zip https://github.com/oven-sh/bun/releases/download/bun-v1.3.10/bun-linux-x64-baseline.zip || exit 76
python3 -c "import zipfile,sys; open(sys.argv[2],'wb').write(zipfile.ZipFile(sys.argv[1]).read('bun-linux-x64-baseline/bun'))" $C/bun.zip $BUN; chmod 755 $BUN
[ "$(sha256sum $BUN | cut -c1-64)" = 795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822 ] || { echo exe; exit 90; }
. $L/env.line
ulimit -s 1048576
( while :; do printf "%s anon_kib=%s cache_kib=%s\n" "$(date +%s)" "$(awk '/^AnonPages/{print $2}' /proc/meminfo)" "$(awk '/^Cached/{print $2}' /proc/meminfo)"; sleep 5; done ) > $O/mem.samples &
S=$!
TMPDIR=$PWD/build/spill BP_COMPILER_PROFILE=$PWD/$O/profile.jsonl BP_COMPILER_SPILL_PROFILE=1 \
  /usr/bin/time -f "emit-c(composite) exit=%x wall=%e s peakRSS=%M KiB" -o $O/time.txt timeout 2800 $BUN $C/compiler/bend2/main.ts src/server/main.bend -o $O/hub.c > $O/emit.log 2>&1
echo "rc=$?" >> $O/time.txt
kill $S
cat $O/time.txt; head -40 $O/emit.log; ls -l $O; sort -t= -k2 -n -r $O/mem.samples | head -1
awk -F'anon_kib=' '{split($2,a," "); if(a[1]>m)m=a[1]}END{print "peak anon KiB", m}' $O/mem.samples
