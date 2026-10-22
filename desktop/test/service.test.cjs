const { test } = require('node:test');
const assert = require('node:assert/strict');
const { hello, lockLine, Service, preferredPort, servicePort, startingNote } = require('../service.cjs');
test('decodes the real hello wire shape and rejects alien greetings', () => {
  assert.deepEqual(hello(Buffer.from('a1696261636b706c616e6565312e322e33', 'hex')), { backplane: '1.2.3' });
  assert.throws(() => hello(Buffer.from('a0', 'hex')));
  assert.throws(() => hello(Buffer.from('ff', 'hex')));
});
test('owned-service port allocation preserves origin across sequential app restarts', async t => {
  const net=require('node:net'),crypto=require('node:crypto');const home='/tmp/bp-owned-origin-'+crypto.randomUUID();
  const candidate=preferredPort(home),probe=net.createServer();
  try { await new Promise((resolve,reject)=>{probe.once('error',reject);probe.listen(candidate,'127.0.0.1',resolve);}); }
  catch(error){if(error.code==='EADDRINUSE'){t.skip('Preferred synthetic port already occupied');return;}throw error;}
  await new Promise(resolve=>probe.close(resolve));
  const first=await servicePort(home),second=await servicePort(home+'/child/..');assert.equal(first,candidate);assert.equal(second,first);
});
test('an occupied preferred port falls back without attaching to or signalling its owner', async()=>{
  const net=require('node:net'),crypto=require('node:crypto');const home='/tmp/bp-owned-collision-'+crypto.randomUUID(),server=net.createServer();
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(preferredPort(home),'127.0.0.1',resolve);});
  try { const chosen=await servicePort(home);assert.notEqual(chosen,server.address().port);assert(server.listening); }
  finally { await new Promise(resolve=>server.close(resolve)); }
});
test('discovery accepts only the existing explicit flock contract', () => {
  assert.deepEqual(lockLine('123 3787 flock\n'), { posixPid: 123, port: 3787 });
  for (const line of ['0 3787 flock', '123 65536 flock', '123 3787', '123 3787 flock\nextra']) assert.throws(() => lockLine(line));
});
test('does not signal a service it merely discovered', () => {
  const manager = new Service({ home: '/tmp/test-backplane-electron' });
  manager.connection = { pid: process.pid }; manager.stop();
  assert.equal(manager.child, null);
});
test('authenticates the endpoint without treating POSIX lock IDs as native PIDs', async () => {
  const http = require('node:http');
  const fs = require('node:fs/promises');
  const path = require('node:path');
  const os = require('node:os');
  const home = await fs.mkdtemp(path.join(os.tmpdir(),'bp-lock-identity-'));
  const token = 'a'.repeat(32);
  const server = http.createServer((req,res) => {
    if(req.url === '/hello') { res.end(Buffer.from('a1696261636b706c616e6565312e322e33','hex')); return; }
    res.statusCode=req.headers.authorization === `Bearer ${token}` ? 200 : 401; res.end();
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const port=server.address().port;
  try {
    await fs.writeFile(path.join(home,'token'),token);
    await fs.writeFile(path.join(home,'hub.lock'),`2147483647 ${port} flock\n`);
    const service=new Service({home,expectedVersion:'1.2.3'});
    const connection=await service.discover();
    assert.deepEqual(connection.lockIdentity,{posixPid:2147483647});
    assert.equal(service.child,null);
    await fs.writeFile(path.join(home,'token'),'b'.repeat(32));
    await assert.rejects(()=>service.discover(),/does not authenticate/);
  } finally { await new Promise(resolve=>server.close(resolve)); await fs.rm(home,{recursive:true,force:true}); }
});
test('the waiting screen reads a starting hub\'s note', () => {
  assert.equal(startingNote({status:'starting',note:'building store 42%'}),'Building the database… 42%');
  assert.equal(startingNote({status:'starting'}),'Starting your service…');
  assert.equal(startingNote(null),'Starting your service…');
});
test('a hub answering 503 starting is waited for past the 45 s limit, with its note shown', async () => {
  const http=require('node:http'),fs=require('node:fs/promises'),path=require('node:path'),os=require('node:os');
  const home=await fs.mkdtemp(path.join(os.tmpdir(),'bp-starting-')),token='a'.repeat(32);
  // each 503 moves the clock 20 s: six of them are two minutes, past any fixed deadline
  const now=Date.now;let skew=0,busy=6;Date.now=()=>now()+skew;
  const server=http.createServer((req,res)=>{
    if(req.url==='/hello'){
      if(busy>0){busy--;skew+=20000;res.statusCode=503;res.setHeader('Content-Type','application/json');res.end(JSON.stringify({status:'starting',note:`building store ${(6-busy)*10}%`}));return;}
      res.end(Buffer.from('a1696261636b706c616e6565312e322e33','hex'));return;
    }
    res.statusCode=req.headers.authorization===`Bearer ${token}`?200:401;res.end();
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  try {
    await fs.writeFile(path.join(home,'token'),token);
    await fs.writeFile(path.join(home,'hub.lock'),`123 ${server.address().port} flock\n`);
    const seen=[],service=new Service({home,expectedVersion:'1.2.3'});
    const connection=await service.start({onStatus:m=>seen.push(m)});
    assert.equal(connection.version,'1.2.3');assert.equal(service.child,null);
    assert.deepEqual(seen,[10,20,30,40,50,60].map(n=>`Building the database… ${n}%`));
  } finally { Date.now=now;await new Promise(resolve=>server.close(resolve));await fs.rm(home,{recursive:true,force:true}); }
});
