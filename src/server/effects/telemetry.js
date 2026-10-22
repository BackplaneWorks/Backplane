// The JS twin of telemetry.c. A release's credentials are not in the
// emitted JS: scripts/build-js.sh puts them in a prelude it compiles in
// (scripts/telemetry-header.py writing .js), which sets
// globalThis.BACKPLANE_BAKED; without one every value is "" and telemetry
// stays off, as in an unbaked C build.

function tf_baked(name) {
  const baked = globalThis.BACKPLANE_BAKED || {};
  const v = name.startsWith("BACKPLANE_TELEMETRY_") ? baked[name] : undefined;
  return typeof v === "string" ? v : "";
}

// as tf_platform_run: "<os>-<arch>", os linux/darwin/windows, arch x64/arm64
function tf_platform() {
  const os = process.platform === "darwin" ? "darwin"
    : process.platform === "win32" ? "windows" : "linux";
  const arch = process.arch === "arm64" ? "arm64" : "x64";
  return os + "-" + arch;
}

// as tf_pow_run: the first hex counter whose "bp-pow1:<id>:<pow>" has a
// SHA-256 starting with 18 zero bits (a 36-char id only; "" if none in 2^26)
function tf_pow(id) {
  if (id.length !== 36) {
    return "";
  }
  const { createHash } = require("node:crypto");
  for (let i = 0; i < (1 << 26); i++) {
    const pow = i.toString(16);
    const msg = "bp-pow1:" + id + ":" + pow;
    if (msg.length > 119) {
      continue;
    }
    const h = createHash("sha256").update(msg).digest();
    if (h[0] === 0 && h[1] === 0 && (h[2] & 0xc0) === 0) {
      return pow;
    }
  }
  return "";
}

io_eff(CID(Tf.baked), tf_baked);
io_eff(CID(Tf.platform), tf_platform);
io_eff(CID(Tf.pow), tf_pow);
