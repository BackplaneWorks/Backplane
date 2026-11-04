"""Render the short report from retained measurements, with explicit scope."""
import json
from pathlib import Path
import statistics

root=Path('build/bend-vs-ts')
samples=json.loads((root/'runtime/samples.json').read_text())

def median(path,size,mode,key,div=1):
    return statistics.median(s[key]/div for s in samples if (s['path'],s['size'],s['mode'])==(path,size,mode))

def row(label,path,size,div=1):
    def m(mode,key): return median(path,size,mode,key,div)
    def rss(mode): return statistics.median(s['resources']['peak_rss_kib']/1024 for s in samples if (s['path'],s['size'],s['mode'])==(path,size,mode))
    def per(mode,key): return statistics.median(s[key]/s['repeats']/div for s in samples if (s['path'],s['size'],s['mode'])==(path,size,mode))
    return f'| {label} | {m("bend","msPerOp"):.3f} / {m("ts","msPerOp"):.3f} | {m("bend","msPerOp")/m("ts","msPerOp"):.1f}× | {rss("bend"):.0f} / {rss("ts"):.0f} | {per("bend","allocated_bytes_estimate")/1048576:.2f} / {per("ts","allocated_bytes_estimate")/1048576:.2f} | {per("bend","gc_pause_ms"):.3f} / {per("ts","gc_pause_ms"):.3f} |'

rows='\n'.join([row('Status, 500 threads','status',500),row('Status, 1,000 threads','status',1000),row('Broadcast, 1.45M events × 4 views','broadcast',1450000),row('Broadcast, 2.90M events × 4 views','broadcast',2900000),row('History query, 420,004 entries','history',1,16),row('History query, 840,008 entries','history',2,16)])
def ms(path,size,mode): return median(path,size,mode,'msPerOp')
def doubling(path,a,b,mode): return ms(path,b,mode)/ms(path,a,mode)
def span(path,size,mode):
    values=[s['msPerOp']/1000 for s in samples if (s['path'],s['size'],s['mode'])==(path,size,mode)]
    return f'{min(values):.2f}–{max(values):.2f}'
compile_rows=[]
for title,file in [('Whole hub Bend emit (8 vCPU / 16 GiB)','emit.json'),('Bun executable link (4 vCPU / 8 GiB)','runtime/bun-link-hub.resources.json'),('tsc --noEmit, equivalents only','runtime/tsc-noemit.resources.json'),('tsc emit, equivalents only','runtime/tsc-emit.resources.json'),('Bun transpile, equivalents only','runtime/bun-build-ts.resources.json')]:
    d=json.loads((root/file).read_text())
    compile_rows.append(f'| {title} | {d["seconds"]:.3f} | {d["peak_rss_kib"]/1024:.1f} |')
