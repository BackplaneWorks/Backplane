// Pane proportions belong to this browser, hub and project. The saved-file
// viewer and its canvases retain their own source/camera lifetimes.
const DEFAULTS = { primary: 1.15 / 2.15, secondary: 0.5, home: 0.68 };
const clamp = (n, lo, hi) => Math.max(lo, Math.min(hi, n));
const attr = (el, name, value) => {
  value = String(value);
  if (el.getAttribute(name) !== value) el.setAttribute(name, value);
};

export function splitLimits(space, handle) {
  const available = Math.max(1, space - handle);
  const min = Math.min(0.45, 96 / available);
  return { available, min, max: 1 - min };
}

export function installProjectSplitters({ document: doc = document, window: win = window,
  storage = win.localStorage, scope = () => win.location.origin } = {}) {
  const ratios = new Map(), observed = new Set();
  let active = null;
  const observer = typeof win.ResizeObserver === 'function' ? new win.ResizeObserver(() => sync()) : null;
  const keyOf = edge => {
    const project = edge.closest('[data-split-project]')?.dataset.splitProject;
    return project ? 'backplane-project-split:v1:' + JSON.stringify([scope(), project, edge.dataset.projectSplit]) : null;
  };
  const read = (key, name) => {
    if (key && ratios.has(key)) return ratios.get(key);
    let ratio = DEFAULTS[name] ?? 0.5;
    try {
      const saved = key && storage.getItem(key), value = saved == null ? NaN : Number(saved);
      if (Number.isFinite(value) && value > 0 && value < 1) ratio = value;
    } catch { /* Unavailable storage still permits resizing this view. */ }
    if (key) {
      ratios.set(key, ratio);
      if (ratios.size > 128) ratios.delete(ratios.keys().next().value);
    }
    return ratio;
  };
  const geometry = edge => {
    const group = edge.parentElement;
    const horizontal = win.getComputedStyle(group).flexDirection.startsWith('column');
    const box = group.getBoundingClientRect(), grip = edge.getBoundingClientRect();
    return { group, horizontal, ...splitLimits(horizontal ? box.height : box.width, horizontal ? grip.height : grip.width) };
  };
  const paint = (edge, ratio, g = geometry(edge)) => {
    const value = clamp(ratio, g.min, g.max);
    attr(g.group, 'data-split-active', '1');
    const css = String(value);
    if (g.group.style.getPropertyValue('--split-ratio') !== css) g.group.style.setProperty('--split-ratio', css);
    attr(edge, 'aria-orientation', g.horizontal ? 'horizontal' : 'vertical');
    attr(edge, 'aria-valuemin', Math.round(g.min * 100));
    attr(edge, 'aria-valuemax', Math.round(g.max * 100));
    attr(edge, 'aria-valuenow', Math.round(value * 100));
    const first = edge.dataset.firstLabel || 'First pane', second = edge.dataset.secondLabel || 'Second pane';
    attr(edge, 'aria-valuetext', `${first} ${Math.round(value * 100)}%, ${second} ${Math.round((1 - value) * 100)}%`);
    return value;
  };
  const save = (edge, ratio, key = keyOf(edge)) => {
    if (!key) return;
    ratios.set(key, ratio);
    try { storage.setItem(key, String(ratio)); } catch {}
  };
  const reset = edge => {
    const key = keyOf(edge), ratio = DEFAULTS[edge.dataset.projectSplit] ?? 0.5;
    if (key) ratios.set(key, ratio);
    try { if (key) storage.removeItem(key); } catch {}
    paint(edge, ratio);
  };
  const edgeOf = event => event.target?.closest?.('[data-project-split]');
  const down = event => {
    const edge = edgeOf(event);
    if (!edge || event.button !== 0) return;
    active?.cancel();
    const key = keyOf(edge), g = geometry(edge);
    if (g.available <= 1) return;
    event.preventDefault();
    edge.focus({ preventScroll: true });
    const start = g.horizontal ? event.clientY : event.clientX;
    const ratio = clamp(read(key, edge.dataset.projectSplit), g.min, g.max);
    let value = ratio;
    const id = event.pointerId;
    try { edge.setPointerCapture(id); } catch {}
    edge.classList.add('on');
    doc.body.classList.add('resizing-project-split');
    const changedOwner = () => !edge.isConnected || keyOf(edge) !== key;
    const move = e => {
      if (e.pointerId !== id) return;
      if (changedOwner()) { finish(false); return; }
      const at = g.horizontal ? e.clientY : e.clientX;
      value = paint(edge, ratio + (at - start) / g.available, g);
    };
    const finish = keep => {
      if (!active || active.edge !== edge) return;
      active = null;
      edge.removeEventListener('pointermove', move);
      edge.removeEventListener('pointerup', up);
      edge.removeEventListener('pointercancel', cancel);
      edge.removeEventListener('lostpointercapture', cancel);
      edge.classList.remove('on');
      doc.body.classList.remove('resizing-project-split');
      if (keep && !changedOwner()) save(edge, value, key);
      else if (!changedOwner()) paint(edge, ratio);
      try { edge.releasePointerCapture(id); } catch {}
    };
    const up = e => { if (e.pointerId === id) finish(true); };
    const cancel = e => { if (e.pointerId === id) finish(false); };
    active = { edge, key, geometry:g, cancel: () => finish(false) };
    edge.addEventListener('pointermove', move);
    edge.addEventListener('pointerup', up);
    edge.addEventListener('pointercancel', cancel);
    edge.addEventListener('lostpointercapture', cancel);
  };
  const keyboard = event => {
    const edge = edgeOf(event);
    if (!edge || event.ctrlKey || event.metaKey || event.altKey || event.isComposing || event.keyCode === 229) return;
    const g = geometry(edge);
    const keys = g.horizontal ? ['ArrowUp', 'ArrowDown'] : ['ArrowLeft', 'ArrowRight'];
    if (![...keys, 'Home', 'End', 'Enter'].includes(event.key)) return;
    event.preventDefault();
    active?.cancel();
    if (event.key === 'Enter') { reset(edge); return; }
    const current = clamp(read(keyOf(edge), edge.dataset.projectSplit), g.min, g.max);
    const step = event.shiftKey ? 0.1 : 0.02;
    const next = event.key === 'Home' ? g.min : event.key === 'End' ? g.max
      : current + (event.key === keys[0] ? -step : step);
    save(edge, paint(edge, next, g));
  };
  const double = event => { const edge = edgeOf(event); if (edge) { active?.cancel(); reset(edge); } };
  function sync() {
    if (active && (!active.edge.isConnected || keyOf(active.edge) !== active.key)) active.cancel();
    if (active) {
      const g = geometry(active.edge);
      if (g.horizontal !== active.geometry.horizontal || g.available !== active.geometry.available) active.cancel();
    }
    const groups = new Set();
    for (const edge of doc.querySelectorAll('[data-project-split]')) {
      const group = edge.parentElement;
      groups.add(group);
      if (active?.edge !== edge) paint(edge, read(keyOf(edge), edge.dataset.projectSplit));
      if (!observed.has(group)) { observed.add(group); observer?.observe(group); }
    }
    for (const group of observed) if (!groups.has(group)) {
      observer?.unobserve(group); observed.delete(group);
      group.removeAttribute('data-split-active'); group.style.removeProperty('--split-ratio');
    }
  }
  doc.addEventListener('pointerdown', down);
  doc.addEventListener('keydown', keyboard);
  doc.addEventListener('dblclick', double);
  win.addEventListener('resize', sync);
  return { sync, dispose() {
    active?.cancel(); observer?.disconnect();
    doc.removeEventListener('pointerdown', down);
    doc.removeEventListener('keydown', keyboard);
    doc.removeEventListener('dblclick', double);
    win.removeEventListener('resize', sync);
  } };
}
