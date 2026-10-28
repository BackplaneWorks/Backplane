// Bounded CLI/OAuth stand-in for account qualification. No network or inference.
const fs=require('node:fs'),path=require('node:path'),rl=require('node:readline');
const provider=process.argv[2],args=process.argv.slice(3);
const store=provider==='claude'?process.env.CLAUDE_CONFIG_DIR:process.env.CODEX_HOME;
const emit=o=>process.stdout.write(JSON.stringify(o)+'\n');
if(args.includes('--version')){console.log(provider==='claude'?'2.1.300 (Claude Code fixture)':'codex-cli 0.165.0-fixture');process.exit(0);}
const marker=store&&path.join(store,'fixture-auth.json');
const fixture=()=>{for(const dir of [store,store&&path.dirname(store)])try{return JSON.parse(fs.readFileSync(path.join(dir,'fixture.json'),'utf8'));}catch{}return {};};
const identity=()=>{try{return JSON.parse(fs.readFileSync(marker,'utf8'));}catch{return null;}};
const save=()=>{if(!store||!store.includes('accounts-v1'))throw Error('fixture login outside owned store');fs.writeFileSync(marker,JSON.stringify({email:fixture().email||'fixture@example.test',workspace:fixture().workspace||'workspace-fixture'}),{mode:0o600});};
const audit=(kind,details={})=>{if(process.env.BP_ACCOUNTS_AUDIT)fs.appendFileSync(process.env.BP_ACCOUNTS_AUDIT,JSON.stringify({kind,provider,store:store||'legacy',home:process.env.HOME,ambientApi:!!process.env.OPENAI_API_KEY||!!process.env.ANTHROPIC_API_KEY,ambientProfile:!!process.env.ANTHROPIC_PROFILE,...details})+'\n');};
let nativeSession='';
const session=()=>nativeSession;
const takeSession=resumed=>{
 const registry=store&&path.join(store,'fixture-sessions.json');let sessions=[];
 try{sessions=JSON.parse(fs.readFileSync(registry,'utf8'));}catch{}
 if(resumed){if(!sessions.includes(resumed))process.exit(94);nativeSession=resumed;}
 else {nativeSession='fixture-session-'+require('node:crypto').randomUUID();if(registry)fs.writeFileSync(registry,JSON.stringify([...sessions,nativeSession].slice(-100)),{mode:0o600});}
};
const own=()=>{if(store&&!identity())process.exit(92);};
if(provider==='claude'&&args[0]==='auth'){
 const op=args[1];audit(op);
 if(op==='status'){const i=identity();emit({loggedIn:!!i,authMethod:i?'claude.ai':'none',email:i?.email,orgUuid:i?.workspace,configDirectory:store,subscriptionType:'fixture'});process.exit(i?0:1);}
 if(op==='logout'){try{fs.unlinkSync(marker);}catch{}console.log('Logged out');process.exit(0);}
 if(op==='login'){console.log('https://claude.ai/oauth/authorize?state=fixture-'+path.basename(store));const input=rl.createInterface({input:process.stdin});input.on('line',line=>{if(line==='fixture-code'){save();process.exit(0);}else process.exit(93);});}
}else if(provider==='codex'&&args.includes('app-server')){
 const input=rl.createInterface({input:process.stdin});let login;
 input.on('line',line=>{const q=JSON.parse(line);if(!Object.hasOwn(q,'id'))return;let result={};audit(q.method);
  if(q.method==='initialize')result={userAgent:'codex-fixture'};
  else if(q.method==='account/read'){const i=identity();result={account:i?{type:'chatgpt',email:i.email,planType:'fixture'}:null,workspaceRouting:i?{chatgptAccountId:i.workspace}:null};}
  else if(q.method==='account/rateLimits/read')result={rateLimitsByLimitId:{codex:{limitId:'codex',primary:{usedPercent:fixture().used||23,windowDurationMins:300,resetsAt:Math.floor(Date.now()/1000)+300}}}};
  else if(q.method==='model/list')result={data:[{model:'fixture-model',displayName:'Fixture model',supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'high'}]}]};
  else if(q.method==='account/login/start'){login='fixture-'+path.basename(store);result={loginId:login,verificationUrl:'https://auth.openai.com/codex/device',userCode:'FIXTURE-ONLY'};setTimeout(()=>{save();emit({method:'account/login/completed',params:{loginId:login,success:true}});},fixture().delay||150);}
  else if(q.method==='account/logout'){try{fs.unlinkSync(marker);}catch{}}
  else if(q.method==='account/login/cancel')setTimeout(()=>process.exit(0),5);
  else {emit({id:q.id,error:{code:-32601,message:'Unsupported fixture method'}});return;}
  emit({id:q.id,result});
 });
}else if(provider==='codex'&&args.includes('exec')){
 own();const at=args.indexOf('resume'),resumed=at>=0?args[at+1]:'';
 takeSession(resumed);audit(args.includes('--ephemeral')?'writing':'turn',{resumed,session:session(),args:args.filter((x,i)=>x==='-m'||args[i-1]==='-m')});
 const prompt=args.at(-1)||'';
 const answer=()=>{emit({type:'thread.started',thread_id:session()});emit({type:'item.completed',item:{type:'agent_message',text:'Fixture answer '+(identity()?.email||'legacy')}});emit({type:'turn.completed',usage:{input_tokens:1,output_tokens:1}});process.exit(0);};
 setTimeout(answer,prompt.includes('hold-fixture')?3000:30);
}else if(provider==='claude'&&args.includes('--input-format')){
 const at=args.indexOf('--resume'),resumed=at>=0?args[at+1]:'';
 const input=rl.createInterface({input:process.stdin});
 input.on('line',line=>{const q=JSON.parse(line);
  if(q.type==='control_request'){emit({type:'control_response',response:{subtype:'success',request_id:q.request_id,response:{models:[{value:'fixture-claude',displayName:'Fixture Claude',supportedEffortLevels:['low','high']}]}}});return;}
  if(q.type!=='user')return;own();if(!nativeSession)takeSession(resumed);audit('turn',{resumed,session:session()});
  emit({type:'system',subtype:'init',session_id:session(),model:'fixture-claude',tools:[],mcp_servers:[],claude_code_version:'2.1.300',capabilities:[]});
  emit({type:'rate_limit_event',rate_limit_info:{unifiedWindows:{five_hour:{utilization:0.23,resetsAt:Math.floor(Date.now()/1000)+300}}}});
  emit({type:'assistant',session_id:session(),message:{id:'fixture-'+Date.now(),role:'assistant',content:[{type:'text',text:'Fixture Claude answer '+(identity()?.email||'legacy')}]}});
  emit({type:'result',subtype:'success',is_error:false,session_id:session(),result:'Fixture Claude answer',num_turns:1,usage:{input_tokens:1,output_tokens:1},modelUsage:{}});
 });
}else if(provider==='claude'&&args.includes('-p')){
 own();audit('writing');process.stdin.resume();process.stdin.on('end',()=>{emit({type:'result',result:'Fixture title',is_error:false});process.exit(0);});
}else process.exit(0);
