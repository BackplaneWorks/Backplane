// Synchronous RFC 1951 raw DEFLATE stream decoder (one instance per WebSocket).
// Keeps the 32 KiB sliding window across sync-flushed frames and enforces a per-frame size cap.

const MAX_WINDOW = 32768;
const WINDOW_MASK = MAX_WINDOW - 1;
const MAX_FRAME_BYTES = 16 << 20;

const LEN_BASE = [
  3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
  35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LEN_EXTRA = [
  0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
  3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE = [
  1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193,
  257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA = [
  0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
  7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
];
const CL_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

interface HuffTree {
  counts: Int32Array;
  symbols: Int32Array;
  maxBits: number;
}

function buildTree(lengths: Uint8Array, count: number): HuffTree {
  const counts = new Int32Array(16);
  let maxBits = 0;
  for (let i = 0; i < count; i++) {
    const l = lengths[i];
    if (l > 0) {
      counts[l]++;
      if (l > maxBits) maxBits = l;
    }
  }
  const offsets = new Int32Array(16);
  let total = 0;
  for (let b = 1; b <= 15; b++) {
    offsets[b] = total;
    total += counts[b];
  }
  const symbols = new Int32Array(total);
  for (let i = 0; i < count; i++) {
    const l = lengths[i];
    if (l > 0) symbols[offsets[l]++] = i;
  }
  return { counts, symbols, maxBits };
}

const FIXED_LIT = (() => {
  const l = new Uint8Array(288);
  l.fill(8, 0, 144);
  l.fill(9, 144, 256);
  l.fill(7, 256, 280);
  l.fill(8, 280, 288);
  return buildTree(l, 288);
})();

const FIXED_DIST = (() => {
  const l = new Uint8Array(32);
  l.fill(5);
  return buildTree(l, 32);
})();

export class RawInflater {
  private win = new Uint8Array(MAX_WINDOW);
  private winPos = 0;

  public feed(src: Uint8Array, maxBytes = MAX_FRAME_BYTES): Uint8Array {
    let bytePos = 0;
    let bitBuf = 0;
    let bitCnt = 0;

    const needBits = (n: number): number => {
      while (bitCnt < n) {
        if (bytePos >= src.length) throw new Error("truncated deflate frame");
        bitBuf |= src[bytePos++] << bitCnt;
        bitCnt += 8;
      }
      const v = bitBuf & ((1 << n) - 1);
      bitBuf >>>= n;
      bitCnt -= n;
      return v;
    };

    const decodeSym = (t: HuffTree): number => {
      let code = 0;
      let first = 0;
      let index = 0;
      for (let len = 1; len <= t.maxBits; len++) {
        code |= needBits(1);
        const c = t.counts[len];
        if (code - c < first) return t.symbols[index + (code - first)];
        index += c;
        first = (first + c) << 1;
        code <<= 1;
      }
      throw new Error("invalid deflate huffman code");
    };

    let out = new Uint8Array(Math.min(Math.max(src.length * 3, 256), maxBytes));
    let outLen = 0;

    const ensureCap = (extra: number): void => {
      const next = outLen + extra;
      if (next > maxBytes) throw new Error("inflated frame exceeds limit");
      if (next <= out.length) return;
      let cap = out.length;
      while (cap < next) cap = Math.min(cap * 2, maxBytes);
      const grown = new Uint8Array(cap);
      grown.set(out.subarray(0, outLen));
      out = grown;
    };

    const emitByte = (b: number): void => {
      ensureCap(1);
      out[outLen++] = b;
      this.win[this.winPos] = b;
      this.winPos = (this.winPos + 1) & WINDOW_MASK;
    };

    while (bytePos < src.length || bitCnt > 0) {
      // Stop at trailing zero-padding bits when all bytes are consumed (sync flush ends byte-aligned).
      if (bytePos >= src.length && bitCnt < 3) break;
      const bfinal = needBits(1);
      const btype = needBits(2);
      if (btype === 0) {
        // Stored block: discard remaining bits in current byte.
        bitBuf = 0;
        bitCnt = 0;
        if (bytePos + 4 > src.length) throw new Error("truncated stored block");
        const len = src[bytePos] | (src[bytePos + 1] << 8);
        const nlen = src[bytePos + 2] | (src[bytePos + 3] << 8);
        bytePos += 4;
        if ((len ^ 0xffff) !== nlen) throw new Error("invalid stored block length");
        if (bytePos + len > src.length) throw new Error("truncated stored payload");
        ensureCap(len);
        for (let i = 0; i < len; i++) {
          const b = src[bytePos++];
          out[outLen++] = b;
          this.win[this.winPos] = b;
          this.winPos = (this.winPos + 1) & WINDOW_MASK;
        }
        // Empty stored block (00 00 ff ff) at end of input marks sync flush boundary.
        if (len === 0 && bytePos >= src.length) break;
      } else if (btype === 1 || btype === 2) {
        let litTree = FIXED_LIT;
        let distTree = FIXED_DIST;
        if (btype === 2) {
          const hlit = needBits(5) + 257;
          const hdist = needBits(5) + 1;
          const hclen = needBits(4) + 4;
          const clLens = new Uint8Array(19);
          for (let i = 0; i < hclen; i++) clLens[CL_ORDER[i]] = needBits(3);
          const clTree = buildTree(clLens, 19);
          const total = hlit + hdist;
          const lens = new Uint8Array(total);
          let i = 0;
          while (i < total) {
            const sym = decodeSym(clTree);
            if (sym < 16) {
              lens[i++] = sym;
            } else if (sym === 16) {
              if (i === 0) throw new Error("invalid repeat in code lengths");
              const rep = needBits(2) + 3;
              const prev = lens[i - 1];
              if (i + rep > total) throw new Error("code length overflow");
              for (let r = 0; r < rep; r++) lens[i++] = prev;
            } else if (sym === 17) {
              const rep = needBits(3) + 3;
              if (i + rep > total) throw new Error("code length overflow");
              i += rep;
            } else if (sym === 18) {
              const rep = needBits(7) + 11;
              if (i + rep > total) throw new Error("code length overflow");
              i += rep;
            } else {
              throw new Error("invalid code length symbol");
            }
          }
          litTree = buildTree(lens, hlit);
          distTree = buildTree(lens.subarray(hlit), hdist);
        }
        while (true) {
          const sym = decodeSym(litTree);
          if (sym < 256) {
            emitByte(sym);
          } else if (sym === 256) {
            break;
          } else {
            const li = sym - 257;
            if (li >= LEN_BASE.length) throw new Error("invalid length symbol");
            const length = LEN_BASE[li] + (LEN_EXTRA[li] ? needBits(LEN_EXTRA[li]) : 0);
            const di = decodeSym(distTree);
            if (di >= DIST_BASE.length) throw new Error("invalid distance symbol");
            const dist = DIST_BASE[di] + (DIST_EXTRA[di] ? needBits(DIST_EXTRA[di]) : 0);
            ensureCap(length);
            let copyPos = (this.winPos - dist) & WINDOW_MASK;
            for (let k = 0; k < length; k++) {
              const b = this.win[copyPos];
              copyPos = (copyPos + 1) & WINDOW_MASK;
              out[outLen++] = b;
              this.win[this.winPos] = b;
              this.winPos = (this.winPos + 1) & WINDOW_MASK;
            }
          }
        }
      } else {
        throw new Error("invalid deflate block type");
      }
      if (bfinal) break;
    }

    return out.subarray(0, outLen);
  }
}
