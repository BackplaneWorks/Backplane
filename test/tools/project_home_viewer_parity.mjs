// Runs inside viewer_integration_e2e's private hub, using the production UI,
// original-model worker and saved board. No substitute renderer or snapshot
// asset is supplied to either view. Run after the build halt is lifted:
// bun test/tools/viewer_integration_e2e.mjs --bin BUILD/backplane-serve \
//   --web BUILD/web --project REAL_PROJECT --quality required --home-parity
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

export function pixelDiff(a, b, tolerance = 8) {
  assert.equal(a.length, b.length, 'frame dimensions differ');
  assert.ok(a.length && a.length % 4 === 0, 'empty or incomplete RGBA frame');
  let different = 0, absolute = 0;
  for (let p = 0; p < a.length; p += 4) {
    let peak = 0;
    for (let c = 0; c < 3; c++) {
      const delta = Math.abs(a[p + c] - b[p + c]);
      peak = Math.max(peak, delta); absolute += delta;
    }
    if (peak > tolerance) different++;
  }
  return { fraction: different / (a.length / 4), mean: absolute / (a.length / 4 * 3), tolerance };
}

export async function homeViewerParity({ page, probeOriginal, waitOriginal, sourceRevision, pcb, output, record }) {
  // Equal canvas dimensions make this an actual pixel comparison rather than
  // a comparison of two different crops/fit distances. Production CSS remains
  // in force for the separate desktop/mobile screenshots below.
  const sizing = await page.addStyleTag({ content: '#solid-workbench .viewer-stage:has(> #solid){width:400px!important;height:320px!important;min-height:320px!important;flex:none!important}' });
  const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const camera = () => page.evaluate(() => {
    const state = window.__viewerAcceptance.workerCameras.at(-1)?.state;
    return state && { cam: state.cam, projection: state.projection, exposure: state.exposure };
  });
  const frame = async name => {
    await waitOriginal('raster'); await settle();
    const status = await probeOriginal('status');
    assert.ok(!status.error && status.visible, 'original renderer must be visible');
    assert.equal(status.source, pcb); assert.equal(status.revision, sourceRevision);
    const png = await page.locator('.solid-pathtrace').screenshot();
    writeFileSync(join(output, name + '.png'), png);
    const pixels = await page.evaluate(async base64 => {
      const bitmap = await createImageBitmap(new Blob([Uint8Array.from(atob(base64), c => c.charCodeAt(0))], { type: 'image/png' }));
      const canvas = document.createElement('canvas'); canvas.width = bitmap.width; canvas.height = bitmap.height;
      const ctx = canvas.getContext('2d'); ctx.drawImage(bitmap, 0, 0); bitmap.close();
      return { width: canvas.width, height: canvas.height, rgba: Array.from(ctx.getImageData(0, 0, canvas.width, canvas.height).data) };
    }, png.toString('base64'));
    const load = await page.evaluate(() => window.__viewerAcceptance.workerLoads.at(-1));
    return { pixels, status, load, camera: await camera() };
  };
  await settle();
  const full = await frame('parity-full-viewer');
  await page.locator('[data-on-click="proj-home"][data-value="p1"]').first().click();
  await page.locator('main[data-project-home="p1"]').waitFor();
  // Do not click 3D here: this also tests the actual first-open default.
  await page.locator('#solid-workbench').waitFor();
  assert.equal(await page.getByLabel('Rendering', { exact: true }).inputValue(), 'studio');
  const embedded = await frame('parity-home-viewer');
  assert.deepEqual([embedded.pixels.width, embedded.pixels.height], [full.pixels.width, full.pixels.height]);
  for (const key of ['source', 'revision', 'sha256']) assert.equal(embedded.load[key], full.load[key], `home original asset ${key}`);
  assert.deepEqual(embedded.camera, full.camera, 'fresh home uses the same default camera, without reflection');
  assert.equal(embedded.status.materialPolicy, full.status.materialPolicy);
  const diff = pixelDiff(full.pixels.rgba, embedded.pixels.rgba);
  assert.ok(diff.fraction <= 0.001 && diff.mean <= 0.3, `home/full board pixel difference ${JSON.stringify(diff)}`);
  // An empty background could pass an equality test. Require actual variation.
  assert.ok(new Set(embedded.pixels.rgba).size > 32, 'comparison contains actual shaded board pixels');
  record('home and full Studio render the same saved board within pixel tolerance', { diff, source: pcb, revision: sourceRevision, sha256: embedded.load.sha256 });

  await page.getByRole('button', { name: 'Top', exact: true }).click(); await settle();
  const pose = await camera();
  await page.evaluate(() => { window.__homeParityCanvas = document.querySelector('#solid'); window.__homeParityWorker = window.__viewerAcceptanceWorker; });
  await page.getByRole('button', { name: 'Open viewer', exact: true }).click();
  await page.locator('main[data-home-viewer-full="1"]').waitFor(); await settle();
  assert.ok(await page.evaluate(() => window.__homeParityCanvas === document.querySelector('#solid') && window.__homeParityWorker === window.__viewerAcceptanceWorker), 'expansion retains the same renderer and canvas');
  assert.deepEqual(await camera(), pose, 'Open viewer retains the exact camera');
  await page.getByRole('button', { name: 'Back to project', exact: true }).click();
  await page.locator('main[data-home-viewer-full="0"]').waitFor(); await settle();
  assert.deepEqual(await camera(), pose, 'return retains the exact camera');
  record('Open viewer and Back to project controls retain mounted renderer and camera');
  const beforeSplit = await frame('parity-home-before-split');
  // Allocate room through the real accessible controls, so the fixed-size
  // comparison canvas is not clipped by the other two Split panes.
  await page.locator('[data-project-split="home"]').press('End');
  await page.locator('.viewer-tabs [data-on-click="home-view"][data-value="split"]').click();
  await page.locator('#combo-3d #solid-workbench').waitFor();
  await page.locator('[data-project-split="primary"]').press('Home');
  await page.locator('[data-project-split="secondary"]').press('Home');
  const split = await frame('parity-home-split');
  for (const key of ['source', 'revision', 'sha256']) assert.equal(split.load[key], beforeSplit.load[key]);
  assert.deepEqual(split.camera, beforeSplit.camera, 'Split keeps the shared 3D pose');
  const splitDiff = pixelDiff(beforeSplit.pixels.rgba, split.pixels.rgba);
  assert.ok(splitDiff.fraction <= 0.001 && splitDiff.mean <= 0.3, `Split pixel difference ${JSON.stringify(splitDiff)}`);
  record('Split uses the same Studio workbench, model, materials and camera', { diff: splitDiff });
  await page.locator('.viewer-tabs [data-on-click="home-view"][data-value="3d"]').click();
  await waitOriginal('raster');
  await sizing.evaluate(el => el.remove());
  const handle = page.getByRole('separator', { name: 'Resize viewer and project details' });
  await handle.focus();
  const priorWidth = (await page.locator('#home-live-viewer').boundingBox()).width;
  await handle.press('ArrowLeft'); await settle();
  assert.ok((await page.locator('#home-live-viewer').boundingBox()).width < priorWidth, 'home separator keyboard resizes the real viewer');
  await handle.press('Enter'); await settle();
  record('home separator keyboard resizes and resets the production layout');
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 900 }); await settle();
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 2));
    await page.screenshot({ path: join(output, `home-parity-${width}.png`) });
  }
  await page.setViewportSize({ width: 1360, height: 900 });
  await page.locator('[data-on-click="select"][data-value="t1"]').first().click();
  await page.locator('#solid-workbench[data-scope="t1"]').waitFor();
  await waitOriginal('raster');
}
