// Keep native window decorations and shortcuts without an application menu bar.
function applicationMenu(Menu, platform) {
  return platform === 'darwin' ? Menu.buildFromTemplate([{label:'Backplane', submenu:[
    {role:'about'}, {type:'separator'}, {role:'hide'}, {role:'hideOthers'}, {role:'unhide'},
    {role:'toggleDevTools', visible:false, accelerator:'Command+Shift+I', acceleratorWorksWhenHidden:true},
    {type:'separator'}, {role:'quit'}
  ]}]) : null;
}
function installShortcuts(window, electron, browser = () => null, platform = process.platform, terminalFocus = () => false) {
  // DevTools consumes input before before-input-event. A hidden window menu
  // keeps its toggle native even while the tools have the keyboard.
  if (platform !== 'darwin' && window.setMenu) {
    window.setMenu(electron.Menu.buildFromTemplate([{label:'Developer Tools', visible:false,
      accelerator:'Control+Shift+I', acceleratorWorksWhenHidden:true, click:() => window.webContents.toggleDevTools()}]));
    window.setAutoHideMenuBar(false);window.setMenuBarVisibility(false);
  }
  const bind = contents => {
    let rightAlt = false;
    contents.on('before-input-event', (event, input) => {
    if (contents !== window.webContents && contents.getOwnerBrowserWindow?.() !== window) return;
    // Windows/Linux AltGr often arrives as Ctrl+Alt. Keep the right Alt
    // key's text-entry chord, including on layouts that report it as Alt.
    if (input.code === 'AltRight' || input.key === 'AltGraph') rightAlt = input.type !== 'keyUp';
    // Own-window shortcut works from the Browser guest too, without
    // registering a desktop-global shortcut or changing OS key bindings.
    if (input.type === 'keyDown' && !input.isComposing && !input.isAutoRepeat && !input.shift && !rightAlt &&
        !input.modifiers?.some(value => /altgr|altgraph/i.test(value)) &&
        input.key.toLowerCase() === 'i' && input.control &&
        (platform === 'darwin' ? input.meta && !input.alt : input.alt && !input.meta)) {
      event.preventDefault();window.webContents.focus();window.webContents.send('desktop:improvements', {});return;
    }
    if (input.type !== 'keyDown' || input.alt || input.isComposing) return;
    const key = input.key.toLowerCase();
    if (platform === 'darwin' && input.control && input.meta && key === 'f') { event.preventDefault(); window.setFullScreen(!window.isFullScreen()); return; }
    const command = platform === 'darwin' ? input.meta && !input.control : input.control && !input.meta;
    if (!command && key !== 'f11') return;
    const focused = electron.webContents.getFocusedWebContents();
    // A PTY owns Ctrl+C/A/Z/...; copying/pasting uses Ctrl+Shift+C/V on
    // Linux/Windows. macOS's Command editing shortcuts remain native.
    if (platform !== 'darwin' && command && focused === window.webContents && terminalFocus()) {
      if (command && input.shift && (key === 'c' || key === 'v')) {
        event.preventDefault(); focused[key === 'c' ? 'copy' : 'paste']();
      }
      return;
    }
    const edit = {z:input.shift?'redo':'undo', y:'redo', x:'cut', c:'copy', v:'paste', a:'selectAll'}[key];
    if (command && edit) {
      event.preventDefault();
      if (focused === window.webContents) focused[edit](); else browser()?.edit(edit);
    } else if (key === 'f11') {
      event.preventDefault(); window.setFullScreen(!window.isFullScreen());
    } else if (command && platform === 'darwin' && key === 'm') {
      event.preventDefault();window.minimize();
    } else if (command && key === 'i' && input.shift) {
      event.preventDefault();window.webContents.toggleDevTools();
    } else if (command && ['l','r','0','+','=','-','w','q'].includes(key)) {
      event.preventDefault();
      if (key === 'l') { window.webContents.focus(); window.webContents.send('desktop:browser-focus-address', {}); }
      if (key === 'r') input.shift ? window.webContents.reloadIgnoringCache() : window.webContents.reload();
      if (['0','+','=','-'].includes(key)) window.webContents.setZoomLevel(key === '0' ? 0 : Math.max(-5, Math.min(8, window.webContents.getZoomLevel() + (key === '-' ? -0.5 : 0.5))));
      if (key === 'w') window.close();
      if (key === 'q') electron.app.quit();
    }
    });
  };
  bind(window.webContents);
  const created = (_event, contents) => bind(contents);
  electron.app.on?.('web-contents-created', created);
  window.once?.('closed', () => electron.app.removeListener?.('web-contents-created', created));
}
module.exports = { applicationMenu, installShortcuts };
