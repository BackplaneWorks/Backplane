// Written by Backplane for its Oh My Pi threads; rewritten before every turn.
// Backplane's MCP tools for this thread (BACKPLANE_OMP_MCP), as omp tools named
// as Claude sees them (mcp__backplane__<tool>), each a call to the MCP server.
export default async function (pi) {
  const url = process.env.BACKPLANE_OMP_MCP || '';
  if (!url) return;
  let session = '', n = 0;
  const call = async (method, params, signal) => {
    const headers = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' };
    if (session) headers['mcp-session-id'] = session;
    const id = ++n;
    const r = await fetch(url, { method: 'POST', headers, signal, body: JSON.stringify({ jsonrpc: '2.0', id, method, params }) });
    session = r.headers.get('mcp-session-id') || session;
    const text = (await r.text()).trim();
    const body = text.startsWith('{') ? text : (text.split('\n').filter(l => l.startsWith('data:')).map(l => l.slice(5)).pop() || '{}');
    const o = JSON.parse(body);
    if (o.error) throw new Error(o.error.message || 'Backplane MCP error');
    return o.result || {};
  };
  let tools = [];
  try {
    await call('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'backplane-omp', version: '1' } });
    tools = (await call('tools/list', {})).tools || [];
  } catch (e) {
    pi.on('session_start', (_e, ctx) => ctx.ui.notify('Backplane tools are not available: ' + String(e && e.message || e), 'warning'));
    return;
  }
  for (const t of tools) {
    pi.registerTool({
      name: 'mcp__backplane__' + t.name, label: 'backplane/' + t.name,
      description: t.description || ('Backplane: ' + t.name),
      parameters: t.inputSchema || { type: 'object', properties: {} },
      approval: 'read', loadMode: 'essential', strict: false,
      async execute(_id, params, signal) {
        const r = await call('tools/call', { name: t.name, arguments: params || {} }, signal);
        return { content: Array.isArray(r.content) ? r.content : [{ type: 'text', text: JSON.stringify(r) }], isError: !!r.isError };
      },
    });
  }
}
