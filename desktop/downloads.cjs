'use strict';
const path = require('node:path');

function hubDownload(url, origin) {
  try {
    const u = new URL(url), o = new URL(origin);
    return ['http:', 'https:'].includes(u.protocol) && u.origin === o.origin && !u.username && !u.password
      && ['/files/get', '/files/zip'].includes(u.pathname);
  } catch { return false; }
}
function downloadName(name) {
  let base = String(name || '').split(/[\\/]/).pop().replace(/[\x00-\x1f\x7f:<>|?*"]/g, '_').replace(/[. ]+$/, '').trim();
  if (/^(con|prn|aux|nul|com[0-9¹²³]|lpt[0-9¹²³])(\..*)?$/i.test(base)) base = '_' + base;
  return base && !/^\.*$/.test(base) ? [...base].slice(0, 180).join('') : 'download';
}
// Keep Electron's native Save dialog, including overwrite confirmation and
// Cancel. Never setSavePath: that would silently bypass the dialog.
function installDownloads(session, { origin, downloads, onDone = () => {}, dialog, getWindow = () => null }) {
  const listener = (_event, item) => {
    if (!hubDownload(item.getURL(), origin())) return;
    item.setSaveDialogOptions({ defaultPath: path.join(downloads(), downloadName(item.getFilename())) });
    let finished = false, progress;
    item.once('done', (_e, state) => {
      finished = true; progress?.abort();
      getWindow()?.setProgressBar?.(-1);
      onDone(state, item.getSavePath());
    });
    item.on('updated', () => {
      // The Save dialog must finish first. This sheet supplies a Cancel
      // control during a transfer; completion dismisses it automatically.
      if (finished || !item.getSavePath()) return;
      const win = getWindow();
      const total = item.getTotalBytes();
      win?.setProgressBar?.(total > 0 ? item.getReceivedBytes() / total : 2);
      if (!dialog || !win || progress) return;
      progress = new AbortController();
      dialog.showMessageBox(win, { message: 'Downloading ' + downloadName(item.getFilename()),
        buttons: ['Cancel download'], cancelId: 0, signal: progress.signal })
        .then(() => { if (!finished) item.cancel(); }).catch(() => { if (!finished) item.cancel(); });
    });
  };
  session.on('will-download', listener);
  return () => session.removeListener('will-download', listener);
}
module.exports = { installDownloads, hubDownload, downloadName };
