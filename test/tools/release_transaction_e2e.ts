import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";

// Runs the real release job and its real child processes. Every repository,
// home, CAD executable and potential publication command belongs to this test.
const binary = resolve(process.argv[2] ?? "build/release-child");
const baseline = process.argv.includes("--baseline");
const root = mkdtempSync(join(tmpdir(), "bp-release-transaction-"));
let assertions = 0;
function check(ok: unknown, name: string) {
  if (!ok) throw new Error(`FAIL ${name}`);
  assertions++; console.log(`ok ${name}`);
}
function run(args: string[], cwd: string, env: Record<string, string>) {
  const r = Bun.spawnSync(args, { cwd, env, stdout: "pipe", stderr: "pipe", timeout: 90_000 });
  if (r.exitCode !== 0) throw new Error(`${args[0]} exited ${r.exitCode}: ${r.stderr.toString()} ${r.stdout.toString()}`);
  return r.stdout.toString().trim();
}
function hash(path: string) { return createHash("sha256").update(readFileSync(path)).digest("hex"); }
try {
  const user = join(root, "user"), home = join(root, "home"), fake = join(root, "bin"), temp = join(root, "tmp");
  for (const p of [user, home, fake, temp]) mkdirSync(p);
  const env: Record<string, string> = {
    PATH: process.env.PATH!, HOME: user, BACKPLANE_HOME: home, TMPDIR: temp,
    XDG_CONFIG_HOME: join(user, ".config"), XDG_DATA_HOME: join(user, ".local/share"),
    GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: join(root, "gitconfig"),
    GIT_AUTHOR_NAME: "Fixture", GIT_AUTHOR_EMAIL: "fixture@example.invalid",
    GIT_COMMITTER_NAME: "Fixture", GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    BACKPLANE_KICAD_CLI: join(fake, "kicad-cli"), RELEASE_PUBLICATION_LOG: join(root, "publication.log"),
  };
  const locate = (name: string) => run(["sh", "-c", `command -v ${name}`], root, env);
  const git = locate("git"), realFind = locate("find"), realMkdir = locate("mkdir"), node = locate("node");
  const quote = (s: string) => `'${s.replaceAll("'", "'\\''")}'`;
  writeFileSync(join(fake, "git"), `#!/bin/sh
if [ "$1" = push ]; then
  printf '%s\\n' "$*" >> "$RELEASE_PUBLICATION_LOG"
  if [ "\${RELEASE_TEST_PUBLISH:-}" != 1 ]; then exit 91; fi
  if [ "$RELEASE_MODE" = move-tag-on-push ]; then
    ${quote(git)} -C "$RELEASE_PROJECT" commit --allow-empty -qm 'local tag movement during publication'
    ${quote(git)} -C "$RELEASE_PROJECT" tag -f -a fixture-tag -m 'moved local tag' HEAD >/dev/null
  fi
fi
exec ${quote(git)} "$@"
`);
  writeFileSync(join(fake, "gh"), '#!/bin/sh\nif [ "${RELEASE_TEST_PUBLISH:-}" = 1 ]; then echo gh >> "$RELEASE_PUBLICATION_LOG"; echo https://fixture.invalid/local-release; exit 0; fi\necho gh >> "$RELEASE_PUBLICATION_LOG"\nexit 91\n');
  writeFileSync(join(fake, "find"), `#!/bin/sh\ncase "$PWD" in *.pending.*) if [ "$RELEASE_MODE" = list-fail ]; then echo 'synthetic listing failure' >&2; exit 26; fi;; esac\nexec ${quote(realFind)} "$@"\n`);
  writeFileSync(join(fake, "mkdir"), `#!/bin/sh\ncase "$*" in *.pending.*/gerbers*) if [ "$RELEASE_MODE" = mkdir-fail ]; then echo 'synthetic mkdir failure' >&2; exit 27; fi;; esac\nexec ${quote(realMkdir)} "$@"\n`);
  writeFileSync(join(fake, "kicad-cli"), `#!${node}
const fs = require('fs'), cp = require('child_process'), path = require('path');
const args = process.argv.slice(2), mode = process.env.RELEASE_MODE;
if (args[0] === 'version' || args.includes('--version')) { console.log('10.0.6'); process.exit(0); }
if (args[0] === 'api-server') process.exit(2);
const out = args[args.indexOf('-o')+1], input = args.at(-1);
const data = fs.readFileSync(input, 'utf8');
const checking = args[1] === 'erc' || args[1] === 'drc';
fs.appendFileSync(process.env.RELEASE_CALLS, JSON.stringify({args, checking, data})+'\\n');
if (checking) {
  if (mode === 'check-fail') { console.error('synthetic CAD invocation failure'); process.exit(23); }
  if (mode === 'missing-report') process.exit(0);
  if (mode === 'malformed-report') { fs.writeFileSync(out, '{broken'); process.exit(0); }
  if (mode === 'wrong-shape') { fs.writeFileSync(out, '{}'); process.exit(0); }
  const violations = mode === 'design-error' ? [{type:'clearance',severity:'error',description:'synthetic design finding',items:[]}] : [];
  fs.writeFileSync(out, JSON.stringify(args[1] === 'erc' ? {sheets:[{path:'/',violations}]} : {violations,unconnected_items:[],schematic_parity:[]}));
  if (mode === 'move-head' && args[1] === 'erc') {
    fs.writeFileSync(path.join(process.env.RELEASE_PROJECT,'head-moved.txt'),'new committed source');
    cp.execFileSync(process.env.RELEASE_REAL_GIT,['add','head-moved.txt'],{cwd:process.env.RELEASE_PROJECT});
    cp.execFileSync(process.env.RELEASE_REAL_GIT,['commit','-qm','move HEAD during checks'],{cwd:process.env.RELEASE_PROJECT});
  }
} else {
  const exportCount = fs.readFileSync(process.env.RELEASE_CALLS,'utf8').trim().split('\\n').map(JSON.parse).filter(c=>!c.checking).length;
  if (mode === 'empty-export' || (mode === 'missing-drill' && exportCount === 2)) process.exit(0);
  if (mode === 'empty-file' && exportCount === 3) { fs.writeFileSync(out,''); process.exit(0); }
  if (mode === 'export-fail') { fs.writeFileSync(out.endsWith('/') ? out+'partial.out' : out,'PARTIAL'); console.error('synthetic export failure'); process.exit(24); }
  if (out.endsWith('/')) { fs.mkdirSync(out,{recursive:true}); fs.writeFileSync(out+args[2]+'.out',data); }
  else fs.writeFileSync(out,data);
  if (mode === 'metadata-fail') fs.mkdirSync(path.join(out.includes('/gerbers/') ? path.dirname(out.endsWith('/') ? out.slice(0,-1) : out) : path.dirname(out),'manifest.json'),{recursive:true});
}
`);
  for (const name of ["git", "gh", "find", "mkdir", "kicad-cli"]) chmodSync(join(fake, name), 0o755);
  env.PATH = `${fake}:${env.PATH}`;
  let sequence = 0;
  function project(name: string, gateOff = false) {
    const p = join(root, `${++sequence}-${name}`); mkdirSync(p); mkdirSync(join(p, ".backplane"));
    writeFileSync(join(p, "example.kicad_pcb"), '(kicad_pcb (version 20240108) (generator "COMMITTED"))\n');
    writeFileSync(join(p, "example.kicad_sch"), '(kicad_sch (version 20250114) (generator "COMMITTED"))\n');
    if (gateOff) writeFileSync(join(p, ".backplane/checks.json"), JSON.stringify({ gate: { erc: "off", drc: "off" } }));
    for (const args of [["init", "-q"], ["add", "."], ["commit", "-qm", "Synthetic committed design"]]) run([git, ...args], p, env);
    return { p, sha: run([git,"rev-parse","HEAD"],p,env), calls: join(root, `calls-${sequence}.jsonl`) };
  }
  function job(f: ReturnType<typeof project>, mode: string, flags = "", tag = "fixture-tag") {
    const e = {...env, RELEASE_MODE: mode, RELEASE_CALLS: f.calls, RELEASE_PROJECT: f.p, RELEASE_REAL_GIT: git};
    return JSON.parse(run([binary, "--release-job", f.p, tag, home, flags], root, e)).report;
  }
  function child(f: ReturnType<typeof project>, mode: string, tag = "fixture-tag") {
    const e = {...env, RELEASE_MODE: mode, RELEASE_CALLS: f.calls, RELEASE_PROJECT: f.p, RELEASE_REAL_GIT: git};
    return JSON.parse(run([binary, "--review", "release", f.p, tag, home], root, e));
  }
  function calls(f: ReturnType<typeof project>) { return existsSync(f.calls) ? readFileSync(f.calls,"utf8").trim().split("\n").filter(Boolean).map(JSON.parse) : []; }
  const output = (f: ReturnType<typeof project>) => join(f.p,".backplane/releases/fixture-tag");
  if (baseline) {
    for (const mode of ["check-fail", "missing-report", "wrong-shape"]) {
      const f=project(mode); const r=job(f,mode);
      check(!r.err && existsSync(join(output(f),"manifest.json")), `baseline silently releases after ${mode}`);
    }
    const f=project("dirty"); writeFileSync(join(f.p,"example.kicad_pcb"),'(kicad_pcb (generator "DIRTY"))\n');
    const r=job(f,"ok");
    check(!r.err && calls(f).some(c=>c.checking&&c.data.includes("DIRTY")) && calls(f).some(c=>!c.checking&&c.data.includes("COMMITTED")),"baseline checks dirty work but exports committed bytes");
    const m=project("metadata-fail"); const mr=job(m,"metadata-fail");
    check(!mr.err && !statSync(join(output(m),"manifest.json")).isFile(),"baseline reports success despite unwritable manifest");
    console.log(`${assertions} baseline defects reproduced`);
  } else {
    for (const mode of ["check-fail", "missing-report", "malformed-report", "wrong-shape"]) {
      for (const force of ["", "f"]) {
        const f = project(mode, true); const r = job(f, mode, force);
        check(!!r.err && r.err.includes("checks could not complete"), `${mode} refuses release with gate off and force=${!!force}`);
        check(!run([git,"tag","--list"],f.p,env),`${mode} creates no tag`);
        check(!existsSync(output(f)) && !calls(f).some(c=>!c.checking),`${mode} creates no exports`);
      }
    }
    for (const dirty of ["unstaged", "staged", "untracked", "tracked-config", "untracked-config", "untracked-checklist"]) {
      const f=project(dirty,dirty!=="untracked-config" && dirty!=="untracked-checklist");
      if (dirty==="untracked-config") writeFileSync(join(f.p,".backplane/checks.json"),"{}");
      else if (dirty==="untracked-checklist") writeFileSync(join(f.p,".backplane/checklist.md"),"- [x] approved\n");
      else if (dirty==="untracked") writeFileSync(join(f.p,"new.kicad_pcb"),"untracked");
      else if (dirty==="tracked-config") writeFileSync(join(f.p,".backplane/checks.json"),"{}");
      else { writeFileSync(join(f.p,"example.kicad_pcb"),"DIRTY"); if(dirty==="staged") run([git,"add","example.kicad_pcb"],f.p,env); }
      const r=job(f,"ok"); check(!!r.err && r.err.includes("commit or stash"),`${dirty} work refused visibly`);
      check(!calls(f).length && !run([git,"tag","--list"],f.p,env),`${dirty} starts no checks or tag`);
    }
    const moved=project("move-head"); const movedResult=job(moved,"move-head");
    check(!!movedResult.err && movedResult.err.includes("HEAD moved"),"HEAD movement refuses tag after immutable checks");
    check(calls(moved).filter(c=>c.checking).every(c=>c.data.includes("COMMITTED")),"both checks use immutable source despite HEAD movement");
    check(!run([git,"tag","--list"],moved.p,env),"HEAD movement leaves no tag");
    const design=project("design-default"); check(!!job(design,"design-error").err,"default gate blocks design findings");
    const forced=project("design-force"); check(!job(forced,"design-error","f").err,"deliberate force allows design findings");
    const off=project("design-off",true); check(!job(off,"design-error").err,"deliberate gate off allows design findings");
    for (const mode of ["export-fail", "metadata-fail", "mkdir-fail", "list-fail", "empty-export", "missing-drill", "empty-file"]) {
      const f=project(mode); const r=job(f,mode);
      check(!!r.err && !r.md,`${mode} reaches visible error without success report`);
      check(!existsSync(output(f)),`${mode} publishes no partial output directory`);
      check(!readdirSync(join(f.p,".backplane/releases")).some(n=>n.includes(".pending.") || n.endsWith(".lock")),`${mode} removes staging and releases lock`);
      const retry=job(f,"ok"); check(!retry.err && existsSync(join(output(f),"manifest.json")),`${mode} retries explicitly at same tag`);
    }
    const externalBoard=join(root,"external.kicad_pcb"), externalSch=join(root,"external.kicad_sch");
    writeFileSync(externalBoard,'(kicad_pcb (generator "EXTERNAL"))\n');
    writeFileSync(externalSch,'(kicad_sch (generator "EXTERNAL"))\n');
    for(const kind of ["absolute-config","parent-config","linked-board","external-sheet"]) {
      const f=project(kind);
      if(kind==="absolute-config") writeFileSync(join(f.p,".backplane.json"),JSON.stringify({pcb:externalBoard,schematic:externalSch}));
      if(kind==="parent-config") writeFileSync(join(f.p,".backplane.json"),JSON.stringify({pcb:"../../external.kicad_pcb",schematic:externalSch}));
      if(kind==="linked-board") { rmSync(join(f.p,"example.kicad_pcb")); symlinkSync(externalBoard,join(f.p,"example.kicad_pcb")); }
      if(kind==="external-sheet") writeFileSync(join(f.p,"example.kicad_sch"),'(kicad_sch (sheet (property "Sheetname" "outside") (property "Sheetfile" "'+externalSch+'")))\n');
      run([git,"add","."],f.p,env);run([git,"commit","-qm",kind],f.p,env);
      const r=job(f,"ok","f");
      check(!!r.err && !r.md,`${kind} cannot release external mutable source`);
      check(!calls(f).length && !run([git,"tag","--list"],f.p,env),`${kind} runs no CAD commands or tag`);
    }
    const noDesign=project("no-design");
    for(const n of ["example.kicad_pcb","example.kicad_sch"]) rmSync(join(noDesign.p,n));
    run([git,"add","-u"],noDesign.p,env);run([git,"commit","-qm","no CAD"],noDesign.p,env);
    check(!!job(noDesign,"ok").err && !existsSync(output(noDesign)),"no CAD inputs cannot publish metadata-only release");
    const guardRoot=join(root,"guard");mkdirSync(guardRoot);mkdirSync(join(guardRoot,"nested"));writeFileSync(join(guardRoot,"inside.kicad_pcb"),"inside");
    const boundEnv={...env,RELEASE_MODE:"ok"};
    check(run([binary,"--bound",guardRoot,join(guardRoot,"nested/../inside.kicad_pcb")],root,boundEnv)==="bound","internal committed parent references remain valid");
    const escaped=Bun.spawnSync([binary,"--bound",guardRoot,join(guardRoot,"../external.kicad_pcb")],{env:boundEnv,stdout:"pipe",stderr:"pipe"});
    check(escaped.exitCode!==0 && (escaped.stdout.toString()+escaped.stderr.toString()).includes("escapes committed snapshot"),"existing parent traversal outside snapshot refused");
    const good=project("success"); const success=job(good,"ok"); const dir=output(good);
    check(!success.err && success.md.includes(good.sha),"success reports the immutable source commit");
    const manifest=JSON.parse(readFileSync(join(dir,"manifest.json"),"utf8"));
    check(manifest.commit===good.sha && manifest.tag==="fixture-tag", "manifest and tag bind to checked commit");
    check(run([git,"rev-parse","fixture-tag^{commit}"],good.p,env)===good.sha,"annotated tag names checked commit");
    check(manifest.files.length===7 && manifest.files.every((n:string)=>readFileSync(join(dir,n),"utf8").includes("COMMITTED")),"all seven outputs use immutable committed bytes");
    const hashes=Object.fromEntries([...manifest.files,"manifest.json","notes.md"].map((n:string)=>[n,hash(join(dir,n))]));
    const before=calls(good).length; const existing=child(good,"export-fail");
    check(!!existing.err && existing.err.includes("preserved unchanged"),"retry refuses a complete previous output");
    check(calls(good).length===before,"complete output is not re-exported");
    check(Object.entries(hashes).every(([n,h])=>hash(join(dir,n))===h),"previous complete output bytes remain intact");
    check(!readdirSync(temp).some(n=>n.startsWith("bp-review.") || n.startsWith("bp-release.")),"all extracted check and release sources cleaned");
    check(!existsSync(env.RELEASE_PUBLICATION_LOG),"no test uses a publishing command");
    const published=project("local-publication");
    const remote=join(root,"local-remote.git");run([git,"init","--bare","-q",remote],root,env);run([git,"remote","add","origin",remote],published.p,env);
    const pubEnv={...env,RELEASE_MODE:"move-tag-on-push",RELEASE_CALLS:published.calls,RELEASE_PROJECT:published.p,RELEASE_REAL_GIT:git,RELEASE_TEST_PUBLISH:"1"};
    const pub=JSON.parse(run([binary,"--release-job",published.p,"fixture-tag",home,"p"],root,pubEnv)).report;
    check(!pub.err && pub.md.includes("Published:"),"stand-in publication completes after local tag movement");
    check(run([git,"--git-dir",remote,"rev-parse","fixture-tag^{commit}"],root,env)===published.sha,"immutable refspec publishes checked commit despite moved local tag");
    check(run([git,"rev-parse","fixture-tag^{commit}"],published.p,env)!==published.sha,"local publication negative control really moved tag");
    check(readFileSync(env.RELEASE_PUBLICATION_LOG,"utf8").includes(":refs/tags/fixture-tag"),"publication pushes immutable object refspec");
    console.log(`${assertions} release transaction assertions passed`);
  }
} finally { rmSync(root, {recursive:true,force:true}); }
