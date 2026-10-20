#!/bin/sh
# Standalone native typography/layout regression. Safe for a disposable Boat
# sandbox: no hub, app window, device, install, display timing or FPS claim.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/text-scaling
export BACKPLANE_GOOGLE_BAKE=0 BACKPLANE_LOW_MEM=1
export BACKPLANE_JOBS=${BACKPLANE_JOBS:-2}
export BACKPLANE_CPUS=${BACKPLANE_CPUS-}
if command -v clang-19 >/dev/null 2>&1; then
  export CC=clang-19
elif command -v clang >/dev/null 2>&1; then
  export CC=clang
else
  echo 'No native clang toolchain in this sandbox; no build attempted.' >&2
  exit 78
fi
scripts/build-app.sh test/native/textscale_frame.bend build/text-scaling/textscale_frame
# Preserve the exact native component independently of sandbox font support.
sha256sum build/text-scaling/textscale_frame > build/text-scaling/native-binary.sha256
tar -czf build/text-scaling/native-component.tar.gz -C build/text-scaling textscale_frame native-binary.sha256
build/text-scaling/textscale_frame --threads 2 --gpu off > build/text-scaling/native-results.log
cat build/text-scaling/native-results.log
for image in build/text-scaling/native-100.ppm build/text-scaling/native-150.ppm build/text-scaling/menu-100.ppm build/text-scaling/menu-150.ppm build/text-scaling/narrow-100.ppm build/text-scaling/narrow-150.ppm build/text-scaling/top-100.ppm build/text-scaling/top-150.ppm; do
  python3 scripts/ppm-to-png.py "$image" "${image%.ppm}.png"
done
tar -czf build/text-scaling/native-review.tar.gz -C build/text-scaling native-results.log native-binary.sha256 native-100.png native-150.png menu-100.png menu-150.png narrow-100.png narrow-150.png top-100.png top-150.png
if grep -q '^FAIL' build/text-scaling/native-results.log; then exit 1; fi
printf 'Native component metrics/interaction checks PASS\n'
