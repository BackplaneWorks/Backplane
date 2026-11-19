import assert from 'node:assert/strict';
import { checkProtectedAssets, checkSessionCookie } from './security.mjs';

// rpc is the synthetic native client's data/1 transport, connected to the
// same temporary hub. Approval goes through the action shown in Devices.
async function deviceButton(rpc, name, action) {
  const { rows } = await rpc('devices.page', {});
  const row = rows.find(row => row.label === name && row.buttons?.some(button => button.action === action));
  assert.ok(row, 'expected device action is absent');
  return { row, button: row.buttons.find(button => button.action === action) };
}

export async function browserTasks({ context, origin, rpc, thread, assets, replyText, screenshot }) {
  const page = await context.newPage();
  const name = 'Headless browser';
  const testId = id => page.getByTestId(id);
  await page.goto(origin);
  await testId('pair-card').waitFor();
  await screenshot(page, 'before-pair');
  await testId('pair-name').fill(name);
  await testId('pair-request-btn').click();
  await testId('pair-code').waitFor();
  const code = (await testId('pair-code').innerText()).replaceAll(' ', '');
  assert.match(code, /^\d{6}$/);
  const approval = await deviceButton(rpc, name, 'device-web-approve');
  assert.ok(approval.row.note.includes(code), 'browser and native comparison codes differ');
  const connected = page.waitForEvent('websocket', socket => new URL(socket.url()).pathname === '/ws');
  const approvedAt = performance.now();
  await rpc('devices.page.act', approval.button);
  await testId('sidebar-list').waitFor();
  assert.ok(performance.now() - approvedAt <= 1000, 'approval to app exceeded one second');
  const socket = await connected;
  await checkSessionCookie(context, origin, 'https://other.example.test');
  await checkProtectedAssets(page, origin, assets, true);

  await testId('thread-row-' + thread).click();
  await testId('timeline').waitFor();
  await testId('composer-input').fill('/');
  await testId('slash-menu').waitFor();
  await testId('composer-input').fill('Hello from the browser test');
  await testId('composer-send').click();
  await testId('timeline').getByText(replyText, { exact: true }).waitFor();
  await screenshot(page, 'paired-chat');

  const revocation = await deviceButton(rpc, name, 'device-revoke');
  const closed = socket.waitForEvent('close', { timeout: 1000 });
  await Promise.all([closed, rpc('devices.page.act', revocation.button)]);
  await testId('pair-card').waitFor();
  await checkProtectedAssets(page, origin, assets, false);
  await screenshot(page, 'revoked');
}