report=f'''# Bend versus TypeScript — measured 2026-11-04 UTC

**Bend offers provable policy and deterministic shared logic; its current JS representation and whole-program compiler impose substantial costs. This supports a hybrid hub, not a wholesale rewrite.**

Source `a4a14b3728237aeb669cf9e74588fa154d410c7f`, Bend 2.0.36, Bun 1.3.10, Ryzen 9700X Boat VMs. Runtime: 4 vCPU / 8 GiB; three fresh processes, four warmups, medians, JIT on. Snapshot: exactly 1,450,000 events, SHA-256 `fd3330c645e5a96f5241425125568d5108e05adee5c15cd226d5e2db2bdb47b9`. Only anonymized projections were uploaded; no live hub ran.

All paired outputs matched: every broadcast routing/pace field; status names/counts; history IDs, bounds, bodies and every renderer text block. Bend is the actual hub emit after today's native replacements, with its bootstrap removed. TypeScript uses ordinary Maps/arrays. Parsing/setup/conversion are outside timing.

| Path / input | Wall ms, Bend / TS | Ratio | Peak RSS MiB, B / TS | Allocated MiB, B / TS* | GC pause ms, B / TS |
|---|---:|---:|---:|---:|---:|
{rows}

*Allocation is a JSC-managed lower-bound estimate (`ca` plus concurrent `a`); native SQLite/malloc is not exhaustively counted. GC excludes the final forced collection. RSS includes the whole process, fixture parsing, JIT and output buffers; it is not retained hub-state size. History is measured in 16-query batches then divided by 16. Zero GC means no observed in-window pause.*

**Attribution.** With the index already built, 1,000 phases took **{ms('phase',1000,'bend'):.3f} ms in Bend, {ms('phase',1000,'ts-trie'):.3f} ms in TypeScript using the same Bend tries, and {ms('phase',1000,'ts'):.3f} ms using native Maps**. This points to representation/library costs, not a universal 15× generated-control-flow penalty. Status index construction also performs more intermediate grouping/relationship work; its full {ms('status',1000,'bend')/ms('status',1000,'ts'):.1f}× gap cannot be assigned solely to codegen. Keeping Bend but using its old scan path costs **{ms('status',1000,'bend-scan'):.2f} versus {ms('status',1000,'bend'):.2f} ms**: an independently measured **{ms('status',1000,'bend-scan')/ms('status',1000,'bend'):.1f}× algorithm difference**. Removing today's native patches costs **{ms('status',1000,'bend-raw'):.2f} versus {ms('status',1000,'bend'):.2f} ms** ({ms('status',1000,'bend-raw')/ms('status',1000,'bend'):.1f}×); the raw emitter is much worse than what ships.

Broadcast uses the same linear touch/pace algorithm: its remaining gap combines strict evaluation, string/list intermediates and generated calls. History uses identical indexed SQL: TS without renderer blocks takes **{ms('history',1,'ts-query')/16:.3f} ms/query**, with them **{ms('history',1,'ts')/16:.3f} ms**, versus Bend's **{ms('history',1,'bend')/16:.3f} ms**. The remaining cost includes building blocks and a temporary persistent index, not a full-log scan.

**Scaling.** Doubling threads took **{doubling('status',500,1000,'bend'):.2f}× / {doubling('status',500,1000,'ts'):.2f}×** (Bend/TS); doubling history **{doubling('history',1,2,'bend'):.2f}× / {doubling('history',1,2,'ts'):.2f}×**. Broadcast's measured doubling was **{doubling('broadcast',1450000,2900000,'bend'):.2f}× / {doubling('broadcast',1450000,2900000,'ts'):.2f}×**, but host noise was substantial (Bend 1.45M: **{span('broadcast',1450000,'bend')} s**; 2.90M: **{span('broadcast',2900000,'bend')} s**). An earlier independent run doubled **2.22× / 2.07×**. There is no evidence here of an O(n²) hot path; precise scaling needs quieter hosts. Neither an invariant language multiplier nor a 100× algorithm claim is justified.

| Compile phase (single run) | Wall s | Peak RSS MiB |
|---|---:|---:|
{chr(10).join(compile_rows)}

The emit alone peaks at **14.45 GiB**; the expensive stage is Bend, not Bun linking. **The TS comparison covers only the equivalents; Bend checks/emits the whole hub import closure and proofs. These are rough references, not an equivalent full-app build speedup.**

**Recommendation.** Keep Bend for small pure policies, protocol decisions, geometry and laws worth proving. Move hub I/O, mutable read-model indexes, history materialization and high-volume routing mechanics to TypeScript. Pass small inputs/results across that boundary; avoid walking whole states through Nat marshalling. Keep event-transition policy in Bend until replay is measured separately. This test measures three kernels, not full replay, UI rendering, network throughput or the economic value of proofs; their absolute hot-path times alone do not explain multi-second UI stalls.

Method/reproduction: [harness](../../../test/perf/bend-vs-ts/README.md). Raw samples and provenance: [RESULTS.json](RESULTS.json). The real snapshot is mostly settled and text is anonymized to ASCII of matching code-point lengths; status is a metadata projection, not the full bot/mail/settings state.

Validation: paired runtime/digest checks passed; TypeScript strict check/emit passed; phase/Unicode branch checks and 15 laws-shape tests passed. Repository-wide checking hits the pre-existing `test/stream_step_proof.bend:30` `fallback_outs` proof mismatch. Full `PROOF.bend` also fails in existing `MailProof.keeps` (336.9 s, 13.53 GiB RSS). No production code or laws changed. Validation alone used 32 GiB after the memory guard refused 16 GiB; benchmark numbers above are from 8/16 GiB VMs.
'''
(root/'REPORT.md').write_text(report)
result={'source_revision':'a4a14b3728237aeb669cf9e74588fa154d410c7f','samples':samples,
        'hardware':json.loads((root/'runtime/hardware.json').read_text()),
        'compile':{p:json.loads((root/p).read_text()) for p in ['emit.json','runtime/bun-link-hub.resources.json','runtime/tsc-noemit.resources.json','runtime/tsc-emit.resources.json','runtime/bun-build-ts.resources.json']},
        'boat_jobs':{}}
for name,job in [('emit','j20261104012226-997392'),('runtime','j20261104014346-1192988'),('earlier_runtime','j20261104013139-1091181')]:
    p=Path.home()/'.local/state/backplane/boat/jobs'/job/'provenance.json'
    if p.exists(): result['boat_jobs'][name]=json.loads(p.read_text())
earlier=Path.home()/'.local/state/backplane/boat/jobs/j20261104013139-1091181/artifacts/runtime/samples.json'
if earlier.exists(): result['earlier_broadcast_samples']=[s for s in json.loads(earlier.read_text()) if s['path']=='broadcast']
result['validation']={'runtime':'all exact-output and digest checks passed','tsc':'strict noEmit and emit passed',
                      'manifest_check':'baseline failure: test/stream_step_proof.bend:30 fallback_outs',
                      'full_proof':'baseline failure: PROOF.bend:12355 MailProof.keeps',
                      'source_unchanged':True}
(root/'RESULTS.json').write_text(json.dumps(result,indent=2)+'\n')
