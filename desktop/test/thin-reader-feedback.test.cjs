'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=()=>fs.readFileSync(process.env.READER_HOST_SOURCE||path.join(__dirname,'../../src/web/host.js'),'utf8');
test('rejected current inline reader response produces visible feedback instead of a dead button',()=>{
 const src=source(),a=src.indexOf('function showThinMessage(message)'),b=src.indexOf('function stopThin(',a),warnings=[];let paints=0;
 const show=vm.runInNewContext('('+src.slice(a,b)+')',{viewMode:'view-v1',thinContinuations:{accept:()=>false},showViewStatus:s=>warnings.push(s),later(){paints++;}});
 show({id:'report',version:'wrong',text:'stale'});assert.equal(paints,0);assert.equal(warnings.length,1);assert.match(warnings[0],/continuation.*unavailable/i);
});
test('actual ThinHost warning hook forwards rejected chunk/transport feedback to the existing status UI',()=>{
 const src=source(),start=src.indexOf('thin = new ThinHost('),tail=src.slice(start),line=/^\s*warning:\s*(.+),$/m.exec(tail);assert(line,'Current ThinHost must wire its existing warning callback');
 const warnings=[],warning=vm.runInNewContext('('+line[1]+')',{showViewStatus:s=>warnings.push(s)});warning('Message continuation is unavailable; reopen it explicitly');assert.equal(warnings.length,1);
});
test('accepted inline reader clears its stale feedback and renders without a modal or extra request',()=>{
 const src=source(),a=src.indexOf('function showThinMessage(message)'),b=src.indexOf('function stopThin(',a),warnings=[];let paints=0;
 const show=vm.runInNewContext('('+src.slice(a,b)+')',{viewMode:'view-v1',thinContinuations:{accept:()=>true},showViewStatus:s=>warnings.push(s),later(){paints++;}});
 show({id:'report',version:'1',text:'accepted'});assert.equal(paints,1);assert.deepEqual(warnings,['']);
});
