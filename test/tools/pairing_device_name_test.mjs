import {test} from 'node:test';
import assert from 'node:assert/strict';
import {defaultDeviceName} from '../../src/web/pairing-device.js';

test('pairing derives editable, header-safe names from advertised OS and browser', () => {
  const cases = [
    [{userAgent:'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Version/18.0 Mobile/15 Safari/604.1'}, false, 'iPhone / Safari'],
    [{platform:'MacIntel',maxTouchPoints:5,userAgent:'Version/18.0 Safari/605.1'}, false, 'iPad / Safari'],
    [{platform:'Linux x86_64',userAgent:'Mozilla/5.0 Chrome/142.0 Safari/537.36'}, true, 'Linux / Desktop'],
    [{platform:'Linux x86_64',userAgent:'Mozilla/5.0 Firefox/140.0'}, false, 'Linux / Firefox'],
    [{userAgent:'Mozilla/5.0 (Linux; Android 15) Chrome/142.0 Mobile Safari/537.36'}, false, 'Android / Chrome'],
    [{userAgentData:{platform:'Windows'},userAgent:'Chrome/142.0 Safari/537.36 Edg/142.0'}, false, 'Windows / Edge'],
    [{platform:'MacIntel',userAgent:'Chrome/142.0 Safari/537.36'}, false, 'macOS / Chrome'],
    [{userAgent:'Mozilla/5.0 (X11; CrOS x86_64) Chrome/142.0 Safari/537.36'}, false, 'ChromeOS / Chrome'],
    [{}, false, 'Device / Browser'],
  ];
  for (const [nav, desktop, expected] of cases) {
    assert.equal(defaultDeviceName(nav, desktop), expected);
    assert.match(defaultDeviceName(nav, desktop), /^[\x20-\x7e]+$/);
  }
});
