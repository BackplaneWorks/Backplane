// One exclusive directory per turn. No shell, argv-sized write, or symlink.
function omp_launch_prepare(home, prompt, extension) {
  const fs = require("node:fs"), path = require("node:path");
  let dir;
  try {
    // Detection can be stale after an uninstall; fail before spawning env.
    // Bun's default lookup can retain the startup PATH. The hub merges
    // provider locations later; use the same explicit PATH as Proc.spawn.
    const key = Object.keys(process.env).find(k => k.toUpperCase() === "PATH");
    if (!Bun.which("omp", { PATH: key ? process.env[key] : "" })) return io_fail(2);
    const root = path.resolve(home, "omp");
    fs.mkdirSync(root, { recursive: true, mode: 0o700 });
    dir = fs.mkdtempSync(path.join(root, "launch-"));
    fs.writeFileSync(path.join(dir, "system-prompt.txt"), prompt, { flag: "wx", mode: 0o600 });
    fs.writeFileSync(path.join(dir, "backplane.js"), extension, { flag: "wx", mode: 0o600 });
    return io_done(dir);
  } catch (error) {
    if (dir) fs.rmSync(dir, { recursive: true, force: true });
    return io_fail(globalThis.BP_H.errno(error));
  }
}

function omp_launch_remove(dir) {
  // Only the directory returned by prepare, after Grok.run returns.
  try { require("node:fs").rmSync(dir, { recursive: true, force: true }); }
  catch (error) { console.warn("Could not remove Oh My Pi launch files: " + error.code); }
  return { $: "Unit" };
}

io_eff(CID(OmpLaunch.prepare), omp_launch_prepare);
io_eff(CID(OmpLaunch.remove), omp_launch_remove);
