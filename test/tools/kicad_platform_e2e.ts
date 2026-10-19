// Synthetic, offline acceptance of the production discovery and installer.
// scripts/build-app.sh test/native/kicad_platform.bend build/kicad_platform
// bun test/tools/kicad_platform_e2e.ts build/kicad_platform
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, rmSync, lstatSync, symlinkSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
const BASE = "30884966ef59c383581bcd1b7653f8f739cf009e";
function baselineSource(): string {
  const result = spawnSync("git", ["show", `${BASE}:src/server/kicad.bend`], {encoding:"utf8"});
  if (result.error || result.status !== 0) {
    const detail = result.error?.message ?? (result.stderr.trim() || `exit ${result.status}, signal ${result.signal}`);
    throw new Error(`Pinned KiCad baseline ${BASE} is unavailable: ${detail}\nFetch the exact fixture input before running acceptance: git fetch --no-tags --depth=1 origin ${BASE}`);
  }
  return result.stdout;
}
// Fail before running native fixtures when a shallow checkout lacks the pinned
// regression input. A failed git command's stdout is never accepted as source.
const baseline = baselineSource();
const binary = resolve(process.argv[2] ?? "build/kicad_platform");
const root = mkdtempSync(join(tmpdir(), "bp-kicad-platform-"));
const bin = join(root, "bin"), data = join(root, "data"), user = join(root, "user"), exe = join(root, "app");
for (const dir of [bin, data, user, exe]) mkdirSync(dir);
let checks = 0;
function check(name: string, condition: unknown) {
  if (!condition) throw new Error(name);
  console.log(`ok ${name}`); checks++;
}
function script(path: string, text: string) { mkdirSync(dirname(path), {recursive: true}); writeFileSync(path, `#!/bin/sh\n${text}\n`); chmodSync(path, 0o755); }
function which(name: string) { const p = spawnSync("sh", ["-c", 'command -v "$1"', "sh", name], {encoding: "utf8"}).stdout.trim(); if (!p) throw new Error(`missing fixture tool ${name}`); return p; }
const real = Object.fromEntries(["sh", "awk", "tar", "cp", "mv", "rm", "rmdir", "mkdir", "mktemp", "timeout", "sha256sum", "shasum", "perl", "cat", "sleep", "cut", "gzip"].map(n => [n, which(n)]));
for (const [name, target] of Object.entries(real)) symlinkSync(target, join(bin, name));
script(join(bin, "uname"), 'printf "%s\\n" "$TEST_PLATFORM"');
rmSync(join(bin, "mv")); script(join(bin, "mv"), `case "$1" in
  *.install-lock/stage/runtime) [ "\${FAIL_PROMOTE:-0}" != 1 ] || { echo 'injected rename failure' >&2; exit 42; };;
  *.install-lock/stage/previous) [ "\${FAIL_RESTORE:-0}" != 1 ] || { echo 'injected restore failure' >&2; exit 42; };;
esac
'${real.mv}' "$@" || exit $?
case "\${INTERRUPT_PROMOTE:-}:$1:$2" in
  after_backup:*:*.install-lock/stage/previous|after_publish:*.install-lock/stage/runtime:*)
    echo 'injected promotion interruption' >&2; kill -TERM "$PPID";;
esac`);
rmSync(join(bin, "tar")); script(join(bin, "tar"), `'${real.tar}' "$@" || exit $?\n[ "\${FAIL_TAR:-0}" != 1 ] || { echo 'injected partial extraction' >&2; exit 42; }`);
script(join(bin, "curl"), `out=''; while [ "$#" -gt 0 ]; do case "$1" in -o) out=$2; shift 2;; *) url=$1; shift;; esac; done
[ "\${FAIL_DOWNLOAD:-0}" != 1 ] || { echo 'injected download failure' >&2; exit 22; }
case "$url" in *SHA256SUMS) src=$TEST_SUMS;; *) src=$TEST_ARCHIVE;; esac
exec '${real.cp}' "$src" "$out"`);
const env = {...process.env, HOME: user, PATH: bin, TEST_PLATFORM: "Linux x86_64", BACKPLANE_KICAD: "", BACKPLANE_KICAD_CLI: "", BACKPLANE_HOME: data};
function run(args: string[], extra: Record<string,string> = {}) { return spawnSync(binary, args, {env: {...env,...extra}, encoding:"utf8", timeout:30000}); }
function runtime(dir: string, platform?: string, fail = false, bundle = false) {
  const cli = join(dir, bundle ? "Contents/MacOS/kicad-cli" : "bin/kicad-cli");
  const manifest = {sourceRepository:"https://github.com/i2cjak/Backplane_KiCad", sourceCommit:"5dbc22d292704d02b609d5b7a0edc971f1b7f08b", version:"10.99.0-bp.4", kicadVersion:"10.99.0", apiVersion:2, apiDescriptor:"share/backplane/api.desc", capabilities:["api-server","headless-pcb","headless-sch"], ...(platform ? {platform}: {})};
  const describe = {...manifest, platform:platform ?? "linux-x86_64"};
  script(cli, fail ? "echo cannot-execute >&2; exit 126" : `[ "\${FAIL_RUNTIME:-0}" != 1 ] || { echo 'injected runtime failure' >&2; exit 126; }\ncase "$1" in version) printf '%s\\n' "\${TEST_VERSION:-10.99.0}";; api-server) if [ -n "\${TEST_DESCRIBE:-}" ]; then printf '%s\\n' "$TEST_DESCRIBE"; else printf '%s\\n' '${JSON.stringify(describe)}'; fi;; *) exit 1;; esac`);
  const md = bundle ? join(dir,"Contents/Resources") : dir; mkdirSync(md, {recursive:true}); writeFileSync(join(md,"manifest.json"), JSON.stringify(manifest));
  mkdirSync(join(dir,"share/backplane"), {recursive:true}); writeFileSync(join(dir,"share/backplane/api.desc"), "synthetic descriptor");
  return cli;
}
function info(mode: string, platform: string) { const r = run([mode,data,exe], {TEST_PLATFORM:platform}); check(`${mode} ${platform} succeeds`, r.status === 0); return JSON.parse(r.stdout.trim()); }
function snapshot(dir: string): string {
  if (!existsSync(dir)) return "absent";
  return JSON.stringify(readdirSync(dir).sort().map(n => {const p=join(dir,n),s=lstatSync(p); return [n,s.mode,s.isSymbolicLink()?readlinkSync(p):s.isDirectory()?snapshot(p):readFileSync(p).toString("base64")];}));
}
try {
  const linux = runtime(join(data,"kicad/10.99.0-bp.4"));
  runtime(join(exe,"kicad"));
  const mac = runtime(join(user,"Applications/KiCad/KiCad.app"), undefined, false, true);
  // Stock KiCad still supports CLI work without being advertised as IPC-capable.
  rmSync(join(user,"Applications/KiCad/KiCad.app/Contents/Resources/manifest.json"));
  script(mac, "[ \"$1\" = version ] || exit 2; printf '10.0.6\\n'");
  script(join(bin,"kicad-cli"), "[ \"$1\" = version ] || exit 2; printf '10.0.6\\n'");
  check("Mac ignores legacy Linux managed and bundled copies", info("discover","Darwin arm64").cli === mac);
  const stock=info("refresh","Darwin arm64");
  check("stock Mac exports remain available without IPC", stock.cli === mac && stock.status === "stock" && stock.fork === false && stock.can_install === false);
  check("Intel Mac uses stock application", info("discover","Darwin x86_64").cli === mac);
  check("Linux ARM ignores Linux x64 managed install", info("discover","Linux aarch64").cli === join(bin,"kicad-cli"));
  const localMac=runtime(join(data,"kicad/10.99.0-bp.4-macos-arm64"), "macos-arm64");
  check("compatible isolated Mac fork is discoverable", info("discover","Darwin arm64").cli === localMac);
  const before=snapshot(data);
  const unsupported=run(["install",data], {TEST_PLATFORM:"Darwin arm64",FAIL_DOWNLOAD:"1"});
  check("unsupported installer reports unpublished platform accurately", unsupported.status===0 && unsupported.stdout.includes("no published runtime for Darwin arm64") && unsupported.stdout.includes("IPC requires"));
  check("unsupported installer leaves managed files unchanged", snapshot(data)===before);
  runtime(join(exe,"kicad"), "macos-arm64");
  check("Linux ignores wrong-platform bundled runtime", info("discover","Linux x86_64").cli === linux);
  runtime(join(data,"kicad/10.99.0-bp.4"), undefined, true);
  const healthy=info("refresh","Linux x86_64");
  check("broken managed runtime falls back to working stock CLI", healthy.cli===join(bin,"kicad-cli") && healthy.status==="stock");
  const explicit=run(["refresh",data,exe], {BACKPLANE_KICAD:dirname(dirname(linux))});
  check("explicit broken selection remains visible", explicit.status===0 && JSON.parse(explicit.stdout).status==="broken" && JSON.parse(explicit.stdout).cli===linux);
  // The actual check/export child resolver must agree with the hub's validated
  // cached selection, rather than reselecting the first existing managed file.
  writeFileSync(join(data,"kicad-health.json"),JSON.stringify(healthy));
  const cached=info("boot","Linux x86_64");
  check("hub accepts validated stock fallback cache",cached.cli===healthy.cli && cached.status==="stock" && cached.runs===true);
  const resolved=info("resolve","Linux x86_64");
  check("check and export resolver keeps healthy cached fallback",resolved.cli===cached.cli && resolved.status==="stock" && resolved.runs===true);
  const resolveRoot=run(["resolve",data,exe],{BACKPLANE_KICAD:dirname(dirname(linux))});
  check("resolver preserves broken explicit runtime root",resolveRoot.status===0 && JSON.parse(resolveRoot.stdout).cli===linux && JSON.parse(resolveRoot.stdout).status==="broken" && JSON.parse(resolveRoot.stdout).source==="env");
  const resolveCli=run(["resolve",data,exe],{BACKPLANE_KICAD_CLI:linux});
  check("resolver preserves broken explicit CLI argument",resolveCli.status===0 && JSON.parse(resolveCli.stdout).cli===linux && JSON.parse(resolveCli.stdout).status==="broken" && JSON.parse(resolveCli.stdout).source==="env");
  writeFileSync(join(data,"kicad-health.json"),JSON.stringify({...healthy,setting:linux}));
  const resolveSetting=run(["resolve",data,exe]);
  check("resolver preserves cached explicit setting failure",resolveSetting.status===0 && JSON.parse(resolveSetting.stdout).cli===linux && JSON.parse(resolveSetting.stdout).status==="broken" && JSON.parse(resolveSetting.stdout).source==="setting");
  const packageDir=join(root,"package"), archive=join(root,"runtime.tgz"), sums=join(root,"SHA256SUMS"), name="fixture.tar.gz";
  runtime(packageDir,"linux-x86_64");
  const tar=spawnSync(real.tar,["-czf",archive,"-C",root,"package"]); check("synthetic archive created",tar.status===0);
  const sha=createHash("sha256").update(readFileSync(archive)).digest("hex"); writeFileSync(sums,`${sha}  ${name}\n`);
  const final=join(root,"managed","10.99.0-bp.4"); runtime(final); writeFileSync(join(final,"user-marker"),"retain exact bytes\n");
  const prior=snapshot(final);
  const args=["transaction",final,"https://fixture.invalid/",name,sha,"10.99.0-bp.4"];
  const fixture={TEST_ARCHIVE:archive,TEST_SUMS:sums};
  for (const [label,fault,message] of [["download",{FAIL_DOWNLOAD:"1"},"injected download"],["partial extraction",{FAIL_TAR:"1"},"injected partial"],["promotion",{FAIL_PROMOTE:"1"},"injected rename"],["runtime launch",{FAIL_RUNTIME:"1"},"injected runtime"],["runtime version",{TEST_VERSION:"9.0.0"},"identity mismatch"],["wrong core with same build",{TEST_VERSION:"9.0.0-bp.4"},"identity mismatch"]] as const) {
    const r=run(args,{...fixture,...fault}); check(`${label} fails visibly`,r.status!==0 && (r.stdout+r.stderr).includes(message)); check(`${label} preserves prior runtime bytes`,snapshot(final)===prior);
  }
  for(const point of ["after_backup","after_publish"]) {
    const interrupted=run(args,{...fixture,INTERRUPT_PROMOTE:point});
    check(`${point} interruption fails visibly`,interrupted.status!==0 && (interrupted.stdout+interrupted.stderr).includes("injected promotion interruption"));
    check(`${point} interruption restores exact selected bytes`,snapshot(final)===prior);
    check(`${point} interruption removes staging without nested backup`,!existsSync(`${final}.install-lock`) && !existsSync(join(final,"previous")));
  }
  const restoreStage=join(`${final}.install-lock`,"stage");
  const failedRestore=run(args,{...fixture,INTERRUPT_PROMOTE:"after_publish",FAIL_RESTORE:"1"});
  check("failed restoration reports preserved backup path",failedRestore.status!==0 && (failedRestore.stdout+failedRestore.stderr).includes(`restore failed: previous runtime is at ${restoreStage}/previous`));
  check("failed restoration retains exact prior bytes",snapshot(join(restoreStage,"previous"))===prior);
  check("failed restoration keeps new runtime aside and lock",!existsSync(final) && existsSync(join(restoreStage,"failed/bin/kicad-cli")) && existsSync(`${final}.install-lock`));
  const restored=spawnSync(real.mv,[join(restoreStage,"previous"),final]);
  check("reported backup permits explicit recovery",restored.status===0 && snapshot(final)===prior);
  rmSync(`${final}.install-lock`,{recursive:true});
  const goodDescribe=JSON.parse(readFileSync(join(packageDir,"manifest.json"),"utf8"));
  for(const [label,description] of [
    ["malformed describe", "{broken"],
    ["empty describe object", "{}"],
    ["describe array shape", "[]"],
    ["wrong describe version", JSON.stringify({...goodDescribe,version:"10.99.0-bp.3"})],
    ["wrong describe API", JSON.stringify({...goodDescribe,apiVersion:1})],
    ["wrong describe platform", JSON.stringify({...goodDescribe,platform:"macos-arm64"})],
    ["wrong describe source", JSON.stringify({...goodDescribe,sourceCommit:"other"})],
    ["wrong describe repository", JSON.stringify({...goodDescribe,sourceRepository:"https://example.invalid/Backplane_KiCad"})],
    ["missing IPC capabilities", JSON.stringify({...goodDescribe,capabilities:["api-server"]})],
  ]) {
    const result=run(args,{...fixture,TEST_DESCRIBE:description});
    check(`${label} fails visibly`,result.status!==0 && (result.stdout+result.stderr).includes("identity mismatch"));
    check(`${label} preserves prior runtime bytes`,snapshot(final)===prior);
    check(`${label} releases transaction lock`,!existsSync(`${final}.install-lock`));
  }
  const bad=run(args.map((a,i)=>i===4?"0".repeat(64):a),fixture);
  check("checksum mismatch fails visibly",bad.status!==0 && (bad.stdout+bad.stderr).includes("checksum mismatch")); check("checksum failure preserves prior runtime bytes",snapshot(final)===prior);
  rmSync(join(bin,"sha256sum"));
  const shasum=run(args,fixture); if (shasum.status !== 0) console.error(shasum); check("portable shasum fallback installs successfully",shasum.status===0); check("successful transaction publishes new complete runtime",!existsSync(join(final,"user-marker")) && existsSync(join(final,"share/backplane/api.desc")));
  symlinkSync(real.sha256sum,join(bin,"sha256sum"));
  const current=snapshot(final);
  mkdirSync(`${final}.install-lock`);
  const locked=run(args,fixture); check("concurrent installer refused",locked.status!==0 && (locked.stdout+locked.stderr).includes("in progress")); check("lock refusal preserves runtime",snapshot(final)===current); rmSync(`${final}.install-lock`,{recursive:true});
  // Reproduce baseline's partial-overwrite defect using its exact shell body.
  const match=baseline.match(/"set -e; cd .*?"(?=,\n\s+"sh", dir, base, name, KiCad.sha256\(\))/);
  check("baseline installer source found",!!match);
  writeFileSync(join(final,"bin/kicad-cli"),"old runtime bytes\n");
  const baselineBefore=snapshot(final);
  const old=spawnSync(real.sh,["-c",JSON.parse(match![0]),"sh",final,"https://fixture.invalid/",name,sha],{env:{...env,...fixture,FAIL_TAR:"1"},encoding:"utf8"});
  check("baseline partial extraction fails",old.status!==0);
  check("baseline reproduces overwrite of selected runtime",snapshot(final)!==baselineBefore && readFileSync(join(final,"bin/kicad-cli"),"utf8").startsWith("#!/bin/sh"));
  console.log(`${checks} assertions passed`);
} finally { rmSync(root,{recursive:true,force:true}); }
