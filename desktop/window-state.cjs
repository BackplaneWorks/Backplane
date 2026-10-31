'use strict';
const fs = require('node:fs');
const path = require('node:path');

function restoreWindowBounds(file, screen) {
  try {
    const bounds = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (!['x','y','width','height'].every(k => Number.isSafeInteger(bounds[k]) && Math.abs(bounds[k]) <= 1000000) || bounds.width < 360 || bounds.height < 480) return {};
    const area = screen.getDisplayMatching(bounds).workArea;
    const width = Math.min(bounds.width, area.width), height = Math.min(bounds.height, area.height);
    return {width, height, x:Math.max(area.x, Math.min(bounds.x, area.x + area.width - width)), y:Math.max(area.y, Math.min(bounds.y, area.y + area.height - height))};
  } catch { return {}; }
}

function rememberWindowBounds(window, file) {
  let timer;
  const save = () => {
    clearTimeout(timer);
    try {
      // A maximized/fullscreen window still has its last normal bounds.
      const bounds = window.getNormalBounds();
      fs.mkdirSync(path.dirname(file), {recursive:true});
      fs.writeFileSync(file + '.tmp', JSON.stringify(bounds), {mode:0o600});
      fs.renameSync(file + '.tmp', file);
    } catch {}
  };
  const later = () => { clearTimeout(timer);timer = setTimeout(save, 300);timer.unref?.(); };
  window.on('move', later);window.on('resize', later);window.on('close', save);
}
module.exports = {restoreWindowBounds, rememberWindowBounds};
