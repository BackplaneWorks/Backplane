// Window geometry and PTY cell measurement only. Never inspect or log output.
export function installFloatingTerminal({ dispatch, document: doc = document, window: win = window }) {
  let panel, observedScreen, resizeTimer, lastSize = '', drag, previousFocus, owner = '', lastCommand, frame;
  const positions = new Map();
  const context = doc.createElement('canvas').getContext('2d');
  const put = (name, value) => { if (panel.style[name] !== value) panel.style[name] = value; };
  let sentFocus;
  const focusState = () => {
    const active = !!doc.activeElement?.closest?.('[data-keys]');
    if (active !== sentFocus) { sentFocus = active;win.backplaneDesktop?.terminalFocus?.(active); }
  };
  doc.addEventListener('focusin', focusState);
  doc.addEventListener('focusout', () => queueMicrotask(focusState));
  win.addEventListener('focus', () => { sentFocus = undefined;focusState(); });
  win.addEventListener('blur', () => { sentFocus = false;win.backplaneDesktop?.terminalFocus?.(false); });
  const clamp = (n, low, high) => Math.max(low, Math.min(n, Math.max(low, high)));
  const fit = () => {
    if (!panel) return;
    const box = panel.getBoundingClientRect();
    put('maxWidth', `${Math.max(220, win.innerWidth - 16)}px`);
    put('maxHeight', `${Math.max(160, win.innerHeight - 16)}px`);
    put('left', `${clamp(box.left, 8, win.innerWidth - panel.offsetWidth - 8)}px`);
    put('top', `${clamp(box.top, 8, win.innerHeight - panel.offsetHeight - 8)}px`);
  };
  const measure = (opening = false) => {
    if (resizeTimer) return;
    resizeTimer = setTimeout(() => {
      resizeTimer = null;
      if (!panel || !observedScreen) return;
      const style = win.getComputedStyle(observedScreen);
      if (!context) return;
      context.font = style.font || `${style.fontSize} ${style.fontFamily}`;
      const cell = context.measureText('0000000000').width / 10;
      const cols = clamp(Math.floor((observedScreen.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight)) / cell), 20, 400);
      const rows = clamp(Math.floor((observedScreen.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)) / (parseFloat(style.lineHeight) || 17)), 5, 150);
      const size = `${cols}x${rows}`, key = panel.dataset.terminalOwner + ':' + size;
      if (cell > 0 && key !== lastSize) { lastSize = key;dispatch('ws-term-resize', size); }
    // Size a newly mounted session immediately. Only repeated drag/resize
    // events need the trailing debounce; delaying first size delays PTY setup.
    }, opening ? 0 : 120);
  };
  const observer = new ResizeObserver(() => {
    if (!panel || frame) return;
    frame = win.requestAnimationFrame(() => { frame = null;fit();measure(); });
  });
  const sync = records => {
    focusState();
    // Output patches keep the panel and screen. They must not repeatedly
    // measure text, force layout or scan the whole conversation.
    const next = panel?.isConnected ? panel : doc.querySelector('.floating-terminal');
    const nextOwner = next?.dataset.terminalOwner || '';
    if (next !== panel || nextOwner !== owner || panel && !observedScreen?.isConnected) {
      const mounting = next !== panel;
      if (panel) {
        positions.set(owner, { left: panel.style.left, top: panel.style.top, width: panel.style.width, height: panel.style.height });
        observer.disconnect();
      }
      panel = next; owner = nextOwner; observedScreen = null;lastSize = '';
      if (!panel) {
        if (previousFocus?.isConnected) previousFocus.focus();
        return;
      }
      if (mounting) previousFocus = doc.activeElement;
      Object.assign(panel.style, positions.get(panel.dataset.terminalOwner) || {});
      observedScreen = panel.querySelector('.screen');
      if (observedScreen) observer.observe(observedScreen);
      fit();measure(true);
    }
    // Font/scale settings can change cell width without changing the box.
    // Recheck only inherited style changes, never ordinary output patches.
    if (panel && records?.some(r => r.type === 'attributes' && r.attributeName !== 'data-terminal-owner' && (r.target.contains(panel) || r.target === observedScreen))) measure();
    const chosen = doc.querySelector('.workspace-palette .workspace-command.on');
    if (chosen && chosen !== lastCommand) chosen.scrollIntoView({ block: 'nearest' });
    lastCommand = chosen;
  };
  new MutationObserver(sync).observe(doc.getElementById('root') || doc.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-terminal-owner', 'class', 'style'] });
  doc.fonts?.addEventListener('loadingdone', () => { if (panel) measure(); });
  doc.addEventListener('pointerdown', event => {
    const handle = event.target.closest?.('.floating-terminal-handle');
    if (!panel || !handle || event.target.closest('button') || event.button !== 0) return;
    const box = panel.getBoundingClientRect();
    drag = { x: event.clientX - box.left, y: event.clientY - box.top, handle, id: event.pointerId };
    handle.setPointerCapture(event.pointerId);event.preventDefault();
  });
  doc.addEventListener('pointermove', event => {
    if (!drag || !panel || event.pointerId !== drag.id) return;
    put('left', `${event.clientX - drag.x}px`);put('top', `${event.clientY - drag.y}px`);fit();
  });
  const done = event => { if (drag && event.pointerId === drag.id) { drag.handle.releasePointerCapture(drag.id);drag = null; } };
  doc.addEventListener('pointerup', done);doc.addEventListener('pointercancel', done);
  win.addEventListener('resize', () => { fit();measure(); });
  sync();
}
