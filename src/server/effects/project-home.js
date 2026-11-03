// Project contents are read asynchronously; navigation never starts CAD/checks.
// Output is a bounded source window, not a project-size admission decision.
// Count the producer's saved report in constant memory, including minified
// and very large files. Description/item bodies never enter the home snapshot.
async function bp_project_report(file) {
  const result={source:'',date:'',errors:0,warnings:0,unconnected:0};
  const stack=[];let quoted=false,escaped=false,raw='',large=false;
  function token(){
    const current=stack.at(-1);if(!current||current.type!=='object')return;
    const value=large?'':JSON.parse('"'+raw+'"');
    if(current.keyNext){current.key=value;return;}
    if(stack.length===1&&['source','date'].includes(current.key))result[current.key]=value;
    if(current.record==='violations'&&current.key==='severity'){
      if(value==='error')result.errors++;if(value==='warning')result.warnings++;
    }
  }
  for await(const chunk of require('node:fs').createReadStream(file,{encoding:'utf8',highWaterMark:65536})){
    for(const char of chunk){
      if(quoted){
        if(escaped){if(raw.length<4096)raw+=char;else large=true;escaped=false;continue;}
        if(char==='"'){token();quoted=false;continue;}
        if(raw.length<4096)raw+=char;else large=true;
        if(char==='\\')escaped=true;
        continue;
      }
      const current=stack.at(-1);
      if(char==='"'){quoted=true;raw='';large=false;continue;}
      if(char==='{'){
        const record=current?.type==='array'?current.name:'';
        if(record==='unconnected_items')result.unconnected++;
        stack.push({type:'object',keyNext:true,key:'',record});continue;
      }
      if(char==='['){stack.push({type:'array',name:current?.type==='object'?current.key:''});continue;}
      if(char==='}'||char===']'){const prior=stack.pop();if(!prior||prior.type!==(char==='}'?'object':'array'))throw Error('Malformed saved check report.');continue;}
      if(char===','&&current?.type==='object')current.keyNext=true;
      if(char===':'&&current?.type==='object')current.keyNext=false;
    }
  }
  if(quoted||stack.length||!result.source)throw Error('Incomplete saved check report.');
  return result;
}

// Project/fab files may contain arbitrarily large unrelated metadata. Read the
// small routing/measurement projection as a stream rather than a file-size gate.
async function bp_project_metadata(file, keys, maps=[]) {
  const out=Object.create(null), allowed=new Set(keys), nested=new Set(maps), stack=[];
  let quoted=false, escaped=false, token='', overflow=false, primitive='', root=false;
  function put(value){const c=stack.at(-1);if(c?.capture===out&&!allowed.has(c.key))return;if(c?.type==='object'&&!c.keyNext&&c.capture&&c.key&&Object.keys(c.capture).length<128)c.capture[c.key]=value;}
  function flush(){if(!primitive)return;const text=primitive;primitive='';if(text.length<64)put(JSON.parse(text));}
  function string(){const c=stack.at(-1);if(!c||c.type!=='object')return;
    const value=overflow?'':JSON.parse('"'+token+'"');
    if(c.keyNext)c.key=value;
    else if(!overflow&&((stack.length===1&&allowed.has(c.key))||stack.length===2))put(value);
  }
  for await(const chunk of require('node:fs').createReadStream(file,{encoding:'utf8',highWaterMark:65536}))for(const char of chunk){
    if(quoted){if(escaped){if(token.length<8192)token+=char;else overflow=true;escaped=false;continue;}if(char==='"'){string();quoted=false;continue;}if(token.length<8192)token+=char;else overflow=true;if(char==='\\')escaped=true;continue;}
    if(char==='"'){flush();quoted=true;token='';overflow=false;continue;}
    if(/[\s,{}\[\]:]/.test(char))flush();else {if(primitive.length<64)primitive+=char;continue;}
    const current=stack.at(-1);
    if(char==='{'){
      let capture;
      if(!stack.length){if(root)throw Error('Repeated metadata root');root=true;capture=out;}
      else if(stack.length===1&&current?.type==='object'&&nested.has(current.key)){capture=Object.create(null);out[current.key]=capture;}
      stack.push({type:'object',keyNext:true,key:'',capture});continue;
    }
    if(char==='['){stack.push({type:'array'});continue;}
    if(char==='}'||char===']'){const previous=stack.pop();if(!previous||previous.type!==(char==='}'?'object':'array'))throw Error('Malformed metadata');continue;}
    if(char===','&&current?.type==='object')current.keyNext=true;
    if(char===':'&&current?.type==='object')current.keyNext=false;
  }
  flush();if(quoted||stack.length||!root)throw Error('Incomplete metadata');
  // Only approved top-level scalar fields can leave this reader.
  for(const key of Object.keys(out))if(!allowed.has(key)&&!nested.has(key))delete out[key];
  return out;
}

