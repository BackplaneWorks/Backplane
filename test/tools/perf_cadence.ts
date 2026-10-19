// Continuous-interaction evidence. All timestamps use one monotonic clock,
// in milliseconds. A completed layout or XFlush is not a presentation ack.
import {validFeatureWitness} from './perf_policy';
export type UpdateDemand = {
  id: string; scheduled: number; sent: number; observed: number;
  // Expected independently by the driver, not copied from a returned frame.
  revision: string; contentHash: string; pixelHash: string; region: string;
  // Independent oracle describes which surface the demanded ROI contains.
  features: string[];
};
export type DrawnContent = {
  frame: number; at: number; revision: string; contentHash: string; pixelHash: string;
  features: string[]; region: string;
};
export type PresentedContent = { frame: number; at: number };
export type CadenceEvidence = {
  boundary: 'xserver-readback' | 'xserver-ack' | 'compositor-feedback' | 'device-present' | 'native-display-link' | 'missing';
  start: number;
  end: number;
  refreshHz: number;
  refresh: {source:'xvfb-reference'|'randr'|'device-target'; hz:number; intervals_ms:number[]};
  demands: UpdateDemand[];
  drawn: DrawnContent[];
  presented: PresentedContent[];
  // Independent framebuffer observations, never inferred from refresh callbacks
  // or copied from the app's claimed revision/content metadata.
  readbacks?: {at:number; region:string; pixelHash:string}[];
};

const percentile = (xs: number[], p: number) => {
  if (!xs.length) return null;
  const sorted = [...xs].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)];
};
const strictlyIncreasing = (xs: number[]) => xs.every((x, i) => Number.isFinite(x) && (!i || x > xs[i - 1]));
function refreshErrors(e:CadenceEvidence) {
  if(!Number.isFinite(e.refreshHz) || e.refreshHz<60) return ['refresh rate must be finite and at least 60 Hz'];
  if(!e.refresh || e.refresh.hz!==e.refreshHz) return ['refresh source missing or mismatched'];
  if(e.refresh.source==='xvfb-reference')
    return e.boundary==='xserver-readback' && e.refreshHz===60?[]:['invalid reference cadence boundary'];
  if(!['randr','device-target'].includes(e.refresh.source) || !Array.isArray(e.refresh.intervals_ms)
      || e.refresh.intervals_ms.length<100 || e.refresh.intervals_ms.some(t=>!Number.isFinite(t)||t<=0)
      || Math.abs(1000/(percentile(e.refresh.intervals_ms,.5)??Infinity)-e.refreshHz)>e.refreshHz*.02)
    return ['declared refresh does not match observed display source'];
  return [];
}

// The producer must derive content/revision from the rendered output, retaining
// it with cached output. Relabelling a cached frame from current app state is
// invalid. Pixel hashes cover the displayed image, not a redraw request.
export function contentResponses(demands:UpdateDemand[], drawn:DrawnContent[], presented:PresentedContent[], budget:number) {
  const frames=new Map(drawn.map(f=>[f.frame,f]));
  // Each reference describes the cumulative expected state after that input
  // prefix. It is produced before measurement by the independent fresh oracle.
  const references=new Map(demands.map((d,i)=>[d.revision,{d,i}]));
  const used=new Set<number>();
  let previousPixels='';
  return demands.map((d,i)=>{
    const ack=presented.find(p=>{
      const f=frames.get(p.frame),ref=f && references.get(f.revision);
      return p.at>=d.observed && p.at<=d.sent+budget && f && ref && ref.i>=i
        && f.at>=ref.d.observed && f.at<=p.at && f.contentHash===ref.d.contentHash
        && f.pixelHash===ref.d.pixelHash && f.region===ref.d.region && d.region===f.region
        && JSON.stringify(f.features)===JSON.stringify(ref.d.features)
        && (used.has(p.frame) || f.pixelHash!==previousPixels);
    });
    const f=ack && frames.get(ack.frame)!;
    if(!ack || !f) return null;
    used.add(ack.frame); previousPixels=f.pixelHash;
    return {duration_ms:ack.at-d.sent,frame:ack.frame};
  });
}

