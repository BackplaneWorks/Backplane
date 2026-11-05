// node bots.mjs PORT TOKEN PROJECT N IDSFILE : N stand-in "bots" streaming (claude stand-in, mode:stream; the hub's
// BP_STREAM_N / BP_STREAM_MS set the length and pace, mhub.sh passes them). Creates the threads once (titles
// "zqbot NN", ids saved in IDSFILE), then (re)starts a turn on each: run again to restart them.
import { connect } from '../../../skills/backplane-e2e/rpc.mjs';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
const [port, token, project, n = '20', file] = process.argv.slice(2);
const hub = await connect(port, token);
let ids = file && existsSync(file) ? readFileSync(file, 'utf8').trim().split('\n') : [];
if (!ids.length) {
  for (let i = 0; i < Number(n); i++) {
    const { thread } = await hub.call('thread.create', { project, title: `zqbot ${String(i).padStart(2, '0')}`, provider: 'claude' });
    ids.push(thread);
  }
  if (file) writeFileSync(file, ids.join('\n') + '\n');
}
for (const t of ids) await hub.call('turn.start', { thread: t, text: `mode:stream keep talking bot${t.slice(-3)}`, msg: `zq-${t}-${Date.now()}`, mode: '' }).catch((e) => console.error(t, e.message));
console.log(ids.length + ' bots started');
hub.close(); process.exit(0);
