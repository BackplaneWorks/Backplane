// Component metadata pages, kept per (project, PCB revision, schematic
// revision). The first-page child parses the saved CAD once and leaves every
// page's exact response here; the hub answers a later page from it after
// re-checking both files' stamps, instead of starting a child that parses
// both files again for one page. Any doubt answers "" and the child runs.
function cc_modules() { return {fs:require('node:fs'),path:require('node:path'),crypto:require('node:crypto')}; }
function cc_stamp(st) {
  return [st.dev,st.ino,st.size,st.mtimeNs/1000000000n,st.mtimeNs%1000000000n,st.ctimeNs/1000000000n,st.ctimeNs%1000000000n].join(':');
}
function cc_dir(create) {
  const {fs,path}=cc_modules(),base=process.env.XDG_CACHE_HOME||path.join(process.env.HOME||require('node:os').homedir(),'.cache');
  const parent=path.join(base,'backplane-bend'),dir=path.join(parent,'component-pages');
  if(create){fs.mkdirSync(parent,{recursive:true,mode:0o700});if(!fs.existsSync(dir))fs.mkdirSync(dir,{mode:0o700});}
  const st=fs.lstatSync(dir);
  if(!st.isDirectory()||st.isSymbolicLink()||(typeof process.getuid==='function'&&st.uid!==process.getuid())||(st.mode&0o077)!==0)throw Error('Component page cache is unavailable.');
  return dir;
}
function cc_name(crypto,cwd,pcbRev,schRev,offset) {
  return crypto.createHash('sha256').update(JSON.stringify([cwd,pcbRev,schRev,String(offset)])).digest('hex')+'.json';
}
function cc_same(fs,file,rev) {
  if(!file)return rev==='';
  try{return cc_stamp(fs.statSync(file,{bigint:true}))===rev;}catch{return false;}
}
function components_cache_put(cwd,pcb,sch,pcbRev,schRev,offset,text) {
  const {fs,path,crypto}=cc_modules();let temp=null;
  try {
    if(!pcbRev||!/^\d+$/.test(String(offset))||Buffer.byteLength(text)>262144)return '';
    const dir=cc_dir(true),name=cc_name(crypto,fs.realpathSync(cwd),pcbRev,schRev,offset);
    temp=path.join(dir,`.page-${process.pid}-${crypto.randomBytes(12).toString('hex')}.tmp`);
    fs.writeFileSync(temp,JSON.stringify({pcb,sch,pcbRev,schRev,offset:String(offset),text}),{mode:0o600,flag:'wx'});fs.renameSync(temp,path.join(dir,name));temp=null;
    const names=fs.readdirSync(dir).filter(n=>/^[0-9a-f]{64}\.json$/.test(n)).map(n=>({n,t:fs.statSync(path.join(dir,n)).mtimeMs})).sort((a,b)=>b.t-a.t);
    for(const f of names.slice(512))try{fs.unlinkSync(path.join(dir,f.n));}catch{}
  }catch{}finally{if(temp)try{fs.unlinkSync(temp);}catch{}}
  return '';
}
function components_cache_get(cwd,offset,pcbRev,schRev) {
  const {fs,path,crypto}=cc_modules();
  try {
    if(!pcbRev||!/^\d+$/.test(String(offset)))return '';
    const p=path.join(cc_dir(false),cc_name(crypto,fs.realpathSync(cwd),pcbRev,schRev,offset));
    if(!fs.existsSync(p))return '';
    const st=fs.lstatSync(p);
    if(!st.isFile()||st.isSymbolicLink()||st.nlink!==1||(typeof process.getuid==='function'&&st.uid!==process.getuid())||(st.mode&0o077)!==0||st.size>600000)return '';
    const e=JSON.parse(fs.readFileSync(p,'utf8'));
    if(e?.pcbRev!==pcbRev||e?.schRev!==schRev||e?.offset!==String(offset)||typeof e.text!=='string')return '';
    if(!cc_same(fs,e.pcb,pcbRev)||!cc_same(fs,e.sch,schRev))return '';
    return e.text;
  }catch{return '';}
}
// The first page of a read names no revisions: the hub gives the canonical
// files it resolved (the same K.Files.read the child would use), and their
// current stamps pick the page, which must have been kept for those paths.
function components_cache_first(cwd,pcb,sch) {
  const {fs}=cc_modules();
  try {
    if(!pcb)return '';
    const pcbRev=cc_stamp(fs.statSync(pcb,{bigint:true}));
    let schRev='';if(sch){try{schRev=cc_stamp(fs.statSync(sch,{bigint:true}));}catch{return '';}}
    const text=components_cache_get(cwd,'0',pcbRev,schRev);if(!text)return '';
    const {path,crypto}=cc_modules(),e=JSON.parse(fs.readFileSync(path.join(cc_dir(false),cc_name(crypto,fs.realpathSync(cwd),pcbRev,schRev,'0')),'utf8'));
    return e.pcb===pcb&&e.sch===sch?text:'';
  }catch{return '';}
}
io_eff(CID(ComponentsFx.put),components_cache_put);
io_eff(CID(ComponentsFx.get),components_cache_get);
io_eff(CID(ComponentsFx.first),components_cache_first);
