// Validate the actual Android GLES3 studio shaders in isolated WebGL2.
// This checks shader execution, not Android/Metal device runtime.
import assert from 'node:assert/strict';
import {readFileSync, existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const source = readFileSync('mobile/android/app/src/main/java/dev/backplane/mobile/PlotView.kt', 'utf8');
const vertex = /private const val MESH_V = """([\s\S]*?)"""/.exec(source)[1];
const fragment = /private const val MESH_F = """([\s\S]*?)"""/.exec(source)[1];
const {chromium} = await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH || 'tools/browser/node_modules/playwright-core/index.mjs')).href);
const executablePath = process.env.CHROME_BIN || ['/usr/bin/google-chrome-stable', '/usr/bin/chromium'].find(existsSync);
const browser = await chromium.launch({headless: true, executablePath, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader']});
try {
  const page = await browser.newPage();
  const results = await page.evaluate(({vertex, fragment}) => {
    const canvas = document.createElement('canvas');canvas.width = canvas.height = 64;
    const gl = canvas.getContext('webgl2');if (!gl) throw Error('WebGL2 unavailable');
    const program = gl.createProgram();
    for (const [kind, source] of [[gl.VERTEX_SHADER, vertex], [gl.FRAGMENT_SHADER, fragment]]) {
      const shader = gl.createShader(kind);gl.shaderSource(shader, source);gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw Error(gl.getShaderInfoLog(shader));
      gl.attachShader(program, shader);
    }
    gl.linkProgram(program);if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw Error(gl.getProgramInfoLog(program));
    gl.useProgram(program);gl.viewport(0, 0, 64, 64);
    gl.uniformMatrix4fv(gl.getUniformLocation(program, 'mvp'), false, new Float32Array([1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]));
    for (const [name, v] of Object.entries({eye:[0,0,3], right:[1,0,0], up:[0,1,0], forward:[0,0,-1], thermal:[0,25,0]})) gl.uniform3fv(gl.getUniformLocation(program, name), v);
    gl.uniform1f(gl.getUniformLocation(program, 'exposure'), 0);gl.uniform1f(gl.getUniformLocation(program, 'orthographic'), 0);
    const attribute = (index, values) => {
      const buffer = gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER, buffer);gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(values), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(index);gl.vertexAttribPointer(index, 3, gl.FLOAT, false, 0, 0);return buffer;
    };
    attribute(0, [-1,-1,0, 3,-1,0, -1,3,0]);attribute(1, [0,0,1, 0,0,1, 0,0,1]);
    const colors = {mask:[.04,.3,.07], polymer:[.03,.03,.03], ceramic:[.5,.32,.15], solder:[.55,.55,.55], gold:[.7,.5,.1], silk:[1,1,1]};
    const results = {};
    for (const [name, color] of Object.entries(colors)) {
      attribute(2, [...color,...color,...color]);gl.clear(gl.COLOR_BUFFER_BIT);gl.drawArrays(gl.TRIANGLES, 0, 3);
      const rgba = new Uint8Array(4);gl.readPixels(32, 32, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, rgba);results[name] = [...rgba];
    }
    gl.uniform3fv(gl.getUniformLocation(program, 'thermal'), [1,245,1]);gl.drawArrays(gl.TRIANGLES, 0, 3);
    const rgba = new Uint8Array(4);gl.readPixels(32,32,1,1,gl.RGBA,gl.UNSIGNED_BYTE,rgba);results.thermal = [...rgba];
    if (gl.getError() !== gl.NO_ERROR) throw Error('GLES shader draw error');
    return results;
  }, {vertex, fragment});
  for (const color of Object.values(results)) assert.equal(color[3], 255);
  assert.ok(results.silk.slice(0,3).every(x => x > 210), 'white silk must remain legible');
  assert.ok(results.polymer.slice(0,3).every(x => x > 10 && x < 150), 'dark polymer retains detail');
  assert.ok(results.mask[1] > results.mask[0] && results.mask[1] > results.mask[2], 'authored green mask hue retained');
  assert.notDeepEqual(results.thermal, results.silk, 'display-only thermal uniform must change presentation');
  assert.equal(new Set(Object.values(results).map(x => x.join(','))).size, 7);
  console.log(JSON.stringify({browser: await browser.version(), scope: 'actual GLES3 shader compile/link/draw in headless WebGL2', colors: results}));
} finally { await browser.close(); }
