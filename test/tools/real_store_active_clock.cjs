// Linux-only harness clock. Importing this module performs no IO or native call.
// Controller and children read the same kernel CLOCK_MONOTONIC time namespace;
// no process-relative hrtime, epoch clock, guessed offset or fallback is used.
const fail = (ok, code) => { if (!ok) throw Error(code); };
const MAX_NS = (1n << 40n) * 1000000n;
function timespecNs(seconds, nanos) {
  fail(typeof seconds === 'bigint' && typeof nanos === 'bigint' && seconds >= 0n && nanos >= 0n && nanos < 1000000000n, 'clock-timespec');
  const value = seconds * 1000000000n + nanos;
  fail(value < MAX_NS, 'clock-range'); return value;
}
function checkedReader(readNs, contract) {
  let previous = null, failed = false;
  return Object.freeze({ contract: Object.freeze({ ...contract }),
    nowMs() {
      fail(!failed, 'clock-failed');
      try {
        const value = readNs();
        fail(typeof value === 'bigint' && value >= 0n && value < MAX_NS, 'clock-sample');
        fail(previous === null || value >= previous, 'clock-regressed'); previous = value;
        // <2^40 ms bounds floating conversion error below 0.001 ms. Equal
        // samples are legal; missing/regressed/native errors fail permanently.
        return Number(value) / 1000000;
      } catch (e) { failed = true; throw e; }
    } });
}
function assertDomain(clock, id) {
  fail(typeof id === 'string' && /^[a-f0-9]{64}$/.test(id) && id === clock.contract.domain, 'clock-domain');
}
function failureCode(e) {
  return ['clock-timespec', 'clock-range', 'clock-failed', 'clock-sample', 'clock-regressed', 'clock-domain',
    'clock-abi', 'clock-namespace', 'clock-native-open', 'clock-native-read', 'clock-resolution'].includes(e?.message) ? e.message : null;
}
let singleton;
function hostClock() {
  if (singleton) return singleton;
  fail(process.platform === 'linux' && ['x64', 'arm64'].includes(process.arch), 'clock-abi');
  const fs = require('node:fs'), { createHash } = require('node:crypto');
  const boot = fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
  const namespace = fs.readlinkSync('/proc/self/ns/time');
  fail(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(boot) && /^time:\[\d+\]$/.test(namespace) &&
    fs.readlinkSync('/proc/self/ns/time_for_children') === namespace, 'clock-namespace');
  let ffi, library;
  try {
    ffi = require('bun:ffi');
    library = ffi.dlopen('libc.so.6', {
      clock_gettime: { args: [ffi.FFIType.i32, ffi.FFIType.ptr], returns: ffi.FFIType.i32 },
      clock_getres: { args: [ffi.FFIType.i32, ffi.FFIType.ptr], returns: ffi.FFIType.i32 }
    });
  } catch { throw Error('clock-native-open'); }
  // Linux x86_64/aarch64 glibc timespec = two signed 64-bit longs.
  const buffer = new BigInt64Array(2), pointer = ffi.ptr(buffer), CLOCK_MONOTONIC = 1;
  const sample = fn => {
    buffer[0] = -1n; buffer[1] = -1n;
    fail(fn(CLOCK_MONOTONIC, pointer) === 0, 'clock-native-read');
    return timespecNs(buffer[0], buffer[1]);
  };
  try {
    const resolution = sample(library.symbols.clock_getres);
    fail(resolution > 0n && resolution <= 1000n, 'clock-resolution');
    const domain = createHash('sha256').update('CLOCK_MONOTONIC\n' + boot + '\n' + namespace).digest('hex');
    const clock = checkedReader(() => sample(library.symbols.clock_gettime), {
      kind: 'linux-CLOCK_MONOTONIC', domain, abi: process.arch, resolutionNs: Number(resolution),
      conversionErrorBoundMs: 0.001, processOriginOffsetApplied: false, fallback: false
    });
    clock.nowMs(); // Validate the native ABI/sample before exposing the clock.
    // Retain the library handle for the complete process lifetime.
    singleton = clock; return singleton;
  } catch (e) { library.close(); throw e; }
}
module.exports = { hostClock, assertDomain, timespecNs, checkedReader, failureCode };
