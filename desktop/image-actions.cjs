'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { pathToFileURL } = require('node:url');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');
const run = promisify(execFile);
const MAX_PIXELS = 32 * 1024 * 1024;

// Only the local service's existing image endpoint denotes a desktop file.
// Never reinterpret another hub's URL, a URL credential, or a bare path here.
function localImagePath(source, origin, home = os.homedir()) {
  try {
    const base = new URL(origin), url = new URL(source, base);
    if (base.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(base.hostname) ||
        url.origin !== base.origin || url.username || url.password || url.hash ||
        url.pathname !== '/img' || [...url.searchParams.keys()].join(',') !== 'path') return null;
    let file = url.searchParams.get('path');
    if (!file || file.length > 4096 || /[\x00-\x1f]/.test(file) || !/\.(png|jpe?g|gif|webp|bmp)$/i.test(file)) return null;
    if (file.startsWith('~/')) file = home + file.slice(1);
    // Keep symlink/.. semantics identical to the path the hub actually read.
    // POSIX paths on Windows have no proved native mapping; withhold reveal.
    if (process.platform === 'win32' && !/^(?:[A-Za-z]:[\\/]|\\\\)/.test(file)) return null;
    return path.isAbsolute(file) ? file : null;
  } catch { return null; }
}

function createImageActions({ electron, origin, runFile = run, platform = process.platform }) {
  return {
    local(source) { return localImagePath(source, origin()) !== null; },
    async copy(png) {
      if (typeof png !== 'string' || png.length > 18 * 1024 * 1024 || !/^data:image\/png;base64,[A-Za-z0-9+/]+=*$/.test(png)) throw Error('Image is too large or unavailable');
      const bytes = Buffer.from(png.slice(22), 'base64');
      if (bytes.length < 24 || !bytes.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex')) || bytes.toString('ascii', 12, 16) !== 'IHDR') throw Error('Invalid image');
      const width = bytes.readUInt32BE(16), height = bytes.readUInt32BE(20);
      if (!width || !height || width * height > MAX_PIXELS) throw Error('Image is too large');
      const image = electron.nativeImage.createFromBuffer(bytes);
      if (image.isEmpty()) throw Error('Image could not be decoded');
      await electron.clipboard.write([new electron.ClipboardItem({
        'image/png': new Blob([new Uint8Array(image.toPNG())], { type: 'image/png' }),
      })]);
      // Report success only after the platform clipboard contains the bitmap.
      const items = await electron.clipboard.read();
      const item = items.find(value => value.types.includes('image/png'));
      if (!item) throw Error('Clipboard did not accept the image');
      const blob = await item.getType('image/png');
      const copied = electron.nativeImage.createFromBuffer(Buffer.from(await blob.arrayBuffer()));
      if (copied.isEmpty() || !copied.toBitmap().equals(image.toBitmap())) throw Error('Clipboard did not accept the image');
      return { copied: true };
    },
    async reveal(source) {
      const file = localImagePath(source, origin());
      if (!file) throw Error('This image is not a file on this desktop');
      const stat = await fs.stat(file).catch(() => null);
      if (!stat?.isFile()) throw Error('Image file no longer exists');
      if (platform === 'linux') {
        // FileManager1 replies when it accepts the exact file URI. Do not
        // silently fall back to opening its directory or claim it was shown.
        await runFile('dbus-send', ['--session', '--print-reply', '--reply-timeout=3000', '--dest=org.freedesktop.FileManager1', '--type=method_call',
          '/org/freedesktop/FileManager1', 'org.freedesktop.FileManager1.ShowItems', 'array:string:' + pathToFileURL(file).href, 'string:'], { timeout: 4000, maxBuffer: 4096 })
          .catch(() => { throw Error('File manager did not accept the reveal request'); });
        return { accepted: true };
      }
      if (platform === 'darwin') {
        await runFile('/usr/bin/open', ['-R', file], { timeout: 4000, maxBuffer: 4096 }).catch(() => { throw Error('Finder did not accept the reveal request'); });
        return { accepted: true };
      }
      electron.shell.showItemInFolder(file);
      return { requested: true }; // Electron's Windows API has no acknowledgement.
    },
  };
}

function installImageMenu(window, { electron, trusted, actions }) {
  const wc = window.webContents;
  wc.on('context-menu', (_event, params) => {
    if (!trusted(params)) return;
    if (params.isEditable) {
      const item = (role, flag) => ({role, enabled:params.editFlags?.[flag] === true});
      electron.Menu.buildFromTemplate([
        item('undo', 'canUndo'), item('redo', 'canRedo'), {type:'separator'},
        item('cut', 'canCut'), item('copy', 'canCopy'), item('paste', 'canPaste'),
        {type:'separator'}, item('selectAll', 'canSelectAll'),
      ]).popup({window});
      return;
    }
    if (params.mediaType !== 'image') return;
    const items = [{ label: 'Copy image', enabled: params.hasImageContents, click: () => {
      if (wc.isDestroyed() || !trusted(params)) return;
      // Chromium's copyImageAt also writes HTML containing the source URL.
      // Export only the hit image's pixels, then write a native bitmap, so
      // private hub URLs and signed query strings never reach the clipboard.
      const point = JSON.stringify([params.x, params.y]);
      const source = JSON.stringify(params.srcURL);
      void wc.executeJavaScript(`(() => {
        const image = document.elementFromPoint(...${point});
        if (!(image instanceof HTMLImageElement) || image.currentSrc !== ${source} || !image.naturalWidth || image.naturalWidth * image.naturalHeight > ${MAX_PIXELS}) throw Error('Image unavailable');
        const canvas = document.createElement('canvas');
        canvas.width = image.naturalWidth;canvas.height = image.naturalHeight;
        canvas.getContext('2d').drawImage(image, 0, 0);
        return canvas.toDataURL('image/png');
      })()`).then(png => { if (!wc.isDestroyed() && trusted(params)) return actions.copy(png); })
        .catch(() => electron.dialog.showErrorBox('Copy image', 'Image could not be copied. Try Save in the image viewer.'));
    } }];
    if (actions.local(params.srcURL)) items.push({ label: 'Show file', click: () => {
      if (!wc.isDestroyed() && trusted(params)) void actions.reveal(params.srcURL).catch(() => electron.dialog.showErrorBox('Show file', 'The image file could not be revealed. It may be missing, or the file manager is unavailable.'));
    } });
    electron.Menu.buildFromTemplate(items).popup({ window });
  });
}
module.exports = { localImagePath, createImageActions, installImageMenu };
