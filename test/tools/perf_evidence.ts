import {cadenceReport, contentResponses, contentEvidenceErrors, type CadenceEvidence} from './perf_cadence';
import {performanceVerdict} from './perf_gate';
import {streamWindow} from './perf_guard';
import {nativeCoverage,featureRegions} from './perf_policy';

export type CandidateBinding = {
  commit: string; source_sha256: string; binary_sha256: string;
  compiler_sha256: string; fixture_sha256: string; harness_sha256: string;
  host_sha256: string; platform: 'linux-native' | 'ios-device' | 'ios-simulator';
};
export type RawPhase = {
  name: string; start_ms: number; end_ms: number;
  // Actual observations, not the summary's claimed percentile or verdict.
  spans: {name:string; at_ms:number; duration_ms:number}[];
  streams: {id:string; received_ms:number[]}[];
  opens: {expected:string; drawn:string; duration_ms:number}[];
  cadence?: CadenceEvidence;
  content?: CadenceEvidence;
};
export type PerformanceBundle = {
  schema: 1;
  built: CandidateBinding;
  measured: CandidateBinding;
  started_at: number; completed_at: number;
  history_bytes: number;
  device?: {simulator:boolean; identity_sha256:string; model:string; os:string};
  phases: RawPhase[];
};

const hash = (s: unknown) => typeof s === 'string' && /^[a-f0-9]{64}$/.test(s);
const p99 = (xs:number[]) => [...xs].sort((a,b)=>a-b)[Math.ceil(xs.length*.99)-1];
const finiteSamples = (xs:number[], n:number) => xs.length >= n && xs.every(x=>Number.isFinite(x)&&x>=0);
const phaseContent=(p:RawPhase)=>p.name==='continuous-scroll'?p.cadence:p.content;
const responseBudget=(p:RawPhase,e:CadenceEvidence)=>p.name==='input'?16:p.name==='thread-switch'?50:1000/e.refreshHz;
function validPhaseContent(p:RawPhase,e:CadenceEvidence) {
  return !(p.cadence && p.name!=='continuous-scroll') && !(p.content && p.name==='continuous-scroll')
    && contentEvidenceErrors(e).length===0 && e.start>=p.start_ms && e.end<=p.end_ms
    && contentResponses(e.demands,e.drawn,e.presented,responseBudget(p,e)).every(r=>r!==null);
}

