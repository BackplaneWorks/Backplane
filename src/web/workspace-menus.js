// Disclosure, filtering and focus only. Bend supplies every action, label,
// selected identity and confirmation; this module never selects a board.
export function closeWorkspaceMenu(menu, restore = false) {
  menu.open = false;
  // Dismissing an armed confirmation means Cancel, whichever way it closes.
  menu.querySelector('[data-on-click="delete-no"]')?.click();
  if (restore) menu.querySelector(':scope > summary')?.focus({ preventScroll: true });
}

export function installWorkspaceMenus(doc = document, reclaim = () => {}) {
  const close = closeWorkspaceMenu;
  doc.addEventListener('toggle', event => {
    if (event.target.matches?.('.local-menu') && !event.target.open) close(event.target);
  }, true);
  doc.addEventListener('keydown', event => {
    const menu = event.target.closest?.('.local-menu[open]');
    if (!menu || event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === 'Escape') {
      event.preventDefault();event.stopImmediatePropagation();close(menu, true);reclaim();
    }
    if (event.key === 'Tab' && menu.classList.contains('workspace-menu') && matchMedia('(max-width:760px)').matches) {
      const controls = [...menu.querySelectorAll('summary,input,button')].filter(x => x.checkVisibility() && !x.disabled);
      const at = controls.indexOf(doc.activeElement);
      if (event.shiftKey && at === 0 || !event.shiftKey && at === controls.length - 1) {
        event.preventDefault();controls[event.shiftKey ? controls.length - 1 : 0]?.focus();
      }
    }
  }, true);
  doc.addEventListener('pointerdown', event => {
    for (const menu of doc.querySelectorAll('.local-menu[open]')) if (!menu.contains(event.target)) close(menu);
    const picker = doc.querySelector('.account-picker');
    if (picker && !picker.contains(event.target) && !event.target.closest?.('.account-chip')) doc.querySelector('.account-chip')?.click();
  });
  doc.addEventListener('click', event => {
    const summary = event.target.closest?.('.local-menu > summary');
    if (summary && !summary.parentElement.open) close(summary.parentElement);
    const button = event.target.closest?.('.local-menu-body button[data-on-click]');
    const menu = button?.closest('.local-menu');
    if (!menu || ['delete','delete-no'].includes(button.dataset.onClick)) return;
    close(menu, true);
  });
  doc.addEventListener('input', event => {
    if (!event.target.matches?.('[data-board-filter]')) return;
    const menu = event.target.closest('.board-selector'), query = event.target.value.trim().toLocaleLowerCase();
    let matches = 0;
    for (const row of menu.querySelectorAll('[data-board-path]')) {
      row.hidden = !row.dataset.boardPath.toLocaleLowerCase().includes(query);
      if (!row.hidden) matches++;
    }
    menu.querySelector('.board-no-match').hidden = matches > 0;
  });
}
