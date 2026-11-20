// Minimal zero-dependency QR Code generator (ISO/IEC 18004, Versions 1-6, Byte mode, ECC Level L).
// Produces a crisp SVG element with a single <path> (strict CSP compliant, no inline styles).

const SVG_NS = "http://www.w3.org/2000/svg";

// Version table for ECC Level L: [version, totalCodewords, ecCodewordsPerBlock, numBlocks, alignCoords]
const VERSIONS: Array<[number, number, number, number, number[]]> = [
  [1, 26, 7, 1, []],
  [2, 44, 10, 1, [6, 18]],
  [3, 70, 15, 1, [6, 22]],
  [4, 100, 20, 1, [6, 26]],
  [5, 134, 26, 1, [6, 30]],
  [6, 172, 18, 2, [6, 34]],
];

const GF_EXP = new Uint8Array(512);
const GF_LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    GF_EXP[i] = x;
    GF_LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) GF_EXP[i] = GF_EXP[i - 255];
}

function gfMul(a: number, b: number): number {
  return a === 0 || b === 0 ? 0 : GF_EXP[GF_LOG[a] + GF_LOG[b]];
}

function rsGenerator(degree: number): Uint8Array {
  let poly = new Uint8Array([1]);
  for (let i = 0; i < degree; i++) {
    const next = new Uint8Array(poly.length + 1);
    const factor = GF_EXP[i];
    for (let j = 0; j < poly.length; j++) {
      next[j] ^= poly[j];
      next[j + 1] ^= gfMul(poly[j], factor);
    }
    poly = next;
  }
  return poly;
}

function rsEncode(data: Uint8Array, ecLen: number): Uint8Array {
  const gen = rsGenerator(ecLen);
  const rem = new Uint8Array(ecLen);
  for (let i = 0; i < data.length; i++) {
    const factor = data[i] ^ rem[0];
    rem.copyWithin(0, 1);
    rem[ecLen - 1] = 0;
    if (factor !== 0) {
      for (let j = 0; j < ecLen; j++) {
        rem[j] ^= gfMul(gen[j + 1], factor);
      }
    }
  }
  return rem;
}

