// Assemble a reviewable local stage. This script never installs or edits a home.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const [service, web, version, output] = process.argv.slice(2);
for (const value of [service, web, output]) assert(value && path.isAbsolute(value), 'Use absolute service, web and output paths');
assert(version && /^[\w.+-]+$/.test(version), 'Provide the exact audited service version');
assert(!fs.existsSync(output), 'Stage output must be new');
assert.equal(path.basename(service), 'backplane-serve');
assert(fs.statSync(service).isFile());
const electronDist = path.join(__dirname, 'node_modules/electron/dist');
const electronVersion = require('./node_modules/electron/package.json').version;
assert.equal(electronVersion, '44.9.2');
fs.mkdirSync(output, {recursive:true});
fs.cpSync(electronDist, path.join(output,'runtime'), {recursive:true});
// Electron's packaged-state check uses the executable basename.
fs.renameSync(path.join(output,'runtime/electron'),path.join(output,'runtime/backplane-desktop'));
const resourceRoot=path.join(output,'runtime/resources/backplane-service');
const appRoot=path.join(output,'runtime/resources/app');
fs.mkdirSync(resourceRoot,{recursive:true});fs.mkdirSync(appRoot,{recursive:true});
const appFiles=['package.json','main.cjs','preload.cjs','service.cjs','startup.html','startup.css','startup.js'];
const directBrowser=process.env.BACKPLANE_DESKTOP_BROWSER==='1';
if(directBrowser)appFiles.push('browser-controller.cjs','browser-actions.cjs','browser-dom.cjs','browser-channel.cjs','browser-envelope.cjs','browser-wire.cjs','browser-leases.cjs','browser-documents.cjs','browser-dispatch.cjs','browser-guest.cjs','browser-recording-preload.cjs','browser-surface.cjs','browser-shield.html');
for(const file of appFiles) fs.copyFileSync(path.join(__dirname,file),path.join(appRoot,file));
if(directBrowser){
  const wsRoot=path.dirname(require.resolve('ws/package.json'));assert.equal(require(path.join(wsRoot,'package.json')).version,'8.22.0');
  const wsOut=path.join(appRoot,'node_modules/ws');fs.mkdirSync(wsOut,{recursive:true});
  for(const file of ['package.json','index.js','browser.js','wrapper.mjs','LICENSE'])fs.copyFileSync(path.join(wsRoot,file),path.join(wsOut,file));
  fs.cpSync(path.join(wsRoot,'lib'),path.join(wsOut,'lib'),{recursive:true});
}
fs.copyFileSync(service,path.join(resourceRoot,'backplane-serve'));
fs.chmodSync(path.join(resourceRoot,'backplane-serve'),0o755);
// Follow only the emitted index's asset graph. Never package old build chunks,
// homes, source trees, user fonts, caches, experiments, or node_modules.
const webRoot=path.join(resourceRoot,'web');fs.mkdirSync(webRoot);
const {packageWebAssets}=require('./web-assets.cjs');
const seen=packageWebAssets(web,webRoot);
const license=path.resolve(__dirname,'../LICENSE');if(fs.existsSync(license))fs.copyFileSync(license,path.join(resourceRoot,'LICENSE.Backplane'));
const build={serviceVersion:version,electronVersion,sourceTree:process.env.BACKPLANE_SOURCE_TREE||null,qualification:'local Linux review stage; root alone installs; Windows/macOS unqualified'};
if(directBrowser){
  const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(path.join(appRoot,file))).digest('hex');
  build.desktopBrowser={protocol:1,guestManagerSha256:sha('browser-guest.cjs'),preloadSha256:sha('browser-recording-preload.cjs'),recorderSourceSha256:'2e4b03765cc4ae6595e32048555ffe27e750994a85c18b89027064f3c0c15350',subframePreload:true,popupPreloadVerified:false};
}
fs.writeFileSync(path.join(resourceRoot,'BUILD.json'),JSON.stringify(build,null,2)+'\n');
// BACKPLANE_DESKTOP_RESIDENT=1: the app attaches to the resident hub of its
// home and never owns it (no stop, no update), as a release made by
// produce-release.cjs with a resident lifecycle policy: the same
// runtime/resources/DESKTOP-BUILD.json fields, which main.cjs reads first.
if(process.env.BACKPLANE_DESKTOP_RESIDENT==='1'){
  const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(path.join(appRoot,file))).digest('hex');
  const desktop={serviceVersion:version,electronVersion,sourceTree:process.env.BACKPLANE_SOURCE_TREE||null,sourceCommit:process.env.BACKPLANE_SOURCE_COMMIT||null,
    lifecyclePolicy:{mode:'resident-service',managedStop:false,managedUpdate:false},
    qualification:'local Linux stage (stage-linux.cjs); resident service; unsigned'};
  if(directBrowser)desktop.desktopBrowser={protocol:1,guestManagerSha256:sha('browser-guest.cjs'),preloadSha256:sha('browser-recording-preload.cjs')};
  fs.writeFileSync(path.join(output,'runtime/resources/DESKTOP-BUILD.json'),JSON.stringify(desktop,null,2)+'\n');
}
fs.writeFileSync(path.join(output,'backplane-electron'),'#!/bin/sh\nset -eu\n: "${BACKPLANE_HOME:?Set the explicit absolute Backplane home selected for this stage}"\nBACKPLANE_STAGE_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$BACKPLANE_STAGE_ROOT/runtime/backplane-desktop" "$@"\n',{mode:0o755});
const hashes={};
function inventory(dir){for(const entry of fs.readdirSync(dir,{withFileTypes:true})){const file=path.join(dir,entry.name);if(entry.isDirectory())inventory(file);else if(entry.isSymbolicLink())throw Error('Stage symlinks are not supported');else{assert(!/\.(?:ttf|otf|woff2?)$/i.test(file),'No font files in stage');hashes[path.relative(output,file)]=crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');}}}
inventory(output);
fs.writeFileSync(path.join(output,'MANIFEST.json'),JSON.stringify({build,files:hashes},null,2)+'\n');
console.log(JSON.stringify({stage:output,files:Object.keys(hashes).length,webAssets:seen.size,build}));
