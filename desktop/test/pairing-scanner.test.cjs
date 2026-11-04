'use strict';
// pairing-scanner.js's decisions: when the page may offer the camera, and
// what each getUserMedia failure tells the person
const {test} = require('node:test'), assert = require('node:assert/strict');
const load = () => import('../../src/web/pairing-scanner.js');
const media = {mediaDevices: {getUserMedia: async () => ({})}};
test('plain http:// from another machine has no camera: say why, offer nothing to press', async () => {
  const {cameraSupport, scannerWords} = await load();
  assert.deepEqual(cameraSupport({isSecureContext: false, navigator: {}}), {ok: false, why: 'insecure'});
  // (even where a browser leaves mediaDevices on an insecure page)
  assert.deepEqual(cameraSupport({isSecureContext: false, navigator: media}), {ok: false, why: 'insecure'});
  assert.match(scannerWords.insecure, /https:\/\//);assert.match(scannerWords.insecure, /camera app/);assert.match(scannerWords.insecure, /Short code/);
});
test('a secure page with getUserMedia offers the camera; one without says the browser cannot', async () => {
  const {cameraSupport} = await load();
  assert.deepEqual(cameraSupport({isSecureContext: true, navigator: media}), {ok: true, why: ''});
  assert.deepEqual(cameraSupport({isSecureContext: true, navigator: {}}), {ok: false, why: 'unsupported'});
});
test('each getUserMedia failure has its own words', async () => {
  const {cameraError, scannerWords} = await load();
  assert.equal(cameraError({name: 'NotAllowedError'}), 'denied');
  assert.equal(cameraError(Error('app_denied')), 'denied');
  assert.equal(cameraError({name: 'NotFoundError'}), 'missing');
  assert.equal(cameraError({name: 'NotReadableError'}), 'busy');
  assert.equal(cameraError(Error('x')), 'failed');
  for (const k of ['denied', 'missing', 'busy', 'failed', 'hint', 'other']) assert.ok(scannerWords[k].length > 10, k);
});
