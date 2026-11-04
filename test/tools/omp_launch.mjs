import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {expectedArgs, fileTests, errnoTests} from './omp_launch_shared.mjs';
const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
for (const files of ['/tmp/private turn', 'C:\\Users\\User 日本\\private turn']) {
  for (const [model, effort] of [['', ''], ['openai/test', 'high']]) {
    assert.deepEqual(JSON.parse(lib.args('http://127.0.0.1:1/mcp', 'always-ask', 'sessions', files, model, effort)),
      expectedArgs('http://127.0.0.1:1/mcp', 'always-ask', 'sessions', files, model, effort));
  }
}
fileTests(); errnoTests();
assert(lib.why(2,'missing').includes("hub's PATH"));
assert(lib.why(5,'Input/output error').includes('executable and permissions'));
assert.equal(lib.why(13,'Permission denied'),'Permission denied');
console.log('ok OMP production argv, independent private prompt files, write failure cleanup, missing CLI and Windows errno preservation');