function encodeQrMatrix(text: string): { size: number; modules: Uint8Array } | null {
  const bytes = new TextEncoder().encode(text);
  const spec = VERSIONS.find(([, total, ec, blocks]) => {
    const dataCw = total - ec * blocks;
    // Byte mode header (4 bits) + char count (8 bits for V1-9) + payload + 4-bit terminator
    return Math.ceil((4 + 8 + bytes.length * 8) / 8) <= dataCw;
  });
  if (!spec) return null;

  const [ver, totalCw, ecPerBlock, numBlocks, align] = spec;
  const dataCw = totalCw - ecPerBlock * numBlocks;

  // Build bit buffer: mode 0100 (byte), 8-bit length, payload, terminator, pad bytes
  const bits: number[] = [];
  const pushBits = (val: number, len: number) => {
    for (let i = len - 1; i >= 0; i--) bits.push((val >>> i) & 1);
  };
  pushBits(0b0100, 4);
  pushBits(bytes.length, 8);
  for (const b of bytes) pushBits(b, 8);
  for (let i = 0; i < 4 && bits.length < dataCw * 8; i++) bits.push(0);
  while (bits.length % 8 !== 0) bits.push(0);

  const data = new Uint8Array(dataCw);
  for (let i = 0; i < bits.length / 8; i++) {
    let v = 0;
    for (let j = 0; j < 8; j++) v = (v << 1) | bits[i * 8 + j];
    data[i] = v;
  }
  const pads = [0xec, 0x11];
  for (let i = bits.length / 8, p = 0; i < dataCw; i++, p ^= 1) data[i] = pads[p];

  // Split into blocks & interleave
  const blockLen = dataCw / numBlocks;
  const dataBlocks: Uint8Array[] = [];
  const ecBlocks: Uint8Array[] = [];
  for (let b = 0; b < numBlocks; b++) {
    const slice = data.subarray(b * blockLen, (b + 1) * blockLen);
    dataBlocks.push(slice);
    ecBlocks.push(rsEncode(slice, ecPerBlock));
  }
  const codewords: number[] = [];
  for (let i = 0; i < blockLen; i++) for (const blk of dataBlocks) codewords.push(blk[i]);
  for (let i = 0; i < ecPerBlock; i++) for (const blk of ecBlocks) codewords.push(blk[i]);

  const size = ver * 4 + 17;
  const modules = new Uint8Array(size * size);
  const isFunc = new Uint8Array(size * size);
  const setFunc = (r: number, c: number, dark: boolean) => {
    if (r < 0 || c < 0 || r >= size || c >= size) return;
    modules[r * size + c] = dark ? 1 : 0;
    isFunc[r * size + c] = 1;
  };

  // Finder patterns + separators
  const drawFinder = (r0: number, c0: number) => {
    for (let dr = -1; dr <= 7; dr++) {
      for (let dc = -1; dc <= 7; dc++) {
        const r = r0 + dr;
        const c = c0 + dc;
        const inOuter = dr >= 0 && dr <= 6 && dc >= 0 && dc <= 6;
        const dark =
          inOuter &&
          (dr === 0 || dr === 6 || dc === 0 || dc === 6 || (dr >= 2 && dr <= 4 && dc >= 2 && dc <= 4));
        setFunc(r, c, dark);
      }
    }
  };
  drawFinder(0, 0);
  drawFinder(0, size - 7);
  drawFinder(size - 7, 0);

  // Timing patterns
  for (let i = 8; i < size - 8; i++) {
    setFunc(6, i, i % 2 === 0);
    setFunc(i, 6, i % 2 === 0);
  }

  // Alignment patterns
  for (const r of align) {
    for (const c of align) {
      if (isFunc[r * size + c]) continue;
      for (let dr = -2; dr <= 2; dr++) {
        for (let dc = -2; dc <= 2; dc++) {
          const dark = Math.max(Math.abs(dr), Math.abs(dc)) !== 1;
          setFunc(r + dr, c + dc, dark);
        }
      }
    }
  }

  // Reserve format info + dark module
  for (let i = 0; i <= 8; i++) {
    if (!isFunc[8 * size + i]) setFunc(8, i, false);
    if (!isFunc[i * size + 8]) setFunc(i, 8, false);
  }
  for (let i = size - 8; i < size; i++) {
    setFunc(8, i, false);
    setFunc(i, 8, false);
  }
  setFunc(size - 8, 8, true);

  // Place data bits (right-to-left 2-col zigzag) with mask 0: (r + c) % 2 === 0
  let bitIdx = 0;
  const totalBits = codewords.length * 8;
  for (let right = size - 1; right >= 1; right -= 2) {
    if (right === 6) right = 5;
    const upward = ((right + 1) & 2) === 0;
    for (let step = 0; step < size; step++) {
      const r = upward ? size - 1 - step : step;
      for (let k = 0; k < 2; k++) {
        const c = right - k;
        if (isFunc[r * size + c]) continue;
        let bit = 0;
        if (bitIdx < totalBits) {
          bit = (codewords[bitIdx >>> 3] >>> (7 - (bitIdx & 7))) & 1;
          bitIdx++;
        }
        const masked = ((r + c) & 1) === 0 ? bit ^ 1 : bit;
        modules[r * size + c] = masked;
      }
    }
  }

  // Write format info for ECC Level L (01) + Mask 0 (000) -> 0b01000, BCH + XOR 0x5412 = 0x77c4
  const fmt = 0x77c4;
  for (let i = 0; i <= 5; i++) setFunc(8, i, ((fmt >>> (14 - i)) & 1) === 1);
  setFunc(8, 7, ((fmt >>> 8) & 1) === 1);
  setFunc(8, 8, ((fmt >>> 7) & 1) === 1);
  setFunc(7, 8, ((fmt >>> 6) & 1) === 1);
  for (let i = 0; i <= 5; i++) setFunc(5 - i, 8, ((fmt >>> (5 - i)) & 1) === 1);

  for (let i = 0; i < 8; i++) setFunc(8, size - 1 - i, ((fmt >>> i) & 1) === 1);
  for (let i = 0; i < 7; i++) setFunc(size - 7 + i, 8, ((fmt >>> (14 - i)) & 1) === 1);

  return { size, modules };
}

export function renderQrSvg(text: string): SVGSVGElement | null {
  const qr = encodeQrMatrix(text);
  if (!qr) return null;
  const quiet = 2;
  const full = qr.size + quiet * 2;
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${full} ${full}`);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", `QR code for ${text}`);
  svg.setAttribute("shape-rendering", "crispEdges");
  svg.classList.add("bp-qr-svg");

  const bg = document.createElementNS(SVG_NS, "rect");
  bg.setAttribute("width", String(full));
  bg.setAttribute("height", String(full));
  bg.setAttribute("fill", "#ffffff");
  svg.appendChild(bg);

  let d = "";
  for (let r = 0; r < qr.size; r++) {
    for (let c = 0; c < qr.size; c++) {
      if (qr.modules[r * qr.size + c]) {
        d += `M${c + quiet},${r + quiet}h1v1h-1z`;
      }
    }
  }
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", d);
  path.setAttribute("fill", "#000000");
  svg.appendChild(path);
  return svg;
}
