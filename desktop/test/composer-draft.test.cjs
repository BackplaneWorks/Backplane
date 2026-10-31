const {test}=require('node:test'),assert=require('node:assert/strict');
test('a send confirmation clears native text before the next input, without waiting for a render',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');
 const field={value:'first part'},doc={getElementById:()=>field,querySelector:()=>({getAttribute:()=> 'A'})};
 applyComposerDraft(doc,'A','');field.value+='next';assert.equal(field.value,'next');
 applyComposerDraft(doc,'A','next');assert.equal(field.value,'next');
});
test('a consumed prefix reaches the native field while another thread and unchanged caret stay untouched',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');let value='firstnext',writes=0;
 const field={get value(){return value},set value(text){writes++;value=text}},doc={getElementById:()=>field,querySelector:()=>({getAttribute:()=> 'A'})};
 applyComposerDraft(doc,'B','');assert.equal(value,'firstnext');assert.equal(writes,0);
 applyComposerDraft(doc,'A','next');assert.equal(value,'next');assert.equal(writes,1);
 applyComposerDraft(doc,'A','next');assert.equal(writes,1);
 applyComposerDraft({...doc,getElementById:()=>null},'A','');assert.equal(writes,1);
});
