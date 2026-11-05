// bun ws.ts PORT THREAD... : one WebSocket, one "history T" text message per thread; prints the replies.
const [port, ...threads] = process.argv.slice(2);
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
let i = 0;
ws.onopen = () => ws.send('history ' + threads[0]);
ws.onmessage = e => { console.log(String(e.data)); if (++i < threads.length) ws.send('history ' + threads[i]); else { ws.close(); } };
ws.onerror = () => { console.error('ws error'); process.exit(1); };
