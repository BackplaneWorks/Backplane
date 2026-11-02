// Small ordinary table fixtures against Bend's emitted JavaScript parser.
// scripts/test.sh emits md_test.bend and runs this driver automatically.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
const md = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
const render = md['../src/web/dom.Md.render'];
const fixtures = [
  ['| a || c |\n| | b ||', '<table><tr><td>a</td><td></td><td>c</td></tr><tr><td></td><td>b</td><td></td></tr></table>'],
  ['| a | b\n| c | d', '<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>'],
  [' | 板 | 🟢 | \n| café | é |', '<table><tr><td>板</td><td>🟢</td></tr><tr><td>café</td><td>é</td></tr></table>'],
  ['| A | B |\n|---|:-:|\n| **a** | `b` |', '<table><tr><th>A</th><th>B</th></tr><tr><td><strong>a</strong></td><td><code>b</code></td></tr></table>'],
];
for (const [input, expected] of fixtures) assert.equal(md.shows(render(input)), expected);
console.log(`ok ${fixtures.length} emitted Markdown table contracts`);
