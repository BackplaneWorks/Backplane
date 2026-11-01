'use strict';
// Source-only handoff: VM of the actual service discovery with synthetic IO.
// No real service, Electron, socket, profile or process is started by this file.
const {test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const {validateBrowserHello,LIMITS}=require('../browser-envelope.cjs');
function fixture(){
  const origin='http://127.0.0.1:40001',token='a'.repeat(32),calls=[];
  const state={port:40001,version:'b990',paired:true,credential:token,missing:false};
  const module={exports:{}};
  const files={async readFile(file){
    if(file.endsWith('hub.lock')){if(state.missing)throw Object.assign(new Error('No lock'),{code:'ENOENT'});return `123 ${state.port} flock\n`;}
    if(file.endsWith('token'))return state.credential;
    throw new Error('Unexpected file read');
  }};
  const fetch=async(url,options={})=>{
    calls.push({url,...options});
    if(url.endsWith('/hello')){
      const text=Buffer.from(state.version),prefix=Buffer.from([0xa1,0x69]);
      const wire=Buffer.concat([prefix,Buffer.from('backplane'),Buffer.from([0x60+text.length]),text]);
      return{ok:true,async arrayBuffer(){return wire;}};
    }
    assert.equal(url,origin+'/auth/pair');assert.equal(options.method,'POST');
    return{ok:state.paired&&options.headers.Authorization==='Bearer '+token};
  };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../service.cjs'),'utf8'),{
    module,Buffer,process,AbortSignal,fetch,setTimeout,require(name){
      if(name==='node:fs/promises')return files;
      if(name==='node:child_process')return{spawn(){throw new Error('Discovery must not launch a service');},execFile(){throw new Error('No child process allowed');}};
      return name==='./service-diagnostics.cjs'?require('../service-diagnostics.cjs'):require(name);
    },
  },{filename:'service.cjs'});
  const service=new module.exports.Service({home:'/synthetic/browser-rebind',expectedVersion:'b990'});
  return{service,state,calls,origin,select(){service.connection={origin,version:'b990'};}};
}
function hello(version){
  const serviceEpoch='b'.repeat(32),controllerLease='c'.repeat(32);
  return{v:1,op:'hello.accept',id:'a'.repeat(32),serviceEpoch,controllerLease,serverSequence:1,
    payload:{protocol:1,version,hub:'synthetic-home',serviceEpoch,controllerLease,limits:{...LIMITS}}};
}

test('Browser reconnect discovers the authenticated replacement version while startup stays pinned',async()=>{
  const h=fixture();h.select();h.state.version='7c';
  await assert.rejects(()=>h.service.discover(),/differs from the packaged version/);
  assert.equal(h.calls.length,1,'ordinary discovery refuses before pairing');
  const found=await h.service.discover({browserReconnectOrigin:h.origin});
  assert.equal(found.origin,h.origin);assert.equal(found.version,'7c');
  assert.equal(h.calls.length,3);assert.equal(h.calls[2].url,h.origin+'/auth/pair');
  assert.equal(h.calls[2].headers.Authorization,'Bearer '+'a'.repeat(32));
  assert.equal(h.calls[2].redirect,'error');assert.equal(h.service.expectedVersion,'b990');
  assert.equal(h.service.connection.version,'b990');assert.equal(h.service.child,null);
  validateBrowserHello(hello('7c'),{requestId:'a'.repeat(32),expectedVersion:found.version});
  assert.throws(()=>validateBrowserHello(hello('b990'),{requestId:'a'.repeat(32),expectedVersion:found.version}),/does not match/);
  assert.throws(()=>validateBrowserHello(hello('later'),{requestId:'a'.repeat(32),expectedVersion:found.version}),/does not match/);
  const badProtocol=hello('7c');badProtocol.payload.protocol=2;
  assert.throws(()=>validateBrowserHello(badProtocol,{requestId:'a'.repeat(32),expectedVersion:found.version}),/does not match/);
});

test('Browser reconnect refuses an unselected or changed origin before sending credentials',async()=>{
  const h=fixture();
  await assert.rejects(()=>h.service.discover({browserReconnectOrigin:h.origin}),/does not match the selected service/);
  h.select();
  await assert.rejects(()=>h.service.discover({browserReconnectOrigin:'http://127.0.0.1:40002'}),/does not match the selected service/);
  h.state.port=40002;
  await assert.rejects(()=>h.service.discover({browserReconnectOrigin:h.origin}),/origin changed/);
  assert.equal(h.calls.length,0);assert.equal(h.service.child,null);
});

test('Browser reconnect still refuses failed home authentication and malformed credentials',async()=>{
  const h=fixture();h.select();h.state.version='7c';h.state.paired=false;
  await assert.rejects(()=>h.service.discover({browserReconnectOrigin:h.origin}),/does not authenticate/);
  assert.equal(h.calls.length,2);
  h.state.credential='invalid';
  await assert.rejects(()=>h.service.discover({browserReconnectOrigin:h.origin}),/Invalid service credential/);
  assert.equal(h.calls.length,3,'invalid token never sent to pair');assert.equal(h.service.child,null);
});

test('Browser reconnect with no running selected service returns unavailable without starting one',async()=>{
  const h=fixture();h.select();h.state.missing=true;
  assert.equal(await h.service.discover({browserReconnectOrigin:h.origin}),null);
  assert.equal(h.calls.length,0);assert.equal(h.service.child,null);
});
