// Exercise the real release build entry point with cheap process doubles.
// These tests establish dispatch/fail-closed ordering, not native performance.
import assert from 'node:assert/strict';
import {copyFileSync,existsSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';

const root=resolve(import.meta.dir,'../..');
const temp=mkdtempSync(join(tmpdir(),'bp-perf-build-path-'));
function put(name:string,body:string) {
  const path=join(temp,name);mkdirSync(resolve(path,'..'),{recursive:true});
  writeFileSync(path,body,{mode:0o755});
}
try {
  mkdirSync(join(temp,'scripts'));copyFileSync(join(root,'scripts/build.sh'),join(temp,'scripts/build.sh'));
  put('scripts/bendok.sh','bend_check() { echo check >> "$EVENTS"; }\n');
  put('scripts/perf-native-gate.sh',`#!/bin/sh
echo "gate $*" >> "$EVENTS"
if [ "$1" = --check-config ]; then exit "$CONFIG_EXIT"; fi
exit "$GATE_EXIT"
`);
  put('bin/bend',`#!/bin/sh
echo bend >> "$EVENTS"
mkdir -p dist/web
touch dist/web/index.html dist/web/chunk-test.js dist/web/test.css
`);
  put('bin/python3','#!/bin/sh\nexit 0\n');
  put('src/web/sw.js','// isolated build-path fixture\n');
  put('bin/bun',`#!/bin/sh
echo "bun $*" >> "$EVENTS"
if [ "$1" = scripts/perf-package.ts ]; then exit "$VERIFY_EXIT"; fi
mkdir -p dist
echo candidate > dist/backplane
`);
  for(const helper of ['browser','step2glb','voice','kipc'])
    put(`scripts/build-${helper}.sh`,'#!/bin/sh\necho helper >> "$EVENTS"\nexit 0\n');
  put('scripts/build-app.sh','#!/bin/sh\necho ordinary-build >> "$EVENTS"\nexit 92\n');
  const events=join(temp,'events');
  const run=(env:Record<string,string>)=>{
    rmSync(events,{force:true});rmSync(join(temp,'dist'),{recursive:true,force:true});
    const result=Bun.spawnSync(['sh','scripts/build.sh'],{cwd:temp,env:{...process.env,
      PATH:join(temp,'bin')+':'+process.env.PATH,EVENTS:events,BACKPLANE_SKIP_CHECK:'1',
      BACKPLANE_PREBUILT:'0',BACKPLANE_PERF_ENFORCE:'',BACKPLANE_VERSION:'',...env}});
    return {exit:result.exitCode,stderr:result.stderr.toString(),events:existsSync(events)?readFileSync(events,'utf8').trim().split('\n'):[]};
  };
  // Even an explicit 0 cannot bypass a versioned release. Configuration must
  // fail before any Bend, compiler, output deletion or helper invocation.
  for(const env of [
    {BACKPLANE_VERSION:'v9.9.9',BACKPLANE_PERF_ENFORCE:'0'},
    {BACKPLANE_PERF_ENFORCE:'1'},
  ]) {
    const r=run({...env,CONFIG_EXIT:'43'});
    assert.equal(r.exit,43);assert.deepEqual(r.events,['gate --check-config']);
    assert.equal(existsSync(join(temp,'dist')),false);
  }
  const failed=run({BACKPLANE_VERSION:'v9.9.9',CONFIG_EXIT:'0',GATE_EXIT:'44'});
  assert.equal(failed.exit,44,JSON.stringify(failed));
  assert(failed.events.includes('bun scripts/perf-build.ts src/app/main.bend dist/backplane'));
  assert(failed.events.includes('gate dist/backplane build/release-performance'));
  assert(!failed.events.includes('ordinary-build'));
  assert.equal(existsSync(join(temp,'dist/backplane-serve')),false,'failed gate must stop before server copy/package helpers');
  const finished=run({BACKPLANE_VERSION:'v9.9.9',CONFIG_EXIT:'0',GATE_EXIT:'0',VERIFY_EXIT:'46',BACKPLANE_PERF_FIXTURE:'test-fixture'});
  assert.equal(finished.exit,46,JSON.stringify(finished));
  assert.equal(finished.events.filter(e=>e==='helper').length,4);
  assert.equal(finished.events.at(-1),'bun scripts/perf-package.ts dist');
  const developer=run({CONFIG_EXIT:'43'});
  assert.equal(developer.exit,92);assert(developer.events.includes('ordinary-build'));
  assert(!developer.events.some(e=>e.startsWith('gate ')));
  console.log('PASS actual release/CI build path: missing config stops before build; strict failure stops promotion; version cannot opt out');
} finally {rmSync(temp,{recursive:true,force:true});}
