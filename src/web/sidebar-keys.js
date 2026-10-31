// Keyboard equivalents of the visible conversation rows' existing clicks.
export function installSidebarKeys(doc = document) {
  const selector = '.sidebar .thread[role="button"][data-on-click="select"]';
  doc.addEventListener('keydown', event => {
    if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.ctrlKey || event.metaKey || event.altKey) return;
    const row = event.target?.closest?.(selector);
    // Expanders and the buttons inside a row keep their own keys.
    if (!row || event.target !== row || event.shiftKey) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();event.stopImmediatePropagation();
      if (!event.repeat) row.click();
      return;
    }
    if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return;
    const rows = [...doc.querySelectorAll(selector)].filter(el => el.checkVisibility());
    const at = rows.indexOf(row);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? rows.length - 1 :
      Math.max(0, Math.min(rows.length - 1, at + (event.key === 'ArrowDown' ? 1 : -1)));
    event.preventDefault();event.stopImmediatePropagation();rows[next]?.focus();
  });
}
