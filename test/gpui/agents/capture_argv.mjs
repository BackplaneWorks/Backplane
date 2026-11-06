// Run the JS hub with test/tools/claude_standin.ts as `claude`, send one message, and keep the argv the hub started it with.
// usage: bun test/gpui/agents/capture_argv.mjs OUTDIR [extra settings as key=value ...]
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep} from '../perf/lib.mjs';
import {makeFixture} from '../parity/fixture.mjs';
import {execFileSync} from 'node:child_process';
const out=process.argv[2],settings=process.argv.slice(3);
const work=out+'/work';rmSync(work,{recursive:true,force:true});mkdirSync(work+'/fixroot',{recursive:true});mkdirSync(work+'/bin',{recursive:true});
const home=work+'/home';makeFixture(home,work+'/fixroot');
writeFileSync(work+'/bin/claude',`#!/bin/sh\nexec bun ${process.cwd()}/test/tools/claude_standin.ts "$@"\n`);chmodSync(work+'/bin/claude',0o755);
for(const n of['codex','grok','muse'])writeFileSync(work+'/bin/'+n,'#!/bin/sh\nexit 1\n'),chmodSync(work+'/bin/'+n,0o755);
const h=await startHub({bin:process.env.JS_HUB||'build/gpui/backplane-serve',home,port:Number(process.env.PORT||3843),standins:work+'/bin',extraEnv:{FAKE_LOG:work+'/fake.log',BP_STANDIN_MODE:'plain'}});
console.log(execFileSync('test/gpui/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim());
await sleep(3000);const c=await client(h);
for(const s of settings){const [k,...v]=s.split('=');c.send('setting.set',{key:k,value:v.join('=')})}
await sleep(500);
c.send('turn.start',{thread:'t1700000001-2',text:'hello'});await sleep(4000);
const mcp=readFileSync(work+'/fake.log','utf8').split('\n').filter(l=>l.startsWith('MCP ')).at(-1).split(' ')[2];
const rpc=async(method,params,id=1)=>(await (await fetch(mcp,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id,method,params})})).text());
writeFileSync(out+'/mcp-initialize.json',await rpc('initialize',{protocolVersion:'2025-06-18'}));
writeFileSync(out+'/mcp-tools.json',await rpc('tools/list',{}));
writeFileSync(out+'/mcp-ping.json',await rpc('ping',{}));
writeFileSync(out+'/mcp-badtool.json',await rpc('tools/call',{name:'no_such_tool',arguments:{}}));
writeFileSync(out+'/mcp-badmethod.json',await rpc('nope',{}));
writeFileSync(out+'/mcp-orch.json',await rpc('tools/call',{name:'orchestrator_capabilities',arguments:{}}));
writeFileSync(out+'/mcp-threadlist.json',await rpc('tools/call',{name:'thread_list',arguments:{}}));
stopHub(h);await sleep(1500);
const log=readFileSync(work+'/fake.log','utf8').split('\n').filter(l=>l.startsWith('ARGV '));
writeFileSync(out+'/argv.json',log.at(-1).slice(5));console.log('argv ->',out+'/argv.json');process.exit(0);
