// CPU regression through the real store and quality binding; no browser/GPU.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {ComponentStore} from '../../src/web/components.js';
import {ViewerQuality} from '../../src/web/viewer-quality.js';

const source=JSON.parse(readFileSync(process.argv[2], 'utf8'));
assert.equal(source.schema,1);assert(source.components.length>0);
const data={...source,sourceRevision:source.sourceRevision||'saved-board',schematicRevision:source.schematicRevision||'saved-schematic'};
const key='0|saved-project|'+data.source,store=new ComponentStore(key,structuredClone(data));
const sent=[],selections=[];
const quality={components:store,asset:{source:data.source,revision:data.sourceRevision},
  renderer:{setComponents(d){sent.push(d);return Promise.resolve();},selectComponent(id){selections.push(id);}},
  frame:null,materials:null,solder:null};
const sync=()=>ViewerQuality.prototype.syncComponents.call(quality);
const off=store.subscribe(sync);
store.select(data.components[0].id);sync();
assert.equal(sent.length,1);const first=store.data,selected=store.selected;
for(let i=0;i<3;i++)store.setData(key,structuredClone(data));
assert.equal(store.data,first,'identical validated responses retain the existing binding');
assert.equal(sent.length,1,'repeated saved metadata must not rebuild solder during tracing');
assert.equal(store.selected,selected);

// Public stored objects remain mutable. A matching old snapshot must not
// preserve an object that a consumer has changed since the previous read.
const beforeMutation=sent.length;
store.data.components[0].at=[987,654,321];
const held=store.data;
store.setData(key,structuredClone(data));
assert.notEqual(store.data,held,'a mutated retained object must be replaced');
assert.deepEqual(store.data.components[0].at,data.components[0].at);
assert.equal(sent.length,beforeMutation+1,'restoring original metadata rebinds the mutated object');

// Same advertised fingerprint and revisions cannot hide changed source fields.
for(const change of [
  d=>{d.components[0].at=[1,2,3];},
  d=>{d.components[0].models=[{path:'different.step',offset:[0,0,1]}];},
  d=>{d.components[0].pads=[{number:'1',board:[4,5],size:[1,2],shape:'rect'}];},
  d=>{d.components[0].symbol={items:[{shape:{kind:'circle',radius:2}}]};},
  d=>{d.assemblyLimit='new qualification limit';},
]){
  const altered=structuredClone(data);change(altered);const count=sent.length;
  store.setData(key,altered);assert.equal(sent.length,count+1);
  assert.equal(sent.at(-1),altered,'all changed metadata reaches the renderer');
  store.setData(key,structuredClone(data));
}
store.select(selected);store.setData(key,{...structuredClone(data),sourceRevision:'new-board-save'});
assert.equal(store.selected,'');assert.equal(sent.at(-1),null,'new revision withdraws current renderer eligibility');
store.setData(key,structuredClone(data));store.select(selected);
store.setData(key,{...structuredClone(data),schematicRevision:'new-schematic-save'});
assert.equal(store.selected,'','schematic revision still invalidates selection');
store.setData(key,structuredClone(data));store.select(selected);
store.setData('different-view-key',structuredClone(data));assert.equal(store.selected,'');
store.setData(key,{...structuredClone(data),components:[data.components[0],data.components[0]]});
assert(store.error);assert.equal(store.data,null);assert.equal(sent.at(-1),null);
store.setData(key,structuredClone(data));assert.equal(sent.at(-1),store.data,'recovery rebinds source metadata');
store.setData(key,{error:'Read failed'});assert.equal(store.data,null);assert.equal(sent.at(-1),null);
off();console.log('PASS actual saved metadata: identical refresh keeps renderer binding; changed geometry/model/symbol/limits, revisions, key, ambiguity and failure retain invalidation');
