const {test}=require('node:test'), assert=require('node:assert/strict'), fs=require('node:fs'), os=require('node:os'), path=require('node:path');
const source=fs.readFileSync('src/server/effects/pairing.js','utf8').split('let bp_pairing;')[0];
const factory=new Function('require',source+';return pairingStore;')(require);
function fixture(t){const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-devices-'));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));let time=1000000;const opts={home,root:'a'.repeat(32),now:()=>time};return {home,opts,store:factory(opts),advance:n=>{time+=n}};}
test('link and code are one invitation; redemption, restart and revocation preserve other pairings',t=>{
 const f=fixture(t), a=f.store.create(['http://hub:3787']);assert.equal(a.code.length,8);assert.ok(a.link.includes('#invite='));assert.equal(new URL(a.link).search,'');
 const token=new URLSearchParams(new URL(a.link).hash.slice(1)).get('invite');const first=f.store.enroll(token,'Laptop');assert.ok(first.token);assert.equal(f.store.enroll(a.code,'Phone').error,'used_code');
 const b=f.store.create(['http://hub:3787']);const second=f.store.enroll(b.code,'Phone');assert.ok(second.token);assert.equal(f.store.resolve(first.token,3,'/ws'),first.token);
 assert.deepEqual(f.store.revoke(first.id).clients,[3]);assert.equal(f.store.resolve(first.token,4,'/ws'),'');assert.equal(f.store.resolve(second.token,5,'/ws'),second.token);
 const restarted=factory(f.opts);assert.equal(restarted.resolve(second.token,6,'/ws'),second.token);assert.equal(restarted.resolve(first.token,7,'/ws'),'');assert.equal(restarted.resolve(f.opts.root,8,'/ws'),f.opts.root);
 const disk=fs.readFileSync(path.join(f.home,'devices-v1.json'),'utf8');for(const secret of [token,a.code,first.token,second.token,f.opts.root]) assert.ok(!disk.includes(secret));
 assert.equal(f.store.list().devices[1].name,'Phone');assert.ok(!JSON.stringify(f.store.list()).includes('hash'));
 const c=f.store.create(['http://hub:3787']);const repaired=f.store.enroll(c.code,'Laptop');assert.ok(repaired.token);assert.equal(f.store.resolve(repaired.token,9,'/ws'),repaired.token);
});
test('wrong, expired and reused codes have explicit errors; a hub-wide bucket limits guesses',t=>{
 const f=fixture(t),invite=f.store.create(['http://hub:3787']);assert.equal(f.store.enroll('WRONG123','').error,'wrong_code');f.advance(600000);assert.equal(f.store.enroll(invite.code,'').error,'expired_code');
 for(let i=0;i<9;i++) assert.equal(f.store.enroll('NOPE0000','').error,'wrong_code');assert.equal(f.store.enroll(invite.code,'').error,'rate_limited');f.advance(60001);assert.equal(f.store.enroll(invite.code,'').error,'expired_code');
});
test('failed durable write cannot consume an invite or replace an existing registry',t=>{
 const f=fixture(t);const first=f.store.enroll(f.store.create(['http://hub']).code,'Old');let refuse=false;
 const store=factory({...f.opts,fs:{...fs,renameSync(...a){if(refuse)throw Error('synthetic');return fs.renameSync(...a)}}});const invite=store.create(['http://hub']);const before=fs.readFileSync(path.join(f.home,'devices-v1.json'),'utf8');refuse=true;assert.throws(()=>store.enroll(invite.code,'New'));assert.equal(fs.readFileSync(path.join(f.home,'devices-v1.json'),'utf8'),before);refuse=false;assert.ok(store.enroll(invite.code,'New').token);assert.equal(store.resolve(first.token,1,'/ws'),first.token);
});
test('an invitation that cannot be recorded is not issued',t=>{
 const f=fixture(t);const store=factory({...f.opts,fs:{...fs,renameSync(){throw Error('synthetic')}}});assert.throws(()=>store.create(['http://hub']));
 assert.equal(factory(f.opts).list().invitations.length,0);
});
test('every invitation is recorded: issued, used by which device, revoked; a hub refuses one it did not issue',t=>{
 const f=fixture(t), a=f.store.create(['http://hub:3787']), b=f.store.create(['http://hub:3787']), c=f.store.create(['http://hub:3787']);
 const listed=()=>Object.fromEntries(factory(f.opts).list().invitations.map(x=>[x.id,x]));
 assert.deepEqual(Object.values(listed()).map(x=>x.state),['open','open','open']);
 const laptop=f.store.enroll(a.code,'Laptop');assert.ok(laptop.token);
 assert.equal(listed()[a.id].state,'used');assert.equal(listed()[a.id].deviceName,'Laptop');assert.equal(listed()[a.id].device,laptop.id);assert.equal(listed()[a.id].usedAt,1000000);
 // a link this hub never issued, and one it revoked, do not pair
 assert.equal(f.store.enroll('f'.repeat(32),'Forged').error,'wrong_code');
 assert.deepEqual(f.store.uninvite(b.id),{ok:true,clients:[]});assert.equal(listed()[b.id].state,'revoked');
 assert.equal(f.store.enroll(new URLSearchParams(new URL(b.link).hash.slice(1)).get('invite'),'Phone').error,'revoked_code');
 assert.equal(f.store.enroll(b.code,'Phone').error,'revoked_code');
 // a restarted hub still honours an open one and refuses the revoked one
 const restarted=factory(f.opts);assert.ok(restarted.enroll(c.code,'Tablet').token);assert.equal(restarted.enroll(b.code,'Phone').error,'revoked_code');
 // revoking a used link revokes the device it paired, and drops its sockets
 assert.equal(f.store.resolve(laptop.token,7,'/ws'),laptop.token);
 assert.deepEqual(f.store.uninvite(a.id),{ok:true,clients:[7]});assert.equal(f.store.active(7),false);
 assert.equal(f.store.resolve(laptop.token,8,'/ws'),'');assert.equal(listed()[a.id].state,'revoked');assert.equal(listed()[a.id].deviceRevoked,true);
 assert.equal(f.store.uninvite('0'.repeat(32)).error,'unknown_invitation');
 const disk=fs.readFileSync(path.join(f.home,'devices-v1.json'),'utf8');for(const x of [a,b,c])for(const secret of [x.code,new URLSearchParams(new URL(x.link).hash.slice(1)).get('invite')])assert.ok(!disk.includes(secret));
 assert.ok(!JSON.stringify(f.store.list()).includes('hash'));
});
test('a registry from before invitations were recorded still loads; a broken invitation list fails closed',t=>{
 const f=fixture(t),file=path.join(f.home,'devices-v1.json');f.store.enroll(f.store.create(['http://hub']).code,'Old');
 const data=JSON.parse(fs.readFileSync(file,'utf8'));delete data.invitations;fs.writeFileSync(file,JSON.stringify(data));
 const old=factory(f.opts);assert.equal(old.list().devices.length,1);assert.equal(old.list().invitations.length,0);
 data.invitations=[{id:'x'}];fs.writeFileSync(file,JSON.stringify(data));assert.throws(()=>factory(f.opts));
});
test('only sixteen open invitations at once; old used ones are dropped from the list after a hundred',t=>{
 const f=fixture(t);for(let i=0;i<16;i++)assert.ok(f.store.create(['http://hub']).code);assert.equal(f.store.create(['http://hub']).error,'too_many_invitations');
 f.advance(600001);for(let i=0;i<110;i++){f.advance(1);assert.ok(f.store.create(['http://hub']).code);f.advance(600001);}
 // (trimmed as each is issued: the hundred newest closed ones, and the new one)
 assert.equal(f.store.list().invitations.length,101);
});
test('a pairing link is known only for the owner\'s hubs',t=>{
 const code=fs.readFileSync('src/server/effects/pairing.js','utf8').split('let bp_pairing;')[0];
 const known=new Function('require',code+';return pairingKnown;')(require);
 const machines={self:{name:'laptop',url:'http://100.1.0.1:3787',domain:'t.ts.net'},peers:[{name:'desk',url:'http://100.2.0.1:3787'}]};
 for(const o of ['http://100.1.0.1:3787','http://100.2.0.1:3787','https://desk.t.ts.net','http://hub.lan:4000','http://127.0.0.1:3787'])assert.equal(known(o,{routes:['http://127.0.0.1:3787'],machines,peers:['http://hub.lan:4000']}),true,o);
 for(const o of ['http://100.2.0.1:3788','https://evil.example','https://desk.other.ts.net','https://desk.t.ts.net.evil.example','http://user:pw@100.2.0.1:3787','file:///x','not a url'])assert.equal(known(o,{routes:['http://127.0.0.1:3787'],machines,peers:['http://hub.lan:4000']}),false,o);
 assert.equal(known('https://desk.t.ts.net',{}),false);
});
test('malformed registry fails closed; no existing files rewritten',t=>{const f=fixture(t),file=path.join(f.home,'devices-v1.json');fs.writeFileSync(file,'broken');assert.throws(()=>factory(f.opts));assert.equal(fs.readFileSync(file,'utf8'),'broken');});
test('listener route hints strip credentials and precede local desktop origins',t=>{
 const f=fixture(t),effects=new Map();
 const code=fs.readFileSync('src/server/effects/pairing.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
 new Function('require','process','io_eff',code)(require,process,(id,fn)=>effects.set(id,fn));
 assert.equal(effects.get('Pairing.start')(f.home,f.opts.root,'127.0.0.1',3787),'');
 effects.get('Pairing.address')('https://fixture.ts.net/#token='+f.opts.root);
 const issued=JSON.parse(effects.get('Pairing.request')('create','','','127.0.0.1:3787',false));
 assert.equal(issued.routes[0],'https://fixture.ts.net');assert.equal(issued.localOnly,false);assert.ok(!issued.link.includes(f.opts.root));
});
test('hub-to-hub invites are recorded: they expire like device invitations, link once, and revoking one unlinks its machine',t=>{
 const f=fixture(t), peers=path.join(f.home,'secrets','peers');fs.mkdirSync(peers,{recursive:true});
 const secret=p=>{fs.writeFileSync(path.join(peers,p),'{"secret":"x"}');return p;};
 const p1=secret('p'+'1'.repeat(16)),p2=secret('p'+'2'.repeat(16)),p3=secret('p'+'3'.repeat(16)),p4=secret('p'+'4'.repeat(16));
 const a=f.store.machine(p1),b=f.store.machine(p2),c=f.store.machine(p3);assert.ok(a.ok&&b.ok&&c.ok);assert.equal(a.expiresAt,1000000+600000);
 assert.equal(f.store.machine('../x').error,'invalid_request');assert.equal(f.store.machine(p1).error,'invalid_request');
 const listed=()=>Object.fromEntries(factory(f.opts).list().invitations.map(x=>[x.id,x]));
 assert.deepEqual([a,b,c].map(x=>listed()[x.id].kind),['machine','machine','machine']);assert.equal(listed()[a.id].state,'open');
 // used once, by the machine that linked; a peer never recorded is refused
 assert.deepEqual(f.store.link(p1,'desk'),{ok:true});assert.equal(f.store.link(p1,'desk').error,'used_code');
 assert.equal(listed()[a.id].state,'used');assert.equal(listed()[a.id].deviceName,'desk');assert.equal(listed()[a.id].device,p1);assert.equal(listed()[a.id].deviceRevoked,false);
 assert.equal(f.store.link(p4,'old').error,'unknown_invitation');
 // a device code never matches a machine invite, and the other way round
 assert.equal(f.store.enroll(p2,'x').error,'wrong_code');
 // revoked before use: it never links and its secret is gone
 assert.deepEqual(f.store.uninvite(b.id),{ok:true,clients:[]});assert.equal(f.store.link(p2,'x').error,'revoked_code');assert.ok(!fs.existsSync(path.join(peers,p2)));
 // revoked after use: the hub is told which link to remove
 assert.deepEqual(f.store.uninvite(a.id),{ok:true,clients:[],peer:p1});assert.equal(listed()[a.id].deviceRevoked,true);assert.ok(!fs.existsSync(path.join(peers,p1)));
 // unused past ten minutes: it expires and its secret is swept
 f.advance(600000);assert.equal(f.store.link(p3,'late').error,'expired_code');assert.equal(listed()[c.id].state,'expired');assert.ok(!fs.existsSync(path.join(peers,p3)));
 assert.ok(fs.existsSync(path.join(peers,p4)));
 // a link removed in Bots is shown as such
 const d=f.store.machine(secret('p'+'5'.repeat(16)));f.store.link('p'+'5'.repeat(16),'lab');f.store.unlinked('p'+'5'.repeat(16));assert.equal(listed()[d.id].deviceRevoked,true);
 // sixteen open at once, apart from device invitations
 for(let i=0;i<16;i++)assert.ok(f.store.machine('p'+i.toString(16).padStart(16,'a')).ok);assert.equal(f.store.machine('p'+'f'.repeat(16)).error,'too_many_invitations');assert.ok(f.store.create(['http://hub']).code);
 const disk=fs.readFileSync(path.join(f.home,'devices-v1.json'),'utf8');assert.ok(!disk.includes('"secret"'));
 // an older hub's validator (device invitations only) still reads the registry
 const data=JSON.parse(disk);assert.ok(data.invitations.every(x=>/^[a-f0-9]{32}$/.test(x.id)&&/^[a-f0-9]{64}$/.test(x.hash)&&/^[a-f0-9]{64}$/.test(x.codeHash)));
});
// Add a device's addresses (pairingPlan): the tailnet, each local network
// address, this computer, each its own link to the one invitation
const nics={lo:[{family:'IPv4',address:'127.0.0.1',internal:true}],enp3s0:[{family:'IPv4',address:'192.168.1.23',internal:false},{family:'IPv6',address:'fe80::1',internal:false}],
 wlan0:[{family:'IPv4',address:'10.0.0.7',internal:false},{family:'IPv4',address:'169.254.3.4',internal:false}],docker0:[{family:'IPv4',address:'172.17.0.1',internal:false}],'br-1a2b':[{family:'IPv4',address:'172.18.0.1',internal:false}],
 veth9:[{family:'IPv4',address:'172.19.0.1',internal:false}],virbr0:[{family:'IPv4',address:'192.168.122.1',internal:false}],tailscale0:[{family:'IPv4',address:'100.101.102.103',internal:false}],odd0:[{family:'IPv4',address:'100.64.0.9',internal:false}]};
function effectsWith(t,f,interfaces){
 const effects=new Map(), code=fs.readFileSync('src/server/effects/pairing.js','utf8').replace(/CID\(([^)]+)\)/g,'"$1"');
 const req=n=>n==='node:os'?{networkInterfaces:()=>interfaces}:require(n);
 new Function('require','process','io_eff',code)(req,process,(id,fn)=>effects.set(id,fn));
 return {start:(host,port=3787)=>effects.get('Pairing.start')(f.home,f.opts.root,host,port),address:x=>effects.get('Pairing.address')(x),
  ask:(op,secret='',name='',authority='127.0.0.1:3787',https=false)=>JSON.parse(effects.get('Pairing.request')(op,secret,name,authority,https)),lan:(ip,base)=>effects.get('Pairing.lan')(ip,base).$==='True'};
}
const fragment=link=>{const u=new URL(link),p=new URLSearchParams(u.hash.slice(1));return {origin:u.origin,invite:p.get('invite'),routes:JSON.parse(p.get('routes'))};};
test('an invitation lists the tailnet, each local network address and this computer, each with its own link',t=>{
 const f=fixture(t),e=effectsWith(t,f,nics);assert.equal(e.start('127.0.0.1'),'');
 e.address('http://100.101.102.103:3787/#token='+f.opts.root);
 const x=e.ask('create');
 assert.deepEqual(x.addresses.map(a=>[a.kind,a.url,a.listening]),[['tailnet','http://100.101.102.103:3787',true],['lan','http://192.168.1.23:3787',false],['lan','http://10.0.0.7:3787',false],['local','http://127.0.0.1:3787',true]]);
 // one invitation: every link carries the same secret, its own address first
 const parts=x.addresses.map(a=>fragment(a.link));
 assert.equal(new Set(parts.map(p=>p.invite)).size,1);assert.equal(parts[0].invite,fragment(x.link).invite);
 parts.forEach((p,i)=>{assert.equal(p.origin,x.addresses[i].url);assert.equal(p.routes[0],x.addresses[i].url);});
 // a hint is only an address the hub answers on, and never loopback for another device
 assert.deepEqual(parts[0].routes,['http://100.101.102.103:3787']);assert.deepEqual(parts[1].routes,['http://192.168.1.23:3787','http://100.101.102.103:3787']);
 assert.deepEqual(parts[3].routes,['http://127.0.0.1:3787','http://100.101.102.103:3787']);
 // the old fields are the default address's: the first remote one that answers
 assert.equal(x.link,x.addresses[0].link);assert.deepEqual(x.routes,['http://100.101.102.103:3787']);assert.equal(x.localOnly,false);
 for(const a of x.addresses)assert.ok(!a.link.includes(f.opts.root));
 // loopback only: the old fields stay this computer's, as before
 const g=fixture(t),h=effectsWith(t,g,nics);h.start('127.0.0.1');const y=h.ask('create','','','localhost:3787');
 assert.deepEqual(y.addresses.map(a=>[a.kind,a.listening]),[['lan',false],['lan',false],['local',true]]);
 assert.equal(y.addresses[2].url,'http://localhost:3787');assert.equal(y.localOnly,true);assert.equal(y.link,y.addresses[2].link);
});
test('a listener on every interface answers on each local network address and covers the tailnet one',t=>{
 const f=fixture(t),e=effectsWith(t,f,nics);e.start('0.0.0.0');
 const x=e.ask('create','','','192.168.1.23:3787');
 assert.deepEqual(x.addresses.map(a=>[a.kind,a.url,a.listening]),[['tailnet','http://100.101.102.103:3787',true],['lan','http://192.168.1.23:3787',true],['lan','http://10.0.0.7:3787',true],['local','http://127.0.0.1:3787',true]]);
 // (a second tailnet address is a hint after them, never an address of its own)
 assert.deepEqual(x.routes,['http://100.101.102.103:3787','http://192.168.1.23:3787','http://10.0.0.7:3787','http://100.64.0.9:3787']);
 // no container, VM bridge, link-local or second tailnet address is offered as the local network
 assert.ok(!JSON.stringify(x).match(/172\.1[789]|192\.168\.122|169\.254/));assert.equal(x.addresses.filter(a=>a.url.includes('100.64.0.9')).length,0);
 // nor is the lan list fooled by a tailnet address on an odd interface name
 const {pairingPlan}=new Function('require',fs.readFileSync('src/server/effects/pairing.js','utf8').split('let bp_pairing;')[0]+';return {pairingPlan};')(require);
 assert.deepEqual(pairingPlan({host:'127.0.0.1',port:1,interfaces:nics}).choices.map(c=>c.url),['http://192.168.1.23:1','http://10.0.0.7:1','http://127.0.0.1:1']);
 // an origin none of these (a proxy's name) stays a hint after the chosen address
 const p=pairingPlan({host:'127.0.0.1',port:1,interfaces:{},authority:'hub.example',https:true});
 assert.deepEqual(p.choices.map(c=>c.kind),['local']);assert.deepEqual(p.routes,['https://hub.example']);
});
test('local network listening: kept across a restart, recorded when bound, closed when turned off',t=>{
 const f=fixture(t),e=effectsWith(t,f,nics),file=path.join(f.home,'lan-v1.json');e.start('127.0.0.1');
 assert.equal(e.ask('lan-on','172.17.0.1').error,'lan_unknown');assert.equal(e.ask('lan-on','8.8.8.8').error,'lan_unknown');assert.ok(!fs.existsSync(file));
 assert.deepEqual(e.ask('lan-on','192.168.1.23'),{ok:true,ip:'192.168.1.23',port:3787,bind:true});
 assert.deepEqual(JSON.parse(fs.readFileSync(file,'utf8')),{version:1,ips:['192.168.1.23']});if(process.platform!=='win32')assert.equal(fs.statSync(file).mode&0o777,0o600);
 // the hub bound it (loop.js's listener on that address is the one kept)
 let stopped=0,closed=0;const listener={server:{hostname:'192.168.1.23',port:3787,stop(all){stopped+=all?1:0;}},close(){closed++;}};
 globalThis.BP_H={rows:new Map([[1,{server:{hostname:'127.0.0.1',port:3787}}],[2,listener]])};t.after(()=>{delete globalThis.BP_H;});
 const up=e.ask('lan-up','192.168.1.23');assert.equal(up.base,3221225472);assert.equal(up.listening,true);
 assert.equal(e.lan('192.168.1.23',3221225472),true);assert.equal(e.lan('192.168.1.23',2147483648),false);assert.equal(e.lan('10.0.0.7',3221225472),false);
 assert.deepEqual(e.ask('lan-on','192.168.1.23'),{ok:true,ip:'192.168.1.23',listening:true});
 assert.deepEqual(e.ask('create').addresses.map(a=>a.listening),[true,false,true]);
 assert.deepEqual(e.ask('list').lan,[{ip:'192.168.1.23',url:'http://192.168.1.23:3787',listening:true}]);
 // the owner's own hub: a link to that address is followed without asking
 assert.equal(e.ask('known','http://192.168.1.23:3787').known,true);assert.equal(e.ask('known','http://10.0.0.7:3787').known,false);
 // a restart listens there again
 const r=effectsWith(t,f,nics);r.start('127.0.0.1');assert.deepEqual(r.ask('lan-saved'),{ips:['192.168.1.23'],port:3787});
 assert.equal(r.ask('lan-fail','192.168.1.23','boot').error,'lan_unavailable');assert.deepEqual(r.ask('lan-saved').ips,['192.168.1.23']);
 // off: forgotten, its listener and connections closed, the next accept ends the loop
 assert.deepEqual(e.ask('lan-off','192.168.1.23'),{ok:true,ip:'192.168.1.23',listening:false});
 assert.equal(stopped,1);assert.equal(closed,1);assert.equal(e.lan('192.168.1.23',3221225472),false);
 assert.deepEqual(JSON.parse(fs.readFileSync(file,'utf8')).ips,[]);assert.deepEqual(e.ask('list').lan,[]);
 assert.deepEqual(e.ask('create').addresses.map(a=>a.listening),[false,false,true]);
 // opened again: a new id range, so the old loop ends rather than serving twice
 e.ask('lan-on','192.168.1.23');assert.equal(e.ask('lan-up','192.168.1.23').base,3221225472+16777216);assert.equal(e.lan('192.168.1.23',3221225472),false);
 // a bind the person asked for that failed is not kept
 e.ask('lan-on','10.0.0.7');assert.equal(e.ask('lan-fail','10.0.0.7').error,'lan_unavailable');assert.deepEqual(JSON.parse(fs.readFileSync(file,'utf8')).ips,['192.168.1.23']);
 // covered by the main listener: nothing to bind, nothing kept
 const g=fixture(t),c=effectsWith(t,g,nics);c.start('0.0.0.0');assert.deepEqual(c.ask('lan-on','10.0.0.7'),{ok:true,ip:'10.0.0.7',listening:true});
 assert.ok(!fs.existsSync(path.join(g.home,'lan-v1.json')));assert.deepEqual(c.ask('lan-off','10.0.0.7').listening,true);
});
