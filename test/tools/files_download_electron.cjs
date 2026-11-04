// The desktop app's side of the Files tab's downloads, run by Electron itself
// (files_download_e2e.ts starts it): a session like the app's, the app's own
// download handling (desktop/downloads.cjs) installed on it, paired with the
// hub as a remote device (POST /auth/enroll with an invitation, the cookie the
// hub sets) or as the app pairs with its own hub (POST /auth/pair with the
// token), the hub's page loaded, the Files tab opened and Download clicked,
// then a row's download. Prints one JSON line with what happened.
//
//   electron files_download_electron.cjs ORIGIN SECRET invite|token OUTDIR [RESOLVER-RULES]
const { app, BrowserWindow, session } = require('electron');
const fs = require('node:fs'), path = require('node:path');
const { installDownloads } = require('../../desktop/downloads.cjs');

const [origin, secret, kind, out, rules] = process.argv.slice(2).filter(a => !a.startsWith('--'));
if (rules) app.commandLine.appendSwitch('host-resolver-rules', rules);
fs.mkdirSync(path.join(out, 'downloads'), { recursive: true });
app.setPath('userData', path.join(out, 'user'));
app.setPath('downloads', path.join(out, 'downloads'));
const sleep = ms => new Promise(r => setTimeout(r, ms));
const report = { origin, kind, windows: 0, navigations: [], done: [], errors: [] };
const finish = code => { console.log('RESULT ' + JSON.stringify(report)); app.exit(code); };
setTimeout(() => { report.errors.push('timed out'); finish(1); }, 120000);

app.whenReady().then(async () => {
  const ses = session.fromPartition('persist:backplane-ui-e2e');
  const finished = [];
  installDownloads(ses, { origin: () => origin, downloads: () => app.getPath('downloads'), onDone: (state, file) => finished.push({ state, file }) });
  ses.on('will-download', (_e, item) => {
    const options = item.getSaveDialogOptions();
    report.done.push({ name: item.getFilename(), chooser: options.defaultPath });
    if (!options.defaultPath) { report.errors.push('missing native save dialog'); item.cancel(); return; }
    // Headless substitute for choosing this destination in the OS dialog.
    item.setSavePath(path.join(out, 'downloads', item.getFilename()));
  });
  const r = kind === 'invite'
    ? await ses.fetch(origin + '/auth/enroll', { method: 'POST', credentials: 'include', headers: { Authorization: 'Bearer ' + secret, 'X-Backplane-Name': 'e2e laptop' } })
    : await ses.fetch(origin + '/auth/pair', { method: 'POST', credentials: 'include', headers: { Authorization: 'Bearer ' + secret } });
  report.paired = r.status;
  const cookies = await ses.cookies.get({ url: origin, name: 'bp_token' });
  report.cookie = cookies.length ? (cookies[0].value === secret ? 'the secret itself' : 'a device credential') : 'none';
  const win = new BrowserWindow({ show: false, width: 1400, height: 900, webPreferences: { session: ses, contextIsolation: true, sandbox: true, nodeIntegration: false } });
  win.webContents.on('will-navigate', (_e, url) => report.navigations.push(url));
  // (any other window, a download opened in one, counts)
  app.on('browser-window-created', () => { report.windows++; });
  win.webContents.setWindowOpenHandler(({ url }) => { report.navigations.push('window: ' + url); return { action: 'deny' }; });
  await win.loadURL(origin + '/');
  const js = code => win.webContents.executeJavaScript(code);
  const until = async (code, ms = 30000) => { const end = Date.now() + ms; while (Date.now() < end) { if (await js(code)) return true; await sleep(200); } return false; };
  if (!await until('!!document.querySelector(".sidebar .thread")')) { report.errors.push('no thread in the sidebar'); return finish(1); }
  await js('document.querySelector(".sidebar .thread").click()');
  if (!await until('[...document.querySelectorAll(\'[data-on-click="fm"]\')].some(e => e.checkVisibility())')) { report.errors.push('no Files tab'); return finish(1); }
  await js('[...document.querySelectorAll(\'[data-on-click="fm"]\')].find(e => e.checkVisibility()).click()');
  if (!await until('!!document.querySelector(".fm-bar a.fm-dl-bar") && document.querySelectorAll(".fm-item a.fm-dl").length > 3')) { report.errors.push('no download buttons'); return finish(1); }
  const before = win.webContents.getURL();
  await js('document.querySelector(".fm-bar a.fm-dl-bar").click()');
  const end = Date.now() + 60000;
  while (Date.now() < end && finished.length < 1) await sleep(200);
  await js('[...document.querySelectorAll(".fm-item")].find(e => e.querySelector(".fm-name")?.textContent === "ecc83-pp.kicad_pro").querySelector("a.fm-dl").click()');
  while (Date.now() < end && finished.length < 2) await sleep(200);
  report.finished = finished.map(f => ({ ...f, size: fs.existsSync(f.file) ? fs.statSync(f.file).size : -1 }));
  report.stayed = win.webContents.getURL() === before;
  report.rows = await js('document.querySelectorAll(".fm-list .fm-item").length');
  finish(finished.length === 2 && finished.every(f => f.state === 'completed') ? 0 : 1);
}).catch(e => { report.errors.push(String(e && e.stack || e)); finish(1); });
