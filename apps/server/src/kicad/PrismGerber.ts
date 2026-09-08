// @effect-diagnostics nodeBuiltinImport:off
import * as NodeChildProcess from "node:child_process";
import * as NodeCrypto from "node:crypto";

import { prismGerberPython } from "./vendor/prismGerber.ts";

const renderCommand = `${prismGerberPython}
import sys
payload = json.load(sys.stdin)
layer = parse_excellon(payload["content"]) if payload["drill"] else parse_gerber(payload["content"])
bounds = _layer_bounds(layer)
if bounds is None:
    raise ValueError("This layer contains no drawable geometry")
x0, y0, x1, y1 = bounds
y0, y1 = -y1, -y0
margin = max(x1 - x0, y1 - y0, 1) * 0.025
print(render_layer_svg(layer, (x0-margin, y0-margin, x1+margin, y1+margin)))
`;

export function resolvePrismPython(env: NodeJS.ProcessEnv = process.env): string {
  return env.BACKPLANE_PYTHON?.trim() || "python3";
}

const compositeRenderCommand = `${prismGerberPython}
import sys
import re
payload = json.load(sys.stdin)
layers = []
for item in payload["layers"]:
    parsed = parse_excellon(item["content"]) if item["drill"] else parse_gerber(item["content"])
    if _layer_bounds(parsed) is not None:
        layers.append((parsed, item["colour"], item["opacity"]))
if not layers:
    raise ValueError("These layers contain no drawable geometry")
boxes = [_layer_bounds(layer) for layer, _, _ in layers]
x0 = min(box[0] for box in boxes)
y0 = min(box[1] for box in boxes)
x1 = max(box[2] for box in boxes)
y1 = max(box[3] for box in boxes)
y0, y1 = -y1, -y0
margin = max(x1 - x0, y1 - y0, 1) * 0.025
bounds = (x0-margin, y0-margin, x1+margin, y1+margin)
svgs = [render_layer_svg(layer, bounds, colour=colour) for layer, colour, _ in layers]
outer = svgs[0]
view_end = outer.find(">")
body = outer[view_end + 1:outer.rfind("</svg>")]
def namespace_svg(svg, prefix):
    start = svg.find(">") + 1
    extra = svg[start:svg.rfind("</svg>")]
    ids = re.findall(r'id="([^"]+)"', extra)
    for ident in ids:
        replacement = prefix + ident
        extra = extra.replace('id="' + ident + '"', 'id="' + replacement + '"')
        extra = extra.replace("url(#" + ident + ")", "url(#" + replacement + ")")
        extra = extra.replace('href="#' + ident + '"', 'href="#' + replacement + '"')
    return extra
body = namespace_svg(outer, "layer0-")
background = re.match(r"<rect[^>]*/>", body)
if background:
    body = body[:background.end()] + '<g opacity="' + str(payload["layers"][0]["opacity"]) + '">' + body[background.end():] + '</g>'
for index, svg in enumerate(svgs[1:], 1):
    extra = namespace_svg(svg, "layer" + str(index) + "-")
    match = re.match(r"<rect[^>]*/>", extra)
    if match:
        extra = extra[match.end():]
    body += '<g opacity="' + str(payload["layers"][index]["opacity"]) + '">' + extra + '</g>'
print(outer[:view_end + 1] + body + "</svg>")
`;

const MAX_RENDER_CACHE_ENTRIES = 128;
const MAX_RENDER_CACHE_BYTES = 64 * 1024 * 1024;
const renderCache = new Map<string, { svg: string; bytes: number }>();
const renderInflight = new Map<string, Promise<string>>();
const MAX_RENDER_PROCESSES = 4;
const MAX_RENDER_QUEUE = 16;
let activeRenderProcesses = 0;
const renderQueue: Array<() => void> = [];
let renderCacheBytes = 0;

function runWithRenderLimit<T>(task: () => Promise<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    const start = () => {
      activeRenderProcesses += 1;
      void task().then(
        (value) => {
          activeRenderProcesses -= 1;
          renderQueue.shift()?.();
          resolve(value);
        },
        (error) => {
          activeRenderProcesses -= 1;
          renderQueue.shift()?.();
          reject(error);
        },
      );
    };
    if (activeRenderProcesses < MAX_RENDER_PROCESSES) start();
    else if (renderQueue.length >= MAX_RENDER_QUEUE) {
      reject(new Error("Too many Gerber renders are queued; try again shortly"));
    } else renderQueue.push(start);
  });
}

function renderCacheKey(content: string, filename: string): string {
  return `${filename.toLowerCase()}\0${NodeCrypto.createHash("sha256").update(content).digest("hex")}`;
}

