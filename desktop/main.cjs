const electron = require('electron');
const { app, BrowserWindow, Menu, dialog, ipcMain, session, shell } = electron;
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

// Checkout launches always name an isolated home. Packaging supplies its
// explicit product configuration after root's integration review.
const home = process.env.BACKPLANE_HOME || (app.isPackaged ? path.join(os.homedir(), '.backplane-bend') : null);
if (!home || !path.isAbsolute(home)) throw new Error('Set an absolute, isolated BACKPLANE_HOME');
if (!app.isPackaged && path.resolve(home) === path.join(os.homedir(), '.backplane-bend')) throw new Error('Development Electron must use an isolated BACKPLANE_HOME');
app.setPath('userData', path.join(home, 'electron-profile'));
app.setName('Backplane');
const resourceRoot = path.join(process.resourcesPath, 'backplane-service');
const desktopBuild = path.join(process.resourcesPath, 'DESKTOP-BUILD.json');
const build = app.isPackaged ? require(fs.existsSync(desktopBuild) ? desktopBuild : path.join(resourceRoot, 'BUILD.json')) : {};
const directBrowser=build.desktopBrowser?.protocol===1||(!app.isPackaged&&process.env.BACKPLANE_DESKTOP_BROWSER==='1');
if(directBrowser)process.env.BACKPLANE_DESKTOP_BROWSER='1';
// Main/home lifetime, including window/controller replacement. No reset API.
const browserUnknown=createBrowserUnknownState();
const service = new Service({ home, binary: process.env.BACKPLANE_SERVICE || (app.isPackaged ? path.join(resourceRoot, process.platform === 'win32' ? 'backplane-serve.exe' : 'backplane-serve') : undefined), web: process.env.BACKPLANE_WEB || (app.isPackaged ? path.join(resourceRoot, 'web') : undefined), expectedVersion: process.env.BACKPLANE_SERVICE_VERSION || build.serviceVersion, residentService: build.lifecyclePolicy?.mode === 'resident-service' });
const startupPath = path.join(__dirname, 'startup.html');
const startupURL = pathToFileURL(startupPath).href;
const launchedAtMs = Date.now();
let window, origin, connection, serviceReadyAtMs, firstVisibleAtMs, connecting;
let desktopBrowser;
let quitting = false;
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
function external(url) {
  try { const parsed = new URL(url); if (['https:', 'http:', 'mailto:'].includes(parsed.protocol) && !parsed.username && !parsed.password) void shell.openExternal(parsed.href); } catch {}
}
function status(phase, message) {
  if (window && !window.isDestroyed()) window.webContents.send('desktop:connection', { phase, message });
}
async function connect(ses) {
  if (connecting) return connecting;
  connecting = (async () => {
    try {
      connection = await service.start({ onStatus: message => status('waiting', message) }); origin = connection.origin; serviceReadyAtMs = Date.now();
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
function connectBrowser(ses){
  browserStarting??=startBrowser(ses).finally(()=>{browserStarting=null;});
  return browserStarting;
}
async function startBrowser(ses){
  if(desktopBrowser&&!desktopBrowser.isClosed())return;
  const guestPath=path.join(__dirname,'browser-guest.cjs'),preloadPath=path.join(__dirname,'browser-recording-preload.cjs');
  const hash=file=>createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const config=build.desktopBrowser||{};
  desktopBrowser=createDesktopBrowserController({electron,window,appSession:ses,origin,version:connection.version,token:await service.token(),
    guestManagerSha256:config.guestManagerSha256||hash(guestPath),
    recorder:{preloadPath,preloadSha256:config.preloadSha256||hash(preloadPath),recorderSourceSha256:'2e4b03765cc4ae6595e32048555ffe27e750994a85c18b89027064f3c0c15350',subframePreload:true,popupPreloadVerified:false},
    unknownState:browserUnknown,executeCall:createBrowserActions(),
    onState:value=>{if(window&&!window.isDestroyed())window.webContents.send('desktop:browser-state',value);},
    onTakeover:value=>{if(window&&!window.isDestroyed())window.webContents.send('desktop:browser-takeover',value);}});
  await desktopBrowser.start();
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
    return desktopBrowser.present({owner:value.owner,bounds:{x,y,width,height}}).then(result=>({shown:result.shown===true,...(result.message?{message:result.message}:{} )}));
  });
  ipcMain.handle('desktop:browser-hide',event=>{if(!trusted(event)||!directBrowser)throw new Error('Invalid Browser sender');desktopBrowser?.hide();return true;});
  ipcMain.handle('desktop:browser-reconnect',async event=>{if(!trusted(event)||!directBrowser)throw new Error('Invalid Browser sender');await connectBrowser(ses);return desktopBrowser?.state()??{phase:'error',message:'Browser unavailable'};});
  const edit=(label,operation,accelerator)=>({label,accelerator,click:()=>{
    const focused=electron.webContents.getFocusedWebContents();
    if(focused===window?.webContents)focused[operation]();else if(directBrowser)desktopBrowser?.edit(operation);
  }});
  const shellZoom=delta=>{const contents=window?.webContents;if(contents&&!contents.isDestroyed())contents.setZoomLevel(delta===null?0:Math.max(-5,Math.min(8,contents.getZoomLevel()+delta)));};
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    ...(process.platform === 'darwin' ? [{ role: 'appMenu' }] : []),
    { label: 'File', submenu: [{ role: 'close' }, { role: 'quit' }] },
    {label:'Edit',submenu:[edit('Undo','undo','CommandOrControl+Z'),edit('Redo','redo','CommandOrControl+Shift+Z'),{type:'separator'},edit('Cut','cut','CommandOrControl+X'),edit('Copy','copy','CommandOrControl+C'),edit('Paste','paste','CommandOrControl+V'),edit('Select all','selectAll','CommandOrControl+A')]},
    { label: 'View', submenu: [{label:'Address bar',accelerator:'CommandOrControl+L',click:()=>{window?.webContents.focus();window?.webContents.send('desktop:browser-focus-address',{});}},{label:'Reload interface',accelerator:'CommandOrControl+R',click:()=>window?.webContents.reload()},{label:'Reload interface without cache',accelerator:'CommandOrControl+Shift+R',click:()=>window?.webContents.reloadIgnoringCache()},{label:'Interface developer tools',accelerator:'CommandOrControl+Shift+I',click:()=>window?.webContents.toggleDevTools()},{type:'separator'},{label:'Actual size',accelerator:'CommandOrControl+0',click:()=>shellZoom(null)},{label:'Zoom in',accelerator:'CommandOrControl+Plus',click:()=>shellZoom(0.5)},{label:'Zoom out',accelerator:'CommandOrControl+-',click:()=>shellZoom(-0.5)},{ role: 'togglefullscreen' }] }, { role: 'windowMenu' },
  ]));
  void createWindow(ses).then(() => connect(ses)); // Show before any service/history work.
  app.on('activate', () => { if (!BrowserWindow.getAllWindows().length) void createWindow(ses).then(() => connect(ses)); });
}
function createWindow(ses) {
  window = new BrowserWindow({ width: 1400, height: 940, minWidth: 360, minHeight: 480, title: 'Backplane', backgroundColor: '#111410', show: false,
    webPreferences: { session: ses, preload: path.join(__dirname, 'preload.cjs'), nodeIntegration: false, contextIsolation: true, sandbox: true, webSecurity: true, webviewTag: false, spellcheck: true,additionalArguments:directBrowser?['--backplane-direct-browser=1']:[] } });
  const shown = new Promise(resolve => window.once('ready-to-show', () => { firstVisibleAtMs ??= Date.now(); window.show(); resolve(); }));
  window.webContents.setWindowOpenHandler(({ url }) => { external(url); return { action: 'deny' }; });
  window.webContents.on('will-navigate', (event, url) => { if (url !== startupURL && new URL(url).origin !== origin) { event.preventDefault(); external(url); } });
  window.webContents.on('will-attach-webview', event => event.preventDefault());
  let closingWindow = false;
  window.on('close', () => { closingWindow = true;desktopBrowser?.close();desktopBrowser=null; });
  window.webContents.on('render-process-gone', (_event, details) => {desktopBrowser?.hide();if (!quitting && !closingWindow && details.reason !== 'clean-exit' && !window.isDestroyed()) { void window.loadFile(startupPath).then(() => connect(ses)); } });
  void window.loadFile(startupPath);
  return shown;
}
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit(); });
app.on('before-quit', () => { quitting = true; });
// The Bend service belongs to the product, independently of UI windows. Closing,
// reloading, or crashing the renderer must not terminate agents, mail or jobs.
// Explicit service shutdown is a separate service-owned authenticated contract.
