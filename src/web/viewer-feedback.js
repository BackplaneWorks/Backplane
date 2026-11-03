// Only workspace destinations coalesce; sends, destructive actions, field edits
// and terminal toggles retain their ordered, once-only transport semantics.
export function viewerChoice(action, value) {
  return ['view-kind','home-view'].includes(action) && ['board','schematic','3d','split','browser'].includes(value)
    || ['mech','fm','diff'].includes(action) && value === ''
    || action === 'rv-page' && value === 'bom';
}

// An immediate acknowledgement of an accepted click, never a claim that a
// model/file has finished loading. The production viewer owns actual readiness.
export function installViewerFeedback(root = document) {
  let pending = null;
  const sync = () => {
    for (const bar of root.querySelectorAll('.workspace-bar,.viewer-tabs,.top-views')) {
      for (const button of bar.querySelectorAll('button')) {
        const selected = pending && button.dataset.onClick === pending.action && button.dataset.value === pending.value;
        if (selected && button.dataset.viewerPending !== 'true') button.dataset.viewerPending = 'true';
        if (!selected && button.hasAttribute('data-viewer-pending')) button.removeAttribute('data-viewer-pending');
      }
    }
    const status = root.querySelector('.workspace-feedback');
    const loading = root.querySelector('.workspace-bar')?.dataset.loading || 'Loading…';
    if (status) {
      const text = pending ? loading : '';
      if (status.textContent !== text) status.textContent = text;
    }
    // Phones select from the top bar; their desktop rail (and its status)
    // is hidden. Keep acknowledgement visible while the hub draws the pane.
    const top = root.querySelector('.top-views');
    if (top) {
      let note = top.querySelector('.viewer-choice-status');
      if (!note) {
        note = top.ownerDocument.createElement('span');note.className = 'viewer-choice-status';
        note.setAttribute('role', 'status');top.append(note);
      }
      const text = pending ? loading : '';
      if (note.textContent !== text) note.textContent = text;
      const summary = top.querySelector('.workspace-menu > summary');
      if (pending) summary?.setAttribute('aria-busy', 'true');
      else summary?.removeAttribute('aria-busy');
    }
    const viewer = root.querySelector('#viewer');
    if (viewer) {
      if (pending) {
        if (!viewer.hasAttribute('data-viewer-pending')) viewer.dataset.viewerPending = 'true';
        let note = viewer.querySelector('.viewer-pending');
        if (!note) { note = viewer.ownerDocument.createElement('div');note.className = 'viewer-pending';viewer.append(note); }
        if (note.textContent !== loading) note.textContent = loading;
      } else {
        if (viewer.hasAttribute('data-viewer-pending')) viewer.removeAttribute('data-viewer-pending');
        viewer.querySelector('.viewer-pending')?.remove();
      }
    }
  };
  const Observer = root.defaultView?.MutationObserver || MutationObserver;
  const observer = new Observer(sync);observer.observe(root, { subtree: true, childList: true });
  return { choice(value) { pending = value;sync(); }, dispose() { observer.disconnect(); } };
}
