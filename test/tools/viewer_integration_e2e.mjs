// Normal production hub/web integration. No mock renderer, injected App, installed
// service, agent turn or CAD save. Run only against an isolated candidate:
// bun test/tools/viewer_integration_e2e.mjs --bin BUILD/backplane-serve --web BUILD/web
// Optional: --source-root ROOT --project READONLY_PROJECT --out DIR --quality required|auto|off --gpu software|hardware --expect-material realistic --render-material realistic|cad-studio|source-defaults --expect-trace available|unavailable --expect-asset-preparation --api-only --layout-only --load-only --render-only --fidelity-only --restore-only --mask-only --coating-only --geometry-receipt FILE --keep
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import {tmpdir, homedir} from 'node:os';
import {resolve, join, dirname, basename} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash, randomBytes} from 'node:crypto';
const args = process.argv.slice(2), option = (key, fallback = '') => { const i = args.indexOf(`--${key}`);return i < 0 ? fallback : args[i + 1]; };
const gpuMode=option('gpu','software');assert.ok(['software','hardware'].includes(gpuMode),'--gpu must be software or hardware');
const expectedMaterial=option('expect-material','');assert.ok(['','realistic','cad-studio','source-defaults'].includes(expectedMaterial),'invalid expected material policy');
const renderMaterial=option('render-material','');assert.ok(['','realistic','cad-studio','source-defaults'].includes(renderMaterial),'invalid requested render policy');
const expectedTrace=option('expect-trace','available');assert.ok(['available','unavailable'].includes(expectedTrace),'invalid expected trace capability');
assert.ok(gpuMode!=='hardware'||expectedTrace==='available','hardware qualification requires available path tracing');
const browserFlags=gpuMode==='hardware'?['--enable-gpu','--use-angle=vulkan','--disable-vulkan-surface']:['--use-angle=swiftshader','--enable-unsafe-swiftshader'];
const sourceRoot = resolve(option('source-root', process.env.VIEWER_REVIEW_SOURCE_ROOT || '.'));
const project = resolve(option('project', '/home/h/Documents/i2cjak/SMU_Business_Card'));
const configPath = join(project, '.backplane.json'), config = JSON.parse(fs.readFileSync(configPath));
const canonicalProject = resolve(project, config.project), pcb = resolve(project, config.pcb || canonicalProject.replace(/\.kicad_pro$/, '.kicad_pcb'));
const sch = resolve(project, config.schematic || canonicalProject.replace(/\.kicad_pro$/, '.kicad_sch'));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const revision = path => { const s = fs.statSync(path, {bigint:true});return [s.dev,s.ino,s.size,s.mtimeNs/1000000000n,s.mtimeNs%1000000000n,s.ctimeNs/1000000000n,s.ctimeNs%1000000000n].join(':'); };
const sourceInputs = [configPath, canonicalProject, pcb, sch, canonicalProject.replace(/\.kicad_pro$/,'.kicad_prl'), pcb+'.backplane.json', sch+'.backplane.json'].filter(fs.existsSync).map(path => ({path, sha256:digest(fs.readFileSync(path)), revision:revision(path)}));
if (args.includes('--inspect')) { console.log(JSON.stringify({project, canonicalProject, pcb, sch, sourceInputs,gpuMode,expectedTrace,expectedMaterial,browserFlags, dependencies:['candidate backplane-serve','matching web/ with viewer/vendor/{viewer,bvh-worker,render-worker}.js','selected KiCad CLI','playwright-core + Chromium']}, null, 2));process.exit(0); }
const binary = resolve(option('bin')), web = resolve(option('web'));
assert.ok(option('bin') && option('web'), '--bin and --web are required');
assert.ok(!binary.startsWith(join(homedir(), '.local/share/backplane')), 'refusing installed runtime');
assert.ok(!binary.startsWith(join(homedir(), '.backplane-bend')), 'refusing installed runtime');
assert.ok(fs.existsSync(binary) && fs.existsSync(join(web, 'index.html')), 'candidate binary/web missing');
if(option('expected-bin-sha'))assert.equal(digest(fs.readFileSync(binary)),option('expected-bin-sha'),'candidate binary hash differs from qualified build');
const qualityMode = option('quality', 'auto');assert.ok(['auto','off','required'].includes(qualityMode));
const apiOnly = args.includes('--api-only');
const layoutOnly = args.includes('--layout-only');
const loadOnly = args.includes('--load-only'),renderOnly=args.includes('--render-only'),fidelityOnly=args.includes('--fidelity-only'),restoreOnly=args.includes('--restore-only'),maskOnly=args.includes('--mask-only'),coatingOnly=args.includes('--coating-only'), focused = layoutOnly || loadOnly || renderOnly || fidelityOnly || restoreOnly || maskOnly || coatingOnly;
const expectAssetPreparation=args.includes('--expect-asset-preparation'),profileLoading=loadOnly||expectAssetPreparation||fidelityOnly;
const observeWire=profileLoading||restoreOnly||maskOnly||coatingOnly;
assert.ok(!focused||!apiOnly&&qualityMode==='required','focused UI gates require --quality required and the normal UI');
assert.ok([layoutOnly,loadOnly,renderOnly,fidelityOnly,restoreOnly,maskOnly,coatingOnly].filter(Boolean).length<=1,'choose one focused UI gate');
assert.ok(!expectAssetPreparation||focused,'--expect-asset-preparation requires a focused UI gate');
assert.ok(!renderOnly||gpuMode==='hardware'&&expectedTrace==='available'&&!expectAssetPreparation,'--render-only requires hardware available tracing without the loading-profile gate');
assert.ok(!renderMaterial||renderOnly,'--render-material requires --render-only');
assert.ok(!restoreOnly||gpuMode==='hardware'&&option('expected-bin-sha')&&!expectAssetPreparation,'--restore-only requires hardware, the exact service SHA and no separate loading-profile gate');
assert.ok(!fidelityOnly||gpuMode==='hardware'&&option('geometry-receipt')&&option('expected-bin-sha'),'--fidelity-only requires hardware, the exact service SHA and the existing actual outer-copper geometry receipt');
// This mode qualifies frozen A+C through the canonical root build recipe.
// The relocated child artifact has separate provenance; no byte-equivalence claim.
const maskWorker={bytes:88289,sha256:'d02fdf911cd23399d3fa57ff452aee8d155656e1fe1532a65f49ced016762f0f'};
if(maskOnly){
  assert.ok(gpuMode==='hardware'&&option('expected-bin-sha')&&!expectAssetPreparation&&!expectedMaterial,'--mask-only requires hardware, exact service SHA, default CAD studio and no loading-profile gate');
  const path=join(web,'viewer/vendor/render-worker.js');assert.equal(fs.statSync(path).size,maskWorker.bytes);assert.equal(digest(fs.readFileSync(path)),maskWorker.sha256,'mask gate requires the canonical root A+C worker');
}
// Canonical B+B1 worker only; the older A+C mask gate retains its own pin.
// The managed run must additionally guard the new combined candidate receipt,
// complete frozen source/artifact set and canonical CAD before and after.
const coatingWorker={bytes:99802,sha256:'f0cf1d0aada5fd169b4f153d2c2e32e12af37a38838044066ce07bbc7c545865'};
if(coatingOnly){
  assert.ok(gpuMode==='hardware'&&expectedTrace==='available'&&option('expected-bin-sha')&&!expectAssetPreparation&&!expectedMaterial,'--coating-only requires hardware tracing, exact combined service SHA, default CAD studio and no loading-profile gate');
  const path=join(web,'viewer/vendor/render-worker.js');assert.equal(fs.statSync(path).size,coatingWorker.bytes);assert.equal(digest(fs.readFileSync(path)),coatingWorker.sha256,'coating gate requires the canonical B+B1 worker');
}
const geometryReceiptPath=fidelityOnly?resolve(option('geometry-receipt')):null;
const geometryReceipt=geometryReceiptPath&&JSON.parse(fs.readFileSync(geometryReceiptPath));
const geometryReceiptSha256=geometryReceiptPath&&digest(fs.readFileSync(geometryReceiptPath));
const fullGeometry=geometryReceipt?.outputs?.find(row=>basename(row.path).startsWith('board-full-outer-v1-parts-'));
if(fidelityOnly){
  assert.ok(fullGeometry&&geometryReceipt.names?.fullParts,'full outer-copper proof missing');
  assert.equal(geometryReceipt.before?.find(row=>row.path===pcb)?.sha256,digest(fs.readFileSync(pcb)),'geometry proof uses a different saved PCB');
  assert.equal(geometryReceipt.finalPlotSha256,digest(fs.readFileSync(join(sourceRoot,'src/server/plot.bend'))),'export recipe source differs from the qualified geometry proof');
  for(const flag of ['--include-tracks','--include-zones','--cut-vias-in-body'])assert.ok(fullGeometry.command.includes(flag));
  assert.ok(fullGeometry.viaBodyCheck.total>0);assert.equal(fullGeometry.viaBodyCheck.solidCentres,0);assert.equal(fullGeometry.viaBodyCheck.solidBottomCentres,0);
}
const dir = fs.mkdtempSync(join(tmpdir(), 'bp-viewer-integration-')), home = join(dir, 'home'), cache = join(dir, 'cache'), standins = join(dir, 'standins'), fixtureProject = join(dir,'project');
for (const path of [home, cache, standins, fixtureProject]) fs.mkdirSync(path, {mode:0o700});
// The only configuration written is in this synthetic root. Its explicitly
// configured canonical CAD remains the real project, used read-only.
fs.writeFileSync(join(fixtureProject,'.backplane.json'),JSON.stringify({project:canonicalProject})+'\n');
const output = resolve(option('out', join(dir, 'evidence')));fs.mkdirSync(output, {recursive:true});
const token = randomBytes(16).toString('hex'), at = Math.floor(Date.now() / 1000);
const events = [{$:'ProjectCreated', id:'p1', title:'Readonly SMU viewer fixture', root:fixtureProject, at},
  ...['t1','t2'].map((id, i) => ({$:'ThreadCreated', id, project:'p1', title:`Viewer acceptance ${i + 1}`, env:'local', branch:'', worktree:'', provider:'claude', model:'', at:at + i + 1})),
  ...[['restart.continue','off'],['onboard.done','1'],['onboard.uber','1'],['telemetry.enabled','off']].map(([key,value]) => ({$:'SettingSet', key, value, at:at + 3}))];
