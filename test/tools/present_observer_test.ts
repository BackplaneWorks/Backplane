import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
const [observer,fixture,xvfb,display=':178',out='build/cadence/observer-calibration.json']=process.argv.slice(2);
assert.ok(observer&&fixture&&xvfb,'OBSERVER FIXTURE XVFB [DISPLAY OUT]');
assert.match(display,/^:\d+$/);
assert.equal(existsSync(`/tmp/.X11-unix/X${display.slice(1)}`),false,'exclusive calibration display is already owned');
const env={...process.env,DISPLAY:display};
const owned:ReturnType<typeof Bun.spawn>[]=[];
const timer=setTimeout(()=>{for(const p of owned)if(p.exitCode===null)p.kill();process.exit(124);},15000);
const samples:any[]=[];
try {
 const x=Bun.spawn([xvfb,display,'-screen','0','128x96x24','-nolisten','tcp'],{env,stdout:'ignore',stderr:'pipe'});owned.push(x);
 for(let i=0;i<100&&!existsSync(`/tmp/.X11-unix/X${display.slice(1)}`)&&x.exitCode===null;i++) await Bun.sleep(20);
 assert.equal(x.exitCode,null,'test Xvfb exited');
 const f=Bun.spawn([fixture],{env,stdin:'pipe',stdout:'pipe',stderr:'pipe'});owned.push(f);
 const reader=f.stdout.getReader();const decoder=new TextDecoder();let buffer='';
 async function line() {for(;;){const nl=buffer.indexOf('\n');if(nl>=0){const value=buffer.slice(0,nl);buffer=buffer.slice(nl+1);return value;}const r=await reader.read();assert.equal(r.done,false,'fixture terminated before acknowledgement');buffer+=decoder.decode(r.value);}}
 const window=(await line()).match(/^window (\d+)$/)?.[1];assert.ok(window);
 for(const color of ['red','green','pattern']) {
  f.stdin.write(color+'\n');await f.stdin.flush();assert.equal(await line(),'ready '+color);
  for(const [rx,ry,w,h] of [[0,0,96,64],[7,5,31,17]]) {
   const expected=Buffer.alloc(w*h*3);let at=0;
   for(let y=ry;y<ry+h;y++)for(let xx=rx;xx<rx+w;xx++) {
    expected[at++]=color==='red'?255:color==='pattern'?(17*xx+3*y)&255:0;
    expected[at++]=color==='green'?255:color==='pattern'?(5*xx+19*y)&255:0;
    expected[at++]=color==='pattern'?(11*xx+7*y)&255:0;
   }
   const digest=createHash('sha256').update(expected).digest('hex');
   const p=Bun.spawn([observer,window!,String(rx),String(ry),String(w),String(h),'120','60','calibration'],{env,stdout:'pipe',stderr:'pipe'});owned.push(p);
   const text=await new Response(p.stdout).text();assert.equal(await p.exited,0,await new Response(p.stderr).text());
   const rows=text.trim().split('\n').map(l=>JSON.parse(l));const header=rows.shift();
   assert.equal(header.boundary,'xserver-readback');assert.equal(header.physical_scanout_verified,false);
   assert.equal(header.width,w);assert.equal(header.height,h);assert.equal(header.x,rx);assert.equal(header.y,ry);
   assert.ok(rows.length>=4);assert.ok(rows.every(r=>r.kind==='pixels'&&r.pixel_sha256===digest));
   samples.push({color,roi:[rx,ry,w,h],expected_sha256:digest,observations:rows});
  }
 }
 assert.notEqual(samples[0].expected_sha256,samples[2].expected_sha256,'unchanged red pixels cannot satisfy a green demand');
 f.stdin.write('quit\n');await f.stdin.flush();assert.equal(await f.exited,0);
 const hashes=Object.fromEntries([observer,fixture,'test/tools/present_observer.c','test/tools/present_observer_fixture.c'].map(p=>[p,createHash('sha256').update(readFileSync(p)).digest('hex')]));
 writeFileSync(out,JSON.stringify({scope:'RGB/ROI correctness only; X-server readback, not compositor/scanout or FPS acceptance',hashes,samples},null,2));
 console.log('PASS independent X-server RGB observer: full/cropped red/green/nonuniform pixels match known SHA256; stale color differs; no scanout claim');
} finally {
 clearTimeout(timer);
 for(const p of [...owned].reverse())if(p.exitCode===null){p.kill();await p.exited;}
}
