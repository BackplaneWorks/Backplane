// Authenticated presentation aliases are transport references only. Resolve
// them against this reply's exact controls before history/body/UI adoption.
// The server's delta base retains the bounded aliases, never this local copy.
export function restoreFrameAliases(shown, aliases, window) {
  if (!aliases) return shown;
  const keys = Object.keys(aliases);
  if (keys.some(k => !['@compact.entry','@compact.thread'].includes(k) || typeof aliases[k] !== 'string')) throw Error('Invalid frame aliases');
  let left = 20000;
  const visit = (node, depth = 0) => {
    if (--left < 0 || depth >= 64) throw Error('Invalid compact tree');
    if (node?.type !== 'element') return node;
    const attrs = node.attrs.map(a => {
      if (['data-id','data-thread'].includes(a.name) && Object.hasOwn(aliases, a.value)) return { ...a, value: aliases[a.value] };
      if (a.name === 'data-value' && a.value === '@compact.body') {
        const part = window?.partial?.find(p => p.id === aliases['@compact.entry']);
        if (!part) throw Error('Missing source continuation');
        return { ...a, value: JSON.stringify({ entryId: part.id, cursor: '' }) };
      }
      return a;
    });
    return { ...node, key: aliases[node.key] ?? node.key, attrs, kids: node.kids.map(k => visit(k, depth + 1)) };
  };
  return visit(shown);
}

// Viewer and conversation regions belong to the same authenticated reply.
// Reassemble before the normal validator/diff sees either region, so a canvas
// with the normal renderer's key is never removed for an intermediate slot.
export function restoreWindowProjection(shown, projection) {
  if (!projection) return shown;
  if (shown?.type !== 'element' || shown.key !== 'workspace-root' || shown.kids?.length !== 0 ||
      !shown.attrs?.some(a => a.name === 'data-window-region' && a.value === 'root') ||
      projection.root?.type !== 'element' || projection.root.key !== 'workspace-root') throw Error('Invalid canonical root region');
  return projection.root;
}
