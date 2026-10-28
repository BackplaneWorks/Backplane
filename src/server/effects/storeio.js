// Only the derived store is replaced. The append-only event log is never moved.
function store_temp(home) {
  const fs = require('fs'), path = require('path');
  const temp = fs.mkdtempSync(path.join(home, '.store-rebuild-'));
  fs.symlinkSync(path.resolve(home, 'events.jsonl'), path.join(temp, 'events.jsonl'));
  return temp;
}
function store_sync(path) {
  const fs = require('fs'); let fd;
  // Windows FlushFileBuffers needs a writable file handle. Keep POSIX file
  // opening and the separate directory-sync policy unchanged.
  try { fd=fs.openSync(path,process.platform==='win32'?'r+':'r');fs.fsyncSync(fd);return 'ok'; }
  catch(e) { return 'fail '+e.message; }
  finally { if(fd!==undefined)fs.closeSync(fd); }
}
function store_prepare(home) {
  const fs=require('fs'),path=require('path');let fd;
  try {
    fd=fs.openSync(path.join(home,'events.jsonl'),'a+');
    const size=fs.fstatSync(fd).size,last=Buffer.alloc(1);
    if(size && fs.readSync(fd,last,0,1,size-1)===1 && last[0]!==10)fs.writeSync(fd,Buffer.from('\n'));
    fs.fsyncSync(fd);
    const adopted=store_adopt(home);if(adopted!=='ok')return adopted;
    store_existing(home);return 'ok';
  }catch(e){return 'fail '+e.message}
  finally{if(fd!==undefined)fs.closeSync(fd)}
}
function store_promote(home,temp) {
  const fs=require('fs'),path=require('path');
  try {
    if(path.dirname(temp)!==home || !path.basename(temp).startsWith('.store-rebuild-'))throw Error('invalid rebuild directory');
    const next=path.join(temp,'store.db'),target=path.join(home,'store.db');
    // Db.close can leave an empty WAL and its disposable shared index on
    // macOS. Refuse uncheckpointed data or unexpected members before moving
    // the current database; never recursively discard a rebuild directory.
    const sidecars=[];
    for(const name of fs.readdirSync(temp)) {
      if(name==='store.db'||name==='events.jsonl')continue;
      if(name!=='store.db-wal'&&name!=='store.db-shm')throw Error('unexpected rebuild member retained: '+name);
      const file=path.join(temp,name),st=fs.lstatSync(file);
      if(!st.isFile()||st.isSymbolicLink())throw Error('invalid rebuild sidecar retained: '+name);
      if(name==='store.db-wal'&&st.size!==0)throw Error('uncheckpointed rebuild WAL retained');
      sidecars.push(file);
    }
    const sync=store_sync(next);if(sync!=='ok')return sync;
    const backup=target+'.previous-'+Date.now();
    for(const suffix of ['','-wal','-shm'])if(fs.existsSync(target+suffix)){fs.copyFileSync(target+suffix,backup+suffix,fs.constants.COPYFILE_EXCL);const saved=store_sync(backup+suffix);if(saved!=='ok')return saved;}
    // No old connection remains at boot. Keep its WAL evidence beside backup.
    for(const suffix of ['-wal','-shm'])if(fs.existsSync(target+suffix))fs.renameSync(target+suffix,backup+suffix+'.retired');
    fs.renameSync(next,target);
    store_sync_dir(home);
    for(const file of sidecars)fs.rmSync(file,{force:true});
    fs.unlinkSync(path.join(temp,'events.jsonl'));fs.rmdirSync(temp);
    return 'ok';
  }catch(e){return 'fail '+e.message;}
}
io_eff(CID(Disk.temp),store_temp);
io_eff(CID(Disk.promote),store_promote);
io_eff(CID(Disk.sync),store_sync);
io_eff(CID(Disk.prepare),store_prepare);

