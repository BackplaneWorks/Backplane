export function cbor(b: Uint8Array): any {
  let p = 0; const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const len = (ai: number): number => { if (ai < 24) return ai; if (ai === 24) return b[p++]; if (ai === 25) { const v = dv.getUint16(p); p += 2; return v } if (ai === 26) { const v = dv.getUint32(p); p += 4; return v } const v = Number(dv.getBigUint64(p)); p += 8; return v };
  const item = (): any => { const ib = b[p++]; const mt = ib >> 5, ai = ib & 31;
    switch (mt) { case 0: return len(ai); case 1: return -1 - len(ai);
      case 2: { const n = len(ai); const r = b.slice(p, p + n); p += n; return r } case 3: { const n = len(ai); const s = new TextDecoder().decode(b.subarray(p, p + n)); p += n; return s }
      case 4: { const n = len(ai); const a = []; for (let i = 0; i < n; i++) a.push(item()); return a } case 5: { const n = len(ai); const o: any = {}; for (let i = 0; i < n; i++) { const k = item(); o[k] = item() } return o }
      case 6: { len(ai); return item() }
      default: if (ai === 20) return false; if (ai === 21) return true; if (ai === 22 || ai === 23) return null; if (ai === 25) { p += 2; return 0 } if (ai === 26) { const v = dv.getFloat32(p); p += 4; return v } if (ai === 27) { const v = dv.getFloat64(p); p += 8; return v } return null } };
  return item();
}
