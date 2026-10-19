// Render editable screenshot composition and review it at three presentation scales.
import { chromium } from '/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { readFile, writeFile } from 'node:fs/promises';

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(here, '..');
const scales = process.env.README_COMPARE === '1' ? [150, 175, 200] : [200];
const browser = await chromium.launch({ executablePath: '/usr/bin/google-chrome-stable', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 2560, height: 2200 }, deviceScaleFactor: 1 });
  for (const scale of [100, ...scales]) {
    await page.goto(`file://${here}/viewers.html?scale=${scale}`);
    await page.locator('img').evaluateAll(images => Promise.all(images.map(i => i.decode())));
    await page.locator('#composition').screenshot({ path: `${out}/royalblue-viewers${scale === 100 ? '' : `-${scale}`}.png` });
    console.log(`Rendered ${scale}%`);
  }
  await page.goto(`file://${here}/agent-setup.html`);
  await page.locator('img').evaluateAll(images => Promise.all(images.map(i => i.decode())));
  await page.locator('#setup').screenshot({ path: `${out}/agent-setup.png` });
  await page.locator('#setup').evaluate(el => el.style.zoom = 2);
  await page.locator('#setup').screenshot({ path: `${out}/agent-setup-200.png` });

  for (const [source, selector, name] of [
    ['references.html', '#references', 'reference-menus-200.png'],
    ['mailroom.html', '#mailroom', 'mailroom-200.png'],
  ]) {
    await page.goto(`file://${here}/${source}`);
    await page.locator('img').evaluateAll(images => Promise.all(images.map(i => i.decode())));
    await page.locator(selector).screenshot({ path: `${out}/${name}` });
  }

  // Run with Bun: use its Markdown renderer so each README remains the
  // source for paragraphs, links, lists, code, headings and centered images.
  for (const [file, lang, suffix] of [['README.md', 'en', ''], ['README.jp.md', 'ja', '-jp']]) {
  let md = await readFile(resolve(here, `../../../../${file}`), 'utf8');
  let body = Bun.markdown.html(md);
  body = body.replaceAll('src="docs/media/readme/', 'src="../');
  body = body.replace(/href="(?!https?:|mailto:|#|\/)([^"]+)"/g, 'href="../../../../$1"');
  const html = `<!doctype html><html lang="${lang}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${file} preview</title><style>
  *{box-sizing:border-box}body{margin:0;background:#0d1117;color:#e6edf3;font:16px/1.5 system-ui,sans-serif}nav{position:sticky;top:0;display:flex;justify-content:center;gap:8px;padding:12px;background:#010409;border-bottom:1px solid #30363d;z-index:1}button{color:inherit;background:#161b22;border:1px solid #30363d;padding:6px 12px;cursor:pointer}button[aria-pressed=true]{border-color:#58a6ff}article{margin:24px auto;padding:24px;width:min(94%,1000px);border:1px solid #30363d}li+li{margin-top:4px}a{color:#58a6ff}p{margin:24px 0}img{max-width:100%;height:auto}pre{background:#161b22;padding:16px;overflow-x:auto;font:13px/1.6 ui-monospace,monospace}@media(max-width:600px){article{padding:12px;margin:12px auto}ol{padding-left:24px}pre{font-size:11px}}
  </style><nav aria-label="Preview scale">${[150,175,200].map(n=>`<button data-scale="${n}" aria-pressed="false">${n}%</button>`).join('')}</nav><article>${body}</article><script>const article=document.querySelector('article');for(const b of document.querySelectorAll('button'))b.onclick=()=>{article.style.zoom=Number(b.dataset.scale)/100;for(const c of document.querySelectorAll('button'))c.setAttribute('aria-pressed',String(c===b));};document.querySelector('button[data-scale="200"]').click();</script></html>`;
  await writeFile(`${here}/preview${suffix}.html`, html);
  await page.setViewportSize({ width: 2100, height: 1400 });
  for (const scale of scales) {
    await page.goto(`file://${here}/preview${suffix}.html`);
    await page.locator(`button[data-scale="${scale}"]`).click();
    await page.locator('img').evaluateAll(images => Promise.all(images.map(i => i.decode())));
    await page.screenshot({ path: `${out}/readme${suffix}-${scale}.png`, fullPage: true });
    const broken = await page.locator('img').evaluateAll(images => images.filter(i => !i.complete || !i.naturalWidth).map(i => i.src));
    if (broken.length) throw new Error(`Broken images in ${file}: ${broken.join(', ')}`);
  }
  }
} finally { await browser.close(); }
