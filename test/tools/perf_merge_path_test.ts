// No GitHub/network: real Git's compare-and-swap against an isolated bare repo.
import assert from 'node:assert/strict';
import {copyFileSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const temp=mkdtempSync(join(tmpdir(),'bp-perf-merge-')),root=join(temp,'candidate'),remote=join(temp,'remote.git');
const realGit=Bun.which('git')!;
function git(...args:string[]) {
  const p=Bun.spawnSync([realGit,'-C',root,...args]);assert.equal(p.exitCode,0,p.stderr.toString());return p.stdout.toString().trim();
}
function put(file:string,body:string) {writeFileSync(file,body,{mode:0o755});}
try {
  mkdirSync(root);mkdirSync(join(root,'scripts'));mkdirSync(join(temp,'bin'));
  copyFileSync(resolve(import.meta.dir,'../../scripts/perf-merge.sh'),join(root,'scripts/perf-merge.sh'));
  git('init','-q');git('config','user.name','i2cjak');git('config','user.email','i2cjak@users.noreply.github.com');
  git('add','.');git('commit','-qm','base');const base=git('rev-parse','HEAD');
  git('commit','--allow-empty','-qm','measured');const head=git('rev-parse','HEAD');
  git('checkout','-q','--detach',base);git('commit','--allow-empty','-qm','concurrent main');const race=git('rev-parse','HEAD');
  git('checkout','-q','--detach',head);
  assert.equal(Bun.spawnSync([realGit,'init','--bare','-q',remote]).exitCode,0);
  git('remote','add','origin',remote);git('push','-q','origin',base+':refs/heads/main');
  git('push','-q','origin',race+':refs/heads/race-fixture');
  put(join(temp,'bin/git'),`#!/bin/sh
if [ "$1 $2 $3" = 'remote get-url origin' ]; then echo https://github.com/i2cjak/Backplane.git; exit 0; fi
case "$*" in *'push --porcelain'*)
  if [ "$RACE" = 1 ]; then "$REAL_GIT" --git-dir "$TEST_REMOTE" update-ref refs/heads/main "$RACE_SHA" "$PR_BASE"; fi;;
esac
exec "$REAL_GIT" "$@"
`);
  put(join(temp,'bin/gh'),`#!/bin/sh
case "$*" in
  'api user --jq .login') echo i2cjak;;
  *'--jq .base.sha') echo "$PR_BASE";;
  *'--jq .base.ref') echo main;;
  *'--jq .base.repo.full_name') echo i2cjak/Backplane;;
  *'--jq .headRefOid') echo "$PR_HEAD";;
  *'--jq .object.sha') "$REAL_GIT" --git-dir "$TEST_REMOTE" rev-parse refs/heads/main;;
  *) echo "unexpected gh call: $*" >&2; exit 95;;
esac
`);
  put(join(temp,'bin/bun'),'#!/bin/sh\nprintf "%s\\n" "$*" > "$VERIFY_ARGS"\nexit 0\n');
  const args=join(temp,'verify.args');
  const run=(prBase=base,racing=false,extra:string[]=[],prHead=head)=>Bun.spawnSync(['sh','scripts/perf-merge.sh','7','evidence','binary','fixture','receipt',...extra],{cwd:root,
    env:{...process.env,PATH:join(temp,'bin')+':'+process.env.PATH,REAL_GIT:realGit,TEST_REMOTE:remote,
      PR_BASE:prBase,PR_HEAD:prHead,RACE:racing?'1':'0',RACE_SHA:race,VERIFY_ARGS:args}});
  assert.notEqual(run(base,false,['linux-native','timeline']).exitCode,0,'caller cannot reduce policy');
  // Actual main contains a clean, unrelated newer change not in the head.
  git('--git-dir',remote,'update-ref','refs/heads/main',race);
  assert.notEqual(run(race).exitCode,0,'unmeasured combined merge tree rejected');
  assert.equal(git('--git-dir',remote,'rev-parse','refs/heads/main'),race);
  git('--git-dir',remote,'update-ref','refs/heads/main',base);
  const raced=run(base,true);assert.notEqual(raced.exitCode,0,'base race after validation must reject');
  assert.equal(git('--git-dir',remote,'rev-parse','refs/heads/main'),race);
  git('--git-dir',remote,'update-ref','refs/heads/main',base);
  const passed=run();assert.equal(passed.exitCode,0,passed.stderr.toString());
  assert.equal(git('--git-dir',remote,'rev-parse','refs/heads/main'),head);
  assert.equal(readFileSync(args,'utf8').trim(),'scripts/perf-verify.ts evidence binary fixture receipt linux-native long-history timeline subagents rooms bots status');
  git('checkout','-q','--detach',base);mkdirSync(join(root,'mobile'));writeFileSync(join(root,'mobile/phone.swift'),'changed client\n');
  git('add','.');git('commit','-qm','changed unsupported native client');const phone=git('rev-parse','HEAD');
  git('--git-dir',remote,'update-ref','refs/heads/main',base);
  const unsupported=run(base,false,[],phone);assert.notEqual(unsupported.exitCode,0);
  assert.match(unsupported.stderr.toString(),/additional changed-client performance evidence/);
  console.log('PASS real atomic FF-only Git integration: old base, caller-reduced coverage and last-moment base race rejected; exact measured head retained');
} finally {rmSync(temp,{recursive:true,force:true});}
