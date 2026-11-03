const {test} = require('node:test'), assert = require('node:assert/strict');
const {invitation,pair} = require('../pairing.cjs');
test('desktop protocol links contain only an invitation, strict origins and no userinfo',()=>{
 const link='https://hub.example:3787/pair#invite='+'a'.repeat(32);
 assert.deepEqual(invitation('backplane://pair?url='+encodeURIComponent(link)), {origin:'https://hub.example:3787',kind:'invite',secret:'a'.repeat(32)});
 for (const bad of ['file:///tmp/#invite='+'a'.repeat(32),'https://user:pass@hub/#invite='+'a'.repeat(32),'https://hub/#invite=bad','backplane://other?url='+encodeURIComponent(link)]) assert.equal(invitation(bad),null);
});
test('desktop enrollment is bounded, does not follow redirects and never returns credentials',async()=>{
 const calls=[],text='https://hub/pair#code=ABCD2345';
 const r=await pair({text,name:'Laptop',fetch:async(...v)=>{calls.push(v);return {ok:true,json:async()=>({token:'b'.repeat(32)})}}});
 assert.deepEqual(r,{ok:true,origin:'https://hub'});assert.equal(calls[0][0],'https://hub/auth/enroll');assert.equal(calls[0][1].redirect,'error');assert.equal(calls[0][1].headers.Authorization,'Bearer ABCD2345');
 const bad=await pair({text,fetch:async()=>{throw Error(text)}});assert.deepEqual(bad,{ok:false,error:'hub_unreachable'});
});
test('web parses link/QR/code equivalently and removes invitation fragments before exchange',async()=>{
 const {pairingInput,codeLink}=await import('../../src/web/pairing-link.js');
 const link=codeLink('hub:3787','abcd-2345');assert.equal(link,'http://hub:3787/pair#code=ABCD2345');assert.equal(pairingInput(link).secret,'ABCD2345');
 const {pairBrowser}=await import('../../src/web/auth.js');let scrubbed=false,endpoint;
 const ok=await pairBrowser({location:{hash:'#invite='+'c'.repeat(32),pathname:'/pair',search:''},history:{replaceState(){scrubbed=true}},storage:{getItem:()=>null,removeItem(){}},fetch:async(p,o)=>{assert.ok(scrubbed);endpoint=p;assert.equal(o.headers.Authorization,'Bearer '+'c'.repeat(32));return{ok:true}}});assert.ok(ok);assert.equal(endpoint,'/auth/enroll');
});
test('capability discovery distinguishes revoked access from an old hub',async()=>{
 const {discoverViews}=await import('../../src/web/thin-discovery.js');
 for (const status of [401,403]) assert.deepEqual(await discoverViews({fetch:async()=>({status}),validate:()=>false}),{supported:false,unauthorized:true});
 assert.deepEqual(await discoverViews({fetch:async()=>({status:404}),validate:()=>false}),{supported:false});
});
test('desktop protocol enrollment derives a platform name when no label was supplied',async()=>{
 let sent;
 await pair({text:'https://hub/pair#code=ABCD2345',fetch:async(_url,options)=>{sent=options.headers['X-Backplane-Name'];return {ok:true}}});
 assert.equal(sent,({linux:'Linux',darwin:'macOS',win32:'Windows'}[process.platform]||'Device')+' / Desktop');
});
test('a link from outside the app pairs at once only with one of the person\'s own hubs',async()=>{
 const {knownHub}=require('../pairing.cjs'), link=o=>'backplane://pair?url='+encodeURIComponent(o+'/pair#invite='+'a'.repeat(32));
 let asked=[];const fetch=known=>async(url,options)=>{asked.push([url,options.headers['X-Backplane-Origin'],options.headers.Authorization,options.redirect]);return {ok:true,json:async()=>({known})}};
 // this app's own service and the hub it is paired with: no question asked
 assert.equal(await knownHub({text:link('http://127.0.0.1:4100'),local:'http://127.0.0.1:4100',saved:null,token:'t',fetch:fetch(false)}),true);
 assert.equal(await knownHub({text:link('http://100.2.0.1:3787'),local:'http://127.0.0.1:4100',saved:'http://100.2.0.1:3787',token:'t',fetch:fetch(false)}),true);
 assert.equal(asked.length,0);
 // any other address: the own service says whether it is one of the owner's
 assert.equal(await knownHub({text:link('http://100.3.0.1:3787'),local:'http://127.0.0.1:4100',saved:null,token:'t',fetch:fetch(true)}),true);
 assert.deepEqual(asked[0],['http://127.0.0.1:4100/auth/known','http://100.3.0.1:3787','Bearer t','error']);
 assert.equal(await knownHub({text:link('https://evil.example'),local:'http://127.0.0.1:4100',saved:null,token:'t',fetch:fetch(false)}),false);
 // no answer (an older service, or none running) is not a yes
 assert.equal(await knownHub({text:link('https://evil.example'),local:'http://127.0.0.1:4100',saved:null,token:'t',fetch:async()=>({ok:false,status:404})}),false);
 assert.equal(await knownHub({text:link('https://evil.example'),local:'http://127.0.0.1:4100',saved:null,token:'t',fetch:async()=>{throw Error('down')}}),false);
 assert.equal(await knownHub({text:link('https://evil.example'),local:null,saved:null,token:'t',fetch:fetch(true)}),false);
 assert.equal(await knownHub({text:'https://evil.example/pair#invite=bad',local:'http://127.0.0.1:4100',saved:null,token:'t',fetch:fetch(true)}),false);
});
test('a revoked invitation says so',async()=>{
 const r=await pair({text:'https://hub/pair#code=ABCD2345',fetch:async()=>({ok:false,json:async()=>({error:'revoked_code'})})});
 assert.deepEqual(r,{ok:false,error:'revoked_code'});
});
