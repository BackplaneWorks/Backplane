'use strict';
// src/web/phone-other.js: the phone's terminal keys and search by title
const {test}=require('node:test'),assert=require('node:assert/strict');
const load=()=>import('../../src/web/phone-other.js');

test('a key of the row goes to the terminal as the keyboard would send it',async()=>{
  const {termKeyValue,TERM_KEYS}=await load();
  const field={getAttribute:k=>k==='data-for'?'':null},owned={getAttribute:k=>k==='data-for'?'t1':null};
  assert.equal(termKeyValue(field,'Escape',0),'Escape\t0');
  assert.equal(termKeyValue(owned,'ArrowUp',0),'t1\tArrowUp\t0');
  assert.equal(termKeyValue(field,'c',4),'c\t4');
  for (const k of ['Escape','Tab','ArrowUp','ArrowDown','ArrowLeft','ArrowRight']) assert.ok(TERM_KEYS.some(x=>x.key===k),k);
  assert.ok(TERM_KEYS.some(x=>x.ctrl),'a sticky Ctrl');
});

test('with Ctrl held, the first character typed is the chord and the rest is typed',async()=>{
  const {ctrlChord}=await load();
  assert.deepEqual(ctrlChord('C'),{key:'c',rest:''});
  assert.deepEqual(ctrlChord('dls'),{key:'d',rest:'ls'});
  assert.equal(ctrlChord(''),null);
});

test('search by title: every word of the query in a sidebar row, each thread once',async()=>{
  const {titleHits}=await load();
  const row=(id,title,project)=>({getAttribute:()=>id,querySelector:()=>({textContent:title}),
    closest:()=>({querySelector:()=>({querySelector:()=>({textContent:project})})})});
  const rows=[row('a','I2C pull-ups','SENSOR'),row('b','Bias the tube stage','AMP'),row('a','I2C pull-ups','SENSOR'),row('c','Power tree pull review','MULTI')];
  const doc={querySelectorAll:()=>rows};
  assert.deepEqual(titleHits(doc,'pull-ups').map(h=>h.id),['a']);
  assert.deepEqual(titleHits(doc,'PULL').map(h=>h.id),['a','c']);
  assert.deepEqual(titleHits(doc,'tube bias'),[{id:'b',title:'Bias the tube stage',project:'AMP'}]);
  assert.deepEqual(titleHits(doc,'  '),[]);
  assert.equal(titleHits(doc,'pull',1).length,1);
});

test('a phone\'s back closes the overlay on top first: search, Devices, palette, terminal',async()=>{
  const {phoneOverlay}=await load();
  const sent=[],clicked=[];
  const doc=(present)=>({querySelector:sel=>{const hit=present.find(p=>sel.includes(p));return hit?{click:()=>clicked.push(hit)}:null;}});
  assert.equal(phoneOverlay(doc([]),()=>{}),null);
  const both=phoneOverlay(doc(['#thin-result','.floating-terminal']),(a,v)=>sent.push(a));
  assert.equal(both.name,'search');both.close();assert.deepEqual(clicked,['#thin-result']);
  const t=phoneOverlay(doc(['.floating-terminal']),(a)=>sent.push(a));t.close();
  const pal=phoneOverlay(doc(['.workspace-palette-backdrop']),(a)=>sent.push(a));pal.close();
  assert.deepEqual(sent,['ws-term-hide','ws-palette-close']);
});
