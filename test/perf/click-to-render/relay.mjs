// node relay.mjs LISTEN_PORT TARGET_PORT RTT_MS : a TCP relay that delays every chunk by RTT/2 each way (FIFO per
// direction), so a browser pointed at LISTEN_PORT sees the hub across a network with that round-trip time.
// Bandwidth is not limited. Run inside the hub's namespace (mhub.sh node NAME relay.mjs ...).
import net from 'node:net';
const [lp, tp, rtt] = process.argv.slice(2).map(Number);
const half = rtt / 2;
const pipe = (from, to) => {
  from.on('data', (b) => { if (!half) to.write(b); else setTimeout(() => to.write(b), half); });
  from.on('end', () => setTimeout(() => to.end(), half));
  from.on('error', () => to.destroy());
};
net.createServer((c) => {
  c.setNoDelay(true);
  const u = net.connect(tp, '127.0.0.1'); u.setNoDelay(true);
  pipe(c, u); pipe(u, c);
}).listen(lp, '127.0.0.1', () => console.error(`relay ${lp} -> ${tp} rtt ${rtt} ms`));
