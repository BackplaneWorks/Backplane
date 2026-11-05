// Written by Backplane for its Pi threads; rewritten before every turn.
// The thread's Backplane MCP server, and an approval before each tool the
// thread's mode does not allow (BACKPLANE_PI_MODE: ask, edit or allow).
export default function (pi) {
  const url = process.env.BACKPLANE_PI_MCP || '';
  const mode = process.env.BACKPLANE_PI_MODE || 'ask';
  if (url) pi.registerMcpServer('backplane', { url, exposure: 'direct', description: 'Backplane: this thread, its project, viewers and other threads' });
  const reads = new Set(['read', 'grep', 'find', 'ls']);
  const edits = new Set(['edit', 'write']);
  pi.on('tool_call', async (event, ctx) => {
    const name = String(event.toolName);
    if (mode === 'allow' || reads.has(name) || name.startsWith('mcp__backplane__')) return;
    if (mode === 'edit' && edits.has(name)) return;
    const ok = await ctx.ui.confirm('backplane:approval', JSON.stringify({ tool: name, input: event.input }), { signal: ctx.signal });
    if (!ok) return { block: true, reason: 'The user declined this tool call.' };
  });
}
