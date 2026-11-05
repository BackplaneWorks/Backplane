'use strict';
// TEST package only: retain the original stage/hash/architecture guards.
// No release acceptance is created and normal release archive is not called.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const [tools,stage,out]=process.argv.slice(2).map(x=>path.resolve(x));
const p=require(path.join(tools,'desktop/produce-release.cjs'));
assert(!fs.existsSync(out),'Preserve existing output');
const s=p.json(path.join(stage,'STAGE.json'));
assert.equal(s.testOnly,true);assert.equal(s.releaseQualified,false);
assert.equal(s.release.commit,'be904cd4cb21ad6febe17e674918c96945113d07');
assert.equal(s.release.tree,'1034e9dc35fb517f1186ebcdb2d8f3351beb7a59');
assert.equal(s.release.version,'0.14.2');assert.equal(s.release.channel,'release');
assert.equal(s.payloadManifestSha256,p.contentDigest(s.members));
p.validateSigningChanges(stage,s.target==='darwin-arm64');
const t=p.TARGETS[s.target];assert(t);
p.architecture(path.join(stage,t.executable),s.target);
p.helpers(path.join(stage,t.resources,'backplane-service'),s.target);
const build=p.json(path.join(stage,t.resources,'backplane-service/BUILD.json'));
assert.equal(build.status,'AUTHORIZED_COMPOSITE_SERVICE_BUILD');
assert.equal(build.testOnly,true);assert.equal(build.releaseQualified,false);
const inspected=p.inspectStage(stage),before=p.inventory(stage,s.target);
fs.mkdirSync(out,{recursive:true});
const name='backplane-v0.14.2-'+s.target+t.suffix,file=path.join(out,name);
p.runPython(['archive',stage,file,t.root+'/',String(s.sourceDateEpoch)]);
assert.deepEqual(p.inventory(stage,s.target),before,'Payload changed while archiving');
const receipt={schema:1,status:'CURRENT_AUTHORIZED_PACKAGE_READY_PLATFORM_GATE_PENDING',releaseUseAuthorized:true,byteQualification:'NO_CURRENT_SOURCE_STOCK_COMPARISON_CARRIED_OVER_OPTION2_AFA96792',
 target:s.target,sourceCommit:s.release.commit,sourceTree:s.release.tree,version:'0.14.2',
 channel:'release',testOnly:true,releaseQualified:false,stockEquivalent:false,
 buildProvenance:build.buildProvenance,buildReceiptSHA256:p.digest(path.join(stage,t.resources,'backplane-service/BUILD.json')),
 stage:inspected,members:before,archive:{path:file,sha256:p.digest(file),bytes:fs.statSync(file).size},
 runtimeAcceptance:'UNVERIFIED; native consumer follows this exact artifact',
 publication:'PREAUTHORIZED_AFTER_NEW_COMBINED_WINDOWS_PASS_AND_ALL7PACKAGES'};
p.write(path.join(out,'TEST-PACKAGE.json'),receipt);
p.write(path.join(out,'PACKAGE.json'),receipt);
console.log(JSON.stringify(receipt));
