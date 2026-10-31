'use strict';

function installLoadRecovery(window, {startupPath, startupURL, origin, session, status, reconnect, quitting, setTimer=setTimeout, clearTimer=clearTimeout}) {
  let recovery;
  const active = run => run && run === recovery && !quitting() && !window.isDestroyed() && window.webContents.getURL() === startupURL;
  const wait = async run => {
    if (!active(run) || run.pending) return;
    run.pending = true;
    try {
      // Probe the selected service. An outage must not start another hub.
      const response = await session.fetch(origin() + '/hello', {cache:'no-store', signal:AbortSignal.timeout(1000)});
      if (!active(run)) return;
      if (response.ok) { recovery = null;await reconnect();return; }
    } catch {} finally { run.pending = false; }
    if (active(run)) { run.timer = setTimer(() => { run.timer = null;void wait(run); }, 1000);run.timer?.unref?.(); }
  };
  window.webContents.on('did-finish-load', () => {
    // The failed navigation can finish before the replacement document does.
    // Start only after the local document really owns the main frame.
    if (active(recovery) && !recovery.timer) void wait(recovery);
  });
  window.webContents.on('did-fail-load', (_event, code, _description, url, main) => {
    if (!main || code === -3 || quitting() || window.isDestroyed()) return;
    try { if (new URL(url).origin !== origin()) return; } catch { return; }
    clearTimer(recovery?.timer);recovery = {pending:false, timer:null};
    // Keep the error in main before replacing Chromium's empty error page.
    status('error', 'Service unavailable. Retrying…');
    void window.loadFile(startupPath).catch(() => {});
  });
  window.on('close', () => { clearTimer(recovery?.timer);recovery = null; });
}
module.exports = {installLoadRecovery};
