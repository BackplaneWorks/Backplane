// These effects copy original GLB bytes, never decoded Bend byte lists. Heavy
// preparation/hash work runs only in the dedicated --review asset child.
function ra_modules() {
  return {fs:require('node:fs'),path:require('node:path'),crypto:require('node:crypto')};
}
const RA_MAX_BYTES=64*1024*1024, RA_KEEP_BYTES=256*1024*1024, RA_KEEP_FILES=8, RA_MAX_AGE=7*86400000;
function ra_json(value) {
  const text=JSON.stringify(value);
  return Buffer.byteLength(text)<=16000?text:'{"error":"Asset response exceeds its metadata limit."}';
}
function ra_error(error) { return ra_json({error:String(error?.message||error).slice(0,400)}); }
function ra_stamp(st) {
  return [st.dev,st.ino,st.size,st.mtimeNs/1000000000n,st.mtimeNs%1000000000n,st.ctimeNs/1000000000n,st.ctimeNs%1000000000n].join(':');
}
function ra_regular(fs,p,{privateFile=false}={}) {
  const st=fs.lstatSync(p,{bigint:true});
  if(!st.isFile()||st.isSymbolicLink()||st.nlink!==1n)throw Error('Asset must be a regular file without links.');
  if(privateFile&&typeof process.getuid==='function'&&st.uid!==BigInt(process.getuid()))throw Error('Asset is not owned by this user.');
  if(st.size<20n||st.size>BigInt(RA_MAX_BYTES))throw Error('GLB asset must be between 20 bytes and 64 MiB.');
  return st;
}
function ra_cache(create=false) {
  const {fs,path}=ra_modules(),base=process.env.XDG_CACHE_HOME||path.join(process.env.HOME||require('node:os').homedir(),'.cache');
  const parent=path.join(base,'backplane-bend'),dir=path.join(parent,'render-assets');
  if(create)fs.mkdirSync(parent,{recursive:true,mode:0o700});
  // A cache root supplied through a symlink must not turn lookup into an
  // arbitrary-file capability. Parent cache locations may themselves be links;
  // the dedicated private render-assets directory may not.
  if(create&&!fs.existsSync(dir))fs.mkdirSync(dir,{mode:0o700});
  const st=fs.lstatSync(dir);
  if(!st.isDirectory()||st.isSymbolicLink())throw Error('Private asset cache directory is unavailable.');
  if(typeof process.getuid==='function'&&st.uid!==process.getuid())throw Error('Asset cache is not owned by this user.');
  if(create)fs.chmodSync(dir,0o700);
  if((fs.statSync(dir).mode&0o077)!==0)throw Error('Asset cache must be private.');
  return dir;
}
function ra_authorize(cwd,requested,canonical,kind,expected) {
  const {fs,path}=ra_modules();
  if(!['board','3d','mech','step','glb','2','3','4'].includes(kind))throw Error('Unsupported viewer asset kind.');
  const root=fs.realpathSync(cwd),source=path.resolve(cwd,requested||canonical||''),real=fs.realpathSync(source),ext=path.extname(source).toLowerCase();
  if(!['.kicad_pcb','.step','.stp','.glb'].includes(ext))throw Error('Only saved PCB, STEP or GLB assets are supported.');
  const isCanonical=!!canonical&&source===path.resolve(cwd,canonical)&&ext==='.kicad_pcb';
  if(!(real.startsWith(root+path.sep)||isCanonical))throw Error('Asset source must be inside this project or its configured canonical board.');
  const st=fs.statSync(source,{bigint:true});
  if(!st.isFile())throw Error('Asset source is not a regular file.');
  // A source symlink resolving within the authorized project is acceptable;
  // publication and lookup themselves never create or follow cache links.
  const revision=ra_stamp(st);
  if(!expected||revision!==expected)throw Error('Saved source revision changed or is not available; refresh the viewer.');
  return {source,realSource:real,revision,kind:ext==='.kicad_pcb'?'board':ext==='.glb'?'glb':'step'};
}
function ra_read(fs,fd,length,position) {
  const b=Buffer.allocUnsafe(length);let at=0;
  while(at<length){const n=fs.readSync(fd,b,at,length-at,position+at);if(!n)throw Error('GLB asset is truncated.');at+=n;}
  return b;
}
function ra_glb(fs,fd,size,inspect=false) {
  const head=ra_read(fs,fd,20,0);
  if(head.readUInt32LE(0)!==0x46546c67||head.readUInt32LE(4)!==2||head.readUInt32LE(8)!==size)throw Error('Invalid GLB header or declared byte length.');
  let at=12,chunks=0,json=null;
  while(at<size) {
    if(++chunks>32||at+8>size)throw Error('Invalid GLB chunk table.');
    const h=ra_read(fs,fd,8,at),n=h.readUInt32LE(0),type=h.readUInt32LE(4);
    if(n%4||at+8+n>size||(!n&&type===0x4e4f534a))throw Error('Invalid GLB chunk length.');
    if(chunks===1) {
      if(type!==0x4e4f534a)throw Error('GLB must start with a JSON chunk.');
      if(inspect){json=JSON.parse(ra_read(fs,fd,n,at+8).toString('utf8'));if(json.asset?.version!=='2.0')throw Error('Unsupported glTF asset version.');}
    }
    at+=8+n;
  }
  if(at!==size)throw Error('Invalid GLB length.');
  return json;
}
function ra_open(fs,p) {
  const before=ra_regular(fs,p),fd=fs.openSync(p,fs.constants.O_RDONLY|(fs.constants.O_NOFOLLOW||0));
  try {
    const actual=fs.fstatSync(fd,{bigint:true});
    if(ra_stamp(before)!==ra_stamp(actual))throw Error('Asset changed while opening.');
    return {fd,st:actual};
  } catch(e) {fs.closeSync(fd);throw e;}
}
function ra_hash(fs,crypto,fd,size,outFd=null) {
  const hash=crypto.createHash('sha256'),chunk=Buffer.allocUnsafe(256*1024);let offset=0;
  while(offset<size) {
    const n=fs.readSync(fd,chunk,0,Math.min(chunk.length,size-offset),offset);
    if(!n)throw Error('Asset changed while copying.');
    hash.update(chunk.subarray(0,n));
    if(outFd!==null){let wrote=0;while(wrote<n){const count=fs.writeSync(outFd,chunk,wrote,n-wrote);if(count<=0)throw Error('Asset copy made no progress.');wrote+=count;}}
    offset+=n;
  }
  return hash.digest('hex');
}
function ra_prune(dir,keep) {
  const {fs,path}=ra_modules(),now=Date.now(),files=[];
  for(const name of fs.readdirSync(dir)) {
    if(/^\.asset-\d+-[0-9a-f]{24}\.tmp$/.test(name)){
      try {const p=path.join(dir,name),st=fs.lstatSync(p);if(st.isFile()&&!st.isSymbolicLink()&&st.nlink===1&&now-st.mtimeMs>3600000&&(typeof process.getuid!=='function'||st.uid===process.getuid()))fs.unlinkSync(p);}catch{}
      continue;
    }
    if(!/^[0-9a-f]{64}\.glb$/.test(name))continue;
    const p=path.join(dir,name);
    try {const st=ra_regular(fs,p,{privateFile:true});files.push({p,name,size:Number(st.size),time:Number(st.mtimeNs/1000000n)});}catch{}
  }
  files.sort((a,b)=>(Number(b.name===keep)-Number(a.name===keep))||b.time-a.time);let bytes=0,count=0;
  for(const f of files) {
    if(f.name!==keep&&(count>=RA_KEEP_FILES||bytes+f.size>RA_KEEP_BYTES||now-f.time>RA_MAX_AGE)){fs.unlinkSync(f.p);continue;}
    count++;bytes+=f.size;
  }
}
// A published asset is remembered per (project, kind, exact revision): the
// revision is the source's dev:ino:size:mtime:ctime stamp, so it names one
// file. The hub answers a later request for that same revision from this
// private index without starting a child (about 0.24 s of every request);
// any doubt falls back to the child.
function ra_memo_name(crypto,cwd,kind,revision) {
  return crypto.createHash('sha256').update(JSON.stringify([cwd,String(kind),revision])).digest('hex')+'.json';
}
function ra_memo_dir(create) {
  const {fs,path}=ra_modules(),dir=path.join(ra_cache(create),'memo');
  if(create&&!fs.existsSync(dir))fs.mkdirSync(dir,{mode:0o700});
  const st=fs.lstatSync(dir);
  if(!st.isDirectory()||st.isSymbolicLink()||(typeof process.getuid==='function'&&st.uid!==process.getuid())||(st.mode&0o077)!==0)throw Error('Asset memo directory is unavailable.');
  return dir;
}
function ra_memo_write(cwd,kind,revision,asset) {
  const {fs,path,crypto}=ra_modules();let temp=null;
  try {
    const dir=ra_memo_dir(true),name=ra_memo_name(crypto,fs.realpathSync(cwd),kind,revision);
    temp=path.join(dir,`.memo-${process.pid}-${crypto.randomBytes(12).toString('hex')}.tmp`);
    fs.writeFileSync(temp,JSON.stringify({kind:String(kind),revision,asset}),{mode:0o600,flag:'wx'});fs.renameSync(temp,path.join(dir,name));temp=null;
    const names=fs.readdirSync(dir).filter(n=>/^[0-9a-f]{64}\.json$/.test(n)).map(n=>({n,t:fs.statSync(path.join(dir,n)).mtimeMs})).sort((a,b)=>b.t-a.t);
    for(const f of names.slice(64))try{fs.unlinkSync(path.join(dir,f.n));}catch{}
  }catch{}finally{if(temp)try{fs.unlinkSync(temp);}catch{}}
}
function render_asset_memo(cwd,kind,expected) {
  const {fs,path,crypto}=ra_modules();let file=null;
  try {
    if(!expected)return '';
    const dir=ra_memo_dir(false),p=path.join(dir,ra_memo_name(crypto,fs.realpathSync(cwd),kind,expected));
    if(!fs.existsSync(p))return '';
    const st=fs.lstatSync(p);
    if(!st.isFile()||st.isSymbolicLink()||st.nlink!==1||(typeof process.getuid==='function'&&st.uid!==process.getuid())||(st.mode&0o077)!==0||st.size>16000)return '';
    const memo=JSON.parse(fs.readFileSync(p,'utf8')),asset=memo?.asset;
    if(memo?.revision!==expected||memo?.kind!==String(kind)||!asset||asset.revision!==expected||!/^[0-9a-f]{64}$/.test(asset.sha256||''))return '';
    // the source is still exactly the revision asked for
    if(ra_stamp(fs.statSync(asset.source,{bigint:true}))!==expected)return '';
    // and its published copy is still the private, immutable file it was
    const a=path.join(ra_cache(false),asset.sha256+'.glb');ra_regular(fs,a,{privateFile:true});file=ra_open(fs,a);
    if((Number(file.st.mode)&0o222)!==0||Number(file.st.size)!==asset.bytes)return '';
    return ra_json({asset});
  }catch{return '';}finally{if(file)fs.closeSync(file.fd);}
}
function render_asset_authorize(cwd,requested,canonical,kind,expected) {
  try {return ra_json(ra_authorize(cwd,requested,canonical,kind,expected));}catch(e){return ra_error(e);}
}
function render_asset_publish(cwd,source,canonical,kind,expected,glb,missing) {
  const {fs,path,crypto}=ra_modules();let input=null,outFd=null,temp=null;
  try {
    const authorized=ra_authorize(cwd,source,canonical,kind,expected),dir=ra_cache(true);
    // The GLB path is produced by the existing exporter in this child, never
    // accepted as an HTTP client path. Native GLB uses its authorized source.
    input=ra_open(fs,glb);const size=Number(input.st.size),json=ra_glb(fs,input.fd,size,true);
    temp=path.join(dir,`.asset-${process.pid}-${crypto.randomBytes(12).toString('hex')}.tmp`);
    outFd=fs.openSync(temp,'wx',0o600);
    const sha256=ra_hash(fs,crypto,input.fd,size,outFd);fs.fsyncSync(outFd);fs.closeSync(outFd);outFd=null;
    if(ra_stamp(fs.fstatSync(input.fd,{bigint:true}))!==ra_stamp(input.st)||ra_stamp(fs.statSync(glb,{bigint:true}))!==ra_stamp(input.st))throw Error('Exported GLB changed while publishing.');
    const after=ra_authorize(cwd,source,canonical,kind,expected);
    if(after.realSource!==authorized.realSource)throw Error('Saved source target changed while preparing the asset.');
    const destination=path.join(dir,sha256+'.glb');
    if(fs.existsSync(destination)) {
      const old=ra_open(fs,destination);try {ra_regular(fs,destination,{privateFile:true});if((Number(old.st.mode)&0o222)!==0)throw Error('Cached asset is not immutable.');if(Number(old.st.size)!==size||ra_hash(fs,crypto,old.fd,size)!==sha256)throw Error('Immutable cached asset failed its content hash.');}finally{fs.closeSync(old.fd);}
      fs.unlinkSync(temp);temp=null;
    }else {fs.chmodSync(temp,0o444);fs.renameSync(temp,destination);temp=null;}
    ra_prune(dir,sha256+'.glb');
    const extras=json.asset?.extras||{},orientation=extras.unit==='mm'&&String(extras.up).toUpperCase()==='Z'?'step-z-up-mm':authorized.kind==='step'?'step-z-up-mm':'gltf-y-up';
    const asset={url:'/viewer-asset?id='+sha256,source:authorized.source,revision:authorized.revision,bytes:size,sha256,orientation};
    if(missing)asset.missingModels=missing.slice(0,500);
    ra_memo_write(cwd,kind,expected,asset);
    return ra_json({asset});
  }catch(e){return ra_error(e);}finally{if(input)fs.closeSync(input.fd);if(outFd!==null)fs.closeSync(outFd);if(temp)try{fs.unlinkSync(temp);}catch{}}
}
function render_asset_lookup(id) {
  const {fs,path}=ra_modules();let file=null;
  try {
    if(!/^[0-9a-f]{64}$/.test(id))throw Error('Invalid asset identifier.');
    const p=path.join(ra_cache(false),id+'.glb');ra_regular(fs,p,{privateFile:true});file=ra_open(fs,p);ra_glb(fs,file.fd,Number(file.st.size));
    if((Number(file.st.mode)&0o222)!==0)throw Error('Cached asset is not immutable.');
    return ra_json({path:p,size:Number(file.st.size)});
  }catch(e){return ra_error(e);}finally{if(file)fs.closeSync(file.fd);}
}
io_eff(CID(RenderAssetFx.authorize),render_asset_authorize);
io_eff(CID(RenderAssetFx.publish),render_asset_publish);
io_eff(CID(RenderAssetFx.lookup),render_asset_lookup);
io_eff(CID(RenderAssetFx.memo),render_asset_memo);
