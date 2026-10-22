// Compile the helper into one executable (scripts/build-browser.sh).
// playwright-core reads its package.json and browsers.json at run time via
// require(path.join(packageRoot, ...)), and bun --compile freezes packageRoot
// to the build machine's node_modules: a release binary then fails on start
// with "Cannot find module /home/runner/.../package.json". The plugin makes
// those requires static, so the JSON is bundled into the binary.
const outfile = process.argv[2];
if (!outfile) throw new Error("usage: bun build.ts OUTFILE");

const lookup = /require\(import_path\d*\.default\.join\(packageRoot, "(package|browsers)\.json"\)\)/g;

const result = await Bun.build({
  entrypoints: [`${import.meta.dir}/src/main.ts`],
  compile: { outfile },
  minify: true,
  external: ["chromium-bidi"],
  plugins: [{
    name: "playwright-json",
    setup(build) {
      build.onLoad({ filter: /playwright-core[\\/]lib[\\/][^\\/]+\.js$/ }, async ({ path }) => ({
        contents: (await Bun.file(path).text()).replace(lookup, 'require("../$1.json")'),
        loader: "js",
      }));
    },
  }],
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
