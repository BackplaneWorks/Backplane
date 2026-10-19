import { mkdtemp, mkdir, writeFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// The fixture calls the production command, subprocess effect and JSON encoder.
const runner = resolve(process.argv[2] ?? "build/file-paths");
const root = await mkdtemp(join(tmpdir(), "backplane-file-paths-"));
const home = join(root, "home");
const env = { ...process.env, HOME: home, BACKPLANE_HOME: home, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: join(home, "gitconfig") };
let checks = 0;
function check(ok: boolean, name: string) {
  if (!ok) throw new Error(name);
  checks++;
  console.log(`ok ${name}`);
}
async function command(args: string[], cwd: string) {
  const child = Bun.spawn(args, { cwd, env, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(`${args[0]} exited ${code}: ${err}`);
  return out;
}
async function listed(mode: string, cwd: string): Promise<string[]> {
  const value = JSON.parse(await command([runner, mode, cwd], root));
  check(Array.isArray(value) && value.every(p => typeof p === "string"), `${mode}: protocol remains an array of strings`);
  return value;
}
const names = ["café.step", "雪.kicad_pcb", 'quote".step', "back\\slash.step", "with spaces.step", "tab\there.step", "line\nbreak.step", "-dash.step", " ", "nested/one\ntwo.step"];
async function populate(cwd: string) {
  await mkdir(join(cwd, "nested"), { recursive: true });
  await Promise.all(names.map(name => writeFile(join(cwd, name), "example\n")));
}
try {
  await mkdir(home);
  for (const mode of ["git", "find"]) {
    const cwd = join(root, `${mode} project`);
    await populate(cwd);
    if (mode === "git") {
      await command(["git", "init", "-q"], cwd);
      await command(["git", "add", "--", names[0], names[2], names[6]], cwd);
    }
    const paths = await listed(mode, cwd);
    check(paths.length === names.length, `${mode}: one record per file, including an embedded newline`);
    for (let i = 0; i < names.length; i++) {
      check(paths.includes(names[i]), `${mode}: filename ${i + 1} survives byte for byte`);
    }
    check((await Promise.all(paths.map(p => stat(join(cwd, p))))).every(s => s.isFile()), `${mode}: every returned path names an actual file`);
    const bounded = join(root, `${mode} bounded`);
    await mkdir(bounded);
    const many = Array.from({ length: 3002 }, (_, i) => `item-${String(i).padStart(4, "0")}\npart.step`);
    await Promise.all(many.map(name => writeFile(join(bounded, name), "x")));
    if (mode === "git") await command(["git", "init", "-q"], bounded);
    const limited = await listed(mode, bounded);
    check(limited.length === 3000, `${mode}: the existing limit counts complete paths`);
    check(new Set(limited).size === 3000 && limited.every(p => many.includes(p)), `${mode}: no record is split or invented at the limit`);
    const empty = join(root, `${mode} empty`);
    await mkdir(empty);
    if (mode === "git") await command(["git", "init", "-q"], empty);
    check((await listed(mode, empty)).length === 0, `${mode}: an empty tree has no paths`);
  }
  console.log(`${checks} assertions, 0 failures`);
} finally {
  await rm(root, { recursive: true, force: true });
}
