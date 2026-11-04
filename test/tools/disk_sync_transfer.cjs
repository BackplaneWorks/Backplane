// Reuse lane A's unchanged PR925 component helpers; never edit/fork them.
// The public input is staged by the Boat packet, not copied into this PR.
const fs = require('node:fs'), vm = require('node:vm'), { join } = require('node:path');
const { sha } = require('./disk_sync_component.cjs');
const PIN = 'dff6b994065190f3cdeaed62781e4ad296dbb4c7f6361a52f6db6942b7808633';
function transfer({ web = '', file = '' } = {}) {
  const template = fs.readFileSync(process.env.DISK_SYNC_TRANSFER_HARNESS, 'utf8');
  if (sha(template) !== PIN) throw Error('unreviewed transfer harness');
  const from = template.indexOf('function server(idleText: string) {'), to = template.indexOf('\nfor (let round = 0;', from);
  if (from < 0 || to < 0) throw Error('transfer helper shape');
  const helpers = template.slice(from, to);
  const host = fs.readFileSync('src/server/effects/host.js', 'utf8'), loop = fs.readFileSync('src/server/effects/loop.js', 'utf8');
  const sa = host.indexOf('function sock_send_static('), sb = host.indexOf('\nfunction sock_send_preview(', sa);
  const ia = loop.indexOf('async function bp_idle(io) {'), ib = loop.indexOf('\n// A slow synchronous', ia);
  if (sa < 0 || sb < 0 || ia < 0 || ib < 0) throw Error('production helper shape');
  const senderSource = host.slice(sa, sb), idle = loop.slice(ia, ib), sockets = new Set();
  const bun = { listen(options) {
    const callbacks = options.socket;
    return Bun.listen({ ...options, socket: { ...callbacks,
      open(socket) { sockets.add(socket); callbacks.open(socket); },
      close(socket) { sockets.delete(socket); callbacks.close(socket); },
      error(socket, error) { sockets.delete(socket); callbacks.error(socket, error); } } });
  } };
  const js = new Bun.Transpiler({ loader: 'ts' }).transformSync(helpers);
  const reuse = vm.runInNewContext(js + '\n;({server,get});', { fs, join, runInNewContext: vm.runInNewContext,
    request: require('node:http').request, performance, setImmediate, setTimeout, clearTimeout,
    senderSource, web, file, Bun: bun, Buffer, process, console });
  return { server: () => reuse.server(idle), get: reuse.get, sockets,
    identity: { template_sha256: PIN, exact_helpers_sha256: sha(helpers), transpiled_helpers_sha256: sha(js),
      sender_sha256: sha(senderSource), idle_sha256: sha(idle), loop_sha256: sha(loop) } };
}
module.exports = { transfer, PIN };
