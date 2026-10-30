// Compile the helper into one executable (scripts/build-kipc.sh). The socket
// worker is a second entry point, so the binary carries it.
const outfile = process.argv[2];
if (!outfile) throw new Error("usage: bun build.ts OUTFILE");

const result = await Bun.build({
  entrypoints: [`${import.meta.dir}/src/main.ts`, `${import.meta.dir}/src/worker.ts`],
  // BUN_TARGET cross-compiles; BUN_EXECUTABLE is that target's Bun runtime
  // (a release pins it, scripts/build-helpers.sh), else bun fetches it
  compile: {
    outfile,
    ...(process.env.BUN_TARGET ? { target: process.env.BUN_TARGET } : {}),
    ...(process.env.BUN_EXECUTABLE ? { executablePath: process.env.BUN_EXECUTABLE } : {}),
  },
  minify: true,
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
