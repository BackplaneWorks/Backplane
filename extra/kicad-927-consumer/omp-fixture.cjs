'use strict';
// PREPARED ONLY: not executed or qualified. OMP v18.16.0, disposable profiles.
// No process launch, source-effect lifting, hub mutation or listener on import.
// Consumer owns the real packaged UI/hub, account/JobObject checks and verdict.
//
// const f = require('./fixture.cjs').prepare(profile, project, out);
// await f.startFixture();                   // writes models.yml with bound port
// launchPackagedApp({env:{...cleanEnv,...f.env}}); // before hub/model detection
// select OMP and f.model in the real UI; send an ordinary test message.
// Assert observer records + loopback request + reply f.reply + hub SessionSet.
// await f.closeFixture();                   // consumer owns profile cleanup
//
// Native auto-discovery: <active-agent-dir>/extensions/*.js, even with -e.
// https://github.com/can1357/oh-my-pi/blob/v18.16.0/docs/extension-loading.md
// Model config: <active-agent-dir>/models.yml, auth:none, loopback only.
// https://github.com/can1357/oh-my-pi/blob/v18.16.0/docs/models.md
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const exclusive = (file, text) => fs.writeFileSync(file, text, {flag:'wx',mode:0o600});
const append = (file, record) => fs.appendFileSync(file, JSON.stringify(record)+'\n', {mode:0o600});