export function contentEvidenceErrors(e:CadenceEvidence) {
  const invalid: string[] = refreshErrors(e);
  if (!['xserver-readback', 'compositor-feedback', 'device-present'].includes(e.boundary)) invalid.push('content presentation boundary missing; callbacks alone are insufficient');
  const hash = (x: string) => typeof x === 'string' && /^[a-f0-9]{64}$/.test(x);
  const frames = new Map(e.drawn.map(f => [f.frame, f]));
  if (frames.size !== e.drawn.length || e.drawn.some(f => !Number.isSafeInteger(f.frame) || f.frame < 1
      || !Number.isFinite(f.at) || f.at<e.start || f.at>e.end || !Array.isArray(f.features) || f.features.some(x=>typeof x!=='string') || !f.region || !f.revision || !hash(f.contentHash) || !hash(f.pixelHash))) invalid.push('invalid or duplicate drawn frame');
  if (new Set(e.presented.map(p => p.frame)).size !== e.presented.length) invalid.push('duplicate presented frame');
  if (new Set(e.demands.map(d => d.id)).size !== e.demands.length
      || new Set(e.demands.map(d=>d.revision)).size !== e.demands.length
      || e.demands.some(d => !d.id || !d.region || !d.revision || !hash(d.contentHash) || !hash(d.pixelHash)
        || !validFeatureWitness(d.features,d.region))) invalid.push('invalid or duplicate content demand');
  const references=new Map(e.demands.map(d=>[d.revision,d]));
  if(e.drawn.some(f=>{
    const d=references.get(f.revision);
    return !validFeatureWitness(f.features,f.region) || !d || d.region!==f.region
      || JSON.stringify(d.features)!==JSON.stringify(f.features);
  })) invalid.push('drawn feature labels differ from demanded surface');
  if (e.presented.some(p => !frames.has(p.frame) || frames.get(p.frame)!.at > p.at)) invalid.push('ack lacks preceding drawn content');
  if (!Number.isFinite(e.start) || !Number.isFinite(e.end) || e.end<=e.start) invalid.push('invalid measured window');
  if (!strictlyIncreasing(e.presented.map(p=>p.at))) invalid.push('invalid presentation order');
  if (!strictlyIncreasing(e.demands.map(d=>d.scheduled))) invalid.push('invalid demand order');
  if (e.presented.some(p=>p.at<e.start || p.at>e.end)) invalid.push('ack outside measured window');
  if (e.demands.some(d=>!Number.isFinite(d.sent) || d.sent<d.scheduled || d.sent>e.end || d.scheduled<e.start
      || !Number.isFinite(d.observed) || d.observed<d.sent || d.observed>e.end)) invalid.push('invalid dispatched/observed demand');
  const pixels=e.readbacks ?? [];
  if(!pixels.length) invalid.push('independent pixel readback missing');
  if(pixels.some(p=>!Number.isFinite(p.at) || p.at<e.start || p.at>e.end || !p.region || !hash(p.pixelHash)))
    invalid.push('invalid pixel readback');
  const regions=new Set(pixels.map(p=>p.region));
  for(const region of regions) if(!strictlyIncreasing(pixels.filter(p=>p.region===region).map(p=>p.at)))
    invalid.push('invalid pixel readback order');
  if(e.presented.some(p=>{
    const f=frames.get(p.frame);
    return !f || !pixels.some(x=>x.at>=f.at && x.at<=p.at && x.region===f.region && x.pixelHash===f.pixelHash);
  })) invalid.push('presented content lacks matching independent pixels');
  return invalid;
}

export function cadenceReport(e: CadenceEvidence) {
  const period = 1000 / e.refreshHz;
  const duration = e.end - e.start;
  const invalid=contentEvidenceErrors(e);
  const times=e.presented.map(p=>p.at);
  if (!Number.isFinite(duration) || duration < 10000) invalid.push('continuous interaction shorter than 10 seconds');
  if (!strictlyIncreasing(times)) invalid.push('presentation timestamps invalid or not strictly increasing');
  if (!strictlyIncreasing(e.demands.map(d => d.scheduled))) invalid.push('demand timestamps invalid or not strictly increasing');
  if (e.demands.some(d => !Number.isFinite(d.sent) || d.sent < d.scheduled)) invalid.push('input dispatch timestamps invalid');
  if (times.some(t => t < e.start || t > e.end)) invalid.push('presentation outside measured window');
  if (e.demands.some(d => d.scheduled < e.start || d.scheduled + period > e.end + 1e-7)) invalid.push('demand deadline outside measured window');
  if (e.demands.length < 300) invalid.push('fewer than 300 update demands');
  if (e.presented.length < 300) invalid.push('fewer than 300 observed presentations');

  // Include idle gaps at both edges: a burst at startup cannot conceal a
  // frozen app for the rest of the measurement. Retain each raw interval.
  const gaps: number[] = [];
  let previous = e.start;
  for (const t of times) { gaps.push(t - previous); previous = t; }
  gaps.push(e.end - previous);
  const dispatch = e.demands.map(d => d.sent - d.scheduled);
  const demandGaps = e.demands.slice(1).map((d, i) => d.scheduled - e.demands[i].scheduled);
  if (!e.demands.length || e.demands[0].scheduled - e.start > period
      || e.end - (e.demands.at(-1)!.scheduled + period) > period
      || demandGaps.some(gap => gap > period * 1.01)) invalid.push('continuous input demand has gaps');
  if (dispatch.some(delay => delay >= period)) invalid.push('input driver missed a refresh deadline');

  // Only the frame containing this demand's actual drawn revision can
  // satisfy it. A fresh timestamp/refresh counter over stale pixels cannot.
  // A frame is credited once; bursts never repay earlier missed deadlines.
  const response=contentResponses(e.demands,e.drawn,e.presented,period);
  const missed=response.filter(t=>t===null).length;
  const checks = {
    'cadence workload valid': invalid.length === 0,
    'every demanded update meets refresh deadline': missed === 0 && e.demands.length > 0,
  };
  return {
    boundary: e.boundary,
    content_correlated: true,
    // X server acknowledgement does not establish compositor scanout.
    physical_scanout_verified: false,
    compositor_feedback_verified: e.boundary === 'compositor-feedback',
    cadence_scope: e.refresh?.source==='xvfb-reference'?'60Hz reference X-server pixel readback; compositor/scanout unverified':e.boundary,
    duration_ms: duration, refresh_hz: e.refreshHz, samples: e.presented.length,
    demands: e.demands.length,
    credited_content_frames: new Set(response.filter(t=>t!==null).map(t=>t!.frame)).size,
    fps: new Set(response.filter(t=>t!==null).map(t=>t!.frame)).size * 1000 / duration,
    gap_p50_ms: percentile(gaps, .5), gap_p99_ms: percentile(gaps, .99),
    longest_stall_ms: Math.max(...gaps), missed_refresh_deadlines: missed,
    dispatch_p99_ms: percentile(dispatch, .99), gaps_ms: gaps,
    demand_to_ack_ms: response.map(t=>t?.duration_ms??null), invalid, checks,
    accepted: Object.values(checks).every(Boolean),
  };
}
