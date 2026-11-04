// A real TCP test of the exact production FilesHttp boundary, with only
// the actor's trusted root lookup substituted. Run exclusively on Boat.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, truncateSync, symlinkSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
const dir = mkdtempSync(join(tmpdir(), 'bp-files-http-')), root = join(dir, 'project'), home = join(dir, 'hub');
mkdirSync(root); mkdirSync(home); mkdirSync(join(root, 'docs'));
const name = 'schéma – ✓.txt';
writeFileSync(join(root, 'docs', name), 'download bytes ✓\n');
writeFileSync(join(home, 'token'), 'do not serve');
symlinkSync(home, join(root, 'escape'));
const listener = Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});
const port = listener.port; listener.stop(true);
const child = Bun.spawn(['bend', resolve(process.argv[2])], {
  env:{...process.env,FILES_TEST_ROOT:root,FILES_TEST_HOME:home,FILES_TEST_PORT:String(port)}, stdout:'pipe',stderr:'pipe'
});
let output = '', errors = '';
const collect = async (stream: ReadableStream<Uint8Array>, error=false) => {
  for await (const b of stream) { if(error) errors += new TextDecoder().decode(b); else output += new TextDecoder().decode(b); }
};
const readers = [collect(child.stdout), collect(child.stderr, true)];
const base = `http://127.0.0.1:${port}`;
const auth = { Authorization:'Bearer paired-fixture' };
const path = (route:string, p:string, thread='t') => `${base}/files/${route}?thread=${encodeURIComponent(thread)}&path=${encodeURIComponent(p)}`;
try {
  const until = Date.now()+90000;
  while(!output.includes('FILES_HTTP_READY')) {
    if(Date.now()>until || child.exitCode !== null) throw Error('fixture failed: '+output+errors);
    await Bun.sleep(50);
  }
  const request = async (route:string,p:string,headers:Record<string,string>=auth,thread='t') => fetch(path(route,p,thread),{headers,signal:AbortSignal.timeout(10000)});
  let r = await request('get','docs/'+name);
  assert.equal(r.status,200); assert.equal(await r.text(),'download bytes ✓\n');
  assert.ok(r.headers.get('content-disposition')?.includes("filename*=UTF-8''sch%C3%A9ma"));
  console.log('ok real HTTP: file bytes and unicode filename');
  r=await request('zip','docs'); assert.equal(r.status,200);
  const zip=join(dir,'docs.zip'); writeFileSync(zip,Buffer.from(await r.arrayBuffer()));
  assert.equal(spawnSync('unzip',['-tq',zip]).status,0);
  assert.equal(spawnSync('unzip',['-p',zip,'docs/'+name]).stdout.toString(),'download bytes ✓\n');
  console.log('ok real HTTP: streamed folder zip extracts exactly');
  for(const p of ['..','../hub/token','/etc/passwd','C:/Windows/win.ini','docs\\..\\x']) {
    assert.equal((await request('get',p)).status,400,p);
  }
  assert.equal((await request('get','escape/token')).status,404);
  assert.equal((await request('zip','escape')).status,404);
  console.log('ok real HTTP: traversal and symlink escapes refused');
  assert.equal((await request('get','docs/'+name,{})).status,401);
  assert.equal((await request('zip','docs',{})).status,401);
  assert.equal((await request('zip','docs',{...auth,'Sec-Fetch-Site':'cross-site'})).status,401);
  assert.equal((await request('zip','',auth,'unknown')).status,404);
  console.log('ok real HTTP: credentials and browser-origin gate');
  mkdirSync(join(root,'huge')); writeFileSync(join(root,'huge/big.bin'),''); truncateSync(join(root,'huge/big.bin'),1073741825);
  r=await request('zip','huge'); assert.equal(r.status,413); assert.match(await r.text(),/1 GiB/);
  console.log('ok real HTTP: huge folder refused before ZIP headers');
  mkdirSync(join(root,'cancel')); writeFileSync(join(root,'cancel/big.bin'),''); truncateSync(join(root,'cancel/big.bin'),512*1024*1024);
  const abort = new AbortController();
  r=await fetch(path('zip','cancel'),{headers:auth,signal:abort.signal});
  const reader=r.body!.getReader(); assert.ok(!(await reader.read()).done); abort.abort();
  try { await reader.read(); } catch {}
  r=await request('get','docs/'+name); assert.equal(r.status,200); await r.text();
  console.log('ok real HTTP: cancel a live archive; server remains responsive');
} finally {
  child.kill(); await child.exited; await Promise.all(readers); rmSync(dir,{recursive:true,force:true});
}
