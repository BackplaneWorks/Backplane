// bend test/tools/history_render.bend -o build/history-render.mjs
// node test/tools/history_render.mjs build/history-render.mjs [baseline.json]
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
const { default: B } = await import(pathToFileURL(resolve(process.argv[2])));
const baseline = process.argv[3] && JSON.parse(readFileSync(process.argv[3], "utf8"));
const encode = v => JSON.stringify(v, (_, x) => typeof x === "bigint" ? x.toString() : x);
// Iterative traversal: a long list of ticks must not overflow the test
// runner's JSON serializer stack. Preserve all keys, text and leaf values.
const hash = x => {
  const h = createHash("sha256"), todo = [x];
  while (todo.length) {
    const v = todo.pop();
    if (v && typeof v === "object") {
      const keys = Object.keys(v);
      h.update(Array.isArray(v) ? "array" : "object");
      h.update(JSON.stringify(keys));
      for (let i = keys.length - 1; i >= 0; i--) todo.push(v[keys[i]]);
    } else {
      const leaf = encode(v) ?? "undefined";
      h.update(`${typeof v}:${leaf.length}:${leaf}`);
    }
  }
  return h.digest("hex");
};
const results = [];
const samples = [];
for (const n of [0, 3, 64, 256, 1024]) {
  const u = B.make(BigInt(n));
  assert.equal(B.bar(u, {$: "En"}).n, BigInt(n), `fixture track ${n}`);
  for (const language of ["En", "Ja"]) {
    const l = {$: language};
    for (const variant of ["live", "hist-first", "hist-last", "hist-diff", "hist-play", "compare", "schematic", "panel"]) {
      const state = variant === "live" ? u : B.variant(u, variant);
      for (const client of ["bar", "web", "phone", "webmenu"]) {
        const value = B[client](state, l);
        const id = `${n}/${language}/${variant}/${client}`;
        const digest = hash(value);
        if (baseline) assert.equal(digest, baseline.results.find(x=>x.id===id)?.sha256, id);
        results.push({id, sha256:digest});
      }
    }
    for (const client of ["bar", "web", "phone"]) {
      const times=[];
      for(let i=0;i<7;i++) {const t=performance.now(); B[client](u,l); times.push(performance.now()-t);}
      times.sort((a,b)=>a-b);
      samples.push({steps:n, language, client, medianMs:times[3],p95Ms:times[6]});
    }
  }
}
console.log(JSON.stringify({runtime:process.version,results,samples,baselineCompared:!!baseline},null,2));
