import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// Synthetic tags and stand-in CAD tools only. No hub, remote, or publishing.
const binary = resolve(process.argv[2] ?? "build/release-child");
const broken = process.argv.includes("--expect-broken");
const base = mkdtempSync(join(tmpdir(), "bp-release-exports-"));
let checks = 0;
function check(ok: unknown, name: string) {
  if (!ok) throw new Error(`FAIL ${name}`);
  console.log(`ok ${name}`);
  checks++;
}
function run(args: string[], cwd: string, env: Record<string, string>) {
  const r = Bun.spawnSync(args, { cwd, env, stdout: "pipe", stderr: "pipe", timeout: 60_000 });
  if (r.exitCode !== 0) throw new Error(`${args[0]} exited ${r.exitCode}: ${r.stderr.toString()} ${r.stdout.toString()}`);
  return r.stdout.toString().trim();
}
try {
  const user = join(base, "user"), home = join(base, "home"), fake = join(base, "fake"), temp = join(base, "tmp");
  for (const p of [user, home, fake, temp]) mkdirSync(p);
  const publication = join(base, "publication.log");
  const env: Record<string, string> = {
    HOME: user, BACKPLANE_HOME: home, TMPDIR: temp, PATH: process.env.PATH!,
    XDG_CONFIG_HOME: join(user, ".config"), XDG_DATA_HOME: join(user, ".local/share"),
    GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: join(base, "gitconfig"),
    GIT_AUTHOR_NAME: "Fixture", GIT_AUTHOR_EMAIL: "fixture@example.invalid",
    GIT_COMMITTER_NAME: "Fixture", GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    BACKPLANE_KICAD_CLI: join(fake, "kicad-cli"), RELEASE_PUBLICATION_LOG: publication,
  };
  const git = run(["sh", "-c", "command -v git"], base, env);
  writeFileSync(join(fake, "git"), `#!/bin/sh\nif [ "$1" = push ]; then echo git-push >> "$RELEASE_PUBLICATION_LOG"; exit 91; fi\nexec '${git.replaceAll("'", "'\\''")}' "$@"\n`);
  writeFileSync(join(fake, "gh"), '#!/bin/sh\necho gh >> "$RELEASE_PUBLICATION_LOG"\nexit 91\n');
  writeFileSync(join(fake, "kicad-cli"), `#!/bin/sh
set -eu
# Metadata probes are resolver traffic, not release export steps.
case "\${1:-}" in
  version|--version) printf '10.0.6\\n'; exit 0 ;;
  api-server) exit 2 ;;
esac
printf '%s\\n' "$*" >> "$RELEASE_CALLS"
n=$(wc -l < "$RELEASE_CALLS" | tr -d ' ')
if [ "$n" = "$RELEASE_FAIL_AT" ]; then
  echo "synthetic export failure at $n" >&2
  exit 23
fi
out=''
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then out=$2; shift; fi
  shift
done
case "$out" in
  */) mkdir -p "$out"; out="$out""export-$n.out" ;;
esac
[ -n "$out" ]
printf 'synthetic output %s\\n' "$n" > "$out"
`);
  for (const name of ["git", "gh", "kicad-cli"]) chmodSync(join(fake, name), 0o755);
  env.PATH = `${fake}:${env.PATH}`;
  const ids = ["gerbers", "drill", "cpl", "bom", "sch-pdf", "pcb-pdf", "step"];
  for (const failAt of [1, 4, 7, 0]) {
    const project = join(base, `project-${failAt}`);
    mkdirSync(project);
    writeFileSync(join(project, "example.kicad_pcb"), '(kicad_pcb (version 20240108))\n');
    writeFileSync(join(project, "example.kicad_sch"), '(kicad_sch (version 20250114))\n');
    for (const args of [["init", "-q"], ["add", "."], ["commit", "-qm", "Synthetic design"], ["tag", "-a", "fixture-tag", "-m", "Synthetic tag"]]) run([git, ...args], project, env);
    const calls = join(base, `calls-${failAt}.log`);
    const childEnv = { ...env, RELEASE_CALLS: calls, RELEASE_FAIL_AT: String(failAt) };
    const result = JSON.parse(run([binary, "--review", "release", project, "fixture-tag", home], base, childEnv));
    const invoked = readFileSync(calls, "utf8").trim().split("\n");
    const dir = join(project, ".backplane", "releases", "fixture-tag");
    if (failAt && !broken) {
      check(typeof result.err === "string" && result.err.includes(`(${ids[failAt - 1]})`), `failure ${failAt} identifies first failed step`);
      check(result.err.includes("exit 23") && result.err.includes(`synthetic export failure at ${failAt}`), `failure ${failAt} retains exit code and diagnostic`);
      check(invoked.length === failAt, `failure ${failAt} stops later exports`);
      check(!("dir" in result) && !("files" in result) && !("notes" in result), `failure ${failAt} has no success payload`);
      check(!existsSync(join(dir, "manifest.json")) && !existsSync(join(dir, "notes.md")), `failure ${failAt} writes no success metadata`);
    } else if (failAt) {
      check(!result.err && invoked.length === 7 && existsSync(join(dir, "manifest.json")), `baseline reproduces false success after failure ${failAt}`);
    } else {
      check(!result.err && result.dir === dir, "successful exports retain success payload");
      check(invoked.length === 7, "successful release runs all seven exports");
      check(result.files.length === 7 && result.files.every((f: string) => existsSync(join(dir, f))), "success lists actual output files");
      const manifest = JSON.parse(readFileSync(join(dir, "manifest.json"), "utf8"));
      check(manifest.tag === "fixture-tag" && JSON.stringify(manifest.files) === JSON.stringify(result.files), "success manifest matches outputs");
      check(readFileSync(join(dir, "notes.md"), "utf8") === result.notes, "success notes match result");
    }
    check(!readdirSync(temp).some(n => n.startsWith("bp-release.")), `case ${failAt} removes extracted source`);
    check(run([git, "tag", "--list"], project, env) === "fixture-tag", `case ${failAt} preserves original tag`);
  }
  if (binary.endsWith("release-child")) {
    const report = JSON.parse(run([binary, "--failed-report", base], base, env)).report;
    check(report.err.includes("synthetic export failure") && report.md === "", "failed child result reaches release error report");
  }
  check(!existsSync(publication), "no failed release attempts to publish");
  console.log(`${checks} release export assertions passed${broken ? " (baseline reproduction)" : ""}`);
} finally {
  rmSync(base, { recursive: true, force: true });
}