// Complete lines only; a single very long event may exceed the normal 256 KiB
// chunk. Memory is bounded by one event plus one chunk, never the whole tail.
function store_list(xs) {
  let out={$:CID(Nil)};
  for(let i=xs.length-1;i>=0;i--)out={$:CID(Con),head:xs[i],tail:out};
  return out;
}
function store_read(path,off) {
  const fs=require('fs');let fd;
  try {
    const start=Number(off);if(!Number.isSafeInteger(start)||start<0)throw Error('invalid log offset');
    fd=fs.openSync(path,'r');const size=fs.fstatSync(fd).size;
    if(start>size)throw Error('checkpoint past log end');
    if(start===size)return store_list(['ok']);
    const parts=[];let pos=start,used=0,end=-1;
    while(pos<size && end<0){
      const b=Buffer.allocUnsafe(Math.min(262144,size-pos)),n=fs.readSync(fd,b,0,b.length,pos);
      if(!n)throw Error('short log read');
      const got=b.subarray(0,n);parts.push(got);pos+=n;used+=n;
      const nl=got.lastIndexOf(10);if(nl>=0)end=used-n+nl+1;
    }
    if(end<0)throw Error('unterminated log tail');
    const bytes=Buffer.concat(parts,used).subarray(0,end);
    return store_list(['ok',...bytes.toString('utf8').slice(0,-1).split('\n')]);
  } catch(e){return store_list(['fail '+e.message])}
  finally{if(fd!==undefined)fs.closeSync(fd)}
}
io_eff(CID(Disk.read),store_read);

// A rejected append must be removed durably before another write is allowed.
// Do not depend on a PATH command, and never extend a log to an invalid offset.
function store_cut(path,len) {
  const fs=require('fs');let fd;
  try {
    if(!/^(0|[1-9][0-9]*)$/.test(len))return false;
    const n=Number(len);if(!Number.isSafeInteger(n))return false;
    fd=fs.openSync(path,'r+');if(n>fs.fstatSync(fd).size)return false;
    fs.ftruncateSync(fd,n);fs.fsyncSync(fd);return true;
  }catch(error){return false}
  finally{if(fd!==undefined)fs.closeSync(fd)}
}
io_eff(CID(Disk.cut),store_cut);


