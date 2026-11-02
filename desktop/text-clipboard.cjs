'use strict';
// A write-only bridge: the authenticated app's main frame can copy text;
// guest pages and subframes never receive access to the OS clipboard.
function installTextClipboard(ipcMain, clipboard, trusted) {
  ipcMain.handle('desktop:clipboard-write-text', async (event, text) => {
    if (!trusted(event)) throw Error('Invalid clipboard sender');
    if (typeof text !== 'string' || Buffer.byteLength(text, 'utf8') > 4 * 1024 * 1024) throw Error('Invalid clipboard text');
    await clipboard.writeText(text);
    return {copied:true};
  });
}
module.exports = {installTextClipboard};
