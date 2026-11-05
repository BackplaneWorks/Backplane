// bun baseline.ts DB PORT : the same /history in Bun + bun:sqlite + plain TS policy (the production-style
// split: policy in the host language). Same routes, same body.
import {Database} from 'bun:sqlite';
import {answer} from './oracle';
const [path, port] = process.argv.slice(2);
const db = new Database(path, {readonly: true});
Bun.serve({port: Number(port), hostname: '127.0.0.1', fetch(req) {
  const u = new URL(req.url);
  if (u.pathname === '/history') return new Response(answer(db, u.searchParams.get('thread') ?? ''), {headers: {'Content-Type': 'application/json'}});
  return new Response('not found', {status: 404});
}});
console.error('baseline on', port);
