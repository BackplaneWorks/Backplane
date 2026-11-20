// Assertions shared by the headless web rig. Call with an isolated Playwright
// browser context after pairing and with URLs for populated synthetic fixtures.
import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { request } from 'node:http';

// Exercise the same private backend stream used by the fixture TLS terminator.
export async function checkRequestBoundary(socketPath, origin, session) {
  const authority = new URL(origin).host;
  async function status(method, path, headers) {
    return new Promise((resolve, reject) => {
      const req = request({socketPath, method, path, headers:{Host:authority,
        'Tailscale-User-Login':'person@example.com', ...headers}}, response => {
        response.resume();
        resolve(response.statusCode);
      });
      req.on('upgrade', (_, socket) => { socket.destroy(); resolve(101); });
      req.on('error', reject);
      req.setTimeout(2500, () => req.destroy(Error('fixture request timed out')));
      req.end();
    });
  }
  const upgrade = {Upgrade:'websocket', Connection:'Upgrade', 'Sec-WebSocket-Version':'13',
    'Sec-WebSocket-Key':randomBytes(16).toString('base64')};
  const ws = '/ws?data=1&enc=json';
  assert.equal(await status('GET', ws, {...upgrade, Origin:origin}), 401);
  const Cookie = '__Host-bp_web=' + session;
  for (const originHeaders of [{}, {Origin:'https://other.example.test'}]) {
    assert.equal(await status('POST','/auth/web/logout',{Cookie,...originHeaders}), 401);
    assert.equal(await status('GET',ws,{Cookie,...upgrade,...originHeaders}), 401);
  }
  assert.equal(await status('GET','/',{Cookie,Host:'other.example.test'}),401);
}

export async function checkAccess(page, enabled) {
  const statuses = await page.evaluate(async () => {
    const paths = ['/', '/assets/app.js', '/files/get', '/message-image', '/bots/t1/screen.jpg',
      '/auth/web/request', '/auth/web/poll', '/auth/web/logout'];
    return Promise.all(paths.map(async (path, i) => {
      if (i >= 5) {
        // Use an invalid name for the request so this check cannot create a pending approval.
        return (await fetch(path, {method:'POST', body:'{}'})).status;
      }
      return (await fetch(path)).status;
    }));
  });
  assert.deepEqual(statuses, enabled ? [200,401,401,401,401,400,401,401] : Array(8).fill(404));
}

export async function checkRegisteredRoutes(page, routes) {
  assert.ok(routes.length > 0 && routes.every(path => path.startsWith('/') && !path.startsWith('//')));
  const refused = await page.evaluate(async paths => {
    for (const path of paths) {
      for (const method of ['GET', 'POST']) {
        const response = await fetch(path, {method});
        if (![401,404].includes(response.status)) return {path, method, status:response.status};
      }
    }
    return null;
  }, [...new Set([...routes, '/ws?data=1&enc=json', '/hello'])]);
  assert.equal(refused, null, JSON.stringify(refused));
}

export async function checkApprovalRestricted(page) {
  const result = await page.evaluate(() => new Promise((resolve, reject) => {
    const socket = new WebSocket('/ws?data=1&enc=json');
    const timer = setTimeout(() => { socket.close(); reject(Error('browser RPC timeout')); }, 4000);
    socket.onmessage = event => {
      const frame = JSON.parse(event.data);
      if (frame.t === 'hello') socket.send(JSON.stringify({id:1,m:'devices.page.act',p:{action:'device-web-approve',value:'fixture'}}));
      if (frame.t === 'reply' && frame.id === 1) {
        clearTimeout(timer);
        socket.close();
        resolve({ok:frame.ok, error:frame.error});
      }
    };
  }));
  assert.equal(result.ok, false);
  assert.equal(result.error, 'not allowed for browser');
}

export async function checkPageHeaders(page) {
  const result = await page.evaluate(async () => {
    const headers = (await fetch('/')).headers;
    return {
      csp: headers.get('content-security-policy'),
      sniff: headers.get('x-content-type-options'),
      referrer: headers.get('referrer-policy'),
      externalScripts: [...document.scripts].every(s => s.src && new URL(s.src).origin === location.origin),
    };
  });
  const directives = result.csp.split(';').map(part => part.trim());
  for (const directive of ["default-src 'none'", "script-src 'self'", "object-src 'none'", "frame-ancestors 'none'"]) {
    assert.ok(directives.includes(directive), directive);
  }
  assert.equal(result.sniff, 'nosniff');
  assert.equal(result.referrer, 'no-referrer');
  assert.ok(result.externalScripts);
}

export async function checkSessionCookie(context, origin, siblingOrigin, readRegistry) {
  const primary = new URL(origin);
  const sibling = new URL(siblingOrigin);
  assert.equal(primary.protocol, 'https:');
  assert.equal(sibling.protocol, 'https:');
  assert.notEqual(primary.hostname, sibling.hostname);

  const cookies = await context.cookies(origin);
  const sessions = cookies.filter(cookie => cookie.name === '__Host-bp_web');
  assert.equal(sessions.length, 1);
  const [session] = sessions;
  assert.equal(session.domain, primary.hostname);
  assert.equal(session.path, '/');
  assert.equal(session.secure, true);
  assert.equal(session.httpOnly, true);
  assert.equal(session.sameSite, 'Strict');
  assert.ok(session.value.length > 0);
  assert.ok(!cookies.some(cookie => cookie.name === '__Host-bp_web_pair'));
  const stored = await readRegistry();
  const digest = createHash('sha256').update(session.value).digest('hex');
  assert.ok(!stored.includes(session.value), 'browser credential was stored unsealed');
  assert.ok(JSON.parse(stored).devices.some(device => device.kind === 'browser' && device.hash === digest));
  const siblings = await context.cookies(siblingOrigin);
  assert.ok(!siblings.some(cookie => cookie.name === session.name));
  return session.value;
}

export async function checkProtectedAssets(page, origin, fixtures, paired) {
  for (const {path, body} of fixtures) {
    const url = new URL(path, origin);
    assert.equal(url.origin, new URL(origin).origin);
    assert.ok(!url.searchParams.has('token'));
    assert.ok(body.length > 0);
    const result = await page.evaluate(async ({ href, expected }) => {
      const response = await fetch(href, { cache: 'no-store', redirect: 'error' });
      const bytes = new Uint8Array(await response.arrayBuffer());
      return {
        status: response.status,
        cache: response.headers.get('cache-control') ?? '',
        matches: bytes.length === expected.length && bytes.every((byte, i) => byte === expected[i]),
      };
    }, { href: url.href, expected: [...body] });
    assert.equal(result.status, paired ? 200 : 401);
    if (paired) {
      const directives = result.cache.toLowerCase().split(',').map(part => part.trim());
      assert.ok(directives.includes('private'));
      assert.ok(directives.includes('no-store'));
      assert.ok(result.matches, 'unexpected fixture content');
    }
  }
}
