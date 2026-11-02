// A single writer with deliberately slow shutdown. Uses only the test's home.
import { appendFileSync, closeSync, openSync, unlinkSync } from 'node:fs';
const args = process.argv.slice(2);
if (args[0] === '--version') { console.log('codex-cli 0.0.0');process.exit(0); }
// Account/model discovery can start app-server too; only exec owns a turn.
if (args[0] !== 'exec') process.exit(0);
const root = process.env.STEER_FIXTURE!;
const log = (ev: string) => appendFileSync(root + '/agent.jsonl', JSON.stringify({ ev, pid: process.pid, args, at: Date.now() }) + '\n');
let lock: number;
try { lock = openSync(root + '/writer', 'wx'); }
catch { log('conflict');console.error('thread already has an active writer');process.exit(1); }
log('start');
process.stdout.write(JSON.stringify({ type: 'thread.started', thread_id: 'fixture-session' }) + '\n');
process.on('SIGTERM', () => {
  log('stopping');
  setTimeout(() => { closeSync(lock);unlinkSync(root + '/writer');log('exited');process.exit(143); }, 600);
});
setInterval(() => {}, 1000);