// Called only by Disk.prepare, after the hub's exclusive home lock and before
// any database handle is opened. Offline preparation never replaces a live
// handle: startup validates the entire immutable log prefix before promotion.
function store_hash(file,limit) {
  const fs=require('fs'),hash=require('crypto').createHash('sha256');
  const fd=fs.openSync(file,'r');let pos=0;
  try {
    const size=fs.fstatSync(fd).size,stop=limit===undefined?size:limit;
    if(!Number.isSafeInteger(stop)||stop<0||stop>size)throw Error('invalid prefix length');
    const b=Buffer.allocUnsafe(1048576);
    while(pos<stop){const n=fs.readSync(fd,b,0,Math.min(b.length,stop-pos),pos);if(!n)throw Error('short prefix read');hash.update(b.subarray(0,n));pos+=n;}
    return hash.digest('hex');
  }finally{fs.closeSync(fd)}
}
function store_validate(file) {
  const {Database}=require('bun:sqlite'),db=new Database(file,{readonly:true,safeIntegers:true});
  try {
    const m=Object.fromEntries(db.query('SELECT k,v FROM meta').values());
    if(m.schema!=='1'||m.codec!=='5')throw Error('snapshot schema/codec mismatch');
    for(const k of ['applied_seq','applied_off','ord'])if(!/^(0|[1-9][0-9]*)$/.test(m[k]||''))throw Error('invalid checkpoint metadata');
    // Missing tables/columns must rebuild, not silently answer empty pages.
    for(const sql of [
      'SELECT seq,off,len,wire FROM line LIMIT 0',
      'SELECT seq,ord,id,thread,src,kind,role,akind,text,at FROM entry LIMIT 0',
      'SELECT word,ord FROM term LIMIT 0','SELECT text FROM entry_fts LIMIT 0',
      'SELECT seq,id,room,src,text,hop,at FROM post LIMIT 0',
      'SELECT seq,id,conv,src,dst,kind,ref,want,head,size,at FROM mail LIMIT 0',
      'SELECT k,v,cold FROM setting LIMIT 0','SELECT domain,key,json FROM obj LIMIT 0',
      ...['project','thread','task','ask','queue','bot','room','routine','conv','mark'].map(t=>'SELECT json FROM '+t+' LIMIT 0')
    ])db.query(sql).values();
    const [count,last,end]=db.query('SELECT count(*),coalesce(max(seq),0),coalesce(max(off+len+1),0) FROM line').values()[0];
    const [ordinal]=db.query('SELECT coalesce(max(ord),0)+1 FROM entry').values()[0];
    if(BigInt(m.applied_seq)!==count||count!==last||end>BigInt(m.applied_off)||BigInt(m.ord)!==ordinal)throw Error('snapshot fact/checkpoint mismatch');
    return m;
  }finally{db.close()}
}
function store_existing(home) {
  const fs=require('fs'),path=require('path'),file=path.join(home,'store.db');
  if(!fs.existsSync(file))return;
  try{store_validate(file)}catch(error){
    // Retain the complete old cache and its WAL evidence. Sto.open will build
    // a new cache atomically from the authoritative log, before serving views.
    const old=file+'.invalid-'+Date.now();
    for(const suffix of ['','-wal','-shm'])if(fs.existsSync(file+suffix))fs.renameSync(file+suffix,old+suffix);
    store_sync_dir(home);
    console.warn('[store] incompatible/incomplete cache retained for rebuild: '+error.message);
  }
}
function store_adopt(home) {
  const fs=require('fs'),path=require('path'),dir=path.join(home,'.store-next');
  if(!fs.existsSync(dir))return 'ok';
  const file=path.join(dir,'store.db'),manifest=path.join(dir,'READY.json');let m;
  try {
    if(!fs.lstatSync(dir).isDirectory()||fs.lstatSync(dir).isSymbolicLink()||!fs.lstatSync(file).isFile()||fs.lstatSync(file).isSymbolicLink())throw Error('invalid prepared path');
    m=JSON.parse(fs.readFileSync(manifest,'utf8'));
    if(m.version!==1||m.codec!==5||m.schema!==1||!Number.isSafeInteger(m.prefixBytes)||!Number.isSafeInteger(m.appliedSeq)||!m.storeSha256||!m.prefixSha256)throw Error('invalid prepared manifest');
    if(store_hash(file)!==m.storeSha256)throw Error('prepared database hash mismatch');
    const meta=store_validate(file);
    if(meta.applied_off!==String(m.prefixBytes)||meta.applied_seq!==String(m.appliedSeq))throw Error('prepared checkpoint mismatch');
    if(store_hash(path.join(home,'events.jsonl'),m.prefixBytes)!==m.prefixSha256)throw Error('prepared log prefix mismatch');
  }catch(error){
    fs.renameSync(dir,dir+'.rejected-'+Date.now());
    console.warn('[store] prepared cache rejected; normal verified recovery continues: '+error.message);
    return 'ok';
  }
  try {
    const durable=store_sync(file);if(durable!=='ok')return durable;
    const receipt=dir+'.adopting-'+Date.now();fs.renameSync(dir,receipt);
    const temp=store_temp(home);fs.renameSync(path.join(receipt,'store.db'),path.join(temp,'store.db'));
    const result=store_promote(home,temp);if(result!=='ok')return result;
    fs.writeFileSync(path.join(receipt,'ADOPTED.json'),JSON.stringify({state:'adopted',...m})+'\n');
    console.warn('[store] prepared checkpoint adopted at byte '+m.prefixBytes+' event '+m.appliedSeq);
    return 'ok';
  }catch(error){return 'fail prepared cache promotion: '+error.message}
}

// Windows exposes no directory descriptor for fsync. File contents are still
// synced before replacement; POSIX additionally persists the directory entry.
function store_sync_dir(path) {
  if(process.platform==='win32')return;
  const fs=require('fs'),fd=fs.openSync(path,'r');try{fs.fsyncSync(fd)}finally{fs.closeSync(fd)}
}
