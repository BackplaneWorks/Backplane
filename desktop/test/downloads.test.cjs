'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { installDownloads, hubDownload, downloadName } = require('../downloads.cjs');
test('Files downloads stay on their authenticated hub origin', () => {
  assert.ok(hubDownload('https://hub/files/get?thread=t&path=a', 'https://hub/'));
  for (const url of ['https://other/files/get', 'file:///files/get', 'https://hub/img', 'https://hub.evil/files/get', 'https://a@hub/files/get']) assert.ok(!hubDownload(url, 'https://hub'));
});
test('safe unicode suggestion, with a native save dialog and cancellation', () => {
  const session = new EventEmitter(), item = new EventEmitter();
  let options, result;
  item.getURL = () => 'https://hub/files/zip';
  item.getFilename = () => 'Schéma ✓.zip';
  item.getSavePath = () => '';
  item.setSavePath = () => assert.fail('must show the native save dialog');
  item.setSaveDialogOptions = v => { options = v; };
  const remove = installDownloads(session, { origin: () => 'https://hub', downloads: () => '/downloads', onDone: (...args) => { result = args; } });
  session.emit('will-download', {}, item);
  assert.deepEqual(options, { defaultPath: '/downloads/Schéma ✓.zip' });
  item.emit('done', {}, 'cancelled');
  assert.deepEqual(result, ['cancelled', '']);
  remove(); assert.equal(session.listenerCount('will-download'), 0);
  for (const name of ['../../CON.txt', 'C:\\NUL', '\r\n', '..']) assert.ok(!/[\\/\r\n]/.test(downloadName(name)));
});

test('a transfer can be cancelled after choosing its destination', async () => {
  const session = new EventEmitter(), item = new EventEmitter();
  let cancel = 0, resolveDialog, signal;
  item.getURL = () => 'https://hub/files/get'; item.getFilename = () => 'board.step';
  item.setSaveDialogOptions = () => {}; item.getSavePath = () => '/chosen/board.step';
  item.getTotalBytes = () => -1; item.getReceivedBytes = () => 12;
  item.cancel = () => { cancel++; item.emit('done', {}, 'cancelled'); };
  installDownloads(session, { origin: () => 'https://hub', downloads: () => '/downloads', getWindow: () => ({setProgressBar(){}}),
    dialog: { showMessageBox(_win, options) { signal = options.signal; return new Promise(r => { resolveDialog = r; }); } } });
  session.emit('will-download', {}, item); item.emit('updated');
  resolveDialog({ response: 0 }); await Promise.resolve();
  assert.equal(cancel, 1); assert.ok(signal.aborted);
});