// This function is stringified into an auto-discovered ES module. It only
// observes; no registerTool, instructions override, -e replacement or stdout IO.
function observer(pi, fs, path, crypto, manifestPath) {
  const m = JSON.parse(fs.readFileSync(manifestPath,'utf8'));
  const sha = s => crypto.createHash('sha256').update(s).digest('hex');
  const flag = name => {const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
  const observe = (phase, ctx) => {
    const parts=ctx.getSystemPrompt();
    const system=Array.isArray(parts)?parts.join('\n'):String(parts);
    const promptPath=flag('--append-system-prompt');
    const prompt=promptPath?fs.readFileSync(promptPath,'utf8'):'';
    const builtIns=[];
    for(let i=0;i<process.argv.length-1;i++)if(['-e','--extension'].includes(process.argv[i])) {
      const file=process.argv[i+1];
      builtIns.push({path:file,isBackplane:path.basename(file)==='backplane.js',exists:fs.existsSync(file),
        sha256:fs.existsSync(file)?sha(fs.readFileSync(file)):null});
    }
    const record={phase,at:new Date().toISOString(),nonce:m.nonce,pid:process.pid,ppid:process.ppid,
      execPath:process.execPath,cwd:ctx.cwd,mode:ctx.mode,model:ctx.model?.id??null,
      provider:ctx.model?.provider??null,sessionId:ctx.sessionManager.getSessionId(),
      promptPath,promptUTF16Length:prompt.length,promptSHA256:sha(prompt),systemUTF16Length:system.length,
      systemSHA256:sha(system),startMarkerInPrompt:prompt.includes(m.startMarker),
      endMarkerInPrompt:prompt.includes(m.endMarker),allSkillDescriptionsInPrompt:m.descriptions.every(s=>prompt.includes(s)),
      allSkillDescriptionsInSystem:m.descriptions.every(s=>system.includes(s)),builtInExtensions:builtIns,
      backplaneMCPEndpointPresent:Boolean(process.env.BACKPLANE_OMP_MCP),
      observerPath:m.extension};
    record.attestationPass=record.mode==='rpc'&&record.provider==='backplane-smoke'&&record.model==='local'&&
      record.promptUTF16Length>32767&&record.startMarkerInPrompt&&record.endMarkerInPrompt&&
      record.allSkillDescriptionsInPrompt&&record.allSkillDescriptionsInSystem&&
      builtIns.some(x=>x.isBackplane&&x.exists)&&record.backplaneMCPEndpointPresent;
    fs.appendFileSync(path.join(m.observerDir,'observer-'+process.pid+'.jsonl'),JSON.stringify(record)+'\n',{mode:0o600});
  };
  pi.on('session_start',(_event,ctx)=>observe('session_start',ctx));
  pi.on('before_agent_start',(_event,ctx)=>observe('before_agent_start',ctx));
}

function prepare(profile, project, out) {
  for(const [name,dir] of Object.entries({profile,project,out})) {
    if(!path.isAbsolute(dir))throw Error(name+' must be an absolute disposable test path');
    if(!fs.statSync(dir).isDirectory()||fs.lstatSync(dir).isSymbolicLink())throw Error(name+' must be a real directory');
  }
  const nonce=crypto.randomBytes(8).toString('hex');
  // Refuse existing evidence/config instead of changing another test's inputs.
  const evidenceDir=path.join(out,'omp-packaged-fixture');fs.mkdirSync(evidenceDir,{mode:0o700});
  const observerDir=path.join(evidenceDir,'observer');fs.mkdirSync(observerDir,{mode:0o700});
  const agentDir=path.join(profile,'.omp','agent');
  const extensionDir=path.join(agentDir,'extensions');fs.mkdirSync(extensionDir,{recursive:true,mode:0o700});
  const models=path.join(agentDir,'models.yml');
  if(fs.existsSync(models)||fs.existsSync(path.join(agentDir,'models.yaml'))||fs.existsSync(path.join(agentDir,'models.json')))
    throw Error('Use a fresh test agent directory; existing model configuration is never overwritten');
  const extension=path.join(extensionDir,'bp949-packaged-observer-'+nonce+'.js');
  const manifest=path.join(evidenceDir,'manifest.json');
  const requests=path.join(evidenceDir,'requests.jsonl');
  const summary=path.join(evidenceDir,'fixture-summary.json');
  const startMarker='BP949_SKILLS_START_'+nonce,endMarker='BP949_SKILLS_END_'+nonce;
  const descriptions=[],skills=[];
  // Keep individual descriptions below 1KB; their sum alone exceeds the
  // Windows argv limit. User-scoped skills survive a thread worktree's cwd.
  for(let i=0;i<64;i++) {
    const name='bp949-fixture-'+nonce+'-'+String(i).padStart(2,'0');
    const description=(i===0?startMarker+' ':'')+'Synthetic packaged OMP acceptance padding '+
      String(i).padStart(2,'0')+' '+('padding 日本 UTF16 ').repeat(34)+(i===63?' '+endMarker:'');
    const dir=path.join(profile,'.claude','skills',name);fs.mkdirSync(dir,{recursive:true,mode:0o700});
    const skill=path.join(dir,'SKILL.md');
    exclusive(skill,'---\nname: '+name+'\ndescription: '+description+'\nuser-invocable: false\n---\nSynthetic acceptance fixture only.\n');
    descriptions.push(description);skills.push(skill);
  }
  if(descriptions.join('\n').length<=32767)throw Error('Fixture padding must exceed the Windows command-line bound');
  const metadata={schema:1,scope:'PREPARED ONLY; actual packaged consumer determines acceptance',ompVersion:'18.16.0',
    nonce,profile,project,agentDir,extension,observerDir,models,descriptions,skills,startMarker,endMarker,
    syntheticDescriptionUTF16Length:descriptions.join('\n').length,
    readinessEvidence:'session_start follows ready enqueue in upstream rpc-mode; matching hub SessionSet derives from successful get_state. No raw-ready capture is claimed.'};
  exclusive(manifest,JSON.stringify(metadata,null,2)+'\n');
  exclusive(extension,"import fs from 'node:fs';\nimport path from 'node:path';\nimport crypto from 'node:crypto';\n"+
    'const install = '+observer.toString()+';\nexport default function(pi) { install(pi,fs,path,crypto,'+JSON.stringify(manifest)+'); }\n');
  const env={HOME:profile,USERPROFILE:profile,PI_CODING_AGENT_DIR:agentDir};
  const model='backplane-smoke/local',reply='OMP_PACKAGE_OK_'+nonce;
  const record={schema:1,nonce,model,reply,status:'PREPARED_NOT_STARTED',requests:0,completions:0,errors:[]};
  let server=null,started=false;
  const writeSummary=()=>fs.writeFileSync(summary,JSON.stringify(record,null,2)+'\n',{mode:0o600});
  const textOf=content=>typeof content==='string'?content:Array.isArray(content)?content.filter(x=>x?.type==='text').map(x=>x.text??'').join('\n'):'';
  async function startFixture() {
    if(started)throw Error('Fixture may be started only once');started=true;
    server=http.createServer(async(req,res)=>{
      const row={at:new Date().toISOString(),method:req.method,path:req.url,peer:req.socket.remoteAddress,
        authHeaderPresent:Boolean(req.headers.authorization)};
      record.requests++;
      try {
        if(!['127.0.0.1','::ffff:127.0.0.1'].includes(req.socket.remoteAddress))throw Error('Only loopback requests are permitted');
        if(req.method==='GET'&&req.url==='/v1/models') {
          row.kind='model-list';res.writeHead(200,{'content-type':'application/json'});
          res.end(JSON.stringify({object:'list',data:[{id:'local',object:'model',owned_by:'backplane-smoke'}]}));return;
        }
        if(req.method!=='POST'||req.url!=='/v1/chat/completions')throw Error('Unexpected fixture endpoint');
        const chunks=[];let bytes=0;
        for await(const chunk of req){bytes+=chunk.length;if(bytes>4*1024*1024)throw Error('Fixture request exceeds 4 MiB');chunks.push(chunk);}
        const raw=Buffer.concat(chunks).toString('utf8'),body=JSON.parse(raw);
        const policy=(body.messages??[]).filter(x=>['system','developer'].includes(x.role)).map(x=>textOf(x.content)).join('\n');
        Object.assign(row,{kind:'completion',bodyBytes:bytes,bodySHA256:hash(raw),model:body.model,stream:body.stream,
          policyUTF16Length:policy.length,startMarker:policy.includes(startMarker),endMarker:policy.includes(endMarker),
          allSkillDescriptions:descriptions.every(x=>policy.includes(x))});
        if(body.model!=='local'||body.stream!==true||!row.startMarker||!row.endMarker||!row.allSkillDescriptions||policy.length<=32767)
          throw Error('Actual model request did not contain the complete large skill prompt or expected streaming model');
        record.completions++;
        const base={id:'chatcmpl-bp949-'+nonce,object:'chat.completion.chunk',created:Math.floor(Date.now()/1000),model:'local'};
        const frame=(delta,finish_reason)=>'data: '+JSON.stringify({...base,choices:[{index:0,delta,finish_reason}]})+'\n\n';
        res.writeHead(200,{'content-type':'text/event-stream','cache-control':'no-cache','connection':'close'});
        res.end(frame({role:'assistant',content:reply},null)+frame({},'stop')+'data: [DONE]\n\n');
        row.response=reply;
      } catch(error) {
        row.error=error.message;record.errors.push(error.message);
        if(!res.headersSent)res.writeHead(400,{'content-type':'application/json'});
        res.end(JSON.stringify({error:{message:error.message}}));
      } finally {append(requests,row);writeSummary();}
    });
    server.requestTimeout=30000;server.headersTimeout=15000;
    await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
    const baseUrl='http://127.0.0.1:'+server.address().port+'/v1';
    try {
      exclusive(models,'providers:\n  backplane-smoke:\n    baseUrl: '+baseUrl+'\n    api: openai-completions\n    auth: none\n    models:\n      - id: local\n        name: Packaged OMP acceptance fixture\n        reasoning: false\n        input: [text]\n        contextWindow: 262144\n        maxTokens: 64\n');
      Object.assign(record,{status:'LISTENING_NOT_QUALIFIED',baseUrl,modelsSHA256:hash(fs.readFileSync(models))});writeSummary();
      return {baseUrl,model,reply,env,evidencePaths};
    } catch(error) {await closeFixture();throw error;}
  }
  async function closeFixture() {
    if(server) {
      const own=server;server=null;
      await new Promise((resolve,reject)=>{own.close(error=>error?reject(error):resolve());own.closeAllConnections?.();});
    }
    record.status='CLOSED_CONSUMER_VERDICT_REQUIRED';writeSummary();
  }
  const evidencePaths={manifest,extension,models,observerDir,observerGlob:path.join(observerDir,'observer-*.jsonl'),requests,summary};
  return {env,model,reply,startFixture,closeFixture,evidencePaths,markers:{start:startMarker,end:endMarker},skills};
}
module.exports={prepare};
