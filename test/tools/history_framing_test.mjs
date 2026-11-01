import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

// Exercise the real plot camera without constructing a browser canvas.
const source = readFileSync(new URL('../../src/web/plot2d.js', import.meta.url), 'utf8');
const start = source.indexOf('class View {');
assert.ok(start >= 0);
const end = source.indexOf('\n}\n', start) + 3;
const context = {};
vm.runInNewContext(source.slice(start, end) + '\nthis.PlotView = View;', context);
const before = [100000, 100000, 130000, 140000];
const after = [0, 0, 30000, 40000];
const make = () => Object.assign(Object.create(context.PlotView.prototype), {
  plot: { box: before }, viewport: { width: 600, height: 720 },
  gesture: 0, scale: 1, ox: 0, oy: 0, touched: false, back: null,
  paints: 0, later() { this.paints++; }
});
const pose = v => [v.scale, v.ox, v.oy];
const fits = (v, box) => {
  const [x0, y0, x1, y1] = box;
  assert.ok(v.ox + x0 * v.scale >= -1e-6);
  assert.ok(v.oy + y0 * v.scale >= -1e-6);
  assert.ok(v.ox + x1 * v.scale <= v.viewport.width + 1e-6);
  assert.ok(v.oy + y1 * v.scale <= v.viewport.height + 1e-6);
};
const fitted = make();
fitted.fit();
const oldPose = pose(fitted);
fitted.show({ box: after });
assert.notDeepEqual(pose(fitted), oldPose);
fits(fitted, after);
fitted.show({ box: before });
fits(fitted, before);
console.log('ok distant snapshots keep an untouched Fit in both directions');

const manual = make();
manual.fit();manual.scale *= 1.7;manual.ox += 42;manual.oy -= 19;manual.touched = true;
const manualPose = pose(manual);
manual.show({ box: after });
assert.deepEqual(pose(manual), manualPose);
console.log('ok snapshot changes preserve the person’s exact pan and zoom');

const repeated = make();
repeated.fit();const gesture = repeated.gesture;
repeated.show({ box: [...before] });
assert.equal(repeated.gesture, gesture);
assert.equal(repeated.paints, 2);
console.log('ok streamed chunks with unchanged bounds do not refit');

const empty = make();empty.plot = null;
empty.show({ box: after });fits(empty, after);
assert.equal(empty.fitted, true);
console.log('ok the first available snapshot still fits');
