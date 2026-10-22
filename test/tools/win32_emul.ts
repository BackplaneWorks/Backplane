// Finds the libc a JS build would still reach on Windows, on Linux, in
// seconds, without a Windows machine: a preload that says process.platform
// is "win32", stubs kernel32 (a handle 100, every call succeeds) and fails
// every dlopen of libc as Windows does ('Failed to open library
// "libc.so.6": error code 126'), printing the caller's stack to stderr
// first ("LIBC DLOPEN <symbols>" and the frames).
//
//   bun --preload test/tools/win32_emul.ts build/x.js [ARGS]
//
// Effects that branch on the platform take their win32 path; a crash or a
// LIBC line names what still needs one. It is not Windows: paths, PATHEXT,
// ConPTY and Bun.spawn's own Windows code are not exercised.
Object.defineProperty(process, "platform", { value: "win32" });
const ffi = require("bun:ffi");
const real = ffi.dlopen;
ffi.dlopen = (name: any, syms: any) => {
  if (/libc\.so|libSystem/.test(String(name))) {
    const st = (new Error().stack ?? "").split("\n").slice(2, 9).map((l) => l.trim()).join("\n   ");
    console.error("LIBC DLOPEN " + Object.keys(syms).slice(0, 3).join(",") + "\n   " + st);
    throw new Error('Failed to open library "libc.so.6": error code 126');
  }
  if (/kernel32/i.test(String(name))) {
    return { symbols: Object.fromEntries(Object.keys(syms).map((k) => [k, (..._: any[]) => k === "CreateFileW" ? 100 : k === "GetLastError" ? 0 : 1])), close() {} };
  }
  return real(name, syms);
};
