#!/bin/sh
# One source-bound serial Boat campaign. Never uses real hub data or keys.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/telemetry-pcb
export BEND_NO_TELEMETRY=1 BACKPLANE_TELEMETRY=off BACKPLANE_ORDER_SERVER=http://127.0.0.1:9
export BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576 2>/dev/null || true
. scripts/bendok.sh
# Check the modified hub entrypoint, without loading the global law suite.
bend_check src/server/server.bend > build/telemetry-pcb/server-check.log 2>&1
printf '%s\n' 'ok modified server entrypoint typechecks'
TEST_JOBS=1 TEST_FILES='test/telemetry_pcb_laws_test.bend test/telemetry_pcb_test.bend test/telemetry_usage_test.bend test/telemetry_test.bend test/telemetry_choice_test.bend test/telemetry_activity_test.bend test/telemetry_delivery_test.bend test/telemetry_ui_test.bend test/onboard_test.bend test/provider_setup_test.bend test/sidebar_draft_image_test.bend' scripts/test.sh > build/telemetry-pcb/focused.log 2>&1
cat build/telemetry-pcb/focused.log
bend test/native/telemetry_usage_delivery.bend -o build/telemetry-pcb/usage-delivery.js > build/telemetry-pcb/worker-emit.log 2>&1
bun build --compile build/telemetry-pcb/usage-delivery.js --outfile build/telemetry-pcb/usage-delivery > build/telemetry-pcb/worker-build.log 2>&1
python3 test/tools/telemetry_pcb_git_test.py --worker build/telemetry-pcb/usage-delivery > build/telemetry-pcb/git.log 2>&1
cat build/telemetry-pcb/git.log
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s test/tools -p telemetry_aws_probe_test.py > build/telemetry-pcb/harness.log 2>&1
cat build/telemetry-pcb/harness.log
sha256sum build/telemetry-pcb/usage-delivery test/tools/telemetry_aws_probe.py > build/telemetry-pcb/artifacts.sha256
cat build/telemetry-pcb/artifacts.sha256
printf '%s\n' 'PASS PCB focused gate; global PROOF and broad regression gates OMITTED by fast policy' > build/telemetry-pcb/RESULT.txt
