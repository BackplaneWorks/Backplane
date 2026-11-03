import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
const {restoreWindowProjection}=await import('data:text/javascript;base64,'+Buffer.from(readFileSync(new URL('../../src/web/conversation-frame.js',import.meta.url))).toString('base64'));
const node=(key,kids=[],attrs=[],tag='div')=>({type:'element',tag,key,attrs,kids});
const slot=()=>node('workspace-root',[],[{name:'data-window-region',value:'root'}]);
test('restores exact canonical ancestors and canvas before painting',()=>{
 const canvas=node('solid',[],[],'canvas'),viewer=node('viewer',[node('solid-workbench',[canvas])],[],'aside');
 const root=node('workspace-root',[node('app',[node('thread',[],[],'main'),viewer],[{name:'class',value:'app panel picked light top'},{name:'data-bub-home',value:'exact snapshot'}])]);
 assert.strictEqual(restoreWindowProjection(slot(),{root}),root);assert.strictEqual(root.kids[0].kids[1].kids[0].kids[0],canvas);
});
test('keeps read/source boundaries, canonical folds, continuations and long identities exact',()=>{
 const root=node('workspace-root',[node('timeline',[node('source-'+ 'x'.repeat(300000),[{type:'text',text:'@window:0'}])],[{name:'data-reading-seen',value:'committed-source'}])]);
 assert(JSON.stringify(slot()).length<1000);assert.strictEqual(restoreWindowProjection(slot(),{root}),root);
});
test('rejects missing, malformed or noncanonical root regions',()=>{
 assert.throws(()=>restoreWindowProjection(slot(),{root:null}));
 assert.throws(()=>restoreWindowProjection(node('app'),{root:node('workspace-root')}));
 assert.throws(()=>restoreWindowProjection(slot(),{root:node('viewer')}));
});
