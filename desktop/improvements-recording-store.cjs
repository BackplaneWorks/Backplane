'use strict';
const nativeFs=require('node:fs'),path=require('node:path');
const LIMITS=Object.freeze({maxBytes:9*1024*1024,diskBytes:32*1024*1024,ttlMs:86400000,keep:2});
const valid=id=>typeof id==='string'&&/^[a-f0-9]{32}$/.test(id);
function createRecordingStore(home,{fs=nativeFs,now=Date.now}={}){
 const dir=path.join(home,'diagnostics-recordings');
 const owned=n=>/^[a-f0-9]{32}\.webm$/.test(n)||/^pending-[a-f0-9]{32}\.webm$/.test(n);
 function directory(){try{fs.mkdirSync(dir,{mode:0o700});}catch(e){if(e.code!=='EEXIST')throw e;}const st=fs.lstatSync(dir);if(!st.isDirectory()||st.isSymbolicLink())throw Error('Unsafe recording storage');fs.chmodSync(dir,0o700);}
 function stat(name){const st=fs.lstatSync(path.join(dir,name));if(!st.isFile()||st.nlink!==1||st.isSymbolicLink())throw Error('Unsafe recording storage');return st;}
 function rows(){directory();return fs.readdirSync(dir).map(name=>{if(!owned(name))throw Error('Unexpected recording storage entry');return{name,stat:stat(name)};});}
 function usage(){const all=rows();return Math.max(all.reduce((n,r)=>n+r.stat.size,0),all.reduce((n,r)=>n+Math.max(r.stat.size,(r.stat.blocks||0)*512),0))+(fs.lstatSync(dir).blocks||0)*512;}
 function removeName(name){stat(name);fs.unlinkSync(path.join(dir,name));}
 function prune(reserve=0){let all=rows();for(const r of all)if(r.name.startsWith('pending-')||!r.stat.size||r.stat.size>LIMITS.maxBytes||r.stat.mtimeMs<now()-LIMITS.ttlMs||r.stat.mtimeMs>now()+60000)removeName(r.name);
  all=rows().sort((a,b)=>a.stat.mtimeMs-b.stat.mtimeMs);
  while(all.length>LIMITS.keep-(reserve?1:0)||usage()+reserve+8192>LIMITS.diskBytes){const old=all.shift();if(!old)throw Error('Recording storage full');removeName(old.name);}
 }
 function put(id,input){if(!valid(id)||!(input instanceof ArrayBuffer||ArrayBuffer.isView(input))||!input.byteLength||input.byteLength>LIMITS.maxBytes)throw Error('Invalid recording');
  const data=Buffer.from(input instanceof ArrayBuffer?new Uint8Array(input):new Uint8Array(input.buffer,input.byteOffset,input.byteLength));if(!data.subarray(0,4).equals(Buffer.from([0x1a,0x45,0xdf,0xa3])))throw Error('Expected WebM recording');
  let fd,temporary='pending-'+id+'.webm';
  try{prune(data.length);if(fs.existsSync(path.join(dir,id+'.webm')))throw Error('Recording already exists');fd=fs.openSync(path.join(dir,temporary),fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL|(fs.constants.O_NOFOLLOW||0),0o600);
   const st=fs.fstatSync(fd);if(!st.isFile()||st.nlink!==1)throw Error('Unsafe recording storage');fs.fchmodSync(fd,0o600);
   if(fs.writeSync(fd,data,0,data.length,0)!==data.length)throw Error('Short recording write');fs.fsyncSync(fd);fs.closeSync(fd);fd=undefined;
   if(usage()>LIMITS.diskBytes)throw Error('Recording storage full');fs.renameSync(path.join(dir,temporary),path.join(dir,id+'.webm'));if(usage()>LIMITS.diskBytes){removeName(id+'.webm');throw Error('Recording storage full');}return true;
  }catch{try{if(fd!==undefined)fs.closeSync(fd);}catch{}try{removeName(temporary);}catch{}throw Error('Could not keep the recording. Free local storage and record again.');}
 }
 function read(id){if(!valid(id))throw Error('Invalid recording');prune();const name=id+'.webm',before=stat(name);let fd;try{fd=fs.openSync(path.join(dir,name),fs.constants.O_RDONLY|(fs.constants.O_NOFOLLOW||0));const after=fs.fstatSync(fd);if(after.ino!==before.ino||after.dev!==before.dev||after.nlink!==1)throw Error('Unsafe recording storage');return fs.readFileSync(fd);}finally{if(fd!==undefined)fs.closeSync(fd);}}
 function remove(id){if(!valid(id))throw Error('Invalid recording');directory();try{removeName(id+'.webm');}catch(e){if(e.code!=='ENOENT')throw e;}return true;}
 function clear(){for(const r of rows())removeName(r.name);return true;}
 return{put,read,remove,clear,prune,usage,dir};
}
module.exports={createRecordingStore,LIMITS};
