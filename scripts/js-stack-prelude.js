// The hub's stack, set before anything runs (scripts/build-js.sh puts this
// first in the compiled backplane-serve, ahead of the baked prelude).
//
// Bend's JS walks over large data recurse deeply, and Bun bounds its JS stack
// by two things, both needed: the main thread's RLIMIT_STACK (the kernel
// places the stack from it at exec, so it cannot be raised afterwards) and
// BUN_JSC_maxPerThreadStackUsage (read once at start). With 8 MiB a compiled
// bun reaches ~40k frames; with a 4 GiB soft limit and 4e9 ~31M (an
// unlimited soft limit makes JSC bound itself small again). So when either
// is short, the hub raises the soft limit to 4 GiB, sets the variable and
// executes itself again, once (BACKPLANE_STACK_SET marks the second run).
// It is the first code in the binary: nothing (the hub lock, a socket, an
// adopted agent) is open yet. With the limit and the variable already set
// (the service's LimitSTACK, an exec in place) it does nothing; with only the
// limit set it still executes once, as JSC reads the variable only at
// start. The hub's own children (--plot, --review, --telemetry-pow) raise
// theirs the same way: they parse large files too.
// Stack pages are only committed when touched. Children are started with
// 8 MiB again (host.js's host_child_stack). Sys.exec in place keeps both, so
// a restart does not repeat this.
//
// Only a compiled hub on Linux or macOS (process.argv[1] under /$bunfs/):
// `bun serve.js` and the tests run as they are. macOS's hard stack limit is
// usually 64 MiB, below 4 GiB: there it takes what the hard limit allows. Anything
// failing (no ffi, setrlimit or execve refused) runs on, never exits.
(() => {
  try {
    if (process.platform !== "linux" && process.platform !== "darwin") {
      return;
    }
    if (process.env.BACKPLANE_STACK_SET || !String(process.argv[1] ?? "").startsWith("/$bunfs/")) {
      return;
    }
    process.env.BACKPLANE_STACK_SET = "1";
    const ffi = require("bun:ffi");
    const lib = ffi.dlopen(process.platform === "darwin" ? "libSystem.dylib" : "libc.so.6", {
      getrlimit: { args: ["i32", "ptr"], returns: "i32" },
      setrlimit: { args: ["i32", "ptr"], returns: "i32" },
      execve: { args: ["ptr", "ptr", "ptr"], returns: "i32" },
    }).symbols;
    const want = 4n << 30n;
    const inf = 0xffffffffffffffffn;
    const lim = new BigUint64Array(2);
    // RLIMIT_STACK is 3 on Linux and macOS
    if (lib.getrlimit(3, ffi.ptr(lim)) !== 0) {
      return;
    }
    const soft = lim[0];
    const hard = lim[1];
    // 4 GiB, or as much as the hard limit allows (said once, by the hub
    // itself, not by its --plot, --review... children)
    const goal = hard !== inf && hard < want ? hard : want;
    if (goal < want && !/^--(plot|review|render|telemetry-pow)/.test(String(process.argv[2] ?? ""))) {
      require("fs").writeSync(2, "backplane: the stack is held to " + Number(goal >> 10n)
        + " KiB by its hard limit (below 4 GiB): deep work on large data may overflow it\n");
    }
    const jsc = process.env.BUN_JSC_maxPerThreadStackUsage;
    const short = soft === inf || soft < goal;
    if (!short && jsc) {
      return;
    }
    if (short && lib.setrlimit(3, ffi.ptr(new BigUint64Array([goal, hard]))) !== 0) {
      return;
    }
    if (!jsc) {
      process.env.BUN_JSC_maxPerThreadStackUsage = "4000000000";
    }
    // a compiled bun's argv is ["bun", "/$bunfs/root/<name>", ...args]: the
    // binary itself, then the arguments (the bunfs path is not one: the
    // hub's strict command line would refuse it)
    const cs = (s) => Buffer.from(s + "\0");
    const strs = [process.execPath, ...process.argv.slice(2)].map(cs);
    const envs = Object.entries(process.env)
      .filter(([k, v]) => v !== undefined && k !== "" && !k.includes("=") && !String(v).includes("\0"))
      .map(([k, v]) => cs(k + "=" + v));
    const vec = (bs) => {
      const v = new BigUint64Array(bs.length + 1);
      bs.forEach((b, i) => { v[i] = BigInt(ffi.ptr(b)); });
      return v;
    };
    const argv = vec(strs);
    const envp = vec(envs);
    lib.execve(ffi.ptr(strs[0]), ffi.ptr(argv), ffi.ptr(envp));
    // only on failure: run on as we are
  } catch (_) {
  }
})();
