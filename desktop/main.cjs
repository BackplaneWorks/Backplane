const electron = require('electron');
const { app, BrowserWindow, Menu, dialog, ipcMain, session, shell } = electron;
const { installShortcuts, applicationMenu } = require('./window.cjs');
const { restoreWindowBounds, rememberWindowBounds } = require('./window-state.cjs');
const { installLoadRecovery } = require('./load-recovery.cjs');
const { iconPath } = require('./branding.cjs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const os = require('node:os');
const { Service } = require('./service.cjs');
const { createBrowserUnknownState } = require('./browser-dispatch.cjs');
const { createDesktopBrowserController } = require('./browser-controller.cjs');
const { createBrowserActions } = require('./browser-actions.cjs');
const { exactObject } = require('./browser-wire.cjs');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const { createConnectionLog } = require('./connection-log.cjs');
const { createImageActions, installImageMenu } = require('./image-actions.cjs');

// Checkout launches always name an isolated home. Packaging supplies its
// explicit product configuration after root's integration review.
const home = process.env.BACKPLANE_HOME || (app.isPackaged ? path.join(os.homedir(), '.backplane-bend') : null);
if (!home || !path.isAbsolute(home)) throw new Error('Set an absolute, isolated BACKPLANE_HOME');
if (!app.isPackaged && path.resolve(home) === path.join(os.homedir(), '.backplane-bend')) throw new Error('Development Electron must use an isolated BACKPLANE_HOME');
app.setPath('userData', path.join(home, 'electron-profile'));
const connectionLog = createConnectionLog(home);
app.setName('Backplane');
if (process.platform === 'linux') app.setDesktopName?.('backplane.desktop');
const resourceRoot = path.join(process.resourcesPath, 'backplane-service');
const desktopBuild = path.join(process.resourcesPath, 'DESKTOP-BUILD.json');
const build = app.isPackaged ? require(fs.existsSync(desktopBuild) ? desktopBuild : path.join(resourceRoot, 'BUILD.json')) : {};
const directBrowser=build.desktopBrowser?.protocol===1||(!app.isPackaged&&process.env.BACKPLANE_DESKTOP_BROWSER==='1');
// A local increment can retain the existing native Browser service while
// withholding its newer cooperative renderer interface until qualified.
const rendererBrowser=directBrowser&&build.desktopBrowser?.rendererBridge!==false;
if(directBrowser)process.env.BACKPLANE_DESKTOP_BROWSER='1';
// Main/home lifetime, including window/controller replacement. No reset API.
const browserUnknown=createBrowserUnknownState();
const service = new Service({ home, binary: process.env.BACKPLANE_SERVICE || (app.isPackaged ? path.join(resourceRoot, process.platform === 'win32' ? 'backplane-serve.exe' : 'backplane-serve') : undefined), web: process.env.BACKPLANE_WEB || (app.isPackaged ? path.join(resourceRoot, 'web') : undefined), expectedVersion: process.env.BACKPLANE_SERVICE_VERSION || build.serviceVersion, residentService: build.lifecyclePolicy?.mode === 'resident-service' });
const startupPath = path.join(__dirname, 'startup.html');
const startupURL = pathToFileURL(startupPath).href;
const launchedAtMs = Date.now();
let window, origin, connection, serviceReadyAtMs, firstVisibleAtMs, connecting;
let connectionStatus = {phase:'connecting', message:'Connecting to your service…'};
let desktopBrowser;
const imageActions = createImageActions({ electron, origin: () => origin });
let quitting = false;
let terminalFocused = false;
if (!app.requestSingleInstanceLock()) app.quit();
else {
  app.on('second-instance', () => { window?.show(); if (window?.isMinimized()) window.restore(); window?.focus(); });
  app.whenReady().then(start).catch(error => { dialog.showErrorBox('Backplane could not start', error.message); app.quit(); });
}
function senderIsWindow(event) {
  return window && event.sender === window.webContents && event.senderFrame === window.webContents.mainFrame;
}
function trusted(event) {
  try { return senderIsWindow(event) && origin && new URL(event.senderFrame.url).origin === origin; } catch { return false; }
}
// The transport port may change while the home and its conversations do not.
// Only the authenticated main frame can read or keep these text drafts.
const draftsFile = path.join(app.getPath('userData'), 'drafts.json');
const validDraft = ([key, text]) => typeof key === 'string' && key.startsWith('backplane-draft:') && key.length <= 4096 && (text === null || typeof text === 'string' && text.length <= 4 * 1024 * 1024);
let drafts = new Map();
try { drafts = new Map(JSON.parse(fs.readFileSync(draftsFile, 'utf8')).filter(entry => Array.isArray(entry) && entry.length === 2 && validDraft(entry))); } catch {}
ipcMain.on('desktop:drafts', (event, value) => {
  if (!trusted(event)) { event.returnValue = null;return; }
  if (value?.op === 'read') { event.returnValue = [...drafts];return; }
  if (value?.op !== 'write' || !validDraft([value.key, value.text])) { event.returnValue = null;return; }
  try {
    const next = new Map(drafts);next.set(value.key, value.text);
    fs.mkdirSync(path.dirname(draftsFile), {recursive:true});
    fs.writeFileSync(draftsFile + '.tmp', JSON.stringify([...next]), {mode:0o600});
    fs.renameSync(draftsFile + '.tmp', draftsFile);drafts = next;
    event.returnValue = true;
  } catch { event.returnValue = null; }
});
// The theme the hub's page last painted (host.js themeSeen), one word in
// this home's profile, so a cold window, the startup page and the page's
// first frame already have it. Nothing is read from or sent to the page but
// "light" or "dark"; with no word kept it is the hub's own default, dark.
const themeFile = path.join(app.getPath('userData'), 'theme');
let theme = 'dark';
try { if (fs.readFileSync(themeFile, 'utf8') === 'light') theme = 'light'; } catch {}
const themeBackground = () => theme === 'light' ? '#fafaf8' : '#111410';
ipcMain.on('desktop:theme', (event, value) => {
  if (!trusted(event) || (value !== 'light' && value !== 'dark') || value === theme) return;
  theme = value;window.setBackgroundColor(themeBackground());
  try { fs.writeFileSync(themeFile + '.tmp', theme);fs.renameSync(themeFile + '.tmp', themeFile); } catch {}
});
ipcMain.on('desktop:theme-now', event => {
  event.returnValue = senderIsWindow(event) && (event.senderFrame.url === startupURL || trusted(event)) ? theme : 'dark';
});
ipcMain.on('desktop:terminal-focus', (event, active) => {
  if (trusted(event) && typeof active === 'boolean') terminalFocused = active;
});
function external(url) {
  try { const parsed = new URL(url); if (['https:', 'http:', 'mailto:'].includes(parsed.protocol) && !parsed.username && !parsed.password) void shell.openExternal(parsed.href); } catch {}
}
function status(phase, message) {
  connectionStatus = {phase, message};
  if (window && !window.isDestroyed()) window.webContents.send('desktop:connection', { phase, message });
}
ipcMain.on('desktop:connection-now', event => {
  event.returnValue = senderIsWindow(event) && event.senderFrame.url === startupURL ? connectionStatus : null;
});
async function connect(ses) {
  if (connecting) return connecting;
  status('connecting', 'Connecting to your service…');
  connecting = (async () => {
    try {
      connection = await service.start(); origin = connection.origin; serviceReadyAtMs = Date.now();
      // Credential material stays in main and the service's HttpOnly cookie.
      const response = await ses.fetch(`${origin}/auth/pair`, { method: 'POST', credentials: 'include', headers: { Authorization: `Bearer ${await service.token()}` }, redirect: 'error' });
      if (!response.ok) throw new Error('Could not authenticate with the service');
      if (!quitting && window && !window.isDestroyed()) await window.loadURL(origin);
      // The in-app browser starts only after the UI is up and never holds it:
      // a hub that does not serve it (started by systemd, --headless or an
      // older install), or one that never answers, leaves the browser
      // unavailable with Reconnect (desktop:browser-reconnect) and nothing else.
      if(directBrowser)void connectBrowser(ses).catch(()=>{});
    } catch (error) {
      status('error', error.message);
    } finally { connecting = null; }
  })();
  return connecting;
}
// one attempt at a time: the start after load and a Reconnect share it
let browserStarting=null;
function connectBrowser(ses,reconnect=false){
  browserStarting??=startBrowser(ses,reconnect).finally(()=>{browserStarting=null;});
  return browserStarting;
}
async function startBrowser(ses,reconnect){
  if(desktopBrowser&&!desktopBrowser.isClosed())return;
  const browserWindow=window,selectedConnection=connection;
  // A user-requested fresh controller binds to the currently authenticated
  // service version at this same selected origin. Never replay a prior call.
  const browserConnection=reconnect?await service.discover({browserReconnectOrigin:selectedConnection.origin}):selectedConnection;
  if(!browserConnection)throw new Error('Selected Browser service unavailable');
  const guestPath=path.join(__dirname,'browser-guest.cjs'),preloadPath=path.join(__dirname,'browser-recording-preload.cjs');
  const hash=file=>createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const config=build.desktopBrowser||{};
  const token=await service.token();
  if(quitting||window!==browserWindow||browserWindow.isDestroyed()||connection!==selectedConnection||origin!==browserConnection.origin)return;
  const controller=createDesktopBrowserController({electron,window:browserWindow,appSession:ses,origin:browserConnection.origin,version:browserConnection.version,token,
    guestManagerSha256:config.guestManagerSha256||hash(guestPath),
    recorder:{preloadPath,preloadSha256:config.preloadSha256||hash(preloadPath),recorderSourceSha256:'2e4b03765cc4ae6595e32048555ffe27e750994a85c18b89027064f3c0c15350',subframePreload:true,popupPreloadVerified:false},
    unknownState:browserUnknown,executeCall:createBrowserActions(),
    onState:value=>{if(window===browserWindow&&!browserWindow.isDestroyed())browserWindow.webContents.send('desktop:browser-state',value);},
    onTakeover:value=>{if(window===browserWindow&&!browserWindow.isDestroyed())browserWindow.webContents.send('desktop:browser-takeover',value);}});
  desktopBrowser=controller;
  await controller.start();
  if(window!==browserWindow||browserWindow.isDestroyed())controller.close();
}
function start() {
  const ses = session.fromPartition('persist:backplane-ui');
  const permitted = (wc, permission, url) => {
    try { return wc === window?.webContents && origin && new URL(url).origin === origin && ['notifications', 'clipboard-sanitized-write'].includes(permission); } catch { return false; }
  };
  ses.setPermissionRequestHandler((wc, permission, callback, details) => callback(permitted(wc, permission, details.requestingUrl)));
  ses.setPermissionCheckHandler((wc, permission, requestingOrigin) => permitted(wc, permission, requestingOrigin));
  ses.webRequest.onHeadersReceived((details, callback) => {
    if (new URL(details.url).origin !== origin) return callback({});
    callback({ responseHeaders: { ...details.responseHeaders, 'Content-Security-Policy': ["default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; media-src 'self' blob:; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'self'; worker-src 'self' blob:"] } });
  });
  ipcMain.handle('desktop:version', event => {
    if (!trusted(event)) throw new Error('Invalid sender');
    return { electron: process.versions.electron, service: connection.version, launchedAtMs, firstVisibleAtMs, serviceReadyAtMs };
  });
  ipcMain.handle('desktop:retry', event => {
    if (!senderIsWindow(event) || event.senderFrame.url !== startupURL) throw new Error('Invalid sender');
    void connect(ses); return true;
  });
  ipcMain.handle('desktop:choose-folder', async event => {
    if (!trusted(event)) throw new Error('Invalid sender');
    const result = await dialog.showOpenDialog(window, { properties: ['openDirectory'] });
    return result.canceled ? null : result.filePaths[0];
  });
  for (const operation of ['local', 'copy', 'reveal']) ipcMain.handle('desktop:image-' + operation, (event, value) => {
    if (!trusted(event)) throw new Error('Invalid image sender');
    return imageActions[operation](value);
  });
  ipcMain.handle('desktop:browser-present',(event,value)=>{
    if(!trusted(event)||!directBrowser)throw new Error('Invalid Browser sender');
    exactObject(value,['owner','bounds']);exactObject(value.bounds,['x','y','width','height']);
    if(typeof value.owner!=='string'||!value.owner.length||value.owner.length>512||/[\x00-\x1f]/.test(value.owner))throw new Error('Invalid Browser owner');
    if(Object.values(value.bounds).some(v=>!Number.isFinite(v)||Math.abs(v)>1000000)||value.bounds.width<1||value.bounds.height<1)throw new Error('Invalid Browser geometry');
    const zoom=window.webContents.getZoomFactor(),box=window.getContentBounds();
    const x=Math.max(0,Math.min(box.width,Math.round(value.bounds.x*zoom))),y=Math.max(0,Math.min(box.height,Math.round(value.bounds.y*zoom)));
    const width=Math.min(box.width-x,Math.round(value.bounds.width*zoom)),height=Math.min(box.height-y,Math.round(value.bounds.height*zoom));
    if(!desktopBrowser||desktopBrowser.isClosed())return{shown:false,phase:'error',message:'Browser unavailable'};
    if(width<1||height<1){desktopBrowser.hide();return{shown:false,message:'Browser panel is hidden'};}
    return desktopBrowser.present({owner:value.owner,bounds:{x,y,width,height}}).then(result=>({shown:result.shown===true,...(result.message?{message:result.message}:{}),...(result.phase?{phase:result.phase}:{})}));
  });
  ipcMain.handle('desktop:browser-hide',event=>{if(!trusted(event)||!directBrowser)throw new Error('Invalid Browser sender');desktopBrowser?.hide();return true;});
  // Reloaded renderers subscribe after main may already have become ready or
  // failed. Query the current state instead of depending on a one-shot event.
  ipcMain.handle('desktop:browser-state',event=>{if(!trusted(event)||!directBrowser)throw new Error('Invalid Browser sender');return desktopBrowser?.state()??{phase:'connecting',message:'Connecting Browser…'};});
  ipcMain.handle('desktop:browser-reconnect',async event=>{if(!trusted(event)||!directBrowser)throw new Error('Invalid Browser sender');await connectBrowser(ses,true);return desktopBrowser?.state()??{phase:'error',message:'Browser unavailable'};});
  Menu.setApplicationMenu(applicationMenu(Menu, process.platform));
  app.setAppUserModelId?.('works.backplane.desktop');
  if (process.platform === 'darwin') app.dock?.setIcon(iconPath());
  void createWindow(ses).then(() => connect(ses)); // Show before any service/history work.
  app.on('activate', () => { if (!BrowserWindow.getAllWindows().length) void createWindow(ses).then(() => connect(ses)); });
}
function createWindow(ses) {
  const boundsFile = path.join(app.getPath('userData'), 'window-bounds.json');
  window = new BrowserWindow({ width: 1400, height: 940, ...restoreWindowBounds(boundsFile, electron.screen), minWidth: 360, minHeight: 480, title: 'Backplane', icon: iconPath(), backgroundColor: themeBackground(), show: false,
    webPreferences: { session: ses, preload: path.join(__dirname, 'preload.cjs'), nodeIntegration: false, contextIsolation: true, sandbox: true, webSecurity: true, webviewTag: false, spellcheck: true,additionalArguments:rendererBrowser?['--backplane-direct-browser=1']:[] } });
  window.webContents.on('console-message', (details, _level, legacyMessage) => {
    // Only our main frame, fixed-prefix and schema-sanitized records reach disk.
    if (!details.frame || details.frame === window.webContents.mainFrame) connectionLog.capture(details.message ?? legacyMessage);
  });
  window.setMenuBarVisibility(false);
  rememberWindowBounds(window, boundsFile);
  installLoadRecovery(window, {startupPath, startupURL, origin:() => origin, session:ses, status, reconnect:() => connect(ses), quitting:() => quitting});
  installImageMenu(window, { electron, actions: imageActions, trusted: params => {
    try { return params.frame === window.webContents.mainFrame && new URL(params.pageURL).origin === origin && new URL(params.frameURL).origin === origin; } catch { return false; }
  } });
  installShortcuts(window, electron, () => directBrowser ? desktopBrowser : null, process.platform, () => terminalFocused);
  window.webContents.on('did-start-navigation', () => { terminalFocused = false; });
  window.webContents.on('blur', () => { terminalFocused = false; });
  const shown = new Promise(resolve => window.once('ready-to-show', () => { firstVisibleAtMs ??= Date.now(); window.show(); resolve(); }));
  window.webContents.setWindowOpenHandler(({ url }) => { external(url); return { action: 'deny' }; });
  window.webContents.on('will-navigate', (event, url) => { if (url !== startupURL && new URL(url).origin !== origin) { event.preventDefault(); external(url); } });
  window.webContents.on('will-attach-webview', event => event.preventDefault());
  let closingWindow = false;
  window.on('close', () => { connectionLog.record({event:'window-close',cause:quitting?'app-quit':'window-close'});closingWindow = true;desktopBrowser?.close();desktopBrowser=null; });
  window.webContents.on('render-process-gone', (_event, details) => {connectionLog.record({event:'renderer-gone',cause:'renderer',rendererReason:details.reason,exitCode:details.exitCode});desktopBrowser?.hide();if (!quitting && !closingWindow && details.reason !== 'clean-exit' && !window.isDestroyed()) { void window.loadFile(startupPath).then(() => connect(ses)); } });
  void window.loadFile(startupPath);
  return shown;
}
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit(); });
app.on('before-quit', () => { connectionLog.record({event:'app-quit',cause:'app-quit'});quitting = true; });
// The Bend service belongs to the product, independently of UI windows. Closing,
// reloading, or crashing the renderer must not terminate agents, mail or jobs.
// Explicit service shutdown is a separate service-owned authenticated contract.
