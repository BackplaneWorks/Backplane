// bun test/e2e/rig-hub.mjs HUB_BIN ROOT PORT [threads]: a seeded fixture hub (stand-in agents) that stays up; prints its state as JSON and writes ROOT/state.json
import {writeFileSync} from 'node:fs';
import {startFixtureHub} from './lib/hubfx.mjs';
const [bin,root,port,threads]=process.argv.slice(2);
const fx=await startFixtureHub({bin,root,port:Number(port),threads:Number(threads||3),seedTurns:false});
writeFileSync(root+'/state.json',JSON.stringify({...fx.state,seed:fx.seedReport}));console.log(JSON.stringify(fx.state));
process.on('SIGTERM',async()=>{await fx.stop();process.exit(0)});setInterval(()=>{},1e6);
