// UI intent only. The actor validates current source/target, searches metadata,
// and returns a bounded page. A server patch never counts as a new user click.
export function installBoardCatalog(doc, selected, dispatch) {
  const states = new WeakMap();
  function state(menu, source = menu.dataset.boardCatalog, scope = menu.dataset.scope) {
    let value = states.get(menu);
    if (!value || value.source !== source || value.scope !== scope) {
      value = { source, scope, intent: null, eventOpen: menu.dataset.renderOpen === '1' };
      states.set(menu, value);
    }
    return value;
  }
  function current(menu) {
    return menu?.isConnected !== false && menu?.dataset?.scope === selected();
  }
  function command(menu, open, query = '', offset = 0) {
    if (!current(menu)) return;
    dispatch('board-catalog', JSON.stringify({ source: menu.dataset.boardCatalog, open, query: query.slice(0, 160), offset }));
  }
  function ready(menu) {
    const input = menu.querySelector('[data-board-query]');
    return !input || input.dataset.composing !== '1' && input.dataset.renderQuery === input.value;
  }
  doc.addEventListener('toggle', event => {
    const menu = event.target;
    if (!menu?.matches?.('[data-board-catalog]') || !current(menu)) return;
    const value = state(menu);
    if (value.eventOpen === menu.open) return;
    value.intent = value.eventOpen = menu.open;
    command(menu, menu.open, menu.querySelector('[data-board-query]')?.value || '');
  }, true);
  // Old pager/selection buttons must not replace a search still in flight.
  // Capture runs before the generic action handler or menu-dismissal listener.
  doc.addEventListener('click', event => {
    const button = event.target.closest?.('[data-on-click="board-catalog"],[data-on-click="board-pick"]');
    const menu = button?.closest('[data-board-catalog]');
    if (menu && (!current(menu) || !menu.open || !ready(menu))) {
      event.preventDefault();event.stopImmediatePropagation();
    }
  }, true);
  doc.addEventListener('compositionstart', event => {
    if (event.target.matches?.('[data-board-query]')) event.target.dataset.composing = '1';
  });
  doc.addEventListener('compositionend', event => {
    if (!event.target.matches?.('[data-board-query]')) return;
    event.target.dataset.composing = '0';
    const menu = event.target.closest('[data-board-catalog]');
    if (menu?.open) command(menu, true, event.target.value);
  });
  doc.addEventListener('keydown', event => {
    const menu = event.target.closest?.('[data-board-catalog]');
    if (!menu?.open || !current(menu) || event.isComposing || event.target.dataset?.composing === '1' || event.ctrlKey || event.metaKey || event.altKey) return;
    const search = menu.querySelector('[data-board-query]');
    if (!search || !['Enter', 'ArrowDown', 'ArrowUp'].includes(event.key)) return;
    const query = search.value.trim().toLocaleLowerCase();
    const rows = ready(menu) ? [...menu.querySelectorAll('.board-option')].filter(row => !row.disabled && !row.hidden && row.dataset.boardPath.toLocaleLowerCase().includes(query)) : [];
    if (event.key === 'Enter' && event.target !== search) return;
    event.preventDefault();event.stopImmediatePropagation();
    if (event.key === 'Enter') rows[0]?.click();
    else {
      const at = rows.indexOf(event.target), next = event.key === 'ArrowDown' ? at + 1 : at < 0 ? rows.length - 1 : at - 1;
      (rows[next] || search)?.focus({ preventScroll: true });
    }
  }, true);
  doc.addEventListener('input', event => {
    if (!event.target.matches?.('[data-board-query]')) return;
    const menu = event.target.closest('[data-board-catalog]');
    if (menu?.open && !event.isComposing && event.target.dataset.composing !== '1') command(menu, true, event.target.value);
  });
  return {
    // Called before the keyed host patch adds/removes `open`. Keep the newest
    // local close across a delayed open ACK; mark the resulting toggle as a
    // renderer operation. A new source/target starts a fresh closed intent.
    renderOpen(menu, source, scope, serverOpen) {
      const value = state(menu, source, scope);
      const open = value.intent === false ? false : serverOpen;
      value.eventOpen = open;
      return open;
    },
  };
}
