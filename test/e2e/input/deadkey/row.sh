# run inside the container as u: $1 = before|after
cd /ws && export NO_SCOPE=1 BOAT_SANDBOX=1 IME_BACKEND=ibus BACKPLANE_GPUI_ASSETS=/xim/$1/assets PATH=/xim/bin:$PATH
rm -rf out/e2e/deadkey-$1 out/e2e-run/deadkey; mkdir -p out/e2e
dbus-run-session -- bun test/e2e/input/deadkey-run.mjs --hub-bin /xim/$1/backplane-hub --gpui-bin /xim/$1/backplane-gpui --out out/e2e/deadkey-$1 > out/deadkey-$1.log 2>&1; rc=$?
grep -v '^dbus-daemon\|^fusermount\|^SpiRegistry' out/deadkey-$1.log | tail -15; grep -i 'panicked\|xim' out/e2e/deadkey-$1/client.stderr.log | head -5
python3 -c "import json;j=json.load(open('out/e2e/deadkey-$1/deadkey-summary.json'));print(json.dumps({k:j.get(k) for k in ['verdict','reason','ime','deadAcuteKey','composerRead','hubMessageText','client']}))"
echo "row $1 exit $rc"
