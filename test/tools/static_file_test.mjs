// Production HTTP static sender, real files, deterministic socket wakeups.
import assert from 'node:assert/strict';
import {test,after} from 'node:test';
import * as fs from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {gzipSync,brotliCompressSync} from 'node:zlib';
import {runInNewContext} from 'node:vm';
const source=fs.readFileSync('src/server/effects/host.js','utf8');
const start=source.indexOf('function sock_send_static('),end=source.indexOf('\nfunction sock_send_preview(',start);
assert.ok(start>=0&&end>start);const effect=source.slice(start,end);
const dir=fs.mkdtempSync(join(tmpdir(),'bp-static-'));
after(()=>fs.rmSync(dir,{recursive:true,force:true}));
const file=join(dir,'main.js'),bytes=Buffer.alloc(4*65536+39);
for(let i=0;i<bytes.length;i++)bytes[i]=i%251;
fs.writeFileSync(file,bytes);const zipped=gzipSync(bytes);fs.writeFileSync(file+'.gz',zipped);
const frames="X-Frame-Options: DENY\r\nContent-Security-Policy: frame-ancestors 'none'\r\n";
const spec=(extra={})=>({path:file,ctype:'text/javascript; charset=utf-8',cache:'no-cache',extra:'',coding:0,piece:65536,...extra});
function sender({backend='bun',platform=process.platform,maxWrite=Infinity,writes=[],override={},throwWrite=false}={}){
  const sent=[],reads=[],parks=[],opened=new Set(),closed=[];let clock=0,errno=0,live=true;
  const shim={...fs,...override,
    openSync(...a){const fd=(override.openSync||fs.openSync)(...a);opened.add(fd);return fd;},
    closeSync(fd){assert.ok(opened.delete(fd),'close once');closed.push(fd);return(override.closeSync||fs.closeSync)(fd);},
    readFileSync(){assert.fail('whole file read is forbidden');},
    readSync(fd,b,off,n,pos){assert.ok(b.length<=65536);assert.ok(n<=65536);reads.push({n,pos});return(override.readSync||fs.readSync)(fd,b,off,n,pos);},
  };
  const put=b=>{if(throwWrite)throw{errno:-32};const n=writes.length?writes.shift():Math.min(b.length,maxWrite);if(n>0)sent.push(Buffer.from(b.subarray(0,n)));return n;};
  const k=()=>assert.fail('effect does not invoke continuation itself');
  const sandbox={Buffer,process:{platform},performance:{now:()=>clock},require:n=>{assert.equal(n,'fs');return shim;},
    io_tup:(socket,result)=>({socket,result}),io_done:()=>({ok:true}),io_fail:errno=>({ok:false,errno}),host_errno:e=>Math.abs(e.errno||5),
    io_sys:()=>{assert.notEqual(platform,'win32');return{mac:platform==='darwin',ptr:(b,at)=>b.subarray(at),errno:()=>errno};},
    host_write:(socket,b,n)=>{assert.equal(socket,73);const r=put(b.subarray(0,n));errno=r===-11&&platform==='darwin'?35:-r;return r<0?-1:r;},
    io_park_on:(fd,out,continuation,more,deadline)=>{assert.equal(continuation,k);parks.push({fd,out,more,deadline});},
  };
  if(backend==='bun')sandbox.BP_H={EAGAIN:11,is:s=>s===73,get:()=>live?{write:put}:undefined};
  const send=runInNewContext(`(${effect})`,sandbox);
  return{sent,reads,parks,opened,closed,start:r=>send(73,r||spec(),k),disconnect(){live=false;},
    resume(time=clock){clock=time;assert.ok(parks.length);return parks.shift().more();},
    drain(r){let fuel=10000;while(r===undefined&&fuel-->0)r=this.resume();assert.ok(fuel>0);assert.equal(opened.size,0);return r;},
    response(){const b=Buffer.concat(sent),at=b.indexOf('\r\n\r\n');assert.ok(at>0);return{head:b.subarray(0,at+4).toString(),body:b.subarray(at+4)};},
  };
}
function header({status='200 OK',type='text/javascript; charset=utf-8',length=bytes.length,cache='no-cache',extra='',gzip=false,br=false}={}){
  return `HTTP/1.1 ${status}\r\nContent-Type: ${type}\r\nContent-Length: ${length}\r\nCache-Control: ${cache}\r\nConnection: close\r\n${extra}${status==='200 OK'?`${br?'Content-Encoding: br\r\n':gzip?'Content-Encoding: gzip\r\n':''}Vary: Accept-Encoding\r\n`:''}\r\n`;
}
for(const options of [{backend:'bun'},{backend:'posix'},{backend:'bun',platform:'win32'},{backend:'bun',platform:'darwin'},{backend:'posix',platform:'darwin'}]){
  test(`${options.backend}/${options.platform||process.platform}: exact body/headers, bounded pieces and fair yields`,()=>{
    const s=sender(options),r=s.start();assert.equal(r,undefined);assert.equal(s.parks[0].fd,undefined);assert.equal(s.reads.length,1);
    assert.equal(s.drain(r).result.ok,true);const res=s.response();assert.equal(res.head,header());assert.deepEqual(res.body,bytes);assert.equal(s.closed.length,1);
  });
}
test('gzip preference, original MIME/cache/frame headers and Vary survive unchanged',()=>{
  const s=sender();assert.equal(s.drain(s.start(spec({coding:1,ctype:'text/html; charset=utf-8',cache:'public, max-age=31536000, immutable',extra:frames}))).result.ok,true);
  const r=s.response();assert.equal(r.head,header({type:'text/html; charset=utf-8',cache:'public, max-age=31536000, immutable',extra:frames,gzip:true,length:zipped.length}));assert.deepEqual(r.body,zipped);
});
for(const reason of ['missing','empty','directory','unreadable']){
  test(`unusable gzip (${reason}) falls back to exact plain bytes`,()=>{
    const p=join(dir,'fallback-'+reason+'.js');fs.writeFileSync(p,bytes);
    if(reason==='empty')fs.writeFileSync(p+'.gz','');
    if(reason==='directory')fs.mkdirSync(p+'.gz');
    const s=sender({override:reason==='unreadable'?{openSync(path,...args){if(path.endsWith('.gz'))throw{errno:-13};return fs.openSync(path,...args);}}:{}});
    assert.equal(s.drain(s.start(spec({path:p,coding:1}))).result.ok,true);assert.equal(s.response().head,header());assert.deepEqual(s.response().body,bytes);
  });
}
for(const reason of ['missing','empty','directory','nul']){
  test(`unusable plain (${reason}) retains the original 404`,()=>{
    const p=join(dir,'plain-'+reason);if(reason==='empty')fs.writeFileSync(p,'');if(reason==='directory')fs.mkdirSync(p);
    const s=sender();assert.equal(s.drain(s.start(spec({path:reason==='nul'?p+'\0':p,coding:1}))).result.ok,true);
    assert.equal(s.response().head,header({status:'404 Not Found',type:'text/plain; charset=utf-8',length:9}));assert.equal(s.response().body.toString(),'not found');
  });
}
test('static symlinks retain their existing supported behavior',()=>{
  const p=join(dir,'linked.js');fs.symlinkSync(file,p);const s=sender();s.drain(s.start(spec({path:p})));assert.deepEqual(s.response().body,bytes);
});
for(const backend of ['bun','posix']){
  test(`${backend}: partial headers/body, repeated EAGAIN and zero writes preserve bytes`,()=>{
    const s=sender({backend,maxWrite:113,writes:[9,-11,0]});assert.equal(s.start(),undefined);assert.equal(s.reads.length,0);assert.equal(s.parks[0].out,true);
    assert.equal(s.resume(999),undefined);assert.equal(s.reads.length,0);assert.equal(s.drain(s.resume(1000)).result.ok,true);assert.deepEqual(s.response().body,bytes);
  });
  test(`${backend}: idle expiry and backend errors release every descriptor`,()=>{
    const s=sender({backend,writes:[-11,-11]});s.start();assert.equal(s.resume(29999),undefined);assert.equal(s.resume(30000).result.errno,process.platform==='darwin'?60:110);assert.equal(s.opened.size,0);
    const failed=sender({backend,writes:[-32]});assert.equal(failed.start().result.errno,32);assert.equal(failed.opened.size,0);
    const thrown=sender({backend,throwWrite:true});assert.equal(thrown.start().result.errno,32);assert.equal(thrown.opened.size,0);
  });
}
test('write progress renews timeout and disconnected handles release the file',()=>{
  const renewed=sender({writes:[1,-11,1,-11]});renewed.start();renewed.resume(29999);assert.equal(renewed.drain(renewed.resume(30001)).result.ok,true);
  const s=sender({writes:[-11]});s.start();s.disconnect();assert.equal(s.resume().result.errno,9);assert.equal(s.opened.size,0);
});
test('short reads are complete; truncation/read errors cannot append an error body',()=>{
  const short=sender({override:{readSync:(fd,b,off,n,pos)=>fs.readSync(fd,b,off,Math.min(n,127),pos)}});assert.equal(short.drain(short.start()).result.ok,true);assert.deepEqual(short.response().body,bytes);
  const truncated=sender({override:{readSync:()=>0}});assert.equal(truncated.start().result.errno,5);assert.equal(truncated.opened.size,0);assert.equal(truncated.response().body.length,0);
  const failed=sender({override:{readSync:()=>{throw{errno:-5};}}});assert.equal(failed.start().result.errno,5);assert.equal(failed.opened.size,0);
});
test('open descriptor pins replacement and initial length despite path changes',()=>{
  const p=join(dir,'replaced.js');fs.writeFileSync(p,bytes);const s=sender();const r=s.start(spec({path:p}));fs.renameSync(p,p+'.old');fs.writeFileSync(p,'new');fs.appendFileSync(p+'.old','appended');
  assert.equal(s.drain(r).result.ok,true);assert.deepEqual(s.response().body,bytes);
});
for(const bad of [{piece:0},{piece:65537},{piece:NaN},{coding:3},{ctype:'text/html\r\nInjected: true'},{cache:'\0'},{extra:'\0'}]){
  test(`invalid typed metadata ${JSON.stringify(bad)} fails before opening/sending`,()=>{const s=sender();assert.equal(s.start(spec(bad)).result.errno,22);assert.equal(s.sent.length,0);assert.equal(s.opened.size,0);});
}
test('brotli preference: .br first, else .gz, else plain bytes',()=>{
  const p=join(dir,'brotli.js');fs.writeFileSync(p,bytes);const gz=gzipSync(bytes),br=brotliCompressSync(bytes);fs.writeFileSync(p+'.gz',gz);fs.writeFileSync(p+'.br',br);
  let s=sender();assert.equal(s.drain(s.start(spec({path:p,coding:2}))).result.ok,true);assert.equal(s.response().head,header({br:true,length:br.length}));assert.deepEqual(s.response().body,br);
  s=sender();assert.equal(s.drain(s.start(spec({path:p,coding:1}))).result.ok,true);assert.equal(s.response().head,header({gzip:true,length:gz.length}));
  fs.rmSync(p+'.br');s=sender();assert.equal(s.drain(s.start(spec({path:p,coding:2}))).result.ok,true);assert.equal(s.response().head,header({gzip:true,length:gz.length}));assert.deepEqual(s.response().body,gz);
  fs.rmSync(p+'.gz');s=sender();assert.equal(s.drain(s.start(spec({path:p,coding:2}))).result.ok,true);assert.equal(s.response().head,header());assert.deepEqual(s.response().body,bytes);
});
