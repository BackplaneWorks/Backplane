// A fixture hub for the Android size matrix (android.py): the Rust hub on a temp home, stand-in agents, a seeded project, threads, bots and a room.
//   bun test/e2e/layout/android-fixture.mjs --hub-bin PATH --root DIR --port N [--threads 6]
// Prints one JSON line {url, token, tokenFile, threads, project} when ready; stops on SIGTERM. Run from the repo root.
import {startFixtureHub} from '../lib/hubfx.mjs';import {writeFileSync} from 'node:fs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};
const fx=await startFixtureHub({bin:arg('hub-bin'),root:arg('root'),port:Number(arg('port',3871)),threads:Number(arg('threads',6)),extraEnv:{TZ:'Etc/UTC'}});
const out={url:fx.state.url,token:fx.state.token,tokenFile:fx.state.root+'/home/token',threads:fx.state.threads,project:fx.state.project};
writeFileSync(arg('root')+'/fixture.json',JSON.stringify(out));console.log(JSON.stringify(out));
process.on('SIGTERM',async()=>{await fx.stop();process.exit(0)});await new Promise(()=>{});