export function verifyPerformanceBundle(bundle:PerformanceBundle, expected:CandidateBinding,
    requiredCoverage:string[], now=Date.now()) {
  const errors:string[]=[];
  if(bundle.schema !== 1) errors.push('unsupported evidence schema');
  for(const [key,value] of Object.entries(expected)) {
    if(!value || (key.endsWith('_sha256') && !hash(value))) errors.push(`invalid expected ${key}`);
    if(['commit','source_sha256','binary_sha256','compiler_sha256'].includes(key)
        && bundle.built?.[key as keyof CandidateBinding] !== value) errors.push(`build binding mismatch: ${key}`);
    if(bundle.measured?.[key as keyof CandidateBinding] !== value) errors.push(`measurement binding mismatch: ${key}`);
  }
  if(!/^[a-f0-9]{40}$/.test(expected.commit)) errors.push('candidate must be an exact commit SHA');
  if(!Number.isFinite(bundle.started_at) || !Number.isFinite(bundle.completed_at)
      || bundle.completed_at < bundle.started_at || bundle.completed_at > now
      || now-bundle.started_at>24*3600*1000) errors.push('missing, future or expired run timestamps');
  if(!Number.isFinite(bundle.history_bytes) || bundle.history_bytes<40_000_000) errors.push('long history fixture missing');
  if(expected.platform==='ios-simulator') errors.push('simulator is correctness evidence, not physical iOS performance acceptance');
  if(expected.platform==='ios-device' && (!bundle.device || bundle.device.simulator!==false
      || !hash(bundle.device.identity_sha256) || !bundle.device.model || !bundle.device.os)) errors.push('physical device identity missing');

  const phases = bundle.phases ?? [];
  const names = new Set(phases.map(p=>p.name));
  if(names.size!==phases.length) errors.push('duplicate scenario names');
  for(const name of ['continuous-scroll','input','thread-switch','background'])
    if(!names.has(name)) errors.push(`required scenario missing: ${name}`);
  const coverage=new Set(phases.flatMap(p=>{
    const evidence=phaseContent(p);
    if(!evidence || !validPhaseContent(p,evidence)) return [];
    const budget=responseBudget(p,evidence);
    const responses=contentResponses(evidence.demands,evidence.drawn,evidence.presented,budget);
    const drawn=new Map(evidence.drawn.map(f=>[f.frame,f]));
    return responses.flatMap((r,i)=>r && drawn.get(r.frame)?.region===evidence.demands[i].region
      ? drawn.get(r.frame)!.features : []);
  }));
  for(const feature of new Set([...nativeCoverage,...requiredCoverage])) {
    if(!featureRegions[feature]) errors.push(`unknown feature requirement: ${feature}`);
    if(!coverage.has(feature)) errors.push(`feature scenario missing: ${feature}`);
  }
  const checks=phases.map(phase=>{
    const values=(name:string)=>phase.spans.filter(s=>s.name===name).map(s=>s.duration_ms);
    const frames=values('ui.frame'), input=values('ui.input'), lines=values('hub.agent_line');
    const validSpans=phase.spans.every(s=>Number.isFinite(s.at_ms) && s.at_ms>=phase.start_ms
      && s.at_ms+s.duration_ms<=phase.end_ms && Number.isFinite(s.duration_ms)&&s.duration_ms>=0);
    const streamIds=new Set(phase.streams.map(s=>s.id));
    const streams=phase.streams.length===10 && streamIds.size===10
      && phase.streams.every(s=>s.id && s.received_ms.every(Number.isFinite)
        && streamWindow(s.received_ms,phase.start_ms,phase.end_ms).sustained);
    const c:Record<string,boolean|null>={
      'phase >=20 seconds':Number.isFinite(phase.start_ms)&&Number.isFinite(phase.end_ms)&&phase.end_ms-phase.start_ms>=20000,
      'valid complete spans':validSpans,
      'all ten streams sustained':streams,
      'at least 300 frames':finiteSamples(frames,300),
      'frame p99<=8ms':finiteSamples(frames,300)&&p99(frames)<=8,
      'no streaming frame>16ms':finiteSamples(frames,300)&&frames.every(x=>x<=16),
      'agent_line p99<=2ms':finiteSamples(lines,100)&&p99(lines)<=2,
      'supplied content matches phase window/budget':!(phase.content || phase.cadence)
        || !!phaseContent(phase) && validPhaseContent(phase,phaseContent(phase)!),
    };
    if(phase.name==='input' || phase.name==='thread-switch') {
      const evidence=phase.content;
      const count=phase.name==='input'?100:10;
      const budget=phase.name==='input'?16:50;
      const result=evidence && contentResponses(evidence.demands,evidence.drawn,evidence.presented,budget);
      c['exact demanded content within budget']=!!evidence && evidence.demands.length>=count
        && ['xserver-readback','compositor-feedback','device-present'].includes(evidence.boundary)
        && evidence.start>=phase.start_ms && evidence.end<=phase.end_ms
        && contentEvidenceErrors(evidence).length===0 && !!result?.every(t=>t!==null);
    }
    if(phase.name==='input') c['input p99<=16ms']=finiteSamples(input,100)&&p99(input)<=16;
    if(phase.name==='thread-switch') c['ten actual drawn opens<=50ms']=phase.opens.length===10
      && phase.opens.every(o=>o.expected && o.drawn===o.expected && Number.isFinite(o.duration_ms)&&o.duration_ms>=0&&o.duration_ms<=50);
    if(phase.name==='continuous-scroll') {
      const cadence=phase.cadence && cadenceReport(phase.cadence);
      c['content-correlated native cadence']=!!cadence?.accepted;
      c['cadence covers full phase']=!!phase.cadence && Math.abs(phase.cadence.start-phase.start_ms)<1
        && Math.abs(phase.cadence.end-phase.end_ms)<1;
      if(expected.platform==='ios-device') c['physical presented-content boundary']=phase.cadence?.boundary==='device-present';
    }
    return {name:phase.name,checks:c};
  });
  const verdict=performanceVerdict(checks,errors.length===0,false,'bound-evidence');
  return {...verdict,failures:[...errors,...verdict.failures],phases:checks};
}
