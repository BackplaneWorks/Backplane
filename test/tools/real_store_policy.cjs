// Resource/readiness assertions shared by the private copied-store runner and
// cheap CI regressions. All times are monotonic; RSS samples are unforced.
function verdict(samples, readyMs, durationMs) {
  const start=Math.max(300000,readyMs+60000),steady=samples.filter(s=>s.elapsedMs>=start);
  const memoryOk=steady.length>=60&&steady.at(-1).elapsedMs-steady[0].elapsedMs>=900000;
  let slopeKiBPerHour=null;
  if(memoryOk){
    const x0=steady[0].elapsedMs, xs=steady.map(s=>(s.elapsedMs-x0)/3600000);
    const meanX=xs.reduce((a,b)=>a+b,0)/xs.length,meanY=steady.reduce((a,b)=>a+b.VmRSS,0)/steady.length;
    slopeKiBPerHour=steady.reduce((n,s,i)=>n+(xs[i]-meanX)*(s.VmRSS-meanY),0)/xs.reduce((n,x)=>n+(x-meanX)**2,0);
  }
  const checks={usableWithin15s:Number.isFinite(readyMs)&&readyMs<=15000,soaked30Minutes:durationMs>=1800000,
    completeUnforcedSamples:memoryOk&&samples.every((s,i)=>Number.isFinite(s.VmRSS)&&s.VmRSS>0&&s.VmSwap===0&&(!i||s.elapsedMs-samples[i-1].elapsedMs<=15000)),
    steadyUnder1GiB:memoryOk&&steady.every(s=>s.VmRSS<=1048576),boundedGrowth:memoryOk&&slopeKiBPerHour<=65536&&steady.at(-1).VmRSS-steady[0].VmRSS<=65536};
  return{checks,ok:Object.values(checks).every(Boolean),slopeKiBPerHour,steadySamples:steady.length,steadyStartKiB:steady[0]?.VmRSS,steadyEndKiB:steady.at(-1)?.VmRSS,peakRssKiB:Math.max(0,...samples.map(s=>s.VmRSS)),peakHwmKiB:Math.max(0,...samples.map(s=>s.VmHWM))};
}
module.exports={verdict};
