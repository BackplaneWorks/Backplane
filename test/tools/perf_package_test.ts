// Real tiny tar/ar/SquashFS fixtures. No Backplane binary/build or acceptance run.
import assert from 'node:assert/strict';
import {createHash,generateKeyPairSync} from 'node:crypto';
import {chmodSync,copyFileSync,cpSync,mkdirSync,mkdtempSync,readFileSync,writeFileSync,rmSync,symlinkSync,linkSync,unlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {packageNames,verifyPackageContents,verifyPackageRecord,verifyPublication,releasePayload,verifyCompletedPayload} from '../../scripts/perf-package';
import {attestBuild} from '../../scripts/perf-signature';
import {sourceSha,harnessSha} from '../../scripts/perf-verify';
const tmp=mkdtempSync(join(tmpdir(),'bp-perf-package-')),root=join(tmp,'candidate'),keys=join(tmp,'keys');
const previous={...process.env};
function run(argv:string[]) {
  const p=Bun.spawnSync(argv,{cwd:root});assert.equal(p.exitCode,0,p.stderr.toString());return p.stdout.toString().trim();
}
function put(path:string,body:string) {mkdirSync(join(path,'..'),{recursive:true});writeFileSync(path,body);}
try {
  mkdirSync(root);mkdirSync(keys);
  run(['git','init','-q']);put(join(root,'.gitignore'),'build/\n');
  for(const [path,body] of Object.entries({'LICENSE':'synthetic license','deploy/backplane.desktop':'synthetic desktop entry',
    'assets/icon/backplane.svg':'synthetic svg','assets/icon/backplane-256.png':'synthetic png'})) put(join(root,path),body);
  run(['git','add','.']);
  run(['git','-c','user.name=i2cjak','-c','user.email=i2cjak@users.noreply.github.com','commit','-qm','synthetic package fixture']);
  const pair=generateKeyPairSync('ed25519');
  put(join(keys,'public'),pair.publicKey.export({type:'spki',format:'pem'}).toString());
  put(join(keys,'private'),pair.privateKey.export({type:'pkcs8',format:'pem'}).toString());
  process.env.BACKPLANE_PERF_TRUST_KEY=join(keys,'public');process.env.BACKPLANE_PERF_SIGNING_KEY=join(keys,'private');
  const dir=join(root,'build'),tag='v9.9.9-dev.1',target='linux-x64',body='synthetic package payload\n';mkdirSync(dir);
  const expected=createHash('sha256').update(body).digest('hex'),names=packageNames(tag,target);
  const tree=join(dir,'tree'),staging=join(dir,'staging'),app=join(staging,'AppDir'),deb=join(staging,'deb'),dist=join(dir,'dist');
  const files={'backplane':body,'backplane-serve':body,'web/app.js':'candidate web','web/app.js.gz':'candidate compressed web',
    'backplane-browser':'candidate browser','backplane-kipc':'candidate IPC','LICENSE':'synthetic license',
    'backplane.desktop':'synthetic desktop entry','icon/backplane.svg':'synthetic svg','icon/backplane-256.png':'synthetic png'};
  for(const [path,content] of Object.entries(files)) {
    for(const base of [dist,join(tree,`backplane-${target}`),join(app,'usr/lib/backplane'),join(deb,'opt/backplane')]) {
      put(join(base,path),content);
      if(['backplane','backplane-serve','backplane-browser','backplane-kipc'].includes(path)) chmodSync(join(base,path),0o755);
    }
  }
  put(join(app,'AppRun'),'synthetic launcher');chmodSync(join(app,'AppRun'),0o755);
  for(const name of ['backplane','backplane-serve']) {put(join(deb,'usr/bin',name),'synthetic wrapper');chmodSync(join(deb,'usr/bin',name),0o755);}
  put(join(staging,'control/control'),'Package: synthetic\n');put(join(dir,'debian-binary'),'2.0\n');
  const payload=releasePayload(root,dist);
  const verify=()=>verifyPackageContents(dir,tag,target,expected,payload,staging);
  const packTar=()=>run(['tar','-czf',join(dir,names[0]),'-C',tree,`backplane-${target}`]);packTar();
  const packApp=(source=app)=>{
    run(['mksquashfs',source,join(dir,'payload.squashfs'),'-noappend','-no-progress','-no-xattrs','-processors','1']);
    writeFileSync(join(dir,names[1]),Buffer.concat([Buffer.from('synthetic AppImage header\n'),readFileSync(join(dir,'payload.squashfs'))]));
  };packApp();
  const packDeb=(source=deb,control=join(staging,'control'))=>{
    run(['tar','-czf',join(dir,'data.tar.gz'),'-C',source,'.']);
    run(['tar','-czf',join(dir,'control.tar.gz'),'-C',control,'.']);
    run(['ar','rc',join(dir,names[2]),join(dir,'debian-binary'),join(dir,'control.tar.gz'),join(dir,'data.tar.gz')]);
  };packDeb();
  const packages=await verify();
  const commit=run(['git','rev-parse','HEAD']),digest=createHash('sha256').update('synthetic evidence fixture').digest('hex');
  const record={schema:1,kind:'native-performance-package',tag,target,packages,accepted_at:Date.now(),
    candidate:{commit,platform:'linux-native' as const,source_sha256:sourceSha(root),harness_sha256:harnessSha(root),
      binary_sha256:expected,compiler_sha256:digest,fixture_sha256:digest,host_sha256:digest},evidence_sha256:digest,payload_manifest_sha256:digest,
    measurement_started_at:Date.now()-10000,measurement_completed_at:Date.now()-1000,
    commit,tree:run(['git','rev-parse','HEAD^{tree}'])};
  const completed={schema:1,kind:'completed-dist-payload',candidate:record.candidate,entries:payload};
  writeFileSync(join(dist,'backplane.perf-dist.json'),JSON.stringify(attestBuild(root,completed)));
  verifyCompletedPayload(root,dist,record.candidate);
  put(join(dist,'web/app.js'),'late replaced dist web');
  assert.throws(()=>verifyCompletedPayload(root,dist,record.candidate),/finished dist differs/);put(join(dist,'web/app.js'),files['web/app.js']);
  const manifest=join(dir,`backplane-${tag}-${target}.perf-package.json`);
  const sign=(value:typeof record)=>writeFileSync(manifest,JSON.stringify(attestBuild(root,value)));
  sign(record);verifyPackageRecord(root,dir,tag,target);
  const publication=join(dir,'publication');mkdirSync(publication);
  for(const name of names) copyFileSync(join(dir,name),join(publication,name));
  copyFileSync(manifest,join(publication,`backplane-${tag}-${target}.perf-package.json`));
  const armNames=packageNames(tag,'linux-arm64'),armPackages:Record<string,string>={};
  for(let i=0;i<names.length;i++) {copyFileSync(join(dir,names[i]),join(publication,armNames[i]));armPackages[armNames[i]]=packages[names[i]];}
  writeFileSync(join(publication,`backplane-${tag}-linux-arm64.perf-package.json`),JSON.stringify(attestBuild(root,{...record,target:'linux-arm64',packages:armPackages})));
  put(join(publication,`backplane-${tag}-darwin-arm64.tar.gz`),'synthetic nongated mac fixture');
  put(join(publication,`backplane-${tag}-android.apk`),'synthetic nongated Android fixture');
  verifyPublication(root,publication,tag);
  put(join(publication,'unexpected.exe'),'must not publish beside valid receipts');
  assert.throws(()=>verifyPublication(root,publication,tag),/artifact set/);rmSync(join(publication,'unexpected.exe'));
  // The original executable and server copy both need exact payload equality.
  put(join(tree,`backplane-${target}/backplane-serve`),'post-gate overwrite');packTar();
  await assert.rejects(verify,/differs from completed-build payload/);
  assert.throws(()=>verifyPackageRecord(root,dir,tag,target),/changed after acceptance/);
  put(join(tree,`backplane-${target}/backplane-serve`),body);packTar();
  for(const path of ['web/app.js','backplane-browser']) {
    put(join(tree,`backplane-${target}`,path),'stale/wrong packaged member');packTar();
    await assert.rejects(verify,/differs from completed-build payload/);
    put(join(tree,`backplane-${target}`,path),files[path as keyof typeof files]);packTar();
  }
  const tarExe=join(tree,`backplane-${target}/backplane`);
  chmodSync(tarExe,0o644);packTar();await assert.rejects(verify,/content\/type\/mode/);chmodSync(tarExe,0o755);
  for(const link of ['symlink','hardlink']) {
    unlinkSync(tarExe);
    if(link==='symlink') symlinkSync('backplane-serve',tarExe);else linkSync(join(tree,`backplane-${target}/backplane-serve`),tarExe);
    packTar();await assert.rejects(verify,/regular package payload/);
    unlinkSync(tarExe);put(tarExe,body);chmodSync(tarExe,0o755);
  }packTar();
  const wrongApp=join(dir,'wrong-app');cpSync(app,wrongApp,{recursive:true});
  put(join(wrongApp,'usr/lib/backplane/web/app.js'),'wrong AppImage web');packApp(wrongApp);
  await assert.rejects(verify,/AppImage differs/);
  put(join(wrongApp,'usr/lib/backplane/web/app.js'),files['web/app.js']);
  chmodSync(join(wrongApp,'usr/lib/backplane/backplane'),0o644);packApp(wrongApp);
  await assert.rejects(verify,/AppImage differs/);packApp();
  const wrongDeb=join(dir,'wrong-deb');cpSync(deb,wrongDeb,{recursive:true});
  put(join(wrongDeb,'opt/backplane/backplane-browser'),'wrong deb helper');packDeb(wrongDeb);
  await assert.rejects(verify,/deb data.tar.gz differs/);
  put(join(wrongDeb,'opt/backplane/backplane-browser'),files['backplane-browser']);
  chmodSync(join(wrongDeb,'opt/backplane/backplane-serve'),0o644);packDeb(wrongDeb);
  await assert.rejects(verify,/deb data.tar.gz differs/);packDeb();
  put(join(staging,'control/postinst'),'unexpected maintainer script');packDeb();
  await assert.rejects(verify,/unexpected deb control\/scripts/);unlinkSync(join(staging,'control/postinst'));packDeb();
  const restored=await verify();
  sign({...record,packages:restored,accepted_at:Date.now()-25*3600*1000});
  assert.throws(()=>verifyPackageRecord(root,dir,tag,target),/stale/);
  sign({...record,packages:restored,measurement_started_at:Date.now()-25*3600*1000});
  assert.throws(()=>verifyPackageRecord(root,dir,tag,target),/stale/);
  sign({...record,packages:restored,commit:'f'.repeat(40)});
  assert.throws(()=>verifyPackageRecord(root,dir,tag,target),/wrong-candidate/);
  writeFileSync(manifest,JSON.stringify(record));assert.throws(()=>verifyPackageRecord(root,dir,tag,target),/signature/);
  console.log('PASS full tar/deb/AppImage payloads: stale web/helper, lost executable bits, symlink/hardlink, extra scripts and post-build dist/package drift reject');
} finally {
  for(const key of Object.keys(process.env)) if(!(key in previous)) delete process.env[key];
  Object.assign(process.env,previous);rmSync(tmp,{recursive:true,force:true});
}
