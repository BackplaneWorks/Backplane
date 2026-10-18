// Compile the helper into one executable (scripts/build-kipc.sh). The socket
// worker is a second entry point, so the binary carries it.
const outfile = process.argv[2];
if (!outfile) throw new Error("usage: bun build.ts OUTFILE");

const result = await Bun.build({
  entrypoints: [`${import.meta.dir}/src/main.ts`, `${import.meta.dir}/src/worker.ts`],
  compile: { outfile },
  minify: true,
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
