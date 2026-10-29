'use strict';
// Supplied, qualified web artifacts only. Never emits Bend, fetches or runs a payload.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const p=require('./produce-release.cjs');
const VERSION='0.13.1',GIT=/^[a-f0-9]{40}$/,HEX=/^[a-f0-9]{64}$/;
const check=(v,m)=>{if(!v)throw Error(m);};
function archive(requestFile,output){
 check(path.isAbsolute(output)&&!fs.existsSync(output),'Distribution output must be a new absolute directory');
 const request=p.json(requestFile),r=request.release;
 check(request.schema===1&&request.kind==='backplane-web-release-inputs'&&request.target==='web','Wrong web producer request');
 check(r&&r.version===VERSION&&r.tag==='v'+VERSION&&r.channel==='release'&&GIT.test(r.commit)&&GIT.test(r.tree),'Actual stable web freeze required');
 const freeze=p.readReference(r.freezeReceipt);
 check(freeze.schema===1&&freeze.kind==='backplane-release-freeze'&&freeze.status==='FROZEN'&&['version','tag','channel','commit','tree'].every(k=>freeze[k]===r[k])&&Number.isSafeInteger(freeze.sourceDateEpoch)&&freeze.sourceDateEpoch>=315532800&&freeze.sourceDateEpoch<=4354819199,'Web freeze identity mismatch');
 const temp=fs.mkdtempSync(path.join(os.tmpdir(),'bp-web-producer-'));
 try{
  const component=p.payload(request.web,temp,'web'),web=component.receipt,emission=p.readReference(request.emissionReceipt),source=p.readReference(emission.sourceQualification);
  check(freeze.components?.web?.webManifestSha256===component.ref.sha256&&freeze.components.web.emissionReceiptSha256===request.emissionReceipt.sha256,'Web input not bound to release freeze');
  check(emission.schema===1&&emission.kind==='backplane-web-emission'&&emission.status==='VERIFIED'&&emission.target==='web'&&emission.version===VERSION&&emission.channel==='release'&&emission.sourceCommit===r.commit&&emission.sourceTree===r.tree&&emission.entrypoint==='src/web/index.html'&&emission.webManifestSha256===component.ref.sha256,'Missing genuine matching web emission evidence');
  check(emission.compiler&&typeof emission.compiler.version==='string'&&emission.compiler.version&&HEX.test(emission.compiler.sha256),'Missing pinned web compiler identity');
  check(source.schema===1&&source.kind==='backplane-web-source-qualification'&&source.status==='PASS'&&source.commit===r.commit&&source.tree===r.tree&&source.version===VERSION&&Array.isArray(source.checks)&&source.checks.length>0,'Missing web source qualification');
  for(const ref of source.checks)p.reference(ref);
  p.generatedWeb(component.root,web,r);
  check(Array.isArray(emission.packages)&&emission.packages.length>0,'Missing web redistribution notices');
  for(const item of emission.packages){check(item.name&&item.version&&item.sourceDisposition&&Array.isArray(item.licenses)&&item.licenses.length>0,'Incomplete web notice');for(const n of item.licenses)check(web.members[n.path]?.sha256===n.sha256,'Web notice not in exact asset manifest');}
  const root=path.join(output,'backplane-web');fs.mkdirSync(root,{recursive:true});fs.cpSync(component.root,path.join(root,'web'),{recursive:true,dereference:false,errorOnExist:true,force:false});p.generatedWeb(path.join(root,'web'),web,r);
  const provenance=path.join(root,'PROVENANCE');fs.mkdirSync(provenance);
  for(const[name,ref]of Object.entries({freeze:r.freezeReceipt,emission:request.emissionReceipt,source:emission.sourceQualification}))fs.copyFileSync(p.reference(ref),path.join(provenance,name+'.json'));
  const logs=path.join(provenance,'checks');fs.mkdirSync(logs);const checks=[];
  for(const[index,ref]of source.checks.entries()){const name=index+'.log';fs.copyFileSync(p.reference(ref),path.join(logs,name));checks.push({path:'PROVENANCE/checks/'+name,sha256:ref.sha256,bytes:ref.bytes});}
  fs.copyFileSync(p.reference(component.ref),path.join(root,'WEB.json'));
  p.write(path.join(root,'NOTICES.json'),{schema:1,kind:'backplane-web-notices',packages:emission.packages});
  const members=Object.fromEntries(Object.entries(p.inventory(root,'linux-x64')).map(([n,v])=>{check(v.type==='file','Web package symlink refused');return['backplane-web/'+n,{sha256:v.sha256,bytes:v.bytes}];}));
  const name='backplane-v'+VERSION+'-web.tar.gz',filename=path.join(output,name);p.runPython(['archive',root,filename,'backplane-web/',String(freeze.sourceDateEpoch)]);
  const bound={path:name,sha256:p.digest(filename),bytes:fs.statSync(filename).size};fs.writeFileSync(filename+'.sha256',bound.sha256+'  '+name+'\n');
  const receipt={schema:1,kind:'backplane-target-package',target:'web',sourceCommit:r.commit,sourceTree:r.tree,version:VERSION,channel:'release',archive:bound,members,web:{...web,root:'backplane-web/web'},webManifestSha256:component.ref.sha256,compiler:emission.compiler,sourceChecks:checks,sourceQualified:true,releaseQualified:false,qualificationScope:'Supplied emitted web/source/notices only; actual web runtime and full release owner coverage remain separate'};
  const receiptFile=path.join(output,'WEB-PACKAGE.json');p.write(receiptFile,receipt);
  return{status:'PACKAGED_FROM_QUALIFIED_WEB_ARTIFACT',archive:filename,sha256:bound.sha256,receipt:receiptFile,receiptSha256:p.digest(receiptFile),published:false};
 }finally{fs.rmSync(temp,{recursive:true,force:true});}
}
function main(argv){
 if(argv.length===1&&argv[0]==='describe'){console.log(JSON.stringify({interface:'backplane-web-release-producer-v1',version:VERSION,target:'web',operation:'archive --request REQUEST.json --output NEW_ABS_DIST',artifact:'backplane-v0.12.0-web.tar.gz',execution:'Offline supplied-artifact adapter; no Bend emission, service/native build or publication'},null,2));return;}
 const[op,...rest]=argv,args={};check(op==='archive'&&rest.length===4,'Usage: archive --request FILE --output NEW_ABS_DIST');for(let i=0;i<rest.length;i+=2){check(['--request','--output'].includes(rest[i])&&!Object.hasOwn(args,rest[i]),'Unknown/duplicate option');args[rest[i]]=rest[i+1];}check(args['--request']&&args['--output'],'Both options required');console.log(JSON.stringify(archive(path.resolve(args['--request']),args['--output'])));
}
if(require.main===module)try{main(process.argv.slice(2));}catch(e){console.error('Web producer: '+e.message);process.exitCode=1;}
module.exports={archive};
