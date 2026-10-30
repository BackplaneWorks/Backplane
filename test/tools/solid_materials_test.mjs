import assert from 'node:assert/strict';
import {materialEstimate} from '../../src/web/solid-materials.js';
const examples=[
  ['polymer',[0.007,0.008,0.009]],['metal-or-gray',[0.645,0.638,0.572]],
  ['ceramic-or-substrate',[0.119,0.059,0.038]],['mask-or-color',[0.005,0.04,0.01]],
  ['copper-or-gold',[0.7,0.5,0.03]],['silk-or-white',[1,1,1]],
];
for(const [kind,rgb] of examples){
  const m=materialEstimate(...rgb);assert.equal(m.kind,kind);
  assert.equal(m.source,'color-estimate');assert.ok(m.roughness>=0.18&&m.roughness<=1);
  assert.ok(m.metalness>=0&&m.metalness<=1);
}
assert.ok(materialEstimate(...examples[1][1]).metalness>materialEstimate(...examples[2][1]).metalness);
assert.ok(materialEstimate(...examples[1][1]).roughness<materialEstimate(...examples[2][1]).roughness);
console.log('ok real model color estimates separate warm metal endcaps, dark ceramic, polymer, mask, copper and silk; provenance explicit');
