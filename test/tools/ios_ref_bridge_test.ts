import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {encode} from './ios_test_cbor';
import {cbor,plain} from '../../src/web/wire.js';
const [bridge, fixture] = process.argv.slice(2);
const box:any={console}, server:any={console};
vm.runInNewContext(readFileSync(bridge,'utf8'),box);
vm.runInNewContext(readFileSync(fixture,'utf8'),server);
const B=box.Backplane, keys=JSON.parse(B.headerKeys());
const decode=(cmd:any)=>plain(cbor(new Uint8Array(Buffer.from(cmd.data,'base64')),keys.keys,keys.words));
const recv=(hub:string,o:any)=>{const b:number[]=[];encode(o,b);return JSON.parse(B.recv(hub,Buffer.from(b).toString('base64'),false,false));};
const req=(hub:string,uri:string)=>JSON.stringify({hub,uri});
const thread=(id:string,project:string,title:string)=>({$:'ThreadCreated',id,project,title,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:100});
B.start('link-test','{}'); B.hubs(['lap:3787','box:3787']);
B.online('lap:3787',true,true);B.online('box:3787',true,true);
recv('lap:3787',{t:'log',items:[{$:'ProjectCreated',id:'p1',title:'Lap',root:'/lap',at:50},thread('t2','p1','Duplicate')]});
recv('box:3787',{t:'log',items:[{$:'ProjectCreated',id:'p2',title:'Box',root:'/box',at:50},thread('t2','p2','Two'),thread('link~t2','p2','Remote')]});
for(const uri of ['backplane://thread/t2','backplane://open?thread=t2','https://box:3787/#/thread/t2']) {
 const out=JSON.parse(B.act('ref-open',req('box:3787',uri)));
 assert.equal(out.screen.hub,'box:3787');assert.equal(out.screen.thread.id,'box:3787|t2');
}
assert.equal(JSON.parse(B.act('ref-open',req('box:3787','backplane://thread/link~t2'))).screen.thread.id,'box:3787|link~t2');
for(const [hub,uri] of [['unknown:3787','backplane://thread/t2'],['box:3787','backplane://thread/missing'],['box:3787','backplane://surprise?url=http://bad']]) {
 const out=JSON.parse(B.act('ref-open',req(hub,uri)));
 assert.equal(out.screen.thread.id,'box:3787|link~t2');
 assert.equal(out.cmds.filter((c:any)=>c.type==='send').length,0);
}
// Exercise recent-first routing through the actual production bridge and
// Session.render: do not let complete local history conceal a missing route.
B.start('recent-link-test','{}');B.hubs(['lap:3787','box:3787']);
B.online('lap:3787',true,true);B.online('box:3787',true,true);
recv('box:3787',{t:'mobile',generation:'link-generation'});
const out=JSON.parse(B.act('ref-open',req('box:3787','backplane://thread/t2')));
const r=out.cmds.filter((c:any)=>c.type==='send').map(decode).find((r:any)=>r.m==='mobile.screen'&&r.p.action==='ref-open');
assert.ok(r,'link preempts recent presentation refresh on owning hub');
assert.equal(r.p.key,'box:3787');
const result=JSON.parse(server.RecentFixture.render(JSON.stringify(r.p)));
const shown=recv('box:3787',{t:'reply',id:r.id,ok:true,result});
assert.equal(shown.screen.thread.id,'box:3787|t2');
console.log('PASS actual production bridge canonical/legacy/Web links, two-hub duplicate IDs, remote IDs, unresolved rejection and recent-first session routing');
