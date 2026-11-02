import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const { default: bend } = await import(pathToFileURL(process.argv[2]).href);
const api = name => bend['../src/core/issue-report.Issue.' + name];
const get = (value, key) => {
  for (let fields = value.fields; typeof fields?.key === 'string'; fields = fields.tail) {
    if (fields.key === key) return fields.value.text;
  }
  throw Error('Missing field ' + key);
};

assert.equal(api('template')(), readFileSync(new URL('../../docs/reporting/template.md', import.meta.url), 'utf8'));
assert.equal(api('unslop')(), readFileSync(new URL('../../docs/reporting/unslop/SKILL.md', import.meta.url), 'utf8'));
console.log('ok exact versioned template and supplied skill are embedded');

assert.equal(api('target')('', false), 'issue');
assert.equal(api('target')('', true), 'internal');
assert.equal(api('target')('issue', true), 'issue');
assert.equal(api('target')('internal', false), 'internal');
console.log('ok JavaScript Boolean ABI preserves legacy draft destination');

const text = '  Repro 🐛 日本語\r\n<literal>\t' + 'x'.repeat(100000);
const body = api('report')(text, 'Darwin', 'v2.3.4+private.machine', '', '');
assert.equal(body, text + '\n\n---\nOS: macOS\nApp version: 2.3.4\n');
assert.equal(api('url.body')(body), '');
console.log('ok large Unicode report stays exact with oversized link omitted');

for (const text of ['A & B #1? 100%\n日本語🐛', '?'.repeat(2600), 'x'.repeat(7915), 'x'.repeat(7916)]) {
  const preview = api('preview')(text, 'iOS', '1.2.3', '', '');
  const body = get(preview, 'body');
  const expected = api('url.prefix')() + encodeURIComponent(body).replace(/[!'()*]/g, c => '%' + c.charCodeAt(0).toString(16).toUpperCase());
  assert.equal(get(preview, 'url'), expected.length <= 8000 ? expected : '');
}
console.log('ok emitted URL matches complete UTF-8 body and exact byte budget');
