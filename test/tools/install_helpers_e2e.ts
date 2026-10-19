// Exercise the real release installer and updater with generated files.
// No release download, installed Backplane, hub, or provider is used.
//
//   bun test/tools/install_helpers_e2e.ts --installer-only
//   bend test/native/update_swap.bend -o build/update-swap
//   bun test/tools/install_helpers_e2e.ts build/update-swap
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const args = process.argv.slice(2);
if (args.length !== 1) throw new Error("usage: install_helpers_e2e.ts --installer-only|SWAP_FIXTURE");
const fixture = args[0] === "--installer-only" ? undefined : resolve(args[0]);
if (fixture && !existsSync(fixture)) throw new Error(`compile test/native/update_swap.bend first: ${fixture}`);
const template = readFileSync("deploy/install.sh.in", "utf8");
const helpers = ["backplane-browser", "backplane-step2glb", "backplane-voice", "backplane-kipc"];
const platforms = [
  { name: "linux-x64", os: "Linux", arch: "x86_64" },
  { name: "linux-arm64", os: "Linux", arch: "aarch64" },
  { name: "darwin-arm64", os: "Darwin", arch: "arm64" },
];
const version = "v0.0.0-install-test";
const root = realpathSync(mkdtempSync(join(tmpdir(), "bp-install-helpers-")));
let checks = 0, failures = 0;
function check(label: string, ok: boolean, detail?: unknown) {
  checks++;
  console.log(`${ok ? "ok" : "FAIL"} ${label}${ok ? "" : `: ${JSON.stringify(detail)?.slice(0, 500)}`}`);
  if (!ok) failures++;
}
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
const text = (path: string) => existsSync(path) ? readFileSync(path, "utf8") : "";
const marker = (name: string, revision: string) => `#!/bin/sh\nprintf '%s\\n' '${revision} ${name}'\n`;
function put(path: string, value: string, executable = false) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, value);
  if (executable) chmodSync(path, 0o755);
}
function stage(dir: string, revision: string, present: string[]) {
  for (const name of ["backplane", "backplane-serve", ...present]) put(join(dir, name), marker(name, revision), true);
  put(join(dir, "web", "index.html"), `${revision} web`);
  put(join(dir, "LICENSE"), `${revision} license`);
  put(join(dir, "licenses", "NOTICE"), `${revision} notice`);
}
function snapshot(dir: string): unknown[] {
  return readdirSync(dir).sort().map(name => {
    const path = join(dir, name), st = lstatSync(path);
    if (st.isSymbolicLink()) return [name, "link", readlinkSync(path)];
    if (st.isDirectory()) return [name, "dir", snapshot(path)];
    return [name, st.mode & 0o777, readFileSync(path).toString("base64")];
  });
}
function run(command: string, argv: string[], env: Record<string, string>, cwd: string) {
  const result = spawnSync(command, argv, { env, cwd, encoding: "utf8", timeout: 15000 });
  if (result.error) throw result.error;
  return result;
}
function environment(dir: string) {
  const home = join(dir, "user"), temp = join(dir, "temp");
  mkdirSync(home, { recursive: true });
  mkdirSync(temp, { recursive: true });
  return { HOME: home, BACKPLANE_HOME: join(home, "backplane-test"), TMPDIR: temp, PATH: "/usr/bin:/bin", LANG: "C", BACKPLANE_NO_UPDATE: "1" };
}
function installed(label: string, prefix: string, present: string[], revision = "new") {
  for (const name of ["backplane", "backplane-serve", ...present]) {
    const path = join(prefix, name);
    check(`${label}: ${name} installed beside hub`, text(path) === marker(name, revision), text(path));
    check(`${label}: ${name} remains executable`, existsSync(path) && (lstatSync(path).mode & 0o111) !== 0);
  }
  check(`${label}: current web client installed`, text(join(prefix, "web", "index.html")) === `${revision} web`);
}

