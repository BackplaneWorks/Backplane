// Written by Backplane for its Oh My Pi threads; rewritten before every turn.
// Backplane's MCP tools for this thread (BACKPLANE_OMP_MCP) and the built-in
// parts search (BACKPLANE_OMP_PCBPARTS, unset when it is off), as omp tools
// named as Claude sees them (mcp__<server>__<tool>), each a call to its server.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

const stdio = (server) => {
  const child = spawn(server.command, server.args, {
    env: { ...process.env, ...Object.fromEntries(server.env.map(({ name, value }) => [name, value])) },
    stdio: ['pipe', 'pipe', 'ignore'],
  });
  const pending = new Map();
  const rejectAll = error => {
    for (const done of pending.values()) done(null, error);
  };
  child.on('error', rejectAll);
  child.on('close', () => rejectAll(new Error('MCP server closed its connection')));
  createInterface({ input: child.stdout }).on('line', line => {
    try {
      const reply = JSON.parse(line);
      pending.get(reply.id)?.(reply, null);
    } catch { /* stdio transports require newline-delimited JSON */ }
  });
  return (request, signal) => new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(signal.reason);
    const cleanup = () => {
      clearTimeout(timer);
      signal?.removeEventListener('abort', cancel);
      pending.delete(request.id);
    };
    const done = (reply, error) => {
      cleanup();
      if (error) reject(error); else resolve(reply);
    };
    const cancel = () => done(null, signal.reason);
    const timer = setTimeout(() => done(null, new Error('MCP request timed out')), 30_000);
    signal?.addEventListener('abort', cancel, { once: true });
    if (request.id !== undefined) pending.set(request.id, done);
    child.stdin.write(JSON.stringify(request) + '\n', error => {
      if (error || request.id === undefined) done({}, error);
    });
  });
};

const connect = async (pi, server, url, config = null) => {
  if (config?.type === 'sse') return;
  const pipe = config?.command ? stdio(config) : null;
  let session = '', n = 0;
  const post = (body, signal) => {
    const headers = { ...Object.fromEntries((config?.headers || []).map(({ name, value }) => [name, value])), 'content-type': 'application/json', accept: 'application/json, text/event-stream' };
    if (session) headers['mcp-session-id'] = session;
    return fetch(url, { method: 'POST', headers, signal, body: JSON.stringify(body) });
  };
  const call = async (method, params, signal) => {
    const request = { jsonrpc: '2.0', id: ++n, method, params };
    if (pipe) {
      const reply = await pipe(request, signal);
      if (reply.error) throw new Error(reply.error.message || server + ' MCP error');
      return reply.result || {};
    }
    const r = await post(request, signal);
    session = r.headers.get('mcp-session-id') || session;
    const text = (await r.text()).trim();
    const body = text.startsWith('{') ? text : (text.split('\n').filter(l => l.startsWith('data:')).map(l => l.slice(5)).pop() || '{}');
    const o = JSON.parse(body);
    if (o.error) throw new Error(o.error.message || server + ' MCP error');
    return o.result || {};
  };
  let tools = [];
  try {
    await call('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'backplane-omp', version: '1' } });
    const ready = { jsonrpc: '2.0', method: 'notifications/initialized' };
    if (pipe) await pipe(ready); else await post(ready).then(r => r.text()).catch(() => {});
    tools = (await call('tools/list', {})).tools || [];
  } catch (e) {
    const who = server === 'backplane' ? 'Backplane tools' : 'The ' + server + ' tools';
    pi.on('session_start', (_e, ctx) => ctx.ui.notify(who + ' are not available: ' + String(e && e.message || e), 'warning'));
    return;
  }
  for (const t of tools) {
    pi.registerTool({
      name: 'mcp__' + server + '__' + t.name, label: server + '/' + t.name,
      description: t.description || (server + ': ' + t.name),
      parameters: t.inputSchema || { type: 'object', properties: {} },
      approval: config ? undefined : 'read', loadMode: 'essential', strict: false,
      async execute(_id, params, signal) {
        const r = await call('tools/call', { name: t.name, arguments: params || {} }, signal);
        return { content: Array.isArray(r.content) ? r.content : [{ type: 'text', text: JSON.stringify(r) }], isError: !!r.isError };
      },
    });
  }
};
export default async function (pi) {
  const url = process.env.BACKPLANE_OMP_MCP || '';
  if (!url) return;
  const parts = process.env.BACKPLANE_OMP_PCBPARTS || '';
  const servers = JSON.parse(process.env.BACKPLANE_OMP_SERVERS || '[]');
  delete process.env.BACKPLANE_OMP_SERVERS;
  await Promise.all([connect(pi, 'backplane', url), parts ? connect(pi, 'pcbparts', parts) : null,
    ...servers.map(server => connect(pi, server.name, server.url, server))]);
}