function rememberRender(key: string, svg: string): void {
  const bytes = Buffer.byteLength(svg, "utf8");
  const existing = renderCache.get(key);
  if (existing) renderCacheBytes -= existing.bytes;
  renderCache.delete(key);
  if (bytes > MAX_RENDER_CACHE_BYTES) return;
  renderCache.set(key, { svg, bytes });
  renderCacheBytes += bytes;
  while (renderCache.size > MAX_RENDER_CACHE_ENTRIES || renderCacheBytes > MAX_RENDER_CACHE_BYTES) {
    const oldest = renderCache.keys().next().value;
    if (oldest === undefined) break;
    const removed = renderCache.get(oldest);
    if (removed) renderCacheBytes -= removed.bytes;
    renderCache.delete(oldest);
  }
}

/** Clears the process-local render cache; useful after changing renderer settings. */
export function clearPrismGerberRenderCache(): void {
  renderCache.clear();
  renderInflight.clear();
  renderCacheBytes = 0;
}

/** Runs Prism's original fabrication renderer on a saved snapshot, without opening the source file.
 * Results are content keyed and concurrent requests for the same layer share one Python process. */
export function renderPrismGerber(content: string, filename: string): Promise<string> {
  const key = renderCacheKey(content, filename);
  const cached = renderCache.get(key);
  if (cached) {
    renderCache.delete(key);
    renderCache.set(key, cached);
    return Promise.resolve(cached.svg);
  }
  const running = renderInflight.get(key);
  if (running) return running;
  const promise = runPrismCommand(
    renderCommand,
    { content, drill: /\.(?:drl|xln)$/i.test(filename) },
    key,
  );
  renderInflight.set(key, promise);
  void promise.then(
    () => renderInflight.delete(key),
    () => renderInflight.delete(key),
  );
  return promise;
}

function runPrismCommand(command: string, payload: unknown, key: string): Promise<string> {
  return runWithRenderLimit(
    () =>
      new Promise<string>((resolve, reject) => {
        const child = NodeChildProcess.execFile(
          resolvePrismPython(),
          ["-c", command],
          {
            timeout: 20_000,
            maxBuffer: 16 * 1024 * 1024,
            windowsHide: true,
          },
          (error, stdout, stderr) => {
            if (error) {
              reject(
                new Error(
                  `Prism could not render this layer: ${stderr.trim().split("\n").at(-1) || error.message}`,
                ),
              );
            } else {
              rememberRender(key, stdout);
              resolve(stdout);
            }
          },
        );
        child.stdin?.on("error", () => {});
        child.stdin?.end(JSON.stringify(payload));
      }),
  );
}

export type PrismGerberCompositeLayer = {
  content: string;
  filename: string;
  colour?: string;
};

function compositeStyle(filename: string): { colour: string; opacity: number; rank: number } {
  const name = filename.toLowerCase();
  if (/(?:\.gtl$|f[_\-.]?cu)/.test(name)) return { colour: "#39d353", opacity: 0.65, rank: 10 };
  if (/(?:\.gbl$|b[_\-.]?cu)/.test(name)) return { colour: "#f85149", opacity: 0.65, rank: 11 };
  if (/\.g\d+$/.test(name) || /in(?:ner|ternal)/.test(name))
    return { colour: "#58a6ff", opacity: 0.6, rank: 12 };
  if (/(?:\.gto$|\.gbo$|silk|fab)/.test(name)) return { colour: "#f0f6fc", opacity: 1, rank: 40 };
  if (/(?:\.gtp$|\.gbp$|paste)/.test(name)) return { colour: "#d29922", opacity: 0.9, rank: 30 };
  if (/(?:\.gts$|\.gbs$|mask)/.test(name)) return { colour: "#238636", opacity: 0.35, rank: 20 };
  if (/(?:\.gm\d+$|\.gko$|edge|outline)/.test(name))
    return { colour: "#f2cc60", opacity: 1, rank: 50 };
  if (/(?:\.drl$|\.xln$|drill)/.test(name)) return { colour: "#00d4ff", opacity: 1, rank: 60 };
  return { colour: "#8b949e", opacity: 0.45, rank: 70 };
}

export function renderPrismGerberComposite(
  layers: readonly PrismGerberCompositeLayer[],
): Promise<string> {
  const payloadLayers = layers
    .map((layer) => ({
      content: layer.content,
      drill: /\.(?:drl|xln)$/i.test(layer.filename),
      ...compositeStyle(layer.filename),
      ...(layer.colour ? { colour: layer.colour } : {}),
    }))
    .sort((a, b) => a.rank - b.rank);
  const key = `composite\0${NodeCrypto.createHash("sha256").update(JSON.stringify(payloadLayers)).digest("hex")}`;
  const cached = renderCache.get(key);
  if (cached) {
    renderCache.delete(key);
    renderCache.set(key, cached);
    return Promise.resolve(cached.svg);
  }
  const running = renderInflight.get(key);
  if (running) return running;
  const promise = runPrismCommand(compositeRenderCommand, { layers: payloadLayers }, key);
  renderInflight.set(key, promise);
  void promise.then(
    () => renderInflight.delete(key),
    () => renderInflight.delete(key),
  );
  return promise;
}
