// Long-lived fixture hub for device journeys (iOS/Android): run on h@backplane or Boat-class machines, NEVER the desktop.
//   bun serve-fixture.mjs --hub-bin PATH --root DIR --host 100.x.y.z --port 3850 [--threads 20] [--policy-bin PATH --window-bin PATH]   (the last two for the Rust hub)
// The hub listens on 127.0.0.1:<port+1000>; a TCP proxy on <host>:<port> sits in front so the app can be cut off like a sleeping phone:
//   curl -X POST http://<host>:<port+1>/ctl/cut?secs=45     drop every connection and refuse new ones for 45 s
//   curl -X POST http://<host>:<port+1>/ctl/restart         restart the hub process on the same home (a reconnect against a fresh process)
//   curl -X POST http://<host>:<port+1>/ctl/agents?count=20 start 20 long-streaming stand-in turns (the long-session journey)
//   curl      http://<host>:<port+1>/ctl/state              {url, token path, threads, seedReport}
import {writeFileSync} from 'node:fs';import net from 'node:net';
import {startFixtureHub,startAgents} from './lib/hubfx.mjs';import {startHub,stopHub,sleep} from '../perf/lib.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};
const bin=arg('hub-bin'),root=arg('root'),host=arg('host','127.0.0.1'),port=Number(arg('port',3850)),inner=port+1000;
if(!bin||!root)throw Error('--hub-bin and --root required');
// Rust hub flags: --policy-bin PATH --window-bin PATH (binaries copied next to the hub); TZ=Etc/UTC for recordings
const extraArgs=[];for(const k of['policy-bin','window-bin'])if(arg(k))extraArgs.push('--'+k,arg(k));
const extraEnv={TZ:process.env.TZ||'Etc/UTC'};
const fx=await startFixtureHub({bin,root,port:inner,threads:Number(arg('threads',20)),extraArgs,extraEnv});
let refuseUntil=0;const conns=new Set();
const proxy=net.createServer(c=>{if(Date.now()<refuseUntil){c.destroy();return}conns.add(c);const u=net.connect(inner,'127.0.0.1');conns.add(u);c.pipe(u);u.pipe(c);for(const s of[c,u])s.on('close',()=>{conns.delete(c);conns.delete(u);c.destroy();u.destroy()});s_err(c);s_err(u)});
const s_err=s=>s.on('error',()=>{});
proxy.listen(port,host);
const ctl=Bun.serve({hostname:host,port:port+1,async fetch(req){const u=new URL(req.url);
 if(u.pathname==='/ctl/cut'){const secs=Number(u.searchParams.get('secs')||30);refuseUntil=Date.now()+secs*1000;for(const c of conns)c.destroy();return Response.json({cut:secs})}
 if(u.pathname==='/ctl/restart'){stopHub(fx.hub);await sleep(2000);const h=await startHub({bin,home:root+'/home',port:inner,standins:root+'/bin',extraEnv:{FAKE_SECS:'600',...extraEnv},extraArgs,log:root+'/hub-restart.log'});fx.hub=h;return Response.json({restarted:h.pid})}
 if(u.pathname==='/ctl/agents'){const n=Number(u.searchParams.get('count')||20);startAgents(fx,n);return Response.json({agents:n})}
 if(u.pathname==='/ctl/state')return Response.json({proxy:`${host}:${port}`,tokenFile:root+'/home/token',threads:fx.state.threads,project:fx.state.project,seedReport:fx.seedReport});
 return new Response('not found',{status:404})}});
writeFileSync(root+'/fixture.json',JSON.stringify({proxy:`${host}:${port}`,ctl:`http://${host}:${port+1}`,tokenFile:root+'/home/token',threads:fx.state.threads.length}));
console.log('fixture hub ready: app -> http://'+host+':'+port+'  token '+root+'/home/token  ctl http://'+host+':'+(port+1));
process.on('SIGTERM',async()=>{await fx.stop();process.exit(0)});await new Promise(()=>{});