fs.writeFileSync(join(home,'events.jsonl'), events.map(x => JSON.stringify(x)).join('\n') + '\n');
fs.writeFileSync(join(home,'token'), token, {mode:0o600});
for (const name of ['claude','codex','grok','muse','opencode','pi','omp','agy']) fs.writeFileSync(join(standins,name), '#!/bin/sh\nexit 127\n', {mode:0o755});
const stepHelper=option('step2glb','/home/h/.local/share/backplane/backplane-step2glb');
if(fs.existsSync(stepHelper))fs.symlinkSync(stepHelper,join(standins,'backplane-step2glb'));
const lease = Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const port = lease.port;lease.stop(true);
assert.ok(port !== 3787 && port !== 3788, 'live/dev listener port refused');
const origin = `http://127.0.0.1:${port}`, facts = [], began = performance.now();
const record = (name, data = {}) => { facts.push({name, elapsedMs:Math.round(performance.now()-began), ...data});console.log(`ok ${name}`); };
const runtime = option('kicad-cli', '/home/h/.backplane-bend/kicad/10.99.0-bp.4/bin/kicad-cli');
assert.ok(fs.existsSync(runtime), 'selected KiCad CLI missing');
const proc = Bun.spawn([binary,'--home',home,'--host','127.0.0.1','--port',String(port),'--web',web,'--no-tailscale','--headless','--foreground'], {
  env:{...process.env, DISPLAY:'', WAYLAND_DISPLAY:'', BACKPLANE_HOME:home, XDG_CACHE_HOME:cache, BACKPLANE_AGENTS:standins,
    BACKPLANE_KICAD_CLI:runtime, PATH:standins+':'+process.env.PATH, BACKPLANE_NO_UPDATE:'1', BACKPLANE_TELEMETRY:'off', BACKPLANE_PEERS:'', BACKPLANE_DEPLOY_LOCKS:join(dir,'unused-locks')},
  stdout:Bun.file(join(output,'hub.stdout')), stderr:Bun.file(join(output,'hub.stderr')),
});
const {cbor, plain} = await import(pathToFileURL(join(sourceRoot,'src/web/wire.js')).href);
const {ViewerRpc} = await import(pathToFileURL(join(sourceRoot,'src/web/viewer-rpc.js')).href);
const dictionary = name => { const text = fs.readFileSync(join(sourceRoot,'src/core/cbor.bend'),'utf8'), at = text.indexOf(`def Cbor.${name}()`);return JSON.parse(text.slice(text.indexOf('[',at),text.indexOf(']',at)+1)); };
const keys = dictionary('keys'), words = dictionary('words');
function encode(value) {
  const head = (major,n) => n < 24 ? [major*32+n] : n < 256 ? [major*32+24,n] : n < 65536 ? [major*32+25,n>>8,n&255] : [major*32+26,n>>>24&255,n>>>16&255,n>>>8&255,n&255];
  if (value === null) return [246];if (typeof value === 'boolean') return [value?245:244];
  // Backplane deliberately carries numbers above U32 as tag 7 decimal text.
  if (typeof value === 'number') { assert.ok(Number.isSafeInteger(value)&&value>=0);return value<=0xffffffff?head(0,value):[199,...encode(String(value))]; }
  if (typeof value === 'string') { const bytes = [...new TextEncoder().encode(value)];return [...head(3,bytes.length),...bytes]; }
  if (Array.isArray(value)) return [...head(4,value.length),...value.flatMap(encode)];
  const entries = Object.entries(value);return [...head(5,entries.length),...entries.flatMap(([key,v]) => [...encode(key),...encode(v)])];
}
for(const id of [101,0xffffffff,2**46+1,2**47+1])assert.equal(plain(cbor(Uint8Array.from(encode({id})),keys,words)).id,id,'test transport must preserve production RPC IDs');
let socket, browser, page, viewerRpc, gpuEvidence, requestID = 100, metadata, asset, onBrowserRequest;
const pending = new Map(), errors = [], network = [], workerURLs = [], http = [], componentPages = [], browserRpc = [];
async function ready() {
  const until = performance.now() + 30000;
  for (;;) {
    if (proc.exitCode !== null) throw Error(`candidate exited ${proc.exitCode}`);
    try { if ((await fetch(origin+'/hello',{signal:AbortSignal.timeout(1000)})).ok) return; } catch {}
    if (performance.now() >= until) throw Error('candidate listener did not become ready');
    await new Promise(resolve => setTimeout(resolve,50));
  }
}
function rpc(method, params, timeoutMs = 180000) {
  const id = ++requestID;
  return new Promise((resolve,reject) => {
    const timer = setTimeout(() => { pending.delete(String(id));reject(Error(`${method} timed out`)); }, timeoutMs);
    pending.set(String(id), {resolve,reject,timer});socket.send(Uint8Array.from(encode({id,m:method,p:params})));
  });
}
async function stream(url, headers, savePath=null) {
  const attempt={url,authenticated:!!headers.Cookie};http.push(attempt);
  const start = performance.now();let response;
  try { response = await fetch(new URL(url,origin), {headers, signal:AbortSignal.timeout(30000)}); }
  catch(error) { attempt.error=String(error);throw error; }
  attempt.status=response.status;
  const hash = createHash('sha256'), reader = response.body.getReader(), prefix = [], body=[];let bytes=0, chunks=0, firstByteMs=null;
  for (;;) { const item=await reader.read();if(item.done)break;if(firstByteMs===null)firstByteMs=performance.now()-start;hash.update(item.value);bytes+=item.value.length;chunks++;if(prefix.length<20)prefix.push(...item.value.subarray(0,20-prefix.length));assert.ok(bytes<=67108864,'asset stream exceeded cap');if(savePath)body.push(Buffer.from(item.value)); }
  if(savePath)fs.writeFileSync(savePath,Buffer.concat(body));
  return {status:response.status, headers:Object.fromEntries(response.headers), bytes, chunks, firstByteMs, ms:performance.now()-start, sha256:hash.digest('hex'), prefix};
}
const proxy = {'Tailscale-User-Login':'untrusted-fixture'}, auth = {...proxy, Cookie:`bp_token=${token}`};
// Inspect the delivered binary independently of Three/the production loader.
// Accessor fingerprints use the existing geometry receipt's byte convention,
// allowing its actual via-centre proof to follow identical substrate geometry.
function inspectFullOriginal(bytes,proof){
  assert.equal(bytes.toString('ascii',0,4),'glTF');assert.equal(bytes.readUInt32LE(4),2);assert.equal(bytes.readUInt32LE(8),bytes.length);
  let json,bin;
  for(let offset=12;offset<bytes.length;){
    const length=bytes.readUInt32LE(offset),type=bytes.readUInt32LE(offset+4);offset+=8;
    assert.ok(offset+length<=bytes.length,'truncated original GLB chunk');
    if(type===0x4e4f534a){assert.ok(!json);json=JSON.parse(bytes.subarray(offset,offset+length).toString('utf8'));}
    if(type===0x004e4942){assert.ok(!bin);bin=bytes.subarray(offset,offset+length);}
    offset+=length;
  }
  assert.ok(json&&bin);assert.equal(json.buffers.length,1);assert.ok(!json.buffers[0].uri);
  const widths={SCALAR:1,VEC2:2,VEC3:3,VEC4:4},sizes={5121:1,5123:2,5125:4,5126:4};
  const meshes=[];let triangles=0,vertices=0,normals=0,primitiveMeshes=0;
  for(const mesh of json.meshes){
    const hash=createHash('sha256');let meshTriangles=0;
    for(const primitive of mesh.primitives){
      assert.equal(primitive.mode??4,4,'expected actual triangle geometry');
      const position=json.accessors[primitive.attributes.POSITION],normal=json.accessors[primitive.attributes.NORMAL];
      assert.ok(position&&normal);vertices+=position.count;normals+=normal.count;
      const count=primitive.indices===undefined?position.count:json.accessors[primitive.indices].count;
      assert.equal(count%3,0);triangles+=count/3;meshTriangles+=count/3;primitiveMeshes++;
      for(const id of [...Object.values(primitive.attributes),primitive.indices].filter(id=>id!==undefined)){
        const accessor=json.accessors[id],view=json.bufferViews[accessor.bufferView],width=widths[accessor.type]*sizes[accessor.componentType];
        assert.ok(view&&width&&!accessor.sparse);assert.equal(view.buffer??0,0);
        const start=(view.byteOffset||0)+(accessor.byteOffset||0),stride=view.byteStride||width;
        assert.ok(stride>=width&&Number.isSafeInteger(accessor.count)&&accessor.count>0&&start>=0&&start+(accessor.count-1)*stride+width<=bin.length,'invalid original accessor bounds');
        hash.update(JSON.stringify([accessor.count,accessor.componentType,accessor.type]));
        if(stride===width)hash.update(bin.subarray(start,start+accessor.count*width));
        else for(let i=0;i<accessor.count;i++)hash.update(bin.subarray(start+i*stride,start+i*stride+width));
      }
    }
    meshes.push({name:mesh.name||'',triangles:meshTriangles,sha256:hash.digest('hex')});
  }
  const actual={bytes:bytes.length,sha256:digest(bytes),triangles,vertices,normals,meshCount:json.meshes.length,primitiveMeshes,materials:json.materials.length,meshes};
  for(const field of ['bytes','triangles','vertices','normals','meshCount','primitiveMeshes','materials'])assert.equal(actual[field],proof[field],`delivered full geometry ${field} differs from actual proof`);
  assert.deepEqual(meshes,proof.meshes,'delivered source geometry fingerprints differ from the via/copper proof');
  for(const suffix of ['_copper','_via','_PCB'])assert.ok(meshes.some(mesh=>mesh.name.endsWith(suffix)&&mesh.triangles>0));
  const substrates=new Set(json.meshes.flatMap((mesh,i)=>mesh.name?.endsWith('_PCB')?[i]:[]));
  for(const node of json.nodes.filter(node=>substrates.has(node.mesh)))assert.ok(!node.matrix&&!node.translation&&!node.rotation&&!node.scale,'qualified via substrate placement changed');
  return actual;
}
let fidelityInventory;
const exportCachePath=fullGeometry&&join(cache,'backplane-bend/models',basename(geometryReceipt.names.fullParts)),publishedCacheDir=join(cache,'backplane-bend/render-assets');
const observeCaches=()=>({xdgCacheHome:cache,exportCache:{path:exportCachePath,present:!!exportCachePath&&fs.existsSync(exportCachePath),entries:fs.existsSync(join(cache,'backplane-bend/models'))?fs.readdirSync(join(cache,'backplane-bend/models')).sort():[]},publishedCache:{path:publishedCacheDir,present:fs.existsSync(publishedCacheDir),entries:fs.existsSync(publishedCacheDir)?fs.readdirSync(publishedCacheDir).sort():[]}});
try {
  await ready();record('isolated loopback hub ready', {port, binarySha256:digest(fs.readFileSync(binary))});
  socket = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}&view=1&mobile=1`);socket.binaryType='arraybuffer';
  viewerRpc=new ViewerRpc(request=>socket.send(Uint8Array.from(encode(request))),{timeoutMs:120000});
  socket.onmessage = event => {
    try {
      const frame=typeof event.data==='string'?JSON.parse(event.data):plain(cbor(new Uint8Array(event.data),keys,words));
      if(frame.result?.components?.page)componentPages.push({page:frame.result.components.page,bytes:new TextEncoder().encode(JSON.stringify(frame.result)).length,sourceRevision:frame.result.components.sourceRevision,schematicRevision:frame.result.components.schematicRevision});
      if(viewerRpc.receive(frame))return;
      if(frame.t!=='reply')return;const call=pending.get(String(frame.id));if(!call)return;
      pending.delete(String(frame.id));clearTimeout(call.timer);call.resolve(frame);
    } catch(error) { errors.push(`control decode: ${error.message}`); }
  };
  await new Promise((resolve,reject) => { socket.onopen=resolve;socket.onerror=reject; });
  if(fidelityOnly)record('independent export and private publication cache state before the first RPC',observeCaches());
  let metadataError;
  try { metadata=await viewerRpc.components('t1'); } catch(error) { metadataError=error.message; }
  if(metadataError)record('component metadata blocked', {error:metadataError});
  else {
  assert.ok(metadata?.components?.length,'saved component metadata is empty');
  assert.equal(resolve(metadata.source),pcb);assert.equal(metadata.sourceRevision,revision(pcb));
  assert.equal(metadata.schematicRevision,revision(sch));
  if(!focused){const redirected = await rpc('kicad.components',{thread:'t1',path:join(dir,'untrusted.kicad_pcb')});assert.equal(redirected.result?.components?.source,metadata.source);}
  assert.equal(new Set(metadata.components.map(c=>c.id)).size,metadata.components.length);
  if(metadata.page){assert.equal(metadata.page.complete,true);assert.equal(metadata.page.total,metadata.components.length);assert.ok(componentPages.every(p=>p.bytes<220000));}
  if(!focused){const stalePage=await rpc('kicad.components',{thread:'t1',offset:32,sourceRevision:'stale-revision',schematicRevision:metadata.schematicRevision});assert.ok(stalePage.ok===false||stalePage.result?.error||stalePage.result?.components?.error,'stale page revision was accepted');}
  record('actual saved component metadata and revisions', {count:metadata.components.length, source:metadata.source, revision:metadata.sourceRevision,pages:componentPages});
  }
  const sourceRevision=revision(pcb);
  if(focused){
    const cacheBefore=fidelityOnly?observeCaches():null;
    const exportAt=performance.now();asset=await viewerRpc.renderAsset('t1',pcb,'2',sourceRevision);
    assert.equal(asset.source,pcb);assert.equal(asset.revision,sourceRevision);
    const prepareMs=performance.now()-exportAt;
    record('API original export warmed before cold browser context',{ms:prepareMs,bytes:asset.bytes,sha256:asset.sha256,sourceRevision,...(fidelityOnly?{cacheBefore,cacheAfter:observeCaches(),cacheScope:cacheBefore.exportCache.present?'existing export observed; publication/RPC duration is not cold KiCad export timing':'isolated export file absent before RPC; full export plus publication/RPC duration',durationScope:'RPC includes authorization, cache lookup, export if absent and immutable publication; not isolated CLI CPU time'}:{})});
    if(fidelityOnly){
      const actualPath=join(output,'delivered-full-original.glb'),valid=await stream(asset.url,auth,actualPath);
      assert.equal(valid.status,200);assert.equal(valid.sha256,asset.sha256);assert.equal(valid.bytes,asset.bytes);
      assert.equal(valid.headers['content-type'],'model/gltf-binary');assert.equal(Number(valid.headers['content-length']),asset.bytes);assert.match(valid.headers['cache-control'],/private.*immutable/);
      fidelityInventory=inspectFullOriginal(fs.readFileSync(actualPath),fullGeometry);
      assert.ok(fs.existsSync(exportCachePath),'new full recipe cache file not observed');assert.equal(digest(fs.readFileSync(exportCachePath)),asset.sha256);
      const published=join(publishedCacheDir,asset.sha256+'.glb'),before={export:revision(exportCachePath),published:revision(published)};
      const warmAt=performance.now(),warm=await viewerRpc.renderAsset('t1',pcb,'2',sourceRevision);
      assert.equal(warm.sha256,asset.sha256);assert.deepEqual({export:revision(exportCachePath),published:revision(published)},before,'warm request replaced immutable export/publication');
      record('new-service authenticated full original GLB and warm namespace reuse',{prepareMs,warmRpcMs:performance.now()-warmAt,stream:valid,cacheAfter:observeCaches(),inventory:fidelityInventory,geometryReceipt:{path:geometryReceiptPath,sha256:geometryReceiptSha256,viaBodyCheck:fullGeometry.viaBodyCheck},scope:'delivered geometry fingerprints bind the existing via top/bottom centre proof; mesh labels alone do not prove holes'});
    }
  }
  if(!focused){
  const unknown = await rpc('kicad.components',{thread:'unknown'});assert.equal(unknown.ok,false);
  const wrong = await rpc('kicad.renderAsset',{thread:'t1',path:pcb,kind:'2',revision:'stale-revision'});assert.ok(wrong.ok===false||wrong.result?.error);
  const outside = join(dir,'outside.glb');fs.writeFileSync(outside,Buffer.alloc(24));
  const boundary = await rpc('kicad.renderAsset',{thread:'t1',path:outside,kind:'mech',revision:revision(outside)});assert.ok(boundary.ok===false||boundary.result?.error,'outside project asset was authorized');
  const exportStart=performance.now(), original = await rpc('kicad.renderAsset',{thread:'t1',path:pcb,kind:'2',revision:sourceRevision});asset=original.result?.asset;
  assert.ok(original.ok!==false && asset,JSON.stringify(original).slice(0,700));assert.equal(asset.source,pcb);assert.equal(asset.revision,sourceRevision);
  assert.match(asset.sha256,/^[0-9a-f]{64}$/);assert.equal(asset.url,`/viewer-asset?id=${asset.sha256}`);assert.ok(asset.bytes>20&&asset.bytes<=67108864);
  record('actual cold original GLB export and authorization', {ms:performance.now()-exportStart, bytes:asset.bytes, sha256:asset.sha256, orientation:asset.orientation});
  assert.equal((await stream(asset.url,proxy)).status,401,'proxy request inherited loopback trust');
  record('actual asset HTTP rejects unauthenticated proxy request');
  const valid=await stream(asset.url,auth);assert.equal(valid.status,200);assert.equal(valid.bytes,asset.bytes);assert.equal(valid.sha256,asset.sha256);
  assert.equal(Number(valid.headers['content-length']),asset.bytes);assert.equal(valid.headers['content-type'],'model/gltf-binary');assert.equal(valid.headers['x-content-type-options'],'nosniff');assert.match(valid.headers['cache-control'],/private.*immutable/);
  assert.deepEqual(valid.prefix.slice(0,8),[103,108,84,70,2,0,0,0]);
  for (const id of ['../token','A'.repeat(64),'0'.repeat(63),'f'.repeat(64),asset.sha256+'.glb']) assert.equal((await stream('/viewer-asset?id='+encodeURIComponent(id),auth)).status,404,`bad asset id ${id}`);
  const irrelevant=await stream(asset.url+'&path='+encodeURIComponent('/etc/passwd'),auth);assert.equal(irrelevant.sha256,asset.sha256,'client path redirected asset');
  const snapshot=join(cache,'backplane-bend/render-assets',asset.sha256+'.glb'), before=revision(snapshot);
  const warmStart=performance.now(), warm=await rpc('kicad.renderAsset',{thread:'t1',path:pcb,kind:'2',revision:sourceRevision});assert.equal(warm.result?.asset?.sha256,asset.sha256);assert.equal(revision(snapshot),before);
  record('exact-revision warm asset RPC', {ms:performance.now()-warmStart});
  record('actual authenticated binary HTTP stream, hash, bad IDs and immutable warm cache', valid);
  }

  if(!apiOnly) {
  assert.ok(!metadataError,`normal component UI blocked: ${metadataError}`);
  const pw=option('playwright',process.env.PLAYWRIGHT_CORE_PATH||join(homedir(),'Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs'));
  const {chromium}=await import(pathToFileURL(resolve(pw)).href);
  const chrome=option('chrome',process.env.CHROME_BIN||'/usr/bin/google-chrome-stable');
  browser=await chromium.launch({headless:true,executablePath:chrome,args:browserFlags,env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:''}});
  const context=await browser.newContext({viewport:{width:1360,height:900},hasTouch:true,reducedMotion:'reduce'});
  context.on('response',response=>network.push({url:response.url(),status:response.status()}));
  page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));page.on('worker',worker=>workerURLs.push(worker.url()));
  page.on('websocket',connection=>{
    const calls=new Map();
    const decode=payload=>typeof payload==='string'?JSON.parse(payload):plain(cbor(new Uint8Array(payload),keys,words));
    connection.on('framesent',({payload})=>{try{
      const request=decode(payload);
      if(!['kicad.components','kicad.renderAsset'].includes(request?.m))return;
      calls.set(request.id,request.m);
      browserRpc.push({direction:'request',at:performance.now()-began,id:request.id,method:request.m,thread:request.p?.thread,offset:request.p?.offset,sourceRevision:request.p?.sourceRevision,revision:request.p?.revision,pending:calls.size});
      onBrowserRequest?.(request);
    }catch{}});
    connection.on('framereceived',({payload})=>{try{
      const reply=decode(payload),method=calls.get(reply?.id);if(!method||reply.t!=='reply')return;
      calls.delete(reply.id);
      browserRpc.push({direction:'reply',at:performance.now()-began,id:reply.id,method,ok:reply.ok,error:reply.error||reply.result?.error,page:reply.result?.components?.page,pending:calls.size});
    }catch{}});
  });
  if(observeWire)await page.addInitScript({content:`(()=>{${fs.readFileSync(join(sourceRoot,'src/web/wire.js'),'utf8').replace(/^export /gm,'')}window.__viewerDecodeFrame=(data)=>typeof data==='string'?JSON.parse(data):plain(cbor(new Uint8Array(data),${JSON.stringify(keys)},${JSON.stringify(words)}));})();`});
  await page.addInitScript(({profileLoad,observeRestore,observeMask}) => {
    localStorage.setItem('backplane-selected','t1');
    const receipt=window.__viewerAcceptance={draws:{}, matrices:{}, events:0, blankIntervals:[], viewClicks:[],workerStatus:[], workerBounds:[], workerLoads:[], workerCameras:[],workerSelections:[],workerMaterials:[],workerComponents:[],workerSolder:[],workerModes:[],workerTerminations:0,workerLifecycle:[],wireFrames:[],timeOrigin:performance.timeOrigin};
    if(observeMask)Object.assign(receipt,{maskStatus:[],workerAcks:[]});
    Object.assign(receipt,{workerSerial:0,socketSerial:0,activeSocketGeneration:0,socketEvents:[],workerTeardowns:[],threadClicks:[],cachedPaint:[]});
    if(profileLoad){
      const NativeSocket=window.WebSocket;
      window.WebSocket=class extends NativeSocket{
        constructor(...args){super(...args);this.acceptanceGeneration=++receipt.socketSerial;
          this.acceptanceViewerSocket=new URL(args[0],location.href).pathname==='/ws';
          if(this.acceptanceViewerSocket)receipt.activeSocketGeneration=this.acceptanceGeneration;
          receipt.socketEvents.push({event:'construct',generation:this.acceptanceGeneration,viewer:this.acceptanceViewerSocket,at:performance.now()});
          this.addEventListener('open',()=>receipt.socketEvents.push({event:'open',generation:this.acceptanceGeneration,viewer:this.acceptanceViewerSocket,at:performance.now()}));
          this.addEventListener('close',()=>receipt.socketEvents.push({event:'close',generation:this.acceptanceGeneration,viewer:this.acceptanceViewerSocket,at:performance.now()}));
          this.addEventListener('message',event=>receipt.wireFrames.push({direction:'receive',generation:this.acceptanceGeneration,at:performance.now(),data:event.data}));
        }
        send(data){receipt.wireFrames.push({direction:'send',generation:this.acceptanceGeneration,at:performance.now(),data});return super.send(data);}
      };
    }
    if(observeRestore){
      let previous='';
      const sample=()=>{
        const root=document.querySelector('#solid-workbench'),canvas=root?.querySelector('#solid'),worker=window.__viewerAcceptanceWorker;
        const value={cached:document.documentElement?.dataset.backplaneCached==='true',thread:document.querySelector('main[data-thread]')?.dataset.thread,scope:root?.dataset.scope,key:root?.dataset.key,originalVisible:root?.classList.contains('solid-original-visible')===true,canvasConnected:!!canvas?.isConnected,workerInstance:worker?.acceptanceInstance,workerScope:worker?.acceptanceScope};
        const token=JSON.stringify(value);if(token!==previous){previous=token;receipt.cachedPaint.push({at:performance.now(),...value});}
      };
      new MutationObserver(sample).observe(document,{childList:true,subtree:true,attributes:true,attributeFilter:['data-backplane-cached','data-thread','data-scope','data-key','class']});
      document.addEventListener('click',event=>{const row=event.target?.closest?.('[data-on-click="select"]');if(row)receipt.threadClicks.push({at:performance.now(),thread:row.dataset.value});},{capture:true});
    }
    document.addEventListener('click',event=>{if(event.target?.closest?.('[data-on-click="view-kind"][data-value="3d"]'))receipt.viewClicks.push({at:performance.now(),kind:'3d'});},{capture:true});
    // Observe the normal worker protocol without replacing renderer behavior.
    const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker {
      constructor(url,options){
        const constructAt=performance.now();
        super(url,options);this.acceptanceRenderer=String(url).includes('render-worker');this.acceptanceCalls=new Map();
        if(this.acceptanceRenderer){this.acceptanceInstance=++receipt.workerSerial;this.acceptanceScope=document.querySelector('#solid-workbench')?.dataset.scope;this.acceptanceGeneration=receipt.activeSocketGeneration;window.__viewerAcceptanceWorker=this;receipt.workerLifecycle.push({event:'constructor',at:constructAt,returnedAt:performance.now(),url:String(url),instance:this.acceptanceInstance,scope:this.acceptanceScope,generation:this.acceptanceGeneration});}
        if(this.acceptanceRenderer)this.addEventListener('message',({data})=>{
          if(data.status&&observeMask)receipt.maskStatus.push({...data.status,at:performance.now(),instance:this.acceptanceInstance});
          if(data.status){receipt.workerStatus.push({...data.status,at:performance.now(),observedWorkerInstance:this.acceptanceInstance,observedThreadScope:this.acceptanceScope,observedConnectionGeneration:this.acceptanceGeneration});if(receipt.workerStatus.length>200)receipt.workerStatus.shift();}
          const call=this.acceptanceCalls.get(data.id);if(!call)return;this.acceptanceCalls.delete(data.id);
          if(observeMask)receipt.workerAcks.push({method:call.method,id:data.id,postAt:call.at,at:performance.now(),instance:this.acceptanceInstance,result:data.result,error:data.error});
          if(call.method==='init'||call.method==='load')receipt.workerLifecycle.push({event:'reply',method:call.method,id:data.id,at:performance.now(),result:data.result,error:data.error});
          if(call.method==='bounds')receipt.workerBounds.push({id:call.args.id,result:data.result,error:data.error});
          if(call.method==='material-policy')receipt.workerMaterials.push({policy:call.args.policy,result:data.result,error:data.error});
          if(call.method==='components')receipt.workerComponents.push({source:call.args.source,revision:call.args.sourceRevision,result:data.result,error:data.error});
          if(call.method==='solder-preview')receipt.workerSolder.push({enabled:call.args.enabled,result:data.result,error:data.error});
        });
      }
      postMessage(message,...rest){
        if(this.acceptanceRenderer){
          const {method,args,id}=message;
          if(method==='init'||method==='load')receipt.workerLifecycle.push({event:'post',method,id,at:performance.now()});
          if(id)this.acceptanceCalls.set(id,{method,at:performance.now(),args:method==='bounds'?{id:args.id}:method==='material-policy'?{policy:args.policy}:method==='components'?{source:args?.source,sourceRevision:args?.sourceRevision}:method==='solder-preview'?{enabled:args.enabled}:null});
          if(method==='load'){this.acceptanceKey=args.key;receipt.workerLoads.push({key:args.key,revision:args.revision,source:args.source,sha256:args.sha256,url:args.url,at:performance.now(),instance:this.acceptanceInstance,scope:this.acceptanceScope,generation:this.acceptanceGeneration});}
          if(observeMask&&method==='mode')receipt.workerModes.push({at:performance.now(),instance:this.acceptanceInstance,scope:this.acceptanceScope,mode:args.mode,options:args.options});
          if(method==='camera'){receipt.workerCameras.push({key:this.acceptanceKey,state:args,...(observeMask?{at:performance.now(),instance:this.acceptanceInstance}:{})});if(receipt.workerCameras.length>40)receipt.workerCameras.shift();}
          if(method==='selection')receipt.workerSelections.push({key:this.acceptanceKey,id:args.id});
        }
        return super.postMessage(message,...rest);
      }
      terminate(){if(this.acceptanceRenderer){receipt.workerTerminations++;receipt.workerTeardowns.push({at:performance.now(),instance:this.acceptanceInstance,scope:this.acceptanceScope});}return super.terminate();}
    };
    const loadedCanvases=new WeakSet();
    const locations=new WeakMap();
    for(const proto of [WebGLRenderingContext.prototype,typeof WebGL2RenderingContext==='function'&&WebGL2RenderingContext.prototype].filter(Boolean)) {
      const location=proto.getUniformLocation, matrix=proto.uniformMatrix4fv, draw=proto.drawArrays;
      proto.getUniformLocation=function(program,name){const at=location.call(this,program,name);if(at)locations.set(at,name);return at;};
      proto.uniformMatrix4fv=function(at,transpose,value){if(this.canvas.id==='solid'&&locations.get(at)==='m')receipt.matrices[this.canvas.dataset.scope]=Array.from(value);return matrix.call(this,at,transpose,value);};
      proto.drawArrays=function(mode,first,count){const result=draw.call(this,mode,first,count);if(this.canvas.id==='solid'&&count>0){const k=this.canvas.dataset.scope;receipt.draws[k]=(receipt.draws[k]||0)+1;loadedCanvases.add(this.canvas);}return result;};
    }
    document.addEventListener('solid-camera-change',()=>receipt.events++);
    let blankAt=null;
    setInterval(()=>{const c=document.querySelector('#solid');if(!c||c.getBoundingClientRect().width<1){blankAt=null;return;}const scope=c.dataset.scope;
      const note=c.parentElement.querySelector('.viewer-note')?.textContent||'', stat=c.parentElement.querySelector('.viewer-stat')?.textContent||'';
      if(!loadedCanvases.has(c)&&!note&&!stat){blankAt??=performance.now();if(performance.now()-blankAt>500)receipt.blankIntervals.push({scope,ms:performance.now()-blankAt});}else blankAt=null;
    },100);
  },{profileLoad:observeWire,observeRestore:restoreOnly||coatingOnly,observeMask:maskOnly||coatingOnly});
  await page.goto(origin+'/#token='+token,{waitUntil:'domcontentloaded'});
  // A separate 4 px context proves the requested backend with a real shader
  // readback. These pixels never stand in for saved CAD or the viewer frame.
  const gpuProbe=await page.evaluate(()=>{
    const canvas=document.createElement('canvas');canvas.width=4;canvas.height=4;
    const gl=canvas.getContext('webgl2');if(!gl)return {webgl2:false};
    const debug=gl.getExtension('WEBGL_debug_renderer_info'),vertex=gl.createShader(gl.VERTEX_SHADER),fragment=gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(vertex,'#version 300 es\nvoid main(){vec2 p=gl_VertexID==0?vec2(-1.,-1.):gl_VertexID==1?vec2(3.,-1.):vec2(-1.,3.);gl_Position=vec4(p,0.,1.);}');
    gl.shaderSource(fragment,'#version 300 es\nprecision highp float;out vec4 color;void main(){color=vec4(.125,.5,.875,1.);}');
    gl.compileShader(vertex);gl.compileShader(fragment);const program=gl.createProgram();gl.attachShader(program,vertex);gl.attachShader(program,fragment);gl.linkProgram(program);gl.useProgram(program);gl.drawArrays(gl.TRIANGLES,0,3);
    const pixels=new Uint8Array(4);gl.readPixels(1,1,1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixels);
    const result={webgl2:true,unmasked:!!debug,vendor:gl.getParameter(debug?debug.UNMASKED_VENDOR_WEBGL:gl.VENDOR),renderer:gl.getParameter(debug?debug.UNMASKED_RENDERER_WEBGL:gl.RENDERER),pixels:[...pixels],error:gl.getError(),floatTargets:!!gl.getExtension('EXT_color_buffer_float'),linked:gl.getProgramParameter(program,gl.LINK_STATUS)};
    gl.deleteProgram(program);gl.deleteShader(vertex);gl.deleteShader(fragment);gl.getExtension('WEBGL_lose_context')?.loseContext();return result;
  });
  gpuEvidence={mode:gpuMode,flags:browserFlags,browser:browser.version(),probe:gpuProbe,scope:'separate backend capability readback; actual viewer geometry is verified independently'};
  assert.ok(gpuProbe.webgl2&&gpuProbe.unmasked&&gpuProbe.linked&&gpuProbe.floatTargets,'requested GPU context unavailable');assert.equal(gpuProbe.error,0);
  assert.ok(gpuProbe.pixels.every((v,i)=>Math.abs(v-[32,128,223,255][i])<=1),'GPU shader readback differs');
  const isSoftware=/swiftshader|llvmpipe|software/i.test(gpuProbe.renderer);
  assert.equal(isSoftware,gpuMode==='software',`requested ${gpuMode}, actual renderer ${gpuProbe.renderer}`);
  if(gpuMode==='hardware')assert.match(gpuProbe.renderer,/RTX\s*5080/i,'this machine hardware gate requires the qualified RTX 5080');
  record('actual requested browser GPU backend and shader readback verified',gpuEvidence);
  await page.locator('[data-on-click="select"][data-value="t1"]').first().waitFor({timeout:60000});
  await page.locator('[data-on-click="select"][data-value="t1"]').first().click();
  // main's workspace menu repeats the bar's buttons while collapsed (hidden);
  // a person clicks the visible one.
  const choose3d=()=>page.locator('.workspace-bar [data-on-click="view-kind"][data-value="3d"]:visible').first().click();
  await choose3d();
  await page.waitForFunction(()=>window.__viewerAcceptance.draws.t1>0&&document.querySelector('#solid-workbench')?.dataset.scope==='t1'&&document.querySelector('#solid')?.parentElement.querySelector('.viewer-stat')?.textContent,{},{timeout:180000});
  assert.ok((await context.cookies(origin)).some(c=>c.name==='bp_token'&&c.httpOnly));
  const bar=page.locator('.solid-toolbar'),renderMode=bar.getByLabel('Rendering',{exact:true});
  let workerProbeID=-2000000;
  const probeOriginal=(method,args={})=>page.evaluate(({id,method,args})=>new Promise(resolve=>{
    const worker=window.__viewerAcceptanceWorker;
    const timer=setTimeout(()=>{worker.removeEventListener('message',reply);resolve({error:`actual worker ${method} probe timed out`});},5000);
    const reply=({data})=>{if(data.id!==id)return;clearTimeout(timer);worker.removeEventListener('message',reply);resolve(data.error?{error:data.error}:data.result);};
    worker.addEventListener('message',reply);worker.postMessage({id,method,args});
  }),{id:workerProbeID--,method,args});
  const waitAlignment=async()=>{
    const exactBodies=await renderMode.inputValue()!=='fast';
    const text=exactBodies?'Saved revisions match. Model bodies use exact component references; unmatched bodies are not selectable.':'Saved PCB and assembly revisions match. Canvas selection identifies footprint geometry.';
    await page.locator('.solid-inspector > .solid-explanation').filter({hasText:text}).waitFor({timeout:60000});
  };
  await renderMode.waitFor();assert.equal(await renderMode.inputValue(),'studio','normal UI must start in Studio');
  const waitOriginal=async phase=>{
    await page.waitForFunction(({phase,revision})=>{
      const root=document.querySelector('#solid-workbench'),states=window.__viewerAcceptance.workerStatus;
      return root?.classList.contains('solid-original-visible')&&states.some(s=>s.phase===phase&&s.visible&&s.key===root.dataset.key&&s.revision===revision);
    },{phase,revision:sourceRevision},{timeout:180000});
    assert.ok(!/failed|timed out|Preview available/i.test(await page.locator('.solid-render-status').textContent()));
  };
  if(qualityMode==='required'){
    await waitOriginal('raster');assert.match(await page.locator('.solid-render-status').textContent(),/Studio.*original model/i);
    gpuEvidence.workerContext=await page.evaluate(()=>new Promise(resolve=>{
      const worker=window.__viewerAcceptanceWorker,id=-1000001;
      const timer=setTimeout(()=>{worker.removeEventListener('message',reply);resolve({error:'actual worker context probe timed out'});},5000);
      const reply=({data})=>{if(data.id!==id)return;clearTimeout(timer);worker.removeEventListener('message',reply);resolve(data.error?{error:data.error}:data.result);};
      worker.addEventListener('message',reply);worker.postMessage({id,method:'status',args:{}});
    }));
    assert.ok(gpuEvidence.workerContext?.worker&&!gpuEvidence.workerContext?.error,'actual original-model worker context probe failed');
    assert.equal(gpuEvidence.workerContext.pathTraceAvailable,expectedTrace==='available','actual worker trace capability differs from requested gate');
    assert.equal(gpuEvidence.workerContext.traceAllocated,false,'Studio must not allocate path tracing');
    assert.equal(gpuEvidence.workerContext.bvhBuilding,false,'Studio must not build ray acceleration');
    assert.equal(gpuEvidence.workerContext.bvhReady,false,'Studio must not prebuild ray acceleration');
    gpuEvidence.workerIdentityScope='actual original-model worker GL identity, including nullable unmasked extension fields; separate shader probe is independently labeled';
    if(gpuMode==='hardware'){
      assert.match(gpuEvidence.workerContext.unmaskedRenderer||'',/RTX\s*5080/i,'actual original-model worker must prove RTX 5080, not a masked generic identity');
      assert.match(gpuEvidence.workerContext.unmaskedVendor||'',/NVIDIA/i);
      assert.ok(!/swiftshader|llvmpipe|software/i.test(gpuEvidence.workerContext.unmaskedRenderer),'original-model worker unexpectedly reports software rendering');
    }
    if(expectedMaterial){assert.equal(await bar.getByLabel('Material finish',{exact:true}).inputValue(),expectedMaterial,'initial material selection differs');assert.equal(gpuEvidence.workerContext.materialPolicy,expectedMaterial,'initial actual worker material policy differs');}
    record('normal UI automatic Studio first visible frame matches saved revision',await page.evaluate(()=>({status:window.__viewerAcceptance.workerStatus.at(-1),load:window.__viewerAcceptance.workerLoads.at(-1)})));
  }
  let initialLoadingProfile;
  async function captureLoadingProfile(){
    // All wire decoding happens after firstFrame. Arrival timestamps were
    // captured before the application's message handler, on the page clock.
    const observations=await page.evaluate(async revision=>{
      const r=window.__viewerAcceptance,key=document.querySelector('#solid-workbench').dataset.key,wire=[];
      const calls=new Map();
      for(const frame of r.wireFrames){
        const data=frame.data instanceof Blob?await frame.data.arrayBuffer():frame.data;
        const decoded=window.__viewerDecodeFrame(data);if(!decoded)continue;
        if(frame.direction==='send'&&['kicad.renderAsset','kicad.components'].includes(decoded.m)){
          calls.set(decoded.id,decoded.m);wire.push({direction:'request',at:frame.at,id:decoded.id,method:decoded.m,thread:decoded.p?.thread,path:decoded.p?.path,kind:decoded.p?.kind,revision:decoded.p?.revision,offset:decoded.p?.offset});
        }else if(frame.direction==='receive'&&decoded.t==='plotrevision'){
          wire.push({direction:'revision',at:frame.at,key:decoded.key,revision:decoded.revision});
        }else if(frame.direction==='receive'&&decoded.t==='reply'&&calls.has(decoded.id)){
          const asset=decoded.result?.asset;wire.push({direction:'reply',at:frame.at,id:decoded.id,method:calls.get(decoded.id),ok:decoded.ok,error:decoded.error||decoded.result?.error,asset:asset&&{source:asset.source,revision:asset.revision,sha256:asset.sha256},page:decoded.result?.components?.page});
        }
      }
      const resources=performance.getEntriesByType('resource').filter(e=>/vendor|render-worker|chunk-/.test(new URL(e.name).pathname)).map(e=>({...e.toJSON(),name:new URL(e.name).pathname}));
      return {mainTimeOrigin:r.timeOrigin,click:r.viewClicks[0],wire,workerLifecycle:r.workerLifecycle,workerLoads:r.workerLoads,load:r.workerLoads.find(s=>s.key===key&&s.revision===revision),bytes:r.workerStatus.find(s=>s.key===key&&s.revision===revision&&s.reason==='Parsing original model'),firstFrame:r.workerStatus.find(s=>s.key===key&&s.revision===revision&&s.visible&&s.firstFrame),mainResources:resources};
    },sourceRevision);
    const workerResources=[];
    for(const worker of page.workers().filter(w=>new URL(w.url()).pathname.endsWith('/render-worker.js'))){
      const timing=await worker.evaluate(()=>({timeOrigin:performance.timeOrigin,now:performance.now(),resources:performance.getEntriesByType('resource').map(e=>({...e.toJSON(),name:new URL(e.name).pathname,assetURL:new URL(e.name).pathname==='/viewer-asset'?e.name:undefined,assetSha256:new URL(e.name).pathname==='/viewer-asset'?new URL(e.name).searchParams.get('id'):undefined}))}));
      workerResources.push({url:new URL(worker.url()).pathname,...timing,resources:timing.resources.map(e=>({...e,mainStartAt:timing.timeOrigin-observations.mainTimeOrigin+e.startTime,mainResponseEndAt:timing.timeOrigin-observations.mainTimeOrigin+e.responseEnd}))});
    }
    fs.writeFileSync(join(output,'loading-observations.json'),JSON.stringify({observations,workerResources},null,2)+'\n');
    const {click,load,firstFrame,wire,workerLifecycle}=observations;
    // Progress status messages are throttled. Resource responseEnd proves
    // network body completion even when the parsing status was not emitted;
    // it does not claim arrayBuffer/hash completion or the start of parsing.
    const assetId=load&&new URL(load.url).searchParams.get('id');
    const assetResource=workerResources.flatMap(w=>w.resources).find(e=>e.name==='/viewer-asset'&&e.assetURL===load.url&&e.assetSha256===assetId&&e.mainStartAt>=load?.at&&e.responseStatus===200);
    const bytes=observations.bytes?{...observations.bytes,measurement:'main receipt of worker parsing status'}:assetResource&&{at:assetResource.mainResponseEndAt,key:load.key,revision:load.revision,source:load.source,measurement:'worker resource responseEnd: network body complete; parse boundary unobserved'};
    assert.ok(click&&load&&firstFrame,'missing initial frame milestones');
    const revision=wire.find(s=>s.direction==='revision'&&s.key===load.key&&s.revision===sourceRevision&&s.at>=click.at);
    const pcbRevision=wire.find(s=>s.direction==='revision'&&s.key==='0'+load.key.slice(1)&&s.revision===sourceRevision&&s.at>=click.at);
    const request=wire.find(s=>s.direction==='request'&&s.method==='kicad.renderAsset'&&s.revision===sourceRevision&&s.at>=click.at);
    const reply=wire.find(s=>s.direction==='reply'&&s.id===request?.id);
    const constructor=workerLifecycle.find(s=>s.event==='constructor'&&s.at>=click.at);
    const initPost=workerLifecycle.find(s=>s.event==='post'&&s.method==='init'&&s.at>=constructor?.at);
    const initReply=workerLifecycle.find(s=>s.event==='reply'&&s.method==='init'&&s.id===initPost?.id);
    const vendor=workerResources.flatMap(w=>w.resources).find(e=>e.name.endsWith('/viewer/vendor/viewer.js'));
    assert.ok(revision&&request&&reply&&constructor&&initPost&&initReply&&vendor,'missing revision/RPC/worker/vendor loading evidence');
    assert.equal(reply.asset?.sha256,assetId);assert.equal(reply.ok,true);assert.equal(reply.asset?.revision,sourceRevision);assert.equal(reply.asset?.source,pcb);assert.ok(!initReply.error);
    assert.ok(click.at<=revision.at&&request.at<=reply.at&&reply.at<=constructor.at&&constructor.at<=initPost.at&&initPost.at<=initReply.at&&initReply.at<=load.at&&load.at<=firstFrame.at&&(!bytes||load.at<=bytes.at&&bytes.at<=firstFrame.at),'loading stages out of order');
    if(expectAssetPreparation||fidelityOnly){
      assert.ok(pcbRevision&&pcbRevision.at<=request.at&&request.at<revision.at,'authenticated asset preparation must start on the matching PCB revision before independent 3D confirmation');
      assert.equal(String(request.kind),'2');assert.equal(request.path,load.key.split('|').slice(2).join('|'));
    }else assert.ok(revision.at<=request.at,'sequential asset request preceded its 3D revision');
    for(const actualLoad of observations.workerLoads.filter(s=>s.at>=click.at)){
      assert.ok(actualLoad.at>=revision.at,'original Worker.load preceded independent 3D revision');
      assert.equal(actualLoad.source,pcb);assert.equal(actualLoad.revision,sourceRevision);assert.equal(actualLoad.key,revision.key);
    }
    assert.equal(firstFrame.source,pcb);assert.equal(firstFrame.revision,sourceRevision);assert.equal(firstFrame.key,revision.key);
    const milestones={click:click.at,pcbRevision:pcbRevision?.at,plotRevision:revision.at,assetRequest:request.at,assetReply:reply.at,workerConstructor:constructor.at,workerConstructorReturn:constructor.returnedAt,initPost:initPost.at,vendorStart:vendor.mainStartAt,vendorResponseEnd:vendor.mainResponseEndAt,initReply:initReply.at,loadPost:load.at,bytes:bytes?.at??null,firstFrame:firstFrame.at};
    const intervals={clickToRevisionMs:revision.at-click.at,revisionToAssetRequestMs:request.at-revision.at,assetRpcMs:reply.at-request.at,replyToWorkerConstructorMs:constructor.at-reply.at,constructorToInitPostMs:initPost.at-constructor.at,workerInitMs:initReply.at-initPost.at,vendorFetchMs:vendor.duration,vendorResponseEndToInitReplyMs:initReply.at-vendor.mainResponseEndAt,initReplyToLoadPostMs:load.at-initReply.at,modelLoadPostToBytesMs:bytes?bytes.at-load.at:null,bytesToFirstFrameMs:bytes?firstFrame.at-bytes.at:null,clickToFrameMs:firstFrame.at-click.at};
    if(expectAssetPreparation||fidelityOnly)Object.assign(intervals,{preparationLeadBefore3DRevisionMs:revision.at-request.at,assetReplyRelativeTo3DRevisionMs:reply.at-revision.at});
    const profile={scope:'one warm-export/cold-browser loading observation; no authority or production changes',expectAssetPreparation:expectAssetPreparation||fidelityOnly,bytesMeasurement:bytes?.measurement||'unmeasured',assetResourceId:assetId,bytes,clock:'main performance.now; worker resource timeOrigin converted onto main clock; wire decoded after firstFrame',limits:'vendor fetch is directly measured; responseEnd to init reply includes module evaluation, renderer setup and GPU initialization and does not isolate parse CPU time; preparation reply may follow 3D confirmation and no speed threshold is imposed',source:pcb,sourceRevision,milestones,intervals,observations,workerResources};
    fs.writeFileSync(join(output,'loading-profile.json'),JSON.stringify(profile,null,2)+'\n');
    record('focused actual revision/RPC/worker/vendor/original-frame loading profile',{milestones,intervals});
    assert.deepEqual(errors,[]);
    return profile;
  }
  if(profileLoading)initialLoadingProfile=await captureLoadingProfile();
  async function checkCachedRestore(prepared){
    let {key,selected,bounds,saved}=prepared||{};
    if(!prepared){
      key=await page.locator('#solid-workbench').getAttribute('data-key');
      await bar.getByRole('button',{name:'Components',exact:true}).click();await waitAlignment();
      selected=metadata.components.find(c=>c.reference==='U1');assert.ok(selected);
      const initialMatrix=await page.evaluate(()=>window.__viewerAcceptance.matrices.t1);
      await page.locator('.component-bom').getByRole('button',{name:'U1',exact:true}).click();
      await page.waitForFunction(id=>window.__viewerAcceptance.workerSelections.some(s=>s.id===id),selected.id);
      bounds=await probeOriginal('bounds',{id:selected.id});assert.equal(bounds?.id,selected.id);assert.equal(bounds.source,pcb);assert.equal(bounds.revision,sourceRevision);
      await bar.getByRole('button',{name:'Frame part',exact:true}).click();
      await page.waitForFunction(before=>window.__viewerAcceptance.matrices.t1?.some((v,i)=>Math.abs(v-before[i])>1e-5),initialMatrix);
      await page.locator('#solid').focus();await page.keyboard.press('ArrowLeft');
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      saved=await page.evaluate(()=>({camera:window.__viewerAcceptance.workerCameras.at(-1)?.state,matrix:window.__viewerAcceptance.matrices.t1,instance:window.__viewerAcceptanceWorker.acceptanceInstance}));
      assert.equal(saved.camera.selection.id,selected.id);assert.equal(saved.camera.selection.sourceRevision,sourceRevision);
    }
    await page.locator('[data-on-click="select"][data-value="t2"]').first().click();await choose3d();
    await page.waitForFunction(initial=>document.querySelector('#solid-workbench')?.dataset.scope==='t2'&&window.__viewerAcceptanceWorker?.acceptanceScope==='t2'&&window.__viewerAcceptanceWorker.acceptanceInstance!==initial,saved.instance);
    const departed=await page.evaluate(()=>window.__viewerAcceptanceWorker.acceptanceInstance);
    await page.locator('[data-on-click="select"][data-value="t1"]').first().click();
    await page.waitForFunction(({initial,departed,key,revision,sha256})=>{
      const root=document.querySelector('#solid-workbench'),worker=window.__viewerAcceptanceWorker,r=window.__viewerAcceptance;
      return document.documentElement.dataset.backplaneCached==='false'&&document.querySelector('main[data-thread]')?.dataset.thread==='t1'&&root?.dataset.scope==='t1'&&root.classList.contains('solid-original-visible')&&worker?.acceptanceScope==='t1'&&![initial,departed].includes(worker.acceptanceInstance)&&r.workerStatus.some(s=>s.observedWorkerInstance===worker.acceptanceInstance&&s.observedThreadScope==='t1'&&s.key===key&&s.revision===revision&&s.sha256===sha256&&s.visible&&s.firstFrame);
    },{initial:saved.instance,departed,key,revision:sourceRevision,sha256:asset.sha256},{timeout:180000});
    await waitAlignment();const restored=await probeOriginal('status');
    assert.equal(restored.source,pcb);assert.equal(restored.revision,sourceRevision);assert.equal(restored.key,key);assert.equal(restored.sha256,asset.sha256);
    assert.equal(restored.visible,true);assert.equal(restored.firstFrame,true);assert.equal(restored.traceAllocated,false);
    assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
    assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds);
    await page.waitForFunction(expected=>window.__viewerAcceptance.matrices.t1?.every((v,i)=>Math.abs(v-expected[i])<1e-6),saved.matrix);
    assert.deepEqual(await page.evaluate(coating=>{const r=window.__viewerAcceptance,w=window.__viewerAcceptanceWorker;return (coating?r.workerCameras.findLast(c=>c.instance===w.acceptanceInstance):r.workerCameras.at(-1))?.state;},coatingOnly),saved.camera,'cached paint restore changed source selection or the nondefault camera');
    // Observe actual traffic without delaying it. Decode only after the current
    // restored worker has proved its own frame, keeping the load path untouched.
    const observations=await page.evaluate(async()=>{
      const r=window.__viewerAcceptance,revisions=[];
      for(const frame of r.wireFrames){if(frame.direction!=='receive')continue;
        const data=frame.data instanceof Blob?await frame.data.arrayBuffer():frame.data,decoded=window.__viewerDecodeFrame(data);
        if(decoded?.t==='plotrevision'&&decoded.key?.startsWith('2|'))revisions.push({at:frame.at,generation:frame.generation,key:decoded.key,revision:decoded.revision});
      }
      const current=window.__viewerAcceptanceWorker;
      return {cachedPaint:r.cachedPaint,threadClicks:r.threadClicks,socketEvents:r.socketEvents,revisions,loads:r.workerLoads,statuses:r.workerStatus,constructors:r.workerLifecycle.filter(e=>e.event==='constructor'),teardowns:r.workerTeardowns,current:{instance:current.acceptanceInstance,scope:current.acceptanceScope,generation:current.acceptanceGeneration}};
    });
    const returnClick=observations.threadClicks.findLast(c=>c.thread==='t1');assert.ok(returnClick);
    const firstFrame=observations.statuses.find(s=>s.observedWorkerInstance===observations.current.instance&&s.observedThreadScope==='t1'&&s.at>=returnClick.at&&s.visible&&s.firstFrame&&s.key===key&&s.revision===sourceRevision&&s.sha256===asset.sha256);
    assert.ok(firstFrame,'an old worker status cannot prove the restored worker frame');
    if(coatingOnly){assert.equal(firstFrame.source,pcb);assert.equal(firstFrame.observedConnectionGeneration,observations.current.generation);assert.ok(observations.loads.some(l=>l.instance===observations.current.instance&&l.scope==='t1'&&l.generation===observations.current.generation),'current restored worker load/scope/generation was not observed');}
    const events=[...observations.socketEvents.filter(e=>e.viewer).map(e=>({...e,type:'socket'})),...observations.revisions.map(e=>({...e,type:'revision'}))].sort((a,b)=>a.at-b.at);
    const ledger=new Map(),authorizedLoads=[];let eventAt=0,currentGeneration=0;
    for(const load of observations.loads){
      while(eventAt<events.length&&events[eventAt].at<=load.at){
        const e=events[eventAt++];
        if(e.type==='socket'&&e.event==='construct'){ledger.clear();currentGeneration=e.generation;ledger.set(e.generation,new Map());}
        else if(e.type==='socket'&&e.event==='close'){ledger.delete(e.generation);if(currentGeneration===e.generation)currentGeneration=0;}
        else if(e.type==='revision'&&ledger.has(e.generation))ledger.get(e.generation).set(e.key,e);
      }
      assert.equal(load.generation,currentGeneration,'load reused a retired connection revision');
      const confirmation=ledger.get(load.generation)?.get(load.key);
      assert.ok(confirmation&&confirmation.revision===load.revision,'original load lacks matching independent 3D confirmation in this connection generation');
      assert.equal(load.key,key);assert.equal(load.source,pcb);assert.equal(load.revision,sourceRevision);assert.equal(load.sha256,asset.sha256);assert.equal(new URL(load.url).searchParams.get('id'),asset.sha256);
      authorizedLoads.push({instance:load.instance,scope:load.scope,loadAt:load.at,confirmationAt:confirmation.at,generation:load.generation,key:load.key,revision:load.revision});
    }
    for(const instance of [saved.instance,departed])assert.ok(observations.teardowns.some(t=>t.instance===instance),'departed worker was not terminated');
    const cachedPhase=observations.cachedPaint.find(s=>s.at>=returnClick.at&&s.at<=firstFrame.at&&s.cached&&s.thread==='t1'&&s.scope==='t1'&&s.key===key&&s.canvasConnected);
    const evidence={scope:'one natural painted-thread restore; no artificial transport delay or authority-negative suite',coverage:cachedPhase?'exercised':'unexercised: cached phase not observed',saved,departed,restored,bounds,selection:selected.id,cachedPhase:cachedPhase||null,currentFirstFrame:firstFrame,returnClick,authorizedLoads,observations};
    fs.writeFileSync(join(output,'cached-viewer-restore.json'),JSON.stringify(evidence,null,2)+'\n');
    assert.deepEqual(errors,[]);assert.ok(network.every(x=>!x.url.includes(token)));
    assert.ok(cachedPhase,'UNEXERCISED: actual cached painted t1 viewer phase was not observed; do not claim restore coverage');
    await page.locator('.solid-pathtrace').screenshot({path:join(output,'cached-viewer-restored.png')});
    record('actual cached painted thread restore retains independently confirmed source and current-worker frame',{cachedPhase,currentFirstFrame:firstFrame,authorizedLoads,selection:selected.id,bounds,restored,scope:'prior same-connection independent 3D revision reuse allowed; no fresh packet required per remount, no missing-authority counterfactual claim'});
    return evidence;
  }
  if(coatingOnly){
    const key=await page.locator('#solid-workbench').getAttribute('data-key'),stages=[];
    const owner=()=>page.evaluate(()=>{const w=window.__viewerAcceptanceWorker;return {instance:w.acceptanceInstance,scope:w.acceptanceScope,generation:w.acceptanceGeneration};});
    const pose=()=>page.evaluate(()=>{const r=window.__viewerAcceptance,w=window.__viewerAcceptanceWorker;return {camera:r.workerCameras.findLast(c=>c.instance===w.acceptanceInstance)?.state,matrix:r.matrices.t1,instance:w.acceptanceInstance};});
    const initialOwner=await owner(),initial=await probeOriginal('status');
    const identify=status=>{
      assert.ok(status&&!status.error,JSON.stringify(status));assert.equal(status.worker,true);assert.equal(status.key,key);assert.equal(status.source,pcb);assert.equal(status.revision,sourceRevision);assert.equal(status.sha256,asset.sha256);
      for(const [field,value]of Object.entries({triangles:311551,vertices:351351,authoredNormals:351351,sourceMeshes:22357,materials:20}))assert.equal(status[field],value,'original source '+field+' changed');
      assert.deepEqual(status.materialRoleCounts,initial.materialRoleCounts);assert.equal(Object.values(status.materialRoleCounts).reduce((a,b)=>a+b,0),20);
      assert.equal(status.visible,true);assert.equal(status.firstFrame,true);assert.equal(status.pathTraceAvailable,true);assert.match(status.unmaskedRenderer||'',/RTX\s*5080/i);
    };
    const raster=status=>{identify(status);assert.equal(status.phase,'raster');assert.equal(status.traceAllocated,false);assert.equal(status.bvhBuilding,false);assert.equal(status.bvhReady,false);assert.equal(status.samples,0);};
    const evidence=async()=>{
      const observations=await page.evaluate(()=>{const r=window.__viewerAcceptance;return {statuses:r.maskStatus,acks:r.workerAcks,modes:r.workerModes,loads:r.workerLoads,cameras:r.workerCameras,solder:r.workerSolder,teardowns:r.workerTeardowns};});
      fs.writeFileSync(join(output,'coating-lifecycle.json'),JSON.stringify({scope:'normal toolbar32 coating trace with illustrative solder, Fast/Studio and one natural cached restore; functional only, no appearance acceptance or speed threshold',coatingWorker,stages,observations},null,2)+'\n');
    };
    const checkpoint=async(label,status,extra={})=>{stages.push({label,at:await page.evaluate(()=>performance.now()),owner:await owner(),status,...extra});await evidence();};
    const frameOf=instance=>page.evaluate(({instance,key,source,revision,sha256})=>{
      const r=window.__viewerAcceptance,w=window.__viewerAcceptanceWorker;
      return r.maskStatus.find(s=>s.instance===instance&&instance===w.acceptanceInstance&&s.key===key&&s.source===source&&s.revision===revision&&s.sha256===sha256&&s.visible&&s.firstFrame);
    },{instance,key,source:pcb,revision:sourceRevision,sha256:asset.sha256});
    raster(initial);assert.equal(initialOwner.scope,'t1');assert.ok(initialOwner.generation>0);assert.equal(initial.materialPolicy,'cad-studio');assert.equal(initial.illustrativeSolder,false);assert.equal(initial.maskReliefMaps,0);
    const originalFrame=await frameOf(initialOwner.instance);assert.ok(originalFrame,'current original Studio frame is required before derived coating');await checkpoint('original Studio frame',initial,{originalFrame});
    await bar.getByRole('button',{name:'Components',exact:true}).click();await waitAlignment();
    const selected=metadata.components.find(c=>c.reference==='U1');assert.equal(selected?.id,'a249b1bf-7fe7-4abb-a884-9e32cc3761c3');
    await page.locator('.component-bom').getByRole('button',{name:'U1',exact:true}).click();
    const bounds=await probeOriginal('bounds',{id:selected.id});assert.equal(bounds?.id,selected.id);assert.equal(bounds.source,pcb);assert.equal(bounds.revision,sourceRevision);assert.ok(bounds.min.every((v,i)=>Number.isFinite(v)&&v<=bounds.max[i]));
    await bar.getByRole('button',{name:'Fit',exact:true}).click();await page.locator('#solid').focus();await page.keyboard.press('ArrowLeft');
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    assert.ok(!(await probeOriginal('interactive',{on:false}))?.error);const saved=await pose();
    assert.equal(saved.camera.selection.id,selected.id);assert.equal(saved.camera.selection.sourceRevision,sourceRevision);assert.ok(saved.matrix?.length===16&&saved.matrix.every(Number.isFinite));
    const preserved=async label=>{
      assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds,label+': original bounds changed');
      assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
      const current=await pose();assert.deepEqual(current.camera,saved.camera,label+': source camera/selection changed');assert.ok(current.matrix?.every((v,i)=>Math.abs(v-saved.matrix[i])<1e-6),label+': camera matrix changed');
    };
    const menu=bar.locator('.solid-camera'),finish=bar.getByLabel('Material finish',{exact:true}),solder=bar.getByLabel('Illustrative solder',{exact:true});
    await menu.locator(':scope > summary').click();await finish.waitFor({state:'visible'});
    assert.equal(await finish.locator('option[value="realistic"]').evaluate(o=>o.disabled),false);await finish.selectOption('realistic');
    await page.waitForFunction(instance=>window.__viewerAcceptance.workerAcks.some(a=>a.instance===instance&&a.method==='material-policy'&&a.result?.materialPolicy==='realistic'),initialOwner.instance);
    // Do not wait for the solder-OFF relief: ON must replace that epoch before
    // the ready representation that the normal trace request captures.
    const beforeSolder=await probeOriginal('status'),solderAt=await page.evaluate(()=>performance.now()),solderCount=await page.evaluate(()=>window.__viewerAcceptance.workerSolder.length);
    assert.equal(beforeSolder.illustrativeSolder,false);assert.equal(await solder.isEnabled(),true);await solder.check();
    await page.waitForFunction(count=>window.__viewerAcceptance.workerSolder.length>count,solderCount);
    const solderReply=await page.evaluate(()=>window.__viewerAcceptance.workerSolder.at(-1));
    assert.ok(solderReply.result&&!solderReply.error,JSON.stringify(solderReply));assert.equal(solderReply.enabled,true);assert.equal(solderReply.result.enabled,true);assert.equal(solderReply.result.source,pcb);assert.equal(solderReply.result.revision,sourceRevision);
    assert.equal(solderReply.result.coverage.joints,116);assert.equal(solderReply.result.coverage.components,58);assert.equal(solderReply.result.coverage.sourceCandidates.source,pcb);assert.equal(solderReply.result.coverage.sourceCandidates.revision,sourceRevision);
    await menu.locator(':scope > summary').click();await checkpoint('normal UI solder ON',await probeOriginal('status'),{solderAt,solderReply});
    let active,deadline=performance.now()+60000;
    do{
      active=await probeOriginal('status');
      if(active?.error||active.maskReliefState==='fallback'||active.rasterBatchState==='fallback'||active.materialPolicy==='realistic'&&active.illustrativeSolder&&active.maskReliefState==='ready'&&active.rasterBatchState==='active'&&active.rasterBatchEpoch>beforeSolder.rasterBatchEpoch)break;
      await page.waitForTimeout(150);
    }while(performance.now()<deadline);
    await checkpoint('solder-owned ready coating',active);assert.deepEqual(await owner(),initialOwner);raster(active);
    assert.equal(active.materialPolicy,'realistic');assert.equal(active.illustrativeSolder,true);assert.equal(active.solderJoints,116);assert.ok(active.rasterBatchEpoch>beforeSolder.rasterBatchEpoch);
    assert.equal(active.rasterBatchState,'active',active.rasterBatchReason);assert.equal(active.rasterBatchMultiDraw,true);assert.ok(active.rasterBatchBytes>0);
    assert.equal(active.maskReliefState,'ready',active.maskReliefReason);assert.equal(active.maskReliefMaps,2);assert.equal(active.maskReliefEstimated,true);assert.ok(active.maskReliefBytes>0);assert.equal(active.maskReliefBytes,active.maskReliefStats?.ownedBytes);assert.equal(active.maskReliefStats.sourceSHA256,asset.sha256);await preserved('ready coating');
    record('normal UI solder ON owns fresh ready coating before trace',{originalFrame,owner:initialOwner,solderReply,status:active,selection:selected.id,bounds});
    assert.equal(await renderMode.locator('option[value="pathtrace"]').evaluate(o=>o.disabled),false);
    const traceAt=await page.evaluate(()=>performance.now());await renderMode.selectOption('pathtrace');
    const samples=bar.getByLabel('Path tracing sample limit',{exact:true});await samples.waitFor({state:'visible'});
    const options=await samples.locator('option').evaluateAll(os=>os.map(o=>Number(o.value)));assert.ok(options.includes(32)&&options.every(n=>Number.isInteger(n)&&n>=32));await samples.selectOption('32');assert.equal(await samples.inputValue(),'32');
    await checkpoint('normal toolbar trace32 requested',null,{traceAt});
    await page.waitForFunction(({instance,traceAt})=>window.__viewerAcceptance.maskStatus.some(s=>s.instance===instance&&s.at>=traceAt&&(['converged','paused','error','unsupported'].includes(s.phase)||s.tracePresentation==='original-source'||!!s.denoiseGuideReason)),{instance:initialOwner.instance,traceAt},{timeout:180000});
    const traced=await probeOriginal('status');await checkpoint('trace terminal status',traced);assert.deepEqual(await owner(),initialOwner);identify(traced);
    assert.equal(traced.phase,'converged',traced.denoiseGuideReason||traced.reason);assert.ok(Number.isFinite(traced.samples)&&traced.samples>=32);assert.equal(traced.denoised,true);assert.equal(traced.denoisePasses,3);assert.equal(traced.denoiseGuideReason,'');
    assert.equal(traced.traceAllocated,true);assert.equal(traced.bvhReady,true);assert.equal(traced.bvhBuilding,false);assert.equal(traced.tracePresentation,'estimated-mask-relief');assert.match(traced.tracePresentationReason,/Captured ready estimated coating/);
    assert.equal(traced.materialPolicy,'realistic');assert.equal(traced.illustrativeSolder,true);assert.equal(traced.solderJoints,116);assert.equal(traced.maskReliefState,'ready');assert.equal(traced.maskReliefEstimated,true);assert.equal(traced.maskReliefMaps,2);assert.equal(traced.maskReliefBytes,active.maskReliefBytes);
    assert.equal(traced.shadingGuideMappedSurfaces,2);assert.equal(traced.shadingGuidePasses,2);assert.ok(traced.shadingGuideBytes>0);assert.equal(traced.coatingTraceTextureLayers,2);assert.ok(traced.coatingTraceTextureBytes>0);
    assert.equal(traced.hdrResolve,'pixel-area');assert.equal(traced.resolveMaxTexelsPerPixel,9);assert.equal(traced.resolvePending,false);assert.equal(traced.resolvePendingReason,'');
    for(const field of ['traceTargetWidth','traceTargetHeight','guideWidth','guideHeight','resolveInputWidth','resolveInputHeight','resolveWidth','resolveHeight','renderWidth','renderHeight'])assert.ok(Number.isSafeInteger(traced[field])&&traced[field]>0,'invalid actual '+field);
    assert.equal(traced.guideWidth,traced.traceTargetWidth);assert.equal(traced.guideHeight,traced.traceTargetHeight);assert.equal(traced.shadingGuideBytes,traced.guideWidth*traced.guideHeight*8);
    assert.equal(traced.resolveInputWidth,traced.traceTargetWidth);assert.equal(traced.resolveInputHeight,traced.traceTargetHeight);assert.equal(traced.resolveWidth,traced.renderWidth);assert.equal(traced.resolveHeight,traced.renderHeight);assert.ok(traced.resolveBytes>0);await preserved('converged coating trace');
    const traceFrame=join(output,'coating-trace32-converged.png');await page.locator('.solid-pathtrace').screenshot({path:traceFrame});
    const milestones=await page.evaluate(({instance,traceAt,solderAt})=>{
      const r=window.__viewerAcceptance,seen=[...r.maskStatus.filter(s=>s.instance===instance).map(s=>({at:s.at,status:s})),...r.workerAcks.filter(a=>a.instance===instance&&a.result?.maskReliefState).map(a=>({at:a.at,status:a.result}))].sort((a,b)=>a.at-b.at);
      return {building:seen.find(s=>s.at>=solderAt&&s.status.illustrativeSolder&&s.status.maskReliefState==='building')||null,ready:seen.find(s=>s.at>=solderAt&&s.status.illustrativeSolder&&s.status.maskReliefState==='ready')||null,converged:seen.find(s=>s.at>=traceAt&&s.status.phase==='converged')||null,modes:r.workerModes.filter(m=>m.instance===instance&&m.at>=traceAt)};
    },{instance:initialOwner.instance,traceAt,solderAt});
    assert.ok(milestones.ready&&milestones.ready.at>=originalFrame.at&&milestones.ready.at<=traceAt);assert.ok(milestones.converged);assert.ok(milestones.modes.some(m=>m.mode==='pathtrace'&&m.options?.samples===32));assert.ok(milestones.modes.every(m=>m.mode!=='pathtrace'||m.options?.samples>=32));
    await checkpoint('completed coating frame',traced,{milestones,firstFrameToReadyObservedMs:milestones.ready.at-originalFrame.at,traceRequestToConvergedObservedMs:milestones.converged.at-traceAt,deriveReportedMs:active.maskReliefStats.milliseconds,traceFrame:{path:traceFrame,sha256:digest(fs.readFileSync(traceFrame))},limits:'Intervals include intentional UI actions; no derivation-start, isolated performance or appearance claim. All trace pixels have solder ON.'});
    record('normal toolbar32 completes captured coating trace with owned shading guide',{status:traced,milestones,traceFrame});
    await renderMode.selectOption('fast');await page.waitForFunction(instance=>!document.querySelector('.solid-pathtrace')&&window.__viewerAcceptance.workerTeardowns.some(t=>t.instance===instance),initialOwner.instance);
    assert.equal(await page.locator('#solid-workbench').evaluate(r=>r.classList.contains('solid-original-visible')),false);await checkpoint('Fast retires traced worker',null,{retiredInstance:initialOwner.instance});
    await renderMode.selectOption('studio');
    await page.waitForFunction(({previous,key,source,revision,sha256})=>{const r=window.__viewerAcceptance,w=window.__viewerAcceptanceWorker,root=document.querySelector('#solid-workbench');return w?.acceptanceInstance!==previous&&w?.acceptanceScope==='t1'&&root?.classList.contains('solid-original-visible')&&r.maskStatus.some(s=>s.instance===w.acceptanceInstance&&s.key===key&&s.source===source&&s.revision===revision&&s.sha256===sha256&&s.visible&&s.firstFrame);},{previous:initialOwner.instance,key,source:pcb,revision:sourceRevision,sha256:asset.sha256},{timeout:180000});
    const reopenedOwner=await owner(),reopened=await probeOriginal('status');await checkpoint('fresh Studio original frame',reopened,{originalFrame:await frameOf(reopenedOwner.instance)});raster(reopened);assert.equal(reopened.materialPolicy,'realistic');await waitAlignment();await preserved('Fast Studio');
    // Illustrative solder belongs to the retired renderer; reopening starts
    // from the original source. Do not silently regenerate joints for this step.
    assert.equal(reopened.illustrativeSolder,false);assert.equal(reopened.solderJoints,0);
    const restored=await checkCachedRestore({key,selected,bounds,saved:{...saved,instance:reopenedOwner.instance}});raster(restored.restored);assert.deepEqual(restored.restored.materialRoleCounts,initial.materialRoleCounts);
    await checkpoint('natural chat cached restore',restored.restored,{cachedPhase:restored.cachedPhase,currentFirstFrame:restored.currentFirstFrame,authorizedLoads:restored.authorizedLoads});
    assert.equal(await renderMode.inputValue(),'studio');assert.deepEqual(errors,[]);assert.ok(network.every(x=>!x.url.includes(token)));
    record('coating trace Fast Studio and natural cached restoration keep original authority',{retired:initialOwner,reopened:reopenedOwner,currentFirstFrame:restored.currentFirstFrame,selection:selected.id,bounds});
  }else if(maskOnly){
    const key=await page.locator('#solid-workbench').getAttribute('data-key'),rows=[],captures=[],stages=[];
    const currentInstance=()=>page.evaluate(()=>window.__viewerAcceptanceWorker.acceptanceInstance);
    const camera=()=>page.evaluate(()=>{const r=window.__viewerAcceptance,w=window.__viewerAcceptanceWorker;return {camera:r.workerCameras.findLast(c=>c.instance===w.acceptanceInstance)?.state,matrix:r.matrices.t1,instance:w.acceptanceInstance};});
    const firstInstance=await currentInstance(),initial=await probeOriginal('status');
    const identify=s=>{
      assert.ok(s&&!s.error,JSON.stringify(s));assert.equal(s.worker,true);assert.equal(s.source,pcb);assert.equal(s.key,key);assert.equal(s.revision,sourceRevision);assert.equal(s.sha256,asset.sha256);
      for(const [field,value]of Object.entries({triangles:311551,vertices:351351,authoredNormals:351351,sourceMeshes:22357,materials:20}))assert.equal(s[field],value,'original source '+field+' changed');
      assert.deepEqual(s.materialRoleCounts,initial.materialRoleCounts);assert.equal(Object.values(s.materialRoleCounts).reduce((a,b)=>a+b,0),20);
      assert.equal(s.lighting,'neutral-z-up-softkey-fill-rim-lower-diffuser');assert.equal(s.traceAllocated,false);assert.equal(s.bvhBuilding,false);assert.equal(s.bvhReady,false);assert.equal(s.samples,0);assert.match(s.unmaskedRenderer||'',/RTX\s*5080/i);
      assert.equal(s.visible,true);assert.equal(s.firstFrame,true);
    };
    const writeEvidence=async()=>{
      const observations=await page.evaluate(()=>{const r=window.__viewerAcceptance;return {statuses:r.maskStatus,acks:r.workerAcks,loads:r.workerLoads,teardowns:r.workerTeardowns,cameras:r.workerCameras};});
      fs.writeFileSync(join(output,'mask-lifecycle.json'),JSON.stringify({scope:'original-source normal UI mask relief raster lifecycle; full frames only, no isolated relief ROI, tracing, performance threshold or visual acceptance',maskWorker,rows,captures,stages,observations},null,2)+'\n');
    };
    const waitState=async(label,policy,relief,solderOn,afterEpoch=-1)=>{
      const instance=await currentInstance(),deadline=performance.now()+60000;let status;
      do{
        status=await probeOriginal('status');rows.push({label,instance,at:await page.evaluate(()=>performance.now()),status});
        if(status?.error)break;
        if(status.materialPolicy===policy&&status.illustrativeSolder===solderOn&&status.rasterBatchEpoch>afterEpoch){
          if(status.maskReliefState==='fallback'||status.rasterBatchState==='fallback')break;
          if(status.rasterBatchState==='active'&&(relief?status.maskReliefState==='ready':status.maskReliefState==='idle'))break;
        }
        await page.waitForTimeout(150);
      }while(performance.now()<deadline);
      await writeEvidence();assert.equal(await currentInstance(),instance,label+': worker changed while awaiting owned state');identify(status);
      assert.equal(status.materialPolicy,policy);assert.equal(status.illustrativeSolder,solderOn);assert.ok(status.rasterBatchEpoch>afterEpoch,label+': raster ownership did not advance');
      assert.equal(status.rasterBatchState,'active',status.rasterBatchReason||label);assert.equal(status.rasterBatchMultiDraw,true);assert.ok(status.rasterBatchBytes>0);
      assert.equal(status.maskReliefState,relief?'ready':'idle',status.maskReliefReason||label);assert.equal(status.maskReliefEstimated,relief);
      assert.equal(status.maskReliefMaps,relief?2:0);assert.ok(Number.isSafeInteger(status.maskReliefBytes));
      if(relief){assert.ok(status.maskReliefBytes>0);assert.equal(status.maskReliefStats?.sourceSHA256,asset.sha256);assert.equal(status.maskReliefBytes,status.maskReliefStats?.ownedBytes);assert.ok(Number.isFinite(status.maskReliefStats?.milliseconds)&&status.maskReliefStats.milliseconds>=0);}
      else assert.equal(status.maskReliefBytes,0);
      return status;
    };
    identify(initial);assert.equal(initial.materialPolicy,'cad-studio');assert.equal(initial.maskReliefMaps,0);assert.equal(initial.maskReliefBytes,0);assert.equal(initial.maskReliefEstimated,false);assert.equal(initial.illustrativeSolder,false);
    const originalFrame=await page.evaluate(({instance,key,revision,sha256})=>window.__viewerAcceptance.maskStatus.find(s=>s.instance===instance&&s.key===key&&s.revision===revision&&s.sha256===sha256&&s.visible&&s.firstFrame),{instance:firstInstance,key,revision:sourceRevision,sha256:asset.sha256});
    assert.ok(originalFrame,'initial CAD frame must come from the current original worker');
    await bar.getByRole('button',{name:'Components',exact:true}).click();await waitAlignment();
    const selected=metadata.components.find(c=>c.reference==='U1');assert.equal(selected?.id,'a249b1bf-7fe7-4abb-a884-9e32cc3761c3');
    await page.locator('.component-bom').getByRole('button',{name:'U1',exact:true}).click();
    const bounds=await probeOriginal('bounds',{id:selected.id});assert.equal(bounds?.id,selected.id);assert.equal(bounds.source,pcb);assert.equal(bounds.revision,sourceRevision);
    assert.ok(bounds.min.every((v,i)=>Number.isFinite(v)&&v<=bounds.max[i]));
    // A normal camera action followed by a public worker settle ACK flushes the
    // queued camera. It never supplies a pose, material, sample count or fixture.
    const settle=async()=>{
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const ack=await probeOriginal('interactive',{on:false});assert.ok(!ack?.error,JSON.stringify(ack));
      return probeOriginal('status');
    };
    await bar.getByRole('button',{name:'Fit',exact:true}).click();await page.locator('#solid').focus();await page.keyboard.press('ArrowLeft');await settle();let saved=await camera();
    assert.equal(saved.camera.selection.id,selected.id);assert.equal(saved.camera.selection.sourceRevision,sourceRevision);
    const preserved=async label=>{
      assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds,label+': original model bounds changed');
      assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
      const state=await camera();assert.deepEqual(state.camera,saved.camera,label+': saved camera/UUID changed');
      assert.ok(state.matrix?.every((v,i)=>Math.abs(v-saved.matrix[i])<1e-6),label+': camera matrix changed');
    };
    const menu=bar.locator('.solid-camera'),finish=bar.getByLabel('Material finish',{exact:true}),solder=bar.getByLabel('Illustrative solder',{exact:true});
    const open=async()=>{if(await menu.getAttribute('open')===null)await menu.locator(':scope > summary').click();};
    const close=async()=>{if(await menu.getAttribute('open')!==null)await menu.locator(':scope > summary').click();};
    const policy=async value=>{
      const start=await page.evaluate(()=>performance.now()),count=await page.evaluate(()=>window.__viewerAcceptance.workerMaterials.length);
      await open();await finish.waitFor({state:'visible'});assert.equal(await finish.locator('option[value="'+value+'"]').evaluate(o=>o.disabled),false);await finish.selectOption(value);
      await page.waitForFunction(({count,value})=>{const r=window.__viewerAcceptance.workerMaterials;return r.length>count&&r.at(-1)?.policy===value;},{count,value});
      const reply=await page.evaluate(()=>window.__viewerAcceptance.workerMaterials.at(-1));assert.ok(reply.result&&!reply.error,JSON.stringify(reply));assert.equal(reply.result.materialPolicy,value);await close();
      stages.push({type:'policy',value,start,reply});return reply.result;
    };
    await open();assert.equal(await solder.isChecked(),false);await close();
    await policy('realistic');let active=await waitState('initial Realistic relief','realistic',true,false);await preserved('initial relief');
    const timing=await page.evaluate(({instance,key})=>{
      const r=window.__viewerAcceptance;
      const seen=[...r.maskStatus.map(s=>({at:s.at,status:s})),...r.workerAcks.filter(a=>a.instance===instance&&a.result?.maskReliefState).map(a=>({at:a.at,status:a.result}))].filter(s=>(s.status.instance??instance)===instance&&s.status.key===key).sort((a,b)=>a.at-b.at);
      return {building:seen.find(s=>s.status.maskReliefState==='building')||null,ready:seen.find(s=>s.status.maskReliefState==='ready')||null};
    },{instance:firstInstance,key});
    assert.ok(timing.ready&&timing.ready.at>=originalFrame.at,'relief readiness cannot establish the original frame');if(timing.building)assert.ok(timing.building.at>=originalFrame.at);
    stages.push({type:'timing',originalFrame,buildingObservedAt:timing.building?.at??null,readyObservedAt:timing.ready?.at??null,firstFrameToBuildingObservationMs:timing.building?timing.building.at-originalFrame.at:null,firstFrameToReadyObservationMs:timing.ready?timing.ready.at-originalFrame.at:null,actualDerivationReportedMs:active.maskReliefStats.milliseconds,scope:'observed state/ACK milestones; no invented derivation-start boundary or speed threshold'});
    record('normal saved source Realistic relief owns two maps after original frame',{status:active,originalFrame,timing,selection:selected.id,bounds});
    // These are whole delivered frames, never a relief-only pixel comparison.
    for(const [label,button,z]of [['board',null,null],['top','Top',-1],['bottom','Bottom',1]]){
      if(button){await bar.getByRole('button',{name:button,exact:true}).click();await bar.getByRole('button',{name:'Fit',exact:true}).click();}const status=await settle();identify(status);
      assert.equal(status.maskReliefState,'ready');assert.equal(status.maskReliefMaps,2);const pose=await camera();if(z!==null)assert.ok(Math.abs(pose.camera.cam.f[2]-z)<1e-6,'normal orientation control did not select '+label);
      assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds);assert.equal(pose.camera.selection.id,selected.id);
      await page.locator('.solid-pathtrace').screenshot({path:join(output,'mask-'+label+'.png')});captures.push({label,status,pose});
    }
    await bar.getByRole('button',{name:'Iso',exact:true}).click();await bar.getByRole('button',{name:'Fit',exact:true}).click();await settle();saved=await camera();
    const toggle=async enabled=>{
      const before=active.rasterBatchEpoch,count=await page.evaluate(()=>window.__viewerAcceptance.workerSolder.length);
      await open();assert.equal(await solder.isEnabled(),true);await solder.setChecked(enabled);
      await page.waitForFunction(count=>window.__viewerAcceptance.workerSolder.length>count,count);
      const reply=await page.evaluate(()=>window.__viewerAcceptance.workerSolder.at(-1));assert.ok(reply.result&&!reply.error,JSON.stringify(reply));assert.equal(reply.enabled,enabled);assert.equal(reply.result.enabled,enabled);assert.equal(reply.result.source,pcb);assert.equal(reply.result.revision,sourceRevision);await close();
      assert.equal(reply.result.coverage.joints,enabled?116:0);if(enabled){assert.equal(reply.result.coverage.components,58);assert.equal(reply.result.coverage.sourceCandidates.source,pcb);assert.equal(reply.result.coverage.sourceCandidates.revision,sourceRevision);}
      stages.push({type:'solder',enabled,reply});active=await waitState('solder '+enabled,'realistic',true,enabled,before);assert.equal(active.solderJoints,enabled?116:0);await preserved('solder '+enabled);
    };
    await toggle(true);await toggle(false);record('normal illustrative solder OFF ON OFF rebuilds owned relief',{status:active,selection:selected.id,bounds});
    let epoch=active.rasterBatchEpoch;const sourcePolicy=await policy('source-defaults');
    assert.equal(sourcePolicy.maskReliefMaps,0);assert.equal(sourcePolicy.maskReliefBytes,0);assert.equal(sourcePolicy.maskReliefEstimated,false);
    active=await waitState('source policy retires relief','source-defaults',false,false,epoch);await preserved('source policy');
    epoch=active.rasterBatchEpoch;await policy('realistic');active=await waitState('Realistic policy restores relief','realistic',true,false,epoch);await preserved('Realistic restore');
    const departed=await currentInstance();await renderMode.selectOption('fast');
    await page.waitForFunction(instance=>!document.querySelector('.solid-pathtrace')&&window.__viewerAcceptance.workerTeardowns.some(t=>t.instance===instance),departed);
    stages.push({type:'Fast',retiredInstance:departed,at:await page.evaluate(()=>performance.now())});await renderMode.selectOption('studio');
    await page.waitForFunction(({departed,key,revision,sha256})=>{const r=window.__viewerAcceptance,w=window.__viewerAcceptanceWorker,root=document.querySelector('#solid-workbench');return w?.acceptanceInstance!==departed&&w?.acceptanceScope==='t1'&&root?.classList.contains('solid-original-visible')&&r.maskStatus.some(s=>s.instance===w.acceptanceInstance&&s.key===key&&s.revision===revision&&s.sha256===sha256&&s.visible&&s.firstFrame);},{departed,key,revision:sourceRevision,sha256:asset.sha256},{timeout:180000});
    active=await waitState('new current Studio worker','realistic',true,false);await waitAlignment();await preserved('Fast Studio reopen');
    const authority=await page.evaluate(async()=>{
      const r=window.__viewerAcceptance,events=[];
      for(const frame of r.wireFrames){if(frame.direction!=='receive')continue;const data=frame.data instanceof Blob?await frame.data.arrayBuffer():frame.data,decoded=window.__viewerDecodeFrame(data);if(decoded?.t==='plotrevision'&&decoded.key?.startsWith('2|'))events.push({type:'revision',at:frame.at,generation:frame.generation,key:decoded.key,revision:decoded.revision});}
      return {events:[...events,...r.socketEvents.filter(e=>e.viewer).map(e=>({...e,type:'socket'}))].sort((a,b)=>a.at-b.at),loads:r.workerLoads,teardowns:r.workerTeardowns,current:r.workerSerial};
    });
    let generation=0,eventIndex=0;const ledger=new Map(),authorizedLoads=[];
    for(const load of authority.loads){
      while(eventIndex<authority.events.length&&authority.events[eventIndex].at<=load.at){const e=authority.events[eventIndex++];if(e.type==='socket'&&e.event==='construct'){ledger.clear();generation=e.generation;}else if(e.type==='socket'&&e.event==='close'&&generation===e.generation){ledger.clear();generation=0;}else if(e.type==='revision'&&e.generation===generation)ledger.set(e.key,e);}
      const confirmed=ledger.get(load.key);assert.equal(load.generation,generation);assert.ok(confirmed&&confirmed.revision===load.revision,'current worker lacks live-generation independent 3D confirmation');
      assert.equal(load.key,key);assert.equal(load.revision,sourceRevision);assert.equal(load.source,pcb);assert.equal(load.sha256,asset.sha256);assert.equal(new URL(load.url).searchParams.get('id'),asset.sha256);authorizedLoads.push({load,confirmation:confirmed});
    }
    const restoredInstance=await currentInstance();assert.ok(authorizedLoads.some(x=>x.load.instance===restoredInstance),'reopened current worker load was not observed');
    stages.push({type:'restored',status:active,instance:await currentInstance(),authorizedLoads,selection:selected.id,bounds,saved});await writeEvidence();
    assert.equal(await renderMode.inputValue(),'studio');assert.deepEqual(errors,[]);assert.ok(network.every(x=>!x.url.includes(token)));
    record('mask source policy and Fast Studio retirement retain current-worker source authority',{status:active,authorizedLoads,selection:selected.id,bounds,scope:'focused raster lifecycle; explicit fallback fails with saved diagnostics; no trace or visual acceptance'});
  }else if(restoreOnly){
    await checkCachedRestore();
  }else if(fidelityOnly){
    const key=await page.locator('#solid-workbench').getAttribute('data-key');
    const batchStates=new Set(['idle','building','ready','active','fallback']);
    const identify=status=>{
      assert.ok(status&&!status.error,JSON.stringify(status));assert.equal(status.worker,true);
      assert.equal(status.key,key);assert.equal(status.source,pcb);assert.equal(status.revision,sourceRevision);assert.equal(status.sha256,asset.sha256);
      assert.equal(status.triangles,fidelityInventory.triangles);assert.equal(status.authoredNormals,fidelityInventory.normals);assert.equal(status.sourceMeshes,fidelityInventory.primitiveMeshes);
      assert.equal(status.traceAllocated,false);assert.equal(status.bvhBuilding,false);assert.equal(status.bvhReady,false);
      assert.ok(batchStates.has(status.rasterBatchState));assert.equal(typeof status.rasterBatchReason,'string');
      for(const field of ['rasterBatchSourceMeshes','rasterBatchGroups','rasterBatchTransparentMeshes','rasterBatchBytes','rasterBatchEpoch'])assert.ok(Number.isSafeInteger(status[field])&&status[field]>=0,`invalid actual ${field}`);
      assert.ok(status.rasterBatchBuildMs===null||Number.isFinite(status.rasterBatchBuildMs)&&status.rasterBatchBuildMs>=0);
      assert.equal(status.rasterBatchMultiDraw,true,'this hardware gate requires actual multi-draw support');
      assert.match(status.unmaskedRenderer||'',/RTX\s*5080/i);
    };
    const waitBatch=async(policy,epochAfter=-1,expected=null)=>{
      const status=await page.evaluate(({policy,epochAfter,source,revision,startID})=>new Promise(resolve=>{
        const worker=window.__viewerAcceptanceWorker,rows=window.__viewerAcceptance.batchObservations??=[],session=window.__viewerAcceptance.workerLifecycle.filter(e=>e.event==='constructor').length;
        let id=startID,deadline,poll,finished=false;
        const finish=value=>{if(finished)return;finished=true;clearTimeout(deadline);clearTimeout(poll);worker.removeEventListener('message',receive);resolve(value);};
        const send=()=>{id--;worker.postMessage({id,method:'status',args:{}});};
        const receive=({data})=>{if(data.id!==id)return;if(data.error){finish({error:data.error});return;}
          const s=data.result;rows.push({session,at:performance.now(),status:s});
          if(s?.source===source&&s.revision===revision&&s.materialPolicy===policy&&s.rasterBatchEpoch>epochAfter&&s.rasterBatchState==='active'){finish(s);return;}
          if(s?.rasterBatchState==='fallback'){finish(s);return;}
          poll=setTimeout(send,150);
        };
        worker.addEventListener('message',receive);deadline=setTimeout(()=>finish({error:'Actual raster batching did not become active in 60 seconds',last:rows.at(-1)}),60000);send();
      }),{policy,epochAfter,source:pcb,revision:sourceRevision,startID:(workerProbeID-=10000)});
      identify(status);assert.equal(status.rasterBatchState,'active',status.rasterBatchReason);assert.equal(status.materialPolicy,policy);
      assert.ok(Number.isFinite(status.rasterBatchBuildMs)&&status.rasterBatchBytes>0,'active batching lacks completed build statistics');
      if(expected){assert.equal(status.rasterBatchGroups,expected.groups);assert.equal(status.rasterBatchTransparentMeshes,expected.transparent);}
      // Active describes a derived draw. Original visible/firstFrame remains
      // an independent requirement; neither field can substitute for the other.
      assert.equal(status.visible,true);assert.equal(status.firstFrame,true);
      return status;
    };
    let active=await waitBatch('cad-studio',-1,{groups:18,transparent:2});
    assert.equal(active.illustrativeSolder,false);assert.equal(active.solderJoints,0);
    record('full original source becomes visibly ready and uses actual raster batches',{status:active,inventory:fidelityInventory});
    await bar.getByRole('button',{name:'Components',exact:true}).click();await waitAlignment();
    await page.waitForFunction(({source,revision})=>window.__viewerAcceptance.workerComponents.some(c=>c.source===source&&c.revision===revision&&c.result),{source:pcb,revision:sourceRevision});
    const mapping=await page.evaluate(({source,revision})=>window.__viewerAcceptance.workerComponents.findLast(c=>c.source===source&&c.revision===revision&&c.result),{source:pcb,revision:sourceRevision});
    assert.equal(metadata.components.length,96);assert.equal(mapping.result.mapped,89,'mapped source bodies must remain distinct from metadata components');
    const selected=metadata.components.find(c=>c.reference==='U1');assert.ok(selected);assert.equal(selected.id,'a249b1bf-7fe7-4abb-a884-9e32cc3761c3');
    await page.locator('.component-bom').getByRole('button',{name:'U1',exact:true}).click();
    await page.waitForFunction(id=>window.__viewerAcceptance.workerSelections.some(s=>s.id===id),selected.id);
    const bounds=await probeOriginal('bounds',{id:selected.id});assert.equal(bounds?.id,selected.id);assert.equal(bounds.source,pcb);assert.equal(bounds.revision,sourceRevision);
    assert.ok(bounds.min.every((v,i)=>Number.isFinite(v)&&v<=bounds.max[i]));
    await bar.getByRole('button',{name:'Frame part',exact:true}).click();
    await page.waitForFunction(pivot=>window.__viewerAcceptance.workerCameras.some(c=>c.state.cam.pivot.every((v,i)=>Math.abs(v-pivot[i])<1e-6)),bounds.min.map((v,i)=>(v+bounds.max[i])*50*(i===1?-1:1)));
    await bar.getByRole('button',{name:'Fit',exact:true}).click();
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const saved=await page.evaluate(()=>({camera:window.__viewerAcceptance.workerCameras.at(-1)?.state,matrix:window.__viewerAcceptance.matrices.t1}));
    const preserved=async label=>{
      assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds,`${label}: source body bounds changed`);
      assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
      assert.deepEqual(await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state),saved.camera,`${label}: camera or source selection changed`);
      await page.waitForFunction(expected=>window.__viewerAcceptance.matrices.t1?.every((v,i)=>Math.abs(v-expected[i])<1e-6),saved.matrix);
    };
    await page.locator('.solid-pathtrace').screenshot({path:join(output,'fidelity-cad-studio.png')});
    const cameraMenu=bar.locator('.solid-camera'),finish=bar.getByLabel('Material finish',{exact:true}),solder=page.getByLabel('Illustrative solder',{exact:true});
    const openCamera=async()=>{if(await cameraMenu.getAttribute('open')===null)await cameraMenu.locator(':scope > summary').click();};
    const closeCamera=async()=>{if(await cameraMenu.getAttribute('open')!==null)await cameraMenu.locator(':scope > summary').click();};
    await openCamera();assert.equal(await solder.isChecked(),false);assert.equal(await solder.isEnabled(),true);
    await finish.selectOption('realistic');await closeCamera();
    const realistic=await waitBatch('realistic',active.rasterBatchEpoch,{groups:20,transparent:0});
    assert.deepEqual(realistic.materialRoleCounts,active.materialRoleCounts);await preserved('Realistic');active=realistic;
    await page.locator('.solid-pathtrace').screenshot({path:join(output,'fidelity-realistic.png')});
    // Replace an asynchronous policy build through real controls. Only the
    // final policy/epoch may own derived statistics and the active draw.
    await openCamera();await finish.selectOption('cad-studio');await finish.selectOption('realistic');await closeCamera();
    active=await waitBatch('realistic',active.rasterBatchEpoch,{groups:20,transparent:0});await preserved('rapid policy replacement');
    record('full source mapping selection and camera survive material epoch replacement',{metadataComponents:96,mappedSourceBodies:mapping.result.mapped,selection:selected.id,bounds,realistic,status:active});
    const setSolder=async enabled=>{
      const count=await page.evaluate(()=>window.__viewerAcceptance.workerSolder.length);
      await openCamera();await solder.setChecked(enabled);
      await page.waitForFunction(count=>window.__viewerAcceptance.workerSolder.length>count,count);
      const reply=await page.evaluate(()=>window.__viewerAcceptance.workerSolder.at(-1));
      assert.equal(reply.enabled,enabled);assert.ok(reply.result&&!reply.error,JSON.stringify(reply));assert.equal(reply.result.enabled,enabled);
      assert.equal(reply.result.source,pcb);assert.equal(reply.result.revision,sourceRevision);assert.equal(await solder.isChecked(),enabled);await closeCamera();return reply.result;
    };
    const on=await setSolder(true),candidates=on.coverage.sourceCandidates;
    assert.equal(on.coverage.joints,116);assert.equal(on.coverage.components,58);assert.equal(on.coverage.anchors.length,116);
    assert.ok(candidates);assert.equal(candidates.source,pcb);assert.equal(candidates.revision,sourceRevision);
    for(const [field,value]of Object.entries({meshes:4027,triangles:43500,padMeshes:2465,rcMeshes:1562}))assert.equal(candidates[field],value,`actual solder candidate ${field}`);
    assert.ok(Number.isSafeInteger(candidates.visitedNodes)&&candidates.visitedNodes>0);assert.ok(Number.isSafeInteger(candidates.padDefinition)&&candidates.padDefinition>=0);assert.match(candidates.padOriginalName,/_pad$/);assert.ok(typeof candidates.selection==='string'&&candidates.selection.length>0);
    for(const [field,max]of Object.entries({maxNodes:60000,maxMeshes:12000,maxTriangles:300000}))assert.ok(Number.isSafeInteger(candidates.limits?.[field])&&candidates.limits[field]>0&&candidates.limits[field]<=max);
    const rc=new Map(metadata.components.filter(c=>/^[RC]\d+$/.test(c.reference)).map(c=>[c.id,c.reference]));
    assert.ok(on.coverage.anchors.every(a=>rc.get(a.sourceUuid)===a.reference));assert.equal(new Set(on.coverage.anchors.map(a=>a.sourceUuid)).size,58);
    active=await waitBatch('realistic',active.rasterBatchEpoch);assert.equal(active.illustrativeSolder,true);assert.equal(active.solderJoints,116);await preserved('illustrative solder ON');
    await page.locator('.solid-pathtrace').screenshot({path:join(output,'fidelity-illustrative-solder.png')});
    const off=await setSolder(false);assert.equal(off.coverage.joints,0);
    active=await waitBatch('realistic',active.rasterBatchEpoch,{groups:20,transparent:0});assert.equal(active.illustrativeSolder,false);assert.equal(active.solderJoints,0);await preserved('illustrative solder OFF');
    record('actual original solder candidates toggle OFF ON OFF across clean raster epochs',{on,off,status:active,scope:'illustrative estimated geometry only; no measured manufactured joints'});
    // Disposal/reopen covers the new source/build ownership hook only; this
    // does not repeat the old reflow, layer, tracing or responsive-layout suites.
    const constructors=await page.evaluate(()=>window.__viewerAcceptance.workerLifecycle.filter(e=>e.event==='constructor').length),terminations=await page.evaluate(()=>window.__viewerAcceptance.workerTerminations);
    await page.locator('[data-on-click="select"][data-value="t2"]').first().click();await choose3d();
    await page.waitForFunction(before=>document.querySelector('#solid-workbench')?.dataset.scope==='t2'&&window.__viewerAcceptance.workerLifecycle.filter(e=>e.event==='constructor').length>before,constructors);
    await page.locator('[data-on-click="select"][data-value="t1"]').first().click();await waitOriginal('raster');await waitAlignment();
    active=await waitBatch('realistic',-1,{groups:20,transparent:0});await preserved('source disposal/reopen');assert.equal(active.illustrativeSolder,false);
    assert.ok(await page.evaluate(before=>window.__viewerAcceptance.workerTerminations>before,terminations));
    const observations=await page.evaluate(()=>window.__viewerAcceptance.batchObservations||[]),epochs=new Map();
    for(const row of observations){
      const s=row.status,previous=epochs.get(row.session)??-1;assert.ok(s.rasterBatchEpoch>=previous,'raster ownership epoch went backwards within a worker');
      if(s.rasterBatchEpoch>previous&&['idle','building'].includes(s.rasterBatchState)){
        assert.equal(s.rasterBatchGroups,0);assert.equal(s.rasterBatchBytes,0);assert.equal(s.rasterBatchBuildMs,null,'new epoch retained previous completed build timing');
      }
      epochs.set(row.session,s.rasterBatchEpoch);
    }
    assert.deepEqual(errors,[]);assert.ok(network.every(x=>!x.url.includes(token)));assert.equal(await renderMode.inputValue(),'studio');
    fs.writeFileSync(join(output,'raster-lifecycle.json'),JSON.stringify({mapping,selection:selected.id,bounds,observations,restored:active},null,2)+'\n');
    record('full original source survives asynchronous raster policy solder and disposal lifecycle',{restored:active,sourceBounds:bounds,selection:selected.id,scope:'active derived raster plus independent original visible/firstFrame; no path tracing or product GL submission-count claim'});
  }else if(renderOnly){
    const identity=status=>{
      assert.ok(status&&!status.error,JSON.stringify(status));assert.equal(status.worker,true);
      assert.equal(status.source,pcb);assert.equal(status.revision,sourceRevision);
      assert.equal(status.key,assetKey);assert.equal(status.sha256,asset.sha256);
      assert.match(status.unmaskedRenderer||'',/RTX\s*5080/i);
    };
    const assetKey=await page.locator('#solid-workbench').getAttribute('data-key');
    const studio=await probeOriginal('status');identity(studio);
    assert.equal(studio.phase,'raster');assert.equal(studio.visible,true);assert.equal(studio.firstFrame,true);
    assert.equal(studio.traceAllocated,false);assert.equal(studio.bvhBuilding,false);assert.equal(studio.bvhReady,false);
    await bar.getByRole('button',{name:'Components',exact:true}).click();await waitAlignment();
    const selected=metadata.components.find(c=>c.reference==='U1'&&c.pads?.length)||metadata.components.find(c=>c.pads?.length&&c.reference);
    assert.ok(selected,'render lifecycle requires a saved source component');
    await page.locator('.component-bom').getByRole('button',{name:selected.reference,exact:true}).click();
    await page.waitForFunction(id=>window.__viewerAcceptance.workerSelections.some(s=>s.id===id),selected.id);
    const bounds=await probeOriginal('bounds',{id:selected.id});
    assert.equal(bounds?.id,selected.id);assert.equal(bounds.source,pcb);assert.equal(bounds.revision,sourceRevision);
    await bar.getByRole('button',{name:'Fit',exact:true}).click();
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const finish=bar.getByLabel('Material finish',{exact:true});
    if(renderMaterial&&await finish.inputValue()!==renderMaterial){
      const cameraMenu=bar.locator('.solid-camera');
      if(await cameraMenu.getAttribute('open')===null)await cameraMenu.locator(':scope > summary').click();
      await finish.waitFor({state:'visible'});
      assert.equal(await finish.locator(`option[value="${renderMaterial}"]`).evaluate(o=>o.disabled),false);
      await finish.selectOption(renderMaterial);
      await page.waitForFunction(policy=>window.__viewerAcceptance.workerMaterials.at(-1)?.result?.materialPolicy===policy,renderMaterial);
      await cameraMenu.locator(':scope > summary').click();
    }
    const materialPolicy=await finish.inputValue(),qualified=await probeOriginal('status');identity(qualified);
    assert.equal(qualified.materialPolicy,materialPolicy);
    assert.ok(qualified.materialPolicies?.includes(materialPolicy));
    assert.deepEqual(qualified.materialRoleCounts,studio.materialRoleCounts,'material choice changed source role coverage');
    if(materialPolicy==='realistic'){
      assert.ok(qualified.materialPolicies.includes('realistic'));
      for(const role of ['soldermask','silkscreen','pad','substrate'])assert.ok(qualified.materialRoleCounts?.[role]>0,`Realistic source role missing: ${role}`);
    }
    const saved=await page.evaluate(()=>({camera:window.__viewerAcceptance.workerCameras.at(-1)?.state,matrix:window.__viewerAcceptance.matrices.t1}));
    assert.equal(saved.camera.selection.id,selected.id);assert.equal(saved.camera.selection.sourceRevision,sourceRevision);
    record('focused renderer starts with exact original Studio source and qualified selected material',{studio,materialPolicy,qualified,selection:{id:selected.id,bounds}});
    assert.equal(await renderMode.locator('option[value="pathtrace"]').evaluate(o=>o.disabled),false);
    await renderMode.selectOption('pathtrace');
    const samples=bar.getByLabel('Path tracing sample limit',{exact:true});
    const sampleOptions=await samples.locator('option').evaluateAll(options=>options.map(o=>Number(o.value)));
    assert.ok(sampleOptions.length&&sampleOptions.every(n=>Number.isInteger(n)&&n>=32),'normal toolbar must retain its supported minimum sample limit');
    await samples.selectOption('32');assert.equal(await samples.inputValue(),'32');
    await page.waitForFunction(({key,revision})=>window.__viewerAcceptance.workerStatus.some(s=>s.key===key&&s.revision===revision&&s.visible&&s.samples>=1&&s.resolvePending===false&&['pathtrace','paused','converged'].includes(s.phase)),{key:assetKey,revision:sourceRevision},{timeout:180000});
    const traced=await probeOriginal('status');identity(traced);
    const resolved=status=>{
      assert.ok(Number.isFinite(status.samples)&&Math.floor(status.samples)>=1,'no completed positive sample');
      assert.equal(status.traceAllocated,true);assert.equal(status.hdrResolve,'pixel-area');assert.equal(status.resolveMaxTexelsPerPixel,9);
      assert.equal(status.resolvePending,false,'matching-size HDR resolve must finish');assert.equal(status.resolvePendingReason,'');
      for(const key of ['traceTargetWidth','traceTargetHeight','resolveInputWidth','resolveInputHeight','resolveWidth','resolveHeight','renderWidth','renderHeight'])assert.ok(Number.isSafeInteger(status[key])&&status[key]>0,`invalid actual ${key}`);
      assert.equal(status.resolveInputWidth,status.traceTargetWidth);assert.equal(status.resolveInputHeight,status.traceTargetHeight);
      assert.equal(status.resolveWidth,status.renderWidth);assert.equal(status.resolveHeight,status.renderHeight);
      assert.ok(status.resolveBytes>0);assert.equal(status.materialPolicy,materialPolicy);
    };
    resolved(traced);
    // Trace starts only through the normal toolbar (32 minimum). This existing
    // worker presentation command selects raw resolved HDR solely for evidence;
    // it does not lower samples, replace geometry or start a test-only renderer.
    const presented=await probeOriginal('present',{denoised:false});
    assert.ok(presented&&!presented.error);assert.equal(presented.source,pcb);assert.equal(presented.revision,sourceRevision);assert.equal(presented.denoised,false);
    const raw=await probeOriginal('status');identity(raw);resolved(raw);
    const rawFramePath=join(output,'render-only-raw-resolved.png');
    await page.locator('.solid-pathtrace').screenshot({path:rawFramePath,timeout:30000});
    const rawFrame={path:rawFramePath,bytes:fs.statSync(rawFramePath).size,sha256:digest(fs.readFileSync(rawFramePath))};
    record('focused normal toolbar trace produces completed samples and raw pixel-area HDR frame',{sampleLimit:32,traced,presented,raw,rawFrame,scope:'functional resolved-frame evidence; no convergence or realism approval'});
    const terminations=await page.evaluate(()=>window.__viewerAcceptance.workerTerminations);
    await renderMode.selectOption('fast');
    await page.waitForFunction(before=>!document.querySelector('.solid-pathtrace')&&window.__viewerAcceptance.workerTerminations>before,terminations);
    assert.equal(await page.locator('#solid-workbench').evaluate(root=>root.classList.contains('solid-original-visible')),false);
    await page.waitForFunction(ref=>document.querySelector('.component-selection-overlay')?.textContent.includes(ref),selected.reference);
    record('focused Fast removes original canvas and terminates its worker',{terminations:await page.evaluate(()=>window.__viewerAcceptance.workerTerminations)});
    const loads=await page.evaluate(()=>window.__viewerAcceptance.workerLoads.length);
    await renderMode.selectOption('studio');
    await page.waitForFunction(({loads,key,revision})=>{const r=window.__viewerAcceptance,load=r.workerLoads.at(-1);return r.workerLoads.length>loads&&load.key===key&&load.revision===revision&&document.querySelector('#solid-workbench')?.classList.contains('solid-original-visible');},{loads,key:assetKey,revision:sourceRevision},{timeout:180000});
    await waitAlignment();const restored=await probeOriginal('status');identity(restored);
    assert.equal(restored.phase,'raster');assert.equal(restored.visible,true);assert.equal(restored.firstFrame,true);assert.equal(restored.traceAllocated,false);assert.equal(restored.bvhBuilding,false);assert.equal(restored.bvhReady,false);
    assert.equal(restored.materialPolicy,materialPolicy);
    assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
    await page.waitForFunction(expected=>window.__viewerAcceptance.matrices.t1?.every((v,i)=>Math.abs(v-expected[i])<1e-6),saved.matrix);
    assert.deepEqual(await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state),saved.camera,'render mode lifecycle changed source selection or camera');
    assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds,'render lifecycle changed actual source body bounds');
    assert.deepEqual(errors,[]);assert.ok(network.every(x=>!x.url.includes(token)));
    await page.locator('.solid-pathtrace').screenshot({path:join(output,'render-only-restored-studio.png')});
    record('focused Studio restores original same-revision source selection and camera',{restored,selection:selected.id});
  }else if(!loadOnly){
  if(qualityMode==='off')await renderMode.selectOption('fast');
  await bar.getByRole('button',{name:'PCB',exact:true}).click();
  await page.locator('.solid-split .solid-pcb canvas#plot').waitFor({state:'visible'});
  const layers=page.locator('.solid-pcb-controls .layers'),pcbCanvas=page.locator('.solid-pcb canvas#plot');
  if(!focused){
  await layers.locator('summary').click();
  const front=layers.locator('.layer-row').filter({has:page.getByRole('button',{name:'F.Cu',exact:true})});
  const layer=Number(await front.getAttribute('data-layer')),bit=2**layer;
  const oldOff=Number(await pcbCanvas.getAttribute('data-off'));
  await front.locator('[data-on-click="view-layer"]').click();
  await page.waitForFunction(({oldOff,bit})=>Math.floor(Number(document.querySelector('.solid-pcb #plot').dataset.off)/bit)%2!==Math.floor(oldOff/bit)%2,{oldOff,bit});
  await front.locator('[data-on-click="view-layer"]').click();
  await page.waitForFunction(oldOff=>Number(document.querySelector('.solid-pcb #plot').dataset.off)===oldOff,oldOff);
  await front.getByRole('button',{name:'F.Cu',exact:true}).click();
  await layers.locator('[data-on-click="view-layer-mode"][data-value="1"]').click();
  await page.waitForFunction(layer=>Number(document.querySelector('.solid-pcb #plot').dataset.dim)>0&&Number(document.querySelector('.solid-pcb #plot').dataset.lift)===layer,layer);
  const dim=Number(await pcbCanvas.getAttribute('data-dim'));
  await layers.locator('[data-on-click="view-layer-mode"][data-value="2"]').click();
  await page.waitForFunction(()=>Number(document.querySelector('.solid-pcb #plot').dataset.off)>0);
  await layers.locator('[data-on-click="view-layer-mode"][data-value="0"]').click();
  await page.waitForFunction(oldOff=>{const d=document.querySelector('.solid-pcb #plot').dataset;return Number(d.off)===oldOff&&Number(d.dim)===0;},oldOff);
  await layers.locator('summary').click();record('normal split PCB layers toggle and All/Dim/Only propagate to actual canvas', {layer,bit,dim});
  }
  await bar.getByRole('button',{name:'Components',exact:true}).click();
  await waitAlignment();
  const selected=metadata.components.find(c=>c.reference==='U1'&&c.pads?.length)||metadata.components.find(c=>c.pads?.length&&c.reference);
  assert.ok(selected,'no saved component with pads');await page.locator('.component-bom').getByRole('button',{name:selected.reference,exact:true}).click();
  await page.locator('.component-bom tr[aria-selected="true"]').waitFor();
  assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
  await page.getByRole('img',{name:`${selected.reference} saved footprint, numbered pads`,exact:true}).waitFor();
  if(await renderMode.inputValue()==='fast')await page.waitForFunction(ref=>document.querySelector('.component-selection-overlay')?.textContent.includes(ref),selected.reference);
  else {
    await page.waitForFunction(id=>window.__viewerAcceptance.workerSelections.some(s=>s.id===id&&s.key===document.querySelector('#solid-workbench')?.dataset.key),selected.id);
    assert.equal(await page.locator('.component-selection-overlay').count(),0,'original-body selection must not use a footprint overlay');
  }
  if(qualityMode==='required') {
    const beforeFrame=await page.evaluate(()=>window.__viewerAcceptance.matrices.t1);
    await bar.getByRole('button',{name:'Frame part',exact:true}).click();
    await page.waitForFunction(id=>window.__viewerAcceptance.workerBounds.some(b=>b.id===id),selected.id);
    const framed=await page.evaluate(id=>window.__viewerAcceptance.workerBounds.findLast(b=>b.id===id),selected.id);
    assert.ok(framed.result&&!framed.error,JSON.stringify(framed));
    assert.equal(framed.result.revision,sourceRevision);assert.equal(framed.result.source,pcb);assert.equal(framed.result.id,selected.id);
    assert.ok(framed.result.min.every((v,i)=>Number.isFinite(v)&&v<=framed.result.max[i]));
    await page.waitForFunction(before=>window.__viewerAcceptance.matrices.t1.some((v,i)=>Math.abs(v-before[i])>1e-5),beforeFrame);
    const pivot=framed.result.min.map((v,i)=>(v+framed.result.max[i])*50*(i===1?-1:1));
    await page.waitForFunction(pivot=>window.__viewerAcceptance.workerCameras.some(c=>c.state.cam.pivot.every((v,i)=>Math.abs(v-pivot[i])<1e-6)),pivot);
    record('normal UI selected component frames actual same-revision source bounds',{reference:selected.reference,bounds:framed.result,pivot});
    if(!focused)await bar.getByRole('button',{name:'Fit',exact:true}).click();
  }
  if(layoutOnly){
    const initialTiming=initialLoadingProfile?{...initialLoadingProfile.observations,bytes:initialLoadingProfile.bytes}:await page.evaluate(revision=>{
      const r=window.__viewerAcceptance,key=document.querySelector('#solid-workbench').dataset.key;
      return {click:r.viewClicks[0],load:r.workerLoads.find(s=>s.key===key&&s.revision===revision),bytes:r.workerStatus.find(s=>s.key===key&&s.revision===revision&&s.reason==='Parsing original model'),firstFrame:r.workerStatus.find(s=>s.key===key&&s.revision===revision&&s.visible&&s.firstFrame)};
    },sourceRevision);
    assert.ok(initialTiming.click&&initialTiming.load&&initialTiming.firstFrame,'initial original-frame milestones are missing');
    assert.ok(initialTiming.click.at<=initialTiming.load.at&&initialTiming.load.at<=initialTiming.firstFrame.at&&(!initialTiming.bytes||initialTiming.load.at<=initialTiming.bytes.at&&initialTiming.bytes.at<=initialTiming.firstFrame.at),'original-frame milestones are out of order');
    record('cold browser original GPU frame timing with API export warmed',{clock:'main page performance.now; worker milestones use main receipt time unless bytesMeasurement specifies converted worker resource timing',bytesMeasurement:initialTiming.bytes?.measurement||(initialTiming.bytes?'main receipt of worker parsing status':'unmeasured'),browserContext:'new isolated context',apiExport:'warmed by explicit preflight; excluded from click clock',clickAt:initialTiming.click.at,workerLoadPostAt:initialTiming.load.at,bytesArrivalAt:initialTiming.bytes?.at??null,firstFrameAt:initialTiming.firstFrame.at,clickToFrameMs:initialTiming.firstFrame.at-initialTiming.click.at,bytesToFrameMs:initialTiming.bytes?initialTiming.firstFrame.at-initialTiming.bytes.at:null,key:initialTiming.firstFrame.key,sourceRevision});
    const measurements=[],frameReveals=[];
    const assertFrameRevealsCanvas=async label=>{
      // Reproduce the BOM-induced crop in the owning pane. Capture again at
      // the actual click so Playwright's actionability scrolling cannot hide it.
      await page.locator('.component-bom tr[aria-selected="true"]').evaluate(row=>row.scrollIntoView({block:'center',inline:'nearest',behavior:'instant'}));
      await page.locator('#solid-workbench .solid-layout').evaluate(layout=>{layout.scrollTop=layout.scrollHeight-layout.clientHeight;});
      const button=bar.getByRole('button',{name:'Frame part',exact:true});
      await button.evaluate(button=>{
        const snapshot=()=>{
          const root=document.querySelector('#solid-workbench'),layout=root.querySelector('.solid-layout'),b=layout.getBoundingClientRect();
          const rect=canvas=>{const r=canvas.getBoundingClientRect();return {className:canvas.className,id:canvas.id,left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:r.width,height:r.height};};
          return {scrollTop:layout.scrollTop,viewport:{left:b.left+layout.clientLeft,top:b.top+layout.clientTop,right:b.left+layout.clientLeft+layout.clientWidth,bottom:b.top+layout.clientTop+layout.clientHeight},canvases:[...root.querySelectorAll('#solid,.solid-pathtrace')].map(rect),windowScroll:{x:scrollX,y:scrollY},documentScroll:{left:document.scrollingElement.scrollLeft,top:document.scrollingElement.scrollTop}};
        };
        window.__viewerFrameRevealSnapshot=snapshot;
        button.addEventListener('click',()=>{window.__viewerFrameRevealBefore=snapshot();},{capture:true,once:true});
      });
      const count=await page.evaluate(()=>window.__viewerAcceptance.workerBounds.length);
      await button.click();
      await page.waitForFunction(count=>window.__viewerAcceptance.workerBounds.length>count,count);
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const evidence=await page.evaluate(()=>({before:window.__viewerFrameRevealBefore,after:window.__viewerFrameRevealSnapshot()}));
      assert.ok(evidence.before.scrollTop>0,`${label}: Frame part must start with its own layout scrolled`);
      assert.ok(evidence.before.canvases.some(c=>c.top<evidence.before.viewport.top-1),`${label}: reproduce the clipped canvas before the action`);
      assert.equal(evidence.after.scrollTop,0,`${label}: Frame part did not reveal the top of its own layout`);
      assert.deepEqual(evidence.after.windowScroll,evidence.before.windowScroll,`${label}: Frame part scrolled the window`);
      assert.deepEqual(evidence.after.documentScroll,evidence.before.documentScroll,`${label}: Frame part scrolled the document`);
      assert.ok(evidence.after.canvases.some(c=>c.className==='solid-pathtrace'),`${label}: original-model GPU canvas missing`);
      for(const c of evidence.after.canvases){
        const v=evidence.after.viewport;
        assert.ok(c.width>0&&c.height>=259&&c.left>=v.left-1&&c.right<=v.right+1&&c.top>=v.top-1&&c.bottom<=v.bottom+1,`${label}: full ${c.id||c.className} DOM rectangle is clipped by its layout viewport: ${JSON.stringify({canvas:c,viewport:v})}`);
      }
      frameReveals.push({label,...evidence});record('Frame part reveals full actual canvas inside its own layout viewport',{label,...evidence});
    };
    const measurePane=async label=>{
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const m=await page.evaluate(()=>{
        const root=document.querySelector('#solid-workbench'),rect=selector=>{const b=root.querySelector(selector).getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height,bottom:b.bottom};};
        return {viewport:{width:innerWidth,height:innerHeight,scrollWidth:document.documentElement.scrollWidth},pane:root.getBoundingClientRect().width,model:rect('#solid'),pcb:rect('.solid-pcb #plot'),inspector:rect('.solid-inspector'),scope:root.dataset.scope,key:root.dataset.key};
      });
      const minWidth=Math.min(320,m.pane-16);
      for(const [name,box]of [['3D',m.model],['PCB',m.pcb]]){assert.ok(box.width>=minWidth-1,`${label} ${name} width ${box.width} < ${minWidth}`);assert.ok(box.width<=m.pane+1,`${label} ${name} overflows pane`);assert.ok(box.height>=259,`${label} ${name} height ${box.height} < 260`);}
      assert.ok(m.pcb.y>=m.model.bottom-2,`${label} canvases must stack in a narrow pane`);
      assert.ok(m.inspector.y>=m.pcb.bottom-2,`${label} inspector must follow both canvases`);
      assert.ok(m.inspector.width>=m.pane-16,`${label} inspector must use the pane width`);
      assert.ok(m.inspector.width<=m.pane+1&&m.viewport.scrollWidth<=m.viewport.width+2,`${label} layout overflows horizontally`);
      assert.equal(m.scope,'t1');measurements.push({label,...m});record('narrow pane actual canvas and inspector dimensions',{label,...m});
    };
    let stable=await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state);
    assert.equal(stable.selection.id,selected.id);assert.equal(stable.selection.sourceRevision,sourceRevision);
    await assertFrameRevealsCanvas('desktop 1360 with split and inspector');
    stable=await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state);
    await measurePane('desktop 1360 with split and inspector');
    await page.screenshot({path:join(output,'pane-desktop.png')});
    for(const viewport of [{width:900,height:900},{width:390,height:844}]){
      await page.setViewportSize(viewport);await waitAlignment();
      const state=await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state);
      assert.deepEqual(state,stable,'pane resize changed camera or selected source state');
      assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
      await assertFrameRevealsCanvas(`viewport ${viewport.width}`);await measurePane(`viewport ${viewport.width}`);
      stable=await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state);
      assert.equal(stable.selection.id,selected.id);assert.equal(stable.selection.sourceRevision,sourceRevision);
    }
    await page.locator('#solid').scrollIntoViewIfNeeded();
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const beforeTouch=await page.evaluate(()=>window.__viewerAcceptance.matrices.t1),box=await page.locator('#solid').boundingBox();
    const x=box.x+box.width/2,y=box.y+box.height/2;
    assert.ok(await page.evaluate(({x,y})=>document.elementFromPoint(x,y)?.id==='solid',{x,y}),'pane touch point must hit the actual unobstructed 3D canvas');
    const touch=await context.newCDPSession(page);
    for(const [type,dx]of [['touchStart',0],['touchMove',12],['touchMove',28],['touchEnd',28]])await touch.send('Input.dispatchTouchEvent',{type,touchPoints:type==='touchEnd'?[]:[{x:x+dx,y,id:1}]});
    await page.waitForFunction(before=>window.__viewerAcceptance.matrices.t1.some((v,i)=>Math.abs(v-before[i])>1e-5),beforeTouch);await touch.detach();
    const beforeBounds=await probeOriginal('bounds',{id:selected.id});assert.ok(beforeBounds&&!beforeBounds.error);
    const boundsCount=await page.evaluate(()=>window.__viewerAcceptance.workerBounds.length);
    await bar.getByRole('button',{name:'Frame part',exact:true}).click();
    await page.waitForFunction(count=>window.__viewerAcceptance.workerBounds.length>count,boundsCount);
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const afterBounds=await probeOriginal('bounds',{id:selected.id});assert.deepEqual(afterBounds,beforeBounds,'pane touch/framing changed source model bounds');
    assert.equal(afterBounds.revision,sourceRevision);assert.equal(afterBounds.source,pcb);
    const matrix=await page.evaluate(()=>window.__viewerAcceptance.matrices.t1);
    for(const x of [afterBounds.min[0],afterBounds.max[0]])for(const y of [afterBounds.min[1],afterBounds.max[1]])for(const z of [afterBounds.min[2],afterBounds.max[2]]){
      const p=[x*100,-y*100,z*100,1],clip=Array.from({length:4},(_,i)=>p.reduce((sum,v,j)=>sum+matrix[j*4+i]*v,0));
      assert.ok(clip[3]>0&&Math.abs(clip[0]/clip[3])<=1.001&&Math.abs(clip[1]/clip[3])<=1.001,'Frame part clips selected source body after narrow-pane touch');
    }
    assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
    const state=await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state);assert.equal(state.selection.id,selected.id);assert.equal(state.selection.sourceRevision,sourceRevision);
    assert.deepEqual(errors,[]);assert.ok(network.every(x=>!x.url.includes(token)));
    await page.locator('#solid').scrollIntoViewIfNeeded();await page.screenshot({path:join(output,'pane-touch.png')});
    record('focused pane layout retains source selection and fits actual body after touch',{measurements,frameReveals,source:pcb,sourceRevision,reference:selected.reference,bounds:afterBounds});
    if(expectAssetPreparation){
      // The sidebar is intentionally hidden at the preceding touch viewport.
      // Reopen uses visible desktop navigation, as the full UI gate does.
      await page.setViewportSize({width:1360,height:900});await waitAlignment();
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const saved=await page.evaluate(()=>({camera:window.__viewerAcceptance.workerCameras.at(-1)?.state,matrix:window.__viewerAcceptance.matrices.t1,terminations:window.__viewerAcceptance.workerTerminations}));
      let timer;
      const preparing=new Promise((resolve,reject)=>{
        timer=setTimeout(()=>reject(Error('t2 never prepared an original asset')),60000);
        onBrowserRequest=request=>{if(request.m==='kicad.renderAsset'&&request.p?.thread==='t2'){clearTimeout(timer);onBrowserRequest=null;resolve({id:request.id,revision:request.p.revision});}};
      });
      try{
        await page.locator('[data-on-click="select"][data-value="t2"]').first().click();await choose3d();
        const preparation=await preparing;
        const pendingAtSwitch=!browserRpc.some(row=>row.direction==='reply'&&row.id===preparation.id);
        await page.locator('[data-on-click="select"][data-value="t1"]').first().click();
        await page.waitForFunction(()=>document.querySelector('#solid-workbench')?.dataset.scope==='t1');
        await waitOriginal('raster');await waitAlignment();
        const restored=await probeOriginal('status');
        assert.equal(restored.source,pcb);assert.equal(restored.revision,sourceRevision);assert.equal(restored.traceAllocated,false);
        assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
        await page.waitForFunction(expected=>window.__viewerAcceptance.matrices.t1?.every((x,i)=>Math.abs(x-expected[i])<1e-6),saved.matrix);
        const camera=await page.evaluate(()=>window.__viewerAcceptance.workerCameras.at(-1)?.state);
        assert.deepEqual(camera,saved.camera,'rapid reopen changed the selected source camera');
        assert.ok(await page.evaluate(before=>window.__viewerAcceptance.workerTerminations>before,saved.terminations),'disposing the old viewer did not terminate its worker');
        assert.deepEqual(errors,[]);assert.ok(!/Viewer reads are busy|failed|timed out/i.test(await page.locator('.solid-render-status').textContent()));
        record('focused rapid reopen during original asset preparation restores same source selection and camera',{preparation,pendingAtSwitch,restored,scope:'actual normal t1→t2 asset preparation→t1; controller cancellation regression separately covers deterministic abort/late-rejection ownership'});
      }finally{clearTimeout(timer);onBrowserRequest=null;}
    }
  }else{
  await bar.locator('summary').filter({hasText:'Camera'}).click();
  if(qualityMode==='required'){
    assert.equal(await page.getByLabel('Illustrative solder',{exact:true}).isEnabled(),true,'aligned Studio must enable the illustrative solder control');
    assert.equal(await page.getByLabel('Illustrative solder',{exact:true}).isChecked(),false,'illustrative geometry must start off');
  }
  await page.getByLabel('Depth of field aperture',{exact:true}).selectOption('57');
  await page.getByRole('button',{name:'Lock focus',exact:true}).click();
  assert.match(await page.getByLabel('Focus distance',{exact:true}).textContent(),/locked/);
  await page.getByLabel('Projection',{exact:true}).selectOption('orthographic');
  await page.getByLabel('Exposure',{exact:true}).evaluate(input=>{input.value='0.7';input.dispatchEvent(new Event('input',{bubbles:true}));});
  await page.locator('#solid').focus();await page.keyboard.press('ArrowLeft');await page.keyboard.press('Shift+ArrowUp');
  await page.waitForFunction(()=>window.__viewerAcceptance.matrices.t1?.length===16);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const materialFinish=bar.getByLabel('Material finish',{exact:true});
  let materialRoundtripRoles;
  const checkMaterialRoles=(result,policy)=>{
    assert.ok(['realistic','cad-studio','source-defaults'].every(p=>result.materialPolicies?.includes(p)),'worker material capability list incomplete');
    const roles=result.materialRoleCounts;assert.ok(roles&&typeof roles==='object','actual worker omitted material roles');
    const known=new Set(['soldermask','silkscreen','pad','copper','substrate','unclassified','ambiguous']);
    assert.ok(Object.entries(roles).every(([role,count])=>known.has(role)&&Number.isSafeInteger(count)&&count>=0),'invalid original-material role counts');
    const total=Object.values(roles).reduce((sum,n)=>sum+n,0);assert.ok(total>0,'no original material classified');
    if(Number.isSafeInteger(result.materials))assert.equal(total,result.materials,'roles must count original materials, not component instances');
    if(policy==='realistic')for(const role of ['soldermask','silkscreen','pad','substrate'])assert.ok(roles[role]>0,`real SMU original material role missing: ${role}`);
    for(const key of ['explicitPresentationOverrideCount','presentationOverrideCount','missingFactorCount','estimatedFactorCount','unmappedMaterialCount'])assert.ok(Number.isSafeInteger(result[key])&&result[key]>=0,`invalid worker material counter ${key}`);
    assert.ok(result.explicitPresentationOverrideCount<=result.presentationOverrideCount);
    assert.ok(result.estimatedFactorCount<=result.missingFactorCount);
    assert.ok(Array.isArray(result.materialWarnings)&&result.materialWarnings.every(w=>typeof w==='string'));
    return roles;
  };
  if(qualityMode==='required'&&await materialFinish.count()){
    const supported=await materialFinish.locator('option[value="realistic"]').isEnabled();
    if(expectedMaterial==='realistic')assert.ok(supported,'Realistic must be advertised by the actual renderer');
    if(supported){
      const before=await page.evaluate(()=>({matrix:window.__viewerAcceptance.matrices.t1,camera:window.__viewerAcceptance.workerCameras.at(-1)?.state}));
      const bounds=await probeOriginal('bounds',{id:selected.id});assert.ok(bounds&&!bounds.error,'selected source body bounds unavailable before material changes');
      for(const policy of ['realistic','source-defaults','realistic']){
        const count=await page.evaluate(()=>window.__viewerAcceptance.workerMaterials.length);
        await materialFinish.selectOption(policy);
        await page.waitForFunction(({count,policy})=>{const rows=window.__viewerAcceptance.workerMaterials;return rows.length>count&&rows.at(-1).policy===policy;},{count,policy});
        const actual=await page.evaluate(()=>window.__viewerAcceptance.workerMaterials.at(-1));
        assert.ok(actual.result&&!actual.error,JSON.stringify(actual));assert.equal(actual.result.materialPolicy,policy);
        assert.equal(actual.result.source,pcb);assert.equal(actual.result.revision,sourceRevision);
        const roles=checkMaterialRoles(actual.result,policy);
        if(policy==='realistic'){if(materialRoundtripRoles)assert.deepEqual(roles,materialRoundtripRoles,'material roles changed after reversible policy roundtrip');else materialRoundtripRoles=roles;}
        assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
        const after=await page.evaluate(()=>({matrix:window.__viewerAcceptance.matrices.t1,camera:window.__viewerAcceptance.workerCameras.at(-1)?.state}));
        assert.deepEqual(after,before,'material finish moved the camera or changed selection');
        assert.equal(after.camera.selection.sourceRevision,sourceRevision);
        record('normal UI actual worker material policy retains source revision and selected camera',{policy,result:actual.result});
      }
      assert.deepEqual(await probeOriginal('bounds',{id:selected.id}),bounds,'material roundtrip changed source body bounds');
    }else record('Realistic renderer policy not advertised',{expectedMaterial});
  }else if(expectedMaterial)assert.fail('expected Material finish control is unavailable');
  await page.screenshot({path:join(output,'normal-split-components.png')});

  await bar.getByRole('button',{name:'Reflow',exact:true}).click();
  await page.locator('.reflow-graphs svg').first().waitFor();assert.equal(await page.locator('.reflow-graphs svg').count(),2);
  await page.locator('.reflow-editor summary').click();
  for(let n=await page.locator('.reflow-point').count();n>2;n--)await page.locator('.reflow-point').last().getByRole('button').click();
  const fields=page.locator('.reflow-point input');for(const [i,value]of ['0','25','20','250'].entries())await fields.nth(i).fill(value);
  await page.locator('.solid-inspector').getByRole('button',{name:'Apply',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.reflow-scrub')?.max==='20');
  await page.locator('.reflow-scrub').evaluate(input=>{input.value='20';input.dispatchEvent(new Event('input',{bubbles:true}));});
  const readings=await page.locator('.reflow-readings dd').allTextContents();
  const expectedBoard=25+11.25*(20-23*(1-(23/24)**20));
  assert.ok(readings[0].includes('250.0'));assert.ok(Math.abs(parseFloat(readings[1])-expectedBoard)<.15,JSON.stringify({readings,expectedBoard}));assert.ok(readings[2].includes('0.0%'));
  await fields.nth(2).fill('0');await page.locator('.solid-inspector').getByRole('button',{name:'Apply',exact:true}).click();
  assert.ok(await page.locator('.solid-error').textContent(),'invalid time ordering was accepted');
  await fields.nth(2).fill('20');await page.locator('.solid-inspector').getByRole('button',{name:'Apply',exact:true}).click();
  await page.screenshot({path:join(output,'normal-reflow.png')});
  await bar.getByRole('button',{name:'Reflow',exact:true}).click();
  record('normal UI linked shared reflow planner, graphs, numerical endpoint and invalid-profile rejection', {readings,expectedBoard});

  const vendors=['viewer.js','bvh-worker.js','render-worker.js'];const haveVendors=vendors.every(name=>fs.existsSync(join(web,'viewer/vendor',name)));
  if(qualityMode==='required')assert.ok(haveVendors,'quality vendor assets missing');
  if(qualityMode!=='off'&&haveVendors) {
    let status,active;
    if(expectedTrace==='unavailable'){
      const fallback=await probeOriginal('status');
      assert.equal(fallback.pathTraceAvailable,false);assert.ok(typeof fallback.pathTraceReason==='string'&&fallback.pathTraceReason.trim().length>0,'unavailable tracing needs a reason');
      assert.equal(fallback.phase,'raster');assert.equal(fallback.visible,true);assert.equal(fallback.samples,0);
      assert.equal(fallback.traceAllocated,false);assert.equal(fallback.bvhBuilding,false);assert.equal(fallback.bvhReady,false);
      assert.equal(fallback.source,pcb);assert.equal(fallback.revision,sourceRevision);
      assert.equal(await renderMode.inputValue(),'studio');
      assert.equal(await renderMode.locator('option[value="pathtrace"]').evaluate(option=>option.disabled),true,'native Path trace option must be disabled');
      status=await page.locator('.solid-render-status').textContent();active=true;
      record('normal UI unavailable path tracing keeps visible Studio; fallback only',{status,worker:fallback});
    }else{
    assert.equal(await renderMode.locator('option[value="pathtrace"]').evaluate(option=>option.disabled),false,'available Path trace option is disabled');
    await renderMode.selectOption('pathtrace');
    await bar.getByLabel('Path tracing sample limit',{exact:true}).selectOption('32');
    await page.waitForFunction(revision=>{
      const root=document.querySelector('#solid-workbench'),note=document.querySelector('.solid-render-status')?.textContent||'';
      return /unavailable|Preview available|failed|timed out/i.test(note)||root?.classList.contains('solid-original-visible')&&window.__viewerAcceptance.workerStatus.some(s=>s.revision===revision&&s.key===root.dataset.key&&s.visible&&s.samples>0&&['pathtrace','paused','converged'].includes(s.phase));
    },sourceRevision,{timeout:180000});
    status=await page.locator('.solid-render-status').textContent();
    active=await page.locator('#solid-workbench').evaluate(root=>root.classList.contains('solid-original-visible'));
    if(qualityMode==='required')assert.ok(active&&!/unavailable|failed|timed out|Preview available/i.test(status),status);
    let traceTiming;
    if(active){
      assert.ok(workerURLs.some(url=>url.includes('render-worker')));await page.locator('#solid').focus();
      const inputAt=performance.now();await page.keyboard.press('ArrowRight');const keyDispatchMs=performance.now()-inputAt;
      traceTiming=await page.evaluate(()=>new Promise(resolve=>{
        const worker=window.__viewerAcceptanceWorker,started=performance.now(),id=-1000000;
        const timing={frames:[],workerAckMs:null,workerStatus:null,workerProbeSent:true,visibility:document.visibilityState};let done=false;
        const finish=reason=>{if(done)return;done=true;clearTimeout(timer);worker.removeEventListener('message',reply);timing.reason=reason;timing.elapsedMs=performance.now()-started;window.__viewerAcceptance.traceTiming=timing;resolve(timing);};
        const reply=({data})=>{if(data.id!==id)return;timing.workerAckMs=performance.now()-started;timing.workerStatus=data.result;timing.workerError=data.error;if(timing.frames.length>=3)finish('frames and worker reply');};
        worker.addEventListener('message',reply);const timer=setTimeout(()=>finish('5 s diagnostic deadline'),5000);
        worker.postMessage({id,method:'status',args:{}});
        const frame=()=>{if(done)return;timing.frames.push(performance.now()-started);if(timing.frames.length>=3&&timing.workerAckMs!==null)finish('frames and worker reply');else requestAnimationFrame(frame);};requestAnimationFrame(frame);
      }));
      traceTiming.keyDispatchMs=keyDispatchMs;
    }
    record('normal UI path trace has real same-revision samples; visual quality unqualified',{status,active,traceTiming,lastStatus:await page.evaluate(()=>window.__viewerAcceptance.workerStatus.at(-1))});
    if(gpuMode==='hardware'){
      assert.ok(Number.isFinite(traceTiming?.workerAckMs)&&!traceTiming?.workerError,'hardware trace input did not receive an actual worker reply');
      assert.ok(traceTiming.frames.length>=3,'hardware trace input blocked animation frames');
      assert.equal(traceTiming.workerStatus?.pathTraceAvailable,true);assert.equal(traceTiming.workerStatus?.source,pcb);assert.equal(traceTiming.workerStatus?.revision,sourceRevision);
      assert.match(traceTiming.workerStatus?.unmaskedRenderer||'',/RTX\s*5080/i,'active trace worker lost qualified hardware identity');
    }
    }
    const terminations=await page.evaluate(()=>window.__viewerAcceptance.workerTerminations);
    const fastAt=performance.now();
    await renderMode.selectOption('fast');
    const fastSelectMs=performance.now()-fastAt;
    await page.waitForFunction(()=>!document.querySelector('.solid-pathtrace'));
    const fastCanvasRemovedMs=performance.now()-fastAt;
    assert.equal(await page.locator('#solid-workbench').evaluate(root=>root.classList.contains('solid-original-visible')),false);
    assert.ok(await page.evaluate(before=>window.__viewerAcceptance.workerTerminations>before,terminations),'Fast did not terminate the original-model worker');
    assert.equal(await page.getByLabel('Illustrative solder',{exact:true}).isEnabled(),false,'Fast must disable the original-model-only solder control');
    await page.waitForFunction(ref=>document.querySelector('.component-selection-overlay')?.textContent.includes(ref),selected.reference);
    record(expectedTrace==='available'?'normal UI Studio to Path trace to Fast worker lifecycle':'normal UI Studio fallback to Fast without path tracing', {active,status,workerURLs,fastSelectMs,fastCanvasRemovedMs,fastOverlayReadyMs:performance.now()-fastAt});
    await page.screenshot({path:join(output,'normal-fast.png')});
  } else record('quality renderer not exercised', {qualityMode,haveVendors});
  await page.setViewportSize({width:390,height:844});
  if(await bar.locator('.solid-camera').getAttribute('open')!==null)await bar.locator('.solid-camera > summary').click();
  await page.waitForFunction(()=>document.documentElement.scrollWidth<=innerWidth+2);
  assert.ok(await page.evaluate(()=>matchMedia('(prefers-reduced-motion: reduce)').matches));
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const beforeTouch=await page.evaluate(()=>window.__viewerAcceptance.matrices.t1);
  const box=await page.locator('#solid').boundingBox(), touch=await context.newCDPSession(page);
  assert.ok(box?.width>0&&box.height>0,'compact 3D canvas is empty');
  const x=box.x+box.width/2,y=box.y+Math.min(box.height/2,100);
  assert.ok(await page.evaluate(({x,y})=>document.elementFromPoint(x,y)?.id==='solid',{x,y}),'touch must start on the unobstructed canvas');
  for(const [type,dx]of [['touchStart',0],['touchMove',12],['touchMove',28],['touchEnd',28]])await touch.send('Input.dispatchTouchEvent',{type,touchPoints:type==='touchEnd'?[]:[{x:x+dx,y,id:1}]});
  await page.waitForFunction(before=>{const m=window.__viewerAcceptance.matrices.t1;return m&&m.every(Number.isFinite)&&m.some((x,i)=>Math.abs(x-before[i])>1e-5);},beforeTouch);
  await touch.detach();
  const browserReceipt=await page.evaluate(()=>({draws:window.__viewerAcceptance.draws,cameraEvents:window.__viewerAcceptance.events,blankIntervals:window.__viewerAcceptance.blankIntervals,workerStatus:window.__viewerAcceptance.workerStatus,workerLoads:window.__viewerAcceptance.workerLoads,workerBounds:window.__viewerAcceptance.workerBounds,workerSelections:window.__viewerAcceptance.workerSelections,workerMaterials:window.__viewerAcceptance.workerMaterials,workerTerminations:window.__viewerAcceptance.workerTerminations}));
  assert.deepEqual(browserReceipt.blankIntervals,[],'normal UI left a new model blank without a loading explanation');
  assert.deepEqual(errors,[],'normal UI/control exceptions');
  assert.ok(network.every(x=>!x.url.includes(token)),'browser transport URL leaked pairing material');
  record('normal UI compact viewport and truthful loading state', browserReceipt);
  await page.screenshot({path:join(output,'normal-compact.png')});

  // Keep rapid reopen mandatory, after independent controls, so its failure
  // cannot hide useful evidence from the rest of this historical candidate.
  await page.setViewportSize({width:1360,height:900});
  if(await bar.getByRole('button',{name:'Components',exact:true}).getAttribute('aria-expanded')!=='true')await bar.getByRole('button',{name:'Components',exact:true}).click();
  if(qualityMode==='required'){await renderMode.selectOption('studio');await waitOriginal('raster');}
  await waitAlignment();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const poseA=await page.evaluate(()=>window.__viewerAcceptance.matrices.t1);
  await page.locator('[data-on-click="select"][data-value="t2"]').first().click();await choose3d();
  await page.waitForFunction(()=>window.__viewerAcceptance.draws.t2>0&&document.querySelector('#solid-workbench')?.dataset.scope==='t2');
  await bar.getByRole('button',{name:'Top',exact:true}).click();
  await page.locator('[data-on-click="select"][data-value="t1"]').first().click();
  await page.waitForFunction(()=>document.querySelector('#solid-workbench')?.dataset.scope==='t1'&&window.__viewerAcceptance.draws.t1>1);
  await waitAlignment();
  assert.equal(await page.getByLabel('Projection',{exact:true}).inputValue(),'orthographic');
  assert.equal(Number(await page.getByLabel('Exposure',{exact:true}).inputValue()),.7);
  assert.equal(await page.getByLabel('Depth of field aperture',{exact:true}).inputValue(),'57');
  if(materialRoundtripRoles){
    assert.equal(await materialFinish.inputValue(),'realistic','thread reopen lost material finish');
    const restoredMaterial=await probeOriginal('status');assert.equal(restoredMaterial.materialPolicy,'realistic');assert.equal(restoredMaterial.source,pcb);assert.equal(restoredMaterial.revision,sourceRevision);assert.deepEqual(checkMaterialRoles(restoredMaterial,'realistic'),materialRoundtripRoles);
  }
  assert.equal(await page.locator('.component-bom tr[aria-selected="true"]').getAttribute('data-component'),selected.id);
  assert.ok(await page.locator('#solid-workbench').evaluate(root=>root.classList.contains('solid-split')));
  await page.waitForFunction(expected=>{const m=window.__viewerAcceptance.matrices.t1;return m&&m.every((x,i)=>Math.abs(x-expected[i])<1e-6);},poseA);
  assert.deepEqual(errors,[],'normal UI/control exceptions after reopen');
  record('normal UI real 3D/PCB, source component selection, two-thread camera/state restoration', {reference:selected.reference,uuid:selected.id,pads:selected.pads.length});
  }
  }
  }
  const eventsAfter=fs.readFileSync(join(home,'events.jsonl'),'utf8');assert.ok(!/"\$":"TurnChanged"[^\n]*"running"/.test(eventsAfter),'fixture launched an agent turn');
  for(const input of sourceInputs){assert.equal(digest(fs.readFileSync(input.path)),input.sha256,`readonly input changed: ${input.path}`);assert.equal(revision(input.path),input.revision,`readonly input revision changed: ${input.path}`);}
  if(geometryReceiptPath)assert.equal(digest(fs.readFileSync(geometryReceiptPath)),geometryReceiptSha256,'referenced geometry proof changed during qualification');
  if(maskOnly){const path=join(web,'viewer/vendor/render-worker.js');assert.equal(fs.statSync(path).size,maskWorker.bytes);assert.equal(digest(fs.readFileSync(path)),maskWorker.sha256,'frozen mask worker changed during qualification');}
  if(coatingOnly){const path=join(web,'viewer/vendor/render-worker.js');assert.equal(fs.statSync(path).size,coatingWorker.bytes);assert.equal(digest(fs.readFileSync(path)),coatingWorker.sha256,'frozen coating worker changed during qualification');assert.equal(digest(fs.readFileSync(binary)),option('expected-bin-sha'),'combined service changed during qualification');}
  const artifacts={binarySha256:digest(fs.readFileSync(binary)),indexSha256:digest(fs.readFileSync(join(web,'index.html'))),wireSha256:digest(fs.readFileSync(join(sourceRoot,'src/web/wire.js'))),viewerRpcSha256:digest(fs.readFileSync(join(sourceRoot,'src/web/viewer-rpc.js'))),cborSha256:digest(fs.readFileSync(join(sourceRoot,'src/core/cbor.bend')))};
  const receipt={scope:coatingOnly?'focused normal toolbar32 coating trace and natural cached restoration only':maskOnly?'focused original-source mask relief raster lifecycle only':restoreOnly?'focused natural cached painted viewer restoration only':fidelityOnly?'focused delivered full-geometry raster fidelity and lifecycle only':renderOnly?'focused normal toolbar HDR renderer lifecycle only':loadOnly?'focused initial original-model loading profile':layoutOnly?'focused narrow-pane layout only':apiOnly?'HTTP asset API only':'normal production UI and HTTP asset API',expectations:{gpu:gpuMode,trace:fidelityOnly||restoreOnly||maskOnly?'not exercised':expectedTrace,material:expectedMaterial||null,renderMaterial:renderMaterial||null},binary,web,sourceRoot,project,fixtureProject,sourceInputs,artifacts,asset,gpuEvidence,...(maskOnly?{maskWorker}:{}),...(coatingOnly?{coatingWorker}:{}),...(fidelityOnly?{geometryReceipt:{path:geometryReceiptPath,sha256:geometryReceiptSha256},fidelityInventory}:{}),facts,http,network,workerURLs,browserRpc,elapsedMs:performance.now()-began};
  fs.writeFileSync(join(output,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify({state:'passed',scope:receipt.scope,evidence:output,assetSha256:asset?.sha256,facts:facts.map(x=>x.name)}));
} catch(error) {
  const sourceAfter=sourceInputs.map(input=>({path:input.path,sha256:digest(fs.readFileSync(input.path)),revision:revision(input.path)}));
  let ui;
  if(page&&!page.isClosed())try{
    ui={url:page.url().replaceAll(token,'[redacted]'),text:(await page.locator('body').innerText()).replaceAll(token,'[redacted]').slice(0,12000),observations:await page.evaluate(()=>window.__viewerAcceptance)};
    await page.screenshot({path:join(output,'failure.png')});
  }catch{}
  fs.writeFileSync(join(output,'failure.json'),JSON.stringify({error:String(error.stack||error),expectations:{gpu:gpuMode,trace:expectedTrace,material:expectedMaterial||null},facts,errors,http,network,workerURLs,browserRpc,gpuEvidence,sourceInputs,sourceAfter,ui},null,2)+'\n');
  throw error;
} finally {
  viewerRpc?.reset();
  for(const call of pending.values()){clearTimeout(call.timer);call.reject(Error('test shutdown'));}pending.clear();
  socket?.close();await browser?.close();if(proc.exitCode===null)proc.kill('SIGTERM');await proc.exited;
  // Evidence is retained on failure or when explicitly requested. Never copy
  // the synthetic pairing token into the receipt or outside the fixture home.
  if(!args.includes('--keep')&&facts.some(x=>x.name==='normal UI compact viewport and truthful loading state')&&!output.startsWith(dir))fs.rmSync(dir,{recursive:true,force:true});
}
