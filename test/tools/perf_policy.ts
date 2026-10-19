// The caller may add coverage requirements, but cannot remove these surfaces.
export const nativeCoverage = ['long-history','timeline','subagents','rooms','bots','status'] as const;
export const featureRegions:Readonly<Record<string,string>> = {
  'long-history':'timeline',timeline:'timeline',subagents:'subagents',rooms:'rooms',bots:'bots',status:'status',
  mail:'mailroom',links:'timeline',
};
export function validFeatureWitness(features:unknown,region:string):features is string[] {
  return Array.isArray(features) && features.length>0 && new Set(features).size===features.length
    && features.every(f=>typeof f==='string' && featureRegions[f]===region);
}
