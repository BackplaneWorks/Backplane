// send-turn.mjs PORT TOKEN_FILE THREAD TEXT: a person's message to a thread over the data socket (the turn starts on the hub's agent)
import {readFileSync} from 'node:fs';
import {sock} from '../e2e/lib/hubrows.mjs';
const [port, tokf, thread, text] = process.argv.slice(2);
const c = await sock({token: readFileSync(tokf, 'utf8').trim()}, Number(port));
console.log(JSON.stringify(await c.rpc('message.send', {thread, text}))); c.ws.close(); process.exit(0);
