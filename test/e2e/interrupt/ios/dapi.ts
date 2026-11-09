// dapi.ts PORT TOKEN_FILE METHOD JSON_PARAMS: one data/1 call to a loopback hub; prints the reply's JSON
import { readFileSync } from "fs";
const [port, tokf, m, p] = process.argv.slice(2);
const tok = readFileSync(tokf, "utf8").trim();
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?data=1&enc=json&token=${tok}`);
ws.onmessage = (e) => { const j = JSON.parse(String(e.data)); if (j.t === "reply" && j.id === 1) { console.log(JSON.stringify(j)); ws.close(); process.exit(0); } };
ws.onopen = () => ws.send(JSON.stringify({ id: 1, m, p: JSON.parse(p || "{}") }));
setTimeout(() => { console.log('{"error":"timeout"}'); process.exit(1); }, 10000);