try {
  // uname selects each supported archive; curl only copies a generated
  // archive after checking the exact requested release asset URL.
  for (const platform of platforms) {
    for (const variant of ["modern", "without-kipc", "without-helpers"]) {
      const present = variant === "modern" ? helpers : variant === "without-kipc" ? helpers.slice(0, 3) : [];
      const pack = join(root, "archives", platform.name, variant);
      const source = join(pack, `backplane-${platform.name}`);
      stage(source, "new", present);
      const archive = join(pack, `backplane-${version}-${platform.name}.tar.gz`);
      const packed = run("/usr/bin/tar", ["-czf", archive, "-C", pack, `backplane-${platform.name}`], environment(pack), pack);
      if (packed.status !== 0) throw new Error(packed.stderr);
      const sum = createHash("sha256").update(readFileSync(archive)).digest("hex");
      for (const upgrade of [false, true]) {
        const label = `installer ${platform.name} ${variant} ${upgrade ? "upgrade" : "fresh"}`;
        const dir = join(root, "installer", platform.name, variant, upgrade ? "upgrade" : "fresh");
        const prefix = join(dir, "prefix with spaces"), bin = join(dir, "bin with spaces"), fake = join(dir, "fake");
        const env = environment(dir);
        mkdirSync(bin, { recursive: true });
        mkdirSync(prefix, { recursive: true });
        if (upgrade) {
          stage(prefix, "old", helpers);
          put(join(prefix, "web.old", "obsolete"), "obsolete");
          put(join(prefix, "licenses", "obsolete"), "obsolete");
        }
        put(join(fake, "uname"), "#!/bin/sh\ncase $1 in -s) printf '%s\\n' \"$TEST_OS\";; -m) printf '%s\\n' \"$TEST_ARCH\";; *) exit 94;; esac\n", true);
        put(join(fake, "curl"), `#!/bin/sh
set -eu
output= url=
while [ "$#" -gt 0 ]; do
  case $1 in
    -o) output=$2; shift 2;;
    --proto) shift 2;;
    --tlsv1.2|-fsSL) shift;;
    https://*) url=$1; shift;;
    *) exit 93;;
  esac
done
[ "$url" = "$EXPECTED_URL" ] && [ -n "$output" ] || exit 92
printf '%s\\n' "$url" >> "$DOWNLOAD_LOG"
cp "$OFFLINE_ARCHIVE" "$output"
`, true);
        const url = `https://github.com/i2cjak/Backplane/releases/download/${version}/backplane-${version}-${platform.name}.tar.gz`;
        const script = join(dir, "install.sh");
        put(script, template.replaceAll("@VERSION@", version)
          .replaceAll("@SHA_LINUX_X64@", platform.name === "linux-x64" ? sum : "1".repeat(64))
          .replaceAll("@SHA_LINUX_ARM64@", platform.name === "linux-arm64" ? sum : "2".repeat(64))
          .replaceAll("@SHA_DARWIN_ARM64@", platform.name === "darwin-arm64" ? sum : "3".repeat(64)));
        const installedEnv = { ...env, PATH: `${fake}:/usr/bin:/bin`, BACKPLANE_PREFIX: prefix, BACKPLANE_BIN: bin, TEST_OS: platform.os, TEST_ARCH: platform.arch, EXPECTED_URL: url, OFFLINE_ARCHIVE: archive, DOWNLOAD_LOG: join(dir, "downloads") };
        const result = run("/bin/sh", [script], installedEnv, dir);
        check(`${label}: installer succeeds`, result.status === 0, result.stderr);
        check(`${label}: selects the requested platform archive`, text(installedEnv.DOWNLOAD_LOG).trim() === url, text(installedEnv.DOWNLOAD_LOG));
        installed(label, prefix, present);
        for (const name of helpers.filter(name => !present.includes(name))) {
          check(`${label}: absent optional ${name} stays compatible`, upgrade ? text(join(prefix, name)) === marker(name, "old") : !existsSync(join(prefix, name)));
        }
        for (const name of ["backplane", "backplane-serve"]) {
          check(`${label}: ${name} command links to prefix`, lstatSync(join(bin, name)).isSymbolicLink() && readlinkSync(join(bin, name)) === join(prefix, name));
        }
        check(`${label}: license installed`, text(join(prefix, "LICENSE")) === "new license");
        check(`${label}: license notices installed`, text(join(prefix, "licenses", "NOTICE")) === "new notice" && !existsSync(join(prefix, "licenses", "obsolete")));
        check(`${label}: previous web client retained only for upgrade`, upgrade ? text(join(prefix, "web.old", "index.html")) === "old web" && !existsSync(join(prefix, "web.old", "obsolete")) : !existsSync(join(prefix, "web.old")));

        // A rejected archive must leave this successful installation,
        // its helper versions, notices, backups and command links alone.
        const before = [snapshot(prefix), snapshot(bin)];
        put(script, text(script).replaceAll(sum, "0".repeat(64)));
        const rejected = run("/bin/sh", [script], installedEnv, dir);
        check(`${label}: checksum mismatch is refused`, rejected.status !== 0 && rejected.stderr.includes("does not match its sha256"), rejected);
        check(`${label}: checksum failure preserves existing installation`, same([snapshot(prefix), snapshot(bin)], before));
      }
    }
  }

  if (fixture) {
    // The actual native Update.swap handles both archive generations.
    // Missing optional files must remain compatible with older releases.
    for (const variant of ["modern", "without-kipc", "without-helpers"]) {
      const present = variant === "modern" ? helpers : variant === "without-kipc" ? helpers.slice(0, 3) : [];
      for (const upgrade of [false, true]) {
        const label = `updater ${variant} ${upgrade ? "upgrade" : "fresh"}`;
        const dir = join(root, "updater", variant, upgrade ? "upgrade" : "fresh");
        const source = join(dir, "source with spaces"), prefix = join(dir, "prefix with spaces");
        const env = environment(dir);
        stage(source, "new", present);
        mkdirSync(prefix, { recursive: true });
        if (upgrade) stage(prefix, "old", helpers);
        const tag = "v0.0.0-updater-test";
        const result = run(fixture, [source, prefix, tag], env, dir);
        check(`${label}: production file swap succeeds`, result.status === 0 && result.stdout.includes("ok update swap") && !/^FAIL\b/m.test(result.stdout), result);
        installed(label, prefix, present);
        for (const name of present) check(`${label}: ${name} moved from archive`, !existsSync(join(source, name)));
        for (const name of helpers.filter(name => !present.includes(name))) {
          check(`${label}: missing optional ${name} leaves existing install alone`, upgrade ? text(join(prefix, name)) === marker(name, "old") : !existsSync(join(prefix, name)));
        }
        check(`${label}: previous web retained under release tag`, upgrade ? text(join(prefix, `web.old-${tag}`, "index.html")) === "old web" : !existsSync(join(prefix, `web.old-${tag}`)));
      }
    }
  }
} finally {
  rmSync(root, { recursive: true, force: true });
}
console.log(`${checks} assertions, ${failures} failures${fixture ? "" : " (installer only)"}`);
process.exit(failures ? 1 : 0);
