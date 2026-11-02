// Exercise the actual stateless bridge adapters with a stubbed shared policy.
// Core format/normalization checks live with the parent's issue-report tests.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
const source = fs.readFileSync(new URL('../../../src/mobile/bridge.js', import.meta.url), 'utf8');
const section = source.slice(source.indexOf('  improvementsPreview('), source.indexOf('  issueReport('));
const edited = source.match(/  improvementsPreviewBody\(body\) \{[^]*?\n  \},/)[0];
const text = '  + & ? # / 日本語 👩🏽‍💻\r\nkeep trailing spaces  \n';
let args;
const IssueScreen = {
  improvements_preview(...values) { args = values; return {body: values[0], url: ''}; },
  improvements_preview_body(body) { args = [body]; return {body, url: ''}; }
};
const methods = vm.runInNewContext(`({${section}${edited}})`, { IssueScreen, plain: value => value });
assert.equal(JSON.parse(methods.improvementsPreview(text, 'iOS', '1.2.3')).body, text);
assert.deepEqual(args, [text, 'iOS', '1.2.3', '', '']);
assert.equal(JSON.parse(methods.improvementsPreview(text, 'Android', '1.2.3', 'manual details', '')).body, text);
assert.deepEqual(args, [text, 'Android', '1.2.3', 'manual details', '']);
assert.equal(JSON.parse(methods.improvementsPreviewBody(text)).body, text);
assert.deepEqual(args, [text]);
console.log('Native bridge exact explicit-input checks passed; no hub or GitHub requests executed');
