'use strict';
// Put back the file modes a component's receipt records, on a payload whose
// modes a transfer dropped (a GitHub artifact keeps none). A Windows component
// measured on Windows records 0666/0444, and the producer compares modes as
// recorded unless it runs on win32 itself. Hashes and sizes are checked, never
// changed: a file that differs stops it.
//   node desktop/restore-modes.cjs RECEIPT.json PAYLOAD_DIR
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const [receipt, root] = process.argv.slice(2);
if (!receipt || !root) { console.error('usage: restore-modes.cjs RECEIPT.json PAYLOAD_DIR'); process.exit(2); }
const members = JSON.parse(fs.readFileSync(receipt, 'utf8')).members;
let n = 0;
for (const [name, m] of Object.entries(members)) {
  if (m.type !== 'file') continue;
  const file = path.join(root, name), data = fs.readFileSync(file);
  if (data.length !== m.bytes || crypto.createHash('sha256').update(data).digest('hex') !== m.sha256) { console.error('restore-modes: ' + name + ' differs from its receipt'); process.exit(1); }
  fs.chmodSync(file, m.mode); n++;
}
console.log('restore-modes: ' + n + ' files');
