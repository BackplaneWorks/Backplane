// Assertions shared by the headless web rig. Call with an isolated Playwright
// browser context after pairing and with URLs for populated synthetic fixtures.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

export async function checkAccess(page, enabled) {
  const statuses = await page.evaluate(async () => {
    const paths = ['/', '/assets/app.js', '/files/get', '/message-image', '/bots/t1/screen.jpg',
      '/auth/web/request', '/auth/web/poll', '/auth/web/logout'];
    return Promise.all(paths.map(async (path, i) => {
      if (i >= 5 && location.pathname === '/') {
        // Use an invalid name for the request so this check cannot create a pending approval.
        return (await fetch(path, {method:'POST', body:'{}'})).status;
      }
      return (await fetch(path)).status;
    }));
  });
  assert.deepEqual(statuses, enabled ? [200,401,401,401,401,400,401,401] : Array(8).fill(404));
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
