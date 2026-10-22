'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const p=require('../produce-release.cjs');
test('Windows shell successor rejects arbitrary source, target, release and payload overrides',()=>{
 const source={...p.WINDOWS_SHELL_SOURCE},release={version:'0.12.1',channel:'release',commit:'8147b68dbc9bcddad7e3b51ae7768ed9eb211ecc',tree:'37c64023d084bca7f209db9d14a84f59d365f696'};
 const receipt={schema:1,kind:'backplane-electron-shell',status:'VERIFIED',target:'windows-x64',version:'0.12.1',channel:'release',sourceCommit:source.commit,sourceTree:source.tree,sourceFiles:{'desktop/service.cjs':source.serviceSha256},members:{'service.cjs':{type:'file',sha256:source.serviceSha256}}};
 const check=(r=receipt,s=source,t='windows-x64',base=release)=>p.shellIdentity(r,base,t,{windowsShellSource:s});
 assert.doesNotThrow(()=>check());
 for(const target of ['linux-x64','linux-arm64','darwin-arm64'])assert.throws(()=>check(receipt,source,target),/release\/target/);
 for(const key of ['commit','tree','serviceSha256'])assert.throws(()=>check(receipt,{...source,[key]:'0'.repeat(source[key].length)}),/Unapproved/);
 assert.throws(()=>check(receipt,{...source,extra:true}),/Unapproved/);
 assert.throws(()=>check({...receipt,sourceCommit:release.commit}),/Component source/);
 assert.throws(()=>check({...receipt,sourceFiles:{'desktop/service.cjs':'0'.repeat(64)}}),/source\/payload/);
 assert.throws(()=>check({...receipt,members:{'service.cjs':{type:'symlink',sha256:source.serviceSha256}}}),/source\/payload/);
 assert.throws(()=>check(receipt,source,'windows-x64',{...release,commit:'0'.repeat(40)}),/release\/target/);
 assert.throws(()=>p.shellIdentity(receipt,release,'windows-x64',{}),/Component source/);
});
