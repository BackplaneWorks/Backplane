// Written by Backplane for its Oh My Pi threads; rewritten before every turn.
// Backplane's MCP tools for this thread (BACKPLANE_OMP_MCP) and the built-in
// parts search (BACKPLANE_OMP_PCBPARTS, unset when it is off), as omp tools
// named as Claude sees them (mcp__<server>__<tool>), each a call to its server.
const connect = async (pi, server, url) => {
  let session = '', n = 0;
  const post = (body, signal) => {
    const headers = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' };
    if (session) headers['mcp-session-id'] = session;
    return fetch(url, { method: 'POST', headers, signal, body: JSON.stringify(body) });
  };
  const call = async (method, params, signal) => {
    const r = await post({ jsonrpc: '2.0', id: ++n, method, params }, signal);
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
    await post({ jsonrpc: '2.0', method: 'notifications/initialized' }).then(r => r.text()).catch(() => {});
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
      approval: 'read', loadMode: 'essential', strict: false,
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
  await Promise.all([connect(pi, 'backplane', url), parts ? connect(pi, 'pcbparts', parts) : null]);
}
