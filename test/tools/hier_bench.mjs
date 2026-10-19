// bend src/core/hier.bend -o build/hier.mjs
// node test/tools/hier_bench.mjs build/hier.mjs [max rows, default 1024]
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";
const { default: H } = await import(pathToFileURL(resolve(process.argv[2])));
const max = Number(process.argv[3] || 1024);
const out = [];
for (const n of [4, 8, 12, 16, 20, 40, 64, 128, 256, 512, 1024].filter(n=>n<=max)) {
  const rows = Array.from({length:n},(_,i)=>`${i%4}\t${i%17===0?1:0}\tSheet ${i} 日本語\t/p/sheet${i}.kicad_sch`);
  const input=rows.join("\n");
  const t=performance.now();const rs=H["Hier.read"](input);const ms=performance.now()-t;
  const actual=H["Hier.show"](rs);
  if(actual!==input) throw new Error(`round trip failed at ${n} rows`);
  out.push({rows:n,ms});
}
const blank="\n\n0\t0\tRoot\t/p/root\n\n1\t1\tChild\t/p/root\n\n";
if(H["Hier.show"](H["Hier.read"](blank))!=="0\t0\tRoot\t/p/root\n1\t1\tChild\t/p/root") throw new Error("empty rows changed");
console.log(JSON.stringify({timings:out,emptyRows:"pass"},null,2));