async function bp_project_home(root, wire) {
  const fs=require('node:fs/promises'), path=require('node:path'), crypto=require('node:crypto');
  const request=JSON.parse(wire||'{}'), artifacts=[], issues=[];
  let real;
  try {real=await fs.realpath(root);} catch {return JSON.stringify({project_home:request.project,root,artifacts,error:'Project folder is unavailable.'});}
  const within=p=>p===real||p.startsWith(real+path.sep);
  async function file(rel){try{const p=await fs.realpath(path.resolve(real,rel));if(!within(p))return null;const s=await fs.stat(p);return s.isFile()?{path:p,mtime:s.mtimeMs,size:s.size}:null;}catch{return null}}
  async function text(rel,limit=4096,offset=0){const f=await file(rel);if(!f)return null;const h=await fs.open(f.path,'r');try{const b=Buffer.alloc(Math.max(0,Math.min(limit,f.size-offset)));const r=await h.read(b,0,b.length,offset);let n=r.bytesRead,value='';for(;;){try{value=new TextDecoder('utf8',{fatal:true}).decode(b.subarray(0,n));break}catch(e){if(offset+r.bytesRead===f.size||r.bytesRead-n>=3)throw Error('This file is not valid UTF-8; open the original file to inspect it.');n--;}}return {...f,text:value,bytes:n,next:offset+n<f.size?offset+n:null};}finally{await h.close()}}
  async function add(kind,rel,title,summary){const f=await file(rel);if(!f)return null;const row={kind,...f,title,summary:summary||''};if(kind==='mech'){const thumb=await file(path.join(path.dirname(f.path),'renders',path.basename(f.path,path.extname(f.path))+'.iso.png'));if(thumb)row.thumbnail=thumb.path;}artifacts.push(row);return row;}
  const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
  async function digest(rel){const f=await file(rel);if(!f)return hash('');const h=crypto.createHash('sha256');for await(const b of require('node:fs').createReadStream(f.path,{highWaterMark:65536}))h.update(b);return h.digest('hex');}
  if(request.op==='intent'){
    const dest=path.join(real,'.backplane','intent.md'), parent=path.dirname(dest);
    await fs.mkdir(parent,{recursive:true});
    if(!within(await fs.realpath(parent)))return JSON.stringify({error:'Intent directory leaves this project.',project_home:request.project});
    if(await digest(dest)!==request.revision)return JSON.stringify({error:'Goal and notes changed. Reload before saving; your edit is retained.',project_home:request.project});
    const prior=await file(dest),offset=Math.max(0,Number(request.editor)||0),remove=Math.max(0,Number(request.removeBytes)||0);
    if(offset>(prior?.size||0)||offset+remove>(prior?.size||0))return JSON.stringify({error:'This edit no longer matches the saved section.',project_home:request.project});
    const temp=path.join(parent,'.intent-'+crypto.randomUUID()+'.tmp');
    const out=await fs.open(temp,'wx',0o600);let input;
    try{
      if(prior)input=await fs.open(prior.path,'r');
      async function copy(start,end){const b=Buffer.alloc(65536);let at=start;while(at<end){const r=await input.read(b,0,Math.min(b.length,end-at),at);if(!r.bytesRead)throw Error('The intent changed while saving.');await out.writeFile(b.subarray(0,r.bytesRead));at+=r.bytesRead;}}
      if(prior)await copy(0,offset);
      await out.writeFile(String(request.text));
      if(prior)await copy(offset+remove,prior.size);
      await out.sync();await out.close();
      if(await digest(dest)!==request.revision)throw Error('Goal and notes changed while saving. Your edit is retained.');
      await fs.rename(temp,dest);
    }finally{await input?.close().catch(()=>{});await out.close().catch(()=>{});await fs.unlink(temp).catch(()=>{});}
  }
  if(request.op==='document'){
    const f=await text(request.path,8192,Math.max(0,Number(request.offset)||0));
    return JSON.stringify({project_home:request.project,document:f||{error:'File is unavailable.'}});
  }
  let config={},invalidConfig=false;const cfg=await file('.backplane.json');try{if(cfg)config=await bp_project_metadata(cfg.path,['project','pcb','schematic','drc','erc']);}catch{invalidConfig=true;issues.push('Project configuration could not be read.')}
  let stem=config.project?String(config.project).replace(/\.kicad_pro$/,''):null;
  // Discovery is incremental and skips generated/reference/dependency trees.
  const skip=new Set(['.git','.backplane','node_modules','build','dist','mfg','extra','target','.easyeda_cache']);
  let scanned=0, scanNext=null;
  if(!invalidConfig&&!stem&&!config.pcb&&!config.schematic){
    const start=Math.max(0,Number(request.scan)||0);let ordinal=0;
    async function scan(dir,depth){if(depth>4||scanned>=256)return;let it;try{it=await fs.opendir(dir)}catch{return}
      try{for await(const e of it){if(e.isSymbolicLink()||skip.has(e.name))continue;const p=path.join(dir,e.name);if(e.isDirectory()){await scan(p,depth+1);}else{ordinal++;if(ordinal<=start)continue;scanned++;if(e.name.endsWith('.kicad_pro')&&!stem)stem=p.slice(0,-10);if(scanned>=256){scanNext=ordinal;break;}}if(stem||scanned>=256)break;}}catch(e){issues.push('Some project files could not be scanned.')}}
    await scan(real,0);
  }
  const pcb=await add('board',config.pcb||(stem?stem+'.kicad_pcb':''),'Board');
  const sch=await add('schematic',config.schematic||(stem?stem+'.kicad_sch':''),'Schematic');
  // Empty candidates must not resolve the root directory as an artifact.
  const editor=Math.max(0,Number(request.editor)||0);
  const savedIntent=await text('.backplane/intent.md',8192,editor);
  let intentRevision=hash('');
  if(savedIntent){await add('intent','.backplane/intent.md','Goal & notes',savedIntent.text);intentRevision=await digest('.backplane/intent.md');}
  const spec=await text('docs/SPEC.md',4096);if(spec)await add('spec','docs/SPEC.md','Specification',spec.text);
  const readme=await text('README.md',4096);if(readme)await add('readme','README.md','README',readme.text);
  // Linked fab files are evidence only when their exact source still exists.
  async function fab(candidate){const raw=await file(candidate);if(!raw)return;try{const v=await bp_project_metadata(raw.path,['name','source','kind','qty','order','engineering_status'],['options','features']),source=typeof v.source==='string'?await file(path.resolve(path.dirname(raw.path),v.source)):null;if(source)await add('fab',candidate,'Fab readiness',JSON.stringify({name:v.name,source:v.source,kind:v.kind,order:v.order!==false,qty:v.qty,options:v.options,features:v.features,engineering_status:v.engineering_status}));}catch{issues.push('Fab metadata is invalid.')}}
  if(pcb)await fab(pcb.path.replace(/\.kicad_pcb$/,'.fab.json'));
  let next=null, ordinal=0;const after=Math.max(0,Number(request.after)||0);let seen=0;
  try{const it=await fs.opendir(path.join(real,'mech'));for await(const e of it){ordinal++;if(ordinal<=after)continue;seen++;if(e.isFile()&&/\.(step|stp|glb)$/i.test(e.name)){const part=await add('mech',path.join('mech',e.name),e.name);if(part)await fab(part.path.replace(/\.(step|stp|glb)$/i,'.fab.json'));}if(artifacts.length>=26||seen>=256){next=ordinal;break;}}}catch{}
  // Keep only four newest candidates per kind; parse counts after discovery.
  // Directory traversal is asynchronous and never retains the full listing.
  if(pcb||sch){
    const candidates={drc:[],erc:[]};
    async function reportCandidate(rel,kind){const f=await file(rel);if(!f)return;candidates[kind].push(f);candidates[kind].sort((a,b)=>b.mtime-a.mtime);candidates[kind].length=Math.min(candidates[kind].length,4);}
    try{const it=await fs.opendir(path.join(real,'build','reports'));for await(const e of it){if(!e.isFile()||!/(drc|erc).*\.json$/i.test(e.name))continue;await reportCandidate(path.join('build','reports',e.name),/erc/i.test(e.name)?'erc':'drc');}}catch{}
    for(const kind of ['drc','erc']){
      if(typeof config[kind]==='string')await reportCandidate(config[kind],kind);
      for(const raw of candidates[kind])try{
        const report=await bp_project_report(raw.path);
        if(![pcb?.path,sch?.path].filter(Boolean).some(p=>path.basename(p)===path.basename(report.source)))continue;
        await add(kind,raw.path,kind.toUpperCase(),JSON.stringify({...report,stale:raw.mtime<Math.max(pcb?.mtime||0,sch?.mtime||0)}));break;
      }catch{}
    }
  }
  for(const name of ['mfg/bom.csv','mfg/BOM.csv','bom.csv','BOM.csv']){const raw=await text(name,4096);if(raw){await add('bom',name,'BOM',raw.text);break;}}
  // Git metadata is independent of CAD eligibility. All argv are literal;
  // snapshots use the person's normal CLI credentials, never shell commands.
  const git=await fs.stat(path.join(real,'.git')).then(()=>true,()=>false);
  if(git){
    const run=(exe,args)=>new Promise(resolve=>require('node:child_process').execFile(exe,args,{cwd:real,timeout:1500,maxBuffer:32768,env:{...process.env,GIT_TERMINAL_PROMPT:'0',GIT_OPTIONAL_LOCKS:'0'}},(err,out)=>resolve(err?null:String(out))));
    const changes=await run('git',['-c','core.fsmonitor=false','log','-6','--format=%h %s']);
    if(changes)artifacts.push({kind:'changes',path:path.join(real,'.git'),title:'Recent changes',summary:changes.trim(),mtime:Date.now(),size:0});
    const remote=await run('git',['-c','core.fsmonitor=false','config','--get','remote.origin.url']);
    const match=remote?.trim().match(/^(?:https:\/\/github\.com\/|git@github\.com:)([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+?)(?:\.git)?$/);
    if(match){
      const raw=await run('gh',['pr','list','--repo',match[1],'--state','open','--limit','12','--json','number,title,url']);
      let summary='Pull requests could not be refreshed. Open the repository to inspect them.';
      if(raw)try{const rows=JSON.parse(raw);summary=rows.length?rows.map(r=>'['+String(r.title).replace(/[\[\]\n]/g,' ')+' (#'+r.number+')]('+r.url+')').join('\n\n'):'No open pull requests in the saved snapshot.'}catch{}
      artifacts.push({kind:'prs',path:path.join(real,'.git'),title:'Open pull requests',summary:summary+'\n\n[Open repository](https://github.com/'+match[1]+')',mtime:Date.now(),size:0});
    }
  }
  const revision=hash(JSON.stringify(artifacts.map(a=>[a.path,a.mtime,a.size])));
  return JSON.stringify({project_home:request.project,root:real,artifacts:artifacts.slice(0,32),intentRevision,intentComplete:!savedIntent||savedIntent.next===null,revision,next,after,scanNext,issues,intentSaved:request.op==='intent',intentOffset:editor,intentNext:savedIntent?.next??null,intentBytes:savedIntent?.bytes||0,intent:savedIntent?.text||'# Goal\n\n# Parts\n\n# Notes\n'});
}
function bp_project_home_effect(root,wire,k){return io_park_promise(bp_project_home(root,wire),k,(v,e)=>e?JSON.stringify({project_home:JSON.parse(wire).project,error:String(e.message||e)}):v);}
io_eff(CID(ProjectHome.read),bp_project_home_effect);
